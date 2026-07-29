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


def comb_bands(members: Sequence[dict[str, Any]], x0: float,
               x1: float) -> list[dict[str, Any]]:
    """Group a cell's comb dividers into bands, one band per field.

    Dividers of one comb share a y extent exactly (they are drawn by the same
    loop), so grouping on the band extent is safe and needs no pitch assumption.

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

    bands: list[dict[str, Any]] = []
    for (band_y0, band_y1), band in sorted(by_band.items()):
        xs = sorted(q(centre(d)) for d in band)
        boundaries = [q(x0), *xs, q(x1)]
        deltas = [q(b - a) for a, b in zip(boundaries, boundaries[1:])]
        thicknesses = collections.Counter(d["thickness_pt"] for d in band)
        grays = sorted({d["gray"] for d in band if d["gray"] is not None})
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
            "y0": q(band_y0), "y1": q(band_y1),
            "height_pt": q(band_y1 - band_y0),
        })
    bands.sort(key=lambda b: (b["y0"], -b["divider_count"]))
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


def build_cells(page_index: int, xl: Lattice, yl: Lattice,
                dsu: DisjointSet, v_at: list[list[bool]], h_at: list[list[bool]],
                v_ink: Sequence[dict[str, Any]], h_ink: Sequence[dict[str, Any]],
                dividers: Sequence[dict[str, Any]],
                text_runs: Sequence[dict[str, Any]]) -> tuple[list[dict[str, Any]], list[str]]:
    nx, ny = len(xl) - 1, len(yl) - 1
    components: dict[int, list[tuple[int, int]]] = collections.defaultdict(list)
    for j in range(ny):
        for i in range(nx):
            components[dsu.find(j * nx + i)].append((j, i))

    boxes: list[dict[str, Any]] = []
    for squares in components.values():
        js = [j for j, _ in squares]
        is_ = [i for _, i in squares]
        j0, j1, i0, i1 = min(js), max(js) + 1, min(is_), max(is_) + 1
        if not encloses_paper(xl, i0, i1) or not encloses_paper(yl, j0, j1):
            continue
        boxes.append({
            "j0": j0, "j1": j1, "i0": i0, "i1": i1,
            "rectangular": len(squares) == (j1 - j0) * (i1 - i0),
        })
    boxes.sort(key=lambda b: (yl.positions[b["j0"]], xl.positions[b["i0"]]))

    cells: list[dict[str, Any]] = []
    for n, box in enumerate(boxes):
        j0, j1, i0, i1 = box["j0"], box["j1"], box["i0"], box["i1"]
        x0, x1 = xl.positions[i0], xl.positions[i1]
        y0, y1 = yl.positions[j0], yl.positions[j1]

        border: dict[str, Any] = {}
        for side, (lat, index, ink, lo, hi, present) in {
            "top": (yl, j0, h_ink, xl.ink_hi[i0], xl.ink_lo[i1],
                    all(h_at[j0][i] for i in range(i0, i1))),
            "bottom": (yl, j1, h_ink, xl.ink_hi[i0], xl.ink_lo[i1],
                       all(h_at[j1][i] for i in range(i0, i1))),
            "left": (xl, i0, v_ink, yl.ink_hi[j0], yl.ink_lo[j1],
                     all(v_at[i0][j] for j in range(j0, j1))),
            "right": (xl, i1, v_ink, yl.ink_hi[j0], yl.ink_lo[j1],
                      all(v_at[i1][j] for j in range(j0, j1))),
        }.items():
            if not present:
                border[side] = None
                continue
            thickness, gray, all_t = line_thickness_gray(
                lat, index, ink, lo, hi, "h" if side in ("top", "bottom") else "v")
            border[side] = {"thickness_pt": thickness, "gray": gray,
                            "thicknesses_pt": all_t}

        border_count = sum(1 for b in border.values() if b is not None)
        cell = {
            "id": f"p{page_index}c{n}",
            "x0": x0, "y0": y0, "x1": x1, "y1": y1,
            "row": j0, "col": i0, "row_span": j1 - j0, "col_span": i1 - i0,
            "rectangular": box["rectangular"],
            "border": border,
            "border_count": border_count,
            "text_run_ids": [],
            "is_empty": True,
            "kind": "blank",
        }
        cells.append(cell)

    for cell, members in zip(cells, assign_points(
            cells, [(centre(d), (d["y0"] + d["y1"]) / 2.0, d) for d in dividers])[0]):
        bands = comb_bands(members, cell["x0"], cell["x1"])
        if bands:
            cell["comb"] = max(bands, key=lambda b: (b["divider_count"], -b["y0"]))
            if len(bands) > 1:
                cell["combs"] = bands

    assigned, unplaced = assign_points(
        cells, [((r["x0"] + r["x1"]) / 2.0, (r["y0"] + r["y1"]) / 2.0, index)
                for index, r in enumerate(text_runs)])
    for cell, members in zip(cells, assigned):
        cell["text_run_ids"] = [f"p{page_index}t{i}" for i in sorted(members)]
    unassigned = [f"p{page_index}t{i}" for i in sorted(unplaced)]

    for cell in cells:
        cell["is_empty"] = not cell["text_run_ids"]
        cell["kind"] = classify_cell(cell["is_empty"], cell["border_count"], "comb" in cell)
    return cells, unassigned


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
    structural = [r for r in rules if r["role"] == "structural"]
    # Grey ornament. It must be painted, but never as a black border -- the
    # raster-era mistake this project has already paid for once.
    decorative = [r for r in rules if r["role"] == "decorative"]

    horizontals = sorted((r for r in structural if r["axis"] == "h"),
                         key=lambda r: (r["y0"], r["x0"]))
    verticals = sorted((r for r in structural if r["axis"] == "v"),
                       key=lambda r: (r["x0"], r["y0"]))
    dividers, borders = split_verticals(verticals, horizontals)

    xl = build_lattice(borders, verticals, "v")
    yl = build_lattice(horizontals, horizontals, "h")

    if len(xl) < 2 or len(yl) < 2:
        cells: list[dict[str, Any]] = []
        unassigned = [f"p{index}t{i}" for i in range(len(page["text_runs"]))]
        growables: list[dict[str, Any]] = []
        regions: list[dict[str, Any]] = []
        v_at = h_at = []
    else:
        dsu, v_at, h_at = merge_grid(xl, yl)
        cells, unassigned = build_cells(index, xl, yl, dsu, v_at, h_at, verticals,
                                        horizontals, dividers, page["text_runs"])
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
        "regions": regions,
        "growable": growables,
        "decorative_rules": decorative,
        "comb_divider_ids": [d["id"] for d in dividers],
        "unassigned_text_run_ids": unassigned,
        "stats": {
            "x_lattice": len(xl),
            "y_lattice": len(yl),
            "cells": len(cells),
            "cells_non_rectangular": sum(1 for c in cells if not c["rectangular"]),
            "regions": len(regions),
            "growables": len(growables),
            "comb_cells": len(comb_cells),
            "comb_slots": sum(c["comb"]["cells"] for c in comb_cells),
            "comb_dividers": len(dividers),
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

    check(layout["form"]["code"] == "2551Q", "form code is not 2551Q")
    check(len(layout["pages"]) == 2, "expected 2 pages")

    for page in layout["pages"]:
        n = page["index"]
        check(bool(page["cells"]), f"page {n} produced no cells")
        check(bool(page["regions"]), f"page {n} produced no regions")
        check(any("comb" in c for c in page["cells"]), f"page {n} found no comb cell")

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
