#!/usr/bin/env python3
"""Round-trip every generated form, score it, and assert what scoring cannot see.

Two independent halves, deliberately:

**The round trip** prints each generated bundle to PDF with Chromium, re-extracts
it with the same extractor, and diffs against the source IR. Scoring is blunt --
rules and text runs recovered, as percentages -- and answers "did the browser
reproduce the geometry we told it to".

**The assertions** answer the question the round trip cannot ask. A round trip
compares our output against our own input, so anything wrong in the input is
reproduced faithfully and scores 100%. That is how this audit reported
`rules 100% on 51/51` while 137 real defects were present: a black rectangle over
a header, a seal printed upside-down, statutory tax brackets a taxpayer could
type over, money grids with no input fields at all. Every one of those survives a
perfect round trip.

The eight assertions in GOAL.md close that gap. Each publishes a boolean per form
under its own key so `gate.py` can demand it, plus a detail record naming the
offenders -- an assertion that fails without naming what failed is not
actionable. **True means the assertion holds.** Anything that cannot be evaluated
is `False` with a reason, never `True` and never absent: `gate.py` counts absence
as unevaluable and fails, which is the whole point.

The assertions read the source PDF's own drawing and text operators wherever the
question is "what does the official form actually print". That keeps them
independent of the IR schema and of the module whose output they are checking --
`comb_slots_match_printed` in particular must not be scored by the code that
produced the number under test.

Nothing here rasterises. Every measurement is a coordinate or a codepoint.
"""

from __future__ import annotations

import argparse
import collections
import copy
import dataclasses
import functools
import hashlib
import json
import math
import pathlib
import re
import sys
import time
import traceback
from typing import Any, Iterable, Sequence

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import extract  # noqa: E402
import verify  # noqa: E402

# --------------------------------------------------------------------------
# tolerances
#
# None of these widen a verify.py tolerance; they are the thresholds the
# assertions need and each is derived from one that already exists.
# --------------------------------------------------------------------------

# Two rectangles "overlap" only if they share area in both axes. Sharing an edge
# is not an overlap: an input butted against the glyph beside it is correct.
OVERLAP_EPS_PT = 0.05

# A rule sits below the guide cut only if it clears it by more than the position
# tolerance. Exactly at the cut is exactly right.
CUT_EPS_PT = 0.25

# Shear small enough to be invisible is not a transform. 0.01pt is a 25th of the
# 0.25pt position tolerance, measured over the image's unit square, so the worst
# displacement it admits is 0.01pt. One corpus placement (0605 xref 13) carries
# b = 5.3e-05 and c = 1.7e-06 -- arithmetic noise in the producer, not a skew.
TRANSFORM_EPS = 0.01

# Comb geometry, for the printed-compartment oracle.
#   MERGE: two verticals closer than this cannot be two character compartments.
#          The tightest comb pitch in the corpus is 10.32pt, so this is a sixth
#          of the smallest real gap. It exists because the generator draws one
#          divider as two collinear pieces 0.6pt apart when the piece below the
#          band is thicker than the tick above it.
#   EDGE:  a vertical this close to the cell's own side is that side's border.
#   MINLEN: shorter ink than this is a decimal point or a dot leader, not a
#          divider. The shortest real tick measured is 2.88pt.
#   EDGE:  a divider is interior only if its *ink* clears the cell's own sides
#          by this much. Two facts set it. The cell box is not the frame's
#          centreline -- on 1701 the 1.44pt page frame's outer edge lands 0.15pt
#          inside the cell's x1, so a centre-based or hairline margin counts the
#          frame as a slot boundary and invents 46 merges across 1701 and 1701A.
#          And the 1st-percentile comb pitch in this corpus is 10.24pt, so no
#          real compartment boundary is ever within 2pt of its own cell's side.
#          The measured mismatch count is flat from 2.0 to 3.0pt, which is what
#          being past the artefact and short of real data looks like. (A handful
#          of degenerate combs report a sub-1pt pitch; those are broken combs,
#          and this margin can hide a boundary inside one.)
COMB_MERGE_PT = 1.5
COMB_EDGE_PT = 2.0
COMB_FALLBACK_HALFWIDTH_PT = 0.6   # for an `l` op that declares no stroke width
COMB_MINLEN_PT = 0.8
COMB_YSLACK_PT = 0.5
COMB_MAX_WIDTH_PT = 2.5   # 1.44pt group separators are in; column borders are not

# Default offender preview limit. Assertions that need exhaustive evidence may
# opt out explicitly; the comb assertion does because its full disagreement set
# is the evidence the referee needs.
MAX_OFFENDERS = 12

# The rate/reference tables. `reflow_rate_without_description` is about a
# relocated *table*, and these are the marker patterns guides.py assigns to one.
RATE_TABLE_MARKERS = frozenset({"table-n", "alphanumeric-tax-code"})

ASSERTION_KEYS = (
    "inputs_over_printed_text",
    "comb_slots_match_printed",
    "money_boxes_have_inputs",
    "rules_below_guide_cut",
    "run_colour_matches_ir",
    "reflow_rate_without_description",
    "image_transform_applied",
    "no_invented_codepoints",
)

Rect = tuple[float, float, float, float]


# --------------------------------------------------------------------------
# geometry
# --------------------------------------------------------------------------


def overlaps(a: Rect, b: Rect, eps: float = OVERLAP_EPS_PT) -> bool:
    return (min(a[2], b[2]) - max(a[0], b[0]) > eps
            and min(a[3], b[3]) - max(a[1], b[1]) > eps)


class InkIndex:
    """Printed glyph boxes on one page, bucketed by y so lookups stay cheap.

    A linear scan is 17s over the corpus for one assertion; two assertions need
    it, and an audit nobody runs protects nothing.
    """

    BUCKET_PT = 8.0

    def __init__(self, boxes: Iterable[tuple[Rect, Any]]) -> None:
        self.buckets: dict[int, list[tuple[Rect, Any]]] = collections.defaultdict(list)
        for box, tag in boxes:
            lo = int(math.floor(box[1] / self.BUCKET_PT))
            hi = int(math.floor(box[3] / self.BUCKET_PT))
            for key in range(lo, hi + 1):
                self.buckets[key].append((box, tag))

    def hits(self, rect: Rect) -> list[tuple[Rect, Any]]:
        lo = int(math.floor(rect[1] / self.BUCKET_PT))
        hi = int(math.floor(rect[3] / self.BUCKET_PT))
        out = []
        for key in range(lo, hi + 1):
            for box, tag in self.buckets.get(key, ()):
                if overlaps(rect, box):
                    out.append((box, tag))
        return out

    def any_hit(self, rect: Rect) -> tuple[Rect, Any] | None:
        for hit in self.hits(rect):
            return hit
        return None


def glyph_boxes(run: dict) -> list[Rect]:
    """One box per *inked* glyph in a text run.

    The run bbox is the wrong unit for "does an input sit on pre-printed text".
    A label like `'Yes            No'` is one run whose bbox spans the checkbox
    drawn in the gap between the two words; scored by bbox it reports a collision
    that is not there, and 269 of the 362 bbox collisions in this corpus are that
    artefact. A glyph's own advance box is where the ink is.
    """
    out: list[Rect] = []
    offsets = run.get("char_origin_offsets_pt") or ()
    widths = run.get("char_widths_pt") or ()
    if len(offsets) != len(run["text"]) or len(widths) != len(run["text"]):
        # Without per-glyph metrics the run bbox is all there is; say so by
        # returning it, so the assertion errs towards reporting a collision.
        return [(run["x0"], run["y0"], run["x1"], run["y1"])]
    origin = run.get("origin_x", run["x0"])
    for char, offset, width in zip(run["text"], offsets, widths):
        if not char.strip():
            continue
        x = origin + offset
        out.append((x, run["y0"], x + width, run["y1"]))
    return out


# --------------------------------------------------------------------------
# emitted-document parsing
#
# The markup, not the layout engine. emit.py writes every position it means as
# an inline `pt` value in the same coordinate space as the IR, so parsing is
# exact, deterministic and about 400x faster than driving a browser. Playwright
# would only add its own layout opinion between us and the numbers we wrote.
# --------------------------------------------------------------------------

CELL_RE = re.compile(r'<div id="(p(\d+)c\d+)" class="([^"]*)"([^>]*)>')
# The last cell on a page is followed by the page's closing tags, an inert band
# template, or the band script, so cell content has to stop there too. Template
# inputs are blueprints, not live inputs belonging to the preceding cell.
CELL_BOUNDARY_RE = re.compile(
    r'<div id="p\d+c\d+" class="|<div class="page |<template|<script')
STYLE_BOX_RE = re.compile(
    r'style="left:([-\d.]+)pt;top:([-\d.]+)pt;width:([-\d.]+)pt;height:([-\d.]+)pt"')
