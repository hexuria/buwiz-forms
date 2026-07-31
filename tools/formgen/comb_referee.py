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
_CELL_PAGE_RE = re.compile(r"^p(\d+)c\d+$")
_CELL_SLOT_RE = re.compile(r"^(p\d+c\d+)-s(\d+)$")
_PAGE_RE = re.compile(r"^page-(\d+)$")
# emit.py serialises point geometry to four decimal places.  Two independently
# rounded endpoints can differ by at most two ten-thousandths of a point.
HTML_GEOMETRY_EPSILON_PT = 0.0002
SVG_INLINE_STYLE_PROPERTIES = frozenset({
    "clip-path",
    "display",
    "fill",
    "fill-opacity",
    "fill-rule",
    "filter",
    "marker-end",
    "marker-mid",
    "marker-start",
    "mask",
    "opacity",
    "paint-order",
    "stroke",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-opacity",
    "stroke-width",
    "transform",
    "vector-effect",
    "visibility",
})
UNSUPPORTED_SVG_PRESENTATION_ATTRIBUTES = frozenset({
    "backdrop-filter",
    "isolation",
    "mix-blend-mode",
    "transform-box",
    "transform-origin",
})
HTML_VOID_ELEMENTS = frozenset({
    "area",
    "base",
    "br",
    "col",
    "embed",
    "hr",
    "img",
    "input",
    "link",
    "meta",
    "param",
    "source",
    "track",
    "wbr",
})
HTML_RENDER_AFFECTING_INLINE_PROPERTIES = frozenset({
    "all",
    "animation",
    "animation-name",
    "backdrop-filter",
    "clip",
    "clip-path",
    "contain",
    "content-visibility",
    "display",
    "filter",
    "isolation",
    "mix-blend-mode",
    "opacity",
    "perspective",
    "rotate",
    "scale",
    "transform",
    "translate",
    "visibility",
    "zoom",
})
# These are the only structural declarations emitted by emit.py's document
# stylesheet.  Point geometry itself remains bound independently to the layout
# artifact below.  Rejecting any other structural selector/property prevents a
# stylesheet from moving or hiding a comb while its inline numbers still look
# canonical.
HTML_STYLESHEET_STRUCTURAL_DECLARATIONS = frozenset({
    (".band", "height", "100%"),
    (".band", "left", "0"),
    (".band", "position", "absolute"),
    (".band", "top", "0"),
    (".band", "width", "100%"),
    (".c", "position", "absolute"),
    (".c", "z-index", "6"),
    (".doc-link", "display", "none"),
    (".doc-link", "left", "0"),
    (".doc-link", "position", "absolute"),
    (".doc-link", "top", "0"),
    (".doc-link", "z-index", "9"),
    (".f,.f .s", "overflow", "hidden"),
    (".fi", "inset", "0"),
    (".fi", "position", "absolute"),
    (".img", "display", "block"),
    (".img", "position", "absolute"),
    (".img", "z-index", "6"),
    (".page", "overflow", "hidden"),
    (".page", "position", "relative"),
    (".r", "position", "absolute"),
    (".rl", "left", "0"),
    (".rl", "position", "absolute"),
    (".rl", "top", "0"),
    (".s", "position", "absolute"),
    (".t", "position", "absolute"),
    (".t", "z-index", "5"),
})
HTML_STYLESHEET_STRUCTURAL_PROPERTIES = frozenset({
    "all",
    "animation",
    "animation-name",
    "backdrop-filter",
    "bottom",
    "clip",
    "clip-path",
    "contain",
    "content-visibility",
    "display",
    "filter",
    "height",
    "inset",
    "isolation",
    "left",
    "mix-blend-mode",
    "opacity",
    "overflow",
    "perspective",
    "position",
    "right",
    "rotate",
    "scale",
    "top",
    "transform",
    "translate",
    "visibility",
    "width",
    "z-index",
    "zoom",
})
HTML_REQUIRED_STYLESHEET_DECLARATIONS = frozenset({
    (".page", "overflow", "hidden"),
    (".page", "position", "relative"),
    (".c", "position", "absolute"),
    (".s", "position", "absolute"),
})
HTML_STYLESHEET_FIXED_VALUES: dict[tuple[str, str], frozenset[str]] = {
    ("*", "box-sizing"): frozenset({"border-box"}),
    ("*", "margin"): frozenset({"0"}),
    ("*", "padding"): frozenset({"0"}),
    ("html,body", "-webkit-print-color-adjust"): frozenset({"exact"}),
    ("html,body", "background"): frozenset({"#fff"}),
    ("html,body", "print-color-adjust"): frozenset({"exact"}),
    (".page", "background"): frozenset({"#fff"}),
    (".page", "break-after"): frozenset({"page"}),
    (".page", "overflow"): frozenset({"hidden"}),
    (".page", "page-break-after"): frozenset({"always"}),
    (".page", "position"): frozenset({"relative"}),
    (".page:last-of-type", "break-after"): frozenset({"auto"}),
    (".page:last-of-type", "page-break-after"): frozenset({"auto"}),
    (".rl", "left"): frozenset({"0"}),
    (".rl", "position"): frozenset({"absolute"}),
    (".rl", "top"): frozenset({"0"}),
    (".r", "position"): frozenset({"absolute"}),
    (".t", "position"): frozenset({"absolute"}),
    (".t", "text-rendering"): frozenset({"geometricprecision"}),
    (".t", "white-space"): frozenset({"pre"}),
    (".t", "z-index"): frozenset({"5"}),
    (".c", "position"): frozenset({"absolute"}),
    (".c", "z-index"): frozenset({"6"}),
    (".s", "position"): frozenset({"absolute"}),
    (".img", "display"): frozenset({"block"}),
    (".img", "position"): frozenset({"absolute"}),
    (".img", "z-index"): frozenset({"6"}),
    (".band", "height"): frozenset({"100%"}),
    (".band", "left"): frozenset({"0"}),
    (".band", "position"): frozenset({"absolute"}),
    (".band", "top"): frozenset({"0"}),
    (".band", "width"): frozenset({"100%"}),
    (".f,.f .s", "overflow"): frozenset({"hidden"}),
    (".fc", "text-align"): frozenset({"center"}),
    (".fi", "-webkit-appearance"): frozenset({"none"}),
    (".fi", "appearance"): frozenset({"none"}),
    (".fi", "background"): frozenset({"none", "transparent"}),
    (".fi", "border"): frozenset({"0"}),
    (".fi", "border-radius"): frozenset({"0"}),
    (".fi", "box-shadow"): frozenset({"none"}),
    (".fi", "caret-color"): frozenset({"#000000", "transparent"}),
    (".fi", "color"): frozenset({"#000000"}),
    (".fi", "font-family"): frozenset({
        '"ebirforms arimo", arimo, arial, helvetica, sans-serif',
        '"ebirforms tinos", tinos, "times new roman", times, serif',
    }),
    (".fi", "font-feature-settings"): frozenset({"normal"}),
    (".fi", "font-kerning"): frozenset({"none"}),
    (".fi", "font-style"): frozenset({"normal"}),
    (".fi", "font-variant-ligatures"): frozenset({"none"}),
    (".fi", "font-variation-settings"): frozenset({'"wght" 400'}),
    (".fi", "font-weight"): frozenset({"400"}),
    (".fi", "inset"): frozenset({"0"}),
    (".fi", "margin"): frozenset({"0"}),
    (".fi", "outline"): frozenset({"0"}),
    (".fi", "padding"): frozenset({"0"}),
    (".fi", "position"): frozenset({"absolute"}),
    (".fi", "text-rendering"): frozenset({"geometricprecision"}),
    (".fi:focus", "background"): frozenset({"rgba(255,213,0,.35)"}),
    (".fi:hover", "background"): frozenset({"rgba(21,101,192,.07)"}),
    (".doc-link", "background"): frozenset({"#fff"}),
    (".doc-link", "color"): frozenset({"#0645ad"}),
    (".doc-link", "display"): frozenset({"none"}),
    (".doc-link", "font"): frozenset({
        "12px/1.5 system-ui,-apple-system,sans-serif",
    }),
    (".doc-link", "left"): frozenset({"0"}),
    (".doc-link", "padding"): frozenset({"2px 8px"}),
    (".doc-link", "position"): frozenset({"absolute"}),
    (".doc-link", "text-decoration"): frozenset({"underline"}),
    (".doc-link", "top"): frozenset({"0"}),
    (".doc-link", "z-index"): frozenset({"9"}),
    ("@font-face", "font-display"): frozenset({"block"}),
    ("@font-face", "font-family"): frozenset({
        '"ebirforms arimo"', '"ebirforms tinos"',
    }),
    ("@font-face", "font-style"): frozenset({"italic", "normal"}),
    ("@font-face", "font-weight"): frozenset({"100 900", "400", "700"}),
    ("@font-face", "src"): frozenset({
        'url("fonts/arimo-latin-wght-italic.woff2") format("woff2")',
        'url("fonts/arimo-latin-wght-normal.woff2") format("woff2")',
        'url("fonts/tinos-latin-400-normal.woff2") format("woff2")',
        'url("fonts/tinos-latin-700-normal.woff2") format("woff2")',
    }),
    ("@page", "margin"): frozenset({"0"}),
}
HTML_RUNTIME_SCRIPT_SHA256 = (
    "8822f0d4efe00ffbf32e2e0fe2922139419f08184143699b789a0aa5050e649d",
    "e2b0b7794d0b72c3d5ab818c290ffca183f3b1fff9797e487450a3ca4b0f4049",
)
HTML_ROOT_ATTRIBUTES = frozenset({
    "data-form",
    "data-revision",
    "data-rule-backend",
    "data-schema-version",
    "data-source-sha256",
    "lang",
})
HTML_COMB_ATTRIBUTES = frozenset({
    "class",
    "data-cell-kind",
    "data-col",
    "data-comb-capacity",
    "data-comb-pitch",
    "data-comb-slots",
    "data-field-kind",
    "data-field-name",
    "data-row",
    "id",
    "style",
})
HTML_BAND_ATTRIBUTES = frozenset({
    "class",
    "data-band",
    "data-capacity",
    "data-overflow-rows",
    "data-rendered-rows",
    "data-row-pitch",
    "id",
})
HTML_INPUT_ATTRIBUTES = frozenset({
    "autocomplete",
    "class",
    "data-slot-index",
    "id",
    "maxlength",
    "name",
    "spellcheck",
    "type",
})
HTML_ALLOWED_TAGS = frozenset({
    "a",
    "body",
    "div",
    "g",
    "head",
    "html",
    "image",
    "input",
    "meta",
    "path",
    "rect",
    "script",
    "style",
    "svg",
    "template",
    "title",
})
EXPECTED_HTML_STRUCTURE_SHA256 = {
    "0605-1999": "c0f5b317acc3f7a91218d0052c230f7543586207269335e9f469d5ff68c686ce",
    "0619e-2018": "62aa9ffe1abd268aafd9b1f92ab785a2b330d539d41e65e81f98694ce4bfd26e",
    "0619f-2018": "6de62567cc02542290a0f3309f0e03a8edd7c3c453fc09b72c700320ab3a9b8c",
    "0620-2019": "e94671f3ef65b735e1fefea378e0e5fd03e4b8f031f3dede04bde376cab9b6cf",
    "1600-pt-2018": "75916042d009e371b46e85e6c01c7c3f4319589489d2054baffa7afbc5df98b0",
    "1600-vt-2018": "95eac4a8323344437e4b7e9671f42d58fdf23446938e5ca975193407be897ebd",
    "1600wp-2010": "9e36cf1c5193097277e8338ce5ed3de99523aeed38e058c970bd8d7f35856fb7",
    "1601-fq-2020": "9d4962553754d39a7385c18c371bc7b622876b8c43cad2e36f5d724d1415f3f6",
    "1601c-2018": "141c580333d7944152f1bc8b6890569f983bedd692e10270cbd2ee0767d85d3e",
    "1601eq-2019": "e6339bb8ecaca4ff42f9fe61ad96bd502237ee169e1186b659b1bd80a1f74189",
    "1602q-2019": "aecb3ad69147973490d938b6a302cb4ca08a402084347ae20f25ae78ddbad18f",
    "1603q-2018": "0d88492b0da6af76b4a353b104e3f264f69c8cbc35997f437064522f1316cff2",
    "1604c-2018": "5eca15e11e0f9eb6a0caad505e160974c574292f526ff5573ddea39fa891c38a",
    "1604e-2018": "02a044a9b710d951e82f83bb492a46c27e30c3c97ea69094f5771143f829f25b",
    "1604f-2018": "6aa25dcbdd09c7f397be82fd5f08c752db17a6563f33e9dbd9c7d30b53a06eb6",
    "1606-2018": "db5b2a0db989eedfe5a71880400417f6557e65b0f97059d4f7ec0e60514ddd93",
    "1621-2019": "a0a62ddc125cde598038900b1ca6c0b14e4420a70e7421acdcdb4f8191bd1c2a",
    "1700-2018": "dcb4cc299ffb130c0db4078d2c3eedd409c079ecfd6de683d3e0263aa283f77b",
    "1701-2018-attachment": "d0b89cc8cf9ecb1fff1628685d6075bd66577e8df4268bcbc85e210a273e58cc",
    "1701-2018-conso": "1b4b8822aeb6b41b9dc9e018ee4cc4f73a706f285a3eb8dc698fbf74fe06ec28",
    "1701-2018": "762624a4eb1f28b710a7f8c404d8dab730803bf416cf4838f89b37b086cf6cbe",
    "1701a-2018": "69dd21e623831c9e4cf4b154575c4a10c1fde38138f10504dbc2bc30a2dbc95c",
    "1701ms-2024": "cd74c59cbef774eddc05d5b82345c7b8a5555e786f9c856375bd735c49f70be8",
    "1701q-2018": "7f905f0a6f6dbc3ace2f3fc8ece417482bd9e0b9657185c5a0222d7c5b2eed4b",
    "1702ex-2018": "d50e0337d4b3cfaf21d170e47a96c219357406dbcc2740d2fff499c0133a084d",
    "1702mx-2018c-attachment": "961ab19566da9e55a5f2ecae456f53a52c21eb545f61cd9cdbb55e269743f56f",
    "1702mx-2018c": "40852f243f9c9c8acd48b4c0b6cb1c5181b4d7a354da7d024c3f4db8f1508520",
    "1702q-2018": "ea9096a3966edc0c767a0160aa153b152702601d87fd452eddf454627c96e14d",
    "1702rt-2018c": "44036c7fdd62cb5475a295f54c9f566f4204d1df1fe07af90b0f245fa601c6cb",
    "1706-2018": "cb054a57cc52c0cc1f50928b10861428a14ef6ff263c4d08759ceba1d2be6340",
    "1707-2021": "380b0fdd62fff628e73c98fa177a0bb84cd1bffe763197849d884d6b41405522",
    "1707a-2021": "5e1becbcf8ff4aad926a13c8d2bc1d0fb5edacdea6391b56c99d47f8929fa487",
    "1709-2020": "4da50040ad141ab34452e7da1543c82f676f68abc9cb37d7705121cb2a1b1e10",
    "1800-2018": "dea65d9aaa76e629d60078997d76d67c19bff8b69f03c3a98d9575ec01387c48",
    "1801-2018": "4314255e1f23997da0625d2e27e8ccd5542610a962857be4c1b80b9232967a76",
    "2000-dst-2018": "08bd02c06327e8f55118e80bf27560d129ac09761465598c4ff17646ce76cb2f",
    "2000-ot-2018": "43c7f29626a4280cc5ddd79935ad8b081d57f881458c3ba5c3591d9e684ff828",
    "2200a-2020": "5dc5f65fdac719faf84c5e2d5c72a466e53092b0ad21162cf290e9cee0fe0cdc",
    "2200c-2018": "ddc92a67ccb86db40a370f944cafad539e99c0de37a33dfafe8f73193e6da6f1",
    "2200m-2018": "0426ea453c7dcc316a3ac626f4f25b738eb9325ba00aa14d7315df92c3d7ef0c",
    "2200p-2020": "f437a0b499919f641eb6a73cb61edb96244f37266af634a47a61f970e1cca664",
    "2200s-2018": "fc661952fe593c6e94d7c057fc4c123c25963c7f49fc995fa86b86454a6723b8",
    "2200t-2022": "83b4d70dc7579dc07003253ecdabd6786c86bb05bf065588a3f1dc8c8d8e67af",
    "2316-2021": "242fedf3bfc88bbc8f74555158b8ea035f6bb134634b21d4ddf9c0b214b18725",
    "2550-ds-2025": "9edf4d25ed8cb162d290423e24c60d416a5fb3458f05866e909c3cd3833ad5ff",
    "2550m-2007": "7b58e855ad91bf225fe0fc58eb34942dc0e70c83106910d311fcf95915017bee",
    "2550q-2024": "78241985e9a8c3a65a2c63d21d5cc4b046cd6bcb576e045e3b21b72468d296b9",
    "2551m-2002": "fa1af132bb6f1fb89d2dfcd863843b1748d838178c6bc606f133306e8a0226ab",
    "2551q-2018": "38bd02121b3dc7e0396c815f09a6f333ca4f6dcdf5656b13a171a32fa6091595",
    "2552-2018": "56f5993ccf453b938ec12b50535120adc3ebc396792787aeb4880826c8e4214a",
    "2553-1999": "a307ae2b29c772a67582ac67ba3e8ec52d7e640b22061d2fec77871d9b70767e",
}
if set(EXPECTED_HTML_STRUCTURE_SHA256) != set(EXPECTED_COMBS_BY_SLUG):
    raise RuntimeError("HTML structural pins disagree with the referee corpus")


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
        """Largest singular value of the affine linear part.

        A determinant/geometric-mean scale is not conservative under
        anisotropic transforms: ``scale(10, 1)`` would report only sqrt(10)
        even though a stroked curve can expand tenfold in x.  The largest
        singular value bounds the transformed stroke in every direction.
        """
        trace = self.a * self.a + self.b * self.b + self.c * self.c + self.d * self.d
        determinant = self.a * self.d - self.b * self.c
        discriminant = max(0.0, trace * trace - 4 * determinant * determinant)
        return math.sqrt(max(0.0, (trace + math.sqrt(discriminant)) / 2))

    def is_similarity(self) -> bool:
        """Whether the linear part preserves angles up to a uniform scale."""
        first = self.a * self.a + self.b * self.b
        second = self.c * self.c + self.d * self.d
        dot = self.a * self.c + self.b * self.d
        scale = max(1.0, first, second)
        return (abs(first - second) <= 1e-10 * scale
                and abs(dot) <= 1e-10 * scale)


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
    tone: float | None = None
    order: int = -1
    clipped: bool = True


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


def emitted_structure_sha256(payload: bytes) -> str:
    """Hash every emitted byte; any change requires an explicit pin review."""
    return sha256_bytes(payload)


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
        arguments = match.group(2)
        if re.sub(r"[\s,]+", "", re.sub(_NUMBER, "", arguments)):
            raise RefereeError(
                f"unsupported SVG transform units: {match.group(0)}")
        values = [float(v) for v in re.findall(_NUMBER, arguments)]
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


