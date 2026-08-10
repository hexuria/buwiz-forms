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

    python3 tools/formgen/extract.py --self-test              # the official pins
    python3 tools/formgen/extract.py --self-test --fixtures   # the tracked corpus
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

# The distance at which a clip edge and the edge it cuts are the same edge.
#
# It is not a tolerance on the geometry; it is the resolution of the numbers.
# MuPDF holds PDF coordinates as C floats, so a stream that writes 414.125
# hands back an edge of 414.125012 once a stroke's half-width has been added,
# and 2dp quantisation lands those two on opposite sides of a grid point -- a
# 1pt rule reported as 0.99pt.
#
# The value is measured, not chosen. Over all 53 forms, every positive overhang
# of a painted edge past its own scissor falls into two populations with an
# empty band between them: 399 of them are float32 noise, the largest exactly
# 0.0001221pt (2^-13, the ulp at 700pt, on edges both sides of which read
# 430.97), and the next 273 start at 1e-3pt, which is the finest distinction the
# BIR generator draws at all. Nothing lies between 1.3e-4 and 1e-3. This sits in
# that band: twice the worst noise, four times below the smallest real cut.
CLIP_COINCIDENCE_PT = 2.5e-4

# MuPDF reports this codepoint for a glyph it could not map to Unicode. It is
# the honest answer and the only one this module will substitute; see
# extract_text_runs.
UNMAPPED_CODEPOINT = "�"

# A run of underscores is the sheet DRAWING A WRITING LINE. Typographically it
# is a rule set in a text operator, and publishing it as text is what made an
# input on the blank overlap a printed run (review finding F200). Three is the
# floor because two underscores are a punctuation mark -- an ellipsis stand-in,
# a fill character -- while three already span more than a glyph's width and
# read as a line on paper. The number is a property of the shape, not of any
# form: nothing below keys on a form code.
RULED_BLANK_MIN_GLYPHS = 3
RULED_BLANK_CHARACTER = "_"
RULED_BLANK_CODEPOINT = ord(RULED_BLANK_CHARACTER)

# MuPDF's own name cleaner: the table PDF readers use to decide which base-14
# face stands in for an unembedded one ("Arial" -> "Helvetica",
# "TimesNewRoman" -> "Times-Roman"). It is asked rather than reimplemented, so
# the face whose outline this module measures is the face MuPDF actually drew
# with, and a name it does not recognise stays unrecognised instead of being
# guessed at. Absent binding -> no unembedded face is ever resolvable, which
# fails closed.
try:  # pragma: no cover - binding presence is environmental
    _clean_font_name = fitz.mupdf.pdf_clean_font_name
except AttributeError:  # pragma: no cover - older PyMuPDF
    _clean_font_name = None


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
                 "paint_seq", "paint_seq_max", "paint_spans")

    def __init__(self, axis: str, near: float, far: float, start: float, end: float,
                 gray: float | None, rgb: tuple[float, float, float] | None,
                 paint_seq: int, paint_seq_max: int,
                 paint_spans: Sequence[tuple[float, float, int]]) -> None:
        self.axis = axis      # "h" or "v"
        self.near = near      # y0 for h, x0 for v
        self.far = far        # y1 for h, x1 for v
        self.start = start    # x0 for h, y0 for v
        self.end = end        # x1 for h, y1 for v
        self.gray = gray
        self.rgb = rgb
        self.paint_seq = paint_seq          # first contributing op
        self.paint_seq_max = paint_seq_max  # last contributing op
        # Every offered long-axis interval survives, including exact duplicates.
        # The canonical geometry-first order is part of the extractor contract;
        # paint_seq retains the independent source paint order.
        self.paint_spans = tuple(paint_spans)

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
            "paint_spans": [
                {"start_pt": q(start), "end_pt": q(end), "paint_seq": seq}
                for start, end, seq in self.paint_spans
            ],
        }