SLOT_RE = re.compile(
    r'<div class="s" data-slot="(\d+)" '
    r'style="left:([-\d.]+)pt;top:([-\d.]+)pt;width:([-\d.]+)pt;height:([-\d.]+)pt"\s*>'
    r'(.*?)</div>', re.S)
INPUT_RE = re.compile(r'<input\b([^>]*)>')
INSET_RE = re.compile(
    r'inset:([-\d.]+)pt ([-\d.]+)pt ([-\d.]+)pt ([-\d.]+)pt')
RUN_RE = re.compile(r'<div class="t" id="p(\d+)t(\d+)" style="([^"]*)"')
PAGE_SPLIT_RE = re.compile(r'<div class="page page-(\d+)"')
SVG_RECT_RE = re.compile(r'<rect\b([^>]*)/>')
SVG_IMAGE_RE = re.compile(r'<image\b([^>]*)/>')
ATTR_RE = re.compile(r'([-a-zA-Z0-9:]+)="([^"]*)"')
COLOR_RE = re.compile(r'(?:^|;)color:#([0-9a-fA-F]{6})')
SECTION_RE = re.compile(r'<section class="gl-page"([^>]*)>(.*?)</section>', re.S)
ROW_RE = re.compile(r'<tr>(.*?)</tr>', re.S)
TD_RE = re.compile(r'<td[^>]*>(.*?)</td>', re.S)
TAG_RE = re.compile(r'<[^>]*>')


@dataclasses.dataclass
class Cell:
    id: str
    page: int
    classes: str
    attrs: str
    rect: Rect
    inner: str

    @property
    def comb_slots_attr(self) -> int | None:
        got = re.search(r'data-comb-slots="(\d+)"', self.attrs)
        return int(got.group(1)) if got else None


def parse_cells(html: str) -> list[Cell]:
    """Every field/label cell div with its box, in document order."""
    starts = list(CELL_RE.finditer(html))
    cells: list[Cell] = []
    for index, match in enumerate(starts):
        limit = starts[index + 1].start() if index + 1 < len(starts) else len(html)
        stop = CELL_BOUNDARY_RE.search(html, match.end(), limit)
        inner = html[match.end():stop.start() if stop else limit]
        box = STYLE_BOX_RE.search(match.group(4))
        if not box:
            continue
        left, top, width, height = (float(box.group(i)) for i in (1, 2, 3, 4))
        cells.append(Cell(id=match.group(1), page=int(match.group(2)),
                          classes=match.group(3), attrs=match.group(4),
                          rect=(left, top, left + width, top + height),
                          inner=inner))
    return cells


def slot_boxes(cell: Cell) -> list[tuple[int, Rect, bool]]:
    """Comb slots as (index, absolute box, whether it holds an input)."""
    left, top, _, _ = cell.rect
    out = []
    for match in SLOT_RE.finditer(cell.inner):
        x, y, w, h = (float(match.group(i)) for i in (2, 3, 4, 5))
        out.append((int(match.group(1)), (left + x, top + y, left + x + w, top + y + h),
                    "<input" in match.group(6)))
    return out


def input_boxes(cell: Cell) -> list[Rect]:
    """Every editable box this cell renders, in page coordinates.

    Comb inputs live inside their slot div and fill it; a plain text input fills
    the cell minus its declared inset. Both are absolute numbers in the markup,
    so no layout pass is needed to know where a taxpayer can type. A comb slot
    is clipped to its parent cell because `.f` uses `overflow:hidden`; geometry
    outside that box cannot paint typed text over the page.
    """
    left, top, right, bottom = cell.rect
    out: list[Rect] = []
    for _, box, has_input in slot_boxes(cell):
        if has_input:
            clipped = (max(left, box[0]), max(top, box[1]),
                       min(right, box[2]), min(bottom, box[3]))
            if clipped[2] > clipped[0] and clipped[3] > clipped[1]:
                out.append(clipped)
    for match in INPUT_RE.finditer(cell.inner):
        attrs = match.group(1)
        if "data-slot-index" in attrs:
            continue    # already counted via its slot
        inset = INSET_RE.search(attrs)
        if inset:
            t, r, b, l = (float(inset.group(i)) for i in (1, 2, 3, 4))
            out.append((left + l, top + t, right - r, bottom - b))
        else:
            out.append(cell.rect)
    return out


def parse_run_styles(html: str) -> dict[tuple[int, int], str]:
    return {(int(m.group(1)), int(m.group(2))): m.group(3) for m in RUN_RE.finditer(html)}


def page_chunks(html: str) -> dict[int, str]:
    """The markup of each `.page` div, keyed by its 1-based page number."""
    parts = PAGE_SPLIT_RE.split(html)
    return {int(parts[i]): parts[i + 1] for i in range(1, len(parts), 2)}


def attrs_of(text: str) -> dict[str, str]:
    return {k: v for k, v in ATTR_RE.findall(text)}


# --------------------------------------------------------------------------
# source-PDF oracles
#
# Everything below reads the pinned official PDF, so it answers "what does the
# form print" rather than "what did we decide the form prints".
# --------------------------------------------------------------------------


@functools.lru_cache(maxsize=1)
def source_index(root: str) -> dict[str, tuple[pathlib.Path, ...]]:
    base = pathlib.Path(root).expanduser()
    index: dict[str, list[pathlib.Path]] = collections.defaultdict(list)
    if base.is_dir():
        for pdf in sorted(base.rglob("*.pdf")):
            index[pdf.name].append(pdf)
    return {name: tuple(paths) for name, paths in index.items()}


def resolve_source(ir: dict, root: str) -> pathlib.Path | None:
    """The pinned PDF this IR came from, confirmed by hash.

    The IR records only a basename (`external:bir2550m.pdf`) and the corpus has
    duplicate folders offering the same name, so the recorded sha256 is what
    decides. A near-miss is not accepted: an assertion scored against the wrong
    revision is worse than one that reports it could not be scored.
    """
    source = ir.get("source") or {}
    name = str(source.get("file", "")).split(":", 1)[-1]
    wanted = source.get("sha256")
    for candidate in source_index(root).get(name, ()):
        if hashlib.sha256(candidate.read_bytes()).hexdigest() == wanted:
            return candidate
    return None


def vertical_segments(page) -> list[tuple[float, float, float, float]]:
    """Every vertical ink segment the page draws, as (x centre, y0, y1, width).

    Read straight off `get_drawings()`, i.e. off the content stream's own `re`,
    `l` and `qu` operators. This deliberately duplicates nothing from
    extract.py's rule classifier or lattice.py's comb detector: the number it
    produces is the number those two are being checked against, and an oracle
    that shares code with its subject proves only that the code is consistent.

    The width is carried because it is what distinguishes a comb tick from the
    cell's own side; see COMB_EDGE_PT.
    """
    out: list[tuple[float, float, float, float]] = []
    for drawing in page.get_drawings():
        stroke = float(drawing.get("width") or 0.0)
        for item in drawing["items"]:
            op = item[0]
            if op == "re":
                rect = item[1]
                if 0 < rect.width <= COMB_MAX_WIDTH_PT and rect.height > 0:
                    out.append((rect.x0 + rect.width / 2, rect.y0, rect.y1, rect.width))
            elif op == "l":
                p0, p1 = item[1], item[2]
                if abs(p1.x - p0.x) <= 0.3 and abs(p1.y - p0.y) > 0:
                    out.append(((p0.x + p1.x) / 2, min(p0.y, p1.y), max(p0.y, p1.y),
                                stroke or 2 * COMB_FALLBACK_HALFWIDTH_PT))
            elif op == "qu":
                quad = item[1]
                xs = (quad.ul.x, quad.ur.x, quad.ll.x, quad.lr.x)
                ys = (quad.ul.y, quad.ur.y, quad.ll.y, quad.lr.y)
                if max(xs) - min(xs) <= COMB_MAX_WIDTH_PT and max(ys) - min(ys) > 0:
                    out.append(((max(xs) + min(xs)) / 2, min(ys), max(ys),
                                max(xs) - min(xs)))
    return out


