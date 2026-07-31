#!/usr/bin/env python3
"""Emit a self-contained HTML form from the IR + box model + font plan.

This is the generator half of the pipeline. Everything it writes is derived
from numbers the PDF itself carries, so nothing here is traced, eyeballed or
tuned: `extract.py` says where the ink is, `lattice.py` says what the ink
*means*, `fonts.py` says which face reproduces the advances, and this module
turns those three into markup. `verify.py` then prints the markup back to PDF,
re-extracts it with the same extractor and diffs IR against IR. No raster is
produced or consulted at any point.

Two rule backends are implemented, because the choice is not obvious and the
project's architecture depends on measuring it rather than assuming it:

  --rule-backend svg  one inline <svg> per page, every rule a <rect> in page
                      units. Measured: SVG rects round-trip through Chromium
                      print-to-PDF at zero delta on every edge.
  --rule-backend css  every rule an absolutely positioned div painted with
                      background-color (never `border`, whose shorthand is
                      snapped and collapsed separately). Measured: Chromium
                      snaps CSS box geometry to the 0.75pt device grid when
                      printing, so 0.24 / 0.48 / 1.44pt all collapse onto
                      {0.75, 1.5} and positions drift up to ~0.27pt.

Apart from the rule layer the two documents are byte-for-byte the same idea:
same page boxes, same text layer, same field layer, same growable template.
That is deliberate -- it is what makes the round-trip a controlled comparison
of the rule layer alone.

One sheet, two documents
------------------------

Given a `guides.py` plan this module emits either half of the sheet:

  --document form   everything the plan does *not* claim. Page boxes, page count
                    and @page are unchanged, so a page whose lower 70% became
                    empty keeps its full height. That is the point: the form's
                    geometry has to stay bit-identical, and the freed space is
                    what a growable band expands into.
  --document guide  only what the plan claims, plus any standalone guide PDF.

A straddler is claimed by nobody and therefore stays on the form. Losing a rule
off the form is a geometry regression; a duplicated rule on the guide is
cosmetic. With no `--guide-plan` the form document is byte-identical to what
this module emitted before the split existed, which `--self-test` asserts.

The guide does not need parity, and one of its pages is actively wrong without
reflowing: 1603Q's guideline block is two columns of 6pt prose, and placing
those as positioned runs is what makes them overlap. `--guide-layout reflow`
(the default for the guide) groups the runs into reading order and emits
flowing HTML, which fixes the overlap by construction rather than by nudging
coordinates. `--guide-layout absolute` keeps the positioned form.

A guide is a document people print, so the reflowed guide carries an `@page` of
its own: the form's paper, with a reading margin instead of the form's
`margin:0`. A standalone guide PDF is reflowed into the same document from its
own extraction (`--guide-source`) rather than embedded with `<object>`, because
an embedded PDF is a second document with its own pagination and does not print
with the one around it. The pinned PDF stays beside the HTML and is linked.

Usage:
    python3 tools/formgen/emit.py \
        --ir build/ir/2551q-2018.ir.json \
        --layout build/layout/2551q-2018.layout.json \
        --font-plan build/fonts/2551q-2018.fontplan.json \
        --rule-backend svg \
        --out build/html/2551q-2018.svg.html

    python3 tools/formgen/emit.py --ir ... --layout ... --font-plan ... \
        --guide-plan build/guides/2551q-2018.guide.json \
        --document guide --out build/html/2551q-2018.guide.html
"""

from __future__ import annotations

import argparse
import filecmp
import json
import math
import operator
import pathlib
import re
import sys
import tempfile
import urllib.parse
from typing import Any, Iterable, Sequence

SCHEMA_VERSION = 1

_ROOT = pathlib.Path(__file__).resolve().parents[2]

# Arial Narrow is not a separately drawn design: it is Arial rendered through a
# constant horizontal scale. Measured across 70 glyphs the scale is 0.820047
# with a maximum deviation of 0.000691 (0.0039pt at the 9.48pt this form uses),
# which is below the extractor's own 2dp quantisation. Rendering those runs as
# Arimo under scaleX() is therefore the same operation the PDF performs with
# its Tz operator, and it is exact in a way that reaching for a *different*
# condensed design is not: Roboto Condensed is off by up to 1.358pt (0.14em) on
# the same runs. If fonts.py starts emitting `horizontal_scale` this constant
# is not consulted -- the plan wins.
ARIAL_NARROW_HORIZONTAL_SCALE = 0.820047

# Mirrors fonts.py: tracking below both of these is float noise in the source
# and emitting it would be a claim the measurement does not support.
LETTER_SPACING_EPSILON_PT = 0.01
LETTER_SPACING_ACCUMULATED_PT = 0.05

# Row separators sit centred on a growable's row_y; a rule is treated as a
# boundary when its centre is within this of one. Half the thickest observed
# rule (1.44pt) is the loosest this may ever be without a 1.44pt row-local rule
# hugging a boundary being misread as the boundary itself.
BAND_EPSILON_PT = 0.6

# z-order. The rule layer is at the bottom by construction; within it, every
# painted rect is emitted in the source's own content-stream order. Painting a
# decorative rule black is a documented past failure of this project, so the
# grey travels with the rect and is never normalised.
Z_RULES = 1
Z_TEXT = 5
Z_CELLS = 6

# One CSS pixel in points. Chromium's print pipeline treats this as the device
# grid: CSS box geometry and text baselines are floored onto it, which is the
# single fact that both rule backends and the text layer have to survive.
DEVICE_PX_PT = 0.75


# ---------------------------------------------------------------------------
# Formatting
# ---------------------------------------------------------------------------


def fmt(value: float) -> str:
    """Format one pt scalar for CSS/SVG: fixed precision, no locale, no '-0'.

    Determinism is the property that makes 'convert the other 34 forms' a
    matter of running the script, so no value reaches the output through
    `repr()`, whose shortest-round-trip form differs between float paths that
    are numerically identical.
    """
    text = f"{float(value):.4f}".rstrip("0").rstrip(".")
    return "0" if text in ("", "-", "-0") else text


def parse_pt(value: str | float | None, default: float = 0.0) -> float:
    """Read a pt scalar back out of the font plan's CSS strings."""
    if value is None:
        return default
    if isinstance(value, (int, float)):
        return float(value)
    match = re.fullmatch(r"\s*(-?\d*\.?\d+)\s*pt\s*", str(value))
    return float(match.group(1)) if match else default


def esc_text(text: str) -> str:
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def esc_attr(text: str) -> str:
    return esc_text(text).replace('"', "&quot;")


def paint_color(rgb: Sequence[float] | None, gray: float | None) -> str:
    """Exact source colour as #rrggbb.

    The IR keeps the literal fill value, which is the only thing that
    distinguishes a black rule from grey decoration; both are reproduced, and
    neither is rounded toward the other.
    """
    if rgb:
        channels = [float(c) for c in rgb[:3]]
    elif gray is not None:
        channels = [float(gray)] * 3
    else:
        channels = [0.0, 0.0, 0.0]
    return "#" + "".join(f"{max(0, min(255, round(c * 255))):02x}" for c in channels)


def text_color(value: Any) -> str:
    """PyMuPDF span colour (packed sRGB int) as #rrggbb."""
    try:
        return f"#{int(value) & 0xFFFFFF:06x}"
    except (TypeError, ValueError):
        return "#000000"


def run_id(page_index: int, run_index: int) -> str:
    """The id lattice.py already uses to point at a text run."""
    return f"p{page_index}t{run_index}"


def style_attr(pairs: Iterable[tuple[str, str | None]]) -> str:
    """Join declarations into a style attribute value.

    The result is escaped by the caller through `esc_attr`, which matters more
    than it looks: a CSS font-family stack and font-variation-settings both
    contain double quotes, and emitting them raw would close the attribute
    mid-value and silently drop every declaration after the family name.
    """
    return ";".join(f"{name}:{value}" for name, value in pairs if value is not None)


# ---------------------------------------------------------------------------
# Rects
# ---------------------------------------------------------------------------


def paint_key(box: dict[str, Any], tie: str) -> tuple[int, int, float, float, str]:
    """Sort key that reproduces the source's paint order, total and stable.

    `paint_seq` is the index of the op that first painted the box and
    `paint_seq_max` the last op a merged bar absorbed, so a bar the generator
    drew as fifteen short rects sorts where it started and still breaks ties
    against a bar that started at the same op. Position and id follow, because
    two boxes from one op must not depend on dict order to decide which is
    emitted first -- determinism is the property this pipeline protects above
    any individual form's score.
    """
    return (int(box["paint_seq"]), int(box["paint_seq_max"]),
            float(box["y0"]), float(box["x0"]), tie)


class Rect:
    """One painted rectangle in page points, with the id of its source rule."""

    __slots__ = ("x", "y", "w", "h", "fill", "source_id", "role")

    def __init__(self, x: float, y: float, w: float, h: float, fill: str,
                 source_id: str | None = None, role: str = "structural") -> None:
        self.x, self.y, self.w, self.h = x, y, w, h
        self.fill = fill
        self.source_id = source_id
        self.role = role

    @classmethod
    def from_box(cls, box: dict[str, Any], source_id: str | None = None) -> "Rect":
        return cls(box["x0"], box["y0"], box["x1"] - box["x0"], box["y1"] - box["y0"],
                   paint_color(box.get("rgb"), box.get("gray")), source_id,
                   str(box.get("role", "structural")))

    def shifted(self, dy: float) -> "Rect":
        return Rect(self.x, self.y + dy, self.w, self.h, self.fill, self.source_id,
                    self.role)

    def to_json(self) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "x": round(self.x, 4), "y": round(self.y, 4),
            "w": round(self.w, 4), "h": round(self.h, 4), "fill": self.fill,
        }
        if self.source_id:
            payload["id"] = self.source_id
        return payload


# A placement matrix is applied only when it reproduces the box the extractor
# also recorded. The two are independent views of the same placement -- the
# matrix comes from get_image_info(), the box from get_image_rects() -- so a
# disagreement means one of them is wrong about this instance, and the box is
# the one the round-trip differ compares. Half the extractor's own 2dp
# quantisation is the widest disagreement that is pure rounding.
PLACEMENT_MATRIX_EPSILON_PT = 0.005


def matrix_box(matrix: Sequence[float]) -> tuple[float, float, float, float]:
    """The axis-aligned box the unit square lands in under `matrix`."""
    a, b, c, d, e, f = (float(v) for v in matrix)
    xs = (e, a + e, c + e, a + c + e)
    ys = (f, b + f, d + f, b + d + f)
    return min(xs), min(ys), max(xs), max(ys)


def placement_matrix(image: dict[str, Any]) -> list[float] | None:
    """The image's 6-element PDF matrix, or None to fall back to its box.

    A box cannot express a flip, and four forms place artwork with a negative
    `d`: 1600-PT's masthead and 2550M's, 2551M's and 2553's seal all rendered
    upside-down with their rim lettering reading bottom-to-top, because the IR's
    x0..y1 is all the emitter used. 0605 places its seal with a small non-zero
    `b`, which a box cannot express either.

    The matrix maps the image's own unit square -- (0,0) at its top-left, y
    downwards -- into page points, which is the same convention an SVG <image>
    at (0,0) sized 1x1 uses, so carrying it needs no conversion and no
    special-casing of the flip. `transform` is None wherever get_image_info()
    and get_image_rects() could not be reconciled; that is extract.py's honest
    answer and this returns None in turn rather than inventing an identity.
    """
    matrix = image.get("transform")
    if not matrix or len(matrix) != 6:
        return None
    x0, y0, x1, y1 = matrix_box(matrix)
    if max(abs(x0 - float(image["x0"])), abs(y0 - float(image["y0"])),
           abs(x1 - float(image["x1"])), abs(y1 - float(image["y1"]))
           ) > PLACEMENT_MATRIX_EPSILON_PT:
        return None
    return [float(v) for v in matrix]


def path_data(path: dict[str, Any]) -> str:
    """One non-rectilinear path as SVG path data, in page points.

    The IR's ops are the PDF operators, and SVG's commands are the same
    operators under different names, so this is a rename and not a conversion:
    `l` is `L`, `c` is `C`, a `re` subpath is its four corners, and `closed`
    (which extract.py *measures* rather than trusting the flag) is `Z`. No
    coordinate is recomputed, which is what keeps a triangle's apex on the
    source's own point.
    """
    parts: list[str] = []
    for sub in path["subpaths"]:
        ops = sub["ops"]
        rect_only = all(op["op"] == "re" for op in ops)
        if not rect_only:
            start = sub["start"]
            parts.append(f"M{fmt(start[0])} {fmt(start[1])}")
        for op in ops:
            points = [float(v) for v in op["points"]]
            if op["op"] == "l":
                for index in range(0, len(points), 2):
                    parts.append(f"L{fmt(points[index])} {fmt(points[index + 1])}")
            elif op["op"] == "c":
                for index in range(0, len(points), 6):
                    parts.append("C" + " ".join(fmt(v) for v in points[index:index + 6]))
            elif op["op"] == "re":
                # A rect is always its own closed subpath in the IR; it is
                # written out in full here so that a subpath is never left
                # relying on a `start` that belongs to a different operator.
                x0, y0, x1, y1 = points
                parts.append(f"M{fmt(x0)} {fmt(y0)}L{fmt(x1)} {fmt(y0)}"
                             f"L{fmt(x1)} {fmt(y1)}L{fmt(x0)} {fmt(y1)}Z")
            else:  # extract.py raises on anything else, so this cannot be reached
                raise SystemExit(f"unknown path op {op['op']!r} in {path['id']}")
        if sub["closed"] and not rect_only:
            parts.append("Z")
    return "".join(parts)


def path_paints(path: dict[str, Any]) -> tuple[str | None, str | None]:
    """(fill, stroke) as #rrggbb, either being None when the path has none.

    Both are carried because a path may do both and the two differ: 2551M's
    pre-printed decimal points are filled *and* stroked, so collapsing them to
    one ink would lose the fact that the mark is 0.72pt wider than its fill.
    """
    has_fill = path.get("fill") is not None or path.get("fill_gray") is not None
    has_stroke = (float(path.get("stroke_width_pt") or 0.0) > 0.0
                  and (path.get("stroke") is not None
                       or path.get("stroke_gray") is not None))
    return (paint_color(path.get("fill"), path.get("fill_gray")) if has_fill else None,
            paint_color(path.get("stroke"), path.get("stroke_gray")) if has_stroke else None)


def path_svg(path: dict[str, Any]) -> str:
    """The <path> element itself, shared by both backends.

    `fill-rule` is only stated where the source stated it, so a path with the
    nonzero default keeps the SVG default and the two documents stay
    comparable.
    """
    fill, stroke = path_paints(path)
    attributes = [f'd="{path_data(path)}"',
                  f'fill="{fill}"' if fill else 'fill="none"']
    if fill and path.get("even_odd"):
        attributes.append('fill-rule="evenodd"')
    if stroke:
        attributes.append(f'stroke="{stroke}"')
        attributes.append(f'stroke-width="{fmt(path["stroke_width_pt"])}"')
    attributes.append(f'data-path-id="{esc_attr(str(path["id"]))}"')
    return f'<path {" ".join(attributes)}/>'


class RuleBackend:
    """Paints rects, paths and placed artwork. The only thing that differs per backend.

    Images and paths ride the backend too. Both are geometry in a box exactly as
    a rule is, so routing them through the same code is what makes the
    round-trip a comparison of *every* painted mark on the page rather than of
    the rules alone.
    """

    name = ""

    def open_page(self, page: dict[str, Any]) -> str:
        raise NotImplementedError

    def close_page(self) -> str:
        raise NotImplementedError

    def rects(self, rects: Sequence[Rect], layer: str) -> str:
        raise NotImplementedError

    def band_container(self, band_id: str, rects: Sequence[Rect]) -> str:
        raise NotImplementedError

    def image(self, image: dict[str, Any], href: str, present: bool) -> str:
        raise NotImplementedError

    def path(self, path: dict[str, Any], page: dict[str, Any]) -> str:
        raise NotImplementedError


class SvgBackend(RuleBackend):
    """One <svg> per page whose user units are page points, 1:1.

    The viewBox is the MediaBox and the element is sized in pt, so a rect's
    x/y/width/height are the PDF's own numbers with no unit conversion left to
    the browser. preserveAspectRatio="none" removes even the fitting step,
    which is already an identity here but need not stay one for a form whose
    page box is not this one.
    """

    name = "svg"

    def open_page(self, page: dict[str, Any]) -> str:
        return (f'<svg class="rl" xmlns="http://www.w3.org/2000/svg" '
                f'viewBox="0 0 {fmt(page["width_pt"])} {fmt(page["height_pt"])}" '
                f'preserveAspectRatio="none" '
                f'style="width:{fmt(page["width_pt"])}pt;'
                f'height:{fmt(page["height_pt"])}pt;z-index:{Z_RULES}">')

    def close_page(self) -> str:
        return "</svg>"

    def rects(self, rects: Sequence[Rect], layer: str) -> str:
        if not rects:
            return ""
        parts = [f'<g class="layer-{layer}">']
        parts.extend(self._rect(rect) for rect in rects)
        parts.append("</g>")
        return "".join(parts)

    @staticmethod
    def _rect(rect: Rect) -> str:
        """One rect, anti-aliased.

        shape-rendering="crispEdges" was here to keep rules sharp, and measuring
        it refuted that. Disabling anti-aliasing makes coverage all-or-nothing
        against the pixel centre, so a rule thinner than one device pixel does
        not get sharper -- it disappears. At device_scale_factor 1 that erased
        every 0.24pt comb divider and every 0.48pt box outline on 2552 page 1,
        which is exactly the structure a comb field consists of.

        Against the official raster, per device-pixel row over the whole page,
        mean |tone delta| with crispEdges vs without: 5.013 -> 1.327 at dsf 1,
        2.789 -> 0.959 at dsf 2, 1.793 -> 0.806 at dsf 4 (2552 p1; 2551Q and
        2316 agree, 2.709 -> 0.889 and 2.462 -> 0.840 at dsf 2). Restricting the
        removal to sub-pixel decorative rects, which is the narrower fix,
        recovers almost none of that: 2.789 -> 2.708 on the same page, and
        nothing at all on 2316, whose structure is stroked black.

        Anti-aliasing is also what the official raster does, so a sub-pixel rule
        lands as the mid-grey tone the source produces rather than as ink or
        nothing. Print is unaffected either way: printing to PDF keeps the rects
        as vectors, and the IR round-trip is unchanged by this attribute.
        """
        rid = f' data-rule-id="{esc_attr(rect.source_id)}"' if rect.source_id else ""
        return (f'<rect x="{fmt(rect.x)}" y="{fmt(rect.y)}" width="{fmt(rect.w)}" '
                f'height="{fmt(rect.h)}" fill="{rect.fill}"{rid}/>')

    def band_container(self, band_id: str, rects: Sequence[Rect]) -> str:
        body = "".join(self._rect(rect) for rect in rects)
        return f'<g class="layer-band" id="band-rules-{band_id}">{body}</g>'

    def image(self, image: dict[str, Any], href: str, present: bool) -> str:
        matrix = placement_matrix(image)
        if matrix is None:
            geometry = (f'x="{fmt(image["x0"])}" y="{fmt(image["y0"])}" '
                        f'width="{fmt(image["x1"] - image["x0"])}" '
                        f'height="{fmt(image["y1"] - image["y0"])}"')
        else:
            # The unit square is the image's own space with (0,0) at its
            # top-left, which is exactly what the matrix is defined against, so
            # the placement is the source's matrix and nothing else.
            geometry = ('x="0" y="0" width="1" height="1" transform="matrix('
                        + ",".join(fmt(v) for v in matrix) + ')"')
        tag = (f'<image href="{esc_attr(href)}" {geometry} preserveAspectRatio="none" '
               if present else
               f'<rect fill="none" {geometry} data-missing-src="{esc_attr(href)}" ')
        return f'{tag}data-sha256="{esc_attr(image["sha256"])}"/>'

    def path(self, path: dict[str, Any], page: dict[str, Any]) -> str:
        return path_svg(path)


class CssBackend(RuleBackend):
    """Every rule an absolutely positioned div painted with background-color.

    `border` is deliberately not used: the shorthand is resolved against the
    used border-width, which Chromium snaps independently of the box position,
    so a 0.24pt border and a 0.24pt-tall background box do not even fail the
    same way. Painting the box makes the failure mode a single one, which is
    what makes the backend comparison mean something.
    """

    name = "css"

    def open_page(self, page: dict[str, Any]) -> str:
        return f'<div class="rl" style="z-index:{Z_RULES}">'

    def close_page(self) -> str:
        return "</div>"

    def rects(self, rects: Sequence[Rect], layer: str) -> str:
        if not rects:
            return ""
        parts = [f'<div class="layer-{layer}">']
        parts.extend(self._rect(rect) for rect in rects)
        parts.append("</div>")
        return "".join(parts)

    @staticmethod
    def _rect(rect: Rect) -> str:
        rid = f' data-rule-id="{esc_attr(rect.source_id)}"' if rect.source_id else ""
        style = style_attr((
            ("left", f"{fmt(rect.x)}pt"), ("top", f"{fmt(rect.y)}pt"),
            ("width", f"{fmt(rect.w)}pt"), ("height", f"{fmt(rect.h)}pt"),
            ("background-color", rect.fill),
        ))
        return f'<div class="r" style="{esc_attr(style)}"{rid}></div>'

    def band_container(self, band_id: str, rects: Sequence[Rect]) -> str:
        body = "".join(self._rect(rect) for rect in rects)
        return f'<div class="layer-band" id="band-rules-{band_id}">{body}</div>'

    def image(self, image: dict[str, Any], href: str, present: bool) -> str:
        matrix = placement_matrix(image)
        if matrix is None:
            style = style_attr((
                ("left", f"{fmt(image['x0'])}pt"), ("top", f"{fmt(image['y0'])}pt"),
                ("width", f"{fmt(image['x1'] - image['x0'])}pt"),
                ("height", f"{fmt(image['y1'] - image['y0'])}pt"),
            ))
        else:
            # A 1pt box scaled by the matrix: scale is unit-agnostic, so a and d
            # land the same lengths the SVG backend does. Only the translation
            # differs -- CSS `matrix()` takes it in px, never in the element's
            # own unit -- so e and f are divided by the device pixel, which is
            # exact in binary and therefore costs no precision.
            style = style_attr((
                ("left", "0"), ("top", "0"), ("width", "1pt"), ("height", "1pt"),
                ("transform-origin", "0 0"),
                ("transform", "matrix("
                 + ",".join(fmt(v) for v in matrix[:4])
                 + f",{fmt(matrix[4] / DEVICE_PX_PT)},{fmt(matrix[5] / DEVICE_PX_PT)})"),
            ))
        common = (f'class="img" data-sha256="{esc_attr(image["sha256"])}" '
                  f'style="{esc_attr(style)}"')
        if present:
            return f'<img src="{esc_attr(href)}" alt="" {common}>'
        return f'<div data-missing-src="{esc_attr(href)}" {common}></div>'

    def path(self, path: dict[str, Any], page: dict[str, Any]) -> str:
        """A path cannot be a CSS box, so it is an SVG of its own.

        No CSS property draws a Bezier, and 257 of the corpus's 944 paths are
        filled curves. The element is page-sized with the page's own viewBox, so
        the geometry inside it is identical to the SVG backend's and the two
        backends still differ in exactly one thing: how *rules* are painted.
        Document order is preserved, so the source's paint order survives.
        """
        style = style_attr((
            ("position", "absolute"), ("left", "0"), ("top", "0"),
            ("width", f"{fmt(page['width_pt'])}pt"),
            ("height", f"{fmt(page['height_pt'])}pt"),
        ))
        return (f'<svg class="pl" xmlns="http://www.w3.org/2000/svg" '
                f'viewBox="0 0 {fmt(page["width_pt"])} {fmt(page["height_pt"])}" '
                f'preserveAspectRatio="none" style="{esc_attr(style)}">'
                f'{path_svg(path)}</svg>')


BACKENDS = {"svg": SvgBackend, "css": CssBackend}


# ---------------------------------------------------------------------------
# Typography
# ---------------------------------------------------------------------------


class RunStyle:
    """Everything needed to place one text run, resolved from the font plan."""

    __slots__ = ("css", "scale_x", "baseline_offset_pt", "top_pt", "translate_y_pt",
                 "font_family", "font_file", "css_style", "font_face_weight",
                 "unresolved", "ink")

    def __init__(self, css: dict[str, Any], scale_x: float | None,
                 baseline_offset_pt: float, top_pt: float, translate_y_pt: float,
                 font_family: str | None, font_file: str | None, css_style: str,
                 unresolved: bool, font_face_weight: str = "100 900",
                 ink: Ink | None = None) -> None:
        self.css = css
        # What this run actually puts on the page: its ink and the anchor that
        # ink sits on. A run with no padding is its own ink, so this is the
        # identity for all but the padded ones. None means "not measured", and
        # then the run's whole string is emitted at its first character's origin.
        self.ink = ink
        self.scale_x = scale_x
        self.baseline_offset_pt = baseline_offset_pt
        self.top_pt = top_pt
        self.translate_y_pt = translate_y_pt
        self.font_family = font_family
        self.font_file = font_file
        self.css_style = css_style
        # The `@font-face` weight *descriptor* for this run's file, straight from
        # the plan. Defaulted to the variable range so a plan written before
        # fonts.py emitted `font_face` still produces what it produced before.
        self.font_face_weight = font_face_weight
        self.unresolved = unresolved

    def text_of(self, run: dict[str, Any]) -> str:
        return self.ink.text if self.ink is not None else run["text"]

    def origin_of(self, run: dict[str, Any]) -> float:
        return (self.ink.origin_x if self.ink is not None
                else float(run["origin_x"] or 0.0))


def _donor_face(faces: Sequence[dict[str, Any]], face: dict[str, Any]) -> dict[str, Any] | None:
    """The metric-compatible face that can carry a non-compatible one under scaleX.

    Chosen by CSS weight and style rather than by name, so the correction is a
    property of the plan and not a hardcoded 'Arial Narrow -> Arimo' table.
    """
    candidates = [f for f in faces
                  if f.get("status") == "resolved" and f.get("metric_compatible")
                  and f.get("css_style") == face.get("css_style")
                  and f.get("css_weight") == face.get("css_weight")
                  and f.get("css_family")]
    return sorted(candidates, key=lambda f: f["face_key"])[0] if candidates else None


def _horizontal_scale(entry: dict[str, Any], face: dict[str, Any]) -> float | None:
    """Scale the plan itself declares, if fonts.py has started emitting one."""
    for source in (entry, face):
        value = source.get("horizontal_scale")
        if value is not None:
            return float(value)
    return None


