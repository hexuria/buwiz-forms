#!/usr/bin/env python3
"""Extract an exact geometry + typography IR from one pinned BIR form PDF.

This reads the PDF *content stream*, never a raster. Every number it emits is
the number the PDF itself carries, so the unembedded-font problem that blocks
pixel comparison does not arise: we record font identity and advance metrics,
not glyph outlines.

The IR is the contract consumed by lattice.py, emit.py and verify.py, and is
also what verify.py re-extracts from our own Chromium-printed PDF so the two
can be diffed numerically.

Usage:
    python3 tools/formgen/extract.py \
        --pdf "/path/2551Q Jan 2018 ENCS final rev 3_copy.pdf" \
        --form-code 2551Q --revision 2018 \
        --expected-sha256 <64 hex> \
        --out build/ir/2551q-2018.ir.json

    python3 tools/formgen/extract.py --self-test
"""

from __future__ import annotations

import argparse
import collections
import copy
import hashlib
import json
import math
import pathlib
import sys
from typing import Any, Callable, Sequence

try:
    import fitz  # PyMuPDF
except ImportError:  # pragma: no cover - environment guard
    sys.exit("PyMuPDF is required: pip install pymupdf")

# 2: pages gained `paths` (non-rectilinear ink), images gained `transform` and
# soft-mask fields, and text runs gained `unmapped_glyphs`.
SCHEMA_VERSION = 2

# Coordinates are quantised to this many decimal places before any grouping.
# The BIR generator emits values with at most 2dp, so this is lossless for the
# source and merely tames float noise on the Chromium round-trip side.
QUANT = 2

# A filled rect is treated as a rule when its short side is at or below this.
# Observed BIR rule thicknesses are 0.24 / 0.48 / 0.72 / 0.96 / 1.44 pt.
MAX_RULE_THICKNESS_PT = 1.5

# Two collinear segments join when the gap between them is at or below this.
# Joints are patched by exact corner squares, so a positive epsilon is only
# needed to absorb float error, not to bridge real gaps.
JOIN_EPSILON_PT = 0.011

# The floor on "these two points coincide": it decides where one subpath ends and
# the next begins, and it is the alignment tolerance for a zero-width path. The
# generator emits exact values, so this only absorbs float noise; a real segment
# is compared against its own stroke width instead, in is_bar_like.
AXIS_EPSILON_PT = 1e-6

# MuPDF reports this codepoint for a glyph it could not map to Unicode. It is
# the honest answer and the only one this module will substitute; see
# extract_text_runs.
UNMAPPED_CODEPOINT = "�"


def q(value: float) -> float:
    """Quantise a coordinate. Returns a float that round-trips through JSON."""
    return round(float(value) + 0.0, QUANT)


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


# ---------------------------------------------------------------------------
# Colour
# ---------------------------------------------------------------------------


def to_gray(color: Sequence[float] | None) -> float | None:
    """Collapse an RGB triple to a single tone, or None for 'no paint'.

    BIR greys are near-neutral (0.8509, 0.6509, 0.7489) but not exactly so; the
    channel spread is under 1e-3. Anything genuinely chromatic is kept as RGB by
    the caller and never reduced here.
    """
    if color is None:
        return None
    if len(color) == 1:
        return round(float(color[0]), 4)
    r, g, b = (float(c) for c in color[:3])
    if max(r, g, b) - min(r, g, b) > 0.01:
        return None
    return round((r + g + b) / 3.0, 4)


def classify_tone(gray: float | None) -> str:
    """Name a tone band. This is what tells a black rule from grey decoration.

    CLAUDE.md records that acting on raster ink-presence painted black over
    grey decoration. In the content stream the distinction is a literal value,
    so the classification is exact rather than inferred.
    """
    if gray is None:
        return "chromatic"
    if gray <= 0.15:
        return "structural"
    if gray >= 0.98:
        return "knockout"
    return "decorative"


# ---------------------------------------------------------------------------
# Rule extraction
# ---------------------------------------------------------------------------


class Segment:
    """One maximal axis-aligned filled bar."""

    __slots__ = ("axis", "near", "far", "start", "end", "gray", "rgb",
                 "paint_seq", "paint_seq_max")

    def __init__(self, axis: str, near: float, far: float, start: float, end: float,
                 gray: float | None, rgb: tuple[float, float, float] | None,
                 paint_seq: int, paint_seq_max: int) -> None:
        self.axis = axis      # "h" or "v"
        self.near = near      # y0 for h, x0 for v
        self.far = far        # y1 for h, x1 for v
        self.start = start    # x0 for h, y0 for v
        self.end = end        # x1 for h, y1 for v
        self.gray = gray
        self.rgb = rgb
        self.paint_seq = paint_seq          # first contributing op
        self.paint_seq_max = paint_seq_max  # last contributing op

    @property
    def thickness(self) -> float:
        return q(self.far - self.near)

    @property
    def length(self) -> float:
        return q(self.end - self.start)

    def to_ir(self, index: int) -> dict[str, Any]:
        if self.axis == "h":
            x0, y0, x1, y1 = self.start, self.near, self.end, self.far
        else:
            x0, y0, x1, y1 = self.near, self.start, self.far, self.end
        return {
            "id": f"{self.axis}{index}",
            "axis": self.axis,
            "x0": q(x0), "y0": q(y0), "x1": q(x1), "y1": q(y1),
            "thickness_pt": self.thickness,
            "length_pt": self.length,
            "gray": self.gray,
            "rgb": list(self.rgb) if self.rgb is not None else None,
            "role": classify_tone(self.gray),
            "paint_seq": self.paint_seq,
            "paint_seq_max": self.paint_seq_max,
        }


def merge_intervals(intervals: list[tuple[float, float, int]]
                    ) -> list[tuple[float, float, int, int]]:
    """Union 1-D intervals, joining anything within JOIN_EPSILON_PT.

    Each input carries the paint sequence of the op that contributed it, and
    each merged run reports the first and last sequence it spans. A bar the
    generator drew as fifteen short rects therefore still knows where it sits in
    the page's paint order, which is what lets emit.py reproduce the source's
    z-order instead of guessing at it from tone.
    """
    if not intervals:
        return []
    intervals.sort()
    merged = [[intervals[0][0], intervals[0][1], intervals[0][2], intervals[0][2]]]
    for start, end, seq in intervals[1:]:
        if start <= merged[-1][1] + JOIN_EPSILON_PT:
            merged[-1][1] = max(merged[-1][1], end)
            merged[-1][2] = min(merged[-1][2], seq)
            merged[-1][3] = max(merged[-1][3], seq)
        else:
            merged.append([start, end, seq, seq])
    return [(a, b, lo, hi) for a, b, lo, hi in merged]


class PaintOrder:
    """Every painting op on the page, numbered in content-stream order.

    get_drawings() is content-stream order but reports a path that both fills
    and strokes as *one* entry, and PDF paints those as two ops: the fill first,
    then the outline on top of it. Numbering by drawing index would therefore
    tie a checkbox's white interior to its own black border and leave the winner
    to a tiebreak -- which is how the interior came to erase the border.

    get_bboxlog() is the one view that lists fills, strokes and images together
    as separate entries in stream order, so the ordinal comes from there and the
    walk below reconciles it against get_drawings().
    """

    __slots__ = ("fill", "stroke", "images", "total")

    def __init__(self, fill: list[int], stroke: list[int],
                 images: list[tuple[fitz.Rect, int]], total: int) -> None:
        self.fill = fill        # per drawing: ordinal of its fill op, or -1
        self.stroke = stroke    # per drawing: ordinal of its stroke op, or -1
        self.images = images    # (placement, ordinal) in stream order
        self.total = total      # one past the last ordinal


def paint_order(page: fitz.Page, drawings: Sequence[dict[str, Any]]) -> PaintOrder:
    """Number every fill, stroke and image op on the page.

    Raises rather than guessing if the log and the drawings disagree: a silent
    fallback would emit a plausible document whose z-order is not the source's,
    and z-order is exactly what this data exists to reproduce.
    """
    fill = [-1] * len(drawings)
    stroke = [-1] * len(drawings)
    images: list[tuple[fitz.Rect, int]] = []
    ordinal = 0
    index = 0

    for kind, box in page.get_bboxlog():
        if kind == "fill-image":
            images.append((fitz.Rect(box), ordinal))
        elif kind == "fill-path":
            if index >= len(drawings):
                raise SystemExit("paint order desync: more fills than drawings")
            fill[index] = ordinal
            # A fill-only path is finished; a fill+stroke path keeps the slot
            # until its stroke arrives.
            if str(drawings[index].get("type", "")) == "f":
                index += 1
        elif kind == "stroke-path":
            if index >= len(drawings):
                raise SystemExit("paint order desync: more strokes than drawings")
            stroke[index] = ordinal
            index += 1
        else:
            continue
        ordinal += 1

    if index != len(drawings):
        raise SystemExit(
            f"paint order desync: consumed {index} of {len(drawings)} drawings")
    return PaintOrder(fill, stroke, images, ordinal)


def is_bar_like(p0: fitz.Point, p1: fitz.Point, thickness: float) -> bool:
    """Whether a line segment inks the same pixels as an axis-aligned bar.

    Exact alignment is the wrong test, and 2316 is why: twelve of its box
    separators are stroked segments that lean 0.17pt across 14.5pt, a third of
    their own 0.45pt stroke width. The bar and the segment cover the same ink, and
    the bar is what lattice.py has to see to find a box side, so calling those
    twelve "diagonal" would move real structure out of `rules` to no visual gain.

    A filled edge is the opposite case. It has no stroke width, so any lean at all
    is shape rather than rule -- which is exactly the 0605 triangle whose three
    edges the classifier was flattening into hairlines.
    """
    lean = min(abs(p0.x - p1.x), abs(p0.y - p1.y))
    return lean <= max(thickness, AXIS_EPSILON_PT)