def merge_intervals(intervals: list[tuple[float, float, int]]
                    ) -> list[tuple[
                        float, float, int, int,
                        tuple[tuple[float, float, int], ...],
                    ]]:
    """Union 1-D intervals, joining anything within JOIN_EPSILON_PT.

    Each input carries the paint sequence of the op that contributed it, and
    every merged run retains every input interval as well as the first and last
    sequence it spans. The contributor list is scoped to that run, ordered by
    ``(start, end, paint_seq)``, and deliberately not deduplicated. A bar the
    generator painted twice therefore records two full-length contributors; a
    late joint patch records only its tiny interval. That distinction is what
    lets lattice.py reconstruct exact per-slab paint order instead of assigning
    the merged hull to every contributing op.
    """
    if not intervals:
        return []

    ordered: list[tuple[float, float, int]] = []
    for start, end, seq in intervals:
        if (isinstance(start, bool) or not isinstance(start, (int, float))
                or isinstance(end, bool) or not isinstance(end, (int, float))
                or not math.isfinite(float(start))
                or not math.isfinite(float(end))):
            raise ValueError(f"invalid paint interval coordinates: {(start, end)!r}")
        if isinstance(seq, bool) or not isinstance(seq, int) or seq < 0:
            raise ValueError(f"invalid paint interval ordinal: {seq!r}")
        quantised = (q(start), q(end), seq)
        if quantised[1] <= quantised[0]:
            raise ValueError(f"invalid paint interval extent: {quantised[:2]!r}")
        ordered.append(quantised)

    ordered.sort(key=lambda span: (span[0], span[1], span[2]))
    first = ordered[0]
    merged: list[list[Any]] = [
        [first[0], first[1], first[2], first[2], [first]],
    ]
    for start, end, seq in ordered[1:]:
        if start <= merged[-1][1] + JOIN_EPSILON_PT:
            merged[-1][1] = max(merged[-1][1], end)
            merged[-1][2] = min(merged[-1][2], seq)
            merged[-1][3] = max(merged[-1][3], seq)
            merged[-1][4].append((start, end, seq))
        else:
            merged.append([start, end, seq, seq, [(start, end, seq)]])
    return [
        (q(a), q(b), lo, hi, tuple(spans))
        for a, b, lo, hi, spans in merged
    ]


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

    `clip` carries the other half of "what does this op actually ink": the
    scissor in force when it paints. Neither get_drawings() nor get_bboxlog()
    applies it -- both report the geometry the path *states* -- so an op drawn
    outside its own clip arrives here looking exactly like one that paints.

    `text` is the same walk's answer for the ops that draw glyphs, keyed by
    their position in the bbox log, which is what `get_texttrace()` reports as
    a span's `seqno`. It carries how many painting ops precede each text op and
    the box MuPDF bounds that op's ink to, so a rule reclassified out of a text
    run (see `ruled_blank_bars`) can state its place in the paint order and be
    checked against the reader's own measurement of where that text inks.
    Glyph ops deliberately do NOT consume an ordinal: doing so would renumber
    every rule in the corpus to record a fact only this one path needs.
    """

    __slots__ = ("fill", "stroke", "images", "clip", "total", "text")

    def __init__(self, fill: list[int], stroke: list[int],
                 images: list[tuple[fitz.Rect, int]],
                 clip: list[tuple[float, float, float, float] | None],
                 total: int,
                 text: dict[int, tuple[int, tuple[float, float, float, float]]]
                 | None = None) -> None:
        self.fill = fill        # per drawing: ordinal of its fill op, or -1
        self.stroke = stroke    # per drawing: ordinal of its stroke op, or -1
        self.images = images    # (placement, ordinal) in stream order
        self.clip = clip        # per drawing: active scissor, or None for unclipped
        self.total = total      # one past the last ordinal
        # bbox-log index -> (painting ops before it, the ink box it bounds)
        self.text = text if text is not None else {}


# The drawing types get_drawings() reports: fill, stroke, and both. Everything
# else `extended=True` adds -- clips and transparency groups -- is structure
# around those ops rather than an op, which is what makes the two lists
# comparable entry by entry.
PAINTING_TYPES = frozenset({"f", "s", "fs"})


def clip_scissors(page: fitz.Page, drawings: Sequence[dict[str, Any]],
                  ) -> list[tuple[float, float, float, float] | None]:
    """The scissor rectangle in force for each drawing, or None if unclipped.

    A PDF `W n` restricts every later op in its `q`/`Q` block to the intersection
    of the current clip with the path just built, and Poppler honours it. This
    module did not: it read `get_drawings()`, which states each path's own
    geometry and says nothing about the scissor, so ink drawn wholly outside its
    clip -- which the reader never sees -- was extracted as page structure. 1701
    page 1 does exactly that: `434.35 836.38 163.97 55.224 re W* n` then a fill
    at x 602.16, 3.8pt beyond the clip's right edge, repeated down the page. The
    result was a black bar painted where the official form is blank.

    `get_drawings(extended=True)` is the same walk with the clip and group
    entries left in. An item at level L sits inside the nearest preceding item at
    level L-1, so a scissor per level, dropped whenever a shallower item arrives,
    reproduces the `q`/`Q` stack. Each `clip` entry's own `scissor` is already
    cumulative, but it is intersected with its parent here anyway: relying on
    that would make this walk correct only by MuPDF's convention.

    A transparency `group` is deliberately not a scissor. Its rect is the page's
    MediaBox, and clipping to the paper edge is a separate question -- about ink
    that overhangs the sheet -- which nothing here has decided. Where a form
    means it, it says so in the stream: 2551Q draws a knockout to x=612.12 and
    then clips it with a literal `0.00000912 0 612 936 re W* n`, and that clip
    is honoured below because it is a clip.

    Raises rather than guessing if the two walks disagree, for the same reason
    paint_order does: a fallback would publish a plausible document whose ink is
    not the source's.
    """
    scissors: dict[int, tuple[float, float, float, float]] = {}
    clips: list[tuple[float, float, float, float] | None] = []

    for item in page.get_drawings(extended=True):
        level = int(item.get("level", 0))
        for deeper in [key for key in scissors if key >= level]:
            del scissors[deeper]
        parent = scissors.get(level - 1)
        kind = str(item.get("type", ""))
        if kind == "clip":
            box = fitz.Rect(item["scissor"])
            here = (box.x0, box.y0, box.x1, box.y1)
            scissors[level] = here if parent is None else (
                max(here[0], parent[0]), max(here[1], parent[1]),
                min(here[2], parent[2]), min(here[3], parent[3]))
        elif kind in PAINTING_TYPES:
            clips.append(parent)
        elif kind == "group":
            # Not a scissor (see above), but it does nest: anything inside it
            # inherits whatever clip the group itself sits under.
            scissors[level] = parent

    if len(clips) != len(drawings):
        raise SystemExit(
            f"clip stack desync: {len(clips)} painting ops in the extended walk, "
            f"{len(drawings)} drawings")
    return clips


def paint_order(page: fitz.Page, drawings: Sequence[dict[str, Any]]) -> PaintOrder:
    """Number every fill, stroke and image op on the page, and scissor each one.

    Raises rather than guessing if the log and the drawings disagree: a silent
    fallback would emit a plausible document whose z-order is not the source's,
    and z-order is exactly what this data exists to reproduce.
    """
    fill = [-1] * len(drawings)
    stroke = [-1] * len(drawings)
    images: list[tuple[fitz.Rect, int]] = []
    text: dict[int, tuple[int, tuple[float, float, float, float]]] = {}
    ordinal = 0
    index = 0

    for log_index, (kind, box) in enumerate(page.get_bboxlog()):
        if "text" in kind:
            # Recorded, not numbered: see PaintOrder. `ordinal` here is the
            # count of painting ops already numbered, so the glyphs paint after
            # every ordinal below it and before the one that will take it.
            text[log_index] = (ordinal, (float(box[0]), float(box[1]),
                                         float(box[2]), float(box[3])))
            continue
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
    # After the bbox reconciliation, so that a mutated drawings list still trips
    # the check the desync probes are aimed at.
    return PaintOrder(fill, stroke, images,
                      clip_scissors(page, drawings), ordinal, text)


def clipped(x0: float, y0: float, x1: float, y1: float,
            clip: tuple[float, float, float, float] | None,
            ) -> tuple[float, float, float, float] | None:
    """One painted rect, cut down to what the scissor lets through.

    None means the clip removes it entirely -- the op inks nothing and must not
    reach the IR at all. Emptiness is judged on the quantised extent, the same
    grid every coordinate in this module lands on, so "thinner than the IR can
    express" and "absent" are the same answer rather than two.

    An edge only moves when the scissor is inside it by more than the two
    numbers can distinguish; see CLIP_COINCIDENCE_PT. Taking the plain minimum
    instead would report a rule as 0.01pt thinner than it is drawn every time a
    box is stroked exactly to its own clip, which is 1701MS's whole page 1.
    """
    if clip is None:
        return (x0, y0, x1, y1)
    nx0 = clip[0] if clip[0] - x0 > CLIP_COINCIDENCE_PT else x0
    ny0 = clip[1] if clip[1] - y0 > CLIP_COINCIDENCE_PT else y0
    nx1 = clip[2] if x1 - clip[2] > CLIP_COINCIDENCE_PT else x1
    ny1 = clip[3] if y1 - clip[3] > CLIP_COINCIDENCE_PT else y1
    if q(nx1 - nx0) <= 0 or q(ny1 - ny0) <= 0:
        return None
    return (nx0, ny0, nx1, ny1)


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


def cap_extension_pt(item: dict[str, Any]) -> float:
    """How far this path's stroke inks past an open end of its own geometry.

    PDF 32000-1 8.4.3.3. A butt cap (0) stops the ink at the endpoint. A round
    cap (1) adds a semicircle of radius half the stroke width and a projecting
    square cap (2) adds a half-width square, so both extend the painted bar by
    exactly half a stroke width -- the same half-width the perpendicular axis
    already gets. Reporting only the declared endpoints therefore publishes an
    IR that is short of the ink on both counts for two thirds of this corpus's
    open strokes (340 of 569 carry a round or projecting cap).

    Only a path that actually strokes has caps: a fill paints the region its
    subpaths enclose and stops there.
    """
    if str(item.get("type", "")) not in ("s", "fs"):
        return 0.0
    if item.get("color") is None:
        return 0.0
    width = float(item.get("width") or 0.0)
    if width <= 0.0:
        return 0.0
    cap = item.get("lineCap")
    style = int(cap[0]) if cap else 0
    return width / 2.0 if style in (1, 2) else 0.0


def open_stroke_ends(items: Sequence[Sequence[Any]]) -> dict[int, tuple[bool, bool]]:
    """Which ops own a start cap, an end cap, or both.

    A cap exists at exactly two places on a subpath, and only if the subpath is
    open: `re` and `qu` are closed by definition, a polyline that returns to its
    own start is closed by measurement, and every interior vertex of an open
    polyline is a *join*, not a cap. Capping per op instead would grow a
    rectangle drawn as four `l` ops by half a stroke on all four sides -- 133 of
    them in this corpus, 12 with a round cap.

    Subpaths are reconstructed exactly as subpaths_of does, on the same
    quantised coincidence test, so the two views of one path can never disagree
    about where it starts and ends.
    """
    groups: list[list[int]] = []
    current: list[int] | None = None
    cursor: fitz.Point | None = None

    for index, op in enumerate(items):
        kind = op[0]
        if kind in ("re", "qu"):
            current, cursor = None, None
            continue
        if kind not in ("l", "c"):
            raise SystemExit(f"unknown path op {kind!r}")
        points = list(op[1:])
        first, last = points[0], points[-1]
        if (current is None or cursor is None
                or abs(first.x - cursor.x) > AXIS_EPSILON_PT
                or abs(first.y - cursor.y) > AXIS_EPSILON_PT):
            current = []
            groups.append(current)
        current.append(index)
        cursor = last

    ends: dict[int, tuple[bool, bool]] = {}
    for group in groups:
        start = items[group[0]][1]
        finish = items[group[-1]][-1]
        if (q(start.x), q(start.y)) == (q(finish.x), q(finish.y)):
            continue
        head = ends.get(group[0], (False, False))
        ends[group[0]] = (True, head[1])
        tail = ends.get(group[-1], (False, False))
        ends[group[-1]] = (tail[0], True)
    return ends


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


def extract_segments(drawings: Sequence[dict[str, Any]], order: PaintOrder,
                     ruled_blanks: Sequence[dict[str, Any]] = ()) -> list[Segment]:
    """Turn every filled rect into maximal horizontal and vertical bars.

    The BIR generator draws a long border as a run of short filled rects plus a
    square at each joint. A square is thin on *both* axes, so it is offered to
    both groupings; the interval union absorbs the double count and the joint
    disappears into the segments it was patching.

    `ruled_blanks` are the bars a run of underscore glyphs draws (see
    `ruled_blank_bars`). They are offered into the SAME grouping rather than
    appended afterwards, because a bar that abuts a drawn rule of the same tone
    on the same band is one stroke on paper: emitting it as two would print two
    abutting rects, and re-extracting our own print would union them back into
    one and report the pair as a missing rule against an extra one.
    """
    h_groups: dict[tuple[float, float, float | None, Any], list[tuple[float, float, int]]] = (
        collections.defaultdict(list))
    v_groups: dict[tuple[float, float, float | None, Any], list[tuple[float, float, int]]] = (
        collections.defaultdict(list))

    def offer(x0: float, y0: float, x1: float, y1: float,
              gray: float | None, rgb: Any, seq: int,
              scissor: tuple[float, float, float, float] | None) -> None:
        """Route one axis-aligned bar, as its scissor lets it through, into the
        horizontal and/or vertical group. Raw coordinates in; the quantisation
        happens after the clip, because the clip is where the bar really ends."""
        box = clipped(x0, y0, x1, y1, scissor)
        if box is None:
            return
        x0, y0, x1, y1 = q(box[0]), q(box[1]), q(box[2]), q(box[3])
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
        # What the scissor lets through, not what the path states.
        scissor = order.clip[index]

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
                    offer(r.x0 - half, y - half, r.x1 + half, y + half,
                          s_gray, s_rgb, stroke_seq, scissor)
                for x in (r.x0, r.x1):
                    offer(x - half, r.y0 - half, x + half, r.y1 + half,
                          s_gray, s_rgb, stroke_seq, scissor)

        fill = item.get("fill")
        if fill is None:
            fill = stroke
            if fill is None:
                continue
        gray = to_gray(fill)
        rgb = tuple(round(float(c), 4) for c in fill[:3]) if len(fill) >= 3 else None

        # Caps live on the open ends of the path's own subpaths, so both are
        # settled once per path rather than guessed per op.
        cap_extension = cap_extension_pt(item)
        cap_ends = open_stroke_ends(item["items"]) if cap_extension > 0.0 else {}

        for op_index, op in enumerate(item["items"]):
            if op[0] == "re":
                rect = op[1]
            elif op[0] == "l":
                # A zero-area line: give it the path's stroke width as thickness.
                p0, p1 = op[1], op[2]
                width = float(item.get("width") or 0.0) or 0.24
                # A capped end paints half a stroke width past the coordinate
                # the source states; see cap_extension_pt. `lead` belongs to p0
                # and `trail` to p1, and which of those is the low coordinate is
                # the segment's direction, not its geometry.
                head, tail = cap_ends.get(op_index, (False, False))
                lead = cap_extension if head else 0.0
                trail = cap_extension if tail else 0.0
                if abs(p0.y - p1.y) <= abs(p0.x - p1.x):
                    forward = p0.x <= p1.x
                    low = min(p0.x, p1.x) - (lead if forward else trail)
                    high = max(p0.x, p1.x) + (trail if forward else lead)
                    rect = fitz.Rect(low, p0.y - width / 2,
                                     high, p0.y + width / 2)
                else:
                    forward = p0.y <= p1.y
                    low = min(p0.y, p1.y) - (lead if forward else trail)
                    high = max(p0.y, p1.y) + (trail if forward else lead)
                    rect = fitz.Rect(p0.x - width / 2, low,
                                     p0.x + width / 2, high)
            else:
                continue

            offer(rect.x0, rect.y0, rect.x1, rect.y1, gray, rgb, fill_seq, scissor)

    for bar in ruled_blanks:
        # No scissor: `extract_text_runs` does not model the clip on a text op
        # either, so the glyphs these bars replace were already published
        # unclipped. Inventing one here would be a new claim, not a kept one.
        offer(bar["x0"], bar["y0"], bar["x1"], bar["y1"],
              bar["gray"], bar["rgb"], bar["paint_seq"], None)

    segments: list[Segment] = []
    for (near, far, gray, rgb), spans in h_groups.items():
        for start, end, lo, hi, paint_spans in merge_intervals(spans):
            segments.append(Segment(
                "h", near, far, start, end, gray, rgb, lo, hi, paint_spans))
    for (near, far, gray, rgb), spans in v_groups.items():
        for start, end, lo, hi, paint_spans in merge_intervals(spans):
            segments.append(Segment(
                "v", near, far, start, end, gray, rgb, lo, hi, paint_spans))

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
        scissor = order.clip[index]
        for op in item["items"]:
            if op[0] != "re":
                continue
            rect = op[1]
            box = clipped(rect.x0, rect.y0, rect.x1, rect.y1, scissor)
            if box is None:
                continue
            width, height = q(box[2] - box[0]), q(box[3] - box[1])
            if width <= MAX_RULE_THICKNESS_PT or height <= MAX_RULE_THICKNESS_PT:
                continue
            fills.append({
                "x0": q(box[0]), "y0": q(box[1]),
                "x1": q(box[2]), "y1": q(box[3]),
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
        # A curve cannot be truncated the way a bar can -- cutting a Bezier at a
        # scissor edge is path arithmetic, not a coordinate swap -- so only the
        # unambiguous answer is given here. Wholly outside its clip, the path
        # inks nothing and is dropped; wholly inside, it is untouched. A path the
        # scissor genuinely cuts stops extraction rather than shipping a shape
        # that is not the source's: no form in this corpus has one, and the day
        # one appears is the day to decide what to draw, not to guess.
        scissor = order.clip[index]
        if scissor is not None:
            visible = clipped(rect.x0, rect.y0, rect.x1, rect.y1, scissor)
            if visible is None:
                continue
            if (q(visible[0]), q(visible[1]), q(visible[2]), q(visible[3])) != (
                    q(rect.x0), q(rect.y0), q(rect.x1), q(rect.y1)):
                raise SystemExit(
                    "a non-rectilinear path is cut by its clip and this module "
                    "cannot truncate curves: path "
                    f"{(q(rect.x0), q(rect.y0), q(rect.x1), q(rect.y1))} "
                    f"under scissor {tuple(q(v) for v in scissor)}")
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


# ---------------------------------------------------------------------------
# Ruled blanks
# ---------------------------------------------------------------------------


def glyph_provenance(page: fitz.Page) -> dict[tuple[float, float],
                                              tuple[int, int]]:
    """Per glyph origin, the glyph id drawn there and the op that drew it.

    Read from `get_texttrace()`, which is the operator stream rather than
    rawdict's reconstruction of it, so the glyph id is the id of the outline
    MuPDF actually rendered -- in an unembedded face, the substitute's. That id
    is the key that lets a face be *identified* instead of assumed: a candidate
    face is accepted only when the id it assigns to the character matches the
    one drawn here.

    `seqno` is the span's index in the same bbox log `paint_order` walks, which
    is how a glyph reaches its place in the page's paint order and the box
    MuPDF bounds its ink to.

    Origins that carry more than one glyph are dropped rather than resolved by
    a tiebreak, exactly as `unmapped_glyph_origins` drops them: the point of
    this map is to be certain about the glyph it names.
    """
    seen: dict[tuple[float, float], set[tuple[int, int]]] = (
        collections.defaultdict(set))
    for span in page.get_texttrace():
        seqno = span.get("seqno")
        if not isinstance(seqno, int):
            continue
        for char in span["chars"]:
            origin = (q(char[2][0]), q(char[2][1]))
            seen[origin].add((int(char[1]), seqno))
    return {origin: next(iter(entries))
            for origin, entries in seen.items() if len(entries) == 1}


def substitutable_faces(page: fitz.Page,
                        doc: fitz.Document) -> dict[str, tuple[fitz.Font, ...]]:
    """Every face the page could be drawing each BaseFont with.

    Two kinds, and the difference is the whole point. A font whose program the
    file embeds is measured from that program. A font it does not embed is
    drawn by MuPDF from a substitute, and the substitute is resolved by asking
    MuPDF's own name cleaner which base-14 face stands in -- never by a table
    written here, and never by a font installed on this machine, which is the
    machine-dependence extract_text_runs refuses on the metrics side for the
    same reason.

    A name MuPDF's cleaner does not recognise (this corpus has one: Arial
    Narrow) yields no face at all. That is the intended answer, not a gap:
    caller-side, no face means no measurement means the glyphs stay text.

    Several resources can share one BaseFont -- 1701 page 4 carries both an
    unembedded `Arial,Italic` and an embedded Type0 subset of the same name --
    so the value is every candidate, and the caller resolves between them by
    the glyph id the page actually drew.
    """
    faces: dict[str, list[fitz.Font]] = collections.defaultdict(list)
    for xref, ext, _ftype, basefont, _res, _enc, _ in page.get_fonts(full=True):
        font: fitz.Font | None = None
        if ext not in ("n/a", "", None):
            try:
                buffer = doc.extract_font(xref)[3]
                font = fitz.Font(fontbuffer=buffer) if buffer else None
            except Exception:  # noqa: BLE001 - an unreadable program is "no face"
                font = None
        elif _clean_font_name is not None:
            try:
                font = fitz.Font(_clean_font_name(basefont))
            except Exception:  # noqa: BLE001 - not a base-14 name is "no face"
                font = None
        if font is not None:
            faces[basefont].append(font)
    return {name: tuple(fonts) for name, fonts in faces.items()}


def glyph_ink_box(faces: Sequence[fitz.Font], codepoint: int,
                  glyph_id: int) -> tuple[float, float, float, float] | None:
    """The em-relative ink box of one glyph, or None when it is not derivable.

    Derivable means: exactly one of the candidate faces assigns `glyph_id` to
    `codepoint`, and that face states a finite, non-degenerate outline box for
    it. Two faces that agree on the box are one answer; two that disagree are
    an ambiguity and get None, because a rule drawn at the wrong band is worse
    than a glyph left as text.

    The box is in font units with y counting UP from the baseline, which is why
    the caller flips it into page coordinates rather than adding it.
    """
    boxes: set[tuple[float, float, float, float]] = set()
    for font in faces:
        try:
            if font.has_glyph(codepoint) != glyph_id:
                continue
            box = font.glyph_bbox(codepoint)
        except Exception:  # noqa: BLE001 - a face that cannot answer is not one
            continue
        if box is None:
            continue
        values = (float(box.x0), float(box.y0), float(box.x1), float(box.y1))
        if not all(math.isfinite(value) for value in values):
            continue
        if values[2] <= values[0] or values[3] <= values[1]:
            continue
        boxes.add(values)
    if len(boxes) != 1:
        return None
    return next(iter(boxes))


def baseline_groups(chars: Sequence[dict[str, Any]]) -> list[tuple[int, int]]:
    """A span's glyphs cut into maximal stretches that share one baseline.

    A run in this IR states ONE `baseline_y` and one `origin_x`, and every
    per-character offset is measured from them. MuPDF's `rawdict` span is not
    bound by that contract: its line builder groups glyphs that are merely close
    enough, so a span can carry glyphs set on two different baselines. Where it
    does, `_text_run` published the FIRST glyph's baseline for all of them and a
    box that unions both lines -- and emit.py places the whole run on that
    baseline. 1707-A page 1 is the visible case: a positioning space at baseline
    113.78 sits in the same span as `Calendar` at 117.50, so the word was set
    3.72pt above the `1 For` / `Fiscal` it is printed on (finding F070), and
    2200-P page 2 does the same to ` Total Tax-` by 4.80pt (F102).

    Splitting is the faithful answer rather than a repair: the source drew each
    stretch with its own text operator at its own `Td`, so one stretch per
    baseline is what the file says. Comparison is on the quantised baseline --
    two glyphs the IR cannot tell apart are one baseline here too.

    Half-open [start, end) indices into `chars`, in the span's own order. A span
    with one baseline yields exactly one group covering all of it, which is what
    keeps this a no-op for 19,309 of the corpus's 19,333 ink-bearing spans.
    """
    groups: list[tuple[int, int]] = []
    start = 0
    for index in range(1, len(chars) + 1):
        if (index < len(chars)
                and q(chars[index]["origin"][1]) == q(chars[start]["origin"][1])):
            continue
        groups.append((start, index))
        start = index
    return groups


def ruled_blank_groups(text: str) -> list[tuple[int, int]]:
    """Every maximal run of at least RULED_BLANK_MIN_GLYPHS underscores.

    A group is bounded by any other character, so `'XA ____ % X ='` carries
    one and `'_ _ _'` carries none. Half-open [start, end) indices into `text`,
    which is the run's text AFTER unmappable glyphs have been substituted --
    a glyph the file gives no meaning is not an underscore, and must break a
    group rather than be absorbed into one.
    """
    groups: list[tuple[int, int]] = []
    start: int | None = None
    for index, character in enumerate(text):
        if character == RULED_BLANK_CHARACTER:
            if start is None:
                start = index
            continue
        if start is not None and index - start >= RULED_BLANK_MIN_GLYPHS:
            groups.append((start, index))
        start = None
    if start is not None and len(text) - start >= RULED_BLANK_MIN_GLYPHS:
        groups.append((start, len(text)))
    return groups


def ruled_blank_bars(chars: Sequence[dict[str, Any]], size: float,
                     faces: Sequence[fitz.Font],
                     provenance: dict[tuple[float, float], tuple[int, int]],
                     text_ops: dict[int, tuple[int,
                                               tuple[float, float, float, float]]],
                     colour: Any) -> tuple[list[dict[str, Any]], str]:
    """One underscore group as the bar it draws, or a refusal and its reason.

    Every step can only fail closed. The band is the glyphs' OWN ink -- the
    outline box of the face that drew them, scaled by the size the file states
    and hung off the baseline the file states -- so nothing here is a
    typographic convention or an underline position invented for the purpose.

    One group can be drawn by several text operators -- 1701 page 4 sets its
    33-underscore blank as five of them -- so the group is cut into pieces at
    the points where its PAINT ORDINAL changes, and each piece is offered with
    its own ordinal for `merge_intervals` to union. Not at every operator
    boundary: consecutive text ops with no painting op between them land on the
    same ordinal and are one paint event in this model, and cutting there would
    publish two rules for one blank wherever the face's underscore is narrower
    than its advance (Times-Roman leaves 0.003em, which is 0.04pt at 12pt --
    four times the join epsilon, which exists for float error and not for real
    gaps).

    The last step is what makes the rest checkable: each piece must lie inside
    the box MuPDF independently bounds its own text ops' ink to
    (`PaintOrder.text`, from the same bbox log the paint ordinals come from).
    That box is the reader's own measurement, taken without any of the font
    arithmetic above, and it is expanded by one unit for antialiasing -- so a
    band derived off the wrong baseline, at the wrong scale, or in the wrong
    coordinate space cannot be contained by it and is refused.
    """
    if not chars:
        return [], "no glyphs"
    marks = [provenance.get((q(char["origin"][0]), q(char["origin"][1])))
             for char in chars]
    if any(mark is None for mark in marks):
        return [], "glyph id and paint op not stated once per glyph"
    if len({mark[0] for mark in marks}) != 1:
        return [], "one group drawn with more than one glyph"
    glyph_id = marks[0][0]
    if glyph_id <= 0:
        return [], "glyph is the face's .notdef"
    if len({q(char["origin"][1]) for char in chars}) != 1:
        return [], "glyphs do not share one baseline"
    if any(mark[1] not in text_ops for mark in marks):
        return [], "text op has no place in the paint order"

    box = glyph_ink_box(faces, RULED_BLANK_CODEPOINT, glyph_id)
    if box is None:
        return [], "no single face states this glyph's outline"

    channels = colour if isinstance(colour, (list, tuple)) else (
        (((int(colour) >> 16) & 0xFF) / 255.0,
         ((int(colour) >> 8) & 0xFF) / 255.0,
         (int(colour) & 0xFF) / 255.0) if isinstance(colour, int) else None)
    if channels is None:
        return [], "run states no fill colour"
    rgb = tuple(round(float(channel), 4) for channel in channels[:3])

    baseline = float(chars[0]["origin"][1])
    # Font space counts y up from the baseline; the page counts it down.
    y0 = baseline - size * box[3]
    y1 = baseline - size * box[1]
    span_x0 = float(chars[0]["origin"][0]) + size * box[0]
    span_x1 = float(chars[-1]["origin"][0]) + size * box[2]
    if not all(math.isfinite(value) for value in (span_x0, y0, span_x1, y1)):
        return [], "band is not a finite rectangle"
    if q(y1) - q(y0) <= 0 or q(span_x1) - q(span_x0) <= 0:
        return [], "band is degenerate once quantised"
    if q(y1) - q(y0) > MAX_RULE_THICKNESS_PT:
        return [], "band is thicker than a rule"
    if q(span_x1) - q(span_x0) <= MAX_RULE_THICKNESS_PT:
        return [], "bar is shorter than a rule's own thickness"

    ordinals = [text_ops[mark[1]][0] for mark in marks]
    bars: list[dict[str, Any]] = []
    start = 0
    for position in range(len(chars) + 1):
        if position < len(chars) and ordinals[position] == ordinals[start]:
            continue
        piece = chars[start:position]
        ink = [text_ops[mark[1]][1] for mark in marks[start:position]]
        bound = (min(b[0] for b in ink), min(b[1] for b in ink),
                 max(b[2] for b in ink), max(b[3] for b in ink))
        x0 = float(piece[0]["origin"][0]) + size * box[0]
        x1 = float(piece[-1]["origin"][0]) + size * box[2]
        if not (bound[0] - AXIS_EPSILON_PT <= x0
                and bound[1] - AXIS_EPSILON_PT <= y0
                and x1 <= bound[2] + AXIS_EPSILON_PT
                and y1 <= bound[3] + AXIS_EPSILON_PT):
            return [], "band is not inside the reader's own bound for this ink"
        bars.append({
            "x0": x0, "y0": y0, "x1": x1, "y1": y1,
            "gray": to_gray(rgb),
            "rgb": rgb,
            "paint_seq": ordinals[start],
        })
        start = position
    return bars, ""


def _text_run(span: dict[str, Any], chars: Sequence[dict[str, Any]],
              letters: Sequence[str], unmapped_glyphs: list[dict[str, Any]],
              direction: Sequence[float],
              box: tuple[float, float, float, float]) -> dict[str, Any]:
    """One run, from the glyphs it keeps and the extent it was measured at.

    Split out of extract_text_runs so a run and a FRAGMENT of a run are built
    by the same arithmetic. `box` is the run's own rectangle: the span's for a
    whole span, and the kept glyphs' for a fragment, because a fragment that
    reported the span's extent would claim ink where a ruled blank now is.
    """
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
    return {
        "text": "".join(letters),
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
        "x0": q(box[0]), "y0": q(box[1]),
        "x1": q(box[2]), "y1": q(box[3]),
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
    }


def extract_text_runs(page: fitz.Page, doc: fitz.Document, order: PaintOrder,
                      ) -> tuple[list[dict[str, Any]],
                                 list[dict[str, Any]],
                                 dict[str, Any]]:
    """Every visible text run, and the bars its ruled blanks actually draw.

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

    A rawdict span is cut at every baseline change before any of that happens
    (`baseline_groups`). One run states one baseline, and MuPDF's line builder
    does not guarantee one: where a span carries two, the first glyph's baseline
    used to be published for all of them and emit.py set the whole run there.

    A run of underscores is the one shape here that is not text at all. The
    sheet is drawing a writing line with a text operator, and while it stays a
    run the strip it draws is inside a printed run's box, so an input placed on
    the blank necessarily overlaps printed ink and is refused (F200). Each
    group of at least RULED_BLANK_MIN_GLYPHS underscores therefore leaves the
    run and is returned as a BAR at those glyphs' own ink band, measured by
    `ruled_blank_bars`; the run splits into the fragments either side of it, and
    a fragment left holding only whitespace is dropped exactly as a
    whitespace-only span already was. Every group the measurement refuses stays
    text and is counted by reason in the third return value: a band that cannot
    be derived is never approximated.
    """
    runs: list[dict[str, Any]] = []
    bars: list[dict[str, Any]] = []
    refusals: collections.Counter[str] = collections.Counter()
    groups_seen = 0
    groups_published = 0
    raw = page.get_text("rawdict")
    unmapped = unmapped_glyph_origins(page)
    provenance: dict[tuple[float, float], tuple[int, int]] | None = None
    faces: dict[str, tuple[fitz.Font, ...]] | None = None

    for block in raw["blocks"]:
        if block["type"] != 0:
            continue
        for line in block["lines"]:
            direction = line.get("dir", (1.0, 0.0))
            for span in line["spans"]:
                span_chars = span.get("chars") or []
                span_unmapped = []
                span_letters = []
                for position, char in enumerate(span_chars):
                    glyph_id = unmapped.get((q(char["origin"][0]), q(char["origin"][1])))
                    if glyph_id is None:
                        span_letters.append(char["c"])
                        continue
                    span_letters.append(UNMAPPED_CODEPOINT)
                    span_unmapped.append({
                        "index": position,
                        "glyph_id": glyph_id,
                        "rawdict_codepoint": ord(char["c"]),
                    })

                # One stretch per baseline: see baseline_groups. For a span the
                # file set on a single baseline this is the span itself, and the
                # box below is its own bbox to the last quantised place.
                for base_start, base_end in baseline_groups(span_chars):
                    chars = span_chars[base_start:base_end]
                    letters = span_letters[base_start:base_end]
                    unmapped_glyphs = [
                        {**entry, "index": entry["index"] - base_start}
                        for entry in span_unmapped
                        if base_start <= entry["index"] < base_end]
                    text = "".join(letters)
                    if not text.strip():
                        continue
                    box = (min(char["bbox"][0] for char in chars),
                           min(char["bbox"][1] for char in chars),
                           max(char["bbox"][2] for char in chars),
                           max(char["bbox"][3] for char in chars))

                    groups = ruled_blank_groups(text)
                    groups_seen += len(groups)
                    # Both maps cost a walk of the page's operator stream, so
                    # they are built only where a candidate group exists.
                    if groups and provenance is None:
                        provenance = glyph_provenance(page)
                        faces = substitutable_faces(page, doc)
                    size = round(float(span["size"]), 3)
                    published: list[tuple[int, int]] = []
                    for start, end in groups:
                        if abs(float(direction[1])) > 1e-6:
                            refusals["run is rotated"] += 1
                            continue
                        pieces, reason = ruled_blank_bars(
                            chars[start:end], size,
                            (faces or {}).get(span["font"], ()),
                            provenance or {}, order.text, span.get("color"))
                        if not pieces:
                            refusals[reason] += 1
                            continue
                        bars.extend(pieces)
                        groups_published += 1
                        published.append((start, end))

                    if not published:
                        runs.append(_text_run(span, chars, letters,
                                              unmapped_glyphs, direction, box))
                        continue

                    # What the bars left behind, in reading order.
                    fragments: list[tuple[int, int]] = []
                    cursor = 0
                    for start, end in published:
                        if start > cursor:
                            fragments.append((cursor, start))
                        cursor = end
                    if cursor < len(chars):
                        fragments.append((cursor, len(chars)))
                    for start, end in fragments:
                        kept = chars[start:end]
                        piece = letters[start:end]
                        if not "".join(piece).strip():
                            continue
                        runs.append(_text_run(
                            span, kept, piece,
                            [{**entry, "index": entry["index"] - start}
                             for entry in unmapped_glyphs
                             if start <= entry["index"] < end],
                            direction,
                            (kept[0]["bbox"][0], box[1],
                             kept[-1]["bbox"][2], box[3])))

    runs.sort(key=lambda r: (r["y0"], r["x0"]))
    return runs, bars, {
        "ruled_blank_groups": groups_seen,
        "ruled_blank_published": groups_published,
        "ruled_blank_bars": len(bars),
        "ruled_blank_text_retained": sum(refusals.values()),
        "ruled_blank_refusals": dict(sorted(refusals.items())),
    }


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

    This is the *measurement* digest: what a reader of THIS PDF sees. It is the
    right formula for a candidate -- verify.py and audit.py extract our own
    Chromium-printed PDF with it -- and the wrong one for a reference IR whose
    masked images ship as re-encoded PNG assets; those pin
    shipped_pixel_sha256() instead, and the distinction is load-bearing (see
    that function for the measured proof).
    """
    try:
        return pixmap_sha256(painted_pixmap(doc, xref))
    except Exception:  # noqa: BLE001 - undecodable is a real answer, not a failure
        return None