def printed_compartments(segments: Sequence[tuple[float, float, float, float]],
                         cell: Rect) -> tuple[int, list[float]]:
    """How many compartments the source prints inside this cell, and where.

    The window is the cell box -- geometry the lattice derives from rule
    intersections, not from its comb code -- and the counting is done here from
    raw drawing ops. So the count is independent of the slot number it is
    compared against, which is the point of the exercise.

    Two refinements, both forced by the corpus:

    * One divider can arrive as two collinear pieces whose centres differ by
      0.6pt, because the piece that continues below the tick band is thicker.
      Centres within COMB_MERGE_PT are one divider.
    * A cell can enclose more than one band of verticals -- 1707 p1c50 holds a
      full-height column border at x 233.69 *and* 24 comb ticks hanging from its
      bottom rule. Dividers are therefore grouped into bands by y-overlap and
      the most populous band is the comb; the border is not miscounted as a
      slot boundary.
    """
    x0, y0, x1, y1 = cell

    def interior(x: float, a: float, b: float, width: float) -> bool:
        half = max(width, 0.0) / 2
        return (b - a >= COMB_MINLEN_PT
                and a >= y0 - COMB_YSLACK_PT and b <= y1 + COMB_YSLACK_PT
                and x - half > x0 + COMB_EDGE_PT and x + half < x1 - COMB_EDGE_PT)

    spans: dict[float, tuple[float, float]] = {}
    for x, a, b, width in segments:
        if not interior(x, a, b, width):
            continue
        prior = spans.get(round(x, 4))
        spans[round(x, 4)] = (min(prior[0], a), max(prior[1], b)) if prior else (a, b)

    dividers: list[tuple[float, float, float]] = []
    for x in sorted(spans):
        a, b = spans[x]
        if dividers and x - dividers[-1][0] <= COMB_MERGE_PT:
            last = dividers[-1]
            dividers[-1] = (last[0], min(last[1], a), max(last[2], b))
            continue
        dividers.append((x, a, b))

    bands: list[dict[str, Any]] = []
    for x, a, b in dividers:
        for band in bands:
            shared = min(band["y1"], b) - max(band["y0"], a)
            if shared >= 0.5 * min(band["y1"] - band["y0"], b - a):
                band["y0"] = min(band["y0"], a)
                band["y1"] = max(band["y1"], b)
                band["xs"].append(x)
                break
        else:
            bands.append({"y0": a, "y1": b, "xs": [x]})
    if not bands:
        return 1, []
    best = max(bands, key=lambda b: (len(b["xs"]), -(b["y1"] - b["y0"])))
    return len(best["xs"]) + 1, [round(x, 2) for x in best["xs"]]


def drawn_codepoints(page) -> dict[tuple[float, float], set[int]]:
    """Codepoint(s) the page draws at each glyph origin.

    `get_texttrace()` reports what the font's encoding actually yields, U+FFFD
    included; `get_text("rawdict")` -- which extract.py reads -- substitutes a
    plausible character when the encoding fails. Comparing the two at the origin
    is how an invented character is caught, since the origin is the one thing
    both views agree on.
    """
    seen: dict[tuple[float, float], set[int]] = {}
    for span in page.get_texttrace():
        for char in span["chars"]:
            key = (round(char[2][0], 2), round(char[2][1], 2))
            seen.setdefault(key, set()).add(char[0])
    return seen


def transform_signature(matrix: Sequence[float]) -> tuple[int, int, bool]:
    """Orientation of a placement matrix: (x sign, y sign, sheared).

    Magnitude is already checked by the image's bbox, so what has to survive to
    the SVG is *orientation*. PyMuPDF reports the placement in the same
    y-downward space the page and our SVG use, so an upright image has d > 0 and
    2550M's flipped seal has d < 0; an SVG that reproduces it must therefore
    carry a negative y scale. Comparing signs rather than the six numbers keeps
    the assertion from dictating how emit.py decomposes the matrix.
    """
    a, b, c, d = (float(v) for v in matrix[:4])
    return (-1 if a < 0 else 1, -1 if d < 0 else 1,
            abs(b) > TRANSFORM_EPS or abs(c) > TRANSFORM_EPS)


SVG_TRANSFORM_RE = re.compile(r'(matrix|scale|translate|rotate)\(([^)]*)\)')


def svg_signature(transform: str | None) -> tuple[int, int, bool]:
    """The same orientation signature, read off an SVG transform list."""
    if not transform:
        return (1, 1, False)
    a, b, c, d = 1.0, 0.0, 0.0, 1.0
    for name, body in SVG_TRANSFORM_RE.findall(transform):
        nums = [float(v) for v in re.split(r'[\s,]+', body.strip()) if v]
        if name == "matrix" and len(nums) == 6:
            na, nb, nc, nd = nums[:4]
        elif name == "scale":
            na, nb, nc, nd = nums[0], 0.0, 0.0, (nums[1] if len(nums) > 1 else nums[0])
        elif name == "rotate" and nums:
            rad = math.radians(nums[0])
            na, nb, nc, nd = math.cos(rad), math.sin(rad), -math.sin(rad), math.cos(rad)
        else:
            continue    # translate has no linear part
        a, b, c, d = (a * na + c * nb, b * na + d * nb,
                      a * nc + c * nd, b * nc + d * nd)
    return transform_signature((a, b, c, d))


# --------------------------------------------------------------------------
# the bundle every assertion reads
# --------------------------------------------------------------------------


@dataclasses.dataclass
class Bundle:
    slug: str
    ir: dict
    layout: dict | None
    plan: dict | None
    form_html: str | None
    guide_html: str | None
    pdf: pathlib.Path | None

    @functools.cached_property
    def pages(self) -> dict[int, dict]:
        return {p["index"]: p for p in self.ir["pages"]}

    @functools.cached_property
    def layout_pages(self) -> dict[int, dict]:
        return {p["index"]: p for p in (self.layout or {}).get("pages", ())}

    @functools.cached_property
    def cells(self) -> list[Cell]:
        return parse_cells(self.form_html) if self.form_html else []

    @functools.cached_property
    def layout_cells(self) -> dict[str, dict]:
        return {c["id"]: c for p in self.layout_pages.values() for c in p["cells"]}

    @functools.cached_property
    def regions(self) -> list[dict]:
        return list((self.plan or {}).get("inline") or ())

    @functools.cached_property
    def relocated_cells(self) -> set[str]:
        out: set[str] = set()
        for region in self.regions:
            out.update(region.get("cell_ids") or ())
        return out

    @functools.cached_property
    def relocated_runs(self) -> set[tuple[int, int]]:
        out: set[tuple[int, int]] = set()
        for region in self.regions:
            for index in region.get("text_run_indices") or ():
                out.add((region["page"], index))
        return out

    @functools.cached_property
    def emitted_runs(self) -> dict[tuple[int, int], str]:
        return parse_run_styles(self.form_html) if self.form_html else {}

    @functools.cached_property
    def ink(self) -> dict[int, InkIndex]:
        """Printed glyph ink, per page, for the runs the form document emits."""
        boxes: dict[int, list[tuple[Rect, Any]]] = collections.defaultdict(list)
        for (page, index) in self.emitted_runs:
            run = self.pages.get(page, {}).get("text_runs", [])[index:index + 1]
            if not run:
                continue
            for box in glyph_boxes(run[0]):
                boxes[page].append((box, index))
        return {page: InkIndex(items) for page, items in boxes.items()}

    @functools.cached_property
    def doc(self):
        if self.pdf is None:
            return None
        import fitz  # local import: only the PDF oracles need it
        return fitz.open(self.pdf)

    @functools.cached_property
    def verticals(self) -> dict[int, list[tuple[float, float, float]]]:
        if self.doc is None:
            return {}
        return {index: vertical_segments(self.doc[index - 1]) for index in self.pages}

    def run_text(self, page: int, index: int) -> str:
        runs = self.pages.get(page, {}).get("text_runs", [])
        return runs[index]["text"] if 0 <= index < len(runs) else ""

    def close(self) -> None:
        if "doc" in self.__dict__ and self.__dict__["doc"] is not None:
            self.__dict__["doc"].close()


def held(**detail: Any) -> dict[str, Any]:
    return {"holds": True, "reason": "", "offenders": [], **detail}


def broken(reason: str, offenders: Sequence[Any] = (),
           *, offender_limit: int | None = MAX_OFFENDERS,
           **detail: Any) -> dict[str, Any]:
    all_offenders = list(offenders)
    published = (all_offenders if offender_limit is None
                 else all_offenders[:offender_limit])
    return {"holds": False, "reason": reason,
            "offender_count": len(all_offenders),
            "offenders_published": len(published),
            "offenders_omitted": len(all_offenders) - len(published),
            "offenders_complete": len(published) == len(all_offenders),
            "offenders": published, **detail}


# --------------------------------------------------------------------------
# assertion 1 -- no input over pre-printed text
# --------------------------------------------------------------------------