def is_rectilinear(item: dict[str, Any]) -> bool:
    """Whether every op in this path is representable as an axis-aligned bar.

    A path that is not is more than mis-measured by the rule classifier below --
    it is silently *changed* by it. 0605 draws each "write here" marker as one
    filled triangle; forced through the classifier its three edges become three
    axis-aligned hairlines and the fill is discarded, so a solid black arrow
    prints as a light grey open "F". Such paths are extracted whole instead, by
    extract_paths.

    A curve is never bar-like: 0605's pre-printed decimal points are four `c` ops
    each and no bar describes a circle, however small.
    """
    thickness = float(item.get("width") or 0.0)
    for op in item["items"]:
        if op[0] == "re":
            continue
        if op[0] == "l" and is_bar_like(op[1], op[2], thickness):
            continue
        return False
    return True


def extract_segments(drawings: Sequence[dict[str, Any]], order: PaintOrder) -> list[Segment]:
    """Turn every filled rect into maximal horizontal and vertical bars.

    The BIR generator draws a long border as a run of short filled rects plus a
    square at each joint. A square is thin on *both* axes, so it is offered to
    both groupings; the interval union absorbs the double count and the joint
    disappears into the segments it was patching.
    """
    h_groups: dict[tuple[float, float, float | None, Any], list[tuple[float, float, int]]] = (
        collections.defaultdict(list))
    v_groups: dict[tuple[float, float, float | None, Any], list[tuple[float, float, int]]] = (
        collections.defaultdict(list))

    def offer(x0: float, y0: float, x1: float, y1: float,
              gray: float | None, rgb: Any, seq: int) -> None:
        """Route one axis-aligned bar into the horizontal and/or vertical group."""
        width, height = q(x1 - x0), q(y1 - y0)
        if width <= 0 or height <= 0:
            return
        if height <= MAX_RULE_THICKNESS_PT:
            h_groups[(y0, y1, gray, rgb)].append((x0, x1, seq))
        if width <= MAX_RULE_THICKNESS_PT:
            v_groups[(x0, x1, gray, rgb)].append((y0, y1, seq))

    for index, item in enumerate(drawings):
        if not is_rectilinear(item):
            continue  # extract_paths owns it, whole
        # A path may both fill and stroke. The fill paints bars and tint bands;
        # the stroke paints outlines. Forms differ in which they use -- 2551Q is
        # almost entirely filled bars, while 2316 draws 95 of its boxes as
        # stroked rectangles. Treating a stroked rectangle as "not a rule" loses
        # that form's entire structure, so both are handled here.
        # The two carry different ordinals because PDF paints them in that order.
        fill_seq = order.fill[index] if order.fill[index] >= 0 else order.stroke[index]
        stroke_seq = order.stroke[index] if order.stroke[index] >= 0 else order.fill[index]

        stroke = item.get("color")
        stroke_width = float(item.get("width") or 0.0)
        if stroke is not None and stroke_width > 0:
            s_gray = to_gray(stroke)
            s_rgb = tuple(round(float(c), 4) for c in stroke[:3]) if len(stroke) >= 3 else None
            half = stroke_width / 2.0
            for op in item["items"]:
                if op[0] != "re":
                    continue
                r = op[1]
                # Each edge of the outline is a bar centred on the rect's edge.
                for y in (r.y0, r.y1):
                    offer(q(r.x0 - half), q(y - half), q(r.x1 + half), q(y + half),
                          s_gray, s_rgb, stroke_seq)
                for x in (r.x0, r.x1):
                    offer(q(x - half), q(r.y0 - half), q(x + half), q(r.y1 + half),
                          s_gray, s_rgb, stroke_seq)

        fill = item.get("fill")
        if fill is None:
            fill = stroke
            if fill is None:
                continue
        gray = to_gray(fill)
        rgb = tuple(round(float(c), 4) for c in fill[:3]) if len(fill) >= 3 else None

        for op in item["items"]:
            if op[0] == "re":
                rect = op[1]
            elif op[0] == "l":
                # A zero-area line: give it the path's stroke width as thickness.
                p0, p1 = op[1], op[2]
                width = float(item.get("width") or 0.0) or 0.24
                if abs(p0.y - p1.y) <= abs(p0.x - p1.x):
                    rect = fitz.Rect(min(p0.x, p1.x), p0.y - width / 2,
                                     max(p0.x, p1.x), p0.y + width / 2)
                else:
                    rect = fitz.Rect(p0.x - width / 2, min(p0.y, p1.y),
                                     p0.x + width / 2, max(p0.y, p1.y))
            else:
                continue

            offer(q(rect.x0), q(rect.y0), q(rect.x1), q(rect.y1), gray, rgb, fill_seq)

    segments: list[Segment] = []
    for (near, far, gray, rgb), spans in h_groups.items():
        for start, end, lo, hi in merge_intervals(spans):
            segments.append(Segment("h", near, far, start, end, gray, rgb, lo, hi))
    for (near, far, gray, rgb), spans in v_groups.items():
        for start, end, lo, hi in merge_intervals(spans):
            segments.append(Segment("v", near, far, start, end, gray, rgb, lo, hi))

    # A lone joint square that merged into nothing is noise, not structure.
    segments = [s for s in segments if s.length > MAX_RULE_THICKNESS_PT]
    segments.sort(key=lambda s: (s.axis, s.near, s.start))
    return segments


def extract_area_fills(drawings: Sequence[dict[str, Any]],
                       order: PaintOrder) -> list[dict[str, Any]]:
    """Filled regions that are not rules: tint bands and white knockouts."""
    fills: list[dict[str, Any]] = []
    for index, item in enumerate(drawings):
        fill = item.get("fill")
        if fill is None or not is_rectilinear(item):
            continue
        seq = order.fill[index] if order.fill[index] >= 0 else order.stroke[index]
        gray = to_gray(fill)
        for op in item["items"]:
            if op[0] != "re":
                continue
            rect = op[1]
            width, height = q(rect.width), q(rect.height)
            if width <= MAX_RULE_THICKNESS_PT or height <= MAX_RULE_THICKNESS_PT:
                continue
            fills.append({
                "x0": q(rect.x0), "y0": q(rect.y0),
                "x1": q(rect.x1), "y1": q(rect.y1),
                "gray": gray,
                "rgb": [round(float(c), 4) for c in fill[:3]] if len(fill) >= 3 else None,
                "role": classify_tone(gray),
                "paint_seq": seq,
                "paint_seq_max": seq,
            })
    fills.sort(key=lambda f: (f["y0"], f["x0"]))
    return fills


# ---------------------------------------------------------------------------
# Non-rectilinear paths
# ---------------------------------------------------------------------------


def subpaths_of(items: Sequence[Sequence[Any]]) -> list[dict[str, Any]]:
    """Group one path's ops into subpaths, each with its own start point.

    get_drawings() reports no moveto, so the only evidence that a new subpath
    began is that an op does not start where the previous one ended. `re` and
    `qu` are always closed subpaths of their own.

    Coordinates follow SVG's convention -- the start point is stated once and
    each op then carries only the points that op introduces -- because that is
    also how the PDF operators are written, so nothing is derived here.

    `closed` is measured, not declared: get_drawings() carries a single
    closePath flag for the whole path, which says nothing about which subpath it
    applied to, while "the last point coincides with the first" is a fact.
    """
    subs: list[dict[str, Any]] = []
    current: dict[str, Any] | None = None
    cursor: fitz.Point | None = None

    for op in items:
        kind = op[0]
        if kind == "re":
            rect = op[1]
            subs.append({
                "start": [q(rect.x0), q(rect.y0)],
                "closed": True,
                "ops": [{"op": "re",
                         "points": [q(rect.x0), q(rect.y0), q(rect.x1), q(rect.y1)]}],
            })
            current, cursor = None, None
            continue
        if kind == "qu":
            corners = [op[1].ul, op[1].ur, op[1].lr, op[1].ll]
            subs.append({
                "start": [q(corners[0].x), q(corners[0].y)],
                "closed": True,
                "ops": [{"op": "l", "points": [q(p.x), q(p.y)]} for p in corners[1:]],
            })
            current, cursor = None, None
            continue
        if kind not in ("l", "c"):
            # Silently dropping an op would publish a path that is not the
            # source's while looking like one.
            raise SystemExit(f"unknown path op {kind!r}")

        points = list(op[1:])
        first, last = points[0], points[-1]
        if (current is None or cursor is None
                or abs(first.x - cursor.x) > AXIS_EPSILON_PT
                or abs(first.y - cursor.y) > AXIS_EPSILON_PT):
            current = {"start": [q(first.x), q(first.y)], "closed": False, "ops": []}
            subs.append(current)
        current["ops"].append({
            "op": kind,
            "points": [c for p in points[1:] for c in (q(p.x), q(p.y))],
        })
        cursor = last

    for sub in subs:
        if sub["closed"] or not sub["ops"]:
            continue
        tail = sub["ops"][-1]["points"]
        sub["closed"] = tail[-2:] == sub["start"]
    return subs


