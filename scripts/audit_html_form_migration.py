#!/usr/bin/env python3
"""Audit HTML form migration claims without promoting a renderer.

The migration manifest separates route selection, independently reviewable
capabilities, and final release readiness. This audit accepts conservative
false flags, but fails closed when a positive readiness claim is not backed by
the relevant implementation registry or hashed, structured evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import struct
import subprocess
import sys
import zlib
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any
from urllib.parse import urlsplit


MIGRATION_MANIFEST = PurePosixPath(
    "packages/form-specs/form-migration-status.json"
)
RELEASE_EVIDENCE = PurePosixPath(
    "packages/form-specs/form-release-evidence.json"
)
REFERENCE_MANIFEST = PurePosixPath(
    "packages/form-renderer/references/manifest.json"
)
SOURCE_CATALOG = PurePosixPath(
    "packages/form-renderer/references/source-catalog.json"
)
RENDER_PROVIDER_DIR = PurePosixPath("crates/bir-print/src/html_forms")
COMPONENT_REGISTRY = PurePosixPath(
    "packages/form-renderer/src/forms/registry.ts"
)
SPEC_REGISTRY = PurePosixPath("packages/form-specs/src/index.ts")
RUST_CAPABILITY_MANIFEST = PurePosixPath(
    "packages/form-specs/generated/form-capabilities.json"
)
SHA256 = re.compile(r"^[0-9a-f]{64}$")
GIT_REVISION = re.compile(r"^[0-9a-f]{40}(?:[0-9a-f]{24})?$")
SUPPORTED_PAPER_SIZES = {
    (612.0, 792.0),
    (612.0, 936.0),
    (612.0, 1008.0),
}

# Every capability below is explicit in schema v3. Structural capabilities are
# bound to code registries; release capabilities additionally require evidence.
CAPABILITY_FIELDS = (
    "typed_model",
    "xml_round_trip",
    "formula_evidence",
    "persistence",
    "queue_submission",
    "editor",
    "render_contract",
    "html_component",
    "html_spec",
    "pagination",
    "visual_parity",
    "native_preview",
    "native_print",
    "pdf_export",
    "packaged_offline",
)
PROMOTION_FLAGS = CAPABILITY_FIELDS
BOOLEAN_STATUS_FIELDS = ("release_ready",)
ROUTES = {"disabled", "experimental", "html_only"}
STRUCTURAL_CAPABILITIES = (
    "render_contract",
    "html_component",
    "html_spec",
    "pagination",
)
EVIDENCE_CAPABILITIES = (
    "visual_parity",
    "native_preview",
    "native_print",
    "pdf_export",
    "packaged_offline",
)
PLATFORMS = ("macos", "windows", "linux")
ROLLBACK_CASES = {
    "release_ready_false",
    "kill_switch",
    "missing_assets",
    "renderer_error",
    "late_renderer_error",
    "readiness_timeout",
    "invalid_geometry",
    "rejected_pdf",
    "destination_preserved",
    "no_temp_leaks",
    "draft_unchanged",
}

# Positive evidence is accepted only from producers whose output this audit can
# independently bind to an attested execution. Recomputing pixels proves the
# declared comparison, but it cannot prove that an "actual" PNG came from the
# checked-in Playwright driver instead of a re-encoded reference. All producer
# registries therefore remain empty until their CI/package drivers emit
# verifiable attestations and immutable run transcripts. A hand-authored JSON
# report must never promote these gates.
VISUAL_EVIDENCE_PRODUCER = "playwright-form-parity-v1"
VISUAL_EVIDENCE_PRODUCER_PATH = PurePosixPath(
    "packages/form-renderer/visual/form-parity.spec.ts"
)
TRUSTED_VISUAL_EVIDENCE_PRODUCERS: frozenset[str] = frozenset()
TRUSTED_PLATFORM_EVIDENCE_PRODUCERS: frozenset[str] = frozenset()
TRUSTED_ROLLBACK_EVIDENCE_PRODUCERS: frozenset[str] = frozenset()

# Evidence is bound to the most recent commit that changed this curated source
# set.  Evidence/status/docs are deliberately excluded: a later evidence-only
# commit can still prove the exact renderer source revision that produced it,
# while any committed or uncommitted renderer change invalidates that proof.
CURATED_SOURCE_PATHS = (
    ".github/workflows",
    "Cargo.lock",
    "Cargo.toml",
    "justfile",
    "package-lock.json",
    "package.json",
    "tsconfig.base.json",
    "apps/form-preview",
    "assets/macos",
    "assets/windows",
    "crates/bir-core/Cargo.toml",
    "crates/bir-core/src",
    "crates/bir-desktop",
    "crates/bir-print",
    "entitlements.dev.plist",
    "entitlements.plist",
    "installer.iss",
    "installer.wxs",
    "packages/form-contracts",
    "packages/form-renderer",
    "packages/form-specs/package.json",
    "packages/form-specs/src",
    "packages/form-specs/tsconfig.json",
    "scripts",
)


@dataclass(frozen=True)
class AuditResult:
    errors: tuple[str, ...]
    statuses: tuple[str, ...]

    @property
    def passed(self) -> bool:
        return not self.errors


@dataclass(frozen=True)
class RevisionContext:
    source_revision: str
    dirty_paths: tuple[str, ...] = ()


@dataclass(frozen=True)
class PngRgba:
    width: int
    height: int
    pixels: bytes


def git_revision_context(root: Path) -> RevisionContext:
    """Resolve the curated source revision and its exact worktree state."""

    try:
        source_revision = subprocess.run(
            ["git", "rev-list", "-1", "HEAD", "--", *CURATED_SOURCE_PATHS],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        status = subprocess.run(
            [
                "git",
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "--",
                *CURATED_SOURCE_PATHS,
            ],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError) as error:
        raise RuntimeError(f"cannot inspect Git revision/worktree: {error}") from error
    if not GIT_REVISION.fullmatch(source_revision):
        raise RuntimeError(
            "cannot resolve a canonical commit for the curated renderer source paths"
        )
    return RevisionContext(
        source_revision=source_revision,
        dirty_paths=tuple(line for line in status.splitlines() if line.strip()),
    )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def png_dimensions(path: Path) -> tuple[int, int]:
    with path.open("rb") as stream:
        header = stream.read(24)
    if len(header) != 24 or header[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG file")
    return struct.unpack(">II", header[16:24])


def read_png_rgba(path: Path) -> PngRgba:
    """Decode the evidence PNG subset without trusting its reported metrics.

    Playwright and pngjs emit non-interlaced, 8-bit RGB/RGBA PNGs.  Keeping the
    decoder here makes the promotion audit independent of the TypeScript
    evidence producer and its changed-pixel counters.
    """

    payload = path.read_bytes()
    if not payload.startswith(b"\x89PNG\r\n\x1a\n"):
        raise ValueError("not a PNG file")
    offset = 8
    header: tuple[int, int, int, int, int, int, int] | None = None
    compressed = bytearray()
    saw_end = False
    while offset < len(payload):
        if offset + 12 > len(payload):
            raise ValueError("has a truncated PNG chunk")
        length = struct.unpack(">I", payload[offset : offset + 4])[0]
        kind = payload[offset + 4 : offset + 8]
        data_start = offset + 8
        data_end = data_start + length
        crc_end = data_end + 4
        if crc_end > len(payload):
            raise ValueError("has a truncated PNG chunk payload")
        data = payload[data_start:data_end]
        expected_crc = struct.unpack(">I", payload[data_end:crc_end])[0]
        actual_crc = zlib.crc32(kind + data) & 0xFFFFFFFF
        if actual_crc != expected_crc:
            raise ValueError(f"has an invalid {kind.decode('ascii', 'replace')} CRC")
        if kind == b"IHDR":
            if header is not None or length != 13:
                raise ValueError("has an invalid IHDR chunk")
            header = struct.unpack(">IIBBBBB", data)
        elif kind == b"IDAT":
            compressed.extend(data)
        elif kind == b"IEND":
            if length != 0:
                raise ValueError("has an invalid IEND chunk")
            saw_end = True
            offset = crc_end
            break
        offset = crc_end
    if header is None or not saw_end or offset != len(payload):
        raise ValueError("is missing a canonical IHDR/IEND structure")
    width, height, bit_depth, color_type, compression, filtering, interlace = header
    if width < 1 or height < 1:
        raise ValueError("has invalid dimensions")
    if (
        bit_depth != 8
        or color_type not in {2, 6}
        or compression != 0
        or filtering != 0
        or interlace != 0
    ):
        raise ValueError("must be a non-interlaced 8-bit RGB or RGBA PNG")
    channels = 3 if color_type == 2 else 4
    row_bytes = width * channels
    expected_size = (row_bytes + 1) * height
    try:
        decompressor = zlib.decompressobj()
        raw = decompressor.decompress(bytes(compressed), expected_size + 1)
        if len(raw) > expected_size or decompressor.unconsumed_tail:
            raise ValueError("has an oversized decompressed image payload")
        raw += decompressor.flush(max(1, expected_size + 1 - len(raw)))
    except zlib.error as error:
        raise ValueError(f"has invalid compressed image data: {error}") from error
    if (
        len(raw) != expected_size
        or decompressor.unconsumed_tail
        or decompressor.unused_data
        or not decompressor.eof
    ):
        raise ValueError("has an invalid decompressed image size")

    scanlines = bytearray(height * row_bytes)
    source_offset = 0
    prior = bytearray(row_bytes)
    for row in range(height):
        filter_kind = raw[source_offset]
        source_offset += 1
        filtered = raw[source_offset : source_offset + row_bytes]
        source_offset += row_bytes
        reconstructed = bytearray(row_bytes)
        for index, value in enumerate(filtered):
            left = reconstructed[index - channels] if index >= channels else 0
            up = prior[index]
            upper_left = prior[index - channels] if index >= channels else 0
            if filter_kind == 0:
                predictor = 0
            elif filter_kind == 1:
                predictor = left
            elif filter_kind == 2:
                predictor = up
            elif filter_kind == 3:
                predictor = (left + up) // 2
            elif filter_kind == 4:
                estimate = left + up - upper_left
                left_distance = abs(estimate - left)
                up_distance = abs(estimate - up)
                upper_left_distance = abs(estimate - upper_left)
                predictor = (
                    left
                    if left_distance <= up_distance
                    and left_distance <= upper_left_distance
                    else up
                    if up_distance <= upper_left_distance
                    else upper_left
                )
            else:
                raise ValueError(f"uses unsupported PNG filter {filter_kind}")
            reconstructed[index] = (value + predictor) & 0xFF
        start = row * row_bytes
        scanlines[start : start + row_bytes] = reconstructed
        prior = reconstructed

    if color_type == 6:
        pixels = bytes(scanlines)
    else:
        rgba = bytearray(width * height * 4)
        for source, destination in zip(
            range(0, len(scanlines), 3),
            range(0, len(rgba), 4),
            strict=True,
        ):
            rgba[destination : destination + 3] = scanlines[source : source + 3]
            rgba[destination + 3] = 255
        pixels = bytes(rgba)
    return PngRgba(width=width, height=height, pixels=pixels)


def _pixel_word(pixels: bytes, index: int) -> bytes:
    start = index * 4
    return pixels[start : start + 4]


def _color_delta(
    first: bytes,
    second: bytes,
    first_offset: int,
    second_offset: int,
    y_only: bool,
) -> float:
    r1, g1, b1, a1 = first[first_offset : first_offset + 4]
    r2, g2, b2, a2 = second[second_offset : second_offset + 4]
    dr = r1 - r2
    dg = g1 - g2
    db = b1 - b2
    da = a1 - a2
    if dr == dg == db == da == 0:
        return 0.0
    if a1 < 255 or a2 < 255:
        red_background = 48 + 159 * (first_offset % 2)
        green_background = 48 + 159 * (int(first_offset / 1.618033988749895) % 2)
        blue_background = 48 + 159 * (int(first_offset / 2.618033988749895) % 2)
        dr = (r1 * a1 - r2 * a2 - red_background * da) / 255
        dg = (g1 * a1 - g2 * a2 - green_background * da) / 255
        db = (b1 * a1 - b2 * a2 - blue_background * da) / 255
    luminance = dr * 0.29889531 + dg * 0.58662247 + db * 0.11448223
    if y_only:
        return luminance
    chroma_i = dr * 0.59597799 - dg * 0.27417610 - db * 0.32180189
    chroma_q = dr * 0.21147017 - dg * 0.52261711 + db * 0.31114694
    delta = (
        0.5053 * luminance * luminance
        + 0.299 * chroma_i * chroma_i
        + 0.1957 * chroma_q * chroma_q
    )
    return -delta if luminance > 0 else delta


def _has_many_siblings(
    pixels: bytes,
    x1: int,
    y1: int,
    width: int,
    height: int,
) -> bool:
    x0 = max(x1 - 1, 0)
    y0 = max(y1 - 1, 0)
    x2 = min(x1 + 1, width - 1)
    y2 = min(y1 + 1, height - 1)
    value = _pixel_word(pixels, y1 * width + x1)
    matches = 1 if x1 in {x0, x2} or y1 in {y0, y2} else 0
    for x in range(x0, x2 + 1):
        for y in range(y0, y2 + 1):
            if x == x1 and y == y1:
                continue
            if value == _pixel_word(pixels, y * width + x):
                matches += 1
                if matches > 2:
                    return True
    return False


def _antialiased(
    image: bytes,
    other: bytes,
    x1: int,
    y1: int,
    width: int,
    height: int,
) -> bool:
    x0 = max(x1 - 1, 0)
    y0 = max(y1 - 1, 0)
    x2 = min(x1 + 1, width - 1)
    y2 = min(y1 + 1, height - 1)
    pixel_index = y1 * width + x1
    zeroes = 1 if x1 in {x0, x2} or y1 in {y0, y2} else 0
    minimum = maximum = 0.0
    minimum_xy = maximum_xy = (0, 0)
    for x in range(x0, x2 + 1):
        for y in range(y0, y2 + 1):
            if x == x1 and y == y1:
                continue
            delta = _color_delta(
                image,
                image,
                pixel_index * 4,
                (y * width + x) * 4,
                True,
            )
            if delta == 0:
                zeroes += 1
                if zeroes > 2:
                    return False
            elif delta < minimum:
                minimum = delta
                minimum_xy = (x, y)
            elif delta > maximum:
                maximum = delta
                maximum_xy = (x, y)
    if minimum == 0 or maximum == 0:
        return False
    return (
        _has_many_siblings(image, *minimum_xy, width, height)
        and _has_many_siblings(other, *minimum_xy, width, height)
    ) or (
        _has_many_siblings(image, *maximum_xy, width, height)
        and _has_many_siblings(other, *maximum_xy, width, height)
    )


def pixelmatch_mask(
    expected: PngRgba,
    actual: PngRgba,
    threshold: float,
) -> tuple[int, bytes]:
    """Recompute pixelmatch's default anti-alias-aware count and diff mask."""

    if (expected.width, expected.height) != (actual.width, actual.height):
        raise ValueError("image dimensions do not match")
    if len(expected.pixels) != len(actual.pixels):
        raise ValueError("image pixel buffers do not match")
    mask = bytearray(len(expected.pixels))
    if expected.pixels == actual.pixels:
        return 0, bytes(mask)
    maximum_delta = 35215 * threshold * threshold
    changed = 0
    for y in range(expected.height):
        for x in range(expected.width):
            index = y * expected.width + x
            offset = index * 4
            if _pixel_word(expected.pixels, index) == _pixel_word(actual.pixels, index):
                continue
            delta = _color_delta(
                expected.pixels,
                actual.pixels,
                offset,
                offset,
                False,
            )
            if abs(delta) <= maximum_delta:
                continue
            if _antialiased(
                expected.pixels,
                actual.pixels,
                x,
                y,
                expected.width,
                expected.height,
            ) or _antialiased(
                actual.pixels,
                expected.pixels,
                x,
                y,
                expected.width,
                expected.height,
            ):
                continue
            mask[offset : offset + 4] = b"\xff\x00\x00\xff"
            changed += 1
    return changed, bytes(mask)