def check_inputs_over_printed_text(b: Bundle) -> dict[str, Any]:
    """C6's belt: a taxpayer must not be able to type over the printed form.

    The defect this catches is a statutory rate table emitted as fields -- on
    1700 page 2 the DOM really does offer an editable box over
    "Not over P 250,000". Scored against glyph ink, so the decorative `.` inside
    a money comb is the only kind of hit that is arguable, and it is reported
    rather than excused: a slot that already holds ink is a slot nothing should
    be typed into.
    """
    if b.form_html is None:
        return broken("no emitted form document to check")
    offenders = []
    for cell in b.cells:
        index = b.ink.get(cell.page)
        if index is None:
            continue
        for box in input_boxes(cell):
            hit = index.any_hit(box)
            if hit is None:
                continue
            offenders.append({
                "cell": cell.id,
                "page": cell.page,
                "run": hit[1],
                "text": b.run_text(cell.page, hit[1])[:60],
            })
            break
    if offenders:
        return broken(f"{len(offenders)} input(s) sit on printed glyph ink", offenders,
                      cells_checked=len(b.cells))
    return held(cells_checked=len(b.cells))


# --------------------------------------------------------------------------
# assertion 2 -- comb slots equal printed compartments
# --------------------------------------------------------------------------


def check_comb_slots_match_printed(b: Bundle) -> dict[str, Any]:
    """C5: a slot count short of the printed one centres a digit on a bar.

    The printed count comes from `printed_compartments`, i.e. from the source
    PDF's drawing operators, because two existing oracles disagree about this
    and one of them is lattice.py's own comb code.

    The verdict is scored against the *emitted* slot count, because that is the
    comb a taxpayer types into. The lattice's own slot count is recorded beside
    it: the two differ whenever the emitted document predates a lattice change,
    and telling that apart from a real merge is the difference between a
    regeneration and a fix. `layout_mismatches` is therefore the number to quote
    when comparing this oracle against another one -- it is free of emission lag.
    """
    if b.layout is None:
        return broken("no layout to read comb geometry from")
    if b.doc is None:
        return broken("source PDF not resolved; printed compartments unknown")
    emitted_slots = {cell.id: len(slot_boxes(cell)) or cell.comb_slots_attr
                     for cell in b.cells}
    offenders, checked = [], 0
    layout_mismatches, stale_emission = 0, 0
    for page_index, page in sorted(b.layout_pages.items()):
        segments = b.verticals.get(page_index) or []
        for cell in page["cells"]:
            comb = cell.get("comb")
            if not comb or cell["id"] in b.relocated_cells:
                continue
            checked += 1
            latticed = comb["cells"]
            slots = emitted_slots.get(cell["id"]) or latticed
            printed, xs = printed_compartments(
                segments, (cell["x0"], cell["y0"], cell["x1"], cell["y1"]))
            layout_mismatches += latticed != printed
            stale_emission += slots != latticed
            if slots != printed:
                offenders.append({"cell": cell["id"], "page": page_index,
                                  "slots": slots, "latticed": latticed,
                                  "printed": printed,
                                  "printed_divider_x": xs[:16]})
    counts = {"combs_checked": checked, "layout_mismatches": layout_mismatches,
              "emission_behind_layout": stale_emission}
    if offenders:
        return broken(f"{len(offenders)} of {checked} combs disagree with the "
                      f"printed compartment count", offenders,
                      offender_limit=None, **counts)
    return held(**counts)


# --------------------------------------------------------------------------
# assertion 3 -- every printed money box has an input
# --------------------------------------------------------------------------


def check_money_boxes_have_inputs(b: Bundle) -> dict[str, Any]:
    """C4: 2000-DST's entire page-1 money grid was unfillable.

    A box is fillable-by-construction if the source drew a comb in it -- a comb
    exists only to receive typed characters -- or if it is an enclosed empty
    rectangle. Either way it must carry an input.

    A comb slot that already holds printed ink is excluded, which is what keeps
    this from contradicting assertion 1, and a comb whose *every* slot is inked
    is not a money box at all: those are the container cells where a header ran
    across a run of ticks, and demanding inputs there would put a field over the
    heading. 49 cells in this corpus are excluded that way, and the count is
    reported so the exclusion cannot hide anything.
    """
    if b.form_html is None:
        return broken("no emitted form document to check")
    if b.layout is None:
        return broken("no layout to identify printed boxes from")
    offenders, checked, fully_inked = [], 0, 0
    by_id = {cell.id: cell for cell in b.cells}
    for cell_id, layout_cell in b.layout_cells.items():
        if cell_id in b.relocated_cells:
            continue
        cell = by_id.get(cell_id)
        if cell is None:
            continue
        index = b.ink.get(cell.page)
        comb = layout_cell.get("comb")
        if comb:
            slots = slot_boxes(cell)
            if not slots:
                checked += 1
                offenders.append({"cell": cell_id, "page": cell.page,
                                  "why": "comb printed, no slots emitted",
                                  "printed_slots": comb["cells"]})
                continue
            free = [(i, box, has) for i, box, has in slots
                    if index is None or index.any_hit(box) is None]
            if not free:
                fully_inked += 1
                continue
            checked += 1
            missing = [i for i, _, has in free if not has]
            if missing:
                offenders.append({"cell": cell_id, "page": cell.page,
                                  "why": "comb slots with no input",
                                  "slots": len(slots), "ink_free": len(free),
                                  "without_input": missing[:8]})
            continue
        border = layout_cell.get("border") or {}
        enclosed = all(border.get(side) for side in ("top", "bottom", "left", "right"))
        if enclosed and layout_cell.get("is_empty") and layout_cell.get("rectangular"):
            checked += 1
            if not input_boxes(cell):
                offenders.append({"cell": cell_id, "page": cell.page,
                                  "why": "enclosed empty box, no input"})
    if offenders:
        return broken(f"{len(offenders)} of {checked} printed boxes are not fillable",
                      offenders, boxes_checked=checked, combs_fully_inked=fully_inked)
    return held(boxes_checked=checked, combs_fully_inked=fully_inked)


# --------------------------------------------------------------------------
# assertion 4 -- nothing form-side below the guide cut
# --------------------------------------------------------------------------


def check_rules_below_guide_cut(b: Bundle) -> dict[str, Any]:
    """C7: an orphaned frame down two-thirds of a page.

    Awarding a straddling rule to the form was chosen so a cut could never lose
    one. The cost is 1600-PT keeping `v85` and `v148`, each 1.44 x 461.33pt,
    with the table they framed now in the guide. Both the IR's form side and the
    emitted SVG are checked: the IR says what we decided to keep, the SVG says
    what a taxpayer sees.
    """
    if not b.regions:
        return held(reason="", cuts=0)
    if b.form_html is None:
        return broken("guide plan cuts pages but no form document to check")
    chunks = page_chunks(b.form_html)
    offenders, fills_below = [], 0
    for region in b.regions:
        page_index, cut = region["page"], float(region["cut_y_pt"])
        claimed = set(region.get("rule_ids") or ())
        for rule in b.pages.get(page_index, {}).get("rules", ()):
            if rule["id"] in claimed:
                continue
            if rule["y1"] > cut + CUT_EPS_PT:
                offenders.append({"page": page_index, "rule": rule["id"],
                                  "y1": rule["y1"], "cut_y": cut, "where": "ir"})
        for match in SVG_RECT_RE.finditer(chunks.get(page_index, "")):
            attrs = attrs_of(match.group(1))
            try:
                bottom = float(attrs["y"]) + float(attrs["height"])
            except (KeyError, ValueError):
                continue
            if bottom <= cut + CUT_EPS_PT:
                continue
            if "data-rule-id" in attrs:
                offenders.append({"page": page_index, "rule": attrs["data-rule-id"],
                                  "y1": round(bottom, 2), "cut_y": cut,
                                  "where": "emitted"})
            else:
                fills_below += 1
    if offenders:
        return broken(f"{len(offenders)} form-side rule(s) cross the guide cut",
                      offenders, cuts=len(b.regions), area_fills_below_cut=fills_below)
    return held(cuts=len(b.regions), area_fills_below_cut=fills_below)


# --------------------------------------------------------------------------
# assertion 5 -- emitted colour equals the IR's
# --------------------------------------------------------------------------