def inline_style(element: ET.Element) -> dict[str, str]:
    raw_style = element.get("style", "")
    if ("/*" in raw_style or "*/" in raw_style
            or re.search(r"!\s*important\b", raw_style,
                         flags=re.IGNORECASE)):
        raise RefereeError(
            "SVG inline CSS comments and !important are unsupported")
    result: dict[str, str] = {}
    for part in raw_style.split(";"):
        part = part.strip()
        if not part:
            continue
        if ":" not in part:
            raise RefereeError(f"malformed SVG inline style: {raw_style}")
        key, value = part.split(":", 1)
        key = key.strip().lower()
        if (not key or key in result
                or key not in SVG_INLINE_STYLE_PROPERTIES):
            raise RefereeError(
                f"unsupported or duplicate SVG CSS property: {key}")
        result[key] = value.strip()
    return result


def parse_style(element: ET.Element, inherited: dict[str, str]) -> dict[str, str]:
    style = dict(inherited)
    inline = inline_style(element)
    for key in ("fill", "fill-opacity", "fill-rule",
                "stroke", "stroke-opacity",
                "stroke-width", "stroke-dasharray", "stroke-dashoffset",
                "stroke-linecap", "stroke-linejoin", "stroke-miterlimit",
                "vector-effect", "paint-order",
                "marker-start", "marker-mid", "marker-end",
                "display", "visibility"):
        if key in element.attrib:
            style[key] = element.attrib[key]
        if key in inline:
            style[key] = inline[key]
    local_opacity = clamp_opacity(inline.get(
        "opacity", element.get("opacity", "1")))
    style["_cumulative-opacity"] = str(
        float(inherited.get("_cumulative-opacity", "1"))
        * local_opacity
    )
    return style


def svg_keyword(style: dict[str, str], key: str, default: str,
                allowed: Sequence[str]) -> str:
    value = style.get(key, default).strip().lower()
    if value not in allowed:
        raise RefereeError(f"unsupported SVG {key} value: {value}")
    return value


def reject_unsupported_svg_presentation(element: ET.Element) -> None:
    unsupported = sorted(
        set(element.attrib) & UNSUPPORTED_SVG_PRESENTATION_ATTRIBUTES)
    if unsupported:
        raise RefereeError(
            "unsupported SVG presentation attribute(s): "
            + ", ".join(unsupported))


def clamp_opacity(value: str | float) -> float:
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        raise RefereeError(f"invalid SVG opacity: {value}")
    if not math.isfinite(parsed):
        raise RefereeError(f"non-finite SVG opacity: {value}")
    return max(0.0, min(1.0, parsed))


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
    opacity = clamp_opacity(style.get("_cumulative-opacity", "1"))
    opacity *= clamp_opacity(style.get(f"{key}-opacity", "1"))
    if opacity <= 1e-12:
        return None
    # Composite over white paper; this preserves decorative greys rather than
    # treating every non-white paint as black.
    return 1 - opacity * (1 - tone)


def effective_opacity(style: dict[str, str], key: str) -> float:
    return (clamp_opacity(style.get("_cumulative-opacity", "1"))
            * clamp_opacity(style.get(f"{key}-opacity", "1")))


def arc_bound_points(start: tuple[float, float], rx: float, ry: float,
                     rotation: float, large_arc: float, sweep: float,
                     end: tuple[float, float]) -> list[tuple[float, float]]:
    """Return a conservative local-space box for one SVG elliptical arc.

    The complete ellipse is deliberately bounded, not only the selected arc.
    That may make a nearby comb unevaluable, but it can never hide painted
    geometry.  The SVG endpoint-to-centre conversion below follows the W3C
    implementation notes and includes the mandatory radii expansion.
    """
    rx, ry = abs(rx), abs(ry)
    if rx <= 1e-12 or ry <= 1e-12 or start == end:
        return [start, end]
    phi = math.radians(rotation % 360.0)
    cos_phi, sin_phi = math.cos(phi), math.sin(phi)
    dx = (start[0] - end[0]) / 2
    dy = (start[1] - end[1]) / 2
    x_prime = cos_phi * dx + sin_phi * dy
    y_prime = -sin_phi * dx + cos_phi * dy
    scale = (x_prime * x_prime) / (rx * rx) + (
        y_prime * y_prime) / (ry * ry)
    if scale > 1:
        factor = math.sqrt(scale)
        rx *= factor
        ry *= factor
    numerator = max(
        0.0,
        rx * rx * ry * ry
        - rx * rx * y_prime * y_prime
        - ry * ry * x_prime * x_prime,
    )
    denominator = (
        rx * rx * y_prime * y_prime
        + ry * ry * x_prime * x_prime
    )
    coefficient = 0.0 if denominator <= 1e-24 else math.sqrt(
        numerator / denominator)
    if bool(round(large_arc)) == bool(round(sweep)):
        coefficient = -coefficient
    cx_prime = coefficient * rx * y_prime / ry
    cy_prime = -coefficient * ry * x_prime / rx
    cx = (
        cos_phi * cx_prime - sin_phi * cy_prime
        + (start[0] + end[0]) / 2
    )
    cy = (
        sin_phi * cx_prime + cos_phi * cy_prime
        + (start[1] + end[1]) / 2
    )
    x_radius = math.sqrt((rx * cos_phi) ** 2 + (ry * sin_phi) ** 2)
    y_radius = math.sqrt((rx * sin_phi) ** 2 + (ry * cos_phi) ** 2)
    return [
        start,
        end,
        (cx - x_radius, cy - y_radius),
        (cx - x_radius, cy + y_radius),
        (cx + x_radius, cy - y_radius),
        (cx + x_radius, cy + y_radius),
    ]


def path_subpaths(
        data: str
) -> tuple[
    list[tuple[list[tuple[float, float]], bool]],
    list[list[tuple[float, float]]],
    bool,
]:
    """Parse SVG paths without letting one curve poison an entire page.

    Linear subpaths are returned for exact rectangle/line handling.  A subpath
    containing a Bezier or arc is returned as a conservative point cloud: an
    affine transform of that cloud still bounds the painted curve because
    Beziers stay inside their control hull and arcs use the full ellipse box.
    ``malformed`` is separate because an unparsed command has no honest local
    bound and must remain page-wide UNEVALUABLE.
    """
    tokens = _PATH_TOKEN_RE.findall(data)
    subpaths: list[tuple[list[tuple[float, float]], bool]] = []
    unsupported_subpaths: list[list[tuple[float, float]]] = []
    current: list[tuple[float, float]] = []
    unsupported_points: list[tuple[float, float]] = []
    command: str | None = None
    cursor = (0.0, 0.0)
    start = (0.0, 0.0)
    previous_op: str | None = None
    cubic_control: tuple[float, float] | None = None
    quadratic_control: tuple[float, float] | None = None
    i = 0
    malformed = False

    def number(index: int) -> float:
        if index >= len(tokens) or tokens[index].isalpha():
            raise ValueError("missing path coordinate")
        return float(tokens[index])

    def point(x: float, y: float, relative: bool,
              base: tuple[float, float]) -> tuple[float, float]:
        return (x + base[0], y + base[1]) if relative else (x, y)

    def flush(closed: bool) -> None:
        nonlocal current, unsupported_points
        if current:
            if unsupported_points:
                unsupported_subpaths.append([
                    *current,
                    *unsupported_points,
                ])
            else:
                subpaths.append((current, closed))
        current = []
        unsupported_points = []

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
                flush(True)
                cursor = start
                command = None
                previous_op = "Z"
                cubic_control = None
                quadratic_control = None
                continue
            if op in ("M", "L"):
                x, y = number(i), number(i + 1)
                i += 2
                x, y = point(x, y, relative, cursor)
                if op == "M":
                    flush(False)
                    current = [(x, y)]
                    start = (x, y)
                    command = "l" if relative else "L"
                else:
                    current.append((x, y))
                    if unsupported_points:
                        unsupported_points.append((x, y))
                cursor = (x, y)
            elif op == "H":
                x = number(i)
                i += 1
                if relative:
                    x += cursor[0]
                cursor = (x, cursor[1])
                current.append(cursor)
                if unsupported_points:
                    unsupported_points.append(cursor)
            elif op == "V":
                y = number(i)
                i += 1
                if relative:
                    y += cursor[1]
                cursor = (cursor[0], y)
                current.append(cursor)
                if unsupported_points:
                    unsupported_points.append(cursor)
            elif op == "C":
                base = cursor
                control1 = point(number(i), number(i + 1), relative, base)
                control2 = point(number(i + 2), number(i + 3), relative, base)
                end = point(number(i + 4), number(i + 5), relative, base)
                i += 6
                unsupported_points.extend((cursor, control1, control2, end))
                current.append(end)
                cursor = end
                cubic_control = control2
                quadratic_control = None
            elif op == "S":
                base = cursor
                reflected = (
                    (2 * cursor[0] - cubic_control[0],
                     2 * cursor[1] - cubic_control[1])
                    if previous_op in ("C", "S") and cubic_control is not None
                    else cursor
                )
                control2 = point(number(i), number(i + 1), relative, base)
                end = point(number(i + 2), number(i + 3), relative, base)
                i += 4
                unsupported_points.extend(
                    (cursor, reflected, control2, end))
                current.append(end)
                cursor = end
                cubic_control = control2
                quadratic_control = None
            elif op == "Q":
                base = cursor
                control = point(number(i), number(i + 1), relative, base)
                end = point(number(i + 2), number(i + 3), relative, base)
                i += 4
                unsupported_points.extend((cursor, control, end))
                current.append(end)
                cursor = end
                quadratic_control = control
                cubic_control = None
            elif op == "T":
                base = cursor
                control = (
                    (2 * cursor[0] - quadratic_control[0],
                     2 * cursor[1] - quadratic_control[1])
                    if previous_op in ("Q", "T")
                    and quadratic_control is not None
                    else cursor
                )
                end = point(number(i), number(i + 1), relative, base)
                i += 2
                unsupported_points.extend((cursor, control, end))
                current.append(end)
                cursor = end
                quadratic_control = control
                cubic_control = None
            elif op == "A":
                rx, ry = number(i), number(i + 1)
                rotation = number(i + 2)
                large_arc, sweep = number(i + 3), number(i + 4)
                if large_arc not in (0.0, 1.0) or sweep not in (0.0, 1.0):
                    raise ValueError("invalid SVG arc flag")
                end = point(number(i + 5), number(i + 6), relative, cursor)
                i += 7
                unsupported_points.extend(arc_bound_points(
                    cursor, rx, ry, rotation, large_arc, sweep, end))
                current.append(end)
                cursor = end
                cubic_control = None
                quadratic_control = None
            else:
                malformed = True
                break
            if op not in ("C", "S"):
                cubic_control = None
            if op not in ("Q", "T"):
                quadratic_control = None
            previous_op = op
    except (ValueError, IndexError):
        malformed = True

    flush(False)
    return subpaths, unsupported_subpaths, malformed


def bbox(points: Sequence[tuple[float, float]]) -> tuple[float, float, float, float]:
    xs = [point[0] for point in points]
    ys = [point[1] for point in points]
    return min(xs), min(ys), max(xs), max(ys)


def transformed_bbox(points: Sequence[tuple[float, float]],
                     transform: Matrix) -> tuple[float, float, float, float]:
    return bbox([transform.point(x, y) for x, y in points])