def _source_tracking(run: dict[str, Any], first: int, last: int,
                     scale: float) -> float | None:
    """The source's own per-gap tracking across glyphs [first, last], un-scaled.

    Derived from the IR alone, which is what makes it deterministic and free of
    any platform font: `char_origin_offsets_pt` gives each glyph's painted
    origin and `char_widths_pt` the advance the face itself would have made, so
    their difference over a gap is the tracking the generator added there.

    A scale multiplies the whole inline box, tracking included, exactly as the
    PDF's Tz operator does: if the unscaled face advances `natural` and we add
    `sp` per gap, the painted advance is scale*(natural + sp*(n-1)). The Narrow
    face *is* the donor face scaled, so `natural` is the PDF's own advances
    divided by the scale and the scale cancels to a single division here.
    """
    gaps = last - first
    if gaps <= 0 or scale <= 0:
        return None
    offsets = run["char_origin_offsets_pt"]
    widths = run["char_widths_pt"]
    if last >= len(offsets) or last >= len(widths):
        return None
    painted = float(offsets[last]) - float(offsets[first])
    natural = sum(float(w) for w in widths[first:last])
    spacing = (painted - natural) / (scale * gaps)
    if (abs(spacing) < LETTER_SPACING_EPSILON_PT
            and abs(spacing) * gaps < LETTER_SPACING_ACCUMULATED_PT):
        return None
    return round(spacing, 4)


def _rescaled_letter_spacing(run: dict[str, Any], scale: float) -> float | None:
    """Tracking for a run whose face is reached through scaleX(scale)."""
    return _source_tracking(run, 0, len(run["text"]) - 1, scale)


class Ink:
    """The part of a run that puts ink on paper, and where its left edge sits.

    A run's padding spaces are position, not content. The BIR generator writes a
    checkbox label as `' 2nd '` and puts 6.86pt of the 9.36pt leading advance in
    a TJ offset, so reproducing the string with the substitute face's own space
    advance -- plus whatever share of the run's tracking the space happens to get
    -- lands the ink somewhere else entirely: measured, 'Calendar' on 2550-DS,
    2550Q and 1707A comes out 4.9-6.0pt to the left, inside the checkbox beside
    it, and 2550Q's 'Quarter' 0.71pt to the right, on top of its neighbour.

    So a padded run is emitted as its ink, anchored on the first *visible*
    glyph's own origin, which is also the point verify.py measures. Nothing is
    lost: a space paints nothing, and the advance it was carrying is expressed
    by the anchor instead of by a font's space width.

    Trimming is restricted to a run whose ink is a single word, and the
    restriction is what keeps it honest rather than convenient. The font plan's
    `letter-spacing` is a claim about the string the plan measured, so emitting a
    different string means re-deriving it, and re-deriving it over a range that
    still contains word gaps would smear those gaps across the letters -- the
    same defect this is undoing. 4390 of the corpus's 10732 padded runs are
    single-word; the rest keep their padding and today's rendering exactly.
    """

    __slots__ = ("text", "origin_x", "letter_spacing_pt", "trimmed")

    def __init__(self, text: str, origin_x: float,
                 letter_spacing_pt: float | None, trimmed: bool) -> None:
        self.text = text
        self.origin_x = origin_x
        self.letter_spacing_pt = letter_spacing_pt
        self.trimmed = trimmed


def run_ink(run: dict[str, Any], scale: float | None) -> Ink:
    """What to emit for one run: its ink, its anchor, and its own tracking."""
    text = run["text"]
    origin = float(run["origin_x"] or 0.0)
    visible = [index for index, char in enumerate(text) if not char.isspace()]
    if not visible:  # extract.py drops whitespace-only runs; this cannot happen
        return Ink(text, origin, None, False)
    first, last = visible[0], visible[-1]
    if (first, last) == (0, len(text) - 1):
        return Ink(text, origin, None, False)
    if any(char.isspace() for char in text[first:last + 1]):
        return Ink(text, origin, None, False)
    offsets = run["char_origin_offsets_pt"]
    if first >= len(offsets):
        return Ink(text, origin, None, False)
    return Ink(text[first:last + 1], origin + float(offsets[first]),
               _source_tracking(run, first, last, scale if scale else 1.0), True)


def _round_half_up(value: float) -> float:
    """Blink rounds font metrics with roundf, i.e. half away from zero."""
    return math.floor(value + 0.5) if value >= 0 else -math.floor(-value + 0.5)


def _baseline_offset_px(css: dict[str, Any], face: dict[str, Any], run: dict[str, Any],
                        warnings: list[str]) -> float:
    """Where Blink puts the baseline inside the run's line box, in device px.

    Measured, not guessed. A probe that printed the same string at 30 tops in
    0.05pt steps came back with exactly three baselines, all on the 0.75pt
    device grid: Blink floors the block's top to an integer device pixel and
    floors the baseline again, and it rounds the face's ascent and descent to
    integer pixels before computing half-leading. Reproducing that arithmetic
    is what lets `translate_y_pt` below cancel it exactly.

    The ascent and descent are the *emitted* face's hhea values -- the ones
    Chromium will actually use -- not the PDF's, which belong to a face we are
    not shipping.
    """
    size = parse_pt(css.get("font-size"), float(run["size_pt"]))
    metrics = face.get("vertical_metrics") or {}
    ascender = metrics.get("css_hhea_ascender")
    descender = metrics.get("css_hhea_descender")
    if ascender is None or descender is None:
        ascender = float(run["ascender"])
        descender = float(run["descender"])
        warnings.append(
            f"face {face.get('face_key')!r} carries no shipped-face vertical metrics; "
            f"baselines fall back to the PDF's own ascender/descender, which belong "
            f"to a face we do not ship")
    size_px = size / DEVICE_PX_PT
    ascent_px = _round_half_up(float(ascender) * size_px)
    descent_px = _round_half_up(-float(descender) * size_px)
    content_px = ascent_px + descent_px
    line_px = parse_pt(css.get("line-height"), content_px * DEVICE_PX_PT) / DEVICE_PX_PT
    return (line_px - content_px) / 2.0 + ascent_px


def _vertical_placement(baseline_y: float, offset_px: float) -> tuple[float, float, float]:
    """Split a baseline into a snapped `top` and an exact translateY residual.

    The probe above proves two things at once: `top` alone cannot express a
    sub-0.75pt baseline, and a `transform` can -- 30 translateY steps of 0.05pt
    produced 30 distinct baselines, off-lattice, tracking the request exactly.

    So the box is placed on the device grid, where Blink's flooring is a
    no-op and therefore harmless, and the remaining fraction is carried by a
    transform, which is applied after layout and is not snapped. Nothing here
    is a fudge factor: `residual` is whatever the reproduced Blink arithmetic
    left over, and it is zero when the baseline already lands on the grid.
    """
    top_px = math.floor((baseline_y - offset_px * DEVICE_PX_PT) / DEVICE_PX_PT)
    painted_px = top_px + math.floor(offset_px)
    return (top_px * DEVICE_PX_PT,
            baseline_y - painted_px * DEVICE_PX_PT,
            offset_px * DEVICE_PX_PT)


def resolve_run_styles(ir: dict[str, Any], plan: dict[str, Any],
                       warnings: list[str]) -> dict[tuple[int, int], RunStyle]:
    """Join the font plan onto the IR runs and apply the Narrow correction.

    The plan as generated maps Arial Narrow onto Roboto Condensed and says so
    honestly (`metric_compatible: false`). Roboto Condensed is an independently
    drawn design, so its glyph origins are wrong inside the run even when
    letter-spacing restores the total width. Where the plan does not already
    carry a `horizontal_scale`, this retargets such runs onto the plan's own
    metric-compatible face under a horizontal scale, which is the operation the
    PDF is performing.
    """
    faces = {f["face_key"]: f for f in plan["faces"]}
    face_list = list(plan["faces"])
    runs_by_key = {(int(e["page"]), int(e["run_index"])): e for e in plan["runs"]}

    styles: dict[tuple[int, int], RunStyle] = {}
    corrected: set[str] = set()

    for page in ir["pages"]:
        for index, run in enumerate(page["text_runs"]):
            key = (int(page["index"]), index)
            entry = runs_by_key.get(key)
            if entry is None or not entry.get("css"):
                warnings.append(
                    f"page {page['index']} run {index}: the font plan resolves no face "
                    f"for {run['text'][:24]!r}; it is emitted with the generic stack so "
                    f"the run is still present, and it will fail the round-trip")
                css = {
                    "font-family": "sans-serif",
                    "font-size": f"{fmt(run['size_pt'])}pt",
                    "font-weight": 700 if run["bold"] else 400,
                    "font-style": "italic" if run["italic"] else "normal",
                    "line-height": f"{fmt(run['line_height_pt'])}pt",
                }
                offset_px = _baseline_offset_px(css, {}, run, [])
                top_pt, translate_y, offset_pt = _vertical_placement(
                    float(run["baseline_y"]), offset_px)
                styles[key] = RunStyle(css, None, offset_pt, top_pt, translate_y,
                                       None, None, css["font-style"], True,
                                       ink=run_ink(run, None))
                continue

            face = faces[entry["face_key"]]
            css = dict(entry["css"])
            scale = _horizontal_scale(entry, face)
            emitted_face = face

            if scale is None and not face.get("metric_compatible", False):
                donor = _donor_face(face_list, face)
                if donor is not None:
                    scale = ARIAL_NARROW_HORIZONTAL_SCALE
                    emitted_face = donor
                    css["font-family"] = donor["css_family_stack"]
                    spacing = _rescaled_letter_spacing(run, scale)
                    css["letter-spacing"] = (f"{fmt(spacing)}pt"
                                             if spacing is not None else None)
                    if face["face_key"] not in corrected:
                        corrected.add(face["face_key"])
                        warnings.append(
                            f"face {face['face_key']} is served by "
                            f"{donor['css_family']} under scaleX("
                            f"{ARIAL_NARROW_HORIZONTAL_SCALE}) instead of the plan's "
                            f"{face.get('css_family')}: the plan itself reports the "
                            f"latter is not metric-compatible, and this family is the "
                            f"donor family at a constant horizontal scale. Emit "
                            f"`horizontal_scale` from fonts.py to make this the plan's "
                            f"own decision.")
                else:
                    warnings.append(
                        f"face {face['face_key']} is not metric-compatible and the plan "
                        f"offers no compatible donor at the same weight/style; its runs "
                        f"keep the plan's substitute and its glyph origins are wrong")

            offset_px = _baseline_offset_px(css, emitted_face, run, warnings)
            top_pt, translate_y, offset_pt = _vertical_placement(
                float(run["baseline_y"]), offset_px)
            styles[key] = RunStyle(
                css=css,
                scale_x=scale,
                baseline_offset_pt=offset_pt,
                top_pt=top_pt,
                translate_y_pt=translate_y,
                font_family=emitted_face.get("css_family"),
                font_file=emitted_face.get("font_file"),
                css_style=str(emitted_face.get("css_style") or "normal"),
                unresolved=False,
                font_face_weight=str(
                    (emitted_face.get("font_face") or {}).get("weight") or "100 900"),
                ink=run_ink(run, scale),
            )
    return styles


# Emitted in this order, so a diff of two generated documents is readable.
# `transform`, `transform-origin` and `display` are managed here rather than
# copied: the first two are derived from the plan's horizontal_scale together
# with the baseline this module computes, and `display` is blockified by CSS on
# an absolutely positioned box anyway. Everything else the plan carries is
# passed through, including keys added after this was written.
FONT_CSS_ORDER = ("font-family", "font-size", "font-weight", "font-style",
                  "letter-spacing", "line-height", "font-kerning",
                  "font-variant-ligatures", "font-feature-settings",
                  "font-variation-settings")
FONT_CSS_MANAGED = frozenset({"transform", "transform-origin", "display"})


def font_declarations(run: dict[str, Any], style: RunStyle) -> list[tuple[str, str | None]]:
    """The plan's CSS for one run, verbatim, in a fixed order.

    With one substitution, and only where the emitted string is not the string
    the plan measured: the plan's `letter-spacing` is
    (measured_advance - natural_advance) / gaps over that whole string, so it is
    not this string's tracking once the padding spaces are gone. Ink re-derives
    it over the glyphs actually emitted; see Ink for why that is restricted to a
    run whose ink is one word.
    """
    css = style.css
    trimmed = style.ink.trimmed if style.ink is not None else False
    pairs: list[tuple[str, str | None]] = []
    for name in FONT_CSS_ORDER:
        value = css.get(name)
        if name == "letter-spacing" and trimmed:
            spacing = style.ink.letter_spacing_pt
            value = None if spacing is None else f"{fmt(spacing)}pt"
        pairs.append((name, None if value is None else str(value)))
    for name in sorted(set(css) - set(FONT_CSS_ORDER) - FONT_CSS_MANAGED):
        value = css[name]
        pairs.append((name, None if value is None else str(value)))
    pairs.append(("color", text_color(run.get("color"))))
    return pairs


def is_scaled(style: RunStyle) -> bool:
    return style.scale_x is not None and abs(style.scale_x - 1.0) > 1e-9


def transform_declarations(style: RunStyle, origin_x: float) -> list[tuple[str, str | None]]:
    """The run's transform: sub-device-pixel placement, then scaleX.

    Measured: an untransformed box keeps sub-pixel *horizontal* text placement
    (245 of 254 glyph origins land off the device grid) while its baseline is
    floored onto that grid, and a translate fixes the baseline without
    disturbing x. A scaleX behaves differently -- it snaps the box's x as well,
    which showed up as 0.02-0.37pt origin errors on exactly the ten scaled
    runs. So a scaled run gives up `left` entirely and carries both axes in the
    transform, where the box origin is (0, grid) and nothing is left to snap.

    scaleX is written last, which in CSS matrix order means it applies to the
    element first, about the run's left baseline -- the point the PDF's Tz
    operator scales about. The identity scale and a zero residual are both
    omitted, so a run already on the grid keeps a transform-free box.
    """
    operations = []
    if is_scaled(style):
        operations.append(f"translate({fmt(origin_x)}pt,{fmt(style.translate_y_pt)}pt)")
        operations.append(f"scaleX({style.scale_x})")
    elif abs(style.translate_y_pt) > 1e-9:
        operations.append(f"translateY({fmt(style.translate_y_pt)}pt)")
    if not operations:
        return []
    return [("transform", " ".join(operations)),
            ("transform-origin", f"0 {fmt(style.baseline_offset_pt)}pt")]


def text_markup(run: dict[str, Any], identifier: str, style: RunStyle,
                extra: Sequence[tuple[str, str]] = ()) -> str:
    """One absolutely positioned run, anchored on its own glyph origin.

    The anchor is the first *visible* glyph's origin_x and the run's baseline_y,
    never the bbox: a bbox is ink extent and therefore depends on which glyphs
    happen to be in the string. How those two numbers reach the page --
    `left`/`top` or a transform -- is decided by `transform_declarations`,
    because Chromium snaps the two axes differently. See Ink for why the padding
    spaces are not part of what is emitted.
    """
    origin = style.origin_of(run)
    pairs: list[tuple[str, str | None]] = [
        ("left", "0" if is_scaled(style) else f"{fmt(origin)}pt"),
        ("top", f"{fmt(style.top_pt)}pt"),
    ]
    pairs.extend(font_declarations(run, style))
    pairs.extend(transform_declarations(style, origin))
    attrs = "".join(f' {name}="{esc_attr(value)}"' for name, value in extra)
    unresolved = ' data-unresolved="true"' if style.unresolved else ""
    return (f'<div class="t" id="{identifier}" style="{esc_attr(style_attr(pairs))}"'
            f'{attrs}{unresolved}>{esc_text(style.text_of(run))}</div>')


def text_json(run: dict[str, Any], identifier: str, style: RunStyle,
              row_index: int, column_role: str | None) -> dict[str, Any]:
    """The same run as data, for the growable template's JS renderer."""
    scale = (style.scale_x if style.scale_x is not None
             and abs(style.scale_x - 1.0) > 1e-9 else None)
    return {
        "id": identifier,
        "row": row_index,
        "role": column_role,
        "text": style.text_of(run),
        "x": round(style.origin_of(run), 4),
        "baseline_y": round(float(run["baseline_y"]), 4),
        "style": style_attr(font_declarations(run, style)),
        # The renderer re-derives `top` and the translateY residual from these
        # two, because a row shifted to an overflow position lands on a
        # different point of the device grid and its residual is not the
        # template row's.
        "baseline_offset_pt": round(style.baseline_offset_pt, 4),
        "scale_x": scale,
    }


# ---------------------------------------------------------------------------
# Fields: the surface a taxpayer types on
# ---------------------------------------------------------------------------
#
# A cell of kind "field" is a box the official sheet left blank. Until this
# layer existed the generated document reproduced the blank exactly and gave
# nobody anywhere to type: 2551Q carried 147 cells marked `data-cell-kind=
# "field"` and not one input. The field layer adds the typing surface without
# adding a single unit of ink, which is what makes it safe to bolt onto a
# document whose whole value is that it round-trips.
#
# Three properties are load-bearing:
#
#   * An empty field prints as nothing. Every affordance is a `:hover`/`:focus`
#     rule under `@media screen`, and print has neither state, so the printed
#     sheet is the same PDF it was before this layer existed even if a
#     stylesheet transform drops the media guard -- which is a thing that has
#     actually happened to this repo's packaged bundles.
#   * A field never states its geometry twice. A comb slot is already a
#     positioned box, so its input is `inset:0` inside it; only a plain field
#     states an inset, and it states offsets rather than a width, so the same
#     markup fits a band row whose cell is a different size.
#   * The typography is the font plan's, not the browser's. A blank has no text
#     of its own to measure, so the face is derived from the sheet's own body
#     text (see FieldFace) and served by the same @font-face, at a size fitted
#     to the box the form actually drew.


# Below this a fitted field would be typographically useless, and a box that
# small is more likely a mis-detected cell than a place to write. It is
# reported, never silently corrected: the box is the source's.
FIELD_MIN_SIZE_PT = 4.0

# Used only when the plan carries no shipped-face vertical metrics, which is
# also warned about. 1.2em is the browser's own `normal` line box, so a field
# fitted through it is no worse placed than an unstyled one.
FALLBACK_LINE_SPAN_EM = 1.2


class FieldFace:
    """The face a taxpayer's own entries are typed in, derived from the sheet.

    A blank has no text of its own, so its typography has to come from
    somewhere else, and the only defensible source is the form's own printed
    body text: whatever face and size the sheet sets most of its characters in
    is what a filled entry has to match, or a filled form stops matching its
    own blanks. It is chosen by glyph count over the *font plan*, so an entry
    is served by the identical @font-face, kerning, ligature and variation
    settings as the pre-printed runs beside it -- not by a look-alike, and not
    by the platform UI font an unstyled <input> would use.

    Candidates are restricted to resolved, metric-compatible faces at unit
    horizontal scale: a face reached through scaleX() carries its advances
    through a transform that would have to be re-derived per box, and a
    non-metric-compatible face has wrong glyph origins by the plan's own
    admission. Upright regular wins over bold and italic where both exist,
    because the runs a BIR sheet sets bold are its headings, not its data.
    """

    __slots__ = ("css", "size_pt", "letter_spacing_pt", "line_span_em", "face_key")

    def __init__(self, css: dict[str, Any], size_pt: float, letter_spacing_pt: float,
                 line_span_em: float, face_key: str) -> None:
        self.css = css
        self.size_pt = size_pt
        self.letter_spacing_pt = letter_spacing_pt
        self.line_span_em = line_span_em
        self.face_key = face_key


def resolve_field_face(plan: dict[str, Any], warnings: list[str]) -> FieldFace | None:
    """The modal body face of the font plan, by glyph count. Deterministic."""
    faces = {f["face_key"]: f for f in plan["faces"]}
    groups: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for entry in plan["runs"]:
        face = faces.get(entry.get("face_key"))
        css = entry.get("css") or {}
        if face is None or face.get("status") != "resolved":
            continue
        if not face.get("metric_compatible") or not css.get("font-family"):
            continue
        scale = entry.get("horizontal_scale")
        if scale is not None and abs(float(scale) - 1.0) > 1e-9:
            continue
        groups.setdefault((str(entry["face_key"]), str(css.get("font-size"))),
                          []).append(entry)

    if not groups:
        warnings.append(
            "the font plan resolves no metric-compatible face at unit scale, so a field "
            "input would be typed in the browser's UI font; no typing surface is emitted "
            "rather than one whose advances are nobody's")
        return None

    def rank(item: tuple[tuple[str, str], list[dict[str, Any]]]) -> tuple[int, int]:
        face = faces[item[0][0]]
        upright = (int(face.get("css_weight") or 400) == 400
                   and str(face.get("css_style") or "normal") == "normal")
        return (1 if upright else 0, sum(int(e.get("chars") or 0) for e in item[1]))

    (face_key, _size), entries = max(sorted(groups.items()), key=rank)
    donor = max(sorted(entries, key=lambda e: (int(e["page"]), int(e["run_index"]))),
                key=lambda e: int(e.get("chars") or 0))
    face = faces[face_key]

    metrics = face.get("vertical_metrics") or {}
    ascender = metrics.get("css_hhea_ascender")
    descender = metrics.get("css_hhea_descender")
    if ascender is None or descender is None:
        warnings.append(
            f"face {face_key!r} carries no shipped-face vertical metrics; field sizes are "
            f"fitted through the browser's {fmt(FALLBACK_LINE_SPAN_EM)}em default line box "
            f"instead of the face's own")
        span = FALLBACK_LINE_SPAN_EM
    else:
        span = float(ascender) - float(descender)

    dropped = {"font-size", "line-height", "letter-spacing", "color"}
    css = {name: value for name, value in (donor.get("css") or {}).items()
           if name not in FONT_CSS_MANAGED and name not in dropped and value is not None}
    # The typed value is the taxpayer's own ink and is always black; the donor
    # run's colour belongs to the printed label it was measured from.
    css["color"] = "#000000"
    return FieldFace(css, parse_pt((donor.get("css") or {}).get("font-size")),
                     parse_pt((donor.get("css") or {}).get("letter-spacing")),
                     span, face_key)


def _border_thickness(cell: dict[str, Any], side: str) -> float:
    entry = (cell.get("border") or {}).get(side) or {}
    return float(entry.get("thickness_pt") or 0.0)


def _floor2(value: float) -> float:
    """A fitted size, rounded down to the extractor's own 2dp quantisation.

    Down, never to nearest: a size rounded up is a size that no longer fits the
    box the form drew, and fitting it is the entire exercise.
    """
    return math.floor(value * 100.0) / 100.0


class FieldBox:
    """One field's writing box and the text metrics fitted into it.

    The writing box is what the source drew, not the cell rect. A comb's is the
    comb's own sub-band -- the extent of the dividers, 7.44pt inside an 18.90pt
    cell on 2551Q -- and a plain field's is the cell inset by the thickness of
    the rule bounding each side. A cell rect runs along its rules' *centres*
    (rule v173 spans x 136.10-136.58 and the cell edge is 136.34), so half a
    thickness already clears the ink; using the whole one clears it and leaves
    the same distance again as clearance. It is the cell's own measurement
    rather than a constant, so a 1.44pt underline and a 0.24pt comb divider do
    not get the same margin.
    """

    __slots__ = ("kind", "inset_trbl", "size_pt", "line_height_pt",
                 "letter_spacing_pt", "capacity")

    def __init__(self, kind: str, inset_trbl: tuple[float, float, float, float] | None,
                 size_pt: float, line_height_pt: float,
                 letter_spacing_pt: float | None, capacity: int | None) -> None:
        self.kind = kind
        self.inset_trbl = inset_trbl
        self.size_pt = size_pt
        self.line_height_pt = line_height_pt
        self.letter_spacing_pt = letter_spacing_pt
        self.capacity = capacity

    @property
    def metrics_key(self) -> tuple[float, float, float | None]:
        return (self.size_pt, self.line_height_pt, self.letter_spacing_pt)


def field_box(cell: dict[str, Any], face: FieldFace) -> FieldBox | None:
    """Fit the body face into one field's writing box. None if it cannot fit."""
    comb = cell.get("comb")
    if comb:
        kind, capacity = "comb", int(comb["cells"])
        inset: tuple[float, float, float, float] | None = None
        height = float(comb.get("height_pt", cell["y1"] - cell["y0"]))
    else:
        kind, capacity = "text", None
        top = _border_thickness(cell, "top")
        right = _border_thickness(cell, "right")
        bottom = _border_thickness(cell, "bottom")
        left = _border_thickness(cell, "left")
        # A cell no bigger than its own borders on an axis gives up the
        # clearance on that axis rather than the box: clearance is the optional
        # part. 2551Q has eight 0.72pt-wide slivers classified as fields with a
        # 1.44pt rule down one side, and inset by it they would be inputs of
        # negative width.
        if top + bottom >= cell["y1"] - cell["y0"]:
            top = bottom = 0.0
        if left + right >= cell["x1"] - cell["x0"]:
            left = right = 0.0
        inset = (top, right, bottom, left) if any((top, right, bottom, left)) else None
        height = (cell["y1"] - cell["y0"]) - top - bottom
    if height <= 0.0 or face.line_span_em <= 0.0:
        return None

    size = min(face.size_pt, _floor2(height / face.line_span_em))
    if size <= 0.0:
        return None
    spacing = None
    if face.size_pt > 0 and abs(face.letter_spacing_pt) > 0:
        # Tracking is a per-em property of the run it was measured from, so it
        # travels to a refitted size in proportion rather than as absolute pt.
        scaled = round(face.letter_spacing_pt * size / face.size_pt, 4)
        if abs(scaled) >= LETTER_SPACING_EPSILON_PT:
            spacing = scaled
    return FieldBox(kind, inset, size, round(height, 4), spacing, capacity)


# A cell whose own width is mostly pre-printed glyph ink is not a blank the
# taxpayer can write in, whatever the box detector made of it: it is a table
# cell whose text lattice.py assigned to a neighbour because the run crosses
# cell boundaries. A majority is the test, and the corpus separates on it rather
# than being tuned to it -- measured over all 4908 non-comb field cells, 50 sit
# at 0.53-0.91 (every statutory bracket on 1700/1701/1701A/1701Q/1701MS page 2,
# 2200A's "Unit of Measure Tax Rate" header, 2200P's "XP010 Lubricating Oils",
# 1601EQ's and 1600WP's ATC rows, 2000-DST's rate table, 1801's OCT/TCT header)
# and the next one down is at 0.44. Nothing lands between them. The remaining
# 4858 are at 0.0.
PREPRINTED_COVERAGE = 0.5