def _read_json(path: Path, label: str, errors: list[str]) -> dict[str, Any] | None:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        errors.append(f"missing {label}: {path}")
        return None
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        errors.append(f"cannot read {label} {path}: {error}")
        return None
    if not isinstance(value, dict):
        errors.append(f"{label} must contain a JSON object")
        return None
    return value


def _repo_path(
    root: Path,
    value: object,
    label: str,
    errors: list[str],
) -> tuple[Path, str] | None:
    if not isinstance(value, str) or not value:
        errors.append(f"{label} path is missing")
        return None
    if "\\" in value:
        errors.append(f"{label} path must use repository-relative POSIX syntax: {value}")
        return None
    relative = PurePosixPath(value)
    if relative.is_absolute() or ".." in relative.parts:
        errors.append(f"{label} path escapes the repository: {value}")
        return None
    candidate = root.joinpath(*relative.parts)
    try:
        candidate.resolve().relative_to(root.resolve())
    except ValueError:
        errors.append(f"{label} path escapes the repository: {value}")
        return None
    if candidate.is_symlink():
        errors.append(f"{label} must not be a symlink: {value}")
        return None
    return candidate, relative.as_posix()


def _audit_hashed_file(
    root: Path,
    path_value: object,
    hash_value: object,
    label: str,
    errors: list[str],
) -> tuple[Path, str] | None:
    resolved = _repo_path(root, path_value, label, errors)
    if resolved is None:
        return None
    path, relative = resolved
    if not path.is_file():
        errors.append(f"missing {label}: {relative}")
        return None
    if not isinstance(hash_value, str) or not SHA256.fullmatch(hash_value):
        errors.append(f"{label} sha256 is missing or invalid: {relative}")
        return None
    actual = sha256_file(path)
    if actual != hash_value:
        errors.append(
            f"{label} sha256 mismatch for {relative}: expected {hash_value}, got {actual}"
        )
        return None
    return path, relative