def extract_paths(drawings: Sequence[dict[str, Any]],
                  order: PaintOrder) -> list[dict[str, Any]]:
    """Paths that no axis-aligned bar can represent, kept whole and in paint order.

    A third kind of ink beside rules and area fills. Two families appear in this
    corpus and both were being lost:

      * The solid "write here" triangles (0605, 1600WP, 2550M, 2551M, 2553).
        These reached the rule classifier, which flattened each one into three
        hairlines and dropped the fill.
      * The pre-printed decimal points inside money boxes (0605, 2551M, 2553).
        These are filled Bezier circles about 1.7 x 1.5pt. They were dropped
        outright -- not, as it looked, because MAX_RULE_THICKNESS_PT rejected a
        shape that is thin on one axis only, but because only `re` and `l` ops
        ever reached either classifier and a circle is four `c` ops.

    Both colours are recorded separately: a path may fill and stroke, and 2551M's
    decimal points do both, so collapsing them to one "ink" would lose the fact
    that the mark is 0.72pt wider than its fill.
    """
    paths: list[dict[str, Any]] = []
    for index, item in enumerate(drawings):
        if is_rectilinear(item):
            continue
        fill = item.get("fill")
        stroke = item.get("color")
        stroke_width = float(item.get("width") or 0.0)
        if stroke is not None and stroke_width <= 0:
            stroke = None
        if fill is None and stroke is None:
            continue

        fill_gray = to_gray(fill)
        stroke_gray = to_gray(stroke)
        rect = item["rect"]
        # The fill lands under the stroke, so the first op is the fill's when
        # there is one -- the same reconciliation extract_segments makes.
        first = order.fill[index] if order.fill[index] >= 0 else order.stroke[index]
        last = order.stroke[index] if order.stroke[index] >= 0 else order.fill[index]
        paths.append({
            "id": None,  # assigned after the sort so ids read in document order
            "x0": q(rect.x0), "y0": q(rect.y0), "x1": q(rect.x1), "y1": q(rect.y1),
            "fill": [round(float(c), 4) for c in fill[:3]] if fill is not None and len(fill) >= 3 else None,
            "fill_gray": fill_gray,
            "stroke": [round(float(c), 4) for c in stroke[:3]] if stroke is not None and len(stroke) >= 3 else None,
            "stroke_gray": stroke_gray,
            "stroke_width_pt": q(stroke_width) if stroke is not None else 0.0,
            "even_odd": bool(item.get("even_odd")),
            # The tone of the ink that decides whether this mark is structure:
            # the fill when the path has one, otherwise the outline.
            "role": classify_tone(fill_gray if fill is not None else stroke_gray),
            "subpaths": subpaths_of(item["items"]),
            "paint_seq": first,
            "paint_seq_max": last,
        })
    paths.sort(key=lambda p: (p["y0"], p["x0"]))
    for position, path in enumerate(paths):
        path["id"] = f"path{position}"
    return paths


# ---------------------------------------------------------------------------
# Typography
# ---------------------------------------------------------------------------

# PyMuPDF span flag bits.
FLAG_SUPERSCRIPT = 1 << 0
FLAG_ITALIC = 1 << 1
FLAG_SERIF = 1 << 2
FLAG_MONOSPACE = 1 << 3
FLAG_BOLD = 1 << 4


def split_font_name(raw: str) -> dict[str, Any]:
    """Split a PDF BaseFont into family + declared style.

    'ABCDEE+Arial Narrow,Italic' -> family 'Arial Narrow', italic, subset tag
    'ABCDEE'. The subset tag is provenance only; it never reaches the CSS.
    """
    subset = None
    name = raw
    if "+" in name[:8]:
        subset, name = name.split("+", 1)
    style_bold = False
    style_italic = False
    if "," in name:
        name, _, styles = name.partition(",")
        for token in styles.split(","):
            token = token.strip().lower()
            if token in {"bold", "black", "heavy"}:
                style_bold = True
            elif token in {"italic", "oblique"}:
                style_italic = True
            elif token == "bolditalic":
                style_bold = style_italic = True
    elif name.endswith(("-Bold", "-Italic", "-BoldItalic")):
        base, _, suffix = name.rpartition("-")
        name = base
        style_bold = "Bold" in suffix
        style_italic = "Italic" in suffix
    return {
        "family": name.strip(),
        "declared_bold": style_bold,
        "declared_italic": style_italic,
        "subset_tag": subset,
    }


def has_tounicode(doc: fitz.Document, xref: int) -> bool:
    """Whether this font object carries a ToUnicode CMap."""
    got = doc.xref_get_key(xref, "ToUnicode")
    return bool(got) and got[0] != "null"


def unmapped_glyph_origins(page: fitz.Page) -> dict[tuple[float, float], int]:
    """Glyph origins where MuPDF could not map the drawn glyph to a codepoint.

    This exists because the two text views disagree, and the one extract_text_runs
    reads is the one that guesses. On 2550M page 4 and 2553 page 2, seven glyphs
    are drawn from a symbolic Wingdings face with no ToUnicode CMap:
    get_texttrace() reports them honestly as U+FFFD with glyph id 131, while
    get_text("rawdict") reports 'SECTION SIGN' -- the WinAnsi meaning of the byte
    0xA7, which this font does not use. A section sign looks like content, so the
    lie is not detectable downstream; the same glyph on 1601C, whose font carries
    a usable encoding, reads U+F0A7.

    The glyph id is the invariant across all three readings, so it is what gets
    carried. Ambiguous origins -- more than one glyph drawn at the same point --
    are dropped rather than resolved by a tiebreak, since the point of this map
    is to be certain about the glyph it names.
    """
    seen: dict[tuple[float, float], set[int]] = collections.defaultdict(set)
    for span in page.get_texttrace():
        for char in span["chars"]:
            codepoint, glyph_id, origin = char[0], char[1], char[2]
            if codepoint in (0, 0xFFFD):
                seen[(q(origin[0]), q(origin[1]))].add(glyph_id)
    return {key: next(iter(ids)) for key, ids in seen.items() if len(ids) == 1}


def font_table(page: fitz.Page, doc: fitz.Document) -> dict[str, dict[str, Any]]:
    """Every font resource on the page, keyed by BaseFont name.

    `embedded` records whether the outlines travel with the file. It is
    provenance, not a blocker: /Widths gives exact advances either way, and
    advances are what layout depends on.
    """
    table: dict[str, dict[str, Any]] = {}
    for xref, ext, ftype, basefont, resource, encoding, _ in page.get_fonts(full=True):
        parts = split_font_name(basefont)
        entry = {
            "basefont": basefont,
            "resource": resource,
            "type": ftype,
            "encoding": encoding,
            "embedded": ext not in ("n/a", "", None),
            "embedded_format": ext if ext not in ("n/a", "", None) else None,
            # Whether the file states what its codepoints mean. Without a
            # ToUnicode CMap the text reported below is MuPDF's derivation from
            # the font's own encoding, and for a symbolic face -- Wingdings,
            # Symbol -- that derivation can fail outright; see
            # unmapped_glyph_origins.
            "has_tounicode": has_tounicode(doc, xref),
            **parts,
        }
        try:
            descriptor = doc.xref_get_key(xref, "FontDescriptor")
            if descriptor and descriptor[0] == "xref":
                dref = int(descriptor[1].split()[0])
                for key in ("Flags", "StemV", "ItalicAngle", "CapHeight", "XHeight",
                            "Ascent", "Descent", "FontWeight"):
                    got = doc.xref_get_key(dref, key)
                    if got and got[0] in ("int", "float"):
                        entry[key.lower()] = float(got[1])
        except Exception:  # noqa: BLE001 - descriptor is best-effort provenance
            pass
        table[basefont] = entry
    return table


def extract_text_runs(page: fitz.Page) -> list[dict[str, Any]]:
    """Every visible text run with the metrics needed to reproduce its layout.

    This records only what the PDF itself states: glyph origins, per-glyph
    advances from /Widths, and the resulting run extent. It deliberately does
    NOT compute a "natural advance" or derive letter-spacing.

    An earlier version did, via fitz.Font(fontname="Arial,Bold"). MuPDF resolves
    only base-14 aliases, so that call raised for every real face, the exception
    was swallowed, and the field was null on all 310 runs of 2551Q while looking
    like a measurement. Worse, any fix using a locally installed Arial would make
    extraction machine-dependent, and determinism is the property that makes this
    pipeline worth having.

    Deriving tracking requires the metrics of the face we will actually ship, so
    it belongs to fonts.py, which reads the bundled WOFF2 directly. Everything
    here is a fact of the source file.

    `char_origin_offsets_pt` looks redundant beside `char_advances_pt` and is
    not. Every advance is quantised to QUANT places, so summing a prefix of them
    accumulates that rounding: measured across this corpus the accumulated error
    reaches 0.86pt on a 255-glyph run, eight times verify.py's advance tolerance.
    verify.py has to locate an *interior* glyph exactly whenever the rasteriser
    merges two runs into one span, and each offset here is a single subtraction
    rounded once, so it carries no accumulation at all.

    `unmapped_glyphs` is the one place this function overrules its source. Where
    MuPDF drew a glyph it could not map to a codepoint, `text` carries U+FFFD and
    the entry names the glyph id, rather than the plausible-looking character
    rawdict substitutes. Anything downstream then prints a visible replacement
    mark it can be told to fix, instead of a section sign nobody can tell is
    wrong. See unmapped_glyph_origins.
    """
    runs: list[dict[str, Any]] = []
    raw = page.get_text("rawdict")
    unmapped = unmapped_glyph_origins(page)

    for block in raw["blocks"]:
        if block["type"] != 0:
            continue
        for line in block["lines"]:
            direction = line.get("dir", (1.0, 0.0))
            for span in line["spans"]:
                chars = span.get("chars") or []
                unmapped_glyphs = []
                letters = []
                for position, char in enumerate(chars):
                    glyph_id = unmapped.get((q(char["origin"][0]), q(char["origin"][1])))
                    if glyph_id is None:
                        letters.append(char["c"])
                        continue
                    letters.append(UNMAPPED_CODEPOINT)
                    unmapped_glyphs.append({
                        "index": position,
                        "glyph_id": glyph_id,
                        "rawdict_codepoint": ord(char["c"]),
                    })
                text = "".join(letters)
                if not text.strip():
                    continue

                origins = [c["origin"][0] for c in chars]
                widths = [q(c["bbox"][2] - c["bbox"][0]) for c in chars]
                offsets = [q(o - origins[0]) for o in origins] if chars else []
                advances: list[float] = []
                for i in range(len(chars) - 1):
                    advances.append(q(origins[i + 1] - origins[i]))
                if chars:
                    advances.append(q(chars[-1]["bbox"][2] - origins[-1]))

                measured = q(chars[-1]["bbox"][2] - chars[0]["origin"][0]) if chars else 0.0
                size = round(float(span["size"]), 3)
                flags = int(span["flags"])
                parts = split_font_name(span["font"])
                runs.append({
                    "text": text,
                    "font": span["font"],
                    "family": parts["family"],
                    "size_pt": size,
                    "bold": bool(flags & FLAG_BOLD) or parts["declared_bold"],
                    "italic": bool(flags & FLAG_ITALIC) or parts["declared_italic"],
                    "serif": bool(flags & FLAG_SERIF),
                    "monospace": bool(flags & FLAG_MONOSPACE),
                    "superscript": bool(flags & FLAG_SUPERSCRIPT),
                    "flags": flags,
                    "color": span.get("color"),
                    "x0": q(span["bbox"][0]), "y0": q(span["bbox"][1]),
                    "x1": q(span["bbox"][2]), "y1": q(span["bbox"][3]),
                    "baseline_y": q(chars[0]["origin"][1]) if chars else None,
                    "origin_x": q(chars[0]["origin"][0]) if chars else None,
                    "ascender": round(float(span.get("ascender", 0.0)), 4),
                    "descender": round(float(span.get("descender", 0.0)), 4),
                    "line_height_pt": q(
                        (float(span.get("ascender", 0.0)) - float(span.get("descender", 0.0)))
                        * size),
                    "measured_advance_pt": measured,
                    "char_origin_offsets_pt": offsets,
                    "char_advances_pt": advances,
                    "char_widths_pt": widths,
                    "direction": [round(float(direction[0]), 4), round(float(direction[1]), 4)],
                    "rotated": abs(float(direction[1])) > 1e-6,
                    "unmapped_glyphs": unmapped_glyphs,
                })
    runs.sort(key=lambda r: (r["y0"], r["x0"]))
    return runs


