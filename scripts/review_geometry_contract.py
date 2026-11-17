#!/usr/bin/env python3
"""Render review overlays for a geometry contract, for mandatory human sign-off.

WHY THIS EXISTS. extract_geometry_contract.py generates candidates, not truth.
Comb detection in particular is heuristic: on 2316 the spike found 28-29 of 29
combs while also emitting 3-4 over-wide false positives, and no threshold makes
that number 29/29 without inventing agreement. A candidate that nobody looked
at is not evidence, so this draws every detected feature over a 144-DPI raster
of the official page and produces a checklist the reviewer fills in.

Nothing here promotes anything. The overlays are review artifacts; acceptance is
recorded by a human, and semantic naming - which comb is a TIN and which is a
ZIP - is never inferred by either script.

The rasters this writes are calibration-only, exactly like every other official
raster in this repository. They must not become runtime assets or backgrounds.

Colour key drawn on each page:
  red      comb candidate (container detector) + its interior divider ticks
  orange   comb candidate (tick-run detector only; no container resolved)
  blue     checkbox candidate
  green    horizontal rule, thickness-scaled
  purple   vertical rule
  cyan     image bbox
  magenta  sub-pixel rule (cannot raster darker than mid-grey)
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Sequence

try:
    import fitz  # PyMuPDF
except ImportError:  # pragma: no cover - environment guard
    print("PyMuPDF is required: pip install pymupdf", file=sys.stderr)
    raise SystemExit(2)


SCHEMA = "bir-geometry-contract/draft-0"

RED = (0.86, 0.15, 0.15)
ORANGE = (0.95, 0.55, 0.10)
BLUE = (0.10, 0.35, 0.90)
GREEN = (0.10, 0.60, 0.25)
PURPLE = (0.50, 0.20, 0.70)
CYAN = (0.05, 0.65, 0.75)
MAGENTA = (0.90, 0.10, 0.70)


def draw_page(
    page: "fitz.Page",
    contract_page: dict[str, Any],
    layers: Sequence[str],
) -> None:
    shape = page.new_shape()

    if "rules" in layers:
        for rule in contract_page["rules_h"]:
            colour = MAGENTA if rule["subpixel"] else GREEN
            shape.draw_line(
                fitz.Point(rule["start_pt"], rule["pos_pt"]),
                fitz.Point(rule["end_pt"], rule["pos_pt"]),
            )
            shape.finish(color=colour, width=0.4)
        for rule in contract_page["rules_v"]:
            colour = MAGENTA if rule["subpixel"] else PURPLE
            shape.draw_line(
                fitz.Point(rule["pos_pt"], rule["start_pt"]),
                fitz.Point(rule["pos_pt"], rule["end_pt"]),
            )
            shape.finish(color=colour, width=0.4)

    if "images" in layers:
        for image in contract_page["images"]:
            x0, y0, x1, y1 = image["bbox_pt"]
            shape.draw_rect(fitz.Rect(x0, y0, x1, y1))
            shape.finish(color=CYAN, width=0.9)

    if "checkboxes" in layers:
        for box in contract_page["checkbox_candidates"]:
            shape.draw_rect(
                fitz.Rect(
                    box["x0_pt"] - 1.2,
                    box["y0_pt"] - 1.2,
                    box["x1_pt"] + 1.2,
                    box["y1_pt"] + 1.2,
                )
            )
            shape.finish(color=BLUE, width=0.8)

    if "combs" in layers:
        # Tick-run candidates first, so container candidates draw on top: where
        # both agree the reviewer sees red, where only the weaker detector fired
        # the orange stays visible and needs a closer look.
        for comb in contract_page["comb_candidates_tickrun"]:
            shape.draw_rect(
                fitz.Rect(
                    comb["x_start_pt"] - 1.5,
                    comb["y0_pt"] - 1.5,
                    comb["x_end_pt"] + 1.5,
                    comb["y1_pt"] + 1.5,
                )
            )
            shape.finish(color=ORANGE, width=0.7)

        for comb in contract_page["comb_candidates_container"]:
            shape.draw_rect(
                fitz.Rect(comb["x0_pt"], comb["y0_pt"], comb["x1_pt"], comb["y1_pt"])
            )
            shape.finish(color=RED, width=1.0)
            # Mark each interior divider so a miscount is visible, not implied.
            cells = comb["cells"]
            if cells > 1:
                span = comb["x1_pt"] - comb["x0_pt"]
                for index in range(1, cells):
                    x = comb["x0_pt"] + span * index / cells
                    shape.draw_line(
                        fitz.Point(x, comb["y1_pt"]),
                        fitz.Point(x, comb["y1_pt"] + 2.5),
                    )
                shape.finish(color=RED, width=0.5)

    shape.commit()


def checklist(contract: dict[str, Any]) -> str:
    lines = [
        f"# Geometry contract review - {contract['form_key']}",
        "",
        f"Source: {contract['source']['name']}",
        f"SHA-256: {contract['source']['sha256']}",
        f"Extractor: {contract['extractor']['version']} "
        f"(PyMuPDF {contract['extractor']['pymupdf']})",
        f"Coalescing gap tolerance: {contract['coalescing']['selected_gap_tol_pt']}pt "
        f"(plateau length {contract['coalescing']['plateau_length']})",
        "",
        "Candidates below are UNREVIEWED. Mark each accept or reject against the",
        "overlay PNG and the pinned PDF. Detection is not an oracle: over-wide",
        "comb boxes and lattice-implied checkboxes are the known failure modes.",
        "Semantic naming is not proposed here and must be supplied by hand.",
        "",
    ]
    for page in contract["pages"]:
        lines.append(f"## Page {page['page'] + 1}")
        lines.append("")
        counts = page["counts"]
        lines.append(
            f"Rules: {counts['rules_h']}h / {counts['rules_v']}v coalesced from "
            f"{counts['raw_segments']} raw segments. "
            f"Weights (device px at 144 DPI): {page['rule_weight_histogram_device_px']}"
        )
        lines.append("")
        lines.append("### Comb candidates (container detector)")
        lines.append("")
        lines.append("| accept? | x0..x1 px | y0..y1 px | cells | pitch px | divider px | regular | container |")
        lines.append("| --- | --- | --- | --- | --- | --- | --- | --- |")
        for comb in page["comb_candidates_container"]:
            lines.append(
                f"|  | {comb['x0_px']:.1f}..{comb['x1_px']:.1f} "
                f"| {comb['y0_px']:.1f}..{comb['y1_px']:.1f} "
                f"| {comb['cells']} | {comb['pitch_px']:.2f} "
                f"| {comb['divider_thickness_px']:.2f}"
                f"{' (sub-px)' if comb['divider_subpixel'] else ''} "
                f"| {'yes' if comb['regular'] else 'NO'} "
                f"| {comb['container_source']} |"
            )
        lines.append("")
        lines.append("### Checkbox candidates")
        lines.append("")
        lines.append("| accept? | x0..x1 px | y0..y1 px | size pt | source |")
        lines.append("| --- | --- | --- | --- | --- |")
        for box in page["checkbox_candidates"]:
            lines.append(
                f"|  | {box['x0_px']:.1f}..{box['x1_px']:.1f} "
                f"| {box['y0_px']:.1f}..{box['y1_px']:.1f} "
                f"| {box['width_pt']:.2f}x{box['height_pt']:.2f} | {box['source']} |"
            )
        lines.append("")
    return "\n".join(lines) + "\n"


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Render review overlays for a geometry contract.",
    )
    parser.add_argument("--repo", default=".", help="repository root")
    parser.add_argument(
        "--contract", required=True, type=Path, help="path to contract.json"
    )
    parser.add_argument("--pdf", required=True, type=Path, help="pinned official PDF")
    parser.add_argument(
        "--output", "--out", dest="output", required=True, type=Path,
        help="directory for overlay PNGs and the review checklist",
    )
    parser.add_argument("--dpi", type=int, default=144, help="raster DPI (default 144)")
    parser.add_argument(
        "--layers",
        default="combs,checkboxes,rules,images",
        help="comma-separated subset of combs,checkboxes,rules,images",
    )
    args = parser.parse_args(argv)

    repo = Path(args.repo).resolve()
    contract_path = (
        args.contract if args.contract.is_absolute() else repo / args.contract
    ).resolve()
    if not contract_path.is_file():
        print(f"contract not found: {contract_path}", file=sys.stderr)
        return 2

    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    if contract.get("schema") != SCHEMA:
        print(
            f"unsupported contract schema: {contract.get('schema')!r} (want {SCHEMA})",
            file=sys.stderr,
        )
        return 2

    pdf = args.pdf.resolve()
    if not pdf.is_file():
        print(f"official PDF not found: {pdf}", file=sys.stderr)
        return 2

    # The overlay is only meaningful against the exact bytes the contract came
    # from, so re-verify rather than trusting the filename.
    import hashlib

    digest = hashlib.sha256()
    with pdf.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    if digest.hexdigest() != contract["source"]["sha256"]:
        print(
            "PDF does not match the contract's pinned SHA-256; refusing to render",
            file=sys.stderr,
        )
        return 1

    layers = [layer.strip() for layer in args.layers.split(",") if layer.strip()]
    output = (args.output if args.output.is_absolute() else repo / args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)

    doc = fitz.open(pdf)
    written = []
    for contract_page in contract["pages"]:
        page = doc[contract_page["page"]]
        draw_page(page, contract_page, layers)
        pixmap = page.get_pixmap(dpi=args.dpi)
        name = (
            f"{contract['form_code'].lower()}-{contract['revision']}"
            f"-page-{contract_page['page'] + 1}-review.png"
        )
        target = output / name
        pixmap.save(target)
        written.append(str(target))
    doc.close()

    checklist_path = output / "REVIEW.md"
    checklist_path.write_text(checklist(contract), encoding="utf-8")
    written.append(str(checklist_path))

    print(
        json.dumps(
            {
                "form_key": contract["form_key"],
                "dpi": args.dpi,
                "layers": layers,
                "written": written,
                "status": "UNREVIEWED - human accept/reject required",
            },
            indent=1,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