def _audit_official_source(code: str, source: object, errors: list[str]) -> None:
    if not isinstance(source, str):
        errors.append(f"{code}: official_source must be an official BIR HTTPS PDF URL")
        return
    parsed = urlsplit(source)
    if (
        parsed.scheme != "https"
        or parsed.hostname
        not in {"bir.gov.ph", "www.bir.gov.ph", "bir-cdn.bir.gov.ph"}
        or not parsed.path.lower().endswith(".pdf")
    ):
        errors.append(f"{code}: official_source must be an official BIR HTTPS PDF URL")


def _audit_source_path(value: object, label: str, errors: list[str]) -> None:
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{label}: source-pack path must be a non-empty relative path")
        return
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts:
        errors.append(f"{label}: source-pack path must remain inside the evidence root")


def _audit_source_catalog_entry(
    form: dict[str, Any],
    source: dict[str, Any],
    errors: list[str],
) -> None:
    code = str(form.get("code", "?"))
    revision = str(form.get("revision", "?"))
    label = f"{code}:{revision} source catalog"
    for field in ("code", "revision"):
        if source.get(field) != form.get(field):
            errors.append(f"{label}: {field} is inconsistent")
    _audit_source_path(source.get("source_pack_relative_path"), label, errors)
    if not isinstance(source.get("sha256"), str) or not SHA256.fullmatch(
        str(source.get("sha256", ""))
    ):
        errors.append(f"{label}: source PDF sha256 is invalid")
    _audit_official_source(code, source.get("official_source_url"), errors)
    if source.get("source_review") != "verified_official_bytes":
        errors.append(f"{label}: source_review must be verified_official_bytes")

    width = _as_number(source.get("page_width_pt"))
    height = _as_number(source.get("page_height_pt"))
    page_count = source.get("page_count")
    if width is None or height is None or (width, height) not in SUPPORTED_PAPER_SIZES:
        errors.append(f"{label}: unsupported paper geometry {width} x {height} pt")
    if not isinstance(page_count, int) or isinstance(page_count, bool) or page_count < 1:
        errors.append(f"{label}: page_count must be positive")

    sample_directory = source.get("sample_xml_directory")
    if sample_directory is not None:
        _audit_source_path(sample_directory, f"{label} XML directory", errors)

    companions = source.get("conditional_companions", [])
    if not isinstance(companions, list):
        errors.append(f"{label}: conditional_companions must be an array")
        return
    for index, companion in enumerate(companions):
        companion_label = f"{label} companion {index + 1}"
        if not isinstance(companion, dict):
            errors.append(f"{companion_label}: entry must be an object")
            continue
        _audit_source_path(
            companion.get("source_pack_relative_path"), companion_label, errors
        )
        if not isinstance(companion.get("sha256"), str) or not SHA256.fullmatch(
            str(companion.get("sha256", ""))
        ):
            errors.append(f"{companion_label}: sha256 is invalid")
        _audit_official_source(code, companion.get("official_source_url"), errors)
        if companion.get("source_review") != "verified_official_bytes":
            errors.append(
                f"{companion_label}: source_review must be verified_official_bytes"
            )
        companion_width = _as_number(companion.get("page_width_pt"))
        companion_height = _as_number(companion.get("page_height_pt"))
        if (
            companion_width is None
            or companion_height is None
            or (companion_width, companion_height) not in SUPPORTED_PAPER_SIZES
            and (companion_height, companion_width) not in SUPPORTED_PAPER_SIZES
        ):
            errors.append(
                f"{companion_label}: unsupported paper geometry "
                f"{companion_width} x {companion_height} pt"
            )
        companion_pages = companion.get("page_count")
        if (
            not isinstance(companion_pages, int)
            or isinstance(companion_pages, bool)
            or companion_pages < 1
        ):
            errors.append(f"{companion_label}: page_count must be positive")


def _capability(form: dict[str, Any], name: str) -> object:
    capabilities = form.get("capabilities")
    return capabilities.get(name) if isinstance(capabilities, dict) else None


def _html_enabled(form: dict[str, Any]) -> bool:
    return form.get("route") in {"experimental", "html_only"}


def _rust_provider_keys(root: Path, errors: list[str]) -> set[tuple[str, str]]:
    directory = root.joinpath(*RENDER_PROVIDER_DIR.parts)
    if not directory.is_dir():
        errors.append(f"missing Rust HTML provider directory: {RENDER_PROVIDER_DIR}")
        return set()
    keys: set[tuple[str, str]] = set()
    for path in sorted(directory.rglob("*.rs")):
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            errors.append(f"cannot read Rust HTML provider {path}: {error}")
            continue
        for provider in re.finditer(
            r"RenderFormProvider\s*=\s*RenderFormProvider\s*\{(.*?)\n\};",
            source,
            flags=re.DOTALL,
        ):
            body = provider.group(1)
            code = re.search(r'\bcode:\s*"([^"]+)"', body)
            revision = re.search(r'\brevision:\s*"([^"]+)"', body)
            if code is None or revision is None:
                errors.append(f"{path}: provider must declare literal code and revision")
                continue
            key = (code.group(1), revision.group(1))
            if key in keys:
                errors.append(f"duplicate Rust HTML provider {key[0]}:{key[1]}")
            keys.add(key)
    return keys


def _typescript_registry_keys(
    root: Path,
    relative: PurePosixPath,
    declaration: str,
    errors: list[str],
) -> set[tuple[str, str]]:
    path = root.joinpath(*relative.parts)
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        errors.append(f"cannot read TypeScript registry {relative}: {error}")
        return set()
    match = re.search(
        rf"\b{re.escape(declaration)}\b[^=]*=\s*\{{(.*?)\n\}};",
        source,
        flags=re.DOTALL,
    )
    if match is None:
        errors.append(f"cannot find {declaration} in {relative}")
        return set()
    return {
        (code, revision)
        for code, revision in re.findall(r'"([^"]+):([^"]+)"\s*:', match.group(1))
    }


def _as_number(value: object) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    number = float(value)
    return number if math.isfinite(number) else None


def _audit_source_revision(
    report: dict[str, Any],
    label: str,
    expected_revision: str | None,
    observed_revisions: list[tuple[str, str]],
    errors: list[str],
) -> None:
    revision = report.get("source_revision")
    if not isinstance(revision, str) or not GIT_REVISION.fullmatch(revision):
        errors.append(f"{label}: source_revision must be a canonical Git commit hash")
        return
    observed_revisions.append((label, revision))
    if expected_revision is None:
        errors.append(
            f"{label}: source_revision could not be bound to curated renderer source"
        )
    elif revision != expected_revision:
        errors.append(
            f"{label}: stale source_revision {revision}; current curated source is "
            f"{expected_revision}"
        )


def _audit_evidence_pointer(
    root: Path,
    pointer: object,
    label: str,
    errors: list[str],
) -> dict[str, Any] | None:
    """Load a curated evidence report through a passed, hashed pointer."""

    if pointer is None:
        return None
    if not isinstance(pointer, dict):
        errors.append(f"{label}: evidence must be null or an object")
        return None
    if pointer.get("passed") is not True:
        errors.append(f"{label}: curated evidence pointer must have passed=true")
    asset = _audit_hashed_file(
        root,
        pointer.get("path"),
        pointer.get("sha256"),
        f"{label} evidence",
        errors,
    )
    if asset is None:
        return None
    report = _read_json(asset[0], f"{label} evidence", errors)
    if report is None:
        return None
    if report.get("schema_version") != 1:
        errors.append(f"{label}: evidence schema_version must be 1")
    if report.get("passed") is not True:
        errors.append(f"{label}: referenced evidence report must have passed=true")
    return report