def masked_asset_png(doc: fitz.Document, xref: int) -> bytes:
    """The exact PNG bytes an offline bundle ships for a soft-masked XObject.

    The single encode site, shared by asset_for_xref() (which writes these bytes
    to disk) and shipped_pixel_sha256() (which pins their decoded samples). Two
    call sites each doing their own tobytes("png") would invite the exact bug
    this factoring closes: a digest computed over bytes nobody ships.
    """
    return painted_pixmap(doc, xref).tobytes("png")


def shipped_pixel_sha256(doc: fitz.Document, xref: int) -> str | None:
    """Hash the samples a consumer of the shipped bytes will decode.

    For an unmasked image the bundle ships the source's own compressed stream,
    so this is decoded_pixel_sha256 exactly. For a soft-masked image the bundle
    ships the composited pixmap re-encoded as PNG (see asset_for_xref), and
    MuPDF's PNG encode/decode round trip is not bit-faithful for that pixmap:
    on 1701MS's seal, 3 of 7,900 pixels -- all partial-alpha edge pixels, alpha
    236-245 -- come back with the red channel lower by exactly one step, and
    the drift repeats on every further round trip (measured: five successive
    re-encodes produced five distinct digests), so there is no fixed point to
    pin instead. Hashing the in-memory pixmap therefore pinned a picture nobody
    ships: Chromium re-embeds the asset bit-exactly, the candidate extraction
    decoded exactly the asset's samples, and verify.diff_images -- which pairs
    images by digest identity -- reported the seal missing while every pixel on
    paper was right.

    So: encode once, via the same masked_asset_png() the asset writer uses,
    decode those exact bytes back, and hash the decoded samples with the one
    digest formula. Still an exact SHA-256 over white-composited samples --
    nothing is tolerated, the subject is simply the shipped artifact instead of
    an intermediate buffer. Returns None when the XObject cannot be decoded,
    which the caller must treat as "unknown", never "equal".
    """
    try:
        if not smask_xref(doc, xref):
            return decoded_pixel_sha256(doc, xref)
        return pixmap_sha256(fitz.Pixmap(masked_asset_png(doc, xref)))
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
    return name, masked_asset_png(doc, xref)


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


def extract_images(page: fitz.Page, doc: fitz.Document, order: PaintOrder,
                   shipped_pixels: bool = False) -> list[dict[str, Any]]:
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

    `shipped_pixels` selects which decoded samples `pixel_sha256` is over: the
    ones embedded in this PDF (a candidate being measured), or the ones in the
    asset a bundle ships for it (a reference IR). The two differ only for
    soft-masked images, whose asset is a re-encode; see shipped_pixel_sha256
    for why one digest cannot serve both sides.

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
        pixel_digest = (shipped_pixel_sha256 if shipped_pixels
                        else decoded_pixel_sha256)(doc, xref)
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