def check_run_colour_matches_ir(b: Bundle) -> dict[str, Any]:
    """C8: 1600-PT and 1600-VT publish 25 white runs each, in black.

    They are BIR reviewer initials, invisible on the official paper. Rendering
    them black turns something the source hid into something that reads as ATC
    data, so this is a disclosure defect and not a styling one.

    The form document is checked run by run against the IR. The guide can only be
    checked by containment -- its reflow merges runs into table cells and drops
    the run ids -- so the test there is that every non-black colour a relocated
    run carries appears somewhere in the guide's markup. That is weaker, and it
    is enough to catch a document that declares no colour at all, which is the
    defect.
    """
    if b.form_html is None:
        return broken("no emitted form document to check")
    offenders = []
    checked = 0
    for page_index, page in sorted(b.pages.items()):
        for index, run in enumerate(page.get("text_runs") or ()):
            key = (page_index, index)
            style = b.emitted_runs.get(key)
            if style is None:
                if key not in b.relocated_runs:
                    offenders.append({"page": page_index, "run": index,
                                      "why": "neither emitted nor relocated",
                                      "text": run["text"][:40]})
                continue
            checked += 1
            got = COLOR_RE.search(style)
            want = int(run.get("color") or 0)
            if got is None:
                offenders.append({"page": page_index, "run": index,
                                  "why": "no colour declared",
                                  "ir_color": f"#{want:06x}"})
            elif int(got.group(1), 16) != want:
                offenders.append({"page": page_index, "run": index,
                                  "why": "colour differs",
                                  "emitted": f"#{int(got.group(1), 16):06x}",
                                  "ir_color": f"#{want:06x}"})
    guide_colours = {int(b.pages[page]["text_runs"][index].get("color") or 0)
                     for page, index in b.relocated_runs
                     if page in b.pages
                     and index < len(b.pages[page]["text_runs"])}
    guide_colours.discard(0)
    if guide_colours:
        markup = (b.guide_html or "").lower()
        for colour in sorted(guide_colours):
            if f"#{colour:06x}" not in markup:
                offenders.append({"why": "relocated run's colour absent from guide",
                                  "ir_color": f"#{colour:06x}",
                                  "runs": sum(1 for p, i in b.relocated_runs
                                              if int(b.pages[p]["text_runs"][i]
                                                     .get("color") or 0) == colour)})
    if offenders:
        return broken(f"{len(offenders)} run colour(s) do not match the IR",
                      offenders, runs_checked=checked)
    return held(runs_checked=checked)


# --------------------------------------------------------------------------
# assertion 6 -- a relocated rate row keeps its description
# --------------------------------------------------------------------------


def check_reflow_rate_without_description(b: Bundle) -> dict[str, Any]:
    """C9: the only *correctness* hazard in the review.

    1600-PT's guide shows a two-line ATC description, then a row holding only
    "3% | WB 050". A reader can attach that rate to the wrong nature of payment,
    which on a withholding return is a wrong remittance. The signature is
    machine-checkable: an empty description cell beside a non-empty rate.

    A rate table reflowed as prose fails too. 2551M's table is flattened into
    running text, which destroys the column relationship outright -- there are no
    rows left to check, and reporting that as "no bad rows found" is exactly the
    blindness this file exists to remove.
    """
    tables = [r for r in b.regions if r.get("marker_pattern") in RATE_TABLE_MARKERS]
    if not tables:
        return held(rate_tables=0)
    if b.guide_html is None:
        return broken(f"{len(tables)} rate table(s) relocated but no guide document")
    sections = {}
    for match in SECTION_RE.finditer(b.guide_html):
        attrs = attrs_of(match.group(1))
        if "data-page" in attrs:
            sections[int(attrs["data-page"])] = (attrs.get("data-flow"), match.group(2))
    offenders, rows_checked = [], 0
    for region in tables:
        page_index = region["page"]
        section = sections.get(page_index)
        if section is None:
            offenders.append({"page": page_index, "why": "no guide section for page"})
            continue
        flow, body = section
        if flow != "table":
            offenders.append({"page": page_index, "why": f"rate table reflowed as {flow}; "
                                                         "row structure not recoverable",
                              "marker": region.get("marker", "")[:50]})
            continue
        for row in ROW_RE.finditer(body):
            cells = [TAG_RE.sub("", c).replace("&amp;", "&").strip()
                     for c in TD_RE.findall(row.group(1))]
            if len(cells) < 2:
                continue
            rows_checked += 1
            if not cells[0] and any(cells[1:]):
                offenders.append({"page": page_index, "why": "rate without description",
                                  "row": [c[:24] for c in cells]})
    if offenders:
        return broken(f"{len(offenders)} relocated rate row(s)/table(s) lost their "
                      f"description", offenders, rate_tables=len(tables),
                      rows_checked=rows_checked)
    return held(rate_tables=len(tables), rows_checked=rows_checked)


# --------------------------------------------------------------------------
# assertion 7 -- a non-upright image is emitted with its transform
# --------------------------------------------------------------------------


def check_image_transform_applied(b: Bundle) -> dict[str, Any]:
    """C3: 2550M's seal prints upside-down, rim lettering bottom-to-top.

    The placement matrix is read from the source PDF rather than from the IR, so
    the assertion holds its own evidence and stays evaluable across an IR schema
    change. Orientation signatures are compared as multisets per page: pairing
    individual placements would need a hash the emitter is free to change, while
    "this page draws one y-flipped image and the SVG flips none" is decidable
    from either document alone.
    """
    if b.doc is None:
        return broken("source PDF not resolved; placement matrices unknown")
    if b.form_html is None:
        return broken("no emitted form document to check")
    chunks = page_chunks(b.form_html)
    offenders = []
    placements = 0
    for page_index in sorted(b.pages):
        want: collections.Counter = collections.Counter()
        for info in b.doc[page_index - 1].get_image_info(xrefs=True):
            placements += 1
            want[transform_signature(info["transform"])] += 1
        got: collections.Counter = collections.Counter()
        for match in SVG_IMAGE_RE.finditer(chunks.get(page_index, "")):
            got[svg_signature(attrs_of(match.group(1)).get("transform"))] += 1
        if want == got:
            continue
        for signature in sorted(set(want) | set(got)):
            if want[signature] == got[signature]:
                continue
            offenders.append({"page": page_index,
                              "orientation": {"x_sign": signature[0],
                                              "y_sign": signature[1],
                                              "sheared": signature[2]},
                              "source_placements": want[signature],
                              "emitted": got[signature]})
    if offenders:
        return broken("emitted image orientation differs from the source's",
                      offenders, placements=placements)
    return held(placements=placements)


# --------------------------------------------------------------------------
# assertion 8 -- no invented codepoints
# --------------------------------------------------------------------------


INVENTED_SUSPECTS = "?§"


def check_no_invented_codepoints(b: Bundle) -> dict[str, Any]:
    """C1: a character that looks like content but is not in the source.

    Both known lies are covered. `?` is what a dropped glyph used to become, and
    a `?` inside a checkbox makes lattice.py classify the cell as a label, so the
    taxpayer cannot tick it. `§` is subtler and worse: on 2550M page 4 and 2553
    page 2 seven glyphs come from a Wingdings face with no ToUnicode CMap, and
    rawdict reports SECTION SIGN -- the WinAnsi meaning of a byte the font does
    not use. Nothing downstream can tell that apart from a real section sign.

    Checked per glyph origin against `get_texttrace()`, which reports U+FFFD
    rather than guessing, so this names the exact character. A `?` the source
    really does state passes: 2200S's checkbox glyph is drawn from codepoint
    U+003F and the assertion says so, which is the honest answer even though the
    glyph is a Wingdings box.
    """
    if b.doc is None:
        return broken("source PDF not resolved; drawn codepoints unknown")
    offenders, examined = [], 0
    for page_index, page in sorted(b.pages.items()):
        runs = page.get("text_runs") or ()
        suspect = [(i, r) for i, r in enumerate(runs)
                   if any(ch in r["text"] for ch in INVENTED_SUSPECTS)]
        if not suspect:
            continue
        drawn = drawn_codepoints(b.doc[page_index - 1])
        for index, run in suspect:
            offsets = run.get("char_origin_offsets_pt") or ()
            baseline = run.get("baseline_y")
            for position, char in enumerate(run["text"]):
                if char not in INVENTED_SUSPECTS:
                    continue
                examined += 1
                if baseline is None or position >= len(offsets):
                    offenders.append({"page": page_index, "run": index,
                                      "char_index": position, "char": char,
                                      "why": "run has no per-glyph origin to check"})
                    continue
                key = (round(run.get("origin_x", run["x0"]) + offsets[position], 2),
                       round(baseline, 2))
                codepoints = drawn.get(key)
                if codepoints is None:
                    offenders.append({"page": page_index, "run": index,
                                      "char_index": position, "char": char,
                                      "why": "no glyph drawn at this origin"})
                elif ord(char) not in codepoints:
                    offenders.append({"page": page_index, "run": index,
                                      "char_index": position, "char": char,
                                      "why": "source draws a different codepoint",
                                      "source_codepoints": [f"U+{c:04X}"
                                                            for c in sorted(codepoints)],
                                      "font": run.get("font"),
                                      "text": run["text"][:40]})
    if offenders:
        return broken(f"{len(offenders)} character(s) the source does not state",
                      offenders, characters_examined=examined)
    return held(characters_examined=examined)