# ---------------------------------------------------------------------------
# Raster artwork
# ---------------------------------------------------------------------------


def smask_xref(doc: fitz.Document, xref: int) -> int:
    """The soft-mask XObject shaping this image, or 0 when it has none."""
    got = doc.xref_get_key(xref, "SMask")
    if got and got[0] == "xref":
        return int(got[1].split()[0])
    return 0


def painted_pixmap(doc: fitz.Document, xref: int) -> fitz.Pixmap:
    """The image as the page actually paints it: soft mask composited in.

    fitz.Pixmap(doc, xref) and doc.extract_image(xref) both return the *base*
    image and discard the /SMask, and for this corpus the base image is not the
    picture. 1604E xref 39's base stream is 39 compressed bytes of flat black
    over 120x48 samples and its soft mask is entirely transparent, so the mark is
    invisible in the official and painting the base puts a black block across the
    pre-printed "Item:" label. Its neighbour xref 37 is grey 0xD9 (the 0.8509
    decorative tone) wherever the mask is opaque and black elsewhere, so painting
    the base frames the "For BIR Use Only" band in black.

    That black is /Matte padding: these masks declare Matte [0 0 0], meaning the
    samples are premultiplied against black, which is exactly the value the mask
    then removes. Compositing is therefore not a cosmetic improvement -- it is
    the only reading of the file that is correct.

    Any alpha channel MuPDF hands back on an *unmasked* image is dropped, because
    there it is an artefact of the decode rather than a statement of the file.
    """
    base = fitz.Pixmap(doc, xref)
    if base.alpha:
        base = fitz.Pixmap(base, 0)
    mask_xref = smask_xref(doc, xref)
    if not mask_xref:
        return base
    mask = fitz.Pixmap(doc, mask_xref)
    if (mask.width, mask.height) != (base.width, base.height):
        # fz_new_pixmap_from_color_and_mask needs matching extents. Every mask in
        # this corpus matches; refusing to guess keeps a future mismatch loud.
        raise SystemExit(
            f"soft mask {mask_xref} is {mask.width}x{mask.height}, "
            f"image {xref} is {base.width}x{base.height}")
    return fitz.Pixmap(base, mask)


def pixmap_sha256(pix: fitz.Pixmap) -> str | None:
    """Digest a pixmap as it appears on paper: composited over white.

    Factored out so the self-test can hash the *base* image with the same
    formula the IR uses for the painted one. Two spellings of one digest would
    drift, and then a compositing regression would read as a hash difference
    rather than as the missing mask it is.

    The white composite is what lets the digest survive a round trip. Under a
    transparent pixel the source keeps whatever RGB its base stream happened to
    carry -- usually black -- while Chromium flattens against the page when it
    re-embeds. Hashing raw samples therefore reported one barcode as two
    different pictures: on 1601-FQ, 1709 and 2200T a placement landed at exactly
    the right bbox, from exactly the right staged file, and still counted as
    missing. Compositing first asks the question that actually matters -- would
    a reader see the same thing -- and both sides then agree exactly.
    """
    if pix.colorspace is None:
        return None
    if pix.colorspace.n != 3:
        pix = fitz.Pixmap(fitz.csRGB, pix)
    samples = pix.samples
    if pix.alpha:
        stride = pix.n
        flat = bytearray(pix.width * pix.height * 3)
        for index in range(pix.width * pix.height):
            alpha = samples[index * stride + 3]
            for channel in range(3):
                value = samples[index * stride + channel]
                flat[index * 3 + channel] = (value * alpha + 255 * (255 - alpha)) // 255
        samples = bytes(flat)
    return hashlib.sha256(
        f"{pix.width}x{pix.height}:".encode() + samples).hexdigest()


def decoded_pixel_sha256(doc: fitz.Document, xref: int) -> str | None:
    """Hash an image's painted samples, normalised to RGB.

    Colourspace is normalised because a re-encode can legitimately change it
    (a greyscale seal round-tripping as RGB) while every visible sample is the
    same. Alpha survives only when the source declares a soft mask, because there
    it carries the shape: without it, 1604E's two masked images hash as
    indistinguishable flat black rectangles, so this digest -- the equality test
    -- would report a black block and the label it hides as the same picture.
    Returns None when the XObject cannot be decoded, which the caller must treat
    as "unknown", never "equal".
    """
    try:
        return pixmap_sha256(painted_pixmap(doc, xref))
    except Exception:  # noqa: BLE001 - undecodable is a real answer, not a failure
        return None


def asset_file_name(doc: fitz.Document, xref: int, payload: dict[str, Any]) -> str:
    """The filename an offline bundle stores this XObject under.

    Keyed to the *provenance* hash -- sha256 over the compressed base stream --
    because that is what pins an asset to exact reviewed bytes. Only the file's
    contents change when the source declares a soft mask, never its name, so
    emit.py's existing lookup keeps resolving. Base stream to soft mask is 1:1
    across all 51 forms, so two masked images cannot claim one name with
    different pixels.
    """
    extension = "png" if smask_xref(doc, xref) else payload.get("ext", "png")
    return f"{sha256_bytes(payload['image'])}.{extension}"


def asset_for_xref(doc: fitz.Document, xref: int) -> tuple[str, bytes] | None:
    """The filename and bytes an offline bundle must store for this XObject.

    This is the entry point for whatever writes the assets to disk. It exists
    because doc.extract_image(xref) -- the obvious call, and the one in use --
    returns the base image and silently discards the soft mask, which for nine
    forms means writing a black rectangle where the official prints a label.

    Returns None when the XObject cannot be read, which the caller must treat as
    "no asset", never as an empty one.
    """
    try:
        payload = doc.extract_image(xref)
    except Exception:  # noqa: BLE001 - a broken XObject must not stop the form
        return None
    name = asset_file_name(doc, xref, payload)
    if not smask_xref(doc, xref):
        return name, payload["image"]
    return name, painted_pixmap(doc, xref).tobytes("png")


class Placements:
    """The placement matrices on a page, consumable one per drawn instance.

    get_images() is keyed by xref and get_image_rects() reports only boxes, so the
    matrix has to come from get_image_info() and be matched back by box. Matches
    are consumed, so a form that places the same XObject twice gets each
    instance's own matrix rather than the first one twice.
    """

    __slots__ = ("_by_xref",)

    def __init__(self, page: fitz.Page) -> None:
        self._by_xref: dict[int, list[tuple[fitz.Rect, list[float]]]] = (
            collections.defaultdict(list))
        for info in page.get_image_info(xrefs=True):
            matrix = [round(float(v), 4) for v in info["transform"]]
            self._by_xref[int(info.get("xref") or 0)].append(
                (fitz.Rect(info["bbox"]), matrix))

    def take(self, xref: int, rect: fitz.Rect) -> list[float] | None:
        """The matrix that placed this box, or None when the views disagree.

        None is the honest answer; an invented identity matrix would claim the
        image is unflipped, which is the very error this field exists to fix.
        """
        candidates = self._by_xref.get(xref) or []
        for index, (box, matrix) in enumerate(candidates):
            if max(abs(box.x0 - rect.x0), abs(box.y0 - rect.y0),
                   abs(box.x1 - rect.x1), abs(box.y1 - rect.y1)) <= 0.05:
                del candidates[index]
                return matrix
        return None


