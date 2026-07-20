#!/usr/bin/env python3
"""Derive a reviewable geometry contract from a pinned official BIR PDF.

WHY THIS EXISTS. The first ten converted forms spent roughly 40% of their
commits on calibration churn: a coordinate was guessed, rendered, pixel-diffed,
nudged, and re-diffed until it looked right. That loop discovers geometry it
could have read. Every number it hunts for is already in the PDF's content
stream as an exact vector coordinate.

This tool reads those coordinates and emits them as a contract. It is
*candidate generation*, not an oracle. Comb detection, checkbox detection and
every semantic question ("is this field a TIN or a ZIP?") require human review
against the overlays produced by scripts/review_geometry_contract.py. The
contract records what the PDF draws; it never records what a field means.

The output is calibration input only. Official PDFs and their rasters must
never become runtime assets or page backgrounds.

Two drawing styles occur in this corpus and both are handled:

  * "Word-clean" (e.g. 2316): stroked line segments and real container
    rectangles. Rule weight is the stroke width.
  * "Excel-fragmented" (e.g. 1602Q, 2551Q): no strokes at all - every rule is a
    thin *filled* rectangle, shattered into hundreds of collinear pieces with
    square joint fragments at the seams. Rule weight is the rectangle's
    thickness, and collinear coalescing is mandatory before the geometry is
    legible. 2551Q page 1 is 3432 filled rects that coalesce to ~200 rules.

A consequence of the second style that matters more than it looks: many rules
are drawn *thinner than one device pixel* at 144 DPI. A 0.24pt divider is 0.48
device px wide. It is pure black ink in the PDF, but it can never rasterize
darker than about 50% grey, because it only ever covers half a pixel. Guides
that "look mid-grey" in the reference raster are therefore usually black ink at
sub-pixel width, not grey ink - so a detector keyed on near-black raster tone
misses them while the vector geometry states them exactly. Every rule carries
`subpixel` and `predicted_min_tone` so this is visible without re-deriving it.

Determinism is a hard requirement: two runs over the same bytes must produce
byte-identical output, so nothing timing- or path-dependent enters the
document. Wall-clock numbers go to the stdout report, never into the contract.

KNOWN LIMITATIONS — read before consuming this contract.

Adversarial review found these; they are recorded rather than hidden, because a
generator that trusts the contract beyond these bounds will emit wrong geometry.

1. Comb candidates are CANDIDATES. `review` is a constant "candidate" on every
   entry and carries no information: irregular runs are filtered upstream, so
   the field can never say otherwise. Human accept/reject through
   review_geometry_contract.py is mandatory, not advisory.

2. Over-merged combs are not flagged. Where several fields share one
   uninterrupted uniform-pitch run with no heavier divider between them,
   geometry cannot separate them and the contract reports one wide comb. On
   2551Q this yields 55 geometric combs against 96 rendered fields; the 26-,
   28-, 31- and 40-cell entries are over-merges and look identical in the
   output to the correct 16-cell Item 12A. Splitting them is a semantic
   decision only a human can make.

3. The two detectors are not reconciled. They agree 34/34 on 2551Q and
   disagree 4-vs-29 on 2316. That agreement is the strongest available
   confidence signal and it is currently computed by neither detector nor
   emitted; consumers should compare the two lists themselves.

4. Merged and spanned table cells are unproven. The spike flagged this and
   nothing here resolves it; do not derive a cell graph from the rule lattice
   without checking the result against the page.

5. `extractor.pymupdf` is baked into the pinned bytes, so a PyMuPDF upgrade
   changes the contract hash and fails --check-only for a reason unrelated to
   geometry. Re-pin deliberately on upgrade.

6. Knockout semantics are partial. White rectangles painted over shaded bands
   are captured, but thin white "eraser" rects that cover a black rule are
   dropped below a 6pt threshold while the rule beneath is still emitted, so a
   few erased rules survive in the contract that do not appear on the page.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import tempfile
import time
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable, Sequence

try:
    import fitz  # PyMuPDF
except ImportError:  # pragma: no cover - environment guard
    print(
        "PyMuPDF is required: pip install pymupdf",
        file=sys.stderr,
    )
    raise SystemExit(2)


SCHEMA = "bir-geometry-contract/draft-0"
EXTRACTOR_VERSION = "1.0.0"

FORM_CODE_RE = re.compile(r"^[A-Za-z0-9-]+$")
REVISION_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")

# 144 DPI is the reference raster resolution used by the whole parity pipeline.
SCALE = 144.0 / 72.0

# A filled rect is a rule (not an area fill) when its short side is at or below
# this, in points. 1.8pt is the thickest rule weight observed in the corpus;
# 2.2 leaves headroom without swallowing real boxes.
RULE_MAX_THICKNESS_PT = 2.2
# ...and only when it is at least this many times longer than it is thick.
# Below this it is a joint fragment or a glyph-ish speck, not a rule.
RULE_MIN_ASPECT = 2.0

# Anything at or above this in every channel is background paint, not ink.
WHITE_LEVEL = 0.95

# Candidate coalescing tolerances, swept for the plateau. Ascending.
GAP_TOL_SWEEP = (0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0)
# Two sweep steps are "the same step" when their rule counts differ by less
# than this fraction. See plateau_gap_tol for why exact equality is useless.
PLATEAU_REL = 0.01

# Comb detection bounds, in points.
COMB_MIN_TICKS = 4
COMB_MIN_PITCH_PT = 4.0
COMB_MAX_PITCH_PT = 40.0
COMB_PITCH_TOL = 0.22  # fractional deviation from the running pitch
# How far two divider ticks' ends may differ and still count as one band.
# Not cosmetic: in 2551Q's item 12A, 13 of the 15 dividers carry a 0.48pt joint
# stub at the top and 2 do not, so the ticks differ in length by half a point.
# Keying bands on exact extents split that comb into runs of 13 and 2, and the
# run of 2 fell under COMB_MIN_TICKS - reporting 14 cells for a 16-cell field.
COMB_BAND_TOL_PT = 2.0

# Checkbox side length bounds, in points.
CHECKBOX_MIN_PT = 5.0
CHECKBOX_MAX_PT = 16.0
CHECKBOX_SQUARENESS_PT = 3.0


# --------------------------------------------------------------------------
# small helpers
# --------------------------------------------------------------------------


def rpt(value: float) -> float:
    """Round to 0.01pt. Sub-hundredth precision is below plate tolerance."""
    return round(float(value) + 0.0, 2)


def rpx(value: float) -> float:
    """Points to 144-DPI pixels, rounded to 0.01px."""
    return round(float(value) * SCALE, 2)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def gray_of(color: Sequence[float] | None) -> float | None:
    """Perceptual-free mean of an RGB triple, or None for no paint."""
    if not color:
        return None
    return round(sum(color) / len(color), 4)


def is_white(color: Sequence[float] | None) -> bool:
    return bool(color) and min(color) >= WHITE_LEVEL


def predicted_min_tone(thickness_px: float, gray: float | None) -> int:
    """Darkest 8-bit tone this rule can reach in a 144-DPI raster.

    A rule narrower than one device pixel cannot fully ink any pixel, no matter
    how black it is: coverage caps the darkness. This is why sub-pixel black
    guides read as mid-grey in the reference PNGs.
    """
    ink = 0.0 if gray is None else max(0.0, min(1.0, gray))
    coverage = max(0.0, min(1.0, thickness_px))
    return int(round(255.0 * (1.0 - coverage * (1.0 - ink))))


def device_px(thickness_px: float) -> int:
    """Device pixel rows a rule of this thickness occupies at 144 DPI."""
    return max(1, int(round(thickness_px)))


# --------------------------------------------------------------------------
# primitive classification
# --------------------------------------------------------------------------


def classify_primitives(page: "fitz.Page") -> dict[str, Any]:
    """Flatten page.get_drawings() into rules / areas / curves.

    Both drawing styles land here. Stroked line segments carry their stroke
    width; filled rectangles carry their own thickness as the weight. Square
    fragments too small to have an orientation are set aside as joints - they
    are seam artifacts of Excel border export and would otherwise pollute the
    rule lattice. Coalescing bridges the gaps they leave.
    """
    rules_h: list[dict[str, Any]] = []
    rules_v: list[dict[str, Any]] = []
    areas: list[dict[str, Any]] = []
    joints: list[dict[str, Any]] = []
    curves = 0
    segments = 0
    white_rects = 0

    for drawing in page.get_drawings():
        fill = drawing.get("fill")
        stroke = drawing.get("color")
        stroke_w = float(drawing.get("width") or 0.0)

        for item in drawing["items"]:
            op = item[0]
            segments += 1

            if op == "l":
                p1, p2 = item[1], item[2]
                x0, x1 = sorted((p1.x, p2.x))
                y0, y1 = sorted((p1.y, p2.y))
                # A stroked segment's footprint is its length by its width.
                weight = stroke_w if stroke_w > 0 else 0.24
                if (y1 - y0) <= (x1 - x0):
                    rules_h.append(
                        {
                            "start": x0,
                            "end": x1,
                            "pos": (y0 + y1) / 2.0,
                            "thickness": weight,
                            "gray": gray_of(stroke),
                            "src": "stroke",
                        }
                    )
                else:
                    rules_v.append(
                        {
                            "start": y0,
                            "end": y1,
                            "pos": (x0 + x1) / 2.0,
                            "thickness": weight,
                            "gray": gray_of(stroke),
                            "src": "stroke",
                        }
                    )
                continue

            if op in ("re", "qu"):
                rect = item[1].rect if op == "qu" else item[1]
                width = float(rect.width)
                height = float(rect.height)
                paint = fill if fill is not None else stroke
                if is_white(paint):
                    white_rects += 1
                    # White FILL is background. A white-filled rect with a dark
                    # STROKE is not: it is a bordered field box, the most common
                    # primitive in this corpus, and collapsing it to background
                    # silently deletes all four of its borders. Measured before
                    # this branch existed: 659 such rects across 32 of the 35
                    # forms emitted a contract asserting no ink where the
                    # official draws a visible outline, so any generator
                    # consuming it would have drawn borderless fields.
                    if stroke is not None and not is_white(stroke) and stroke_w > 0:
                        edge_weight = stroke_w
                        edge_gray = gray_of(stroke)
                        rules_h.append({
                            "start": rect.x0, "end": rect.x1, "pos": rect.y0,
                            "thickness": edge_weight, "gray": edge_gray,
                            "src": "rect-edge",
                        })
                        rules_h.append({
                            "start": rect.x0, "end": rect.x1, "pos": rect.y1,
                            "thickness": edge_weight, "gray": edge_gray,
                            "src": "rect-edge",
                        })
                        rules_v.append({
                            "start": rect.y0, "end": rect.y1, "pos": rect.x0,
                            "thickness": edge_weight, "gray": edge_gray,
                            "src": "rect-edge",
                        })
                        rules_v.append({
                            "start": rect.y0, "end": rect.y1, "pos": rect.x1,
                            "thickness": edge_weight, "gray": edge_gray,
                            "src": "rect-edge",
                        })
                    # Keep the area regardless so container detection still
                    # sees the box.
                    areas.append(
                        {
                            "x0": rect.x0,
                            "y0": rect.y0,
                            "x1": rect.x1,
                            "y1": rect.y1,
                            "gray": gray_of(paint),
                            "filled": fill is not None,
                            "stroke_w": stroke_w,
                            "stroke_gray": gray_of(stroke) if stroke is not None else None,
                        }
                    )
                    continue

                short = min(width, height)
                long_side = max(width, height)
                if short <= RULE_MAX_THICKNESS_PT and long_side >= short * RULE_MIN_ASPECT:
                    record = {
                        "thickness": short,
                        "gray": gray_of(paint),
                        "src": "fill" if fill is not None else "rect",
                    }
                    if width >= height:
                        rules_h.append(
                            {
                                **record,
                                "start": rect.x0,
                                "end": rect.x1,
                                "pos": (rect.y0 + rect.y1) / 2.0,
                            }
                        )
                    else:
                        rules_v.append(
                            {
                                **record,
                                "start": rect.y0,
                                "end": rect.y1,
                                "pos": (rect.x0 + rect.x1) / 2.0,
                            }
                        )
                elif short <= RULE_MAX_THICKNESS_PT:
                    joints.append(
                        {
                            "x0": rect.x0,
                            "y0": rect.y0,
                            "x1": rect.x1,
                            "y1": rect.y1,
                        }
                    )
                else:
                    areas.append(
                        {
                            "x0": rect.x0,
                            "y0": rect.y0,
                            "x1": rect.x1,
                            "y1": rect.y1,
                            "gray": gray_of(paint),
                            "filled": fill is not None,
                            "stroke_w": stroke_w,
                        }
                    )
                continue

            if op == "c":
                curves += 1
                continue

    return {
        "rules_h": rules_h,
        "rules_v": rules_v,
        "areas": areas,
        "joints": joints,
        "curves": curves,
        "segments": segments,
        "white_rects": white_rects,
    }


# --------------------------------------------------------------------------
# collinear coalescing + the tolerance plateau sweep
# --------------------------------------------------------------------------


def coalesce(
    rules: Iterable[dict[str, Any]],
    gap_tol: float,
    pos_tol: float = 0.6,
    thickness_tol: float = 0.25,
) -> list[dict[str, Any]]:
    """Merge collinear, touching, same-weight rule fragments into visual rules.

    Weight is part of the merge key on purpose. 2551Q draws its Part boundaries
    at 1.44pt directly against ordinary 0.48pt rules; merging on position alone
    would average those into a single fictional weight and erase exactly the
    distinction the renderer has to reproduce.
    """
    buckets: dict[tuple[int, int], list[dict[str, Any]]] = defaultdict(list)
    for rule in rules:
        key = (
            int(round(rule["pos"] / pos_tol)),
            int(round(rule["thickness"] / thickness_tol)),
        )
        buckets[key].append(rule)

    merged: list[dict[str, Any]] = []
    for key in sorted(buckets):
        group = buckets[key]
        group.sort(key=lambda r: (r["start"], r["end"]))
        current = dict(group[0])
        current["count"] = 1
        for rule in group[1:]:
            if rule["start"] <= current["end"] + gap_tol:
                current["end"] = max(current["end"], rule["end"])
                current["pos"] = (current["pos"] + rule["pos"]) / 2.0
                current["thickness"] = max(current["thickness"], rule["thickness"])
                current["count"] += 1
                if rule["gray"] is not None:
                    if current["gray"] is None:
                        current["gray"] = rule["gray"]
                    else:
                        current["gray"] = min(current["gray"], rule["gray"])
            else:
                merged.append(current)
                current = dict(rule)
                current["count"] = 1
        merged.append(current)

    merged.sort(key=lambda r: (round(r["pos"], 3), round(r["start"], 3)))
    return merged


def plateau_gap_tol(pages_rules: Sequence[tuple[list, list]]) -> dict[str, Any]:
    """Pick the coalescing tolerance from the longest stable plateau.

    Sweeping the tolerance and counting resulting rules produces a staircase:
    counts fall steeply while real fragments are still being joined, then flat
    while nothing changes, then fall again as genuinely distinct rules start
    being welded together. The correct tolerance is the *smallest* one on the
    longest flat step - the point where fragmentation is resolved but nothing
    real has been destroyed yet. Choosing the smallest on the plateau keeps the
    result conservative; choosing the longest plateau keeps it stable.
    """
    counts: list[int] = []
    for tol in GAP_TOL_SWEEP:
        total = 0
        for rules_h, rules_v in pages_rules:
            total += len(coalesce(rules_h, tol))
            total += len(coalesce(rules_v, tol))
        counts.append(total)

    # Exact equality almost never occurs on real forms - 2551Q's staircase is
    # 1436, 1226, 1190, 1189, 1028, 1012, 1011, 1009, which has no two equal
    # neighbours at all. A plateau is therefore defined by *relative* stability:
    # consecutive tolerances whose counts differ by under PLATEAU_REL are the
    # same step. Without this the selector silently falls back to the first
    # tolerance in the sweep and reports a meaningless "plateau of 1".
    steps: list[list[int]] = [[0]]
    for index in range(1, len(counts)):
        previous = counts[steps[-1][-1]]
        if previous and abs(counts[index] - previous) <= PLATEAU_REL * previous:
            steps[-1].append(index)
        else:
            steps.append([index])

    best = max(steps, key=lambda step: (len(step), -step[0]))
    selected = best[0]

    return {
        "selected_gap_tol_pt": GAP_TOL_SWEEP[selected],
        "plateau_length": len(best),
        "plateau_relative_tolerance": PLATEAU_REL,
        "plateau_is_degenerate": len(best) == 1,
        "sweep": [
            {"gap_tol_pt": tol, "merged_rules": count}
            for tol, count in zip(GAP_TOL_SWEEP, counts)
        ],
    }


def emit_rule(rule: dict[str, Any], axis: str) -> dict[str, Any]:
    thickness_pt = rpt(rule["thickness"])
    thickness_px = rpx(rule["thickness"])
    return {
        "axis": axis,
        "pos_pt": rpt(rule["pos"]),
        "start_pt": rpt(rule["start"]),
        "end_pt": rpt(rule["end"]),
        "length_pt": rpt(rule["end"] - rule["start"]),
        "pos_px": rpx(rule["pos"]),
        "start_px": rpx(rule["start"]),
        "end_px": rpx(rule["end"]),
        "thickness_pt": thickness_pt,
        "thickness_px": thickness_px,
        "device_px": device_px(thickness_px),
        "subpixel": thickness_px < 1.0,
        "gray": rule["gray"],
        "predicted_min_tone": predicted_min_tone(thickness_px, rule["gray"]),
        "fragments": rule.get("count", 1),
        "source": rule.get("src", "fill"),
    }


# --------------------------------------------------------------------------
# comb detection - two independent detectors, both reported
# --------------------------------------------------------------------------


def group_bands(
    rules: Sequence[dict[str, Any]], tol: float = COMB_BAND_TOL_PT
) -> list[tuple[float, float, list[dict[str, Any]]]]:
    """Cluster rules that share a start/end extent within `tol`.

    Compared against each band's first member rather than a running mean, so a
    long chain of slightly-drifting ticks cannot creep into an arbitrarily wide
    band. Deterministic: rules are pre-sorted.
    """
    bands: list[dict[str, Any]] = []
    for rule in sorted(rules, key=lambda r: (r["start"], r["end"], r["pos"])):
        placed = False
        for band in bands:
            if (
                abs(rule["start"] - band["start"]) <= tol
                and abs(rule["end"] - band["end"]) <= tol
            ):
                band["items"].append(rule)
                placed = True
                break
        if not placed:
            bands.append(
                {"start": rule["start"], "end": rule["end"], "items": [rule]}
            )
    return [
        (band["start"], band["end"], band["items"])
        for band in sorted(bands, key=lambda b: (b["start"], b["end"]))
    ]


def _regular_runs(positions: Sequence[float]) -> list[list[float]]:
    """Split sorted positions into maximal runs of near-constant pitch."""
    runs: list[list[float]] = []
    run: list[float] = [positions[0]] if positions else []
    pitch: float | None = None
    for index in range(1, len(positions)):
        gap = positions[index] - positions[index - 1]
        if not (COMB_MIN_PITCH_PT <= gap <= COMB_MAX_PITCH_PT):
            if len(run) >= COMB_MIN_TICKS:
                runs.append(run)
            run = [positions[index]]
            pitch = None
            continue
        reference = pitch if pitch is not None else gap
        if abs(gap - reference) <= COMB_PITCH_TOL * reference:
            run.append(positions[index])
            pitch = (
                gap if pitch is None else (pitch * (len(run) - 2) + gap) / (len(run) - 1)
            )
        else:
            if len(run) >= COMB_MIN_TICKS:
                runs.append(run)
            run = [positions[index - 1], positions[index]]
            pitch = gap
    if len(run) >= COMB_MIN_TICKS:
        runs.append(run)
    return runs


def detect_combs_tickrun(rules_v: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    """Detector 1: vertical ticks sharing a y-band at a constant pitch.

    Style-agnostic and assumption-light. It does not know where the field's
    outer border is, so it reports raw tick counts and leaves the border
    question to the container detector.
    """
    combs: list[dict[str, Any]] = []
    for y0, y1, band in group_bands(rules_v):
        if len(band) < COMB_MIN_TICKS:
            continue
        xs = [rule["pos"] for rule in band]
        for run in _regular_runs(sorted(xs)):
            pitch = (run[-1] - run[0]) / (len(run) - 1)
            combs.append(
                {
                    "detector": "tickrun",
                    "x_start_pt": rpt(run[0]),
                    "x_end_pt": rpt(run[-1]),
                    "y0_pt": rpt(y0),
                    "y1_pt": rpt(y1),
                    "x_start_px": rpx(run[0]),
                    "x_end_px": rpx(run[-1]),
                    "y0_px": rpx(y0),
                    "y1_px": rpx(y1),
                    "tick_count": len(run),
                    "cells_between_ticks": len(run) - 1,
                    "pitch_pt": rpt(pitch),
                    "pitch_px": rpx(pitch),
                }
            )
    combs.sort(key=lambda c: (c["y0_pt"], c["x_start_pt"]))
    return combs


def detect_combs_container(
    rules_h: Sequence[dict[str, Any]],
    rules_v: Sequence[dict[str, Any]],
    areas: Sequence[dict[str, Any]],
) -> list[dict[str, Any]]:
    """Detector 2: a tick run resolved against its enclosing container.

    The cell count is what the taxpayer actually sees, and it depends on where
    the field's own borders are - not on how many ticks happen to share a band.
    This finds the smallest container that encloses a run and then classifies
    each tick as an interior divider or as the container's own edge, so
    `cells = interior_dividers + 1` holds regardless of whether the border
    strokes were drawn in the same band as the dividers.

    Smallest-container preference matters: a comb usually sits inside a row box
    inside a section box, and only the innermost one gives the right cell count.

    Containers come from two sources so that both drawing styles are covered:
    explicit rectangles (Word-clean forms), and rectangles implied by the rule
    lattice (Excel-fragmented forms, which draw no container rects at all).
    """
    candidates: list[dict[str, Any]] = []

    for y0, y1, band in group_bands(rules_v):
        if len(band) < COMB_MIN_TICKS:
            continue
        by_pos = {rpt(r["pos"]): r for r in band}
        for run in _regular_runs(sorted(by_pos)):
            container = _smallest_container(
                run[0], run[-1], y0, y1, rules_h, rules_v, areas, exclude=set(run)
            )

            left = container["x0"] if container else run[0]
            right = container["x1"] if container else run[-1]
            # A tick within a hair of the container edge IS the edge.
            edge_tol = 1.5
            interior = [
                x for x in run if x - left > edge_tol and right - x > edge_tol
            ]
            if len(interior) < 2:
                continue
            gaps = [b - a for a, b in zip(interior, interior[1:])]
            pitch = sum(gaps) / len(gaps) if gaps else 0.0
            regular = all(abs(g - pitch) <= COMB_PITCH_TOL * pitch for g in gaps) if gaps else False

            widths = sorted(by_pos[x]["thickness"] for x in interior)
            median_w = widths[len(widths) // 2]

            candidates.append(
                {
                    "detector": "container",
                    "container_source": container["source"] if container else "none",
                    "x0_pt": rpt(left),
                    "x1_pt": rpt(right),
                    "y0_pt": rpt(container["y0"] if container else y0),
                    "y1_pt": rpt(container["y1"] if container else y1),
                    "x0_px": rpx(left),
                    "x1_px": rpx(right),
                    "y0_px": rpx(container["y0"] if container else y0),
                    "y1_px": rpx(container["y1"] if container else y1),
                    "ticks_y0_pt": rpt(y0),
                    "ticks_y1_pt": rpt(y1),
                    "interior_dividers": len(interior),
                    # MEASURED divider positions, not just a count. A consumer
                    # given only a count and a pitch must reconstruct dividers
                    # uniformly, and measured across the corpus that is off by
                    # up to 2.53 device px, with 24 of 81 combs at least 1px out
                    # - the same magnitude and class as the 2551Q Part II defect
                    # this tool exists to eliminate. Emitting the real positions
                    # is the difference between describing a defect and
                    # supplying what is needed to fix it.
                    "interior_divider_x_pt": [rpt(x) for x in interior],
                    "interior_divider_x_px": [rpx(x) for x in interior],
                    "cells": len(interior) + 1,
                    "pitch_pt": rpt(pitch),
                    "pitch_px": rpx(pitch),
                    "pitch_is_uniform": regular,
                    "divider_thickness_pt": rpt(median_w),
                    "divider_thickness_px": rpx(median_w),
                    "divider_subpixel": rpx(median_w) < 1.0,
                    "review": "candidate",
                }
            )

    # Container-anchored pass. The band pass above needs a run of COMB_MIN_TICKS
    # dividers to fire, which is right for 2551Q's 12-to-40-cell combs but blind
    # to the 3-to-5-cell combs that dominate Word-clean forms: a 3-cell comb has
    # only 2 interior ticks. Anchoring on an explicit container rect and counting
    # what falls inside it needs no run length at all. Without this pass 2316
    # page 1 reports 4 combs where the spike reports 32.
    for area in areas:
        width = area["x1"] - area["x0"]
        height = area["y1"] - area["y0"]
        if width < COMB_MIN_PITCH_PT * 2 or not (4.0 <= height <= 40.0):
            continue
        inside = [
            rule
            for rule in rules_v
            if area["x0"] + 1.0 < rule["pos"] < area["x1"] - 1.0
            and rule["start"] <= area["y1"] - height * 0.25
            and rule["end"] >= area["y0"] + height * 0.25
        ]
        if len(inside) < 2:
            continue
        xs = sorted(rule["pos"] for rule in inside)
        edges = [area["x0"]] + xs + [area["x1"]]
        gaps = [b - a for a, b in zip(edges, edges[1:])]
        pitch = sum(gaps) / len(gaps)
        if not (COMB_MIN_PITCH_PT <= pitch <= COMB_MAX_PITCH_PT):
            continue
        regular = all(abs(g - pitch) <= COMB_PITCH_TOL * pitch for g in gaps)
        if not regular:
            continue
        widths = sorted(rule["thickness"] for rule in inside)
        median_w = widths[len(widths) // 2]
        candidates.append(
            {
                "detector": "container",
                "container_source": "rect",
                "x0_pt": rpt(area["x0"]),
                "x1_pt": rpt(area["x1"]),
                "y0_pt": rpt(area["y0"]),
                "y1_pt": rpt(area["y1"]),
                "x0_px": rpx(area["x0"]),
                "x1_px": rpx(area["x1"]),
                "y0_px": rpx(area["y0"]),
                "y1_px": rpx(area["y1"]),
                "ticks_y0_pt": rpt(area["y0"]),
                "ticks_y1_pt": rpt(area["y1"]),
                "interior_dividers": len(xs),
                # Measured positions, for the reason documented on the other
                # detector: a count plus a pitch does not reconstruct these.
                "interior_divider_x_pt": [rpt(x) for x in xs],
                "interior_divider_x_px": [rpx(x) for x in xs],
                "cells": len(xs) + 1,
                "pitch_pt": rpt(pitch),
                "pitch_px": rpx(pitch),
                "pitch_is_uniform": regular,
                "divider_thickness_pt": rpt(median_w),
                "divider_thickness_px": rpx(median_w),
                "divider_subpixel": rpx(median_w) < 1.0,
                "review": "candidate",
            }
        )

    # Deduplicate identical geometry produced by overlapping bands, preferring
    # the entry with the most interior dividers (the fullest description).
    best: dict[tuple[float, float, float], dict[str, Any]] = {}
    for candidate in candidates:
        key = (candidate["x0_pt"], candidate["x1_pt"], candidate["y0_pt"])
        incumbent = best.get(key)
        if incumbent is None or candidate["interior_dividers"] > incumbent["interior_dividers"]:
            best[key] = candidate
    out = sorted(best.values(), key=lambda c: (c["y0_pt"], c["x0_pt"]))
    return out


def _smallest_container(
    x_lo: float,
    x_hi: float,
    y_lo: float,
    y_hi: float,
    rules_h: Sequence[dict[str, Any]],
    rules_v: Sequence[dict[str, Any]],
    areas: Sequence[dict[str, Any]],
    exclude: set[float] | None = None,
) -> dict[str, Any] | None:
    """Innermost rectangle enclosing a tick run, from rects or the lattice.

    `exclude` holds the run's own tick positions. Without it the search happily
    returns the outermost *dividers* as the field's borders, which drops one
    cell off each end - 2551Q item 12A read 14 cells instead of 16 that way.
    """
    pad = 1.0
    skip = exclude or set()
    best: dict[str, Any] | None = None

    for area in areas:
        if (
            area["x0"] <= x_lo + pad
            and area["x1"] >= x_hi - pad
            and area["y0"] <= y_lo + pad
            and area["y1"] >= y_hi - pad
        ):
            box = {
                "x0": area["x0"],
                "x1": area["x1"],
                "y0": area["y0"],
                "y1": area["y1"],
                "source": "rect",
            }
            if best is None or _box_area(box) < _box_area(best):
                best = box

    # Lattice-implied container: nearest vertical rule left and right that span
    # the tick band, nearest horizontal rule above and below.
    lefts = [
        r["pos"]
        for r in rules_v
        if rpt(r["pos"]) not in skip
        and r["pos"] <= x_lo + pad
        and r["start"] <= y_lo + pad
        and r["end"] >= y_hi - pad
    ]
    rights = [
        r["pos"]
        for r in rules_v
        if rpt(r["pos"]) not in skip
        and r["pos"] >= x_hi - pad
        and r["start"] <= y_lo + pad
        and r["end"] >= y_hi - pad
    ]
    tops = [
        r["pos"]
        for r in rules_h
        if r["pos"] <= y_lo + pad and r["start"] <= x_lo + pad and r["end"] >= x_hi - pad
    ]
    bottoms = [
        r["pos"]
        for r in rules_h
        if r["pos"] >= y_hi - pad and r["start"] <= x_lo + pad and r["end"] >= x_hi - pad
    ]
    if lefts and rights and tops and bottoms:
        box = {
            "x0": max(lefts),
            "x1": min(rights),
            "y0": max(tops),
            "y1": min(bottoms),
            "source": "lattice",
        }
        if box["x1"] > box["x0"] and box["y1"] > box["y0"]:
            if best is None or _box_area(box) < _box_area(best):
                best = box

    return best


def _box_area(box: dict[str, Any]) -> float:
    return max(0.0, box["x1"] - box["x0"]) * max(0.0, box["y1"] - box["y0"])


# --------------------------------------------------------------------------
# checkboxes, fills, text, images
# --------------------------------------------------------------------------


def detect_checkboxes(
    areas: Sequence[dict[str, Any]],
    rules_h: Sequence[dict[str, Any]],
    rules_v: Sequence[dict[str, Any]],
) -> list[dict[str, Any]]:
    """Small squares, from explicit rects and from the rule lattice.

    Excel-style forms draw a checkbox as four separate thin rects, so a
    rect-only detector finds nothing on them. The lattice pass closes four rules
    into a square instead.
    """
    found: dict[tuple[float, float, float, float], dict[str, Any]] = {}

    for area in areas:
        width = area["x1"] - area["x0"]
        height = area["y1"] - area["y0"]
        if (
            CHECKBOX_MIN_PT <= width <= CHECKBOX_MAX_PT
            and CHECKBOX_MIN_PT <= height <= CHECKBOX_MAX_PT
            and abs(width - height) <= CHECKBOX_SQUARENESS_PT
        ):
            key = (rpt(area["x0"]), rpt(area["y0"]), rpt(area["x1"]), rpt(area["y1"]))
            found[key] = {"source": "rect"}

    verticals = sorted(rules_v, key=lambda r: r["pos"])
    for index, left in enumerate(verticals):
        for right in verticals[index + 1 :]:
            width = right["pos"] - left["pos"]
            if width < CHECKBOX_MIN_PT:
                continue
            if width > CHECKBOX_MAX_PT:
                break
            y0 = max(left["start"], right["start"])
            y1 = min(left["end"], right["end"])
            height = y1 - y0
            if not (CHECKBOX_MIN_PT <= height <= CHECKBOX_MAX_PT):
                continue
            if abs(width - height) > CHECKBOX_SQUARENESS_PT:
                continue
            has_top = any(
                abs(r["pos"] - y0) <= 1.2
                and r["start"] <= left["pos"] + 1.2
                and r["end"] >= right["pos"] - 1.2
                for r in rules_h
            )
            has_bottom = any(
                abs(r["pos"] - y1) <= 1.2
                and r["start"] <= left["pos"] + 1.2
                and r["end"] >= right["pos"] - 1.2
                for r in rules_h
            )
            if has_top and has_bottom:
                key = (rpt(left["pos"]), rpt(y0), rpt(right["pos"]), rpt(y1))
                found.setdefault(key, {"source": "lattice"})

    out = []
    for (x0, y0, x1, y1) in sorted(found):
        out.append(
            {
                "x0_pt": x0,
                "y0_pt": y0,
                "x1_pt": x1,
                "y1_pt": y1,
                "x0_px": rpx(x0),
                "y0_px": rpx(y0),
                "x1_px": rpx(x1),
                "y1_px": rpx(y1),
                "width_pt": rpt(x1 - x0),
                "height_pt": rpt(y1 - y0),
                "source": found[(x0, y0, x1, y1)]["source"],
                "review": "candidate",
            }
        )
    return out


def knockout_regions(areas: Sequence[dict[str, Any]], min_pt: float = 6.0) -> list[dict[str, Any]]:
    """White rectangles painted over shaded bands.

    These are not background: on a grey-banded form a white rect is how the
    field's writable extent is declared. 2551Q item 17 is the case that forced
    this to be a first-class output - the field has no border rules at all, so
    an extractor that discards white paint reports nothing there and the field's
    x-extent has to be re-measured by hand, which is the loop this tool exists
    to end. (Its visible "underline" is typed underscore glyphs, not geometry -
    see the text runs.)
    """
    out = []
    for area in areas:
        gray = area.get("gray")
        if gray is None or gray < WHITE_LEVEL:
            continue
        width = area["x1"] - area["x0"]
        height = area["y1"] - area["y0"]
        if width < min_pt or height < min_pt:
            continue
        out.append(
            {
                "x0_pt": rpt(area["x0"]),
                "y0_pt": rpt(area["y0"]),
                "x1_pt": rpt(area["x1"]),
                "y1_pt": rpt(area["y1"]),
                "x0_px": rpx(area["x0"]),
                "y0_px": rpx(area["y0"]),
                "x1_px": rpx(area["x1"]),
                "y1_px": rpx(area["y1"]),
                "width_pt": rpt(width),
                "height_pt": rpt(height),
            }
        )
    out.sort(key=lambda r: (r["y0_pt"], r["x0_pt"], r["x1_pt"], r["y1_pt"]))
    return out


def coalesce_fills(areas: Sequence[dict[str, Any]], tol: float = 1.0) -> list[dict[str, Any]]:
    """Merge touching same-tone ink areas into regions. White paint is skipped."""
    by_tone: dict[float, list[list[float]]] = defaultdict(list)
    for area in areas:
        gray = area.get("gray")
        if gray is None or gray >= WHITE_LEVEL:
            continue
        by_tone[round(gray, 3)].append([area["x0"], area["y0"], area["x1"], area["y1"]])

    regions: list[dict[str, Any]] = []
    for tone in sorted(by_tone):
        boxes = sorted(by_tone[tone], key=lambda b: (b[1], b[0]))
        merged: list[list[float]] = []
        for box in boxes:
            hit = None
            for candidate in merged:
                if (
                    box[0] <= candidate[2] + tol
                    and box[2] >= candidate[0] - tol
                    and box[1] <= candidate[3] + tol
                    and box[3] >= candidate[1] - tol
                ):
                    hit = candidate
                    break
            if hit is None:
                merged.append(list(box))
            else:
                hit[0] = min(hit[0], box[0])
                hit[1] = min(hit[1], box[1])
                hit[2] = max(hit[2], box[2])
                hit[3] = max(hit[3], box[3])
        changed = True
        while changed:
            changed = False
            for i in range(len(merged)):
                for j in range(i + 1, len(merged)):
                    a, b = merged[i], merged[j]
                    if (
                        b[0] <= a[2] + tol
                        and b[2] >= a[0] - tol
                        and b[1] <= a[3] + tol
                        and b[3] >= a[1] - tol
                    ):
                        a[0] = min(a[0], b[0])
                        a[1] = min(a[1], b[1])
                        a[2] = max(a[2], b[2])
                        a[3] = max(a[3], b[3])
                        del merged[j]
                        changed = True
                        break
                if changed:
                    break
        for box in merged:
            regions.append(
                {
                    "gray": tone,
                    "x0_pt": rpt(box[0]),
                    "y0_pt": rpt(box[1]),
                    "x1_pt": rpt(box[2]),
                    "y1_pt": rpt(box[3]),
                    "x0_px": rpx(box[0]),
                    "y0_px": rpx(box[1]),
                    "x1_px": rpx(box[2]),
                    "y1_px": rpx(box[3]),
                }
            )
    regions.sort(key=lambda r: (r["y0_pt"], r["x0_pt"], r["gray"]))
    return regions


def extract_text(page: "fitz.Page") -> tuple[list[dict[str, Any]], dict[str, int]]:
    payload = page.get_text("dict")
    runs: list[dict[str, Any]] = []
    pathologies = {"rotated": 0, "white": 0, "tiny": 0, "empty": 0}

    for block in payload["blocks"]:
        if block["type"] != 0:
            continue
        for line in block["lines"]:
            direction = line.get("dir", (1.0, 0.0))
            rotated = abs(direction[0] - 1.0) > 1e-3 or abs(direction[1]) > 1e-3
            for span in line["spans"]:
                text = span["text"]
                if not text.strip():
                    pathologies["empty"] += 1
                    continue
                flags = span["flags"]
                font = span["font"]
                color = span.get("color", 0)
                if rotated:
                    pathologies["rotated"] += 1
                if color == 0xFFFFFF:
                    pathologies["white"] += 1
                if span["size"] < 3.0:
                    pathologies["tiny"] += 1
                bbox = span["bbox"]
                runs.append(
                    {
                        "text": text,
                        "bbox_pt": [rpt(v) for v in bbox],
                        "bbox_px": [rpx(v) for v in bbox],
                        "size_pt": rpt(span["size"]),
                        "font": font,
                        "bold": bool(flags & (1 << 4)) or "Bold" in font,
                        "italic": bool(flags & (1 << 1))
                        or "Italic" in font
                        or "Oblique" in font,
                        "color": color,
                        "rotated": rotated,
                    }
                )

    runs.sort(key=lambda r: (r["bbox_pt"][1], r["bbox_pt"][0], r["text"]))
    return runs, pathologies


def extract_images(page: "fitz.Page") -> list[dict[str, Any]]:
    images = []
    for info in page.get_image_info(xrefs=True):
        bbox = info["bbox"]
        images.append(
            {
                "xref": info.get("xref"),
                "bbox_pt": [rpt(v) for v in bbox],
                "bbox_px": [rpx(v) for v in bbox],
                "pixel_width": info.get("width"),
                "pixel_height": info.get("height"),
                "colorspace": info.get("cs-name"),
            }
        )
    images.sort(key=lambda i: (i["bbox_pt"][1], i["bbox_pt"][0], i["xref"] or 0))
    return images


# --------------------------------------------------------------------------
# contract assembly
# --------------------------------------------------------------------------


def build_contract(
    pdf: Path,
    form_code: str,
    revision: str,
    digest: str,
    repo: Path,
) -> tuple[dict[str, Any], dict[str, Any]]:
    started = time.perf_counter()
    doc = fitz.open(pdf)

    if doc.is_form_pdf:
        # Stated as an observation, not a capability. No PDF in this corpus has
        # AcroForm fields; if one ever does, the contract must say so loudly
        # rather than silently ignoring a whole field model.
        acroform_note = "AcroForm fields present - review before trusting geometry"
    else:
        acroform_note = "no AcroForm fields (expected for this corpus)"

    primitives = []
    for page in doc:
        primitives.append(classify_primitives(page))

    sweep = plateau_gap_tol([(p["rules_h"], p["rules_v"]) for p in primitives])
    gap_tol = sweep["selected_gap_tol_pt"]

    try:
        source = pdf.resolve().relative_to(repo.resolve()).as_posix()
    except ValueError:
        source = f"external:{pdf.name}"

    contract: dict[str, Any] = {
        "schema": SCHEMA,
        "form_code": form_code,
        "revision": revision,
        "form_key": f"{form_code}v{revision}",
        "source": {
            "name": pdf.name,
            "path": source,
            "sha256": digest,
            "bytes": pdf.stat().st_size,
        },
        "extractor": {
            "script": "scripts/extract_geometry_contract.py",
            "version": EXTRACTOR_VERSION,
            "pymupdf": fitz.VersionBind,
        },
        "units": {
            "pt": "PDF points (1/72 inch)",
            "px": "144 DPI device pixels; px = pt * 2",
        },
        "notes": [
            "Calibration input only. Never a runtime asset or page background.",
            "Comb and checkbox entries are CANDIDATES requiring human review via "
            "scripts/review_geometry_contract.py. Semantic naming is always human.",
            "predicted_min_tone caps how dark a rule can raster: a sub-pixel rule "
            "cannot ink a full pixel, so black sub-pixel guides read as mid-grey.",
            acroform_note,
        ],
        "coalescing": sweep,
        "page_count": doc.page_count,
        "pages": [],
    }

    for page in doc:
        prim = primitives[page.number]
        merged_h = coalesce(prim["rules_h"], gap_tol)
        merged_v = coalesce(prim["rules_v"], gap_tol)

        rules_h = [emit_rule(r, "h") for r in merged_h]
        rules_v = [emit_rule(r, "v") for r in merged_v]

        tickrun = detect_combs_tickrun(merged_v)
        container = detect_combs_container(merged_h, merged_v, prim["areas"])
        checkboxes = detect_checkboxes(prim["areas"], merged_h, merged_v)
        fills = coalesce_fills(prim["areas"])
        knockouts = knockout_regions(prim["areas"])
        text_runs, pathologies = extract_text(page)
        images = extract_images(page)

        weights = Counter(r["device_px"] for r in rules_h + rules_v)

        contract["pages"].append(
            {
                "page": page.number,
                "size_pt": [rpt(page.rect.width), rpt(page.rect.height)],
                "size_px": [rpx(page.rect.width), rpx(page.rect.height)],
                "counts": {
                    "raw_segments": prim["segments"],
                    "rules_h": len(rules_h),
                    "rules_v": len(rules_v),
                    "joint_fragments": len(prim["joints"]),
                    "white_rects": prim["white_rects"],
                    "curves": prim["curves"],
                    "fill_regions": len(fills),
                    "knockout_regions": len(knockouts),
                    "text_runs": len(text_runs),
                    "comb_candidates_tickrun": len(tickrun),
                    "comb_candidates_container": len(container),
                    "checkbox_candidates": len(checkboxes),
                    "images": len(images),
                },
                "rule_weight_histogram_device_px": {
                    str(k): weights[k] for k in sorted(weights)
                },
                "rules_h": rules_h,
                "rules_v": rules_v,
                "fill_regions": fills,
                "knockout_regions": knockouts,
                "comb_candidates_tickrun": tickrun,
                "comb_candidates_container": container,
                "checkbox_candidates": checkboxes,
                "text_runs": text_runs,
                "text_pathologies": pathologies,
                "images": images,
            }
        )

    doc.close()
    report = {
        "form_key": contract["form_key"],
        "pages": contract["page_count"],
        "gap_tol_pt": gap_tol,
        "seconds": round(time.perf_counter() - started, 4),
        "per_page": [
            {"page": p["page"], **p["counts"]} for p in contract["pages"]
        ],
    }
    return contract, report


def serialize(contract: dict[str, Any]) -> str:
    return json.dumps(contract, indent=1, sort_keys=True, ensure_ascii=False) + "\n"


def write_atomic(path: Path, payload: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=path.parent, delete=False
    ) as handle:
        handle.write(payload)
        temp = Path(handle.name)
    os.replace(temp, path)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Extract a geometry contract from a pinned official BIR PDF.",
    )
    parser.add_argument("--repo", default=".", help="repository root")
    parser.add_argument("--form-code", required=True, help="e.g. 2551Q")
    parser.add_argument("--revision", required=True, help="e.g. 2018")
    parser.add_argument("--pdf", required=True, type=Path, help="pinned official PDF")
    parser.add_argument(
        "--expected-sha256",
        required=True,
        help="64 lowercase hex characters; extraction fails closed on mismatch",
    )
    parser.add_argument(
        "--output",
        "--out",
        dest="output",
        required=True,
        type=Path,
        help="output directory (contract.json is written inside it)",
    )
    parser.add_argument(
        "--check-only",
        action="store_true",
        help="verify the existing contract matches a fresh extraction; write nothing",
    )
    args = parser.parse_args(argv)

    repo = Path(args.repo).resolve()
    pdf = args.pdf.resolve()
    if not pdf.is_file():
        print(f"official PDF not found: {pdf}", file=sys.stderr)
        return 2

    form_code = args.form_code.strip().upper()
    revision = args.revision.strip()
    if not FORM_CODE_RE.fullmatch(form_code):
        print("form code must be letters, digits or hyphens", file=sys.stderr)
        return 2
    if not REVISION_RE.fullmatch(revision):
        print("revision contains unsupported characters", file=sys.stderr)
        return 2

    expected = args.expected_sha256.strip().lower()
    if not SHA256_RE.fullmatch(expected):
        print("--expected-sha256 must be 64 lowercase hex characters", file=sys.stderr)
        return 2

    actual = sha256_file(pdf)
    if actual != expected:
        print(
            f"official PDF SHA-256 mismatch: expected {expected}, got {actual}",
            file=sys.stderr,
        )
        return 1

    contract, report = build_contract(pdf, form_code, revision, actual, repo)
    payload = serialize(contract)

    destination = (
        args.output if args.output.is_absolute() else repo / args.output
    ).resolve() / "contract.json"

    if args.check_only:
        if not destination.is_file():
            print(f"no contract to check at {destination}", file=sys.stderr)
            return 1
        current = destination.read_text(encoding="utf-8")
        if current != payload:
            print(f"contract is stale: {destination}", file=sys.stderr)
            return 1
        report["check_only"] = "match"
    else:
        write_atomic(destination, payload)
        report["written"] = str(destination)

    print(json.dumps(report, indent=1, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