def transformed_ellipse_bbox(cx: float, cy: float, rx: float, ry: float,
                             transform: Matrix
                             ) -> tuple[float, float, float, float]:
    centre_x, centre_y = transform.point(cx, cy)
    radius_x = math.sqrt(
        (transform.a * rx) ** 2 + (transform.c * ry) ** 2)
    radius_y = math.sqrt(
        (transform.b * rx) ** 2 + (transform.d * ry) ** 2)
    return (
        centre_x - radius_x, centre_y - radius_y,
        centre_x + radius_x, centre_y + radius_y,
    )


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
    match = re.fullmatch(rf"\s*({_NUMBER})(?:px)?\s*", raw)
    if not match:
        raise RefereeError(f"non-numeric SVG {name}: {raw}")
    value = float(match.group(1))
    if not math.isfinite(value):
        raise RefereeError(f"non-finite SVG {name}: {raw}")
    return value


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

    def next_order() -> int:
        nonlocal order
        value = order
        order += 1
        return value

    def add_rect(box_value: tuple[float, float, float, float], tone: float,
                 kind: str, element_id: str, clipped: bool,
                 paint_order: int | None = None) -> None:
        x0, y0, x1, y1 = box_value
        if x1 <= x0 or y1 <= y0:
            return
        if paint_order is None:
            paint_order = next_order()
        paints.append(Paint(round(x0, 6), round(y0, 6), round(x1, 6),
                            round(y1, 6), round(tone, 8), paint_order, kind,
                            element_id, clipped))

    def add_unsupported(
        box_value: tuple[float, float, float, float],
        reason: str,
        element_id: str,
        tone: float | None = None,
        clipped: bool = True,
        paint_order: int | None = None,
    ) -> None:
        x0, y0, x1, y1 = box_value
        if x1 <= x0 or y1 <= y0:
            return
        if paint_order is None:
            paint_order = next_order()
        unsupported.append(UnsupportedRegion(
            round(x0, 6), round(y0, 6), round(x1, 6), round(y1, 6),
            reason, element_id,
            None if tone is None else round(tone, 8),
            paint_order, clipped,
        ))

    def stroke_metrics(style: dict[str, str], matrix: Matrix
                       ) -> tuple[float, float, bool, str]:
        """Return transformed width, conservative join pad, ambiguity, cap."""
        vector_effect = style.get("vector-effect", "none").strip().lower()
        if vector_effect in ("", "none"):
            scale = matrix.stroke_scale()
            vector_ambiguous = False
            transform_ambiguous = not matrix.is_similarity()
        elif vector_effect == "non-scaling-stroke":
            scale = 1.0
            vector_ambiguous = False
            transform_ambiguous = False
        else:
            # Keep a conservative extent while refusing to treat the stroke as
            # an exact divider.
            scale = max(1.0, matrix.stroke_scale())
            vector_ambiguous = True
            transform_ambiguous = True
        width_value = float(style.get("stroke-width", "1")) * scale
        dash = style.get("stroke-dasharray", "none").strip().lower()
        dashed = dash not in ("", "none")
        linecap = style.get("stroke-linecap", "butt").strip().lower()
        linejoin = style.get("stroke-linejoin", "miter").strip().lower()
        semantics_ambiguous = (
            vector_ambiguous
            or transform_ambiguous
            or dashed
            or linecap not in ("butt", "round", "square")
            or linejoin not in ("miter", "round", "bevel")
        )
        try:
            miter_limit = float(style.get("stroke-miterlimit", "4"))
        except ValueError:
            miter_limit = 4.0
            semantics_ambiguous = True
        if not math.isfinite(miter_limit) or miter_limit < 1:
            miter_limit = 4.0
            semantics_ambiguous = True
        join_pad = width_value / 2
        if linejoin == "miter":
            join_pad *= miter_limit
        return width_value, join_pad, semantics_ambiguous, linecap

    def nondefault_paint_order(style: dict[str, str]) -> bool:
        """Whether SVG paint ordering is not the default fill-then-stroke.

        The referee records fills and strokes as separate ordered rectangles.
        Treating an explicit reordering as if it were the default can invert
        final ownership at a divider.  No corpus result may depend on that
        approximation, so an explicit ordering remains locally ambiguous.
        """
        return style.get("paint-order", "normal").strip().lower() not in (
            "", "normal")

    def add_glyph_reference(
        referenced: ET.Element,
        instance_matrix: Matrix,
        instance_style: dict[str, str],
        instance_clipped: bool,
        element_id: str,
        href: str,
    ) -> None:
        """Record glyph paint as possible occlusion, never as a divider."""

        def visit(node: ET.Element, parent_matrix: Matrix,
                  inherited_style: dict[str, str],
                  inherited_clipped: bool) -> None:
            node_tag = node.tag.rsplit("}", 1)[-1]
            reject_unsupported_svg_presentation(node)
            node_inline = inline_style(node)
            transform_text = node_inline.get(
                "transform", node.get("transform"))
            node_matrix = parent_matrix.then(parse_transform(
                None if transform_text in (None, "", "none")
                else transform_text))
            node_style = parse_style(node, inherited_style)
            display = svg_keyword(
                node_style, "display", "inline", ("inline", "none"))
            visibility = svg_keyword(
                node_style, "visibility", "visible",
                ("visible", "hidden", "collapse"))
            if display == "none":
                return
            node_effects = {
                key: node_inline.get(key, node.get(key))
                for key in ("clip-path", "mask", "filter")
            }
            node_clipped = inherited_clipped or any(
                value not in (None, "", "none")
                for value in node_effects.values())
            if visibility in ("hidden", "collapse"):
                # Visibility is inherited, but a descendant can restore it.
                # Display:none, handled above, prunes the whole subtree.
                for child in node:
                    visit(child, node_matrix, node_style, node_clipped)
                return
            if node_tag in ("g", "symbol"):
                for child in node:
                    visit(child, node_matrix, node_style, node_clipped)
                return
            if node_tag != "path":
                add_unsupported(
                    (0.0, 0.0, width, height),
                    f"unsupported glyph use target: {href}", element_id)
                return
            linear, curved, malformed = path_subpaths(node.get("d", ""))
            points = [
                point for subpath, _closed in linear for point in subpath
            ]
            points.extend(point for subpath in curved for point in subpath)
            glyph_fill = effective_tone(node_style, "fill")
            glyph_stroke = effective_tone(node_style, "stroke")
            if malformed:
                add_unsupported(
                    (0.0, 0.0, width, height),
                    f"malformed glyph use: {href}", element_id,
                    glyph_stroke if glyph_stroke is not None else glyph_fill)
                return
            if not points or (glyph_fill is None and glyph_stroke is None):
                return
            transformed = [
                node_matrix.point(x, y) for x, y in points
            ]
            glyph_box = bbox(transformed)
            if glyph_fill is not None:
                add_unsupported(
                    glyph_box,
                    f"glyph use may occlude geometry: {href}",
                    element_id, glyph_fill,
                    node_clipped
                    or effective_opacity(node_style, "fill") < 1.0 - 1e-8
                    or nondefault_paint_order(node_style))
            if glyph_stroke is not None:
                _width, glyph_pad, glyph_ambiguous, _cap = stroke_metrics(
                    node_style, node_matrix)
                add_unsupported(
                    (glyph_box[0] - glyph_pad,
                     glyph_box[1] - glyph_pad,
                     glyph_box[2] + glyph_pad,
                     glyph_box[3] + glyph_pad),
                    f"stroked glyph use may occlude geometry: {href}",
                    element_id, glyph_stroke,
                    node_clipped or glyph_ambiguous
                    or nondefault_paint_order(node_style))

        visit(referenced, instance_matrix, instance_style, instance_clipped)

    def walk(element: ET.Element, parent_matrix: Matrix,
             inherited: dict[str, str], in_defs: bool = False,
             clipped: bool = False) -> None:
        tag = element.tag.rsplit("}", 1)[-1]
        if tag == "defs":
            return
        reject_unsupported_svg_presentation(element)
        local_inline = inline_style(element)
        transform_text = local_inline.get(
            "transform", element.get("transform"))
        local = parse_transform(
            None if transform_text in (None, "", "none") else transform_text)
        matrix = parent_matrix.then(local)
        style = parse_style(element, inherited)
        local_effects = {
            key: local_inline.get(key, element.get(key))
            for key in ("clip-path", "mask", "filter")
        }
        clipped_here = clipped or any(
            value not in (None, "", "none") for value in local_effects.values())
        display = svg_keyword(
            style, "display", "inline", ("inline", "none"))
        visibility = svg_keyword(
            style, "visibility", "visible",
            ("visible", "hidden", "collapse"))
        if display == "none":
            return
        if visibility in ("hidden", "collapse"):
            # ``visibility`` is inherited but, unlike ``display:none``, a
            # descendant may explicitly restore ``visibility:visible``.
            for child in element:
                walk(child, matrix, style, in_defs, clipped_here)
            return
        element_id = element.get("id") or f"{tag}-{len(paints) + len(unsupported)}"
        marker_values = [
            style.get(name, "none").strip().lower()
            for name in ("marker-start", "marker-mid", "marker-end")
        ]
        if any(value not in ("", "none") for value in marker_values):
            add_unsupported(
                (0.0, 0.0, width, height),
                "SVG marker paint is not resolved", element_id)
            return
        if tag == "switch":
            add_unsupported(
                (0.0, 0.0, width, height),
                "SVG switch conditional selection is not resolved",
                element_id)
            return
        if local_effects["filter"] not in (None, "", "none"):
            add_unsupported(
                (0.0, 0.0, width, height),
                "SVG filter has unbounded paint effects", element_id)
        if tag == "svg" and element is not root:
            nested_width = attr_float(element, "width")
            nested_height = attr_float(element, "height")
            nested_x = attr_float(element, "x")
            nested_y = attr_float(element, "y")
            if nested_width > 0 and nested_height > 0:
                add_unsupported(
                    transformed_bbox(
                        [(nested_x, nested_y),
                         (nested_x + nested_width, nested_y),
                         (nested_x + nested_width, nested_y + nested_height),
                         (nested_x, nested_y + nested_height)],
                        matrix,
                    ),
                    "nested SVG viewport", element_id)
            else:
                add_unsupported(
                    (0.0, 0.0, width, height),
                    "unbounded nested SVG viewport", element_id)
            return

        if tag == "path":
            subpaths, curved, malformed = path_subpaths(element.get("d", ""))
            fill = effective_tone(style, "fill")
            stroke = effective_tone(style, "stroke")
            fill_rule = style.get("fill-rule", "nonzero").strip().lower()
            fill_subpaths = [
                (points, closed)
                for points, closed in subpaths
                if len(points) >= 3
                and bbox(points)[2] - bbox(points)[0] > 1e-9
                and bbox(points)[3] - bbox(points)[1] > 1e-9
            ]
            rectangular_boxes = [
                bbox(points)
                for points, closed in fill_subpaths
                if closed and is_axis_aligned_rectangle(points)
            ]
            rectangles_overlap = any(
                min(first[2], second[2]) - max(first[0], second[0]) > 1e-8
                and min(first[3], second[3]) - max(first[1], second[1]) > 1e-8
                for index, first in enumerate(rectangular_boxes)
                for second in rectangular_boxes[index + 1:]
            )
            compound_fill_ambiguous = (
                fill_rule not in ("", "nonzero", "evenodd")
                or (
                    len(fill_subpaths) > 1
                    and (
                        len(rectangular_boxes) != len(fill_subpaths)
                        or rectangles_overlap
                    )
                )
            )
            if fill is not None and compound_fill_ambiguous:
                fill_points = [
                    point for points, _closed in subpaths for point in points
                ]
                fill_points.extend(
                    point for points in curved for point in points)
                if fill_points:
                    add_unsupported(
                        transformed_bbox(fill_points, matrix),
                        ("non-default or compound SVG fill topology"),
                        element_id, fill, clipped_here)
                fill = None
            fill_ambiguous = (
                clipped_here
                or effective_opacity(style, "fill") < 1.0 - 1e-8
                or nondefault_paint_order(style))
            stroke_width, join_pad, stroke_semantics_ambiguous, linecap = (
                stroke_metrics(style, matrix)
            )
            stroke_ambiguous = (
                clipped_here
                or effective_opacity(style, "stroke") < 1.0 - 1e-8
                or stroke_semantics_ambiguous
                or nondefault_paint_order(style)
            )
            for points, closed in subpaths:
                if len(points) < 2:
                    continue
                transformed = [matrix.point(x, y) for x, y in points]
                if fill is not None:
                    if closed and is_axis_aligned_rectangle(transformed):
                        add_rect(bbox(transformed), fill, "fill",
                                 element_id, fill_ambiguous)
                    elif closed or len(transformed) >= 3:
                        x0, y0, x1, y1 = bbox(transformed)
                        add_unsupported(
                            (x0, y0, x1, y1),
                            ("non-rectangular closed SVG fill" if closed else
                             "implicitly closed SVG fill"),
                            element_id, fill, fill_ambiguous)
                if stroke is not None:
                    half = stroke_width / 2
                    cap_pad = half if linecap in ("round", "square") else 0.0
                    pairs = list(zip(transformed, transformed[1:]))
                    if closed:
                        pairs.append((transformed[-1], transformed[0]))
                    for (x0, y0), (x1, y1) in pairs:
                        if abs(x1 - x0) <= 1e-6 and abs(y1 - y0) > 0:
                            add_rect((min(x0, x1) - half,
                                      min(y0, y1) - cap_pad,
                                      max(x0, x1) + half,
                                      max(y0, y1) + cap_pad),
                                     stroke, "stroke", element_id,
                                     stroke_ambiguous)
                        elif (abs(y1 - y0) > abs(x1 - x0)
                              and abs(x1 - x0) / 2 <= POSITION_TOL_PT):
                            centre_x = (x0 + x1) / 2
                            drift_pad = abs(x1 - x0) / 2
                            add_rect(
                                (centre_x - half - drift_pad,
                                 min(y0, y1) - cap_pad,
                                 centre_x + half + drift_pad,
                                 max(y0, y1) + cap_pad),
                                stroke, "near-vertical-stroke", element_id,
                                stroke_ambiguous)
                        elif (abs(y1 - y0) <= 1e-6
                              and abs(x1 - x0) > 0):
                            add_rect((min(x0, x1) - cap_pad,
                                      min(y0, y1) - half,
                                      max(x0, x1) + cap_pad,
                                      max(y0, y1) + half),
                                     stroke, "stroke", element_id,
                                     stroke_ambiguous)
                        elif abs(x1 - x0) > 0 or abs(y1 - y0) > 0:
                            add_unsupported(
                                (min(x0, x1) - join_pad,
                                 min(y0, y1) - join_pad,
                                 max(x0, x1) + join_pad,
                                 max(y0, y1) + join_pad),
                                "diagonal SVG path stroke", element_id,
                                stroke, stroke_ambiguous)
            if curved and (fill is not None or stroke is not None):
                pad = join_pad if stroke is not None else 0.0
                curve_tone = stroke if stroke is not None else fill
                for points in curved:
                    transformed = [matrix.point(x, y) for x, y in points]
                    x0, y0, x1, y1 = bbox(transformed)
                    add_unsupported(
                        (x0 - pad, y0 - pad, x1 + pad, y1 + pad),
                        "curved SVG path", element_id, curve_tone,
                        clipped_here or stroke_semantics_ambiguous)
            if malformed and (fill is not None or stroke is not None):
                add_unsupported(
                    (0.0, 0.0, width, height),
                    "malformed or unknown SVG path command", element_id,
                    stroke if stroke is not None else fill)
        elif tag == "rect":
            x, y = attr_float(element, "x"), attr_float(element, "y")
            w, h = attr_float(element, "width"), attr_float(element, "height")
            if w < 0 or h < 0:
                add_unsupported(
                    (0.0, 0.0, width, height),
                    "negative SVG rect extent", element_id)
                return
            if w == 0 or h == 0:
                return
            points = [matrix.point(x, y), matrix.point(x + w, y),
                      matrix.point(x + w, y + h), matrix.point(x, y + h)]
            fill = effective_tone(style, "fill")
            stroke = effective_tone(style, "stroke")
            stroke_width, join_pad, stroke_semantics_ambiguous, _linecap = (
                stroke_metrics(style, matrix)
            )
            fill_ambiguous = (
                clipped_here
                or effective_opacity(style, "fill") < 1.0 - 1e-8
                or nondefault_paint_order(style))
            stroke_ambiguous = (
                clipped_here
                or effective_opacity(style, "stroke") < 1.0 - 1e-8
                or stroke_semantics_ambiguous
                or nondefault_paint_order(style)
            )
            rounded = attr_float(element, "rx") > 0 or attr_float(element, "ry") > 0
            if rounded and (fill is not None or stroke is not None):
                box_value = bbox(points)
                add_unsupported(
                    (box_value[0] - (join_pad if stroke is not None else 0.0),
                     box_value[1] - (join_pad if stroke is not None else 0.0),
                     box_value[2] + (join_pad if stroke is not None else 0.0),
                     box_value[3] + (join_pad if stroke is not None else 0.0)),
                    "rounded SVG rect", element_id,
                    stroke if stroke is not None else fill,
                    clipped_here or stroke_semantics_ambiguous)
                fill = None
                stroke = None
            elif not is_axis_aligned_rectangle(points):
                x0, y0, x1, y1 = bbox(points)
                add_unsupported(
                    (x0 - (join_pad if stroke is not None else 0.0),
                     y0 - (join_pad if stroke is not None else 0.0),
                     x1 + (join_pad if stroke is not None else 0.0),
                     y1 + (join_pad if stroke is not None else 0.0)),
                    "transformed SVG rect is not axis-aligned", element_id,
                    stroke if stroke is not None else fill,
                    clipped_here or stroke_semantics_ambiguous)
                fill = None
                stroke = None
            if fill is not None:
                add_rect(bbox(points), fill, "fill", element_id, fill_ambiguous)
            if stroke is not None:
                half = stroke_width / 2
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
            stroke_width, join_pad, stroke_semantics_ambiguous, linecap = (
                stroke_metrics(style, matrix)
            )
            stroke_ambiguous = (
                clipped_here
                or effective_opacity(style, "stroke") < 1.0 - 1e-8
                or stroke_semantics_ambiguous
                or nondefault_paint_order(style)
            )
            half = stroke_width / 2
            cap_pad = half if linecap in ("round", "square") else 0.0
            if stroke is not None and abs(p1[0] - p0[0]) <= 1e-6:
                add_rect((min(p0[0], p1[0]) - half,
                          min(p0[1], p1[1]) - cap_pad,
                          max(p0[0], p1[0]) + half,
                          max(p0[1], p1[1]) + cap_pad),
                         stroke, "stroke", element_id, stroke_ambiguous)
            elif (stroke is not None
                  and abs(p1[1] - p0[1]) > abs(p1[0] - p0[0])
                  and abs(p1[0] - p0[0]) / 2 <= POSITION_TOL_PT):
                centre_x = (p0[0] + p1[0]) / 2
                drift_pad = abs(p1[0] - p0[0]) / 2
                add_rect(
                    (centre_x - half - drift_pad,
                     min(p0[1], p1[1]) - cap_pad,
                     centre_x + half + drift_pad,
                     max(p0[1], p1[1]) + cap_pad),
                    stroke, "near-vertical-line", element_id,
                    stroke_ambiguous)
            elif stroke is not None and abs(p1[1] - p0[1]) <= 1e-6:
                add_rect((min(p0[0], p1[0]) - cap_pad,
                          min(p0[1], p1[1]) - half,
                          max(p0[0], p1[0]) + cap_pad,
                          max(p0[1], p1[1]) + half),
                         stroke, "stroke", element_id, stroke_ambiguous)
            elif stroke is not None:
                add_unsupported(
                    (min(p0[0], p1[0]) - join_pad,
                     min(p0[1], p1[1]) - join_pad,
                     max(p0[0], p1[0]) + join_pad,
                     max(p0[1], p1[1]) + join_pad),
                    "diagonal SVG line", element_id, stroke,
                    stroke_ambiguous)
        elif tag == "image":
            x, y = attr_float(element, "x"), attr_float(element, "y")
            w, h = attr_float(element, "width"), attr_float(element, "height")
            if w < 0 or h < 0:
                add_unsupported(
                    (0.0, 0.0, width, height),
                    "negative SVG image extent", element_id)
                return
            if w > 0 and h > 0:
                x0, y0, x1, y1 = transformed_bbox(
                    [(x, y), (x + w, y), (x + w, y + h), (x, y + h)],
                    matrix)
                add_unsupported(
                    (x0, y0, x1, y1), "embedded raster image", element_id)
        elif tag == "use":
            href = element.get(xlink_href) or element.get("href") or ""
            referenced = definitions.get(href.removeprefix("#"))
            if referenced is None:
                add_unsupported(
                    (0.0, 0.0, width, height),
                    f"unresolved SVG use reference: {href}", element_id)
            elif href.startswith("#glyph-"):
                referenced_tag = referenced.tag.rsplit("}", 1)[-1]
                if (referenced_tag == "symbol"
                        and (referenced.get("viewBox") is not None
                             or element.get("width") is not None
                             or element.get("height") is not None)):
                    add_unsupported(
                        (0.0, 0.0, width, height),
                        f"glyph symbol viewport is not resolved: {href}",
                        element_id)
                else:
                    glyph_matrix = matrix.then(Matrix(
                        e=attr_float(element, "x"),
                        f=attr_float(element, "y")))
                    add_glyph_reference(
                        referenced, glyph_matrix, style, clipped_here,
                        element_id, href)
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
                add_unsupported(
                    (x0, y0, x1, y1),
                    f"embedded raster use: {href}", element_id)
            else:
                add_unsupported(
                    (0.0, 0.0, width, height),
                    f"unsupported SVG use reference: {href}", element_id)
        elif tag in ("circle", "ellipse", "polygon", "polyline"):
            points: list[tuple[float, float]] = []
            shape_box: tuple[float, float, float, float] | None = None
            if tag == "circle":
                cx, cy = attr_float(element, "cx"), attr_float(element, "cy")
                rx = ry = attr_float(element, "r")
                if rx < 0:
                    add_unsupported(
                        (0.0, 0.0, width, height),
                        "negative SVG circle radius", element_id)
                    return
                shape_box = transformed_ellipse_bbox(
                    cx, cy, rx, ry, matrix)
            elif tag == "ellipse":
                cx, cy = attr_float(element, "cx"), attr_float(element, "cy")
                rx, ry = attr_float(element, "rx"), attr_float(element, "ry")
                if rx < 0 or ry < 0:
                    add_unsupported(
                        (0.0, 0.0, width, height),
                        "negative SVG ellipse radius", element_id)
                    return
                shape_box = transformed_ellipse_bbox(
                    cx, cy, rx, ry, matrix)
            else:
                values = [float(value) for value in re.findall(
                    _NUMBER, element.get("points", ""))]
                points = list(zip(values[0::2], values[1::2]))
                if points:
                    shape_box = transformed_bbox(points, matrix)
            shape_fill = effective_tone(style, "fill")
            shape_stroke = effective_tone(style, "stroke")
            if (shape_box is not None
                    and (shape_fill is not None or shape_stroke is not None)):
                x0, y0, x1, y1 = shape_box
                _width, shape_pad, shape_ambiguous, _cap = stroke_metrics(
                    style, matrix)
                pad = shape_pad if shape_stroke is not None else 0.0
                add_unsupported(
                    (x0 - pad, y0 - pad, x1 + pad, y1 + pad),
                    f"unsupported SVG {tag}", element_id,
                    shape_stroke if shape_stroke is not None else shape_fill,
                    clipped_here or shape_ambiguous)
        elif tag not in (
                "svg", "g", "a", "switch", "metadata", "title", "desc",
                "symbol", "clipPath", "mask"):
            add_unsupported(
                (0.0, 0.0, width, height),
                f"unsupported SVG element: {tag}", element_id)

        # Glyph uses are text, not vector compartment geometry. Other uses and
        # images were recorded above as unsupported visible regions.
        if tag not in ("use", "image", "symbol", "clipPath", "mask"):
            for child in element:
                walk(child, matrix, style, in_defs, clipped_here)

    for stylesheet in (
            element for element in root.iter()
            if element.tag.rsplit("}", 1)[-1] == "style"):
        add_unsupported(
            (0.0, 0.0, width, height),
            "embedded SVG stylesheet is not resolved",
            stylesheet.get("id") or "style",
        )

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
    def __init__(self, require_runtime_contract: bool = True) -> None:
        super().__init__(convert_charrefs=True)
        self.require_runtime_contract = require_runtime_contract
        self.document_contract_checked = False
        self.template_depth = 0
        self.doctype_count = 0
        self.doctype_valid = True
        self.element_stack: list[tuple[str, bool, str]] = []
        self.div_stack: list[
            tuple[str | None, int | None, int | None]
        ] = []
        self.physical_slots: dict[str, list[int]] = {}
        self.editable_slots: dict[str, list[int]] = {}
        self.slot_geometry: dict[str, list[dict[str, float | int]]] = {}
        self.comb_geometry: dict[str, tuple[float, float]] = {}
        self.comb_position: dict[str, tuple[float, float]] = {}
        self.comb_page: dict[str, int] = {}
        self.declared_slots: dict[str, tuple[int, int]] = {}
        self.invalid_bindings: list[str] = []
        self.comb_containers: set[str] = set()
        self.root: dict[str, str | None] | None = None
        self.pages: list[int] = []
        self.page_geometry: list[tuple[int, float, float]] = []
        self.style_depth = 0
        self.style_count = 0
        self.style_parts: list[str] = []
        self.stylesheet_structural_declarations: set[
            tuple[str, str, str]
        ] = set()
        self.stylesheet_page_sizes: list[tuple[float, float]] = []
        self.script_depth = 0
        self.script_attrs: tuple[tuple[str, str | None], ...] | None = None
        self.script_parts: list[str] = []
        self.runtime_script_hashes: list[str] = []
        self.band_data_scripts = 0

    def handle_starttag(self, tag: str,
                        attrs: list[tuple[str, str | None]]) -> None:
        self._validate_global_tag(tag, attrs)
        values = dict(attrs)
        if tag == "template":
            self.template_depth += 1
            return
        if self.template_depth:
            return
        render_safe = self._push_render_element(tag, attrs)
        if tag == "style":
            self.style_count += 1
            if self.style_depth:
                self.invalid_bindings.append("HTML has a nested style element")
            if values:
                self.invalid_bindings.append(
                    "HTML style element has unsupported attributes")
            self.style_depth += 1
            self.style_parts = []
        if tag == "script":
            if self.script_depth:
                self.invalid_bindings.append("HTML has a nested script element")
            self.script_depth += 1
            self.script_attrs = tuple(sorted(attrs))
            self.script_parts = []
        if tag == "meta" and values != {"charset": "utf-8"}:
            self.invalid_bindings.append(
                "HTML has unsupported meta directives")
        if tag in ("base", "embed", "iframe", "link", "noscript", "object"):
            self.invalid_bindings.append(
                f"HTML has unsupported rendering element: {tag}")
        if tag == "html":
            if self.root is not None:
                raise RefereeError("HTML has more than one root element")
            self.root = values
            return
        if tag == "div":
            parent_cell, _parent_slot, parent_page = (
                self.div_stack[-1]
                if self.div_stack else (None, None, None)
            )
            identifier = values.get("id") or ""
            cell = parent_cell
            slot_index: int | None = None
            page_index = parent_page
            page_match = _PAGE_RE.fullmatch(identifier)
            if page_match and "page" in (values.get("class") or "").split():
                page_index = int(page_match.group(1))
                self.pages.append(page_index)
                style = values.get("style") or ""
                geometry = self._geometry_style(
                    style, ("width", "height"))
                if geometry is None:
                    raise RefereeError(
                        f"HTML page {page_index} has non-canonical geometry")
                self.page_geometry.append((
                    page_index, geometry["width"], geometry["height"],
                ))
            if (_CELL_RE.fullmatch(identifier)
                    and values.get("data-field-kind") == "comb"):
                cell = identifier
                if not render_safe:
                    self.invalid_bindings.append(
                        f"comb is outside rendered layout: {identifier}")
                if identifier in self.comb_containers:
                    self.invalid_bindings.append(
                        f"duplicate comb container: {identifier}")
                self.comb_containers.add(identifier)
                if values.get("data-field-name") != identifier:
                    self.invalid_bindings.append(
                        f"comb field binding disagrees: {identifier}")
                style = values.get("style") or ""
                geometry = self._geometry_style(
                    style, ("left", "top", "width", "height"))
                if geometry is None:
                    self.invalid_bindings.append(
                        f"comb geometry is non-canonical: {identifier}")
                else:
                    left, top = geometry["left"], geometry["top"]
                    width, height = geometry["width"], geometry["height"]
                    self.comb_geometry[identifier] = (width, height)
                    self.comb_position[identifier] = (left, top)
                cell_page_match = _CELL_PAGE_RE.fullmatch(identifier)
                expected_page = (
                    int(cell_page_match.group(1))
                    if cell_page_match is not None else None
                )
                if page_index is None or expected_page != page_index:
                    self.invalid_bindings.append(
                        f"comb page binding disagrees: {identifier}")
                else:
                    self.comb_page[identifier] = page_index
                try:
                    declared_capacity = int(
                        values.get("data-comb-capacity") or "")
                    declared_count = int(values.get("data-comb-slots") or "")
                except ValueError:
                    declared_capacity = declared_count = -1
                self.declared_slots[identifier] = (
                    declared_capacity, declared_count)
            slot = values.get("data-slot")
            if cell and slot is not None and "s" in (
                    values.get("class") or "").split():
                if not render_safe:
                    self.invalid_bindings.append(
                        f"slot is outside rendered layout: {cell}-s{slot}")
                try:
                    slot_index = int(slot)
                except ValueError:
                    slot_index = -1
                self.physical_slots.setdefault(cell, []).append(slot_index)
                style = values.get("style") or ""
                geometry = self._geometry_style(
                    style, ("left", "top", "width", "height"))
                if geometry is None:
                    self.invalid_bindings.append(
                        f"slot geometry is non-canonical: {cell}-s{slot_index}")
                else:
                    self.slot_geometry.setdefault(cell, []).append({
                        "index": slot_index,
                        **geometry,
                    })
            self.div_stack.append((cell, slot_index, page_index))
            return
        if tag != "input":
            return
        slot = values.get("data-slot-index")
        identifier = values.get("id") or ""
        match = _CELL_SLOT_RE.fullmatch(identifier)
        if slot is None or match is None:
            return
        if not render_safe:
            self.invalid_bindings.append(
                f"editable input is outside rendered layout: {identifier}")
        try:
            index = int(slot)
        except ValueError:
            index = -1
        if index != int(match.group(2)):
            index = -1
        parent_cell, parent_slot, _parent_page = (
            self.div_stack[-1]
            if self.div_stack else (None, None, None))
        if parent_cell != match.group(1) or parent_slot != index:
            self.invalid_bindings.append(
                f"editable input is outside its physical slot: {identifier}")
            index = -1
        self.editable_slots.setdefault(match.group(1), []).append(index)

    def handle_decl(self, decl: str) -> None:
        self.doctype_count += 1
        if decl.strip().lower() != "doctype html":
            self.doctype_valid = False

    def handle_endtag(self, tag: str) -> None:
        if self.template_depth:
            if tag == "template":
                self.template_depth -= 1
            return
        if tag == "style":
            if not self.style_depth:
                raise RefereeError("HTML has an unmatched closing style")
            self.style_depth -= 1
            self._validate_stylesheet("".join(self.style_parts))
            self.style_parts = []
        if tag == "script":
            if not self.script_depth:
                raise RefereeError("HTML has an unmatched closing script")
            self.script_depth -= 1
            self._validate_script(
                self.script_attrs, "".join(self.script_parts))
            self.script_attrs = None
            self.script_parts = []
        if tag == "div":
            if not self.div_stack:
                raise RefereeError("HTML has an unmatched closing div")
            self.div_stack.pop()
        if tag not in HTML_VOID_ELEMENTS:
            if not self.element_stack or self.element_stack[-1][0] != tag:
                raise RefereeError(
                    f"HTML has an unmatched closing element: {tag}")
            self.element_stack.pop()

    def handle_data(self, data: str) -> None:
        if self.style_depth and not self.template_depth:
            self.style_parts.append(data)
        if self.script_depth and not self.template_depth:
            self.script_parts.append(data)

    def _validate_global_tag(
            self, tag: str,
            attrs: Sequence[tuple[str, str | None]]) -> None:
        names = [name for name, _value in attrs]
        if len(names) != len(set(names)):
            self.invalid_bindings.append(
                f"HTML {tag} element has duplicate attributes")
        if any(name.startswith("on") for name in names):
            self.invalid_bindings.append(
                f"HTML {tag} element has executable event attributes")
        if tag not in HTML_ALLOWED_TAGS:
            self.invalid_bindings.append(
                f"HTML has an unsupported emitter element: {tag}")
            return
        values = dict(attrs)
        keys = set(values)
        valid = True
        if tag in ("body", "head", "style", "title"):
            valid = not values
        elif tag == "html":
            valid = keys <= HTML_ROOT_ATTRIBUTES
        elif tag == "meta":
            valid = values == {"charset": "utf-8"}
        elif tag == "a":
            valid = (
                keys == {"class", "href"}
                and values.get("class") == "doc-link"
            )
        elif tag == "script":
            valid = (
                not values
                or values == {
                    "id": "formgen-bands",
                    "type": "application/json",
                }
            )
        elif tag == "template":
            valid = keys == {
                "data-band", "data-band-index", "data-capacity",
                "data-row-pitch", "data-row-y", "data-template-row", "id",
            }
        elif tag == "svg":
            valid = keys == {
                "class", "preserveaspectratio", "style", "viewbox", "xmlns",
            }
        elif tag == "g":
            valid = keys in ({"class"}, {"class", "id"})
        elif tag == "rect":
            valid = keys in (
                {"fill", "height", "width", "x", "y"},
                {"data-rule-id", "fill", "height", "width", "x", "y"},
            )
        elif tag == "path":
            valid = keys in (
                {"d", "data-path-id", "fill"},
                {"d", "data-path-id", "fill", "fill-rule"},
                {"d", "data-path-id", "fill", "stroke", "stroke-width"},
                {
                    "d", "data-path-id", "fill", "fill-rule",
                    "stroke", "stroke-width",
                },
            )
        elif tag == "image":
            valid = keys == {
                "data-sha256", "height", "href", "preserveaspectratio",
                "transform", "width", "x", "y",
            }
        elif tag == "input":
            valid = keys in (
                HTML_INPUT_ATTRIBUTES,
                HTML_INPUT_ATTRIBUTES - {"id", "name"},
                {
                    "autocomplete", "class", "id", "name", "spellcheck",
                    "style", "type",
                },
                {
                    "autocomplete", "class", "spellcheck", "style", "type",
                },
            )
        if not valid:
            self.invalid_bindings.append(
                f"HTML {tag} element is outside the emitter grammar")

    @staticmethod
    def _element_role(tag: str, values: dict[str, str | None]) -> str:
        classes = set((values.get("class") or "").split())
        if tag in ("html", "body"):
            return tag
        if tag == "div" and "page" in classes:
            return "page"
        if tag == "div" and classes == {"layer-cells"}:
            return "cells"
        if tag == "div" and classes == {"layer-text"}:
            return "text-layer"
        if tag == "div" and classes == {"band"}:
            return "band"
        if tag == "div" and values.get("data-field-kind") == "comb":
            return "comb"
        if tag == "div" and classes == {"s"} and "data-slot" in values:
            return "slot"
        if tag == "div" and classes == {"t"}:
            return "text"
        if tag == "div" and classes in ({"c"}, {"c", "f"}):
            return "cell"
        return "other"

    @staticmethod
    def _emitter_attributes_valid(
            role: str, values: dict[str, str | None]) -> bool:
        keys = set(values)
        if role == "html":
            return keys <= HTML_ROOT_ATTRIBUTES
        if role == "body":
            return not values
        if role == "page":
            identifier = values.get("id") or ""
            match = _PAGE_RE.fullmatch(identifier)
            return (
                keys == {"class", "id", "style"}
                and match is not None
                and set((values.get("class") or "").split())
                == {"page", f"page-{match.group(1)}"}
            )
        if role == "cells":
            return values == {"class": "layer-cells"}
        if role == "text-layer":
            return values == {"class": "layer-text"}
        if role == "band":
            return (
                keys == HTML_BAND_ATTRIBUTES
                and values.get("class") == "band"
            )
        if role == "comb":
            return (
                keys in (
                    HTML_COMB_ATTRIBUTES,
                    HTML_COMB_ATTRIBUTES | {"data-rectangular"},
                )
                and values.get("class") == "c f"
                and values.get("data-field-kind") == "comb"
            )
        if role == "slot":
            return (
                keys == {"class", "data-slot", "style"}
                and values.get("class") == "s"
            )
        if role == "text":
            return (
                values.get("class") == "t"
                and keys in (
                    {"class", "id", "style"},
                    {"class", "data-unresolved", "id", "style"},
                    {"class", "data-band-row", "id", "style"},
                    {
                        "class", "data-band-row", "data-unresolved",
                        "id", "style",
                    },
                )
            )
        if role == "cell":
            base = {
                "class", "data-cell-kind", "data-col", "data-row",
                "id", "style",
            }
            field = base | {"data-field-kind", "data-field-name"}
            return keys in (
                base,
                base | {"data-preprinted"},
                base | {"data-rectangular"},
                field,
                field | {"data-rectangular"},
            )
        return True

    def _push_render_element(
            self, tag: str, attrs: Sequence[tuple[str, str | None]]) -> bool:
        parent_safe = self.element_stack[-1][1] if self.element_stack else True
        values = dict(attrs)
        role = self._element_role(tag, values)
        render_safe = parent_safe
        if tag == "div" and role == "other":
            self.invalid_bindings.append(
                "HTML has an unsupported div outside the emitter grammar")
            render_safe = False
        if not self._emitter_attributes_valid(role, values):
            render_safe = False
        parent_roles = [item[2] for item in self.element_stack]
        if role == "comb" and parent_roles not in (
                ["html", "body", "page", "cells"],
                ["html", "body", "page", "band"]):
            render_safe = False
        if role in ("cells", "text-layer", "band") and parent_roles != [
                "html", "body", "page"]:
            render_safe = False
        if role == "page" and parent_roles != ["html", "body"]:
            render_safe = False
        if role == "text" and (
                not parent_roles
                or parent_roles[-1] not in ("text-layer", "band")):
            render_safe = False
        if role == "cell" and (
                not parent_roles
                or parent_roles[-1] not in ("cells", "band")):
            render_safe = False
        if role == "slot" and (
                not parent_roles or parent_roles[-1] != "comb"):
            render_safe = False
        if tag == "input" and parent_roles and parent_roles[-1] == "slot":
            if (set(values) != HTML_INPUT_ATTRIBUTES
                    or values.get("type") != "text"
                    or values.get("maxlength") != "1"
                    or values.get("autocomplete") != "off"
                    or values.get("spellcheck") != "false"
                    or re.fullmatch(
                        r"fi fh\d+ fc", values.get("class") or "") is None):
                render_safe = False
        if "hidden" in values:
            render_safe = False
        if tag in ("details", "dialog") and "open" not in values:
            render_safe = False
        raw_style = values.get("style") or ""
        if ("/*" in raw_style or "*/" in raw_style
                or re.search(r"!\s*important\b", raw_style,
                             flags=re.IGNORECASE)):
            render_safe = False
        declarations: set[str] = set()
        for raw in raw_style.split(";"):
            raw = raw.strip()
            if not raw:
                continue
            if ":" not in raw:
                render_safe = False
                continue
            key = raw.split(":", 1)[0].strip().lower()
            if not key or key in declarations:
                render_safe = False
                continue
            declarations.add(key)
            if key in HTML_RENDER_AFFECTING_INLINE_PROPERTIES:
                render_safe = False
        if tag not in HTML_VOID_ELEMENTS:
            self.element_stack.append((tag, render_safe, role))
        return render_safe

    @staticmethod
    def _css_leaf_blocks(css: str) -> list[tuple[str, str]]:
        """Return qualified leaf rules while respecting strings and nesting."""
        if "/*" in css or "*/" in css:
            raise RefereeError("HTML stylesheet comments are unsupported")

        def matching_brace(start: int, end: int) -> int:
            depth = 1
            quote: str | None = None
            escaped = False
            index = start + 1
            while index < end:
                char = css[index]
                if quote is not None:
                    if escaped:
                        escaped = False
                    elif char == "\\":
                        escaped = True
                    elif char == quote:
                        quote = None
                elif char in ("'", '"'):
                    quote = char
                elif char == "{":
                    depth += 1
                elif char == "}":
                    depth -= 1
                    if depth == 0:
                        return index
                index += 1
            raise RefereeError("HTML stylesheet has unbalanced braces")

        def parse_range(start: int, end: int) -> list[tuple[str, str]]:
            blocks: list[tuple[str, str]] = []
            cursor = start
            while cursor < end:
                open_brace = css.find("{", cursor, end)
                if open_brace < 0:
                    if "}" in css[cursor:end]:
                        raise RefereeError(
                            "HTML stylesheet has an unmatched closing brace")
                    break
                header = css[cursor:open_brace].strip()
                if ";" in header:
                    header = header.rsplit(";", 1)[-1].strip()
                close_brace = matching_brace(open_brace, end)
                body = css[open_brace + 1:close_brace]
                nested = parse_range(open_brace + 1, close_brace)
                if nested:
                    blocks.extend(nested)
                else:
                    if not header:
                        raise RefereeError(
                            "HTML stylesheet has a rule without a selector")
                    blocks.append((header, body))
                cursor = close_brace + 1
            return blocks

        return parse_range(0, len(css))

    def _validate_stylesheet(self, css: str) -> None:
        if re.search(r"!\s*important\b", css, flags=re.IGNORECASE):
            self.invalid_bindings.append(
                "HTML stylesheet uses unsupported !important")
            return
        if re.search(
                r"@(charset|container|import|layer|namespace|scope|supports)"
                r"\b",
                css,
                flags=re.IGNORECASE):
            self.invalid_bindings.append(
                "HTML stylesheet uses unsupported conditional or statement "
                "at-rules")
            return
        media_conditions = re.findall(
            r"@media\s+([^{}]+)\{", css, flags=re.IGNORECASE)
        if any(condition.strip().lower() not in ("print", "screen")
               for condition in media_conditions):
            self.invalid_bindings.append(
                "HTML stylesheet uses an unsupported media condition")
            return
        for selector, body in self._css_leaf_blocks(css):
            normalized_selector = re.sub(r"\s+", " ", selector.strip())
            if "#" in normalized_selector or "[" in normalized_selector:
                self.invalid_bindings.append(
                    "HTML stylesheet uses an unsupported structural selector")
                continue
            declarations: set[str] = set()
            for raw in body.split(";"):
                raw = raw.strip()
                if not raw:
                    continue
                if ":" not in raw:
                    self.invalid_bindings.append(
                        "HTML stylesheet has a malformed declaration")
                    continue
                key, value = raw.split(":", 1)
                key = key.strip().lower()
                value = re.sub(r"\s+", " ", value.strip().lower())
                if not key or key in declarations:
                    self.invalid_bindings.append(
                        "HTML stylesheet has a duplicate declaration")
                    continue
                declarations.add(key)
                if not self._stylesheet_declaration_allowed(
                        normalized_selector, key, value):
                    self.invalid_bindings.append(
                        "HTML stylesheet is outside the emitter grammar: "
                        + f"{normalized_selector} {{{key}:{value}}}")
                    continue
                declaration = (normalized_selector, key, value)
                if normalized_selector == "@page" and key == "size":
                    size = re.fullmatch(
                        rf"({_NUMBER})pt ({_NUMBER})pt", value)
                    assert size is not None
                    self.stylesheet_page_sizes.append((
                        float(size.group(1)), float(size.group(2))))
                if declaration in HTML_STYLESHEET_STRUCTURAL_DECLARATIONS:
                    self.stylesheet_structural_declarations.add(declaration)

    @staticmethod
    def _stylesheet_declaration_allowed(
            selector: str, key: str, value: str) -> bool:
        fixed = HTML_STYLESHEET_FIXED_VALUES.get((selector, key))
        if fixed is not None:
            return value in fixed
        if re.fullmatch(r"\.fh\d+", selector):
            if key not in ("font-size", "letter-spacing", "line-height"):
                return False
            match = re.fullmatch(rf"({_NUMBER})pt", value)
            if match is None:
                return False
            number = float(match.group(1))
            return math.isfinite(number) and (
                key == "letter-spacing" or number > 0)
        if selector == ".fi" and key == "word-spacing":
            match = re.fullmatch(rf"({_NUMBER})pt", value)
            return match is not None and math.isfinite(float(match.group(1)))
        if selector == "@page" and key == "size":
            values = re.fullmatch(
                rf"({_NUMBER})pt ({_NUMBER})pt", value)
            return (
                values is not None
                and float(values.group(1)) > 0
                and float(values.group(2)) > 0
            )
        return False

    def _validate_script(
            self,
            attrs: tuple[tuple[str, str | None], ...] | None,
            body: str,
            ) -> None:
        if attrs == (
                ("id", "formgen-bands"),
                ("type", "application/json"),
                ):
            self.band_data_scripts += 1
            try:
                bands = json.loads(body)
            except json.JSONDecodeError:
                self.invalid_bindings.append(
                    "HTML band data script is malformed")
                return
            if not isinstance(bands, list):
                self.invalid_bindings.append(
                    "HTML band data script is not a list")
            return
        if attrs:
            self.invalid_bindings.append(
                "HTML has an unsupported executable script")
            return
        self.runtime_script_hashes.append(
            sha256_bytes(body.encode("utf-8")))

    @staticmethod
    def _geometry_style(
            style: str, required: Sequence[str]
            ) -> dict[str, float] | None:
        """Parse the complete inline geometry grammar, rejecting overrides."""
        declarations: dict[str, str] = {}
        for raw in style.split(";"):
            raw = raw.strip()
            if not raw:
                continue
            if ":" not in raw:
                return None
            key, value = raw.split(":", 1)
            key = key.strip().lower()
            if not key or key in declarations:
                return None
            declarations[key] = value.strip()
        if set(declarations) != set(required):
            return None
        result: dict[str, float] = {}
        for name in required:
            match = re.fullmatch(rf"\s*({_NUMBER})pt\s*",
                                 declarations[name])
            if match is None:
                return None
            value = float(match.group(1))
            if not math.isfinite(value):
                return None
            result[name] = value
        return result