def extract_page(page: fitz.Page, doc: fitz.Document, index: int,
                 shipped_pixels: bool = False) -> dict[str, Any]:
    drawings = list(page.get_drawings())
    order = paint_order(page, drawings)
    # Text first: a ruled blank leaves the runs as a bar, and the bar is a rule
    # of the page like any other, so it has to be in hand before the segments
    # are unioned.
    text_runs, ruled_blanks, blank_stats = extract_text_runs(page, doc, order)
    segments = extract_segments(drawings, order, ruled_blanks)
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
        "text_runs": text_runs,
        "images": extract_images(page, doc, order, shipped_pixels),
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
            **blank_stats,
        },
    }


def extract(pdf_path: pathlib.Path, form_code: str, revision: str,
            expected_sha256: str | None, *,
            shipped_pixels: bool = False) -> dict[str, Any]:
    """The IR of one PDF. `shipped_pixels` picks the masked-image digest side.

    False (the default) measures the PDF as embedded -- what every in-process
    caller wants: audit.py and fill_check.py extract Chromium-printed
    candidates, verify.py's self-tests extract synthetic PDFs, and this
    module's own self-test reads source fixtures directly. True pins the digest
    of the asset a bundle ships instead, which only the reference IR wants; the
    CLI -- batch.py's route to build/ir/ and the staged guides -- is the only
    producer of shipping IRs, so it is the only caller that passes it.
    """
    digest = sha256_file(pdf_path)
    if expected_sha256 and digest != expected_sha256.lower():
        raise SystemExit(
            f"PDF hash mismatch\n  expected {expected_sha256.lower()}\n  actual   {digest}")

    doc = fitz.open(pdf_path)
    pages = [extract_page(page, doc, i + 1, shipped_pixels)
             for i, page in enumerate(doc)]

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
# These assertions are measured against the real corpus, and that is what makes
# them evidence: every property below was established by reading these exact
# files, whereas a synthetic fixture can only encode what this module already
# believes. Each is pinned by hash, because a self-test that silently scored a
# different revision of a form would be worse than none.
#
# The files are official BIR documents and are deliberately untracked, so this
# table cannot run anywhere but a machine that holds them. FIXTURE_PROFILE below
# is the second, weaker corpus that can -- it does not replace this one, and
# `--self-test` with no flag still means this one.
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

# The three literal tones the BIR generator paints with, and the band each one
# has to land in. Black ink is structure; 0.8509 is the decorative grey that
# measures tone ~217 on paper and is near-invisible, and CLAUDE.md records
# painting it black as a shipped failure; 1.0 is a knockout, white paper
# deliberately punched through a tint band. The corpus must contain ink of all
# three, so this pins the classifier's answer about real ink rather than
# restating a lookup table.
SELF_TEST_TONES: tuple[tuple[float, str], ...] = (
    (0.0, "structural"),
    (0.8509, "decorative"),
    (1.0, "knockout"),
)


class SelfTestProfile:
    """One corpus, and every number pinned against it.

    The checks below used to read these as module constants, which tied them to
    the six official PDFs. Those files are untracked -- they are official
    documents, and gitignored on purpose -- so on any machine without them the
    entire self-test could only report "cannot run". In CI that is the failure
    this project has already shipped once: a green tick that evaluated nothing.

    Grouping the pins into a profile lets the *same* checks, and the same
    mutation probes, run over a second corpus that is small enough to
    track. Only the subjects move; no check, threshold or assertion is relaxed
    for the synthetic run, and none of them is skipped.
    """

    __slots__ = ("name", "source_root", "fixtures", "paper", "determinism_form",
                 "masked", "flipped", "paths_form", "triangles",
                 "decimal_points", "tones", "retexted_glyphs",
                 "retexted_glyph_id", "retexted_rawdict_codepoint",
                 "bar_like_form", "leaning_bars", "is_evidence")

    def __init__(self, *, name: str, source_root: pathlib.Path,
                 fixtures: dict[str, tuple[str, str, str]],
                 paper: tuple[str, float, float, int], determinism_form: str,
                 masked: tuple[str, int, int], flipped: tuple[str, int],
                 paths_form: str, triangles: int, decimal_points: int,
                 tones: tuple[tuple[float, str], ...],
                 retexted_glyphs: dict[str, int], retexted_glyph_id: int,
                 retexted_rawdict_codepoint: int, bar_like_form: str,
                 leaning_bars: int, is_evidence: bool) -> None:
        self.name = name
        self.source_root = source_root
        self.fixtures = fixtures
        self.paper = paper
        self.determinism_form = determinism_form
        self.masked = masked
        self.flipped = flipped
        self.paths_form = paths_form
        self.triangles = triangles
        self.decimal_points = decimal_points
        self.tones = tones
        self.retexted_glyphs = retexted_glyphs
        self.retexted_glyph_id = retexted_glyph_id
        self.retexted_rawdict_codepoint = retexted_rawdict_codepoint
        self.bar_like_form = bar_like_form
        self.leaning_bars = leaning_bars
        # Whether this corpus is evidence about the world or a restatement of
        # this module's own beliefs. Printed with the result so a passing
        # synthetic run can never be read as the real one.
        self.is_evidence = is_evidence


REAL_PROFILE = SelfTestProfile(
    name="pinned BIR PDFs",
    source_root=SELF_TEST_SOURCE_ROOT,
    fixtures=SELF_TEST_FIXTURES,
    paper=SELF_TEST_PAPER,
    determinism_form=SELF_TEST_DETERMINISM_FORM,
    masked=SELF_TEST_MASKED,
    flipped=SELF_TEST_FLIPPED,
    paths_form=SELF_TEST_PATHS_FORM,
    triangles=SELF_TEST_TRIANGLES,
    decimal_points=SELF_TEST_DECIMAL_POINTS,
    tones=SELF_TEST_TONES,
    retexted_glyphs=SELF_TEST_RETEXTED_GLYPHS,
    retexted_glyph_id=SELF_TEST_RETEXTED_GLYPH_ID,
    retexted_rawdict_codepoint=SELF_TEST_RETEXTED_RAWDICT_CODEPOINT,
    bar_like_form=SELF_TEST_BAR_LIKE_FORM,
    leaning_bars=SELF_TEST_LEANING_BARS,
    is_evidence=True,
)

# ---------------------------------------------------------------------------
# The synthetic corpus
# ---------------------------------------------------------------------------

# Built by tools/formgen/fixtures/make_fixtures.py, tracked, and pinned by the
# same sha256 mechanism as the real corpus -- a rebuild that moved one byte
# fails extraction here rather than quietly scoring different files. Each
# builder's docstring names the real form it stands in for and the defect that
# form taught us about.
#
# This corpus is weaker than the real one, on purpose and unavoidably: it can
# only reproduce structures we already know how to describe. It exists so that
# every check, and every mutation probe, actually *runs* somewhere other than
# one laptop. It is not a substitute for the pinned officials, and the summary
# line says so on every run.
FIXTURE_SOURCE_ROOT = pathlib.Path(__file__).resolve().parent / "fixtures"

FIXTURE_FIXTURES: dict[str, tuple[str, str, str]] = {
    # Paper, determinism, merged runs drawn as short bars plus corner squares,
    # all four thicknesses, both decorative greys, a white knockout, a
    # fill+stroke drawing and a comb band.
    "FIXTURE-RULES": (
        "rules.pdf", "0001",
        "42b7bf64491e53d5afec01e1876530268b1e4611c950acd76ebfdeae9fef29f5"),
    # Non-rectilinear ink: filled triangles and filled Bezier marks.
    "FIXTURE-PATHS": (
        "paths.pdf", "0001",
        "b771d06e7919c10e769a0b6cc3c51304ab5764fa715386082db0392a0533b4bf"),
    # One partly transparent soft mask and one that is fully opaque.
    "FIXTURE-MASKS": (
        "masks.pdf", "0001",
        "4fe33a46370e9b5c483e0f4ec9188c095c15c95ba9f61079ec675edc17a80795"),
    # A placement matrix with a negative `d`: the vertical flip.
    "FIXTURE-FLIP": (
        "flip.pdf", "0001",
        "cf0dd9aae1686a5bef15b12a429dd4c8ed8b0f0aa053f6b19a4833650345cdf0"),
    # A glyph with no Unicode meaning, beside a question mark that has one.
    "FIXTURE-GLYPHS": (
        "glyphs.pdf", "0001",
        "fff3c4e2b8dea5ccb7d4eca709b6c287706c19a53d4a5ce9586fa523fbb88b1d"),
    # Twelve stroked separators leaning less than their own stroke width.
    "FIXTURE-LEAN": (
        "lean.pdf", "0001",
        "ef4165fb57e736f187910103be99ade15aa1a7ca3547470980fa6f43e17811ac"),
}

FIXTURE_PROFILE = SelfTestProfile(
    name="synthetic fixtures",
    source_root=FIXTURE_SOURCE_ROOT,
    fixtures=FIXTURE_FIXTURES,
    paper=("FIXTURE-RULES", 612.0, 936.0, 2),
    determinism_form="FIXTURE-RULES",
    # xref 5 is the base image, xref 8 its soft mask. Both are functions of the
    # pinned bytes, so a rebuild that renumbered them fails the pin first.
    masked=("FIXTURE-MASKS", 5, 8),
    flipped=("FIXTURE-FLIP", 5),
    paths_form="FIXTURE-PATHS",
    # 30 lone markers plus 3 that share their path with a rect, mirroring 0605.
    triangles=33,
    decimal_points=10,
    # The same three literal tones, on the same three kinds of ink: black bars,
    # a 0.8509 tint band with grey rules over it, and a white knockout box.
    tones=SELF_TEST_TONES,
    retexted_glyphs={"FIXTURE-GLYPHS": 1},
    # A Type3 glyph is addressed by its code, so the id and the code byte
    # rawdict falls back to are the same number here; on 2550M they are 131 and
    # 0xA7. What both corpora share is the substitution itself: the file states
    # no meaning, and rawdict offers a section sign that reads as content.
    retexted_glyph_id=0xA7,
    retexted_rawdict_codepoint=0xA7,
    bar_like_form="FIXTURE-LEAN",
    leaning_bars=12,
    is_evidence=False,
)

# Synthetic merge contract. The input is intentionally out of geometry order.
# Cluster one contains two distinct full repaints, an exact duplicate contributor,
# and a tiny much-later patch; cluster two joins only because its 0.01pt gap is
# within the source join epsilon. Exact equality below proves that contributors
# stay scoped to their own cluster and that neither deduplication nor hull
# expansion can turn the late patch into a full repaint.
SELF_TEST_MERGE_INTERVALS: tuple[tuple[float, float, int], ...] = (
    (30.0, 31.0, 12),
    (5.0, 5.25, 99),
    (0.0, 10.0, 8),
    (31.01, 32.0, 10),
    (9.5, 12.0, 3),
    (0.0, 10.0, 7),
    (0.0, 10.0, 8),
)
SELF_TEST_MERGED_INTERVALS = [
    (0.0, 12.0, 3, 99, (
        (0.0, 10.0, 7),
        (0.0, 10.0, 8),
        (0.0, 10.0, 8),
        (5.0, 5.25, 99),
        (9.5, 12.0, 3),
    )),
    (30.0, 32.0, 10, 12, (
        (30.0, 31.0, 12),
        (31.01, 32.0, 10),
    )),
]

PAINT_SPAN_KEYS = frozenset({"start_pt", "end_pt", "paint_seq"})
MISSING_PAINT_SPANS = object()


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


# A page whose whole purpose is to be clipped, written as a content stream so
# the assertion below compares two independent statements: what the PDF says to
# paint, and what the IR ended up holding. It is built here rather than tracked
# under fixtures/ because every case in it is a property of the `W n` operator
# rather than of any BIR form -- and because a check that only runs where the
# official PDFs live is the green tick this project has already shipped once.
#
# Page is 200x200. PDF y counts up from the bottom; the IR's counts down from
# the top, which is why the expectations below are the 200-complement.
CLIP_PROBE_STREAM = b"""0 g
q 20 100 60 60 re W n
30 110 40 40 re f
100 110 40 40 re f
60 110 40 40 re f
25 150 40 1 re f
90 150 30 1 re f
60 145 40 1 re f
q 40 120 60 20 re W n
85 122 10 16 re f
45 122 30 16 re f
Q
50 105 20 10 re f
Q
150 20 30 30 re f
"""

# (x0, y0, x1, y1) in IR coordinates, and why each one is there. Anything the
# probe page paints and this table does not name is a failure, in both
# directions -- a missed drop and an over-eager one look the same on paper.
CLIP_PROBE_AREA_FILLS: tuple[tuple[tuple[float, float, float, float], str], ...] = (
    ((30.0, 50.0, 70.0, 90.0), "wholly inside its clip"),
    ((60.0, 50.0, 80.0, 90.0), "straddles the clip's right edge, truncated to it"),
    ((45.0, 62.0, 75.0, 78.0), "inside both nested clips"),
    ((50.0, 85.0, 70.0, 95.0), "inside the outer clip, after the inner one popped"),
    ((150.0, 150.0, 180.0, 180.0), "never clipped at all"),
)

# The same, for ink thin enough to become a rule. The dropped one is the defect
# this table exists for: a bar drawn outside its scissor, which Poppler does not
# paint and which reached the IR as page structure.
CLIP_PROBE_RULES: tuple[tuple[tuple[float, float, float, float], str], ...] = (
    ((25.0, 49.0, 65.0, 50.0), "a bar inside its clip"),
    ((60.0, 54.0, 80.0, 55.0), "a bar cut short by the clip's right edge"),
)

# What the probe page proves is absent: two ops outside their scissor, one of
# them inside the *inner* clip but outside the outer, so intersecting is the
# only walk that drops it -- taking the innermost scissor alone would keep it.
CLIP_PROBE_DROPPED = 3