def extract_images(page: fitz.Page, doc: fitz.Document,
                   order: PaintOrder) -> list[dict[str, Any]]:
    """Embedded XObjects with their placement, content hash and paint order.

    Hashing here is what lets emit.py carry the exact official bytes through and
    lets the offline verifier keep rejecting anything else.

    Two hashes, because they answer different questions. `sha256` is over the
    compressed stream and is the provenance identity: it pins an asset to exact
    reviewed bytes. `pixel_sha256` is over the decoded samples and is the
    *equality* test. Chromium re-encodes an image when it prints, so the stream
    hash changes while every pixel stays identical -- which is why the audit
    reported nine forms with missing artwork that was demonstrably present.
    Compare pixels to ask "is this the same picture"; compare streams to ask
    "is this the same file".

    `transform` is the full 6-element placement matrix, not the bounding box the
    other four fields give. Four forms place an image with a negative `d` --
    1600-PT's masthead, 2550M's, 2551M's and 2553's seal -- which is a vertical
    flip, and a box cannot express one, so the seal rendered upside down with its
    rim lettering reading bottom-to-top. The matrix is carried rather than a
    "flipped" flag so rotation and skew are covered too; 0605 already places its
    seal with a small non-zero `b`.
    """
    taken = [False] * len(order.images)
    placements = Placements(page)

    def sequence_of(rect: fitz.Rect) -> int:
        """The ordinal of the op that placed this rect, or one past the page.

        get_images() is keyed by xref, so the placement has to be matched back
        to the log by geometry. Falling past the end reproduces the behaviour
        that predates this field -- artwork on top of the rule layer -- which is
        the honest answer when the two views cannot be reconciled, rather than
        an invented position in the middle of them.
        """
        for i, (box, seq) in enumerate(order.images):
            if not taken[i] and max(abs(box.x0 - rect.x0), abs(box.y0 - rect.y0),
                                    abs(box.x1 - rect.x1), abs(box.y1 - rect.y1)) <= 0.05:
                taken[i] = True
                return seq
        return order.total

    images: list[dict[str, Any]] = []
    for info in page.get_images(full=True):
        xref = info[0]
        try:
            payload = doc.extract_image(xref)
        except Exception:  # noqa: BLE001
            continue
        pixel_digest = decoded_pixel_sha256(doc, xref)
        mask_xref = smask_xref(doc, xref)
        asset_file = asset_file_name(doc, xref, payload)
        for rect in page.get_image_rects(xref):
            seq = sequence_of(rect)
            images.append({
                "xref": xref,
                "name": info[7],
                "x0": q(rect.x0), "y0": q(rect.y0),
                "x1": q(rect.x1), "y1": q(rect.y1),
                "transform": placements.take(xref, rect),
                # An image is one op, so it spans no range of them.
                "paint_seq": seq, "paint_seq_max": seq,
                "width_px": info[2],
                "height_px": info[3],
                "bpc": info[4],
                "colorspace": info[5],
                "ext": payload.get("ext"),
                "sha256": sha256_bytes(payload["image"]),
                "pixel_sha256": pixel_digest,
                # The mask is part of the picture, so the file a bundle stores is
                # not the stream `sha256` identifies; see asset_for_xref.
                "smask_xref": mask_xref or None,
                "masked": bool(mask_xref),
                "asset_file": asset_file,
                "bytes": len(payload["image"]),
            })
    images.sort(key=lambda i: (i["y0"], i["x0"]))
    return images


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------


def extract_page(page: fitz.Page, doc: fitz.Document, index: int) -> dict[str, Any]:
    drawings = list(page.get_drawings())
    order = paint_order(page, drawings)
    segments = extract_segments(drawings, order)
    rules = [s.to_ir(i) for i, s in enumerate(segments)]
    paths = extract_paths(drawings, order)

    thicknesses = collections.Counter(r["thickness_pt"] for r in rules if r["role"] == "structural")
    box = page.mediabox
    return {
        "index": index,
        "width_pt": q(box.width),
        "height_pt": q(box.height),
        "rotation": page.rotation,
        "rules": rules,
        "area_fills": extract_area_fills(drawings, order),
        "paths": paths,
        "text_runs": extract_text_runs(page),
        "images": extract_images(page, doc, order),
        "stats": {
            "rules_total": len(rules),
            "paths_total": len(paths),
            "paths_filled": sum(1 for p in paths if p["fill"] is not None),
            "rules_horizontal": sum(1 for r in rules if r["axis"] == "h"),
            "rules_vertical": sum(1 for r in rules if r["axis"] == "v"),
            "rules_structural": sum(1 for r in rules if r["role"] == "structural"),
            "rules_decorative": sum(1 for r in rules if r["role"] == "decorative"),
            "structural_thickness_histogram": dict(sorted(thicknesses.items())),
            "drawings_raw": len(drawings),
        },
    }


def extract(pdf_path: pathlib.Path, form_code: str, revision: str,
            expected_sha256: str | None) -> dict[str, Any]:
    digest = sha256_file(pdf_path)
    if expected_sha256 and digest != expected_sha256.lower():
        raise SystemExit(
            f"PDF hash mismatch\n  expected {expected_sha256.lower()}\n  actual   {digest}")

    doc = fitz.open(pdf_path)
    pages = [extract_page(page, doc, i + 1) for i, page in enumerate(doc)]

    fonts: dict[str, dict[str, Any]] = {}
    for page in doc:
        fonts.update(font_table(page, doc))

    sizes = {(p["width_pt"], p["height_pt"]) for p in pages}
    return {
        "schema_version": SCHEMA_VERSION,
        "form": {"code": form_code, "revision": revision},
        "source": {
            "file": f"external:{pdf_path.name}",
            "sha256": digest,
            "bytes": pdf_path.stat().st_size,
            "page_count": doc.page_count,
        },
        "generator": {
            "producer": "tools/formgen/extract.py",
            "pymupdf_version": fitz.VersionBind,
            "mupdf_version": fitz.VersionFitz,
            "schema_version": SCHEMA_VERSION,
        },
        "paper": {
            "uniform": len(sizes) == 1,
            "width_pt": pages[0]["width_pt"],
            "height_pt": pages[0]["height_pt"],
            "distinct_sizes": sorted(f"{w}x{h}" for w, h in sizes),
        },
        "fonts": dict(sorted(fonts.items())),
        "pages": pages,
    }


# ---------------------------------------------------------------------------
# Self-test
# ---------------------------------------------------------------------------

# Where the pinned source PDFs live. The default matches batch.py's, so the
# self-test runs with no arguments -- which is how gate.py invokes it.
SELF_TEST_SOURCE_ROOT = pathlib.Path.home() / "Downloads/forms"

# code -> (path under the source root, revision, sha256 of the pinned bytes).
#
# The assertions below are measured against the real corpus, never a synthetic
# fixture: a fixture would encode what this module already believes, and every
# property here was established by reading these exact files. Each is pinned by
# hash, because a self-test that silently scored a different revision of a form
# would be worse than none.
#
# Naming forms here is not the per-form special-casing the constraints forbid.
# No extraction behaviour keys on a code; these are the *subjects* of the
# assertions rather than exceptions to them, and each fixture earns its place by
# exercising a property no other form does.
SELF_TEST_FIXTURES: dict[str, tuple[str, str, str]] = {
    # The reference form: paper, determinism, and 572 comb dividers.
    "2551Q": ("2551Qv2018/2551Q Jan 2018 ENCS final rev 3_copy.pdf", "2018",
              "1f270ecf66d778836a14697863e420ff65d5ed0a5576a6cf58b97c9a8e8c9b24"),
    # Non-rectilinear ink: filled triangles and Bezier decimal points.
    "0605": ("0605/0605version1999_09.02.2022_copy.pdf", "1999",
             "de04419766c59bf27fdeb854c0f7c3f98601900caa20630442e671e2313e536f"),
    # Soft masks: two /SMask placements whose base streams are /Matte padding.
    "1604E": ("1604Ev2018/1604E Jan 2018 ENCS Final2.pdf", "2018",
              "1db203442630c74ff4c95b509e204f542c5ba8fb1bd812440793e314ce709876"),
    # A vertically flipped placement, and four unmappable Wingdings glyphs.
    "2550M": ("2550M/bir2550m.pdf", "2007",
              "9fb4101ace8c781436dac85df138a8fb9790775291affe2dada030c490d0d2b6"),
    # The other three unmappable glyphs.
    "2553": ("2553v1999/42792553.pdf", "1999",
             "e52f96fe48aba2890078f889930744a4e13a4defe1284aa9c5292e2c702a20e5"),
    # Twelve leaning stroked separators that must stay rules.
    "2316": ("2316v2021/2316 Sep 2021 ENCS_Final_corrected.pdf", "2021",
             "8e927e65b096d7a786ba7d36c55c28ee3de3546278880d9de8c11a91d1b48d60"),
}

# (code, width_pt, height_pt, page_count). Folio, not A4 and not Letter.
SELF_TEST_PAPER = ("2551Q", 612.0, 936.0, 2)

# The form whose two extractions must serialise identically.
SELF_TEST_DETERMINISM_FORM = "2551Q"

# (code, image xref, its /SMask xref). The base stream here is 39 compressed
# bytes of flat black; the mask is the whole picture.
SELF_TEST_MASKED = ("1604E", 39, 40)

# (code, image xref) whose placement matrix has a negative `d` -- the seal that
# rendered upside-down while the IR carried only a bounding box.
SELF_TEST_FLIPPED = ("2550M", 51)

# 0605 page 1: 30 lone "write here" triangles plus 3 that share a path with a
# rect, and 10 pre-printed decimal points drawn as filled Bezier circles.
SELF_TEST_PATHS_FORM = "0605"
SELF_TEST_TRIANGLES = 33
SELF_TEST_DECIMAL_POINTS = 10

# The thickness extract_segments invents for a zero-width `l` op. 0605's ink is
# all triangles and circles, so after the diagonal paths stopped being forced
# axis-aligned the form carries no rule of this thickness at all -- the bucket
# was entirely the invented default.
SELF_TEST_PHANTOM_THICKNESS_PT = 0.24

# Characters whose presence in a run must be corroborated by the source's own
# glyph log: both are what a mis-read symbolic glyph looks like when it lands as
# something that reads as content.
SELF_TEST_CORROBORATED_CHARACTERS = {"?": 0x003F, "§": 0x00A7}

# The seven glyphs rawdict mis-reported. Wingdings glyph 131 with no ToUnicode
# CMap; rawdict offers the WinAnsi meaning of byte 0xA7, which this font does not
# use, so a section sign appears where the file states nothing at all.
SELF_TEST_RETEXTED_GLYPHS = {"2550M": 4, "2553": 3}
SELF_TEST_RETEXTED_GLYPH_ID = 131
SELF_TEST_RETEXTED_RAWDICT_CODEPOINT = 0x00A7

# 2316's stroked box separators: they lean up to 0.24pt across 14.96pt, well
# inside their own 0.45pt stroke width, so they ink the same pixels as a bar and
# must stay in `rules` where lattice.py can find a box side.
SELF_TEST_BAR_LIKE_FORM = "2316"
SELF_TEST_LEANING_BARS = 12


