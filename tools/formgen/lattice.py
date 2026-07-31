#!/usr/bin/env python3
"""Turn the flat rule list in the IR into the box model the form is drawn on.

The BIR generator does not draw tables. It draws several hundred independent
filled bars, and every container the eye sees -- the outer box, a column, a
row, a character cell -- is an emergent property of where those bars happen to
line up. This module recovers the containers, because the containers are what
text gets laid out inside.

Three findings from the real 2551Q drive the whole design:

1. Tone, not thickness, says whether a bar is a border. `role == "decorative"`
   bars (gray 0.65-0.85) are grey ornament; painting them black is the exact
   mistake this project has made before, so they are carried on the page in
   their own list and never enter the lattice.

2. A vertical bar is a *comb divider* -- one tick between two character slots
   of a single field -- iff its bottom edge lands inside a structural
   horizontal that spans it while its top edge lands inside nothing. Combs
   hang from nothing; real column borders are supported at both ends. On the
   real 2551Q this splits 377/379 of page 1's 0.24pt verticals and 195/197 of
   page 2's as combs, and the handful it leaves as borders are exactly the
   "For BIR Use Only" panel dividers -- which are 0.24pt but genuinely
   structural. Thickness fails in both directions: money digit-group
   separators are combs at 0.96pt and 1.44pt.

3. Where a column border crosses a comb band it is *drawn thinner inside the
   band*, so the border arrives as three or four collinear fragments with the
   middle one classified as a comb. Span coverage is therefore tested against
   the union of all collinear structural ink at a lattice position, not
   against the borders alone. Without this the Tax Due column boundary at
   x=575.98 vanishes from all six Schedule 1 rows.

Comb fields are emitted as ONE cell carrying `comb: {cells: N, ...}`, never as
N cells: a 12-digit money comb is one field with twelve slots, not twelve
containers. Slot boundaries are carried through as measured x values because
the 14.16pt slot pitch is not uniform -- the content stream carries 14.04,
14.18, 14.28 among the 14.16s, and index*pitch would drift off the paper.

Finding 2 answers a different question from "where does a slot end", and
conflating the two cost 1886 slots. A comb *divider* is discovered by hanging
from nothing; a slot *boundary* is any black column crossing the band, whatever
the IR filed it as -- see `comb_boundary_candidates` and `endpoint_band`. 471 combs
reported fewer slots than the source prints, among them every TIN on the corpus
(1707 p2c5 read 11 slots for 14 boxes, three of them double width), because
their digit-group separators are drawn heavier than a character tick and land in
some other bucket. A typed character then centres on the black bar.

A fourth finding comes from the corpus rather than from 2551Q: a boundary is
not always one bar, and the inside of a boundary is never a cell. 119 places
draw a boundary as a *stack* -- a 0.14pt hairline on a 0.96pt bar (0605
y=232.4), a double rule of two 1.44pt bars around a 1.2pt white core (0619E
y=150.1, that core sometimes explicitly painted by a `knockout` rule), a
double hairline 0.65pt apart (1600WP x=357.0), a double 0.72pt box edge (2551M
x=284.2). Others draw one rule that *jogs*: the left page frame of 1606 steps
from x=26.64 to x=27.00 half way down, 1701 CONSO steps its right frame twice.
Centre clustering keeps the bars apart in every one of those, so the walk
emitted 1100 sub-2pt cells between them, 1092 of them classified `field` --
which is how a 0.36pt field input reached the page 36 times on one sheet.

Ink settles both. `fuse_boundaries` merges two clusters into one lattice line
when the paper between them, measured where the two actually run together, is
thinner than the bars drawing them. `encloses_paper` then refuses any cell
whose bounding lines leave no paper between their ink at all, which is what a
jog leaves. Neither is allowed to move a comb or a growable band; both counts
are unchanged across the corpus.

Usage:
    python3 tools/formgen/lattice.py --ir build/ir/2551q-2018.ir.json \\
        --out build/layout/2551q-2018.layout.json --summary
    python3 tools/formgen/lattice.py --self-test --ir build/ir/2551q-2018.ir.json
"""

from __future__ import annotations

import argparse
import collections
import json
import math
import pathlib
import sys
from typing import Any, Iterable, Sequence

SCHEMA_VERSION = 1

# Coordinates are already quantised to 2dp by extract.py; keep that.
QUANT = 2

# Collinear bars of one logical line vary by up to a rule width. 0.24-1.44pt
# bars are drawn centred on nominal positions, so 0.3pt clusters them without
# ever merging two genuinely distinct lines (the tightest real pair on the
# 2551Q is 0.48pt apart).
CLUSTER_TOL_PT = 0.3

# Two collinear fragments of one border count as continuous within this gap.
JOIN_EPSILON_PT = 0.05

# Row pitch tolerance for growable detection. The Schedule 1 band is 18.24pt
# for rows 1-5 and 18.27pt for row 6 -- real drift, not rounding.
PITCH_TOL_PT = 0.3

MIN_GROWABLE_ROWS = 3

# A run of rows with only an outer box is a page margin, not a table.
MIN_GROWABLE_COLUMNS = 3


def q(value: float) -> float:
    return round(float(value) + 0.0, QUANT)


Interval = tuple[float, float]
Point = tuple[float, float]

# One bar reduced to what the paper-versus-ink test needs: near edge, far edge,
# and the weight it is drawn at.
InkSpan = tuple[float, float, float]


def union_intervals(intervals: Iterable[Interval]) -> list[Interval]:
    """Union 1-D intervals, joining anything within JOIN_EPSILON_PT."""
    items = sorted(intervals)
    if not items:
        return []
    merged: list[list[float]] = [list(items[0])]
    for start, end in items[1:]:
        if start <= merged[-1][1] + JOIN_EPSILON_PT:
            merged[-1][1] = max(merged[-1][1], end)
        else:
            merged.append([start, end])
    return [(a, b) for a, b in merged]


def intersect_intervals(left: Sequence[Interval],
                        right: Sequence[Interval]) -> list[Interval]:
    """Positive-width intersections of two ordered interval unions."""
    intersections: list[Interval] = []
    left_index = right_index = 0
    while left_index < len(left) and right_index < len(right):
        left_start, left_end = left[left_index]
        right_start, right_end = right[right_index]
        start, end = max(left_start, right_start), min(left_end, right_end)
        if end > start:
            intersections.append((start, end))
        if left_end < right_end:
            left_index += 1
        else:
            right_index += 1
    return union_intervals(intersections)


def covers(spans: Sequence[Interval], lo: float, hi: float) -> bool:
    return any(a <= lo + CLUSTER_TOL_PT and b >= hi - CLUSTER_TOL_PT for a, b in spans)


# ---------------------------------------------------------------------------
# Rule triage
# ---------------------------------------------------------------------------


def centre(rule: dict[str, Any]) -> float:
    """Centre of a rule across its thin axis."""
    if rule["axis"] == "h":
        return (rule["y0"] + rule["y1"]) / 2.0
    return (rule["x0"] + rule["x1"]) / 2.0


def tone_role(gray: float | None) -> str:
    """Apply extract.py's exact tone bands to a path paint layer."""
    if gray is None:
        return "chromatic"
    if gray <= 0.15:
        return "structural"
    if gray >= 0.98:
        return "knockout"
    return "decorative"


def supported_at(y: float, x: float, horizontals: Sequence[dict[str, Any]]) -> bool:
    """True when point (x, y) lies inside the ink of a horizontal spanning x."""
    return any(h["x0"] - CLUSTER_TOL_PT <= x <= h["x1"] + CLUSTER_TOL_PT
               and h["y0"] <= y <= h["y1"]
               for h in horizontals)