def _audit_visual_report(
    root: Path,
    report: dict[str, Any],
    form: dict[str, Any],
    reference: dict[str, Any],
    reference_manifest_hash: str,
    label: str,
    errors: list[str],
) -> None:
    if report.get("gate") != "visual_parity":
        errors.append(f"{label}: gate must be visual_parity")
    producer = report.get("producer")
    if producer not in TRUSTED_VISUAL_EVIDENCE_PRODUCERS:
        errors.append(
            f"{label}: no trusted attested visual evidence producer is "
            "registered; reporter-shaped or hand-authored reports cannot "
            "promote this gate"
        )
    if report.get("promotion_eligible") is not True:
        errors.append(f"{label}: non-promoting diagnostic output is not evidence")
    if report.get("source_worktree_clean") is not True:
        errors.append(f"{label}: producer must start from a clean curated source worktree")
    if report.get("producer_path") != VISUAL_EVIDENCE_PRODUCER_PATH.as_posix():
        errors.append(f"{label}: producer_path is inconsistent")
    _audit_hashed_file(
        root,
        report.get("producer_path"),
        report.get("producer_sha256"),
        f"{label} producer",
        errors,
    )
    if report.get("references_manifest") != REFERENCE_MANIFEST.as_posix():
        errors.append(f"{label}: references_manifest is inconsistent")
    if report.get("references_manifest_sha256") != reference_manifest_hash:
        errors.append(f"{label}: references_manifest_sha256 is inconsistent")

    pages = report.get("pages")
    expected_pages = reference.get("pages")
    page_count = reference.get("page_count")
    if not isinstance(pages, list):
        errors.append(f"{label}: pages must be an array")
        return
    if not isinstance(expected_pages, list) or not isinstance(page_count, int):
        return
    if report.get("expected_page_count") != page_count:
        errors.append(f"{label}: expected_page_count does not match references")
    if report.get("measured_page_count") != page_count or len(pages) != page_count:
        errors.append(f"{label}: every official page must have measured evidence")

    expected_by_page = {
        page.get("page"): page for page in expected_pages if isinstance(page, dict)
    }
    seen: set[int] = set()
    for page in pages:
        if not isinstance(page, dict):
            errors.append(f"{label}: every page evidence item must be an object")
            continue
        number = page.get("page")
        expected = expected_by_page.get(number)
        if not isinstance(number, int) or expected is None or number in seen:
            errors.append(f"{label}: page evidence must be unique and sequential")
            continue
        seen.add(number)
        prefix = f"{label} page {number}"
        if page.get("form_code") != form.get("code"):
            errors.append(f"{prefix}: form_code is inconsistent")
        if page.get("form_revision") != form.get("revision"):
            errors.append(f"{prefix}: form_revision is inconsistent")
        if page.get("fixture") != reference.get("fixture"):
            errors.append(f"{prefix}: fixture path is inconsistent")
        if page.get("fixture_sha256") != reference.get("fixture_sha256"):
            errors.append(f"{prefix}: fixture_sha256 is inconsistent")
        if page.get("reference") != expected.get("reference_png"):
            errors.append(f"{prefix}: reference path is inconsistent")
        if page.get("reference_sha256") != expected.get("reference_png_sha256"):
            errors.append(f"{prefix}: reference_sha256 is inconsistent")
        if page.get("comparison") != "official-complete-page-v1":
            errors.append(
                f"{prefix}: comparison must be official-complete-page-v1"
            )
        expected_ink_missing = _as_number(
            page.get("expected_ink_missing_percent")
        )
        unexpected_actual_ink = _as_number(
            page.get("unexpected_actual_ink_percent")
        )
        if expected_ink_missing is None or not 0 <= expected_ink_missing <= 100:
            errors.append(
                f"{prefix}: expected_ink_missing_percent is invalid"
            )
        if unexpected_actual_ink is None or not 0 <= unexpected_actual_ink <= 100:
            errors.append(
                f"{prefix}: unexpected_actual_ink_percent is invalid"
            )

        actual_asset = _audit_hashed_file(
            root,
            page.get("actual"),
            page.get("actual_sha256"),
            f"{prefix} rendered screenshot",
            errors,
        )
        diff_asset = _audit_hashed_file(
            root,
            page.get("diff"),
            page.get("diff_sha256"),
            f"{prefix} diff mask",
            errors,
        )
        reference_asset = _audit_hashed_file(
            root,
            page.get("reference"),
            page.get("reference_sha256"),
            f"{prefix} reference PNG",
            errors,
        )
        artifact_paths = {
            page.get("actual"), page.get("diff"), page.get("reference")
        }
        if len(artifact_paths) != 3:
            errors.append(f"{prefix}: actual, diff, and reference paths must be distinct")
        if (
            isinstance(page.get("actual_sha256"), str)
            and page.get("actual_sha256") == page.get("reference_sha256")
        ):
            errors.append(
                f"{prefix}: rendered screenshot must be derived independently "
                "from the reference PNG"
            )

        expected_width = expected.get("reference_width_px")
        expected_height = expected.get("reference_height_px")
        if (
            page.get("expected_width") != expected_width
            or page.get("expected_height") != expected_height
            or page.get("actual_width") != expected_width
            or page.get("actual_height") != expected_height
        ):
            errors.append(f"{prefix}: expected/actual dimensions do not match")

        changed_pixels = page.get("changed_pixels")
        changed_percent = _as_number(page.get("changed_percent"))
        maximum = _as_number(page.get("max_changed_percent"))
        threshold = _as_number(page.get("pixelmatch_threshold"))
        pixel_count = (
            expected_width * expected_height
            if isinstance(expected_width, int) and isinstance(expected_height, int)
            else None
        )
        valid_changed_pixels = (
            isinstance(changed_pixels, int)
            and not isinstance(changed_pixels, bool)
            and changed_pixels >= 0
            and pixel_count is not None
            and pixel_count > 0
            and changed_pixels <= pixel_count
        )
        if not valid_changed_pixels:
            errors.append(f"{prefix}: changed_pixels is invalid")
        if changed_percent is None or not 0 <= changed_percent <= 100:
            errors.append(f"{prefix}: changed_percent is invalid")
        computed_percent = (
            (changed_pixels / pixel_count) * 100
            if valid_changed_pixels
            else None
        )
        if (
            computed_percent is not None
            and changed_percent is not None
            and not math.isclose(
                changed_percent,
                computed_percent,
                rel_tol=1e-12,
                abs_tol=1e-9,
            )
        ):
            errors.append(
                f"{prefix}: changed_percent does not match changed_pixels "
                f"({computed_percent:.12f}% recomputed)"
            )
        if maximum is None or maximum > 1 or maximum < 0:
            errors.append(f"{prefix}: max_changed_percent must be at most 1")
        if threshold is None or threshold > 0.1 or threshold < 0:
            errors.append(f"{prefix}: pixelmatch_threshold must be at most 0.1")
        independently_changed: int | None = None
        if (
            actual_asset is not None
            and diff_asset is not None
            and reference_asset is not None
            and threshold is not None
            and 0 <= threshold <= 0.1
        ):
            try:
                if png_dimensions(actual_asset[0]) != (expected_width, expected_height):
                    raise ValueError("rendered screenshot dimensions are invalid")
                if png_dimensions(diff_asset[0]) != (expected_width, expected_height):
                    raise ValueError("diff mask dimensions are invalid")
                if png_dimensions(reference_asset[0]) != (expected_width, expected_height):
                    raise ValueError("reference PNG dimensions are invalid")
                actual_image = read_png_rgba(actual_asset[0])
                reference_image = read_png_rgba(reference_asset[0])
                diff_image = read_png_rgba(diff_asset[0])
                independently_changed, expected_mask = pixelmatch_mask(
                    reference_image,
                    actual_image,
                    threshold,
                )
            except (OSError, ValueError) as error:
                errors.append(f"{prefix}: cannot independently verify visual artifacts: {error}")
            else:
                if (
                    diff_image.width,
                    diff_image.height,
                    diff_image.pixels,
                ) != (
                    reference_image.width,
                    reference_image.height,
                    expected_mask,
                ):
                    errors.append(
                        f"{prefix}: diff mask does not match independently recomputed pixelmatch output"
                    )
                if changed_pixels != independently_changed:
                    errors.append(
                        f"{prefix}: changed_pixels does not match rendered screenshot "
                        f"({independently_changed} independently recomputed)"
                    )
        comparison_count = (
            independently_changed
            if independently_changed is not None
            else changed_pixels
        )
        recomputed_pass = (
            (comparison_count / pixel_count) * 100 <= maximum
            if isinstance(comparison_count, int)
            and pixel_count is not None
            and pixel_count > 0
            and maximum is not None
            else None
        )
        if recomputed_pass is False:
            errors.append(f"{prefix}: recomputed changed percent exceeds release threshold")
        if recomputed_pass is not None and page.get("passed") is not recomputed_pass:
            errors.append(f"{prefix}: passed does not match recomputed visual result")
        if page.get("passed") is not True:
            errors.append(f"{prefix}: passed must be true")
    if seen != set(range(1, page_count + 1)):
        errors.append(f"{label}: page evidence is incomplete")


