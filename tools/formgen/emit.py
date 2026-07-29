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


class RuleBackend:
    """Paints rects and placed artwork. The only thing that differs per backend.

    Images ride the backend too. An image is geometry in a box exactly as a
    rule is, so routing it through the same code is what makes the round-trip a
    comparison of *every* painted rectangle on the page rather than of the
    rules alone.
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
        geometry = (f'x="{fmt(image["x0"])}" y="{fmt(image["y0"])}" '
                    f'width="{fmt(image["x1"] - image["x0"])}" '
                    f'height="{fmt(image["y1"] - image["y0"])}"')
        tag = (f'<image href="{esc_attr(href)}" {geometry} preserveAspectRatio="none" '
               if present else
               f'<rect fill="none" {geometry} data-missing-src="{esc_attr(href)}" ')
        return f'{tag}data-sha256="{esc_attr(image["sha256"])}"/>'


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
        style = style_attr((
            ("left", f"{fmt(image['x0'])}pt"), ("top", f"{fmt(image['y0'])}pt"),
            ("width", f"{fmt(image['x1'] - image['x0'])}pt"),
            ("height", f"{fmt(image['y1'] - image['y0'])}pt"),
        ))
        common = (f'class="img" data-sha256="{esc_attr(image["sha256"])}" '
                  f'style="{esc_attr(style)}"')
        if present:
            return f'<img src="{esc_attr(href)}" alt="" {common}>'
        return f'<div data-missing-src="{esc_attr(href)}" {common}></div>'


BACKENDS = {"svg": SvgBackend, "css": CssBackend}


# ---------------------------------------------------------------------------
# Typography
# ---------------------------------------------------------------------------


class RunStyle:
    """Everything needed to place one text run, resolved from the font plan."""

    __slots__ = ("css", "scale_x", "baseline_offset_pt", "top_pt", "translate_y_pt",
                 "font_family", "font_file", "css_style", "font_face_weight",
                 "unresolved")

    def __init__(self, css: dict[str, Any], scale_x: float | None,
                 baseline_offset_pt: float, top_pt: float, translate_y_pt: float,
                 font_family: str | None, font_file: str | None, css_style: str,
                 unresolved: bool, font_face_weight: str = "100 900") -> None:
        self.css = css
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


def _rescaled_letter_spacing(run: dict[str, Any], scale: float) -> float | None:
    """Tracking for a run whose face is reached through scaleX(scale).

    The scale multiplies the whole inline box, tracking included, exactly as
    the PDF's Tz operator does. So if the unscaled face advances `natural` and
    we add `sp` per gap, the painted advance is scale*(natural + sp*(n-1)).
    `natural` is the PDF's own per-glyph advances divided by the scale -- the
    Narrow face *is* the donor face scaled -- which cancels to

        sp = (measured_advance - sum(char_widths)) / (scale * (n - 1))

    i.e. the source's own tracking, un-scaled. This needs no font file and no
    platform font, so it stays deterministic.
    """
    gaps = len(run["text"]) - 1
    if gaps <= 0 or scale <= 0:
        return None
    source_tracking = float(run["measured_advance_pt"]) - sum(run["char_widths_pt"])
    spacing = source_tracking / (scale * gaps)
    if (abs(spacing) < LETTER_SPACING_EPSILON_PT
            and abs(spacing) * gaps < LETTER_SPACING_ACCUMULATED_PT):
        return None
    return round(spacing, 4)


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
                                       None, None, css["font-style"], True)
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
    """The plan's CSS for one run, verbatim, in a fixed order."""
    css = style.css
    pairs: list[tuple[str, str | None]] = []
    for name in FONT_CSS_ORDER:
        value = css.get(name)
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

    The anchor is the first glyph's origin_x and the run's baseline_y, never
    the bbox: a bbox is ink extent and therefore depends on which glyphs happen
    to be in the string. How those two numbers reach the page -- `left`/`top`
    or a transform -- is decided by `transform_declarations`, because Chromium
    snaps the two axes differently.
    """
    pairs: list[tuple[str, str | None]] = [
        ("left", "0" if is_scaled(style) else f"{fmt(run['origin_x'])}pt"),
        ("top", f"{fmt(style.top_pt)}pt"),
    ]
    pairs.extend(font_declarations(run, style))
    pairs.extend(transform_declarations(style, float(run["origin_x"])))
    attrs = "".join(f' {name}="{esc_attr(value)}"' for name, value in extra)
    unresolved = ' data-unresolved="true"' if style.unresolved else ""
    return (f'<div class="t" id="{identifier}" style="{esc_attr(style_attr(pairs))}"'
            f'{attrs}{unresolved}>{esc_text(run["text"])}</div>')


def text_json(run: dict[str, Any], identifier: str, style: RunStyle,
              row_index: int, column_role: str | None) -> dict[str, Any]:
    """The same run as data, for the growable template's JS renderer."""
    scale = (style.scale_x if style.scale_x is not None
             and abs(style.scale_x - 1.0) > 1e-9 else None)
    return {
        "id": identifier,
        "row": row_index,
        "role": column_role,
        "text": run["text"],
        "x": round(float(run["origin_x"]), 4),
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
# Cells and combs
# ---------------------------------------------------------------------------


def cell_markup(cell: dict[str, Any], id_attribute: str = "id") -> str:
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
    """
    width = cell["x1"] - cell["x0"]
    height = cell["y1"] - cell["y0"]
    style = style_attr((
        ("left", f"{fmt(cell['x0'])}pt"), ("top", f"{fmt(cell['y0'])}pt"),
        ("width", f"{fmt(width)}pt"), ("height", f"{fmt(height)}pt"),
    ))
    kind = cell["kind"]
    classes = "c f" if kind == "field" else "c"
    attrs = [f'{id_attribute}="{esc_attr(cell["id"])}"', f'class="{classes}"',
             f'data-cell-kind="{esc_attr(kind)}"',
             f'data-row="{cell["row"]}"', f'data-col="{cell["col"]}"']
    if not cell.get("rectangular", True):
        attrs.append('data-rectangular="false"')

    comb = cell.get("comb")
    body = ""
    if comb:
        attrs.append(f'data-comb-slots="{comb["cells"]}"')
        attrs.append(f'data-comb-pitch="{fmt(comb["pitch_pt"])}"')
        body = comb_slots_markup(cell, comb)
    return f'<div {" ".join(attrs)} style="{esc_attr(style)}">{body}</div>'


def comb_slots_markup(cell: dict[str, Any], comb: dict[str, Any]) -> str:
    """N slots inside ONE field, from the comb's own measured slot_x."""
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
        parts.append(f'<div class="s" data-slot="{index}" style="{esc_attr(style)}"></div>')
    return "".join(parts)


def cell_json(cell: dict[str, Any]) -> dict[str, Any]:
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
              runs_by_id: dict[str, dict[str, Any]]) -> dict[str, Any]:
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
            "cells": [cell_json(c) for c in sorted(plan.cells_by_row.get(index, []),
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


def band_template_markup(plan: BandPlan, blob_index: int) -> str:
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
        parts.append(cell_markup(relative, id_attribute="data-cell-id"))
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
    the cut, so a straddler is simply never claimed. That is the whole of "the
    form wins every straddler" as far as the emitter is concerned: there is no
    second rule here that could disagree with the detector's, and no element
    can be dropped by both documents because the two sides are complements of
    one set.
    """

    __slots__ = ("guide_side", "rule_ids", "cell_ids", "run_indices",
                 "fill_indices", "image_indices")

    def __init__(self, guide_side: bool, rule_ids: frozenset[str] = frozenset(),
                 cell_ids: frozenset[str] = frozenset(),
                 run_indices: frozenset[int] = frozenset(),
                 fill_indices: frozenset[int] = frozenset(),
                 image_indices: frozenset[int] = frozenset()) -> None:
        self.guide_side = guide_side
        self.rule_ids = rule_ids
        self.cell_ids = cell_ids
        self.run_indices = run_indices
        self.fill_indices = fill_indices
        self.image_indices = image_indices

    def _keep(self, ref: Any, claimed: frozenset) -> bool:
        return (ref in claimed) if self.guide_side else (ref not in claimed)

    def keep_rule(self, rule_id: str) -> bool:
        return self._keep(rule_id, self.rule_ids)

    def keep_cell(self, cell_id: str) -> bool:
        return self._keep(cell_id, self.cell_ids)

    def keep_run(self, run_index: int) -> bool:
        return self._keep(run_index, self.run_indices)

    def keep_fill(self, index: int) -> bool:
        return self._keep(index, self.fill_indices)

    def keep_image(self, index: int) -> bool:
        return self._keep(index, self.image_indices)

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
                         self.fill_indices, self.image_indices)


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


def _line_text(line: Sequence[dict[str, Any]]) -> str:
    """One line's runs concatenated, with a space wherever the source left one."""
    parts: list[str] = []
    previous: dict[str, Any] | None = None
    for run in line:
        if previous is not None:
            gap = float(run["x0"]) - float(previous["x1"])
            if gap > WORD_GAP_FRACTION * float(run["size_pt"]):
                parts.append(" ")
        parts.append(run["text"])
        previous = run
    return "".join(parts)


def _is_heading(line: Sequence[dict[str, Any]], column_width: float) -> bool:
    return (all(run.get("bold") for run in line)
            and _line_ink(line) < HEADING_WIDTH_FRACTION * column_width
            and bool(_line_text(line).strip()))


def _blocks_of_lines(lines: Sequence[Sequence[dict[str, Any]]],
                     column_width: float, heading_tag: str) -> list[tuple[str, str]]:
    """One column's lines as (tag, text) blocks: headings and paragraphs."""
    baselines = [_line_baseline(line) for line in lines]
    gaps = [b - a for a, b in zip(baselines, baselines[1:])]
    typical = _median(gaps) or 1.0
    right_edge = max((max(float(r["x1"]) for r in line) for line in lines), default=0.0)

    out: list[tuple[str, str]] = []
    buffer: list[str] = []

    def flush() -> None:
        if buffer:
            out.append(("p", " ".join(" ".join(buffer).split())))
            buffer.clear()

    previous: Sequence[dict[str, Any]] | None = None
    for index, line in enumerate(lines):
        text = _line_text(line)
        if not text.strip():
            continue
        if _is_heading(line, column_width):
            flush()
            out.append((heading_tag, " ".join(text.split())))
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
        buffer.append(text)
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
            for tag, text in _blocks_of_lines(lines, column_width, "h3"):
                parts.append(f"<{tag}>{esc_text(text)}</{tag}>")
            parts.append("</div>")

    emit_block(0)
    for index, line in enumerate(spanning_lines):
        for tag, text in _blocks_of_lines([line], width, "h2"):
            parts.append(f"<{tag}>{esc_text(text)}</{tag}>")
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
        for tag, text in _blocks_of_lines(lines, grid[0][1] - grid[0][0] if grid else 1.0,
                                          "h3"):
            parts.append(f"<{tag}>{esc_text(text)}</{tag}>")
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
            text = " ".join(_line_text(sorted(group, key=lambda r: float(r["origin_x"]))).split())
            body.append(f"<{tag}{attribute}>{esc_text(text)}</{tag}>")
            index += span
        rows.append(f"<tr>{''.join(body)}</tr>")
    return f'<table class="gl-table">{"".join(rows)}</table>'


def reflow_page(page_ir: dict[str, Any], split: PageSplit) -> str:
    """One page's guide region as flowing HTML."""
    runs = [run for index, run in enumerate(page_ir["text_runs"])
            if index in split.run_indices]
    if not runs:
        return ""
    grid, flow = _column_bands(runs)
    prose = _is_prose(runs, flow)
    body = _prose_markup(runs, flow) if prose else _table_markup(runs, grid)
    return (f'<section class="gl-page" data-page="{page_ir["index"]}" '
            f'data-flow="{"prose" if prose else "table"}" '
            f'data-columns="{len(flow) if prose else len(grid)}">{body}</section>')


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
@media print{.gl{max-width:none;padding:0}}"""


# Shared by both guide layouts: the absolute one needs it too, and it is the
# only styling that document has beyond the form's own scaffolding.
GUIDE_PDF_CSS = """.gl-pdf{margin:2.5em auto;max-width:46em;
font:16px/1.6 Georgia,"Times New Roman",serif}
.gl-pdf object{display:block;width:100%;height:90vh;border:1px solid #bbb}
@media print{.gl-pdf object{height:auto}}"""


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
function applyCell(el,cell){
  el.removeAttribute("data-cell-id");
  el.id=cell.id;
  el.setAttribute("data-row",cell.row);
  el.setAttribute("data-col",cell.col);
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
  return el;
}
function cellElement(cell){
  var el=document.createElement("div");
  el.id=cell.id;
  el.className=cell.kind==="field"?"c f":"c";
  el.setAttribute("data-cell-kind",cell.kind);
  el.setAttribute("data-row",cell.row);
  el.setAttribute("data-col",cell.col);
  el.style.cssText="left:"+cell.x+"pt;top:"+cell.y+"pt;width:"+cell.w+
    "pt;height:"+cell.h+"pt";
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
      el.appendChild(slot);
    }
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


class Options:
    __slots__ = ("rule_backend", "fonts_dir", "assets_dir", "out_dir", "band_rows",
                 "title", "guide_plan", "document", "guide_layout", "guide_href",
                 "form_href", "guide_pdf_dir")

    def __init__(self, rule_backend: str, fonts_dir: str, assets_dir: str,
                 out_dir: pathlib.Path | None, band_rows: int | None,
                 title: str | None, guide_plan: dict[str, Any] | None = None,
                 document: str = "form", guide_layout: str = "reflow",
                 guide_href: str = "guide.html", form_href: str = "index.html",
                 guide_pdf_dir: str = "guides") -> None:
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


def font_face_css(styles: dict[tuple[int, int], RunStyle], options: Options,
                  warnings: list[str]) -> str:
    """@font-face for exactly the faces the emitted runs actually reference.

    Keyed by weight as well as by family and style, and the weight *descriptor*
    comes from the plan rather than being a constant. A variable file legitimately
    covers `100 900` on its own; a static family keeps each weight in a separate
    file, and declaring one of those over the whole range makes Chromium serve
    that single file for every weight and synthesise the rest -- emboldened
    outlines with advances no measurement in the plan covers.
    """
    used: dict[tuple[str, str, str], str] = {}
    for style in styles.values():
        if style.font_family and style.font_file:
            used.setdefault(
                (style.font_family, style.css_style, style.font_face_weight),
                style.font_file)
    blocks = []
    for (family, css_style, font_weight), font_file in sorted(used.items()):
        name = pathlib.PurePosixPath(font_file).name
        href = f"{options.fonts_dir.rstrip('/')}/{name}" if options.fonts_dir else name
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
              warnings: list[str], split: PageSplit = WHOLE_PAGE) -> str:
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
            painted.append((paint_key(fill, ""), "rect", Rect.from_box(fill)))
    for rule in page_ir["rules"]:
        if rule["id"] not in band_rule_ids and split.keep_rule(rule["id"]):
            painted.append((paint_key(rule, rule["id"]), "rect",
                            Rect.from_box(rule, rule["id"])))
    for image_index, image in enumerate(page_ir["images"]):
        if split.keep_image(image_index):
            painted.append((paint_key(image, image["sha256"]), "image", image))
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
        parts.append(text_markup(run, rid, styles[(index, run_index)]))
    parts.append("</div>")

    # -- field layer ---------------------------------------------------------
    parts.append('<div class="layer-cells">')
    for cell in page_layout["cells"]:
        if cell["id"] in band_cell_ids or not split.keep_cell(cell["id"]):
            continue
        parts.append(cell_markup(cell))
    parts.append("</div>")

    # -- growable bands: template + pre-rendered rows ------------------------
    for plan in plans:
        rows = plan.capacity if options.band_rows is None else min(options.band_rows,
                                                                   plan.capacity)
        parts.append(band_template_markup(plan, len(band_blobs)))
        parts.append(f'<div class="band" id="band-content-{esc_attr(plan.band["id"])}" '
                     f'data-band="{esc_attr(plan.band["id"])}" '
                     f'data-rendered-rows="{rows}" data-overflow-rows="0" '
                     f'data-capacity="{plan.capacity}" '
                     f'data-row-pitch="{fmt(plan.band["row_pitch_pt"])}">')
        for row in range(rows):
            for cell in sorted(plan.cells_by_row.get(row, []), key=lambda c: (c["x0"], c["id"])):
                parts.append(cell_markup(cell))
            for rid, _cell, _ in sorted(plan.texts_by_row.get(row, []), key=_run_order):
                key = (index, _run_index_of(rid))
                parts.append(text_markup(runs_by_id[rid], rid, styles[key],
                                         extra=(("data-band-row", str(row)),)))
        parts.append("</div>")
        band_blobs.append(band_json(plan, rows, styles, runs_by_id))

    parts.append("</div>")
    return "".join(parts)


def doc_link_markup(href: str, label: str) -> str:
    return f'<a class="doc-link" href="{esc_attr(href)}">{esc_text(label)}</a>'


def standalone_pdf_markup(split: DocumentSplit, options: Options,
                          warnings: list[str]) -> str:
    """The guide PDFs batch.py skips, embedded after the inline guide pages.

    Embedded rather than converted. Converting them would mean running the
    extractor, the lattice and the font planner over a second document to
    produce something whose parity is explicitly not measured -- and the pinned
    PDF is already the exact artefact, so handing it to the viewer's PDF engine
    loses nothing the guide document was carrying.
    """
    if not split.standalone_pdfs:
        return ""
    directory = options.guide_pdf_dir.rstrip("/")
    parts = ['<section class="gl-pdf">']
    for source in split.standalone_pdfs:
        name = pathlib.PurePosixPath(source).name
        href = f"{directory}/{name}" if directory else name
        # BIR ships these with spaces in the file name ("1701Q Guide Jan 2018.pdf"),
        # which is a valid path and not a valid URL.
        url = urllib.parse.quote(href)
        if options.out_dir is not None and not (options.out_dir / href).is_file():
            warnings.append(
                f"missing guide PDF {href}: the guide document embeds it but it is not "
                f"beside the output, so the section renders empty")
        parts.append(f"<h2>{esc_text(name)}</h2>")
        parts.append(f'<object data="{esc_attr(url)}" type="application/pdf" '
                     f'data-source="{esc_attr(name)}">'
                     f'<a href="{esc_attr(url)}">{esc_text(name)}</a></object>')
    parts.append("</section>")
    return "".join(parts)


def _head(ir: dict[str, Any], title: str, styles: str, backend_name: str | None,
          document: str) -> list[str]:
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
    return [
        "<!doctype html>",
        f"<html {' '.join(attributes)}>",
        "<head>",
        '<meta charset="utf-8">',
        f"<title>{esc_text(title)}</title>",
        "<!-- Generated by tools/formgen/emit.py from the pinned PDF's own content "
        "stream. Do not hand-edit: regenerate. -->",
        "<style>",
        styles,
        "</style>",
        "</head>",
        "<body>",
    ]


def build_reflow_guide(ir: dict[str, Any], split: DocumentSplit, options: Options,
                       warnings: list[str]) -> tuple[str, list[str]]:
    """The guide as a readable document: no coordinates, therefore no overlap."""
    form = ir["form"]
    title = options.title or (f"BIR Form {form['code']} ({form['revision']}) "
                              f"-- Guidelines and Instructions")
    by_index = {int(page["index"]): page for page in ir["pages"]}
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
        body.append(reflow_page(page, split.page(index)))
    body.append(standalone_pdf_markup(split, options, warnings))
    body.append("</div>")

    head = _head(ir, title,
                 "\n".join([BASE_CSS.rstrip(), DOC_LINK_CSS, GUIDE_CSS, GUIDE_PDF_CSS]),
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
    if options.document == "guide" and options.guide_layout == "reflow":
        return build_reflow_guide(ir, split, options, warnings)

    backend = BACKENDS[options.rule_backend]()
    styles = resolve_run_styles(ir, plan, warnings)

    # The form keeps every page at its full height even where the guide took the
    # lower 70% of one: the page box, the page count and @page are the form's
    # geometry, and the freed space is what a growable band expands into. The
    # guide document carries only the pages it actually has content on.
    wanted = (set(split.guide_pages) if options.document == "guide"
              else {int(page["index"]) for page in ir["pages"]})

    band_blobs: list[dict[str, Any]] = []
    pages = [emit_page(page_ir, page_layout, styles, backend, options, band_blobs,
                       warnings, split.page(int(page_ir["index"])))
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

    styles_css = [page_css(ir), font_face_css(styles, options, warnings),
                  BASE_CSS.rstrip()]
    if link:
        styles_css.append(DOC_LINK_CSS)
    if split.guide_side and split.standalone_pdfs:
        styles_css.append(GUIDE_PDF_CSS)
    head = _head(ir, title, "\n".join(styles_css), backend.name,
                 options.document if options.document != "form" else "form")
    if link:
        head.append(link)
    tail = [
        '<script type="application/json" id="formgen-bands">',
        json.dumps(band_blobs, ensure_ascii=False, separators=(",", ":")),
        "</script>",
        "<script>",
        BAND_JS,
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
    """The emitted document sliced per page, for per-page inventory checks."""
    out: dict[int, str] = {}
    chunks = PAGE_SPLIT_RE.split(html)
    for index in range(1, len(chunks) - 1, 2):
        out[int(chunks[index])] = chunks[index + 1]
    return out


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

        _check(sorted(form_pages) == sorted(whole_pages),
               f"{backend_name} form keeps every page",
               f"{sorted(form_pages)} == {sorted(whole_pages)}", failures)
        _check(sorted(guide_pages) == sorted(claimed),
               f"{backend_name} guide carries only pages with a guide region",
               f"{sorted(guide_pages)} == {sorted(claimed)}", failures)

        for index, body in whole_pages.items():
            entry = claimed.get(index)
            band_rules = set(re.findall(r'id="band-rules-([^"]+)"', body))

            for label, pattern in (("rect", RECT_RE), ("text run", RUN_RE)):
                everything = pattern.findall(body)
                mine = pattern.findall(form_pages[index])
                theirs = pattern.findall(guide_pages.get(index, ""))
                _check(len(mine) + len(theirs) == len(everything),
                       f"{backend_name} p{index} {label}s sum to the whole",
                       f"{len(mine)} form + {len(theirs)} guide == {len(everything)}",
                       failures)
                # Order is preserved on both sides, so a positional comparison
                # is enough to prove nothing was re-laid-out.
                merged = sorted(mine + theirs)
                _check(merged == sorted(everything),
                       f"{backend_name} p{index} {label}s are byte-identical after the split",
                       f"{len(everything)} compared", failures)

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
            _check(not (form_rules & guide_rules),
                   f"{backend_name} p{index} no rule is emitted twice",
                   f"{len(form_rules & guide_rules)} shared", failures)
            _check(set(entry["rule_ids"]) & form_rules == set(),
                   f"{backend_name} p{index} the form drops exactly the claimed rules",
                   f"{len(set(entry['rule_ids']) & form_rules)} claimed rules kept",
                   failures)

            # Straddlers: claimed by nobody, therefore the form's.
            straddling_rules = [s["ref"] for s in entry["straddlers"] if s["kind"] == "rule"]
            _check(all(ref in form_rules for ref in straddling_rules),
                   f"{backend_name} p{index} the form keeps every straddling rule",
                   f"{len(straddling_rules)} straddler(s)", failures)

            _check(not (set(entry["rule_ids"]) & band_rules),
                   f"{backend_name} p{index} no growable band overlaps the guide region",
                   f"{len(band_rules)} band container(s)", failures)

        # Cells are addressed by id, so they are compared as sets.
        for index, entry in claimed.items():
            form_cells = set(CELL_RE.findall(form_pages[index]))
            guide_cells = set(CELL_RE.findall(guide_pages.get(index, "")))
            whole_cells = set(CELL_RE.findall(whole_pages[index]))
            _check(form_cells | guide_cells == whole_cells and not (form_cells & guide_cells),
                   f"{backend_name} p{index} cells partition exactly",
                   f"{len(form_cells)} + {len(guide_cells)} == {len(whole_cells)}",
                   failures)

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
    else:
        print(f"form/guide split: skipped, no guide plan at {guide_plan_path}",
              file=sys.stderr)

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

    ir = json.loads(args.ir.read_text(encoding="utf-8"))
    layout = json.loads(args.layout.read_text(encoding="utf-8"))
    plan = json.loads(args.font_plan.read_text(encoding="utf-8"))
    guide_plan = (json.loads(args.guide_plan.read_text(encoding="utf-8"))
                  if args.guide_plan else None)

    out_dir = args.out.resolve().parent if args.out else None
    options = Options(args.rule_backend, args.fonts_dir, args.assets_dir,
                      out_dir, args.band_rows, args.title, guide_plan,
                      args.document, args.guide_layout, args.guide_href,
                      args.form_href, args.guide_pdf_dir)
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