def emitted_slots(path: pathlib.Path) -> dict[str, dict[str, Any]]:
    parser = SlotParser()
    parser.feed(path.read_text(encoding="utf-8"))
    parser.close()
    if (parser.template_depth or parser.div_stack or parser.element_stack
            or parser.style_depth or parser.script_depth):
        raise RefereeError("HTML ended with unclosed structural elements")
    return slot_records(parser)


def slot_records(
        parser: SlotParser,
        expected: dict[str, dict[str, Any]] | None = None,
        ) -> dict[str, dict[str, Any]]:
    result = {}
    if parser.require_runtime_contract and not parser.document_contract_checked:
        if parser.doctype_count != 1 or not parser.doctype_valid:
            parser.invalid_bindings.append(
                "HTML is not bound to one standards-mode doctype")
        if parser.style_count != 1:
            parser.invalid_bindings.append(
                f"HTML has {parser.style_count} document stylesheets, expected 1")
        if parser.band_data_scripts != 1:
            parser.invalid_bindings.append(
                "HTML has no unique formgen band-data script")
        if tuple(parser.runtime_script_hashes) != HTML_RUNTIME_SCRIPT_SHA256:
            parser.invalid_bindings.append(
                "HTML runtime scripts disagree with the reviewed emitter")
        page_sizes = {
            (width, height)
            for _index, width, height in parser.page_geometry
        }
        if (len(parser.stylesheet_page_sizes) != 1
                or len(page_sizes) != 1
                or parser.stylesheet_page_sizes[0] != next(
                    iter(page_sizes), None)):
            parser.invalid_bindings.append(
                "HTML @page size disagrees with emitted page geometry")
        parser.document_contract_checked = True
    missing_stylesheet_contract = (
        HTML_REQUIRED_STYLESHEET_DECLARATIONS
        - parser.stylesheet_structural_declarations
    )
    if missing_stylesheet_contract:
        parser.invalid_bindings.append(
            "HTML stylesheet is missing required structural declarations: "
            + ", ".join(
                f"{selector} {{{key}:{value}}}"
                for selector, key, value in sorted(missing_stylesheet_contract)
            ))
    page_bounds = {
        index: (width, height)
        for index, width, height in parser.page_geometry
    }
    for cell in sorted(parser.comb_containers):
        physical = parser.physical_slots.get(cell, [])
        ordered = sorted(physical)
        editable = sorted(parser.editable_slots.get(cell, ()))
        physical_set = set(physical)
        geometry = sorted(
            parser.slot_geometry.get(cell, ()),
            key=lambda item: int(item["index"]),
        )
        container = parser.comb_geometry.get(cell)
        container_position = parser.comb_position.get(cell)
        page_index = parser.comb_page.get(cell)
        page = page_bounds.get(page_index) if page_index is not None else None
        container_on_page = (
            container is not None
            and container_position is not None
            and page is not None
            and float(container_position[0]) >= -HTML_GEOMETRY_EPSILON_PT
            and float(container_position[1]) >= -HTML_GEOMETRY_EPSILON_PT
            and float(container_position[0]) + float(container[0])
            <= float(page[0]) + HTML_GEOMETRY_EPSILON_PT
            and float(container_position[1]) + float(container[1])
            <= float(page[1]) + HTML_GEOMETRY_EPSILON_PT
        )
        declared_capacity, declared_count = parser.declared_slots.get(
            cell, (-1, -1))
        expected_cell = expected.get(cell) if expected is not None else None
        expected_geometry = (
            expected_cell.get("slots")
            if isinstance(expected_cell, dict) else None
        )
        layout_binding_valid = (
            isinstance(expected_cell, dict)
            and page_index == expected_cell.get("page_index")
            and container_position is not None
            and container is not None
            and all(
                abs(actual - target) <= HTML_GEOMETRY_EPSILON_PT
                for actual, target in zip(
                    (*container_position, *container),
                    (
                        float(expected_cell["left"]),
                        float(expected_cell["top"]),
                        float(expected_cell["width"]),
                        float(expected_cell["height"]),
                    ),
                )
            )
            and isinstance(expected_geometry, list)
            and len(geometry) == len(expected_geometry)
            and all(
                int(actual["index"]) == int(target["index"])
                and all(
                    abs(float(actual[name]) - float(target[name]))
                    <= HTML_GEOMETRY_EPSILON_PT
                    for name in ("left", "top", "width", "height")
                )
                for actual, target in zip(geometry, expected_geometry)
            )
        )
        geometry_valid = (
            container is not None
            and container_on_page
            and float(container[0]) > 0
            and float(container[1]) > 0
            and bool(geometry)
            and len(geometry) == len(physical)
            and [int(item["index"]) for item in geometry] == ordered
            and all(
                float(item["width"]) > 0
                and float(item["height"]) > 0
                and float(item["left"]) >= -HTML_GEOMETRY_EPSILON_PT
                and float(item["left"]) + float(item["width"])
                <= float(container[0]) + HTML_GEOMETRY_EPSILON_PT
                and max(0.0, float(item["top"]))
                < min(float(container[1]),
                      float(item["top"]) + float(item["height"]))
                for item in geometry
            )
        )
        if geometry_valid and geometry:
            geometry_valid = (
                abs(float(geometry[0]["left"]))
                <= HTML_GEOMETRY_EPSILON_PT
                and abs(
                    float(geometry[-1]["left"])
                    + float(geometry[-1]["width"])
                    - float(container[0])
                ) <= HTML_GEOMETRY_EPSILON_PT
                and all(
                    abs(
                        float(right["left"])
                        - (float(left["left"]) + float(left["width"]))
                    ) <= HTML_GEOMETRY_EPSILON_PT
                    and abs(
                        max(0.0, float(right["top"]))
                        - max(0.0, float(left["top"]))
                    )
                    <= HTML_GEOMETRY_EPSILON_PT
                    and abs(
                        min(float(container[1]),
                            float(right["top"]) + float(right["height"]))
                        - min(float(container[1]),
                              float(left["top"]) + float(left["height"]))
                    )
                    <= HTML_GEOMETRY_EPSILON_PT
                    for left, right in zip(geometry, geometry[1:])
                )
            )
        result[cell] = {
            "count": len(physical),
            "indexes": ordered,
            "editable_indexes": editable,
            "declared_capacity": declared_capacity,
            "declared_count": declared_count,
            "page_index": page_index,
            "container_position": (
                list(container_position)
                if container_position is not None else None
            ),
            "container_geometry": (
                list(container) if container is not None else None
            ),
            "layout_binding_valid": layout_binding_valid,
            "expected_geometry": expected_cell,
            "slot_geometry": geometry,
            "valid": (
                len(physical) == len(set(physical))
                and -1 not in physical
                and ordered == list(range(len(physical)))
                and declared_capacity == len(physical)
                and declared_count == len(physical)
                and geometry_valid
                and layout_binding_valid
                and len(editable) == len(set(editable))
                and -1 not in editable
                and all(index in physical_set for index in editable)
                and not parser.invalid_bindings
            ),
        }
    return result