def _audit_platform_report(
    root: Path,
    report: dict[str, Any],
    form: dict[str, Any],
    reference: dict[str, Any],
    platform: str,
    gate: str,
    label: str,
    errors: list[str],
) -> None:
    producer = report.get("producer")
    if producer not in TRUSTED_PLATFORM_EVIDENCE_PRODUCERS:
        errors.append(
            f"{label}: no trusted packaged platform evidence producer is "
            "registered; hand-authored reports cannot promote this gate"
        )
    if report.get("gate") != gate:
        errors.append(f"{label}: gate must be {gate}")
    if report.get("form_code") != form.get("code"):
        errors.append(f"{label}: form_code is inconsistent")
    if report.get("form_revision") != form.get("revision"):
        errors.append(f"{label}: form_revision is inconsistent")
    if report.get("platform") != platform:
        errors.append(f"{label}: platform must be {platform}")
    if not isinstance(report.get("architecture"), str) or not report["architecture"]:
        errors.append(f"{label}: architecture is required")
    if not isinstance(report.get("artifact_kind"), str) or not report["artifact_kind"]:
        errors.append(f"{label}: artifact_kind is required")
    artifact = _audit_hashed_file(
        root,
        report.get("artifact_path"),
        report.get("artifact_sha256"),
        f"{label} packaged artifact",
        errors,
    )

    renderer_assets = report.get("renderer_assets")
    if not isinstance(renderer_assets, list) or not renderer_assets:
        errors.append(f"{label}: renderer_assets must be a non-empty array")
    else:
        seen_assets: set[str] = set()
        for index, asset in enumerate(renderer_assets):
            if not isinstance(asset, dict):
                errors.append(f"{label}: renderer_assets[{index}] is invalid")
                continue
            verified_asset = _audit_hashed_file(
                root,
                asset.get("path"),
                asset.get("sha256"),
                f"{label} renderer_assets[{index}]",
                errors,
            )
            if verified_asset is not None:
                if verified_asset[1] in seen_assets:
                    errors.append(f"{label}: renderer_assets paths must be unique")
                seen_assets.add(verified_asset[1])
                if artifact is not None and verified_asset[1] == artifact[1]:
                    errors.append(
                        f"{label}: packaged artifact cannot double as a renderer asset"
                    )

    for key in ("network_disabled_runtime", "readiness"):
        value = report.get(key)
        if not isinstance(value, dict) or value.get("passed") is not True:
            errors.append(f"{label}: {key}.passed must be true")

    pages = report.get("renderer_pages")
    page_count_value = reference.get("page_count")
    page_count = (
        page_count_value
        if isinstance(page_count_value, int) and not isinstance(page_count_value, bool)
        else 0
    )
    if not isinstance(pages, list) or len(pages) != page_count:
        errors.append(f"{label}: renderer_pages must cover every official page")
    else:
        seen: set[int] = set()
        for page in pages:
            if not isinstance(page, dict):
                errors.append(f"{label}: renderer page evidence is invalid")
                continue
            number = page.get("page")
            if not isinstance(number, int) or number in seen:
                errors.append(f"{label}: renderer pages must be unique")
                continue
            seen.add(number)
            width = reference.get("page_width_pt")
            height = reference.get("page_height_pt")
            if (
                page.get("expected_width_pt") != width
                or page.get("expected_height_pt") != height
                or page.get("actual_width_pt") != width
                or page.get("actual_height_pt") != height
                or page.get("passed") is not True
            ):
                errors.append(f"{label}: renderer page {number} geometry is invalid")
        if seen != set(range(1, page_count + 1)):
            errors.append(f"{label}: renderer page evidence is incomplete")

    native_print = report.get("native_print")
    if (
        not isinstance(native_print, dict)
        or native_print.get("exercised") is not True
        or native_print.get("passed") is not True
    ):
        errors.append(f"{label}: native_print must be exercised and passed")
    native_pdf = report.get("native_pdf_export")
    if (
        not isinstance(native_pdf, dict)
        or native_pdf.get("exercised") is not True
        or native_pdf.get("passed") is not True
    ):
        errors.append(f"{label}: native_pdf_export must be exercised and passed")
    pdf_validation = report.get("pdf_validation")
    if not isinstance(pdf_validation, dict) or pdf_validation.get("passed") is not True:
        errors.append(f"{label}: pdf_validation.passed must be true")

    if gate == "packaged_offline":
        if report.get("network_runtime_exercised") is not True:
            errors.append(f"{label}: packaged evidence must exercise the network runtime")
        if report.get("packaged_runtime_promotion_satisfied") is not True:
            errors.append(
                f"{label}: static or development evidence cannot satisfy packaged promotion"
            )
        artifact_kind = str(report.get("artifact_kind", "")).lower()
        if any(marker in artifact_kind for marker in ("development", "source", "staged")):
            errors.append(f"{label}: artifact_kind is not a packaged application")


def _audit_rollback_report(
    root: Path,
    report: dict[str, Any],
    form: dict[str, Any],
    reference: dict[str, Any],
    label: str,
    errors: list[str],
) -> None:
    producer = report.get("producer")
    if producer not in TRUSTED_ROLLBACK_EVIDENCE_PRODUCERS:
        errors.append(
            f"{label}: no trusted rollback evidence producer is registered; "
            "hand-authored reports cannot promote release readiness"
        )
    if report.get("gate") != "rollback_drill":
        errors.append(f"{label}: gate must be rollback_drill")
    if report.get("form_code") != form.get("code"):
        errors.append(f"{label}: form_code is inconsistent")
    if report.get("form_revision") != form.get("revision"):
        errors.append(f"{label}: form_revision is inconsistent")
    if report.get("fixture_sha256") != reference.get("fixture_sha256"):
        errors.append(f"{label}: fixture_sha256 is inconsistent")
    cases = report.get("cases")
    passed_cases: set[object] = set()
    seen_cases: set[object] = set()
    if not isinstance(cases, list):
        errors.append(f"{label}: cases must be an array")
        cases = []
    for index, case in enumerate(cases):
        if not isinstance(case, dict):
            errors.append(f"{label}: rollback case {index} must be an object")
            continue
        name = case.get("name")
        if name not in ROLLBACK_CASES:
            errors.append(f"{label}: unknown rollback case {name!r}")
        if name in seen_cases:
            errors.append(f"{label}: rollback cases must be unique")
        seen_cases.add(name)
        _audit_hashed_file(
            root,
            case.get("artifact_path"),
            case.get("artifact_sha256"),
            f"{label} case {name!r} artifact",
            errors,
        )
        if case.get("passed") is True:
            passed_cases.add(name)
    missing = sorted(ROLLBACK_CASES - passed_cases)
    if missing:
        errors.append(f"{label}: missing passed rollback cases: {', '.join(missing)}")
    for before, after, name in (
        ("destination_before", "destination_after", "destination"),
        ("draft_before", "draft_after", "draft"),
    ):
        before_value = report.get(before)
        after_value = report.get(after)
        before_asset = _audit_hashed_file(
            root,
            before_value.get("path") if isinstance(before_value, dict) else None,
            before_value.get("sha256") if isinstance(before_value, dict) else None,
            f"{label} {name} before snapshot",
            errors,
        )
        after_asset = _audit_hashed_file(
            root,
            after_value.get("path") if isinstance(after_value, dict) else None,
            after_value.get("sha256") if isinstance(after_value, dict) else None,
            f"{label} {name} after snapshot",
            errors,
        )
        if (
            before_asset is not None
            and after_asset is not None
            and sha256_file(before_asset[0]) != sha256_file(after_asset[0])
        ):
            errors.append(f"{label}: {name} before/after snapshot contents changed")
        if (
            before_asset is not None
            and after_asset is not None
            and before_asset[1] == after_asset[1]
        ):
            errors.append(f"{label}: {name} before/after snapshots must be distinct files")
    temporary_manifest = _audit_hashed_file(
        root,
        report.get("temporary_files_manifest_path"),
        report.get("temporary_files_manifest_sha256"),
        f"{label} temporary-files manifest",
        errors,
    )
    if temporary_manifest is not None:
        manifest = _read_json(
            temporary_manifest[0], f"{label} temporary-files manifest", errors
        )
        if manifest is not None and manifest.get("remaining") != []:
            errors.append(f"{label}: temporary-files manifest must have no remaining files")
    if report.get("temporary_files_remaining") != 0:
        errors.append(f"{label}: temporary_files_remaining must be zero")