def texttrace_codepoints(page: fitz.Page) -> dict[tuple[float, float], set[int]]:
    """Every codepoint get_texttrace() reports, keyed by glyph origin.

    Deliberately *not* the view extract_text_runs reads. rawdict guesses a
    WinAnsi meaning for a byte in a symbolic font with no ToUnicode CMap, which
    is how seven glyphs came to read as section signs; get_texttrace() answers
    U+FFFD instead. Checking emitted text against this map is therefore a check
    against the file rather than against the same guess.
    """
    by_origin: dict[tuple[float, float], set[int]] = collections.defaultdict(set)
    for span in page.get_texttrace():
        for char in span["chars"]:
            by_origin[(q(char[2][0]), q(char[2][1]))].add(char[0])
    return dict(by_origin)


def mask_is_opaque(doc: fitz.Document, smask_xref: int | None) -> bool:
    """True when a soft mask hides nothing, so compositing it changes no pixel."""
    if not smask_xref:
        return False
    try:
        mask = fitz.Pixmap(doc, smask_xref)
    except Exception:  # noqa: BLE001 - undecodable is not "opaque"
        return False
    stride = mask.n
    samples = mask.samples
    return all(samples[i] == 255 for i in range(0, len(samples), stride))


def base_pixel_sha256(doc: fitz.Document, xref: int) -> str | None:
    """The digest of an image with its soft mask deliberately discarded.

    This is what the IR used to carry, and the only way to prove compositing
    actually happened: 1604E's masked base streams are flat /Matte padding, so a
    painted digest equal to this one means the mask was dropped again.
    """
    try:
        base = fitz.Pixmap(doc, xref)
        if base.alpha:
            base = fitz.Pixmap(base, 0)
        return pixmap_sha256(base)
    except Exception:  # noqa: BLE001 - undecodable is a real answer here too
        return None


def leaning_bars(page: fitz.Page) -> list[dict[str, Any]]:
    """Every `l` op that leans off-axis, with the bar it must have become.

    The bar geometry is re-derived from the op rather than read back out of
    extract_segments, so the check compares two independent statements: what the
    content stream draws, and what the IR ended up holding.
    """
    bars: list[dict[str, Any]] = []
    for item in page.get_drawings():
        width = float(item.get("width") or 0.0)
        for op in item["items"]:
            if op[0] != "l":
                continue
            p0, p1 = op[1], op[2]
            lean = min(abs(p0.x - p1.x), abs(p0.y - p1.y))
            if lean <= AXIS_EPSILON_PT:
                continue
            half = width / 2.0
            if abs(p0.y - p1.y) <= abs(p0.x - p1.x):
                bar = {"axis": "h", "near": q(p0.y - half), "far": q(p0.y + half),
                       "start": q(min(p0.x, p1.x)), "end": q(max(p0.x, p1.x))}
            else:
                bar = {"axis": "v", "near": q(p0.x - half), "far": q(p0.x + half),
                       "start": q(min(p0.y, p1.y)), "end": q(max(p0.y, p1.y))}
            bar.update(page=page.number + 1, lean_pt=round(lean, 3),
                       stroke_width_pt=q(width),
                       bar_like=is_bar_like(p0, p1, width),
                       rectilinear=is_rectilinear(item))
            bars.append(bar)
    return bars


def paint_order_desync_probes(page: fitz.Page,
                              drawings: list[dict[str, Any]]) -> dict[str, bool]:
    """Whether paint_order refuses a drawings list the bbox log cannot explain.

    The reconciliation must raise rather than fall back: a fallback publishes a
    plausible document whose z-order is not the source's, and z-order is the
    whole reason this data exists. Both directions are probed because they trip
    different branches -- a short list runs out of slots mid-log, a long one is
    left holding an unconsumed drawing.
    """
    probes: dict[str, bool] = {}
    for name, mutated in (("fewer_drawings_than_ops", drawings[:-1]),
                          ("more_drawings_than_ops", [*drawings, drawings[-1]])):
        try:
            paint_order(page, mutated)
        except SystemExit:
            probes[name] = True
        else:
            probes[name] = False
    return probes


def gather_evidence(source_root: pathlib.Path) -> dict[str, Any]:
    """Extract every fixture once, plus the source facts the checks compare to.

    Everything a check reads lives in this bundle, and nothing in it is a live
    fitz object. That is what lets mutation_probes deep-copy it, break one
    property, and watch exactly one check trip.
    """
    evidence: dict[str, Any] = {
        "ir": {}, "serialisations": [], "base_pixel_sha256": {},
        "mask_is_opaque": {},
        "codepoints": {}, "leaning_bars": {}, "desync": {},
    }
    for code, (relative, revision, digest) in SELF_TEST_FIXTURES.items():
        path = source_root / relative
        # extract() verifies the pin, so a swapped file fails here by name.
        evidence["ir"][code] = extract(path, code, revision, digest)
        doc = fitz.open(path)
        evidence["codepoints"][code] = {
            page.number + 1: texttrace_codepoints(page) for page in doc}
        evidence["leaning_bars"][code] = [bar for page in doc
                                          for bar in leaning_bars(page)]
        first = doc[0]
        evidence["desync"][code] = paint_order_desync_probes(
            first, list(first.get_drawings()))
        evidence["base_pixel_sha256"][code] = {
            image["xref"]: base_pixel_sha256(doc, image["xref"])
            for page in evidence["ir"][code]["pages"] for image in page["images"]}
        # A soft mask may be declared and still be entirely opaque -- 2316's is,
        # all 12,960 of its alpha samples being 255 -- and then compositing it is
        # a no-op and the painted digest legitimately equals the base image's.
        # Without this the self-test demanded a difference that cannot exist.
        evidence["mask_is_opaque"][code] = {
            image["xref"]: mask_is_opaque(doc, image.get("smask_xref"))
            for page in evidence["ir"][code]["pages"] for image in page["images"]}
        doc.close()

    code = SELF_TEST_DETERMINISM_FORM
    relative, revision, digest = SELF_TEST_FIXTURES[code]
    again = extract(source_root / relative, code, revision, digest)
    evidence["serialisations"] = [
        json.dumps(evidence["ir"][code], ensure_ascii=False),
        json.dumps(again, ensure_ascii=False),
    ]
    return evidence


def check_determinism(evidence: dict[str, Any]) -> list[str]:
    """Two extractions of one PDF must serialise to the same bytes."""
    payloads = evidence["serialisations"]
    if len(payloads) != 2:
        return [f"determinism needs two extractions, got {len(payloads)}"]
    if payloads[0] != payloads[1]:
        return [f"two extractions of {SELF_TEST_DETERMINISM_FORM} differ: "
                f"{len(payloads[0])} vs {len(payloads[1])} chars"]
    return []


def check_paper(evidence: dict[str, Any]) -> list[str]:
    """Paper is the PDF's own MediaBox, per page, exactly."""
    code, width, height, page_count = SELF_TEST_PAPER
    ir = evidence["ir"][code]
    paper = ir["paper"]
    failures: list[str] = []
    if (paper["width_pt"], paper["height_pt"]) != (width, height):
        failures.append(f"{code} paper {paper['width_pt']}x{paper['height_pt']}pt "
                        f"!= {width}x{height}pt")
    if not paper["uniform"]:
        failures.append(f"{code} paper is not uniform: {paper['distinct_sizes']}")
    if len(ir["pages"]) != page_count:
        failures.append(f"{code} has {len(ir['pages'])} pages, expected {page_count}")
    if ir["source"]["page_count"] != page_count:
        failures.append(f"{code} source page_count {ir['source']['page_count']} "
                        f"!= {page_count}")
    off = [page["index"] for page in ir["pages"]
           if (page["width_pt"], page["height_pt"]) != (width, height)]
    if off:
        failures.append(f"{code} pages {off} are not {width}x{height}pt")
    return failures


def check_paint_seq(evidence: dict[str, Any]) -> list[str]:
    """Every piece of ink knows where it sits in the page's paint order."""
    failures: list[str] = []
    for code, ir in evidence["ir"].items():
        for page in ir["pages"]:
            for kind in ("rules", "area_fills", "paths", "images"):
                for position, item in enumerate(page[kind]):
                    first = item.get("paint_seq")
                    last = item.get("paint_seq_max")
                    label = f"{code} p{page['index']} {kind}[{position}]"
                    if not isinstance(first, int) or first < 0:
                        failures.append(f"{label} paint_seq is {first!r}")
                    elif not isinstance(last, int) or last < first:
                        failures.append(f"{label} paint_seq_max {last!r} < {first}")
    return failures


def check_paint_order_reconciliation(evidence: dict[str, Any]) -> list[str]:
    """The reconciliation raises on disagreement; it never falls back."""
    failures: list[str] = []
    for code, probes in evidence["desync"].items():
        if not probes:
            failures.append(f"{code}: no reconciliation probe ran")
        for name, raised in sorted(probes.items()):
            if not raised:
                failures.append(f"{code}: paint_order accepted a desynced "
                                f"drawings list ({name})")
    return failures


