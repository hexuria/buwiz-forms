#!/usr/bin/env python3
"""Audit HTML form migration claims without promoting a renderer.

The migration manifest deliberately separates an implementation that is
available for experimental preview (``html_enabled``) from one that is safe to
route to users (``release_ready``).  This audit accepts conservative false
flags, but fails closed when a positive readiness claim is not backed by
hashed, structured evidence.

The current rebuild is intentionally scoped to BIR Form 2551Q January 2018.
The implementation is generic enough to reject a future HTML-enabled scaffold
record, but it does not infer readiness or update either manifest.
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
SUPPORT_SOURCE = PurePosixPath("crates/bir-core/src/forms/support_level.rs")

TARGET_FORM = ("2551Q", "2018")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
GIT_REVISION = re.compile(r"^[0-9a-f]{40}(?:[0-9a-f]{24})?$")
SUPPORTED_PAPER_SIZES = {
    (612.0, 792.0),
    (612.0, 936.0),
    (612.0, 1008.0),
}

# Every boolean below is a promotion fact.  ``contract_complete`` is included
# explicitly; the donor audit omitted it even though the current manifest owns
# that gate.
PROMOTION_FLAGS = (
    "contract_complete",
    "typed_model_complete",
    "xml_complete",
    "formula_evidence_complete",
    "layout_calibrated",
    "validation_complete",
    "carry_over_complete",
    "pagination_complete",
    "visual_parity_complete",
    "native_print_export_verified",
    "packaged_offline_verified",
)
BOOLEAN_STATUS_FIELDS = ("html_enabled", *PROMOTION_FLAGS, "release_ready")
PLATFORMS = ("macos", "windows")
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
UNBOUND_PROMOTION_FLAGS = (
    "contract_complete",
    "typed_model_complete",
    "xml_complete",
    "formula_evidence_complete",
    "layout_calibrated",
    "validation_complete",
    "carry_over_complete",
    "pagination_complete",
)

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
    "formtypes/2551Qv2018",
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


def _rust_fileable_codes(root: Path, errors: list[str]) -> set[str]:
    path = root.joinpath(*SUPPORT_SOURCE.parts)
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        errors.append(f"cannot read Rust support source {path}: {error}")
        return set()
    match = re.search(
        r"const\s+FILEABLE_FORM_CODES:\s*&\[&str\]\s*=\s*&\[(.*?)\];",
        source,
        flags=re.DOTALL,
    )
    if not match:
        errors.append("cannot find Rust FILEABLE_FORM_CODES")
        return set()
    return set(re.findall(r'"([^"]+)"', match.group(1)))


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

    expected_paths = {
        "metadata": f"formtypes/{form_id}/metadata.json",
        "formtype": f"formtypes/{form_id}/formtype.json",
        "template": f"formtypes/{form_id}/template.typ",
    }
    loaded: dict[str, dict[str, Any]] = {}
    for asset_name, expected_path in expected_paths.items():
        if reference.get(asset_name) != expected_path:
            errors.append(f"{label}: {asset_name} must be {expected_path}")
        asset = _audit_hashed_file(
            root,
            reference.get(asset_name),
            reference.get(f"{asset_name}_sha256"),
            f"{label} {asset_name}",
            errors,
        )
        if asset and asset_name in {"metadata", "formtype"}:
            parsed = _read_json(asset[0], f"{label} {asset_name}", errors)
            if parsed is not None:
                loaded[asset_name] = parsed

    metadata = loaded.get("metadata", {})
    formtype = loaded.get("formtype", {})
    if metadata.get("form_id") != form_id:
        errors.append(f"{label}: metadata form_id is inconsistent")
    if formtype.get("form_id") != form_id:
        errors.append(f"{label}: formtype form_id is inconsistent")
    _audit_official_source(code, metadata.get("official_source"), errors)
    if not isinstance(metadata.get("sha256"), str) or not SHA256.fullmatch(
        str(metadata.get("sha256", ""))
    ):
        errors.append(f"{label}: metadata official source sha256 is invalid")
    if reference.get("official_source") != metadata.get("official_source"):
        errors.append(f"{label}: official_source is inconsistent with metadata")
    if reference.get("official_source_sha256") != metadata.get("sha256"):
        errors.append(f"{label}: official_source_sha256 is inconsistent with metadata")

    width = _as_number(metadata.get("page_width_pt"))
    height = _as_number(metadata.get("page_height_pt"))
    page_count = metadata.get("page_count")
    if width is None or height is None or (width, height) not in SUPPORTED_PAPER_SIZES:
        errors.append(f"{label}: unsupported paper metadata {width} x {height} pt")
    if not isinstance(page_count, int) or isinstance(page_count, bool) or page_count < 1:
        errors.append(f"{label}: metadata page_count must be positive")
        page_count = 0
    if (
        reference.get("page_width_pt") != metadata.get("page_width_pt")
        or reference.get("page_height_pt") != metadata.get("page_height_pt")
        or reference.get("page_count") != page_count
    ):
        errors.append(f"{label}: page geometry is inconsistent with metadata")

    fields = formtype.get("fields")
    placeholders = [
        field.get("key")
        for field in fields
        if isinstance(field, dict)
        and isinstance(field.get("key"), str)
        and field["key"].startswith("_field_")
    ] if isinstance(fields, list) else []
    if form.get("layout_calibrated") is True and placeholders:
        errors.append(f"{label}: calibrated layout still has placeholder field keys")

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
        source_path = f"formtypes/{form_id}/pages/page{expected_number}.svg"
        if page.get("source_svg") != source_path:
            errors.append(f"{label}: page {expected_number} source_svg must be {source_path}")
        _audit_hashed_file(
            root,
            page.get("source_svg"),
            page.get("source_svg_sha256"),
            f"{label} page {expected_number} source SVG",
            errors,
        )
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


def audit_repository(
    root: Path,
    *,
    revision_context: RevisionContext | None = None,
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
    evidence = _read_json(
        root.joinpath(*RELEASE_EVIDENCE.parts), "release evidence", errors
    )
    if migration is None or references is None or evidence is None:
        return AuditResult(tuple(sorted(set(errors))), ())

    if migration.get("schema_version") != 2:
        errors.append("migration manifest schema_version must be 2")
    if references.get("schema_version") != 1:
        errors.append("reference manifest schema_version must be 1")
    if references.get("calibration_only") is not True:
        errors.append("reference manifest must be calibration_only")
    if references.get("runtime_background_allowed") is not False:
        errors.append("reference manifest must forbid runtime backgrounds")
    if references.get("dpi") != 144:
        errors.append("reference manifest dpi must be 144")
    if evidence.get("schema_version") != 1:
        errors.append("release evidence schema_version must be 1")

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
        if form.get("support_level") not in {"ImplementedInApp", "ScaffoldOnly"}:
            errors.append(f"{key[0]}:{key[1]}: unsupported support_level")
        if form.get("support_level") == "ScaffoldOnly" and form.get("html_enabled"):
            errors.append(f"{key[0]}:{key[1]}: scaffold form cannot enable HTML")
        unbound = [flag for flag in UNBOUND_PROMOTION_FLAGS if form.get(flag) is True]
        if unbound:
            errors.append(
                f"{key[0]}:{key[1]}: promotion flags lack a trusted derived "
                "evidence producer: " + ", ".join(unbound)
            )
        if form.get("release_ready"):
            if not form.get("html_enabled"):
                errors.append(f"{key[0]}:{key[1]}: release_ready requires html_enabled")
            if form.get("support_level") != "ImplementedInApp":
                errors.append(
                    f"{key[0]}:{key[1]}: release_ready requires ImplementedInApp"
                )
            incomplete = [flag for flag in PROMOTION_FLAGS if form.get(flag) is not True]
            if incomplete:
                errors.append(
                    f"{key[0]}:{key[1]}: release_ready is missing promotion flags: "
                    + ", ".join(incomplete)
                )

    if TARGET_FORM not in forms:
        errors.append("migration manifest must contain 2551Q:2018")

    rust_fileable = _rust_fileable_codes(root, errors)
    for key, form in forms.items():
        code = form.get("code")
        if form.get("support_level") == "ImplementedInApp" and code not in rust_fileable:
            errors.append(f"{key[0]}:{key[1]}: Rust does not mark the form fileable")
        if form.get("support_level") == "ScaffoldOnly" and code in rust_fileable:
            errors.append(f"{key[0]}:{key[1]}: Rust marks a scaffold form fileable")

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

    evidence_forms = evidence.get("forms")
    if not isinstance(evidence_forms, dict):
        errors.append("release evidence forms must be an object")
        evidence_forms = {}
    expected_evidence_keys = {
        f"{key[0]}:{key[1]}" for key, form in forms.items() if form.get("html_enabled")
    }
    unknown_evidence = sorted(set(evidence_forms) - {f"{k[0]}:{k[1]}" for k in forms})
    if unknown_evidence:
        errors.append("release evidence has unknown forms: " + ", ".join(unknown_evidence))
    missing_evidence = sorted(expected_evidence_keys - set(evidence_forms))
    if missing_evidence:
        errors.append("HTML-enabled forms lack release evidence: " + ", ".join(missing_evidence))

    positive_status_claim = any(
        form.get("release_ready") is True
        or any(form.get(flag) is True for flag in PROMOTION_FLAGS)
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
        status = "release" if form.get("release_ready") else (
            "experimental" if form.get("html_enabled") else "legacy"
        )
        statuses.append(f"{code}:{revision} {status}")
        if not form.get("html_enabled"):
            continue
        reference = reference_forms.get(key)
        if reference is None:
            errors.append(f"{code}:{revision}: HTML-enabled form lacks references")
            continue
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
        if form.get("visual_parity_complete") and visual is None:
            errors.append(f"{form_key}: visual_parity_complete lacks passed evidence")

        for evidence_key, flag, gate in (
            ("native_print_export", "native_print_export_verified", "native_print_export"),
            ("packaged_offline", "packaged_offline_verified", "packaged_offline"),
        ):
            platform_values = form_evidence.get(evidence_key)
            if not isinstance(platform_values, dict):
                errors.append(f"{form_key}: {evidence_key} must contain macos and windows")
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
                if form.get(flag) and report is None:
                    errors.append(
                        f"{form_key}: {flag} lacks passed {platform} evidence"
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
    args = parser.parse_args()
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
    if args.require_clean_source:
        return 0
    result = audit_repository(args.root)
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