CHECKS = {
    "inputs_over_printed_text": check_inputs_over_printed_text,
    "comb_slots_match_printed": check_comb_slots_match_printed,
    "money_boxes_have_inputs": check_money_boxes_have_inputs,
    "rules_below_guide_cut": check_rules_below_guide_cut,
    "run_colour_matches_ir": check_run_colour_matches_ir,
    "reflow_rate_without_description": check_reflow_rate_without_description,
    "image_transform_applied": check_image_transform_applied,
    "no_invented_codepoints": check_no_invented_codepoints,
}
assert tuple(CHECKS) == ASSERTION_KEYS, "GOAL.md names these eight, in this order"


def evaluate_assertions(bundle: Bundle) -> dict[str, Any]:
    """Run all eight and flatten them into the per-form record.

    A raising check is a failing check. It cannot be a passing one: an assertion
    that throws has not looked at the form, and "we did not look" is the exact
    reading of the audit that let 137 defects through.
    """
    details: dict[str, Any] = {}
    flat: dict[str, Any] = {}
    for key, check in CHECKS.items():
        started = time.perf_counter()
        try:
            detail = check(bundle)
        except Exception as exc:  # noqa: BLE001 - see docstring
            detail = broken(f"{type(exc).__name__}: {exc}",
                            trace=traceback.format_exc(limit=2))
        detail["seconds"] = round(time.perf_counter() - started, 3)
        details[key] = detail
        flat[key] = bool(detail["holds"])
    flat["assertions"] = details
    flat["assertions_held"] = sum(1 for key in ASSERTION_KEYS if flat[key])
    return flat


def load_bundle(slug: str, ir_dir: pathlib.Path, html_dir: pathlib.Path,
                layout_dir: pathlib.Path, guide_dir: pathlib.Path | None,
                source_root: str) -> Bundle:
    def maybe_json(path: pathlib.Path) -> dict | None:
        return json.loads(path.read_text(encoding="utf-8")) if path.is_file() else None

    def maybe_text(path: pathlib.Path) -> str | None:
        return path.read_text(encoding="utf-8") if path.is_file() else None

    ir = json.loads((ir_dir / f"{slug}.ir.json").read_text(encoding="utf-8"))
    return Bundle(
        slug=slug,
        ir=ir,
        layout=maybe_json(layout_dir / f"{slug}.layout.json"),
        plan=maybe_json(guide_dir / f"{slug}.guide.json") if guide_dir else None,
        form_html=maybe_text(html_dir / f"{slug}.html"),
        guide_html=maybe_text(html_dir / f"{slug}.guide.html"),
        pdf=resolve_source(ir, source_root),
    )


def form_side(reference: dict, plan: dict | None) -> tuple[dict, dict]:
    """Drop everything the guide plan moved out, from the reference IR.

    The form document no longer contains the guide's rules and strings, so
    scoring it against the whole source IR counts correctly-relocated content as
    missing. That is how a corpus at 100% rules came to read 42/51: nothing had
    moved on the sheet, the denominator was simply the wrong one.

    Indices are removed high-to-low so earlier removals cannot shift later ones.
    """
    if not plan or not plan.get("inline"):
        return reference, {"rules": 0, "text_runs": 0, "images": 0}

    filtered = copy.deepcopy(reference)
    removed = {"rules": 0, "text_runs": 0, "images": 0}
    by_page = {region["page"]: region for region in plan["inline"]}

    for page in filtered["pages"]:
        region = by_page.get(page["index"])
        if region is None:
            continue

        claimed_rules = set(region.get("rule_ids") or ())
        if claimed_rules:
            before = len(page["rules"])
            page["rules"] = [r for r in page["rules"] if r["id"] not in claimed_rules]
            removed["rules"] += before - len(page["rules"])

        for index in sorted(region.get("text_run_indices") or (), reverse=True):
            if 0 <= index < len(page["text_runs"]):
                del page["text_runs"][index]
                removed["text_runs"] += 1

        for index in sorted(region.get("image_indices") or (), reverse=True):
            if 0 <= index < len(page["images"]):
                del page["images"][index]
                removed["images"] += 1

        # Fills and paths are relocated too, and leaving them behind is what made
        # four pages look non-empty to the reference while emit.py correctly
        # dropped them: 0605 page 2 held 44 orphan fills and 532 orphan paths
        # after every rule, run and image on it had moved to the guide.
        for key, bucket in (("area_fill_indices", "area_fills"),
                            ("path_indices", "paths")):
            claimed = region.get(key) or ()
            items = page.get(bucket) or []
            for index in sorted(claimed, reverse=True):
                if 0 <= index < len(items):
                    del items[index]
                    removed[bucket] = removed.get(bucket, 0) + 1

        # stats are what the rule denominator is read from, so they must follow.
        page["stats"]["rules_structural"] = sum(
            1 for r in page["rules"] if r["role"] == "structural")

    # A page whose every element was relocated is not printed by the form at
    # all -- emit.py drops it rather than emitting a blank sheet. The reference
    # has to drop it too, or the page counts disagree and verify calls that a
    # paper mismatch: five forms failed exactly this way (0605, 1702Q, 2200P,
    # 2550M, 2553), all with identical page dimensions and rotations.
    kept = [p for p in filtered["pages"]
            if p["rules"] or p["text_runs"] or p["images"]
            or p.get("area_fills") or p.get("paths")]
    removed["pages"] = len(filtered["pages"]) - len(kept)
    if removed["pages"]:
        filtered["pages"] = kept
        for index, page in enumerate(kept, 1):
            page["index"] = index
        filtered["source"] = dict(filtered.get("source") or {})
        filtered["source"]["page_count"] = len(kept)

    return filtered, removed


def round_trip(bundle: Bundle, html_path: pathlib.Path,
               work: pathlib.Path) -> dict[str, Any]:
    """Print with Chromium, re-extract, diff against the source IR."""
    reference, relocated = form_side(bundle.ir, bundle.plan)
    record: dict[str, Any] = {"guide_relocated": relocated}

    pdf = work / f"{bundle.slug}.audit.pdf"
    pdf.parent.mkdir(parents=True, exist_ok=True)

    paper = reference["paper"]
    verify.html_to_pdf(html_path, pdf, paper["width_pt"], paper["height_pt"])

    candidate = extract.extract(pdf, reference["form"]["code"],
                                reference["form"]["revision"], None)
    report = verify.diff_ir(reference, candidate, verify.Tolerances(),
                            roles=["structural"])
    totals = report.get("totals", {})

    # Denominators come from the source IR, so a percentage always answers
    # "of what the official form contains, how much did we reproduce".
    rules_ref = sum(p["stats"]["rules_structural"] for p in reference["pages"])
    text_ref = sum(len(p["text_runs"]) for p in reference["pages"])
    rules_missing = totals.get("rules_missing", 0)
    text_missing = totals.get("text_missing", 0)

    # verify.py short-circuits on a paper mismatch and never walks the pages, so
    # every total comes back 0. Zero missing rules is indistinguishable from a
    # perfect form unless the record says which it is -- and reading the first
    # as the second is precisely the failure this project keeps paying for. The
    # gate treats `measured: false` as unevaluable, which counts as a failure.
    measured = report.get("hard_failure") is None

    record.update({
        "measured": measured,
        "hard_failure": report.get("hard_failure"),
        "paper_ok": bool(report.get("paper", {}).get("ok", True)),
        "rules_ref": rules_ref,
        "rules_missing": rules_missing,
        "rules_extra": totals.get("rules_extra", 0),
        "rules_thickness_violations": totals.get("rules_thickness_violations", 0),
        "rules_pct": round(100.0 * (rules_ref - rules_missing) / rules_ref, 2) if rules_ref else None,
        "text_ref": text_ref,
        "text_missing": text_missing,
        "text_extra": totals.get("text_extra", 0),
        "text_pct": round(100.0 * (text_ref - text_missing) / text_ref, 2) if text_ref else None,
        "images_missing": totals.get("images_missing", 0),
        "images_placement_violations": totals.get("images_placement_violations", 0),
    })
    return record


def score(slug: str, ir_dir: pathlib.Path, html_dir: pathlib.Path,
          layout_dir: pathlib.Path, guide_dir: pathlib.Path | None,
          work: pathlib.Path, source_root: str,
          roundtrip: bool = True) -> dict:
    """One form's record: the eight assertions, then the round-trip score.

    The assertions run first and are kept whatever the round trip does. A
    Chromium failure must not also erase the checks that do not need Chromium --
    losing them is how the record would come to say nothing while looking
    complete.
    """
    record: dict = {"slug": slug, "status": "error", "error": None}
    bundle = None
    try:
        bundle = load_bundle(slug, ir_dir, html_dir, layout_dir, guide_dir, source_root)
        record.update(evaluate_assertions(bundle))
    except Exception as exc:  # noqa: BLE001 - one bad form must not stop the sweep
        reason = f"{type(exc).__name__}: {exc}"
        record.update({key: False for key in ASSERTION_KEYS})
        record["assertions"] = {key: broken(reason) for key in ASSERTION_KEYS}
        record["assertions_held"] = 0
        record["error"] = reason

    try:
        if bundle is None:
            raise RuntimeError(record["error"] or "bundle not loaded")
        html_path = html_dir / f"{slug}.html"
        if not roundtrip:
            record["status"] = "ok"
            record["roundtrip"] = "skipped"
        else:
            record.update(round_trip(bundle, html_path, work))
            record["status"] = "ok"
    except Exception as exc:  # noqa: BLE001
        record["error"] = f"{type(exc).__name__}: {exc}"
        record["trace"] = traceback.format_exc(limit=3)
    finally:
        if bundle is not None:
            bundle.close()
    return record


