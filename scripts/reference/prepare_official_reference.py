#!/usr/bin/env python3
"""Pin and render an official BIR PDF into calibration-only PNG references."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shutil
import struct
import subprocess
import tempfile
from pathlib import Path
from typing import Any


FORM_CODE_RE = re.compile(r"^[A-Za-z0-9]+$")
REVISION_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
PAGES_RE = re.compile(r"^Pages:\s+(\d+)\s*$", re.MULTILINE)
SIZE_RE = re.compile(r"^Page size:\s+([0-9.]+)\s+x\s+([0-9.]+)\s+pts", re.MULTILINE)


def identity(form_code: str, revision: str) -> tuple[str, str, str, str]:
    code = form_code.strip().upper()
    rev = revision.strip()
    if not FORM_CODE_RE.fullmatch(code):
        raise ValueError("form code must contain only letters and digits")
    if not REVISION_RE.fullmatch(rev):
        raise ValueError("revision contains unsupported characters")
    return code, rev, code.lower(), f"{code}v{rev}"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_tool(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        raise RuntimeError(f"required Poppler tool is missing: {name}")
    return path


def pdf_metadata(pdf: Path) -> tuple[int, float, float]:
    command = [require_tool("pdfinfo"), str(pdf)]
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    pages = PAGES_RE.search(result.stdout)
    size = SIZE_RE.search(result.stdout)
    if pages is None or size is None:
        raise RuntimeError("pdfinfo did not report page count and uniform page size")
    return int(pages.group(1)), float(size.group(1)), float(size.group(2))


def png_size(path: Path) -> tuple[int, int]:
    with path.open("rb") as handle:
        header = handle.read(24)
    if len(header) != 24 or header[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"not a valid PNG: {path}")
    return struct.unpack(">II", header[16:24])


def source_path(pdf: Path, root: Path) -> str:
    try:
        return pdf.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return f"external:{pdf.name}"


def resolve_from_root(value: str | None, root: Path, default: Path) -> Path:
    if value is None:
        return default.resolve()
    path = Path(value)
    return (path if path.is_absolute() else root / path).resolve()


def atomic_json(path: Path, value: Any) -> None:
    payload = json.dumps(value, indent=2, sort_keys=True) + "\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=path.parent, delete=False
    ) as handle:
        handle.write(payload)
        temp = Path(handle.name)
    os.replace(temp, path)


def install_reference(source: Path, destination: Path, replace: bool) -> None:
    if destination.exists():
        if sha256_file(source) == sha256_file(destination):
            return
        if not replace:
            raise RuntimeError(
                f"reference exists with different bytes: {destination}; pass --replace"
            )
    os.replace(source, destination)


def render_references(args: argparse.Namespace) -> dict[str, Any]:
    root = Path(args.repo).resolve()
    pdf = args.pdf.resolve()
    if not pdf.is_file():
        raise FileNotFoundError(f"official PDF not found: {pdf}")
    expected_digest = args.expected_sha256.lower()
    if not SHA256_RE.fullmatch(expected_digest):
        raise ValueError(
            "--expected-sha256 must be 64 lowercase hexadecimal characters"
        )
    actual_digest = sha256_file(pdf)
    if actual_digest != expected_digest:
        raise ValueError(
            f"official PDF SHA-256 mismatch: expected {expected_digest}, got {actual_digest}"
        )

    code, revision, slug, form_id = identity(args.form_code, args.revision)
    page_count, width_pt, height_pt = pdf_metadata(pdf)
    if args.page_width_pt is not None and (
        not math.isclose(width_pt, args.page_width_pt, abs_tol=0.25)
        or not math.isclose(height_pt, args.page_height_pt, abs_tol=0.25)
    ):
        raise ValueError(
            "official PDF page size mismatch: "
            f"expected {args.page_width_pt} x {args.page_height_pt} pt, "
            f"got {width_pt} x {height_pt} pt"
        )

    output_dir = resolve_from_root(
        args.output_dir, root, root / "packages/form-renderer/references"
    )
    if not args.check_only:
        output_dir.mkdir(parents=True, exist_ok=True)
    expected_width_px = round(width_pt * args.dpi / 72.0)
    expected_height_px = round(height_pt * args.dpi / 72.0)
    page_records: list[dict[str, Any]] = []

    temporary_parent = output_dir if output_dir.is_dir() else None
    with tempfile.TemporaryDirectory(
        prefix="official-ref-", dir=temporary_parent
    ) as directory:
        temp_dir = Path(directory)
        for page in range(1, page_count + 1):
            prefix = temp_dir / f"page-{page}"
            subprocess.run(
                [
                    require_tool("pdftoppm"),
                    "-f",
                    str(page),
                    "-l",
                    str(page),
                    "-singlefile",
                    "-r",
                    str(args.dpi),
                    "-png",
                    str(pdf),
                    str(prefix),
                ],
                check=True,
                capture_output=True,
            )
            rendered = prefix.with_suffix(".png")
            width_px, height_px = png_size(rendered)
            if (width_px, height_px) != (expected_width_px, expected_height_px):
                raise ValueError(
                    f"page {page} pixel size mismatch: expected "
                    f"{expected_width_px} x {expected_height_px}, "
                    f"got {width_px} x {height_px}"
                )
            name = f"{slug}-{revision.lower()}-page-{page}.png"
            destination = output_dir / name
            page_records.append(
                {
                    "page": page,
                    "reference_png": source_path(destination, root),
                    "reference_png_sha256": sha256_file(rendered),
                    "reference_width_px": width_px,
                    "reference_height_px": height_px,
                }
            )
            if not args.check_only:
                install_reference(rendered, destination, args.replace)

    record = {
        "schema_version": 1,
        "calibration_only": True,
        "runtime_background_allowed": False,
        "form": {
            "code": code,
            "revision": revision,
            "form_id": form_id,
            "official_source": args.source_url,
            "official_source_file": source_path(pdf, root),
            "official_source_sha256": actual_digest,
            "page_count": page_count,
            "page_width_pt": width_pt,
            "page_height_pt": height_pt,
            "dpi": args.dpi,
            "pages": page_records,
            "reference_provenance": {
                "kind": "official_pdf_raster",
                "replacement_required": False,
                "runtime_eligible": False,
            },
        },
        "generator": "prepare_official_reference.py (Poppler)",
    }
    if not args.check_only:
        manifest = resolve_from_root(
            args.manifest_out,
            root,
            output_dir / f"{slug}-{revision.lower()}-source.json",
        )
        atomic_json(manifest, record)
    return record


def self_test() -> None:
    code, revision, slug, form_id = identity("2551q", "2018")
    assert (code, revision, slug, form_id) == (
        "2551Q",
        "2018",
        "2551q",
        "2551Qv2018",
    )
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "one.png"
        path.write_bytes(
            b"\x89PNG\r\n\x1a\n"
            + b"\x00\x00\x00\rIHDR"
            + struct.pack(">II", 1224, 1872)
        )
        assert png_size(path) == (1224, 1872)
    print("prepare_official_reference.py self-test: ok")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=".")
    parser.add_argument("--form-code")
    parser.add_argument("--revision")
    parser.add_argument("--pdf", type=Path)
    parser.add_argument("--expected-sha256")
    parser.add_argument("--source-url", default=None)
    parser.add_argument("--dpi", type=int, default=144)
    parser.add_argument(
        "--page-width-pt",
        type=float,
        help="optional asserted width; omitted values are locked from pdfinfo",
    )
    parser.add_argument(
        "--page-height-pt",
        type=float,
        help="optional asserted height; omitted values are locked from pdfinfo",
    )
    parser.add_argument("--output-dir")
    parser.add_argument("--manifest-out")
    parser.add_argument("--replace", action="store_true")
    parser.add_argument("--check-only", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.dpi <= 0:
        parser.error("--dpi must be positive")
    if (args.page_width_pt is None) != (args.page_height_pt is None):
        parser.error("--page-width-pt and --page-height-pt must be supplied together")
    if args.page_width_pt is not None and (
        args.page_width_pt <= 0 or args.page_height_pt <= 0
    ):
        parser.error("asserted page geometry must be positive")
    required = (
        args.form_code,
        args.revision,
        args.pdf,
        args.expected_sha256,
        args.source_url,
    )
    if not args.self_test and not all(required):
        parser.error(
            "--form-code, --revision, --pdf, --expected-sha256, and --source-url are required"
        )
    return args


def main() -> int:
    args = parse_args()
    if args.self_test:
        self_test()
        return 0
    try:
        record = render_references(args)
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        raise SystemExit(f"reference preparation failed: {error}") from error
    print(json.dumps(record, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