def split_verticals(verticals: Sequence[dict[str, Any]],
                    horizontals: Sequence[dict[str, Any]]
                    ) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Partition structural verticals into (comb dividers, box borders).

    The discriminator is geometric support, never thickness. A comb divider
    hangs from nothing and lands on its row's baseline rule; anything else --
    supported at both ends, or a fragment of a border interrupted by a comb
    band -- is a border.
    """
    combs: list[dict[str, Any]] = []
    borders: list[dict[str, Any]] = []
    for rule in verticals:
        x = centre(rule)
        top = supported_at(rule["y0"], x, horizontals)
        bottom = supported_at(rule["y1"], x, horizontals)
        (combs if bottom and not top else borders).append(rule)
    return combs, borders


def comb_boundary_candidates(verticals: Sequence[dict[str, Any]],
                             area_fills: Sequence[dict[str, Any]]
                             ) -> list[dict[str, Any]]:
    """Every black column on the page, as a candidate comb slot boundary.

    Three sources, because the IR has already split this ink three ways on
    distinctions a comb knows nothing about. Measured over the corpus, the
    boundaries recovered break down as:

      * 6051 verticals `split_verticals` called comb dividers, but which a
        *different* band claimed. 2551Q page 2 draws its TIN group separators
        1.44pt lower than its character ticks (y1 126.86 against 125.42), so
        grouping dividers on their exact y extent files the three separators as a
        band of their own and the TIN reports 11 slots for 14 printed boxes;
      * 256 verticals it called borders, correctly: 48 of them are supported at
        both ends because they run the full row height (2200C x=59.76, a 0.48pt
        bar spanning y 115.22-132.14 across a comb band of 126.50-132.14), and
        208 are supported at neither, which is what 1701MS's traced geometry
        does to a 1.5pt separator (x=370.43, y 160.72-165.82);
      * 274 `area_fills`, the filled rects extract.py judged too thick to be a
        rule at all -- its cut is 1.5pt and 1707's TIN separators are 2.16pt
        wide, so they never reach the rule list.

    Horizontals are not candidates: a band is bounded top and bottom by
    horizontal rules, and no horizontal in the corpus is thick enough to reach
    from one to the other.

    Sorted so that de-duplication inside a band is order-independent.
    """
    candidates = list(verticals)
    candidates += [{
        "axis": "v",
        "x0": f["x0"], "y0": f["y0"], "x1": f["x1"], "y1": f["y1"],
        "thickness_pt": q(f["x1"] - f["x0"]),
        "gray": f["gray"],
        "role": f["role"],
        # Final-paint visibility is part of comb ownership. Keep the source
        # ordinal on a thick separator instead of turning it into timeless ink.
        "paint_seq": f.get("paint_seq", -1),
        "paint_seq_max": f.get("paint_seq_max", f.get("paint_seq", -1)),
    } for f in area_fills if f["role"] == "structural"]
    candidates.sort(key=lambda r: (centre(r), r["y0"], r["y1"]))
    return candidates


def paint_ordinal(paint: dict[str, Any]) -> int:
    """Last source operation represented by one extracted paint rectangle."""
    return int(paint.get("paint_seq_max", paint.get("paint_seq", -1)))


def paint_ordinal_range(paint: dict[str, Any]) -> tuple[int, int]:
    """Inclusive source-order bounds represented by one extracted paint.

    ``extract.merge_intervals`` may merge collinear fragments painted at
    different points in the content stream.  The merged rectangle then carries
    only its first and last source ordinals; assigning the whole rectangle to
    the last ordinal can revive an earlier fragment through an intervening
    knockout.  Preserve that uncertainty here so the compositor can certify a
    role only when every potentially topmost layer has the same role.
    """
    first = int(paint.get("paint_seq", -1))
    last = int(paint.get("paint_seq_max", first))
    return min(first, last), max(first, last)


def point_segment_distance(point: Point, start: Point, end: Point) -> float:
    """Euclidean distance from a point to one finite line segment."""
    px, py = point
    x0, y0 = start
    x1, y1 = end
    dx, dy = x1 - x0, y1 - y0
    length_sq = dx * dx + dy * dy
    if length_sq == 0:
        return math.hypot(px - x0, py - y0)
    along = max(0.0, min(1.0, ((px - x0) * dx + (py - y0) * dy) / length_sq))
    return math.hypot(px - (x0 + along * dx), py - (y0 + along * dy))


def flatten_cubic(start: Point, first: Point, second: Point, end: Point,
                  depth: int = 0) -> list[Point]:
    """Flatten one cubic at the existing source-coordinate join precision."""
    if (depth >= 16
            or max(point_segment_distance(first, start, end),
                   point_segment_distance(second, start, end))
            <= JOIN_EPSILON_PT):
        return [end]

    p01 = ((start[0] + first[0]) / 2.0, (start[1] + first[1]) / 2.0)
    p12 = ((first[0] + second[0]) / 2.0, (first[1] + second[1]) / 2.0)
    p23 = ((second[0] + end[0]) / 2.0, (second[1] + end[1]) / 2.0)
    p012 = ((p01[0] + p12[0]) / 2.0, (p01[1] + p12[1]) / 2.0)
    p123 = ((p12[0] + p23[0]) / 2.0, (p12[1] + p23[1]) / 2.0)
    middle = ((p012[0] + p123[0]) / 2.0, (p012[1] + p123[1]) / 2.0)
    return [
        *flatten_cubic(start, p01, p012, middle, depth + 1),
        *flatten_cubic(middle, p123, p23, end, depth + 1),
    ]


def flattened_subpaths(path: dict[str, Any]) -> list[tuple[list[Point], bool]]:
    """Reconstruct the actual line/cubic outline carried by one IR path."""
    flattened: list[tuple[list[Point], bool]] = []
    for subpath in path.get("subpaths") or ():
        start_raw = subpath.get("start") or ()
        if len(start_raw) != 2:
            continue
        start = (float(start_raw[0]), float(start_raw[1]))
        points = [start]
        cursor = start
        for operation in subpath.get("ops") or ():
            values = [float(value) for value in operation.get("points") or ()]
            if operation.get("op") == "l" and len(values) == 2:
                cursor = (values[0], values[1])
                points.append(cursor)
            elif operation.get("op") == "c" and len(values) == 6:
                first = (values[0], values[1])
                second = (values[2], values[3])
                end = (values[4], values[5])
                points.extend(flatten_cubic(cursor, first, second, end))
                cursor = end
            elif operation.get("op") == "re" and len(values) == 4:
                x0, y0, x1, y1 = values
                points = [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]
                cursor = points[-1]
            else:
                # extract.py rejects unknown operations. If a newer IR reaches
                # this older compositor, retain the bbox as uncertainty rather
                # than inventing path geometry.
                points = []
                break
        closed = bool(subpath.get("closed"))
        if points and closed and points[-1] != points[0]:
            points.append(points[0])
        if len(points) >= 2:
            flattened.append((points, closed))
    return flattened


def path_paint_layers(paint: dict[str, Any]) -> list[dict[str, Any]]:
    """Split a non-rectilinear fill+stroke into its true paint-order layers."""
    if "subpaths" not in paint:
        return [paint]
    flattened = flattened_subpaths(paint)
    layers: list[dict[str, Any]] = []
    if paint.get("fill") is not None:
        layers.append({
            **paint,
            "_path_layer": "fill",
            "_flattened": flattened,
            "role": tone_role(paint.get("fill_gray")),
            "paint_seq": int(paint.get("paint_seq", -1)),
            "paint_seq_max": int(paint.get("paint_seq", -1)),
        })
    if paint.get("stroke") is not None and float(paint.get("stroke_width_pt") or 0) > 0:
        layers.append({
            **paint,
            "_path_layer": "stroke",
            "_flattened": flattened,
            "role": tone_role(paint.get("stroke_gray")),
            "paint_seq": int(paint.get("paint_seq_max", paint.get("paint_seq", -1))),
            "paint_seq_max": int(paint.get("paint_seq_max", paint.get("paint_seq", -1))),
        })
    return layers


def path_segments(paint: dict[str, Any]) -> Iterable[tuple[Point, Point]]:
    for points, _closed in paint.get("_flattened") or ():
        yield from zip(points, points[1:])


def paint_bounds(paint: dict[str, Any]) -> tuple[float, float, float, float]:
    """Paint bbox, including the outside half of a path stroke."""
    half = (float(paint.get("stroke_width_pt") or 0) / 2.0
            if paint.get("_path_layer") == "stroke" else 0.0)
    return (
        float(paint["x0"]) - half,
        float(paint["y0"]) - half,
        float(paint["x1"]) + half,
        float(paint["y1"]) + half,
    )


def point_in_path(paint: dict[str, Any], point: Point) -> bool:
    """PDF nonzero/even-odd fill containment over flattened source subpaths."""
    px, py = point
    winding = 0
    crossings = 0
    for points, _closed in paint.get("_flattened") or ():
        polygon = points if points[-1] == points[0] else [*points, points[0]]
        for start, end in zip(polygon, polygon[1:]):
            if point_segment_distance(point, start, end) <= JOIN_EPSILON_PT:
                return True
            x0, y0 = start
            x1, y1 = end
            if (y0 > py) == (y1 > py):
                continue
            x_cross = x0 + (py - y0) * (x1 - x0) / (y1 - y0)
            if x_cross <= px:
                continue
            crossings += 1
            winding += 1 if y1 > y0 else -1
    return bool(crossings % 2) if paint.get("even_odd") else winding != 0


def exact_rectangular_path_fill_covers(
        paint: dict[str, Any],
        x0: float, y0: float, x1: float, y1: float) -> bool:
    """Whether one path fill is exactly one axis-aligned covering rectangle.

    Point samples cannot prove coverage for an arbitrary polygon: an even-odd
    compound path can cover every sampled corner and centre while leaving a
    small hole elsewhere. Keep complex fills unresolved. A single rectangular
    subpath has no hidden interior topology, so bbox containment is then exact.
    """
    if paint.get("_path_layer") != "fill":
        return False
    flattened = list(paint.get("_flattened") or ())
    if len(flattened) != 1:
        return False
    points = list(flattened[0][0])
    if points and points[-1] == points[0]:
        points.pop()
    simplified: list[Point] = []
    for point in points:
        if not simplified or point != simplified[-1]:
            simplified.append(point)
    if len(simplified) != 4:
        return False
    xs = sorted({point[0] for point in simplified})
    ys = sorted({point[1] for point in simplified})
    if len(xs) != 2 or len(ys) != 2:
        return False
    corners = {
        (xs[0], ys[0]), (xs[1], ys[0]),
        (xs[1], ys[1]), (xs[0], ys[1]),
    }
    if set(simplified) != corners:
        return False
    polygon = [*simplified, simplified[0]]
    if any(a[0] != b[0] and a[1] != b[1]
           for a, b in zip(polygon, polygon[1:])):
        return False
    return xs[0] <= x0 and xs[1] >= x1 and ys[0] <= y0 and ys[1] >= y1


def paint_covers_point(paint: dict[str, Any], point: Point) -> bool:
    """Whether one exact paint layer covers a point in the candidate slab."""
    x, y = point
    x0, y0, x1, y1 = paint_bounds(paint)
    if not (x0 <= x <= x1 and y0 <= y <= y1):
        return False
    layer = paint.get("_path_layer")
    if layer == "fill":
        return point_in_path(paint, point)
    if layer == "stroke":
        half = float(paint.get("stroke_width_pt") or 0) / 2.0
        return any(point_segment_distance(point, start, end) <= half
                   for start, end in path_segments(paint))
    return True


def path_x_edges(paint: dict[str, Any], y: float) -> list[float]:
    """Actual path crossings of a horizontal probe, used to split x slabs."""
    edges: list[float] = []
    half = (float(paint.get("stroke_width_pt") or 0) / 2.0
            if paint.get("_path_layer") == "stroke" else 0.0)
    for start, end in path_segments(paint):
        x0, y0 = start
        x1, y1 = end
        if y0 == y1:
            if abs(y - y0) <= half + JOIN_EPSILON_PT:
                edges.extend((min(x0, x1) - half, max(x0, x1) + half))
            continue
        if not (min(y0, y1) - half <= y <= max(y0, y1) + half):
            continue
        probe_y = max(min(y, max(y0, y1)), min(y0, y1))
        x_cross = x0 + (probe_y - y0) * (x1 - x0) / (y1 - y0)
        edges.extend((x_cross - half, x_cross + half))
    return edges


def path_y_edges(paint: dict[str, Any], x: float) -> list[float]:
    """Actual path crossings of a vertical probe, used to split y slabs."""
    edges: list[float] = []
    half = (float(paint.get("stroke_width_pt") or 0) / 2.0
            if paint.get("_path_layer") == "stroke" else 0.0)
    for start, end in path_segments(paint):
        x0, y0 = start
        x1, y1 = end
        if x0 == x1:
            if abs(x - x0) <= half + JOIN_EPSILON_PT:
                edges.extend((min(y0, y1) - half, max(y0, y1) + half))
            continue
        if not (min(x0, x1) - half <= x <= max(x0, x1) + half):
            continue
        probe_x = max(min(x, max(x0, x1)), min(x0, x1))
        y_cross = y0 + (probe_x - x0) * (y1 - y0) / (x1 - x0)
        edges.extend((y_cross - half, y_cross + half))
    return edges


def segments_intersect(first: tuple[Point, Point],
                       second: tuple[Point, Point]) -> bool:
    """Closed segment intersection without a fitted geometry tolerance."""
    (a, b), (c, d) = first, second

    def orientation(p: Point, q_: Point, r: Point) -> float:
        return ((q_[0] - p[0]) * (r[1] - p[1])
                - (q_[1] - p[1]) * (r[0] - p[0]))

    def within(p: Point, q_: Point, r: Point) -> bool:
        return (min(p[0], r[0]) <= q_[0] <= max(p[0], r[0])
                and min(p[1], r[1]) <= q_[1] <= max(p[1], r[1]))

    ab_c, ab_d = orientation(a, b, c), orientation(a, b, d)
    cd_a, cd_b = orientation(c, d, a), orientation(c, d, b)
    if ((ab_c > 0) != (ab_d > 0)) and ((cd_a > 0) != (cd_b > 0)):
        return True
    return ((ab_c == 0 and within(a, c, b))
            or (ab_d == 0 and within(a, d, b))
            or (cd_a == 0 and within(c, a, d))
            or (cd_b == 0 and within(c, b, d)))


def path_paint_intersects_rect(paint: dict[str, Any],
                               x0: float, y0: float,
                               x1: float, y1: float) -> bool:
    """Actual subpath/fill/stroke intersection with one divider corridor."""
    bx0, by0, bx1, by1 = paint_bounds(paint)
    if bx1 < x0 or bx0 > x1 or by1 < y0 or by0 > y1:
        return False
    half = (float(paint.get("stroke_width_pt") or 0) / 2.0
            if paint.get("_path_layer") == "stroke" else 0.0)
    rx0, ry0, rx1, ry1 = x0 - half, y0 - half, x1 + half, y1 + half
    corners = [(rx0, ry0), (rx1, ry0), (rx1, ry1), (rx0, ry1)]
    edges = list(zip(corners, [*corners[1:], corners[0]]))
    for start, end in path_segments(paint):
        if (rx0 <= start[0] <= rx1 and ry0 <= start[1] <= ry1
                or rx0 <= end[0] <= rx1 and ry0 <= end[1] <= ry1):
            return True
        if any(segments_intersect((start, end), edge) for edge in edges):
            return True
    return (paint.get("_path_layer") == "fill"
            and any(point_in_path(paint, corner) for corner in corners))


class FinalPaint:
    """Query final visible structural ink without reviving an overpainted mark.

    The IR keeps rectilinear rules and area fills in separate geometry lists,
    but both carry their exact content-stream ordinal. A divider that was later
    covered by a white knockout is still present in ``rules``; treating that
    stale record as a comb anchor creates a field the final page does not draw.

    Visibility is measured over endpoint slabs, not at one midpoint. For every
    positive-height y slab we partition the candidate's width at every paint
    edge and retain the x intervals whose every potentially topmost layer is
    structural.  We then follow common x coverage through consecutive slabs.
    Merely having some ink at unrelated x positions above and below a knockout
    is not a continuous divider.  The resulting y spans are cached because the
    same candidate is considered by its seed band and by neighbouring endpoint
    bands.
    """

    __slots__ = ("paints", "path_paints", "_visible")

    def __init__(self, paints: Sequence[dict[str, Any]]) -> None:
        expanded = [layer for paint in paints for layer in path_paint_layers(paint)]
        self.paints = tuple(expanded)
        self.path_paints = tuple(paint for paint in expanded if "_path_layer" in paint)
        self._visible: dict[
            tuple[str, float, float, float, float], list[Interval]
        ] = {}

    def visible_intervals(self, ink: dict[str, Any]) -> list[Interval]:
        """Final-visible y spans of a vertical candidate."""
        return self.visible_spans(ink, "v")

    def visible_spans(self, ink: dict[str, Any], axis: str) -> list[Interval]:
        """Final-visible long-axis spans with one common thin-axis witness."""
        if axis not in ("h", "v"):
            raise ValueError(f"unsupported paint visibility axis: {axis}")
        x0 = float(ink["x0"])
        y0 = float(ink["y0"])
        x1 = float(ink["x1"])
        y1 = float(ink["y1"])
        key = (axis, x0, y0, x1, y1)
        cached = self._visible.get(key)
        if cached is not None:
            return cached

        relevant = [
            (index, paint) for index, paint in enumerate(self.paints)
            if paint_bounds(paint)[2] > x0 and paint_bounds(paint)[0] < x1
            and paint_bounds(paint)[3] > y0 and paint_bounds(paint)[1] < y1
        ]
        primary0, primary1 = ((y0, y1) if axis == "v" else (x0, x1))
        cross0, cross1 = ((x0, x1) if axis == "v" else (y0, y1))
        endpoints = {primary0, primary1}
        for _, paint in relevant:
            px0, py0, px1, py1 = paint_bounds(paint)
            paint_primary0, paint_primary1 = (
                (py0, py1) if axis == "v" else (px0, px1))
            endpoints.update((
                max(primary0, paint_primary0),
                min(primary1, paint_primary1),
            ))
            for points, _closed in paint.get("_flattened") or ():
                coordinate = 1 if axis == "v" else 0
                endpoints.update(
                    point[coordinate] for point in points
                    if primary0 < point[coordinate] < primary1)

        slab_visibility: list[tuple[float, float, list[Interval]]] = []
        ordered_primary = sorted(endpoints)
        for a, b in zip(ordered_primary, ordered_primary[1:]):
            if b <= a:
                continue
            primary_centre = (a + b) / 2.0
            active = [
                (index, paint) for index, paint in relevant
                if (
                    paint_bounds(paint)[1] <= primary_centre
                    <= paint_bounds(paint)[3]
                    if axis == "v"
                    else paint_bounds(paint)[0] <= primary_centre
                    <= paint_bounds(paint)[2]
                )
            ]
            cross_edges = {cross0, cross1}
            for _, paint in active:
                if "_path_layer" in paint:
                    path_edges = (
                        path_x_edges(paint, primary_centre)
                        if axis == "v"
                        else path_y_edges(paint, primary_centre)
                    )
                    cross_edges.update(
                        max(cross0, min(cross1, edge))
                        for edge in path_edges)
                    if paint.get("role") != "structural":
                        # A nonrect knockout/decorative path can sweep across
                        # the thin axis within one primary slab. Its midpoint
                        # section is not a whole-slab witness, so conservatively
                        # include its complete cross-axis bbox as a mask.
                        px0, py0, px1, py1 = paint_bounds(paint)
                        path_cross0, path_cross1 = (
                            (px0, px1) if axis == "v" else (py0, py1))
                        cross_edges.update((
                            max(cross0, path_cross0),
                            min(cross1, path_cross1),
                        ))
                else:
                    px0, py0, px1, py1 = paint_bounds(paint)
                    paint_cross0, paint_cross1 = (
                        (px0, px1) if axis == "v" else (py0, py1))
                    cross_edges.update((
                        max(cross0, paint_cross0),
                        min(cross1, paint_cross1),
                    ))

            visible_cross: list[Interval] = []
            ordered_cross = sorted(cross_edges)
            for left, right in zip(ordered_cross, ordered_cross[1:]):
                if right <= left:
                    continue
                cross_centre = (left + right) / 2.0
                point = (
                    (cross_centre, primary_centre)
                    if axis == "v"
                    else (primary_centre, cross_centre)
                )
                def covers_witness(paint: dict[str, Any]) -> bool:
                    if ("_path_layer" in paint
                            and paint.get("role") != "structural"):
                        px0, py0, px1, py1 = paint_bounds(paint)
                        return (
                            px0 <= point[0] <= px1
                            and py0 <= point[1] <= py1
                        )
                    return paint_covers_point(paint, point)

                covering = [
                    (paint_ordinal(paint), index, paint)
                    for index, paint in active
                    if covers_witness(paint)
                ]
                if not covering:
                    continue
                # A merged paint may represent fragments from a source-order
                # range.  A layer can still be topmost unless another layer's
                # earliest possible ordinal is later than its latest possible
                # ordinal.  Mixed roles among those candidates are ambiguous,
                # so fail closed instead of globally ordering the merge by its
                # final fragment.
                ranged = [
                    (*paint_ordinal_range(paint), index, paint)
                    for _ordinal, index, paint in covering
                ]
                latest_floor = max(first for first, _last, _index, _paint in ranged)
                potentially_topmost = [
                    paint for _first, last, _index, paint in ranged
                    if last >= latest_floor
                ]
                if (potentially_topmost
                        and all(paint.get("role") == "structural"
                                for paint in potentially_topmost)
                        and any("_path_layer" not in paint
                                for paint in potentially_topmost)):
                    visible_cross.append((left, right))
            slab_visibility.append((a, b, union_intervals(visible_cross)))

        # Track every distinct fixed-x witness through successive y slabs.
        # Starting a fresh track on every slab matters: a second x corridor can
        # appear while an older one remains, then become the only survivor.
        # Tracks with identical common coverage are equivalent; retaining the
        # earliest start bounds the state without losing a possible witness.
        tracks: list[tuple[float, float, list[Interval]]] = []
        completed: list[Interval] = []
        for a, b, visible_x in slab_visibility:
            next_tracks: list[tuple[float, float, list[Interval]]] = []
            for start, _end, common_x in tracks:
                overlap = intersect_intervals(common_x, visible_x)
                if overlap:
                    next_tracks.append((start, b, overlap))
                else:
                    completed.append((start, a))
            if visible_x:
                next_tracks.append((a, b, visible_x))

            deduplicated: dict[tuple[Interval, ...],
                               tuple[float, float, list[Interval]]] = {}
            for track in next_tracks:
                identity = tuple(track[2])
                prior = deduplicated.get(identity)
                if prior is None or track[0] < prior[0]:
                    deduplicated[identity] = track
            tracks = list(deduplicated.values())
        completed.extend((start, end) for start, end, _common_x in tracks)

        # Do not union adjacent spans: adjacency without a common x witness is
        # precisely the stale-divider false positive this compositor prevents.
        unique = sorted(set(completed))
        result = [
            span for span in unique
            if span[1] > span[0]
            and not any(
                other != span
                and other[0] <= span[0] + JOIN_EPSILON_PT
                and other[1] >= span[1] - JOIN_EPSILON_PT
                for other in unique
            )
        ]
        self._visible[key] = result
        return result

    def structural_across(self, ink: dict[str, Any], y0: float, y1: float) -> bool:
        """Whether final structural ink survives across the whole open band."""
        return any(a <= y0 + JOIN_EPSILON_PT and b >= y1 - JOIN_EPSILON_PT
                   for a, b in self.visible_intervals(ink))

    def structural_across_axis(self, ink: dict[str, Any],
                               lo: float, hi: float, axis: str) -> bool:
        """Whether final structural ink survives one full horizontal/vertical run."""
        return any(
            start <= lo + JOIN_EPSILON_PT
            and end >= hi - JOIN_EPSILON_PT
            for start, end in self.visible_spans(ink, axis)
        )

    def definitely_erased(self, ink: dict[str, Any]) -> bool:
        """Whether one known-later nonstructural layer covers the whole bbox.

        False visibility can also mean source-order or moving-path uncertainty.
        Such geometry may remain as an explicitly unresolved lattice hint, but
        a single later rectangular knockout/decorative layer proven to cover the
        complete candidate is absent and must not define a cell. For path fills,
        only one exact axis-aligned rectangle is a coverage proof; samples of an
        arbitrary compound path cannot rule out a hole.
        """
        x0, y0, x1, y1 = (
            float(ink["x0"]), float(ink["y0"]),
            float(ink["x1"]), float(ink["y1"]),
        )
        _ink_first, ink_last = paint_ordinal_range(ink)
        for paint in self.paints:
            if paint.get("role") == "structural":
                continue
            paint_first, _paint_last = paint_ordinal_range(paint)
            if paint_first <= ink_last:
                continue
            px0, py0, px1, py1 = paint_bounds(paint)
            if not (px0 <= x0 and px1 >= x1 and py0 <= y0 and py1 >= y1):
                continue
            if "_path_layer" not in paint:
                return True
            if exact_rectangular_path_fill_covers(
                    paint, x0, y0, x1, y1):
                return True
        return False


# ---------------------------------------------------------------------------
# Lattice
# ---------------------------------------------------------------------------


class Lattice:
    """One axis of the page grid: clustered line positions plus their ink."""

    __slots__ = ("positions", "ink_lo", "ink_hi", "spans", "members")

    def __init__(self, positions: list[float], ink_lo: list[float], ink_hi: list[float],
                 spans: list[list[Interval]], members: list[list[dict[str, Any]]]) -> None:
        self.positions = positions   # clustered centres, ascending
        self.ink_lo = ink_lo         # near edge of the cluster's ink
        self.ink_hi = ink_hi         # far edge of the cluster's ink
        self.spans = spans           # unioned along-line extent of ALL collinear ink
        self.members = members       # the border rules that defined the position

    def __len__(self) -> int:
        return len(self.positions)


def cluster_collinear(defining: Sequence[dict[str, Any]]) -> list[list[dict[str, Any]]]:
    """Chain rules whose centres lie within a rule width of each other."""
    groups: list[list[dict[str, Any]]] = []
    for rule in sorted(defining, key=centre):
        if groups and centre(rule) - centre(groups[-1][-1]) <= CLUSTER_TOL_PT:
            groups[-1].append(rule)
        else:
            groups.append([rule])
    return groups


def total_length(spans: Sequence[Interval]) -> float:
    return sum(b - a for a, b in spans)


def overlap_length(left: Sequence[Interval], right: Sequence[Interval]) -> float:
    """Length of paper where two along-axis extents coincide."""
    total = 0.0
    for a0, a1 in left:
        for b0, b1 in right:
            total += max(0.0, min(a1, b1) - max(a0, b0))
    return total


# A composite boundary's bars are drawn over the same run, so each must shadow
# this much of the other's length before they may fuse.
MIN_BOUNDARY_OVERLAP = 0.5


class Bar:
    """One drawn rule reduced to the four numbers the fusion test needs."""

    __slots__ = ("start", "end", "near", "far", "thickness")

    def __init__(self, rule: dict[str, Any], axis: str) -> None:
        near, far = ("y0", "y1") if axis == "h" else ("x0", "x1")
        along = ("x0", "x1") if axis == "h" else ("y0", "y1")
        self.start, self.end = rule[along[0]], rule[along[1]]
        self.near, self.far = rule[near], rule[far]
        self.thickness = rule["thickness_pt"]


class GroupGeometry:
    """One centre-cluster measured on both axes, before boundary fusion."""

    __slots__ = ("rules", "bars", "position", "ink_lo", "ink_hi", "span")

    def __init__(self, rules: list[dict[str, Any]], all_ink: Sequence[dict[str, Any]],
                 axis: str) -> None:
        near, far = ("y0", "y1") if axis == "h" else ("x0", "x1")
        along = ("x0", "x1") if axis == "h" else ("y0", "y1")
        self.rules = rules
        self.bars = [Bar(r, axis) for r in rules]
        self.position = q(sum(centre(r) for r in rules) / len(rules))
        self.ink_lo = q(min(r[near] for r in rules))
        self.ink_hi = q(max(r[far] for r in rules))
        # Coverage is measured against this cluster's own centre, so fusing two
        # clusters later cannot move the span of either one.
        self.span = union_intervals((r[along[0]], r[along[1]]) for r in all_ink
                                    if abs(centre(r) - self.position) <= CLUSTER_TOL_PT)


def bars_over(bars: Sequence[Bar], where: Sequence[Interval]) -> list[Bar]:
    """The bars of a cluster that run where `where` runs."""
    return [b for b in bars if overlap_length([(b.start, b.end)], where) > 0]


def is_one_boundary(lower: GroupGeometry, upper: GroupGeometry) -> bool:
    """True when two collinear clusters are two bars of ONE drawn boundary.

    The comparison is local. A cluster gathers every collinear fragment on the
    page, so its ink extent and its length belong to no single place: the
    0.24pt hairline at 1600WP x=357.00 shares a cluster with three 0.76pt bars
    500pt further down the sheet. Each cluster is therefore cut down to the
    bars that actually run where the other one runs, and the test is applied to
    those.

    The test itself is ink against paper, never distance. A boundary drawn as a
    stack of bars -- a hairline lying on a bar, or the two rules of a double
    rule -- leaves either no paper at all between the bars or a white core
    thinner than the bars around it, and reads as one heavier line. A real pair
    of boundaries encloses more paper than its own ink: the narrowest genuine
    cell in the corpus is the 4.8pt dash gap between two TIN comb groups
    (2550M x=99.84), 4.08pt of paper inside two 0.72pt edges.

    Length decides the rest, because bars of one boundary are drawn over the
    same run. Where the bars physically overlap there is no paper between them
    wherever they coincide, so it is enough that the shorter one is shadowed --
    that is a 5pt corner tick sitting on a full-width rule (2553 y=39.4). Where
    paper separates them, both have to match, which stops a row of four 14pt
    field underlines being swallowed by the Part header bar 2.2pt below them
    (1601C y=124.4).

    Fragments of one rule that merely *follow* each other down the page never
    fuse here, however far their ink overlaps, because moving a page frame to
    the average of its own jogs drags every cell edge on that side with it.
    `encloses_paper` deals with those where they do damage.
    """
    here = bars_over(lower.bars, upper.span)
    there = bars_over(upper.bars, lower.span)
    if not here or not there:
        return False

    paper = min(b.near for b in there) - max(b.far for b in here)
    if paper >= max(b.thickness for b in here) + max(b.thickness for b in there):
        return False

    runs = (union_intervals((b.start, b.end) for b in here),
            union_intervals((b.start, b.end) for b in there))
    lengths = (total_length(runs[0]), total_length(runs[1]))
    if min(lengths) <= 0:
        return False
    shared = overlap_length(*runs)
    return shared >= MIN_BOUNDARY_OVERLAP * (min(lengths) if paper <= 0 else max(lengths))


def fuse_boundaries(groups: Sequence[GroupGeometry]) -> list[list[GroupGeometry]]:
    """Merge runs of clusters that together draw one boundary."""
    fused: list[list[GroupGeometry]] = []
    for group in groups:
        if fused and is_one_boundary(fused[-1][-1], group):
            fused[-1].append(group)
        else:
            fused.append([group])
    return fused


def build_lattice(defining: Sequence[dict[str, Any]], all_ink: Sequence[dict[str, Any]],
                  axis: str) -> Lattice:
    """Cluster `defining` rules into lattice lines, then measure coverage.

    Positions come only from `defining` (borders), so comb dividers never
    invent a column. Coverage comes from `all_ink` (borders + combs), because
    a border crossing a comb band is drawn thinner *inside* the band and would
    otherwise read as three disconnected fragments.

    Clustering happens twice, on two different questions. Centre clustering
    gathers the collinear fragments of one bar; boundary fusion then gathers
    the bars of one composite boundary, so that the paper inside a double rule
    never becomes a cell. A lattice line that survives fusion is byte-identical
    to what centre clustering alone produced.
    """
    if not defining:
        return Lattice([], [], [], [], [])

    groups = [GroupGeometry(g, all_ink, axis) for g in cluster_collinear(defining)]

    positions: list[float] = []
    ink_lo: list[float] = []
    ink_hi: list[float] = []
    spans: list[list[Interval]] = []
    members: list[list[dict[str, Any]]] = []
    for boundary in fuse_boundaries(groups):
        rules = [r for g in boundary for r in g.rules]
        positions.append(q(sum(centre(r) for r in rules) / len(rules)))
        ink_lo.append(min(g.ink_lo for g in boundary))
        ink_hi.append(max(g.ink_hi for g in boundary))
        spans.append(union_intervals(i for g in boundary for i in g.span))
        members.append(rules)
    return Lattice(positions, ink_lo, ink_hi, spans, members)


def line_thickness_gray(lattice: Lattice, index: int, all_ink: Sequence[dict[str, Any]],
                        lo: float, hi: float, axis: str) -> tuple[float, float, list[float]]:
    """Weight and tone of the ink on lattice line `index` over span lo..hi.

    Thickness is the maximum: where a border thins to 0.24 crossing a comb
    band its real weight is the 0.48 it carries everywhere else. Tone is the
    darkest, because a border is as visible as its darkest segment.

    The line's own defining rules count whatever their distance from the
    clustered centre. On a fused composite boundary the centre sits in the
    white core of the double rule, further from either bar than the clustering
    tolerance, and a distance-only scan would report the boundary as absent.
    """
    a0, a1 = ("x0", "x1") if axis == "h" else ("y0", "y1")
    position = lattice.positions[index]
    own = {id(r) for r in lattice.members[index]}
    hits = [r for r in all_ink
            if (abs(centre(r) - position) <= CLUSTER_TOL_PT or id(r) in own)
            and r[a1] > lo - CLUSTER_TOL_PT and r[a0] < hi + CLUSTER_TOL_PT]
    if not hits:
        hits = lattice.members[index]
    thicknesses = sorted({r["thickness_pt"] for r in hits})
    grays = [r["gray"] for r in hits if r["gray"] is not None]
    return (max(thicknesses), min(grays) if grays else 0.0, thicknesses)


# ---------------------------------------------------------------------------
# Cells
# ---------------------------------------------------------------------------


class DisjointSet:
    def __init__(self, size: int) -> None:
        self.parent = list(range(size))

    def find(self, a: int) -> int:
        while self.parent[a] != a:
            self.parent[a] = self.parent[self.parent[a]]
            a = self.parent[a]
        return a

    def union(self, a: int, b: int) -> None:
        ra, rb = self.find(a), self.find(b)
        if ra != rb:
            # Always keep the lower index as root so components are stable.
            self.parent[max(ra, rb)] = min(ra, rb)


def merge_grid(xl: Lattice, yl: Lattice) -> tuple[DisjointSet, list[list[bool]], list[list[bool]]]:
    """Fuse adjacent grid squares that no rule separates.

    A lattice line existing somewhere on the page says nothing about whether it
    bounds a given square, so every adjacency is decided by an explicit span
    coverage test.
    """
    nx, ny = len(xl) - 1, len(yl) - 1
    # v_at[i][j]: vertical lattice line i carries ink across grid row j.
    v_at = [[covers(xl.spans[i], yl.ink_hi[j], yl.ink_lo[j + 1]) for j in range(ny)]
            for i in range(len(xl))]
    h_at = [[covers(yl.spans[j], xl.ink_hi[i], xl.ink_lo[i + 1]) for i in range(nx)]
            for j in range(len(yl))]

    dsu = DisjointSet(max(nx * ny, 1))
    for j in range(ny):
        for i in range(nx):
            here = j * nx + i
            if i + 1 < nx and not v_at[i + 1][j]:
                dsu.union(here, here + 1)
            if j + 1 < ny and not h_at[j + 1][i]:
                dsu.union(here, here + nx)
    return dsu, v_at, h_at


def distinct_boundary(here: InkSpan, there: InkSpan) -> bool:
    """Whether paper survives between two bars, making two boundaries."""
    paper = max(here[0], there[0]) - min(here[1], there[1])
    return paper > here[2] + there[2] + JOIN_EPSILON_PT


def endpoint_band(seed: Sequence[dict[str, Any]],
                  extra: Sequence[dict[str, Any]],
                  x0: float, x1: float,
                  frame: Sequence[InkSpan],
                  final_paint: FinalPaint
                  ) -> tuple[list[dict[str, Any]], float, float,
                             list[dict[str, Any]]] | None:
    """Final-visible endpoint topology plus every competing topology.

    Heavy digit-group separators are often nested inside the thin character
    ticks rather than sharing both endpoints. On 2550Q, for example, the thin
    ticks run y=141.62..147.92 while each 2.20pt separator runs
    y=142.12..147.92. Requiring the heavy bar to contain the whole thin seed
    drops exactly three boundaries from every 12-slot money field.

    Partitioning at every endpoint exposes the common intersection directly:
    one 0.50pt slab has the eight thin ticks and the remaining 5.80pt slab has
    all eleven boundaries. Coverage is useful only to choose a deterministic
    representation; it cannot prove that one source topology owns the field.
    Every non-identical topology is therefore returned as evidence and makes
    the resulting comb unresolved. The returned y range is the longest
    continuous slab carrying the representative topology, so every reported
    divider really spans the reported band.

    A raw rule is not evidence by itself. Each slab is composited through
    ``FinalPaint`` first, which prevents a later white knockout from reviving a
    stale divider anchor.
    """
    if not seed:
        return None
    band_y0 = min(float(ink["y0"]) for ink in seed)
    band_y1 = max(float(ink["y1"]) for ink in seed)
    if band_y1 <= band_y0:
        return None

    # `extra` contains the vertical rule list as well as thick area fills, so
    # the seed objects occur twice. Geometry plus source ordinal is their stable
    # identity; no iteration-order accident may add the same boundary twice.
    pool: list[dict[str, Any]] = []
    seen: set[tuple[float, float, float, float, float, int, int]] = set()
    for ink in [*seed, *extra]:
        if ink["x0"] <= x0 + CLUSTER_TOL_PT or ink["x1"] >= x1 - CLUSTER_TOL_PT:
            continue
        if ink["y1"] <= band_y0 or ink["y0"] >= band_y1:
            continue
        first, last = paint_ordinal_range(ink)
        key = (float(ink["x0"]), float(ink["y0"]),
               float(ink["x1"]), float(ink["y1"]),
               float(ink["thickness_pt"]), first, last)
        if key in seen:
            continue
        seen.add(key)
        pool.append(ink)
    if not pool:
        return None

    seed_keys = set()
    for ink in seed:
        first, last = paint_ordinal_range(ink)
        seed_keys.add((
            float(ink["x0"]), float(ink["y0"]),
            float(ink["x1"]), float(ink["y1"]),
            float(ink["thickness_pt"]), first, last,
        ))
    endpoints = {band_y0, band_y1}
    for ink in pool:
        for a, b in final_paint.visible_intervals(ink):
            lo, hi = max(band_y0, a, float(ink["y0"])), min(
                band_y1, b, float(ink["y1"]))
            if hi > lo:
                endpoints.update((lo, hi))

    slab_records: list[tuple[float, float, tuple[float, ...],
                             list[dict[str, Any]]]] = []
    ordered = sorted(endpoints)
    for a, b in zip(ordered, ordered[1:]):
        if b <= a:
            continue
        active = [
            ink for ink in pool
            if ink["y0"] <= a + JOIN_EPSILON_PT
            and ink["y1"] >= b - JOIN_EPSILON_PT
            and final_paint.structural_across(ink, a, b)
        ]
        if not any((
                float(ink["x0"]), float(ink["y0"]),
                float(ink["x1"]), float(ink["y1"]),
                float(ink["thickness_pt"]), *paint_ordinal_range(ink),
        ) in seed_keys for ink in active):
            continue

        # Later paint wins when coincident records describe one boundary; a
        # thicker bar then wins a source-order tie. Distinct x boundaries remain
        # sorted left-to-right regardless of input list order.
        active.sort(key=lambda ink: (
            centre(ink), -paint_ordinal(ink), -float(ink["thickness_pt"]),
            float(ink["y0"]), float(ink["y1"])))
        taken = list(frame)
        topology_ink: list[dict[str, Any]] = []
        for ink in active:
            here = (float(ink["x0"]), float(ink["x1"]),
                    float(ink["thickness_pt"]))
            if not all(distinct_boundary(here, other) for other in taken):
                continue
            taken.append(here)
            topology_ink.append(ink)
        topology = tuple(q(centre(ink)) for ink in topology_ink)
        if topology:
            slab_records.append((a, b, topology, topology_ink))

    if not slab_records:
        return None

    coverage: dict[tuple[float, ...], float] = collections.defaultdict(float)
    for a, b, topology, _ in slab_records:
        coverage[topology] += b - a
    topology_runs: dict[tuple[float, ...], list[Interval]] = {}
    topology_evidence: list[dict[str, Any]] = []
    for topology in sorted(coverage):
        records = sorted(
            (a, b) for a, b, candidate, _inks in slab_records
            if candidate == topology)
        runs: list[Interval] = []
        for a, b in records:
            if not runs:
                runs.append((a, b))
                continue
            run_start, run_end = runs[-1]
            continuous = (
                a <= run_end + JOIN_EPSILON_PT
                and all(any(
                    q(centre(ink)) == divider_x
                    and float(ink["y0"]) <= run_start + JOIN_EPSILON_PT
                    and float(ink["y1"]) >= b - JOIN_EPSILON_PT
                    and final_paint.structural_across(ink, run_start, b)
                    for ink in pool
                ) for divider_x in topology)
            )
            if continuous:
                runs[-1] = (run_start, max(run_end, b))
            else:
                runs.append((a, b))
        topology_runs[topology] = runs
        hull_start, hull_end = records[0][0], records[-1][1]
        corridors_continuous = all(any(
            q(centre(ink)) == divider_x
            and float(ink["y0"]) <= hull_start + JOIN_EPSILON_PT
            and float(ink["y1"]) >= hull_end - JOIN_EPSILON_PT
            and final_paint.structural_across(ink, hull_start, hull_end)
            for ink in pool
        ) for divider_x in topology)
        topology_evidence.append({
            "divider_x": list(topology),
            "coverage_pt": q(coverage[topology]),
            "runs": [[q(a), q(b)] for a, b in runs],
            "corridors_continuous": corridors_continuous,
        })
    maximal = [
        topology for topology in coverage
        if not any(
            set(topology) < set(other)
            for other in coverage
        )
    ]
    # Representation is not adjudication: when one topology contains every
    # competing topology, carry that complete measured divider set while the
    # resolution remains explicitly unresolved. If alternatives are
    # incomparable, retain the old deterministic display choice only.
    chosen = (
        maximal[0] if len(maximal) == 1
        else min(coverage, key=lambda topology: (
            -coverage[topology], -len(topology), topology))
    )
    runs = topology_runs[chosen]
    chosen_y0, chosen_y1 = min(
        runs, key=lambda span: (-(span[1] - span[0]), span[0], span[1]))

    representatives = min(
        (inks for a, b, topology, inks in slab_records
         if topology == chosen
         and a < chosen_y1 and b > chosen_y0),
        key=lambda inks: tuple(
            (q(centre(ink)), -paint_ordinal(ink),
             -float(ink["thickness_pt"])) for ink in inks),
    )
    return representatives, chosen_y0, chosen_y1, topology_evidence


def band_ink(extra: Sequence[dict[str, Any]], x0: float, x1: float,
             band_y0: float, band_y1: float,
             claimed: Sequence[InkSpan]) -> list[dict[str, Any]]:
    """Legacy full-span query retained for focused callers and self-tests.

    A slot boundary is black ink drawn from the band's top edge to its bottom
    edge, inside the field. Nothing else about it matters, and in particular
    neither thickness nor end support does:

      * a digit-group separator that runs the whole row height is supported at
        both ends, so `split_verticals` correctly calls it a border -- 2200C
        x=59.76 is a 0.48pt bar spanning y 115.22-132.14 across the item-1 comb
        band at y 126.50-132.14, and it is one of that comb's slot boundaries;
      * above 1.5pt a bar is not a rule at all. 1707's TIN separators are
        2.16 x 6.96pt black rects, so extract.py files them as area fills and
        they never reach the vertical list. Fourteen printed TIN boxes became
        eleven slots and a typed character centred on top of the black bar.

    Thickness *ranks* a boundary -- 0.24 is a character tick, 0.96/1.44/2.16 a
    group separator -- and both ranks stay visible in
    `divider_thicknesses_pt`. It never decides whether the boundary exists.

    Containment, not the centre, is the x test here: an extra may be as wide as
    a slot, and a fill spanning the whole cell has its centre inside the cell
    while bounding nothing.

    Two pieces of ink are the same boundary when the paper between them is
    thinner than the ink drawing them -- `is_one_boundary`'s test, applied for
    the same reason one lattice line away. It settles both ends of the problem
    with no new constant:

      * 0605 x=226.0 draws one bar as a 0.14pt hairline overlapping a 0.96pt bar.
        Their centres are 0.42pt apart, further than any clustering tolerance,
        and they are still one line.
      * 1701A x=599.04 is the inner bar of the right page frame, 1.86pt of paper
        inside the 1.44pt bar the cell ends on. Counting it would add a 1.86pt
        slot no character fits in. A composite boundary is not a slot boundary
        twice.

    The narrowest genuine slot in the corpus survives this: the 4.08pt TIN dash
    gap at 2550M x=99.84 holds 4.08pt of paper inside two 0.72pt edges.
    """
    taken = list(claimed)
    found: list[dict[str, Any]] = []
    for ink in extra:
        if ink["x0"] <= x0 + CLUSTER_TOL_PT or ink["x1"] >= x1 - CLUSTER_TOL_PT:
            continue
        if ink["y0"] > band_y0 + CLUSTER_TOL_PT or ink["y1"] < band_y1 - CLUSTER_TOL_PT:
            continue
        here = (ink["x0"], ink["x1"], ink["thickness_pt"])
        if not all(distinct_boundary(here, other) for other in taken):
            continue
        taken.append(here)
        found.append(ink)
    return found


def comb_bands(members: Sequence[dict[str, Any]], extra: Sequence[dict[str, Any]],
               x0: float, x1: float,
               edge_thickness: tuple[float, float],
               final_paint: FinalPaint) -> list[dict[str, Any]]:
    """Group a cell's comb dividers into bands, one band per field.

    Dividers of one comb share a y extent exactly (they are drawn by the same
    loop), so grouping on the band extent is safe and needs no pitch assumption.
    The band a comb divider discovers is then filled in from `extra`, the black
    ink that spans a common final-visible endpoint slab without being a comb
    divider -- see `endpoint_band`.
    Where a field's ticks are not all drawn to the same length the field arrives
    as two overlapping bands, and the one carrying the complete set of
    boundaries is the shorter one -- the only band every boundary spans. The
    reported band is therefore the writing box the *shortest* tick measures,
    which on 449 cells is 0.48pt less than before. That is the price of the slot
    count being right, and it keeps `y0`/`y1` honest: every boundary listed in
    `slot_x` really does run the full height reported here.

    A divider landing on this cell's own left or right edge is not a slot
    divider: it is the thinned middle fragment of the column border crossing
    the comb band (page 2, x = 320.69 and 575.98), which the comb split has to
    classify as a comb because it really does hang from nothing. The test is
    deliberately local -- x = 221.57 is a real slot divider in the Schedule 1
    money comb even though an unrelated panel elsewhere on page 2 puts a
    lattice line at that same x.
    """
    inside = [d for d in members if x0 + CLUSTER_TOL_PT < centre(d) < x1 - CLUSTER_TOL_PT]
    if not inside:
        return []

    by_band: dict[tuple[float, float], list[dict[str, Any]]] = collections.defaultdict(list)
    for d in inside:
        by_band[(d["y0"], d["y1"])].append(d)

    # The cell's own edges are the outermost boundaries, so an extra has to be a
    # separate boundary from those too -- see `band_ink`.
    left, right = edge_thickness
    frame = [(x0 - left / 2.0, x0 + left / 2.0, left),
             (x1 - right / 2.0, x1 + right / 2.0, right)]

    bands: list[dict[str, Any]] = []
    for (_band_y0, _band_y1), seed in sorted(by_band.items()):
        selected = endpoint_band(seed, extra, x0, x1, frame, final_paint)
        if selected is None:
            continue
        band, band_y0, band_y1, topology_evidence = selected
        xs = sorted(q(centre(d)) for d in band)
        boundaries = [q(x0), *xs, q(x1)]
        deltas = [q(b - a) for a, b in zip(boundaries, boundaries[1:])]
        # A single divider cannot prove two character compartments when one
        # side can hold at least two copies of the other. Likewise, a large
        # *interior* gap splits two anchor runs rather than joining them into
        # one comb. These are topology warnings, not grounds for retiring an
        # existing subject: the source referee may be unable to distinguish a
        # deliberately unequal field from unrelated geometry. The comparison
        # is exactly two measured paper widths and is carried as unresolved
        # evidence for a gate/referee to adjudicate independently.
        #
        # This rejects page-header inner frames and side-by-side fields that a
        # broad cell had merged (address + ZIP is the recurring case), while an
        # edge label may still precede a run of at least three measured slots.
        smallest = min(deltas)
        incoherent_pair = len(deltas) == 2 and max(deltas) >= 2 * smallest
        split_anchor_runs = any(delta >= 2 * smallest for delta in deltas[1:-1])
        reason_codes: list[str] = []
        if incoherent_pair:
            reason_codes.append("unequal-two-slot-topology")
        if split_anchor_runs:
            reason_codes.append("split-anchor-run-topology")
        if len(topology_evidence) > 1:
            reason_codes.append("competing-endpoint-topologies")
        if any(len(evidence["runs"]) > 1
               and not evidence["corridors_continuous"]
               for evidence in topology_evidence):
            reason_codes.append("disconnected-final-visible-corridor")
        ordered_band = sorted(band, key=lambda ink: (
            q(centre(ink)), -paint_ordinal(ink),
            -float(ink["thickness_pt"])))
        thicknesses = collections.Counter(d["thickness_pt"] for d in ordered_band)
        grays = sorted({d["gray"] for d in ordered_band if d["gray"] is not None})
        bands.append({
            "cells": len(xs) + 1,
            "divider_count": len(xs),
            # Modal slot width. Ties break on the smaller value for determinism.
            "pitch_pt": min(collections.Counter(deltas).most_common(),
                            key=lambda kv: (-kv[1], kv[0]))[0],
            "pitch_min_pt": min(deltas),
            "pitch_max_pt": max(deltas),
            # Measured boundaries. Never synthesise slot x from index * pitch:
            # the real lattice carries 14.04-14.28 where 14.16 is nominal.
            "slot_x": boundaries,
            "divider_x": xs,
            "divider_thickness_pt": min(thicknesses.most_common(),
                                        key=lambda kv: (-kv[1], kv[0]))[0],
            # Thickness inside a comb encodes RANK, not membership: 0.24 is a
            # character divider, 0.96/1.44 a digit-group (thousands) separator.
            "divider_thicknesses_pt": sorted(thicknesses),
            "divider_gray": grays[0] if grays else None,
            "divider_paint_seq": [paint_ordinal(d) for d in ordered_band],
            "divider_paint_ranges": [
                list(paint_ordinal_range(d)) for d in ordered_band
            ],
            "y0": q(band_y0), "y1": q(band_y1),
            "height_pt": q(band_y1 - band_y0),
            "resolution": {
                "status": "unresolved" if reason_codes else "resolved",
                "method": "final-visible-endpoint-slab",
                "reason_codes": reason_codes,
            },
        })
        if (len(topology_evidence) > 1
                or "disconnected-final-visible-corridor" in reason_codes):
            bands[-1]["resolution"]["endpoint_topologies"] = topology_evidence
    # Different seed endpoint groups can converge on the same common slab (the
    # 2551Q TIN has long group bars and shorter character ticks). It is one
    # physical band, so retain it once.
    coalesced: list[dict[str, Any]] = []
    for band in sorted(
            bands,
            key=lambda value: (
                tuple(value["divider_x"]), value["y0"], value["y1"])):
        existing = next((
            value for value in coalesced
            if value["divider_x"] == band["divider_x"]
            and abs(float(value["y0"]) - float(band["y0"]))
            <= JOIN_EPSILON_PT
            and abs(float(value["y1"]) - float(band["y1"]))
            <= JOIN_EPSILON_PT
        ), None)
        if existing is None:
            coalesced.append(band)
            continue
        # Both endpoint groups prove the same divider topology. Their common
        # intersection is the band every representative actually spans.
        existing["y0"] = q(max(float(existing["y0"]), float(band["y0"])))
        existing["y1"] = q(min(float(existing["y1"]), float(band["y1"])))
        existing["height_pt"] = q(existing["y1"] - existing["y0"])
        resolution = existing["resolution"]
        other_resolution = band["resolution"]
        reasons = sorted({
            *(resolution.get("reason_codes") or ()),
            *(other_resolution.get("reason_codes") or ()),
        })
        if reasons:
            resolution["status"] = "unresolved"
            resolution["reason_codes"] = reasons
        evidence = [
            *(resolution.get("endpoint_topologies") or ()),
            *(other_resolution.get("endpoint_topologies") or ()),
        ]
        if evidence:
            unique_evidence = {
                (
                    tuple(item["divider_x"]),
                    tuple(tuple(run) for run in item["runs"]),
                ): item
                for item in evidence
            }
            resolution["endpoint_topologies"] = [
                unique_evidence[key] for key in sorted(unique_evidence)
            ]
    bands = coalesced
    bands.sort(key=lambda b: (b["y0"], -b["divider_count"]))
    return bands


def legacy_comb_bands(members: Sequence[dict[str, Any]],
                      extra: Sequence[dict[str, Any]],
                      x0: float, x1: float,
                      edge_thickness: tuple[float, float]
                      ) -> list[dict[str, Any]]:
    """Reconstruct the pre-partition subject ledger without promoting it.

    This is deliberately the old full-span detector. It is not a second answer
    to final-paint topology; it is the continuity denominator that prevents a
    row-run partition or a new compositor from silently deleting a published
    subject. A final-visible contract replaces it when that contract has at
    least as many measured boundaries. A reduction remains active-unresolved,
    or retained-unresolved when its nonrectangular owner no longer exists.
    """
    inside = [d for d in members
              if x0 + CLUSTER_TOL_PT < centre(d) < x1 - CLUSTER_TOL_PT]
    if not inside:
        return []

    by_band: dict[tuple[float, float], list[dict[str, Any]]] = (
        collections.defaultdict(list))
    for divider in inside:
        by_band[(divider["y0"], divider["y1"])].append(divider)

    left, right = edge_thickness
    frame = [(x0 - left / 2.0, x0 + left / 2.0, left),
             (x1 - right / 2.0, x1 + right / 2.0, right)]
    bands: list[dict[str, Any]] = []
    for (band_y0, band_y1), seed in sorted(by_band.items()):
        ink = [
            *seed,
            *band_ink(extra, x0, x1, band_y0, band_y1, [
                *frame,
                *((d["x0"], d["x1"], d["thickness_pt"]) for d in seed),
            ]),
        ]
        ordered = sorted(ink, key=lambda divider: (
            q(centre(divider)), -paint_ordinal(divider),
            -float(divider["thickness_pt"])))
        xs = [q(centre(divider)) for divider in ordered]
        boundaries = [q(x0), *xs, q(x1)]
        deltas = [q(b - a) for a, b in zip(boundaries, boundaries[1:])]
        thicknesses = collections.Counter(d["thickness_pt"] for d in ordered)
        grays = sorted({d["gray"] for d in ordered if d["gray"] is not None})
        bands.append({
            "cells": len(xs) + 1,
            "divider_count": len(xs),
            "pitch_pt": min(collections.Counter(deltas).most_common(),
                            key=lambda item: (-item[1], item[0]))[0],
            "pitch_min_pt": min(deltas),
            "pitch_max_pt": max(deltas),
            "slot_x": boundaries,
            "divider_x": xs,
            "divider_thickness_pt": min(
                thicknesses.most_common(),
                key=lambda item: (-item[1], item[0]))[0],
            "divider_thicknesses_pt": sorted(thicknesses),
            "divider_gray": grays[0] if grays else None,
            "divider_paint_seq": [paint_ordinal(d) for d in ordered],
            "divider_paint_ranges": [
                list(paint_ordinal_range(d)) for d in ordered
            ],
            "y0": q(band_y0), "y1": q(band_y1),
            "height_pt": q(band_y1 - band_y0),
            "resolution": {
                "status": "unresolved",
                "method": "legacy-continuity",
                "reason_codes": ["legacy-continuity-only"],
            },
        })
    bands.sort(key=lambda band: (band["y0"], -band["divider_count"]))
    return bands


def encloses_paper(lattice: Lattice, first: int, last: int) -> bool:
    """True when unpainted paper survives between two lattice lines.

    Fusion catches the boundaries that are drawn as a stack of bars, but a
    frame rule may also *jog*: 1606 draws its left page frame as one chain of
    segments whose x centre steps from 26.64 to 27.00 half way down the sheet,
    and 1701 CONSO steps its right frame twice. Those fragments are too far
    apart to cluster and never coincide along the page, so both centres stand
    as lattice lines with the ink of one bar spanning both. A cell between them
    contains no paper -- nothing can be printed there and nobody can write
    there -- so it is a walk artefact, not a container.
    """
    return lattice.ink_lo[last] > lattice.ink_hi[first]


def classify_cell(is_empty: bool, border_count: int, has_comb: bool) -> str:
    if is_empty and border_count >= 3:
        return "field"
    if is_empty:
        return "blank"
    # Pre-printed text sitting in a comb -- the "%" glyph, the money decimal
    # point, the TIN group dashes -- is decoration on a fillable field.
    return "mixed" if has_comb else "label"


def rectangular_row_runs(squares: Sequence[tuple[int, int]],
                         v_at: Sequence[Sequence[bool]],
                         h_at: Sequence[Sequence[bool]]
                         ) -> list[dict[str, int | bool]]:
    """Partition one connected grid component into painted-edge rectangles.

    A DSU component need not be rectangular. A missing edge can connect two
    broad regions through one narrow opening, producing an L, T, or frame-shaped
    component. Emitting its bounding box claims all the squares in the holes and
    overlaps real neighbouring cells; the largest examples cover most of a
    page and steal unrelated comb anchors by midpoint.

    The partition is deterministic and respects every visible internal edge:

    1. split each row into maximal horizontal runs, stopping at a vertical rule
       even if the squares reconnect elsewhere in the component;
    2. stack equal runs through consecutive rows only when their entire shared
       horizontal seam is open.

    Every returned rectangle is therefore made only of the component's squares
    and crosses no painted internal boundary. It is a partition, not a new
    geometry inference.
    """
    by_row: dict[int, list[int]] = collections.defaultdict(list)
    for j, i in squares:
        by_row[j].append(i)

    runs_by_row: dict[int, list[tuple[int, int]]] = {}
    for j, columns in by_row.items():
        ordered = sorted(set(columns))
        if not ordered:
            continue
        runs: list[tuple[int, int]] = []
        start = previous = ordered[0]
        for i in ordered[1:]:
            separated = i != previous + 1 or v_at[i][j]
            if separated:
                runs.append((start, previous + 1))
                start = i
            previous = i
        runs.append((start, previous + 1))
        runs_by_row[j] = runs

    rectangles: list[dict[str, int | bool]] = []
    active: dict[tuple[int, int], dict[str, int | bool]] = {}
    previous_row: int | None = None
    for j in sorted(runs_by_row):
        if previous_row is None or j != previous_row + 1:
            rectangles.extend(active.values())
            active = {}

        next_active: dict[tuple[int, int], dict[str, int | bool]] = {}
        for i0, i1 in runs_by_row[j]:
            span = (i0, i1)
            prior = active.get(span)
            seam_open = (
                prior is not None
                and int(prior["j1"]) == j
                and all(not h_at[j][i] for i in range(i0, i1))
            )
            if seam_open:
                prior["j1"] = j + 1
                next_active[span] = prior
            else:
                next_active[span] = {
                    "j0": j, "j1": j + 1,
                    "i0": i0, "i1": i1,
                    "rectangular": True,
                }

        rectangles.extend(rect for span, rect in active.items()
                          if span not in next_active or next_active[span] is not rect)
        active = next_active
        previous_row = j
    rectangles.extend(active.values())
    rectangles.sort(key=lambda box: (
        int(box["j0"]), int(box["i0"]),
        int(box["j1"]), int(box["i1"])))
    return rectangles


def crosses_painted_internal_edge(box: dict[str, int | bool],
                                  v_at: Sequence[Sequence[bool]],
                                  h_at: Sequence[Sequence[bool]]) -> bool:
    """Whether an emitted rectangle spans any visible internal separator."""
    j0, j1 = int(box["j0"]), int(box["j1"])
    i0, i1 = int(box["i0"]), int(box["i1"])
    return (
        any(v_at[i][j]
            for i in range(i0 + 1, i1)
            for j in range(j0, j1))
        or any(h_at[j][i]
               for j in range(j0 + 1, j1)
               for i in range(i0, i1))
    )


def assign_comb_anchors(cells: Sequence[dict[str, Any]],
                        dividers: Sequence[dict[str, Any]],
                        xl: Lattice, yl: Lattice,
                        final_paint: FinalPaint
                        ) -> tuple[list[list[dict[str, Any]]],
                                   list[dict[str, Any]],
                                   list[dict[str, Any]]]:
    """Give a final-visible divider band to exactly one rectangular subject.

    Midpoint ownership is unsafe: the bounding box of a non-rectangular DSU
    component can contain a divider whose actual band lies in another cell, and
    a later knockout can erase a raw mark while leaving its midpoint record
    behind. Ownership now requires the divider's full final-visible band to fit
    within the cell's painted outer ink bounds.

    Adjacent cells share their boundary ink. A band wholly inside that shared
    strip can therefore have two owners; that is ambiguous and is left
    unassigned rather than awarded by list order.
    """
    buckets: list[list[dict[str, Any]]] = [[] for _ in cells]
    unplaced: list[dict[str, Any]] = []
    ambiguous: list[dict[str, Any]] = []

    for divider in dividers:
        if not final_paint.structural_across(
                divider, float(divider["y0"]), float(divider["y1"])):
            unplaced.append(divider)
            continue
        cx = centre(divider)
        owners: list[int] = []
        for n, cell in enumerate(cells):
            i0, i1 = int(cell["col"]), int(cell["col"] + cell["col_span"])
            j0, j1 = int(cell["row"]), int(cell["row"] + cell["row_span"])
            if not (cell["x0"] + CLUSTER_TOL_PT
                    < cx
                    < cell["x1"] - CLUSTER_TOL_PT):
                continue
            if (divider["y0"] >= yl.ink_lo[j0] - CLUSTER_TOL_PT
                    and divider["y1"] <= yl.ink_hi[j1] + CLUSTER_TOL_PT
                    and divider["x0"] >= xl.ink_lo[i0] - CLUSTER_TOL_PT
                    and divider["x1"] <= xl.ink_hi[i1] + CLUSTER_TOL_PT):
                owners.append(n)
        if len(owners) == 1:
            buckets[owners[0]].append(divider)
        elif owners:
            ambiguous.append(divider)
        else:
            unplaced.append(divider)
    return buckets, unplaced, ambiguous


def comb_band_owners(cells: Sequence[dict[str, Any]],
                     x0: float, x1: float, y0: float, y1: float,
                     xl: Lattice, yl: Lattice) -> list[int]:
    """Cells whose painted outer bounds wholly contain one selected comb band."""
    owners: list[int] = []
    for n, cell in enumerate(cells):
        i0, i1 = int(cell["col"]), int(cell["col"] + cell["col_span"])
        j0, j1 = int(cell["row"]), int(cell["row"] + cell["row_span"])
        if (x0 >= xl.ink_lo[i0] - CLUSTER_TOL_PT
                and x1 <= xl.ink_hi[i1] + CLUSTER_TOL_PT
                and y0 >= yl.ink_lo[j0] - CLUSTER_TOL_PT
                and y1 <= yl.ink_hi[j1] + CLUSTER_TOL_PT):
            owners.append(n)
    return owners


def path_endpoint_conflicts(final_paint: FinalPaint,
                            band: dict[str, Any]) -> list[str]:
    """Later nonrect paint actually intersecting a divider's open endpoint.

    Bbox overlap is not ownership. Each path is reconstructed from its line and
    cubic subpaths, split into fill/stroke paint-order layers, and intersected
    with the narrow endpoint slab of each measured divider. A later intersecting
    layer makes the topology unresolved; it never deletes the subject.
    """
    conflicts: set[str] = set()
    sequences = list(band.get("divider_paint_seq") or ())
    ranges = list(band.get("divider_paint_ranges") or ())
    thicknesses = list(band.get("divider_thicknesses_pt") or ())
    default_thickness = max((float(value) for value in thicknesses), default=0.0)
    for index, divider_x in enumerate(band.get("divider_x") or ()):
        if index < len(ranges) and len(ranges[index]) == 2:
            divider_first = int(ranges[index][0])
        else:
            divider_first = (
                int(sequences[index]) if index < len(sequences) else -1)
        half = default_thickness / 2.0
        x0, x1 = float(divider_x) - half, float(divider_x) + half
        y0 = float(band["y0"]) - JOIN_EPSILON_PT
        y1 = float(band["y0"]) + JOIN_EPSILON_PT
        for path in final_paint.path_paints:
            _path_first, path_last = paint_ordinal_range(path)
            # Only a path known to finish no later than the divider begins is
            # safely earlier. Overlapping source-order ranges may put the path
            # on top at this endpoint and therefore remain unresolved.
            if path_last <= divider_first:
                continue
            if path_paint_intersects_rect(path, x0, y0, x1, y1):
                conflicts.add(
                    f"{path.get('id', 'path')}:{path.get('_path_layer', 'path')}")
    return sorted(conflicts)


def mark_comb_unresolved(comb: dict[str, Any], *reason_codes: str,
                         method: str | None = None) -> dict[str, Any]:
    """Copy a comb contract and append machine-readable uncertainty."""
    marked = dict(comb)
    previous = dict(marked.get("resolution") or {})
    reasons = set(previous.get("reason_codes") or ())
    reasons.update(reason for reason in reason_codes if reason)
    previous.update({
        "status": "unresolved",
        "method": method or previous.get("method") or "unresolved",
        "reason_codes": sorted(reasons),
    })
    marked["resolution"] = previous
    return marked


def geometry_subject_key(page_index: int, bbox: Sequence[float]) -> str:
    """Stable subject identity independent of sequential cell enumeration."""
    return "p{}@{}".format(
        page_index,
        ",".join(f"{q(value):.{QUANT}f}" for value in bbox),
    )


def same_boundary_topology(left: Sequence[float],
                           right: Sequence[float]) -> bool:
    """One-to-one equality at the established lattice clustering tolerance."""
    return (
        len(left) == len(right)
        # Coordinates are decimal-quantised, but subtracting (for example)
        # 10.30 - 10.00 can be a binary float just above 0.30. The epsilon is
        # representation-only and far below the 0.01pt source precision.
        and all(abs(a - b) <= CLUSTER_TOL_PT + 1e-9
                for a, b in zip(sorted(left), sorted(right)))
    )


def boundary_topology_subset(left: Sequence[float],
                             right: Sequence[float]) -> bool:
    """Strict monotone one-to-one subset at the clustering tolerance.

    ``all(any())`` is insufficient here: two close values on the left could
    otherwise reuse one value on the right and manufacture a subset. Both
    topologies are ordered physical boundaries, so a monotone scan is the
    deterministic matching certificate.
    """
    if len(left) >= len(right):
        return False
    available = iter(sorted(float(value) for value in right))
    candidate = next(available, None)
    for wanted in sorted(float(value) for value in left):
        while candidate is not None and candidate < wanted - CLUSTER_TOL_PT:
            candidate = next(available, None)
        if candidate is None or candidate > wanted + CLUSTER_TOL_PT:
            return False
        candidate = next(available, None)
    return True


def source_owned_comb_frame(
        box: dict[str, Any],
        xl: Lattice, yl: Lattice,
        v_at: Sequence[Sequence[bool]],
        h_at: Sequence[Sequence[bool]],
        dividers: Sequence[dict[str, Any]],
        extra_ink: Sequence[dict[str, Any]],
        final_paint: FinalPaint,
        ) -> dict[str, Any] | None:
    """Prove that partial internal verticals belong to one framed comb field.

    Some official fields place a comb against the bottom stroke of a taller
    framed label cell. Collinear fragments from elsewhere make a subset of its
    comb ticks lattice positions, so a generic painted-edge partition would
    shatter the field. Preserve the broad cell only when final paint proves the
    complete outer frame and one resolved, bottom-frame-owned same-band divider
    topology. This is geometry-owned, never form-specific.
    """
    j0, j1 = int(box["j0"]), int(box["j1"])
    i0, i1 = int(box["i0"]), int(box["i1"])
    internal_verticals = [
        (i, j)
        for i in range(i0 + 1, i1)
        for j in range(j0, j1)
        if v_at[i][j]
    ]
    internal_horizontals = [
        (j, i)
        for j in range(j0 + 1, j1)
        for i in range(i0, i1)
        if h_at[j][i]
    ]
    # A crossed edge is what makes a preservation certificate necessary. Some
    # heavy group dividers contribute only horizontal endpoint caps; requiring
    # a vertical crossing would partition those otherwise identical fields.
    if not internal_verticals and not internal_horizontals:
        return None
    if not (
        all(v_at[i0][j] and v_at[i1][j] for j in range(j0, j1))
        and all(h_at[j0][i] and h_at[j1][i] for i in range(i0, i1))
    ):
        return None

    x0, x1 = xl.positions[i0], xl.positions[i1]
    y0, y1 = yl.positions[j0], yl.positions[j1]
    members = [
        divider for divider in dividers
        if x0 + CLUSTER_TOL_PT < centre(divider) < x1 - CLUSTER_TOL_PT
        and y0 <= (float(divider["y0"]) + float(divider["y1"])) / 2.0 <= y1
    ]
    if not members:
        return None
    edge_thickness = (
        max((float(rule["thickness_pt"]) for rule in xl.members[i0]),
            default=q(xl.ink_hi[i0] - xl.ink_lo[i0])),
        max((float(rule["thickness_pt"]) for rule in xl.members[i1]),
            default=q(xl.ink_hi[i1] - xl.ink_lo[i1])),
    )
    bands = comb_bands(
        members, extra_ink, x0, x1, edge_thickness, final_paint)
    bands = [
        band for band in bands
        if float(band["y0"]) >= y0 - CLUSTER_TOL_PT
        and float(band["y1"]) <= y1 + CLUSTER_TOL_PT
    ]
    if len(bands) != 1:
        return None
    band = bands[0]
    resolution = band.get("resolution") or {}
    reason_codes = set(resolution.get("reason_codes") or ())
    frame_resolves_competition = False
    if reason_codes == {"competing-endpoint-topologies"}:
        topologies = [
            tuple(float(value) for value in evidence["divider_x"])
            for evidence in resolution.get("endpoint_topologies") or ()
        ]
        maximal = [
            topology for topology in topologies
            if not any(boundary_topology_subset(topology, other)
                       for other in topologies)
        ]
        frame_resolves_competition = (
            len(maximal) == 1
            and same_boundary_topology(
                tuple(float(value) for value in band["divider_x"]),
                maximal[0])
        )
    if ((resolution.get("status") != "resolved"
         and not frame_resolves_competition)
            or band["y0"] < y0 - CLUSTER_TOL_PT
            or band["y1"] > y1 + CLUSTER_TOL_PT):
        return None

    # The complete outer frame must survive, not only the lower writing band.
    if not (covers(xl.spans[i0], yl.ink_hi[j0], yl.ink_lo[j1])
            and covers(xl.spans[i1], yl.ink_hi[j0], yl.ink_lo[j1])
            and covers(yl.spans[j0], xl.ink_hi[i0], xl.ink_lo[i1])
            and covers(yl.spans[j1], xl.ink_hi[i0], xl.ink_lo[i1])):
        return None

    # One final-visible horizontal lattice boundary must be the connected
    # baseline spanning the paper between both rails.
    baseline_index = j1
    if not (
        yl.ink_lo[baseline_index] - JOIN_EPSILON_PT
        <= float(band["y1"])
        <= yl.ink_hi[baseline_index] + JOIN_EPSILON_PT
        and covers(
            yl.spans[baseline_index], xl.ink_hi[i0], xl.ink_lo[i1])
    ):
        return None

    divider_x = [float(value) for value in band["divider_x"]]
    divider_corridors: list[tuple[float, float]] = []
    for value in divider_x:
        matches = [
            ink for ink in [*members, *extra_ink]
            if abs(centre(ink) - value) <= CLUSTER_TOL_PT
            and float(ink["y0"]) <= float(band["y0"]) + CLUSTER_TOL_PT
            and float(ink["y1"]) >= float(band["y1"]) - CLUSTER_TOL_PT
        ]
        if not matches:
            return None
        divider_corridors.append((
            min(float(ink["x0"]) for ink in matches),
            max(float(ink["x1"]) for ink in matches),
        ))

    # A thick group divider can contribute a short horizontal cap at the band
    # endpoint. Such an edge is owned by the comb only when the whole crossed
    # adjacency lies inside one certified divider corridor. A row separator (or
    # the synthetic 2x2 reconnect seam) cannot satisfy this.
    for j, i in internal_horizontals:
        at_band_endpoint = (
            yl.ink_lo[j] - JOIN_EPSILON_PT
            <= float(band["y0"])
            <= yl.ink_hi[j] + JOIN_EPSILON_PT
            or yl.ink_lo[j] - JOIN_EPSILON_PT
            <= float(band["y1"])
            <= yl.ink_hi[j] + JOIN_EPSILON_PT
        )
        if not at_band_endpoint:
            return None
        if not any(
            corridor_x0 - CLUSTER_TOL_PT <= xl.positions[i]
            and xl.positions[i + 1] <= corridor_x1 + CLUSTER_TOL_PT
            for corridor_x0, corridor_x1 in divider_corridors
        ):
            return None

    for i, j in internal_verticals:
        if not any(abs(xl.positions[i] - value) <= CLUSTER_TOL_PT
                   for value in divider_x):
            return None

        # Attribute the *actual final-visible ink*, not merely the grid row in
        # which coverage was observed. A long same-x separator can overlap a
        # short comb tick near its endpoint while extending far into the label
        # above; testing only the row interval would then preserve a component
        # across real structural ink. The selected band plus the bottom frame
        # stroke is the complete allowed vertical corridor. A divider may cross
        # that baseline stroke, but no matching ink may escape above the band.
        band_y0 = float(band["y0"])
        endpoint_boundaries = [
            boundary
            for boundary in range(j0, j1 + 1)
            if (yl.ink_lo[boundary] - JOIN_EPSILON_PT
                <= band_y0
                <= yl.ink_hi[boundary] + JOIN_EPSILON_PT)
        ]
        allowed_y0 = min(
            [band_y0, *(yl.ink_lo[boundary]
                        for boundary in endpoint_boundaries)])
        allowed_y1 = yl.ink_hi[j1]
        allowed_vertical_ink = union_intervals([
            # Collinear ink from the preceding field can enter the outer top
            # stroke by a rounding sliver without separating this cell's paper.
            (y0, min(y1, yl.ink_hi[j0])),
            (max(y0, allowed_y0), min(y1, allowed_y1)),
        ])
        relevant: list[dict[str, Any]] = []
        seen_relevant: set[int] = set()
        for ink in [*members, *extra_ink, *xl.members[i]]:
            if id(ink) in seen_relevant:
                continue
            seen_relevant.add(id(ink))
            if abs(centre(ink) - xl.positions[i]) > CLUSTER_TOL_PT:
                continue
            # Only ink with positive area inside this component can constrain
            # it. A rule ending in the outer frame just above the component is
            # not an uncertain internal separator.
            if (float(ink["y1"]) <= y0
                    or float(ink["y0"]) >= y1):
                continue
            relevant.append(ink)
        if not relevant:
            return None

        for ink in relevant:
            visible = [
                (max(y0, start), min(y1, end))
                for start, end in final_paint.visible_intervals(ink)
                if min(y1, end) > max(y0, start)
            ]
            if not visible:
                # An uncertain same-x source rule cannot be assumed absent when
                # that assumption is what makes the broad frame rectangular.
                if not final_paint.definitely_erased(ink):
                    return None
                continue
            # Rectangular divider bars in the source can have one square-cap
            # overhang before the common endpoint slab. Attribute at most that
            # bar's own measured weight, contiguous with the selected band.
            # This admits the 0.48pt ragged endpoint present in several forms;
            # it cannot hide a long same-x separator.
            cap_floor = max(
                y0,
                band_y0 - float(ink["thickness_pt"]) - JOIN_EPSILON_PT,
            )
            ink_allowed = union_intervals([
                *allowed_vertical_ink,
                (cap_floor, min(y1, band_y0)),
            ])
            if any(not covers(ink_allowed, start, end)
                   for start, end in visible):
                return None

    return {
        "method": "final-visible-framed-comb",
        "outer_rail_x": [q(x0), q(x1)],
        "baseline_y": q(yl.positions[baseline_index]),
        "band_y": [q(band["y0"]), q(band["y1"])],
        "divider_x": list(band["divider_x"]),
        "resolved_competing_topologies": frame_resolves_competition,
        "internal_lattice_x": sorted({
            q(xl.positions[i]) for i, _j in internal_verticals
        }),
        "internal_cap_edges": [
            [q(yl.positions[j]), q(xl.positions[i]), q(xl.positions[i + 1])]
            for j, i in internal_horizontals
        ],
    }


def build_cells(page_index: int, xl: Lattice, yl: Lattice,
                dsu: DisjointSet, v_at: list[list[bool]], h_at: list[list[bool]],
                v_ink: Sequence[dict[str, Any]], h_ink: Sequence[dict[str, Any]],
                dividers: Sequence[dict[str, Any]],
                extra_ink: Sequence[dict[str, Any]],
                final_paint: FinalPaint,
                text_runs: Sequence[dict[str, Any]],
                legacy_dividers: Sequence[dict[str, Any]] | None = None,
                legacy_extra_ink: Sequence[dict[str, Any]] | None = None,
                final_supported_divider_ids: set[str] | None = None,
                frame_dividers: Sequence[dict[str, Any]] | None = None,
                legacy_xl: Lattice | None = None,
                legacy_yl: Lattice | None = None,
                legacy_dsu: DisjointSet | None = None,
                legacy_v_at: list[list[bool]] | None = None,
                legacy_h_at: list[list[bool]] | None = None,
                legacy_v_ink: Sequence[dict[str, Any]] | None = None,
                legacy_h_ink: Sequence[dict[str, Any]] | None = None,
                uncertain_geometry_ids: set[str] | None = None,
                ) -> tuple[list[dict[str, Any]], list[str],
                           list[dict[str, Any]], list[dict[str, Any]]]:
    nx, ny = len(xl) - 1, len(yl) - 1
    components: dict[int, list[tuple[int, int]]] = collections.defaultdict(list)
    for j in range(ny):
        for i in range(nx):
            components[dsu.find(j * nx + i)].append((j, i))

    certificate_dividers = (
        dividers if frame_dividers is None else frame_dividers)
    certificate_extra = (
        extra_ink if legacy_extra_ink is None else legacy_extra_ink)
    boxes: list[dict[str, Any]] = []
    for root, squares in components.items():
        js = [j for j, _ in squares]
        is_ = [i for _, i in squares]
        component_box: dict[str, Any] = {
            "j0": min(js), "j1": max(js) + 1,
            "i0": min(is_), "i1": max(is_) + 1,
            "component_root": root,
        }
        component_is_rectangular = (
            len(squares)
            == (int(component_box["j1"]) - int(component_box["j0"]))
            * (int(component_box["i1"]) - int(component_box["i0"]))
        )
        component_box["rectangular"] = component_is_rectangular
        # Occupancy alone does not make a safe rectangle. A fully occupied
        # component can reconnect around a partial internal rule (a 2x2 block
        # split only in its top row is the minimal example). Partition every
        # component on painted row runs; keep the broad component_box above
        # solely for the legacy subject/ID continuity ledger.
        comb_frame = (
            source_owned_comb_frame(
                component_box, xl, yl, v_at, h_at,
                certificate_dividers, certificate_extra, final_paint)
            if component_is_rectangular else None
        )
        partitions = (
            [dict(component_box)] if comb_frame is not None
            else rectangular_row_runs(squares, v_at, h_at)
        )
        for partition in partitions:
            partition["component_root"] = root
            if comb_frame is not None:
                partition["comb_frame_certificate"] = comb_frame
            elif crosses_painted_internal_edge(partition, v_at, h_at):
                raise ValueError(
                    "row-run partition crosses a painted internal edge")
        for box in partitions:
            j0, j1 = int(box["j0"]), int(box["j1"])
            i0, i1 = int(box["i0"]), int(box["i1"])
            if not encloses_paper(xl, i0, i1) or not encloses_paper(yl, j0, j1):
                continue
            boxes.append(box)

    continuity_xl = xl if legacy_xl is None else legacy_xl
    continuity_yl = yl if legacy_yl is None else legacy_yl
    continuity_dsu = dsu if legacy_dsu is None else legacy_dsu
    continuity_v_at = v_at if legacy_v_at is None else legacy_v_at
    continuity_h_at = h_at if legacy_h_at is None else legacy_h_at
    continuity_v_ink = v_ink if legacy_v_ink is None else legacy_v_ink
    continuity_h_ink = h_ink if legacy_h_ink is None else legacy_h_ink
    unresolved_geometry_ids = (
        set() if uncertain_geometry_ids is None else uncertain_geometry_ids)
    legacy_components: dict[int, list[tuple[int, int]]] = (
        collections.defaultdict(list))
    legacy_nx, legacy_ny = len(continuity_xl) - 1, len(continuity_yl) - 1
    for j in range(legacy_ny):
        for i in range(legacy_nx):
            legacy_components[
                continuity_dsu.find(j * legacy_nx + i)
            ].append((j, i))

    legacy_boxes: list[dict[str, Any]] = []
    for root, squares in legacy_components.items():
        js = [j for j, _i in squares]
        is_ = [i for _j, i in squares]
        box: dict[str, Any] = {
            "j0": min(js), "j1": max(js) + 1,
            "i0": min(is_), "i1": max(is_) + 1,
            "component_root": root,
        }
        box["rectangular"] = (
            len(squares)
            == (int(box["j1"]) - int(box["j0"]))
            * (int(box["i1"]) - int(box["i0"]))
        )
        if (encloses_paper(
                continuity_xl, int(box["i0"]), int(box["i1"]))
                and encloses_paper(
                    continuity_yl, int(box["j0"]), int(box["j1"]))):
            legacy_boxes.append(box)

    legacy_boxes.sort(
        key=lambda box: (
            continuity_yl.positions[box["j0"]],
            continuity_xl.positions[box["i0"]],
        ))
    for index, box in enumerate(legacy_boxes):
        box["legacy_index"] = index
    boxes.sort(key=lambda b: (yl.positions[b["j0"]], xl.positions[b["i0"]]))

    def materialise_cell(
            box: dict[str, Any], identifier: str,
            cell_xl: Lattice = xl, cell_yl: Lattice = yl,
            cell_v_at: Sequence[Sequence[bool]] = v_at,
            cell_h_at: Sequence[Sequence[bool]] = h_at,
            cell_v_ink: Sequence[dict[str, Any]] = v_ink,
            cell_h_ink: Sequence[dict[str, Any]] = h_ink,
            track_geometry_uncertainty: bool = True,
            ) -> dict[str, Any]:
        j0, j1 = int(box["j0"]), int(box["j1"])
        i0, i1 = int(box["i0"]), int(box["i1"])
        x0, x1 = cell_xl.positions[i0], cell_xl.positions[i1]
        y0, y1 = cell_yl.positions[j0], cell_yl.positions[j1]

        border: dict[str, Any] = {}
        uncertain_border_ids: set[str] = set()
        for side, (lat, index, ink, lo, hi, present) in {
            "top": (
                cell_yl, j0, cell_h_ink,
                cell_xl.ink_hi[i0], cell_xl.ink_lo[i1],
                all(cell_h_at[j0][i] for i in range(i0, i1))),
            "bottom": (
                cell_yl, j1, cell_h_ink,
                cell_xl.ink_hi[i0], cell_xl.ink_lo[i1],
                all(cell_h_at[j1][i] for i in range(i0, i1))),
            "left": (
                cell_xl, i0, cell_v_ink,
                cell_yl.ink_hi[j0], cell_yl.ink_lo[j1],
                all(cell_v_at[i0][j] for j in range(j0, j1))),
            "right": (
                cell_xl, i1, cell_v_ink,
                cell_yl.ink_hi[j0], cell_yl.ink_lo[j1],
                all(cell_v_at[i1][j] for j in range(j0, j1))),
        }.items():
            if not present:
                border[side] = None
                continue
            thickness, gray, all_t = line_thickness_gray(
                lat, index, ink, lo, hi, "h" if side in ("top", "bottom") else "v")
            border[side] = {"thickness_pt": thickness, "gray": gray,
                            "thicknesses_pt": all_t}
            if track_geometry_uncertainty and unresolved_geometry_ids:
                own = {id(rule) for rule in lat.members[index]}
                a0, a1 = (
                    ("x0", "x1") if side in ("top", "bottom")
                    else ("y0", "y1"))
                uncertain_border_ids.update(
                    str(rule.get("id"))
                    for rule in ink
                    if str(rule.get("id")) in unresolved_geometry_ids
                    and (abs(centre(rule) - lat.positions[index])
                         <= CLUSTER_TOL_PT
                         or id(rule) in own)
                    and float(rule[a1]) > lo - CLUSTER_TOL_PT
                    and float(rule[a0]) < hi + CLUSTER_TOL_PT
                )

        border_count = sum(1 for b in border.values() if b is not None)
        cell = {
            "id": identifier,
            "subject_key": geometry_subject_key(page_index, (x0, y0, x1, y1)),
            "x0": x0, "y0": y0, "x1": x1, "y1": y1,
            "row": j0, "col": i0, "row_span": j1 - j0, "col_span": i1 - i0,
            "rectangular": bool(box["rectangular"]),
            "border": border,
            "border_count": border_count,
            "text_run_ids": [],
            "is_empty": True,
            "kind": "blank",
            "_component_root": int(box["component_root"]),
        }
        if "comb_frame_certificate" in box:
            cell["comb_frame_certificate"] = box["comb_frame_certificate"]
        if uncertain_border_ids:
            cell["geometry_resolution"] = {
                "status": "unresolved",
                "reason_codes": ["uncertain-final-paint-boundary"],
                "rule_ids": sorted(uncertain_border_ids),
            }
        return cell

    legacy_cells = [
        materialise_cell(
            box, f"p{page_index}c{box['legacy_index']}",
            continuity_xl, continuity_yl,
            continuity_v_at, continuity_h_at,
            continuity_v_ink, continuity_h_ink,
            track_geometry_uncertainty=False)
        for box in legacy_boxes
    ]
    legacy_id_by_subject = {
        str(cell["subject_key"]): str(cell["id"])
        for cell in legacy_cells
    }

    cells: list[dict[str, Any]] = []
    next_partition_id = len(legacy_boxes)
    for box in boxes:
        cell = materialise_cell(box, "")
        identifier = legacy_id_by_subject.get(str(cell["subject_key"]))
        if identifier is None:
            identifier = f"p{page_index}c{next_partition_id}"
            next_partition_id += 1
        cell["id"] = identifier
        cells.append(cell)

    # The legacy cells are subject-discovery geometry only. They reproduce the
    # published denominator and ids, but a nonrectangular one is never emitted.
    continuity_dividers = (
        dividers if legacy_dividers is None else legacy_dividers)
    continuity_extra = (
        extra_ink if legacy_extra_ink is None else legacy_extra_ink)
    supported_ids = (
        {str(divider.get("id")) for divider in dividers}
        if final_supported_divider_ids is None
        else final_supported_divider_ids)
    legacy_anchor_buckets, _ = assign_points(
        legacy_cells,
        [(centre(divider), (divider["y0"] + divider["y1"]) / 2.0, divider)
         for divider in continuity_dividers],
    )
    legacy_subjects: list[dict[str, Any]] = []
    for cell, members in zip(legacy_cells, legacy_anchor_buckets):
        edges = tuple(
            0.0 if cell["border"][side] is None
            else cell["border"][side]["thickness_pt"]
            for side in ("left", "right"))
        bands = legacy_comb_bands(
            members, continuity_extra, cell["x0"], cell["x1"], edges)
        if not bands:
            continue
        selected = max(
            bands, key=lambda band: (band["divider_count"], -band["y0"]))
        supported_members = [
            member for member in members
            if str(member.get("id")) in supported_ids
        ]
        left, right = edges
        support_frame = [
            (cell["x0"] - left / 2.0, cell["x0"] + left / 2.0, left),
            (cell["x1"] - right / 2.0, cell["x1"] + right / 2.0, right),
        ]
        has_distinct_final_support = any(
            all(distinct_boundary((
                float(member["x0"]), float(member["x1"]),
                float(member["thickness_pt"])), edge)
                for edge in support_frame)
            for member in supported_members
        )
        final_candidates = comb_bands(
            supported_members,
            continuity_extra, cell["x0"], cell["x1"], edges, final_paint)
        final_candidate = (
            max(final_candidates,
                key=lambda band: (band["divider_count"], -band["y0"]))
            if final_candidates else None
        )
        legacy_subjects.append({
            "subject_key": cell["subject_key"],
            "legacy_cell_id": cell["id"],
            "legacy_bbox": [
                cell["x0"], cell["y0"], cell["x1"], cell["y1"],
            ],
            "legacy_cell": cell,
            "legacy_rectangular": bool(cell["rectangular"]),
            "component_root": cell["_component_root"],
            "comb": selected,
            "final_candidate": final_candidate,
            "has_final_support": has_distinct_final_support,
        })

    anchor_buckets, _unplaced_anchors, _ambiguous_anchors = assign_comb_anchors(
        cells, dividers, xl, yl, final_paint)
    for cell_index, (cell, members) in enumerate(zip(cells, anchor_buckets)):
        edges = tuple(0.0 if cell["border"][side] is None
                      else cell["border"][side]["thickness_pt"]
                      for side in ("left", "right"))
        bands = comb_bands(
            members, extra_ink, cell["x0"], cell["x1"], edges, final_paint)
        retained_bands: list[dict[str, Any]] = []
        for band in bands:
            owners = comb_band_owners(
                cells, cell["x0"], cell["x1"],
                band["y0"], band["y1"], xl, yl)
            if owners and cell_index not in owners:
                continue
            if owners != [cell_index]:
                reason = ("ambiguous-band-ownership" if owners
                          else "no-full-band-owner")
                band = mark_comb_unresolved(band, reason)
            conflicts = path_endpoint_conflicts(final_paint, band)
            if conflicts:
                band = mark_comb_unresolved(
                    band, "later-nonrect-path-endpoint-paint")
                band["resolution"]["path_conflicts"] = conflicts
            retained_bands.append(band)
        bands = retained_bands
        if bands:
            chosen_band = max(
                bands, key=lambda b: (b["divider_count"], -b["y0"]))
            certificate = cell.get("comb_frame_certificate") or {}
            chosen_resolution = chosen_band.get("resolution") or {}
            if (certificate.get("resolved_competing_topologies")
                    and set(chosen_resolution.get("reason_codes") or ())
                    == {"competing-endpoint-topologies"}):
                chosen_band = dict(chosen_band)
                chosen_band["resolution"] = {
                    **chosen_resolution,
                    "status": "resolved",
                    "method": "final-visible-framed-comb",
                    "reason_codes": [],
                }
            if cell.get("geometry_resolution"):
                chosen_band = mark_comb_unresolved(
                    chosen_band, "uncertain-final-paint-boundary")
            cell["comb"] = chosen_band
            if len(bands) > 1:
                cell["combs"] = bands

    output_by_subject = {cell["subject_key"]: cell for cell in cells}

    def boundary_rule_evidence(
            legacy_cell: dict[str, Any],
            candidate: dict[str, Any],
            changed_side: int,
            ) -> dict[str, Any]:
        """Source-paint state for one erased-edge replacement candidate."""
        vertical = changed_side in (0, 2)
        if vertical:
            old_index = (
                int(legacy_cell["col"])
                if changed_side == 0
                else int(legacy_cell["col"] + legacy_cell["col_span"])
            )
            new_index = (
                int(candidate["col"])
                if changed_side == 0
                else int(candidate["col"] + candidate["col_span"])
            )
            old_lattice, new_lattice, axis = continuity_xl, xl, "v"
            old_select = (float(legacy_cell["y0"]), float(legacy_cell["y1"]))
            new_select = (float(candidate["y0"]), float(candidate["y1"]))
            old_open = (
                continuity_yl.ink_hi[int(legacy_cell["row"])],
                continuity_yl.ink_lo[
                    int(legacy_cell["row"] + legacy_cell["row_span"])],
            )
            new_open = (
                yl.ink_hi[int(candidate["row"])],
                yl.ink_lo[int(candidate["row"] + candidate["row_span"])],
            )
            span_keys = ("y0", "y1")
        else:
            old_index = (
                int(legacy_cell["row"])
                if changed_side == 1
                else int(legacy_cell["row"] + legacy_cell["row_span"])
            )
            new_index = (
                int(candidate["row"])
                if changed_side == 1
                else int(candidate["row"] + candidate["row_span"])
            )
            old_lattice, new_lattice, axis = continuity_yl, yl, "h"
            old_select = (float(legacy_cell["x0"]), float(legacy_cell["x1"]))
            new_select = (float(candidate["x0"]), float(candidate["x1"]))
            old_open = (
                continuity_xl.ink_hi[int(legacy_cell["col"])],
                continuity_xl.ink_lo[
                    int(legacy_cell["col"] + legacy_cell["col_span"])],
            )
            new_open = (
                xl.ink_hi[int(candidate["col"])],
                xl.ink_lo[int(candidate["col"] + candidate["col_span"])],
            )
            span_keys = ("x0", "x1")

        def relevant_rules(lattice: Lattice, index: int,
                           span: tuple[float, float]
                           ) -> list[dict[str, Any]]:
            start_key, end_key = span_keys
            return [
                rule for rule in lattice.members[index]
                if float(rule[end_key]) > span[0]
                and float(rule[start_key]) < span[1]
            ]

        def state(rules: Sequence[dict[str, Any]],
                  open_span: tuple[float, float]) -> str:
            if any(final_paint.structural_across_axis(
                    rule, open_span[0], open_span[1], axis)
                   for rule in rules):
                return "final_visible"
            if rules and all(final_paint.definitely_erased(rule)
                             for rule in rules):
                return "definitely_erased"
            return "unresolved"

        old_rules = relevant_rules(old_lattice, old_index, old_select)
        new_rules = relevant_rules(new_lattice, new_index, new_select)
        return {
            "changed_side": ("left", "top", "right", "bottom")[changed_side],
            "old_boundary_position": q(old_lattice.positions[old_index]),
            "replacement_boundary_position": q(
                new_lattice.positions[new_index]),
            "old_rule_ids": sorted(str(rule.get("id")) for rule in old_rules),
            "replacement_rule_ids": sorted(
                str(rule.get("id")) for rule in new_rules),
            "old_boundary_final_state": state(old_rules, old_open),
            "replacement_boundary_final_state": state(new_rules, new_open),
        }

    def erased_edge_replacement_candidates(
            subject: dict[str, Any]) -> list[dict[str, Any]]:
        """Unique current rectangles that expand across one erased old edge.

        This is transition evidence, not an activation shortcut. In particular,
        an unresolved replacement rail leaves the legacy subject retained and
        blocking even when the geometric candidate is otherwise one-to-one.
        """
        if not subject["legacy_rectangular"]:
            return []
        legacy_cell = subject["legacy_cell"]
        legacy_bbox = [float(value) for value in subject["legacy_bbox"]]
        legacy_comb = subject["comb"]
        candidates: list[dict[str, Any]] = []
        for candidate in cells:
            current_bbox = [
                float(candidate["x0"]), float(candidate["y0"]),
                float(candidate["x1"]), float(candidate["y1"]),
            ]
            changed = [
                index for index, (old, new) in enumerate(
                    zip(legacy_bbox, current_bbox))
                if q(old) != q(new)
            ]
            if len(changed) != 1:
                continue
            if not (
                current_bbox[0] <= legacy_bbox[0]
                and current_bbox[1] <= legacy_bbox[1]
                and current_bbox[2] >= legacy_bbox[2]
                and current_bbox[3] >= legacy_bbox[3]
            ):
                continue
            if candidate["border_count"] != 4:
                continue
            if not (
                current_bbox[0] < min(legacy_comb["divider_x"])
                and current_bbox[2] > max(legacy_comb["divider_x"])
                and current_bbox[1] <= float(legacy_comb["y0"])
                and current_bbox[3] >= float(legacy_comb["y1"])
            ):
                continue
            current_comb = candidate.get("comb")
            if (current_comb is not None
                    and (int(current_comb["cells"])
                         != int(legacy_comb["cells"])
                         or not same_boundary_topology(
                             current_comb["divider_x"],
                             legacy_comb["divider_x"]))):
                continue
            paint = boundary_rule_evidence(
                legacy_cell, candidate, changed[0])
            if paint["old_boundary_final_state"] != "definitely_erased":
                continue
            blockers = []
            if paint["replacement_boundary_final_state"] != "final_visible":
                blockers.append("replacement-boundary-not-final-visible")
            if current_comb is None:
                blockers.append("no-final-visible-owned-band")
            if candidate.get("geometry_resolution"):
                blockers.extend(
                    candidate["geometry_resolution"].get("reason_codes") or ())
            candidates.append({
                "cell_id": candidate["id"],
                "old_subject_key": subject["subject_key"],
                "new_subject_key": candidate["subject_key"],
                "old_bbox": subject["legacy_bbox"],
                "new_bbox": current_bbox,
                "cells": int(legacy_comb["cells"]),
                "band_y": [
                    float(legacy_comb["y0"]), float(legacy_comb["y1"]),
                ],
                "divider_x": list(legacy_comb["divider_x"]),
                "old_slot_x": list(legacy_comb["slot_x"]),
                "new_slot_x": [
                    current_bbox[0], *legacy_comb["divider_x"],
                    current_bbox[2],
                ],
                "source_paint_evidence": paint,
                "activation_blockers": sorted(set(blockers)),
                "blocks_gate": True,
            })
        candidates.sort(key=lambda item: (
            item["new_bbox"], item["new_subject_key"]))
        for candidate in candidates:
            candidate["one_to_one_geometry_candidate"] = (
                len(candidates) == 1)
        return candidates

    subject_ledger: list[dict[str, Any]] = []
    inference_ledger: list[dict[str, Any]] = []
    legacy_keys = {
        str(subject["subject_key"]) for subject in legacy_subjects
    }
    for subject in legacy_subjects:
        subject_key = str(subject["subject_key"])
        cell = output_by_subject.get(subject_key)
        legacy_comb = subject["comb"]
        final_candidate = subject["final_candidate"]
        if (cell is None
                or (cell.get("comb") is None
                    and final_candidate is None
                    and not subject["has_final_support"])):
            if cell is None:
                sx0, sy0, sx1, sy1 = (
                    float(value) for value in subject["legacy_bbox"])
                mapped = [
                    candidate for candidate in cells
                    if candidate["x0"] >= sx0 - CLUSTER_TOL_PT
                    and candidate["x1"] <= sx1 + CLUSTER_TOL_PT
                    and candidate["y0"] >= sy0 - CLUSTER_TOL_PT
                    and candidate["y1"] <= sy1 + CLUSTER_TOL_PT
                ]
            else:
                mapped = [cell]
            retained = {
                "subject_key": subject_key,
                "legacy_cell_id": subject["legacy_cell_id"],
                "legacy_bbox": subject["legacy_bbox"],
                "cell_id": None,
                "mapped_partition_cell_ids": [cell["id"] for cell in mapped],
                "mapped_partition_subject_keys": [
                    cell["subject_key"] for cell in mapped
                ],
                "state": "retained_unresolved",
                "emission": "suppressed",
                "reason_codes": ([
                    "emission-suppressed-no-rectangular-owner",
                    "painted-edge-partition",
                ] if cell is None else [
                    "emission-suppressed-no-final-visible-band",
                ]),
                "legacy_comb": legacy_comb,
                "requires_independent_evidence": True,
                "permitted_transitions": [
                    "active_composite",
                    "retired_proven_false",
                ],
                "blocks_gate": True,
            }
            replacements = erased_edge_replacement_candidates(subject)
            if replacements:
                retained["erased_edge_replacement_candidates"] = replacements
            subject_ledger.append(retained)
            continue

        resolved = cell.get("comb")
        if (resolved is not None and final_candidate is not None
                and int(final_candidate["cells"]) > int(resolved["cells"])):
            cell["comb"] = mark_comb_unresolved(
                final_candidate, "anchor-ownership-disagreement")
            resolved = cell["comb"]
        if resolved is None:
            if final_candidate is None:
                cell["comb"] = mark_comb_unresolved(
                    legacy_comb, "no-final-visible-band",
                    method="legacy-continuity")
            elif int(final_candidate["cells"]) < int(legacy_comb["cells"]):
                cell["comb"] = mark_comb_unresolved(
                    legacy_comb, "final-visible-count-regression",
                    "no-final-visible-owned-band",
                    method="legacy-continuity")
                cell["comb"]["resolution"]["final_visible_candidate_cells"] = int(
                    final_candidate["cells"])
            else:
                cell["comb"] = mark_comb_unresolved(
                    final_candidate, "no-final-visible-owned-band")
        elif int(resolved["cells"]) < int(legacy_comb["cells"]):
            preserved = mark_comb_unresolved(
                legacy_comb, "final-visible-count-regression",
                method="legacy-continuity")
            preserved["resolution"]["final_visible_candidate_cells"] = int(
                resolved["cells"])
            cell["comb"] = preserved

        if cell.get("geometry_resolution"):
            cell["comb"] = mark_comb_unresolved(
                cell["comb"], "uncertain-final-paint-boundary")

        topology_transition: dict[str, Any] | None = None
        if int(cell["comb"]["cells"]) == int(legacy_comb["cells"]):
            old_divider_x = sorted(
                q(float(value)) for value in legacy_comb["divider_x"])
            new_divider_x = sorted(
                q(float(value)) for value in cell["comb"]["divider_x"])
            if not same_boundary_topology(old_divider_x, new_divider_x):
                # Equal slot counts do not make two physical combs the same
                # subject. A resolved current detector is not independent
                # evidence for moving the reviewed legacy boundaries, so this
                # transition remains blocking until a referee certifies it.
                topology_transition = {
                    "old_divider_x": old_divider_x,
                    "new_divider_x": new_divider_x,
                    "comparison_tolerance_pt": CLUSTER_TOL_PT,
                    "independently_certified": False,
                }
                cell["comb"] = mark_comb_unresolved(
                    cell["comb"],
                    "same-count-boundary-topology-change")
                cell["comb"]["resolution"]["boundary_topology_transition"] = (
                    dict(topology_transition))

        resolution = cell["comb"].get("resolution") or {}
        unresolved = resolution.get("status") != "resolved"
        ledger_entry = {
            "subject_key": subject_key,
            "legacy_cell_id": subject["legacy_cell_id"],
            "legacy_bbox": subject["legacy_bbox"],
            "cell_id": cell["id"],
            "mapped_partition_cell_ids": [cell["id"]],
            "state": "active_unresolved" if unresolved else "active_resolved",
            "reason_codes": list(resolution.get("reason_codes") or ()),
            "cells": int(cell["comb"]["cells"]),
            "blocks_gate": unresolved,
        }
        if topology_transition is not None:
            ledger_entry["old_divider_x"] = list(
                topology_transition["old_divider_x"])
            ledger_entry["new_divider_x"] = list(
                topology_transition["new_divider_x"])
            ledger_entry["boundary_topology_transition"] = (
                dict(topology_transition))
        subject_ledger.append(ledger_entry)

    # A partition-only inferred subject has no reviewed predecessor. Suppress it
    # explicitly instead of silently changing the reviewed 4,442 denominator.
    for cell in cells:
        if "comb" not in cell or cell["subject_key"] in legacy_keys:
            continue
        inference_ledger.append({
            "subject_key": cell["subject_key"],
            "cell_id": cell["id"],
            "bbox": [cell["x0"], cell["y0"], cell["x1"], cell["y1"]],
            "state": "suppressed_unreviewed_inference",
            "reason_codes": ["no-legacy-subject"],
            "inferred_comb": cell["comb"],
            "requires_independent_evidence": True,
            "permitted_transitions": ["active_reviewed"],
            "blocks_gate": True,
        })
        cell.pop("comb")
        cell.pop("combs", None)

    assigned, unplaced = assign_points(
        cells, [((r["x0"] + r["x1"]) / 2.0, (r["y0"] + r["y1"]) / 2.0, index)
                for index, r in enumerate(text_runs)])
    for cell, members in zip(cells, assigned):
        cell["text_run_ids"] = [f"p{page_index}t{i}" for i in sorted(members)]
    unassigned = [f"p{page_index}t{i}" for i in sorted(unplaced)]

    for cell in cells:
        cell["is_empty"] = not cell["text_run_ids"]
        cell["kind"] = classify_cell(cell["is_empty"], cell["border_count"], "comb" in cell)
        cell.pop("_component_root")
    subject_ledger.sort(key=lambda subject: (
        subject["legacy_bbox"] is None,
        subject["legacy_bbox"] or (),
        subject["subject_key"],
    ))
    inference_ledger.sort(key=lambda inference: (
        inference["bbox"], inference["subject_key"]))
    return cells, unassigned, subject_ledger, inference_ledger


def assign_points(cells: Sequence[dict[str, Any]],
                  points: Sequence[tuple[float, float, Any]]
                  ) -> tuple[list[list[Any]], list[Any]]:
    """Give each point to exactly one cell -- the smallest one containing it.

    Cells partition the lattice, so containment is normally unambiguous. It is
    not for the handful of L-shaped merged cells, whose emitted bounding box
    necessarily overlaps a neighbour; without the smallest-area rule those
    overlaps double-count, which is how a page reported more comb slots than it
    had dividers. Area then reading order makes the choice deterministic.
    """
    order = sorted(range(len(cells)),
                   key=lambda n: ((cells[n]["x1"] - cells[n]["x0"])
                                  * (cells[n]["y1"] - cells[n]["y0"]),
                                  cells[n]["y0"], cells[n]["x0"]))
    buckets: list[list[Any]] = [[] for _ in cells]
    unplaced: list[Any] = []
    for cx, cy, payload in points:
        for n in order:
            cell = cells[n]
            if cell["x0"] <= cx <= cell["x1"] and cell["y0"] <= cy <= cell["y1"]:
                buckets[n].append(payload)
                break
        else:
            unplaced.append(payload)
    return buckets, unplaced


# ---------------------------------------------------------------------------
# Growable bands
# ---------------------------------------------------------------------------


def row_signature(v_at: list[list[bool]], row: int, columns: int) -> tuple[int, ...]:
    return tuple(i for i in range(columns) if v_at[i][row])


def column_role(texts: Sequence[str]) -> str | None:
    """How one column of a candidate band varies down the rows.

    "constant" -- every row carries the same pre-printed text (or none): the
    money decimal point, the "%" glyph, an empty comb.
    "enumerated" -- the rows carry consecutive integers: the pre-printed row
    numbers "1".."6" of Schedule 1.
    None -- the rows carry different prose, which means these are distinct
    numbered items that merely happen to be drawn on a regular pitch. Part II
    of the 2551Q (items 15-19) is exactly that shape and must NOT be growable.
    """
    stripped = [t.strip() for t in texts]
    if len(set(stripped)) == 1:
        return "constant"
    try:
        numbers = [int(t) for t in stripped]
    except ValueError:
        return None
    if all(b - a == 1 for a, b in zip(numbers, numbers[1:])):
        return "enumerated"
    return None


def detect_growables(page_index: int, xl: Lattice, yl: Lattice,
                     v_at: list[list[bool]], h_at: list[list[bool]],
                     cells: Sequence[dict[str, Any]],
                     text_runs: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    """Maximal runs of >=3 consecutive rows that are genuinely interchangeable.

    A repeating row band is the only place on a BIR form where a filer may need
    more space than the sheet gives, so this is what a generator has to be able
    to grow -- and what the on-sheet capacity is measured against.

    Identical geometry alone is not enough. Several fixed sections are drawn on
    a perfectly regular pitch with a perfectly regular column structure and are
    still not repeatable, because each row carries its own pre-printed caption.
    The content test below is what separates the two.
    """
    ny = len(yl) - 1
    signatures = [row_signature(v_at, j, len(xl)) for j in range(ny)]
    run_text = {f"p{page_index}t{i}": r["text"] for i, r in enumerate(text_runs)}
    by_position = {(c["row"], c["col"]): c for c in cells}

    growables: list[dict[str, Any]] = []
    start = 0
    while start < ny:
        signature = signatures[start]
        end = start + 1
        while end < ny and signatures[end] == signature:
            end += 1
        run = list(range(start, end))
        start = end

        if len(run) < MIN_GROWABLE_ROWS or len(signature) < MIN_GROWABLE_COLUMNS:
            continue
        i0, i1 = signature[0], signature[-1]

        # Every row must be closed top and bottom across the band's width,
        # otherwise this is a column of free space, not a stack of rows.
        if not all(h_at[j][i] for j in (*run, run[-1] + 1) for i in range(i0, i1)):
            continue

        edges = [yl.positions[j] for j in (*run, run[-1] + 1)]
        deltas = [q(b - a) for a, b in zip(edges, edges[1:])]
        if max(deltas) - min(deltas) > PITCH_TOL_PT:
            continue

        roles: dict[int, str] = {}
        for column in signature[:-1]:
            column_cells = [by_position.get((j, column)) for j in run]
            if any(c is None for c in column_cells):
                roles = {}
                break
            texts = ["".join(run_text[t] for t in c["text_run_ids"]) for c in column_cells]
            role = column_role(texts)
            if role is None:
                roles = {}
                break
            roles[column] = role
        if not roles:
            continue

        band_x0, band_x1 = xl.positions[i0], xl.positions[i1]
        band_y0, band_y1 = edges[0], edges[-1]
        in_band = [c for c in cells
                   if c["x0"] >= band_x0 - CLUSTER_TOL_PT and c["x1"] <= band_x1 + CLUSTER_TOL_PT
                   and c["y0"] >= band_y0 - CLUSTER_TOL_PT and c["y1"] <= band_y1 + CLUSTER_TOL_PT]
        template = [c["id"] for c in in_band if abs(c["y0"] - edges[0]) <= CLUSTER_TOL_PT]
        header = [c["id"] for c in cells
                  if abs(c["y1"] - band_y0) <= CLUSTER_TOL_PT
                  and c["x0"] >= band_x0 - CLUSTER_TOL_PT
                  and c["x1"] <= band_x1 + CLUSTER_TOL_PT]

        growables.append({
            "id": f"p{page_index}g{len(growables)}",
            "kind": "repeating_rows",
            "x0": band_x0, "y0": band_y0, "x1": band_x1, "y1": band_y1,
            # Modal pitch. The band is NOT perfectly regular -- on the 2551Q
            # Schedule 1 rows 1-5 are 18.24pt and row 6 is 18.27pt -- so a
            # generator must use row_y, not index * pitch.
            "row_pitch_pt": min(collections.Counter(deltas).most_common(),
                                key=lambda kv: (-kv[1], kv[0]))[0],
            "row_pitch_min_pt": min(deltas),
            "row_pitch_max_pt": max(deltas),
            "row_count": len(run),
            "row_y": edges,
            "column_x": [xl.positions[i] for i in signature],
            "column_index": list(signature),
            "column_roles": [roles[i] for i in signature[:-1]],
            "header_cell_ids": sorted(header),
            "template_cell_ids": sorted(template),
            "cell_ids": sorted(c["id"] for c in in_band),
            # On-sheet capacity. Overflow beyond this is a continuation sheet,
            # not a taller table.
            "capacity": len(run),
        })
    return growables


# ---------------------------------------------------------------------------
# Regions
# ---------------------------------------------------------------------------


def detect_regions(page_index: int, xl: Lattice, yl: Lattice, v_at: list[list[bool]],
                   cells: Sequence[dict[str, Any]],
                   growables: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    """Maximal runs of rows sharing one left/right enclosure.

    This recovers the boxes a reader sees as units -- a titled schedule, a
    Part -- because a BIR box keeps the same outer verticals for its whole
    height and the gap before the next box carries none.
    """
    ny = len(yl) - 1
    extents: list[tuple[int, int] | None] = []
    for j in range(ny):
        signature = row_signature(v_at, j, len(xl))
        extents.append((signature[0], signature[-1]) if len(signature) >= 2 else None)

    regions: list[dict[str, Any]] = []
    start = 0
    while start < ny:
        extent = extents[start]
        end = start + 1
        while end < ny and extents[end] == extent:
            end += 1
        run_start, run_end, start = start, end, end
        if extent is None:
            continue

        x0, x1 = xl.positions[extent[0]], xl.positions[extent[1]]
        y0, y1 = yl.positions[run_start], yl.positions[run_end]
        in_region = [c for c in cells
                     if c["y0"] >= y0 - CLUSTER_TOL_PT and c["y1"] <= y1 + CLUSTER_TOL_PT
                     and c["x0"] >= x0 - CLUSTER_TOL_PT and c["x1"] <= x1 + CLUSTER_TOL_PT]
        if not in_region:
            continue
        holds_growable = any(g["y0"] >= y0 - CLUSTER_TOL_PT and g["y1"] <= y1 + CLUSTER_TOL_PT
                             for g in growables)
        kind = "table" if holds_growable else ("band" if run_end - run_start == 1 else "block")
        regions.append({
            "id": f"p{page_index}r{len(regions)}",
            "kind": kind,
            "x0": x0, "y0": y0, "x1": x1, "y1": y1,
            "row_count": run_end - run_start,
            "cell_ids": sorted(c["id"] for c in in_region),
        })
    return regions


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------


def build_page(page: dict[str, Any]) -> dict[str, Any]:
    index = page["index"]
    rules = page["rules"]
    final_paint = FinalPaint([*rules, *page["area_fills"], *page["paths"]])
    raw_structural = [rule for rule in rules if rule["role"] == "structural"]
    raw_horizontals = sorted(
        (rule for rule in raw_structural if rule["axis"] == "h"),
        key=lambda rule: (rule["y0"], rule["x0"]))
    raw_verticals = sorted(
        (rule for rule in raw_structural if rule["axis"] == "v"),
        key=lambda rule: (rule["x0"], rule["y0"]))
    raw_dividers, _raw_borders = split_verticals(
        raw_verticals, raw_horizontals)
    raw_extra_ink = comb_boundary_candidates(
        raw_verticals, page["area_fills"])
    raw_xl = build_lattice(_raw_borders, raw_verticals, "v")
    raw_yl = build_lattice(raw_horizontals, raw_horizontals, "h")
    if len(raw_xl) >= 2 and len(raw_yl) >= 2:
        raw_dsu, raw_v_at, raw_h_at = merge_grid(raw_xl, raw_yl)
    else:
        raw_dsu = DisjointSet(1)
        raw_v_at = raw_h_at = []
    proven_structural = [
        rule for rule in rules
        if rule["role"] == "structural"
        and final_paint.structural_across_axis(
            rule,
            float(rule["y0"] if rule["axis"] == "v" else rule["x0"]),
            float(rule["y1"] if rule["axis"] == "v" else rule["x1"]),
            str(rule["axis"]),
        )
    ]
    proven_ids = {str(rule.get("id")) for rule in proven_structural}
    uncertain_structural = [
        rule for rule in raw_structural
        if str(rule.get("id")) not in proven_ids
        and not final_paint.definitely_erased(rule)
    ]
    surviving_ids = {
        str(rule.get("id"))
        for rule in [*proven_structural, *uncertain_structural]
    }
    # If one bar of a fused composite boundary survives, retain the complete
    # measured stack as unresolved geometry so its published centre/IDs do not
    # shift. An isolated erased rule (the adversarial erased baseline) has no
    # surviving boundary mate and remains absent.
    companion_ids: set[str] = set()
    for defining, all_ink, axis in (
        (_raw_borders, raw_verticals, "v"),
        (raw_horizontals, raw_horizontals, "h"),
    ):
        groups = [
            GroupGeometry(group, all_ink, axis)
            for group in cluster_collinear(defining)
        ]
        for boundary in fuse_boundaries(groups):
            boundary_ids = {
                str(rule.get("id"))
                for group in boundary
                for rule in group.rules
            }
            if boundary_ids & surviving_ids:
                companion_ids.update(boundary_ids)
    geometry_ids = surviving_ids | companion_ids
    geometry_structural = [
        rule for rule in raw_structural
        if str(rule.get("id")) in geometry_ids
    ]
    uncertain_ids = geometry_ids - proven_ids
    # Grey ornament. It must be painted, but never as a black border -- the
    # raster-era mistake this project has already paid for once.
    decorative = [r for r in rules if r["role"] == "decorative"]

    horizontals = sorted(
        (rule for rule in proven_structural if rule["axis"] == "h"),
        key=lambda rule: (rule["y0"], rule["x0"]))
    verticals = sorted(
        (rule for rule in proven_structural if rule["axis"] == "v"),
        key=lambda rule: (rule["x0"], rule["y0"]))
    geometry_horizontals = sorted(
        (rule for rule in geometry_structural if rule["axis"] == "h"),
        key=lambda rule: (rule["y0"], rule["x0"]))
    geometry_verticals = sorted(
        (rule for rule in geometry_structural if rule["axis"] == "v"),
        key=lambda rule: (rule["x0"], rule["y0"]))
    dividers, _proven_borders = split_verticals(verticals, horizontals)
    _geometry_dividers, borders = split_verticals(
        geometry_verticals, horizontals)
    support_dividers, _unsupported_verticals = split_verticals(
        raw_verticals, geometry_horizontals)
    final_supported_divider_ids = {
        str(divider.get("id")) for divider in support_dividers
    }
    final_area_fills = [
        fill for fill in page["area_fills"]
        if fill["role"] == "structural"
        and final_paint.structural_across_axis(
            fill, float(fill["y0"]), float(fill["y1"]), "v")
    ]
    extra_ink = comb_boundary_candidates(verticals, final_area_fills)

    xl = build_lattice(borders, geometry_verticals, "v")
    yl = build_lattice(
        geometry_horizontals, geometry_horizontals, "h")

    if len(xl) < 2 or len(yl) < 2:
        cells: list[dict[str, Any]] = []
        unassigned = [f"p{index}t{i}" for i in range(len(page["text_runs"]))]
        growables: list[dict[str, Any]] = []
        regions: list[dict[str, Any]] = []
        comb_subjects: list[dict[str, Any]] = []
        comb_inferences: list[dict[str, Any]] = []
        v_at = h_at = []
    else:
        dsu, v_at, h_at = merge_grid(xl, yl)
        cells, unassigned, comb_subjects, comb_inferences = build_cells(
            index, xl, yl, dsu, v_at, h_at, geometry_verticals,
            geometry_horizontals, dividers, extra_ink, final_paint,
            page["text_runs"],
            legacy_dividers=raw_dividers,
            legacy_extra_ink=raw_extra_ink,
            final_supported_divider_ids=final_supported_divider_ids,
            frame_dividers=support_dividers,
            legacy_xl=raw_xl,
            legacy_yl=raw_yl,
            legacy_dsu=raw_dsu,
            legacy_v_at=raw_v_at,
            legacy_h_at=raw_h_at,
            legacy_v_ink=raw_verticals,
            legacy_h_ink=raw_horizontals,
            uncertain_geometry_ids=uncertain_ids)
        growables = detect_growables(index, xl, yl, v_at, h_at, cells, page["text_runs"])
        regions = detect_regions(index, xl, yl, v_at, cells, growables)

    comb_cells = [c for c in cells if "comb" in c]
    return {
        "index": index,
        "width_pt": page["width_pt"],
        "height_pt": page["height_pt"],
        "rotation": page["rotation"],
        "x_lattice": xl.positions,
        "y_lattice": yl.positions,
        "cells": cells,
        "comb_subjects": comb_subjects,
        "comb_inferences": comb_inferences,
        "regions": regions,
        "growable": growables,
        "decorative_rules": decorative,
        # Support classification is final-visible even when the divider's own
        # merged paint range is uncertain. Keep the established ID inventory;
        # the fully final-visible subset is explicit beside it.
        "comb_divider_ids": [d["id"] for d in support_dividers],
        "comb_divider_final_visible_ids": [d["id"] for d in dividers],
        "unassigned_text_run_ids": unassigned,
        "stats": {
            "x_lattice": len(xl),
            "y_lattice": len(yl),
            "cells": len(cells),
            "cells_non_rectangular": sum(1 for c in cells if not c["rectangular"]),
            "cells_geometry_unresolved": sum(
                bool(c.get("geometry_resolution")) for c in cells),
            "regions": len(regions),
            "growables": len(growables),
            "comb_cells": len(comb_cells),
            "comb_subjects": len(comb_subjects),
            "comb_subjects_active": sum(
                subject["state"].startswith("active_")
                for subject in comb_subjects),
            "comb_subjects_active_resolved": sum(
                subject["state"] == "active_resolved"
                for subject in comb_subjects),
            "comb_subjects_active_unresolved": sum(
                subject["state"] == "active_unresolved"
                for subject in comb_subjects),
            "comb_subjects_retained_unresolved": sum(
                subject["state"] == "retained_unresolved"
                for subject in comb_subjects),
            "comb_subjects_retired": 0,
            "comb_subjects_blocking": sum(
                bool(subject.get("blocks_gate"))
                for subject in comb_subjects),
            "comb_inferences_suppressed": len(comb_inferences),
            "comb_inferences_blocking": sum(
                bool(inference.get("blocks_gate"))
                for inference in comb_inferences),
            "comb_evidence_blocking": (
                sum(bool(subject.get("blocks_gate"))
                    for subject in comb_subjects)
                + sum(bool(inference.get("blocks_gate"))
                      for inference in comb_inferences)
            ),
            "comb_slots": sum(c["comb"]["cells"] for c in comb_cells),
            "comb_dividers": len(support_dividers),
            "comb_dividers_final_visible": len(dividers),
            "border_verticals": len(borders),
            "decorative_rules": len(decorative),
            "text_runs": len(page["text_runs"]),
            "text_runs_unassigned": len(unassigned),
            "cell_kinds": dict(sorted(collections.Counter(c["kind"] for c in cells).items())),
        },
    }


def build_layout(ir: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "form": ir["form"],
        "source": ir["source"],
        "generator": {
            "producer": "tools/formgen/lattice.py",
            "schema_version": SCHEMA_VERSION,
            "consumes_ir_schema_version": ir["schema_version"],
            "cluster_tolerance_pt": CLUSTER_TOL_PT,
            "pitch_tolerance_pt": PITCH_TOL_PT,
        },
        "paper": ir["paper"],
        "pages": [build_page(p) for p in ir["pages"]],
    }


# ---------------------------------------------------------------------------
# Self-test
# ---------------------------------------------------------------------------


def self_test(ir_path: pathlib.Path) -> int:
    """Assert against the real 2551Q, not against a synthetic fixture."""
    ir = json.loads(ir_path.read_text(encoding="utf-8"))
    layout = build_layout(ir)
    failures: list[str] = []

    def check(condition: bool, message: str) -> None:
        if not condition:
            failures.append(message)

    def synthetic_vertical(x: float, y0: float, y1: float,
                           thickness: float, sequence: int,
                           role: str = "structural") -> dict[str, Any]:
        return {
            "axis": "v",
            "x0": q(x - thickness / 2), "x1": q(x + thickness / 2),
            "y0": q(y0), "y1": q(y1),
            "thickness_pt": q(thickness),
            "gray": 0.0 if role == "structural" else 1.0,
            "role": role,
            "paint_seq": sequence, "paint_seq_max": sequence,
        }

    def synthetic_horizontal(y: float, x0: float, x1: float,
                             thickness: float, sequence: int,
                             role: str = "structural") -> dict[str, Any]:
        return {
            "axis": "h",
            "x0": q(x0), "x1": q(x1),
            "y0": q(y - thickness / 2), "y1": q(y + thickness / 2),
            "thickness_pt": q(thickness),
            "gray": 0.0 if role == "structural" else 1.0,
            "role": role,
            "paint_seq": sequence, "paint_seq_max": sequence,
        }

    # Endpoint slabs: three heavy group separators are slightly shorter than
    # the thin seed ticks, and a fourth raw mark is knocked out later. The
    # common final-visible slab must include the former and exclude the latter.
    thin_a = synthetic_vertical(10, 0, 10, 0.2, 1)
    thin_b = synthetic_vertical(20, 0, 10, 0.2, 2)
    heavy = synthetic_vertical(15, 0.5, 10, 2.2, 3)
    stale = synthetic_vertical(25, 0.5, 10, 0.2, 4)
    knockout = {
        **stale,
        "role": "knockout", "gray": 1.0,
        "paint_seq": 5, "paint_seq_max": 5,
    }
    synthetic_paint = FinalPaint([thin_a, thin_b, heavy, stale, knockout])
    endpoint = endpoint_band(
        [thin_a, thin_b], [thin_a, thin_b, heavy, stale],
        0, 30, [(-0.5, 0.5, 1.0), (29.5, 30.5, 1.0)],
        synthetic_paint)
    check(endpoint is not None, "endpoint-slab comb topology was not found")
    if endpoint is not None:
        endpoint_ink, endpoint_y0, endpoint_y1, _topologies = endpoint
        check([q(centre(ink)) for ink in endpoint_ink] == [10.0, 15.0, 20.0],
              "endpoint-slab topology did not add heavy/drop stale boundaries")
        check(q(endpoint_y0) == 0.5 and q(endpoint_y1) == 10.0,
              f"endpoint-slab common intersection {endpoint_y0}..{endpoint_y1}")

    # Y coverage is not enough to prove a divider. Erase the seed, then paint
    # disjoint left/right strips in opposite half-bands: there is structural
    # ink in every y slab, but no x corridor survives through the full height.
    corridor_seed = synthetic_vertical(10, 0, 10, 0.2, 1)
    corridor_knockout = {
        **corridor_seed,
        "role": "knockout", "gray": 1.0,
        "paint_seq": 2, "paint_seq_max": 2,
    }
    left_strip = {
        **corridor_seed,
        "x0": 9.9, "x1": 9.98, "y0": 0.0, "y1": 5.0,
        "thickness_pt": 0.08,
        "paint_seq": 3, "paint_seq_max": 3,
    }
    right_strip = {
        **corridor_seed,
        "x0": 10.02, "x1": 10.1, "y0": 5.0, "y1": 10.0,
        "thickness_pt": 0.08,
        "paint_seq": 4, "paint_seq_max": 4,
    }
    corridor_paint = FinalPaint([
        corridor_seed, corridor_knockout, left_strip, right_strip,
    ])
    check(corridor_paint.visible_intervals(corridor_seed)
          == [(0.0, 5.0), (5.0, 10.0)],
          "disjoint x corridors were merged into one visible y interval")
    check(not corridor_paint.structural_across(corridor_seed, 0.0, 10.0),
          "disjoint x corridors certified a continuous divider")
    corridor_bands = comb_bands(
        [corridor_seed], [corridor_seed], 0.0, 20.0, (0.2, 0.2),
        corridor_paint)
    check(not any(
        band["cells"] == 2
        and band["resolution"]["status"] == "resolved"
        for band in corridor_bands),
        "disjoint x corridors emitted a resolved two-cell comb")

    # A merged rule spanning source ordinals 1..100 does not prove that all of
    # its geometry was painted after an intervening seq-50 knockout.
    ranged_seed = {
        **corridor_seed,
        "paint_seq": 1, "paint_seq_max": 100,
    }
    middle_knockout = {
        **corridor_knockout,
        "paint_seq": 50, "paint_seq_max": 50,
    }
    ranged_paint = FinalPaint([ranged_seed, middle_knockout])
    check(paint_ordinal_range(ranged_seed) == (1, 100),
          "merged paint source-order range was not preserved")
    check(not ranged_paint.visible_intervals(ranged_seed),
          "max-only paint ordering revived an uncertain merged rule")
    check(not ranged_paint.structural_across(ranged_seed, 0.0, 10.0),
          "interleaved paint range certified a continuous divider")
    ranged_bands = comb_bands(
        [ranged_seed], [ranged_seed], 0.0, 20.0, (0.2, 0.2),
        ranged_paint)
    check(not any(
        band["cells"] == 2
        and band["resolution"]["status"] == "resolved"
        for band in ranged_bands),
        "interleaved paint range emitted a resolved two-cell comb")

    # Support and frame geometry must also be final-visible. A vertical tick
    # cannot become a comb merely because its erased raw baseline still exists
    # in the IR, and that erased baseline cannot enter the y lattice.
    erased_baseline = synthetic_horizontal(10, 0, 20, 0.2, 1)
    baseline_knockout = {
        **erased_baseline,
        "role": "knockout", "gray": 1.0,
        "paint_seq": 2, "paint_seq_max": 2,
    }
    unsupported_tick = synthetic_vertical(10, 0, 10, 0.2, 3)
    support_paint = FinalPaint([
        erased_baseline, baseline_knockout, unsupported_tick,
    ])
    check(not support_paint.structural_across_axis(
        erased_baseline, 0.0, 20.0, "h"),
        "an erased horizontal remained final-visible")
    final_support = [
        horizontal for horizontal in [erased_baseline]
        if support_paint.structural_across_axis(
            horizontal, horizontal["x0"], horizontal["x1"], "h")
    ]
    supported_ticks, unsupported_borders = split_verticals(
        [unsupported_tick], final_support)
    check(not final_support and not supported_ticks
          and unsupported_borders == [unsupported_tick],
          "raw erased baseline still classified a comb divider")
    unsupported_bands = comb_bands(
        supported_ticks, [unsupported_tick],
        0.0, 20.0, (0.2, 0.2), support_paint)
    check(not unsupported_bands,
          "tick with no final-visible support emitted a comb")

    # Coverage/density is not source ownership. The shorter lower divider makes
    # a second plausible topology; retain both as evidence and fail closed.
    topology_left = synthetic_vertical(10, 0, 10, 0.2, 10)
    topology_right = synthetic_vertical(20, 0, 10, 0.2, 11)
    topology_middle = synthetic_vertical(15, 6, 10, 0.2, 12)
    topology_paint = FinalPaint([
        topology_left, topology_right, topology_middle,
    ])
    topology_bands = comb_bands(
        [topology_left, topology_right],
        [topology_left, topology_right, topology_middle],
        0.0, 30.0, (0.2, 0.2), topology_paint)
    check(bool(topology_bands),
          "competing endpoint topologies produced no retained subject")
    if topology_bands:
        topology_resolution = topology_bands[0]["resolution"]
        carried = {
            tuple(item["divider_x"])
            for item in topology_resolution.get("endpoint_topologies") or ()
        }
        check(topology_resolution["status"] == "unresolved"
              and "competing-endpoint-topologies"
              in topology_resolution["reason_codes"],
              "coverage winner silently resolved competing endpoint topologies")
        check(carried == {(10.0, 20.0), (10.0, 15.0, 20.0)},
              f"competing endpoint topology evidence was lost: {carried}")
    check(boundary_topology_subset(
        [10.2, 20.0], [10.0, 20.0, 30.0]),
        "near-identical physical boundaries were not matched as a subset")
    check(not boundary_topology_subset(
        [10.0, 10.1], [10.0, 20.0, 30.0]),
        "two topology values reused one physical boundary")
    check(same_boundary_topology(
        [10.0, 20.0], [10.3, 19.7]),
        "exact clustering-tolerance boundary drift was not treated as equal")
    check(not same_boundary_topology(
        [10.0, 20.0], [10.31, 19.7]),
        "outside-tolerance boundary drift was treated as equal")

    # Slot count alone cannot activate a changed legacy subject. Exercise the
    # exact build_cells transition path: both combs have three slots, but the
    # reviewed dividers at 10/20 moved to 8/22 in the current detector.
    ledger_left = {
        **synthetic_vertical(0, 0, 10, 0.2, 40),
        "id": "ledger-left",
    }
    ledger_right = {
        **synthetic_vertical(30, 0, 10, 0.2, 41),
        "id": "ledger-right",
    }
    ledger_top = {
        **synthetic_horizontal(0, 0, 30, 0.2, 42),
        "id": "ledger-top",
    }
    ledger_bottom = {
        **synthetic_horizontal(10, 0, 30, 0.2, 43),
        "id": "ledger-bottom",
    }
    current_ledger_dividers = [
        {
            **synthetic_vertical(x, 5, 10, 0.2, 44 + index),
            "id": f"current-ledger-divider-{index}",
        }
        for index, x in enumerate((8.0, 22.0))
    ]
    legacy_ledger_dividers = [
        {
            **synthetic_vertical(x, 5, 10, 0.2, 46 + index),
            "id": f"legacy-ledger-divider-{index}",
        }
        for index, x in enumerate((10.0, 20.0))
    ]
    ledger_x = Lattice(
        [0.0, 30.0], [-0.1, 29.9], [0.1, 30.1],
        [[(0.0, 10.0)], [(0.0, 10.0)]],
        [[ledger_left], [ledger_right]],
    )
    ledger_y = Lattice(
        [0.0, 10.0], [-0.1, 9.9], [0.1, 10.1],
        [[(0.0, 30.0)], [(0.0, 30.0)]],
        [[ledger_top], [ledger_bottom]],
    )
    ledger_cells, _ledger_text, ledger_subjects, _ledger_inferences = (
        build_cells(
            1, ledger_x, ledger_y, DisjointSet(1),
            [[True], [True]], [[True], [True]],
            [ledger_left, ledger_right], [ledger_top, ledger_bottom],
            current_ledger_dividers, current_ledger_dividers,
            FinalPaint([
                ledger_left, ledger_right, ledger_top, ledger_bottom,
                *current_ledger_dividers,
            ]),
            [],
            legacy_dividers=legacy_ledger_dividers,
            legacy_extra_ink=legacy_ledger_dividers,
        )
    )
    expected_topology_transition = {
        "old_divider_x": [10.0, 20.0],
        "new_divider_x": [8.0, 22.0],
        "comparison_tolerance_pt": CLUSTER_TOL_PT,
        "independently_certified": False,
    }
    ledger_comb_resolution = (
        ledger_cells[0].get("comb", {}).get("resolution", {})
        if len(ledger_cells) == 1 else {}
    )
    check(
        len(ledger_cells) == 1
        and ledger_comb_resolution.get("status") == "unresolved"
        and "same-count-boundary-topology-change"
        in ledger_comb_resolution.get("reason_codes", [])
        and ledger_comb_resolution.get("boundary_topology_transition")
        == expected_topology_transition,
        "same-count topology drift remained a resolved current comb",
    )
    check(
        len(ledger_subjects) == 1
        and ledger_subjects[0].get("state") == "active_unresolved"
        and ledger_subjects[0].get("blocks_gate") is True
        and ledger_subjects[0].get("old_divider_x") == [10.0, 20.0]
        and ledger_subjects[0].get("new_divider_x") == [8.0, 22.0]
        and ledger_subjects[0].get("boundary_topology_transition")
        == expected_topology_transition,
        f"same-count topology drift did not block the subject ledger: "
        f"{ledger_subjects}",
    )

    # A thick group divider can paint a short horizontal endpoint cap without
    # producing an internal vertical lattice edge. That cap is safe only when
    # its complete crossed adjacency lies inside the divider's ink corridor.
    cap_left = synthetic_vertical(0, 0, 10, 0.2, 60)
    cap_right = synthetic_vertical(30, 0, 10, 0.2, 61)
    cap_top = synthetic_horizontal(0, 0, 30, 0.2, 62)
    cap_bottom = synthetic_horizontal(10, 0, 30, 0.2, 63)
    cap_divider = synthetic_vertical(15, 5, 10, 2.0, 64)
    cap_rule = synthetic_horizontal(5, 14, 16, 0.2, 65)
    cap_x = Lattice(
        [0.0, 14.0, 16.0, 30.0],
        [-0.1, 13.9, 15.9, 29.9],
        [0.1, 14.1, 16.1, 30.1],
        [[(0.0, 10.0)], [], [], [(0.0, 10.0)]],
        [[cap_left], [], [], [cap_right]],
    )
    cap_y = Lattice(
        [0.0, 5.0, 10.0],
        [-0.1, 4.9, 9.9],
        [0.1, 5.1, 10.1],
        [[(0.0, 30.0)], [(14.0, 16.0)], [(0.0, 30.0)]],
        [[cap_top], [cap_rule], [cap_bottom]],
    )
    cap_box = {
        "j0": 0, "j1": 2, "i0": 0, "i1": 3,
        "component_root": 0, "rectangular": True,
    }
    cap_v_at = [
        [True, True], [False, False], [False, False], [True, True],
    ]
    cap_h_at = [
        [True, True, True], [False, True, False], [True, True, True],
    ]
    cap_paint = FinalPaint([
        cap_left, cap_right, cap_top, cap_bottom, cap_divider, cap_rule,
    ])
    cap_certificate = source_owned_comb_frame(
        cap_box, cap_x, cap_y, cap_v_at, cap_h_at,
        [cap_divider], [cap_divider], cap_paint)
    check(cap_certificate is not None,
          "a fully divider-owned horizontal endpoint cap was not certified")
    cap_h_at[1] = [True, True, False]
    check(source_owned_comb_frame(
        cap_box, cap_x, cap_y, cap_v_at, cap_h_at,
        [cap_divider], [cap_divider], cap_paint) is None,
        "a horizontal edge spanning slot paper was accepted as a comb cap")

    # A short comb tick sharing x with a longer separator does not own the long
    # ink. The grid row carrying the edge overlaps the band, so the old
    # row-interval test passed; the final-visible extent must reject it.
    extent_left = synthetic_vertical(0, 0, 10, 0.2, 70)
    extent_right = synthetic_vertical(30, 0, 10, 0.2, 71)
    extent_top = synthetic_horizontal(0, 0, 30, 0.2, 72)
    extent_bottom = synthetic_horizontal(10, 0, 30, 0.2, 73)
    long_separator = synthetic_vertical(15, 0, 8, 0.2, 74)
    short_tick = synthetic_vertical(15, 7, 10, 0.2, 75)
    extent_x = Lattice(
        [0.0, 15.0, 30.0],
        [-0.1, 14.9, 29.9], [0.1, 15.1, 30.1],
        [[(0.0, 10.0)], [(0.0, 10.0)], [(0.0, 10.0)]],
        [[extent_left], [long_separator, short_tick], [extent_right]],
    )
    extent_y = Lattice(
        [0.0, 7.0, 9.0, 10.0],
        [-0.1, 6.9, 8.0, 9.9], [0.1, 7.1, 10.0, 10.1],
        [[(0.0, 30.0)], [], [], [(0.0, 30.0)]],
        [[extent_top], [], [], [extent_bottom]],
    )
    extent_v_at = [
        [True, True, True], [False, True, False], [True, True, True],
    ]
    extent_h_at = [
        [True, True], [False, False], [False, False], [True, True],
    ]
    extent_paint = FinalPaint([
        extent_left, extent_right, extent_top, extent_bottom,
        long_separator, short_tick,
    ])
    check(source_owned_comb_frame(
        {
            "j0": 0, "j1": 3, "i0": 0, "i1": 2,
            "component_root": 0, "rectangular": True,
        },
        extent_x, extent_y, extent_v_at, extent_h_at,
        [short_tick], [long_separator, short_tick], extent_paint) is None,
        "same-x long separator escaped the certified comb band")
    short_overhang = synthetic_vertical(15, 6.8, 8, 0.2, 76)
    overhang_x = Lattice(
        [0.0, 15.0, 30.0],
        [-0.1, 14.9, 29.9], [0.1, 15.1, 30.1],
        [[(0.0, 10.0)], [(6.8, 10.0)], [(0.0, 10.0)]],
        [[extent_left], [short_overhang, short_tick], [extent_right]],
    )
    overhang_paint = FinalPaint([
        extent_left, extent_right, extent_top, extent_bottom,
        short_overhang, short_tick,
    ])
    check(source_owned_comb_frame(
        {
            "j0": 0, "j1": 3, "i0": 0, "i1": 2,
            "component_root": 0, "rectangular": True,
        },
        overhang_x, extent_y, extent_v_at, extent_h_at,
        [short_tick], [short_overhang, short_tick],
        overhang_paint) is not None,
        "one-weight square divider cap was not attributed to its comb")

    # A non-rectangular component is a partition of row runs, never its broad
    # bounding box. A visible internal vertical also splits a single row even
    # when the DSU component reconnects elsewhere.
    empty_v = [[False, False] for _ in range(4)]
    empty_h = [[False, False, False] for _ in range(3)]
    l_shape = rectangular_row_runs(
        [(0, 0), (0, 1), (0, 2), (1, 0)], empty_v, empty_h)
    check(l_shape == [
        {"j0": 0, "j1": 1, "i0": 0, "i1": 3, "rectangular": True},
        {"j0": 1, "j1": 2, "i0": 0, "i1": 1, "rectangular": True},
    ], f"non-rectangular row-run partition is wrong: {l_shape}")
    split_v = [[False], [True], [False]]
    split_row = rectangular_row_runs([(0, 0), (0, 1)], split_v,
                                     [[False, False], [False, False]])
    check(split_row == [
        {"j0": 0, "j1": 1, "i0": 0, "i1": 1, "rectangular": True},
        {"j0": 0, "j1": 1, "i0": 1, "i1": 2, "rectangular": True},
    ], f"row-run partition crossed a painted vertical: {split_row}")

    # A component can occupy a full rectangular 2x2 bbox and still reconnect
    # around a painted partial separator. build_cells itself must partition it.
    test_verticals = [
        synthetic_vertical(x, 0, 20, 0.2, 100 + index)
        for index, x in enumerate((0.0, 10.0, 20.0))
    ]
    test_horizontals = [
        synthetic_horizontal(y, 0, 20, 0.2, 110 + index)
        for index, y in enumerate((0.0, 10.0, 20.0))
    ]
    partition_x = Lattice(
        [0.0, 10.0, 20.0], [-0.1, 9.9, 19.9], [0.1, 10.1, 20.1],
        [[(0.0, 20.0)] for _ in range(3)],
        [[rule] for rule in test_verticals])
    partition_y = Lattice(
        [0.0, 10.0, 20.0], [-0.1, 9.9, 19.9], [0.1, 10.1, 20.1],
        [[(0.0, 20.0)] for _ in range(3)],
        [[rule] for rule in test_horizontals])
    partition_v_at = [
        [True, True],
        [True, False],
        [True, True],
    ]
    partition_h_at = [
        [True, True],
        [False, False],
        [True, True],
    ]
    partition_dsu = DisjointSet(4)
    partition_dsu.union(0, 2)
    partition_dsu.union(2, 3)
    partition_dsu.union(3, 1)
    # This looks like a divider candidate, but it lands on the internal seam
    # rather than the outer frame baseline. It must exercise and fail the
    # framed-comb preservation certificate, not bypass it via dividers=[].
    incomplete_frame_divider = synthetic_vertical(10, 0, 10, 0.2, 120)
    partition_cells, _texts, _subjects, _inferences = build_cells(
        1, partition_x, partition_y, partition_dsu,
        partition_v_at, partition_h_at,
        test_verticals, test_horizontals,
        [incomplete_frame_divider], [incomplete_frame_divider],
        FinalPaint([
            *test_verticals, *test_horizontals, incomplete_frame_divider,
        ]), [])
    check([
        (cell["x0"], cell["y0"], cell["x1"], cell["y1"])
        for cell in partition_cells
    ] == [
        (0.0, 0.0, 10.0, 10.0),
        (10.0, 0.0, 20.0, 10.0),
        (0.0, 10.0, 20.0, 20.0),
    ], f"build_cells crossed a partial painted separator: {partition_cells}")
    check(not any(crosses_painted_internal_edge({
        "j0": cell["row"], "j1": cell["row"] + cell["row_span"],
        "i0": cell["col"], "i1": cell["col"] + cell["col_span"],
    }, partition_v_at, partition_h_at) for cell in partition_cells),
          "an emitted build_cells rectangle crosses painted internal ink")

    # Painted-bound ownership admits a baseline-crossing tick to its row but
    # refuses a band contained wholly in the shared boundary ink of two rows.
    test_x = Lattice([0.0, 30.0], [-0.5, 29.5], [0.5, 30.5],
                     [[], []], [[], []])
    test_y = Lattice([0.0, 10.0, 20.0], [-0.5, 9.5, 19.5],
                     [0.5, 10.5, 20.5], [[], [], []], [[], [], []])
    owner_cells = [
        {"x0": 0.0, "y0": 0.0, "x1": 30.0, "y1": 10.0,
         "row": 0, "col": 0, "row_span": 1, "col_span": 1},
        {"x0": 0.0, "y0": 10.0, "x1": 30.0, "y1": 20.0,
         "row": 1, "col": 0, "row_span": 1, "col_span": 1},
    ]
    baseline_tick = synthetic_vertical(15, 5, 10.5, 0.2, 10)
    shared_tick = synthetic_vertical(15, 9.6, 10.4, 0.2, 11)
    owner_paint = FinalPaint([baseline_tick, shared_tick])
    buckets, _unplaced, ambiguous = assign_comb_anchors(
        owner_cells, [baseline_tick, shared_tick], test_x, test_y, owner_paint)
    check(buckets == [[baseline_tick], []],
          f"painted-bound anchor ownership is wrong: {buckets}")
    check(ambiguous == [shared_tick],
          "shared-boundary anchor was guessed instead of left ambiguous")
    check(comb_band_owners(
        owner_cells, 0.0, 30.0, 9.6, 10.4, test_x, test_y) == [0, 1],
        "shared final-visible band did not retain both possible owners")

    # Outlined glyphs can contribute rectilinear stems that look exactly like
    # hanging ticks. Their curved path continuing above the tick disqualifies
    # the subject without knowing a form code or a glyph.
    glyph_band = {
        "y0": 7.0, "y1": 10.0,
        "divider_x": [15.0], "divider_thicknesses_pt": [0.2],
        "divider_paint_seq": [10],
    }
    glyph_path = {
        "id": "glyph-path",
        "x0": 14.95, "x1": 15.05, "y0": 2.0, "y1": 7.05,
        "fill": [0.0, 0.0, 0.0], "fill_gray": 0.0,
        "stroke": None, "stroke_gray": None, "stroke_width_pt": 0.0,
        "even_odd": False, "role": "structural",
        "paint_seq": 11, "paint_seq_max": 11,
        "subpaths": [{
            "start": [14.95, 2.0], "closed": True,
            "ops": [
                {"op": "l", "points": [15.05, 2.0]},
                {"op": "l", "points": [15.05, 7.05]},
                {"op": "l", "points": [14.95, 7.05]},
                {"op": "l", "points": [14.95, 2.0]},
            ],
        }],
    }
    check(path_endpoint_conflicts(FinalPaint([glyph_path]), glyph_band),
          "non-rectilinear glyph continuation was not flagged unresolved")
    earlier_path = {
        **glyph_path,
        "paint_seq": 9,
        "paint_seq_max": 9,
    }
    check(not path_endpoint_conflicts(FinalPaint([earlier_path]), glyph_band),
          "an earlier path incorrectly overruled a later divider")
    compartment_path = {
        **glyph_path,
        "id": "compartment-path",
        # Its bbox reaches the divider corridor, but the actual triangle stays
        # to the right. A bbox-only ownership test gets this wrong.
        "x0": 14.9, "x1": 20.0,
        "subpaths": [{
            "start": [16.0, 2.0], "closed": True,
            "ops": [
                {"op": "l", "points": [20.0, 2.0]},
                {"op": "l", "points": [20.0, 7.05]},
                {"op": "l", "points": [16.0, 2.0]},
            ],
        }],
    }
    check(not path_endpoint_conflicts(
        FinalPaint([compartment_path]), glyph_band),
        "outlined content inside a slot was mistaken for a divider continuation")
    path_knockout = {
        **glyph_path,
        "id": "path-knockout",
        "fill": [1.0, 1.0, 1.0],
        "fill_gray": 1.0,
        "role": "knockout",
        "paint_seq": 12,
        "paint_seq_max": 12,
    }
    path_target = synthetic_vertical(15, 2, 7, 0.1, 1)
    check(not FinalPaint([path_target, path_knockout]).visible_intervals(path_target),
          "a later nonrect path knockout did not remove stale structural ink")
    hole_target = synthetic_vertical(5, 0, 10, 0.8, 1)
    compound_knockout = {
        "id": "compound-knockout",
        "x0": 0.0, "x1": 10.0, "y0": 0.0, "y1": 10.0,
        "fill": [1.0, 1.0, 1.0], "fill_gray": 1.0,
        "stroke": None, "stroke_gray": None, "stroke_width_pt": 0.0,
        "even_odd": True, "role": "knockout",
        "paint_seq": 2, "paint_seq_max": 2,
        "subpaths": [
            {
                "start": [0.0, 0.0], "closed": True,
                "ops": [{"op": "re", "points": [0.0, 0.0, 10.0, 10.0]}],
            },
            {
                "start": [4.8, 1.0], "closed": True,
                "ops": [{"op": "re", "points": [4.8, 1.0, 5.2, 3.0]}],
            },
        ],
    }
    compound_paint = FinalPaint([hole_target, compound_knockout])
    compound_layer = compound_paint.path_paints[0]
    old_samples = [
        (4.6, 0.0), (5.4, 0.0), (5.4, 10.0), (4.6, 10.0),
        (5.0, 5.0),
    ]
    check(all(point_in_path(compound_layer, point)
              for point in old_samples)
          and not point_in_path(compound_layer, (5.0, 2.0)),
          "compound knockout fixture does not expose the unsampled hole")
    check(not compound_paint.definitely_erased(hole_target),
          "five sampled path points falsely proved full erasure over a hole")
    rectangular_knockout = {
        **compound_knockout,
        "id": "rectangular-knockout",
        "even_odd": False,
        "subpaths": [compound_knockout["subpaths"][0]],
    }
    check(FinalPaint([
        hole_target, rectangular_knockout,
    ]).definitely_erased(hole_target),
        "one exact covering rectangle did not prove path erasure")
    swept_target = {
        **synthetic_vertical(0.5, 0, 10, 1.0, 1),
        "x0": 0.0, "x1": 1.0,
    }
    diagonal_knockout = {
        "id": "diagonal-knockout",
        "x0": -0.1, "x1": 1.1, "y0": 0.0, "y1": 10.0,
        "fill": [1.0, 1.0, 1.0], "fill_gray": 1.0,
        "stroke": None, "stroke_gray": None, "stroke_width_pt": 0.0,
        "even_odd": False, "role": "knockout",
        "paint_seq": 2, "paint_seq_max": 2,
        "subpaths": [{
            "start": [-0.1, 0.0], "closed": True,
            "ops": [
                {"op": "l", "points": [0.1, 0.0]},
                {"op": "l", "points": [1.1, 10.0]},
                {"op": "l", "points": [0.9, 10.0]},
                {"op": "l", "points": [-0.1, 0.0]},
            ],
        }],
    }
    swept_paint = FinalPaint([swept_target, diagonal_knockout])
    check(not swept_paint.visible_intervals(swept_target)
          and not swept_paint.structural_across(swept_target, 0.0, 10.0),
          "moving nonrect knockout was certified from one midpoint section")

    # A lone highly unequal split and two anchor runs separated by an interior
    # multi-slot gap are not coherent single combs. They remain present but
    # explicitly unresolved until an independent referee adjudicates them.
    unequal = synthetic_vertical(95, 0, 10, 0.2, 20)
    unequal_bands = comb_bands(
        [unequal], [unequal], 0, 100, (1.0, 1.0),
        FinalPaint([unequal]))
    check(bool(unequal_bands)
          and unequal_bands[0]["resolution"]["status"] == "unresolved"
          and "unequal-two-slot-topology"
          in unequal_bands[0]["resolution"]["reason_codes"],
          "one unequal divider was not retained as unresolved")
    split_run = [synthetic_vertical(x, 0, 10, 0.2, 30 + n)
                 for n, x in enumerate((10, 20, 80, 90))]
    split_bands = comb_bands(
        split_run, split_run, 0, 100, (1.0, 1.0),
        FinalPaint(split_run))
    check(bool(split_bands)
          and split_bands[0]["resolution"]["status"] == "unresolved"
          and "split-anchor-run-topology"
          in split_bands[0]["resolution"]["reason_codes"],
          "two separated anchor runs were not retained as unresolved")

    check(layout["form"]["code"] == "2551Q", "form code is not 2551Q")
    check(len(layout["pages"]) == 2, "expected 2 pages")

    for page in layout["pages"]:
        n = page["index"]
        check(bool(page["cells"]), f"page {n} produced no cells")
        check(bool(page["regions"]), f"page {n} produced no regions")
        check(any("comb" in c for c in page["cells"]), f"page {n} found no comb cell")
        check(not any(not c["rectangular"] for c in page["cells"]),
              f"page {n} retained a non-rectangular cell")
        check(len({cell["id"] for cell in page["cells"]}) == len(page["cells"]),
              f"page {n} contains duplicate stable cell ids")
        check(all(
            cell["subject_key"] == geometry_subject_key(
                n, (cell["x0"], cell["y0"], cell["x1"], cell["y1"]))
            for cell in page["cells"]),
            f"page {n} contains a geometry subject-key mismatch")
        check(page["stats"]["comb_subjects"]
              == (page["stats"]["comb_subjects_active"]
                  + page["stats"]["comb_subjects_retained_unresolved"]),
              f"page {n} subject ledger does not reconcile")
        check(page["stats"]["comb_subjects_active"]
              == (page["stats"]["comb_subjects_active_resolved"]
                  + page["stats"]["comb_subjects_active_unresolved"]),
              f"page {n} active subject states do not reconcile")
        check(page["stats"]["comb_subjects_retired"] == 0,
              f"page {n} silently retired a comb subject")
        check(page["stats"]["comb_evidence_blocking"]
              == (page["stats"]["comb_subjects_blocking"]
                  + page["stats"]["comb_inferences_blocking"]),
              f"page {n} blocking evidence ledger does not reconcile")
        check(page["stats"]["cells_geometry_unresolved"]
              == sum(bool(cell.get("geometry_resolution"))
                     for cell in page["cells"]),
              f"page {n} geometry uncertainty count does not reconcile")

        # Every cell coordinate must be a lattice position, exactly.
        xs, ys = set(page["x_lattice"]), set(page["y_lattice"])
        off = [c["id"] for c in page["cells"]
               if c["x0"] not in xs or c["x1"] not in xs
               or c["y0"] not in ys or c["y1"] not in ys]
        check(not off, f"page {n} cells off the lattice: {off[:5]}")

        # A comb must never have been shattered into per-character cells.
        for cell in page["cells"]:
            comb = cell.get("comb")
            if comb:
                check(comb["cells"] == comb["divider_count"] + 1,
                      f"{cell['id']} comb slot count disagrees with its dividers")
                check(comb["slot_x"] == sorted(comb["slot_x"]),
                      f"{cell['id']} comb slot boundaries are not ascending")

    # The comb discriminator is geometric, so its split is reproducible to the
    # rule. These are the counts the recon pass measured on this exact PDF.
    page1, page2 = layout["pages"]
    ir1, ir2 = ir["pages"]
    check(page1["stats"]["comb_slots"] == 489,
          f"page 1: expected 489 comb slots, got {page1['stats']['comb_slots']}")
    check(page2["stats"]["comb_slots"] == 264,
          f"page 2: expected 264 comb slots, got {page2['stats']['comb_slots']}")

    framed = [
        cell for cell in page1["cells"]
        if [cell["x0"], cell["y0"], cell["x1"], cell["y1"]]
        == [362.71, 283.97, 590.16, 303.29]
    ]
    check(len(framed) == 1,
          "page 1: framed 16-slot composite was partitioned or duplicated")
    if framed:
        certificate = framed[0].get("comb_frame_certificate") or {}
        expected_dividers = [
            376.99, 391.15, 405.31, 419.59, 433.75,
            447.91, 462.1, 476.38, 490.54, 504.7,
            518.98, 533.14, 547.42, 561.7, 575.74,
        ]
        check(certificate.get("method") == "final-visible-framed-comb"
              and certificate.get("band_y") == [295.73, 303.05]
              and certificate.get("divider_x") == expected_dividers
              and (framed[0].get("comb") or {}).get("cells") == 16,
              f"page 1: invalid framed-comb certificate {certificate}")

    def thin_combs(page: dict[str, Any], ir_page: dict[str, Any]) -> int:
        ids = set(page["comb_divider_ids"])
        return sum(1 for r in ir_page["rules"] if r["id"] in ids and r["thickness_pt"] == 0.24)

    check(thin_combs(page1, ir1) == 377,
          f"page 1: expected 377 0.24pt comb dividers, got {thin_combs(page1, ir1)}")
    check(thin_combs(page2, ir2) == 195,
          f"page 2: expected 195 0.24pt comb dividers, got {thin_combs(page2, ir2)}")
    # Thickness inside a comb is rank, not membership: the money combs mix
    # 0.24pt character dividers with 1.44pt thousands separators in one field.
    check(any(max(c["comb"]["divider_thicknesses_pt"]) > 0.24
              for c in page2["cells"] if "comb" in c),
          "page 2: no comb with digit-group separators heavier than 0.24pt")

    # Schedule 1 -- Computation of Tax. The recon pass established that this
    # band is on PAGE 2, not page 1: page 1 is masthead + Parts I-III, all of
    # them fixed-height, and it must carry no growable at all.
    check(not page1["growable"], f"page 1 should have no growable, got {page1['growable']}")

    atc = [g for g in page2["growable"] if abs(g["y0"] - 162.26) <= CLUSTER_TOL_PT]
    check(bool(atc), "page 2: Schedule 1 growable band not found at y=162.26")
    if atc:
        band = atc[0]
        check(band["row_count"] == 6, f"Schedule 1 row count {band['row_count']} != 6")
        check(band["capacity"] == 6, f"Schedule 1 capacity {band['capacity']} != 6")
        check(band["row_pitch_pt"] == 18.24,
              f"Schedule 1 pitch {band['row_pitch_pt']} != 18.24")
        # Row 6 is 18.27pt: the band is regular but not uniform.
        check(band["row_pitch_max_pt"] == 18.27,
              f"Schedule 1 max pitch {band['row_pitch_max_pt']} != 18.27")
        check(band["row_y"][0] == 162.26 and band["row_y"][-1] == 271.73,
              f"Schedule 1 row_y endpoints wrong: {band['row_y'][0]}..{band['row_y'][-1]}")
        expected_columns = [23.04, 37.2, 108.14, 278.21, 292.37, 320.69,
                            349.15, 363.31, 533.5, 547.54, 575.98, 590.14]
        check(band["column_x"] == expected_columns,
              f"Schedule 1 columns {band['column_x']} != {expected_columns}")
        check(len(band["template_cell_ids"]) == 11,
              f"Schedule 1 template row has {len(band['template_cell_ids'])} cells, expected 11")
        # The 12-slot money combs are the whole point: one cell, twelve slots.
        combs = [c["comb"]["cells"] for c in page2["cells"]
                 if c["id"] in set(band["template_cell_ids"]) and "comb" in c]
        check(sorted(combs) == [2, 2, 2, 5, 12, 12],
              f"Schedule 1 template comb shapes {sorted(combs)} != [2, 2, 2, 5, 12, 12]")

    # Determinism: the same IR must serialise byte-identically.
    again = json.dumps(build_layout(ir), sort_keys=False, ensure_ascii=False)
    check(again == json.dumps(layout, sort_keys=False, ensure_ascii=False),
          "layout is not deterministic across two builds")

    for message in failures:
        print(f"FAIL {message}", file=sys.stderr)
    print_summary(layout, sys.stderr)
    print(f"self-test: {'PASS' if not failures else f'{len(failures)} FAILURE(S)'}",
          file=sys.stderr)
    return 1 if failures else 0


def print_summary(layout: dict[str, Any], stream: Any) -> None:
    form = layout["form"]
    print(f"{form['code']} rev {form['revision']}  layout schema {layout['schema_version']}",
          file=stream)
    for page in layout["pages"]:
        s = page["stats"]
        print(f"  page {page['index']}: lattice {s['x_lattice']}x{s['y_lattice']}  "
              f"cells {s['cells']} ({s['cells_non_rectangular']} non-rect)  "
              f"regions {s['regions']}  growables {s['growables']}  "
              f"comb cells {s['comb_cells']} ({s['comb_slots']} slots from "
              f"{s['comb_dividers']} dividers)", file=stream)
        print(f"           kinds {s['cell_kinds']}  "
              f"decorative {s['decorative_rules']}  "
              f"text {s['text_runs']} ({s['text_runs_unassigned']} outside every cell)",
              file=stream)
        for g in page["growable"]:
            print(f"           growable {g['id']}: {g['row_count']} rows x "
                  f"{len(g['column_x'])} columns, pitch {g['row_pitch_pt']}pt "
                  f"({g['row_pitch_min_pt']}-{g['row_pitch_max_pt']}), "
                  f"y {g['y0']}->{g['y1']}, capacity {g['capacity']}", file=stream)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--ir", required=True, type=pathlib.Path,
                        help="IR JSON produced by tools/formgen/extract.py")
    parser.add_argument("--out", type=pathlib.Path, default=None,
                        help="Write layout JSON here (default: stdout).")
    parser.add_argument("--summary", action="store_true",
                        help="Print a per-page summary to stderr.")
    parser.add_argument("--self-test", action="store_true",
                        help="Run assertions against the given IR and exit non-zero on failure.")
    args = parser.parse_args(argv)

    if not args.ir.is_file():
        print(f"no such IR: {args.ir}", file=sys.stderr)
        return 2

    if args.self_test:
        return self_test(args.ir)

    ir = json.loads(args.ir.read_text(encoding="utf-8"))
    layout = build_layout(ir)
    payload = json.dumps(layout, indent=2, sort_keys=False, ensure_ascii=False) + "\n"

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(payload, encoding="utf-8")
    else:
        sys.stdout.write(payload)

    if args.summary:
        print_summary(layout, sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