def check_soft_masks(evidence: dict[str, Any]) -> list[str]:
    """A masked placement reports its mask, and its digest is the composite's."""
    failures: list[str] = []
    code, xref, mask_xref = SELF_TEST_MASKED
    named = [image for page in evidence["ir"][code]["pages"]
             for image in page["images"] if image["xref"] == xref]
    if not named:
        failures.append(f"{code} xref {xref} is not placed on any page")
    for image in named:
        if not image["masked"] or image["smask_xref"] != mask_xref:
            failures.append(f"{code} xref {xref} reports masked="
                            f"{image['masked']} smask_xref={image['smask_xref']}, "
                            f"expected True/{mask_xref}")

    # The general property, over every masked placement in every fixture: the
    # painted digest must differ from the base stream's own bytes and from the
    # base image's pixels, or the mask was discarded somewhere.
    for code, ir in evidence["ir"].items():
        for page in ir["pages"]:
            for image in page["images"]:
                if not image["masked"]:
                    continue
                label = f"{code} p{page['index']} xref {image['xref']}"
                if not image["smask_xref"]:
                    failures.append(f"{label} is masked with no smask_xref")
                if image["pixel_sha256"] is None:
                    failures.append(f"{label} has no pixel_sha256 to compare")
                    continue
                if image["pixel_sha256"] == image["sha256"]:
                    failures.append(f"{label} pixel digest equals the compressed "
                                    f"stream digest")
                base = evidence["base_pixel_sha256"][code].get(image["xref"])
                opaque = evidence["mask_is_opaque"][code].get(image["xref"])
                if base is None:
                    failures.append(f"{label} base image could not be decoded")
                elif image["pixel_sha256"] == base and not opaque:
                    failures.append(f"{label} pixel digest equals the unmasked "
                                    f"base image's -- the soft mask was dropped")
                elif image["pixel_sha256"] != base and opaque:
                    # The converse is worth asserting too, or the exemption
                    # below could hide a compositing bug on the very images it
                    # excuses.
                    failures.append(f"{label} has a fully opaque mask yet its "
                                    f"digest differs from the base image's")
                if not image["asset_file"].endswith(".png"):
                    failures.append(f"{label} asset {image['asset_file']} cannot "
                                    f"carry alpha")
    return failures


def check_transforms(evidence: dict[str, Any]) -> list[str]:
    """Every placement carries its full matrix, and the flip is still negative."""
    failures: list[str] = []
    for code, ir in evidence["ir"].items():
        for page in ir["pages"]:
            for image in page["images"]:
                matrix = image["transform"]
                if not isinstance(matrix, list) or len(matrix) != 6:
                    failures.append(f"{code} p{page['index']} xref {image['xref']} "
                                    f"transform is {matrix!r}, expected 6 elements")

    code, xref = SELF_TEST_FLIPPED
    matrices = [image["transform"] for page in evidence["ir"][code]["pages"]
                for image in page["images"] if image["xref"] == xref]
    if not matrices:
        failures.append(f"{code} xref {xref} is not placed on any page")
    for matrix in matrices:
        if not isinstance(matrix, list) or len(matrix) != 6:
            continue  # already reported above
        if matrix[3] >= 0:
            failures.append(f"{code} xref {xref} has d={matrix[3]}, expected a "
                            f"negative `d` (the vertical flip)")
    return failures


def is_filled_triangle(path: dict[str, Any]) -> bool:
    """A filled path holding a closed three-segment straight-line subpath.

    Three of 0605's thirty markers share their path with a rect, so the test is
    on the subpath rather than on the whole path's op census.
    """
    if path["fill"] is None:
        return False
    return any(sub["closed"] and len(sub["ops"]) == 3
               and all(op["op"] == "l" for op in sub["ops"])
               for sub in path["subpaths"])


def is_filled_curve_mark(path: dict[str, Any]) -> bool:
    """A filled path made only of curves: the pre-printed decimal points."""
    if path["fill"] is None:
        return False
    ops = [op for sub in path["subpaths"] for op in sub["ops"]]
    return bool(ops) and all(op["op"] == "c" for op in ops)


def check_paths(evidence: dict[str, Any]) -> list[str]:
    """Non-rectilinear ink survives whole, and invents no hairlines on the way."""
    ir = evidence["ir"][SELF_TEST_PATHS_FORM]
    page = ir["pages"][0]
    failures: list[str] = []

    triangles = [path["id"] for path in page["paths"] if is_filled_triangle(path)]
    if len(triangles) != SELF_TEST_TRIANGLES:
        failures.append(f"{SELF_TEST_PATHS_FORM} page 1 has {len(triangles)} filled "
                        f"triangle paths, expected {SELF_TEST_TRIANGLES}")
    marks = [path["id"] for path in page["paths"] if is_filled_curve_mark(path)]
    if len(marks) != SELF_TEST_DECIMAL_POINTS:
        failures.append(f"{SELF_TEST_PATHS_FORM} page 1 has {len(marks)} filled "
                        f"decimal-point marks, expected {SELF_TEST_DECIMAL_POINTS}")

    phantom = [f"p{page['index']}:{rule['id']}" for page in ir["pages"]
               for rule in page["rules"]
               if rule["thickness_pt"] == SELF_TEST_PHANTOM_THICKNESS_PT]
    if phantom:
        failures.append(f"{SELF_TEST_PATHS_FORM} carries {len(phantom)} rule(s) at "
                        f"the invented {SELF_TEST_PHANTOM_THICKNESS_PT}pt default "
                        f"({', '.join(phantom[:5])})")
    return failures


def check_codepoints(evidence: dict[str, Any]) -> list[str]:
    """No run holds a character the source did not state.

    The corroboration is against get_texttrace()'s glyph log, not against the
    rawdict reading the run came from, because the two disagree and rawdict is
    the one that guesses.
    """
    failures: list[str] = []
    for code, ir in evidence["ir"].items():
        by_page = evidence["codepoints"][code]
        for page in ir["pages"]:
            stated = by_page.get(page["index"]) or {}
            for run in page["text_runs"]:
                offsets = run["char_origin_offsets_pt"]
                for index, char in enumerate(run["text"]):
                    expected = SELF_TEST_CORROBORATED_CHARACTERS.get(char)
                    if expected is None:
                        continue
                    label = (f"{code} p{page['index']} {char!r} in "
                             f"{run['text'][:32]!r}")
                    if run["origin_x"] is None or index >= len(offsets):
                        failures.append(f"{label} has no origin to corroborate")
                        continue
                    origin = (q(run["origin_x"] + offsets[index]), run["baseline_y"])
                    drawn = stated.get(origin)
                    if drawn is None:
                        failures.append(f"{label} sits at {origin}, which the "
                                        f"source's glyph log does not mention")
                    elif expected not in drawn:
                        failures.append(f"{label} where the source drew "
                                        f"{sorted(hex(c) for c in drawn)}")

    for code, ir in evidence["ir"].items():
        expected = SELF_TEST_RETEXTED_GLYPHS.get(code, 0)
        carried = [(page["index"], run) for page in ir["pages"]
                   for run in page["text_runs"] if run["unmapped_glyphs"]]
        total = sum(len(run["unmapped_glyphs"]) for _, run in carried)
        if total != expected:
            failures.append(f"{code} carries {total} unmapped glyph(s), "
                            f"expected {expected}")
        for index, run in carried:
            for glyph in run["unmapped_glyphs"]:
                label = f"{code} p{index} glyph {glyph['index']}"
                if glyph["glyph_id"] != SELF_TEST_RETEXTED_GLYPH_ID:
                    failures.append(f"{label} is glyph id {glyph['glyph_id']}, "
                                    f"expected {SELF_TEST_RETEXTED_GLYPH_ID}")
                if run["text"][glyph["index"]] != UNMAPPED_CODEPOINT:
                    failures.append(f"{label} reads "
                                    f"{run['text'][glyph['index']]!r}, expected "
                                    f"{UNMAPPED_CODEPOINT!r}")
                if glyph["rawdict_codepoint"] != SELF_TEST_RETEXTED_RAWDICT_CODEPOINT:
                    failures.append(
                        f"{label} records rawdict codepoint "
                        f"{hex(glyph['rawdict_codepoint'])}, expected "
                        f"{hex(SELF_TEST_RETEXTED_RAWDICT_CODEPOINT)} -- the "
                        f"substitution this field exists to record")
    return failures


def bar_matches_rule(rule: dict[str, Any], bar: dict[str, Any]) -> bool:
    """Whether this rule is the bar a leaning segment must have become.

    The rule may be longer than the segment: extract_segments unions collinear
    spans, so a separator drawn as one op merges with anything it abuts. The
    near/far edges are exact; the length is containment.
    """
    if rule["axis"] != bar["axis"]:
        return False
    if rule["axis"] == "h":
        near, far, start, end = rule["y0"], rule["y1"], rule["x0"], rule["x1"]
    else:
        near, far, start, end = rule["x0"], rule["x1"], rule["y0"], rule["y1"]
    return (abs(near - bar["near"]) <= JOIN_EPSILON_PT
            and abs(far - bar["far"]) <= JOIN_EPSILON_PT
            and start <= bar["start"] + JOIN_EPSILON_PT
            and end >= bar["end"] - JOIN_EPSILON_PT)


def check_bar_like(evidence: dict[str, Any]) -> list[str]:
    """A segment leaning less than its own stroke width stays a rule.

    2316's twelve box separators are the case that makes exact axis alignment the
    wrong test: they lean up to 0.24pt over as much as 14.96pt, a third of their
    0.45pt stroke width, so they ink the same pixels as a bar. Diverting them to
    `paths` would move real structure out of lattice.py's reach for no visual
    gain, so this asserts they are still rules -- and that the form gained no
    paths at all.
    """
    code = SELF_TEST_BAR_LIKE_FORM
    ir = evidence["ir"][code]
    bars = evidence["leaning_bars"][code]
    failures: list[str] = []

    if len(bars) != SELF_TEST_LEANING_BARS:
        failures.append(f"{code} draws {len(bars)} leaning segment(s), expected "
                        f"{SELF_TEST_LEANING_BARS}")

    pages = {page["index"]: page for page in ir["pages"]}
    for bar in bars:
        where = (f"{code} p{bar['page']} {bar['axis']} at {bar['near']} "
                 f"{bar['start']}->{bar['end']} (lean {bar['lean_pt']}pt of "
                 f"{bar['stroke_width_pt']}pt stroke)")
        if not bar["bar_like"]:
            failures.append(f"{where} is no longer bar-like")
        if not bar["rectilinear"]:
            failures.append(f"{where} was diverted to extract_paths")
        page = pages.get(bar["page"])
        if page is None:
            failures.append(f"{where} is on a page the IR does not have")
            continue
        matched = [rule["id"] for rule in page["rules"]
                   if bar_matches_rule(rule, bar)]
        if len(matched) != 1:
            failures.append(f"{where} matches {len(matched)} rule(s) {matched[:4]}, "
                            f"expected exactly 1")

    diverted = [f"p{page['index']}:{len(page['paths'])}" for page in ir["pages"]
                if page["paths"]]
    if diverted:
        failures.append(f"{code} gained non-rectilinear paths ({', '.join(diverted)}); "
                        f"its ink is entirely rects and bar-like segments")
    return failures


