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
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import math
import pathlib
import sys
from typing import Any, Sequence

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
        pix = painted_pixmap(doc, xref)
        if pix.colorspace is None:
            return None
        if pix.colorspace.n != 3:
            pix = fitz.Pixmap(fitz.csRGB, pix)
        return hashlib.sha256(
            f"{pix.width}x{pix.height}:".encode() + pix.samples).hexdigest()
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


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--pdf", required=True, type=pathlib.Path)
    parser.add_argument("--form-code", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--expected-sha256", default=None,
                        help="Fail unless the PDF hashes to this. Omit only while exploring.")
    parser.add_argument("--out", type=pathlib.Path, default=None,
                        help="Write IR JSON here (default: stdout).")
    parser.add_argument("--summary", action="store_true",
                        help="Print a human-readable summary to stderr.")
    args = parser.parse_args(argv)

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