def _audit_reference(
    root: Path,
    form: dict[str, Any],
    reference: dict[str, Any],
    dpi: object,
    errors: list[str],
) -> None:
    code = str(form.get("code", "?"))
    revision = str(form.get("revision", "?"))
    form_id = form.get("form_id")
    label = f"{code}:{revision} reference"

    if reference.get("form_id") != form_id:
        errors.append(f"{label}: form_id is inconsistent")
    for key in ("code", "revision"):
        if reference.get(key) != form.get(key):
            errors.append(f"{label}: {key} is inconsistent")
    legacy_fields = [
        field for field in ("metadata", "formtype", "template")
        if field in reference
    ]
    if legacy_fields:
        errors.append(
            f"{label}: reference manifest cannot depend on legacy assets: "
            + ", ".join(legacy_fields)
        )

    _audit_official_source(code, reference.get("official_source"), errors)
    if not isinstance(reference.get("official_source_sha256"), str) or not SHA256.fullmatch(
        str(reference.get("official_source_sha256", ""))
    ):
        errors.append(f"{label}: official_source_sha256 is invalid")

    width = _as_number(reference.get("page_width_pt"))
    height = _as_number(reference.get("page_height_pt"))
    page_count = reference.get("page_count")
    if width is None or height is None or (width, height) not in SUPPORTED_PAPER_SIZES:
        errors.append(f"{label}: unsupported paper geometry {width} x {height} pt")
    if not isinstance(page_count, int) or isinstance(page_count, bool) or page_count < 1:
        errors.append(f"{label}: page_count must be positive")
        page_count = 0

    provenance = reference.get("reference_provenance")
    if not isinstance(provenance, dict):
        errors.append(f"{label}: reference_provenance must be an object")
    else:
        kind = provenance.get("kind")
        if kind not in {"official_pdf_raster", "legacy_filled_calibration_raster"}:
            errors.append(f"{label}: unsupported reference provenance {kind!r}")
        if provenance.get("runtime_eligible") is not False:
            errors.append(f"{label}: calibration reference cannot be runtime eligible")
        if (
            kind == "legacy_filled_calibration_raster"
            and provenance.get("replacement_required") is not True
        ):
            errors.append(f"{label}: legacy raster must remain replacement_required")

    fixture = _audit_hashed_file(
        root,
        reference.get("fixture"),
        reference.get("fixture_sha256"),
        f"{label} fixture",
        errors,
    )
    if fixture:
        fixture_json = _read_json(fixture[0], f"{label} fixture", errors) or {}
        fixture_form = fixture_json.get("form")
        if not isinstance(fixture_form, dict) or (
            fixture_form.get("code"), fixture_form.get("version")
        ) != (form.get("code"), form.get("revision")):
            errors.append(f"{label}: fixture form code/version is inconsistent")

    pages = reference.get("pages")
    if not isinstance(pages, list) or len(pages) != page_count:
        errors.append(f"{label}: pages must contain exactly {page_count} entries")
        return
    dpi_value = _as_number(dpi)
    scale = dpi_value / 72 if dpi_value is not None else 0
    expected_pixels = (
        round(width * scale) if width is not None else None,
        round(height * scale) if height is not None else None,
    )
    for expected_number, page in enumerate(pages, start=1):
        if not isinstance(page, dict):
            errors.append(f"{label}: page {expected_number} must be an object")
            continue
        if page.get("page") != expected_number:
            errors.append(f"{label}: pages must be sequential from page 1")
        if "source_svg" in page:
            errors.append(f"{label}: page {expected_number} cannot depend on a legacy source SVG")
        png = _audit_hashed_file(
            root,
            page.get("reference_png"),
            page.get("reference_png_sha256"),
            f"{label} page {expected_number} reference PNG",
            errors,
        )
        declared_pixels = (
            page.get("reference_width_px"),
            page.get("reference_height_px"),
        )
        if declared_pixels != expected_pixels:
            errors.append(
                f"{label}: page {expected_number} declared PNG dimensions are invalid"
            )
        if png:
            try:
                actual_pixels = png_dimensions(png[0])
            except ValueError as error:
                errors.append(f"{label}: page {expected_number} reference PNG {error}")
            else:
                if actual_pixels != expected_pixels:
                    errors.append(
                        f"{label}: page {expected_number} PNG is "
                        f"{actual_pixels[0]}x{actual_pixels[1]}, expected "
                        f"{expected_pixels[0]}x{expected_pixels[1]}"
                    )


def _release_entry_has_evidence_claim(entry: object) -> bool:
    if not isinstance(entry, dict):
        return entry is not None
    if entry.get("visual_parity") is not None or entry.get("rollback_drill") is not None:
        return True
    for key in ("native_print_export", "packaged_offline"):
        platforms = entry.get(key)
        if isinstance(platforms, dict):
            if any(value is not None for value in platforms.values()):
                return True
        elif platforms is not None:
            return True
    return False


def parse_form_revision(value: str) -> tuple[str, str]:
    """Parse an exact ``FORM:REVISION`` release-gate selector."""

    code, separator, revision = value.partition(":")
    code = code.strip()
    revision = revision.strip()
    if not separator or not code or not revision or ":" in revision:
        raise argparse.ArgumentTypeError(
            f"invalid form selector {value!r}; expected exact FORM:REVISION"
        )
    return code, revision


def _require_hashed_evidence_pointer(
    root: Path,
    pointer: object,
    label: str,
    errors: list[str],
) -> None:
    """Require a passing pointer whose referenced evidence bytes are hashed."""

    if not isinstance(pointer, dict):
        errors.append(f"{label}: required release evidence is missing")
        return
    if pointer.get("passed") is not True:
        errors.append(f"{label}: required release evidence must have passed=true")
    _audit_hashed_file(
        root,
        pointer.get("path"),
        pointer.get("sha256"),
        f"{label} required release evidence",
        errors,
    )


def _audit_required_release_target(
    root: Path,
    key: tuple[str, str],
    form: dict[str, Any] | None,
    form_evidence: object,
    errors: list[str],
) -> None:
    """Fail closed unless one exact form is fully certified for release."""

    label = f"{key[0]}:{key[1]}"
    if form is None:
        errors.append(f"required release target {label} has no migration record")
        return
    if form.get("route") != "html_only":
        errors.append(f"{label}: required release target requires html_only route")
    if form.get("support_level") != "ImplementedInApp":
        errors.append(
            f"{label}: required release target requires ImplementedInApp support"
        )
    if form.get("release_ready") is not True:
        errors.append(f"{label}: required release target requires release_ready=true")
    missing_capabilities = [
        capability
        for capability in PROMOTION_FLAGS
        if _capability(form, capability) is not True
    ]
    if missing_capabilities:
        errors.append(
            f"{label}: required release target is missing capabilities: "
            + ", ".join(missing_capabilities)
        )

    if not isinstance(form_evidence, dict):
        errors.append(f"{label}: required release evidence record is missing")
        form_evidence = {}
    _require_hashed_evidence_pointer(
        root,
        form_evidence.get("visual_parity"),
        f"{label} visual parity",
        errors,
    )
    for evidence_key in ("native_print_export", "packaged_offline"):
        platform_evidence = form_evidence.get(evidence_key)
        if not isinstance(platform_evidence, dict):
            errors.append(
                f"{label}: required {evidence_key} evidence must contain "
                "macos, windows, and linux"
            )
            platform_evidence = {}
        for platform in PLATFORMS:
            _require_hashed_evidence_pointer(
                root,
                platform_evidence.get(platform),
                f"{label} {evidence_key} {platform}",
                errors,
            )
    _require_hashed_evidence_pointer(
        root,
        form_evidence.get("rollback_drill"),
        f"{label} rollback drill",
        errors,
    )