def relocated_cells(data: dict[str, Any]) -> set[str]:
    cells: set[str] = set()
    for region in data.get("inline") or ():
        cells.update(region.get("cell_ids") or ())
    return cells


def emitted_geometry_contract(
        layout: dict[str, Any], guide: dict[str, Any]
        ) -> dict[str, dict[str, Any]]:
    """Build exact main-form comb geometry, including guide cut straddlers."""
    relocated = relocated_cells(guide)
    clipped_form_boxes: dict[str, dict[str, Any]] = {}
    for region in guide.get("inline") or ():
        for straddler in region.get("straddlers") or ():
            if (straddler.get("kind") != "cell"
                    or straddler.get("disposition") != "clipped"):
                continue
            cell_id = str(straddler.get("ref") or "")
            form_box = straddler.get("form")
            if not _CELL_RE.fullmatch(cell_id) or not isinstance(form_box, dict):
                raise RefereeError("guide has an invalid clipped cell straddler")
            if cell_id in clipped_form_boxes:
                raise RefereeError(
                    f"guide clips one cell more than once: {cell_id}")
            clipped_form_boxes[cell_id] = form_box

    result: dict[str, dict[str, Any]] = {}
    for page in layout.get("pages") or ():
        page_index = int(page["index"])
        for cell in page.get("cells") or ():
            comb = cell.get("comb")
            cell_id = str(cell.get("id") or "")
            if not comb or cell_id in relocated:
                continue
            full_box = {
                name: float(cell[name])
                for name in ("x0", "y0", "x1", "y1")
            }
            box = clipped_form_boxes.get(cell_id, full_box)
            if cell_id in clipped_form_boxes:
                straddler = next(
                    item
                    for region in guide.get("inline") or ()
                    for item in region.get("straddlers") or ()
                    if item.get("kind") == "cell"
                    and item.get("ref") == cell_id
                    and item.get("disposition") == "clipped"
                )
                if any(
                    abs(float(straddler[name]) - full_box[name]) > 1e-8
                    for name in ("x0", "y0", "x1", "y1")
                ):
                    raise RefereeError(
                        f"guide/layout clipped cell provenance disagrees: {cell_id}")
            try:
                box_values = {
                    name: float(box[name])
                    for name in ("x0", "y0", "x1", "y1")
                }
                slot_x = [float(value) for value in comb["slot_x"]]
                comb_y0 = float(comb["y0"])
                comb_y1 = float(comb["y1"])
                count = int(comb["cells"])
            except (KeyError, TypeError, ValueError):
                raise RefereeError(
                    f"layout comb geometry is incomplete: {cell_id}")
            if (len(slot_x) != count + 1
                    or any(right <= left
                           for left, right in zip(slot_x, slot_x[1:]))
                    or box_values["x1"] <= box_values["x0"]
                    or box_values["y1"] <= box_values["y0"]
                    or comb_y1 <= comb_y0):
                raise RefereeError(
                    f"layout comb geometry is invalid: {cell_id}")
            result[cell_id] = {
                "page_index": page_index,
                "left": box_values["x0"],
                "top": box_values["y0"],
                "width": box_values["x1"] - box_values["x0"],
                "height": box_values["y1"] - box_values["y0"],
                "slots": [
                    {
                        "index": index,
                        "left": left - box_values["x0"],
                        "top": comb_y0 - box_values["y0"],
                        "width": right - left,
                        "height": comb_y1 - comb_y0,
                    }
                    for index, (left, right)
                    in enumerate(zip(slot_x, slot_x[1:]))
                ],
            }
    return result


def composited_segments(y: float,
                        all_paints: Sequence[Paint]
                        ) -> list[dict[str, Any]]:
    """Return exact final-tone x components on one open horizontal slab.

    Looking only at paint drawn *after* a candidate is order-dependent: a thin
    black line over a broad earlier black fill and the same line below a broad
    later black fill have identical final pixels.  Partitioning at every paint
    edge and selecting the last owner makes those two cases identical and
    prevents a same-tone background from masquerading as a narrow divider.
    """
    active = [
        paint for paint in all_paints
        if paint.y0 <= y <= paint.y1 and paint.x1 - paint.x0 > 1e-9
    ]
    endpoints = sorted({
        coordinate for paint in active for coordinate in (paint.x0, paint.x1)
    })
    atomic: list[dict[str, Any]] = []
    for left, right in zip(endpoints, endpoints[1:]):
        if right - left <= 1e-9:
            continue
        midpoint = (left + right) / 2
        owners = [
            paint for paint in active
            if paint.x0 < midpoint < paint.x1
        ]
        if not owners:
            continue
        owner = max(owners, key=lambda paint: paint.order)
        atomic.append({
            "x0": left,
            "x1": right,
            "tone": owner.tone,
            "clipped": owner.clipped,
            "elements": {owner.element},
            "orders": {owner.order},
        })
    merged: list[dict[str, Any]] = []
    for segment in atomic:
        if (merged
                and abs(float(merged[-1]["x1"])
                        - float(segment["x0"])) <= 1e-6
                and abs(float(merged[-1]["tone"])
                        - float(segment["tone"])) <= 1e-8):
            merged[-1]["x1"] = segment["x1"]
            merged[-1]["clipped"] = (
                bool(merged[-1]["clipped"]) or bool(segment["clipped"]))
            merged[-1]["elements"].update(segment["elements"])
            merged[-1]["orders"].update(segment["orders"])
        else:
            merged.append({
                **segment,
                "elements": set(segment["elements"]),
                "orders": set(segment["orders"]),
            })
    return merged


