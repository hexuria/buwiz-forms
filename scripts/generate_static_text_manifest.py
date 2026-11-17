#!/usr/bin/env python3
"""Emit an ordered, exhaustive static-text manifest from a pinned official BIR PDF.

The manifest this script produces is the ground truth for
``static-text-exhaustive-v1`` (see
``docs/form-print-readiness/official-fidelity-criterion-v1.md`` section 3.2).

Why this exists
---------------
Every pixel component of the visual gate is blind to text content: replacing
each statutory tax rate on 2551Q page 2 with a wrong value measured a max
per-region regression of 0.19e-4 and violated no existing assertion. The
official PDFs do not embed their fonts, so text pixels encode the rasterizer's
substituted outlines and cannot carry a content proof. Content correctness
therefore has to be asserted against the PDF *text layer*, not its raster.

Provenance rule
---------------
The manifest is derived from the pinned official PDF and nothing else. It is
never scraped from our own DOM: a manifest read out of the renderer would
assert only that the renderer equals itself. The extracted strings are then
reviewed by a human before they are pinned into the TypeScript manifest.

Determinism
-----------
The output is a function of (pdf bytes, extraction constants, toolchain
versions) only. Ordering is total, floats are rounded to a fixed number of
decimals, and JSON is emitted with sorted keys. ``--check-only`` re-derives and
byte-compares, so any drift in the PDF or the toolchain fails closed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import unicodedata
from pathlib import Path
from typing import Any, Iterable, Sequence

GENERATOR_NAME = "scripts/generate_static_text_manifest.py"
GENERATOR_VERSION = "1.0.0"
MANIFEST_SCHEMA_VERSION = 1

FORM_CODE_RE = re.compile(r"^[A-Za-z0-9]+$")
REVISION_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")

# Extraction constants. Every one of these changes the emitted manifest, so
# they are recorded in the manifest itself and are part of what --check-only
# compares.
LINE_BAND_TOLERANCE_PT = 3.0
"""Two spans belong to the same visual line when their vertical centres are
within this many points of the band anchor. ~1/3 of an 8-9pt line box, which is
the entire body-text range on these forms."""

RUN_GAP_PT = 6.0
"""Maximum horizontal gap, in points, that still keeps two spans inside one
*run* (a contiguous printed phrase). Wider gaps mean the spans are separate
labels that merely share a line."""

SPACE_GAP_PT = 0.8
"""Below this gap two spans are concatenated with no separator. Official BIR
PDFs routinely split a single printed word across spans (``(TIN`` + ``)``);
inserting a space there would fabricate text that is not on the page.