def probe_pdf(stream: bytes, resources: bytes = b"<<>>",
              extra_objects: Sequence[bytes] = ()) -> bytes:
    """Assemble a 200x200 probe page into a PDF, offsets and all.

    `extra_objects` are numbered from 5, which is what a `resources` dictionary
    naming them has to assume. The defaults reproduce the earlier byte-for-byte
    output, so the clip and cap probes are unaffected by the arrival of a page
    that needs fonts.
    """
    objects = [
        b"<</Type/Catalog/Pages 2 0 R>>",
        b"<</Type/Pages/Kids[3 0 R]/Count 1>>",
        b"<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]"
        b"/Contents 4 0 R/Resources" + resources + b">>",
        b"<</Length %d>>stream\n%s\nendstream" % (len(stream), stream),
        *extra_objects,
    ]
    out = bytearray(b"%PDF-1.4\n")
    offsets = []
    for number, body in enumerate(objects, 1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % number + body + b"\nendobj\n"
    start = len(out)
    out += b"xref\n0 %d\n" % (len(objects) + 1)
    out += b"0000000000 65535 f \n"
    for offset in offsets:
        out += b"%010d 00000 n \n" % offset
    out += b"trailer\n<</Size %d/Root 1 0 R>>\nstartxref\n%d\n%%%%EOF\n" % (
        len(objects) + 1, start)
    return bytes(out)


def clip_probe_pdf() -> bytes:
    """The clip probe as a PDF."""
    return probe_pdf(CLIP_PROBE_STREAM)


# A second written-here page, for the same reason the clip probe is written
# here: a line cap is a property of the `J` operator, not of any BIR form, and
# a check that only runs where the official PDFs live is the green tick this
# project has already shipped once. No fixture in either corpus draws a round
# or projecting cap, so without this page the cap model would ship unproven.
#
# Page is 200x200 and PDF y counts up from the bottom, so an IR y is the
# 200-complement. Stroke width is 1.2pt throughout -- under
# MAX_RULE_THICKNESS_PT so every bar stays a rule -- which puts the half-width,
# and therefore each cap's extension, at exactly 0.6pt.
CAP_PROBE_STREAM = b"""0 G
1.2 w
0 J
20 40 m 20 100 l S
1 J
40 40 m 40 100 l S
2 J
60 40 m 60 100 l S
1 J
80 100 m 80 40 l S
1 J
100 40 m 160 40 l S
1 J
120 100 m 180 100 l 180 130 l 150 130 l 150 160 l 120 160 l 120 100 l S
1 J
20 30 m 80 30 l 80 10 l S
"""

# (x0, y0, x1, y1) in IR coordinates, and what each one is here to settle.
# Anything the probe page paints and this table does not name is a failure, in
# both directions: an unmodelled cap and an over-applied one look the same on a
# single bar and opposite on the corpus.
CAP_PROBE_RULES: tuple[tuple[tuple[float, float, float, float], str], ...] = (
    ((19.4, 100.0, 20.6, 160.0),
     "butt cap: the ink stops at the declared endpoints, length 60.0"),
    ((39.4, 99.4, 40.6, 160.6),
     "round cap: 0.6pt past each endpoint, length 61.2"),
    ((59.4, 99.4, 60.6, 160.6),
     "projecting square cap: the same 0.6pt, length 61.2"),
    ((79.4, 99.4, 80.6, 160.6),
     "round cap on a segment drawn upwards: direction cannot move the ink"),
    ((99.4, 159.4, 160.6, 160.6),
     "round cap on the other axis: the extension follows the long axis"),
    ((120.0, 99.4, 180.0, 100.6),
     "closed subpath: no cap anywhere on it, length 60.0"),
    ((179.4, 70.0, 180.6, 100.0), "closed subpath, second edge"),
    ((150.0, 69.4, 180.0, 70.6), "closed subpath, third edge"),
    ((149.4, 40.0, 150.6, 70.0), "closed subpath, fourth edge"),
    ((120.0, 39.4, 150.0, 40.6), "closed subpath, fifth edge"),
    ((119.4, 40.0, 120.6, 100.0), "closed subpath, the edge that closes it"),
    ((19.4, 169.4, 80.0, 170.6),
     "open polyline, first leg: its free end is capped and its corner is not"),
    ((79.4, 170.0, 80.6, 190.6),
     "open polyline, second leg: the shared corner is a join, the far end a cap"),
)


def cap_probe_ir() -> dict[str, Any]:
    """The cap probe page's rules, and how its subpaths were read."""
    doc = fitz.open(stream=probe_pdf(CAP_PROBE_STREAM), filetype="pdf")
    page = doc[0]
    drawings = list(page.get_drawings())
    ir = extract_page(page, doc, 1)
    caps = [
        {"extension_pt": q(cap_extension_pt(item)),
         "capped_ops": sorted(open_stroke_ends(item["items"]).items())}
        for item in drawings
    ]
    doc.close()
    return {"rules": ir["rules"], "area_fills": ir["area_fills"],
            "paths": ir["paths"], "caps": caps}


def clip_probe_ir() -> dict[str, Any]:
    """The probe page's IR, plus whether a desynced clip walk is refused."""
    doc = fitz.open(stream=clip_probe_pdf(), filetype="pdf")
    page = doc[0]
    drawings = list(page.get_drawings())
    ir = extract_page(page, doc, 1)
    try:
        clip_scissors(page, drawings[:-1])
    except SystemExit:
        refused = True
    else:
        refused = False
    doc.close()
    return {"rules": ir["rules"], "area_fills": ir["area_fills"],
            "drawings": len(drawings), "desync_refused": refused}


# A third written-here page, for the third question no form in either corpus
# can settle on its own: what a run of underscores is. The official corpus does
# carry ruled blanks -- 2551Q has one, and it is measured through evidence["ir"]
# like everything else -- but it carries no two-underscore punctuation beside
# one, and no unresolvable face, so the two ways this must FAIL CLOSED would
# ship unproven. The synthetic corpus carries none of the three.
#
# Page is 200x200 and PDF y counts up from the bottom, so an IR y is the
# 200-complement. The filled bar is drawn FIRST so the text lands at paint
# ordinal 1: a bar published at ordinal 0 would be one this page never proved
# was read off the operator stream at all.
#
#   /F1 Helvetica     -- unembedded and base-14, the shape 111 of this corpus's
#                        118 ruled blanks have: MuPDF substitutes, and the
#                        substitute's own outline is the ink on the page.
#   /F2 ArialNarrow   -- unembedded and NOT base-14, which is 1707's shape:
#                        MuPDF still draws something, and no face this module
#                        can name states that glyph's outline, so the glyphs
#                        stay text.
RULED_BLANK_PROBE_STREAM = b"""0 g
20 20 60 1.2 re f
BT
/F1 10 Tf
0 g
20 160 Td
(AB ____ CD) Tj
ET
BT
/F1 10 Tf
0 g
20 140 Td
(XY __ Z) Tj
ET
BT
/F2 10 Tf
0 g
20 120 Td
(____) Tj
ET
"""

RULED_BLANK_PROBE_RESOURCES = b"<</Font<</F1 5 0 R/F2 6 0 R>>>>"
RULED_BLANK_PROBE_FONTS = (
    b"<</Type/Font/Subtype/Type1/BaseFont/Helvetica/Encoding/WinAnsiEncoding>>",
    b"<</Type/Font/Subtype/Type1/BaseFont/ArialNarrow/Encoding/WinAnsiEncoding>>",
)

# Every rule the probe page publishes, and why it is there. Both directions are
# asserted, as with CAP_PROBE_RULES: a blank that failed to become a rule and
# one invented for a blank that must stay text look the same on a single row
# and opposite on the corpus.
#
# The blank's band is the glyphs' own: Helvetica draws '_' from -0.176 to
# -0.126 em, so at 10pt on a baseline at IR y 40.0 the ink is 41.26 -> 41.76,
# 0.5pt thick. Its x runs from the first glyph's ink edge to the last one's,
# NOT from the run's box: the run starts at 20.0 and the bar starts at 35.9.
RULED_BLANK_PROBE_RULES: tuple[tuple[tuple[float, float, float, float],
                                     float, int, str], ...] = (
    ((20.0, 178.8, 80.0, 180.0), 1.2, 0,
     "the drawn bar, painted before the text: paint ordinal 0"),
    ((35.9, 41.26, 58.58, 41.76), 0.5, 1,
     "the ruled blank, at its glyphs' own ink band and after the bar"),
)

# What the page's text says once the blank has left it. `AB ____ CD` splits at
# the measured extents into the two fragments either side, each ending at its
# OWN outermost glyph rather than at the span's box.
RULED_BLANK_PROBE_MIXED_TEXT = "AB ____ CD"
RULED_BLANK_PROBE_SPLIT_RUNS: tuple[tuple[str, float, float, str], ...] = (
    ("AB ", 20.0, 36.12, "the mixed run's head, ending at its own last glyph"),
    (" CD", 58.36, 75.58, "and its tail, starting at its own first glyph"),
)

# The run that is below the floor, and how many groups the page holds at all.
# Two: the mixed run's blank and the unresolvable face's. `XY __ Z` is not one
# of them and that is the whole assertion -- a group is counted before it is
# measured, so a below-floor run appearing here would mean the floor had moved
# into the refusal path, where a later change could publish it.
RULED_BLANK_PROBE_BELOW_FLOOR = "XY __ Z"
RULED_BLANK_PROBE_GROUPS = 2

# The blank whose band is not derivable, kept verbatim at its own extent, and
# the reason it was refused.
RULED_BLANK_PROBE_RETAINED = ("____", 20.0, 40.0)
RULED_BLANK_PROBE_REFUSAL = "no single face states this glyph's outline"


def ruled_blank_probe_ir() -> dict[str, Any]:
    """The probe page's rules, runs and reclassification census."""
    doc = fitz.open(stream=probe_pdf(RULED_BLANK_PROBE_STREAM,
                                     RULED_BLANK_PROBE_RESOURCES,
                                     RULED_BLANK_PROBE_FONTS),
                    filetype="pdf")
    ir = extract_page(doc[0], doc, 1)
    doc.close()
    return {"rules": ir["rules"], "text_runs": ir["text_runs"],
            "stats": ir["stats"]}


# A fourth written-here page, for the fourth question no form can settle on its
# own: what happens when MuPDF's line builder puts two baselines in one span.
# The official corpus does carry the shape -- 24 spans across 11 forms -- but
# those files are untracked, and the synthetic corpus carries none, so without
# this page the split would ship proven on nobody's machine but the operator's.
#
# Page is 200x200 and PDF y counts up from the bottom, so an IR y is the
# 200-complement. Every op is /F1 Helvetica at 10pt so that font, size and
# colour cannot be what separates the spans -- only the baseline can.
#
#   XY / Z     two INK stretches, 1pt apart and set edge to edge so no space is
#              bridged between them: 1706 page 2's `1 A` shape.
#   ' ' / PQ   a positioning space 4pt above the ink it precedes: the shape 22
#              of the corpus's 24 carry, and the one that moved `Calendar`
#              (F070) and ` Total Tax-` (F102) off their printed baselines.
#   RS         a control on one baseline, which must come through untouched.
BASELINE_PROBE_STREAM = b"""0 g
BT
/F1 10 Tf
20 160 Td
(XY) Tj
ET
BT
/F1 10 Tf
33.34 159 Td
(Z) Tj
ET
BT
/F1 10 Tf
20 130 Td
( ) Tj
ET
BT
/F1 10 Tf
26 126 Td
(PQ) Tj
ET
BT
/F1 10 Tf
20 100 Td
(RS) Tj
ET
"""

BASELINE_PROBE_RESOURCES = b"<</Font<</F1 5 0 R>>>>"
BASELINE_PROBE_FONTS = (
    b"<</Type/Font/Subtype/Type1/BaseFont/Helvetica/Encoding/WinAnsiEncoding>>",
)

# What MuPDF's own grouping does with that page, asserted first and separately.
# Without it the page could stop provoking a merged span -- a reader change, a
# different line tolerance -- and the run table below would then pass while
# proving nothing at all. Each entry is (text, distinct quantised baselines).
BASELINE_PROBE_SPANS: tuple[tuple[str, int], ...] = (
    ("XYZ", 2),
    (" PQ", 2),
    ("RS", 1),
)

# Every run the page publishes, in the extractor's own order, and why. Both
# directions are asserted: a split that failed to happen and one that cut a
# single-baseline span look the same on one row and opposite on the corpus.
BASELINE_PROBE_RUNS: tuple[tuple[str, tuple[float, float, float, float],
                                 float, float, str], ...] = (
    ("XY", (20.0, 29.25, 33.34, 42.99), 40.0, 20.0,
     "the first stretch, at its own baseline and its own box"),
    ("Z", (33.34, 30.25, 39.45, 43.99), 41.0, 33.34,
     "and the second, 1pt below it -- one span, two runs"),
    ("PQ", (26.0, 63.25, 40.45, 76.99), 74.0, 26.0,
     "the ink alone: the space 4pt above it is a whitespace-only stretch and "
     "is dropped exactly as a whitespace-only span already was"),
    ("RS", (20.0, 89.25, 33.89, 102.99), 100.0, 20.0,
     "the control: one baseline in, one run out, at the span's own box"),
)


def baseline_probe_ir() -> dict[str, Any]:
    """The probe page's runs, and MuPDF's own span grouping of it."""
    doc = fitz.open(stream=probe_pdf(BASELINE_PROBE_STREAM,
                                     BASELINE_PROBE_RESOURCES,
                                     BASELINE_PROBE_FONTS),
                    filetype="pdf")
    page = doc[0]
    spans: list[tuple[str, int]] = []
    for block in page.get_text("rawdict")["blocks"]:
        if block["type"] != 0:
            continue
        for line in block["lines"]:
            for span in line["spans"]:
                chars = span.get("chars") or []
                if not chars:
                    continue
                spans.append(("".join(char["c"] for char in chars),
                              len({q(char["origin"][1]) for char in chars})))
    ir = extract_page(page, doc, 1)
    doc.close()
    return {"text_runs": ir["text_runs"], "spans": spans}


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


def gather_evidence(profile: SelfTestProfile,
                    source_root: pathlib.Path) -> dict[str, Any]:
    """Extract every fixture once, plus the source facts the checks compare to.

    Everything a check reads lives in this bundle, and nothing in it is a live
    fitz object. That is what lets mutation_probes deep-copy it, break one
    property, and watch exactly one check trip.

    The profile travels inside the bundle so that a check reads its subjects
    from the corpus it was handed, rather than from a module constant naming a
    form the corpus may not contain.
    """
    evidence: dict[str, Any] = {
        "profile": profile,
        "ir": {}, "serialisations": [], "base_pixel_sha256": {},
        "mask_is_opaque": {},
        "codepoints": {}, "leaning_bars": {}, "desync": {},
        "merged_intervals": merge_intervals(list(SELF_TEST_MERGE_INTERVALS)),
        # Deliberately outside "ir": the clip probe is a page this module wrote
        # to interrogate one operator, not a form, and the corpus-wide checks
        # must not read it as one.
        "clip_probe": clip_probe_ir(),
        # Same reasoning, for the `J` operator.
        "cap_probe": cap_probe_ir(),
        # And for a run of underscores, whose two fail-closed cases no form in
        # either corpus states.
        "ruled_blank_probe": ruled_blank_probe_ir(),
        # And for a rawdict span carrying two baselines, which the tracked
        # corpus does not.
        "baseline_probe": baseline_probe_ir(),
    }
    for code, (relative, revision, digest) in profile.fixtures.items():
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

    code = profile.determinism_form
    relative, revision, digest = profile.fixtures[code]
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
        return [f"two extractions of {evidence['profile'].determinism_form} "
                f"differ: {len(payloads[0])} vs {len(payloads[1])} chars"]
    return []


def check_paper(evidence: dict[str, Any]) -> list[str]:
    """Paper is the PDF's own MediaBox, per page, exactly."""
    code, width, height, page_count = evidence["profile"].paper
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
                    if (isinstance(first, bool)
                            or not isinstance(first, int) or first < 0):
                        failures.append(f"{label} paint_seq is {first!r}")
                    elif (isinstance(last, bool)
                          or not isinstance(last, int) or last < first):
                        failures.append(f"{label} paint_seq_max {last!r} < {first}")
    return failures


def is_finite_coordinate(value: Any) -> bool:
    """Whether a JSON value is a real, finite coordinate rather than bool."""
    return (not isinstance(value, bool)
            and isinstance(value, (int, float))
            and math.isfinite(float(value)))


def is_paint_ordinal(value: Any) -> bool:
    """Whether a JSON value is a non-negative integer paint ordinal."""
    return not isinstance(value, bool) and isinstance(value, int) and value >= 0


def validate_rule_paint_spans(rule: dict[str, Any], label: str) -> list[str]:
    """Validate exact contributor provenance for one emitted merged rule.

    This is deliberately stricter than a shape check. The contributor entries
    must be canonical and complete enough to reproduce the merge: one connected
    JOIN_EPSILON cluster, exact long-axis hull, and exact ordinal min/max. A
    consumer can then ask which source paints covered a local slab without ever
    assigning one contributor's ordinal to the complete merged hull.
    """
    failures: list[str] = []
    raw = rule.get("paint_spans", MISSING_PAINT_SPANS)
    if raw is MISSING_PAINT_SPANS:
        return [f"{label} has no paint_spans"]
    if not isinstance(raw, list):
        return [f"{label} paint_spans is {type(raw).__name__}, expected list"]
    if not raw:
        return [f"{label} paint_spans is empty"]

    parsed: list[tuple[float, float, int]] = []
    for position, span in enumerate(raw):
        span_label = f"{label} paint_spans[{position}]"
        if not isinstance(span, dict):
            failures.append(f"{span_label} is {type(span).__name__}, expected object")
            continue
        keys = frozenset(span)
        if keys != PAINT_SPAN_KEYS:
            missing = sorted(PAINT_SPAN_KEYS - keys)
            extra = sorted(keys - PAINT_SPAN_KEYS)
            failures.append(f"{span_label} keys differ: missing={missing} extra={extra}")
            continue

        start = span["start_pt"]
        end = span["end_pt"]
        seq = span["paint_seq"]
        entry_valid = True
        for field, value in (("start_pt", start), ("end_pt", end)):
            if not is_finite_coordinate(value):
                failures.append(f"{span_label} {field} is not a finite number: "
                                f"{value!r}")
                entry_valid = False
            elif q(value) != float(value):
                failures.append(f"{span_label} {field} is not q-coordinate: "
                                f"{value!r}")
                entry_valid = False
        if not is_paint_ordinal(seq):
            failures.append(f"{span_label} paint_seq is not a non-negative "
                            f"integer: {seq!r}")
            entry_valid = False
        if entry_valid and float(end) <= float(start):
            failures.append(f"{span_label} has non-positive extent "
                            f"{start!r}->{end!r}")
            entry_valid = False
        if entry_valid:
            parsed.append((q(start), q(end), seq))

    # Invalid entries have already earned a precise error. Do not let them
    # participate in sorting or union arithmetic and manufacture secondary ones.
    if len(parsed) != len(raw):
        return failures

    canonical = sorted(parsed, key=lambda span: (span[0], span[1], span[2]))
    if parsed != canonical:
        failures.append(f"{label} paint_spans are not ordered by "
                        "(start_pt, end_pt, paint_seq)")

    axis = rule.get("axis")
    if axis == "h":
        rule_start, rule_end = rule.get("x0"), rule.get("x1")
    elif axis == "v":
        rule_start, rule_end = rule.get("y0"), rule.get("y1")
    else:
        failures.append(f"{label} has invalid axis {axis!r}")
        return failures
    bound_fields = (("x0", rule_start), ("x1", rule_end)) if axis == "h" else (
        ("y0", rule_start), ("y1", rule_end))
    invalid_bounds = False
    for field, value in bound_fields:
        if not is_finite_coordinate(value):
            failures.append(f"{label} {field} is not a finite number: {value!r}")
            invalid_bounds = True
        elif q(value) != float(value):
            failures.append(f"{label} {field} is not q-coordinate: {value!r}")
            invalid_bounds = True
    if invalid_bounds:
        return failures

    cluster_start = canonical[0][0]
    cluster_end = canonical[0][1]
    cluster_count = 1
    for start, end, _seq in canonical[1:]:
        if start > cluster_end + JOIN_EPSILON_PT:
            cluster_count += 1
        cluster_end = max(cluster_end, end)
    if cluster_count != 1:
        failures.append(f"{label} paint_spans form {cluster_count} clusters, "
                        "expected exactly 1")
    if float(rule_start) != cluster_start or float(rule_end) != q(cluster_end):
        failures.append(f"{label} long-axis bounds {rule_start}->{rule_end} "
                        f"do not equal contributor union {cluster_start}->{q(cluster_end)}")

    seqs = [seq for _start, _end, seq in canonical]
    first = rule.get("paint_seq")
    last = rule.get("paint_seq_max")
    if not is_paint_ordinal(first) or first != min(seqs):
        failures.append(f"{label} paint_seq {first!r} != contributor min "
                        f"{min(seqs)}")
    if not is_paint_ordinal(last) or last != max(seqs):
        failures.append(f"{label} paint_seq_max {last!r} != contributor max "
                        f"{max(seqs)}")
    return failures


def check_rule_paint_spans(evidence: dict[str, Any]) -> list[str]:
    """Every newly extracted rule carries a complete contributor contract."""
    failures: list[str] = []
    count = 0
    for code, ir in evidence["ir"].items():
        for page in ir["pages"]:
            for position, rule in enumerate(page["rules"]):
                count += 1
                failures.extend(validate_rule_paint_spans(
                    rule, f"{code} p{page['index']} rules[{position}]"))
    if count == 0:
        failures.append("paint_spans contract measured no rules")
    return failures


def check_interval_provenance(evidence: dict[str, Any]) -> list[str]:
    """The interval union retains exact, duplicate, cluster-scoped contributors."""
    actual = evidence.get("merged_intervals")
    if actual != SELF_TEST_MERGED_INTERVALS:
        return [f"merged interval provenance {actual!r} != "
                f"{SELF_TEST_MERGED_INTERVALS!r}"]
    return []


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
    code, xref, mask_xref = evidence["profile"].masked
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

    code, xref = evidence["profile"].flipped
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
    profile = evidence["profile"]
    code = profile.paths_form
    ir = evidence["ir"][code]
    page = ir["pages"][0]
    failures: list[str] = []

    triangles = [path["id"] for path in page["paths"] if is_filled_triangle(path)]
    if len(triangles) != profile.triangles:
        failures.append(f"{code} page 1 has {len(triangles)} filled "
                        f"triangle paths, expected {profile.triangles}")
    marks = [path["id"] for path in page["paths"] if is_filled_curve_mark(path)]
    if len(marks) != profile.decimal_points:
        failures.append(f"{code} page 1 has {len(marks)} filled "
                        f"decimal-point marks, expected {profile.decimal_points}")

    phantom = [f"p{page['index']}:{rule['id']}" for page in ir["pages"]
               for rule in page["rules"]
               if rule["thickness_pt"] == SELF_TEST_PHANTOM_THICKNESS_PT]
    if phantom:
        failures.append(f"{code} carries {len(phantom)} rule(s) at "
                        f"the invented {SELF_TEST_PHANTOM_THICKNESS_PT}pt default "
                        f"({', '.join(phantom[:5])})")
    return failures


def ink_tones(page: dict[str, Any]) -> list[tuple[str, float | None, str]]:
    """Every piece of ink on a page as (label, its grey, the role it claims).

    A path's tone is its fill's when it has one and its outline's otherwise --
    the same reading extract_paths made -- because a filled mark is the colour
    it is filled with whatever it is outlined in.
    """
    items: list[tuple[str, float | None, str]] = []
    for kind in ("rules", "area_fills"):
        for position, item in enumerate(page[kind]):
            items.append((f"{kind}[{position}]", item["gray"], item["role"]))
    for position, path in enumerate(page["paths"]):
        tone = path["fill_gray"] if path["fill"] is not None else path["stroke_gray"]
        items.append((f"paths[{position}]", tone, path["role"]))
    return items


def check_tone(evidence: dict[str, Any]) -> list[str]:
    """Ink is classified by its literal grey, and all three bands are populated.

    Tone is the one thing separating a black rule from decoration a reader can
    barely see, and the project has already shipped the failure this guards:
    acting on ink *presence* painted 0.8509 grey black, which improved a
    structural-recall metric while putting black over near-invisible
    decoration. So the band boundaries are pinned against literal values the
    generator really uses, and the corpus is required to carry ink of each --
    a threshold that moved would either misname existing ink or empty a band,
    and both are caught here.
    """
    profile = evidence["profile"]
    failures: list[str] = []
    census: collections.Counter[str] = collections.Counter()
    for code, ir in evidence["ir"].items():
        for page in ir["pages"]:
            for label, tone, claimed in ink_tones(page):
                derived = classify_tone(tone)
                census[derived] += 1
                if claimed != derived:
                    failures.append(f"{code} p{page['index']} {label} is grey "
                                    f"{tone!r} but claims role {claimed!r}, "
                                    f"not {derived!r}")
    for value, expected in profile.tones:
        actual = classify_tone(value)
        if actual != expected:
            failures.append(f"grey {value} classifies as {actual!r}, "
                            f"expected {expected!r} -- a tone band moved")
        if not census[expected]:
            failures.append(f"the corpus carries no {expected!r} ink, so the "
                            f"{expected!r} band is pinned against nothing")
    return failures


def check_clips(evidence: dict[str, Any]) -> list[str]:
    """Ink outside its scissor never reaches the IR, and ink inside is untouched.

    Both halves matter. Dropping too little is the shipped defect -- 22 rules
    painted black on 6 forms where the official sheet is blank. Dropping too
    much would be worse and quieter: structure the reader can see, missing.
    """
    probe = evidence["clip_probe"]
    failures: list[str] = []
    if not probe["desync_refused"]:
        failures.append("clip_scissors accepted a drawings list the extended "
                        "walk cannot explain instead of raising")

    for name, expected in (("area_fills", CLIP_PROBE_AREA_FILLS),
                           ("rules", CLIP_PROBE_RULES)):
        want = {box: why for box, why in expected}
        got = {(item["x0"], item["y0"], item["x1"], item["y1"])
               for item in probe[name]}
        for box, why in want.items():
            if box not in got:
                failures.append(f"clip probe lost the {name[:-1]} at {box} "
                                f"({why})")
        for box in sorted(got - set(want)):
            failures.append(f"clip probe kept a {name[:-1]} at {box} that its "
                            f"scissor does not let through")

    kept = len(probe["area_fills"]) + len(probe["rules"])
    if probe["drawings"] - kept != CLIP_PROBE_DROPPED:
        failures.append(
            f"clip probe painted {probe['drawings']} ops and kept {kept}; "
            f"{CLIP_PROBE_DROPPED} are outside their scissor")
    return failures


def check_stroke_caps(evidence: dict[str, Any]) -> list[str]:
    """A stroke's ink runs exactly as far past its endpoints as its cap says.

    Under-reporting is the shipped defect: a round-capped comb tick published at
    its declared endpoints stops short of the rail it lands on, so
    lattice.split_verticals files it as a box border and the compartment it was
    dividing disappears. Over-reporting would be worse and quieter -- capping a
    closed rectangle or an interior polyline vertex grows real structure by half
    a stroke on sides the source never inked -- so both are asserted here, on
    one page whose whole purpose is to state every case of `J` at once.
    """
    probe = evidence["cap_probe"]
    failures: list[str] = []

    want = {box: why for box, why in CAP_PROBE_RULES}
    got = {(rule["x0"], rule["y0"], rule["x1"], rule["y1"])
           for rule in probe["rules"]}
    for box, why in want.items():
        if box not in got:
            failures.append(f"cap probe lost the rule at {box} ({why})")
    for box in sorted(got - set(want)):
        failures.append(f"cap probe painted a rule at {box} that no cap on its "
                        f"page can account for")
    if probe["area_fills"] or probe["paths"]:
        failures.append(
            f"cap probe diverted ink away from `rules`: "
            f"{len(probe['area_fills'])} area fill(s), {len(probe['paths'])} path(s)")

    # The measured extension, stated independently of the geometry above so a
    # coincidence in one bar cannot stand in for the rule.
    extensions = [entry["extension_pt"] for entry in probe["caps"]]
    if extensions != [0.0, 0.6, 0.6, 0.6, 0.6, 0.6, 0.6]:
        failures.append(f"cap probe measured extensions {extensions}, expected "
                        f"0.0 for the butt cap and 0.6 for every other stroke")
    closed = probe["caps"][5]["capped_ops"] if len(probe["caps"]) > 5 else None
    if closed != []:
        failures.append(f"cap probe capped the closed subpath's ops {closed}; "
                        f"a closed subpath has joins everywhere and no cap")
    polyline = probe["caps"][6]["capped_ops"] if len(probe["caps"]) > 6 else None
    if polyline != [(0, (True, False)), (1, (False, True))]:
        failures.append(f"cap probe read the open polyline's ends as {polyline}; "
                        f"only its first and last points are caps")
    return failures


def _probe_groups_in_text(probe: dict[str, Any]) -> int:
    """How many ruled-blank groups the probe page's runs still spell out."""
    return sum(len(ruled_blank_groups(run["text"]))
               for run in probe["text_runs"])


def check_ruled_blank_split(evidence: dict[str, Any]) -> list[str]:
    """A run holding a ruled blank splits into text, the rule, and text.

    The rule is at the GLYPHS' OWN INK BAND -- the outline the face that drew
    them states, at the size and baseline the file states -- and not at the
    run's box, which is most of an em taller and starts three characters to the
    left. Both directions are asserted: a blank that stayed text and a rule
    invented for text that is not a blank are the same failure seen from
    opposite sides.
    """
    probe = evidence["ruled_blank_probe"]
    failures: list[str] = []

    want = {box: (thickness, seq, why)
            for box, thickness, seq, why in RULED_BLANK_PROBE_RULES}
    got = {(rule["x0"], rule["y0"], rule["x1"], rule["y1"]): rule
           for rule in probe["rules"]}
    for box, (thickness, seq, why) in want.items():
        rule = got.get(box)
        if rule is None:
            failures.append(f"ruled-blank probe lost the rule at {box} ({why})")
            continue
        if rule["thickness_pt"] != thickness:
            failures.append(f"ruled-blank probe's rule at {box} is "
                            f"{rule['thickness_pt']}pt thick, expected {thickness}")
        if rule["paint_seq"] != seq or rule["paint_seq_max"] != seq:
            failures.append(
                f"ruled-blank probe's rule at {box} paints at "
                f"{rule['paint_seq']}..{rule['paint_seq_max']}, expected {seq}")
    for box in sorted(got.keys() - want.keys()):
        failures.append(f"ruled-blank probe painted a rule at {box} that "
                        f"nothing on its page draws")

    fragments = {(run["text"], run["x0"], run["x1"]) for run in probe["text_runs"]}
    for text, x0, x1, why in RULED_BLANK_PROBE_SPLIT_RUNS:
        if (text, x0, x1) not in fragments:
            failures.append(f"ruled-blank probe has no run {text!r} at "
                            f"{x0}->{x1} ({why}); it reads "
                            f"{sorted(fragments)}")
    if any(run["text"] == RULED_BLANK_PROBE_MIXED_TEXT
           for run in probe["text_runs"]):
        failures.append(f"ruled-blank probe still prints "
                        f"{RULED_BLANK_PROBE_MIXED_TEXT!r} as text: the blank "
                        f"was published as a rule AND left in the run")
    return failures


def check_ruled_blank_floor(evidence: dict[str, Any]) -> list[str]:
    """Two underscores are punctuation and stay in the text they punctuate.

    The floor is what keeps this from eating an ellipsis stand-in or a fill
    character, and it is asserted on the count as well as on the run: a group
    below the floor must not even be offered for measurement, or a later change
    to the refusal path could start publishing it.
    """
    probe = evidence["ruled_blank_probe"]
    failures: list[str] = []
    below = RULED_BLANK_PROBE_BELOW_FLOOR
    if ruled_blank_groups(below):
        failures.append(f"{below!r} is pinned as below the floor but "
                        f"ruled_blank_groups reads a group in it")
    if not any(run["text"] == below for run in probe["text_runs"]):
        failures.append(f"ruled-blank probe lost {below!r}: a run of "
                        f"{RULED_BLANK_MIN_GLYPHS - 1} underscores is not a "
                        f"blank and must stay text, verbatim")
    counted = probe["stats"]["ruled_blank_groups"]
    if counted != RULED_BLANK_PROBE_GROUPS:
        failures.append(f"ruled-blank probe counted {counted} group(s), "
                        f"expected {RULED_BLANK_PROBE_GROUPS}: the below-floor "
                        f"run must not be one of them")
    return failures


def check_ruled_blank_fail_closed(evidence: dict[str, Any]) -> list[str]:
    """A blank whose ink band is not derivable stays text, and is counted.

    Never guess a band. The probe's second face is unembedded and is not one
    MuPDF's own name cleaner resolves, so no face this module can name states
    that glyph's outline -- exactly 1707's shape. The count and the surviving
    runs are asserted against each other, so neither a silently dropped blank
    nor an unrecorded refusal can pass.
    """
    probe = evidence["ruled_blank_probe"]
    stats = probe["stats"]
    failures: list[str] = []
    text, x0, x1 = RULED_BLANK_PROBE_RETAINED
    if (text, x0, x1) not in {(run["text"], run["x0"], run["x1"])
                              for run in probe["text_runs"]}:
        failures.append(f"ruled-blank probe lost the run {text!r} at "
                        f"{x0}->{x1}: a blank whose band cannot be derived "
                        f"stays text, it does not vanish")
    retained = _probe_groups_in_text(probe)
    if stats["ruled_blank_text_retained"] != retained:
        failures.append(
            f"ruled-blank probe reports {stats['ruled_blank_text_retained']} "
            f"group(s) retained as text but its runs still spell {retained}")
    if retained != 1:
        failures.append(f"ruled-blank probe retained {retained} group(s) as "
                        f"text, expected the one face it cannot resolve")
    if stats["ruled_blank_refusals"] != {RULED_BLANK_PROBE_REFUSAL: 1}:
        failures.append(f"ruled-blank probe recorded refusals "
                        f"{stats['ruled_blank_refusals']}, expected exactly "
                        f"{{{RULED_BLANK_PROBE_REFUSAL!r}: 1}}")
    if stats["ruled_blank_groups"] - stats["ruled_blank_published"] != retained:
        failures.append(
            f"ruled-blank probe saw {stats['ruled_blank_groups']} group(s) and "
            f"published {stats['ruled_blank_published']}; the difference must "
            f"be the {retained} it refused")
    return failures


def check_baseline_split(evidence: dict[str, Any]) -> list[str]:
    """One run states one baseline, and the reader is not asked to guarantee it.

    Asserted in three parts, and the first is what makes the other two mean
    anything: the page must still provoke MuPDF into merging two baselines into
    one span. A reader whose line builder stopped doing that would leave a probe
    that passes while measuring nothing -- the green tick this project has
    already shipped once.

    Then the exact run table, in both directions. A split that failed to happen
    leaves a run at the wrong baseline with a box unioning two lines; a split
    that cut a single-baseline span leaves the control in pieces. Both are
    failures here, and only one of them is the defect that was found.
    """
    probe = evidence["baseline_probe"]
    failures: list[str] = []

    spans = [tuple(entry) for entry in probe["spans"]]
    if spans != list(BASELINE_PROBE_SPANS):
        failures.append(
            f"baseline probe's reader grouped its page into {spans}, expected "
            f"{list(BASELINE_PROBE_SPANS)}: the page exists to carry a span "
            f"with two baselines, and it no longer does")

    want = [(text, box, baseline, origin_x)
            for text, box, baseline, origin_x, _why in BASELINE_PROBE_RUNS]
    got = [(run["text"], (run["x0"], run["y0"], run["x1"], run["y1"]),
            run["baseline_y"], run["origin_x"]) for run in probe["text_runs"]]
    for position in range(max(len(want), len(got))):
        expected = want[position] if position < len(want) else None
        entry = got[position] if position < len(got) else None
        if entry == expected:
            continue
        if expected is None:
            failures.append(f"baseline probe published run {position} as "
                            f"{entry}, which nothing on its page draws")
        elif entry is None:
            failures.append(
                f"baseline probe lost run {position}, {expected} "
                f"({BASELINE_PROBE_RUNS[position][4]})")
        else:
            failures.append(
                f"baseline probe reads run {position} as {entry}, expected "
                f"{expected} ({BASELINE_PROBE_RUNS[position][4]})")
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

    profile = evidence["profile"]
    for code, ir in evidence["ir"].items():
        expected = profile.retexted_glyphs.get(code, 0)
        carried = [(page["index"], run) for page in ir["pages"]
                   for run in page["text_runs"] if run["unmapped_glyphs"]]
        total = sum(len(run["unmapped_glyphs"]) for _, run in carried)
        if total != expected:
            failures.append(f"{code} carries {total} unmapped glyph(s), "
                            f"expected {expected}")
        for index, run in carried:
            for glyph in run["unmapped_glyphs"]:
                label = f"{code} p{index} glyph {glyph['index']}"
                if glyph["glyph_id"] != profile.retexted_glyph_id:
                    failures.append(f"{label} is glyph id {glyph['glyph_id']}, "
                                    f"expected {profile.retexted_glyph_id}")
                if run["text"][glyph["index"]] != UNMAPPED_CODEPOINT:
                    failures.append(f"{label} reads "
                                    f"{run['text'][glyph['index']]!r}, expected "
                                    f"{UNMAPPED_CODEPOINT!r}")
                if glyph["rawdict_codepoint"] != profile.retexted_rawdict_codepoint:
                    failures.append(
                        f"{label} records rawdict codepoint "
                        f"{hex(glyph['rawdict_codepoint'])}, expected "
                        f"{hex(profile.retexted_rawdict_codepoint)} -- the "
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
    profile = evidence["profile"]
    code = profile.bar_like_form
    ir = evidence["ir"][code]
    bars = evidence["leaning_bars"][code]
    failures: list[str] = []

    if len(bars) != profile.leaning_bars:
        failures.append(f"{code} draws {len(bars)} leaning segment(s), expected "
                        f"{profile.leaning_bars}")

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
    ("paint-spans", check_rule_paint_spans),
    ("interval-provenance", check_interval_provenance),
    ("paint-order-reconciliation", check_paint_order_reconciliation),
    ("soft-masks", check_soft_masks),
    ("transforms", check_transforms),
    ("paths", check_paths),
    ("tone", check_tone),
    ("clips", check_clips),
    ("stroke-caps", check_stroke_caps),
    ("ruled-blank-split", check_ruled_blank_split),
    ("ruled-blank-floor", check_ruled_blank_floor),
    ("ruled-blank-fail-closed", check_ruled_blank_fail_closed),
    ("baseline-split", check_baseline_split),
    ("codepoints", check_codepoints),
    ("is-bar-like", check_bar_like),
)


def mutate_determinism(evidence: dict[str, Any]) -> None:
    evidence["serialisations"][1] += " "


def mutate_paper(evidence: dict[str, Any]) -> None:
    evidence["ir"][evidence["profile"].paper[0]]["paper"]["height_pt"] = 792.0


def mutate_paint_seq(evidence: dict[str, Any]) -> None:
    # Use non-rule ink so this mutation isolates the general paint-seq check
    # from the stricter rule-only contributor contract.
    for ir in evidence["ir"].values():
        for page in ir["pages"]:
            for kind in ("area_fills", "paths", "images"):
                if page[kind]:
                    del page[kind][0]["paint_seq"]
                    return
    raise AssertionError("paint-seq mutation found no non-rule ink")


def mutate_paint_spans(evidence: dict[str, Any]) -> None:
    """Strip one rule's contributor list, wherever the corpus keeps its rules."""
    for ir in evidence["ir"].values():
        for page in ir["pages"]:
            if page["rules"]:
                del page["rules"][0]["paint_spans"]
                return
    raise AssertionError("paint-spans mutation found no rule")


def mutate_interval_provenance(evidence: dict[str, Any]) -> None:
    """Lose one exact duplicate while leaving the merged hull and range intact."""
    first = list(evidence["merged_intervals"][0])
    spans = list(first[4])
    del spans[2]
    first[4] = tuple(spans)
    evidence["merged_intervals"][0] = tuple(first)


def mutate_paint_order_reconciliation(evidence: dict[str, Any]) -> None:
    code = next(iter(evidence["desync"]))
    evidence["desync"][code]["fewer_drawings_than_ops"] = False


def mutate_soft_masks(evidence: dict[str, Any]) -> None:
    """Restore the pre-compositing digest: the mask silently dropped again."""
    code, xref, _ = evidence["profile"].masked
    for page in evidence["ir"][code]["pages"]:
        for image in page["images"]:
            if image["xref"] == xref:
                image["pixel_sha256"] = evidence["base_pixel_sha256"][code][xref]


def mutate_transforms(evidence: dict[str, Any]) -> None:
    """Flip the flip back, as a bounding box would have."""
    code, xref = evidence["profile"].flipped
    for page in evidence["ir"][code]["pages"]:
        for image in page["images"]:
            if image["xref"] == xref and image["transform"]:
                image["transform"][3] = abs(image["transform"][3])


def mutate_paths(evidence: dict[str, Any]) -> None:
    """Drop one marker, as the rule classifier used to."""
    page = evidence["ir"][evidence["profile"].paths_form]["pages"][0]
    for position, path in enumerate(page["paths"]):
        if is_filled_triangle(path):
            del page["paths"][position]
            return
    raise AssertionError("paths mutation found no filled triangle")


def mutate_tone(evidence: dict[str, Any]) -> None:
    """Report a decorative grey as structure, which is how it gets painted black."""
    for ir in evidence["ir"].values():
        for page in ir["pages"]:
            for rule in page["rules"]:
                if rule["role"] == "decorative":
                    rule["role"] = "structural"
                    return
    raise AssertionError("tone mutation found no decorative rule")


def mutate_clips(evidence: dict[str, Any]) -> None:
    """Paint the escaped bar after all, exactly as no clip handling did."""
    probe = evidence["clip_probe"]
    escaped = dict(probe["rules"][0])
    escaped.update(x0=90.0, y0=49.0, x1=120.0, y1=50.0)
    probe["rules"] = [*probe["rules"], escaped]


def mutate_codepoints(evidence: dict[str, Any]) -> None:
    """Print the section sign rawdict offered, where the file states nothing."""
    for ir in evidence["ir"].values():
        for page in ir["pages"]:
            for run in page["text_runs"]:
                if run["unmapped_glyphs"]:
                    index = run["unmapped_glyphs"][0]["index"]
                    run["text"] = (run["text"][:index] + "§"
                                   + run["text"][index + 1:])
                    return
    raise AssertionError("codepoints mutation found no unmapped glyph")


def mutate_stroke_caps(evidence: dict[str, Any]) -> None:
    """Publish the round-capped bar at its declared endpoints.

    This is not an invented failure: it is what this module did until the cap
    model landed, and it is the state in which 2550M's four year boxes reached
    the taxpayer as one wide input.
    """
    probe = evidence["cap_probe"]
    for rule in probe["rules"]:
        if (rule["x0"], rule["y0"], rule["x1"], rule["y1"]) == (
                39.4, 99.4, 40.6, 160.6):
            rule.update(y0=100.0, y1=160.0, length_pt=60.0)
            return
    raise AssertionError("stroke-caps mutation found no round-capped bar")


def mutate_ruled_blank_split(evidence: dict[str, Any]) -> None:
    """Publish the blank at its RUN's extent instead of its glyphs' ink.

    The reverted emit-side attempt (F200) placed the input on the run's own
    x-extent, and this is the same mistake one layer up: the run starts at 20.0
    where the ink starts at 35.9, so a rule taken from the box would be drawn
    through `AB `.
    """
    probe = evidence["ruled_blank_probe"]
    for rule in probe["rules"]:
        if (rule["x0"], rule["y0"], rule["x1"], rule["y1"]) == (
                35.9, 41.26, 58.58, 41.76):
            rule.update(x0=20.0, x1=75.58, length_pt=55.58)
            return
    raise AssertionError("ruled-blank-split mutation found no reclassified bar")


def mutate_ruled_blank_floor(evidence: dict[str, Any]) -> None:
    """Take the two-underscore run's underscores out of its text."""
    probe = evidence["ruled_blank_probe"]
    for run in probe["text_runs"]:
        if run["text"] == RULED_BLANK_PROBE_BELOW_FLOOR:
            run["text"] = run["text"].replace(RULED_BLANK_CHARACTER, "")
            return
    raise AssertionError("ruled-blank-floor mutation found no below-floor run")


def mutate_ruled_blank_fail_closed(evidence: dict[str, Any]) -> None:
    """Drop the run whose band could not be derived, publishing nothing for it.

    The quiet failure this exists to catch: ink that leaves the text and never
    arrives in the rules, so the sheet loses a line nobody counted.
    """
    probe = evidence["ruled_blank_probe"]
    before = len(probe["text_runs"])
    probe["text_runs"] = [run for run in probe["text_runs"]
                          if not ruled_blank_groups(run["text"])]
    if len(probe["text_runs"]) == before:
        raise AssertionError("ruled-blank-fail-closed mutation found no "
                             "retained blank")


def mutate_baseline_split(evidence: dict[str, Any]) -> None:
    """Re-read the probe page with the split disabled, exactly as it was read.

    Re-extracted rather than written out as a literal table. The state this must
    reproduce is what the module DID -- one run per span, on its first glyph's
    baseline, boxed to the whole span -- and a hand-written table of that state
    could drift into disagreeing with the expectation beside it for some other
    reason, which would let this mutation trip the check while proving nothing.
    """
    probe = evidence["baseline_probe"]
    whole_span = globals()["baseline_groups"]
    globals()["baseline_groups"] = lambda chars: [(0, len(chars))]
    try:
        unsplit = baseline_probe_ir()
    finally:
        globals()["baseline_groups"] = whole_span
    if unsplit["text_runs"] == probe["text_runs"]:
        raise AssertionError("baseline-split mutation changed nothing: the "
                             "probe page carries no span with two baselines")
    probe["text_runs"] = unsplit["text_runs"]


def mutate_bar_like(evidence: dict[str, Any]) -> None:
    """Divert one leaning separator to paths, as exact alignment would have."""
    code = evidence["profile"].bar_like_form
    bar = evidence["leaning_bars"][code][0]
    for page in evidence["ir"][code]["pages"]:
        if page["index"] != bar["page"]:
            continue
        page["rules"] = [rule for rule in page["rules"]
                         if not bar_matches_rule(rule, bar)]


SELF_TEST_MUTATIONS: tuple[tuple[str, str, Callable[[dict[str, Any]], None]], ...] = (
    ("determinism", "one serialisation gains a byte", mutate_determinism),
    ("paper", "the paper subject's height becomes Letter", mutate_paper),
    ("paint-seq", "a rule loses its paint_seq", mutate_paint_seq),
    ("paint-spans", "a rule loses its contributor list", mutate_paint_spans),
    ("interval-provenance", "an exact duplicate contributor is deduplicated",
     mutate_interval_provenance),
    ("paint-order-reconciliation", "a desync is accepted instead of raising",
     mutate_paint_order_reconciliation),
    ("soft-masks", "a masked image reports its unmasked pixels", mutate_soft_masks),
    ("transforms", "the flipped placement loses its negative d",
     mutate_transforms),
    ("paths", "one filled triangle is dropped", mutate_paths),
    ("tone", "a decorative grey rule is reported as structural", mutate_tone),
    ("clips", "a bar drawn outside its scissor is painted anyway", mutate_clips),
    ("stroke-caps", "a round-capped bar is published at its declared endpoints",
     mutate_stroke_caps),
    ("ruled-blank-split", "a ruled blank is published at its run's extent",
     mutate_ruled_blank_split),
    ("ruled-blank-floor", "a two-underscore run loses its underscores",
     mutate_ruled_blank_floor),
    ("ruled-blank-fail-closed", "a blank with no derivable band is dropped "
     "instead of kept as text", mutate_ruled_blank_fail_closed),
    ("baseline-split", "a span carrying two baselines is published as one run",
     mutate_baseline_split),
    ("codepoints", "an unmappable glyph prints as a section sign", mutate_codepoints),
    ("is-bar-like", "a leaning separator loses its rule", mutate_bar_like),
)


def mutation_probes(evidence: dict[str, Any], stream: Any) -> tuple[list[str], int]:
    """Confirm every check can fail, by breaking its subject and re-running it.

    A self-test that cannot fail is worthless, and every assertion above is of
    the form "the corpus says X" -- so the only way to know one is wired to the
    corpus is to change the corpus in memory and watch it trip. Each probe also
    requires that *only* its own check trips, which is what stops a broad check
    from standing in for a missing one.
    """
    checks = dict(SELF_TEST_CHECKS)
    failures: list[str] = []
    ran = 0
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
        ran += 1
    return failures, ran


def paint_span_contract_probes(stream: Any) -> tuple[list[str], int]:
    """Prove malformed contributor contracts are rejected, case by case."""
    start, end, first, last, spans = SELF_TEST_MERGED_INTERVALS[0]
    valid = Segment(
        "h", 20.0, 20.48, start, end, 0.0, None,
        first, last, spans,
    ).to_ir(0)
    failures: list[str] = []
    unexpected = validate_rule_paint_spans(valid, "valid synthetic rule")
    if unexpected:
        failures.append(f"valid paint_spans contract was rejected: {unexpected}")

    def reverse_spans(rule: dict[str, Any]) -> None:
        rule["paint_spans"].reverse()

    cases: tuple[
        tuple[str, str, Callable[[dict[str, Any]], Any]], ...
    ] = (
        ("missing-field", "has no paint_spans",
         lambda rule: rule.pop("paint_spans")),
        ("wrong-container", "expected list",
         lambda rule: rule.__setitem__("paint_spans", {})),
        ("empty", "is empty",
         lambda rule: rule.__setitem__("paint_spans", [])),
        ("wrong-entry", "expected object",
         lambda rule: rule["paint_spans"].__setitem__(0, [])),
        ("missing-key", "keys differ",
         lambda rule: rule["paint_spans"][0].pop("end_pt")),
        ("extra-key", "keys differ",
         lambda rule: rule["paint_spans"][0].__setitem__("x0", 0.0)),
        ("wrong-coordinate", "not a finite number",
         lambda rule: rule["paint_spans"][0].__setitem__("start_pt", "0")),
        ("bool-coordinate", "not a finite number",
         lambda rule: rule["paint_spans"][0].__setitem__("start_pt", True)),
        ("nan-coordinate", "not a finite number",
         lambda rule: rule["paint_spans"][0].__setitem__("start_pt", math.nan)),
        ("infinite-coordinate", "not a finite number",
         lambda rule: rule["paint_spans"][0].__setitem__("start_pt", math.inf)),
        ("unquantised-coordinate", "not q-coordinate",
         lambda rule: rule["paint_spans"][3].__setitem__("start_pt", 5.001)),
        ("reversed-extent", "non-positive extent",
         lambda rule: rule["paint_spans"][0].__setitem__("end_pt", -1.0)),
        ("wrong-ordinal", "not a non-negative integer",
         lambda rule: rule["paint_spans"][0].__setitem__("paint_seq", 7.0)),
        ("bool-ordinal", "not a non-negative integer",
         lambda rule: rule["paint_spans"][0].__setitem__("paint_seq", True)),
        ("negative-ordinal", "not a non-negative integer",
         lambda rule: rule["paint_spans"][0].__setitem__("paint_seq", -1)),
        ("unsorted", "not ordered", reverse_spans),
        ("disconnected-cluster", "form 2 clusters",
         lambda rule: rule["paint_spans"][-1].update(
             start_pt=20.0, end_pt=21.0)),
        ("wrong-union", "do not equal contributor union",
         lambda rule: rule.__setitem__("x1", 11.99)),
        ("off-grid-parent-x0", "x0 is not q-coordinate",
         lambda rule: rule.__setitem__("x0", 0.004)),
        ("off-grid-parent-x1", "x1 is not q-coordinate",
         lambda rule: rule.__setitem__("x1", 12.004)),
        ("off-grid-parent-y0", "y0 is not q-coordinate",
         lambda rule: rule.update(
             axis="v", x0=20.0, x1=20.48, y0=0.004, y1=12.0)),
        ("off-grid-parent-y1", "y1 is not q-coordinate",
         lambda rule: rule.update(
             axis="v", x0=20.0, x1=20.48, y0=0.0, y1=12.004)),
        ("wrong-min", "contributor min",
         lambda rule: rule.__setitem__("paint_seq", 4)),
        ("wrong-max", "contributor max",
         lambda rule: rule.__setitem__("paint_seq_max", 98)),
    )
    ran = 0
    for name, expected, mutate in cases:
        broken = copy.deepcopy(valid)
        mutate(broken)
        found = validate_rule_paint_spans(broken, name)
        rejected = any(expected in message for message in found)
        if not rejected:
            failures.append(f"paint_spans probe {name!r} was not rejected as "
                            f"{expected!r}: {found}")
        print(f"  probe paint-spans:{name:<19} "
              f"{'OK' if rejected else 'WEAK'}", file=stream)
        ran += 1
    return failures, ran


def self_test(profile: SelfTestProfile, source_root: pathlib.Path) -> int:
    """Assert Round 1's properties against a pinned corpus, then prove they can fail.

    Absence is a failure, not a skip: a self-test that quietly passes because it
    could not find its sources is the same green tick this project has already
    been burned by. That is also why the synthetic corpus exists -- so that CI,
    which can never hold the official PDFs, runs every one of these checks
    instead of reporting that it could not.
    """
    missing = [f"{code}: {source_root / relative}"
               for code, (relative, _, _) in profile.fixtures.items()
               if not (source_root / relative).is_file()]
    if missing:
        print(f"self-test cannot run -- {len(missing)} {profile.name} source "
              f"PDF(s) absent under {source_root}:", file=sys.stderr)
        for entry in missing:
            print(f"  {entry}", file=sys.stderr)
        print("Pass --source-root if the pinned PDFs live elsewhere, or "
              "--fixtures to run the tracked synthetic corpus.", file=sys.stderr)
        return 2

    evidence = gather_evidence(profile, source_root)
    failures: list[str] = []
    for name, check in SELF_TEST_CHECKS:
        found = check(evidence)
        failures.extend(found)
        print(f"  {name:<27} {'PASS' if not found else f'{len(found)} FAILURE(S)'}",
              file=sys.stderr)
        for message in found:
            print(f"    FAIL {message}", file=sys.stderr)

    weak, mutations_ran = mutation_probes(evidence, sys.stderr)
    failures.extend(weak)
    for message in weak:
        print(f"    FAIL {message}", file=sys.stderr)

    contract_failures, contracts_ran = paint_span_contract_probes(sys.stderr)
    failures.extend(contract_failures)
    for message in contract_failures:
        print(f"    FAIL {message}", file=sys.stderr)

    # How many probes ran, asserted rather than assumed. Deleting an assertion
    # leaves a self-test that still prints PASS, because every module here
    # reports a count of *failures* and zero failures is indistinguishable from
    # zero checks. A CI prove-phase fault did exactly that and went undetected.
    expected_mutations = len(SELF_TEST_MUTATIONS)
    expected_contracts = len(SELF_TEST_CHECKS)
    if mutations_ran != expected_mutations:
        failures.append(f"{mutations_ran} mutation probes ran, {expected_mutations} "
                        f"declared -- a probe was removed or skipped")
    if contracts_ran < expected_contracts:
        failures.append(f"only {contracts_ran} paint-span contract probes ran")

    # The corpus is named in the result line, and a synthetic pass says outright
    # that it is not evidence about the official forms. A reader who sees only
    # "self-test: PASS" must not be able to mistake one for the other.
    caveat = "" if profile.is_evidence else (
        " -- synthetic corpus, weaker than the official pins: it proves every "
        "check runs and can fail, not that the real forms extract correctly")
    verdict = "PASS" if not failures else f"{len(failures)} FAILURE(S)"
    print(f"self-test: {verdict} over {len(profile.fixtures)} pinned PDFs, "
          f"{len(SELF_TEST_CHECKS)} checks, {mutations_ran}+{contracts_ran} probes "
          f"({profile.name}){caveat}", file=sys.stderr)
    return 1 if failures else 0


def select_profile(use_fixtures: bool,
                   source_root: pathlib.Path | None) -> tuple[SelfTestProfile,
                                                              pathlib.Path]:
    """Decide which corpus --self-test measures, and where it lives.

    --fixtures is explicit. A --source-root pointing at the tracked fixture
    directory selects the same profile, because the alternative -- looking for
    2551Q under tools/formgen/fixtures and failing -- would be an obviously
    useless reading of that argument. Everything else means the official pins,
    including the no-argument case gate.py uses.
    """
    if use_fixtures:
        return FIXTURE_PROFILE, source_root or FIXTURE_PROFILE.source_root
    if source_root is not None:
        if source_root.resolve() == FIXTURE_PROFILE.source_root:
            return FIXTURE_PROFILE, source_root
        return REAL_PROFILE, source_root
    return REAL_PROFILE, REAL_PROFILE.source_root


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
    parser.add_argument("--fixtures", action="store_true",
                        help="Run --self-test over the tracked synthetic corpus "
                             "instead of the official PDFs. Same checks, same "
                             "mutation probes, weaker subjects -- this is what "
                             "CI runs, because the officials cannot be tracked.")
    # No default, so select_profile can tell "not given" from "given, and it
    # happens to be the fixture directory".
    parser.add_argument("--source-root", type=pathlib.Path, default=None,
                        help="Where --self-test looks for the pinned PDFs "
                             f"(default: {SELF_TEST_SOURCE_ROOT}).")
    args = parser.parse_args(argv)

    if args.self_test:
        return self_test(*select_profile(args.fixtures, args.source_root))

    absent = [name for name, value in (("--pdf", args.pdf),
                                       ("--form-code", args.form_code),
                                       ("--revision", args.revision)) if value is None]
    if absent:
        return print(f"missing required argument(s): {', '.join(absent)}",
                     file=sys.stderr) or 2
    if not args.pdf.is_file():
        return print(f"no such PDF: {args.pdf}", file=sys.stderr) or 2

    # The CLI is the shipping side: batch.py produces build/ir/ and the staged
    # guide IRs through it, and nothing measures a candidate through it. So the
    # masked-image pixel digest pins the shipped asset's decoded samples here,
    # while library callers keep the embedded-samples measurement default.
    ir = extract(args.pdf, args.form_code, args.revision, args.expected_sha256,
                 shipped_pixels=True)
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