def audit_repository(
    root: Path,
    *,
    revision_context: RevisionContext | None = None,
    required_release_ready: tuple[tuple[str, str], ...] = (),
) -> AuditResult:
    root = root.resolve()
    errors: list[str] = []
    statuses: list[str] = []
    migration = _read_json(
        root.joinpath(*MIGRATION_MANIFEST.parts), "migration manifest", errors
    )
    references = _read_json(
        root.joinpath(*REFERENCE_MANIFEST.parts), "reference manifest", errors
    )
    source_catalog = _read_json(
        root.joinpath(*SOURCE_CATALOG.parts), "source catalog", errors
    )
    evidence = _read_json(
        root.joinpath(*RELEASE_EVIDENCE.parts), "release evidence", errors
    )
    rust_capabilities = _read_json(
        root.joinpath(*RUST_CAPABILITY_MANIFEST.parts),
        "generated Rust capability manifest",
        errors,
    )
    if (
        migration is None
        or references is None
        or source_catalog is None
        or evidence is None
        or rust_capabilities is None
    ):
        return AuditResult(tuple(sorted(set(errors))), ())

    if migration.get("schema_version") != 3:
        errors.append("migration manifest schema_version must be 3")
    if references.get("schema_version") != 2:
        errors.append("reference manifest schema_version must be 2")
    if references.get("calibration_only") is not True:
        errors.append("reference manifest must be calibration_only")
    if references.get("runtime_background_allowed") is not False:
        errors.append("reference manifest must forbid runtime backgrounds")
    if references.get("dpi") != 144:
        errors.append("reference manifest dpi must be 144")
    if source_catalog.get("schema_version") != 1:
        errors.append("source catalog schema_version must be 1")
    if source_catalog.get("calibration_only") is not True:
        errors.append("source catalog must be calibration_only")
    if source_catalog.get("runtime_eligible") is not False:
        errors.append("source catalog must not be runtime eligible")
    source_pack = source_catalog.get("source_pack")
    if not isinstance(source_pack, dict):
        errors.append("source catalog source_pack must be an object")
    else:
        if not isinstance(source_pack.get("discovery_root"), str):
            errors.append("source catalog discovery_root must identify operator evidence")
        if not isinstance(source_pack.get("portable_note"), str):
            errors.append("source catalog portable_note must explain runtime isolation")
    if evidence.get("schema_version") != 1:
        errors.append("release evidence schema_version must be 1")
    if rust_capabilities.get("schema_version") != 1:
        errors.append("generated Rust capability manifest schema_version must be 1")

    forms_value = migration.get("forms")
    if not isinstance(forms_value, list):
        errors.append("migration manifest forms must be an array")
        forms_value = []
    forms: dict[tuple[object, object], dict[str, Any]] = {}
    for index, form in enumerate(forms_value):
        if not isinstance(form, dict):
            errors.append(f"migration form at index {index} must be an object")
            continue
        key = (form.get("code"), form.get("revision"))
        if key in forms:
            errors.append(f"duplicate migration form {key[0]}:{key[1]}")
            continue
        forms[key] = form
        missing = [
            field for field in BOOLEAN_STATUS_FIELDS
            if not isinstance(form.get(field), bool)
        ]
        if missing:
            errors.append(
                f"{key[0]}:{key[1]}: missing boolean status fields: {', '.join(missing)}"
            )
        capabilities = form.get("capabilities")
        if not isinstance(capabilities, dict):
            errors.append(f"{key[0]}:{key[1]}: capabilities must be an object")
            capabilities = {}
        missing_capabilities = [
            field for field in CAPABILITY_FIELDS
            if not isinstance(capabilities.get(field), bool)
        ]
        if missing_capabilities:
            errors.append(
                f"{key[0]}:{key[1]}: missing boolean capabilities: "
                + ", ".join(missing_capabilities)
            )
        route = form.get("route")
        if route not in ROUTES:
            errors.append(f"{key[0]}:{key[1]}: unsupported route {route!r}")
        if form.get("support_level") not in {"ImplementedInApp", "ScaffoldOnly"}:
            errors.append(f"{key[0]}:{key[1]}: unsupported support_level")
        if (
            form.get("support_level") == "ImplementedInApp"
            and form.get("release_ready") is not True
        ):
            errors.append(
                f"{key[0]}:{key[1]}: ImplementedInApp requires release_ready"
            )
        if _html_enabled(form):
            missing_structure = [
                flag for flag in STRUCTURAL_CAPABILITIES
                if _capability(form, flag) is not True
            ]
            if missing_structure:
                errors.append(
                    f"{key[0]}:{key[1]}: enabled route is missing structural capabilities: "
                    + ", ".join(missing_structure)
                )
        if form.get("release_ready"):
            if route != "html_only":
                errors.append(f"{key[0]}:{key[1]}: release_ready requires html_only route")
            if form.get("support_level") != "ImplementedInApp":
                errors.append(
                    f"{key[0]}:{key[1]}: release_ready requires ImplementedInApp"
                )
            incomplete = [
                flag for flag in PROMOTION_FLAGS
                if _capability(form, flag) is not True
            ]
            if incomplete:
                errors.append(
                    f"{key[0]}:{key[1]}: release_ready is missing capabilities: "
                    + ", ".join(incomplete)
                )

    rust_forms_value = rust_capabilities.get("forms")
    if not isinstance(rust_forms_value, list):
        errors.append("generated Rust capability manifest forms must be an array")
        rust_forms_value = []
    rust_forms: dict[tuple[object, object], dict[str, Any]] = {}
    for index, rust_form in enumerate(rust_forms_value):
        if not isinstance(rust_form, dict):
            errors.append(
                f"generated Rust capability record at index {index} must be an object"
            )
            continue
        key = (rust_form.get("code"), rust_form.get("revision"))
        if key in rust_forms:
            errors.append(
                f"duplicate generated Rust capability record {key[0]}:{key[1]}"
            )
            continue
        rust_forms[key] = rust_form
        form = forms.get(key)
        if form is None:
            errors.append(
                f"generated Rust capability {key[0]}:{key[1]} has no migration record"
            )
            continue
        label = f"{key[0]}:{key[1]}"
        for field in ("form_id", "support_level", "release_ready"):
            if form.get(field) != rust_form.get(field):
                errors.append(
                    f"{label}: {field} differs from generated Rust capability registry"
                )
        rust_flags = rust_form.get("capabilities")
        if not isinstance(rust_flags, dict):
            errors.append(f"{label}: generated Rust capabilities must be an object")
            rust_flags = {}
        for capability in CAPABILITY_FIELDS:
            expected = rust_flags.get(capability)
            if not isinstance(expected, bool):
                errors.append(
                    f"{label}: generated Rust capability {capability} must be boolean"
                )
            elif _capability(form, capability) is not expected:
                errors.append(
                    f"{label}: capabilities.{capability} differs from generated Rust registry"
                )

    for key in sorted(set(forms) - set(rust_forms)):
        errors.append(
            f"migration record {key[0]}:{key[1]} has no generated Rust capability"
        )

    rust_providers = _rust_provider_keys(root, errors)
    component_keys = _typescript_registry_keys(
        root, COMPONENT_REGISTRY, "FORM_COMPONENT_REGISTRY", errors
    )
    spec_keys = _typescript_registry_keys(
        root, SPEC_REGISTRY, "FORM_SPEC_REGISTRY", errors
    )
    manifest_keys = set(forms)
    for label, registry in (
        ("Rust HTML provider", rust_providers),
        ("React form component", component_keys),
        ("form specification", spec_keys),
    ):
        for key in sorted(registry - manifest_keys):
            errors.append(f"{label} {key[0]}:{key[1]} has no migration record")
    for key, form in forms.items():
        for capability, registry, label in (
            ("render_contract", rust_providers, "Rust HTML provider"),
            ("html_component", component_keys, "React form component"),
            ("html_spec", spec_keys, "form specification"),
        ):
            if _capability(form, capability) is True and key not in registry:
                errors.append(f"{key[0]}:{key[1]}: missing {label}")

    source_values = source_catalog.get("forms")
    if not isinstance(source_values, list):
        errors.append("source catalog forms must be an array")
        source_values = []
    source_forms: dict[tuple[object, object], dict[str, Any]] = {}
    for index, source in enumerate(source_values):
        if not isinstance(source, dict):
            errors.append(f"source catalog form at index {index} must be an object")
            continue
        key = (source.get("code"), source.get("revision"))
        if key in source_forms:
            errors.append(f"duplicate source catalog form {key[0]}:{key[1]}")
            continue
        source_forms[key] = source
        form = forms.get(key)
        if form is None:
            errors.append(
                f"source catalog form {key[0]}:{key[1]} has no migration record"
            )
            continue
        _audit_source_catalog_entry(form, source, errors)
    for key in sorted(set(forms) - set(source_forms)):
        errors.append(
            f"migration record {key[0]}:{key[1]} has no reviewed source catalog entry"
        )

    reference_values = references.get("forms")
    if not isinstance(reference_values, list):
        errors.append("reference manifest forms must be an array")
        reference_values = []
    reference_forms: dict[tuple[object, object], dict[str, Any]] = {}
    for index, reference in enumerate(reference_values):
        if not isinstance(reference, dict):
            errors.append(f"reference form at index {index} must be an object")
            continue
        key = (reference.get("code"), reference.get("revision"))
        if key in reference_forms:
            errors.append(f"duplicate reference form {key[0]}:{key[1]}")
            continue
        reference_forms[key] = reference
        form = forms.get(key)
        if form is None:
            errors.append(f"reference form {key[0]}:{key[1]} has no migration record")
            continue
        _audit_reference(root, form, reference, references.get("dpi", 0), errors)
        source = source_forms.get(key)
        if source is None:
            continue
        for reference_field, source_field in (
            ("official_source", "official_source_url"),
            ("official_source_sha256", "sha256"),
            ("page_count", "page_count"),
            ("page_width_pt", "page_width_pt"),
            ("page_height_pt", "page_height_pt"),
        ):
            if reference.get(reference_field) != source.get(source_field):
                errors.append(
                    f"{key[0]}:{key[1]}: reference {reference_field} differs "
                    "from the reviewed source catalog"
                )

    evidence_forms = evidence.get("forms")
    if not isinstance(evidence_forms, dict):
        errors.append("release evidence forms must be an object")
        evidence_forms = {}
    expected_evidence_keys = {
        f"{key[0]}:{key[1]}" for key, form in forms.items() if _html_enabled(form)
    }
    unknown_evidence = sorted(set(evidence_forms) - {f"{k[0]}:{k[1]}" for k in forms})
    if unknown_evidence:
        errors.append("release evidence has unknown forms: " + ", ".join(unknown_evidence))
    missing_evidence = sorted(expected_evidence_keys - set(evidence_forms))
    if missing_evidence:
        errors.append("HTML-enabled forms lack release evidence: " + ", ".join(missing_evidence))

    for key in sorted(set(required_release_ready)):
        form = forms.get(key)
        _audit_required_release_target(
            root,
            key,
            form,
            evidence_forms.get(f"{key[0]}:{key[1]}"),
            errors,
        )

    positive_status_claim = any(
        form.get("release_ready") is True
        or any(_capability(form, flag) is True for flag in EVIDENCE_CAPABILITIES)
        for form in forms.values()
    )
    positive_evidence_claim = any(
        _release_entry_has_evidence_claim(entry)
        for entry in evidence_forms.values()
    )
    expected_source_revision: str | None = None
    if positive_status_claim or positive_evidence_claim:
        if revision_context is None:
            try:
                revision_context = git_revision_context(root)
            except RuntimeError as error:
                errors.append(str(error))
        if revision_context is not None:
            if not GIT_REVISION.fullmatch(revision_context.source_revision):
                errors.append(
                    "current curated source revision is not a canonical commit hash"
                )
            else:
                expected_source_revision = revision_context.source_revision
            if revision_context.dirty_paths:
                preview = ", ".join(revision_context.dirty_paths[:8])
                if len(revision_context.dirty_paths) > 8:
                    preview += ", ..."
                errors.append(
                    "positive readiness/evidence claims require a clean curated "
                    f"source worktree; dirty entries: {preview}"
                )

    reference_manifest_hash = sha256_file(root.joinpath(*REFERENCE_MANIFEST.parts))
    observed_revisions: list[tuple[str, str]] = []
    for key, form in forms.items():
        code, revision = key
        status = "release" if form.get("release_ready") is True else form.get("route")
        statuses.append(f"{code}:{revision} {status}")
        if not _html_enabled(form):
            continue
        reference = reference_forms.get(key)
        if reference is None:
            errors.append(f"{code}:{revision}: HTML-enabled form lacks references")
            continue
        provenance = reference.get("reference_provenance")
        if (
            form.get("release_ready") is True
            and isinstance(provenance, dict)
            and provenance.get("kind") != "official_pdf_raster"
        ):
            errors.append(
                f"{code}:{revision}: release_ready requires reviewed official-PDF references"
            )
        form_key = f"{code}:{revision}"
        form_evidence = evidence_forms.get(form_key)
        if not isinstance(form_evidence, dict):
            continue
        if form_evidence.get("references_manifest") != REFERENCE_MANIFEST.as_posix():
            errors.append(f"{form_key}: release evidence references_manifest is inconsistent")

        visual = _audit_evidence_pointer(
            root, form_evidence.get("visual_parity"), f"{form_key} visual parity", errors
        )
        if visual is not None:
            _audit_source_revision(
                visual,
                f"{form_key} visual parity",
                expected_source_revision,
                observed_revisions,
                errors,
            )
            _audit_visual_report(
                root,
                visual,
                form,
                reference,
                reference_manifest_hash,
                f"{form_key} visual parity",
                errors,
            )
        if _capability(form, "visual_parity") is True and visual is None:
            errors.append(f"{form_key}: visual_parity capability lacks passed evidence")

        for evidence_key, capabilities, gate in (
            (
                "native_print_export",
                ("native_preview", "native_print", "pdf_export"),
                "native_print_export",
            ),
            ("packaged_offline", ("packaged_offline",), "packaged_offline"),
        ):
            platform_values = form_evidence.get(evidence_key)
            if not isinstance(platform_values, dict):
                errors.append(
                    f"{form_key}: {evidence_key} must contain macos, windows, and linux"
                )
                platform_values = {}
            for platform in PLATFORMS:
                report = _audit_evidence_pointer(
                    root,
                    platform_values.get(platform),
                    f"{form_key} {evidence_key} {platform}",
                    errors,
                )
                if report is not None:
                    _audit_source_revision(
                        report,
                        f"{form_key} {evidence_key} {platform}",
                        expected_source_revision,
                        observed_revisions,
                        errors,
                    )
                    _audit_platform_report(
                        root,
                        report,
                        form,
                        reference,
                        platform,
                        gate,
                        f"{form_key} {evidence_key} {platform}",
                        errors,
                    )
                claimed = [
                    capability for capability in capabilities
                    if _capability(form, capability) is True
                ]
                if claimed and report is None:
                    errors.append(
                        f"{form_key}: {', '.join(claimed)} capabilities lack passed "
                        f"{platform} evidence"
                    )

        rollback = _audit_evidence_pointer(
            root,
            form_evidence.get("rollback_drill"),
            f"{form_key} rollback drill",
            errors,
        )
        if rollback is not None:
            _audit_source_revision(
                rollback,
                f"{form_key} rollback drill",
                expected_source_revision,
                observed_revisions,
                errors,
            )
            _audit_rollback_report(
                root,
                rollback,
                form,
                reference,
                f"{form_key} rollback drill",
                errors,
            )
        if form.get("release_ready") and rollback is None:
            errors.append(f"{form_key}: release_ready lacks passed rollback-drill evidence")

    distinct_revisions = {revision for _, revision in observed_revisions}
    if len(distinct_revisions) > 1:
        details = ", ".join(
            f"{label}={revision}" for label, revision in observed_revisions
        )
        errors.append(f"curated evidence reports use mixed source revisions: {details}")

    return AuditResult(tuple(sorted(set(errors))), tuple(sorted(statuses)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="repository root (used by focused tests and diagnostics)",
    )
    parser.add_argument(
        "--print-source-revision",
        action="store_true",
        help="print the curated renderer source revision for evidence producers",
    )
    parser.add_argument(
        "--require-clean-source",
        action="store_true",
        help=(
            "fail before producing evidence when any curated renderer source "
            "path is dirty"
        ),
    )
    parser.add_argument(
        "--require-release-ready",
        action="append",
        default=[],
        metavar="FORM:REVISION",
        type=parse_form_revision,
        help=(
            "fail unless the exact form is html_only, release_ready, has every "
            "capability, and has passed hashed visual/native/package/rollback evidence; "
            "may be repeated"
        ),
    )
    args = parser.parse_args()
    if args.print_source_revision and args.require_release_ready:
        parser.error(
            "--print-source-revision cannot be combined with --require-release-ready"
        )
    revision_context: RevisionContext | None = None
    if args.print_source_revision or args.require_clean_source:
        try:
            revision_context = git_revision_context(args.root)
        except RuntimeError as error:
            print(error, file=sys.stderr)
            return 1
        if args.require_clean_source and revision_context.dirty_paths:
            preview = ", ".join(revision_context.dirty_paths[:8])
            if len(revision_context.dirty_paths) > 8:
                preview += ", ..."
            print(
                "visual evidence requires a clean curated source worktree; "
                f"dirty entries: {preview}",
                file=sys.stderr,
            )
            return 1
        if args.print_source_revision:
            print(revision_context.source_revision)
            return 0
    if args.require_clean_source and not args.require_release_ready:
        return 0
    result = audit_repository(
        args.root,
        revision_context=revision_context,
        required_release_ready=tuple(args.require_release_ready),
    )
    for status in result.statuses:
        print(status)
    if result.errors:
        print("\nHTML form migration audit failed:", file=sys.stderr)
        for error in result.errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print("\nHTML form migration audit passed without changing readiness.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