This threshold only ever fires between spans the PDF placed on *different*
internal lines that happen to share a visual band. Word spacing inside a
printed line is carried by the span text itself (``'7'`` + ``' RDO Code '``),
which is why :func:`join_spans` concatenates the raw span text and normalizes
once at the end rather than normalizing per span. Normalizing first strips the
separator and silently welds ``7`` onto ``RDO Code``."""

COORD_DECIMALS = 2

# ---------------------------------------------------------------------------
# normalization -- MUST stay identical to normalizeStaticText() in
# packages/form-renderer/visual/official-2551q-static-text.ts
# ---------------------------------------------------------------------------

_NBSP_CLASS = re.compile("[   ]")
_WS_CLASS = re.compile(r"\s+")


def normalize_static_text(value: str) -> str:
    """NFC + NBSP folding + whitespace collapse, matching the TypeScript side."""
    folded = _NBSP_CLASS.sub(" ", unicodedata.normalize("NFC", value))
    return _WS_CLASS.sub(" ", folded).strip()


# ---------------------------------------------------------------------------
# helpers
# ---------------------------------------------------------------------------


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def identity(form_code: str, revision: str) -> tuple[str, str]:
    code = form_code.strip().upper()
    rev = revision.strip()
    if not FORM_CODE_RE.fullmatch(code):
        raise ValueError("form code must contain only letters and digits")
    if not REVISION_RE.fullmatch(rev):
        raise ValueError("revision contains unsupported characters")
    return code, rev


def require_pinned_pdf(pdf: Path, expected_sha256: str) -> str:
    """Fail closed unless the PDF on disk is byte-identical to the pin."""
    expected = expected_sha256.strip().lower()
    if not SHA256_RE.fullmatch(expected):
        raise ValueError("--expected-sha256 must be 64 lowercase hex characters")
    if not pdf.is_file():
        raise FileNotFoundError(f"official PDF is missing: {pdf}")
    actual = sha256_file(pdf)
    if actual != expected:
        raise ValueError(
            "official PDF does not match the pin\n"
            f"  path:     {pdf}\n"
            f"  expected: {expected}\n"
            f"  actual:   {actual}"
        )
    return actual


def source_path(pdf: Path, root: Path) -> str:
    try:
        return pdf.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return f"external:{pdf.name}"


def round_pt(value: float) -> float:
    rounded = round(float(value), COORD_DECIMALS)
    # Normalize -0.0 so the JSON is stable across platforms.
    return rounded + 0.0


# ---------------------------------------------------------------------------
# extraction
# ---------------------------------------------------------------------------


def import_pymupdf():
    try:
        import fitz  # type: ignore
    except ImportError as error:  # pragma: no cover - environment dependent
        raise RuntimeError(
            "PyMuPDF is required: pip install pymupdf"
        ) from error
    return fitz


def collect_spans(page: Any) -> list[dict[str, Any]]:
    """Flatten a page to normalized, non-empty spans.

    PyMuPDF's block/line grouping is not reading order and routinely splits one
    printed line across blocks (on 2551Q page 1, ``For BIR`` and ``BCS/`` share
    a printed row but live in different blocks), so the structure is discarded
    here and rebuilt geometrically below.
    """
    spans: list[dict[str, Any]] = []
    for block in page.get_text("dict")["blocks"]:
        if block.get("type") != 0:
            continue
        for line in block.get("lines", []):
            for span in line.get("spans", []):
                raw = _NBSP_CLASS.sub(
                    " ", unicodedata.normalize("NFC", span.get("text", ""))
                )
                text = normalize_static_text(raw)
                if not text:
                    # Whitespace-only spans carry no content. Their width is
                    # still reflected in the gap between the surrounding
                    # content spans, so dropping them cannot lose a separator.
                    continue
                x0, y0, x1, y1 = span["bbox"]
                spans.append(
                    {
                        "raw": raw,
                        "text": text,
                        "x0": float(x0),
                        "y0": float(y0),
                        "x1": float(x1),
                        "y1": float(y1),
                        "font": str(span.get("font", "")),
                        "size": round(float(span.get("size", 0.0)), 2),
                    }
                )
    return spans


def band_lines(spans: Sequence[dict[str, Any]]) -> list[list[dict[str, Any]]]:
    """Group spans into visual lines, then order each line left to right.

    The band anchor is the first span of the band, never a running mean, so the
    grouping cannot drift and is a pure function of the sorted input.
    """
    ordered = sorted(
        spans,
        key=lambda s: (
            round((s["y0"] + s["y1"]) / 2, 4),
            round(s["x0"], 4),
            s["text"],
        ),
    )
    lines: list[list[dict[str, Any]]] = []
    anchor: float | None = None
    for span in ordered:
        centre = (span["y0"] + span["y1"]) / 2
        if anchor is None or centre - anchor > LINE_BAND_TOLERANCE_PT:
            anchor = centre
            lines.append([span])
        else:
            lines[-1].append(span)
    for line in lines:
        line.sort(key=lambda s: (round(s["x0"], 4), s["text"]))
    return lines


def join_spans(spans: Sequence[dict[str, Any]]) -> str:
    """Reassemble printed text from spans, honouring the real horizontal gaps."""
    parts: list[str] = []
    previous_x1: float | None = None
    for span in spans:
        if previous_x1 is not None and span["x0"] - previous_x1 >= SPACE_GAP_PT:
            parts.append(" ")
        parts.append(span["raw"])
        previous_x1 = span["x1"]
    return normalize_static_text("".join(parts))


def split_runs(line: Sequence[dict[str, Any]]) -> list[list[dict[str, Any]]]:
    """Split one visual line into contiguous printed phrases."""
    runs: list[list[dict[str, Any]]] = []
    previous_x1: float | None = None
    for span in line:
        if previous_x1 is None or span["x0"] - previous_x1 > RUN_GAP_PT:
            runs.append([span])
        else:
            runs[-1].append(span)
        previous_x1 = span["x1"]
    return runs


def extract_pages(pdf: Path) -> tuple[list[dict[str, Any]], dict[str, str]]:
    fitz = import_pymupdf()
    toolchain = {
        "backend": "pymupdf",
        "pymupdf_version": str(getattr(fitz, "VersionBind", "")),
        "mupdf_version": str(getattr(fitz, "VersionFitz", "")),
    }
    pages: list[dict[str, Any]] = []
    line_order = 0
    run_order = 0
    with fitz.open(pdf) as document:
        for page_index in range(document.page_count):
            page = document[page_index]
            lines_out: list[dict[str, Any]] = []
            for line in band_lines(collect_spans(page)):
                line_order += 1
                runs_out: list[dict[str, Any]] = []
                for run in split_runs(line):
                    run_order += 1
                    runs_out.append(
                        {
                            "order": run_order,
                            "text": join_spans(run),
                            "x": round_pt(run[0]["x0"]),
                            "y": round_pt(min(s["y0"] for s in run)),
                            "width": round_pt(run[-1]["x1"] - run[0]["x0"]),
                            "font": run[0]["font"],
                            "size": run[0]["size"],
                            "span_count": len(run),
                        }
                    )
                lines_out.append(
                    {
                        "order": line_order,
                        "text": join_spans(line),
                        "y": round_pt(
                            sum((s["y0"] + s["y1"]) / 2 for s in line) / len(line)
                        ),
                        "runs": runs_out,
                    }
                )
            pages.append(
                {
                    "page": page_index + 1,
                    "width": round_pt(page.rect.width),
                    "height": round_pt(page.rect.height),
                    "lines": lines_out,
                    "text": normalize_static_text(
                        " ".join(line["text"] for line in lines_out)
                    ),
                }
            )
    return pages, toolchain


# ---------------------------------------------------------------------------
# manifest
# ---------------------------------------------------------------------------


def build_manifest(
    *,
    repo: Path,
    form_code: str,
    revision: str,
    pdf: Path,
    sha256: str,
) -> dict[str, Any]:
    pages, toolchain = extract_pages(pdf)
    return {
        "schema_version": MANIFEST_SCHEMA_VERSION,
        "criterion": "static-text-exhaustive-v1",
        "generator": GENERATOR_NAME,
        "generator_version": GENERATOR_VERSION,
        "form": {"code": form_code, "revision": revision},
        "official_source": {
            "path": source_path(pdf, repo),
            "sha256": sha256,
        },
        "extraction": {
            **toolchain,
            "line_band_tolerance_pt": LINE_BAND_TOLERANCE_PT,
            "run_gap_pt": RUN_GAP_PT,
            "space_gap_pt": SPACE_GAP_PT,
            "coord_decimals": COORD_DECIMALS,
            "normalization": "NFC + NBSP fold + whitespace collapse (matches normalizeStaticText)",
        },
        "totals": {
            "pages": len(pages),
            "lines": sum(len(page["lines"]) for page in pages),
            "runs": sum(len(line["runs"]) for page in pages for line in page["lines"]),
        },
        "pages": pages,
        "provenance_note": (
            "Derived from the pinned official PDF text layer only. Never scraped "
            "from the renderer DOM: a DOM-derived manifest would assert only that "
            "the renderer equals itself."
        ),
    }


def serialize(manifest: dict[str, Any]) -> str:
    return json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


# ---------------------------------------------------------------------------
# inventory validation
# ---------------------------------------------------------------------------


def inventory_strings(inventory: dict[str, Any]) -> list[tuple[str, str]]:
    """Every curated string in a hand-authored inventory, with a source label."""
    out: list[tuple[str, str]] = []
    for region in inventory.get("regions", []):
        out.append(("regions", region["text"]))
    for row in inventory.get("atc_rows", []):
        for key in ("code", "description", "rate"):
            if isinstance(row, dict) and key in row:
                out.append((f"atc_rows.{key}", str(row[key])))
        if isinstance(row, str):
            out.append(("atc_rows", row))
    for row in inventory.get("tax_type_rows", []):
        for key in ("code", "description"):
            if isinstance(row, dict) and key in row:
                out.append((f"tax_type_rows.{key}", str(row[key])))
        if isinstance(row, str):
            out.append(("tax_type_rows", row))
    return out


def validate_against_inventory(
    manifest: dict[str, Any], inventory_path: Path
) -> dict[str, Any]:
    inventory = json.loads(inventory_path.read_text(encoding="utf-8"))
    pinned = str(inventory.get("official_source_sha256", "")).lower()
    actual = manifest["official_source"]["sha256"]
    document_text = normalize_static_text(
        " ".join(page["text"] for page in manifest["pages"])
    )
    run_texts = {
        run["text"]
        for page in manifest["pages"]
        for line in page["lines"]
        for run in line["runs"]
    }
    line_texts = {
        line["text"] for page in manifest["pages"] for line in page["lines"]
    }

    document_tokens = set(document_text.split(" "))

    curated = inventory_strings(inventory)
    reproduced: list[dict[str, str]] = []
    missing: list[dict[str, Any]] = []
    for source, raw in curated:
        text = normalize_static_text(raw)
        if not text:
            continue
        if text in run_texts:
            level = "run"
        elif text in line_texts:
            level = "line"
        elif text in document_text:
            level = "stream"
        else:
            # Distinguish "the extractor lost text" from "the extractor
            # linearized a two-column label differently than the DOM does".
            # Only the first is an extractor defect.
            absent = sorted({t for t in text.split(" ") if t not in document_tokens})
            missing.append(
                {
                    "source": source,
                    "text": text,
                    "all_tokens_present": not absent,
                    "absent_tokens": absent,
                }
            )
            continue
        reproduced.append({"source": source, "text": text, "level": level})

    total = len(reproduced) + len(missing)
    fragmented = sum(1 for item in missing if item["all_tokens_present"])
    return {
        "inventory": inventory_path.name,
        "pdf_sha256_matches_inventory_pin": pinned == actual,
        "curated_entries": total,
        "reproduced": len(reproduced),
        "missing": len(missing),
        "reproduction_rate": round(len(reproduced) / total, 4) if total else None,
        "missing_but_fully_present_as_fragments": fragmented,
        "missing_with_absent_tokens": len(missing) - fragmented,
        "content_coverage_rate": (
            round((len(reproduced) + fragmented) / total, 4) if total else None
        ),
        "reproduced_by_level": {
            level: sum(1 for item in reproduced if item["level"] == level)
            for level in ("run", "line", "stream")
        },
        "extractor_runs": len(run_texts),
        "extractor_lines": len(line_texts),
        "missing_entries": missing,
    }


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Emit an ordered, exhaustive static-text manifest from a pinned "
            "official BIR PDF."
        )
    )
    parser.add_argument("--repo", default=".", help="repository root (default: .)")
    parser.add_argument("--form-code", required=True, help="e.g. 2551Q")
    parser.add_argument("--revision", required=True, help="e.g. 2018")
    parser.add_argument("--pdf", required=True, help="path to the pinned official PDF")
    parser.add_argument(
        "--expected-sha256",
        required=True,
        help="pinned sha256 of the official PDF; a mismatch fails closed",
    )
    parser.add_argument(
        "--output",
        default=None,
        help=(
            "manifest path (default: "
            "packages/form-renderer/references/<code>-<revision>-official-static-text.json)"
        ),
    )
    parser.add_argument(
        "--check-only",
        action="store_true",
        help="re-derive and compare against --output instead of writing it",
    )
    parser.add_argument(
        "--validate-inventory",
        action="append",
        default=[],
        metavar="PATH",
        help=(
            "hand-authored *-static-text-inventory.json to measure the extractor "
            "against; may be repeated. Writes a report to stdout and does not "
            "write a manifest."
        ),
    )
    parser.add_argument(
        "--report-missing",
        action="store_true",
        help="include every unreproduced curated string in the validation report",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    repo = Path(args.repo).resolve()
    code, revision = identity(args.form_code, args.revision)
    pdf = Path(args.pdf)
    sha256 = require_pinned_pdf(pdf, args.expected_sha256)
    manifest = build_manifest(
        repo=repo, form_code=code, revision=revision, pdf=pdf, sha256=sha256
    )

    if args.validate_inventory:
        reports = [
            validate_against_inventory(manifest, Path(path))
            for path in args.validate_inventory
        ]
        if not args.report_missing:
            for report in reports:
                report.pop("missing_entries", None)
        print(json.dumps({"validation": reports}, indent=2, ensure_ascii=False))
        return 0

    default_output = (
        repo
        / "packages/form-renderer/references"
        / f"{code.lower()}-{revision}-official-static-text.json"
    )
    output = Path(args.output).resolve() if args.output else default_output
    payload = serialize(manifest)

    if args.check_only:
        if not output.is_file():
            print(f"missing manifest: {output}", file=sys.stderr)
            return 1
        if output.read_text(encoding="utf-8") != payload:
            print(
                f"manifest is out of date: {output}\n"
                "re-run without --check-only to regenerate",
                file=sys.stderr,
            )
            return 1
        print(f"manifest up to date: {output}")
        return 0

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(payload, encoding="utf-8")
    print(
        json.dumps(
            {
                "output": source_path(output, repo),
                "form": manifest["form"],
                "official_source_sha256": sha256,
                "totals": manifest["totals"],
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, RuntimeError, FileNotFoundError) as error:
        # Fail closed with a readable message rather than a traceback: the pin
        # mismatch path is the one a reviewer sees most often.
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(2) from None
