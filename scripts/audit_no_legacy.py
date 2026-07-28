#!/usr/bin/env python3
"""Fail closed when a production package can still reach the legacy renderer.

This gate is intentionally independent of the form-migration readiness audit.
It answers one narrower question: can production code or packaging still ship
Typst, runtime formtypes, a full-page renderer background, the legacy PDF
viewer/fallback route, or a Node runtime?

Documentation, tests, generated build directories, and the pinned official
reference images used only for visual calibration are outside that question.
Using npm/Node to *build* the static renderer is allowed; copying Node or
``node_modules`` into a production artifact is not.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import json
import os
import re
import struct
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Iterable, Iterator, Sequence


CATEGORIES = (
    "typst-runtime",
    "typst-packaging",
    "typ-artifact",
    "runtime-formtypes",
    "full-page-background",
    "legacy-renderer",
    "runtime-node",
)

IGNORED_TOP_LEVEL = {
    ".agent",
    ".codex",
    ".git",
    ".ruff_cache",
    ".scratch",
    ".tmp",
    "docs",
    "node_modules",
    "target",
    "target-linux-check",
    "test-results",
}
IGNORED_DIRECTORY_NAMES = {
    "__pycache__",
    "coverage",
    "dist",
    "tests",
}
# Auditors have to name the tokens they detect. Exempting them is not a
# weakening of the audit: each of these files exists to *find* legacy artifacts,
# and every occurrence below is either a detector pattern or a self-test
# asserting the detector fires.
IGNORED_FILE_NAMES = {
    "audit_no_legacy.py",
    "audit_html_form_migration.py",
    "verify_offline_form_renderer.py",
    "verify_form_conversion.py",
}
OFFICIAL_REFERENCE_ROOTS = (
    PurePosixPath("packages/form-renderer/references"),
)
PACKAGING_FILES = {
    PurePosixPath("justfile"),
    PurePosixPath("installer.iss"),
    PurePosixPath("installer.wxs"),
    PurePosixPath("entitlements.plist"),
    PurePosixPath("entitlements.dev.plist"),
    PurePosixPath("daemon.entitlements.plist"),
    PurePosixPath("crates/bir-desktop/Cargo.toml"),
}
PACKAGING_PREFIXES = (
    PurePosixPath(".github/workflows"),
    PurePosixPath("assets/macos"),
    PurePosixPath("scripts"),
)
PRODUCTION_SOURCE_ROOTS = (
    PurePosixPath("crates"),
    PurePosixPath("apps"),
    PurePosixPath("packages"),
)
RUNTIME_IMAGE_ROOTS = (
    PurePosixPath("assets/form-renderer"),
    PurePosixPath("apps/form-preview/src"),
    PurePosixPath("crates/bir-print/templates"),
    PurePosixPath("packages/form-renderer/src"),
    PurePosixPath("formtypes"),
)

TEXT_SUFFIXES = {
    "",
    ".css",
    ".html",
    ".iss",
    ".js",
    ".json",
    ".jsx",
    ".mjs",
    ".plist",
    ".ps1",
    ".py",
    ".rs",
    ".sh",
    ".toml",
    ".ts",
    ".tsx",
    ".wxs",
    ".xml",
    ".yaml",
    ".yml",
}
IMAGE_SUFFIXES = {
    ".bmp",
    ".gif",
    ".jpeg",
    ".jpg",
    ".pdf",
    ".png",
    ".svg",
    ".webp",
}

TYPST_RUNTIME_PATTERN = re.compile(
    r"(?:"
    r"\b(?:build_typst_command|get_typst_binary|generate_typst|TypstCompile)\b"
    r"|\bCommand::new\s*\([^\n)]*['\"]typst(?:\.exe)?['\"]"
    r"|\b(?:include_str|include_bytes)!\s*\([^\n)]*\.typ['\"]"
    r"|\btypst(?:_cli|_pdf|_svg)?::"
    r"|\bembedded[_ -]?typst\b"
    r")",
    re.IGNORECASE,
)
FORMTYPES_OPERATION_PATTERN = re.compile(
    r"(?:"
    r"\bformtypes_dir\b"
    r"|\bwith_formtypes_dir\b"
    r"|find_resource_dir\s*\(\s*['\"]formtypes['\"]"
    r"|(?:include_str|include_bytes)!\s*\([^\n)]*formtypes[/\\]"
    r"|['\"](?:\.\.?[/\\])*formtypes(?:[/\\]|['\"])"
    r"|[/\\]formtypes[/\\]"
    r")",
    re.IGNORECASE,
)
LEGACY_RENDERER_PATTERN = re.compile(
    r"(?:"
    r"\bPdfViewerView\b"
    r"|\bpdf_viewer\b"
    r"|\b(?:emit_|request_)?legacy_fallback\b"
    r"|\bfallback_from_experimental_html\b"
    r"|\bOpenLegacyFallback\b"
    r"|Open Legacy Preview"
    r"|\blegacy (?:PDF )?(?:preview|renderer)\b"
    r"|\brender_2551q_(?:print|flat|editable|fallback_pdf)\b"
    r")",
    re.IGNORECASE,
)
NODE_RUNTIME_PATTERN = re.compile(
    r"(?:"
    r"(?:cp|copy-item|copyfile|source\s*:|files?\s*=|resources?\s*=|"
    r"bundle|package|install)[^\n]{0,180}\b(?:node(?:\.exe)?|node_modules)\b"
    r"|\b(?:node(?:\.exe)?|node_modules)\b[^\n]{0,180}"
    r"(?:cp|copy-item|copyfile|destdir|destination|resources?|bundle|package)"
    r")",
    re.IGNORECASE,
)
DATA_IMAGE_PATTERN = re.compile(
    r"data:image/(?P<kind>png|jpeg|jpg|webp|gif|bmp|svg\+xml);base64,"
    r"(?P<payload>[A-Za-z0-9+/=]+)",
    re.IGNORECASE,
)
SVG_NUMBER = re.compile(r"^\s*([0-9]+(?:\.[0-9]+)?)")


@dataclass(frozen=True, order=True)
class Violation:
    category: str
    path: str
    line: int
    detail: str

    @property
    def location(self) -> str:
        return self.path if self.line <= 0 else f"{self.path}:{self.line}"


@dataclass(frozen=True)
class AuditResult:
    violations: tuple[Violation, ...]

    @property
    def passed(self) -> bool:
        return not self.violations

    def grouped(self) -> dict[str, tuple[Violation, ...]]:
        return {
            category: tuple(
                violation
                for violation in self.violations
                if violation.category == category
            )
            for category in CATEGORIES
        }


def _relative(path: Path, root: Path) -> PurePosixPath:
    return PurePosixPath(path.relative_to(root).as_posix())


def _is_relative_to(path: PurePosixPath, parent: PurePosixPath) -> bool:
    return path == parent or parent in path.parents


def _is_ignored(path: PurePosixPath) -> bool:
    if not path.parts:
        return False
    if path.parts[0] in IGNORED_TOP_LEVEL:
        return True
    if any(part in IGNORED_DIRECTORY_NAMES for part in path.parts[:-1]):
        return True
    if path.name in IGNORED_FILE_NAMES:
        return True
    if path.name.startswith("test_") or path.name.endswith((".test.ts", ".test.tsx")):
        return True
    return False


def _is_official_reference(path: PurePosixPath) -> bool:
    return any(_is_relative_to(path, root) for root in OFFICIAL_REFERENCE_ROOTS)


def _is_packaging_path(path: PurePosixPath) -> bool:
    if path in PACKAGING_FILES:
        return True
    return any(_is_relative_to(path, prefix) for prefix in PACKAGING_PREFIXES)


def _is_production_source(path: PurePosixPath) -> bool:
    if path.name in {"Cargo.toml", "build.rs"}:
        return True
    return any(_is_relative_to(path, root) for root in PRODUCTION_SOURCE_ROOTS)


def _iter_files(root: Path) -> Iterator[tuple[Path, PurePosixPath]]:
    for current_root, directory_names, file_names in os.walk(root):
        current = Path(current_root)
        current_relative = _relative(current, root)
        directory_names[:] = sorted(
            directory_name
            for directory_name in directory_names
            if not _is_ignored(current_relative / directory_name)
        )
        for file_name in sorted(file_names):
            path = current / file_name
            relative = _relative(path, root)
            if not _is_ignored(relative):
                yield path, relative


def _read_text(path: Path) -> str | None:
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return None


def _strip_rust_test_modules(text: str) -> str:
    """Blank ``#[cfg(test)]`` modules while preserving source line numbers."""

    lines = text.splitlines(keepends=True)
    result = list(lines)
    index = 0
    while index < len(lines):
        if not re.search(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", lines[index]):
            index += 1
            continue
        start = index
        depth = 0
        saw_open = False
        while index < len(lines):
            # This is deliberately a small lexical filter, not a Rust parser.
            # Test modules in this repository use conventional brace layout.
            depth += lines[index].count("{") - lines[index].count("}")
            saw_open = saw_open or "{" in lines[index]
            result[index] = "\n" if lines[index].endswith("\n") else ""
            index += 1
            if saw_open and depth <= 0:
                break
        if index == start:
            index += 1
    return "".join(result)


def _line_violations(
    category: str,
    relative: PurePosixPath,
    text: str,
    pattern: re.Pattern[str],
    detail: str,
) -> Iterator[Violation]:
    for line_number, line in enumerate(text.splitlines(), start=1):
        if pattern.search(line):
            yield Violation(category, relative.as_posix(), line_number, detail)


def _packaging_violations(
    relative: PurePosixPath, text: str
) -> Iterator[Violation]:
    if "typst" in relative.name.lower():
        yield Violation(
            "typst-packaging",
            relative.as_posix(),
            0,
            "Typst-specific packaging/install artifact",
        )
        # The path itself is the precise violation; reporting every internal
        # identifier in a dedicated installer/downloader only adds noise.
        return
    for line_number, line in enumerate(text.splitlines(), start=1):
        if re.search(r"\btypst(?:\.exe)?\b", line, re.IGNORECASE):
            yield Violation(
                "typst-packaging",
                relative.as_posix(),
                line_number,
                "Typst install, signing, CI, or package route",
            )
        if FORMTYPES_OPERATION_PATTERN.search(line):
            yield Violation(
                "runtime-formtypes",
                relative.as_posix(),
                line_number,
                "runtime formtypes install or package route",
            )
        build_time_node = re.search(
            r"(?:actions/setup-node|\binstall node\.js\b|\bnode-version\b)",
            line,
            re.IGNORECASE,
        )
        if not build_time_node and NODE_RUNTIME_PATTERN.search(line):
            yield Violation(
                "runtime-node",
                relative.as_posix(),
                line_number,
                "Node or node_modules copied into a production artifact",
            )


def _image_dimensions(payload: bytes, suffix: str) -> tuple[float, float] | None:
    suffix = suffix.lower()
    if suffix == ".png":
        if len(payload) >= 24 and payload[:8] == b"\x89PNG\r\n\x1a\n":
            width, height = struct.unpack(">II", payload[16:24])
            return float(width), float(height)
        return None
    if suffix in {".jpg", ".jpeg"} and payload.startswith(b"\xff\xd8"):
        offset = 2
        while offset + 4 <= len(payload):
            if payload[offset] != 0xFF:
                offset += 1
                continue
            marker = payload[offset + 1]
            offset += 2
            if marker in {0xD8, 0xD9} or 0xD0 <= marker <= 0xD7:
                continue
            if offset + 2 > len(payload):
                break
            length = struct.unpack(">H", payload[offset : offset + 2])[0]
            if length < 2 or offset + length > len(payload):
                break
            if marker in {
                0xC0,
                0xC1,
                0xC2,
                0xC3,
                0xC5,
                0xC6,
                0xC7,
                0xC9,
                0xCA,
                0xCB,
                0xCD,
                0xCE,
                0xCF,
            } and length >= 7:
                height, width = struct.unpack(">HH", payload[offset + 3 : offset + 7])
                return float(width), float(height)
            offset += length
        return None
    if suffix == ".gif" and len(payload) >= 10 and payload[:6] in {
        b"GIF87a",
        b"GIF89a",
    }:
        width, height = struct.unpack("<HH", payload[6:10])
        return float(width), float(height)
    if suffix == ".bmp" and len(payload) >= 26 and payload[:2] == b"BM":
        width, height = struct.unpack("<ii", payload[18:26])
        return float(abs(width)), float(abs(height))
    if (
        suffix == ".webp"
        and len(payload) >= 30
        and payload[:4] == b"RIFF"
        and payload[8:12] == b"WEBP"
    ):
        kind = payload[12:16]
        if kind == b"VP8X":
            width = int.from_bytes(payload[24:27], "little") + 1
            height = int.from_bytes(payload[27:30], "little") + 1
            return float(width), float(height)
        marker = payload.find(b"\x9d\x01\x2a")
        if marker >= 0 and marker + 7 <= len(payload):
            width, height = struct.unpack("<HH", payload[marker + 3 : marker + 7])
            return float(width & 0x3FFF), float(height & 0x3FFF)
        return None
    if suffix == ".svg":
        try:
            text = payload[:65536].decode("utf-8", errors="replace")
        except (UnicodeDecodeError, ValueError):
            return None
        view_box = re.search(
            r"\bviewBox\s*=\s*['\"]\s*[-+0-9.eE]+\s+[-+0-9.eE]+\s+"
            r"([-+0-9.eE]+)\s+([-+0-9.eE]+)\s*['\"]",
            text,
            re.IGNORECASE,
        )
        if view_box:
            try:
                return float(view_box.group(1)), float(view_box.group(2))
            except ValueError:
                return None
        width_match = re.search(r"\bwidth\s*=\s*['\"]([^'\"]+)", text, re.IGNORECASE)
        height_match = re.search(r"\bheight\s*=\s*['\"]([^'\"]+)", text, re.IGNORECASE)
        if width_match and height_match:
            width = SVG_NUMBER.match(width_match.group(1))
            height = SVG_NUMBER.match(height_match.group(1))
            if width and height:
                return float(width.group(1)), float(height.group(1))
    return None


def _looks_like_full_page(dimensions: tuple[float, float] | None) -> bool:
    if dimensions is None:
        return False
    width, height = dimensions
    if width < 500 or height < 700 or height <= width:
        return False
    ratio = width / height
    return 0.55 <= ratio <= 0.85


def _runtime_image_violations(
    path: Path, relative: PurePosixPath, text: str | None
) -> Iterator[Violation]:
    if _is_official_reference(relative):
        return
    if not any(_is_relative_to(relative, root) for root in RUNTIME_IMAGE_ROOTS):
        return

    suffix = relative.suffix.lower()
    if suffix in IMAGE_SUFFIXES:
        is_formtype_page = (
            relative.parts[0] == "formtypes"
            and ("pages" in relative.parts or relative.name.startswith("preview-"))
        )
        dimensions = None
        if suffix != ".pdf":
            try:
                dimensions = _image_dimensions(path.read_bytes(), suffix)
            except OSError:
                dimensions = None
        if is_formtype_page or suffix == ".pdf" or _looks_like_full_page(dimensions):
            yield Violation(
                "full-page-background",
                relative.as_posix(),
                0,
                "full-page raster/PDF/SVG is reachable from renderer runtime assets",
            )

    if text is None:
        return
    for match in DATA_IMAGE_PATTERN.finditer(text):
        try:
            payload = base64.b64decode(match.group("payload"), validate=True)
        except (binascii.Error, ValueError):
            continue
        kind = match.group("kind").lower()
        suffix_for_kind = ".svg" if kind == "svg+xml" else f".{kind}"
        if not _looks_like_full_page(_image_dimensions(payload, suffix_for_kind)):
            continue
        line_number = text.count("\n", 0, match.start()) + 1
        yield Violation(
            "full-page-background",
            relative.as_posix(),
            line_number,
            "embedded full-page image is reachable from renderer source",
        )


def audit_repository(root: Path) -> AuditResult:
    root = root.resolve()
    violations: set[Violation] = set()

    formtypes = root / "formtypes"
    if formtypes.is_dir():
        violations.add(
            Violation(
                "runtime-formtypes",
                "formtypes/",
                0,
                "top-level runtime formtypes directory exists",
            )
        )

    for path, relative in _iter_files(root):
        suffix = relative.suffix.lower()
        text = _read_text(path) if suffix in TEXT_SUFFIXES else None

        if relative.name.endswith(".typ") or ".typ." in relative.name:
            violations.add(
                Violation(
                    "typ-artifact",
                    relative.as_posix(),
                    0,
                    ".typ artifact exists outside ignored documentation/test paths",
                )
            )

        if _is_packaging_path(relative) and text is not None:
            violations.update(_packaging_violations(relative, text))

        if _is_production_source(relative) and text is not None:
            production_text = (
                _strip_rust_test_modules(text) if suffix == ".rs" else text
            )
            violations.update(
                _line_violations(
                    "typst-runtime",
                    relative,
                    production_text,
                    TYPST_RUNTIME_PATTERN,
                    "production source invokes or embeds Typst",
                )
            )
            if not _is_packaging_path(relative):
                violations.update(
                    _line_violations(
                        "runtime-formtypes",
                        relative,
                        production_text,
                        FORMTYPES_OPERATION_PATTERN,
                        "production source loads runtime formtypes",
                    )
                )
            violations.update(
                _line_violations(
                    "legacy-renderer",
                    relative,
                    production_text,
                    LEGACY_RENDERER_PATTERN,
                    "production source exposes the legacy viewer, route, API, "
                    "or fallback",
                )
            )

        violations.update(_runtime_image_violations(path, relative, text))

    ordered = tuple(
        sorted(
            violations,
            key=lambda item: (
                CATEGORIES.index(item.category),
                item.path,
                item.line,
                item.detail,
            ),
        )
    )
    return AuditResult(ordered)


def audit_package_directory(root: Path, label: str = "package") -> AuditResult:
    """Inspect an assembled application/package, not its build workspace.

    Unlike the repository audit, this scan never exempts ``node_modules`` or
    reference-looking paths: anything inside an assembled package is runtime
    material. The stable ``label`` keeps reports reproducible across runners.
    """

    root = root.resolve()
    violations: set[Violation] = set()
    if not root.is_dir():
        raise ValueError(f"package root does not exist or is not a directory: {root}")

    for current_root, directory_names, file_names in os.walk(root):
        current = Path(current_root)
        relative_directory = PurePosixPath(current.relative_to(root).as_posix())
        directory_names.sort()
        file_names.sort()
        for directory_name in directory_names:
            lowered = directory_name.lower()
            relative = relative_directory / directory_name
            display = f"{label}/{relative.as_posix()}"
            if lowered == "node_modules":
                violations.add(
                    Violation(
                        "runtime-node",
                        display + "/",
                        0,
                        "assembled package contains node_modules",
                    )
                )
            if lowered == "formtypes":
                violations.add(
                    Violation(
                        "runtime-formtypes",
                        display + "/",
                        0,
                        "assembled package contains a runtime formtypes directory",
                    )
                )

        for file_name in file_names:
            path = current / file_name
            relative = relative_directory / file_name
            display = f"{label}/{relative.as_posix()}"
            lowered = file_name.lower()
            suffix = path.suffix.lower()
            if lowered in {
                "node",
                "node.exe",
                "node.dll",
                "libnode.so",
                "libnode.dylib",
            }:
                violations.add(
                    Violation(
                        "runtime-node",
                        display,
                        0,
                        "assembled package contains a Node runtime binary",
                    )
                )
            if lowered in {"typst", "typst.exe"}:
                violations.add(
                    Violation(
                        "typst-packaging",
                        display,
                        0,
                        "assembled package contains a Typst binary",
                    )
                )
            if lowered.endswith(".typ") or ".typ." in lowered:
                violations.add(
                    Violation(
                        "typ-artifact",
                        display,
                        0,
                        "assembled package contains a .typ artifact",
                    )
                )

            payload: bytes | None = None
            if suffix in IMAGE_SUFFIXES:
                try:
                    payload = path.read_bytes()
                except OSError:
                    payload = None
                dimensions = (
                    _image_dimensions(payload, suffix)
                    if payload is not None and suffix != ".pdf"
                    else None
                )
                if suffix == ".pdf" or _looks_like_full_page(dimensions):
                    violations.add(
                        Violation(
                            "full-page-background",
                            display,
                            0,
                            "assembled package contains a full-page renderer asset",
                        )
                    )

            if suffix not in TEXT_SUFFIXES:
                continue
            text = _read_text(path)
            if text is None:
                continue
            if re.search(r"\btypst(?:\.exe)?\b", text, re.IGNORECASE):
                violations.add(
                    Violation(
                        "typst-runtime",
                        display,
                        0,
                        "assembled package text still references Typst",
                    )
                )
            if re.search(r"\bformtypes(?:[/\\]|\b)", text, re.IGNORECASE):
                violations.add(
                    Violation(
                        "runtime-formtypes",
                        display,
                        0,
                        "assembled package text still references runtime formtypes",
                    )
                )
            if LEGACY_RENDERER_PATTERN.search(text):
                violations.add(
                    Violation(
                        "legacy-renderer",
                        display,
                        0,
                        "assembled package contains a legacy viewer, route, or "
                        "fallback",
                    )
                )
            for match in DATA_IMAGE_PATTERN.finditer(text):
                try:
                    embedded = base64.b64decode(
                        match.group("payload"), validate=True
                    )
                except (binascii.Error, ValueError):
                    continue
                kind = match.group("kind").lower()
                embedded_suffix = ".svg" if kind == "svg+xml" else f".{kind}"
                if not _looks_like_full_page(
                    _image_dimensions(embedded, embedded_suffix)
                ):
                    continue
                violations.add(
                    Violation(
                        "full-page-background",
                        display,
                        0,
                        "assembled package embeds a full-page renderer image",
                    )
                )

    return _ordered_result(violations)


def _ordered_result(violations: Iterable[Violation]) -> AuditResult:
    return AuditResult(
        tuple(
            sorted(
                set(violations),
                key=lambda item: (
                    CATEGORIES.index(item.category),
                    item.path,
                    item.line,
                    item.detail,
                ),
            )
        )
    )


def combine_results(results: Iterable[AuditResult]) -> AuditResult:
    return _ordered_result(
        violation for result in results for violation in result.violations
    )


def format_report(result: AuditResult) -> str:
    if result.passed:
        return "No-legacy audit passed: production packages are HTML-only."
    lines = [
        f"No-legacy audit failed: {len(result.violations)} violation(s).",
    ]
    for category, violations in result.grouped().items():
        if not violations:
            continue
        lines.append("")
        lines.append(f"[{category}] {len(violations)}")
        for violation in violations:
            lines.append(f"- {violation.location}: {violation.detail}")
    return "\n".join(lines)


def _json_report(result: AuditResult) -> str:
    grouped = {
        category: [
            {
                "path": violation.path,
                "line": violation.line,
                "detail": violation.detail,
            }
            for violation in violations
        ]
        for category, violations in result.grouped().items()
        if violations
    }
    return json.dumps(
        {
            "passed": result.passed,
            "violation_count": len(result.violations),
            "categories": grouped,
        },
        indent=2,
        sort_keys=True,
    )


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root (defaults to the script's parent repository)",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="emit a deterministic machine-readable report",
    )
    parser.add_argument(
        "--package-root",
        type=Path,
        action="append",
        default=[],
        help="also inspect an assembled package directory (repeatable)",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        package_results = [
            audit_package_directory(package_root, f"package[{index}]")
            for index, package_root in enumerate(args.package_root)
        ]
    except ValueError as error:
        print(f"No-legacy audit configuration error: {error}", file=sys.stderr)
        return 2
    result = combine_results([audit_repository(args.root), *package_results])
    print(_json_report(result) if args.json else format_report(result))
    return 0 if result.passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
