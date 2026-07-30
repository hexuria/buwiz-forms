#!/usr/bin/env python3
"""Independent vector referee for printed comb compartments.

The two existing compartment measurements share too much context to decide
which one is wrong:

* lattice.py classifies extracted rules and builds the emitted slot geometry;
* audit.py asks MuPDF for drawing objects inside a lattice-owned cell.

This tool is deliberately a third implementation.  It invokes Poppler's
``pdftocairo -svg`` as a separate process, parses only the painted vector
geometry in that SVG with the Python standard library, and never imports
audit.py, extract.py, lattice.py, or PyMuPDF.

The layout supplies a *subject* (page and cell rectangle) and the already
recognised divider anchors.  It does not supply the answer.  Between adjacent
recognised anchors, or immediately beyond the anchored run, the referee admits
a missing boundary only when:

1. the source-space gap is an integral number of the measured base pitch;
2. Poppler paints every missing pitch position in one common source band; and
3. every already-recognised anchor is also painted in that band.

An outward boundary must continue the measured source pitch, or be the sole
boundary that symmetrically divides the remaining edge interval.  Cell-edge
ink is never counted as an interior divider.  These constraints make the check
useful for both disputed heavy group separators and truncated first/last ticks
without turning unrelated verticals in a broad mixed cell into character
boxes.  A partial pattern, unsupported vector geometry, a clipped candidate,
missing provenance, or competing source bands is UNEVALUABLE -- never a pass.

Raster output is not produced and cannot affect a verdict.

Examples:

    python3 tools/formgen/comb_referee.py --self-test
    python3 tools/formgen/comb_referee.py --only 1707 \
        --out build/comb-referee-1707.json
    python3 tools/formgen/comb_referee.py \
        --out build/comb-referee.json
"""

from __future__ import annotations

import argparse
import dataclasses
import hashlib
import html.parser
import json
import math
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from collections.abc import Iterable, Sequence
from typing import Any


HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent

REPORT_VERSION = 1
EXPECTED_FORMS = 51
EXPECTED_COMBS = 4442
# Corpus identity pins, not geometry exceptions: every slug follows the same
# parser and decision rules. A substituted/missing form must not pass merely
# because the replacement keeps the two aggregate counts unchanged.
EXPECTED_COMBS_BY_SLUG = {
    "0605-1999": 21,
    "0619e-2018": 60,
    "0619f-2018": 64,
    "0620-2019": 60,
    "1600-pt-2018": 95,
    "1600-vt-2018": 95,
    "1600wp-2010": 17,
    "1601-fq-2020": 106,
    "1601c-2018": 132,
    "1601eq-2019": 99,
    "1602q-2019": 175,
    "1603q-2018": 78,
    "1604c-2018": 19,
    "1604e-2018": 15,
    "1604f-2018": 16,
    "1606-2018": 76,
    "1621-2019": 69,
    "1700-2018": 143,
    "1701-2018": 283,
    "1701-2018-attachment": 123,
    "1701-2018-conso": 40,
    "1701a-2018": 134,
    "1701ms-2024": 136,
    "1701q-2018": 128,
    "1702ex-2018": 149,
    "1702mx-2018c": 117,
    "1702mx-2018c-attachment": 108,
    "1702q-2018": 106,
    "1702rt-2018c": 205,
    "1706-2018": 81,
    "1707-2021": 113,
    "1707a-2021": 97,
    "1709-2020": 19,
    "1800-2018": 108,
    "1801-2018": 102,
    "2000-dst-2018": 131,
    "2000-ot-2018": 75,
    "2200a-2020": 42,
    "2200c-2018": 40,
    "2200m-2018": 86,
    "2200p-2020": 42,
    "2200s-2018": 66,
    "2200t-2022": 90,
    "2316-2021": 28,
    "2550-ds-2025": 77,
    "2550m-2007": 23,
    "2550q-2024": 144,
    "2551m-2002": 15,
    "2551q-2018": 105,
    "2552-2018": 73,
    "2553-1999": 16,
}
if (len(EXPECTED_COMBS_BY_SLUG) != EXPECTED_FORMS
        or sum(EXPECTED_COMBS_BY_SLUG.values()) != EXPECTED_COMBS):
    raise RuntimeError("comb referee corpus pins are internally inconsistent")

# This is verify.py's fixed position tolerance.  It is copied as a bound, not
# exposed as a CLI knob: changing it here would make the referee a third
# independently tunable answer rather than an adjudicator.
POSITION_TOL_PT = 0.25

_NUMBER = r"[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?"
_TRANSFORM_RE = re.compile(r"([A-Za-z]+)\s*\(([^)]*)\)")
_PATH_TOKEN_RE = re.compile(rf"[A-Za-z]|{_NUMBER}")
_CELL_RE = re.compile(r"^p\d+c\d+$")
_CELL_SLOT_RE = re.compile(r"^(p\d+c\d+)-s(\d+)$")
_PAGE_RE = re.compile(r"^page-(\d+)$")


class RefereeError(RuntimeError):
    """A form or corpus cannot be measured with complete provenance."""


@dataclasses.dataclass(frozen=True)
class Matrix:
    """SVG affine matrix: x'=a*x+c*y+e, y'=b*x+d*y+f."""

    a: float = 1.0
    b: float = 0.0
    c: float = 0.0
    d: float = 1.0
    e: float = 0.0
    f: float = 0.0

    def then(self, child: "Matrix") -> "Matrix":
        """Return this matrix applied after ``child`` (self * child)."""
        return Matrix(
            self.a * child.a + self.c * child.b,
            self.b * child.a + self.d * child.b,
            self.a * child.c + self.c * child.d,
            self.b * child.c + self.d * child.d,
            self.a * child.e + self.c * child.f + self.e,
            self.b * child.e + self.d * child.f + self.f,
        )

    def point(self, x: float, y: float) -> tuple[float, float]:
        return (self.a * x + self.c * y + self.e,
                self.b * x + self.d * y + self.f)

    def stroke_scale(self) -> float:
        # Poppler's rule transforms are scale/flip matrices.  The geometric
        # mean also behaves conservatively for a rotated or mildly skewed rule.
        return math.sqrt(abs(self.a * self.d - self.b * self.c))


@dataclasses.dataclass(frozen=True)
class Paint:
    x0: float
    y0: float
    x1: float
    y1: float
    tone: float
    order: int
    kind: str
    element: str
    clipped: bool = False

    @property
    def width(self) -> float:
        return self.x1 - self.x0

    @property
    def height(self) -> float:
        return self.y1 - self.y0

    @property
    def cx(self) -> float:
        return (self.x0 + self.x1) / 2

    def covers(self, x: float, y: float) -> bool:
        return self.x0 <= x <= self.x1 and self.y0 <= y <= self.y1


@dataclasses.dataclass(frozen=True)
class UnsupportedRegion:
    x0: float
    y0: float
    x1: float
    y1: float
    reason: str
    element: str


@dataclasses.dataclass
class SvgPage:
    width: float
    height: float
    paints: list[Paint]
    unsupported: list[UnsupportedRegion]
    sha256: str


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def canonical_digest(value: Any) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":"),
                         ensure_ascii=False).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def parse_transform(text: str | None) -> Matrix:
    if not text:
        return Matrix()
    result = Matrix()
    consumed = ""
    for match in _TRANSFORM_RE.finditer(text):
        consumed += match.group(0)
        name = match.group(1)
        values = [float(v) for v in re.findall(_NUMBER, match.group(2))]
        if name == "matrix" and len(values) == 6:
            op = Matrix(*values)
        elif name == "translate" and len(values) in (1, 2):
            op = Matrix(e=values[0], f=values[1] if len(values) == 2 else 0.0)
        elif name == "scale" and len(values) in (1, 2):
            op = Matrix(a=values[0], d=values[1] if len(values) == 2 else values[0])
        elif name == "rotate" and len(values) in (1, 3):
            radians = math.radians(values[0])
            rotation = Matrix(a=math.cos(radians), b=math.sin(radians),
                              c=-math.sin(radians), d=math.cos(radians))
            if len(values) == 3:
                cx, cy = values[1:]
                op = (Matrix(e=cx, f=cy).then(rotation)
                      .then(Matrix(e=-cx, f=-cy)))
            else:
                op = rotation
        else:
            raise RefereeError(f"unsupported SVG transform: {match.group(0)}")
        result = result.then(op)
    if re.sub(r"[\s,]+", "", text).lower() != re.sub(
            r"[\s,]+", "", consumed).lower():
        raise RefereeError(f"unparsed SVG transform: {text}")
    return result


def parse_style(element: ET.Element, inherited: dict[str, str]) -> dict[str, str]:
    style = dict(inherited)
    inline = {}
    for part in element.get("style", "").split(";"):
        if ":" in part:
            key, value = part.split(":", 1)
            inline[key.strip()] = value.strip()
    for key in ("fill", "fill-opacity", "stroke", "stroke-opacity",
                "stroke-width", "display", "visibility"):
        if key in element.attrib:
            style[key] = element.attrib[key]
        if key in inline:
            style[key] = inline[key]
    local_opacity = inline.get(
        "opacity", element.get("opacity", "1"))
    style["_cumulative-opacity"] = str(
        float(inherited.get("_cumulative-opacity", "1"))
        * float(local_opacity)
    )
    return style