def self_test() -> int:
    """Prove each assertion can fail, and that absence of evidence is failure.

    An assertion that cannot report a violation is decoration, so every one is
    fed a bundle it must reject. The fixtures are tiny by design: the corpus
    proves the assertions find real defects, this proves they would still find
    one if the corpus were clean.
    """
    failures: list[str] = []

    def check(name: str, condition: bool) -> None:
        if not condition:
            failures.append(name)

    ir = {
        "form": {"code": "TEST", "revision": "0000"},
        "source": {"file": "external:none.pdf", "sha256": "0" * 64},
        "paper": {"width_pt": 100.0, "height_pt": 100.0},
        "pages": [{
            "index": 1, "width_pt": 100.0, "height_pt": 100.0, "rotation": 0,
            "rules": [{"id": "h0", "axis": "h", "x0": 0.0, "y0": 90.0, "x1": 50.0,
                       "y1": 90.24, "thickness_pt": 0.24, "role": "structural"}],
            "area_fills": [], "images": [],
            "text_runs": [{
                "text": "Rate?", "font": "Arial", "size_pt": 8.0, "color": 16777215,
                "x0": 10.0, "y0": 10.0, "x1": 30.0, "y1": 18.0, "origin_x": 10.0,
                "baseline_y": 16.0,
                "char_origin_offsets_pt": [0.0, 4.0, 8.0, 12.0, 16.0],
                "char_widths_pt": [4.0, 4.0, 4.0, 4.0, 4.0],
            }],
            "stats": {"rules_structural": 1},
        }],
    }
    html = (
        '<div class="page page-1" id="page-1" style="width:100pt;height:100pt">'
        '<svg class="rl"><rect x="0" y="90" width="50" height="0.24" '
        'fill="#000000" data-rule-id="h0"/></svg>'
        '<div class="layer-text"><div class="t" id="p1t0" style="left:10pt;top:10pt;'
        'color:#000000">Rate?</div></div>'
        '<div id="p1c0" class="c f" data-cell-kind="field" data-field-kind="text" '
        'style="left:8pt;top:8pt;width:30pt;height:12pt">'
        '<input type="text" class="fi" id="p1c0-i" name="p1c0" '
        'style="inset:0pt 0pt 0pt 0pt"></div>'
        '<div id="p1c1" class="c" data-cell-kind="mixed" data-comb-slots="2" '
        'style="left:50pt;top:50pt;width:20pt;height:10pt">'
        '<div class="s" data-slot="0" style="left:0pt;top:0pt;width:10pt;height:10pt">'
        '</div><div class="s" data-slot="1" style="left:10pt;top:0pt;width:10pt;'
        'height:10pt"></div></div>'
        '</div>')
    layout = {"pages": [{"index": 1, "cells": [
        {"id": "p1c0", "x0": 8.0, "y0": 8.0, "x1": 38.0, "y1": 20.0,
         "border": {"top": {}, "bottom": {}, "left": {}, "right": {}},
         "is_empty": False, "rectangular": True, "kind": "field", "text_run_ids": []},
        {"id": "p1c1", "x0": 50.0, "y0": 50.0, "x1": 70.0, "y1": 60.0,
         "border": {"top": {}, "bottom": {}, "left": {}, "right": {}},
         "is_empty": True, "rectangular": True, "kind": "mixed", "text_run_ids": [],
         "comb": {"cells": 2, "divider_x": [60.0], "slot_x": [50.0, 60.0, 70.0],
                  "y0": 56.0, "y1": 60.0}},
    ]}]}
    plan = {"inline": [{"page": 1, "cut_y_pt": 40.0, "rule_ids": [],
                        "text_run_indices": [0], "cell_ids": [],
                        "marker_pattern": "table-n", "marker": "Table 1"}]}
    guide_html = ('<section class="gl-page" data-page="1" data-flow="table">'
                  '<table class="gl-table"><tr><td></td><td>3%</td></tr></table>'
                  '</section>')

    b = Bundle(slug="test", ir=ir, layout=layout, plan=plan, form_html=html,
               guide_html=guide_html, pdf=None)
    results = evaluate_assertions(b)

    # 1: the input at 8..38 x 8..20 covers the glyphs of "Rate?" at 10..30.
    check("inputs_over_printed_text must fail on an input over glyph ink",
          results["inputs_over_printed_text"] is False)
    # 3: p1c1 prints two comb slots, neither carrying an input.
    check("money_boxes_have_inputs must fail on a comb with no inputs",
          results["money_boxes_have_inputs"] is False)
    # 4: h0 ends at y 90.24, the cut is at 40.
    check("rules_below_guide_cut must fail on a rule past the cut",
          results["rules_below_guide_cut"] is False)
    # 5: the run is white in the IR and black in the markup. It is also the
    # relocated run, so the guide-containment half must fire as well.
    check("run_colour_matches_ir must fail on a white run emitted black",
          results["run_colour_matches_ir"] is False)
    # 6: the guide's only row has an empty description and a 3% rate.
    check("reflow_rate_without_description must fail on a rate with no description",
          results["reflow_rate_without_description"] is False)
    # 2, 7, 8 need the source PDF, which this fixture deliberately lacks:
    # unevaluable must read as failure, not as a pass.
    for key in ("comb_slots_match_printed", "image_transform_applied",
                "no_invented_codepoints"):
        check(f"{key} must fail when the source PDF cannot be resolved",
              results[key] is False and "not resolved" in results["assertions"][key]["reason"])
    check("every assertion must name offenders or a reason",
          all(results["assertions"][k]["reason"] or results["assertions"][k]["offenders"]
              for k in ASSERTION_KEYS if not results[k]))

    # The clean side: the same page with the input moved off the ink, the comb
    # filled, the cut below everything, the colour right and the row complete.
    clean_html = html.replace('style="left:8pt;top:8pt;width:30pt;height:12pt"',
                              'style="left:40pt;top:30pt;width:30pt;height:12pt"')
    clean_html = clean_html.replace('color:#000000', 'color:#ffffff')
    clean_html = clean_html.replace(
        '<div class="s" data-slot="0" style="left:0pt;top:0pt;width:10pt;height:10pt">'
        '</div>',
        '<div class="s" data-slot="0" style="left:0pt;top:0pt;width:10pt;height:10pt">'
        '<input type="text" class="fi fc" data-slot-index="0"></div>')
    clean_html = clean_html.replace(
        '<div class="s" data-slot="1" style="left:10pt;top:0pt;width:10pt;height:10pt">'
        '</div>',
        '<div class="s" data-slot="1" style="left:10pt;top:0pt;width:10pt;height:10pt">'
        '<input type="text" class="fi fc" data-slot-index="1"></div>')
    clean_layout = copy.deepcopy(layout)
    clean_layout["pages"][0]["cells"][0]["x0"] = 40.0
    clean_layout["pages"][0]["cells"][0]["x1"] = 70.0
    clean_layout["pages"][0]["cells"][0]["y0"] = 30.0
    clean_layout["pages"][0]["cells"][0]["y1"] = 42.0
    clean_plan = copy.deepcopy(plan)
    clean_plan["inline"][0]["cut_y_pt"] = 95.0
    clean_guide = guide_html.replace("<td></td>", "<td>Franchise tax</td>")
    clean_guide += '<span style="color:#ffffff">initials</span>'
    clean = Bundle(slug="test", ir=ir, layout=clean_layout, plan=clean_plan,
                   form_html=clean_html, guide_html=clean_guide, pdf=None)
    ok = evaluate_assertions(clean)
    for key in ("inputs_over_printed_text", "money_boxes_have_inputs",
                "rules_below_guide_cut", "run_colour_matches_ir",
                "reflow_rate_without_description"):
        check(f"{key} must hold on the corrected fixture: "
              f"{ok['assertions'][key]['reason']}", ok[key] is True)

    # A run assigned to the same lattice cell is still printed ink. Ownership
    # explains the collision; it does not make a live input over that ink safe.
    owned_layout = copy.deepcopy(layout)
    owned_layout["pages"][0]["cells"][0]["text_run_ids"] = ["p1t0"]
    owned = Bundle(slug="owned", ir=ir, layout=owned_layout, plan=None,
                   form_html=html, guide_html=None, pdf=None)
    check("an input over its own cell's printed run still fails",
          check_inputs_over_printed_text(owned)["holds"] is False)

    # The live page's last cell is immediately followed by inert band template
    # markup. An input inside that template must not be attributed to the cell.
    template_html = (
        '<div class="page page-1" id="page-1" style="width:100pt;height:100pt">'
        '<div class="t" id="p1t0" style="left:10pt;top:10pt">Rate?</div>'
        '<div id="p1c9" class="c" data-cell-kind="label" '
        'style="left:8pt;top:8pt;width:30pt;height:12pt"></div></div>'
        '<template id="band-template-p1g0"><input type="text" class="fi" '
        'style="inset:0pt 0pt 0pt 0pt"></template><script></script>')
    template_cells = parse_cells(template_html)
    template_bundle = Bundle(slug="template", ir=ir, layout=None, plan=None,
                             form_html=template_html, guide_html=None, pdf=None)
    check("template inputs do not belong to the preceding live cell",
          len(template_cells) == 1
          and not input_boxes(template_cells[0])
          and check_inputs_over_printed_text(template_bundle)["holds"] is True)

    # A malformed comb can put a slot outside its parent. The real `.f` clips
    # that slot, so glyph ink in the clipped-away area is not under an input.
    off_cell_html = (
        '<div class="page page-1" id="page-1" style="width:100pt;height:100pt">'
        '<div class="t" id="p1t0" style="left:10pt;top:10pt">Rate?</div>'
        '<div id="p1c0" class="c f" data-cell-kind="mixed" '
        'style="left:8pt;top:20pt;width:30pt;height:12pt">'
        '<div class="s" data-slot="0" '
        'style="left:0pt;top:-12pt;width:30pt;height:8pt">'
        '<input type="text" class="fi fc" data-slot-index="0"></div></div></div>')
    off_cell = Bundle(slug="off-cell", ir=ir, layout=None, plan=None,
                      form_html=off_cell_html, guide_html=None, pdf=None)
    check("comb input geometry clipped outside its parent cannot collide",
          check_inputs_over_printed_text(off_cell)["holds"] is True)

    # Most assertion details are bounded previews, and say exactly what they
    # omit. The comb assertion is the referee's evidence packet, so every
    # offender must survive publication -- including the first one beyond the
    # old twelve-item preview.
    preview = broken("preview", list(range(MAX_OFFENDERS + 1)))
    check("bounded offender previews state that one record was omitted",
          len(preview["offenders"]) == MAX_OFFENDERS
          and preview["offenders_published"] == MAX_OFFENDERS
          and preview["offenders_omitted"] == 1
          and preview["offenders_complete"] is False)

    class CombPublicationFixture:
        layout = {"pages": []}
        doc = object()
        cells: list[Cell] = []
        relocated_cells: set[str] = set()
        layout_pages = {1: {"cells": [
            {"id": f"p1c{index}", "x0": 0.0, "y0": 0.0,
             "x1": 40.0, "y1": 10.0, "comb": {"cells": 1}}
            for index in range(MAX_OFFENDERS + 1)
        ]}}
        verticals = {1: [(20.0, 0.0, 10.0, 0.24)]}

    complete = check_comb_slots_match_printed(CombPublicationFixture())
    check("comb publication keeps the offender beyond the old preview limit",
          complete["offender_count"] == MAX_OFFENDERS + 1
          and complete["offenders_published"] == MAX_OFFENDERS + 1
          and complete["offenders_omitted"] == 0
          and complete["offenders_complete"] is True
          and len(complete["offenders"]) == MAX_OFFENDERS + 1
          and complete["offenders"][-1]["cell"] == f"p1c{MAX_OFFENDERS}")

    # Geometry helpers, where an off-by-one epsilon would silently disable an
    # assertion rather than break it.
    check("touching edges do not overlap", not overlaps((0, 0, 10, 10), (10, 0, 20, 10)))
    check("shared area overlaps", overlaps((0, 0, 10, 10), (9, 0, 20, 10)))
    check("whitespace carries no ink",
          len(glyph_boxes({"text": "a b", "x0": 0.0, "y0": 0.0, "x1": 9.0, "y1": 8.0,
                           "origin_x": 0.0,
                           "char_origin_offsets_pt": [0.0, 3.0, 6.0],
                           "char_widths_pt": [3.0, 3.0, 3.0]})) == 2)
    check("an upright placement needs no SVG transform",
          transform_signature((10.0, 0.0, 0.0, 10.0, 0.0, 0.0)) == svg_signature(None))
    check("a y-flipped placement needs a negative y scale",
          transform_signature((10.0, 0.0, -0.0, -10.0, 0.0, 0.0))
          == svg_signature("translate(5,5) scale(1,-1)"))
    check("arithmetic noise is not a shear",
          transform_signature((41.0, 5.3e-05, 1.7e-06, 33.8, 0.0, 0.0))
          == (1, 1, False))
    # Two ticks, one of them drawn as two pieces 0.6pt apart; a 1.44pt frame
    # whose centre is 0.7pt inside the cell but whose ink reaches the side; and
    # a 0.5pt-tall decimal point. Only the two ticks are compartment boundaries.
    segments = [(10.0, 5.0, 9.0, 0.24), (10.6, 9.0, 10.4, 1.44),
                (20.0, 5.0, 9.0, 0.24), (0.7, 0.0, 10.0, 1.44),
                (39.3, 0.0, 10.0, 1.44), (30.0, 6.0, 6.5, 1.68)]
    count, xs = printed_compartments(segments, (0.0, 0.0, 40.0, 10.5))
    check(f"two pieces of one divider count once, frame and decimal point "
          f"excluded (got {count}, xs {xs})", count == 3 and len(xs) == 2)

    for name in failures:
        print(f"FAIL {name}", file=sys.stderr)
    print(f"audit self-test: {len(failures)} failure(s)", file=sys.stderr)
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ir-dir", type=pathlib.Path, default=pathlib.Path("build/ir"))
    parser.add_argument("--html-dir", type=pathlib.Path, default=pathlib.Path("build/html"))
    parser.add_argument("--layout-dir", type=pathlib.Path,
                        default=pathlib.Path("build/layout"),
                        help="Lattice output; the assertions read cell and comb geometry.")
    parser.add_argument("--work", type=pathlib.Path, default=pathlib.Path("build/audit"))
    parser.add_argument("--guide-dir", type=pathlib.Path, default=pathlib.Path("build/guides"),
                        help="Guide plans; content moved to guide.html leaves the form denominator.")
    parser.add_argument("--source-root", type=pathlib.Path,
                        default=pathlib.Path.home() / "Downloads/forms",
                        help="Where the pinned official PDFs live. Three assertions "
                             "read the source's own operators rather than our IR.")
    parser.add_argument("--out", type=pathlib.Path, default=pathlib.Path("build/audit.json"))
    parser.add_argument("--only", action="append", default=None)
    parser.add_argument("--assertions-only", action="store_true",
                        help="Skip the Chromium round trip. Useful while iterating on "
                             "an assertion; not the audit.")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    slugs = sorted(p.name[: -len(".ir.json")] for p in args.ir_dir.glob("*.ir.json"))
    if args.only:
        wanted = {s.lower() for s in args.only}
        slugs = [s for s in slugs if any(w in s for w in wanted)]

    records = []
    for i, slug in enumerate(slugs, 1):
        html = args.html_dir / f"{slug}.html"
        if not html.is_file():
            print(f"[{i:>2}/{len(slugs)}] {slug:<26} no html", file=sys.stderr)
            continue
        record = score(slug, args.ir_dir, args.html_dir, args.layout_dir,
                       args.guide_dir if args.guide_dir.is_dir() else None,
                       args.work, str(args.source_root),
                       roundtrip=not args.assertions_only)
        records.append(record)
        failed = [k for k in ASSERTION_KEYS if not record.get(k)]
        assertions = f"assertions {record.get('assertions_held', 0)}/8"
        if record["status"] == "ok" and record.get("rules_pct") is not None:
            print(f"[{i:>2}/{len(slugs)}] {slug:<26} "
                  f"rules {record['rules_pct']:>6}%  text {record['text_pct']:>6}%  "
                  f"{assertions}  {','.join(failed)}", file=sys.stderr)
        elif record["status"] == "ok":
            print(f"[{i:>2}/{len(slugs)}] {slug:<26} {assertions}  "
                  f"{','.join(failed)}", file=sys.stderr)
        else:
            print(f"[{i:>2}/{len(slugs)}] {slug:<26} {assertions}  "
                  f"ERROR {str(record['error'])[:60]}", file=sys.stderr)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