def merge_centres(paints: Sequence[Paint], y: float,
                  all_paints: Sequence[Paint],
                  max_width: float) -> list[dict[str, Any]]:
    if not paints:
        return []
    tones = {round(paint.tone, 8) for paint in paints}
    if len(tones) != 1:
        raise RefereeError("divider candidates do not have one bound tone")
    target_tone = next(iter(tones))
    active_candidates = [
        paint for paint in paints if paint.y0 <= y <= paint.y1
    ]
    active_candidate_orders = {paint.order for paint in active_candidates}
    components = composited_segments(y, all_paints)
    ambiguous_occlusion = any(
        bool(component["clipped"])
        and any(
            float(component["x1"]) > paint.x0
            and float(component["x0"]) < paint.x1
            for paint in active_candidates
        )
        for component in components
    )
    groups = [
        component for component in components
        if abs(float(component["tone"]) - target_tone) <= 1e-8
        and float(component["x1"]) - float(component["x0"])
        <= max_width + 1e-6
        and bool(component["orders"])
        and set(component["orders"]).issubset(active_candidate_orders)
    ]
    return [{
        "x": round((float(group["x0"]) + float(group["x1"])) / 2, 6),
        "x0": round(float(group["x0"]), 6),
        "x1": round(float(group["x1"]), 6),
        "tone": round(float(group["tone"]), 8),
        "elements": sorted(group["elements"]),
        "clipped": bool(group["clipped"]) or ambiguous_occlusion,
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
    contract_y0, contract_y1 = float(comb["y0"]), float(comb["y1"])
    if contract_y1 <= contract_y0:
        return {"status": "unevaluable",
                "reason": "comb has no positive source band"}

    # The lattice contract can include the outer half of its closing
    # horizontals.  Normalize to the open compartment band: a vertical ending
    # at the near edge of a baseline is topologically coextensive with one
    # painted through that baseline.  This uses vector geometry and the
    # contract's own anchored run, not a form-specific height tolerance.
    anchor_left = min(anchors) if len(anchors) > 1 else x0
    anchor_right = max(anchors) if len(anchors) > 1 else x1
    contract_height = contract_y1 - contract_y0
    def finally_spans(paint: Paint, y: float,
                      left: float, right: float) -> bool:
        return any(
            abs(float(component["tone"]) - divider_tone) <= 1e-8
            and not bool(component["clipped"])
            and float(component["x0"]) <= left + POSITION_TOL_PT
            and float(component["x1"]) >= right - POSITION_TOL_PT
            for component in composited_segments(y, page.paints)
        )

    closing_rules = [
        paint for paint in page.paints
        if not paint.clipped
        and abs(paint.tone - divider_tone) <= 1e-8
        and paint.width > paint.height
        and paint.height <= min(contract_height / 2, pitch / 2)
        and paint.x0 <= anchor_left + POSITION_TOL_PT
        and paint.x1 >= anchor_right - POSITION_TOL_PT
        and finally_spans(paint, paint.y0 + paint.height / 2,
                          anchor_left, anchor_right)
    ]
    top_edges = [
        paint.y1 for paint in closing_rules
        if paint.y0 <= contract_y0 + POSITION_TOL_PT
        and paint.y1 > contract_y0
    ]
    bottom_edges = [
        paint.y0 for paint in closing_rules
        if paint.y0 < contract_y1
        and paint.y1 >= contract_y1 - POSITION_TOL_PT
    ]
    seed_y0 = max([contract_y0, *top_edges])
    seed_y1 = min([contract_y1, *bottom_edges])
    if seed_y1 - seed_y0 <= POSITION_TOL_PT:
        return {
            "status": "unevaluable",
            "reason": "closing rules leave no measurable open compartment band",
            "contract_y0": round(contract_y0, 6),
            "contract_y1": round(contract_y1, 6),
            "open_y0": round(seed_y0, 6),
            "open_y1": round(seed_y1, 6),
        }
    max_width = pitch / 2
    candidates = [
        paint for paint in page.paints
        if abs(paint.tone - divider_tone) <= 1e-8
        and paint.width <= max_width
        and paint.height > paint.width
        and paint.x1 > x0 and paint.x0 < x1
        and paint.y1 > float(cell["y0"]) and paint.y0 < float(cell["y1"])
    ]
    # Glyphs are never divider candidates, so they matter only when they can
    # occlude an eligible interior vertical.  A glyph whose conservative bound
    # merely touches the cell's own side cannot change compartment topology.
    # Apply the same paper-width test used below for outward source candidates
    # before allowing a glyph bound to make the subject unevaluable.
    interior_candidates = [
        paint for paint in candidates
        if paint.x0 > x0 + POSITION_TOL_PT
        and paint.x1 < x1 - POSITION_TOL_PT
        and paint.x0 - x0 > paint.width
        and x1 - paint.x1 > paint.width
    ]

    # A non-uniform empty gap is safe only when the source independently proves
    # that the whole subject is one physical rectangle.  The certificate is
    # deliberately stronger than "there are lines near four sides": one
    # Poppler element must finally own all four complete target-tone edges.
    # This distinguishes a genuinely irregular enclosed comb from two comb runs
    # that lattice.py accidentally joined across a label or gutter.
    cell_y0, cell_y1 = float(cell["y0"]), float(cell["y1"])

    def final_owner(x: float, y: float) -> Paint | None:
        active = [
            paint for paint in page.paints
            if paint.x0 <= x <= paint.x1 and paint.y0 <= y <= paint.y1
        ]
        return max(
            active,
            key=lambda paint: (paint.order, paint.element, paint.kind),
            default=None,
        )

    def final_target_spans_horizontal(y: float) -> bool:
        endpoints = {x0, x1}
        for paint in page.paints:
            if paint.y0 <= y <= paint.y1 and paint.x1 > x0 and paint.x0 < x1:
                endpoints.update((max(x0, paint.x0), min(x1, paint.x1)))
        ordered = sorted(endpoints)
        for left, right in zip(ordered, ordered[1:]):
            if right - left <= 1e-9:
                continue
            owner = final_owner((left + right) / 2, y)
            if (owner is None or owner.clipped
                    or abs(owner.tone - divider_tone) > 1e-8):
                if ((left <= x0 + 1e-9 or right >= x1 - 1e-9)
                        and right - left <= POSITION_TOL_PT):
                    continue
                return False
        return True

    def final_target_spans_vertical(x: float) -> bool:
        endpoints = {cell_y0, cell_y1}
        for paint in page.paints:
            if (paint.x0 <= x <= paint.x1
                    and paint.y1 > cell_y0 and paint.y0 < cell_y1):
                endpoints.update((
                    max(cell_y0, paint.y0), min(cell_y1, paint.y1)))
        ordered = sorted(endpoints)
        for top, bottom in zip(ordered, ordered[1:]):
            if bottom - top <= 1e-9:
                continue
            owner = final_owner(x, (top + bottom) / 2)
            if (owner is None or owner.clipped
                    or abs(owner.tone - divider_tone) > 1e-8):
                if ((top <= cell_y0 + 1e-9
                     or bottom >= cell_y1 - 1e-9)
                        and bottom - top <= POSITION_TOL_PT):
                    continue
                return False
        return True

    subject_frame_elements_cache: list[str] | None = None

    def single_source_frame_elements() -> list[str]:
        nonlocal subject_frame_elements_cache
        if subject_frame_elements_cache is not None:
            return subject_frame_elements_cache
        paints_by_element: dict[str, list[Paint]] = {}
        for paint in page.paints:
            if (not paint.clipped
                    and abs(paint.tone - divider_tone) <= 1e-8):
                paints_by_element.setdefault(paint.element, []).append(paint)
        subject_frame_elements: list[str] = []
        for element, element_paints in sorted(paints_by_element.items()):
            top_lines = [
                paint for paint in element_paints
                if paint.width > paint.height
                and paint.x0 <= x0 + POSITION_TOL_PT
                and paint.x1 >= x1 - POSITION_TOL_PT
                and paint.y0 <= cell_y0 + POSITION_TOL_PT
                and paint.y1 >= cell_y0 - POSITION_TOL_PT
            ]
            bottom_lines = [
                paint for paint in element_paints
                if paint.width > paint.height
                and paint.x0 <= x0 + POSITION_TOL_PT
                and paint.x1 >= x1 - POSITION_TOL_PT
                and paint.y0 <= cell_y1 + POSITION_TOL_PT
                and paint.y1 >= cell_y1 - POSITION_TOL_PT
            ]
            left_lines = [
                paint for paint in element_paints
                if paint.height > paint.width
                and paint.y0 <= cell_y0 + POSITION_TOL_PT
                and paint.y1 >= cell_y1 - POSITION_TOL_PT
                and paint.x0 <= x0 + POSITION_TOL_PT
                and paint.x1 >= x0 - POSITION_TOL_PT
            ]
            right_lines = [
                paint for paint in element_paints
                if paint.height > paint.width
                and paint.y0 <= cell_y0 + POSITION_TOL_PT
                and paint.y1 >= cell_y1 - POSITION_TOL_PT
                and paint.x0 <= x1 + POSITION_TOL_PT
                and paint.x1 >= x1 - POSITION_TOL_PT
            ]
            if (
                any(final_target_spans_horizontal(
                    (paint.y0 + paint.y1) / 2)
                    for paint in top_lines)
                and any(final_target_spans_horizontal(
                    (paint.y0 + paint.y1) / 2)
                    for paint in bottom_lines)
                and any(final_target_spans_vertical(
                    (paint.x0 + paint.x1) / 2)
                    for paint in left_lines)
                and any(final_target_spans_vertical(
                    (paint.x0 + paint.x1) / 2)
                    for paint in right_lines)
            ):
                subject_frame_elements.append(element)
        subject_frame_elements_cache = subject_frame_elements
        return subject_frame_elements_cache
    ambiguous_target_paints = [
        paint for paint in page.paints
        if paint.clipped
        and abs(paint.tone - divider_tone) <= 1e-8
        and paint.height > paint.width
        and paint.x1 > x0 and paint.x0 < x1
        and paint.y1 > seed_y0 and paint.y0 < seed_y1
    ]
    if ambiguous_target_paints:
        return {
            "status": "unevaluable",
            "reason": "ambiguous target-tone SVG paint intersects the comb band",
            "paints": [dataclasses.asdict(paint)
                       for paint in ambiguous_target_paints],
        }

    def unsupported_affects_comb(region: UnsupportedRegion) -> bool:
        if not (
            region.x1 > x0 and region.x0 < x1
            and region.y1 > seed_y0 and region.y0 < seed_y1
            and region.y1 - region.y0 > 1e-6
            and min(region.y1, seed_y1) - max(region.y0, seed_y0)
            > POSITION_TOL_PT
        ):
            return False
        # Poppler normally emits text as glyph ``use`` nodes, but a few
        # official forms carry outlined characters as broad curved paths.
        # Neither kind is a straight compartment boundary.  A broad curved
        # path can therefore affect topology only by covering or joining an
        # eligible source divider; a narrow curved path remains unsupported
        # because its bound could itself occupy a divider lane.
        curved_overlap = (
            min(region.y1, seed_y1) - max(region.y0, seed_y0)
        )
        curved_can_be_divider = (
            region.reason == "curved SVG path"
            and region.x1 - region.x0 <= max_width
            and curved_overlap > (seed_y1 - seed_y0) / 2
        )
        occlusion_only = (
            "glyph use" in region.reason
            or (
                region.reason == "curved SVG path"
                and not curved_can_be_divider
            )
        )
        if not occlusion_only:
            return True
        # Glyphs are explicitly excluded as divider candidates.  Their only
        # topology effect is possible occlusion of an earlier raw divider.
        # Same-tone glyph paint preserves that ink; a differently toned glyph
        # matters only where its conservative bound crosses an earlier
        # candidate rectangle.
        if (region.tone is not None
                and abs(region.tone - divider_tone) <= 1e-8):
            return any(
                region.x1 > paint.x0 and region.x0 < paint.x1
                and region.y1 > max(seed_y0, paint.y0)
                and region.y0 < min(seed_y1, paint.y1)
                for paint in interior_candidates
            )
        return any(
            (region.order < 0 or region.order > paint.order)
            and region.x1 > paint.x0 and region.x0 < paint.x1
            and region.y1 > max(seed_y0, paint.y0)
            and region.y0 < min(seed_y1, paint.y1)
            for paint in interior_candidates
        )

    intersecting_unsupported = [
        region for region in page.unsupported
        if unsupported_affects_comb(region)
    ]
    if intersecting_unsupported:
        return {
            "status": "unevaluable",
            "reason": "unsupported SVG geometry intersects the comb band",
            "unsupported": [dataclasses.asdict(region)
                            for region in intersecting_unsupported],
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
    ignored_slabs: list[dict[str, Any]] = []
    for a, b in zip(ordered_y, ordered_y[1:]):
        # A thinner y-slab is only coordinate noise at a shared endpoint: it
        # cannot establish a geometrically distinct band under the repository's
        # fixed 0.25pt position tolerance.
        if b - a <= POSITION_TOL_PT or b <= seed_y0 or a >= seed_y1:
            if b > seed_y0 and a < seed_y1:
                ignored_slabs.append({
                    "y0": round(max(a, seed_y0), 6),
                    "y1": round(min(b, seed_y1), 6),
                    "reason": "slab is no wider than the fixed position bound",
                })
            continue
        mid = (a + b) / 2
        groups = merge_centres(candidates, mid, page.paints, max_width)
        if not groups:
            ignored_slabs.append({
                "y0": round(a, 6), "y1": round(b, 6),
                "reason": "no candidate divider ink",
            })
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
            interior_groups = [
                group for group in groups
                if not (
                    (group["x0"] <= x0 + POSITION_TOL_PT
                     and group["x1"] >= x0 - POSITION_TOL_PT)
                    or (group["x0"] <= x1 + POSITION_TOL_PT
                        and group["x1"] >= x1 - POSITION_TOL_PT)
                )
            ]
            record = {
                "y0": round(a, 6), "y1": round(b, 6),
                "source_divider_x": [
                    round(float(group["x"]), 6) for group in groups
                ],
            }
            available_anchors = list(anchors)
            partial_matches: list[dict[str, float]] = []
            for group in interior_groups:
                choices = sorted(
                    (abs(float(group["x"]) - anchor), index)
                    for index, anchor in enumerate(available_anchors)
                    if near(float(group["x"]), anchor)
                )
                if not choices:
                    partial_matches = []
                    break
                _distance, index = choices[0]
                anchor = available_anchors.pop(index)
                partial_matches.append({
                    "layout_x": round(anchor, 6),
                    "source_x": round(float(group["x"]), 6),
                    "delta_pt": round(float(group["x"]) - anchor, 6),
                })
            if (interior_groups
                    and len(partial_matches) == len(interior_groups)
                    and any(group["clipped"] for group in interior_groups)):
                bands.append({
                    "status": "unevaluable",
                    "reason": (
                        "a partial source topology has unresolved clipping"
                    ),
                    **record,
                })
            elif (interior_groups
                  and len(partial_matches) == len(interior_groups)):
                partial_x = sorted(
                    round(float(group["x"]), 6)
                    for group in interior_groups)
                bands.append({
                    "status": "measured",
                    "y0": round(a, 6), "y1": round(b, 6),
                    "source_divider_x": partial_x,
                    "extra_divider_x": [],
                    "compartments": len(partial_x) + 1,
                    "anchor_matches": partial_matches,
                    "anchors_complete": False,
                    "positions_match": False,
                    "components": interior_groups,
                })
            elif interior_groups:
                bands.append({
                    "status": "unevaluable",
                    "reason": (
                        "unrecognised candidate ink exists while an anchor is missing"
                    ),
                    **record,
                })
            else:
                ignored_slabs.append({
                    "reason": (
                        "only cell-edge frames remain when an anchor is absent"
                    ),
                    **record,
                })
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
        subject_gap_proofs: list[dict[str, Any]] = []
        unproven_subject_gaps: list[dict[str, Any]] = []
        source_anchors = [float(match["source_x"]) for match in anchor_matches]
        for gap_index, (left, right) in enumerate(
                zip(source_anchors, source_anchors[1:])):
            gap = right - left
            multiple = int(round(gap / pitch))
            integral_residual = abs(gap - multiple * pitch)
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
                        and integral_residual <= POSITION_TOL_PT
                        and max(source_steps) - min(source_steps)
                        <= POSITION_TOL_PT
                        and all(abs(step - pitch) <= POSITION_TOL_PT
                                for step in source_steps)):
                    extras.extend(between)
                    continue
            if multiple <= 1:
                if between:
                    partial.append({
                        "left": round(left, 6), "right": round(right, 6),
                        "reason": (
                            "unexplained target-tone ink exists inside "
                            "a one-pitch anchor gap"
                        ),
                        "pitch_pt": round(pitch, 6),
                        "found_x": [item["x"] for item in between],
                    })
                continue
            # An empty gap wider than one measured pitch needs the stronger,
            # independently verified single-frame certificate above and must
            # contain no unsupported fixed ink.  Even an integral multi-pitch
            # void could be two separate comb runs joined by a bad lattice
            # subject.
            if not between:
                gap_unsupported = [
                    region for region in page.unsupported
                    if region.x1 > left and region.x0 < right
                    and region.y1 > cell_y0 and region.y0 < cell_y1
                    and min(region.x1, right) - max(region.x0, left)
                    > POSITION_TOL_PT
                    and min(region.y1, cell_y1) - max(region.y0, cell_y0)
                    > POSITION_TOL_PT
                ]
                subject_frame_elements = single_source_frame_elements()
                if subject_frame_elements and not gap_unsupported:
                    subject_gap_proofs.append({
                        "left": round(left, 6),
                        "right": round(right, 6),
                        "gap_pt": round(gap, 6),
                        "pitch_pt": round(pitch, 6),
                        "integral_residual_pt": round(
                            integral_residual, 6),
                        "single_frame_elements": subject_frame_elements,
                        "unsupported_regions": [],
                    })
                    continue
                unproven_subject_gaps.append({
                    "left": round(left, 6), "right": round(right, 6),
                    "reason": (
                        "multi-pitch empty gap lacks a clean single-frame proof"
                    ),
                    "pitch_pt": round(pitch, 6),
                    "gap_pt": round(gap, 6),
                    "integral_residual_pt": round(integral_residual, 6),
                    "single_frame_elements": subject_frame_elements,
                    "unsupported_regions": [
                        dataclasses.asdict(region)
                        for region in gap_unsupported
                    ],
                    "found_x": [],
                })
                continue
            if integral_residual > POSITION_TOL_PT:
                partial.append({
                    "left": round(left, 6), "right": round(right, 6),
                    "reason": "anchor gap is not an integral pitch multiple",
                    "pitch_pt": round(pitch, 6),
                    "residual_pt": round(integral_residual, 6),
                    "found_x": [item["x"] for item in between],
                })
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
                    return False, (
                        "off-pitch source ink blocks outward continuation"
                    )
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
        invalid_source_gaps = []
        extra_values = set(unique_extras)
        source_models = [pitch, *[
            right - left for left, right in zip(source_anchors,
                                                source_anchors[1:])
            if right - left > POSITION_TOL_PT
        ]]
        for left, right in zip(source_x, source_x[1:]):
            if left not in extra_values and right not in extra_values:
                continue
            gap = right - left
            model_results = []
            for model in source_models:
                multiple = max(1, int(round(gap / model)))
                model_results.append((
                    abs(gap - multiple * model), model, multiple))
            residual, model, multiple = min(model_results)
            if residual > POSITION_TOL_PT:
                invalid_source_gaps.append({
                    "left": round(left, 6),
                    "right": round(right, 6),
                    "gap_pt": round(gap, 6),
                    "nearest_model_pt": round(model, 6),
                    "nearest_pitch_multiple": multiple,
                    "residual_pt": round(residual, 6),
                })
        if invalid_source_gaps:
            bands.append({
                "status": "unevaluable",
                "reason": "final source gaps are not integral pitch multiples",
                "y0": round(a, 6), "y1": round(b, 6),
                "pitch_pt": round(pitch, 6),
                "invalid_gaps": invalid_source_gaps,
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
            "anchors_complete": True,
            "subject_gap_proofs": subject_gap_proofs,
            "unproven_subject_gaps": unproven_subject_gaps,
            "components": [group for group in groups
                           if any(near(group["x"], x) for x in source_x)],
        })

    measured = [band for band in bands if band["status"] == "measured"]
    ambiguous = [band for band in bands if band["status"] != "measured"]
    seed_span = seed_y1 - seed_y0
    measured_span = sum(
        float(band["y1"]) - float(band["y0"]) for band in measured)

    def topology(band: dict[str, Any]) -> tuple[float, ...]:
        return tuple(round(float(value), 6)
                     for value in band["source_divider_x"])

    measured_topologies = {topology(band) for band in measured}
    topology_coverage = {
        candidate: sum(
            float(band["y1"]) - float(band["y0"])
            for band in measured if topology(band) == candidate
        )
        for candidate in measured_topologies
    }

    def topology_key(candidate: tuple[float, ...]) -> str:
        return ",".join(str(value) for value in candidate)

    coverage_evidence = {
        "contract_y0": round(contract_y0, 6),
        "contract_y1": round(contract_y1, 6),
        "open_y0": round(seed_y0, 6),
        "open_y1": round(seed_y1, 6),
        "contract_span_pt": round(contract_y1 - contract_y0, 6),
        "seed_span_pt": round(seed_span, 6),
        "measured_span_pt": round(measured_span, 6),
        "unmeasured_span_pt": round(max(0.0, seed_span - measured_span), 6),
        "topology_coverage_pt": {
            topology_key(candidate): round(topology_coverage[candidate], 6)
            for candidate in sorted(measured_topologies)
        },
        "ignored_slabs": ignored_slabs,
    }
    if ambiguous:
        return {
            "status": "unevaluable",
            "reason": "one or more source slabs have ambiguous topology",
            **coverage_evidence,
            "bands": bands,
        }
    if not measured:
        reason = (bands[0]["reason"] if bands else
                  "no common Poppler band contains every recognised divider")
        return {
            "status": "unevaluable", "reason": reason,
            **coverage_evidence, "bands": bands,
        }
    if seed_span <= 0 or measured_span <= seed_span / 2:
        return {
            "status": "unevaluable",
            "reason": (
                "source topology does not occupy a strict majority "
                "of the full comb band"
            ),
            **coverage_evidence,
            "bands": measured,
        }
    topologies = measured_topologies
    topology_reason = "one source topology contains every recognised anchor"
    superset_relations: list[dict[str, Any]] = []
    if len(topologies) == 1:
        chosen_topology = next(iter(topologies))
    else:
        # A thick group separator can be slightly shorter than the hairline
        # seeds beside it.  The y partition then has a narrow seed-only cap and
        # a much taller slab with the complete compartment topology.  That is
        # not competing evidence: the longer separator still visibly divides
        # the comb.  Admit the richer topology only when it contains every
        # divider in every other slab (within the fixed position bound) and
        # occupies a strict majority of the measured vertical band.  A short
        # midpoint or two genuinely competing slabs remains UNEVALUABLE.
        def contains(superset: tuple[float, ...],
                     subset: tuple[float, ...]) -> bool:
            available = list(superset)
            for value in subset:
                choices = sorted(
                    (abs(candidate - value), index)
                    for index, candidate in enumerate(available)
                    if near(candidate, value)
                )
                if not choices:
                    return False
                _distance, index = choices[0]
                available.pop(index)
            return True

        for candidate in sorted(topologies):
            for other in sorted(topologies):
                if candidate == other:
                    continue
                superset_relations.append({
                    "candidate": list(candidate),
                    "other": list(other),
                    "contains": contains(candidate, other),
                    "proper": (
                        len(candidate) > len(other)
                        and contains(candidate, other)
                    ),
                })
        dominant = [
            candidate for candidate in topologies
            if all(
                other == candidate
                or (len(candidate) > len(other)
                    and contains(candidate, other))
                for other in topologies
            )
            and topology_coverage[candidate] > seed_span / 2
        ]
        if len(dominant) != 1:
            return {
                "status": "unevaluable",
                "reason": "source slabs have different divider topology",
                **coverage_evidence,
                "topology_superset_relations": superset_relations,
                "bands": measured,
            }
        chosen_topology = dominant[0]
        topology_reason = (
            "one richer source topology contains every other slab and "
            "occupies a strict majority of the comb band"
        )
    chosen = max(
        (band for band in measured if topology(band) == chosen_topology),
        key=lambda band: (
            float(band["y1"]) - float(band["y0"]),
            -float(band["y0"]),
            -float(band["y1"]),
            tuple(float(value) for value in band["source_divider_x"]),
        ),
    )
    if not bool(chosen.get("anchors_complete")):
        return {
            "status": "unevaluable",
            "reason": "dominant source topology omits recognised anchors",
            **coverage_evidence,
            "chosen_topology": list(chosen_topology),
            "topology_superset_relations": superset_relations,
            "bands": measured,
        }
    if chosen.get("unproven_subject_gaps"):
        return {
            "status": "unevaluable",
            "reason": (
                "chosen source topology lacks a clean single-frame subject proof"
            ),
            **coverage_evidence,
            "chosen_topology": list(chosen_topology),
            "topology_superset_relations": superset_relations,
            "bands": measured,
        }
    return {
        "status": "measured",
        "reason": topology_reason,
        **{key: value for key, value in chosen.items() if key != "status"},
        **coverage_evidence,
        "chosen_topology": list(chosen_topology),
        "topology_superset_relations": superset_relations,
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
    html_structure_sha256 = emitted_structure_sha256(html_bytes)
    if html_structure_sha256 != EXPECTED_HTML_STRUCTURE_SHA256.get(slug):
        raise RefereeError(
            f"{slug}: emitted HTML bytes changed from the reviewed pin")
    if layout_comb_count != expected_combs:
        raise RefereeError(
            f"{slug}: layout has {layout_comb_count} combs, "
            f"expected pinned {expected_combs}")
    html_parser = SlotParser()
    html_parser.feed(html_bytes.decode("utf-8"))
    html_parser.close()
    if (html_parser.template_depth or html_parser.div_stack
            or html_parser.element_stack or html_parser.style_depth
            or html_parser.script_depth):
        raise RefereeError(
            f"{slug}: HTML has unclosed structural elements")
    bind_artifacts(slug, layout, ir, guide, html_parser)
    provenance_path, provenance_bytes = bind_tracked_provenance(slug, layout)
    pdf = source_pdf(layout, args.source_root)
    expected_sha = layout["source"]["sha256"]
    pdf_bytes = pdf.read_bytes()
    actual_sha = sha256_bytes(pdf_bytes)
    if actual_sha != expected_sha:
        raise RefereeError(f"{slug}: source hash changed")

    emission_contract = emitted_geometry_contract(layout, guide)
    slots = slot_records(html_parser, emission_contract)
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
                    "emitted_evidence": emitted,
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
            "html_structure_sha256": html_structure_sha256,
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
        "emission_binding_errors": html_parser.invalid_bindings,
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
    assert Matrix(a=10, d=1).stroke_scale() == 10
    try:
        parse_transform("rotate(0.5turn)")
    except RefereeError:
        pass
    else:
        raise AssertionError("CSS angle units were interpreted as SVG degrees")

    subpaths, unsupported, malformed = path_subpaths(
        "M 1 2 L 3 2 L 3 9 L 1 9 Z")
    assert not unsupported and not malformed
    assert len(subpaths) == 1 and subpaths[0][1]
    _subpaths, unsupported, malformed = path_subpaths(
        "M 0 0 C 1 2 3 4 5 6")
    assert unsupported and not malformed
    assert bbox(unsupported[0]) == (0.0, 0.0, 5.0, 6.0)
    _subpaths, unsupported, malformed = path_subpaths(
        "M 10 10 c 1 2 3 4 5 6 s 2 3 4 5")
    assert unsupported and not malformed
    assert bbox(unsupported[0]) == (10.0, 10.0, 19.0, 21.0)
    _subpaths, unsupported, malformed = path_subpaths(
        "M 10 10 A 5 3 0 0 1 20 10")
    assert unsupported and not malformed
    arc_box = bbox(unsupported[0])
    assert arc_box[0] <= 10 and arc_box[2] >= 20
    assert is_axis_aligned_rectangle([(1, 2), (3, 2), (3, 9), (1, 9)])
    assert not is_axis_aligned_rectangle([(1, 2), (3, 2), (2, 9)])

    with tempfile.TemporaryDirectory(prefix="comb-referee-self-test-") as temp:
        svg_path = pathlib.Path(temp) / "synthetic.svg"
        svg_path.write_text(
            '<svg xmlns="http://www.w3.org/2000/svg" '
            'viewBox="0 0 100 100">'
            '<defs><path id="glyph-white" '
            'd="M 0 0 L 3 0 L 3 3 L 0 3 Z"/>'
            '<symbol id="glyph-symbol" viewBox="0 0 10 10">'
            '<path d="M0 0L10 0L10 10L0 10Z"/></symbol></defs>'
            '<path id="triangle" d="M 10 10 L 20 10 L 15 30 Z" fill="#000"/>'
            '<path id="nonpainting" d="M 20 40 L 30 40 L 25 50 Z" '
            'fill="none" stroke="none"/>'
            '<g clip-path="url(#clip)"><rect id="clipped" x="30" y="10" '
            'width="1" height="20" fill="#000"/></g>'
            '<rect id="style-clipped" x="35" y="10" width="1" height="20" '
            'fill="#000" style="clip-path:url(#clip)"/>'
            '<line id="diagonal" x1="40" y1="10" x2="50" y2="30" '
            'stroke="#000" stroke-width="1"/>'
            '<line id="near-diagonal" x1="60" y1="10" x2="60.2" y2="30" '
            'stroke="#000" stroke-width="1"/>'
            '<line id="outside-position-bound" '
            'x1="65" y1="10" x2="65.6" y2="30" '
            'stroke="#000" stroke-width="1"/>'
            '<line id="dashed" x1="55" y1="10" x2="55" y2="30" '
            'stroke="#000" style="stroke-dasharray:1 1"/>'
            '<rect id="translucent" x="70" y="10" width="1" height="20" '
            'fill="#000" opacity="0.5"/>'
            '<rect id="clamped-opacity" x="72" y="10" width="1" height="20" '
            'fill="#000" opacity="2"/>'
            '<rect id="reordered" x="75" y="10" width="1" height="20" '
            'fill="#fff" stroke="#000" '
            'style="paint-order:stroke fill"/>'
            '<path id="curve" d="M 80 80 C 82 82 84 84 86 86" '
            'fill="none" stroke="#000"/>'
            '<path id="anisotropic-curve" transform="scale(10 1)" '
            'd="M 0 60 C 0.5 61 1.5 61 2 60" '
            'fill="none" stroke="#000"/>'
            '<rect id="glyph-base" x="90" y="10" width="1" height="20" '
            'fill="#000"/>'
            '<use id="glyph-knockout" href="#glyph-white" x="89" y="12" '
            'fill="#fff"/>'
            '<line id="anisotropic-line" transform="scale(1 10)" '
            'x1="50" y1="1" x2="50" y2="3" '
            'stroke="#000" stroke-width="0.2"/>'
            '<path id="evenodd" fill="#000" fill-rule="evenodd" '
            'd="M9.9 10L10.1 10L10.1 30L9.9 30Z'
            'M9.9 10L10.1 10L10.1 30L9.9 30Z"/>'
            '<g visibility="HIDDEN"><rect id="visible-child" '
            'visibility="VISIBLE" x="45" y="10" width="1" height="20"/></g>'
            '<g id="glyph-visible-child"><g visibility="hidden">'
            '<path visibility="visible" fill="#fff" '
            'd="M0 0L3 0L3 3L0 3Z"/></g></g>'
            '<use id="glyph-visible-child-use" href="#glyph-visible-child" '
            'x="20" y="20" fill="#fff"/>'
            '<line id="marked" x1="5" y1="5" x2="6" y2="6" '
            'stroke="#000" marker-start="url(#m)"/>'
            '<switch id="conditional"><rect systemLanguage="zz" '
            'x="5" y="10" width="1" height="20"/></switch>'
            '<use id="glyph-symbol-use" href="#glyph-symbol" '
            'x="10" y="2" width="20" height="6" fill="#fff"/>'
            '<rect id="negative-rect" x="20" y="20" '
            'width="-1" height="2" fill="#000"/>'
            '</svg>',
            encoding="utf-8",
        )
        parsed_svg = parse_svg(svg_path)
        assert any(region.reason == "non-rectangular closed SVG fill"
                   for region in parsed_svg.unsupported)
        assert any(region.reason == "diagonal SVG line"
                   for region in parsed_svg.unsupported)
        assert any(paint.element == "near-diagonal"
                   and paint.kind == "near-vertical-line"
                   for paint in parsed_svg.paints)
        assert any(region.element == "outside-position-bound"
                   for region in parsed_svg.unsupported)
        assert any(paint.element == "clipped" and paint.clipped
                   for paint in parsed_svg.paints)
        assert any(paint.element == "style-clipped" and paint.clipped
                   for paint in parsed_svg.paints)
        assert any(paint.element == "dashed" and paint.clipped
                   for paint in parsed_svg.paints)
        assert any(paint.element == "translucent" and paint.clipped
                   for paint in parsed_svg.paints)
        assert any(paint.element == "clamped-opacity" and paint.tone == 0.0
                   for paint in parsed_svg.paints)
        assert any(paint.element == "reordered" and paint.clipped
                   for paint in parsed_svg.paints)
        assert any(paint.element == "anisotropic-line" and paint.clipped
                   for paint in parsed_svg.paints)
        assert any(paint.element == "visible-child"
                   for paint in parsed_svg.paints)
        assert any(region.element == "evenodd"
                   and "compound SVG fill" in region.reason
                   for region in parsed_svg.unsupported)
        assert any(region.element == "marked"
                   and region.reason == "SVG marker paint is not resolved"
                   for region in parsed_svg.unsupported)
        assert any(region.element == "conditional"
                   and "switch conditional" in region.reason
                   for region in parsed_svg.unsupported)
        assert any(region.element == "glyph-symbol-use"
                   and "glyph symbol viewport" in region.reason
                   for region in parsed_svg.unsupported)
        assert any(region.element == "negative-rect"
                   and region.reason == "negative SVG rect extent"
                   for region in parsed_svg.unsupported)
        assert not any(region.element == "nonpainting"
                       for region in parsed_svg.unsupported)
        curve = next(region for region in parsed_svg.unsupported
                     if region.element == "curve")
        assert curve.x0 > 70 and curve.y0 > 70
        anisotropic = next(
            region for region in parsed_svg.unsupported
            if region.element == "anisotropic-curve")
        assert anisotropic.x0 <= -19 and anisotropic.x1 >= 39
        knockout = next(
            region for region in parsed_svg.unsupported
            if region.element == "glyph-knockout")
        assert knockout.tone == 1.0 and knockout.order > 0
        assert any(
            region.element == "glyph-visible-child-use"
            and region.tone == 1.0
            for region in parsed_svg.unsupported
        )

        styled_path = pathlib.Path(temp) / "styled.svg"
        styled_path.write_text(
            '<svg xmlns="http://www.w3.org/2000/svg" '
            'viewBox="0 0 100 100"><defs><style>'
            '.divider{fill:#fff}</style></defs>'
            '<rect class="divider" x="10" y="10" '
            'width="1" height="20"/></svg>',
            encoding="utf-8",
        )
        styled_svg = parse_svg(styled_path)
        assert any(
            region.reason == "embedded SVG stylesheet is not resolved"
            and region.x0 == 0 and region.y0 == 0
            and region.x1 == 100 and region.y1 == 100
            for region in styled_svg.unsupported
        )
        invalid_svg_fragments = {
            "important":
                '<rect style="visibility:hidden!important" '
                'x="1" y="1" width="1" height="1"/>',
            "comment":
                '<rect style="visibility:/*x*/hidden" '
                'x="1" y="1" width="1" height="1"/>',
            "inline-blend":
                '<rect style="mix-blend-mode:screen" '
                'x="1" y="1" width="1" height="1"/>',
            "attribute-blend":
                '<rect mix-blend-mode="screen" '
                'x="1" y="1" width="1" height="1"/>',
        }
        for name, fragment in invalid_svg_fragments.items():
            invalid_svg_path = pathlib.Path(temp) / f"invalid-{name}.svg"
            invalid_svg_path.write_text(
                '<svg xmlns="http://www.w3.org/2000/svg" '
                f'viewBox="0 0 10 10">{fragment}</svg>',
                encoding="utf-8",
            )
            try:
                parse_svg(invalid_svg_path)
            except RefereeError:
                pass
            else:
                raise AssertionError(
                    f"unsupported SVG CSS was accepted: {name}")

    def paint(x: float, a: float = 2, b: float = 8, order: int = 0,
              tone: float = 0.0) -> Paint:
        return Paint(x - 0.1, a, x + 0.1, b, tone, order,
                     "test", f"x{x}-o{order}")

    def source_frame() -> list[Paint]:
        return [
            Paint(-0.1, -0.1, 40.1, 0.1, 0.0, 10,
                  "stroke", "single-frame"),
            Paint(-0.1, 9.9, 40.1, 10.1, 0.0, 11,
                  "stroke", "single-frame"),
            Paint(-0.1, -0.1, 0.1, 10.1, 0.0, 12,
                  "stroke", "single-frame"),
            Paint(39.9, -0.1, 40.1, 10.1, 0.0, 13,
                  "stroke", "single-frame"),
        ]

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

    def parsed_subject(anchor: float) -> dict[str, Any]:
        return {
            "id": "p1c9",
            "x0": anchor - 1.5, "y0": 9.0,
            "x1": anchor + 1.5, "y1": 31.0,
            "comb": {
                "cells": 2, "divider_x": [anchor],
                "pitch_pt": 3.0, "divider_gray": 0.0,
                "y0": 10.0, "y1": 30.0,
            },
        }

    for anchor in (30.5, 35.5, 55.0, 90.5):
        result = classify_band(parsed_subject(anchor), parsed_svg)
        assert result["status"] == "unevaluable", (anchor, result)

    # Unexplained full-height ink inside a one-pitch gap is ambiguous; it
    # cannot be silently ignored to preserve the lattice answer.
    normal = {
        **cell,
        "comb": {"cells": 3, "divider_x": [10.0, 20.0],
                 "pitch_pt": 10.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
    }
    result = classify_band(normal, SvgPage(
        100, 100, [paint(10), paint(15), paint(20)], [], "x"))
    assert result["status"] == "unevaluable", result

    # A final-tone component must still descend from eligible vertical ink.
    # A short square inside the width bound is not a divider candidate.
    square = classify_band(cell, SvgPage(
        100, 100, [
            paint(10), paint(30),
            Paint(18, 2, 22, 6, 0.0, 3, "fill", "square"),
            *source_frame(),
        ], [], "x"))
    assert square["status"] == "measured", square
    assert square["compartments"] == 3 and not square["extra_divider_x"], square

    erased_under_square = classify_band(cell, SvgPage(
        100, 100, [
            paint(10, order=0), paint(20, order=1), paint(30, order=2),
            Paint(19, 2, 21, 8, 1.0, 3, "fill", "white-erasure"),
            Paint(18, 2, 22, 6, 0.0, 4, "fill", "square"),
            *source_frame(),
        ], [], "x"))
    assert erased_under_square["status"] == "measured", erased_under_square
    assert (erased_under_square["compartments"] == 3
            and not erased_under_square["extra_divider_x"]), erased_under_square

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

    nested_majority = classify_band(cell, SvgPage(
        100, 100, [
            paint(10, a=2, b=7.5), paint(20, a=2, b=7.5),
            paint(30, a=2, b=7.5),
            paint(10.2, a=7.5, b=8), paint(30.2, a=7.5, b=8),
        ], [], "x"))
    assert nested_majority["status"] == "measured", nested_majority
    assert nested_majority["compartments"] == 4, nested_majority
    assert nested_majority["seed_span_pt"] == 6.0, nested_majority
    assert nested_majority["measured_span_pt"] == 6.0, nested_majority
    assert nested_majority["chosen_topology"] == [10.0, 20.0, 30.0]
    assert nested_majority["topology_superset_relations"], nested_majority

    # A tiny richer slab cannot win by excluding the anchorless remainder from
    # its denominator.
    mostly_anchorless = classify_band(cell, SvgPage(
        100, 100, [
            paint(30, a=2, b=3),
            paint(20, a=2, b=2.6),
        ], [], "x"))
    assert mostly_anchorless["status"] == "unevaluable", mostly_anchorless

    # Rounding 25/10 to two intervals must not invent a regular subdivision.
    non_integral = {
        **cell,
        "comb": {**cell["comb"], "divider_x": [10.0, 35.0]},
    }
    result = classify_band(non_integral, SvgPage(
        100, 100, [paint(10), paint(20), paint(35)], [], "x"))
    assert result["status"] == "unevaluable", result

    # A fully observed source comb may have deliberately non-uniform
    # compartments.  Pitch is an inference aid for extra painted boundaries,
    # not permission to invent a boundary where Poppler paints none.
    non_uniform = {
        **cell,
        "comb": {**cell["comb"],
                 "cells": 4,
                 "divider_x": [7.5, 19.25, 30.25],
                 "pitch_pt": 7.5},
    }
    result = classify_band(non_uniform, SvgPage(
        100, 100,
        [paint(7.5), paint(19.25), paint(30.25), *source_frame()],
        [], "x"))
    assert result["status"] == "measured", result
    assert result["compartments"] == 4 and not result["extra_divider_x"], result
    assert result["subject_gap_proofs"], result

    # Two independent combs separated by a static-label-sized void are not one
    # non-uniform comb merely because no divider is painted in the void.
    conflated = {
        **cell,
        "x1": 100.0,
        "comb": {**cell["comb"],
                 "cells": 3,
                 "divider_x": [10.0, 80.0],
                 "pitch_pt": 10.0},
    }
    result = classify_band(conflated, SvgPage(
        100, 100, [paint(10), paint(80)], [], "x"))
    assert result["status"] == "unevaluable", result

    short_extra = classify_band(cell, SvgPage(
        100, 100, [
            paint(10), paint(30), paint(20, a=2, b=5),
        ], [], "x"))
    assert short_extra["status"] == "unevaluable", short_extra

    # A white or differently toned band is not a structural closing rule and
    # cannot crop away the part of a comb that contradicts the anchors.
    white_crop = classify_band(cell, SvgPage(
        100, 100, [
            Paint(0, 2, 40, 4.9, 1.0, 0, "fill", "white-band"),
            paint(10, order=1), paint(30, order=2),
            paint(20, a=2, b=4.8, order=3),
        ], [], "x"))
    assert white_crop["status"] == "unevaluable", white_crop
    assert white_crop["open_y0"] == 2.0, white_crop

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

    # Edge bisection cannot overrule final gaps that disagree with the measured
    # pitch.
    edge_split = {
        **cell,
        "comb": {"cells": 3, "divider_x": [20.0, 31.0],
                 "pitch_pt": 9.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
    }
    result = classify_band(edge_split, SvgPage(
        100, 100, [paint(0), paint(10), paint(20), paint(31), paint(40)], [], "x"))
    assert result["status"] == "unevaluable", result

    # A nearer off-pitch stroke cannot be skipped to reach a farther convenient
    # continuation.
    outward_blocked = {
        **cell,
        "comb": {**cell["comb"], "divider_x": [20.0, 30.0]},
    }
    result = classify_band(outward_blocked, SvgPage(
        100, 100, [paint(5), paint(10), paint(20), paint(30)], [], "x"))
    assert result["status"] == "unevaluable", result

    # An off-pitch vertical in a broad mixed interval makes ownership
    # ambiguous; it cannot be silently skipped.
    broad = {
        **cell,
        "comb": {"cells": 2, "divider_x": [30.0],
                 "pitch_pt": 10.0, "divider_gray": 0.0,
                 "y0": 2.0, "y1": 8.0},
        "x1": 100.0,
    }
    result = classify_band(broad, SvgPage(
        100, 100, [paint(0), paint(30), paint(55), paint(100)], [], "x"))
    assert result["status"] == "unevaluable", result

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

    # A clipped anchor subset is ambiguous even when another slab has a
    # complete topology.
    clipped_subset = classify_band(cell, SvgPage(
        100, 100, [
            paint(10, a=2, b=6, order=0),
            paint(30, a=2, b=6, order=1),
            Paint(9.9, 6, 10.1, 8, 0.0, 2,
                  "test", "clipped-anchor", True),
        ], [], "x"))
    assert clipped_subset["status"] == "unevaluable", clipped_subset

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
        *source_frame(),
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
        *source_frame(),
    ], [], "x")
    result = classify_band(cell, grey_overpaint)
    assert result["status"] == "measured" and result["compartments"] == 3, result

    # A broad same-tone fill removes the distinct narrow boundary even though
    # the final pixels remain black.
    black_overpaint = SvgPage(100, 100, [
        paint(10, order=0), paint(20, order=1), paint(30, order=2),
        Paint(15, 0, 25, 10, 0.0, 3, "fill", "broad-black"),
        *source_frame(),
    ], [], "x")
    result = classify_band(cell, black_overpaint)
    assert result["status"] == "measured" and result["compartments"] == 3, result

    # Final topology is independent of whether a same-tone background was
    # painted before or after the narrow mark.
    black_underpaint = SvgPage(100, 100, [
        Paint(15, 0, 25, 10, 0.0, 0, "fill", "broad-black"),
        paint(10, order=1), paint(20, order=2), paint(30, order=3),
        *source_frame(),
    ], [], "x")
    result = classify_band(cell, black_underpaint)
    assert result["status"] == "measured" and result["compartments"] == 3, result

    same_tone_glyph = UnsupportedRegion(
        15, 2, 25, 8, "glyph use may occlude geometry: #glyph-x",
        "glyph", 0.0, 4, False)
    result = classify_band(cell, SvgPage(
        100, 100, [paint(10), paint(20), paint(30)],
        [same_tone_glyph], "x"))
    assert result["status"] == "unevaluable", result

    # A conservative glyph bound touching only the cell-side frame cannot
    # occlude an eligible interior divider.  It must not make an otherwise
    # exhaustive source topology unevaluable.
    edge_glyph = UnsupportedRegion(
        -1, 2, 0.2, 8, "glyph use may occlude geometry: #glyph-edge",
        "glyph", 0.0, 4, False)
    result = classify_band(cell, SvgPage(
        100, 100, [paint(0), paint(10), paint(20), paint(30)],
        [edge_glyph], "x"))
    assert result["status"] == "measured", result
    assert result["compartments"] == 4, result

    # Some official fixed text is encoded as broad curved outlines instead of
    # glyph-use nodes.  A broad curve wholly inside a compartment cannot be a
    # straight divider and cannot occlude one.  A narrow curve, or a broad
    # curve crossing an actual divider, remains unevaluable.
    broad_curve_inside = UnsupportedRegion(
        12, 2, 18, 8, "curved SVG path", "fixed-outline", 0.0, 4, False)
    result = classify_band(cell, SvgPage(
        100, 100, [paint(10), paint(20), paint(30)],
        [broad_curve_inside], "x"))
    assert result["status"] == "measured", result
    narrow_curve = UnsupportedRegion(
        19.8, 2, 20.2, 8, "curved SVG path",
        "narrow-curve", 0.0, 4, False)
    assert classify_band(cell, SvgPage(
        100, 100, [paint(10), paint(20), paint(30)],
        [narrow_curve], "x"))["status"] == "unevaluable"
    short_narrow_curve = UnsupportedRegion(
        15, 7.6, 15.4, 8, "curved SVG path",
        "short-narrow-curve", 0.0, 4, False)
    result = classify_band(cell, SvgPage(
        100, 100, [paint(10), paint(20), paint(30)],
        [short_narrow_curve], "x"))
    assert result["status"] == "measured", result
    broad_curve_crossing = UnsupportedRegion(
        15, 2, 25, 8, "curved SVG path",
        "crossing-outline", 0.0, 4, False)
    assert classify_band(cell, SvgPage(
        100, 100, [paint(10), paint(20), paint(30)],
        [broad_curve_crossing], "x"))["status"] == "unevaluable"

    no_anchor = {**cell, "comb": {**cell["comb"], "cells": 1, "divider_x": []}}
    assert classify_band(no_anchor, page)["status"] == "unevaluable"

    minimal_style = (
        "<style>"
        ".page{position:relative;overflow:hidden}"
        ".c{position:absolute}"
        ".s{position:absolute}"
        "</style>"
    )
    parser = SlotParser(require_runtime_contract=False)
    parser.feed(
        '<html data-form="X">' + minimal_style
        + '<body><div class="page page-1" id="page-1" '
        'style="width:100pt;height:100pt">'
        '<div class="layer-cells">'
        '<div id="p1c1" data-field-kind="comb" data-field-name="p1c1" '
        'class="c f" '
        'data-comb-capacity="2" data-comb-slots="2" '
        'data-comb-pitch="10" data-cell-kind="field" '
        'data-row="0" data-col="0" '
        'style="left:0pt;top:0pt;width:20pt;height:10pt">'
        '<div class="s" data-slot="0" '
        'style="left:0pt;top:0pt;width:10pt;height:10pt">'
        '<input type="text" class="fi fh0 fc" id="p1c1-s0" '
        'name="p1c1" data-slot-index="0" maxlength="1" '
        'autocomplete="off" spellcheck="false"></div>'
        '<div class="s" data-slot="1" '
        'style="left:10pt;top:0pt;width:10pt;height:10pt">'
        '</div></div></div></div>'
        '<template id="band-template-0" data-band="b0" '
        'data-band-index="0" data-capacity="1" data-row-pitch="10" '
        'data-row-y="0" data-template-row="0">'
        '<div class="s" data-slot="2">'
        '<input type="text" class="fi fh0 fc" data-slot-index="2" '
        'maxlength="1" autocomplete="off" spellcheck="false">'
        '</div></template>'
        '</body></html>'
    )
    assert parser.physical_slots == {"p1c1": [0, 1]}
    assert parser.editable_slots == {"p1c1": [0]}
    assert parser.comb_containers == {"p1c1"}
    assert parser.root == {"data-form": "X"}
    assert parser.pages == [1]
    assert parser.page_geometry == [(1, 100.0, 100.0)]
    parser_expected = {"p1c1": {
        "page_index": 1,
        "left": 0.0, "top": 0.0, "width": 20.0, "height": 10.0,
        "slots": [
            {"index": 0, "left": 0.0, "top": 0.0,
             "width": 10.0, "height": 10.0},
            {"index": 1, "left": 10.0, "top": 0.0,
             "width": 10.0, "height": 10.0},
        ],
    }}
    assert slot_records(parser, parser_expected)["p1c1"]["valid"]
    moved_expected = {
        "p1c1": {**parser_expected["p1c1"], "left": 50.0, "top": 50.0},
    }
    assert not slot_records(parser, moved_expected)["p1c1"]["valid"]

    invalid_slots = SlotParser(require_runtime_contract=False)
    invalid_slots.feed(
        '<html>' + minimal_style + '<body>'
        '<div class="page page-1" id="page-1" '
        'style="width:100pt;height:100pt">'
        '<div class="layer-cells">'
        '<div id="p1c1" data-field-kind="comb" data-field-name="p1c1" '
        'class="c f" data-cell-kind="field" data-row="0" data-col="0" '
        'data-comb-pitch="10" '
        'data-comb-capacity="3" data-comb-slots="3" '
        'style="left:0pt;top:0pt;width:20pt;height:10pt">'
        '<div class="s" data-slot="0" '
        'style="left:0pt;top:0pt;width:10pt;height:10pt"></div>'
        '<div class="s" data-slot="1" '
        'style="left:10pt;top:0pt;width:0pt;height:10pt"></div>'
        '<div class="s" data-slot="2" '
        'style="left:9pt;top:0pt;width:11pt;height:10pt"></div>'
        '</div></div></div></body></html>'
    )
    assert not slot_records(invalid_slots)["p1c1"]["valid"]

    invalid_page_binding = SlotParser(require_runtime_contract=False)
    invalid_page_binding.feed(
        '<html>' + minimal_style + '<body>'
        '<div class="page page-1" id="page-1" '
        'style="width:100pt;height:100pt">'
        '<div class="layer-cells">'
        '<div id="p1c1" data-field-kind="comb" data-field-name="p1c1" '
        'class="c f" data-cell-kind="field" data-row="0" data-col="0" '
        'data-comb-pitch="10" '
        'data-comb-capacity="1" data-comb-slots="1" '
        'style="left:999pt;top:0pt;width:10pt;height:10pt">'
        '<div class="s" data-slot="0" '
        'style="left:0pt;top:0pt;width:10pt;height:10pt"></div></div>'
        '<div id="p2c1" data-field-kind="comb" data-field-name="p2c1" '
        'class="c f" data-cell-kind="field" data-row="0" data-col="0" '
        'data-comb-pitch="10" '
        'data-comb-capacity="1" data-comb-slots="1" '
        'style="left:0pt;top:0pt;width:10pt;height:10pt">'
        '<div class="s" data-slot="0" '
        'style="left:0pt;top:0pt;width:10pt;height:10pt"></div></div>'
        '</div></div></body></html>'
    )
    invalid_records = slot_records(invalid_page_binding)
    assert not invalid_records["p1c1"]["valid"]
    assert not invalid_records["p2c1"]["valid"]
    assert any("comb page binding disagrees: p2c1" == error
               for error in invalid_page_binding.invalid_bindings)

    invalid_style = SlotParser(require_runtime_contract=False)
    invalid_style.feed(
        '<html>' + minimal_style + '<body>'
        '<div class="page page-1" id="page-1" '
        'style="width:100pt;height:100pt">'
        '<div class="layer-cells">'
        '<div id="p1c1" data-field-kind="comb" data-field-name="p1c1" '
        'class="c f" data-cell-kind="field" data-row="0" data-col="0" '
        'data-comb-pitch="10" '
        'data-comb-capacity="1" data-comb-slots="1" '
        'style="left:0pt;top:0pt;width:10pt;height:10pt;display:none">'
        '<div class="s" data-slot="0" '
        'style="left:0pt;top:0pt;width:10pt;height:10pt"></div>'
        '</div></div></div></body></html>'
    )
    assert any("comb geometry is non-canonical: p1c1" == error
               for error in invalid_style.invalid_bindings)

    def hidden_layout_record(
            wrapper_open: str = "", wrapper_close: str = "",
            comb_attribute: str = "", extra_style: str = "",
            sibling_html: str = "",
            ) -> tuple[dict[str, Any], SlotParser]:
        hidden_parser = SlotParser(require_runtime_contract=False)
        hidden_parser.feed(
            '<html>' + minimal_style + extra_style
            + '<body><div class="page page-1" id="page-1" '
            'style="width:100pt;height:100pt">'
            + sibling_html
            + '<div class="layer-cells">'
            + wrapper_open
            + '<div class="c" id="p1c1" data-field-kind="comb" '
            'data-field-name="p1c1" data-comb-capacity="1" '
            'data-comb-slots="1" data-comb-pitch="10" '
            'data-cell-kind="field" data-row="0" data-col="0" '
            'style="left:0pt;top:0pt;width:10pt;height:10pt"'
            + comb_attribute + '>'
            '<div class="s" data-slot="0" '
            'style="left:0pt;top:0pt;width:10pt;height:10pt">'
            '<input type="text" class="fi fh0 fc" id="p1c1-s0" '
            'name="p1c1" data-slot-index="0" maxlength="1" '
            'autocomplete="off" spellcheck="false"></div></div>'
            + wrapper_close + '</div></div></body></html>'
        )
        hidden_parser.close()
        return slot_records(hidden_parser)["p1c1"], hidden_parser

    hidden_comb, _hidden_comb_parser = hidden_layout_record(
        comb_attribute=" hidden")
    assert not hidden_comb["valid"]
    hidden_ancestor, _hidden_ancestor_parser = hidden_layout_record(
        wrapper_open="<section hidden>", wrapper_close="</section>")
    assert not hidden_ancestor["valid"]
    inline_hidden, _inline_hidden_parser = hidden_layout_record(
        wrapper_open='<section style="display:none">',
        wrapper_close="</section>",
    )
    assert not inline_hidden["valid"]
    closed_details, _closed_details_parser = hidden_layout_record(
        wrapper_open="<details>", wrapper_close="</details>")
    assert not closed_details["valid"]
    styled_hidden, styled_hidden_parser = hidden_layout_record(
        extra_style="<style>#p1c1{display:none!important}</style>")
    assert not styled_hidden["valid"]
    assert any("!important" in error
               for error in styled_hidden_parser.invalid_bindings)
    for property_name, property_value in (
            ("margin-left", "20pt"),
            ("background", "white"),
            ("mask", "linear-gradient(transparent,transparent)"),
            ):
        styled, _styled_parser = hidden_layout_record(
            extra_style=(
                f"<style>.c{{{property_name}:{property_value}}}</style>"
            ))
        assert not styled["valid"], property_name
    shifted_ancestor, _shifted_ancestor_parser = hidden_layout_record(
        wrapper_open=(
            '<section style="position:relative;left:20pt">'),
        wrapper_close="</section>",
    )
    assert not shifted_ancestor["valid"]
    popover_comb, _popover_parser = hidden_layout_record(
        comb_attribute=" popover")
    assert not popover_comb["valid"]
    noscript_comb, _noscript_parser = hidden_layout_record(
        wrapper_open="<noscript>", wrapper_close="</noscript>")
    assert not noscript_comb["valid"]
    duplicate_attribute, _duplicate_parser = hidden_layout_record(
        comb_attribute=' style="display:none"')
    assert not duplicate_attribute["valid"]
    imported_css, _import_parser = hidden_layout_record(
        extra_style=(
            '<style>@import url("data:text/css,.c%7Bdisplay%3Anone%7D");'
            "</style>"))
    assert not imported_css["valid"]
    conditional_css, _conditional_parser = hidden_layout_record(
        extra_style="<style>@media not all{.c{position:absolute}}</style>")
    assert not conditional_css["valid"]
    supported_css, _supported_parser = hidden_layout_record(
        extra_style=(
            "<style>@supports (unknown: value)"
            "{.c{position:absolute}}</style>"))
    assert not supported_css["valid"]
    overlay, _overlay_parser = hidden_layout_record(
        sibling_html=(
            '<div style="position:fixed;inset:0;background:#fff;'
            'z-index:9999"></div>'))
    assert not overlay["valid"]
    event_image, _event_image_parser = hidden_layout_record(
        sibling_html=(
            '<img src="invalid://x" '
            'onerror="document.body.hidden=true">'))
    assert not event_image["valid"]
    plaintext_parser = SlotParser(require_runtime_contract=False)
    plaintext_parser.feed("<html><body><plaintext>x</plaintext></body></html>")
    assert any("unsupported emitter element: plaintext" in error
               for error in plaintext_parser.invalid_bindings)
    page_size_parser = SlotParser()
    page_size_parser.doctype_count = 1
    page_size_parser.style_count = 1
    page_size_parser.band_data_scripts = 1
    page_size_parser.runtime_script_hashes = list(HTML_RUNTIME_SCRIPT_SHA256)
    page_size_parser.page_geometry = [(1, 612.0, 936.0)]
    page_size_parser.stylesheet_page_sizes = [(1.0, 1.0)]
    slot_records(page_size_parser)
    assert any("@page size disagrees" in error
               for error in page_size_parser.invalid_bindings)
    no_doctype_parser = SlotParser()
    no_doctype_parser.style_count = 1
    no_doctype_parser.band_data_scripts = 1
    no_doctype_parser.runtime_script_hashes = list(
        HTML_RUNTIME_SCRIPT_SHA256)
    no_doctype_parser.page_geometry = [(1, 612.0, 936.0)]
    no_doctype_parser.stylesheet_page_sizes = [(612.0, 936.0)]
    slot_records(no_doctype_parser)
    assert any("standards-mode doctype" in error
               for error in no_doctype_parser.invalid_bindings)
    structure = b"<!doctype html><html><body><div></div></body></html>"
    assert emitted_structure_sha256(structure) != emitted_structure_sha256(
        structure.replace(b"<div>", b"<svg></svg><div>"))
    assert emitted_structure_sha256(
        structure.replace(b"<div>", b"<input id=x><div>")
    ) != emitted_structure_sha256(
        structure.replace(b"<div>", b"<input id=y><div>")
    )
    bogus_runtime = SlotParser()
    bogus_runtime._validate_script((), "document.body.hidden=true")
    slot_records(bogus_runtime)
    assert any("runtime scripts disagree" in error
               for error in bogus_runtime.invalid_bindings)

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
    selected = select_layouts(
        [pathlib.Path("0605-1999.layout.json"),
         pathlib.Path("1701-2018.layout.json")],
        ["0605"],
    )
    assert [path.name for path in selected] == ["0605-1999.layout.json"]
    try:
        select_layouts(selected, ["0605", "0605-1999"])
    except RefereeError:
        pass
    else:
        raise AssertionError("overlapping --only selectors were accepted")
    assert corpus_coverage_ok(
        {"0605-1999"}, [{}], EXPECTED_COMBS_BY_SLUG["0605-1999"], [])
    assert not corpus_coverage_ok(
        {"0605-1999"}, [{}], EXPECTED_COMBS_BY_SLUG["0605-1999"] - 1, [])

    print("comb_referee self-test: pass")
    return 0


def select_layouts(layouts: Sequence[pathlib.Path],
                   selectors: Sequence[str]) -> list[pathlib.Path]:
    """Resolve --only selectors without silently dropping or double-matching."""
    if not selectors:
        return sorted(layouts)
    normalized = [value.lower() for value in selectors]
    if len(normalized) != len(set(normalized)):
        raise RefereeError("--only contains a duplicate selector")
    selected: dict[str, pathlib.Path] = {}
    claimed_by: dict[str, str] = {}
    for selector in normalized:
        matches = [
            path for path in layouts
            if (path.name.removesuffix(".layout.json").lower() == selector
                or path.name.split("-", 1)[0].lower() == selector)
        ]
        if not matches:
            raise RefereeError(f"--only selector matched no layout: {selector}")
        for path in matches:
            slug = path.name.removesuffix(".layout.json")
            if slug in selected:
                raise RefereeError(
                    "--only selectors overlap for "
                    f"{slug}: {claimed_by[slug]}, {selector}")
            selected[slug] = path
            claimed_by[slug] = selector
    return [selected[slug] for slug in sorted(selected)]


def corpus_coverage_ok(selected_slugs: set[str],
                       forms: Sequence[dict[str, Any]],
                       combs: int,
                       errors: Sequence[dict[str, str]]) -> bool:
    if errors or selected_slugs - set(EXPECTED_COMBS_BY_SLUG):
        return False
    expected = sum(EXPECTED_COMBS_BY_SLUG[slug] for slug in selected_slugs)
    return len(forms) == len(selected_slugs) and combs == expected


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
        wanted = [value.lower() for value in args.only or ()]
        layouts = select_layouts(
            sorted(args.layout_dir.glob("*.layout.json")), wanted)
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
        expected_comb_total = sum(
            EXPECTED_COMBS_BY_SLUG[slug] for slug in selected_slugs)
        coverage_ok = corpus_coverage_ok(
            selected_slugs, forms, combs, errors)
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