class PrePrintedInk:
    """Where one page's pre-printed glyphs put ink, asked per cell.

    Ink, not run boxes. A run's bbox spans its whole line -- 'Amended Return?
    Yes        No' is one run 132pt wide -- so measuring coverage from bboxes
    reports every checkbox on the corpus as full and would delete the typing
    surface from all of them. What occupies a box is the *non-space* glyphs,
    and the IR states each one's origin and width, so this is measured rather
    than inferred.

    A run counts against a cell only when at least half of the run's own height
    lies inside it. That is what distinguishes "this text is printed in this
    box" from "the line above dips 0.4pt into it": 1604C, 2316 and 2200S all
    have field cells that a neighbouring line grazes by 1-5% of its height.
    """

    __slots__ = ("_buckets",)

    BUCKET_PT = 16.0

    def __init__(self, runs: Sequence[dict[str, Any]]) -> None:
        self._buckets: dict[int, list[tuple[float, float, list[tuple[float, float]]]]] = {}
        for run in runs:
            spans = _glyph_spans(run)
            if not spans:
                continue
            y0, y1 = float(run["y0"]), float(run["y1"])
            entry = (y0, y1, spans)
            for bucket in range(int(y0 // self.BUCKET_PT), int(y1 // self.BUCKET_PT) + 1):
                self._buckets.setdefault(bucket, []).append(entry)

    def coverage(self, cell: dict[str, Any]) -> float:
        """Fraction of the cell's width covered by pre-printed glyph ink."""
        width = float(cell["x1"]) - float(cell["x0"])
        if width <= 0:
            return 0.0
        x0, x1 = float(cell["x0"]), float(cell["x1"])
        y0, y1 = float(cell["y0"]), float(cell["y1"])
        inside: list[tuple[float, float]] = []
        seen: set[int] = set()
        for bucket in range(int(y0 // self.BUCKET_PT), int(y1 // self.BUCKET_PT) + 1):
            for entry in self._buckets.get(bucket, ()):
                if id(entry) in seen:
                    continue
                seen.add(id(entry))
                run_y0, run_y1, spans = entry
                overlap = min(y1, run_y1) - max(y0, run_y0)
                if run_y1 <= run_y0 or overlap < 0.5 * (run_y1 - run_y0):
                    continue
                for span_x0, span_x1 in spans:
                    low, high = max(x0, span_x0), min(x1, span_x1)
                    if high > low:
                        inside.append((low, high))
        if not inside:
            return 0.0
        inside.sort()
        covered = 0.0
        low, high = inside[0]
        for span_x0, span_x1 in inside[1:]:
            if span_x0 > high:
                covered += high - low
                low, high = span_x0, span_x1
            else:
                high = max(high, span_x1)
        covered += high - low
        return covered / width


def _glyph_spans(run: dict[str, Any]) -> list[tuple[float, float]]:
    """Each non-space glyph's x extent, from the IR's own per-glyph metrics."""
    origin = run.get("origin_x")
    if origin is None:
        return []
    offsets = run.get("char_origin_offsets_pt") or []
    widths = run.get("char_widths_pt") or []
    spans: list[tuple[float, float]] = []
    for index, char in enumerate(run["text"]):
        if char.isspace() or index >= len(offsets) or index >= len(widths):
            continue
        start = float(origin) + float(offsets[index])
        spans.append((start, start + float(widths[index])))
    return spans


def field_verdict(cell: dict[str, Any], ink: PrePrintedInk | None) -> tuple[bool, str]:
    """Whether a taxpayer can type in this cell, and why.

    Two rules, in this order, and the order is the point:

      * **A comb-bearing cell is a field whatever text it also holds.** A comb
        *is* the field -- N boxes drawn with tick marks -- and the pre-printed
        "." or "%" inside it is decoration within that field, not a label. Only
        `kind == "field"` used to reach an input, so 2000-DST's whole page-1
        money grid (`kind="mixed", comb=yes`), 2200A's Part III and IV, 2200P's
        Part III, 1801 item 24, 2316 items 23-24 and 1702EX item 18 had no way
        to type an amount at all, headline payable included. 195 cells across
        37 forms.
      * **A blank the source printed over is not a blank.** Independently of
        whether guides.py relocated the table it belongs to, a cell whose
        geometry is filled with pre-printed text gets no input: on 1700 page 2 a
        taxpayer could type over the statutory bracket "Not over P 250,000".
        This is the belt to that fix's braces and it protects the forms whose
        reference tables are never relocated.
    """
    if cell.get("comb"):
        return True, "comb"
    if cell["kind"] != "field":
        return False, cell["kind"]
    if ink is not None and ink.coverage(cell) > PREPRINTED_COVERAGE:
        return False, "pre-printed"
    return True, "field"


class FieldPlan:
    """Every field on the sheet, and the CSS classes their metrics collapse to.

    The metrics are shared: the 65 comb cells on 2551Q page 1 sit in identically
    sized sub-bands, so writing `font-size`/`line-height`/`letter-spacing` onto
    each of their 488 inputs would repeat one declaration 488 times. They are
    collected first, sorted, then named `fh<n>`, which keeps the class index a
    pure function of the layout and therefore deterministic.
    """

    __slots__ = ("face", "boxes", "classes", "small", "blocked")

    def __init__(self, layout: dict[str, Any], face: FieldFace | None,
                 warnings: list[str], ir: dict[str, Any] | None = None) -> None:
        self.face = face
        self.boxes: dict[str, FieldBox] = {}
        self.classes: dict[tuple[float, float, float | None], str] = {}
        self.small: list[str] = []
        # cell id -> why it has no typing surface, for the markup and the report.
        self.blocked: dict[str, str] = {}
        if face is None:
            return
        # Keyed by page index rather than zipped, so a layout page and an IR page
        # cannot be paired by position if either list is ever ordered otherwise.
        ink_by_page = {int(page["index"]): PrePrintedInk(page["text_runs"])
                       for page in (ir or {}).get("pages", ())}
        for page in layout["pages"]:
            ink = ink_by_page.get(int(page["index"]))
            for cell in page["cells"]:
                fillable, reason = field_verdict(cell, ink)
                if not fillable:
                    if reason == "pre-printed":
                        self.blocked[cell["id"]] = reason
                    continue
                box = field_box(cell, face)
                if box is None:
                    warnings.append(f"cell {cell['id']}: the field's writing box has no "
                                    f"height, so it gets no typing surface")
                    continue
                self.boxes[cell["id"]] = box
                if box.size_pt < FIELD_MIN_SIZE_PT:
                    self.small.append(cell["id"])
        # Sorted through an explicit key: the tracking is None for a field whose
        # scaled tracking rounds to nothing, and a bare tuple sort would compare
        # None against a float the day two boxes share a size and differ there.
        distinct = sorted({b.metrics_key for b in self.boxes.values()},
                          key=lambda k: (k[0], k[1], k[2] is not None, k[2] or 0.0))
        for index, key in enumerate(distinct):
            self.classes[key] = f"fh{index}"
        if self.small:
            warnings.append(
                f"{len(self.small)} field(s) fit the body face at under "
                f"{fmt(FIELD_MIN_SIZE_PT)}pt ({', '.join(self.small[:6])}"
                f"{'...' if len(self.small) > 6 else ''}); the box is the source's own and "
                f"is emitted as measured")
        if self.blocked:
            names = sorted(self.blocked)
            warnings.append(
                f"{len(names)} cell(s) the box detector called blank are filled with "
                f"pre-printed text and get no typing surface ({', '.join(names[:6])}"
                f"{'...' if len(names) > 6 else ''}); a taxpayer must not be able to "
                f"type over a statutory rate")
        if ir is None:
            warnings.append(
                "no IR was given to the field plan, so pre-printed occupancy is "
                "unmeasured and a cell filled with statutory text may be editable")

    def __bool__(self) -> bool:
        return bool(self.boxes)

    def of(self, cell_id: str) -> FieldBox | None:
        return self.boxes.get(cell_id)

    def class_of(self, box: FieldBox) -> str:
        return self.classes[box.metrics_key]


def field_input_markup(cell_id: str, box: FieldBox, fields: FieldPlan,
                       slot_index: int | None, live: bool) -> str:
    """One <input>. Its geometry is its parent's unless it is a plain field.

    A template's inputs carry no `id` and no `name`: template content is the
    blueprint for rows that do not exist yet, and a name is an identity. The
    band renderer stamps both from the row's own cell id as it clones.
    """
    classes = ["fi", fields.class_of(box)]
    attrs: list[str] = []
    if slot_index is None:
        if box.inset_trbl:
            inset = " ".join(f"{fmt(v)}pt" for v in box.inset_trbl)
            attrs.append(f'style="inset:{esc_attr(inset)}"')
    else:
        classes.append("fc")
    identity: list[str] = []
    if live:
        suffix = "-i" if slot_index is None else f"-s{slot_index}"
        identity = [f'id="{esc_attr(cell_id + suffix)}"', f'name="{esc_attr(cell_id)}"']
    if slot_index is not None:
        identity.append(f'data-slot-index="{slot_index}"')
        identity.append('maxlength="1"')
    return ('<input type="text" class="' + " ".join(classes) + '" '
            + " ".join(identity + attrs + ['autocomplete="off"', 'spellcheck="false"'])
            + ">")


def field_json(box: FieldBox, fields: FieldPlan) -> dict[str, Any]:
    """The field as data, so the band renderer can build one from scratch."""
    return {
        "kind": box.kind,
        "class": fields.class_of(box),
        "size_pt": box.size_pt,
        "line_height_pt": box.line_height_pt,
        "letter_spacing_pt": box.letter_spacing_pt,
        "capacity": box.capacity,
        "inset_trbl": ([round(v, 4) for v in box.inset_trbl] if box.inset_trbl else None),
    }


FIELD_CSS_ORDER = ("font-family", "font-weight", "font-style", "font-kerning",
                   "font-variant-ligatures", "font-feature-settings",
                   "font-variation-settings", "color")


def field_css(fields: FieldPlan) -> str:
    """The field layer's stylesheet: one face, N fitted metrics, no printed ink.

    The screen affordances are `:hover`/`:focus` only. Print has neither state,
    so an empty field prints as nothing *by construction*, not merely because
    the `@media print` reset below says so.
    """
    if not fields or fields.face is None:
        return ""
    css = fields.face.css
    declarations = [(name, css[name]) for name in FIELD_CSS_ORDER if name in css]
    declarations.extend((name, css[name])
                        for name in sorted(set(css) - set(FIELD_CSS_ORDER)))
    lines = [
        ".fi{position:absolute;inset:0;appearance:none;-webkit-appearance:none;"
        "border:0;margin:0;padding:0;background:transparent;box-shadow:none;outline:0;"
        "border-radius:0;caret-color:#000000;text-rendering:geometricPrecision;"
        + style_attr(declarations) + "}",
        # A character must not show outside the box it was typed in. It is not
        # hypothetical: 1600-VT classifies a 1.04pt-wide sliver as a field, and
        # the digit typed into it printed 5pt wide across the first slot of the
        # comb beside it. The clip is stated on the field box rather than on the
        # input, because the box is what the source drew. Note what it does and
        # does not do: clipping is not deletion, so the round-trip still finds
        # the glyph at its full extent and fill_check.py still attributes it to
        # the field it was typed into. What this fixes is the sheet.
        ".f,.f .s{overflow:hidden}",
        ".fc{text-align:center}",
    ]
    for key, name in sorted(fields.classes.items(), key=lambda kv: kv[1]):
        size, line_height, spacing = key
        pairs = [("font-size", f"{fmt(size)}pt"),
                 ("line-height", f"{fmt(line_height)}pt"),
                 ("letter-spacing", None if spacing is None else f"{fmt(spacing)}pt")]
        lines.append(f".{name}{{{style_attr(pairs)}}}")
    lines.append("@media screen{.fi:hover{background:rgba(21,101,192,.07)}"
                 ".fi:focus{background:rgba(255,213,0,.35)}}")
    # The caret is the one piece of input chrome that is not a background, a
    # border or an outline, and Chromium prints it: a sheet printed with the
    # cursor still in a field came back with a 0.75x6.75pt black bar that the
    # round-trip reported as an extra structural rule, because that is exactly
    # what an extra 1px vertical bar is. Measured, `caret-color` does *not*
    # suppress it in Chromium's print path -- FIELD_JS dropping the focus on
    # `beforeprint` is what does. This declaration is the second line of
    # defence and the correct thing to say, not the fix.
    lines.append("@media print{.fi{border:0;outline:0;background:none;box-shadow:none;"
                 "-webkit-appearance:none;appearance:none;caret-color:transparent}}")
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Cells and combs
# ---------------------------------------------------------------------------


def cell_markup(cell: dict[str, Any], fields: FieldPlan | None = None,
                id_attribute: str = "id") -> str:
    """One addressable box. Fields carry their comb slots; nothing paints ink.

    The rule layer already holds every border and every comb divider, so a cell
    that also drew them would double the ink and make the round-trip diff
    report phantom extra rules. What a cell contributes is identity and exact
    geometry: `id`, `data-cell-kind`, and for a comb the measured slot
    positions, which are never index*pitch -- the pitch is not uniform and the
    deviations reach 0.12pt.

    Inside a <template> the identity moves to `data-cell-id`: template content
    is a blueprint for rows that do not exist yet, and stamping the row it was
    cut from with a live `id` would hand two elements the same identity the
    moment the renderer clones it.

    A field cell also carries its typing surface. The cell *is* the field --
    including a comb, which is one field drawn with N tick marks and not N
    fields -- so the field's name, kind and comb capacity are attributes of the
    cell, and the inputs under it are how that one field is typed into.
    """
    width = cell["x1"] - cell["x0"]
    height = cell["y1"] - cell["y0"]
    style = style_attr((
        ("left", f"{fmt(cell['x0'])}pt"), ("top", f"{fmt(cell['y0'])}pt"),
        ("width", f"{fmt(width)}pt"), ("height", f"{fmt(height)}pt"),
    ))
    kind = cell["kind"]
    box = fields.of(cell["id"]) if fields is not None else None
    if box is not None and box.kind == "comb" and not cell.get("comb"):
        # A guide cut clipped this cell and its comb band went to the other
        # piece. The field is that piece's; this one is a box, not a blank.
        box = None
    # `.f` is the field box -- the thing that clips a typed character to the box
    # the source drew -- so it follows the typing surface and not the box
    # detector's `kind`. A comb-bearing `mixed` cell is a field; a `field` cell
    # filled with pre-printed text is not.
    classes = "c f" if box is not None else "c"
    attrs = [f'{id_attribute}="{esc_attr(cell["id"])}"', f'class="{classes}"',
             f'data-cell-kind="{esc_attr(kind)}"',
             f'data-row="{cell["row"]}"', f'data-col="{cell["col"]}"']
    if not cell.get("rectangular", True):
        attrs.append('data-rectangular="false"')
    if fields is not None and fields.blocked.get(cell["id"]):
        attrs.append('data-preprinted="true"')
    live = id_attribute == "id"
    if box is not None:
        attrs.append(f'data-field-kind="{esc_attr(box.kind)}"')
        if live:
            attrs.append(f'data-field-name="{esc_attr(cell["id"])}"')
        if box.capacity is not None:
            attrs.append(f'data-comb-capacity="{box.capacity}"')

    comb = cell.get("comb")
    body = ""
    if comb:
        attrs.append(f'data-comb-slots="{comb["cells"]}"')
        attrs.append(f'data-comb-pitch="{fmt(comb["pitch_pt"])}"')
        body = comb_slots_markup(cell, comb, box, fields, live)
    elif box is not None:
        body = field_input_markup(cell["id"], box, fields, None, live)
    return f'<div {" ".join(attrs)} style="{esc_attr(style)}">{body}</div>'


def comb_slots_markup(cell: dict[str, Any], comb: dict[str, Any],
                      box: "FieldBox | None" = None,
                      fields: "FieldPlan | None" = None, live: bool = True) -> str:
    """N slots inside ONE field, from the comb's own measured slot_x.

    Each slot holds one input, and that is the whole of "the glyphs land in the
    slots": the input *is* the slot box, so a centred character is centred on a
    measured slot centre by layout rather than by an advance calculation over a
    pitch that is not uniform (2551Q's pitch deviates by up to 0.12pt).
    """
    slot_x = comb["slot_x"]
    top = float(comb.get("y0", cell["y0"])) - cell["y0"]
    height = float(comb.get("height_pt", cell["y1"] - cell["y0"]))
    parts = []
    for index in range(len(slot_x) - 1):
        left = float(slot_x[index]) - cell["x0"]
        width = float(slot_x[index + 1]) - float(slot_x[index])
        style = style_attr((
            ("left", f"{fmt(left)}pt"), ("top", f"{fmt(top)}pt"),
            ("width", f"{fmt(width)}pt"), ("height", f"{fmt(height)}pt"),
        ))
        inner = ("" if box is None or fields is None
                 else field_input_markup(cell["id"], box, fields, index, live))
        parts.append(f'<div class="s" data-slot="{index}" '
                     f'style="{esc_attr(style)}">{inner}</div>')
    return "".join(parts)


def cell_json(cell: dict[str, Any], fields: "FieldPlan | None" = None) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "id": cell["id"],
        "kind": cell["kind"],
        "row": cell["row"], "col": cell["col"],
        "x": round(cell["x0"], 4), "y": round(cell["y0"], 4),
        "w": round(cell["x1"] - cell["x0"], 4), "h": round(cell["y1"] - cell["y0"], 4),
    }
    comb = cell.get("comb")
    if comb:
        payload["comb"] = {
            "cells": comb["cells"],
            "pitch_pt": comb["pitch_pt"],
            "slot_x": [round(float(v) - cell["x0"], 4) for v in comb["slot_x"]],
            "y": round(float(comb.get("y0", cell["y0"])) - cell["y0"], 4),
            "h": round(float(comb.get("height_pt", cell["y1"] - cell["y0"])), 4),
        }
    box = fields.of(cell["id"]) if fields is not None else None
    if box is not None:
        payload["field"] = field_json(box, fields)
    return payload


# ---------------------------------------------------------------------------
# Growable bands
# ---------------------------------------------------------------------------


class BandPlan:
    """A growable band decomposed into what repeats and what stretches.

    A band is not six copies of a picture. Three different things live in it
    and they grow differently:

      row-local   rules, cells and text wholly inside one row slab -> repeat
      boundary    the separator centred on each row_y -> one per boundary,
                  so N rows need N+1 of them
      spanning    a vertical running the full height of the band -> stretches

    Rows inside the official capacity are emitted from their *measured*
    geometry rather than from a template offset, because the pitch is not
    constant: row 6 is 18.27pt where the others are 18.24pt, and the comb
    sub-bands drift independently of the row rules. Rendering row i from
    row_y[i] is exact; rendering it from y0 + i*pitch is not.
    """

    __slots__ = ("band", "page_index", "rules_by_row", "boundary_rules", "spanning_rules",
                 "cells_by_row", "texts_by_row", "rule_ids", "cell_ids", "run_ids",
                 "template_row", "template_boundary", "ordinal_column")

    def __init__(self, band: dict[str, Any], page_index: int) -> None:
        self.band = band
        self.page_index = page_index
        self.rules_by_row: dict[int, list[Rect]] = {}
        self.boundary_rules: dict[int, list[Rect]] = {}
        self.spanning_rules: list[tuple[Rect, float, float]] = []
        self.cells_by_row: dict[int, list[dict[str, Any]]] = {}
        self.texts_by_row: dict[int, list[tuple[str, dict[str, Any], Any]]] = {}
        self.rule_ids: set[str] = set()
        self.cell_ids: set[str] = set()
        self.run_ids: set[str] = set()
        self.template_row: int = 0
        self.template_boundary: int = 0
        self.ordinal_column: tuple[float, float] | None = None

    @property
    def row_y(self) -> list[float]:
        return [float(v) for v in self.band["row_y"]]

    @property
    def capacity(self) -> int:
        return int(self.band["capacity"])


def _row_of(y_top: float, y_bottom: float, row_y: Sequence[float]) -> int | None:
    """Index of the row slab that wholly contains [y_top, y_bottom], if any."""
    for index in range(len(row_y) - 1):
        if (y_top >= row_y[index] - BAND_EPSILON_PT
                and y_bottom <= row_y[index + 1] + BAND_EPSILON_PT):
            return index
    return None


def _boundary_of(centre: float, row_y: Sequence[float]) -> int | None:
    for index, value in enumerate(row_y):
        if abs(centre - value) <= BAND_EPSILON_PT:
            return index
    return None


def _run_index_of(identifier: str) -> int:
    """Recover the IR run index from a `p<page>t<index>` id."""
    return int(identifier.rsplit("t", 1)[-1])


def _run_order(entry: tuple[str, dict[str, Any], Any]) -> int:
    """Sort band text by IR run index, not by id string ('t9' < 't21')."""
    return _run_index_of(entry[0])


def _modal_index(groups: dict[int, list[Rect]], limit: int) -> int:
    """The lowest index whose relative rule signature is the most common one.

    Rows beyond the official capacity have no measured geometry, so they are
    stamped from a template. Taking the modal row rather than row 0 matters:
    the first and last rows of a band are frequently irregular (a column rule
    that starts above the band, a heavier closing rule), and stamping an
    irregular row would make every overflow row wrong in the same way.
    """
    signatures: dict[tuple, list[int]] = {}
    for index in range(limit):
        rects = groups.get(index, [])
        signature = tuple(sorted((round(r.x, 3), round(r.w, 3), round(r.h, 3), r.fill)
                                 for r in rects))
        signatures.setdefault(signature, []).append(index)
    if not signatures:
        return 0
    best = max(sorted(signatures.items(), key=lambda kv: kv[1][0]), key=lambda kv: len(kv[1]))
    return best[1][0]


def build_band_plan(band: dict[str, Any], page_ir: dict[str, Any],
                    cells_by_id: dict[str, dict[str, Any]]) -> BandPlan:
    plan = BandPlan(band, int(page_ir["index"]))
    row_y = plan.row_y
    top, bottom = row_y[0], row_y[-1]
    x_low = float(band["x0"]) - 2.0
    x_high = float(band["x1"]) + 2.0

    for rule in page_ir["rules"]:
        if rule["x0"] < x_low or rule["x1"] > x_high:
            continue
        rect = Rect.from_box(rule, rule["id"])
        if rule["axis"] == "h":
            centre = (rule["y0"] + rule["y1"]) / 2.0
            boundary = _boundary_of(centre, row_y)
            if boundary is not None:
                plan.boundary_rules.setdefault(boundary, []).append(rect)
                plan.rule_ids.add(rule["id"])
                continue
            row = _row_of(rule["y0"], rule["y1"], row_y)
            if row is not None:
                plan.rules_by_row.setdefault(row, []).append(rect)
                plan.rule_ids.add(rule["id"])
            continue
        # Vertical: full-height verticals stretch, row-local verticals repeat,
        # and anything that crosses only *some* rows is left in the static
        # layer -- it is not part of the repeating unit and inventing a growth
        # rule for it would be a guess.
        if rule["y0"] <= top + BAND_EPSILON_PT and rule["y1"] >= bottom - BAND_EPSILON_PT:
            plan.spanning_rules.append((rect, rule["y0"] - top, rule["y1"] - bottom))
            plan.rule_ids.add(rule["id"])
            continue
        row = _row_of(rule["y0"], rule["y1"], row_y)
        if row is not None:
            plan.rules_by_row.setdefault(row, []).append(rect)
            plan.rule_ids.add(rule["id"])

    rows_seen = sorted({cells_by_id[cid]["row"] for cid in band["cell_ids"]})
    row_number = {value: index for index, value in enumerate(rows_seen)}
    for cid in band["cell_ids"]:
        cell = cells_by_id[cid]
        index = row_number[cell["row"]]
        plan.cells_by_row.setdefault(index, []).append(cell)
        plan.cell_ids.add(cid)
        for rid in cell["text_run_ids"]:
            plan.run_ids.add(rid)
            plan.texts_by_row.setdefault(index, []).append((rid, cell, None))

    plan.template_row = _modal_index(plan.rules_by_row, plan.capacity)
    plan.template_boundary = _modal_index(plan.boundary_rules, plan.capacity + 1)

    roles = band.get("column_roles") or []
    columns = band.get("column_x") or []
    for index, role in enumerate(roles):
        if role == "enumerated" and index + 1 < len(columns):
            plan.ordinal_column = (float(columns[index]), float(columns[index + 1]))
            break
    return plan


def band_rects(plan: BandPlan, rows: int) -> list[Rect]:
    """Every rule of the band for `rows` rendered rows, in paint order."""
    row_y = plan.row_y
    capacity = plan.capacity
    pitch = float(plan.band["row_pitch_pt"])

    def y_at(index: int) -> float:
        if index < len(row_y):
            return row_y[index]
        return row_y[-1] + (index - (len(row_y) - 1)) * pitch

    out: list[Rect] = []
    for index in range(rows):
        if index < capacity:
            out.extend(plan.rules_by_row.get(index, []))
        else:
            base = plan.rules_by_row.get(plan.template_row, [])
            delta = y_at(index) - row_y[plan.template_row]
            out.extend(rect.shifted(delta) for rect in base)
    for index in range(rows + 1):
        if index < len(row_y):
            out.extend(plan.boundary_rules.get(index, []))
        else:
            base = plan.boundary_rules.get(plan.template_boundary, [])
            delta = y_at(index) - row_y[plan.template_boundary]
            out.extend(rect.shifted(delta) for rect in base)
    for rect, d_top, d_bottom in plan.spanning_rules:
        y0 = row_y[0] + d_top
        y1 = y_at(rows) + d_bottom
        out.append(Rect(rect.x, y0, rect.w, y1 - y0, rect.fill, rect.source_id))
    return out


def band_json(plan: BandPlan, rendered_rows: int, styles: dict[tuple[int, int], RunStyle],
              runs_by_id: dict[str, dict[str, Any]],
              fields: FieldPlan | None = None) -> dict[str, Any]:
    """The band as data: what the JS renderer needs to reproduce any row count.

    Rows within capacity ship their measured geometry so a re-render at the
    official row count is exact by construction rather than by arithmetic.
    """
    row_y = plan.row_y
    rows = []
    for index in range(plan.capacity):
        texts = []
        for rid, cell, _ in sorted(plan.texts_by_row.get(index, []), key=_run_order):
            run = runs_by_id[rid]
            key = (plan.page_index, _run_index_of(rid))
            role = None
            if plan.ordinal_column and plan.ordinal_column[0] <= cell["x0"] < plan.ordinal_column[1]:
                role = "enumerated"
            texts.append(text_json(run, rid, styles[key], index, role))
        rows.append({
            "index": index,
            "y": round(row_y[index], 4),
            "rules": [r.to_json() for r in plan.rules_by_row.get(index, [])],
            "cells": [cell_json(c, fields)
                      for c in sorted(plan.cells_by_row.get(index, []),
                                      key=lambda c: (c["x0"], c["id"]))],
            "texts": texts,
        })
    return {
        "id": plan.band["id"],
        "page": plan.page_index,
        "kind": plan.band["kind"],
        "capacity": plan.capacity,
        "rendered_rows": rendered_rows,
        "row_pitch_pt": plan.band["row_pitch_pt"],
        "row_y": [round(v, 4) for v in row_y],
        "template_row": plan.template_row,
        "template_boundary": plan.template_boundary,
        "ordinal_column": ([round(v, 4) for v in plan.ordinal_column]
                           if plan.ordinal_column else None),
        "boundaries": [[r.to_json() for r in plan.boundary_rules.get(i, [])]
                       for i in range(plan.capacity + 1)],
        "spanning": [{"rect": rect.to_json(), "d_top": round(d_top, 4),
                      "d_bottom": round(d_bottom, 4)}
                     for rect, d_top, d_bottom in plan.spanning_rules],
        "rows": rows,
    }


def band_template_markup(plan: BandPlan, blob_index: int,
                         fields: FieldPlan | None = None) -> str:
    """One row of markup, emitted once, cloned by the renderer.

    The template is what makes this a generated form rather than a traced
    picture: the sheet's official row count is data, not structure, so the same
    document renders 3 rows or 6 and the borders grow with them.

    Its cell geometry is stated relative to the row's own top edge, because the
    template describes the *shape* of a row. Where a row actually sits, and the
    small per-row deviations from a nominal pitch, come from the measured row
    data the renderer overlays onto the clone.
    """
    cells = sorted(plan.cells_by_row.get(plan.template_row, []),
                   key=lambda c: (c["x0"], c["id"]))
    row_top = plan.row_y[plan.template_row]
    parts = []
    for cell in cells:
        relative = dict(cell)
        relative["y0"] = cell["y0"] - row_top
        relative["y1"] = cell["y1"] - row_top
        comb = cell.get("comb")
        if comb:
            relative["comb"] = dict(comb)
            relative["comb"]["y0"] = float(comb["y0"]) - row_top
        parts.append(cell_markup(relative, fields, id_attribute="data-cell-id"))
    return (f'<template id="band-template-{esc_attr(plan.band["id"])}" '
            f'data-band="{esc_attr(plan.band["id"])}" '
            f'data-band-index="{blob_index}" '
            f'data-capacity="{plan.capacity}" '
            f'data-row-pitch="{fmt(plan.band["row_pitch_pt"])}" '
            f'data-row-y="{esc_attr(",".join(fmt(v) for v in plan.row_y))}" '
            f'data-template-row="{plan.template_row}">{"".join(parts)}</template>')


# ---------------------------------------------------------------------------
# Assets
# ---------------------------------------------------------------------------


def image_markup(image: dict[str, Any], backend: RuleBackend, options: "Options",
                 warnings: list[str]) -> str:
    """The official raster at its exact rect, addressed by content hash.

    A missing asset is a warning and an empty rect of the exact size, never a
    hard failure and never a substitute drawing: emitting visible placeholder
    ink would put ink in the round-trip that the official form does not have,
    turning a missing file into a fake geometry failure.
    """
    name = f"{image['sha256']}.{image.get('ext') or 'png'}"
    assets_dir = options.assets_dir
    href = f"{assets_dir.rstrip('/')}/{name}" if assets_dir else name
    present = (options.out_dir is None
               or (options.out_dir / assets_dir / name).is_file())
    if not present:
        warnings.append(
            f"missing asset {href}: emitting a transparent placeholder rect at the exact "
            f"rect ({fmt(image['x0'])},{fmt(image['y0'])}) "
            f"{fmt(image['x1'] - image['x0'])}x{fmt(image['y1'] - image['y0'])}pt; the "
            f"round-trip will report this image as missing, which is the truth")
    return backend.image(image, href, present)


# ---------------------------------------------------------------------------
# Splitting the sheet into a form document and a guide document
# ---------------------------------------------------------------------------


class PageSplit:
    """Which of one page's elements the document being emitted may carry.

    guides.py claims an element for the guide only when it lies *wholly* below
    the cut. What crosses the cut is not awarded to either side any more: it is
    **clipped**, and the plan carries both pieces. Awarding straddlers to the
    form was chosen so a cut could never lose a rule and had the opposite
    failure -- 1600-PT kept two 461pt verticals with nothing between them, an
    empty three-sided box down two thirds of the page. So a straddler is carried
    by both documents, each drawing its own piece, and `clipped` is what
    substitutes the piece for the whole element.

    Paths are split here rather than in the plan. guides.py predates the IR's
    `paths` and claims no path ids, so the same rule it applies to every other
    element -- wholly below the cut belongs to the guide -- is applied to paths
    directly. It is not a cosmetic detail: 532 of 0605's 584 paths are on the
    page whose whole content is guide material, and drawing them on the form
    would put 532 marks on a sheet that has nothing else left on it.
    """

    __slots__ = ("guide_side", "rule_ids", "cell_ids", "run_indices",
                 "fill_indices", "image_indices", "path_ids", "pieces")

    def __init__(self, guide_side: bool, rule_ids: frozenset[str] = frozenset(),
                 cell_ids: frozenset[str] = frozenset(),
                 run_indices: frozenset[int] = frozenset(),
                 fill_indices: frozenset[int] = frozenset(),
                 image_indices: frozenset[int] = frozenset(),
                 path_ids: frozenset[str] = frozenset(),
                 pieces: dict[tuple[str, str], dict[str, Any]] | None = None) -> None:
        self.guide_side = guide_side
        self.rule_ids = rule_ids
        self.cell_ids = cell_ids
        self.run_indices = run_indices
        self.fill_indices = fill_indices
        self.image_indices = image_indices
        self.path_ids = path_ids
        # (kind, ref) -> this side's clipped geometry, for straddlers only.
        self.pieces = dict(pieces or {})

    def _keep(self, ref: Any, claimed: frozenset) -> bool:
        return (ref in claimed) if self.guide_side else (ref not in claimed)

    def keep_rule(self, rule_id: str) -> bool:
        return ("rule", rule_id) in self.pieces or self._keep(rule_id, self.rule_ids)

    def keep_cell(self, cell_id: str) -> bool:
        return ("cell", cell_id) in self.pieces or self._keep(cell_id, self.cell_ids)

    def keep_run(self, run_index: int) -> bool:
        return self._keep(run_index, self.run_indices)

    def keep_fill(self, index: int) -> bool:
        return (("area_fill", f"#{index}") in self.pieces
                or self._keep(index, self.fill_indices))

    def keep_image(self, index: int) -> bool:
        return (("image", f"#{index}") in self.pieces
                or self._keep(index, self.image_indices))

    def keep_path(self, path_id: str) -> bool:
        return self._keep(path_id, self.path_ids)

    def clipped(self, item: dict[str, Any], kind: str, ref: str) -> dict[str, Any]:
        """`item` with this side's box, when the cut passes through it.

        Only the box is replaced. Tone, role and paint order are properties of
        the ink and not of where it was cut, and a clipped rule is still the
        same rule -- guides.py recomputes `length_pt` per piece for exactly that
        reason, so that the round-trip differ is given the geometry the document
        actually draws.
        """
        piece = self.pieces.get((kind, ref))
        if piece is None:
            return item
        merged = dict(item)
        merged.update({key: piece[key] for key in ("x0", "y0", "x1", "y1")
                       if key in piece})
        if kind == "cell":
            # A band that fell on the other side of the cut is not this piece's
            # to render, and a cut may not pass through one (guides.py refuses
            # such a cut), so the count is 0 or all of them.
            if "combs" in piece and not piece["combs"]:
                merged.pop("comb", None)
            if "text_run_ids" in piece:
                merged["text_run_ids"] = list(piece["text_run_ids"])
        return merged

    def without_band(self, rule_ids: Iterable[str], cell_ids: Iterable[str],
                     run_indices: Iterable[int]) -> "PageSplit":
        """Unclaim everything a growable band owns: a band is always the form's.

        A band regenerates its own rules, cells and text from one blueprint, so
        it cannot hand half of itself to another document without the renderer
        stamping rows that are missing their geometry. Awarding the whole band
        to the form is the same trade the straddler rule makes -- a duplicated
        line on the guide is cosmetic, a band with a hole in it is not. In the
        current corpus no band overlaps a guide region at all, so this is a
        guard rather than a behaviour; --self-test asserts that stays true.
        """
        return PageSplit(self.guide_side,
                         self.rule_ids - frozenset(rule_ids),
                         self.cell_ids - frozenset(cell_ids),
                         self.run_indices - frozenset(run_indices),
                         self.fill_indices, self.image_indices, self.path_ids,
                         self.pieces)


WHOLE_PAGE = PageSplit(guide_side=False)


class DocumentSplit:
    """The guide plan, joined onto one form's IR and read from one side.

    Constructed once per document. With no plan the form side keeps everything
    and the guide side is empty, which is what makes `--document form` without
    `--guide-plan` byte-identical to what this module emitted before splitting
    existed.
    """

    __slots__ = ("document", "plan", "_pages")

    def __init__(self, plan: dict[str, Any] | None, ir: dict[str, Any],
                 document: str) -> None:
        if document not in ("form", "guide"):
            raise SystemExit(f"unknown document {document!r}")
        self.document = document
        self.plan = plan
        self._pages: dict[int, PageSplit] = {}
        if plan is None:
            if document == "guide":
                raise SystemExit("--document guide needs --guide-plan")
            return

        form = plan.get("form") or {}
        if (form.get("code"), form.get("revision")) != (ir["form"]["code"],
                                                        ir["form"]["revision"]):
            raise SystemExit(
                f"guide plan is for {form.get('code')}-{form.get('revision')} "
                f"but the IR is {ir['form']['code']}-{ir['form']['revision']}")

        by_index = {int(page["index"]): page for page in ir["pages"]}
        side = "guide" if document == "guide" else "form"
        for entry in plan.get("inline", []):
            index = int(entry["page"])
            page = by_index.get(index)
            if page is None:
                raise SystemExit(f"guide plan claims page {index}, which the IR has not")
            claimed = PageSplit(
                guide_side=(document == "guide"),
                rule_ids=frozenset(entry["rule_ids"]),
                cell_ids=frozenset(entry["cell_ids"]),
                run_indices=frozenset(int(i) for i in entry["text_run_indices"]),
                fill_indices=frozenset(int(i) for i in entry["area_fill_indices"]),
                image_indices=frozenset(int(i) for i in entry["image_indices"]),
                path_ids=_claimed_paths(page, float(entry["cut_y_pt"])),
                pieces={(s["kind"], s["ref"]): s[side]
                        for s in entry.get("straddlers", ())
                        if s.get("disposition") == "clipped" and s.get(side)},
            )
            _validate_claims(claimed, page, index)
            self._pages[index] = claimed

    def page(self, page_index: int) -> PageSplit:
        default = PageSplit(guide_side=(self.document == "guide"))
        return self._pages.get(int(page_index), default)

    @property
    def guide_pages(self) -> list[int]:
        return sorted(self._pages)

    @property
    def has_guide(self) -> bool:
        return bool(self._pages) or bool(self.standalone_pdfs)

    @property
    def guide_side(self) -> bool:
        return self.document == "guide"

    @property
    def standalone_pdfs(self) -> list[str]:
        return list((self.plan or {}).get("standalone_pdfs") or [])


def _claimed_paths(page: dict[str, Any], cut_y: float) -> frozenset[str]:
    """The path ids that lie wholly below the cut, i.e. the guide's.

    The same test guides.py applies to rules, fills and images. A path that
    *crosses* the cut would need clipping, which is a geometry operation on
    Bezier segments rather than on a box; none occurs in this corpus, and one
    that did would stay on the form -- losing a mark off the form is worse than
    duplicating it -- and say so through `--self-test`.
    """
    return frozenset(str(path["id"]) for path in page.get("paths", ())
                     if float(path["y0"]) >= cut_y)


def straddling_paths(page: dict[str, Any], cut_y: float) -> list[str]:
    return [str(path["id"]) for path in page.get("paths", ())
            if float(path["y0"]) < cut_y < float(path["y1"])]


def _validate_claims(split: PageSplit, page: dict[str, Any], index: int) -> None:
    """A claim on something the page does not have is a stale plan, not a split."""
    known_rules = {rule["id"] for rule in page["rules"]}
    for label, claimed, universe in (
            ("rule", split.rule_ids, known_rules),
            ("text run", split.run_indices, set(range(len(page["text_runs"])))),
            ("area fill", split.fill_indices, set(range(len(page["area_fills"])))),
            ("image", split.image_indices, set(range(len(page["images"]))))):
        unknown = sorted(claimed - universe, key=str)[:3]
        if unknown:
            raise SystemExit(
                f"guide plan claims {label}(s) {unknown} on page {index}, which the "
                f"IR does not have; the plan was built from a different extraction")


# ---------------------------------------------------------------------------
# Guide reflow
# ---------------------------------------------------------------------------
#
# The guide document does not need parity and cannot usefully have it: 1603Q's
# guideline block is two columns of 6pt prose, and reproducing it as absolutely
# positioned runs is what makes those columns overlap on screen. Reflowing is
# the fix that cannot regress -- the runs stop carrying coordinates at all, so
# there is nothing left to overlap.
#
# Everything below is a heuristic, and is labelled as one in the output
# (`data-flow` on each section). None of it is allowed anywhere near the form
# document.

# x is binned at 1pt to find gutters. Finer resolution buys nothing: the
# narrowest real gutter in the corpus is 4pt (2200C) and the coarsest text is
# 6pt, so a bin below a point only splits glyph-level noise.
GUTTER_BIN_PT = 1.0

# A bin is "empty" when this few runs cover it. It is a fraction of the page's
# own peak coverage rather than an absolute count, because a 40-run guide
# region (2000-OT) and a 213-run one (2551M) cannot share a constant. Measured:
# real gutters sit at 4-8% of peak, prose interiors at 70-100%.
GUTTER_COVERAGE_FRACTION = 0.12
MIN_GUTTER_PT = 4.0

# A column narrower than this is not a column; it is a stray run sitting in a
# gutter (2200C, 2551M) or one cell of a table (1600-PT, 2550M). It is
# dissolved into whichever neighbour it is separated from by the *narrower*
# gutter, which is the neighbour it was most likely part of.
MIN_COLUMN_FRACTION = 0.15

# Two runs are on one line when their baselines are within this. The tightest
# real leading in a guide region is 5.16pt (1600-VT) and the closest pair of
# *different* lines that must not merge is 0.48pt apart in different columns
# (2550M) -- but those are separated by column first, so within a column the
# margin is comfortable.
LINE_BASELINE_TOLERANCE_PT = 2.0

# A run overlapping more than one column by more than this spans them.
COLUMN_OVERLAP_PT = 2.0

# Prose vs table, measured as the median fraction of a column's width that a
# line actually puts ink on -- summed run widths, not the line's extent, so a
# rate column and an ATC column sharing a baseline are not mistaken for one
# full-width line. Across the 17 guide regions this separates cleanly: the two
# ATC tables score 0.36-0.55 per column, the thirteen prose regions 0.61-0.95.
# The threshold sits in that gap; if a future form lands between 0.55 and 0.61
# the classification is a coin toss and the region should be looked at.
PROSE_INK_FILL = 0.60

# Paragraph breaks. A gap this much larger than the column's own median line
# gap is a new block; a previous line ending this far short of the column's
# right edge is a finished paragraph.
PARAGRAPH_GAP_FACTOR = 1.6
SHORT_LINE_FRACTION = 0.15

# A wholly bold line narrower than this fraction of its column is a heading.
HEADING_WIDTH_FRACTION = 0.6

# Runs are glyph runs, not words: they are concatenated unless the source left
# a gap wider than this fraction of the font size, which is a space.
WORD_GAP_FRACTION = 0.15

LIST_MARKER = re.compile(r"^\s*(?:\(?\d{1,2}[.)]|\(?[A-Za-z][.)]|[•▪·*‐-]\s)\s*")


def _coverage_gutters(runs: Sequence[dict[str, Any]], x0: float, x1: float
                      ) -> list[tuple[float, float]]:
    """x intervals across which almost nothing is printed."""
    bins = max(1, int((x1 - x0) / GUTTER_BIN_PT) + 1)
    coverage = [0] * bins
    for run in runs:
        low = max(0, int((float(run["x0"]) - x0) / GUTTER_BIN_PT))
        high = min(bins - 1, int((float(run["x1"]) - x0) / GUTTER_BIN_PT))
        for index in range(low, high + 1):
            coverage[index] += 1
    threshold = max(1.0, GUTTER_COVERAGE_FRACTION * max(coverage))

    gutters: list[tuple[float, float]] = []
    index = 0
    while index < bins:
        if coverage[index] > threshold:
            index += 1
            continue
        end = index
        while end < bins and coverage[end] <= threshold:
            end += 1
        low, high = x0 + index * GUTTER_BIN_PT, x0 + end * GUTTER_BIN_PT
        if high - low >= MIN_GUTTER_PT and index > 0 and end < bins:
            gutters.append((low, high))
        index = end
    return gutters


def _dissolve_narrow(columns: list[tuple[float, float]],
                     gutters: list[tuple[float, float]],
                     width: float) -> list[tuple[float, float]]:
    """Merge away columns too narrow to be one, narrowest gutter first."""
    columns = list(columns)
    gutters = list(gutters)
    while len(columns) > 1:
        widths = [high - low for low, high in columns]
        index = min(range(len(columns)), key=widths.__getitem__)
        if widths[index] >= MIN_COLUMN_FRACTION * width:
            break
        left = gutters[index - 1][1] - gutters[index - 1][0] if index > 0 else None
        right = gutters[index][1] - gutters[index][0] if index < len(gutters) else None
        drop = index if left is None or (right is not None and right < left) else index - 1
        columns[drop:drop + 2] = [(columns[drop][0], columns[drop + 1][1])]
        gutters.pop(drop)
    return columns


def _column_bands(runs: Sequence[dict[str, Any]]
                  ) -> tuple[list[tuple[float, float]], list[tuple[float, float]]]:
    """(grid, flow) column bands: the raw gutter grid, and the dissolved one.

    Both are needed. The grid is the table geometry a tabular region is laid out
    on; the flow columns are the reading columns of a prose region, which is the
    grid with its slivers merged away.
    """
    x0 = min(float(run["x0"]) for run in runs)
    x1 = max(float(run["x1"]) for run in runs)
    gutters = _coverage_gutters(runs, x0, x1)
    grid: list[tuple[float, float]] = []
    cursor = x0
    for low, high in gutters:
        grid.append((cursor, low))
        cursor = high
    grid.append((cursor, x1))
    return grid, _dissolve_narrow(grid, gutters, x1 - x0)


def _group_lines(runs: Sequence[dict[str, Any]]) -> list[list[dict[str, Any]]]:
    """Runs grouped into lines by baseline, each line ordered left to right.

    The anchor is the line's first baseline rather than a running mean, so a
    column of tightly leaded rows cannot drift a line's tolerance downward until
    it swallows the next one.
    """
    lines: list[list[dict[str, Any]]] = []
    anchor = None
    for run in sorted(runs, key=lambda r: (float(r["baseline_y"]), float(r["origin_x"]))):
        baseline = float(run["baseline_y"])
        if anchor is None or baseline - anchor > LINE_BASELINE_TOLERANCE_PT:
            anchor = baseline
            lines.append([])
        lines[-1].append(run)
    return [sorted(line, key=lambda r: float(r["origin_x"])) for line in lines]


def _line_baseline(line: Sequence[dict[str, Any]]) -> float:
    return min(float(run["baseline_y"]) for run in line)


def _line_ink(line: Sequence[dict[str, Any]]) -> float:
    return sum(float(run["x1"]) - float(run["x0"]) for run in line)


def _median(values: Sequence[float]) -> float:
    ordered = sorted(values)
    if not ordered:
        return 0.0
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[middle]
    return (ordered[middle - 1] + ordered[middle]) / 2.0


def _column_of(run: dict[str, Any], columns: Sequence[tuple[float, float]]) -> int | None:
    """The single column this run lives in, or None if it spans several."""
    hits = [index for index, (low, high) in enumerate(columns)
            if min(float(run["x1"]), high) - max(float(run["x0"]), low) > COLUMN_OVERLAP_PT]
    if len(hits) == 1:
        return hits[0]
    if hits:
        return None
    centre = (float(run["x0"]) + float(run["x1"])) / 2.0
    return min(range(len(columns)),
               key=lambda i: abs(centre - (columns[i][0] + columns[i][1]) / 2.0))


def _is_prose(runs: Sequence[dict[str, Any]],
              columns: Sequence[tuple[float, float]]) -> bool:
    if len(columns) < 2:
        return False
    for index, (low, high) in enumerate(columns):
        own = [run for run in runs if _column_of(run, columns) == index]
        lines = _group_lines(own)
        if not lines:
            return False
        if _median([_line_ink(line) / (high - low) for line in lines]) < PROSE_INK_FILL:
            return False
    return True


def _line_pieces(line: Sequence[dict[str, Any]]) -> list[tuple[str, Any]]:
    """One line's runs as (text, colour) pieces, spaced where the source was.

    The colour travels with the text because the source's colour is content:
    1600-PT and 1600-VT each carry 25 runs at 0xFFFFFF -- BIR reviewer initials,
    invisible on the official's white paper -- inside the ATC table the guide
    relocates. The form document has emitted every run's own colour all along;
    the reflowed guide dropped it and published those initials in black, where
    they read as ATC data. So the colour is carried here too, and white runs are
    *not* filtered out: they are in the document, and a form may legitimately
    set white text over a dark band.
    """
    pieces: list[tuple[str, Any]] = []
    previous: dict[str, Any] | None = None
    for run in line:
        if previous is not None:
            gap = float(run["x0"]) - float(previous["x1"])
            if gap > WORD_GAP_FRACTION * float(run["size_pt"]):
                pieces.append((" ", previous.get("color")))
        pieces.append((run["text"], run.get("color")))
        previous = run
    return pieces


def _line_text(line: Sequence[dict[str, Any]]) -> str:
    """One line's runs concatenated, with a space wherever the source left one."""
    return "".join(text for text, _colour in _line_pieces(line))


def _render_pieces(pieces: Sequence[tuple[str, Any]]) -> str:
    """Escaped markup for one block: whitespace collapsed, colour preserved.

    Whitespace is collapsed exactly as `" ".join(text.split())` collapses it --
    this is a reading document and the source's line breaks are not its own --
    and a colour is stated only where it is not the black the stylesheet already
    sets, so a block whose every run is black emits the bytes it emitted before
    colour was carried at all.
    """
    chars: list[tuple[str, Any]] = []
    space = False
    for text, colour in pieces:
        for char in text:
            if char.isspace():
                space = True
                continue
            if space and chars:
                # The space keeps the colour of the text *before* it, so a black
                # word followed by a white one does not extend the white span
                # backwards over the gap.
                chars.append((" ", chars[-1][1]))
            space = False
            chars.append((char, colour))
    out: list[str] = []
    index = 0
    while index < len(chars):
        colour = chars[index][1]
        stop = index
        while stop < len(chars) and chars[stop][1] == colour:
            stop += 1
        body = esc_text("".join(char for char, _colour in chars[index:stop]))
        css = text_color(colour)
        out.append(body if css == "#000000"
                   else f'<span style="color:{css}">{body}</span>')
        index = stop
    return "".join(out)


def _is_heading(line: Sequence[dict[str, Any]], column_width: float) -> bool:
    return (all(run.get("bold") for run in line)
            and _line_ink(line) < HEADING_WIDTH_FRACTION * column_width
            and bool(_line_text(line).strip()))


def _blocks_of_lines(lines: Sequence[Sequence[dict[str, Any]]],
                     column_width: float, heading_tag: str) -> list[tuple[str, str]]:
    """One column's lines as (tag, markup) blocks: headings and paragraphs."""
    baselines = [_line_baseline(line) for line in lines]
    gaps = [b - a for a, b in zip(baselines, baselines[1:])]
    typical = _median(gaps) or 1.0
    right_edge = max((max(float(r["x1"]) for r in line) for line in lines), default=0.0)

    out: list[tuple[str, str]] = []
    buffer: list[tuple[str, Any]] = []

    def flush() -> None:
        if buffer:
            out.append(("p", _render_pieces(buffer)))
            buffer.clear()

    previous: Sequence[dict[str, Any]] | None = None
    for index, line in enumerate(lines):
        text = _line_text(line)
        if not text.strip():
            continue
        if _is_heading(line, column_width):
            flush()
            out.append((heading_tag, _render_pieces(_line_pieces(line))))
            previous = line
            continue
        if previous is not None:
            gap = baselines[index] - baselines[index - 1]
            ended_short = (right_edge - max(float(r["x1"]) for r in previous)
                           > SHORT_LINE_FRACTION * column_width)
            if (gap > PARAGRAPH_GAP_FACTOR * typical
                    or ended_short
                    or _is_heading(previous, column_width)
                    or LIST_MARKER.match(text)):
                flush()
        if buffer:
            buffer.append((" ", None))
        buffer.extend(_line_pieces(line))
        previous = line
    flush()
    return out


def _prose_markup(runs: Sequence[dict[str, Any]],
                  columns: Sequence[tuple[float, float]]) -> str:
    """Multi-column prose in reading order: down a column, then the next.

    A run overlapping several columns is a full-width line and splits the page
    into blocks, so a heading that spans the columns is emitted where it belongs
    rather than being dragged into whichever column it happens to start in.
    Anything sharing a baseline with such a run joins it, which is what keeps
    1603Q's "[January 2018 (ENCS)]" attached to its title instead of opening the
    right-hand column.
    """
    spanning = [run for run in runs if _column_of(run, columns) is None]
    spanning_baselines = sorted({float(run["baseline_y"]) for run in spanning})

    def spans(run: dict[str, Any]) -> bool:
        baseline = float(run["baseline_y"])
        return any(abs(baseline - value) <= LINE_BASELINE_TOLERANCE_PT
                   for value in spanning_baselines)

    spanning_lines = _group_lines([run for run in runs if spans(run)])
    column_runs: dict[int, list[dict[str, Any]]] = {i: [] for i in range(len(columns))}
    for run in runs:
        if spans(run):
            continue
        column_runs[_column_of(run, columns)].append(run)

    cuts = [_line_baseline(line) for line in spanning_lines]
    width = max(high - low for low, high in columns)

    def block_of(baseline: float) -> int:
        return sum(1 for cut in cuts if baseline > cut + LINE_BASELINE_TOLERANCE_PT)

    per_block: dict[int, dict[int, list[list[dict[str, Any]]]]] = {}
    for index, runs_here in column_runs.items():
        for line in _group_lines(runs_here):
            block = block_of(_line_baseline(line))
            per_block.setdefault(block, {}).setdefault(index, []).append(line)

    parts: list[str] = []

    def emit_block(block: int) -> None:
        for index in range(len(columns)):
            lines = per_block.get(block, {}).get(index)
            if not lines:
                continue
            column_width = columns[index][1] - columns[index][0]
            parts.append(f'<div class="gl-col" data-column="{index}">')
            for tag, markup in _blocks_of_lines(lines, column_width, "h3"):
                parts.append(f"<{tag}>{markup}</{tag}>")
            parts.append("</div>")

    emit_block(0)
    for index, line in enumerate(spanning_lines):
        for tag, markup in _blocks_of_lines([line], width, "h2"):
            parts.append(f"<{tag}>{markup}</{tag}>")
        emit_block(index + 1)
    return "".join(parts)


def _table_markup(runs: Sequence[dict[str, Any]],
                  grid: Sequence[tuple[float, float]]) -> str:
    """A tabular guide region as a real table: one row per baseline, cells on the grid.

    Reading order for a table is row-major, and column-major flow would list
    every ATC code and then every description. The grid is the *undissolved*
    gutter geometry, which is exactly the table's own column structure --
    2550M's nine bands are its three repeated (index, industry, ATC) triples.
    """
    lines = _group_lines(runs)
    if len(grid) < 2:
        parts = ['<div class="gl-col" data-column="0">']
        for tag, markup in _blocks_of_lines(lines, grid[0][1] - grid[0][0] if grid else 1.0,
                                            "h3"):
            parts.append(f"<{tag}>{markup}</{tag}>")
        parts.append("</div>")
        return "".join(parts)

    rows: list[str] = []
    for position, line in enumerate(lines):
        cells: dict[int, tuple[int, list[dict[str, Any]]]] = {}
        for run in line:
            hits = [i for i, (low, high) in enumerate(grid)
                    if min(float(run["x1"]), high) - max(float(run["x0"]), low)
                    > COLUMN_OVERLAP_PT]
            if not hits:
                hits = [_column_of(run, grid) or 0]
            start = hits[0]
            span, existing = cells.get(start, (1, []))
            cells[start] = (max(span, len(hits)), existing + [run])
        tag = "th" if all(run.get("bold") for run in line) and position == 0 else "td"
        body: list[str] = []
        index = 0
        while index < len(grid):
            if index not in cells:
                body.append(f"<{tag}></{tag}>")
                index += 1
                continue
            span, group = cells[index]
            attribute = f' colspan="{span}"' if span > 1 else ""
            markup = _render_pieces(
                _line_pieces(sorted(group, key=lambda r: float(r["origin_x"]))))
            body.append(f"<{tag}{attribute}>{markup}</{tag}>")
            index += span
        rows.append(f"<tr>{''.join(body)}</tr>")
    return f'<table class="gl-table">{"".join(rows)}</table>'


def _lattice_rows(cells: Sequence[dict[str, Any]]
                  ) -> list[tuple[float, list[dict[str, Any]]]]:
    """The band's cells grouped into lattice rows, ordered down the page."""
    by_row: dict[Any, list[dict[str, Any]]] = {}
    for cell in cells:
        by_row.setdefault(cell["row"], []).append(cell)
    rows = [(min(float(c["y0"]) for c in group),
             sorted(group, key=lambda c: (float(c["x0"]), c["id"])))
            for group in by_row.values()]
    rows.sort(key=lambda item: (item[0], item[1][0]["id"]))
    return rows


def _column_grid(cells: Sequence[dict[str, Any]]) -> list[float]:
    """The band's column edges: every distinct cell edge, left to right."""
    edges = sorted({round(float(c["x0"]), 2) for c in cells}
                   | {round(float(c["x1"]), 2) for c in cells})
    return edges


def _cells_of_run(run: dict[str, Any], cells: Sequence[dict[str, Any]]
                  ) -> list[dict[str, Any]]:
    """The cells of one lattice row that this run puts ink in, left to right.

    A run frequently spans several cells of its row, because the source draws a
    statutory bracket as one string across the whole row: 1700's
    " Over P 250,000 but not over P 400,000     20% of the excess over P 250,000 "
    is a single run 280pt wide crossing both columns of TABLE 1. It is neither
    split (there is nothing to split it on but a guess) nor dropped into the
    column it happens to cover most, which left the row's description cell empty
    and its rate cell holding both -- the exact pattern C9 is about. It is
    reported as spanning, and the row emits it as one cell with a colspan.
    """
    centre = (float(run["y0"]) + float(run["y1"])) / 2.0
    row = [cell for cell in cells if float(cell["y0"]) <= centre <= float(cell["y1"])]
    hit = [cell for cell in row
           if min(float(run["x1"]), float(cell["x1"]))
           - max(float(run["x0"]), float(cell["x0"])) > COLUMN_OVERLAP_PT]
    if not hit and row:
        best = max(row, key=lambda cell: (
            min(float(run["x1"]), float(cell["x1"]))
            - max(float(run["x0"]), float(cell["x0"])), -float(cell["x0"])))
        overlap = (min(float(run["x1"]), float(best["x1"]))
                   - max(float(run["x0"]), float(best["x0"])))
        hit = [best] if overlap > 0 else []
    return sorted(hit, key=lambda cell: float(cell["x0"]))


def _lattice_table_markup(runs: Sequence[dict[str, Any]],
                          cells: Sequence[dict[str, Any]]) -> str:
    """A relocated table rebuilt on the lattice's own rows and columns.

    This is what keeps a rate attached to its nature of payment. The reflow used
    to lay out one row per printed *line*, so a two-line ATC description stranded
    its rate and code on a row whose description was empty: 1600-PT's guide read
    "Franchise Tax on radio & TV broadcasting companies whose annual gross
    receipts do not exceed P10M &" with no rate, and then a row containing only
    "3% | WB 050". A reader can attach that rate to the wrong nature of payment,
    which is the one defect in the 51-form review that is a correctness hazard
    rather than an appearance one.

    lattice.py already knows the row structure -- the table's own ruled grid --
    so a row here is a lattice row and a cell holds every line of text that falls
    inside it, however many lines that is.

    Returns "" -- meaning "the caller should lay this section out from its lines"
    -- unless the lattice describes *all* of the band's ink. That is an all-or-
    nothing condition rather than a threshold, and the corpus separates on it
    with nothing in between: 12 of the 14 ruled bands put every claimed run in a
    cell, and the two that do not (0605 p2 and 2550M p3, both 0.67) would come
    out as a structured table with a third of its content stranded in full-width
    rows between the rows it belongs to, which is less readable than the uniform
    line layout, not more.
    """
    rows = _lattice_rows(cells)
    edges = _column_grid(cells)
    if len(rows) < 2 or len(edges) < 3:
        return ""
    if any(not _cells_of_run(run, cells) for run in runs):
        return ""

    # The grid is built from the cells' own edges, so a cell's span is a pair of
    # edge *indices* and needs no interval test. Asking which interval an edge
    # falls into asks a question whose answer is on a boundary, and that is how
    # the description column came to swallow the rate column beside it.
    def edge_index(value: float) -> int:
        return min(range(len(edges)), key=lambda i: (abs(edges[i] - value), i))

    # Anchor cell id -> its runs, and how far right the widest of them reaches.
    contents: dict[str, list[dict[str, Any]]] = {}
    reach: dict[str, float] = {}
    for run in runs:
        hit = _cells_of_run(run, cells)
        anchor = hit[0]["id"]
        contents.setdefault(anchor, []).append(run)
        reach[anchor] = max(reach.get(anchor, 0.0), float(hit[-1]["x1"]))

    columns = len(edges) - 1
    blocks: list[tuple[float, str]] = []
    for position, (row_y, row_cells) in enumerate(rows):
        body: list[str] = []
        cursor = 0
        heading = True
        for cell in row_cells:
            start = edge_index(float(cell["x0"]))
            if start < cursor:  # a spanning run already covered this column
                continue
            group = sorted(contents.get(cell["id"], ()),
                           key=lambda r: (float(r["baseline_y"]), float(r["origin_x"])))
            stop = max(start + 1,
                       edge_index(max(float(cell["x1"]),
                                      reach.get(cell["id"], 0.0))))
            while cursor < start:
                body.append("<td></td>")
                cursor += 1
            pieces: list[tuple[str, Any]] = []
            for line in _group_lines(group):
                if pieces:
                    pieces.append((" ", None))
                pieces.extend(_line_pieces(line))
            span = stop - cursor
            attribute = f' colspan="{span}"' if span > 1 else ""
            body.append(f"<td{attribute}>{_render_pieces(pieces)}</td>")
            heading = heading and bool(group) and all(r.get("bold") for r in group)
            cursor = stop
        while cursor < columns:
            body.append("<td></td>")
            cursor += 1
        tag = "th" if heading and position == 0 else "td"
        if tag == "th":
            body = [piece.replace("<td", "<th").replace("</td>", "</th>")
                    for piece in body]
        blocks.append((row_y, f"<tr>{''.join(body)}</tr>"))
    blocks.sort(key=lambda item: item[0])
    return f'<table class="gl-table">{"".join(body for _y, body in blocks)}</table>'


# Two cell edges this close are the same edge. It is the extractor's own 2dp
# quantisation, not a tolerance: an edge is stated to a hundredth of a point and
# nothing between two hundredths exists to be confused.
EDGE_EPSILON_PT = 0.01


def _ruled_bands(cells: Sequence[dict[str, Any]]) -> list[tuple[float, float]]:
    """The y intervals the lattice found ruled structure in, merged."""
    spans = sorted((float(c["y0"]), float(c["y1"])) for c in cells)
    merged: list[list[float]] = []
    for y0, y1 in spans:
        if merged and y0 <= merged[-1][1]:
            merged[-1][1] = max(merged[-1][1], y1)
        else:
            merged.append([y0, y1])
    return [(y0, y1) for y0, y1 in merged]


def _region_sections(runs: Sequence[dict[str, Any]],
                     cells: Sequence[dict[str, Any]]
                     ) -> list[tuple[str, list[dict[str, Any]], list[dict[str, Any]]]]:
    """Split a guide region into ruled and unruled stretches, in page order.

    One region is regularly two documents. 2551M page 2 carries an unruled ATC
    rate table and then the "Guidelines and Instructions" prose, and measuring
    columns and ink-fill over the pair classified the whole thing as prose --
    which is how its rate table came to be published as running text with the
    column relationship destroyed. Splitting at the lattice's own boundary lets
    each stretch be classified on its own evidence.

    The split is on where the lattice found ruled cells, so it costs no
    threshold: a stretch either has a ruled grid in it or it has not.
    """
    bands = _ruled_bands(cells)
    if not bands or not runs:
        return [("unruled", list(runs), [])]

    def band_of(run: dict[str, Any]) -> int | None:
        centre = (float(run["y0"]) + float(run["y1"])) / 2.0
        for index, (y0, y1) in enumerate(bands):
            if y0 <= centre <= y1:
                return index
        return None

    keyed = sorted(((float(r["y0"]), float(r["x0"]), band_of(r), r) for r in runs),
                   key=lambda item: (item[0], item[1]))
    sections: list[tuple[str, list[dict[str, Any]], list[dict[str, Any]]]] = []
    current: int | None = -1
    for _y0, _x0, band, run in keyed:
        if band != current or not sections:
            current = band
            if band is None:
                sections.append(("unruled", [], []))
            else:
                y0, y1 = bands[band]
                inside = [c for c in cells
                          if float(c["y0"]) >= y0 - EDGE_EPSILON_PT
                          and float(c["y1"]) <= y1 + EDGE_EPSILON_PT]
                sections.append(("ruled", [], inside))
        sections[-1][1].append(run)
    return sections


def reflow_page(page_ir: dict[str, Any], split: PageSplit,
                page_layout: dict[str, Any] | None = None) -> str:
    """One page's guide region as flowing HTML.

    With a layout the region is split at the lattice's ruled bands and each
    stretch is laid out as what it is; without one (a standalone guide PDF has
    no lattice) the whole region is classified as before.
    """
    runs = [run for index, run in enumerate(page_ir["text_runs"])
            if index in split.run_indices]
    if not runs:
        return ""
    cells = [cell for cell in (page_layout or {}).get("cells", ())
             if cell["id"] in split.cell_ids]
    parts: list[str] = []
    flows: list[str] = []
    for kind, section_runs, section_cells in _region_sections(runs, cells):
        if not section_runs:
            continue
        markup = (_lattice_table_markup(section_runs, section_cells)
                  if kind == "ruled" else "")
        if markup:
            flows.append("lattice")
            parts.append(markup)
            continue
        grid, flow = _column_bands(section_runs)
        prose = _is_prose(section_runs, flow)
        flows.append("prose" if prose else "table")
        parts.append(_prose_markup(section_runs, flow) if prose
                     else _table_markup(section_runs, grid))
    return (f'<section class="gl-page" data-page="{page_ir["index"]}" '
            f'data-flow="{",".join(flows)}" '
            f'data-sections="{len(parts)}">{"".join(parts)}</section>')


# ---------------------------------------------------------------------------
# Document
# ---------------------------------------------------------------------------


BASE_CSS = """*{margin:0;padding:0;box-sizing:border-box}
html,body{background:#fff;-webkit-print-color-adjust:exact;print-color-adjust:exact}
.page{position:relative;overflow:hidden;background:#fff;break-after:page;page-break-after:always}
.page:last-of-type{break-after:auto;page-break-after:auto}
.rl{position:absolute;left:0;top:0}
.r{position:absolute}
.t{position:absolute;white-space:pre;text-rendering:geometricPrecision;z-index:%(z_text)d}
.c{position:absolute;z-index:%(z_cells)d}
.s{position:absolute}
.img{position:absolute;z-index:%(z_cells)d;display:block}
.band{position:absolute;left:0;top:0;width:100%%;height:100%%}
""" % {"z_text": Z_TEXT, "z_cells": Z_CELLS}


# Emitted only when there is a sibling document to point at, and never part of
# BASE_CSS, so a form with no guide keeps the stylesheet it had.
#
# `position:absolute` is the load-bearing part. `.page` is `position:relative`
# and therefore in normal flow, so a link in flow ahead of it would push every
# page down and move every rule and glyph on the sheet. Out of flow it cannot,
# and `@media print` removes it from the printed document entirely, which is
# the document verify.py measures.
DOC_LINK_CSS = (".doc-link{position:absolute;left:0;top:0;z-index:9;"
                "font:12px/1.5 system-ui,-apple-system,sans-serif;padding:2px 8px;"
                "background:#fff;color:#0645ad;text-decoration:underline}\n"
                "@media print{.doc-link{display:none}}")


# The reflowed guide is a reading document, so it is typeset for reading rather
# than for the source's metrics. No @font-face is emitted for it on purpose:
# shipping the measured WOFF2 with text that has deliberately been re-broken
# would imply an advance-level fidelity this document does not have and is not
# trying to have.
#
# The @media print block is where the guide stops being a web page. Its rules
# are pagination, not decoration: a heading must not be the last thing on a
# sheet, a table row must not be sliced in half, and the navigation links back
# to the form are furniture that means nothing on paper.
GUIDE_CSS = """body{background:#fff;color:#111}
.gl{max-width:46em;margin:0 auto;padding:3em 1.5em 4em;
font:16px/1.6 Georgia,"Times New Roman",serif}
.gl h1{font-size:1.6em;margin:0 0 .2em}
.gl .gl-sub{color:#555;font-size:.9em;margin:0 0 2em}
.gl h2{font-size:1.25em;margin:2em 0 .5em;line-height:1.3}
.gl h3{font-size:1.05em;margin:1.5em 0 .4em}
.gl p{margin:0 0 .8em;text-align:justify;hyphens:auto}
.gl .gl-page{margin:0 0 2.5em}
.gl .gl-table{border-collapse:collapse;width:100%;font-size:.85em;margin:1em 0}
.gl .gl-table td,.gl .gl-table th{border:1px solid #bbb;padding:.25em .5em;
text-align:left;vertical-align:top}
.gl .gl-table th{background:#f0f0f0}
@media print{
.gl{max-width:none;padding:0;font-size:11pt}
.gl h1,.gl h2,.gl h3{break-after:avoid;page-break-after:avoid;
break-inside:avoid;page-break-inside:avoid}
.gl p{orphans:2;widows:2}
.gl li,.gl .gl-sub,.gl .gl-download,.gl .gl-table tr{
break-inside:avoid;page-break-inside:avoid}
.gl .gl-table thead{display:table-header-group}
.gl .gl-page{margin:0 0 1.5em}
}"""


# Shared by both guide layouts: the absolute one needs it too, and it is the
# only styling that document has beyond the form's own scaffolding.
#
# `.gl-pdf object` is the fallback path only -- a guide PDF that came with an
# extraction is reflowed into this document instead, because a plugin viewport
# is not printable content. Where the fallback is taken the print rules make it
# say so rather than printing a blank frame.
GUIDE_PDF_CSS = """.gl-pdf{margin:2.5em auto;max-width:46em;
font:16px/1.6 Georgia,"Times New Roman",serif}
.gl-pdf object{display:block;width:100%;height:90vh;border:1px solid #bbb}
.gl .gl-download{color:#555;font-size:.9em;margin:0 0 1.5em}
@media print{
.gl-pdf{margin:0;max-width:none}
.gl-page+.gl-pdf,.gl-pdf+.gl-pdf{break-before:page;page-break-before:always}
.gl-pdf object{display:none}
}"""


# The form prints with `margin:0` because every coordinate on it is measured
# from the MediaBox and a margin would move all of them. The reflowed guide has
# no such coordinates -- it is prose -- and prose set to the paper's edge runs
# into the unprintable border that consumer printers reserve. 36pt (half an
# inch) clears that border on every common printer, so it is the widest measure
# that is safe to print unscaled anywhere.
GUIDE_PAGE_MARGIN_PT = 36.0


def guide_page_css(ir: dict[str, Any]) -> str:
    """@page for the reflowed guide: the form's paper, with a reading margin.

    Sized from the *form's* paper block rather than from the guide's own
    content, because a guide is printed to be read alongside its form and a
    Legal form whose instructions come out on Letter is two stacks of paper
    instead of one. There is deliberately no per-page rule here as there is for
    the form: the reflowed guide has no page boxes of its own, so it flows
    across as many sheets as the prose needs and every one of them is the same
    size.
    """
    paper = ir["paper"]
    return (f'@page{{size:{fmt(paper["width_pt"])}pt {fmt(paper["height_pt"])}pt;'
            f'margin:{fmt(GUIDE_PAGE_MARGIN_PT)}pt}}')


BAND_JS = r"""(function(){
"use strict";
var SVG_NS="http://www.w3.org/2000/svg";
var backend=document.documentElement.getAttribute("data-rule-backend");
var node=document.getElementById("formgen-bands");
var bands=node?JSON.parse(node.textContent):[];
var byId={};
bands.forEach(function(b){byId[b.id]=b;});

function rowY(band,i){
  var ys=band.row_y;
  if(i<ys.length){return ys[i];}
  return ys[ys.length-1]+(i-(ys.length-1))*band.row_pitch_pt;
}
function shift(rect,dy){
  return {x:rect.x,y:rect.y+dy,w:rect.w,h:rect.h,fill:rect.fill,id:rect.id};
}
function paint(target,rect){
  var el;
  if(backend==="svg"){
    el=document.createElementNS(SVG_NS,"rect");
    el.setAttribute("x",rect.x);el.setAttribute("y",rect.y);
    el.setAttribute("width",rect.w);el.setAttribute("height",rect.h);
    el.setAttribute("fill",rect.fill);
    /* no shape-rendering: a re-rendered row must paint exactly as the
       pre-rendered one does, and _rect() explains why that is anti-aliased */
    if(rect.id){el.setAttribute("data-rule-id",rect.id);}
  }else{
    el=document.createElement("div");
    el.className="r";
    el.style.cssText="left:"+rect.x+"pt;top:"+rect.y+"pt;width:"+rect.w+
      "pt;height:"+rect.h+"pt;background-color:"+rect.fill;
    if(rect.id){el.setAttribute("data-rule-id",rect.id);}
  }
  target.appendChild(el);
}
/* Rules for `rows` rendered rows. Rows inside capacity use their measured
   geometry; only overflow rows are stamped from the template, because the
   measured pitch is not constant (row 6 is 18.27pt, the rest 18.24pt). */
function bandRects(band,rows){
  var out=[],i,j,base,delta;
  for(i=0;i<rows;i++){
    if(i<band.capacity){
      base=band.rows[i].rules;
      for(j=0;j<base.length;j++){out.push(base[j]);}
    }else{
      base=band.rows[band.template_row].rules;
      delta=rowY(band,i)-band.row_y[band.template_row];
      for(j=0;j<base.length;j++){out.push(shift(base[j],delta));}
    }
  }
  for(i=0;i<=rows;i++){
    if(i<band.row_y.length){
      base=band.boundaries[i]||[];
      for(j=0;j<base.length;j++){out.push(base[j]);}
    }else{
      base=band.boundaries[band.template_boundary]||[];
      delta=rowY(band,i)-band.row_y[band.template_boundary];
      for(j=0;j<base.length;j++){out.push(shift(base[j],delta));}
    }
  }
  for(i=0;i<band.spanning.length;i++){
    var s=band.spanning[i];
    var y0=band.row_y[0]+s.d_top;
    var y1=rowY(band,rows)+s.d_bottom;
    out.push({x:s.rect.x,y:y0,w:s.rect.w,h:y1-y0,fill:s.rect.fill,id:s.rect.id});
  }
  return out;
}
function rowGeometry(band,i){
  if(i<band.capacity){return band.rows[i];}
  var template=band.rows[band.template_row];
  var delta=rowY(band,i)-band.row_y[band.template_row];
  return {index:i,y:rowY(band,i),
    cells:template.cells.map(function(c){
      var copy=JSON.parse(JSON.stringify(c));
      copy.id=band.id+"-r"+i+"-c"+c.col;
      copy.y=c.y+delta;
      return copy;}),
    texts:template.texts.map(function(t,j){
      var copy=JSON.parse(JSON.stringify(t));
      copy.id=band.id+"-r"+i+"-t"+j;
      copy.baseline_y=t.baseline_y+delta;
      /* the enumerated column carries the row's own ordinal, not the
         template row's */
      if(t.role==="enumerated"){copy.text=String(i+1);}
      return copy;})};
}
/* One row's cells: clone the <template> for the row's shape, then overlay the
   measured geometry for this particular row. The template says which cells a
   row has, which are fields and how many comb slots each carries; the blob
   says where this row's edges actually are, which is not the template's
   position plus i*pitch. */
function rowCells(band,row){
  var tpl=document.getElementById("band-template-"+band.id);
  var nodes=null;
  if(tpl&&tpl.content){
    nodes=tpl.content.cloneNode(true).querySelectorAll("[data-cell-kind]");
  }
  if(!nodes||nodes.length!==row.cells.length){
    return row.cells.map(function(cell){return cellElement(cell);});
  }
  var out=[],i;
  for(i=0;i<row.cells.length;i++){out.push(applyCell(nodes[i],row.cells[i]));}
  return out;
}
/* A row added at run time has to be as fillable as a pre-rendered one, so the
   field layer is rebuilt with the row's own measurements rather than inherited
   from the template: a band row's comb sub-band is measured per row and the
   template's fitted size is not automatically this row's. */
function fieldMetrics(el,field){
  el.style.fontSize=field.size_pt+"pt";
  el.style.lineHeight=field.line_height_pt+"pt";
  if(field.letter_spacing_pt!==null&&field.letter_spacing_pt!==undefined){
    el.style.letterSpacing=field.letter_spacing_pt+"pt";
  }else{
    el.style.letterSpacing="";
  }
  if(field.inset_trbl){
    el.style.inset=field.inset_trbl[0]+"pt "+field.inset_trbl[1]+"pt "+
      field.inset_trbl[2]+"pt "+field.inset_trbl[3]+"pt";
  }
}
function fieldName(cell,slotIndex){
  return cell.id+(slotIndex===null?"-i":"-s"+slotIndex);
}
function fieldInput(cell,slotIndex){
  var el=document.createElement("input");
  el.type="text";
  el.className="fi "+cell.field["class"]+(slotIndex===null?"":" fc");
  el.id=fieldName(cell,slotIndex);
  el.name=cell.id;
  el.setAttribute("autocomplete","off");
  el.setAttribute("spellcheck","false");
  if(slotIndex!==null){
    el.setAttribute("data-slot-index",slotIndex);
    el.setAttribute("maxlength","1");
  }
  fieldMetrics(el,cell.field);
  return el;
}
/* The clone carries the template's inputs, which are deliberately anonymous:
   an identity in a <template> would be a second element with the row's name. */
function applyFields(el,cell){
  if(!cell.field){return;}
  var inputs=el.querySelectorAll("input.fi"),i,slot;
  for(i=0;i<inputs.length;i++){
    slot=inputs[i].getAttribute("data-slot-index");
    inputs[i].id=fieldName(cell,slot===null?null:slot);
    inputs[i].name=cell.id;
    inputs[i].value="";
    fieldMetrics(inputs[i],cell.field);
  }
}
function applyCell(el,cell){
  el.removeAttribute("data-cell-id");
  el.id=cell.id;
  el.setAttribute("data-row",cell.row);
  el.setAttribute("data-col",cell.col);
  if(cell.field){el.setAttribute("data-field-name",cell.id);}
  el.style.cssText="left:"+cell.x+"pt;top:"+cell.y+"pt;width:"+cell.w+
    "pt;height:"+cell.h+"pt";
  var slots=el.querySelectorAll(".s");
  if(cell.comb&&slots.length===cell.comb.slot_x.length-1){
    for(var i=0;i<slots.length;i++){
      slots[i].style.cssText="left:"+cell.comb.slot_x[i]+"pt;top:"+cell.comb.y+
        "pt;width:"+(cell.comb.slot_x[i+1]-cell.comb.slot_x[i])+
        "pt;height:"+cell.comb.h+"pt";
    }
  }else if(cell.comb){
    /* the row's comb has a different slot count than the template's: rebuild
       rather than reposition, and never invent slots the layout did not
       measure */
    return cellElement(cell);
  }
  applyFields(el,cell);
  return el;
}
function cellElement(cell){
  var el=document.createElement("div");
  el.id=cell.id;
  /* `.f` follows the typing surface, not the box detector's kind: a comb cell
     that also holds a pre-printed decimal point is a field. Mirrors
     cell_markup(). */
  el.className=cell.field?"c f":"c";
  el.setAttribute("data-cell-kind",cell.kind);
  el.setAttribute("data-row",cell.row);
  el.setAttribute("data-col",cell.col);
  el.style.cssText="left:"+cell.x+"pt;top:"+cell.y+"pt;width:"+cell.w+
    "pt;height:"+cell.h+"pt";
  if(cell.field){
    el.setAttribute("data-field-kind",cell.field.kind);
    el.setAttribute("data-field-name",cell.id);
    if(cell.field.capacity!==null&&cell.field.capacity!==undefined){
      el.setAttribute("data-comb-capacity",cell.field.capacity);
    }
  }
  if(cell.comb){
    el.setAttribute("data-comb-slots",cell.comb.cells);
    el.setAttribute("data-comb-pitch",cell.comb.pitch_pt);
    for(var i=0;i<cell.comb.slot_x.length-1;i++){
      var slot=document.createElement("div");
      slot.className="s";
      slot.setAttribute("data-slot",i);
      slot.style.cssText="left:"+cell.comb.slot_x[i]+"pt;top:"+cell.comb.y+
        "pt;width:"+(cell.comb.slot_x[i+1]-cell.comb.slot_x[i])+
        "pt;height:"+cell.comb.h+"pt";
      if(cell.field){slot.appendChild(fieldInput(cell,i));}
      el.appendChild(slot);
    }
  }else if(cell.field){
    el.appendChild(fieldInput(cell,null));
  }
  return el;
}
/* Blink floors a block's top to the device grid and floors the baseline
   inside it, so `top` alone cannot express a sub-0.75pt baseline. Place the
   box on the grid, where that flooring is a no-op, and carry the remainder in
   a transform, which layout does not snap. Mirrors _vertical_placement(). */
var DEVICE_PX_PT=0.75;
function placeBaseline(baselineY,offsetPt){
  var offsetPx=offsetPt/DEVICE_PX_PT;
  var topPx=Math.floor((baselineY-offsetPt)/DEVICE_PX_PT);
  var paintedPx=topPx+Math.floor(offsetPx);
  return {top:topPx*DEVICE_PX_PT,ty:baselineY-paintedPx*DEVICE_PX_PT};
}
function textElement(text){
  var el=document.createElement("div");
  var place=placeBaseline(text.baseline_y,text.baseline_offset_pt);
  var scaled=text.scale_x!==null&&text.scale_x!==undefined;
  var ops=[];
  el.className="t";
  el.id=text.id;
  /* a scaled box snaps x as well, so it carries both axes in the transform */
  el.style.cssText=text.style+";left:"+(scaled?"0":text.x+"pt")+
    ";top:"+place.top+"pt";
  if(scaled){
    ops.push("translate("+text.x+"pt,"+place.ty+"pt)");
    ops.push("scaleX("+text.scale_x+")");
  }else if(Math.abs(place.ty)>1e-9){
    ops.push("translateY("+place.ty+"pt)");
  }
  if(ops.length){
    el.style.transform=ops.join(" ");
    el.style.transformOrigin="0 "+text.baseline_offset_pt+"pt";
  }
  el.textContent=text.text;
  return el;
}
/* Render `rows` rows of `bandId`. Rows beyond the sheet's official capacity
   are not silently overrun: the sheet holds `capacity`, the remainder belongs
   on a continuation page, so the overflow is reported rather than drawn. */
function setBandRows(bandId,rows,options){
  var band=byId[bandId];
  if(!band){throw new Error("no such band: "+bandId);}
  options=options||{};
  var drawn=options.allowOverflow?rows:Math.min(rows,band.capacity);
  var overflow=Math.max(0,rows-drawn);
  var rules=document.getElementById("band-rules-"+bandId);
  var content=document.getElementById("band-content-"+bandId);
  while(rules.firstChild){rules.removeChild(rules.firstChild);}
  while(content.firstChild){content.removeChild(content.firstChild);}
  bandRects(band,drawn).forEach(function(rect){paint(rules,rect);});
  for(var i=0;i<drawn;i++){
    var row=rowGeometry(band,i);
    rowCells(band,row).forEach(function(el){content.appendChild(el);});
    row.texts.forEach(function(text){content.appendChild(textElement(text));});
  }
  content.setAttribute("data-rendered-rows",drawn);
  content.setAttribute("data-overflow-rows",overflow);
  band.rendered_rows=drawn;
  return {rendered:drawn,overflow:overflow,capacity:band.capacity};
}
window.formgen={bands:bands,setBandRows:setBandRows,bandRects:bandRects,rowY:rowY};
})();"""


# A comb is one field drawn with N tick marks, so it has to *behave* as one:
# typing runs through it, backspace runs back through it, and a pasted TIN
# lands in it whole. Every listener is delegated from the document rather than
# bound per input, which is what makes a band row added by setBandRows fillable
# with no re-binding step -- and there are 488 comb inputs on 2551Q page 1
# alone, so 488 listener registrations is not a neutral alternative.
FIELD_JS = r"""(function(){
"use strict";
function isSlot(el){
  return !!(el&&el.classList&&el.classList.contains("fi")&&
            el.hasAttribute("data-slot-index"));
}
function slotsOf(el){
  var cell=el.closest("[data-cell-kind]");
  return cell?cell.querySelectorAll("input.fi[data-slot-index]"):null;
}
function move(el,delta,select){
  var list=slotsOf(el);
  if(!list){return null;}
  var index=parseInt(el.getAttribute("data-slot-index"),10)+delta;
  if(index<0||index>=list.length){return null;}
  list[index].focus();
  if(select&&list[index].select){list[index].select();}
  return list[index];
}
/* Selecting on focus is what lets a filled slot be typed over: maxlength=1
   rejects a second character outright, so without it a comb can be corrected
   only by deleting first. */
document.addEventListener("focusin",function(ev){
  if(isSlot(ev.target)&&ev.target.select){ev.target.select();}
});
document.addEventListener("input",function(ev){
  if(!isSlot(ev.target)){return;}
  if(ev.target.value.length>0){move(ev.target,1,true);}
});
document.addEventListener("keydown",function(ev){
  var el=ev.target;
  if(!isSlot(el)){return;}
  if(ev.key==="Backspace"&&el.value===""){
    var previous=move(el,-1,false);
    if(previous){previous.value="";ev.preventDefault();}
  }else if(ev.key==="ArrowLeft"&&el.selectionStart===0){
    if(move(el,-1,true)){ev.preventDefault();}
  }else if(ev.key==="ArrowRight"&&el.selectionEnd===el.value.length){
    if(move(el,1,true)){ev.preventDefault();}
  }
});
/* maxlength=1 truncates a paste to its first character, which for a TIN is
   the difference between one keystroke and nine. Spread it instead. */
document.addEventListener("paste",function(ev){
  var el=ev.target;
  if(!isSlot(el)){return;}
  var clipboard=ev.clipboardData||window.clipboardData;
  var list=slotsOf(el);
  if(!clipboard||!list){return;}
  var text=(clipboard.getData("text")||"").replace(/\s+/g,"");
  if(!text){return;}
  ev.preventDefault();
  var index=parseInt(el.getAttribute("data-slot-index"),10),written=0,last=el;
  while(index<list.length&&written<text.length){
    list[index].value=text.charAt(written);
    last=list[index];
    index++;written++;
  }
  last.focus();
  if(last.select){last.select();}
});
/* A caret is browser chrome, and Chromium paints it into the printed page:
   printing with the cursor still in a comb produced a 0.75x6.75pt black bar
   which the round-trip read as an extra structural rule -- correctly, because
   on paper that is what it is. `caret-color:transparent` does not suppress it
   in the print path, so the focus itself is dropped for the duration. */
window.addEventListener("beforeprint",function(){
  var el=document.activeElement;
  if(el&&el.classList&&el.classList.contains("fi")){el.blur();}
});
window.formgenFields={slotsOf:slotsOf};
})();"""


class Options:
    __slots__ = ("rule_backend", "fonts_dir", "assets_dir", "out_dir", "band_rows",
                 "title", "guide_plan", "document", "guide_layout", "guide_href",
                 "form_href", "guide_pdf_dir", "guide_sources")

    def __init__(self, rule_backend: str, fonts_dir: str, assets_dir: str,
                 out_dir: pathlib.Path | None, band_rows: int | None,
                 title: str | None, guide_plan: dict[str, Any] | None = None,
                 document: str = "form", guide_layout: str = "reflow",
                 guide_href: str = "guide.html", form_href: str = "index.html",
                 guide_pdf_dir: str = "guides",
                 guide_sources: dict[str, dict[str, Any]] | None = None) -> None:
        self.rule_backend = rule_backend
        self.fonts_dir = fonts_dir
        self.assets_dir = assets_dir
        self.out_dir = out_dir
        self.band_rows = band_rows
        self.title = title
        self.guide_plan = guide_plan
        self.document = document
        self.guide_layout = guide_layout
        self.guide_href = guide_href
        self.form_href = form_href
        self.guide_pdf_dir = guide_pdf_dir
        # Keyed by PDF file name, which is what the guide plan lists. Only the
        # guide document reads it; the form side has no standalone PDFs.
        self.guide_sources = dict(guide_sources or {})


def _font_face_key(family: str, css_style: str, font_weight: str,
                   font_file: str) -> tuple[str, str, str, str]:
    return (family, css_style, font_weight, pathlib.PurePosixPath(font_file).name)


def _field_font_face(plan: dict[str, Any], fields: FieldPlan) -> dict[str, Any] | None:
    """Return the shipped face used by editable fields, if one exists.

    FieldFace deliberately keeps only the key and the CSS declarations needed
    by the input. The plan remains the authority for the file and @font-face
    descriptor, so a field cannot silently fall back to a platform face merely
    because no printed run on this document happened to use the modal face.
    """
    if not fields or fields.face is None:
        return None
    for face in plan.get("faces", ()):
        if face.get("face_key") == fields.face.face_key:
            if (face.get("status") == "resolved"
                    and face.get("css_family") and face.get("font_file")):
                return face
            return None
    return None


def _font_href(font_file: str, options: Options) -> str:
    name = pathlib.PurePosixPath(font_file).name
    return f"{options.fonts_dir.rstrip('/')}/{name}" if options.fonts_dir else name


def font_preload_hrefs(field_face: dict[str, Any] | None,
                       visible_font_files: set[str], options: Options) -> tuple[str, ...]:
    """Preload a field-only face so the isolated audit observes its request.

    An empty input does not make Chromium request its font during the print
    render. If that face is not also used by visible printed text, the CSS would
    otherwise retain a dependency that the request-closure proof correctly
    rejects. Preload only that missing file; visible text already loads the
    other case, and a second request would be noise rather than evidence.
    """
    if field_face is None or not field_face.get("font_file"):
        return ()
    name = pathlib.PurePosixPath(str(field_face["font_file"])).name
    if name in visible_font_files:
        return ()
    return (_font_href(str(field_face["font_file"]), options),)


def font_face_css(styles: dict[tuple[int, int], RunStyle], options: Options,
                  warnings: list[str],
                  extra_faces: Sequence[dict[str, Any]] = ()) -> str:
    """@font-face for exactly the faces the emitted runs actually reference.

    Keyed by weight as well as by family and style, and the weight *descriptor*
    comes from the plan rather than being a constant. A variable file legitimately
    covers `100 900` on its own; a static family keeps each weight in a separate
    file, and declaring one of those over the whole range makes Chromium serve
    that single file for every weight and synthesise the rest -- emboldened
    outlines with advances no measurement in the plan covers.
    """
    used: dict[tuple[str, str, str, str], str] = {}
    for style in styles.values():
        if style.font_family and style.font_file:
            key = _font_face_key(style.font_family, style.css_style,
                                 style.font_face_weight, style.font_file)
            used.setdefault(key, style.font_file)
    for face in extra_faces:
        family = str(face.get("css_family") or "")
        font_file = str(face.get("font_file") or "")
        if not family or not font_file:
            continue
        css_style = str(face.get("css_style") or "normal")
        font_weight = str((face.get("font_face") or {}).get("weight") or "100 900")
        used.setdefault(_font_face_key(family, css_style, font_weight, font_file),
                        font_file)
    blocks = []
    for (family, css_style, font_weight, _name), font_file in sorted(used.items()):
        name = pathlib.PurePosixPath(font_file).name
        href = _font_href(font_file, options)
        if options.out_dir is not None and not (options.out_dir / options.fonts_dir / name).is_file():
            warnings.append(
                f"missing font file {href}: the document references it but it is not "
                f"beside the output, so Chromium will fall back to a platform face and "
                f"every advance in the plan becomes a claim about the wrong font")
        blocks.append(
            f'@font-face{{font-family:"{family}";font-style:{css_style};'
            f'font-weight:{font_weight};font-display:block;'
            f'src:url("{href}") format("woff2")}}')
    return "\n".join(blocks)


def page_css(ir: dict[str, Any]) -> str:
    """@page from the PDF's own MediaBox, per page when they differ.

    Never Letter, never A4, never a constant: 2551Q is 612x936pt, 0619E is
    612x792 and others are 612x1008. A single hardcoded size would move every
    coordinate on 34 of the 35 forms.
    """
    paper = ir["paper"]
    lines = [f'@page{{size:{fmt(paper["width_pt"])}pt {fmt(paper["height_pt"])}pt;margin:0}}']
    if not paper.get("uniform", True):
        for page in ir["pages"]:
            lines.append(f'@page page-{page["index"]}{{size:{fmt(page["width_pt"])}pt '
                         f'{fmt(page["height_pt"])}pt;margin:0}}')
            lines.append(f'.page-{page["index"]}{{page:page-{page["index"]}}}')
    return "\n".join(lines)


def emit_page(page_ir: dict[str, Any], page_layout: dict[str, Any],
              styles: dict[tuple[int, int], RunStyle], backend: RuleBackend,
              options: Options, band_blobs: list[dict[str, Any]],
              warnings: list[str], split: PageSplit = WHOLE_PAGE,
              fields: FieldPlan | None = None,
              used_style_keys: set[tuple[int, int]] | None = None) -> str:
    index = int(page_ir["index"])
    cells_by_id = {c["id"]: c for c in page_layout["cells"]}
    runs = page_ir["text_runs"]
    runs_by_id = {run_id(index, i): run for i, run in enumerate(runs)}

    plans = [build_band_plan(band, page_ir, cells_by_id) for band in page_layout["growable"]]
    band_rule_ids = {rid for plan in plans for rid in plan.rule_ids}
    band_cell_ids = {cid for plan in plans for cid in plan.cell_ids}
    band_run_ids = {rid for plan in plans for rid in plan.run_ids}

    # A band is indivisible and always the form's, so its members leave the
    # guide's claim before anything is filtered, and the guide document drops
    # the bands themselves.
    split = split.without_band(band_rule_ids, band_cell_ids,
                               {_run_index_of(rid) for rid in band_run_ids})
    if split.guide_side:
        plans = []

    # -- rule layer, bottom: the source's own content-stream order ------------
    # Every rule, area fill and image carries the index of the op that painted
    # it, so the layer is emitted in that order. The previous fills ->
    # decorative -> structural bucket order was a guess about z-order, and it
    # was wrong wherever the source paints a *lighter* rect after a darker one:
    # 2552 draws the white knockout inside each checkbox at op 4774 and the
    # grey row separator crossing it at op 172, so bucketing put a light-grey
    # line through every checkbox on the sheet.
    painted: list[tuple[tuple[int, int, float, float, str], str, Any]] = []
    for fill_index, fill in enumerate(page_ir["area_fills"]):
        if split.keep_fill(fill_index):
            painted.append((paint_key(fill, ""), "rect",
                            Rect.from_box(split.clipped(fill, "area_fill",
                                                        f"#{fill_index}"))))
    for rule in page_ir["rules"]:
        if rule["id"] not in band_rule_ids and split.keep_rule(rule["id"]):
            clipped = split.clipped(rule, "rule", rule["id"])
            painted.append((paint_key(rule, rule["id"]), "rect",
                            Rect.from_box(clipped, rule["id"])))
    for image_index, image in enumerate(page_ir["images"]):
        if split.keep_image(image_index):
            painted.append((paint_key(image, image["sha256"]), "image", image))
    # Paths are the third kind of ink, and they paint in the same one order the
    # rules and fills do: 0605 draws its "write here" markers and its
    # pre-printed decimal points as filled paths interleaved with the boxes they
    # sit in, so bucketing them into a layer of their own would be the same
    # z-order guess the fills -> greys -> black bucketing was.
    for path in page_ir.get("paths", ()):
        if split.keep_path(path["id"]):
            painted.append((paint_key(path, str(path["id"])), "path", path))
    painted.sort(key=operator.itemgetter(0))

    parts = [f'<div class="page page-{index}" id="page-{index}" '
             f'style="width:{fmt(page_ir["width_pt"])}pt;'
             f'height:{fmt(page_ir["height_pt"])}pt">']

    parts.append(backend.open_page(page_ir))
    # Consecutive rects of the same role share one group, so the markup still
    # reads as labelled layers without the grouping dictating the paint order.
    run: list[Rect] = []
    run_role = ""
    for _key, kind, payload in painted:
        if kind == "rect" and (not run or payload.role == run_role):
            run_role = payload.role
            run.append(payload)
            continue
        parts.append(backend.rects(run, run_role))
        run = []
        if kind == "image":
            parts.append(image_markup(payload, backend, options, warnings))
        elif kind == "path":
            parts.append(backend.path(payload, page_ir))
        else:
            run_role = payload.role
            run = [payload]
    parts.append(backend.rects(run, run_role))
    for plan in plans:
        rows = plan.capacity if options.band_rows is None else min(options.band_rows,
                                                                   plan.capacity)
        # The rects are the container's direct children, matching what the JS
        # renderer produces, so a re-render is a like-for-like replacement.
        parts.append(backend.band_container(plan.band["id"], band_rects(plan, rows)))
    parts.append(backend.close_page())

    # -- text layer ----------------------------------------------------------
    parts.append('<div class="layer-text">')
    for run_index, run in enumerate(runs):
        rid = run_id(index, run_index)
        if rid in band_run_ids or not split.keep_run(run_index):
            continue
        if used_style_keys is not None:
            used_style_keys.add((index, run_index))
        parts.append(text_markup(run, rid, styles[(index, run_index)]))
    parts.append("</div>")

    # -- field layer ---------------------------------------------------------
    parts.append('<div class="layer-cells">')
    for cell in page_layout["cells"]:
        if cell["id"] in band_cell_ids or not split.keep_cell(cell["id"]):
            continue
        parts.append(cell_markup(split.clipped(cell, "cell", cell["id"]), fields))
    parts.append("</div>")

    # -- growable bands: template + pre-rendered rows ------------------------
    for plan in plans:
        rows = plan.capacity if options.band_rows is None else min(options.band_rows,
                                                                   plan.capacity)
        parts.append(band_template_markup(plan, len(band_blobs), fields))
        parts.append(f'<div class="band" id="band-content-{esc_attr(plan.band["id"])}" '
                     f'data-band="{esc_attr(plan.band["id"])}" '
                     f'data-rendered-rows="{rows}" data-overflow-rows="0" '
                     f'data-capacity="{plan.capacity}" '
                     f'data-row-pitch="{fmt(plan.band["row_pitch_pt"])}">')
        for row in range(rows):
            for cell in sorted(plan.cells_by_row.get(row, []), key=lambda c: (c["x0"], c["id"])):
                parts.append(cell_markup(cell, fields))
            for rid, _cell, _ in sorted(plan.texts_by_row.get(row, []), key=_run_order):
                key = (index, _run_index_of(rid))
                if used_style_keys is not None:
                    used_style_keys.add(key)
                parts.append(text_markup(runs_by_id[rid], rid, styles[key],
                                         extra=(("data-band-row", str(row)),)))
        parts.append("</div>")
        band_blobs.append(band_json(plan, rows, styles, runs_by_id, fields))

    parts.append("</div>")
    return "".join(parts)


def _form_side_inventory(page_ir: dict[str, Any], page_layout: dict[str, Any],
                         split: PageSplit) -> dict[str, int]:
    """How much of one page the form document still carries, per kind.

    Counted from the same predicates emit_page() uses, so "nothing is left"
    means the emitter would draw nothing rather than that a flag says so.
    """
    return {
        "rules": sum(1 for r in page_ir["rules"] if split.keep_rule(r["id"])),
        "area_fills": sum(1 for i in range(len(page_ir["area_fills"]))
                          if split.keep_fill(i)),
        "images": sum(1 for i in range(len(page_ir["images"])) if split.keep_image(i)),
        "paths": sum(1 for p in page_ir.get("paths", ())
                     if split.keep_path(str(p["id"]))),
        "text_runs": sum(1 for i in range(len(page_ir["text_runs"]))
                         if split.keep_run(i)),
        "cells": sum(1 for c in page_layout["cells"] if split.keep_cell(c["id"])),
        "growable": len(page_layout["growable"]),
    }


def doc_link_markup(href: str, label: str) -> str:
    return f'<a class="doc-link" href="{esc_attr(href)}">{esc_text(label)}</a>'


def whole_page_split(page: dict[str, Any]) -> PageSplit:
    """A page of a standalone guide PDF: all of it is guide material.

    guides.py computes which runs of a *form* sheet lie below the cut. A guide
    PDF has no form on it to cut away from, so the claim is the whole page, and
    the same reflow that fixed 1603Q's overlapping columns applies unchanged.
    """
    return PageSplit(guide_side=True,
                     run_indices=frozenset(range(len(page["text_runs"]))))


def guide_source_key(source_ir: dict[str, Any]) -> str:
    """The PDF file name a guide-source IR was extracted from.

    extract.py records `external:<name>` for any PDF outside the repo, which
    every source PDF is. The name is what binds an extraction back to the entry
    in the guide plan; nothing here matches on order, so passing the sources in
    a different order cannot silently pair the wrong text with the wrong link.
    """
    file_name = str(source_ir.get("source", {}).get("file", ""))
    return pathlib.PurePosixPath(file_name.split(":", 1)[-1]).name


def standalone_pdf_markup(split: DocumentSplit, options: Options,
                          warnings: list[str]) -> str:
    """The guide PDFs batch.py skips, reflowed after the inline guide pages.

    These used to be embedded with `<object>`. An embedded PDF is a second
    document with its own pagination: printing the page around it prints the
    plugin's viewport, not the four pages of instructions inside it, so the
    twelve bundles whose whole guide was a linked PDF printed as one near-blank
    sheet. Running the guide PDF through the same extractor and the same reflow
    the inline guides use makes it text in *this* document, which is the only
    form of it that prints, and makes all 29 guides print the same way.

    The conversion is a reading copy, not a replacement: the pinned PDF stays
    beside the HTML and is linked, so the exact artefact is always one click
    away and the printed sheet names it. A PDF with no extraction falls back to
    the embed, with a warning -- degraded rather than dropped.
    """
    if not split.standalone_pdfs:
        return ""
    directory = options.guide_pdf_dir.rstrip("/")
    parts: list[str] = []
    for source in split.standalone_pdfs:
        name = pathlib.PurePosixPath(source).name
        href = f"{directory}/{name}" if directory else name
        # BIR ships these with spaces in the file name ("1701Q Guide Jan 2018.pdf"),
        # which is a valid path and not a valid URL.
        url = urllib.parse.quote(href)
        if options.out_dir is not None and not (options.out_dir / href).is_file():
            warnings.append(
                f"missing guide PDF {href}: the guide document links it but it is not "
                f"beside the output, so the link 404s")
        source_ir = options.guide_sources.get(name)
        converted = source_ir is not None
        parts.append(f'<section class="gl-pdf" data-source="{esc_attr(name)}" '
                     f'data-converted="{"true" if converted else "false"}">')
        parts.append(f"<h2>{esc_text(name)}</h2>")
        parts.append(f'<p class="gl-download">Source PDF: '
                     f'<a href="{esc_attr(url)}">{esc_text(name)}</a></p>')
        if converted:
            for page in source_ir["pages"]:
                parts.append(reflow_page(page, whole_page_split(page)))
        else:
            warnings.append(
                f"no --guide-source for {name}: it is embedded as a linked PDF, which "
                f"does not print with this document")
            parts.append(f'<object data="{esc_attr(url)}" type="application/pdf" '
                         f'data-source="{esc_attr(name)}">'
                         f'<a href="{esc_attr(url)}">{esc_text(name)}</a></object>')
        parts.append("</section>")
    return "".join(parts)


def _head(ir: dict[str, Any], title: str, styles: str, backend_name: str | None,
          document: str, font_preloads: Sequence[str] = ()) -> list[str]:
    form = ir["form"]
    attributes = ['lang="en"', f'data-form="{esc_attr(form["code"])}"',
                  f'data-revision="{esc_attr(form["revision"])}"']
    if backend_name is not None:
        attributes.append(f'data-rule-backend="{backend_name}"')
    attributes.append(f'data-source-sha256="{esc_attr(ir["source"]["sha256"])}"')
    attributes.append(f'data-schema-version="{SCHEMA_VERSION}"')
    # Only the guide announces itself. The form document's <html> tag is left
    # exactly as it was so that a form with no guide is byte-identical to what
    # this module emitted before the split existed.
    if document != "form":
        attributes.append(f'data-document="{esc_attr(document)}"')
    preloads = [
        f'<link rel="preload" href="{esc_attr(href)}" as="font" '
        'type="font/woff2" crossorigin>'
        for href in sorted(set(font_preloads))
    ]
    return [
        "<!doctype html>",
        f"<html {' '.join(attributes)}>",
        "<head>",
        '<meta charset="utf-8">',
        f"<title>{esc_text(title)}</title>",
        "<!-- Generated by tools/formgen/emit.py from the pinned PDF's own content "
        "stream. Do not hand-edit: regenerate. -->",
        *preloads,
        "<style>",
        styles,
        "</style>",
        "</head>",
        "<body>",
    ]


def _validate_guide_sources(split: DocumentSplit, ir: dict[str, Any],
                            options: Options) -> None:
    """A guide-source IR must belong to this form and to a PDF the plan lists.

    Both directions are fatal. An extraction of the wrong PDF would print
    another form's instructions under this form's heading, and a source that
    matches no entry in the plan means the driver and the plan disagree about
    what this bundle's guide is made of. Either way the document would be
    quietly wrong, which is the one outcome this pipeline exists to make
    impossible.
    """
    if not options.guide_sources:
        return
    listed = {pathlib.PurePosixPath(path).name for path in split.standalone_pdfs}
    unknown = sorted(set(options.guide_sources) - listed)
    if unknown:
        raise SystemExit(
            f"--guide-source names {unknown}, which the guide plan does not list as a "
            f"standalone PDF of {ir['form']['code']}-{ir['form']['revision']}")
    for name, source_ir in sorted(options.guide_sources.items()):
        source_form = source_ir.get("form", {})
        if (source_form.get("code"), source_form.get("revision")) != (
                ir["form"]["code"], ir["form"]["revision"]):
            raise SystemExit(
                f"guide source {name} was extracted as "
                f"{source_form.get('code')}-{source_form.get('revision')}, not as "
                f"{ir['form']['code']}-{ir['form']['revision']}")


def build_reflow_guide(ir: dict[str, Any], layout: dict[str, Any],
                       split: DocumentSplit, options: Options,
                       warnings: list[str]) -> tuple[str, list[str]]:
    """The guide as a readable document: no coordinates, therefore no overlap."""
    form = ir["form"]
    title = options.title or (f"BIR Form {form['code']} ({form['revision']}) "
                              f"-- Guidelines and Instructions")
    by_index = {int(page["index"]): page for page in ir["pages"]}
    # The lattice is what tells a relocated *table* from relocated prose, so the
    # guide reads the same layout the form does.
    layout_by_index = {int(page["index"]): page for page in layout["pages"]}
    body = [f'<div class="gl">',
            doc_link_markup(options.form_href, "← Back to the form"),
            f"<h1>{esc_text(title)}</h1>",
            f'<p class="gl-sub">Reference material lifted off the sheet by '
            f'tools/formgen/guides.py. Re-typeset for reading: the text is the '
            f"source's, the line breaks are not.</p>"]
    for index in split.guide_pages:
        page = by_index[index]
        images = [i for i in range(len(page["images"]))
                  if i in split.page(index).image_indices]
        if images:
            warnings.append(
                f"page {index}: the guide region claims {len(images)} image(s); the "
                f"reflowed guide drops them. Use --guide-layout absolute to keep them.")
        body.append(reflow_page(page, split.page(index), layout_by_index.get(index)))
    body.append(standalone_pdf_markup(split, options, warnings))
    body.append("</div>")

    # guide_page_css leads for the same reason page_css does in the form: the
    # sheet the document is printed on is the first thing about it.
    head = _head(ir, title,
                 "\n".join([guide_page_css(ir), BASE_CSS.rstrip(), DOC_LINK_CSS,
                            GUIDE_CSS, GUIDE_PDF_CSS]),
                 None, "guide")
    return "\n".join(head) + "\n" + "".join(body) + "\n</body>\n</html>\n", warnings


def build_document(ir: dict[str, Any], layout: dict[str, Any], plan: dict[str, Any],
                   options: Options) -> tuple[str, list[str]]:
    warnings: list[str] = []
    if ir["source"]["sha256"] != layout["source"]["sha256"]:
        raise SystemExit("layout was built from a different PDF than the IR")
    if ir["source"]["sha256"] != plan["source"]["ir_sha256_of_pdf"]:
        raise SystemExit("font plan was built from a different PDF than the IR")

    split = DocumentSplit(options.guide_plan, ir, options.document)
    _validate_guide_sources(split, ir, options)
    if options.document == "guide" and options.guide_layout == "reflow":
        return build_reflow_guide(ir, layout, split, options, warnings)

    backend = BACKENDS[options.rule_backend]()
    styles = resolve_run_styles(ir, plan, warnings)
    # Built from the whole layout, never from the half being emitted, so a
    # cell's typing surface is the same markup whichever document carries it --
    # which is what lets the split assertions compare the two halves against the
    # undivided document byte for byte.
    fields = FieldPlan(layout, resolve_field_face(plan, warnings), warnings, ir)

    # The form keeps every page at its full height even where the guide took the
    # lower 70% of one: the page box, the page count and @page are the form's
    # geometry, and the freed space is what a growable band expands into. The
    # guide document carries only the pages it actually has content on.
    #
    # A page the guide took *entirely* is the one exception, and it is not the
    # same case: keeping it prints a blank sheet stapled into the middle of the
    # form (2200P p3, 2550M p3 and p4, 0605 p2, 1702Q p3, 2553 p2 all did). There
    # is no geometry left on it to preserve and no band that could expand into
    # it, so the page leaves the form document. The reference the round-trip is
    # scored against has to lose it too, or a correctly dropped sheet reads as a
    # page-count mismatch; the emptiness is asserted here rather than trusted so
    # that a page with anything left on it stays, whatever the plan says.
    wanted = (set(split.guide_pages) if options.document == "guide"
              else {int(page["index"]) for page in ir["pages"]})
    if options.document == "form":
        for page_ir, page_layout in zip(ir["pages"], layout["pages"]):
            index = int(page_ir["index"])
            remains = _form_side_inventory(page_ir, page_layout, split.page(index))
            if index in split.guide_pages and not any(remains.values()):
                wanted.discard(index)
                warnings.append(
                    f"page {index}: every element is guide material, so the form "
                    f"document drops the page rather than printing a blank sheet. "
                    f"Score the form against a reference with this page removed.")

    band_blobs: list[dict[str, Any]] = []
    used_style_keys: set[tuple[int, int]] = set()
    pages = [emit_page(page_ir, page_layout, styles, backend, options, band_blobs,
                       warnings, split.page(int(page_ir["index"])), fields,
                       used_style_keys)
             for page_ir, page_layout in zip(ir["pages"], layout["pages"])
             if int(page_ir["index"]) in wanted]

    form = ir["form"]
    if options.document == "guide":
        title = options.title or (f"BIR Form {form['code']} ({form['revision']}) "
                                  f"-- Guidelines and Instructions")
        link = doc_link_markup(options.form_href, "← Back to the form")
    else:
        title = options.title or f"BIR Form {form['code']} ({form['revision']})"
        link = (doc_link_markup(options.guide_href, "Guidelines and Instructions →")
                if split.has_guide else "")

    field_face = _field_font_face(plan, fields)
    visible_font_files = {
        pathlib.PurePosixPath(styles[key].font_file).name
        for key in used_style_keys
        if styles[key].font_file
    }
    field_preloads = font_preload_hrefs(
        field_face, visible_font_files, options)
    styles_css = [
        page_css(ir),
        font_face_css(
            {key: styles[key] for key in sorted(used_style_keys)},
            options, warnings,
            extra_faces=([field_face] if field_face is not None else [])),
        BASE_CSS.rstrip(),
    ]
    field_style = field_css(fields)
    if field_style:
        styles_css.append(field_style)
    if link:
        styles_css.append(DOC_LINK_CSS)
    if split.guide_side and split.standalone_pdfs:
        styles_css.append(GUIDE_PDF_CSS)
    head = _head(ir, title, "\n".join(styles_css), backend.name,
                 options.document if options.document != "form" else "form",
                 field_preloads)
    if link:
        head.append(link)
    tail = [
        '<script type="application/json" id="formgen-bands">',
        json.dumps(band_blobs, ensure_ascii=False, separators=(",", ":")),
        "</script>",
        "<script>",
        BAND_JS,
        "</script>",
        "<script>",
        FIELD_JS,
        "</script>",
        "</body>",
        "</html>",
        "",
    ]
    body = "\n".join(pages)
    trailer = standalone_pdf_markup(split, options, warnings) if split.guide_side else ""
    if trailer:
        body = f"{body}\n{trailer}" if body else trailer
    return "\n".join(head) + "\n" + body + "\n" + "\n".join(tail), warnings


# ---------------------------------------------------------------------------
# Self-test
# ---------------------------------------------------------------------------


DEFAULT_IR = _ROOT / "build/ir/2551q-2018.ir.json"
DEFAULT_LAYOUT = _ROOT / "build/layout/2551q-2018.layout.json"
DEFAULT_PLAN = _ROOT / "build/fonts/2551q-2018.fontplan.json"
DEFAULT_GUIDE_PLAN = _ROOT / "build/guides/2551q-2018.guide.json"


def _check(ok: bool, label: str, detail: str, failures: list[str]) -> None:
    print(f"  [{'PASS' if ok else 'FAIL'}] {label}: {detail}", file=sys.stderr)
    if not ok:
        failures.append(label)


PAGE_SPLIT_RE = re.compile(r'<div class="page page-(\d+)"')
RECT_RE = re.compile(r'<rect [^>]*?/>|<div class="r" style="[^"]*"[^>]*></div>')
RUN_RE = re.compile(r'<div class="t" id="p\d+t\d+"[^>]*>')
CELL_RE = re.compile(r'<div id="([^"]+)" class="c[^"]*"')


def _pages_of(html: str) -> dict[int, str]:
    """The emitted document sliced per page, for per-page inventory checks.

    The document's tail is cut off first. Without that the last page's slice
    swallows the band blob and the scripts, and two documents with a different
    *page count* then compare unequal on a page they both emitted identically --
    which is exactly the shape of a form that dropped a wholly relocated page.
    """
    body = html.split('<script type="application/json"', 1)[0]
    out: dict[int, str] = {}
    chunks = PAGE_SPLIT_RE.split(body)
    for index in range(1, len(chunks) - 1, 2):
        out[int(chunks[index])] = chunks[index + 1]
    return out


INPUT_RE = re.compile(r"<input\b[^>]*>")
INPUT_NAME_RE = re.compile(r'<input\b[^>]*\bname="([^"]+)"')
INPUT_ID_RE = re.compile(r'<input\b[^>]*\bid="([^"]+)"')


def constructed_assertions(ir: dict[str, Any], layout: dict[str, Any],
                           plan: dict[str, Any], guide_plan: dict[str, Any] | None,
                           failures: list[str]) -> None:
    """The cases 2551Q does not contain, built rather than skipped.

    A check that cannot be evaluated is a failure and not a pass -- that is the
    failure mode this project has already been burned by, an audit that scored
    137 real defects as clean because it only compared what it knew to compare.
    2551Q has no non-rectilinear path, no flipped image, no cell filled with
    statutory text, no page the guide took whole and no straddler, so every one
    of those is staged on its own data here. Nothing is keyed on a form code:
    what is asserted is the *behaviour*, on input shaped like the corpus's.
    """
    # -- paths: geometry, both paints, the fill rule, and the paint order ------
    triangle = {
        "id": "path0", "x0": 32.88, "y0": 27.6, "x1": 36.96, "y1": 31.44,
        "fill": [0.0, 0.0, 0.0], "fill_gray": 0.0, "stroke": None,
        "stroke_gray": None, "stroke_width_pt": 0.0, "even_odd": False,
        "role": "structural", "paint_seq": 0, "paint_seq_max": 0,
        "subpaths": [{"start": [32.88, 27.6], "closed": True, "ops": [
            {"op": "l", "points": [36.96, 29.52]},
            {"op": "l", "points": [32.88, 31.44]},
            {"op": "l", "points": [32.88, 27.6]}]}],
    }
    dot = dict(triangle, id="path1", even_odd=True, stroke=[0.2, 0.2, 0.2],
               stroke_gray=0.2, stroke_width_pt=0.24, paint_seq=1, paint_seq_max=2,
               subpaths=[{"start": [10.0, 20.0], "closed": False, "ops": [
                             {"op": "c", "points": [10.9, 20.0, 11.7, 20.7, 11.7, 21.5]}]},
                         {"start": [1.0, 2.0], "closed": True, "ops": [
                             {"op": "re", "points": [1.0, 2.0, 3.0, 4.5]}]}])
    _check(path_data(triangle) == "M32.88 27.6L36.96 29.52L32.88 31.44L32.88 27.6Z",
           "a filled triangle is one closed path on the source's own points",
           path_data(triangle), failures)
    _check(path_data(dot) == ("M10 20C10.9 20 11.7 20.7 11.7 21.5"
                              "M1 2L3 2L3 4.5L1 4.5Z"),
           "a curve keeps its control points and a `re` subpath its four corners",
           path_data(dot), failures)
    _check(path_paints(triangle) == ("#000000", None)
           and path_paints(dot) == ("#000000", "#333333"),
           "a path carries its fill and its outline separately",
           f"{path_paints(triangle)} / {path_paints(dot)}", failures)
    _check('fill-rule="evenodd"' in path_svg(dot)
           and 'fill-rule' not in path_svg(triangle)
           and 'stroke-width="0.24"' in path_svg(dot),
           "even-odd and the stroke width are stated only where the source states them",
           path_svg(dot), failures)

    staged_ir = json.loads(json.dumps(ir))
    page = staged_ir["pages"][0]
    page["paths"] = [triangle, dot]
    # A rule painted after the paths must still be emitted after them.
    late = max(int(r["paint_seq"]) for r in page["rules"])
    triangle["paint_seq"] = triangle["paint_seq_max"] = late + 1
    for backend_name in sorted(BACKENDS):
        html, _ = build_document(staged_ir, layout, plan,
                                 Options(backend_name, "fonts", "assets", None, None, None))
        drawn = re.findall(r'data-(?:rule|path)-id="([^"]+)"', html)
        _check(drawn.count("path0") == 1 and drawn.count("path1") == 1,
               f"{backend_name} every path is emitted exactly once",
               f"{drawn.count('path0')}/{drawn.count('path1')}", failures)
        _check("path1" in drawn and drawn.index("path1") < drawn.index("path0"),
               f"{backend_name} paths paint in the source's content-stream order",
               f"path1 at {drawn.index('path1')}, path0 at {drawn.index('path0')}",
               failures)

    # -- image placement: the matrix is carried, and only when it fits the box --
    flipped = {"sha256": "f" * 64, "ext": "png", "x0": 30.48, "y0": 37.92,
               "x1": 71.28, "y1": 74.64, "paint_seq": 0, "paint_seq_max": 0,
               "transform": [40.8, 0.0, -0.0, -36.72, 30.48, 74.64]}
    _check(placement_matrix(flipped) == [40.8, 0.0, -0.0, -36.72, 30.48, 74.64],
           "a vertical flip is carried as the source's own matrix",
           str(placement_matrix(flipped)), failures)
    _check(placement_matrix(dict(flipped, transform=None)) is None
           and placement_matrix(dict(flipped, x1=99.0)) is None,
           "a missing or box-contradicting matrix falls back to the box",
           "None for both", failures)
    for backend_name, needle in (("svg", 'transform="matrix(40.8,0,0,-36.72,30.48,74.64)"'),
                                 ("css", "matrix(40.8,0,0,-36.72,40.64,99.52)")):
        staged = json.loads(json.dumps(ir))
        staged["pages"][0]["images"] = [flipped]
        html, _ = build_document(staged, layout, plan,
                                 Options(backend_name, "fonts", "assets", None, None, None))
        _check(needle in html, f"{backend_name} emits the flip",
               needle, failures)

    # -- C6 part 2: pre-printed text over a blank makes it uneditable ----------
    staged_ir = json.loads(json.dumps(ir))
    staged_layout = json.loads(json.dumps(layout))
    victim = next((c for p in staged_layout["pages"] for c in p["cells"]
                   if c["kind"] == "field" and not c.get("comb")
                   and c["x1"] - c["x0"] > 20.0), None)
    if victim is None:
        _check(False, "C6 part 2 has a plain field cell to cover", "none found", failures)
    else:
        page_index = int(victim["id"][1:].split("c")[0])
        page = next(p for p in staged_ir["pages"] if int(p["index"]) == page_index)
        width = victim["x1"] - victim["x0"]
        text = "Not over P 250,000"
        each = width / len(text)
        page["text_runs"].append({
            "text": text, "font": "Arial", "family": "Arial", "size_pt": 7.0,
            "bold": False, "italic": False, "serif": False, "monospace": False,
            "superscript": False, "flags": 0, "color": 0,
            "x0": victim["x0"], "y0": victim["y0"], "x1": victim["x1"],
            "y1": victim["y1"], "baseline_y": victim["y1"], "origin_x": victim["x0"],
            "ascender": 0.9, "descender": -0.2, "line_height_pt": 8.0,
            "measured_advance_pt": width,
            "char_origin_offsets_pt": [each * i for i in range(len(text))],
            "char_advances_pt": [each] * len(text),
            "char_widths_pt": [each] * len(text),
            "direction": [1.0, 0.0], "rotated": False, "unmapped_glyphs": [],
        })
        ink = PrePrintedInk(page["text_runs"])
        _check(not field_verdict(victim, ink)[0]
               and field_verdict(dict(victim, comb={"cells": 3}), ink)[0],
               "pre-printed text blocks a plain field but never a comb",
               f"coverage {ink.coverage(victim):.2f}", failures)
        html, _ = build_document(staged_ir, staged_layout, plan,
                                 Options("svg", "fonts", "assets", None, None, None))
        cell = re.search(rf'<div id="{victim["id"]}"[^>]*>(<input[^>]*>)?', html)
        _check(cell is not None and cell.group(1) is None
               and 'data-preprinted="true"' in cell.group(0),
               "a cell filled with pre-printed text is emitted without an input",
               cell.group(0)[:80] if cell else "cell absent", failures)

    # -- Ink: a padded single-word run is emitted as its ink, at its own origin -
    padded = {"text": " No ", "origin_x": 231.17,
              "char_origin_offsets_pt": [0.0, 9.36, 15.84, 20.88],
              "char_widths_pt": [2.5, 6.5, 5.0, 2.5], "measured_advance_pt": 23.38}
    ink = run_ink(padded, None)
    _check(ink.trimmed and ink.text == "No" and abs(ink.origin_x - 240.53) < 1e-9,
           "a padded run is anchored on its first visible glyph",
           f"{ink.text!r} at {fmt(ink.origin_x)}", failures)
    # The source put 6.86pt of extra advance in the leading space and none
    # between N and o; the plan's uniform model gave that gap 2.2914pt of it,
    # which is what made the round-trip read back ' N o  ' and stop matching.
    _check(abs(ink.letter_spacing_pt or 0.0) <= 0.05,
           "the padding's advance does not become tracking between the letters",
           f"{ink.letter_spacing_pt}pt per gap", failures)
    multi = dict(padded, text=" No Yes ")
    _check(not run_ink(multi, None).trimmed,
           "a run whose ink contains a word gap keeps its padding",
           run_ink(multi, None).text, failures)

    # -- C8: the reflowed guide carries a run's colour ------------------------
    white = {"text": "MMC", "color": 16777215, "x0": 0.0, "x1": 10.0,
             "size_pt": 6.0, "baseline_y": 10.0}
    black = dict(white, text="ATC", color=0, x0=-20.0, x1=-10.0)
    _check(_render_pieces(_line_pieces([black, white]))
           == 'ATC <span style="color:#ffffff">MMC</span>',
           "a white run stays white in the guide and a black one carries no span",
           _render_pieces(_line_pieces([black, white])), failures)

    if guide_plan is None:
        return

    # -- S3: a page whose every element is guide material leaves the form ------
    staged_plan = json.loads(json.dumps(guide_plan))
    staged_layout = json.loads(json.dumps(layout))
    last = ir["pages"][-1]
    # A page whose every element is guide material has no growable band left on
    # it either -- all six such pages in the corpus have none -- and a band is
    # content, so the emitter must keep a page that still has one.
    staged_layout["pages"][-1]["growable"] = []
    staged_plan["inline"] = [{
        "page": int(last["index"]), "cut_y_pt": 0.0, "whole_page": True,
        "rule_ids": [r["id"] for r in last["rules"]],
        "cell_ids": [c["id"] for c in layout["pages"][-1]["cells"]],
        "text_run_indices": list(range(len(last["text_runs"]))),
        "area_fill_indices": list(range(len(last["area_fills"]))),
        "image_indices": list(range(len(last["images"]))),
        "straddlers": [],
    }]
    html, warnings = build_document(ir, staged_layout, plan, Options(
        "svg", "fonts", "assets", None, None, None, staged_plan, "form"))
    _check(f'id="page-{last["index"]}"' not in html
           and any("blank sheet" in w for w in warnings),
           "a wholly relocated page is dropped from the form, not printed blank",
           f"{len(_pages_of(html))} pages emitted", failures)

    # The control: emptiness is asserted from the emitter's own predicates, not
    # taken from the plan's flag, so one element left behind keeps the page.
    if last["rules"]:
        held = json.loads(json.dumps(staged_plan))
        held["inline"][0]["rule_ids"] = held["inline"][0]["rule_ids"][1:]
        kept, _ = build_document(ir, staged_layout, plan, Options(
            "svg", "fonts", "assets", None, None, None, held, "form"))
        _check(f'id="page-{last["index"]}"' in kept,
               "a page with one element left on it is not dropped",
               f"{len(_pages_of(kept))} pages emitted", failures)
    if layout["pages"][-1]["growable"]:
        kept, _ = build_document(ir, layout, plan, Options(
            "svg", "fonts", "assets", None, None, None, staged_plan, "form"))
        _check(f'id="page-{last["index"]}"' in kept,
               "a page that still carries a growable band is not dropped",
               f"{len(_pages_of(kept))} pages emitted", failures)

    # -- straddlers: the form draws its own clipped piece --------------------
    first_page = ir["pages"][0]
    rule = next((r for r in first_page["rules"] if r["y1"] - r["y0"] > 20.0), None)
    if rule is None:
        _check(False, "a straddling rule can be staged", "no tall rule", failures)
        return
    cut = (float(rule["y0"]) + float(rule["y1"])) / 2.0
    staged_plan = json.loads(json.dumps(guide_plan))
    staged_plan["inline"] = [{
        "page": int(first_page["index"]), "cut_y_pt": cut, "whole_page": False,
        "rule_ids": [], "cell_ids": [], "text_run_indices": [],
        "area_fill_indices": [], "image_indices": [],
        "straddlers": [{
            "kind": "rule", "ref": rule["id"], "x0": rule["x0"], "y0": rule["y0"],
            "x1": rule["x1"], "y1": rule["y1"], "detail": "", "disposition": "clipped",
            "form": {"x0": rule["x0"], "y0": rule["y0"], "x1": rule["x1"], "y1": cut},
            "guide": {"x0": rule["x0"], "y0": cut, "x1": rule["x1"], "y1": rule["y1"]},
        }],
    }]
    form_html, _ = build_document(ir, layout, plan, Options(
        "svg", "fonts", "assets", None, None, None, staged_plan, "form"))
    guide_html, _ = build_document(ir, layout, plan, Options(
        "svg", "fonts", "assets", None, None, None, staged_plan, "guide", "absolute"))
    form_rect = re.search(rf'<rect x="[^"]*" y="([^"]*)" width="[^"]*" '
                          rf'height="([^"]*)"[^>]*data-rule-id="{rule["id"]}"', form_html)
    guide_rect = re.search(rf'<rect x="[^"]*" y="([^"]*)" width="[^"]*" '
                           rf'height="([^"]*)"[^>]*data-rule-id="{rule["id"]}"', guide_html)
    _check(form_rect is not None and guide_rect is not None,
           "a clipped straddler is drawn by both documents",
           f"form {bool(form_rect)}, guide {bool(guide_rect)}", failures)
    if form_rect and guide_rect:
        above = float(form_rect.group(1)), float(form_rect.group(2))
        below = float(guide_rect.group(1)), float(guide_rect.group(2))
        _check(abs(above[0] - float(rule["y0"])) < 0.005
               and abs(above[0] + above[1] - cut) < 0.005
               and abs(below[0] - cut) < 0.005
               and abs(below[0] + below[1] - float(rule["y1"])) < 0.005,
               "the two pieces meet at the cut and reconstruct the rule",
               f"{above} + {below} for {rule['y0']}..{rule['y1']} cut {fmt(cut)}",
               failures)


def field_assertions(ir: dict[str, Any], layout: dict[str, Any], plan: dict[str, Any],
                     html: str, rendered: str, failures: list[str]) -> None:
    """Every blank the source drew is typeable, exactly once, and prints clean.

    Counted against the *layout*, not against the markup, so an input the
    generator forgot fails as loudly as one it emitted twice. That matters more
    here than anywhere else in this module: a field that silently does not exist
    looks exactly like a form that prints correctly, which is precisely the
    state this layer was written to end.

    "Every blank" is `field_verdict`'s answer, not `kind == "field"`. The two
    disagree in both directions and each disagreement was a live defect: a
    comb-bearing `mixed` cell is a field the old test did not expect (2000-DST's
    entire money grid), and a `field` cell filled with statutory text is not one
    (1700 page 2's tax brackets).
    """
    fields = FieldPlan(layout, resolve_field_face(plan, []), [], ir)
    expected: dict[str, int] = {}
    for page in layout["pages"]:
        for cell in page["cells"]:
            box = fields.of(cell["id"])
            if box is not None:
                expected[cell["id"]] = box.capacity or 1

    ink = {int(p["index"]): PrePrintedInk(p["text_runs"]) for p in ir["pages"]}
    fillable = [c["id"] for p in layout["pages"] for c in p["cells"]
                if field_verdict(c, ink.get(int(p["index"])))[0]]
    _check(len(expected) == len(fillable) and set(expected) == set(fillable),
           "every fillable cell has a typing surface",
           f"{len(expected)} of {len(fillable)} fillable cells", failures)

    combs = {c["id"] for p in layout["pages"] for c in p["cells"] if c.get("comb")}
    _check(combs <= set(expected),
           "every comb is a field whatever text it also holds",
           f"{len(combs)} comb cells, {len(combs - set(expected))} without a surface",
           failures)

    names = INPUT_NAME_RE.findall(rendered)
    counts: dict[str, int] = {}
    for name in names:
        counts[name] = counts.get(name, 0) + 1
    _check(counts == expected,
           "one input per comb slot, one per plain field, and none anywhere else",
           f"{len(names)} inputs over {len(counts)} names; "
           f"{sum(expected.values())} expected over {len(expected)}", failures)

    identifiers = INPUT_ID_RE.findall(rendered)
    duplicates = sorted({i for i in identifiers if identifiers.count(i) > 1})
    _check(len(identifiers) == len(names) and not duplicates,
           "every input carries a unique id",
           f"{len(identifiers)} ids, {len(duplicates)} duplicated", failures)

    # A <template>'s inputs are a blueprint: an id or a name in there is a
    # second element answering to a row's identity.
    inert = "".join(re.findall(r"<template\b.*?</template>", html, flags=re.S))
    _check(not INPUT_ID_RE.search(inert) and not INPUT_NAME_RE.search(inert),
           "template inputs are anonymous",
           f"{len(INPUT_RE.findall(inert))} template input(s)", failures)

    used = {c for tag in INPUT_RE.findall(html)
            for c in re.findall(r'class="([^"]*)"', tag)[0].split()}
    declared = {"fi", "fc"} | set(fields.classes.values())
    _check(used <= declared and all(f".{name}{{" in html for name in used - {"fi", "fc"}),
           "every input's metrics class is declared",
           f"{len(used)} used, {len(declared)} declared", failures)

    # The affordances are :hover/:focus under @media screen, so an empty field
    # cannot print ink even if the guard is lost; the print reset is the second
    # line of defence, not the first.
    screen = re.search(r"@media screen\{(.*?)\}\n", html + "\n", flags=re.S)
    _check(screen is not None and ":hover" in screen.group(1)
           and ":focus" in screen.group(1),
           "field affordances are screen-only states",
           "hover/focus under @media screen" if screen else "no @media screen block",
           failures)
    _check("@media print{.fi{" in html,
           "the print stylesheet strips input chrome",
           "@media print resets border/outline/background", failures)


def split_assertions(ir: dict[str, Any], layout: dict[str, Any], plan: dict[str, Any],
                     guide_plan: dict[str, Any], failures: list[str]) -> None:
    """The split must be free: same geometry, redistributed, nothing lost.

    Every assertion here compares the *markup* of the two halves against the
    markup of the undivided document, not just counts. A rect that moved by a
    hundredth of a point would still count, and counting is what this pipeline
    has to be unable to be fooled by.
    """
    for backend_name in sorted(BACKENDS):
        base_options = Options(backend_name, "fonts", "assets", None, None, None)
        whole, _ = build_document(ir, layout, plan, base_options)
        form_html, _ = build_document(ir, layout, plan, Options(
            backend_name, "fonts", "assets", None, None, None, guide_plan, "form"))
        guide_html, _ = build_document(ir, layout, plan, Options(
            backend_name, "fonts", "assets", None, None, None, guide_plan, "guide",
            "absolute"))

        whole_pages = _pages_of(whole)
        form_pages = _pages_of(form_html)
        guide_pages = _pages_of(guide_html)
        claimed = {int(e["page"]): e for e in guide_plan["inline"]}

        # Every page except one the guide took whole: that page has no geometry
        # left to preserve and printed as a blank sheet stapled into the form.
        relocated = {int(e["page"]) for e in guide_plan["inline"] if e.get("whole_page")}
        expected_pages = sorted(set(whole_pages) - relocated)
        _check(sorted(form_pages) == expected_pages,
               f"{backend_name} form keeps every page it did not wholly relocate",
               f"{sorted(form_pages)} == {expected_pages} "
               f"({len(relocated)} relocated whole)", failures)
        _check(sorted(guide_pages) == sorted(claimed),
               f"{backend_name} guide carries only pages with a guide region",
               f"{sorted(guide_pages)} == {sorted(claimed)}", failures)

        for index, body in whole_pages.items():
            entry = claimed.get(index)
            if index not in form_pages and index not in guide_pages:
                _check(bool(entry and entry.get("whole_page")),
                       f"{backend_name} p{index} is emitted by one document or claimed whole",
                       f"whole_page={bool(entry and entry.get('whole_page'))}", failures)
                continue
            band_rules = set(re.findall(r'id="band-rules-([^"]+)"', body))

            # A clipped straddler is on both sides, as two pieces neither of
            # which is the whole element, so it is counted out of the identity
            # comparison and checked on its own below. 2551Q has none, so the
            # numbers here are unchanged for the form --self-test runs on.
            clipped = [s for s in (entry or {}).get("straddlers", ())
                       if s.get("disposition") == "clipped"]
            extra_rects = sum(1 for s in clipped
                              if s["kind"] in ("rule", "area_fill", "image"))
            clipped_refs = {s["ref"] for s in clipped}
            for label, pattern in (("rect", RECT_RE), ("text run", RUN_RE)):
                allowance = extra_rects if label == "rect" else 0
                everything = pattern.findall(body)
                mine = pattern.findall(form_pages[index])
                theirs = pattern.findall(guide_pages.get(index, ""))
                _check(len(mine) + len(theirs) == len(everything) + allowance,
                       f"{backend_name} p{index} {label}s sum to the whole",
                       f"{len(mine)} form + {len(theirs)} guide == {len(everything)}"
                       f"{f' + {allowance} clipped' if allowance else ''}",
                       failures)
                # Order is preserved on both sides, so a positional comparison
                # is enough to prove nothing was re-laid-out.
                # Identity is asserted over the marks that carry an id, which is
                # every rule and every text run. An area fill and an image carry
                # no id, so a clipped one of those cannot be told apart from its
                # own whole; on a page that has one, those are held to the count
                # above and to the reconstruction guides.py proves.
                anonymous = any(s["kind"] in ("area_fill", "image") for s in clipped)
                def identified(items: Sequence[str]) -> list[str]:
                    return sorted(item for item in items
                                  if not any(f'"{ref}"' in item for ref in clipped_refs)
                                  and (not anonymous or "data-rule-id=" in item
                                       or label != "rect"))
                _check(identified(mine + theirs) == identified(everything),
                       f"{backend_name} p{index} {label}s are byte-identical after the split",
                       f"{len(everything)} compared, {len(clipped_refs)} clipped"
                       f"{', anonymous fills held to the count' if anonymous else ''}",
                       failures)

            if entry is None:
                _check(form_pages[index] == body,
                       f"{backend_name} p{index} is untouched (no guide region)",
                       f"{len(body)} bytes", failures)
                continue

            # Every rule the guide claims must be gone from the form and present
            # on the guide -- unless a band owns it, in which case the band, and
            # therefore the form, keeps it whole.
            form_rules = set(re.findall(r'data-rule-id="([^"]+)"', form_pages[index]))
            guide_rules = set(re.findall(r'data-rule-id="([^"]+)"',
                                         guide_pages.get(index, "")))
            all_rules = {r["id"] for r in ir["pages"][index - 1]["rules"]
                         if r["role"] == "structural"}
            _check(all_rules <= (form_rules | guide_rules),
                   f"{backend_name} p{index} no structural rule is lost by the split",
                   f"{len(all_rules - (form_rules | guide_rules))} lost", failures)
            shared = (form_rules & guide_rules) - clipped_refs
            _check(not shared,
                   f"{backend_name} p{index} no rule is emitted twice",
                   f"{len(shared)} shared beyond the {len(clipped_refs)} clipped",
                   failures)
            _check(set(entry["rule_ids"]) & form_rules == set(),
                   f"{backend_name} p{index} the form drops exactly the claimed rules",
                   f"{len(set(entry['rule_ids']) & form_rules)} claimed rules kept",
                   failures)

            # Straddlers are clipped, so both documents draw the piece on their
            # own side of the cut and neither may lose the element.
            straddling_rules = [s["ref"] for s in entry["straddlers"] if s["kind"] == "rule"]
            _check(all(ref in form_rules and ref in guide_rules
                       for ref in straddling_rules),
                   f"{backend_name} p{index} both documents draw every straddling rule",
                   f"{len(straddling_rules)} straddler(s)", failures)

            _check(not (set(entry["rule_ids"]) & band_rules),
                   f"{backend_name} p{index} no growable band overlaps the guide region",
                   f"{len(band_rules)} band container(s)", failures)

        # Cells are addressed by id, so they are compared as sets.
        for index, entry in claimed.items():
            form_cells = set(CELL_RE.findall(form_pages[index]))
            guide_cells = set(CELL_RE.findall(guide_pages.get(index, "")))
            whole_cells = set(CELL_RE.findall(whole_pages[index]))
            clipped_cells = {s["ref"] for s in entry.get("straddlers", ())
                             if s["kind"] == "cell" and s.get("disposition") == "clipped"}
            _check(form_cells | guide_cells == whole_cells
                   and not ((form_cells & guide_cells) - clipped_cells),
                   f"{backend_name} p{index} cells partition exactly",
                   f"{len(form_cells)} + {len(guide_cells)} == {len(whole_cells)}, "
                   f"{len(clipped_cells)} clipped", failures)

    # The reflowed guide is a different document, so it is checked for what it
    # promises: every claimed run present, exactly once, and no coordinates.
    reflow, _ = build_document(ir, layout, plan, Options(
        "svg", "fonts", "assets", None, None, None, guide_plan, "guide", "reflow"))
    # Nothing in the reflowed body carries a coordinate, which is why it cannot
    # overlap. (`.doc-link` is absolute on purpose and is not body content.)
    body = reflow.split("</head>", 1)[-1]
    _check('class="page' not in body and 'class="t"' not in body
           and "pt;top:" not in body,
           "reflowed guide places no content absolutely",
           f"{len(reflow)} bytes", failures)
    for entry in guide_plan["inline"]:
        page = ir["pages"][int(entry["page"]) - 1]
        text = re.sub(r"<[^>]+>", " ", reflow)
        # Compared against the source text, so undo esc_text: an ampersand in an
        # ATC row would otherwise read as a dropped run (2550M has five).
        for entity, literal in (("&lt;", "<"), ("&gt;", ">"), ("&amp;", "&")):
            text = text.replace(entity, literal)
        dense = "".join(text.split())
        missing = [index for index in entry["text_run_indices"]
                   if "".join(page["text_runs"][index]["text"].split()) not in dense]
        _check(not missing,
               f"reflowed guide p{entry['page']} carries every claimed run",
               f"{len(entry['text_run_indices'])} runs, {len(missing)} missing", failures)

    print_assertions(ir, layout, plan, guide_plan, reflow, failures)


def _dense_text(html: str) -> str:
    """The document's visible text, whitespace-free, with esc_text undone."""
    text = re.sub(r"<[^>]+>", " ", html)
    for entity, literal in (("&lt;", "<"), ("&gt;", ">"), ("&amp;", "&")):
        text = text.replace(entity, literal)
    return "".join(text.split())


def print_assertions(ir: dict[str, Any], layout: dict[str, Any], plan: dict[str, Any],
                     guide_plan: dict[str, Any], reflow: str,
                     failures: list[str]) -> None:
    """A guide is a document people print, so assert it is printable.

    Without an `@page` a guide prints at the browser's default paper, which is
    Letter wherever the user happens to be and is not the paper its form prints
    on. Without the print block the navigation links print as furniture and
    headings strand at the foot of a sheet. Both are invisible on screen, which
    is exactly why they need an assertion rather than an inspection.
    """
    paper = ir["paper"]
    want_page = (f'@page{{size:{fmt(paper["width_pt"])}pt {fmt(paper["height_pt"])}pt;'
                 f'margin:{fmt(GUIDE_PAGE_MARGIN_PT)}pt}}')
    _check(want_page in reflow, "reflowed guide sets @page from the form's paper",
           want_page, failures)
    _check(f'@page{{size:{fmt(paper["width_pt"])}pt {fmt(paper["height_pt"])}pt;'
           f'margin:0}}' not in reflow,
           "the guide does not inherit the form's zero-margin @page",
           f"{fmt(GUIDE_PAGE_MARGIN_PT)}pt margin", failures)
    _check("@media print{.doc-link{display:none}}" in reflow,
           "the guide hides its cross-document link in print",
           "DOC_LINK_CSS present", failures)
    for declaration in ("break-after:avoid", "orphans:2", "break-inside:avoid"):
        _check(declaration in reflow.split("@media print{", 1)[-1],
               f"the guide's print block declares {declaration}",
               "pagination rule present", failures)

    # A standalone guide PDF, exercised on this form's own IR. 2551Q ships no
    # separate guide, so the case is built rather than found: what is being
    # asserted is that a source IR reaches the document as reflowed text and
    # that the pinned PDF is still linked beside it, and any extraction proves
    # that as well as the real one would.
    name = guide_source_key(ir)
    _check(name.lower().endswith(".pdf"), "guide_source_key reads the source file name",
           name, failures)
    staged = dict(guide_plan, standalone_pdfs=[f"/somewhere/{name}"], standalone_pdf=name)
    converted, _ = build_document(ir, layout, plan, Options(
        "svg", "fonts", "assets", None, None, None, staged, "guide", "reflow",
        guide_sources={name: ir}))
    dense = _dense_text(converted)
    runs = [run["text"] for page in ir["pages"] for run in page["text_runs"]]
    missing = [text for text in runs if "".join(text.split()) not in dense]
    _check(not missing, "a converted guide PDF carries every run of its own extraction",
           f"{len(runs)} runs, {len(missing)} missing", failures)
    _check('data-converted="true"' in converted and "<object" not in converted,
           "a converted guide PDF is reflowed text, not an embedded viewer",
           f"{len(converted)} bytes", failures)
    url = esc_attr(urllib.parse.quote(f"guides/{name}"))
    _check(f'<p class="gl-download">Source PDF: <a href="{url}">' in converted,
           "a converted guide PDF is still linked as the pinned artefact",
           url, failures)

    # The fallback has to stay honest: no extraction means the embed, and the
    # warning has to say that the embed does not print.
    embedded, warnings = build_document(ir, layout, plan, Options(
        "svg", "fonts", "assets", None, None, None, staged, "guide", "reflow"))
    _check('data-converted="false"' in embedded and "<object" in embedded,
           "a guide PDF with no extraction falls back to the embed",
           f"{len(embedded)} bytes", failures)
    _check(any("does not print" in warning for warning in warnings),
           "the fallback warns that an embedded PDF does not print",
           f"{len(warnings)} warning(s)", failures)

    # Wrong file, wrong form: both must fail rather than produce a document.
    for label, sources in (("a PDF the plan does not list", {"not-in-the-plan.pdf": ir}),
                           ("an extraction of another form",
                            {name: dict(ir, form={"code": "9999", "revision": "1999"})})):
        try:
            build_document(ir, layout, plan, Options(
                "svg", "fonts", "assets", None, None, None, staged, "guide", "reflow",
                guide_sources=sources))
        except SystemExit:
            _check(True, f"guide source is rejected: {label}", "SystemExit", failures)
        else:
            _check(False, f"guide source is rejected: {label}", "accepted", failures)


def self_test(ir_path: pathlib.Path, layout_path: pathlib.Path,
              plan_path: pathlib.Path,
              guide_plan_path: pathlib.Path | None = None) -> int:
    """Assert the emitted document carries the source's own inventory, exactly.

    Every assertion is a count or a set equality against the IR/layout, so it
    fails on omission as loudly as on duplication. A generator that silently
    drops a rule or paints one twice is the failure mode this pipeline exists
    to make impossible.
    """
    for path in (ir_path, layout_path, plan_path):
        if not path.is_file():
            print(f"self-test input missing: {path}", file=sys.stderr)
            return 2
    ir = json.loads(ir_path.read_text(encoding="utf-8"))
    layout = json.loads(layout_path.read_text(encoding="utf-8"))
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    failures: list[str] = []

    for backend_name in sorted(BACKENDS):
        print(f"backend {backend_name}", file=sys.stderr)
        options = Options(backend_name, "fonts", "assets", None, None, None)
        html, warnings = build_document(ir, layout, plan, options)

        pages = re.findall(r'<div class="page page-(\d+)"', html)
        _check(len(pages) == len(ir["pages"]), "page count",
               f"{len(pages)} == {len(ir['pages'])}", failures)

        for page in ir["pages"]:
            want = (f'style="width:{fmt(page["width_pt"])}pt;'
                    f'height:{fmt(page["height_pt"])}pt"')
            _check(want in html, f"page {page['index']} box",
                   f"{fmt(page['width_pt'])}x{fmt(page['height_pt'])}pt", failures)
        _check(f'@page{{size:{fmt(ir["paper"]["width_pt"])}pt '
               f'{fmt(ir["paper"]["height_pt"])}pt;margin:0}}' in html,
               "@page from MediaBox",
               f"{fmt(ir['paper']['width_pt'])}x{fmt(ir['paper']['height_pt'])}pt", failures)

        emitted_runs = re.findall(r'<div class="t" id="(p\d+t\d+)"', html)
        expected_runs = [run_id(p["index"], i)
                         for p in ir["pages"] for i in range(len(p["text_runs"]))]
        duplicates = sorted({r for r in emitted_runs if emitted_runs.count(r) > 1})
        _check(sorted(emitted_runs) == sorted(expected_runs) and not duplicates,
               "every text run exactly once",
               f"{len(emitted_runs)} emitted / {len(expected_runs)} in IR, "
               f"{len(duplicates)} duplicated", failures)

        painted = re.findall(r'data-rule-id="([^"]+)"', html)
        expected_rules = {f'{p["index"]}:{r["id"]}'
                          for p in ir["pages"] for r in p["rules"]
                          if r["role"] == "structural"}
        # Rule ids are unique per page only, so scope the comparison per page.
        per_page: list[str] = []
        for chunk in html.split('<div class="page page-')[1:]:
            number = re.match(r"(\d+)", chunk)
            for rid in re.findall(r'data-rule-id="([^"]+)"', chunk):
                per_page.append(f"{number.group(1)}:{rid}")
        missing = expected_rules - set(per_page)
        _check(not missing, "every structural rule in the rule layer",
               f"{len(expected_rules)} expected, {len(missing)} missing, "
               f"{len(painted)} rects carry an id", failures)

        # <template> content is inert: it is the blueprint for rows that do not
        # exist yet, so it must not be counted as rendered geometry.
        rendered = re.sub(r"<template\b.*?</template>", "", html, flags=re.S)
        slots = len(re.findall(r'data-slot="', rendered))
        expected_slots = sum(p["stats"]["comb_slots"] for p in layout["pages"])
        _check(slots == expected_slots, "comb slots outside the template",
               f"{slots} == {expected_slots}", failures)
        comb_cells = {c["id"]: c["comb"]["cells"]
                      for p in layout["pages"] for c in p["cells"] if c.get("comb")}
        bad = []
        for cid, count in sorted(comb_cells.items()):
            match = re.search(rf'(?<![-\w])id="{cid}"[^>]*data-comb-slots="(\d+)"', rendered)
            if match is None or int(match.group(1)) != count:
                bad.append(cid)
        _check(not bad, "comb slot counts match the layout",
               f"{len(comb_cells)} comb cells, {len(bad)} wrong", failures)

        field_assertions(ir, layout, plan, html, rendered, failures)

        bands = [(p["index"], g) for p in layout["pages"] for g in p["growable"]]
        for page_index, band in bands:
            template = re.search(
                rf'<template id="band-template-{band["id"]}"[^>]*'
                rf'data-capacity="(\d+)"[^>]*data-row-pitch="([^"]+)"', html)
            _check(template is not None and int(template.group(1)) == band["capacity"],
                   f"growable {band['id']} is a template",
                   f"capacity {template.group(1) if template else 'absent'} == "
                   f"{band['capacity']}", failures)
            _check(f'data-row-y="{",".join(fmt(v) for v in band["row_y"])}"' in html,
                   f"growable {band['id']} carries measured row_y",
                   f"{len(band['row_y'])} values", failures)
        _check(bool(bands), "at least one growable band", f"{len(bands)} found", failures)

        # The band must be laid out by indexing row_y, never by y0 + i*pitch.
        # 2551Q's last row is 18.27pt where the rest are 18.24, so the two
        # models disagree by 0.03pt at the closing rule -- small enough that
        # only an explicit assertion catches the wrong one.
        for page_ir, page_layout in zip(ir["pages"], layout["pages"]):
            cells_by_id = {c["id"]: c for c in page_layout["cells"]}
            for band in page_layout["growable"]:
                band_plan = build_band_plan(band, page_ir, cells_by_id)
                row_y = band_plan.row_y
                pitch = float(band["row_pitch_pt"])
                nominal = [row_y[0] + i * pitch for i in range(len(row_y))]
                drift = max(abs(a - b) for a, b in zip(row_y, nominal))
                rects = band_rects(band_plan, band_plan.capacity)
                _check({r.source_id for r in rects if r.source_id} == band_plan.rule_ids,
                       f"growable {band['id']} regenerates its own rules",
                       f"{len(rects)} rects, {len(band_plan.rule_ids)} source rules", failures)
                closing = [r for r in rects
                           if abs(r.y + r.h / 2.0 - row_y[-1]) <= BAND_EPSILON_PT]
                indexed = closing and all(
                    abs(r.y + r.h / 2.0 - row_y[-1]) < abs(r.y + r.h / 2.0 - nominal[-1])
                    for r in closing) if drift > 0 else bool(closing)
                _check(bool(indexed),
                       f"growable {band['id']} closes on row_y[-1], not y0+n*pitch",
                       f"{len(closing)} closing rules, pitch drift {fmt(drift)}pt", failures)
                _check(all(f"{fmt(r.y)}" in html for r in closing),
                       f"growable {band['id']} pre-render carries the measured close",
                       ", ".join(fmt(r.y) for r in closing) or "none", failures)

        for warning in warnings:
            print(f"  warn: {warning}", file=sys.stderr)

    with tempfile.TemporaryDirectory() as tmp:
        directory = pathlib.Path(tmp)
        for backend_name in sorted(BACKENDS):
            options = Options(backend_name, "fonts", "assets", None, None, None)
            first = directory / f"{backend_name}.1.html"
            second = directory / f"{backend_name}.2.html"
            first.write_text(build_document(ir, layout, plan, options)[0], encoding="utf-8")
            second.write_text(build_document(ir, layout, plan, options)[0], encoding="utf-8")
            _check(filecmp.cmp(first, second, shallow=False),
                   f"{backend_name} output is byte-identical across runs",
                   f"{first.stat().st_size} bytes", failures)

    if guide_plan_path is not None and guide_plan_path.is_file():
        print("form/guide split", file=sys.stderr)
        guide_plan = json.loads(guide_plan_path.read_text(encoding="utf-8"))
        # The split must cost nothing when there is nothing to split off.
        options = Options("svg", "fonts", "assets", None, None, None)
        plain, _ = build_document(ir, layout, plan, options)
        empty_plan = dict(guide_plan, inline=[], standalone_pdfs=[], standalone_pdf=None)
        with_empty, _ = build_document(ir, layout, plan, Options(
            "svg", "fonts", "assets", None, None, None, empty_plan, "form"))
        _check(plain == with_empty, "a guide plan that claims nothing changes nothing",
               f"{len(plain)} bytes", failures)
        split_assertions(ir, layout, plan, guide_plan, failures)
        for layout_name in ("absolute", "reflow"):
            variant = Options("svg", "fonts", "assets", None, None, None, guide_plan,
                              "guide", layout_name)
            first, _ = build_document(ir, layout, plan, variant)
            second, _ = build_document(ir, layout, plan, variant)
            _check(first == second,
                   f"{layout_name} guide output is byte-identical across runs",
                   f"{len(first)} chars", failures)
        print("constructed cases", file=sys.stderr)
        constructed_assertions(ir, layout, plan, guide_plan, failures)
    else:
        print(f"form/guide split: skipped, no guide plan at {guide_plan_path}",
              file=sys.stderr)
        print("constructed cases", file=sys.stderr)
        constructed_assertions(ir, layout, plan, None, failures)

    print(f"\n{'FAILED: ' + ', '.join(failures) if failures else 'all assertions passed'}",
          file=sys.stderr)
    return 1 if failures else 0


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--ir", type=pathlib.Path, default=DEFAULT_IR)
    parser.add_argument("--layout", type=pathlib.Path, default=DEFAULT_LAYOUT)
    parser.add_argument("--font-plan", type=pathlib.Path, default=DEFAULT_PLAN)
    parser.add_argument("--rule-backend", choices=sorted(BACKENDS), default="svg",
                        help="How rules are painted. Default svg (measured zero delta).")
    parser.add_argument("--fonts-dir", default="fonts",
                        help="Where the bundled WOFF2 live, relative to the HTML.")
    parser.add_argument("--assets-dir", default="assets",
                        help="Where the sha256-named images live, relative to the HTML.")
    parser.add_argument("--band-rows", type=int, default=None,
                        help="Rows to pre-render per growable band (default: capacity).")
    parser.add_argument("--guide-plan", type=pathlib.Path, default=None,
                        help="guides.py plan; splits the sheet into form and guide.")
    parser.add_argument("--document", choices=("form", "guide"), default="form",
                        help="Which half to emit. form keeps its full page boxes.")
    parser.add_argument("--guide-layout", choices=("absolute", "reflow"), default="reflow",
                        help="--document guide only: positioned runs, or reading order.")
    parser.add_argument("--guide-href", default="guide.html",
                        help="Where the form's link to the guide points.")
    parser.add_argument("--form-href", default="index.html",
                        help="Where the guide's link back to the form points.")
    parser.add_argument("--guide-pdf-dir", default="guides",
                        help="Where standalone guide PDFs live, relative to the HTML.")
    parser.add_argument("--guide-source", type=pathlib.Path, action="append", default=None,
                        metavar="IR", help="extract.py IR of a standalone guide PDF. Its "
                                           "text is reflowed into the guide document "
                                           "instead of the PDF being embedded, which is "
                                           "what makes it print. Repeatable.")
    parser.add_argument("--title", default=None)
    parser.add_argument("--out", type=pathlib.Path, default=None,
                        help="Write the HTML here (default: stdout).")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        return self_test(args.ir, args.layout, args.font_plan, args.guide_plan
                         or DEFAULT_GUIDE_PLAN)

    for path in (args.ir, args.layout, args.font_plan):
        if not path.is_file():
            print(f"no such input: {path}", file=sys.stderr)
            return 2
    if args.guide_plan is not None and not args.guide_plan.is_file():
        print(f"no such guide plan: {args.guide_plan}", file=sys.stderr)
        return 2
    if args.document == "guide" and args.guide_plan is None:
        print("--document guide needs --guide-plan", file=sys.stderr)
        return 2

    for path in args.guide_source or ():
        if not path.is_file():
            print(f"no such guide source IR: {path}", file=sys.stderr)
            return 2

    ir = json.loads(args.ir.read_text(encoding="utf-8"))
    layout = json.loads(args.layout.read_text(encoding="utf-8"))
    plan = json.loads(args.font_plan.read_text(encoding="utf-8"))
    guide_plan = (json.loads(args.guide_plan.read_text(encoding="utf-8"))
                  if args.guide_plan else None)
    guide_sources: dict[str, dict[str, Any]] = {}
    for path in args.guide_source or ():
        source_ir = json.loads(path.read_text(encoding="utf-8"))
        guide_sources[guide_source_key(source_ir)] = source_ir

    out_dir = args.out.resolve().parent if args.out else None
    options = Options(args.rule_backend, args.fonts_dir, args.assets_dir,
                      out_dir, args.band_rows, args.title, guide_plan,
                      args.document, args.guide_layout, args.guide_href,
                      args.form_href, args.guide_pdf_dir, guide_sources)
    html, warnings = build_document(ir, layout, plan, options)

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(html, encoding="utf-8")
        print(f"wrote {args.out} ({len(html)} bytes, {args.document} document, "
              f"{args.rule_backend} rule backend)", file=sys.stderr)
    else:
        sys.stdout.write(html)

    for warning in warnings:
        print(f"warn: {warning}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