def colour_tone(value: str | None) -> float | None:
    if value is None or value.strip().lower() in ("none", "transparent"):
        return None
    text = value.strip().lower()
    if text in ("black",):
        return 0.0
    if text in ("white",):
        return 1.0
    if text.startswith("#"):
        raw = text[1:]
        if len(raw) == 3:
            raw = "".join(ch * 2 for ch in raw)
        if len(raw) != 6:
            raise RefereeError(f"unsupported SVG colour: {value}")
        channels = [int(raw[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    else:
        match = re.fullmatch(r"rgb\((.*)\)", text)
        if not match:
            raise RefereeError(f"unsupported SVG colour: {value}")
        parts = [p.strip() for p in match.group(1).split(",")]
        if len(parts) != 3:
            raise RefereeError(f"unsupported SVG colour: {value}")
        channels = []
        for part in parts:
            if part.endswith("%"):
                channels.append(float(part[:-1]) / 100)
            else:
                channels.append(float(part) / 255)
    return max(0.0, min(1.0, 0.2126 * channels[0]
                        + 0.7152 * channels[1] + 0.0722 * channels[2]))


def effective_tone(style: dict[str, str], key: str) -> float | None:
    tone = colour_tone(style.get(key))
    if tone is None:
        return None
    opacity = float(style.get("_cumulative-opacity", "1"))
    opacity *= float(style.get(f"{key}-opacity", "1"))
    # Composite over white paper; this preserves decorative greys rather than
    # treating every non-white paint as black.
    return 1 - opacity * (1 - tone)


def effective_opacity(style: dict[str, str], key: str) -> float:
    return (float(style.get("_cumulative-opacity", "1"))
            * float(style.get(f"{key}-opacity", "1")))


def path_subpaths(data: str) -> tuple[list[tuple[list[tuple[float, float]], bool]],
                                      list[tuple[float, float]] | None]:
    """Parse Poppler's axis-aligned M/L/H/V/Z paths.

    The second return value is an approximate point cloud when an unsupported
    curve/arc command appears.  A narrow unsupported region intersecting a comb
    makes that cell UNEVALUABLE rather than being silently discarded.
    """
    tokens = _PATH_TOKEN_RE.findall(data)
    subpaths: list[tuple[list[tuple[float, float]], bool]] = []
    all_points: list[tuple[float, float]] = []
    current: list[tuple[float, float]] = []
    command: str | None = None
    cursor = (0.0, 0.0)
    start = (0.0, 0.0)
    i = 0
    unsupported = False

    def number(index: int) -> float:
        if index >= len(tokens) or tokens[index].isalpha():
            raise ValueError("missing path coordinate")
        return float(tokens[index])

    try:
        while i < len(tokens):
            token = tokens[i]
            if token.isalpha():
                command = token
                i += 1
            if command is None:
                raise ValueError("path starts without command")
            relative = command.islower()
            op = command.upper()
            if op == "Z":
                if current:
                    subpaths.append((current, True))
                    current = []
                cursor = start
                command = None
                continue
            if op in ("C", "S", "Q", "T", "A"):
                unsupported = True
                # Preserve a conservative approximate point cloud.  Poppler's
                # direct rule geometry is M/L; these are normally glyph-like or
                # diagonal artwork.
                values: list[float] = []
                while i < len(tokens) and not tokens[i].isalpha():
                    values.append(float(tokens[i]))
                    i += 1
                for x, y in zip(values[0::2], values[1::2]):
                    if relative:
                        x += cursor[0]
                        y += cursor[1]
                    all_points.append((x, y))
                continue
            if op in ("M", "L"):
                x, y = number(i), number(i + 1)
                i += 2
                if relative:
                    x += cursor[0]
                    y += cursor[1]
                if op == "M":
                    if current:
                        subpaths.append((current, False))
                    current = [(x, y)]
                    start = (x, y)
                    command = "l" if relative else "L"
                else:
                    current.append((x, y))
                cursor = (x, y)
                all_points.append(cursor)
            elif op == "H":
                x = number(i)
                i += 1
                if relative:
                    x += cursor[0]
                cursor = (x, cursor[1])
                current.append(cursor)
                all_points.append(cursor)
            elif op == "V":
                y = number(i)
                i += 1
                if relative:
                    y += cursor[1]
                cursor = (cursor[0], y)
                current.append(cursor)
                all_points.append(cursor)
            else:
                unsupported = True
                i += 1
    except (ValueError, IndexError):
        unsupported = True

    if current:
        subpaths.append((current, False))
    return subpaths, all_points if unsupported else None


def bbox(points: Sequence[tuple[float, float]]) -> tuple[float, float, float, float]:
    xs = [point[0] for point in points]
    ys = [point[1] for point in points]
    return min(xs), min(ys), max(xs), max(ys)


def transformed_bbox(points: Sequence[tuple[float, float]],
                     transform: Matrix) -> tuple[float, float, float, float]:
    return bbox([transform.point(x, y) for x, y in points])


def is_axis_aligned_rectangle(points: Sequence[tuple[float, float]]) -> bool:
    """Whether a closed point sequence is exactly an axis-aligned rectangle."""
    cleaned: list[tuple[float, float]] = []
    for point in points:
        if not cleaned or (abs(point[0] - cleaned[-1][0]) > 1e-6
                           or abs(point[1] - cleaned[-1][1]) > 1e-6):
            cleaned.append(point)
    if len(cleaned) > 1 and (abs(cleaned[0][0] - cleaned[-1][0]) <= 1e-6
                             and abs(cleaned[0][1] - cleaned[-1][1]) <= 1e-6):
        cleaned.pop()
    if len(cleaned) < 4:
        return False
    x0, y0, x1, y1 = bbox(cleaned)
    if x1 - x0 <= 1e-6 or y1 - y0 <= 1e-6:
        return False
    corners = {
        (round(x0, 6), round(y0, 6)),
        (round(x0, 6), round(y1, 6)),
        (round(x1, 6), round(y0, 6)),
        (round(x1, 6), round(y1, 6)),
    }
    if {(round(x, 6), round(y, 6)) for x, y in cleaned} != corners:
        return False
    return all(
        abs(b[0] - a[0]) <= 1e-6 or abs(b[1] - a[1]) <= 1e-6
        for a, b in zip(cleaned, [*cleaned[1:], cleaned[0]])
    )


def attr_float(element: ET.Element, name: str, default: float = 0.0) -> float:
    raw = element.get(name)
    if raw is None:
        return default
    match = re.match(_NUMBER, raw)
    if not match:
        raise RefereeError(f"non-numeric SVG {name}: {raw}")
    return float(match.group(0))


def parse_svg(path: pathlib.Path) -> SvgPage:
    root = ET.parse(path).getroot()
    view_box = [float(v) for v in re.findall(_NUMBER, root.get("viewBox", ""))]
    if len(view_box) != 4 or view_box[0] != 0 or view_box[1] != 0:
        raise RefereeError(f"unsupported SVG viewBox: {root.get('viewBox')}")
    width, height = view_box[2], view_box[3]
    paints: list[Paint] = []
    unsupported: list[UnsupportedRegion] = []
    order = 0
    definitions = {
        element.get("id"): element for element in root.iter()
        if element.get("id")
    }
    xlink_href = "{http://www.w3.org/1999/xlink}href"

    def add_rect(box_value: tuple[float, float, float, float], tone: float,
                 kind: str, element_id: str, clipped: bool) -> None:
        nonlocal order
        x0, y0, x1, y1 = box_value
        if x1 <= x0 or y1 <= y0:
            return
        paints.append(Paint(round(x0, 6), round(y0, 6), round(x1, 6),
                            round(y1, 6), round(tone, 8), order, kind,
                            element_id, clipped))
        order += 1

    def walk(element: ET.Element, parent_matrix: Matrix,
             inherited: dict[str, str], in_defs: bool = False,
             clipped: bool = False) -> None:
        tag = element.tag.rsplit("}", 1)[-1]
        if tag == "defs":
            return
        local = parse_transform(element.get("transform"))
        matrix = parent_matrix.then(local)
        style = parse_style(element, inherited)
        if style.get("display") == "none" or style.get("visibility") == "hidden":
            return
        clipped_here = clipped or any(
            key in element.attrib for key in ("clip-path", "mask", "filter"))
        element_id = element.get("id") or f"{tag}-{len(paints) + len(unsupported)}"
        if "filter" in element.attrib:
            unsupported.append(UnsupportedRegion(
                0.0, 0.0, width, height,
                "SVG filter has unbounded paint effects", element_id))

        if tag == "path":
            subpaths, approximate = path_subpaths(element.get("d", ""))
            fill = effective_tone(style, "fill")
            stroke = effective_tone(style, "stroke")
            fill_ambiguous = (
                clipped_here or effective_opacity(style, "fill") < 1.0 - 1e-8)
            stroke_ambiguous = (
                clipped_here or effective_opacity(style, "stroke") < 1.0 - 1e-8)
            stroke_width = float(style.get("stroke-width", "1")) * matrix.stroke_scale()
            for points, closed in subpaths:
                if len(points) < 2:
                    continue
                transformed = [matrix.point(x, y) for x, y in points]
                if closed and fill is not None:
                    if is_axis_aligned_rectangle(transformed):
                        add_rect(bbox(transformed), fill, "fill",
                                 element_id, fill_ambiguous)
                    else:
                        x0, y0, x1, y1 = bbox(transformed)
                        unsupported.append(UnsupportedRegion(
                            x0, y0, x1, y1,
                            "non-rectangular closed SVG fill", element_id))
                if stroke is not None:
                    half = stroke_width / 2
                    pairs = list(zip(transformed, transformed[1:]))
                    if closed:
                        pairs.append((transformed[-1], transformed[0]))
                    for (x0, y0), (x1, y1) in pairs:
                        if abs(x1 - x0) <= 1e-6 and abs(y1 - y0) > 0:
                            add_rect((min(x0, x1) - half, min(y0, y1),
                                     max(x0, x1) + half, max(y0, y1)),
                                     stroke, "stroke", element_id,
                                     stroke_ambiguous)
                        elif (abs(y1 - y0) <= 1e-6
                              and abs(x1 - x0) > 0):
                            add_rect((min(x0, x1), min(y0, y1) - half,
                                     max(x0, x1), max(y0, y1) + half),
                                     stroke, "stroke", element_id,
                                     stroke_ambiguous)
                        elif abs(x1 - x0) > 0 or abs(y1 - y0) > 0:
                            unsupported.append(UnsupportedRegion(
                                min(x0, x1) - half, min(y0, y1) - half,
                                max(x0, x1) + half, max(y0, y1) + half,
                                "diagonal SVG path stroke", element_id))
            if approximate:
                # Curves are bounded by their controls, but SVG arcs and
                # malformed/unknown commands are not represented by the
                # approximate point cloud above. Without a complete parser the
                # only honest bound is the page.
                unsupported.append(UnsupportedRegion(
                    0.0, 0.0, width, height,
                    "unsupported SVG path command", element_id))
        elif tag == "rect":
            x, y = attr_float(element, "x"), attr_float(element, "y")
            w, h = attr_float(element, "width"), attr_float(element, "height")
            points = [matrix.point(x, y), matrix.point(x + w, y),
                      matrix.point(x + w, y + h), matrix.point(x, y + h)]
            fill = effective_tone(style, "fill")
            stroke = effective_tone(style, "stroke")
            fill_ambiguous = (
                clipped_here or effective_opacity(style, "fill") < 1.0 - 1e-8)
            stroke_ambiguous = (
                clipped_here or effective_opacity(style, "stroke") < 1.0 - 1e-8)
            if not is_axis_aligned_rectangle(points):
                x0, y0, x1, y1 = bbox(points)
                unsupported.append(UnsupportedRegion(
                    x0, y0, x1, y1,
                    "transformed SVG rect is not axis-aligned", element_id))
                fill = None
                stroke = None
            if fill is not None:
                add_rect(bbox(points), fill, "fill", element_id, fill_ambiguous)
            if stroke is not None:
                half = float(style.get("stroke-width", "1")) * matrix.stroke_scale() / 2
                box_value = bbox(points)
                add_rect((box_value[0] - half, box_value[1],
                          box_value[0] + half, box_value[3]),
                         stroke, "stroke", element_id, stroke_ambiguous)
                add_rect((box_value[2] - half, box_value[1],
                          box_value[2] + half, box_value[3]),
                         stroke, "stroke", element_id, stroke_ambiguous)
                add_rect((box_value[0], box_value[1] - half,
                          box_value[2], box_value[1] + half),
                         stroke, "stroke", element_id, stroke_ambiguous)
                add_rect((box_value[0], box_value[3] - half,
                          box_value[2], box_value[3] + half),
                         stroke, "stroke", element_id, stroke_ambiguous)
        elif tag == "line":
            p0 = matrix.point(attr_float(element, "x1"), attr_float(element, "y1"))
            p1 = matrix.point(attr_float(element, "x2"), attr_float(element, "y2"))
            stroke = effective_tone(style, "stroke")
            stroke_ambiguous = (
                clipped_here or effective_opacity(style, "stroke") < 1.0 - 1e-8)
            if stroke is not None and abs(p1[0] - p0[0]) <= 1e-6:
                half = float(style.get("stroke-width", "1")) * matrix.stroke_scale() / 2
                add_rect((min(p0[0], p1[0]) - half, min(p0[1], p1[1]),
                          max(p0[0], p1[0]) + half, max(p0[1], p1[1])),
                         stroke, "stroke", element_id, stroke_ambiguous)
            elif stroke is not None and abs(p1[1] - p0[1]) <= 1e-6:
                half = float(style.get("stroke-width", "1")) * matrix.stroke_scale() / 2
                add_rect((min(p0[0], p1[0]), min(p0[1], p1[1]) - half,
                          max(p0[0], p1[0]), max(p0[1], p1[1]) + half),
                         stroke, "stroke", element_id, stroke_ambiguous)
            elif stroke is not None:
                half = float(style.get("stroke-width", "1")) * matrix.stroke_scale() / 2
                unsupported.append(UnsupportedRegion(
                    min(p0[0], p1[0]) - half, min(p0[1], p1[1]) - half,
                    max(p0[0], p1[0]) + half, max(p0[1], p1[1]) + half,
                    "diagonal SVG line", element_id))
        elif tag == "image":
            x, y = attr_float(element, "x"), attr_float(element, "y")
            w, h = attr_float(element, "width"), attr_float(element, "height")
            if w > 0 and h > 0:
                x0, y0, x1, y1 = transformed_bbox(
                    [(x, y), (x + w, y), (x + w, y + h), (x, y + h)],
                    matrix)
                unsupported.append(UnsupportedRegion(
                    x0, y0, x1, y1, "embedded raster image", element_id))
        elif tag == "use":
            href = element.get(xlink_href) or element.get("href") or ""
            if href.startswith("#glyph-"):
                pass
            else:
                referenced = definitions.get(href.removeprefix("#"))
                if referenced is None:
                    unsupported.append(UnsupportedRegion(
                        0.0, 0.0, width, height,
                        f"unresolved SVG use reference: {href}", element_id))
                elif referenced.tag.rsplit("}", 1)[-1] == "image":
                    x = attr_float(referenced, "x") + attr_float(element, "x")
                    y = attr_float(referenced, "y") + attr_float(element, "y")
                    w = attr_float(referenced, "width")
                    h = attr_float(referenced, "height")
                    ref_matrix = matrix.then(parse_transform(
                        referenced.get("transform")))
                    x0, y0, x1, y1 = transformed_bbox(
                        [(x, y), (x + w, y), (x + w, y + h), (x, y + h)],
                        ref_matrix)
                    unsupported.append(UnsupportedRegion(
                        x0, y0, x1, y1,
                        f"embedded raster use: {href}", element_id))
                else:
                    unsupported.append(UnsupportedRegion(
                        0.0, 0.0, width, height,
                        f"unsupported SVG use reference: {href}", element_id))
        elif tag in ("circle", "ellipse", "polygon", "polyline"):
            points: list[tuple[float, float]] = []
            if tag == "circle":
                cx, cy = attr_float(element, "cx"), attr_float(element, "cy")
                rx = ry = attr_float(element, "r")
                points = [(cx - rx, cy - ry), (cx + rx, cy + ry)]
            elif tag == "ellipse":
                cx, cy = attr_float(element, "cx"), attr_float(element, "cy")
                rx, ry = attr_float(element, "rx"), attr_float(element, "ry")
                points = [(cx - rx, cy - ry), (cx + rx, cy + ry)]
            else:
                values = [float(value) for value in re.findall(
                    _NUMBER, element.get("points", ""))]
                points = list(zip(values[0::2], values[1::2]))
            if points:
                x0, y0, x1, y1 = transformed_bbox(points, matrix)
                unsupported.append(UnsupportedRegion(
                    x0, y0, x1, y1,
                    f"unsupported SVG {tag}", element_id))
        elif tag not in (
                "svg", "g", "a", "switch", "metadata", "title", "desc",
                "symbol", "clipPath", "mask"):
            unsupported.append(UnsupportedRegion(
                0.0, 0.0, width, height,
                f"unsupported SVG element: {tag}", element_id))

        # Glyph uses are text, not vector compartment geometry. Other uses and
        # images were recorded above as unsupported visible regions.
        if tag not in ("use", "image", "symbol", "clipPath", "mask"):
            for child in element:
                walk(child, matrix, style, in_defs, clipped_here)

    walk(root, Matrix(), {"fill": "black", "stroke": "none",
                          "fill-opacity": "1", "stroke-opacity": "1",
                          "opacity": "1"})
    return SvgPage(width, height, paints, unsupported, sha256_file(path))


def source_pdf(layout: dict[str, Any], source_root: pathlib.Path) -> pathlib.Path:
    source = layout.get("source") or {}
    filename = str(source.get("file", "")).split(":", 1)[-1]
    expected = source.get("sha256")
    if not filename or not expected:
        raise RefereeError("layout has no pinned source filename/hash")
    matches = []
    for candidate in sorted(source_root.rglob(filename)):
        if candidate.is_file() and sha256_file(candidate) == expected:
            matches.append(candidate)
    if not matches:
        raise RefereeError(f"source PDF not found with pinned hash: {filename}")
    # Byte-identical duplicate inputs are semantically the same source.  The
    # lexicographic path is deterministic and every match is reported.
    return matches[0]


def poppler_identity() -> dict[str, str]:
    binary = shutil.which("pdftocairo")
    if binary is None:
        raise RefereeError("pdftocairo is not installed")
    proc = subprocess.run([binary, "-v"], capture_output=True, text=True)
    version = (proc.stdout + proc.stderr).strip().splitlines()
    if proc.returncode != 0 or not version:
        raise RefereeError("pdftocairo -v failed")
    return {
        "version": version[0],
        "binary_path": str(pathlib.Path(binary).resolve()),
        "binary_sha256": sha256_file(pathlib.Path(binary)),
    }


def render_svg_page(binary: str, pdf: pathlib.Path, page_number: int,
                    directory: pathlib.Path) -> pathlib.Path:
    output = directory / f"page-{page_number}.svg"
    proc = subprocess.run(
        [binary, "-svg", "-f", str(page_number), "-l", str(page_number),
         str(pdf), str(output)],
        capture_output=True, text=True,
    )
    if proc.returncode != 0 or not output.is_file():
        detail = (proc.stdout + proc.stderr).strip()
        raise RefereeError(f"pdftocairo page {page_number} failed: {detail}")
    return output


class SlotParser(html.parser.HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.template_depth = 0
        self.div_stack: list[str | None] = []
        self.physical_slots: dict[str, list[int]] = {}
        self.editable_slots: dict[str, list[int]] = {}
        self.comb_containers: set[str] = set()
        self.root: dict[str, str | None] | None = None
        self.pages: list[int] = []
        self.page_geometry: list[tuple[int, float, float]] = []

    def handle_starttag(self, tag: str,
                        attrs: list[tuple[str, str | None]]) -> None:
        values = dict(attrs)
        if tag == "template":
            self.template_depth += 1
            return
        if self.template_depth:
            return
        if tag == "html":
            if self.root is not None:
                raise RefereeError("HTML has more than one root element")
            self.root = values
            return
        if tag == "div":
            parent_cell = self.div_stack[-1] if self.div_stack else None
            identifier = values.get("id") or ""
            cell = parent_cell
            if (_CELL_RE.fullmatch(identifier)
                    and values.get("data-field-kind") == "comb"):
                cell = identifier
                self.comb_containers.add(identifier)
            self.div_stack.append(cell)
            page_match = _PAGE_RE.fullmatch(identifier)
            if page_match and "page" in (values.get("class") or "").split():
                page_index = int(page_match.group(1))
                self.pages.append(page_index)
                style = values.get("style") or ""
                width_match = re.search(
                    rf"(?:^|;)\s*width\s*:\s*({_NUMBER})pt(?:;|$)", style)
                height_match = re.search(
                    rf"(?:^|;)\s*height\s*:\s*({_NUMBER})pt(?:;|$)", style)
                if width_match is None or height_match is None:
                    raise RefereeError(
                        f"HTML page {page_index} has no exact point geometry")
                self.page_geometry.append((
                    page_index, float(width_match.group(1)),
                    float(height_match.group(1)),
                ))
            slot = values.get("data-slot")
            if cell and slot is not None and "s" in (
                    values.get("class") or "").split():
                try:
                    index = int(slot)
                except ValueError:
                    index = -1
                self.physical_slots.setdefault(cell, []).append(index)
            return
        if tag != "input":
            return
        slot = values.get("data-slot-index")
        identifier = values.get("id") or ""
        match = _CELL_SLOT_RE.fullmatch(identifier)
        if slot is None or match is None:
            return
        try:
            index = int(slot)
        except ValueError:
            index = -1
        if index != int(match.group(2)):
            index = -1
        self.editable_slots.setdefault(match.group(1), []).append(index)

    def handle_endtag(self, tag: str) -> None:
        if self.template_depth:
            if tag == "template":
                self.template_depth -= 1
            return
        if tag == "div":
            if not self.div_stack:
                raise RefereeError("HTML has an unmatched closing div")
            self.div_stack.pop()


def emitted_slots(path: pathlib.Path) -> dict[str, dict[str, Any]]:
    parser = SlotParser()
    parser.feed(path.read_text(encoding="utf-8"))
    parser.close()
    if parser.template_depth or parser.div_stack:
        raise RefereeError("HTML ended with unclosed template/div elements")
    return slot_records(parser)


def slot_records(parser: SlotParser) -> dict[str, dict[str, Any]]:
    result = {}
    for cell in sorted(parser.comb_containers):
        physical = parser.physical_slots.get(cell, [])
        ordered = sorted(physical)
        editable = sorted(parser.editable_slots.get(cell, ()))
        physical_set = set(physical)
        result[cell] = {
            "count": len(physical),
            "indexes": ordered,
            "editable_indexes": editable,
            "valid": (
                len(physical) == len(set(physical))
                and -1 not in physical
                and ordered == list(range(len(physical)))
                and len(editable) == len(set(editable))
                and -1 not in editable
                and all(index in physical_set for index in editable)
            ),
        }
    return result


def relocated_cells(data: dict[str, Any]) -> set[str]:
    cells: set[str] = set()
    for region in data.get("inline") or ():
        cells.update(region.get("cell_ids") or ())
    return cells


def visible_segments(paint: Paint, y: float,
                     all_paints: Sequence[Paint]
                     ) -> list[tuple[float, float, bool]]:
    """Final visible x-segments of one rectangle at ``y``.

    Rectangles are the only supported paint primitive. Later paint of the same
    tone preserves the candidate's ink; later paint of another tone removes
    the covered interval. Partial or clipped occlusion is retained as explicit
    ambiguity instead of sampling one convenient point through it.
    """
    if not (paint.y0 <= y <= paint.y1) or paint.tone >= 1.0:
        return []
    segments = [(paint.x0, paint.x1, paint.clipped)]
    partial_or_clipped = paint.clipped
    for later in all_paints:
        if later.order <= paint.order or not (later.y0 <= y <= later.y1):
            continue
        revised: list[tuple[float, float, bool]] = []
        for left, right, ambiguous in segments:
            overlap_left = max(left, later.x0)
            overlap_right = min(right, later.x1)
            if overlap_right <= overlap_left:
                revised.append((left, right, ambiguous))
                continue
            if later.clipped:
                partial_or_clipped = True
                revised.append((left, right, True))
                continue
            if overlap_left > left:
                revised.append((left, overlap_left, True))
            if overlap_right < right:
                revised.append((overlap_right, right, True))
            if overlap_left > left or overlap_right < right:
                partial_or_clipped = True
        segments = revised
        if not segments:
            break
    if partial_or_clipped:
        return [(left, right, True) for left, right, _ in segments]
    return segments


def merge_centres(paints: Sequence[Paint], y: float,
                  all_paints: Sequence[Paint]) -> list[dict[str, Any]]:
    active: list[tuple[float, float, bool, Paint]] = []
    for paint in paints:
        active.extend(
            (left, right, ambiguous, paint)
            for left, right, ambiguous in visible_segments(
                paint, y, all_paints)
        )
    active.sort(key=lambda item: (item[0], item[1], item[3].order))
    groups: list[list[tuple[float, float, bool, Paint]]] = []
    for segment in active:
        if (groups
                and segment[0] <= max(item[1] for item in groups[-1]) + 1e-6):
            groups[-1].append(segment)
        else:
            groups.append([segment])
    return [{
        "x": round((min(item[0] for item in group)
                    + max(item[1] for item in group)) / 2, 6),
        "x0": round(min(item[0] for item in group), 6),
        "x1": round(max(item[1] for item in group), 6),
        "tone": round(min(item[3].tone for item in group), 8),
        "elements": sorted({item[3].element for item in group}),
        "clipped": any(item[2] for item in group),
    } for group in groups]


def near(value: float, target: float) -> bool:
    return abs(value - target) <= POSITION_TOL_PT


def classify_band(cell: dict[str, Any], page: SvgPage) -> dict[str, Any]:
    comb = cell["comb"]
    anchors = [float(value) for value in comb.get("divider_x") or ()]
    if not anchors:
        return {"status": "unevaluable",
                "reason": "no recognised divider anchors; one compartment is unproven"}
    pitch = float(comb.get("pitch_pt") or 0)
    if pitch <= 0:
        return {"status": "unevaluable", "reason": "no positive measured pitch"}
    try:
        divider_tone = float(comb["divider_gray"])
    except (KeyError, TypeError, ValueError):
        return {"status": "unevaluable",
                "reason": "comb has no numeric divider tone contract"}
    x0, x1 = float(cell["x0"]), float(cell["x1"])
    seed_y0, seed_y1 = float(comb["y0"]), float(comb["y1"])
    max_width = pitch / 2
    candidates = [
        paint for paint in page.paints
        if abs(paint.tone - divider_tone) <= 1e-8
        and paint.width <= max_width
        and paint.height > paint.width
        and paint.x1 > x0 and paint.x0 < x1
        and paint.y1 > float(cell["y0"]) and paint.y0 < float(cell["y1"])
    ]

    intersecting_unsupported = [
        region for region in page.unsupported
        if region.x1 > x0 and region.x0 < x1
        and region.y1 > seed_y0 and region.y0 < seed_y1
        and region.y1 - region.y0 > 1e-6
        and min(region.y1, seed_y1) - max(region.y0, seed_y0) > POSITION_TOL_PT
    ]
    if intersecting_unsupported:
        return {
            "status": "unevaluable",
            "reason": "unsupported SVG geometry intersects the comb band",
            "unsupported": [dataclasses.asdict(region)
                            for region in intersecting_unsupported[:8]],
        }

    endpoints = {seed_y0, seed_y1}
    for paint in page.paints:
        if not (paint.x1 > x0 and paint.x0 < x1):
            continue
        a = max(float(cell["y0"]), paint.y0)
        b = min(float(cell["y1"]), paint.y1)
        if b > a and b > seed_y0 and a < seed_y1:
            endpoints.update((a, b))
    ordered_y = sorted(endpoints)
    bands: list[dict[str, Any]] = []
    for a, b in zip(ordered_y, ordered_y[1:]):
        # A thinner y-slab is only coordinate noise at a shared endpoint: it
        # cannot establish a geometrically distinct band under the repository's
        # fixed 0.25pt position tolerance.
        if b - a <= POSITION_TOL_PT or b <= seed_y0 or a >= seed_y1:
            continue
        mid = (a + b) / 2
        groups = merge_centres(candidates, mid, page.paints)
        if not groups:
            continue
        # Match recognised anchors to independently painted source boundaries.
        # A referee must not silently move an anchor to the nearest plausible
        # line: source and lattice positions agree inside the repository's
        # fixed 0.25pt bound or they do not agree.
        available = list(range(len(groups)))
        anchor_matches: list[dict[str, float]] = []
        for anchor in anchors:
            choices = sorted(
                ((abs(groups[index]["x"] - anchor), index)
                 for index in available
                 if abs(groups[index]["x"] - anchor) <= POSITION_TOL_PT),
                key=lambda item: (item[0], groups[item[1]]["x"]),
            )
            if not choices:
                anchor_matches = []
                break
            distance, index = choices[0]
            available.remove(index)
            anchor_matches.append({
                "layout_x": round(anchor, 6),
                "source_x": groups[index]["x"],
                "delta_pt": round(groups[index]["x"] - anchor, 6),
                "group_index": index,
            })
        if len(anchor_matches) != len(anchors):
            continue
        matched_groups = [groups[int(match["group_index"])]
                          for match in anchor_matches]
        if any(group["clipped"] for group in matched_groups):
            bands.append({
                "status": "unevaluable",
                "reason": "a recognised divider is under an unresolved SVG clip",
                "y0": a, "y1": b,
            })
            continue

        # A thick page/frame edge is sometimes a stack of two black bars.  The
        # inner bar can be one pitch beyond the last real comb tick, but the
        # paper between those bars is thinner than the ink drawing them and is
        # therefore not a writable compartment.  Identify the source group
        # crossing each lattice-owned cell edge, then require actual paper
        # wider than the two bars before treating a neighbour as distinct.
        frame_groups = [
            group for group in groups
            if ((group["x0"] <= x0 + POSITION_TOL_PT
                 and group["x1"] >= x0 - POSITION_TOL_PT)
                or (group["x0"] <= x1 + POSITION_TOL_PT
                    and group["x1"] >= x1 - POSITION_TOL_PT))
        ]

        def distinct_from_frames(group: dict[str, Any]) -> bool:
            for frame in frame_groups:
                paper = (max(float(group["x0"]), float(frame["x0"]))
                         - min(float(group["x1"]), float(frame["x1"])))
                weights = ((float(group["x1"]) - float(group["x0"]))
                           + (float(frame["x1"]) - float(frame["x0"])))
                if paper <= weights:
                    return False
            return True

        eligible_groups = [
            group for group in groups
            if distinct_from_frames(group)
        ]

        extras: list[dict[str, Any]] = []
        partial: list[dict[str, Any]] = []
        source_anchors = [float(match["source_x"]) for match in anchor_matches]
        for left, right in zip(source_anchors, source_anchors[1:]):
            gap = right - left
            multiple = int(round(gap / pitch))
            between = sorted(
                (group for group in eligible_groups
                 if left + POSITION_TOL_PT < float(group["x"])
                 < right - POSITION_TOL_PT),
                key=lambda group: float(group["x"]),
            )
            if between:
                source_steps = [
                    b - a for a, b in zip(
                        [left, *(float(group["x"]) for group in between)],
                        [*(float(group["x"]) for group in between), right],
                    )
                ]
                # The source itself can prove a regular subdivision even when
                # lattice.py chose the wrong modal pitch.  This is what happens
                # when a heavy group separator is 0.24pt off the nominal tick
                # position: both neighbouring source compartments still agree
                # with each other inside the fixed position tolerance.
                if (len(between) == max(0, multiple - 1)
                        and max(source_steps) - min(source_steps)
                        <= POSITION_TOL_PT):
                    extras.extend(between)
                    continue
            if multiple <= 1:
                continue
            expected = [
                (left + index * pitch,
                 right - (multiple - index) * pitch)
                for index in range(1, multiple)
            ]
            found: list[dict[str, Any]] = []
            for from_left, from_right in expected:
                hit = next((
                    group for group in eligible_groups
                    if group not in found
                    and (near(group["x"], from_left)
                         or near(group["x"], from_right))
                ), None)
                if hit is not None:
                    found.append(hit)
            if found and len(found) != len(expected):
                partial.append({
                    "left": round(left, 6), "right": round(right, 6),
                    "expected_x": [
                        [round(from_left, 6), round(from_right, 6)]
                        for from_left, from_right in expected
                    ],
                    "found_x": [item["x"] for item in found],
                })
            elif found:
                extras.extend(found)
        if partial:
            bands.append({
                "status": "unevaluable",
                "reason": "only part of an integral-pitch gap is painted",
                "y0": a, "y1": b, "partial": partial,
            })
            continue
        unique_extras = {round(item["x"], 6): item for item in extras}
        if any(item["clipped"] for item in unique_extras.values()):
            bands.append({
                "status": "unevaluable",
                "reason": "a candidate divider is under an unresolved SVG clip",
                "y0": a, "y1": b,
            })
            continue
        source_x = sorted({round(anchor, 6) for anchor in source_anchors}
                          | set(unique_extras))

        # A source run can extend beyond the anchors when lattice.py missed its
        # first or last divider.  Continue only through the nearest same-band
        # interior boundary.  The run's measured gaps (or the fixed lattice
        # pitch when it agrees) must predict that boundary.  One special case
        # is still topological rather than heuristic: a lone first/last
        # boundary can divide the remaining interval to the cell edge into two
        # equal compartments.  That edge-bisection proof is allowed once per
        # side and cannot bootstrap a walk through unrelated ink.
        #
        # Source ink touching the cell edge is its frame, not another divider.
        # Failing to exclude it is catastrophic: every ordinary N-slot comb
        # becomes N+2 merely because its box has two sides.
        interior_groups = [
            group for group in eligible_groups
            if group["x0"] > x0 + POSITION_TOL_PT
            and group["x1"] < x1 - POSITION_TOL_PT
            and float(group["x0"]) - x0
            > float(group["x1"]) - float(group["x0"])
            and x1 - float(group["x1"])
            > float(group["x1"]) - float(group["x0"])
        ]

        def extend(direction: int) -> tuple[bool, str | None]:
            nonlocal source_x
            extensions = 0
            while source_x:
                edge = source_x[0] if direction < 0 else source_x[-1]
                possible = [
                    group for group in interior_groups
                    if not any(near(group["x"], value) for value in source_x)
                    and ((group["x"] < edge - POSITION_TOL_PT)
                         if direction < 0 else
                         (group["x"] > edge + POSITION_TOL_PT))
                ]
                if not possible:
                    return True, None
                candidate = (max(possible, key=lambda item: item["x"])
                             if direction < 0
                             else min(possible, key=lambda item: item["x"]))
                gap = abs(float(candidate["x"]) - edge)
                adjacent = [
                    right - left for left, right in zip(source_x, source_x[1:])
                    if POSITION_TOL_PT < right - left <= 1.5 * pitch
                ]
                pitch_match = any(
                    abs(gap - model) <= POSITION_TOL_PT
                    for model in (pitch, *adjacent)
                )
                paper_edge = x0 if direction < 0 else x1
                edge_bisection = (
                    extensions == 0
                    and abs(abs(float(candidate["x"]) - paper_edge) - gap)
                    <= POSITION_TOL_PT
                )
                if not pitch_match and not edge_bisection:
                    return True, None
                if candidate["clipped"]:
                    return False, "an outward source divider has unresolved clipping"
                value = round(float(candidate["x"]), 6)
                source_x.append(value)
                source_x.sort()
                unique_extras[value] = candidate
                extensions += 1
            return True, None

        left_ok, left_reason = extend(-1)
        right_ok, right_reason = extend(1)
        if not left_ok or not right_ok:
            bands.append({
                "status": "unevaluable",
                "reason": left_reason or right_reason,
                "y0": a, "y1": b,
            })
            continue
        bands.append({
            "status": "measured",
            "y0": round(a, 6), "y1": round(b, 6),
            "source_divider_x": source_x,
            "extra_divider_x": sorted(unique_extras),
            "compartments": len(source_x) + 1,
            "anchor_matches": [
                {key: value for key, value in match.items()
                 if key != "group_index"}
                for match in anchor_matches
            ],
            "positions_match": all(
                abs(float(match["delta_pt"])) <= POSITION_TOL_PT
                for match in anchor_matches
            ),
            "components": [group for group in groups
                           if any(near(group["x"], x) for x in source_x)],
        })

    measured = [band for band in bands if band["status"] == "measured"]
    ambiguous = [band for band in bands if band["status"] != "measured"]
    if ambiguous:
        return {
            "status": "unevaluable",
            "reason": "one or more source slabs have ambiguous topology",
            "bands": bands[:8],
        }
    if not measured:
        reason = (bands[0]["reason"] if bands else
                  "no common Poppler band contains every recognised divider")
        return {"status": "unevaluable", "reason": reason, "bands": bands[:8]}
    topologies = {
        tuple(round(float(value), 6)
              for value in band["source_divider_x"])
        for band in measured
    }
    if len(topologies) != 1:
        return {
            "status": "unevaluable",
            "reason": "source slabs have different divider topology",
            "bands": measured,
        }
    chosen = min(measured,
                 key=lambda band: (band["y1"] - band["y0"],
                                   band["y0"], band["y1"],
                                   band["source_divider_x"]))
    return {
        "status": "measured",
        "reason": "one source topology contains every recognised anchor",
        **{key: value for key, value in chosen.items() if key != "status"},
    }


def audit_evidence(audit_record: dict[str, Any] | None) -> dict[str, Any]:
    if not audit_record:
        return {"complete": False, "reason": "no audit record", "offenders": {}}
    assertion = ((audit_record.get("assertions") or {})
                 .get("comb_slots_match_printed") or {})
    raw_offenders = assertion.get("offenders") or []
    if not isinstance(raw_offenders, list):
        return {"complete": False, "reason": "audit offenders is not a list",
                "offenders": {}}
    valid_items = [
        item for item in raw_offenders
        if isinstance(item, dict) and isinstance(item.get("cell"), str)
    ]
    offenders = {item["cell"]: item for item in valid_items}
    holds = assertion.get("holds")
    count_value = assertion.get("offender_count")
    if count_value is None and holds is True and not raw_offenders:
        count_value = 0
    try:
        count = int(count_value)
        checked = int(assertion["combs_checked"])
        published = int(assertion.get("offenders_published",
                                      len(raw_offenders)))
        omitted = int(assertion.get("offenders_omitted",
                                    count - published))
        layout_mismatches = int(assertion["layout_mismatches"])
        emission_behind = int(assertion["emission_behind_layout"])
    except (KeyError, TypeError, ValueError):
        return {"complete": False,
                "reason": "audit comb summary is incomplete or non-numeric",
                "offenders": offenders}
    complete_flag = assertion.get("offenders_complete",
                                  published == count and omitted == 0)
    errors: list[str] = []
    if len(valid_items) != len(raw_offenders):
        errors.append("malformed offender entries")
    if len(offenders) != len(raw_offenders):
        errors.append("duplicate offender cells")
    if count < 0 or checked < 0 or published < 0 or omitted < 0:
        errors.append("negative audit counts")
    if published != len(raw_offenders):
        errors.append("published count disagrees with offender list")
    if count != published + omitted:
        errors.append("published and omitted counts do not sum to total")
    if omitted != 0 or not bool(complete_flag):
        errors.append("audit offender publication is incomplete")
    if layout_mismatches != count:
        errors.append("layout mismatch count disagrees with offenders")
    if bool(holds) != (count == 0):
        errors.append("audit holds flag disagrees with offender count")
    complete = not errors
    return {
        "complete": complete,
        "reason": "complete" if complete else "; ".join(errors),
        "offender_count": count,
        "offenders_published": published,
        "offenders_omitted": omitted,
        "combs_checked": checked,
        "layout_mismatches": layout_mismatches,
        "emission_behind_layout": emission_behind,
        "offenders": offenders,
        "holds": bool(holds),
    }


def bind_audit_manifest(audit_record: dict[str, Any] | None,
                        expected: dict[
                            str, tuple[pathlib.Path, bool, bytes | None]],
                        audit_producer_bytes: bytes,
                        ) -> tuple[bool, str]:
    if not audit_record:
        return False, "no audit record"
    manifest = audit_record.get("input_manifest")
    if not isinstance(manifest, dict):
        return False, "audit input manifest is missing"
    if (manifest.get("schema") != "formgen-audit-input-manifest-v1"
            or manifest.get("algorithm") != "sha256"):
        return False, "audit input manifest schema/algorithm is unsupported"
    producer = manifest.get("producer")
    if (not isinstance(producer, dict)
            or producer.get("file") != "tools/formgen/audit.py"
            or producer.get("bytes") != len(audit_producer_bytes)
            or producer.get("sha256") != sha256_bytes(audit_producer_bytes)):
        return False, "audit producer hash is stale"
    if manifest.get("complete") is not True or manifest.get("missing_required") != []:
        return False, "audit input manifest is incomplete"
    inputs = manifest.get("inputs")
    if not isinstance(inputs, dict) or set(inputs) != set(expected):
        return False, "audit input manifest roles disagree"
    for role, (path, required, payload) in expected.items():
        entry = inputs.get(role)
        if not isinstance(entry, dict):
            return False, f"audit input entry is missing: {role}"
        present = payload is not None
        if entry.get("file") != path.name or entry.get("required") is not required:
            return False, f"audit input metadata disagrees: {role}"
        if entry.get("present") is not present:
            return False, f"audit input presence is stale: {role}"
        if not present:
            if required or entry.get("bytes") is not None or entry.get("sha256") is not None:
                return False, f"audit input absence is invalid: {role}"
            continue
        assert payload is not None
        if (entry.get("bytes") != len(payload)
                or entry.get("sha256") != sha256_bytes(payload)):
            return False, f"audit input hash is stale: {role}"
    return True, "exact input bytes verified"


def page_signature(value: dict[str, Any]) -> list[tuple[int, float, float]]:
    pages = value.get("pages")
    if not isinstance(pages, list):
        raise RefereeError("artifact pages is not a list")
    signature = [
        (int(page["index"]), float(page["width_pt"]), float(page["height_pt"]))
        for page in pages
    ]
    if [index for index, _, _ in signature] != list(
            range(1, len(signature) + 1)):
        raise RefereeError("artifact pages are not exhaustive and ordered")
    return signature


def bind_artifacts(slug: str, layout: dict[str, Any], ir: dict[str, Any],
                   guide: dict[str, Any], parser: SlotParser) -> None:
    for name, value in (("layout", layout), ("IR", ir), ("guide", guide)):
        if not isinstance(value, dict):
            raise RefereeError(f"{slug}: {name} artifact is not an object")
    for key in ("form", "source", "paper"):
        if ir.get(key) != layout.get(key):
            raise RefereeError(f"{slug}: IR/layout {key} provenance disagrees")
    if guide.get("form") != layout.get("form"):
        raise RefereeError(f"{slug}: guide/layout form provenance disagrees")
    layout_pages = page_signature(layout)
    if page_signature(ir) != layout_pages:
        raise RefereeError(f"{slug}: IR/layout page geometry disagrees")
    source = layout.get("source") or {}
    if int(source.get("page_count", -1)) != len(layout_pages):
        raise RefereeError(f"{slug}: pinned source page count disagrees")
    paper = layout.get("paper") or {}
    if paper.get("uniform") is not True:
        raise RefereeError(f"{slug}: non-uniform paper is unsupported")
    if any(abs(width - float(paper.get("width_pt", -1))) > 1e-8
           or abs(height - float(paper.get("height_pt", -1))) > 1e-8
           for _, width, height in layout_pages):
        raise RefereeError(f"{slug}: layout pages disagree with paper contract")
    form = layout.get("form") or {}
    root = parser.root
    if root is None:
        raise RefereeError(f"{slug}: HTML root metadata is missing")
    expected_root = {
        "data-form": str(form.get("code", "")),
        "data-revision": str(form.get("revision", "")),
        "data-source-sha256": str(source.get("sha256", "")),
        "data-schema-version": str(layout.get("schema_version", "")),
    }
    for key, expected in expected_root.items():
        if not expected or root.get(key) != expected:
            raise RefereeError(
                f"{slug}: HTML {key} disagrees with layout provenance")
    layout_page_indexes = {index for index, _, _ in layout_pages}
    whole_guide_pages: set[int] = set()
    inline = guide.get("inline") or []
    if not isinstance(inline, list):
        raise RefereeError(f"{slug}: guide inline inventory is not a list")
    for region in inline:
        try:
            page = int(region["page"])
            cut = float(region["cut_y_pt"])
            reclaimed = float(region["reclaimed_pct"])
        except (KeyError, TypeError, ValueError):
            raise RefereeError(f"{slug}: guide region provenance is incomplete")
        if page not in layout_page_indexes:
            raise RefereeError(f"{slug}: guide references an unknown page")
        if abs(cut) <= 1e-8 and abs(reclaimed - 100.0) <= 1e-8:
            whole_guide_pages.add(page)
    stats = guide.get("stats") or {}
    if int(stats.get("pages", -1)) != len(layout_pages):
        raise RefereeError(f"{slug}: guide/layout page counts disagree")
    expected_pages = [
        index for index, _, _ in layout_pages if index not in whole_guide_pages
    ]
    if parser.pages != expected_pages:
        raise RefereeError(f"{slug}: HTML/layout page inventory disagrees")
    expected_geometry = [
        (index, width, height)
        for index, width, height in layout_pages
        if index not in whole_guide_pages
    ]
    if parser.page_geometry != expected_geometry:
        raise RefereeError(f"{slug}: HTML/layout page geometry disagrees")


def bind_tracked_provenance(slug: str, layout: dict[str, Any]
                            ) -> tuple[pathlib.Path, bytes]:
    matches = sorted((REPO / "forms").glob(f"**/{slug}/provenance.json"))
    if len(matches) != 1:
        raise RefereeError(
            f"{slug}: expected one tracked provenance record, got {len(matches)}")
    path = matches[0]
    payload = path.read_bytes()
    provenance = json.loads(payload)
    form_sources = [
        source for source in provenance.get("sources") or ()
        if source.get("role") == "form"
    ]
    if len(form_sources) != 1:
        raise RefereeError(
            f"{slug}: tracked provenance has no unique form source")
    pinned = form_sources[0]
    layout_source = layout.get("source") or {}
    form = layout.get("form") or {}
    if (provenance.get("slug") != slug
            or str(provenance.get("revision")) != str(form.get("revision"))
            or pinned.get("sha256") != layout_source.get("sha256")
            or pinned.get("file")
            != str(layout_source.get("file", "")).split(":", 1)[-1]):
        raise RefereeError(
            f"{slug}: layout source disagrees with tracked provenance")
    return path, payload


def comparison(cell: dict[str, Any], audit_complete: bool) -> tuple[str, str]:
    lattice = cell["latticed"]
    emitted = cell["emitted"]
    referee = cell["referee"]
    if emitted != lattice or not cell["emitted_indexes_valid"]:
        return "stale-generation", "emitted physical slots disagree with lattice"
    if not audit_complete or cell["audit_printed"] is None:
        return "unevaluable", "audit evidence is incomplete"
    if referee.get("status") != "measured":
        return "unevaluable", f"referee: {referee.get('reason', 'no reason')}"
    if not bool(referee.get("positions_match")):
        return "stop", "referee positions disagree with lattice anchors"
    source = int(referee["compartments"])
    audit = int(cell["audit_printed"])
    if source == lattice == audit:
        return "agree", "referee, lattice, audit, and emitted agree"
    if source == audit and source != lattice:
        return "repair-lattice", "referee and audit agree against lattice"
    if source == lattice and source != audit:
        return "repair-audit", "referee and lattice agree against audit"
    if lattice == audit and source != lattice:
        return "stop", "lattice and audit agree against the independent referee"
    return "stop", "referee, lattice, and audit all differ"


def cell_sort_key(cell: dict[str, Any]) -> tuple[int, int, str]:
    match = re.fullmatch(r"p(\d+)c(\d+)", str(cell.get("cell", "")))
    if match:
        return int(match.group(1)), int(match.group(2)), str(cell["cell"])
    return int(cell.get("page", 0)), sys.maxsize, str(cell.get("cell", ""))


def changed_snapshot_inputs(form: dict[str, Any],
                            args: argparse.Namespace) -> list[str]:
    slug = form["slug"]
    artifacts = form["artifacts"]
    checks: list[tuple[str, pathlib.Path, str | None]] = [
        ("layout", args.layout_dir / f"{slug}.layout.json",
         artifacts["layout_sha256"]),
        ("ir", args.ir_dir / f"{slug}.ir.json", artifacts["ir_sha256"]),
        ("html", args.html_dir / f"{slug}.html", artifacts["html_sha256"]),
        ("guide", args.guide_dir / f"{slug}.guide.json",
         artifacts["guide_sha256"]),
        ("guide_html", args.html_dir / f"{slug}.guide.html",
         artifacts["guide_html_sha256"]),
        ("tracked_provenance", REPO / artifacts["tracked_provenance_file"],
         artifacts["tracked_provenance_sha256"]),
        ("source", args.source_root / form["source"]["file"],
         form["source"]["sha256"]),
    ]
    changed: list[str] = []
    for role, path, expected in checks:
        if expected is None:
            if path.exists():
                changed.append(role)
            continue
        try:
            actual = sha256_file(path)
        except OSError:
            changed.append(role)
            continue
        if actual != expected:
            changed.append(role)
    return changed


def form_report(layout_path: pathlib.Path, args: argparse.Namespace,
                audit_by_slug: dict[str, dict[str, Any]],
                poppler: dict[str, str]) -> dict[str, Any]:
    slug = layout_path.name.removesuffix(".layout.json")
    html_path = args.html_dir / f"{slug}.html"
    ir_path = args.ir_dir / f"{slug}.ir.json"
    guide_path = args.guide_dir / f"{slug}.guide.json"
    guide_html_path = args.html_dir / f"{slug}.guide.html"
    snapshots: dict[str, bytes | None] = {}
    for role, path, required in (
        ("layout", layout_path, True),
        ("ir", ir_path, True),
        ("html", html_path, True),
        ("guide", guide_path, True),
        ("guide_html", guide_html_path, False),
    ):
        try:
            snapshots[role] = path.read_bytes()
        except FileNotFoundError:
            if required:
                raise RefereeError(
                    f"{slug}: missing artifact: {path.relative_to(REPO)}")
            snapshots[role] = None
    layout_bytes = snapshots["layout"]
    ir_bytes = snapshots["ir"]
    html_bytes = snapshots["html"]
    guide_bytes = snapshots["guide"]
    assert (layout_bytes is not None and ir_bytes is not None
            and html_bytes is not None and guide_bytes is not None)
    layout = json.loads(layout_bytes)
    ir = json.loads(ir_bytes)
    guide = json.loads(guide_bytes)
    layout_comb_count = sum(
        bool(cell.get("comb"))
        for page in layout.get("pages") or ()
        for cell in page.get("cells") or ()
    )
    expected_combs = EXPECTED_COMBS_BY_SLUG.get(slug)
    if expected_combs is None:
        raise RefereeError(f"{slug}: form is not in the pinned referee corpus")
    if layout_comb_count != expected_combs:
        raise RefereeError(
            f"{slug}: layout has {layout_comb_count} combs, "
            f"expected pinned {expected_combs}")
    html_parser = SlotParser()
    html_parser.feed(html_bytes.decode("utf-8"))
    html_parser.close()
    if html_parser.template_depth or html_parser.div_stack:
        raise RefereeError(f"{slug}: HTML has unclosed template/div elements")
    bind_artifacts(slug, layout, ir, guide, html_parser)
    provenance_path, provenance_bytes = bind_tracked_provenance(slug, layout)
    pdf = source_pdf(layout, args.source_root)
    expected_sha = layout["source"]["sha256"]
    pdf_bytes = pdf.read_bytes()
    actual_sha = sha256_bytes(pdf_bytes)
    if actual_sha != expected_sha:
        raise RefereeError(f"{slug}: source hash changed")

    slots = slot_records(html_parser)
    relocated = relocated_cells(guide)
    audit_record = audit_by_slug.get(slug)
    audit = audit_evidence(audit_record)
    manifest_ok, manifest_reason = bind_audit_manifest(audit_record, {
        "ir": (ir_path, True, ir_bytes),
        "layout": (layout_path, True, layout_bytes),
        "html": (html_path, True, html_bytes),
        "guide": (guide_path, True, guide_bytes),
        "guide_html": (
            guide_html_path, False, snapshots["guide_html"]),
    }, args.audit_producer_bytes)
    audit["input_manifest_verified"] = manifest_ok
    audit["input_manifest_reason"] = manifest_reason
    if not manifest_ok:
        audit["complete"] = False
        audit["reason"] = manifest_reason
    cells: list[dict[str, Any]] = []
    page_meta: list[dict[str, Any]] = []

    with tempfile.TemporaryDirectory(prefix=f"comb-referee-{slug}-") as temp:
        directory = pathlib.Path(temp)
        pdf_snapshot = directory / "source.pdf"
        pdf_snapshot.write_bytes(pdf_bytes)
        for page in sorted(layout["pages"], key=lambda item: int(item["index"])):
            page_index = int(page["index"])
            svg_path = render_svg_page(
                poppler["binary_path"], pdf_snapshot, page_index, directory)
            svg = parse_svg(svg_path)
            if (abs(svg.width - float(page["width_pt"])) > POSITION_TOL_PT
                    or abs(svg.height - float(page["height_pt"])) > POSITION_TOL_PT):
                raise RefereeError(
                    f"{slug} page {page_index}: SVG/page dimensions disagree")
            page_meta.append({
                "page": page_index,
                "svg_sha256": svg.sha256,
                "vector_paints": len(svg.paints),
                "unsupported_regions": len(svg.unsupported),
            })
            for cell in page["cells"]:
                if not cell.get("comb") or cell["id"] in relocated:
                    continue
                result = classify_band(cell, svg)
                emitted = slots.get(cell["id"])
                audit_offender = audit["offenders"].get(cell["id"])
                if audit_offender is not None:
                    audit_printed = audit_offender.get("printed")
                    audit_relation = "published-offender"
                elif audit["complete"]:
                    audit_printed = int(cell["comb"]["cells"])
                    audit_relation = "complete-non-offender"
                else:
                    audit_printed = None
                    audit_relation = "unknown-truncated"
                cells.append({
                    "cell": cell["id"],
                    "page": page_index,
                    "bbox": [cell["x0"], cell["y0"], cell["x1"], cell["y1"]],
                    "latticed": int(cell["comb"]["cells"]),
                    "lattice_divider_x": cell["comb"].get("divider_x") or [],
                    "emitted": emitted["count"] if emitted else None,
                    "emitted_indexes_valid": bool(emitted and emitted["valid"]),
                    "audit_printed": audit_printed,
                    "audit_relation": audit_relation,
                    "referee": result,
                })

    cell_ids = {cell["cell"] for cell in cells}
    unexpected_slots = sorted(set(slots) - cell_ids)
    if unexpected_slots:
        raise RefereeError(
            f"{slug}: HTML contains non-layout combs: "
            + ", ".join(unexpected_slots[:8]))
    audit_errors: list[str] = []
    if audit.get("complete"):
        if int(audit["combs_checked"]) != len(cells):
            audit_errors.append(
                f"audit checked {audit['combs_checked']}/{len(cells)} combs")
        unknown = sorted(set(audit["offenders"]) - cell_ids)
        if unknown:
            audit_errors.append(
                f"audit published unknown cells: {', '.join(unknown[:8])}")
    if audit_errors:
        audit["complete"] = False
        audit["reason"] = "; ".join(audit_errors)
    for cell in cells:
        status, reason = comparison(cell, bool(audit.get("complete")))
        cell["comparison_status"] = status
        cell["comparison_reason"] = reason
        cell["four_way"] = {
            "referee": (
                int(cell["referee"]["compartments"])
                if cell["referee"].get("status") == "measured" else None
            ),
            "lattice": cell["latticed"],
            "audit": cell["audit_printed"],
            "emitted": cell["emitted"],
        }

    measured = [cell for cell in cells
                if cell["referee"]["status"] == "measured"]
    unevaluable = [cell for cell in cells
                   if cell["referee"]["status"] != "measured"]
    layout_mismatches = [
        cell for cell in measured
        if int(cell["referee"]["compartments"]) != int(cell["latticed"])
    ]
    position_mismatches = [
        cell for cell in measured
        if not bool(cell["referee"].get("positions_match"))
    ]
    emission_mismatches = [
        cell for cell in cells
        if cell["emitted"] != cell["latticed"]
        or not cell["emitted_indexes_valid"]
    ]
    comparison_counts = {
        name: sum(cell["comparison_status"] == name for cell in cells)
        for name in (
            "agree", "repair-lattice", "repair-audit", "stale-generation",
            "stop", "unevaluable",
        )
    }
    status = "ok"
    reasons: list[str] = []
    if not audit.get("complete"):
        status = "unevaluable"
        reasons.append(f"audit evidence incomplete: {audit.get('reason')}")
    if comparison_counts["unevaluable"]:
        status = "unevaluable"
        reasons.append(f"{comparison_counts['unevaluable']} combs unevaluable")
    if status != "unevaluable" and any(
            comparison_counts[name] for name in (
                "repair-lattice", "repair-audit", "stale-generation", "stop")):
        status = "disagreement"
        reasons.append("one or more four-way comparisons disagree")

    return {
        "slug": slug,
        "status": status,
        "reason": ", ".join(reasons) if reasons else "all combs measured",
        "source": {
            "file": str(pdf.relative_to(args.source_root)),
            "sha256": actual_sha,
        },
        "artifacts": {
            "ir_sha256": sha256_bytes(ir_bytes),
            "layout_sha256": sha256_bytes(layout_bytes),
            "html_sha256": sha256_bytes(html_bytes),
            "guide_sha256": sha256_bytes(guide_bytes),
            "guide_html_sha256": (
                sha256_bytes(snapshots["guide_html"])
                if snapshots["guide_html"] is not None else None
            ),
            "tracked_provenance_file": str(provenance_path.relative_to(REPO)),
            "tracked_provenance_sha256": sha256_bytes(provenance_bytes),
        },
        "poppler": poppler,
        "pages": page_meta,
        "audit_evidence": {
            key: value for key, value in audit.items() if key != "offenders"
        },
        "counts": {
            "combs": len(cells),
            "measured": len(measured),
            "unevaluable": len(unevaluable),
            "referee_layout_mismatches": len(layout_mismatches),
            "referee_layout_position_mismatches": len(position_mismatches),
            "emission_layout_mismatches": len(emission_mismatches),
            "comparisons": comparison_counts,
        },
        "cells": sorted(cells, key=cell_sort_key),
    }


def self_test() -> int:
    assert parse_transform("matrix(1,0,0,-1,0,100)").point(3, 20) == (3, 80)
    translated = parse_transform("translate(10 20) scale(2)")
    assert translated.point(1, 1) == (12, 22)

    subpaths, unsupported = path_subpaths("M 1 2 L 3 2 L 3 9 L 1 9 Z")
    assert unsupported is None and len(subpaths) == 1 and subpaths[0][1]
    _subpaths, unsupported = path_subpaths("M 0 0 C 1 2 3 4 5 6")
    assert unsupported is not None
    assert is_axis_aligned_rectangle([(1, 2), (3, 2), (3, 9), (1, 9)])
    assert not is_axis_aligned_rectangle([(1, 2), (3, 2), (2, 9)])

    with tempfile.TemporaryDirectory(prefix="comb-referee-self-test-") as temp:
        svg_path = pathlib.Path(temp) / "synthetic.svg"
        svg_path.write_text(
            '<svg xmlns="http://www.w3.org/2000/svg" '
            'viewBox="0 0 100 100">'
            '<path id="triangle" d="M 10 10 L 20 10 L 15 30 Z" fill="#000"/>'
            '<g clip-path="url(#clip)"><rect id="clipped" x="30" y="10" '
            'width="1" height="20" fill="#000"/></g>'
            '<line id="diagonal" x1="40" y1="10" x2="50" y2="30" '
            'stroke="#000" stroke-width="1"/>'
            '<line id="near-diagonal" x1="60" y1="10" x2="60.2" y2="30" '
            'stroke="#000" stroke-width="1"/>'
            '<rect id="translucent" x="70" y="10" width="1" height="20" '
            'fill="#000" opacity="0.5"/>'
            '</svg>',
            encoding="utf-8",
        )
        parsed_svg = parse_svg(svg_path)
        assert any(region.reason == "non-rectangular closed SVG fill"
                   for region in parsed_svg.unsupported)
        assert any(region.reason == "diagonal SVG line"
                   for region in parsed_svg.unsupported)
        assert any(region.element == "near-diagonal"
                   for region in parsed_svg.unsupported)
        assert any(paint.element == "clipped" and paint.clipped
                   for paint in parsed_svg.paints)
        assert any(paint.element == "translucent" and paint.clipped
                   for paint in parsed_svg.paints)

    def paint(x: float, a: float = 2, b: float = 8, order: int = 0,
              tone: float = 0.0) -> Paint:
        return Paint(x - 0.1, a, x + 0.1, b, tone, order,
                     "test", f"x{x}-o{order}")

    cell = {
        "id": "p1c0", "x0": 0.0, "y0": 0.0, "x1": 40.0, "y1": 10.0,
        "comb": {"cells": 3, "divider_x": [10.0, 30.0],
                 "pitch_pt": 10.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
    }
    page = SvgPage(100, 100, [paint(10), paint(20), paint(30)], [], "x")
    result = classify_band(cell, page)
    assert result["status"] == "measured", result
    assert result["compartments"] == 4, result
    assert result["extra_divider_x"] == [20.0], result

    # A stray vertical in a normal one-pitch gap is not a compartment.
    normal = {
        **cell,
        "comb": {"cells": 3, "divider_x": [10.0, 20.0],
                 "pitch_pt": 10.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
    }
    result = classify_band(normal, SvgPage(
        100, 100, [paint(10), paint(15), paint(20)], [], "x"))
    assert result["compartments"] == 3 and not result["extra_divider_x"], result

    # Source steps may establish their own regular subdivision while each
    # endpoint remains inside the fixed position bound.
    irregular = {
        **cell,
        "comb": {**cell["comb"], "divider_x": [10.0, 30.2]},
    }
    result = classify_band(irregular, SvgPage(
        100, 100, [paint(10), paint(20.1), paint(30.2)], [], "x"))
    assert result["status"] == "measured" and result["compartments"] == 4, result
    assert result["extra_divider_x"] == [20.1], result

    fragmented = classify_band(cell, SvgPage(
        100, 100, [
            paint(10), paint(30),
            paint(20, a=2, b=5), paint(20, a=5, b=8),
        ], [], "x"))
    assert fragmented["status"] == "measured", fragmented
    assert fragmented["compartments"] == 4, fragmented

    short_extra = classify_band(cell, SvgPage(
        100, 100, [
            paint(10), paint(30), paint(20, a=2, b=5),
        ], [], "x"))
    assert short_extra["status"] == "unevaluable", short_extra

    # The cell's own sides are not two more compartments.
    framed = classify_band(normal, SvgPage(
        100, 100, [paint(0), paint(10), paint(20), paint(40)], [], "x"))
    assert framed["compartments"] == 3 and not framed["extra_divider_x"], framed

    # A source-backed run may extend beyond the recognised anchors.
    outward = {
        **cell,
        "comb": {"cells": 3, "divider_x": [20.0, 30.0],
                 "pitch_pt": 10.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
    }
    result = classify_band(outward, SvgPage(
        100, 100, [paint(0), paint(10), paint(20), paint(30), paint(40)], [], "x"))
    assert result["compartments"] == 4, result
    assert result["extra_divider_x"] == [10.0], result

    # A single truncated edge slot is proven when its divider bisects the
    # remaining source interval, even when the other gaps are irregular.
    edge_split = {
        **cell,
        "comb": {"cells": 3, "divider_x": [20.0, 31.0],
                 "pitch_pt": 9.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
    }
    result = classify_band(edge_split, SvgPage(
        100, 100, [paint(0), paint(10), paint(20), paint(31), paint(40)], [], "x"))
    assert result["compartments"] == 4, result
    assert result["extra_divider_x"] == [10.0], result

    # An unrelated vertical in a broad label interval cannot start a run.
    broad = {
        **cell,
        "comb": {"cells": 2, "divider_x": [30.0],
                 "pitch_pt": 10.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
        "x1": 100.0,
    }
    result = classify_band(broad, SvgPage(
        100, 100, [paint(0), paint(30), paint(55), paint(100)], [], "x"))
    assert result["compartments"] == 2 and not result["extra_divider_x"], result

    # A pitch-aligned grey decoration is not a black compartment boundary.
    grey = classify_band(outward, SvgPage(
        100, 100, [paint(0), paint(10, tone=0.5),
                   paint(20), paint(30), paint(40)], [], "x"))
    assert grey["compartments"] == 3 and not grey["extra_divider_x"], grey

    # Two thick bars with too little paper between them are one frame edge.
    composite = {
        **cell,
        "x1": 42.0,
        "comb": {"cells": 4, "divider_x": [10.0, 20.0, 30.0],
                 "pitch_pt": 10.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
    }
    result = classify_band(composite, SvgPage(
        100, 100, [
            paint(0), paint(10), paint(20), paint(30),
            Paint(39.0, 2, 41.0, 8, 0.0, 0, "test", "inner-frame"),
            Paint(40.5, 2, 43.5, 8, 0.0, 0, "test", "outer-frame"),
        ], [], "x"))
    assert result["compartments"] == 4 and not result["extra_divider_x"], result

    shifted = classify_band(normal, SvgPage(
        100, 100, [paint(7), paint(17)], [], "x"))
    assert shifted["status"] == "unevaluable", shifted

    # Missing layout anchors leave a smaller source topology ambiguous.
    stale = {
        **cell,
        "comb": {"cells": 3, "divider_x": [20.0, 23.0],
                 "pitch_pt": 3.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
    }
    result = classify_band(stale, SvgPage(
        100, 100, [paint(0), paint(20), paint(40)], [], "x"))
    assert result["status"] == "unevaluable", result

    # A short midpoint that does not prove the lattice anchor is unevaluable.
    short_midpoint = {
        **cell,
        "comb": {"cells": 2, "divider_x": [23.0],
                 "pitch_pt": 10.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
    }
    result = classify_band(short_midpoint, SvgPage(
        100, 100, [paint(0), paint(20, b=6), paint(40)], [], "x"))
    assert result["status"] == "unevaluable", result

    # A partially painted three-pitch gap is ambiguous, never rounded down.
    partial = {
        **cell,
        "comb": {"cells": 3, "divider_x": [10.0, 40.0],
                 "pitch_pt": 10.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
        "x1": 50.0,
    }
    result = classify_band(partial, SvgPage(
        100, 100, [paint(10), paint(20), paint(40)], [], "x"))
    assert result["status"] == "unevaluable", result

    # One complete slab cannot overrule a partial subdivision in another.
    mixed_partial = classify_band(partial, SvgPage(
        100, 100, [
            paint(10), paint(20), paint(40),
            paint(30, a=2, b=5),
        ], [], "x"))
    assert mixed_partial["status"] == "unevaluable", mixed_partial

    wide_unsupported = UnsupportedRegion(
        5, 2, 35, 8, "unsupported wide overlay", "overlay")
    result = classify_band(cell, SvgPage(
        100, 100, [paint(10), paint(20), paint(30)],
        [wide_unsupported], "x"))
    assert result["status"] == "unevaluable", result

    # A later white rectangle erases a divider.
    erased = SvgPage(100, 100, [
        paint(10, order=0), paint(20, order=1), paint(30, order=2),
        Paint(19, 0, 21, 10, 1.0, 3, "fill", "white"),
    ], [], "x")
    result = classify_band(cell, erased)
    assert result["status"] == "measured" and result["compartments"] == 3, result

    # A clipped knockout cannot prove that the underlying divider disappeared.
    clipped_erasure = SvgPage(100, 100, [
        paint(10, order=0), paint(20, order=1), paint(30, order=2),
        Paint(19, 0, 21, 10, 1.0, 3, "fill", "clipped-white", True),
    ], [], "x")
    result = classify_band(cell, clipped_erasure)
    assert result["status"] == "unevaluable", result

    # Any later opaque paint owns the final pixel; a grey overpaint hides black.
    grey_overpaint = SvgPage(100, 100, [
        paint(10, order=0), paint(20, order=1), paint(30, order=2),
        Paint(19, 0, 21, 10, 0.5, 3, "fill", "grey"),
    ], [], "x")
    result = classify_band(cell, grey_overpaint)
    assert result["status"] == "measured" and result["compartments"] == 3, result

    # A later broad black fill owns the ink and erases a distinct thin edge.
    black_overpaint = SvgPage(100, 100, [
        paint(10, order=0), paint(20, order=1), paint(30, order=2),
        Paint(15, 0, 25, 10, 0.0, 3, "fill", "broad-black"),
    ], [], "x")
    result = classify_band(cell, black_overpaint)
    assert result["status"] == "measured" and result["compartments"] == 3, result

    no_anchor = {**cell, "comb": {**cell["comb"], "cells": 1, "divider_x": []}}
    assert classify_band(no_anchor, page)["status"] == "unevaluable"

    parser = SlotParser()
    parser.feed(
        '<html data-form="X"><div class="page" id="page-1" '
        'style="width:100pt;height:100pt">'
        '<div id="p1c1" data-field-kind="comb">'
        '<div class="s" data-slot="0">'
        '<input id="p1c1-s0" data-slot-index="0"></div>'
        '<div class="s" data-slot="1"></div></div></div>'
        '<template><div class="s" data-slot="2">'
        '<input id="p1c1-s2" data-slot-index="2"></div></template></html>'
    )
    assert parser.physical_slots == {"p1c1": [0, 1]}
    assert parser.editable_slots == {"p1c1": [0]}
    assert parser.comb_containers == {"p1c1"}
    assert parser.root == {"data-form": "X"}
    assert parser.pages == [1]
    assert parser.page_geometry == [(1, 100.0, 100.0)]

    audit_pass = audit_evidence({
        "assertions": {"comb_slots_match_printed": {
            "holds": True, "offenders": [], "combs_checked": 1,
            "layout_mismatches": 0, "emission_behind_layout": 0,
        }}
    })
    assert audit_pass["complete"] and audit_pass["offender_count"] == 0
    audit_truncated = audit_evidence({
        "assertions": {"comb_slots_match_printed": {
            "holds": False, "offender_count": 2,
            "offenders_published": 1, "offenders_omitted": 1,
            "offenders_complete": False,
            "offenders": [{"cell": "p1c1"}], "combs_checked": 2,
            "layout_mismatches": 2, "emission_behind_layout": 0,
        }}
    })
    assert not audit_truncated["complete"]

    with tempfile.TemporaryDirectory(prefix="comb-referee-audit-bind-") as temp:
        root = pathlib.Path(temp)
        required_path = root / "one.json"
        required_path.write_bytes(b"one")
        optional_path = root / "optional.html"
        expected = {
            "ir": (required_path, True, b"one"),
            "guide_html": (optional_path, False, None),
        }
        audit_producer_bytes = (HERE / "audit.py").read_bytes()
        audit_record = {"input_manifest": {
            "schema": "formgen-audit-input-manifest-v1",
            "algorithm": "sha256",
            "producer": {
                "file": "tools/formgen/audit.py",
                "bytes": len(audit_producer_bytes),
                "sha256": sha256_bytes(audit_producer_bytes),
            },
            "complete": True,
            "missing_required": [],
            "inputs": {
                "ir": {
                    "file": "one.json", "required": True, "present": True,
                    "bytes": 3, "sha256": hashlib.sha256(b"one").hexdigest(),
                },
                "guide_html": {
                    "file": "optional.html", "required": False,
                    "present": False, "bytes": None, "sha256": None,
                },
            },
        }}
        assert bind_audit_manifest(
            audit_record, expected, audit_producer_bytes)[0]
        required_path.write_bytes(b"changed")
        assert bind_audit_manifest(
            audit_record, expected, audit_producer_bytes)[0]
        stale_expected = {**expected, "ir": (required_path, True, b"changed")}
        assert not bind_audit_manifest(
            audit_record, stale_expected, audit_producer_bytes)[0]

    compared = {
        "latticed": 3, "emitted": 3, "emitted_indexes_valid": True,
        "audit_printed": 4,
        "referee": {"status": "measured", "compartments": 4,
                    "positions_match": True},
    }
    assert comparison(compared, True)[0] == "repair-lattice"
    compared["referee"]["compartments"] = 3
    assert comparison(compared, True)[0] == "repair-audit"
    compared["audit_printed"] = 3
    compared["referee"]["compartments"] = 5
    assert comparison(compared, True)[0] == "stop"

    artifact = {
        "schema_version": 1,
        "form": {"code": "X", "revision": "1"},
        "source": {"file": "external:x.pdf", "sha256": "abc",
                   "page_count": 1},
        "paper": {"uniform": True, "width_pt": 100.0, "height_pt": 100.0},
        "pages": [{"index": 1, "width_pt": 100.0, "height_pt": 100.0}],
    }
    ir = {**artifact, "schema_version": 2}
    guide = {"form": artifact["form"], "inline": [], "stats": {"pages": 1}}
    parser.root = {
        "data-form": "X", "data-revision": "1",
        "data-source-sha256": "abc", "data-schema-version": "1",
    }
    bind_artifacts("x-1", artifact, ir, guide, parser)
    bad_ir = {**ir, "source": {**ir["source"], "sha256": "changed"}}
    try:
        bind_artifacts("x-1", artifact, bad_ir, guide, parser)
    except RefereeError:
        pass
    else:
        raise AssertionError("mismatched IR provenance was accepted")

    first = canonical_digest({"b": 2, "a": [1, 2]})
    second = canonical_digest({"a": [1, 2], "b": 2})
    assert first == second
    print("comb_referee self-test: 37 cases pass")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=pathlib.Path,
                        default=pathlib.Path.home() / "Downloads/forms")
    parser.add_argument("--layout-dir", type=pathlib.Path,
                        default=REPO / "build/layout")
    parser.add_argument("--ir-dir", type=pathlib.Path, default=REPO / "build/ir")
    parser.add_argument("--html-dir", type=pathlib.Path, default=REPO / "build/html")
    parser.add_argument("--guide-dir", type=pathlib.Path,
                        default=REPO / "build/guides")
    parser.add_argument("--audit", type=pathlib.Path,
                        default=REPO / "build/audit.json")
    parser.add_argument("--out", type=pathlib.Path,
                        default=REPO / "build/comb-referee.json")
    parser.add_argument("--only", action="append", default=None,
                        help="Restrict to a code or slug (repeatable).")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(list(argv) if argv is not None else None)

    if args.self_test:
        return self_test()
    try:
        producer_bytes = pathlib.Path(__file__).resolve().read_bytes()
        audit_producer_bytes = (HERE / "audit.py").read_bytes()
        audit_bytes = args.audit.read_bytes()
        args.audit_producer_bytes = audit_producer_bytes
        poppler = poppler_identity()
        audit_data = json.loads(audit_bytes)
        if not isinstance(audit_data, list):
            raise RefereeError("audit report is not a list")
        audit_by_slug = {record["slug"]: record for record in audit_data}
        if len(audit_by_slug) != len(audit_data):
            raise RefereeError("audit report contains duplicate form slugs")
        wanted = {value.lower() for value in args.only or ()}
        layouts = sorted(args.layout_dir.glob("*.layout.json"))
        if wanted:
            layouts = [
                path for path in layouts
                if (path.name.removesuffix(".layout.json").lower() in wanted
                    or path.name.split("-", 1)[0].lower() in wanted)
            ]
        if not layouts:
            raise RefereeError("no matching layout files")
        selected_slugs = {
            path.name.removesuffix(".layout.json") for path in layouts
        }
        if not wanted and selected_slugs != set(EXPECTED_COMBS_BY_SLUG):
            missing = sorted(set(EXPECTED_COMBS_BY_SLUG) - selected_slugs)
            extra = sorted(selected_slugs - set(EXPECTED_COMBS_BY_SLUG))
            raise RefereeError(
                "layout corpus identity disagrees"
                + (f"; missing: {', '.join(missing)}" if missing else "")
                + (f"; extra: {', '.join(extra)}" if extra else ""))
        unexpected_selected = sorted(
            selected_slugs - set(EXPECTED_COMBS_BY_SLUG))
        if unexpected_selected:
            raise RefereeError(
                "selected layouts are outside the pinned corpus: "
                + ", ".join(unexpected_selected))
        missing_audit = sorted(selected_slugs - set(audit_by_slug))
        if missing_audit:
            raise RefereeError(
                f"audit report is missing forms: {', '.join(missing_audit)}")
        if not wanted:
            extra_audit = sorted(set(audit_by_slug) - selected_slugs)
            if extra_audit:
                raise RefereeError(
                    f"audit report has unexpected forms: {', '.join(extra_audit)}")

        forms: list[dict[str, Any]] = []
        errors: list[dict[str, str]] = []
        for layout_path in layouts:
            try:
                forms.append(form_report(layout_path, args, audit_by_slug, poppler))
            except Exception as error:  # publish every failed form, fail closed
                errors.append({
                    "slug": layout_path.name.removesuffix(".layout.json"),
                    "error": f"{type(error).__name__}: {error}",
                })

        if args.audit.read_bytes() != audit_bytes:
            errors.append({
                "slug": "<corpus>",
                "error": "RefereeError: audit report changed during referee run",
            })
        if (pathlib.Path(__file__).resolve().read_bytes() != producer_bytes
                or (HERE / "audit.py").read_bytes() != audit_producer_bytes):
            errors.append({
                "slug": "<corpus>",
                "error": "RefereeError: producer code changed during referee run",
            })
        try:
            poppler_changed = (
                sha256_file(pathlib.Path(poppler["binary_path"]))
                != poppler["binary_sha256"]
            )
        except OSError:
            poppler_changed = True
        if poppler_changed:
            errors.append({
                "slug": "<corpus>",
                "error": "RefereeError: Poppler binary changed during referee run",
            })
        for form in forms:
            changed = changed_snapshot_inputs(form, args)
            if changed:
                errors.append({
                    "slug": form["slug"],
                    "error": (
                        "RefereeError: inputs changed during referee run: "
                        + ", ".join(changed)
                    ),
                })

        combs = sum(form["counts"]["combs"] for form in forms)
        measured = sum(form["counts"]["measured"] for form in forms)
        unevaluable = sum(form["counts"]["unevaluable"] for form in forms)
        mismatches = sum(form["counts"]["referee_layout_mismatches"]
                         for form in forms)
        position_mismatches = sum(
            form["counts"]["referee_layout_position_mismatches"]
            for form in forms
        )
        comparison_names = (
            "agree", "repair-lattice", "repair-audit", "stale-generation",
            "stop", "unevaluable",
        )
        comparison_totals = {
            name: sum(form["counts"]["comparisons"][name] for form in forms)
            for name in comparison_names
        }
        complete_corpus = not args.only
        expected_comb_total = sum(
            EXPECTED_COMBS_BY_SLUG[slug] for slug in selected_slugs)
        coverage_ok = (not errors and (not complete_corpus
                       or (len(forms) == EXPECTED_FORMS and combs == EXPECTED_COMBS)))
        if (not coverage_ok
                or any(form["status"] == "unevaluable" for form in forms)):
            corpus_status = "unevaluable"
        elif any(form["status"] == "disagreement" for form in forms):
            corpus_status = "disagreement"
        else:
            corpus_status = "ok"
        report: dict[str, Any] = {
            "schema_version": REPORT_VERSION,
            "producer": "tools/formgen/comb_referee.py",
            "producer_sha256": sha256_bytes(producer_bytes),
            "python_version": sys.version.split()[0],
            "status": corpus_status,
            "poppler": poppler,
            "inputs": {
                "audit_sha256": sha256_bytes(audit_bytes),
                "layout_count": len(layouts),
            },
            "totals": {
                "forms_expected": len(layouts) if args.only else EXPECTED_FORMS,
                "forms_measured": len(forms),
                "forms_error": len(errors),
                "combs_expected": expected_comb_total,
                "combs_found": combs,
                "combs_measured": measured,
                "combs_unevaluable": unevaluable,
                "referee_layout_mismatches": mismatches,
                "referee_layout_position_mismatches": position_mismatches,
                "comparisons": comparison_totals,
                "forms_ok": sum(form["status"] == "ok" for form in forms),
                "forms_disagreement": sum(
                    form["status"] == "disagreement" for form in forms),
                "forms_unevaluable": sum(
                    form["status"] == "unevaluable" for form in forms),
                "audit_evidence_complete_forms": sum(
                    bool(form["audit_evidence"]["complete"]) for form in forms),
            },
            "errors": errors,
            "forms": sorted(forms, key=lambda item: item["slug"]),
        }
        report["payload_sha256"] = canonical_digest(report)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(report, indent=2, sort_keys=True)
                            + "\n", encoding="utf-8")
        print(json.dumps({
            "status": report["status"],
            **report["totals"],
            "out": str(args.out),
            "payload_sha256": report["payload_sha256"],
        }, sort_keys=True))
        if report["status"] == "ok":
            return 0
        return 2 if report["status"] == "unevaluable" else 1
    except (OSError, ValueError, KeyError, RefereeError, json.JSONDecodeError) as error:
        print(f"comb_referee: UNEVALUABLE: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
