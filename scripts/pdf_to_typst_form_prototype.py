#!/usr/bin/env python3
"""
Prototype: convert an official BIR PDF into a Typst-backed form.

This intentionally does not use OCR for the static form. It asks PyMuPDF to
convert each PDF page into SVG paths/images, then creates a Typst document that
uses those SVGs as exact page backgrounds. Dynamic eBIR fields are rendered as
Typst foreground content.

Install:
  python3 -m pip install --user pymupdf
  cargo install typst-cli --version 0.13.1 --locked

Example:
  python3 scripts/pdf_to_typst_form_prototype.py \
    --pdf "https://bir-cdn.bir.gov.ph/local/pdf/2551Q%20Jan%202018%20ENCS%20final%20rev%203_copy.pdf" \
    --xml ../bir-analyze/fixed.xml \
    --out /tmp/bir-typst-2551q \
    --compile
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

import fitz


DEFAULT_2551Q_URL = (
    "https://bir-cdn.bir.gov.ph/local/pdf/"
    "2551Q%20Jan%202018%20ENCS%20final%20rev%203_copy.pdf"
)


@dataclass(frozen=True)
class FieldSpec:
    key: str
    kind: str
    page: int
    x: float
    y: float
    cell_w: float | None = None
    int_cells: int | None = None
    dec_x: float | None = None
    size: float = 8.5


# First calibrated-enough sample map for proving the pipeline. This is not the
# final production map; the point is that field placement is data, not renderer
# code. Coordinates are PDF/Typst points from the top-left of the 612 x 936 page.
FIELD_MAP_2551Q: list[FieldSpec] = [
    FieldSpec("frm2551Qv2018:forThe_1", "checkbox", 0, 87, 108),
    FieldSpec("frm2551Qv2018:forThe_2", "checkbox", 0, 159, 108),
    FieldSpec("__year_ended", "cells", 0, 135, 129, cell_w=14.0),
    FieldSpec("frm2551Qv2018:qtr_1", "checkbox", 0, 241, 124),
    FieldSpec("frm2551Qv2018:qtr_2", "checkbox", 0, 284, 124),
    FieldSpec("frm2551Qv2018:qtr_3", "checkbox", 0, 327, 124),
    FieldSpec("frm2551Qv2018:qtr_4", "checkbox", 0, 370, 124),
    FieldSpec("frm2551Qv2018:amendedRtn_1", "checkbox", 0, 425, 124),
    FieldSpec("frm2551Qv2018:amendedRtn_2", "checkbox", 0, 463, 124),
    FieldSpec("frm2551Qv2018:txtSheets", "cells", 0, 565, 124, cell_w=14.0),
    FieldSpec("frm2551Qv2018:txtTIN1", "cells", 0, 220, 164, cell_w=14.1),
    FieldSpec("frm2551Qv2018:txtTIN2", "cells", 0, 284, 164, cell_w=14.1),
    FieldSpec("frm2551Qv2018:txtTIN3", "cells", 0, 348, 164, cell_w=14.1),
    FieldSpec("frm2551Qv2018:txtBranchCode", "cells", 0, 410, 164, cell_w=14.1),
    FieldSpec("frm2551Qv2018:txtRDOCode", "cells", 0, 548, 164, cell_w=14.1),
    FieldSpec("frm2551Qv2018:registeredName", "text", 0, 33, 186, size=8.5),
    FieldSpec("frm2551Qv2018:registeredAddress", "text", 0, 33, 219, size=7.5),
    FieldSpec("frm2551Qv2018:zipCode", "cells", 0, 542, 244, cell_w=14.1),
    FieldSpec("frm2551Qv2018:telNo", "text", 0, 33, 278, size=8.5),
    FieldSpec("txtEmail", "text", 0, 200, 278, size=8.5),
    FieldSpec("frm2551Qv2018:taxTreaty_1", "checkbox", 0, 195, 292),
    FieldSpec("frm2551Qv2018:taxTreaty_2", "checkbox", 0, 246, 292),
    FieldSpec("frm2551Qv2018:taxRate1", "checkbox", 0, 178, 334),
    FieldSpec("frm2551Qv2018:taxRate2", "checkbox", 0, 348, 334),
    FieldSpec("frm2551Qv2018:txt14", "amount", 0, 384, 376, cell_w=14, int_cells=11, dec_x=553),
    FieldSpec("frm2551Qv2018:txt15", "amount", 0, 384, 408, cell_w=14, int_cells=11, dec_x=553),
    FieldSpec("frm2551Qv2018:txt16", "amount", 0, 384, 427, cell_w=14, int_cells=11, dec_x=553),
    FieldSpec("frm2551Qv2018:txt17", "amount", 0, 384, 446, cell_w=14, int_cells=11, dec_x=553),
    FieldSpec("frm2551Qv2018:txt18", "amount", 0, 384, 464, cell_w=14, int_cells=11, dec_x=553),
    FieldSpec("frm2551Qv2018:txt19", "amount", 0, 384, 482, cell_w=14, int_cells=11, dec_x=553),
    FieldSpec("frm2551Qv2018:txt20", "amount", 0, 384, 519, cell_w=14, int_cells=11, dec_x=553),
    FieldSpec("frm2551Qv2018:txt21", "amount", 0, 384, 537, cell_w=14, int_cells=11, dec_x=553),
    FieldSpec("frm2551Qv2018:txt22", "amount", 0, 384, 556, cell_w=14, int_cells=11, dec_x=553),
    FieldSpec("frm2551Qv2018:txt23", "amount", 0, 384, 574, cell_w=14, int_cells=11, dec_x=553),
    FieldSpec("frm2551Qv2018:txt24", "amount", 0, 384, 593, cell_w=14, int_cells=11, dec_x=553),
    FieldSpec("frm2551Qv2018:txtPg2TIN1", "cells", 1, 25, 113, cell_w=14.1),
    FieldSpec("frm2551Qv2018:txtPg2TIN2", "cells", 1, 67, 113, cell_w=14.1),
    FieldSpec("frm2551Qv2018:txtPg2TIN3", "cells", 1, 109, 113, cell_w=14.1),
    FieldSpec("frm2551Qv2018:txtPg2BranchCode", "cells", 1, 151, 113, cell_w=14.1),
    FieldSpec("frm2551Qv2018:txtPg2TaxpayerName", "text", 1, 225, 113, size=8.5),
    FieldSpec("drpATC1", "text", 1, 52, 168, size=8.5),
    FieldSpec("txtATCAmt1", "amount", 1, 272, 168, cell_w=14, int_cells=11, dec_x=437),
    FieldSpec("txtATCRate1", "text", 1, 337, 168, size=8.5),
    FieldSpec("txtATCDue1", "amount", 1, 512, 168, cell_w=10.5, int_cells=7, dec_x=586),
    FieldSpec("txtTotalSched1", "amount", 1, 512, 284, cell_w=10.5, int_cells=7, dec_x=586),
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pdf", default=DEFAULT_2551Q_URL)
    parser.add_argument("--xml")
    parser.add_argument("--out", required=True)
    parser.add_argument("--compile", action="store_true")
    parser.add_argument("--render-png", action="store_true")
    return parser.parse_args()


def fetch_pdf(source: str, out_dir: Path) -> Path:
    target = out_dir / "source.pdf"
    if source.startswith(("http://", "https://")):
        urllib.request.urlretrieve(source, target)
        return target
    shutil.copyfile(source, target)
    return target


def parse_ebir_xml(path: str | None) -> dict[str, str]:
    if not path:
        return {}
    text = Path(path).read_text(errors="replace")
    values: dict[str, str] = {}
    for body in re.findall(r"<div>(.*?)</div>", text, flags=re.S):
        body = body.strip()
        match = re.match(r"(.+?)=(.*?)\1=$", body)
        if match:
            key, value = match.groups()
            values[key] = urllib_unquote(value)
    if values.get("frm2551Qv2018:rtnMonth") or values.get("frm2551Qv2018:txtYear"):
        values["__year_ended"] = (
            values.get("frm2551Qv2018:rtnMonth", "").zfill(2)
            + values.get("frm2551Qv2018:txtYear", "")
        )
    normalize_2551q_values(values)
    return values


def normalize_2551q_values(values: dict[str, str]) -> None:
    # eBIRForms stores ATC dropdown selection indices. The printable form needs
    # the visible ATC code. This partial map proves the pattern; production
    # should source it from the schema/ATC table.
    atc_index = {
        "1": "PT010",
        "2": "PT040",
        "3": "PT041",
        "4": "PT060",
        "5": "PT070",
        "6": "PT090",
    }
    for i in range(1, 7):
        key = f"drpATC{i}"
        if values.get(key) in atc_index:
            values[key] = atc_index[values[key]]


def urllib_unquote(value: str) -> str:
    return urllib.request.url2pathname(value.replace("+", "%2B")).replace("%20", " ")


def typst_string(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def boolish(value: str) -> bool:
    return value.lower() in {"1", "true", "yes", "y", "x"}


def emit_foreground(fields: dict[str, str], specs: Iterable[FieldSpec], page: int) -> list[str]:
    lines: list[str] = []
    for spec in specs:
        if spec.page != page:
            continue
        value = fields.get(spec.key, "")
        if spec.kind == "checkbox":
            if boolish(value):
                lines.append(f"mark({spec.x}, {spec.y})")
        elif value:
            value = typst_string(value.upper())
            if spec.kind == "cells":
                lines.append(f'cells({spec.x}, {spec.y}, {spec.cell_w}, "{value}")')
            elif spec.kind == "amount":
                if value in {"0", "0.0", "0.00"}:
                    value = "0.00"
                lines.append(
                    f'amount({spec.x}, {spec.y}, {spec.cell_w}, '
                    f'{spec.int_cells}, {spec.dec_x}, "{value}")'
                )
            else:
                lines.append(f'label({spec.x}, {spec.y}, {spec.size}, "{value}")')
    return lines


def generate_typst(pdf_path: Path, out_dir: Path, fields: dict[str, str]) -> Path:
    svg_dir = out_dir / "svgbase"
    svg_dir.mkdir(parents=True, exist_ok=True)

    doc = fitz.open(pdf_path)
    if len(doc) < 1:
        raise RuntimeError("PDF has no pages")

    width = doc[0].rect.width
    height = doc[0].rect.height
    for index, page in enumerate(doc):
        svg = page.get_svg_image(text_as_path=True)
        (svg_dir / f"page{index + 1}.svg").write_text(svg)

    typ = out_dir / "generated.typ"
    lines = [
        f"#set page(width: {width}pt, height: {height}pt, margin: 0pt)",
        '#let put(x, y, body) = place(top + left, dx: x * 1pt, dy: y * 1pt, body)',
        '#let label(x, y, size, body) = put(x, y, text(font: "Arial", size: size * 1pt, body))',
        '#let mark(x, y) = put(x, y, text(font: "Arial", size: 13pt, weight: "bold", "X"))',
        '#let cells(x, y, cw, s) = { for (i, ch) in s.clusters().enumerate() { put(x + i * cw + cw / 2 - 2.3, y, text(font: "Arial", size: 8.5pt, ch)) } }',
        '#let amount(x, y, cw, intcells, decx, s) = { let parts = s.split("."); let int = parts.at(0, default: "0"); let dec = parts.at(1, default: "00"); let start = intcells - int.len(); for (i, ch) in int.clusters().enumerate() { put(x + (start + i) * cw + cw / 2 - 2.3, y, text(font: "Arial", size: 8.5pt, ch)) }; for (i, ch) in dec.clusters().enumerate() { put(decx + i * cw + cw / 2 - 2.3, y, text(font: "Arial", size: 8.5pt, ch)) } }',
    ]

    for index, _page in enumerate(doc):
        foreground = emit_foreground(fields, FIELD_MAP_2551Q, index)
        lines.append(
            f'#page(background: image("svgbase/page{index + 1}.svg", '
            f"width: {width}pt, height: {height}pt), foreground: {{"
        )
        lines.extend(foreground)
        lines.append("})[]")

    typ.write_text("\n".join(lines))
    return typ


def compile_typst(typ_path: Path, out_dir: Path) -> Path:
    output = out_dir / "generated.pdf"
    subprocess.run(
        ["typst", "compile", "--root", str(out_dir), str(typ_path), str(output)],
        check=True,
    )
    return output


def render_pngs(pdf_path: Path, out_dir: Path) -> None:
    doc = fitz.open(pdf_path)
    for index, page in enumerate(doc):
        pix = page.get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False)
        pix.save(out_dir / f"page{index + 1}.png")


def main() -> int:
    args = parse_args()
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    pdf_path = fetch_pdf(args.pdf, out_dir)
    fields = parse_ebir_xml(args.xml)
    typ_path = generate_typst(pdf_path, out_dir, fields)
    print(typ_path)
    if args.compile:
        pdf = compile_typst(typ_path, out_dir)
        print(pdf)
        if args.render_png:
            render_pngs(pdf, out_dir)
            print(out_dir / "page1.png")
    return 0


if __name__ == "__main__":
    sys.exit(main())