SELF_TEST_CHECKS: tuple[tuple[str, Callable[[dict[str, Any]], list[str]]], ...] = (
    ("determinism", check_determinism),
    ("paper", check_paper),
    ("paint-seq", check_paint_seq),
    ("paint-order-reconciliation", check_paint_order_reconciliation),
    ("soft-masks", check_soft_masks),
    ("transforms", check_transforms),
    ("paths", check_paths),
    ("codepoints", check_codepoints),
    ("is-bar-like", check_bar_like),
)


def mutate_determinism(evidence: dict[str, Any]) -> None:
    evidence["serialisations"][1] += " "


def mutate_paper(evidence: dict[str, Any]) -> None:
    evidence["ir"][SELF_TEST_PAPER[0]]["paper"]["height_pt"] = 792.0


def mutate_paint_seq(evidence: dict[str, Any]) -> None:
    del evidence["ir"]["2551Q"]["pages"][0]["rules"][0]["paint_seq"]


def mutate_paint_order_reconciliation(evidence: dict[str, Any]) -> None:
    evidence["desync"]["2551Q"]["fewer_drawings_than_ops"] = False


def mutate_soft_masks(evidence: dict[str, Any]) -> None:
    """Restore the pre-compositing digest: the mask silently dropped again."""
    code, xref, _ = SELF_TEST_MASKED
    for page in evidence["ir"][code]["pages"]:
        for image in page["images"]:
            if image["xref"] == xref:
                image["pixel_sha256"] = evidence["base_pixel_sha256"][code][xref]


def mutate_transforms(evidence: dict[str, Any]) -> None:
    """Flip the flip back, as a bounding box would have."""
    code, xref = SELF_TEST_FLIPPED
    for page in evidence["ir"][code]["pages"]:
        for image in page["images"]:
            if image["xref"] == xref and image["transform"]:
                image["transform"][3] = abs(image["transform"][3])


def mutate_paths(evidence: dict[str, Any]) -> None:
    """Drop one marker, as the rule classifier used to."""
    page = evidence["ir"][SELF_TEST_PATHS_FORM]["pages"][0]
    for position, path in enumerate(page["paths"]):
        if is_filled_triangle(path):
            del page["paths"][position]
            return


def mutate_codepoints(evidence: dict[str, Any]) -> None:
    """Print the section sign rawdict offered, where the file states nothing."""
    for page in evidence["ir"]["2550M"]["pages"]:
        for run in page["text_runs"]:
            if run["unmapped_glyphs"]:
                index = run["unmapped_glyphs"][0]["index"]
                run["text"] = (run["text"][:index] + "§" + run["text"][index + 1:])
                return


def mutate_bar_like(evidence: dict[str, Any]) -> None:
    """Divert one leaning separator to paths, as exact alignment would have."""
    code = SELF_TEST_BAR_LIKE_FORM
    bar = evidence["leaning_bars"][code][0]
    for page in evidence["ir"][code]["pages"]:
        if page["index"] != bar["page"]:
            continue
        page["rules"] = [rule for rule in page["rules"]
                         if not bar_matches_rule(rule, bar)]


SELF_TEST_MUTATIONS: tuple[tuple[str, str, Callable[[dict[str, Any]], None]], ...] = (
    ("determinism", "one serialisation gains a byte", mutate_determinism),
    ("paper", "2551Q's height becomes Letter", mutate_paper),
    ("paint-seq", "a rule loses its paint_seq", mutate_paint_seq),
    ("paint-order-reconciliation", "a desync is accepted instead of raising",
     mutate_paint_order_reconciliation),
    ("soft-masks", "a masked image reports its unmasked pixels", mutate_soft_masks),
    ("transforms", "2550M's seal loses its vertical flip", mutate_transforms),
    ("paths", "one filled triangle is dropped", mutate_paths),
    ("codepoints", "an unmappable glyph prints as a section sign", mutate_codepoints),
    ("is-bar-like", "a leaning separator loses its rule", mutate_bar_like),
)


def mutation_probes(evidence: dict[str, Any], stream: Any) -> list[str]:
    """Confirm every check can fail, by breaking its subject and re-running it.

    A self-test that cannot fail is worthless, and every assertion above is of
    the form "the corpus says X" -- so the only way to know one is wired to the
    corpus is to change the corpus in memory and watch it trip. Each probe also
    requires that *only* its own check trips, which is what stops a broad check
    from standing in for a missing one.
    """
    checks = dict(SELF_TEST_CHECKS)
    failures: list[str] = []
    if set(name for name, _, _ in SELF_TEST_MUTATIONS) != set(checks):
        failures.append("every check needs a mutation that trips it; "
                        f"{sorted(set(checks) - {n for n, _, _ in SELF_TEST_MUTATIONS})} "
                        "have none")
    for name, description, mutate in SELF_TEST_MUTATIONS:
        broken = copy.deepcopy(evidence)
        mutate(broken)
        tripped = sorted(other for other, check in checks.items() if check(broken))
        if name not in tripped:
            failures.append(f"mutation '{description}' did not trip {name}")
        extra = [other for other in tripped if other != name]
        if extra:
            failures.append(f"mutation '{description}' also tripped {extra}")
        print(f"  probe {name:<27} {'OK' if not extra and name in tripped else 'WEAK'}"
              f"  ({description})", file=stream)
    return failures


def self_test(source_root: pathlib.Path) -> int:
    """Assert Round 1's properties against the real PDFs, then prove they can fail.

    Absence is a failure, not a skip: a self-test that quietly passes because it
    could not find its sources is the same green tick this project has already
    been burned by.
    """
    missing = [f"{code}: {source_root / relative}"
               for code, (relative, _, _) in SELF_TEST_FIXTURES.items()
               if not (source_root / relative).is_file()]
    if missing:
        print(f"self-test cannot run -- {len(missing)} source PDF(s) absent under "
              f"{source_root}:", file=sys.stderr)
        for entry in missing:
            print(f"  {entry}", file=sys.stderr)
        print("Pass --source-root if the pinned PDFs live elsewhere.", file=sys.stderr)
        return 2

    evidence = gather_evidence(source_root)
    failures: list[str] = []
    for name, check in SELF_TEST_CHECKS:
        found = check(evidence)
        failures.extend(found)
        print(f"  {name:<27} {'PASS' if not found else f'{len(found)} FAILURE(S)'}",
              file=sys.stderr)
        for message in found:
            print(f"    FAIL {message}", file=sys.stderr)

    weak = mutation_probes(evidence, sys.stderr)
    failures.extend(weak)
    for message in weak:
        print(f"    FAIL {message}", file=sys.stderr)

    print(f"self-test: {'PASS' if not failures else f'{len(failures)} FAILURE(S)'} "
          f"over {len(SELF_TEST_FIXTURES)} pinned PDFs", file=sys.stderr)
    return 1 if failures else 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    # --pdf, --form-code and --revision describe one conversion, so they are
    # required for one -- but --self-test names its own pinned corpus and takes
    # none of them, which is how gate.py invokes it.
    parser.add_argument("--pdf", type=pathlib.Path, default=None)
    parser.add_argument("--form-code", default=None)
    parser.add_argument("--revision", default=None)
    parser.add_argument("--expected-sha256", default=None,
                        help="Fail unless the PDF hashes to this. Omit only while exploring.")
    parser.add_argument("--out", type=pathlib.Path, default=None,
                        help="Write IR JSON here (default: stdout).")
    parser.add_argument("--summary", action="store_true",
                        help="Print a human-readable summary to stderr.")
    parser.add_argument("--self-test", action="store_true",
                        help="Assert this module's properties against the pinned "
                             "source PDFs and exit non-zero on failure.")
    parser.add_argument("--source-root", type=pathlib.Path,
                        default=SELF_TEST_SOURCE_ROOT,
                        help="Where --self-test looks for the pinned PDFs.")
    args = parser.parse_args(argv)

    if args.self_test:
        return self_test(args.source_root)

    absent = [name for name, value in (("--pdf", args.pdf),
                                       ("--form-code", args.form_code),
                                       ("--revision", args.revision)) if value is None]
    if absent:
        return print(f"missing required argument(s): {', '.join(absent)}",
                     file=sys.stderr) or 2
    if not args.pdf.is_file():
        return print(f"no such PDF: {args.pdf}", file=sys.stderr) or 2

    ir = extract(args.pdf, args.form_code, args.revision, args.expected_sha256)
    payload = json.dumps(ir, indent=2, sort_keys=False, ensure_ascii=False) + "\n"

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(payload, encoding="utf-8")
    else:
        sys.stdout.write(payload)

    if args.summary:
        print(f"{ir['form']['code']} rev {ir['form']['revision']}  "
              f"sha256 {ir['source']['sha256'][:16]}…", file=sys.stderr)
        print(f"  paper {ir['paper']['width_pt']}x{ir['paper']['height_pt']}pt  "
              f"uniform={ir['paper']['uniform']}", file=sys.stderr)
        print(f"  fonts {len(ir['fonts'])}: "
              f"{', '.join(sorted({f['family'] for f in ir['fonts'].values()}))}",
              file=sys.stderr)
        for page in ir["pages"]:
            s = page["stats"]
            print(f"  page {page['index']}: {s['rules_structural']} structural rules "
                  f"({s['rules_horizontal']}h/{s['rules_vertical']}v, "
                  f"{s['rules_decorative']} decorative), "
                  f"{s['paths_total']} paths ({s['paths_filled']} filled), "
                  f"{len(page['text_runs'])} text runs, {len(page['images'])} images",
                  file=sys.stderr)
            print(f"           thicknesses {s['structural_thickness_histogram']}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
