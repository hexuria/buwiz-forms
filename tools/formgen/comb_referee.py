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

One fail-closed exception can prove that a lattice anchor is absent, but it is
restricted to an already-active unresolved ledger subject and can never
discover a new comb.  One partial-anchor source topology must occupy the
entire open band; every observed divider must map one-to-one to a declared
anchor; and every missing anchor must have an exact raw target-tone rail that
one supported, unclipped, non-target final owner exhaustively erases across
every open slab.  Clipped paint, unsupported geometry, mixed topmost owners,
or surviving target-tone ink closes the exception.  Subject ownership comes
only from the active ledger identity--this certificate does not claim an
independent source enclosure--and retained subjects remain ineligible.

An outward boundary must continue the measured source pitch, or be the sole
boundary that symmetrically divides the remaining edge interval.  Cell-edge
ink is never counted as an interior divider.  These constraints make the check
useful for both disputed heavy group separators and truncated first/last ticks
without turning unrelated verticals in a broad mixed cell into character
boxes.  Every other partial pattern, unsupported vector geometry, clipped
candidate, missing provenance, or competing source band is UNEVALUABLE --
never a pass.

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
import mimetypes
import os
import pathlib
import platform
import posixpath
import re
import signal
import shutil
import subprocess
import sys
import tempfile
import urllib.parse
import xml.etree.ElementTree as ET
from collections.abc import Iterable, Sequence
from decimal import Decimal, InvalidOperation
from typing import Any


HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent

REPORT_VERSION = 2
EXPECTED_FORMS = 51
EXPECTED_COMBS = 4442
LATTICE_PRODUCER_FILE = "tools/formgen/lattice.py"
LATTICE_PRODUCER_SHA256 = (
    "1449392f72429d2442e7cdd2e3654144c6a4cd13427ee29513f0c7e2c6fd1e1e"
)
AUDIT_PRODUCER_FILE = "tools/formgen/audit.py"
AUDIT_PRODUCER_SHA256 = (
    "6b8b326cd617cf1fa2dc6e35b1209bff2df0f2d2f37720992121f859556d3186"
)
AUDIT_DEPENDENCY_SHA256 = {
    "tools/formgen/extract.py": (
        "85ccfe328f9be6ef02ef06486194fe02d09484e4ef2c33daabac542e08707019"
    ),
    "tools/formgen/verify.py": (
        "8dbeb222c9f04c8c71cf6ccf58acb519631e8e94966128fcdca9a56d097bad44"
    ),
}
AUDIT_INPUT_ROLES = frozenset({
    "ir", "layout", "html", "guide", "guide_html", "source_pdf",
})
AUDIT_ROUNDTRIP_LAUNCH_ARGS = [
    "--disable-background-networking",
    "--disable-component-update",
    "--disable-default-apps",
    "--disable-sync",
    "--metrics-recording-only",
    "--no-first-run",
]
AUDIT_ROUNDTRIP_SCOPE = (
    "playwright-package-tree-and-explicit-chromium-executable"
)
AUDIT_CANDIDATE_MATERIALIZATION = (
    "private-0700-o_excl-o_nofollow-fsynced-unlinked-read-fd"
)
AUDIT_PDF_NORMALIZATION_REPLACEMENT = "D:19700101000000+00'00'"
POPPLER_IDENTITY_TIMEOUT_SECONDS = 10.0
POPPLER_PAGE_TIMEOUT_SECONDS = 60.0
SUBPROCESS_CLEANUP_POLICY = "kill-isolated-process-group"
AUDIT_POSITION_FIELDS = {
    "emission_layout_position": (
        "emission-layout-position-mismatch", False),
    "emission_layout_outer_position": (
        "emission-layout-outer-position-mismatch", True),
    "emission_source_position": (
        "emission-source-position-mismatch", False),
    "emission_source_outer_position": (
        "emission-source-outer-position-mismatch", True),
    "layout_source_outer_position": (
        "layout-source-outer-position-mismatch", True),
}
AUDIT_FAILURE_KINDS = frozenset({
    "source-topology-unevaluable",
    "layout-printed-mismatch",
    "duplicate-layout-subject",
    "emission-container-page-mismatch",
    "emission-container-geometry-mismatch",
    "emission-layout-position-mismatch",
    "emission-layout-outer-position-mismatch",
    "emission-source-position-mismatch",
    "emission-source-outer-position-mismatch",
    "layout-source-outer-position-mismatch",
    "invalid-emission",
    "emission-layout-mismatch",
    "emission-printed-mismatch",
    "unexpected-emitted-comb",
    "emitted-cell-binding-invalid",
    "duplicate-emitted-cell-id",
    "missing-layout-cell-owner",
    "duplicate-layout-cell-owner",
    "emitted-cell-page-mismatch",
    "emitted-cell-geometry-mismatch",
    "unowned-live-comb-markup",
    "comb-inventory-mismatch",
    "comb-owner-registry-invalid",
})
AUDIT_OWNER_CERTIFICATE_CRITERION = (
    "exact-reviewed-layout-comb-subject-owner-v1"
)
ACTIVE_PARTIAL_ANCHOR_CRITERION = (
    "active-full-band-partial-anchor-source-topology-v1"
)
AUDIT_OWNER_CERTIFICATE_VALID_KEYS = frozenset({
    "criterion", "valid", "layout_sha256", "page", "cell_id",
    "legacy_cell_id", "subject_key", "legacy_bbox",
    "bbox_number_format", "state", "supplies_topology",
})
AUDIT_OWNER_CERTIFICATE_INVALID_KEYS = frozenset({
    "criterion", "valid", "reason", "supplies_topology",
})
LATTICE_GENERATOR_KEYS = frozenset({
    "producer",
    "schema_version",
    "consumes_ir_schema_version",
    "cluster_tolerance_pt",
    "pitch_tolerance_pt",
})
LATTICE_GENERATOR_CONTRACT = {
    "producer": LATTICE_PRODUCER_FILE,
    "schema_version": 1,
    "consumes_ir_schema_version": 2,
    "cluster_tolerance_pt": 0.3,
    "pitch_tolerance_pt": 0.3,
}
COMB_SUBJECT_STATES = frozenset({
    "active_resolved",
    "active_unresolved",
    "retained_unresolved",
})
COMB_INFERENCE_STATE = "suppressed_unreviewed_inference"
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

# Reviewed from report payload
# 15b6454ef9c156435fc33d47b177ff4b2db379207fa694bbcdb87200bb341ca4.
# The digest binds all 105 ordered tuples of cell identity, subject identity,
# source verdict, compartment count, divider positions, and position verdict.
REVIEWED_2551Q_REFEREE_TUPLES_SHA256 = (
    "f6fa281a670156784c723911329669849cf433c3f082c3d108a89980f1290414"
)
REVIEWED_2551Q_EXPLICIT_COMPARTMENTS = {
    "p2c5": 14,
    "p2c80": 12,
}

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
_SUBJECT_KEY_RE = re.compile(
    rf"^p(\d+)@({_NUMBER}),({_NUMBER}),({_NUMBER}),({_NUMBER})$")
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
HTML_LINK_ATTRIBUTES = frozenset({
    "as",
    "crossorigin",
    "href",
    "rel",
    "type",
})
HTML_FONT_PRELOAD_HREFS = frozenset({
    "fonts/arimo-latin-wght-italic.woff2",
    "fonts/arimo-latin-wght-normal.woff2",
    "fonts/tinos-latin-400-normal.woff2",
    "fonts/tinos-latin-700-normal.woff2",
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
    "link",
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
    "0605-1999": "483b591602f3b165060066ddad585bbe6d1a4f58b536bfeba062ec0aa7aa8d9a",
    "0619e-2018": "c78a6bc8076613aee50448338bdb668bdb295563eb1b7a4c9a7bac83fb561c60",
    "0619f-2018": "528244da50efc5b4f8df0772872a694838d1656d61b43e2fd1b4a4c2a1d7533b",
    "0620-2019": "b518080567abd01b164206ac65b9764b52fa5f8c269fee7c42f7b453800c2932",
    "1600-pt-2018": "5301c0de14861a70a28c9eac6875ce33dc1859ec8e3b8e3a03dd4d5bc592e00f",
    "1600-vt-2018": "a4d0ae18efd9fd4e031bb3d32429dda12d3798a4d7738cd25eee2e873c48a27f",
    "1600wp-2010": "8102181ae92fb424bb5c481205b2bb5d0ee40793300b7b62690807c37dc642a3",
    "1601-fq-2020": "bcdea19d517354bd99359441be0d72d3a19cbdf4bdee4bfec03a9cae3e303caa",
    "1601c-2018": "0ed9c44270beea02e751b453309aa93028cadd123adc29f77f6817ea61111d33",
    "1601eq-2019": "607c7c408580064895f550852ec6aa4775d7132c123fbd35ead8d7168aaa93e1",
    "1602q-2019": "a0eb6e9126ad482c04215daacda802faba9825362b02560eacc78c3422999ef9",
    "1603q-2018": "7c2396c4e365ca95c94df39465fc86a2a2d77387cf2d3381e7a43ac799426f74",
    "1604c-2018": "8176ede0c118767700770faecf84043988f0477aa9a1ede93a71097c5332c660",
    "1604e-2018": "e002c0fb62d1a72dc7ec5399a35f3d541e2be46d3fe75260f88636f2397621f6",
    "1604f-2018": "ee02bbeb8ecf6a015d0a83127a71c278944e54b8bd240edb755441bb7bef244b",
    "1606-2018": "d0132774c9aa91afec79eacc3ee25ced9bf70605614fdf6513309ee3ee283ed8",
    "1621-2019": "fb866c13bb4a25f6d4d31229033901cba9ed28b218e2b37e4eb1ae22bacc260c",
    "1700-2018": "84bf093a5c29b042355bc9d963cea3d65bb06f5372bd70f82521269fbecd5586",
    "1701-2018-attachment": "ad84cb756901597da0965c5a04bceea074edd4b659690ba3e03da95576241b4a",
    "1701-2018-conso": "c3d048110151d82a2ba3d38fcb91a7db54ccb02a4a66305820e3bc7f6c03e40d",
    "1701-2018": "133c7126bfa2535a7fb77a329ad7482d3312f8dbcd86a2cc9ac48fe2aebd7838",
    "1701a-2018": "e975abc8f2c725c67efb67806c7359a72a8203e518fa43037a3fbc2acbe8db2e",
    "1701ms-2024": "13ea709a53cc2757ee9c5a147388e53da9eef7cda397e4889c60edd8f32d59dc",
    "1701q-2018": "2157e9a63790ddd1e6c8fb61cccd858d5909326e5ad1d70abdf743ef2def024a",
    "1702ex-2018": "75e5a9aab5eb93d769e8b9fbe4fce609280e07685a53b2a1b2cfe49858720e1d",
    "1702mx-2018c-attachment": "72bc8e08425aef7f725bcd9bb1f627270a6c83fe4f587b43436ad00928290fd9",
    "1702mx-2018c": "370d3a6247fb6d9f83fa6b2e1b9cd9a45c47f7be654bab1884fa0e42b3be7c74",
    "1702q-2018": "30fa68fdf171943dfc2a1d384e720c981479228c064a70bcd7c5cd75a7c2d4ce",
    "1702rt-2018c": "397a50a5eba8450d1241894317d2c21a9ea411adb8cbdead474b4d6997d89cd8",
    "1706-2018": "6698006494bb2bf361566c47dbf5acdcfb60257538cd5ba732f3b1e0570d8796",
    "1707-2021": "c81f7b24e0bada91fdddef079aa88983a0c69880e683c48fd4540bd25a674a85",
    "1707a-2021": "44ae7cc734e36c6dbb70ca71b41f1faaadcd8b015b8d541e0d3d2e895099058e",
    "1709-2020": "5844983cc5b9c5a86d3c49032eb2788099cc749013f60b471b5e1b8d5585c268",
    "1800-2018": "362bcd4db98d4894a13cf86bcd90e743f1fe3c4cbbc3aa43a462fc777e348d36",
    "1801-2018": "c0e8c652c7152a28a95a40a2577fa9674f7806b8b73a66372aa19f6a84c5f0ec",
    "2000-dst-2018": "00c02c08e36c762e35a560129e13ec0f2f47d91fcf964ff2bd533d164e09166c",
    "2000-ot-2018": "8c47cab3ee1d5551e5f9c700fb2caf1fbbbf670b57e9c30db0fe3487d814e5cb",
    "2200a-2020": "111e13a655629a58041ecf15b2fe803e6db4efbc30048ed3e8eea8cf93ff163f",
    "2200c-2018": "463438cba23df70dee6b9fd1f3eec9a22d5b11251cf187e361f405f3e429198e",
    "2200m-2018": "c7987b39494e41858414de88f9abc78cba35ad941f9e0bd1e660d6b7227c1030",
    "2200p-2020": "5d147a506d384346d05c37e2528d2e69790b39ea74de3e3b3a69c6ed1351c40b",
    "2200s-2018": "970148fe6b51ee6e85a91d8b3c6c8e04ad4047b47887b25f022b5c3d8f7a1d71",
    "2200t-2022": "cac275661a890625e2429817162c48724fd13dcef91fec9637ac4b9dcc0792db",
    "2316-2021": "b6335d5b824fd6e3e867ccbc67964fbea6a77db740eed0193a0e63e5fbced249",
    "2550-ds-2025": "671a67eecf54ba45ee7b5b57ed54f53cb5dafabeb02729822be729d8fdb327e8",
    "2550m-2007": "1b8ad7d1305cce12d8c6274b702955df35c7ebc0f461c9fc185849b18b54efc7",
    "2550q-2024": "461a4e2401fd219fda01b7437e6231f6ed8d364ce229b14be81adf048919c75b",
    "2551m-2002": "4dc72010661acb6f6814c2f8bae78296dd16612f586a4781b2f1cd4f999eddc3",
    "2551q-2018": "bf44be4d00cc458eacbf23a6d3cf1f0f46445722a25f39af07bec1ead4e2361f",
    "2552-2018": "93582754fea8a085cc6d5f490366d5dde4f61626c2588760864210a35a7b7dea",
    "2553-1999": "b70bce8f2ba36a47a2b67108157d3dfdf561af84868663b054844e24ff94667f",
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


def referee_tuple_digest(cells: Sequence[dict[str, Any]]) -> str:
    tuples = [
        [
            cell["cell"],
            cell["subject_key"],
            cell["referee"]["status"],
            cell["referee"].get("compartments"),
            cell["referee"].get("source_divider_x"),
            cell["referee"].get("positions_match"),
        ]
        for cell in sorted(cells, key=lambda item: item["subject_key"])
    ]
    return canonical_digest(tuples)


def validate_2551q_referee_golden(cells: Sequence[dict[str, Any]]) -> None:
    if len(cells) != EXPECTED_COMBS_BY_SLUG["2551q-2018"]:
        raise RefereeError("2551Q reviewed referee tuple count changed")
    by_cell = {cell["cell"]: cell for cell in cells}
    if len(by_cell) != len(cells):
        raise RefereeError("2551Q reviewed referee tuple identities changed")
    for cell_id, expected in REVIEWED_2551Q_EXPLICIT_COMPARTMENTS.items():
        cell = by_cell.get(cell_id)
        if (cell is None
                or cell["referee"].get("status") != "measured"
                or cell["referee"].get("compartments") != expected):
            raise RefereeError(
                f"2551Q reviewed control changed: {cell_id} != {expected}")
    actual = referee_tuple_digest(cells)
    if actual != REVIEWED_2551Q_REFEREE_TUPLES_SHA256:
        raise RefereeError(
            "2551Q reviewed 105-tuple referee digest changed: " + actual)


def attach_report_digest(report: dict[str, Any]) -> None:
    if "payload_sha256" in report:
        raise RefereeError("report already carries a payload digest")
    report["self_digest"] = {
        "algorithm": "sha256",
        "canonicalization": "json-sort-keys-compact-utf8",
        "excluded_field": "payload_sha256",
    }
    report["payload_sha256"] = canonical_digest(report)


def report_digest_valid(report: dict[str, Any]) -> bool:
    payload_sha256 = report.get("payload_sha256")
    if not isinstance(payload_sha256, str):
        return False
    without_digest = {
        key: value for key, value in report.items()
        if key != "payload_sha256"
    }
    return payload_sha256 == canonical_digest(without_digest)


def report_bytes(report: dict[str, Any]) -> bytes:
    if not report_digest_valid(report):
        raise RefereeError("report self-digest is missing or stale")
    return (json.dumps(report, indent=2, sort_keys=True, ensure_ascii=False)
            + "\n").encode("utf-8")


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


def run_bounded_subprocess(
        command: Sequence[str],
        *,
        timeout_seconds: float,
        label: str,
        ) -> subprocess.CompletedProcess[str]:
    """Run one oracle process in an isolated group with a fixed hard limit."""
    if (not math.isfinite(timeout_seconds) or timeout_seconds <= 0
            or not command or not all(
                isinstance(item, str) and item for item in command)):
        raise RefereeError(f"{label} has an invalid subprocess contract")
    process = subprocess.Popen(
        list(command),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=(os.name == "posix"),
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        if os.name == "posix":
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        else:
            process.kill()
        try:
            process.communicate(timeout=5.0)
        except subprocess.TimeoutExpired:
            process.kill()
            try:
                process.communicate(timeout=5.0)
            except subprocess.TimeoutExpired as error:
                raise RefereeError(
                    f"{label} could not be reaped after "
                    f"{SUBPROCESS_CLEANUP_POLICY}") from error
        raise RefereeError(
            f"{label} exceeded its fixed {timeout_seconds:g}-second "
            f"deadline; cleanup={SUBPROCESS_CLEANUP_POLICY}")
    return subprocess.CompletedProcess(
        args=list(command),
        returncode=process.returncode,
        stdout=stdout,
        stderr=stderr,
    )


def poppler_identity() -> dict[str, Any]:
    binary = shutil.which("pdftocairo")
    if binary is None:
        raise RefereeError("pdftocairo is not installed")
    proc = run_bounded_subprocess(
        [binary, "-v"],
        timeout_seconds=POPPLER_IDENTITY_TIMEOUT_SECONDS,
        label="pdftocairo identity",
    )
    version = (proc.stdout + proc.stderr).strip().splitlines()
    if proc.returncode != 0 or not version:
        raise RefereeError("pdftocairo -v failed")
    return {
        "version": version[0],
        "binary_path": str(pathlib.Path(binary).resolve()),
        "binary_sha256": sha256_file(pathlib.Path(binary)),
        "identity_timeout_seconds": POPPLER_IDENTITY_TIMEOUT_SECONDS,
        "page_timeout_seconds": POPPLER_PAGE_TIMEOUT_SECONDS,
        "subprocess_cleanup_policy": SUBPROCESS_CLEANUP_POLICY,
    }


def render_svg_page(binary: str, pdf: pathlib.Path, page_number: int,
                    directory: pathlib.Path) -> pathlib.Path:
    output = directory / f"page-{page_number}.svg"
    proc = run_bounded_subprocess(
        [binary, "-svg", "-f", str(page_number), "-l", str(page_number),
         str(pdf), str(output)],
        timeout_seconds=POPPLER_PAGE_TIMEOUT_SECONDS,
        label=f"pdftocairo page {page_number}",
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
            if tag == "link":
                parent = self.element_stack[-1][0] if self.element_stack else None
                if parent != "head":
                    self.invalid_bindings.append(
                        "HTML font preload is outside the document head")
            elif tag in ("base", "embed", "iframe", "noscript", "object"):
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
        elif tag == "link":
            valid = (
                set(values) == HTML_LINK_ATTRIBUTES
                and values.get("rel") == "preload"
                and values.get("as") == "font"
                and values.get("type") == "font/woff2"
                and values.get("crossorigin") is None
                and values.get("href") in HTML_FONT_PRELOAD_HREFS
            )
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


def exact_nonnegative_int(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise RefereeError(f"{label} is not a non-negative integer")
    return value


def finite_number(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise RefereeError(f"{label} is not numeric")
    result = float(value)
    if not math.isfinite(result):
        raise RefereeError(f"{label} is not finite")
    return result


def string_list(value: Any, label: str, *, nonempty: bool = False
                ) -> list[str]:
    if (not isinstance(value, list)
            or any(not isinstance(item, str) or not item for item in value)
            or len(value) != len(set(value))
            or (nonempty and not value)):
        raise RefereeError(f"{label} is not a unique string list")
    return value


def same_numbers(left: Sequence[Any], right: Sequence[Any]) -> bool:
    """Exact serialized-number equality, allowing only float representation."""
    return (
        len(left) == len(right)
        and all(abs(float(a) - float(b)) <= 1e-9
                for a, b in zip(left, right))
    )


def decimal_identity(value: Any, label: str) -> str:
    """Return audit.py's exact, non-exponent Decimal identity."""
    if isinstance(value, bool):
        raise RefereeError(f"{label} is not a decimal number")
    try:
        if isinstance(value, Decimal):
            number = value
        elif isinstance(value, int):
            number = Decimal(value)
        elif isinstance(value, str):
            number = Decimal(value)
        else:
            raise RefereeError(f"{label} is not an exact decimal value")
    except InvalidOperation as error:
        raise RefereeError(f"{label} is not a decimal number") from error
    if not number.is_finite():
        raise RefereeError(f"{label} is not a finite decimal number")
    rendered = format(number, "f")
    if "." in rendered:
        rendered = rendered.rstrip("0").rstrip(".")
    return "0" if rendered in {"", "-0"} else rendered


def canonical_decimal_string(value: Any, label: str) -> Decimal:
    if not isinstance(value, str) or not value:
        raise RefereeError(f"{label} is not a decimal string")
    try:
        number = Decimal(value)
    except InvalidOperation as error:
        raise RefereeError(f"{label} is not a decimal string") from error
    if not number.is_finite() or decimal_identity(number, label) != value:
        raise RefereeError(f"{label} is not a canonical decimal string")
    return number


def audit_owner_binding(
        layout_payload: bytes,
        ledger: dict[str, Any],
        ) -> dict[str, Any]:
    """Build exact expected owner certificates from retained layout bytes."""
    try:
        retained = json.loads(
            layout_payload.decode("utf-8"), parse_float=Decimal)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RefereeError(
            "retained layout bytes are not exact UTF-8 JSON") from error
    pages = retained.get("pages") if isinstance(retained, dict) else None
    if not isinstance(pages, list) or not pages:
        raise RefereeError("retained layout has no exact page inventory")
    active_subjects: dict[str, dict[str, Any]] = {}
    for subject in ledger.get("subjects") or ():
        if subject.get("state") not in {
                "active_resolved", "active_unresolved"}:
            continue
        cell_id = subject.get("cell_id")
        if not isinstance(cell_id, str) or cell_id in active_subjects:
            raise RefereeError(
                "active ledger owner identities are not unique")
        active_subjects[cell_id] = subject

    layout_sha256 = sha256_bytes(layout_payload)
    certificates: dict[str, dict[str, Any]] = {}
    for expected_page, page in enumerate(pages, 1):
        if (not isinstance(page, dict)
                or page.get("index") != expected_page
                or not isinstance(page.get("cells"), list)):
            raise RefereeError(
                "retained layout pages are not exhaustive and ordered")
        for cell in page["cells"]:
            if not isinstance(cell, dict):
                raise RefereeError("retained layout contains a malformed cell")
            cell_id = cell.get("id")
            subject = active_subjects.get(cell_id)
            if subject is None:
                continue
            page_match = (
                _CELL_PAGE_RE.fullmatch(cell_id)
                if isinstance(cell_id, str) else None
            )
            if page_match is None or int(page_match.group(1)) != expected_page:
                raise RefereeError(
                    "retained layout owner cell does not identify its page")
            subject_key = cell.get("subject_key")
            if (subject.get("page") != expected_page
                    or subject.get("cell_id") != cell_id
                    or subject.get("legacy_cell_id") != cell_id
                    or subject.get("subject_key") != subject_key):
                raise RefereeError(
                    f"retained layout owner disagrees with ledger: {cell_id}")
            bbox_values = [
                cell.get(name) for name in ("x0", "y0", "x1", "y1")
            ]
            bbox = [
                decimal_identity(
                    value, f"retained layout owner {cell_id} bbox")
                for value in bbox_values
            ]
            bbox_numbers = [Decimal(value) for value in bbox]
            if (bbox_numbers[2] <= bbox_numbers[0]
                    or bbox_numbers[3] <= bbox_numbers[1]):
                raise RefereeError(
                    f"retained layout owner has non-positive bbox: {cell_id}")
            subject_match = (
                _SUBJECT_KEY_RE.fullmatch(subject_key)
                if isinstance(subject_key, str) else None
            )
            if subject_match is None or int(subject_match.group(1)) != expected_page:
                raise RefereeError(
                    f"retained layout owner has invalid subject_key: {cell_id}")
            encoded_bbox = [
                Decimal(subject_match.group(index)) for index in range(2, 6)
            ]
            if encoded_bbox != bbox_numbers:
                raise RefereeError(
                    f"retained layout owner subject_key/bbox differ: {cell_id}")
            if cell_id in certificates:
                raise RefereeError(
                    f"retained layout duplicates active owner: {cell_id}")
            certificates[cell_id] = {
                "criterion": AUDIT_OWNER_CERTIFICATE_CRITERION,
                "valid": True,
                "layout_sha256": layout_sha256,
                "page": expected_page,
                "cell_id": cell_id,
                "legacy_cell_id": cell_id,
                "subject_key": subject_key,
                "legacy_bbox": bbox,
                "bbox_number_format": "canonical-decimal-string-v1",
                "state": subject["state"],
                "supplies_topology": False,
            }
    if set(certificates) != set(active_subjects):
        raise RefereeError(
            "retained layout and active ledger owner inventories differ")
    return {
        "layout_sha256": layout_sha256,
        "cells": certificates,
    }


def validate_audit_owner_certificate(
        value: Any,
        expected: dict[str, Any] | None,
        ) -> dict[str, Any]:
    """Validate one identity-only audit certificate, never source topology."""
    if not isinstance(value, dict):
        raise RefereeError("audit offender owner certificate is missing")
    if value.get("criterion") != AUDIT_OWNER_CERTIFICATE_CRITERION:
        raise RefereeError("audit offender owner criterion is invalid")
    if value.get("valid") is True:
        if (set(value) != AUDIT_OWNER_CERTIFICATE_VALID_KEYS
                or value.get("supplies_topology") is not False):
            raise RefereeError(
                "audit offender valid owner certificate schema is false")
        layout_sha = value.get("layout_sha256")
        if (not isinstance(layout_sha, str)
                or re.fullmatch(r"[0-9a-f]{64}", layout_sha) is None):
            raise RefereeError(
                "audit offender owner layout SHA-256 is invalid")
        page = exact_nonnegative_int(
            value.get("page"), "audit offender owner page")
        if page == 0:
            raise RefereeError("audit offender owner page is not one-based")
        cell_id = value.get("cell_id")
        legacy_cell_id = value.get("legacy_cell_id")
        cell_match = (
            _CELL_PAGE_RE.fullmatch(cell_id)
            if isinstance(cell_id, str) else None
        )
        if (cell_match is None or int(cell_match.group(1)) != page
                or legacy_cell_id != cell_id):
            raise RefereeError(
                "audit offender owner cell identity is invalid")
        subject_key = value.get("subject_key")
        subject_match = (
            _SUBJECT_KEY_RE.fullmatch(subject_key)
            if isinstance(subject_key, str) else None
        )
        raw_bbox = value.get("legacy_bbox")
        if (subject_match is None or int(subject_match.group(1)) != page
                or not isinstance(raw_bbox, list) or len(raw_bbox) != 4):
            raise RefereeError(
                "audit offender owner subject/bbox identity is invalid")
        bbox = [
            canonical_decimal_string(
                item, "audit offender owner legacy_bbox")
            for item in raw_bbox
        ]
        encoded_bbox = [
            Decimal(subject_match.group(index)) for index in range(2, 6)
        ]
        if (encoded_bbox != bbox or bbox[2] <= bbox[0]
                or bbox[3] <= bbox[1]):
            raise RefereeError(
                "audit offender owner subject_key/bbox relation is false")
        if (value.get("bbox_number_format")
                != "canonical-decimal-string-v1"):
            raise RefereeError(
                "audit offender owner bbox number format is invalid")
        if value.get("state") not in {
                "active_resolved", "active_unresolved"}:
            raise RefereeError("audit offender owner state is invalid")
        if expected is not None and value != expected:
            raise RefereeError(
                "audit offender owner certificate is not layout-bound")
        return value
    if (set(value) != AUDIT_OWNER_CERTIFICATE_INVALID_KEYS
            or value.get("valid") is not False
            or value.get("supplies_topology") is not False
            or not isinstance(value.get("reason"), str)
            or not value["reason"]):
        raise RefereeError(
            "audit offender invalid owner certificate schema is false")
    return value


def validate_subject_identity(
        subject_key: Any,
        legacy_cell_id: Any,
        page_index: int,
        bbox_value: Any,
        label: str,
        ) -> tuple[str, str, list[float]]:
    if not isinstance(subject_key, str):
        raise RefereeError(f"{label} has no string subject_key")
    match = _SUBJECT_KEY_RE.fullmatch(subject_key)
    if match is None or int(match.group(1)) != page_index:
        raise RefereeError(f"{label} has an invalid subject_key")
    if not isinstance(legacy_cell_id, str):
        raise RefereeError(f"{label} has no string legacy_cell_id")
    cell_match = _CELL_PAGE_RE.fullmatch(legacy_cell_id)
    if cell_match is None or int(cell_match.group(1)) != page_index:
        raise RefereeError(f"{label} has an invalid legacy_cell_id")
    if not isinstance(bbox_value, list) or len(bbox_value) != 4:
        raise RefereeError(f"{label} has no four-number legacy_bbox")
    bbox_values = [
        finite_number(value, f"{label} legacy_bbox")
        for value in bbox_value
    ]
    if (bbox_values[2] <= bbox_values[0]
            or bbox_values[3] <= bbox_values[1]):
        raise RefereeError(f"{label} has a non-positive legacy_bbox")
    encoded_bbox = [float(match.group(index)) for index in range(2, 6)]
    if not same_numbers(encoded_bbox, bbox_values):
        raise RefereeError(
            f"{label} subject_key disagrees with legacy_bbox")
    return subject_key, legacy_cell_id, bbox_values


def validate_comb_topology(
        comb: Any,
        bbox_value: Sequence[Any],
        label: str,
        ) -> dict[str, Any]:
    if not isinstance(comb, dict):
        raise RefereeError(f"{label} has no comb topology")
    cells = exact_nonnegative_int(comb.get("cells"), f"{label} cells")
    divider_count = exact_nonnegative_int(
        comb.get("divider_count"), f"{label} divider_count")
    if cells < 1 or divider_count != cells - 1:
        raise RefereeError(f"{label} cells/divider_count topology disagrees")
    raw_dividers = comb.get("divider_x")
    raw_slots = comb.get("slot_x")
    if not isinstance(raw_dividers, list) or not isinstance(raw_slots, list):
        raise RefereeError(f"{label} has no divider_x/slot_x topology")
    dividers = [
        finite_number(value, f"{label} divider_x")
        for value in raw_dividers
    ]
    slots = [
        finite_number(value, f"{label} slot_x")
        for value in raw_slots
    ]
    if len(dividers) != divider_count or len(slots) != cells + 1:
        raise RefereeError(f"{label} divider_x/slot_x inventory disagrees")
    if any(right <= left for left, right in zip(slots, slots[1:])):
        raise RefereeError(f"{label} slot_x is not strictly increasing")
    if not same_numbers(slots[1:-1], dividers):
        raise RefereeError(f"{label} divider_x disagrees with slot_x")
    bbox_numbers = [
        finite_number(value, f"{label} bbox") for value in bbox_value
    ]
    if (len(bbox_numbers) != 4
            or not same_numbers((slots[0], slots[-1]),
                                (bbox_numbers[0], bbox_numbers[2]))):
        raise RefereeError(f"{label} slot_x disagrees with subject bbox")
    y0 = finite_number(comb.get("y0"), f"{label} y0")
    y1 = finite_number(comb.get("y1"), f"{label} y1")
    if y1 <= y0:
        raise RefereeError(f"{label} has a non-positive comb band")
    pitch = finite_number(comb.get("pitch_pt"), f"{label} pitch_pt")
    if pitch <= 0:
        raise RefereeError(f"{label} has no positive pitch")
    resolution = comb.get("resolution")
    if not isinstance(resolution, dict):
        raise RefereeError(f"{label} has no resolution record")
    resolution_status = resolution.get("status")
    if resolution_status not in ("resolved", "unresolved"):
        raise RefereeError(f"{label} has an unknown resolution status")
    reason_codes = string_list(
        resolution.get("reason_codes"), f"{label} resolution reason_codes")
    if bool(reason_codes) != (resolution_status == "unresolved"):
        raise RefereeError(f"{label} resolution reasons/status disagree")
    topology = {
        "cells": cells,
        "divider_x": dividers,
        "slot_x": slots,
        "y0": y0,
        "y1": y1,
        "resolution_status": resolution_status,
        "reason_codes": reason_codes,
    }
    topology["sha256"] = canonical_digest(topology)
    return topology


def bind_lattice_generator(
        layout: dict[str, Any],
        lattice_producer_bytes: bytes,
        ) -> dict[str, Any]:
    actual_sha = sha256_bytes(lattice_producer_bytes)
    if actual_sha != LATTICE_PRODUCER_SHA256:
        raise RefereeError(
            "lattice producer bytes disagree with the committed pin")
    generator = layout.get("generator")
    if (not isinstance(generator, dict)
            or set(generator) != LATTICE_GENERATOR_KEYS
            or generator != LATTICE_GENERATOR_CONTRACT):
        raise RefereeError(
            "layout lattice generator contract is missing or stale")
    return {
        "file": LATTICE_PRODUCER_FILE,
        "bytes": len(lattice_producer_bytes),
        "sha256": actual_sha,
        "expected_sha256": LATTICE_PRODUCER_SHA256,
        "layout_generator": dict(generator),
    }


def validate_comb_ledger(
        slug: str,
        layout: dict[str, Any],
        lattice_producer_bytes: bytes,
        ) -> dict[str, Any]:
    """Bind the immutable 4,442-subject denominator to active layout cells.

    The legacy ledger is identity and continuity evidence.  It never promotes
    an unresolved current comb, and a retained subject remains published even
    though no active cell is allowed to emit it.
    """
    expected_total = EXPECTED_COMBS_BY_SLUG.get(slug)
    if expected_total is None:
        raise RefereeError(f"{slug}: form is not in the pinned referee corpus")
    lattice = bind_lattice_generator(layout, lattice_producer_bytes)
    pages = layout.get("pages")
    if not isinstance(pages, list) or not pages:
        raise RefereeError(f"{slug}: layout has no page inventory")

    published_subjects: list[dict[str, Any]] = []
    published_inferences: list[dict[str, Any]] = []
    active_cell_ids: set[str] = set()
    retained_legacy_ids: set[str] = set()
    inference_cell_ids: set[str] = set()
    global_subject_keys: set[str] = set()
    global_legacy_ids: set[str] = set()

    for expected_page, page in enumerate(pages, 1):
        if not isinstance(page, dict) or page.get("index") != expected_page:
            raise RefereeError(
                f"{slug}: ledger pages are not exhaustive and ordered")
        page_index = expected_page
        raw_cells = page.get("cells")
        if not isinstance(raw_cells, list):
            raise RefereeError(f"{slug} page {page_index}: cells is not a list")
        cells_by_id: dict[str, dict[str, Any]] = {}
        cells_by_subject: dict[str, dict[str, Any]] = {}
        for raw_cell in raw_cells:
            if not isinstance(raw_cell, dict):
                raise RefereeError(
                    f"{slug} page {page_index}: malformed layout cell")
            cell_id = raw_cell.get("id")
            subject_key = raw_cell.get("subject_key")
            if not isinstance(cell_id, str) or not _CELL_RE.fullmatch(cell_id):
                raise RefereeError(
                    f"{slug} page {page_index}: layout cell has invalid id")
            cell_match = _CELL_PAGE_RE.fullmatch(cell_id)
            if cell_match is None or int(cell_match.group(1)) != page_index:
                raise RefereeError(
                    f"{slug} page {page_index}: layout cell id is on another page")
            bbox_value = [
                raw_cell.get(name) for name in ("x0", "y0", "x1", "y1")
            ]
            validate_subject_identity(
                subject_key, cell_id, page_index, bbox_value,
                f"{slug} page {page_index} layout cell {cell_id}")
            if cell_id in cells_by_id or subject_key in cells_by_subject:
                raise RefereeError(
                    f"{slug} page {page_index}: duplicate layout cell identity")
            cells_by_id[cell_id] = raw_cell
            cells_by_subject[str(subject_key)] = raw_cell

        if "comb_subjects" not in page:
            raise RefereeError(
                f"{slug} page {page_index}: comb subject ledger is missing")
        subjects = page["comb_subjects"]
        if not isinstance(subjects, list):
            raise RefereeError(
                f"{slug} page {page_index}: comb subject ledger is not a list")
        if "comb_inferences" not in page:
            raise RefereeError(
                f"{slug} page {page_index}: comb inference ledger is missing")
        inferences = page["comb_inferences"]
        if not isinstance(inferences, list):
            raise RefereeError(
                f"{slug} page {page_index}: comb inference ledger is not a list")

        page_subject_keys: set[str] = set()
        page_legacy_ids: set[str] = set()
        page_active_ids: set[str] = set()
        for index, subject in enumerate(subjects):
            label = f"{slug} page {page_index} subject {index}"
            if not isinstance(subject, dict):
                raise RefereeError(f"{label} is not an object")
            subject_key, legacy_cell_id, legacy_bbox = (
                validate_subject_identity(
                    subject.get("subject_key"),
                    subject.get("legacy_cell_id"),
                    page_index,
                    subject.get("legacy_bbox"),
                    label,
                )
            )
            if (subject_key in page_subject_keys
                    or legacy_cell_id in page_legacy_ids
                    or subject_key in global_subject_keys
                    or legacy_cell_id in global_legacy_ids):
                raise RefereeError(
                    f"{label} duplicates a subject_key or legacy_cell_id")
            page_subject_keys.add(subject_key)
            page_legacy_ids.add(legacy_cell_id)
            global_subject_keys.add(subject_key)
            global_legacy_ids.add(legacy_cell_id)
            state = subject.get("state")
            if state not in COMB_SUBJECT_STATES:
                raise RefereeError(
                    f"{label} has unknown or retired state: {state}")
            reason_codes = string_list(
                subject.get("reason_codes"), f"{label} reason_codes",
                nonempty=state != "active_resolved")
            blocks_gate = subject.get("blocks_gate")
            if (not isinstance(blocks_gate, bool)
                    or blocks_gate != (state != "active_resolved")):
                raise RefereeError(
                    f"{label} state/blocks_gate contract disagrees")

            if state.startswith("active_"):
                cell_id = subject.get("cell_id")
                mapped_ids = subject.get("mapped_partition_cell_ids")
                if (not isinstance(cell_id, str)
                        or mapped_ids != [cell_id]
                        or cell_id in page_active_ids):
                    raise RefereeError(
                        f"{label} has no unique one-to-one active cell mapping")
                cell = cells_by_id.get(cell_id)
                if cell is None or cell.get("subject_key") != subject_key:
                    raise RefereeError(
                        f"{label} active cell subject_key/cell_id disagrees")
                cell_bbox = [
                    cell.get(name) for name in ("x0", "y0", "x1", "y1")
                ]
                if not same_numbers(legacy_bbox, cell_bbox):
                    raise RefereeError(
                        f"{label} active cell geometry changed subject identity")
                topology = validate_comb_topology(
                    cell.get("comb"), cell_bbox, f"{label} active cell")
                subject_cells = exact_nonnegative_int(
                    subject.get("cells"), f"{label} cells")
                if subject_cells != topology["cells"]:
                    raise RefereeError(
                        f"{label} ledger/cell comb counts disagree")
                expected_resolution = (
                    "resolved" if state == "active_resolved" else "unresolved")
                if (topology["resolution_status"] != expected_resolution
                        or reason_codes != topology["reason_codes"]):
                    raise RefereeError(
                        f"{label} ledger/cell resolution evidence disagrees")
                transition = subject.get("boundary_topology_transition")
                transition_fields_present = any(
                    key in subject for key in (
                        "old_divider_x", "new_divider_x",
                        "boundary_topology_transition",
                    )
                )
                cell_transition = (
                    (cell.get("comb") or {}).get("resolution") or {}
                ).get("boundary_topology_transition")
                if transition_fields_present or cell_transition is not None:
                    if (not isinstance(transition, dict)
                            or set(transition) != {
                                "old_divider_x", "new_divider_x",
                                "comparison_tolerance_pt",
                                "independently_certified",
                            }
                            or transition != cell_transition
                            or subject.get("old_divider_x")
                            != transition.get("old_divider_x")
                            or subject.get("new_divider_x")
                            != transition.get("new_divider_x")
                            or transition.get("independently_certified") is not False
                            or transition.get("comparison_tolerance_pt")
                            != LATTICE_GENERATOR_CONTRACT[
                                "cluster_tolerance_pt"]
                            or not same_numbers(
                                transition.get("new_divider_x") or (),
                                topology["divider_x"])
                            or len(transition.get("old_divider_x") or ())
                            != topology["cells"] - 1
                            or state != "active_unresolved"
                            or "same-count-boundary-topology-change"
                            not in reason_codes):
                        raise RefereeError(
                            f"{label} boundary topology transition is invalid")
                page_active_ids.add(cell_id)
                active_cell_ids.add(cell_id)
                published_subjects.append({
                    "page": page_index,
                    "subject_key": subject_key,
                    "legacy_cell_id": legacy_cell_id,
                    "cell_id": cell_id,
                    "state": state,
                    "blocks_gate": blocks_gate,
                    "reason_codes": reason_codes,
                    "legacy_bbox": legacy_bbox,
                    "source_cell": cell,
                    "topology": topology,
                    "ledger": subject,
                })
                continue

            if (subject.get("cell_id") is not None
                    or subject.get("emission") != "suppressed"
                    or subject.get("requires_independent_evidence") is not True
                    or subject.get("permitted_transitions") != [
                        "active_composite", "retired_proven_false",
                    ]):
                raise RefereeError(
                    f"{label} retained suppression evidence is incomplete")
            legacy_topology = validate_comb_topology(
                subject.get("legacy_comb"), legacy_bbox,
                f"{label} retained legacy_comb")
            if legacy_topology["resolution_status"] != "unresolved":
                raise RefereeError(
                    f"{label} retained legacy_comb is not unresolved")
            mapped_ids = string_list(
                subject.get("mapped_partition_cell_ids"),
                f"{label} mapped_partition_cell_ids")
            mapped_keys = string_list(
                subject.get("mapped_partition_subject_keys"),
                f"{label} mapped_partition_subject_keys")
            if len(mapped_ids) != len(mapped_keys):
                raise RefereeError(
                    f"{label} retained partition mappings disagree")
            for mapped_id, mapped_key in zip(mapped_ids, mapped_keys):
                mapped_cell = cells_by_id.get(mapped_id)
                if (mapped_cell is None
                        or mapped_cell.get("subject_key") != mapped_key):
                    raise RefereeError(
                        f"{label} retained partition mapping is stale")
            if (subject_key in cells_by_subject
                    and "comb" in cells_by_subject[subject_key]):
                raise RefereeError(
                    f"{label} retained subject still has an active comb")
            retained_legacy_ids.add(legacy_cell_id)
            published_subjects.append({
                "page": page_index,
                "subject_key": subject_key,
                "legacy_cell_id": legacy_cell_id,
                "cell_id": None,
                "state": state,
                "blocks_gate": True,
                "reason_codes": reason_codes,
                "legacy_bbox": legacy_bbox,
                "source_cell": {
                    "id": legacy_cell_id,
                    "subject_key": subject_key,
                    "x0": legacy_bbox[0],
                    "y0": legacy_bbox[1],
                    "x1": legacy_bbox[2],
                    "y1": legacy_bbox[3],
                    "comb": subject["legacy_comb"],
                },
                "topology": legacy_topology,
                "ledger": subject,
            })

        comb_cells = [
            cell for cell in raw_cells if isinstance(cell.get("comb"), dict)
        ]
        comb_cell_ids = {str(cell["id"]) for cell in comb_cells}
        if page_active_ids != comb_cell_ids:
            missing = sorted(comb_cell_ids - page_active_ids)
            extra = sorted(page_active_ids - comb_cell_ids)
            raise RefereeError(
                f"{slug} page {page_index}: active ledger/cell reverse mapping "
                "disagrees"
                + (f"; missing ledger: {', '.join(missing[:8])}"
                   if missing else "")
                + (f"; unknown active: {', '.join(extra[:8])}"
                   if extra else ""))

        page_inference_keys: set[str] = set()
        page_inference_ids: set[str] = set()
        for index, inference in enumerate(inferences):
            label = f"{slug} page {page_index} inference {index}"
            if not isinstance(inference, dict):
                raise RefereeError(f"{label} is not an object")
            state = inference.get("state")
            if state != COMB_INFERENCE_STATE:
                raise RefereeError(
                    f"{label} has unknown or unsuppressed state: {state}")
            subject_key = inference.get("subject_key")
            cell_id = inference.get("cell_id")
            bbox_value = inference.get("bbox")
            if not isinstance(subject_key, str) or not isinstance(cell_id, str):
                raise RefereeError(f"{label} has no subject_key/cell_id")
            match = _SUBJECT_KEY_RE.fullmatch(subject_key)
            cell_match = _CELL_PAGE_RE.fullmatch(cell_id)
            if (match is None or int(match.group(1)) != page_index
                    or cell_match is None
                    or int(cell_match.group(1)) != page_index
                    or not isinstance(bbox_value, list)
                    or len(bbox_value) != 4):
                raise RefereeError(f"{label} identity is invalid")
            bbox_numbers = [
                finite_number(value, f"{label} bbox") for value in bbox_value
            ]
            if not same_numbers(
                    [float(match.group(item)) for item in range(2, 6)],
                    bbox_numbers):
                raise RefereeError(
                    f"{label} subject_key disagrees with bbox")
            if (subject_key in page_inference_keys
                    or cell_id in page_inference_ids
                    or subject_key in global_subject_keys
                    or cell_id in page_active_ids):
                raise RefereeError(
                    f"{label} duplicates a ledger subject or inference")
            cell = cells_by_id.get(cell_id)
            if (cell is None or cell.get("subject_key") != subject_key
                    or "comb" in cell
                    or not same_numbers(
                        bbox_numbers,
                        [cell.get(name)
                         for name in ("x0", "y0", "x1", "y1")])):
                raise RefereeError(
                    f"{label} does not map to one suppressed layout cell")
            if (inference.get("blocks_gate") is not True
                    or inference.get("requires_independent_evidence") is not True
                    or inference.get("permitted_transitions")
                    != ["active_reviewed"]):
                raise RefereeError(
                    f"{label} is not explicit and blocking")
            reason_codes = string_list(
                inference.get("reason_codes"), f"{label} reason_codes",
                nonempty=True)
            topology = validate_comb_topology(
                inference.get("inferred_comb"), bbox_numbers,
                f"{label} inferred_comb")
            page_inference_keys.add(subject_key)
            page_inference_ids.add(cell_id)
            inference_cell_ids.add(cell_id)
            published_inferences.append({
                "page": page_index,
                "subject_key": subject_key,
                "cell_id": cell_id,
                "state": state,
                "blocks_gate": True,
                "reason_codes": reason_codes,
                "bbox": bbox_numbers,
                "topology": topology,
                "ledger": inference,
            })

        stats = page.get("stats")
        if not isinstance(stats, dict):
            raise RefereeError(
                f"{slug} page {page_index}: layout stats are missing")
        active_resolved = sum(
            subject.get("state") == "active_resolved" for subject in subjects)
        active_unresolved = sum(
            subject.get("state") == "active_unresolved" for subject in subjects)
        retained = sum(
            subject.get("state") == "retained_unresolved"
            for subject in subjects)
        subject_blockers = sum(
            subject.get("blocks_gate") is True for subject in subjects)
        inference_blockers = sum(
            inference.get("blocks_gate") is True for inference in inferences)
        expected_stats = {
            "comb_cells": len(comb_cells),
            "comb_subjects": len(subjects),
            "comb_subjects_active": active_resolved + active_unresolved,
            "comb_subjects_active_resolved": active_resolved,
            "comb_subjects_active_unresolved": active_unresolved,
            "comb_subjects_retained_unresolved": retained,
            "comb_subjects_retired": 0,
            "comb_subjects_blocking": subject_blockers,
            "comb_inferences_suppressed": len(inferences),
            "comb_inferences_blocking": inference_blockers,
            "comb_evidence_blocking": (
                subject_blockers + inference_blockers),
            "comb_slots": sum(
                exact_nonnegative_int(
                    cell["comb"].get("cells"),
                    f"{slug} page {page_index} comb cells")
                for cell in comb_cells
            ),
        }
        for key, expected_value in expected_stats.items():
            if stats.get(key) != expected_value:
                raise RefereeError(
                    f"{slug} page {page_index}: ledger stat {key} "
                    f"is {stats.get(key)!r}, expected {expected_value}")

    if len(published_subjects) != expected_total:
        raise RefereeError(
            f"{slug}: subject ledger has {len(published_subjects)} subjects, "
            f"expected pinned {expected_total}")
    if len(global_subject_keys) != expected_total:
        raise RefereeError(f"{slug}: subject ledger identities are not unique")
    active_resolved = sum(
        subject["state"] == "active_resolved"
        for subject in published_subjects)
    active_unresolved = sum(
        subject["state"] == "active_unresolved"
        for subject in published_subjects)
    retained = sum(
        subject["state"] == "retained_unresolved"
        for subject in published_subjects)
    blockers = sum(
        subject["blocks_gate"] for subject in published_subjects
    ) + len(published_inferences)
    return {
        "lattice": lattice,
        "subjects": published_subjects,
        "inferences": published_inferences,
        "active_cell_ids": active_cell_ids,
        "retained_legacy_ids": retained_legacy_ids,
        "inference_cell_ids": inference_cell_ids,
        "counts": {
            "subjects": len(published_subjects),
            "active": active_resolved + active_unresolved,
            "active_resolved": active_resolved,
            "active_unresolved": active_unresolved,
            "retained_unresolved": retained,
            "inferences_suppressed": len(published_inferences),
            "blocking": blockers,
        },
    }


def validate_emission_inventory(
        ledger: dict[str, Any],
        slots: dict[str, dict[str, Any]],
        ) -> dict[str, Any]:
    """Bind every emitted comb to exactly one active ledger subject."""
    active_ids = set(ledger["active_cell_ids"])
    retained_ids = set(ledger["retained_legacy_ids"])
    inference_ids = set(ledger["inference_cell_ids"])
    emitted_ids = set(slots)
    missing = sorted(active_ids - emitted_ids)
    unexpected = sorted(emitted_ids - active_ids)
    retained_emitted = sorted(emitted_ids & retained_ids)
    inference_emitted = sorted(emitted_ids & inference_ids)
    invalid = sorted(
        cell_id for cell_id in active_ids & emitted_ids
        if not bool(slots[cell_id].get("valid"))
    )
    errors: list[str] = []
    if missing:
        errors.append(f"{len(missing)} active ledger subjects are not emitted")
    if unexpected:
        errors.append(f"{len(unexpected)} emitted combs have no active subject")
    if retained_emitted:
        errors.append(
            f"{len(retained_emitted)} retained subjects are still emitted")
    if inference_emitted:
        errors.append(
            f"{len(inference_emitted)} suppressed inferences are still emitted")
    if invalid:
        errors.append(f"{len(invalid)} active emissions are invalid")
    return {
        "complete": not errors,
        "reason": "complete" if not errors else "; ".join(errors),
        "expected_active_cell_ids": sorted(active_ids),
        "emitted_cell_ids": sorted(emitted_ids),
        "missing_active_cell_ids": missing,
        "unexpected_emitted_cell_ids": unexpected,
        "retained_emitted_cell_ids": retained_emitted,
        "inference_emitted_cell_ids": inference_emitted,
        "invalid_active_cell_ids": invalid,
    }


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


def classify_band(
        cell: dict[str, Any],
        page: SvgPage,
        *,
        ledger_state: str | None = None,
        _evaluation_window: tuple[float, float] | None = None,
        ) -> dict[str, Any]:
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
    cell_y0, cell_y1 = float(cell["y0"]), float(cell["y1"])
    band_attached_above = (
        seed_y0 < cell_y0
        and seed_y1 <= cell_y1
        and seed_y1 >= cell_y0 - POSITION_TOL_PT
    )
    band_attached_below = (
        seed_y0 >= cell_y0
        and seed_y1 > cell_y1
        and seed_y0 <= cell_y1 + POSITION_TOL_PT
    )
    attached_external_band = band_attached_above or band_attached_below
    evaluation_y0, evaluation_y1 = (
        _evaluation_window
        if _evaluation_window is not None
        else (cell_y0, cell_y1)
    )
    # Frame ownership and unsupported-gap exclusion must be certified in the
    # same vertical window that supplies the divider topology.  On the first
    # pass this is exactly the original cell rectangle.  On an attached-band
    # retry it prevents an unrelated frame inside the cell from proving an
    # empty multi-pitch gap in the external source band.
    proof_y0, proof_y1 = evaluation_y0, evaluation_y1
    max_width = pitch / 2
    candidates = [
        paint for paint in page.paints
        if abs(paint.tone - divider_tone) <= 1e-8
        and paint.width <= max_width
        and paint.height > paint.width
        and paint.x1 > x0 and paint.x0 < x1
        and paint.y1 > evaluation_y0 and paint.y0 < evaluation_y1
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
        endpoints = {proof_y0, proof_y1}
        for paint in page.paints:
            if (paint.x0 <= x <= paint.x1
                    and paint.y1 > proof_y0 and paint.y0 < proof_y1):
                endpoints.update((
                    max(proof_y0, paint.y0), min(proof_y1, paint.y1)))
        ordered = sorted(endpoints)
        for top, bottom in zip(ordered, ordered[1:]):
            if bottom - top <= 1e-9:
                continue
            owner = final_owner(x, (top + bottom) / 2)
            if (owner is None or owner.clipped
                    or abs(owner.tone - divider_tone) > 1e-8):
                if ((top <= proof_y0 + 1e-9
                     or bottom >= proof_y1 - 1e-9)
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
                and paint.y0 <= proof_y0 + POSITION_TOL_PT
                and paint.y1 >= proof_y0 - POSITION_TOL_PT
            ]
            bottom_lines = [
                paint for paint in element_paints
                if paint.width > paint.height
                and paint.x0 <= x0 + POSITION_TOL_PT
                and paint.x1 >= x1 - POSITION_TOL_PT
                and paint.y0 <= proof_y1 + POSITION_TOL_PT
                and paint.y1 >= proof_y1 - POSITION_TOL_PT
            ]
            left_lines = [
                paint for paint in element_paints
                if paint.height > paint.width
                and paint.y0 <= proof_y0 + POSITION_TOL_PT
                and paint.y1 >= proof_y1 - POSITION_TOL_PT
                and paint.x0 <= x0 + POSITION_TOL_PT
                and paint.x1 >= x0 - POSITION_TOL_PT
            ]
            right_lines = [
                paint for paint in element_paints
                if paint.height > paint.width
                and paint.y0 <= proof_y0 + POSITION_TOL_PT
                and paint.y1 >= proof_y1 - POSITION_TOL_PT
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
        # official forms carry outlined characters as broad curved paths and
        # small arrowheads as simple closed fills.  A bound that cannot itself
        # be a tall, narrow compartment boundary can affect topology only by
        # covering or joining an eligible source divider.  Divider-like bounds,
        # clipped simple fills, and structurally complex fills stay unsupported.
        vertical_overlap = (
            min(region.y1, seed_y1) - max(region.y0, seed_y0)
        )
        region_can_be_divider = (
            region.reason in (
                "curved SVG path",
                "non-rectangular closed SVG fill",
            )
            and region.x1 - region.x0 <= max_width
            and region.y1 - region.y0 > region.x1 - region.x0
            and vertical_overlap > (seed_y1 - seed_y0) / 2
        )
        occlusion_only = (
            "glyph use" in region.reason
            or (
                region.reason == "curved SVG path"
                and not region_can_be_divider
            )
            or (
                region.reason == "non-rectangular closed SVG fill"
                and not region.clipped
                and not region_can_be_divider
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
        a = max(evaluation_y0, paint.y0)
        b = min(evaluation_y1, paint.y1)
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

        # A thick page/frame edge is sometimes a stack of two target-tone
        # bars.  The inner bar can coincide with a stale lattice anchor, but
        # paper narrower than the combined ink weights is not a writable
        # compartment.  Frame evidence comes from all final components, not
        # the width-filtered divider candidates: a broad outer bar is precisely
        # the case that needs to disqualify its narrow neighbour.  Apply this
        # before both complete and partial anchor matching.
        frame_groups = [
            component for component in composited_segments(mid, page.paints)
            if abs(float(component["tone"]) - divider_tone) <= 1e-8
            and ((float(component["x0"]) <= x0 + POSITION_TOL_PT
                  and float(component["x1"]) >= x0 - POSITION_TOL_PT)
                 or (float(component["x0"]) <= x1 + POSITION_TOL_PT
                     and float(component["x1"])
                     >= x1 - POSITION_TOL_PT))
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

        matchable_groups = [
            group for group in groups
            if distinct_from_frames(group)
        ]

        # Match recognised anchors to independently painted source boundaries.
        # A referee must not silently move an anchor to the nearest plausible
        # line: source and lattice positions agree inside the repository's
        # fixed 0.25pt bound or they do not agree.
        available = list(range(len(matchable_groups)))
        anchor_matches: list[dict[str, float]] = []
        for anchor in anchors:
            choices = sorted(
                ((abs(matchable_groups[index]["x"] - anchor), index)
                 for index in available
                 if abs(matchable_groups[index]["x"] - anchor)
                 <= POSITION_TOL_PT),
                key=lambda item: (
                    item[0], matchable_groups[item[1]]["x"]),
            )
            if not choices:
                anchor_matches = []
                break
            distance, index = choices[0]
            available.remove(index)
            anchor_matches.append({
                "layout_x": round(anchor, 6),
                "source_x": matchable_groups[index]["x"],
                "delta_pt": round(
                    matchable_groups[index]["x"] - anchor, 6),
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
                and distinct_from_frames(group)
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
                if len(choices) != 1:
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
                missing_anchor_x = sorted(
                    round(float(anchor), 6)
                    for anchor in available_anchors)
                bands.append({
                    "status": "measured",
                    "y0": round(a, 6), "y1": round(b, 6),
                    "source_divider_x": partial_x,
                    "extra_divider_x": [],
                    "compartments": len(partial_x) + 1,
                    "anchor_matches": partial_matches,
                    "missing_anchor_x": missing_anchor_x,
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
        matched_groups = [matchable_groups[int(match["group_index"])]
                          for match in anchor_matches]
        if any(group["clipped"] for group in matched_groups):
            bands.append({
                "status": "unevaluable",
                "reason": "a recognised divider is under an unresolved SVG clip",
                "y0": a, "y1": b,
            })
            continue

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
                    and region.y1 > proof_y0 and region.y0 < proof_y1
                    and min(region.x1, right) - max(region.x0, left)
                    > POSITION_TOL_PT
                    and min(region.y1, proof_y1) - max(region.y0, proof_y0)
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
        result = {
            "status": "unevaluable", "reason": reason,
            **coverage_evidence, "bands": bands,
        }
        # Preserve the original cell-clipped referee as the first and
        # authoritative attempt.  Only its exact empty-band verdict may retry
        # against a complete source band attached across/outside one cell edge.
        # Detached and two-edge-enveloping bands never retry, and every other
        # fail-closed verdict remains untouched.
        if (_evaluation_window is None
                and not bands
                and reason == (
                    "no common Poppler band contains every recognised divider")
                and attached_external_band):
            return classify_band(
                cell, page, ledger_state=ledger_state,
                _evaluation_window=(seed_y0, seed_y1))
        return result
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
        # An already-active comb can use source absence as evidence against a
        # stale lattice anchor, but only when the source proof is exhaustive.
        # This path must never discover a new comb: retained subjects and raw
        # table/label cells remain ineligible, and every observed divider must
        # still map one-to-one to a declared anchor.  At every remaining anchor
        # Poppler must expose the raw rail and one supported non-target owner
        # must finally erase it across the whole open band.  That proves the
        # smaller final topology without assuming the lattice count is correct.
        partial_bands = [
            band for band in measured
            if not bool(band.get("anchors_complete"))
        ]
        missing_sets = {
            tuple(float(value) for value in band.get("missing_anchor_x", ()))
            for band in partial_bands
        }
        observed_anchor_sets = {
            tuple(sorted(
                float(match["layout_x"])
                for match in band.get("anchor_matches", ())))
            for band in partial_bands
        }
        partial_components_valid = all(
            band.get("source_divider_x")
            and len(band.get("anchor_matches", ()))
            == len(band.get("source_divider_x", ()))
            and len({
                float(match["layout_x"])
                for match in band.get("anchor_matches", ())
            }) == len(band.get("anchor_matches", ()))
            and all(
                not bool(component.get("clipped"))
                for component in band.get("components", ())
            )
            for band in partial_bands
        )
        full_partial_coverage = (
            ledger_state == "active_unresolved"
            and len(topologies) == 1
            and len(partial_bands) == len(measured)
            and bool(partial_bands)
            and not ignored_slabs
            and abs(measured_span - seed_span) <= 1e-6
            and abs(topology_coverage[chosen_topology] - seed_span) <= 1e-6
            and len(missing_sets) == 1
            and len(observed_anchor_sets) == 1
            and partial_components_valid
        )
        missing_anchor_proofs: list[dict[str, Any]] = []
        anchor_corridor_clipped_paints: list[Paint] = []
        anchor_corridor_unsupported_regions: list[UnsupportedRegion] = []
        if full_partial_coverage:
            missing_anchor_x = sorted(next(iter(missing_sets)))
            observed_anchor_x = sorted(next(iter(observed_anchor_sets)))
            if not missing_anchor_x:
                full_partial_coverage = False
            declared_anchor_x = sorted({
                *observed_anchor_x,
                *missing_anchor_x,
            })
            anchor_corridor_clipped_paints = [
                paint for paint in page.paints
                if paint.clipped
                and any(
                    paint.x1 > anchor - POSITION_TOL_PT
                    and paint.x0 < anchor + POSITION_TOL_PT
                    for anchor in declared_anchor_x
                )
                and min(paint.y1, seed_y1) - max(paint.y0, seed_y0)
                > 1e-9
            ]
            anchor_corridor_unsupported_regions = [
                region for region in page.unsupported
                if any(
                    region.x1 > anchor - POSITION_TOL_PT
                    and region.x0 < anchor + POSITION_TOL_PT
                    for anchor in declared_anchor_x
                )
                and min(region.y1, seed_y1) - max(region.y0, seed_y0)
                > 1e-9
            ]
            if (anchor_corridor_clipped_paints
                    or anchor_corridor_unsupported_regions):
                full_partial_coverage = False
            for anchor in missing_anchor_x:
                corridor_x0 = anchor - POSITION_TOL_PT
                corridor_x1 = anchor + POSITION_TOL_PT
                raw_anchor_rails = [
                    paint for paint in page.paints
                    if abs(paint.tone - divider_tone) <= 1e-8
                    and paint.width <= max_width
                    and paint.height > paint.width
                    and near(paint.cx, anchor)
                    and sum(
                        near(paint.cx, missing)
                        for missing in declared_anchor_x
                    ) == 1
                    and paint.x1 > corridor_x0 and paint.x0 < corridor_x1
                    and min(paint.y1, seed_y1) - max(paint.y0, seed_y0)
                    > POSITION_TOL_PT
                ]
                proof_x0 = min(
                    [corridor_x0,
                     *(paint.x0 for paint in raw_anchor_rails)])
                proof_x1 = max(
                    [corridor_x1,
                     *(paint.x1 for paint in raw_anchor_rails)])
                clipped_paints = [
                    paint for paint in page.paints
                    if paint.clipped
                    and paint.x1 > proof_x0 and paint.x0 < proof_x1
                    and min(paint.y1, seed_y1) - max(paint.y0, seed_y0)
                    > 1e-9
                ]
                unsupported_regions = [
                    region for region in page.unsupported
                    if region.x1 > proof_x0 and region.x0 < proof_x1
                    and min(region.y1, seed_y1) - max(region.y0, seed_y0)
                    > 1e-9
                ]
                final_target_segments: list[dict[str, float]] = []
                erasure_slabs: list[dict[str, Any]] = []
                erasure_roles: set[tuple[str, int, str, float]] = set()
                proof_top_role_ambiguities: list[dict[str, Any]] = []
                raw_rail_identity_valid = (
                    len(raw_anchor_rails) == 1
                    and raw_anchor_rails[0].y0 <= seed_y0 + 1e-6
                    and raw_anchor_rails[0].y1 >= seed_y1 - 1e-6
                )
                erasure_valid = raw_rail_identity_valid
                for band in measured:
                    mid = (float(band["y0"]) + float(band["y1"])) / 2
                    final_segments = composited_segments(mid, page.paints)
                    for segment in final_segments:
                        if (abs(float(segment["tone"]) - divider_tone)
                                <= 1e-8
                                and float(segment["x1"]) > proof_x0
                                and float(segment["x0"]) < proof_x1):
                            final_target_segments.append({
                                "y": round(mid, 6),
                                "x0": round(float(segment["x0"]), 6),
                                "x1": round(float(segment["x1"]), 6),
                            })
                    proof_active_paints = [
                        paint for paint in page.paints
                        if paint.y0 <= mid <= paint.y1
                        and paint.x1 > proof_x0 and paint.x0 < proof_x1
                    ]
                    proof_endpoints = {proof_x0, proof_x1}
                    for paint in proof_active_paints:
                        proof_endpoints.update((
                            max(proof_x0, paint.x0),
                            min(proof_x1, paint.x1),
                        ))
                    ordered_proof_x = sorted(proof_endpoints)
                    for left, right in zip(
                            ordered_proof_x, ordered_proof_x[1:]):
                        if right - left <= 1e-9:
                            continue
                        sample_x = (left + right) / 2
                        owners = [
                            paint for paint in proof_active_paints
                            if paint.x0 < sample_x < paint.x1
                        ]
                        if not owners:
                            continue
                        max_order = max(paint.order for paint in owners)
                        top_roles = sorted({
                            (
                                paint.element,
                                paint.order,
                                paint.kind,
                                round(paint.tone, 8),
                                paint.clipped,
                            )
                            for paint in owners
                            if paint.order == max_order
                        })
                        if len(top_roles) > 1:
                            erasure_valid = False
                            proof_top_role_ambiguities.append({
                                "y": round(mid, 6),
                                "x0": round(left, 6),
                                "x1": round(right, 6),
                                "roles": [
                                    {
                                        "element": role[0],
                                        "order": role[1],
                                        "kind": role[2],
                                        "tone": role[3],
                                        "clipped": role[4],
                                    }
                                    for role in top_roles
                                ],
                            })
                    active_rails = [
                        paint for paint in raw_anchor_rails
                        if paint.y0 <= mid <= paint.y1
                    ]
                    raw_intervals = sorted(
                        (paint.x0, paint.x1)
                        for paint in active_rails
                        if paint.x1 - paint.x0 > 1e-9
                    )
                    merged_raw: list[list[float]] = []
                    for left, right in raw_intervals:
                        if (merged_raw
                                and left <= merged_raw[-1][1] + 1e-6):
                            merged_raw[-1][1] = max(
                                merged_raw[-1][1], right)
                        else:
                            merged_raw.append([left, right])
                    slab_evidence: dict[str, Any] = {
                        "y0": round(float(band["y0"]), 6),
                        "y1": round(float(band["y1"]), 6),
                        "sample_y": round(mid, 6),
                        "raw_rail_elements": sorted({
                            paint.element for paint in active_rails
                        }),
                        "raw_intervals": [
                            [round(left, 6), round(right, 6)]
                            for left, right in merged_raw
                        ],
                        "final_owner_segments": [],
                        "ambiguous_top_roles": [],
                    }
                    slab_roles: set[tuple[str, int, str, float]] = set()
                    if len(merged_raw) != 1:
                        erasure_valid = False
                    for raw_left, raw_right in merged_raw:
                        endpoints = {raw_left, raw_right}
                        active_paints = [
                            paint for paint in page.paints
                            if paint.y0 <= mid <= paint.y1
                            and paint.x1 > raw_left and paint.x0 < raw_right
                        ]
                        for paint in active_paints:
                            endpoints.update((
                                max(raw_left, paint.x0),
                                min(raw_right, paint.x1),
                            ))
                        ordered_x = sorted(endpoints)
                        for left, right in zip(ordered_x, ordered_x[1:]):
                            if right - left <= 1e-9:
                                continue
                            sample_x = (left + right) / 2
                            owners = [
                                paint for paint in active_paints
                                if paint.x0 < sample_x < paint.x1
                            ]
                            if not owners:
                                erasure_valid = False
                                continue
                            max_order = max(paint.order for paint in owners)
                            top_owners = [
                                paint for paint in owners
                                if paint.order == max_order
                            ]
                            top_roles = sorted({
                                (
                                    paint.element,
                                    paint.order,
                                    paint.kind,
                                    round(paint.tone, 8),
                                    paint.clipped,
                                )
                                for paint in top_owners
                            })
                            if len(top_roles) != 1:
                                erasure_valid = False
                                slab_evidence["ambiguous_top_roles"].append([
                                    {
                                        "element": role[0],
                                        "order": role[1],
                                        "kind": role[2],
                                        "tone": role[3],
                                        "clipped": role[4],
                                    }
                                    for role in top_roles
                                ])
                                continue
                            owner = top_owners[0]
                            role = (
                                owner.element,
                                owner.order,
                                owner.kind,
                                round(owner.tone, 8),
                            )
                            slab_roles.add(role)
                            erasure_roles.add(role)
                            slab_evidence["final_owner_segments"].append({
                                "x0": round(left, 6),
                                "x1": round(right, 6),
                                "element": owner.element,
                                "order": owner.order,
                                "kind": owner.kind,
                                "tone": round(owner.tone, 8),
                                "clipped": owner.clipped,
                            })
                            if (owner.clipped
                                    or abs(owner.tone - divider_tone)
                                    <= 1e-8):
                                erasure_valid = False
                    if len(slab_roles) != 1:
                        erasure_valid = False
                    erasure_slabs.append(slab_evidence)
                if len(erasure_roles) != 1:
                    erasure_valid = False
                proof = {
                    "layout_x": round(anchor, 6),
                    "corridor_x0": round(corridor_x0, 6),
                    "corridor_x1": round(corridor_x1, 6),
                    "proof_x0": round(proof_x0, 6),
                    "proof_x1": round(proof_x1, 6),
                    "open_y0": round(seed_y0, 6),
                    "open_y1": round(seed_y1, 6),
                    "raw_anchor_rails": [
                        {
                            "element": paint.element,
                            "order": paint.order,
                            "kind": paint.kind,
                            "x0": round(paint.x0, 6),
                            "x1": round(paint.x1, 6),
                            "center_x": round(paint.cx, 6),
                            "delta_pt": round(paint.cx - anchor, 6),
                            "y0": round(paint.y0, 6),
                            "y1": round(paint.y1, 6),
                            "tone": round(paint.tone, 8),
                            "clipped": paint.clipped,
                        }
                        for paint in sorted(
                            raw_anchor_rails,
                            key=lambda item: (
                                item.order, item.element,
                                item.x0, item.y0, item.x1, item.y1),
                        )
                    ],
                    "raw_rail_identity_valid": raw_rail_identity_valid,
                    "proof_top_role_ambiguities": (
                        proof_top_role_ambiguities),
                    "erasure_slabs": erasure_slabs,
                    "erasure_owner_roles": [
                        {
                            "element": role[0],
                            "order": role[1],
                            "kind": role[2],
                            "tone": role[3],
                        }
                        for role in sorted(erasure_roles)
                    ],
                    "clipped_paint_elements": sorted({
                        paint.element for paint in clipped_paints
                    }),
                    "final_target_tone_segments": final_target_segments,
                    "unsupported_region_elements": sorted({
                        region.element for region in unsupported_regions
                    }),
                }
                missing_anchor_proofs.append(proof)
                if (not erasure_valid
                        or clipped_paints or unsupported_regions
                        or final_target_segments):
                    full_partial_coverage = False
            if full_partial_coverage:
                certificate = {
                    "criterion": ACTIVE_PARTIAL_ANCHOR_CRITERION,
                    "valid": True,
                    "ledger_state": ledger_state,
                    "subject_ownership_basis": (
                        "active_unresolved lattice ledger"
                    ),
                    "independent_source_enclosure_proven": False,
                    "divider_count_basis": (
                        "final-composited Poppler vector topology"
                    ),
                    "missing_anchor_basis": (
                        "raw target-tone rail exhaustively replaced by one "
                        "supported unclipped non-target final owner"
                    ),
                    "anchor_corridor_clipped_paint_elements": sorted({
                        paint.element
                        for paint in anchor_corridor_clipped_paints
                    }),
                    "anchor_corridor_unsupported_region_elements": sorted({
                        region.element
                        for region in anchor_corridor_unsupported_regions
                    }),
                    "open_y0": round(seed_y0, 6),
                    "open_y1": round(seed_y1, 6),
                    "coverage_pt": round(measured_span, 6),
                    "source_divider_x": list(chosen_topology),
                    "observed_anchor_x": observed_anchor_x,
                    "missing_anchor_x": missing_anchor_x,
                    "missing_anchor_proofs": missing_anchor_proofs,
                }
                return {
                    "status": "measured",
                    "reason": (
                        "ledger-owned active subject has full-band Poppler "
                        "proof of erased lattice anchors"
                    ),
                    **{key: value for key, value in chosen.items()
                       if key != "status"},
                    **coverage_evidence,
                    "chosen_topology": list(chosen_topology),
                    "topology_superset_relations": superset_relations,
                    "active_partial_anchor_certificate": certificate,
                }
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


def _audit_optional_int(value: Any, label: str) -> int | None:
    if value is None:
        return None
    return exact_nonnegative_int(value, label)


def _audit_number_list(value: Any, label: str) -> list[float] | None:
    if value is None:
        return None
    if not isinstance(value, list):
        raise RefereeError(f"{label} is not a numeric list")
    return [
        finite_number(item, f"{label}[{index}]")
        for index, item in enumerate(value)
    ]


def validate_audit_position_evidence(
        name: str,
        value: Any,
        *,
        outer: bool,
        ) -> bool:
    """Validate one independently published fixed-tolerance relation."""
    if not isinstance(value, dict):
        raise RefereeError(f"audit offender {name} is not an object")
    axis = "outer" if outer else "internal"
    actual_key = f"actual_{axis}_edges_x"
    expected_key = f"expected_{axis}_edges_x"
    required = {
        "comparable", "tolerance_pt", actual_key, expected_key,
        "count_matches", "deltas_pt", "matches",
    }
    allowed = required | {"unavailable_reason"}
    if not required <= set(value) or set(value) - allowed:
        raise RefereeError(
            f"audit offender {name} has an unsupported evidence schema")
    comparable = value["comparable"]
    if not isinstance(comparable, bool):
        raise RefereeError(f"audit offender {name}.comparable is not boolean")
    tolerance = finite_number(
        value["tolerance_pt"], f"audit offender {name}.tolerance_pt")
    if abs(tolerance - HTML_GEOMETRY_EPSILON_PT) > 1e-12:
        raise RefereeError(
            f"audit offender {name} changes the fixed position tolerance")
    actual = _audit_number_list(
        value[actual_key], f"audit offender {name}.{actual_key}")
    expected = _audit_number_list(
        value[expected_key], f"audit offender {name}.{expected_key}")
    if not comparable:
        if (value["count_matches"] is not None
                or value["deltas_pt"] is not None
                or value["matches"] is not None
                or not isinstance(value.get("unavailable_reason"), str)
                or not value["unavailable_reason"]):
            raise RefereeError(
                f"audit offender {name} has malformed unavailable evidence")
        return False
    if not isinstance(value["count_matches"], bool):
        raise RefereeError(
            f"audit offender {name}.count_matches is not boolean")
    count_matches = actual is not None and expected is not None and (
        len(actual) == len(expected))
    if value["count_matches"] is not count_matches:
        raise RefereeError(
            f"audit offender {name} has a false edge-count relation")
    deltas = _audit_number_list(
        value["deltas_pt"], f"audit offender {name}.deltas_pt")
    expected_deltas = (
        [round(left - right, 6) for left, right in zip(actual, expected)]
        if count_matches and actual is not None and expected is not None
        else None
    )
    if ((deltas is None) != (expected_deltas is None)
            or (deltas is not None and expected_deltas is not None
                and not same_numbers(deltas, expected_deltas))):
        raise RefereeError(
            f"audit offender {name} has false edge deltas")
    matches = bool(
        count_matches
        and all(abs(delta) <= tolerance for delta in expected_deltas or ())
    )
    if not isinstance(value["matches"], bool) or value["matches"] is not matches:
        raise RefereeError(
            f"audit offender {name} has a false position verdict")
    return not matches


def validate_audit_container_binding(value: Any) -> dict[str, bool]:
    if not isinstance(value, dict) or set(value) != {
        "expected_page", "emitted_id_page", "emitted_dom_page",
        "page_matches", "expected_rect", "actual_rect", "rect_deltas_pt",
        "rect_matches", "tolerance_pt",
    }:
        raise RefereeError(
            "audit offender has malformed emission container evidence")
    expected_page = exact_nonnegative_int(
        value["expected_page"], "audit offender expected page")
    if expected_page == 0:
        raise RefereeError("audit offender expected page is not one-based")
    emitted_id_page = _audit_optional_int(
        value["emitted_id_page"], "audit offender emitted id page")
    emitted_dom_page = _audit_optional_int(
        value["emitted_dom_page"], "audit offender emitted DOM page")
    expected_rect = _audit_number_list(
        value["expected_rect"], "audit offender expected rect")
    actual_rect = _audit_number_list(
        value["actual_rect"], "audit offender actual rect")
    if expected_rect is None or len(expected_rect) != 4:
        raise RefereeError("audit offender expected rect is not four numbers")
    if actual_rect is not None and len(actual_rect) != 4:
        raise RefereeError("audit offender actual rect is not four numbers")
    page_matches = (
        emitted_id_page == expected_page
        and emitted_dom_page == expected_page
    )
    if (not isinstance(value["page_matches"], bool)
            or value["page_matches"] is not page_matches):
        raise RefereeError("audit offender has a false container-page relation")
    deltas = _audit_number_list(
        value["rect_deltas_pt"], "audit offender rect deltas")
    expected_deltas = (
        [left - right for left, right in zip(actual_rect, expected_rect)]
        if actual_rect is not None else None
    )
    if ((deltas is None) != (expected_deltas is None)
            or (deltas is not None and expected_deltas is not None
                and not same_numbers(deltas, expected_deltas))):
        raise RefereeError("audit offender has false container deltas")
    tolerance = finite_number(
        value["tolerance_pt"], "audit offender container tolerance")
    if abs(tolerance - HTML_GEOMETRY_EPSILON_PT) > 1e-12:
        raise RefereeError(
            "audit offender changes the fixed container tolerance")
    rect_matches = bool(
        expected_deltas is not None
        and all(abs(delta) <= tolerance for delta in expected_deltas)
    )
    if (not isinstance(value["rect_matches"], bool)
            or value["rect_matches"] is not rect_matches):
        raise RefereeError("audit offender has a false container-rect relation")
    return {
        "page_mismatch": not page_matches,
        "rect_mismatch": not rect_matches,
    }


def audit_offender_dimensions(
        item: Any,
        expected_owner: dict[str, Any] | None = None,
        ) -> dict[str, Any]:
    """Re-derive every published offender relation from its raw evidence."""
    if not isinstance(item, dict):
        raise RefereeError("audit offender is not an object")
    required = {
        "cell", "page", "slots", "latticed", "printed",
        "printed_divider_x", "emission_state", "physical_slots",
        "declared_slots", "emitted_occurrences", "layout_relation",
        "emission_relation", "failure_kinds", "why",
    }
    allowed = required | {
        "slot_indexes", "input_slot_indexes", "slot_geometry",
        "emission_container_binding",
        "emission_layout_position", "emission_layout_outer_position",
        "emission_source_position", "source_frame_geometry",
        "emission_source_outer_position", "layout_source_outer_position",
        "source_topology_evidence", "effective_emission_state",
        "source_owner_certificate",
        "emitted_cell_binding_evidence", "raw_dom_evidence",
    }
    if not required <= set(item) or set(item) - allowed:
        raise RefereeError("audit offender has an unsupported schema")
    cell_id = item["cell"]
    if not isinstance(cell_id, str) or not cell_id:
        raise RefereeError("audit offender cell identity is missing")
    if (not _CELL_RE.fullmatch(cell_id)
            and not (cell_id.startswith("<") and cell_id.endswith(">"))):
        # A malformed live marker may carry its literal noncanonical id, but
        # only the raw-DOM relation is allowed to publish it.
        if item.get("failure_kinds") != ["unowned-live-comb-markup"]:
            raise RefereeError("audit offender cell identity is not canonical")
    page = _audit_optional_int(item["page"], f"audit offender {cell_id} page")
    if page == 0:
        raise RefereeError(f"audit offender {cell_id} page is not one-based")
    slots = _audit_optional_int(
        item["slots"], f"audit offender {cell_id} slots")
    latticed = _audit_optional_int(
        item["latticed"], f"audit offender {cell_id} latticed")
    printed = _audit_optional_int(
        item["printed"], f"audit offender {cell_id} printed")
    physical = _audit_optional_int(
        item["physical_slots"], f"audit offender {cell_id} physical slots")
    _audit_optional_int(
        item["declared_slots"], f"audit offender {cell_id} declared slots")
    occurrences = exact_nonnegative_int(
        item["emitted_occurrences"],
        f"audit offender {cell_id} emitted occurrences")
    if slots is not None and physical is not None and slots != physical:
        raise RefereeError(
            f"audit offender {cell_id} slots disagree with physical slots")
    divider_x = _audit_number_list(
        item["printed_divider_x"],
        f"audit offender {cell_id} printed dividers")
    if divider_x is None:
        raise RefereeError(
            f"audit offender {cell_id} printed dividers are missing")
    if printed is None:
        if divider_x:
            raise RefereeError(
                f"audit offender {cell_id} has dividers without a result")
    elif len(divider_x) != max(0, printed - 1):
        raise RefereeError(
            f"audit offender {cell_id} printed topology is inconsistent")
    failure_kinds = string_list(
        item["failure_kinds"],
        f"audit offender {cell_id} failure kinds",
        nonempty=True,
    )
    unknown_kinds = set(failure_kinds) - AUDIT_FAILURE_KINDS
    if unknown_kinds:
        raise RefereeError(
            f"audit offender {cell_id} has unsupported failure kinds: "
            + ", ".join(sorted(unknown_kinds)))
    if not isinstance(item["why"], str) or not item["why"]:
        raise RefereeError(f"audit offender {cell_id} has no explanation")
    if not isinstance(item["emission_state"], str) or not item["emission_state"]:
        raise RefereeError(f"audit offender {cell_id} has no emission state")

    layout_relation = item["layout_relation"]
    if layout_relation == "match":
        if printed is None or latticed is None or printed != latticed:
            raise RefereeError(
                f"audit offender {cell_id} has a false layout match")
        expected_layout_kind = None
    elif layout_relation == "mismatch":
        if printed is None or latticed is None or printed == latticed:
            raise RefereeError(
                f"audit offender {cell_id} has a false layout mismatch")
        expected_layout_kind = "layout-printed-mismatch"
    elif layout_relation == "unevaluable":
        if printed is not None:
            raise RefereeError(
                f"audit offender {cell_id} hides a measured source topology")
        expected_layout_kind = "source-topology-unevaluable"
    elif layout_relation == "duplicate-subject":
        expected_layout_kind = "duplicate-layout-subject"
    elif layout_relation == "registry-invalid":
        expected_layout_kind = None
    elif layout_relation in {
            "not-owned", "cell-binding-invalid", "inventory-invalid"}:
        expected_layout_kind = None
    else:
        raise RefereeError(
            f"audit offender {cell_id} has unsupported layout relation")
    for kind in {
            "layout-printed-mismatch", "source-topology-unevaluable",
            "duplicate-layout-subject"}:
        if ((kind in failure_kinds)
                != (kind == expected_layout_kind)):
            raise RefereeError(
                f"audit offender {cell_id} has a false {kind} relation")

    normal_subject = layout_relation in {"match", "mismatch", "unevaluable"}
    owner_certificate: dict[str, Any] | None = None
    if normal_subject or layout_relation == "duplicate-subject":
        owner_certificate = validate_audit_owner_certificate(
            item.get("source_owner_certificate"), expected_owner)
        if layout_relation == "duplicate-subject":
            if owner_certificate.get("valid") is not False:
                raise RefereeError(
                    f"audit duplicate subject {cell_id} has a valid owner "
                    "certificate")
            if "source_topology_evidence" in item:
                raise RefereeError(
                    f"audit duplicate subject {cell_id} invents source "
                    "topology evidence")
        if owner_certificate.get("valid") is False:
            if (printed is not None
                    or layout_relation not in {
                        "unevaluable", "duplicate-subject"}
                    or item.get("source_frame_geometry") is not None):
                raise RefereeError(
                    f"audit offender {cell_id} lets an invalid owner "
                    "certificate supply source topology")
            if normal_subject:
                topology = item.get("source_topology_evidence")
                if (not isinstance(topology, dict)
                        or set(topology) != {
                            "criterion", "owner_certificate"}
                        or topology.get("criterion")
                        != AUDIT_OWNER_CERTIFICATE_CRITERION
                        or topology.get("owner_certificate")
                        != owner_certificate):
                    raise RefereeError(
                        f"audit offender {cell_id} has malformed invalid-owner "
                        "topology evidence")
    elif layout_relation == "registry-invalid":
        owner_certificate = validate_audit_owner_certificate(
            item.get("source_owner_certificate"), None)
        if (owner_certificate.get("valid") is not False
                or cell_id != "<comb-owner-registry>"
                or page is not None
                or any(value is not None for value in (
                    slots, latticed, printed, physical,
                    item["declared_slots"]))
                or divider_x != []
                or occurrences != 0
                or item["emission_state"] != "not-evaluated"
                or item.get("effective_emission_state") != "not-evaluated"
                or item.get("emission_relation") != "not-evaluated"
                or failure_kinds != ["comb-owner-registry-invalid"]
                or "source_topology_evidence" in item
                or "source_frame_geometry" in item):
            raise RefereeError(
                "audit comb owner-registry offender is malformed")
    elif "source_owner_certificate" in item:
        raise RefereeError(
            f"non-owned audit offender invents owner certificate: {cell_id}")

    topology_evidence = item.get("source_topology_evidence")
    if topology_evidence is not None:
        if not isinstance(topology_evidence, dict):
            raise RefereeError(
                f"audit offender {cell_id} source topology evidence is malformed")
        nested_owner = topology_evidence.get("owner_certificate")
        if nested_owner is not None and nested_owner != owner_certificate:
            raise RefereeError(
                f"audit offender {cell_id} topology owner certificate differs")

    position_mismatch = False
    for field, (kind, outer) in AUDIT_POSITION_FIELDS.items():
        present = field in item
        if normal_subject and not present:
            raise RefereeError(
                f"audit offender {cell_id} omits {field}")
        mismatch = (
            validate_audit_position_evidence(
                field, item[field], outer=outer)
            if present else False
        )
        if (kind in failure_kinds) != mismatch:
            raise RefereeError(
                f"audit offender {cell_id} has a false {kind} relation")
        position_mismatch = position_mismatch or mismatch
    if normal_subject:
        layout_internal = item["emission_layout_position"]
        source_internal = item["emission_source_position"]
        layout_outer = item["emission_layout_outer_position"]
        source_outer = item["emission_source_outer_position"]
        layout_source_outer = item["layout_source_outer_position"]
        internal_actual = _audit_number_list(
            layout_internal["actual_internal_edges_x"],
            f"audit offender {cell_id} layout actual edges")
        source_actual = _audit_number_list(
            source_internal["actual_internal_edges_x"],
            f"audit offender {cell_id} source actual edges")
        if (internal_actual is not None and source_actual is not None
                and not same_numbers(internal_actual, source_actual)):
            raise RefereeError(
                f"audit offender {cell_id} publishes two emitted edge vectors")
        source_expected = _audit_number_list(
            source_internal["expected_internal_edges_x"],
            f"audit offender {cell_id} source expected edges")
        if (printed is None) != (source_expected is None):
            raise RefereeError(
                f"audit offender {cell_id} source divider availability is false")
        if (printed is not None and source_expected is not None
                and not same_numbers(source_expected, divider_x)):
            raise RefereeError(
                f"audit offender {cell_id} source divider evidence disagrees")
        emitted_outer_a = _audit_number_list(
            layout_outer["actual_outer_edges_x"],
            f"audit offender {cell_id} layout actual outer edges")
        emitted_outer_b = _audit_number_list(
            source_outer["actual_outer_edges_x"],
            f"audit offender {cell_id} source actual outer edges")
        if (emitted_outer_a is not None and emitted_outer_b is not None
                and not same_numbers(emitted_outer_a, emitted_outer_b)):
            raise RefereeError(
                f"audit offender {cell_id} publishes two emitted outer vectors")
        layout_expected_outer = _audit_number_list(
            layout_outer["expected_outer_edges_x"],
            f"audit offender {cell_id} layout expected outer edges")
        layout_source_actual = _audit_number_list(
            layout_source_outer["actual_outer_edges_x"],
            f"audit offender {cell_id} layout/source actual outer edges")
        if (layout_expected_outer is not None
                and layout_source_actual is not None
                and not same_numbers(
                    layout_expected_outer, layout_source_actual)):
            raise RefereeError(
                f"audit offender {cell_id} publishes two layout outer vectors")
        source_expected_outer = _audit_number_list(
            source_outer["expected_outer_edges_x"],
            f"audit offender {cell_id} source expected outer edges")
        layout_source_expected = _audit_number_list(
            layout_source_outer["expected_outer_edges_x"],
            f"audit offender {cell_id} layout/source expected outer edges")
        if (source_expected_outer is not None
                and layout_source_expected is not None
                and not same_numbers(
                    source_expected_outer, layout_source_expected)):
            raise RefereeError(
                f"audit offender {cell_id} publishes two source outer vectors")
        frame = item.get("source_frame_geometry")
        if printed is None:
            if frame is not None:
                raise RefereeError(
                    f"audit offender {cell_id} has a frame without topology")
        elif frame is not None:
            if not isinstance(frame, dict):
                raise RefereeError(
                    f"audit offender {cell_id} source frame is malformed")
            try:
                frame_edges = [
                    finite_number(
                        frame["left_rail"]["center_x"],
                        f"audit offender {cell_id} left source rail"),
                    finite_number(
                        frame["right_rail"]["center_x"],
                        f"audit offender {cell_id} right source rail"),
                ]
            except (KeyError, TypeError):
                raise RefereeError(
                    f"audit offender {cell_id} source frame is malformed")
            if (source_expected_outer is None
                    or layout_source_expected is None
                    or not same_numbers(frame_edges, source_expected_outer)
                    or not same_numbers(frame_edges, layout_source_expected)):
                raise RefereeError(
                    f"audit offender {cell_id} source rails disagree")
        elif (source_expected_outer is not None
              or layout_source_expected is not None
              or owner_certificate is None
              or owner_certificate.get("valid") is not True):
            raise RefereeError(
                f"audit offender {cell_id} has an uncertified unframed "
                "source topology")

    container_mismatch = False
    if normal_subject:
        if "emission_container_binding" not in item:
            raise RefereeError(
                f"audit offender {cell_id} omits container binding evidence")
        container = validate_audit_container_binding(
            item["emission_container_binding"])
        expected_page_kind = bool(
            occurrences == 1 and container["page_mismatch"])
        expected_rect_kind = bool(
            occurrences == 1 and container["rect_mismatch"])
        if (("emission-container-page-mismatch" in failure_kinds)
                != expected_page_kind):
            raise RefereeError(
                f"audit offender {cell_id} has a false container-page failure")
        if (("emission-container-geometry-mismatch" in failure_kinds)
                != expected_rect_kind):
            raise RefereeError(
                f"audit offender {cell_id} has a false container-rect failure")
        container_mismatch = expected_page_kind or expected_rect_kind
    elif any(
            kind in failure_kinds for kind in {
                "emission-container-page-mismatch",
                "emission-container-geometry-mismatch",
            }):
        raise RefereeError(
            f"audit offender {cell_id} has unbound container failures")

    binding_invalid = container_mismatch or position_mismatch
    physical_emission_valid = item["emission_state"] == "physical-slots"
    if normal_subject:
        if (("invalid-emission" in failure_kinds)
                != (not physical_emission_valid)):
            raise RefereeError(
                f"audit offender {cell_id} has a false invalid-emission flag")
        layout_slot_mismatch = (
            slots is not None and latticed is not None and slots != latticed)
        printed_slot_mismatch = (
            slots is not None and printed is not None and slots != printed)
        if (("emission-layout-mismatch" in failure_kinds)
                != (physical_emission_valid and layout_slot_mismatch)):
            raise RefereeError(
                f"audit offender {cell_id} has a false layout-slot relation")
        if (("emission-printed-mismatch" in failure_kinds)
                != (physical_emission_valid and printed_slot_mismatch)):
            raise RefereeError(
                f"audit offender {cell_id} has a false printed-slot relation")
        if not physical_emission_valid or binding_invalid:
            expected_emission_relation = "invalid"
        else:
            mismatched = [
                name for name, mismatch in (
                    ("layout", layout_slot_mismatch),
                    ("printed", printed_slot_mismatch),
                ) if mismatch
            ]
            expected_emission_relation = (
                "mismatch-" + "-and-".join(mismatched)
                if mismatched else "match"
            )
        if item["emission_relation"] != expected_emission_relation:
            raise RefereeError(
                f"audit offender {cell_id} has a false emission relation")
        if "effective_emission_state" in item:
            expected_state = (
                "container-binding-invalid"
                if container_mismatch else
                "slot-position-invalid"
                if position_mismatch else
                item["emission_state"]
            )
            if item["effective_emission_state"] != expected_state:
                raise RefereeError(
                    f"audit offender {cell_id} has a false effective state")
    elif layout_relation == "duplicate-subject":
        if item["emission_relation"] not in {"invalid", "unbound"}:
            raise RefereeError(
                f"audit offender {cell_id} has false duplicate binding")
    elif layout_relation == "not-owned":
        expected_kind = (
            "unowned-live-comb-markup"
            if "unowned-live-comb-markup" in failure_kinds
            else "unexpected-emitted-comb"
        )
        expected_relation = (
            "invalid" if expected_kind == "unowned-live-comb-markup"
            else "unexpected"
        )
        if (item["emission_relation"] != expected_relation
                or expected_kind not in failure_kinds):
            raise RefereeError(
                f"audit offender {cell_id} has false unowned-emission evidence")
    elif layout_relation == "cell-binding-invalid":
        if (item["emission_relation"] != "invalid"
                or "emitted-cell-binding-invalid" not in failure_kinds):
            raise RefereeError(
                f"audit offender {cell_id} has false cell-binding evidence")
    elif layout_relation == "registry-invalid":
        if (item["emission_relation"] != "not-evaluated"
                or failure_kinds != ["comb-owner-registry-invalid"]):
            raise RefereeError(
                "audit comb owner-registry relation is false")
    elif (item["emission_relation"] != "inventory-invalid"
          or "comb-inventory-mismatch" not in failure_kinds):
        raise RefereeError(
            f"audit offender {cell_id} has false inventory evidence")

    inventory_binding = bool(set(failure_kinds) & {
        "duplicate-layout-subject", "unexpected-emitted-comb",
        "emitted-cell-binding-invalid", "duplicate-emitted-cell-id",
        "missing-layout-cell-owner", "duplicate-layout-cell-owner",
        "emitted-cell-page-mismatch", "emitted-cell-geometry-mismatch",
        "unowned-live-comb-markup", "comb-inventory-mismatch",
        "comb-owner-registry-invalid",
        "emission-container-page-mismatch",
        "emission-container-geometry-mismatch",
    })
    dimensions = {
        "layout_mismatch": layout_relation == "mismatch",
        "source_unevaluable": layout_relation in {
            "unevaluable", "duplicate-subject", "inventory-invalid"},
        "emission_invalid": bool(
            not physical_emission_valid or binding_invalid),
        "emission_behind": bool(
            layout_relation == "duplicate-subject"
            or not physical_emission_valid
            or binding_invalid
            or (slots is not None and latticed is not None
                and slots != latticed)
            or "unexpected-emitted-comb" in failure_kinds
            or "unowned-live-comb-markup" in failure_kinds),
        "position_mismatch": position_mismatch,
        "inventory_binding": inventory_binding,
    }
    # Pseudo binding/inventory records do not contribute to audit.py's
    # emission-invalid summary unless they are raw live comb markup.
    if not normal_subject:
        dimensions["emission_invalid"] = bool(
            ("unowned-live-comb-markup" in failure_kinds)
            or ("unexpected-emitted-comb" in failure_kinds
                and not physical_emission_valid)
            or (layout_relation == "duplicate-subject"
                and not physical_emission_valid)
        )
        dimensions["emission_behind"] = bool(
            "unexpected-emitted-comb" in failure_kinds
            or "unowned-live-comb-markup" in failure_kinds
            or layout_relation == "duplicate-subject"
        )
    if not any(dimensions.values()):
        raise RefereeError(
            f"audit offender {cell_id} is unsupported by any failure relation")
    return {
        "cell": cell_id,
        "page": page,
        "slots": slots,
        "latticed": latticed,
        "printed": printed,
        "emitted_occurrences": occurrences,
        "layout_relation": layout_relation,
        "emission_state": item["emission_state"],
        "failure_kinds": failure_kinds,
        "source_owner_certificate": owner_certificate,
        "dimensions": dimensions,
    }


def audit_evidence(
        audit_record: dict[str, Any] | None,
        owner_binding: dict[str, Any] | None = None,
        ) -> dict[str, Any]:
    """Validate exhaustive audit publication without conflating dimensions."""
    if not audit_record:
        return {
            "assertion_valid": False,
            "complete": False,
            "reason": "no audit record",
            "errors": ["no audit record"],
            "offenders": {},
        }
    assertions = audit_record.get("assertions")
    assertion = (
        assertions.get("comb_slots_match_printed")
        if isinstance(assertions, dict) else None
    )
    if not isinstance(assertion, dict):
        return {
            "assertion_valid": False,
            "complete": False,
            "reason": "comb audit assertion is missing",
            "errors": ["comb audit assertion is missing"],
            "offenders": {},
        }
    raw_offenders = assertion.get("offenders")
    if not isinstance(raw_offenders, list):
        return {
            "assertion_valid": False,
            "complete": False,
            "reason": "audit offenders is not a list",
            "errors": ["audit offenders is not a list"],
            "offenders": {},
        }
    errors: list[str] = []
    owner_cells: dict[str, dict[str, Any]] | None = None
    if owner_binding is not None:
        if (not isinstance(owner_binding, dict)
                or set(owner_binding) != {"layout_sha256", "cells"}
                or not isinstance(owner_binding.get("layout_sha256"), str)
                or not isinstance(owner_binding.get("cells"), dict)):
            errors.append("audit owner binding context is malformed")
            owner_cells = {}
        else:
            owner_cells = owner_binding["cells"]
            for cell_id, certificate in owner_cells.items():
                try:
                    if (not isinstance(cell_id, str)
                            or certificate.get("cell_id") != cell_id
                            or certificate.get("layout_sha256")
                            != owner_binding["layout_sha256"]):
                        raise RefereeError(
                            "owner binding identity/layout SHA is false")
                    validate_audit_owner_certificate(
                        certificate, certificate)
                except (AttributeError, RefereeError) as error:
                    errors.append(
                        f"audit owner binding {cell_id!r}: {error}")
    dimensions_by_cell: dict[str, dict[str, Any]] = {}
    valid_items: list[dict[str, Any]] = []
    for index, item in enumerate(raw_offenders):
        try:
            raw_cell = item.get("cell") if isinstance(item, dict) else None
            expected_owner = (
                owner_cells.get(raw_cell)
                if owner_cells is not None and isinstance(raw_cell, str)
                else None
            )
            dimensions = audit_offender_dimensions(item, expected_owner)
        except RefereeError as error:
            errors.append(f"offender[{index}]: {error}")
            continue
        cell_id = dimensions["cell"]
        if cell_id in dimensions_by_cell:
            errors.append(f"duplicate offender cell: {cell_id}")
            continue
        dimensions_by_cell[cell_id] = dimensions
        valid_items.append(item)
    offenders = {
        item["cell"]: item for item in valid_items
    }

    try:
        expected_ids = string_list(
            assertion["expected_comb_ids"], "audit expected comb ids")
        checked_ids = string_list(
            assertion["checked_comb_ids"], "audit checked comb ids")
        emitted_ids = string_list(
            assertion["emitted_comb_ids"], "audit emitted comb ids")
        unexpected_ids = string_list(
            assertion["unexpected_emitted_comb_ids"],
            "audit unexpected emitted comb ids")
        duplicate_layout_ids = string_list(
            assertion["duplicate_layout_comb_ids"],
            "audit duplicate layout comb ids")
        duplicate_emitted_ids = string_list(
            assertion["duplicate_emitted_cell_ids"],
            "audit duplicate emitted cell ids")
        counts = {
            key: exact_nonnegative_int(
                assertion[key], f"audit {key.replace('_', ' ')}")
            for key in (
                "combs_expected", "combs_checked", "raw_live_comb_issues",
                "emitted_cell_binding_issues", "layout_mismatches",
                "layout_unevaluable", "emission_behind_layout",
                "owner_certificates_valid", "owner_certificates_invalid",
                "source_u_frame_evaluable",
                "source_certified_unframed_evaluable", "emission_invalid",
            )
        }
    except (KeyError, RefereeError) as error:
        errors.append(str(error))
        expected_ids = []
        checked_ids = []
        emitted_ids = []
        unexpected_ids = []
        duplicate_layout_ids = []
        duplicate_emitted_ids = []
        counts = {
            key: -1 for key in (
                "combs_expected", "combs_checked", "raw_live_comb_issues",
                "emitted_cell_binding_issues", "layout_mismatches",
                "layout_unevaluable", "emission_behind_layout",
                "owner_certificates_valid", "owner_certificates_invalid",
                "source_u_frame_evaluable",
                "source_certified_unframed_evaluable", "emission_invalid",
            )
        }
    if checked_ids != expected_ids:
        errors.append("audit checked IDs are not the exhaustive expected order")
    if counts["combs_expected"] != len(expected_ids):
        errors.append("audit expected count disagrees with expected IDs")
    if counts["combs_checked"] != len(checked_ids):
        errors.append("audit checked count disagrees with checked IDs")
    if owner_cells is not None and expected_ids != list(owner_cells):
        errors.append(
            "audit expected IDs differ from exact retained owner order")
    if emitted_ids != sorted(emitted_ids):
        errors.append("audit emitted IDs are not canonical sorted inventory")
    if unexpected_ids != sorted(set(emitted_ids) - set(expected_ids)):
        errors.append("audit unexpected emitted inventory is false")
    if duplicate_layout_ids != sorted(duplicate_layout_ids):
        errors.append("audit duplicate-layout IDs are not sorted")
    if duplicate_emitted_ids != sorted(duplicate_emitted_ids):
        errors.append("audit duplicate-emitted IDs are not sorted")

    holds = assertion.get("holds")
    inventory_complete = assertion.get("inventory_complete")
    if not isinstance(holds, bool):
        errors.append("audit holds flag is not boolean")
        holds = False
    if not isinstance(inventory_complete, bool):
        errors.append("audit inventory_complete flag is not boolean")
        inventory_complete = False
    count = assertion.get("offender_count", 0 if holds else None)
    published = assertion.get("offenders_published", 0 if holds else None)
    omitted = assertion.get("offenders_omitted", 0 if holds else None)
    complete_flag = assertion.get("offenders_complete", True if holds else None)
    try:
        count = exact_nonnegative_int(count, "audit offender count")
        published = exact_nonnegative_int(
            published, "audit published offender count")
        omitted = exact_nonnegative_int(
            omitted, "audit omitted offender count")
    except RefereeError as error:
        errors.append(str(error))
        count = published = omitted = -1
    if not isinstance(complete_flag, bool):
        errors.append("audit offenders_complete flag is not boolean")
        complete_flag = False
    if published != len(raw_offenders):
        errors.append("audit published count disagrees with offender list")
    if count != published + omitted:
        errors.append("audit published and omitted counts do not sum")
    if omitted != 0 or not complete_flag:
        errors.append("audit offender publication is not exhaustive")

    expected_set = set(expected_ids)
    offender_ids = set(offenders)
    for cell_id in expected_ids:
        if cell_id not in emitted_ids:
            item = offenders.get(cell_id)
            if (item is None
                    or item.get("emission_state") != "missing-emitted-cell"
                    or not set(item.get("failure_kinds") or ()) & {
                        "invalid-emission", "duplicate-layout-subject"}):
                errors.append(
                    f"audit omits missing-emission offender: {cell_id}")
    for cell_id in unexpected_ids:
        item = offenders.get(cell_id)
        if item is None or "unexpected-emitted-comb" not in (
                item.get("failure_kinds") or ()):
            errors.append(
                f"audit omits unexpected-emission offender: {cell_id}")
    derived_duplicate_layout = sorted(
        cell_id for cell_id, detail in dimensions_by_cell.items()
        if detail["layout_relation"] == "duplicate-subject")
    if duplicate_layout_ids != derived_duplicate_layout:
        errors.append("audit duplicate-layout inventory lacks exact offenders")
    raw_issue_ids = {
        cell_id for cell_id, detail in dimensions_by_cell.items()
        if "unowned-live-comb-markup" in detail["failure_kinds"]
    }
    if counts["raw_live_comb_issues"] != len(raw_issue_ids):
        errors.append("audit raw-live-comb count disagrees with offenders")
    inventory_offenders = {
        cell_id for cell_id, detail in dimensions_by_cell.items()
        if "comb-inventory-mismatch" in detail["failure_kinds"]
    }
    owner_registry_offenders = {
        cell_id for cell_id, detail in dimensions_by_cell.items()
        if "comb-owner-registry-invalid" in detail["failure_kinds"]
    }
    binding_issue_ids = set(duplicate_emitted_ids) | set(unexpected_ids)
    for cell_id, detail in dimensions_by_cell.items():
        if set(detail["failure_kinds"]) & {
                "emission-container-page-mismatch",
                "emission-container-geometry-mismatch",
                "emitted-cell-binding-invalid",
                "duplicate-emitted-cell-id",
                "missing-layout-cell-owner",
                "duplicate-layout-cell-owner",
                "emitted-cell-page-mismatch",
                "emitted-cell-geometry-mismatch",
            }:
            binding_issue_ids.add(cell_id)
    if counts["emitted_cell_binding_issues"] != len(binding_issue_ids):
        errors.append("audit cell-binding count disagrees with offenders")
    relevant_duplicate_emitted = (
        set(duplicate_emitted_ids) & (set(expected_ids) | set(emitted_ids)))
    derived_inventory_complete = not (
        unexpected_ids
        or duplicate_layout_ids
        or relevant_duplicate_emitted
        or raw_issue_ids
        or binding_issue_ids
        or inventory_offenders
        or owner_registry_offenders
    )
    if inventory_complete is not derived_inventory_complete:
        errors.append("audit inventory_complete relation is false")

    derived_counts = {
        "layout_mismatches": sum(
            detail["dimensions"]["layout_mismatch"]
            for detail in dimensions_by_cell.values()),
        "layout_unevaluable": sum(
            detail["dimensions"]["source_unevaluable"]
            for detail in dimensions_by_cell.values()),
        "emission_behind_layout": sum(
            detail["dimensions"]["emission_behind"]
            for detail in dimensions_by_cell.values()),
        "emission_invalid": sum(
            detail["dimensions"]["emission_invalid"]
            for detail in dimensions_by_cell.values()),
    }
    for key, derived in derived_counts.items():
        if counts[key] != derived:
            errors.append(
                f"audit {key} count {counts[key]} disagrees with "
                f"{derived} independent offender relations")

    if (counts["owner_certificates_valid"]
            + counts["owner_certificates_invalid"]
            != counts["combs_checked"]):
        errors.append(
            "audit owner certificate counts do not partition checked cells")
    checked_certificates = {
        cell_id: detail["source_owner_certificate"]
        for cell_id, detail in dimensions_by_cell.items()
        if cell_id in expected_set
        and isinstance(detail.get("source_owner_certificate"), dict)
    }
    published_invalid_certificates = sum(
        certificate.get("valid") is False
        for certificate in checked_certificates.values()
    )
    published_valid_certificates = sum(
        certificate.get("valid") is True
        for certificate in checked_certificates.values()
    )
    if (counts["owner_certificates_invalid"]
            != published_invalid_certificates):
        errors.append(
            "audit invalid owner certificate count disagrees with offenders")
    if (published_valid_certificates
            > counts["owner_certificates_valid"]):
        errors.append(
            "audit published valid owner certificates exceed their count")
    if set(checked_certificates) == expected_set and (
            counts["owner_certificates_valid"]
            != published_valid_certificates):
        errors.append(
            "audit complete owner certificate publication disagrees with count")
    if owner_registry_offenders and (
            published_invalid_certificates != counts["combs_checked"]
            or counts["owner_certificates_invalid"]
            != counts["combs_checked"]
            or counts["owner_certificates_valid"] != 0
            or set(checked_certificates) != expected_set):
        errors.append(
            "audit global owner-registry failure does not invalidate every "
            "checked certificate")

    checked_source_unevaluable = {
        cell_id for cell_id, detail in dimensions_by_cell.items()
        if cell_id in expected_set
        and detail["dimensions"]["source_unevaluable"]
    }
    source_evaluable = (
        counts["combs_checked"] - len(checked_source_unevaluable))
    if (counts["source_u_frame_evaluable"]
            + counts["source_certified_unframed_evaluable"]
            != source_evaluable):
        errors.append(
            "audit source frame/unframed counts do not partition evaluable "
            "checked cells")
    published_u_frame = 0
    published_certified_unframed = 0
    for cell_id, detail in dimensions_by_cell.items():
        if cell_id not in expected_set or detail["printed"] is None:
            continue
        certificate = detail.get("source_owner_certificate")
        if (not isinstance(certificate, dict)
                or certificate.get("valid") is not True):
            errors.append(
                f"audit measured source lacks valid owner certificate: {cell_id}")
            continue
        if offenders[cell_id].get("source_frame_geometry") is None:
            published_certified_unframed += 1
        else:
            published_u_frame += 1
    if published_u_frame > counts["source_u_frame_evaluable"]:
        errors.append(
            "audit published U-frame source results exceed their count")
    if (published_certified_unframed
            > counts["source_certified_unframed_evaluable"]):
        errors.append(
            "audit published certified-unframed results exceed their count")
    unsupported_canonical = sorted(
        cell_id for cell_id in offender_ids - expected_set - set(unexpected_ids)
        if _CELL_RE.fullmatch(cell_id)
        and "emitted-cell-binding-invalid"
        not in (offenders[cell_id].get("failure_kinds") or ())
        and "unowned-live-comb-markup"
        not in (offenders[cell_id].get("failure_kinds") or ())
    )
    if unsupported_canonical:
        errors.append(
            "audit publishes canonical offenders outside its inventories: "
            + ", ".join(unsupported_canonical[:8]))
    expected_holds = bool(
        count == 0
        and inventory_complete
        and all(value == 0 for value in derived_counts.values())
    )
    if holds is not expected_holds:
        errors.append("audit holds flag disagrees with independent relations")
    if audit_record.get("comb_slots_match_printed") is not holds:
        errors.append("audit top-level comb verdict disagrees with assertion")
    reason_value = assertion.get("reason")
    if not isinstance(reason_value, str) or (not holds and not reason_value):
        errors.append("audit assertion reason is malformed")

    assertion_valid = not errors
    return {
        "assertion_valid": assertion_valid,
        # Manifest/attestation binding is applied later; this field is kept
        # fail-closed until then.
        "complete": False,
        "reason": (
            "assertion publication verified; attestation not yet bound"
            if assertion_valid else "; ".join(errors)
        ),
        "errors": errors,
        "offender_count": count,
        "offenders_published": published,
        "offenders_omitted": omitted,
        "combs_expected": counts["combs_expected"],
        "combs_checked": counts["combs_checked"],
        "expected_comb_ids": expected_ids,
        "checked_comb_ids": checked_ids,
        "emitted_comb_ids": emitted_ids,
        "unexpected_emitted_comb_ids": unexpected_ids,
        "duplicate_layout_comb_ids": duplicate_layout_ids,
        "duplicate_emitted_cell_ids": duplicate_emitted_ids,
        "raw_live_comb_issues": counts["raw_live_comb_issues"],
        "emitted_cell_binding_issues": (
            counts["emitted_cell_binding_issues"]),
        "inventory_complete": inventory_complete,
        "layout_mismatches": counts["layout_mismatches"],
        "layout_unevaluable": counts["layout_unevaluable"],
        "owner_certificates_valid": counts["owner_certificates_valid"],
        "owner_certificates_invalid": counts["owner_certificates_invalid"],
        "source_u_frame_evaluable": counts["source_u_frame_evaluable"],
        "source_certified_unframed_evaluable": (
            counts["source_certified_unframed_evaluable"]),
        "emission_behind_layout": counts["emission_behind_layout"],
        "emission_invalid": counts["emission_invalid"],
        "offender_dimensions": dimensions_by_cell,
        "offenders": offenders,
        "holds": holds,
    }


class AuditRenderDependencyScanner(html.parser.HTMLParser):
    """Independent local-resource inventory for the frozen audit manifest."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.references: list[tuple[str, str]] = []
        self.errors: list[str] = []
        self.style_depth = 0

    def _add(self, value: str | None, kind: str) -> None:
        if value is not None and value.strip():
            self.references.append((value.strip(), kind))

    def _srcset(self, value: str | None, kind: str) -> None:
        if value:
            for candidate in value.split(","):
                self._add(candidate.strip().split()[0], kind)

    def handle_starttag(
            self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = {key.lower(): value for key, value in attrs}
        lowered = tag.lower()
        if lowered == "style":
            self.style_depth += 1
        self.references.extend(
            (url, "inline-style")
            for url in _audit_css_urls(values.get("style") or "")
        )
        if lowered == "link":
            rel = {
                item.lower() for item in (values.get("rel") or "").split()}
            if rel & {
                    "stylesheet", "preload", "modulepreload",
                    "icon", "manifest"}:
                self._add(values.get("href"), "link")
        elif lowered in {"img", "source"}:
            self._add(values.get("src"), lowered)
            self._srcset(values.get("srcset"), f"{lowered}-srcset")
        elif lowered in {
                "video", "audio", "track", "embed", "iframe"}:
            self._add(values.get("src"), lowered)
            if lowered == "video":
                self._add(values.get("poster"), "video-poster")
        elif lowered == "object":
            self._add(values.get("data"), "object")
        elif lowered == "input" and (
                values.get("type") or "").lower() == "image":
            self._add(values.get("src"), "input-image")
        elif lowered == "image":
            self._add(
                values.get("href") or values.get("xlink:href"), "svg-image")

    def handle_startendtag(
            self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self.handle_starttag(tag, attrs)
        if tag.lower() == "style" and self.style_depth:
            self.style_depth -= 1

    def handle_endtag(self, tag: str) -> None:
        if tag.lower() == "style" and self.style_depth:
            self.style_depth -= 1

    def handle_data(self, data: str) -> None:
        if self.style_depth:
            self.references.extend(
                (url, "style-block") for url in _audit_css_urls(data))


_AUDIT_CSS_URL_RE = re.compile(
    r"""url\(\s*(?P<quote>["']?)(?P<url>.*?)(?P=quote)\s*\)""",
    re.IGNORECASE,
)
_AUDIT_CSS_IMPORT_RE = re.compile(
    r"""@import\s+(?:url\(\s*)?(?P<quote>["'])(?P<url>.*?)(?P=quote)""",
    re.IGNORECASE,
)


def _audit_css_urls(css: str) -> list[str]:
    return [
        *(match.group("url") for match in _AUDIT_CSS_IMPORT_RE.finditer(css)),
        *(match.group("url") for match in _AUDIT_CSS_URL_RE.finditer(css)),
    ]


def _audit_logical_resource(reference: str, base: str) -> str | None:
    parsed = urllib.parse.urlsplit(reference.strip())
    if parsed.scheme.lower() == "data":
        return None
    if (parsed.scheme or parsed.netloc or reference.startswith("//")
            or parsed.path.startswith("/") or parsed.query):
        raise RefereeError(
            f"external, absolute, or query-bearing render resource: {reference}")
    if not parsed.path:
        return None
    decoded = urllib.parse.unquote(parsed.path)
    if ("\\" in decoded
            or any(ord(character) < 32 or ord(character) == 127
                   for character in decoded)):
        raise RefereeError(f"invalid render resource path: {reference}")
    logical = posixpath.normpath(
        posixpath.join(posixpath.dirname(base), decoded))
    if (logical in {"", ".", ".."} or logical.startswith("../")
            or pathlib.PurePosixPath(logical).is_absolute()):
        raise RefereeError(f"render resource escapes snapshot: {reference}")
    return logical


def audit_render_dependencies(
        html_payload: bytes,
        entrypoint: str,
        html_dir: pathlib.Path,
        ) -> tuple[list[dict[str, Any]], list[str]]:
    try:
        text = html_payload.decode("utf-8")
    except UnicodeDecodeError as error:
        return [], [f"HTML is not UTF-8: {error}"]
    scanner = AuditRenderDependencyScanner()
    scanner.feed(text)
    scanner.close()
    errors = list(scanner.errors)
    pending = [
        (reference, entrypoint, kind)
        for reference, kind in scanner.references
    ]
    root = html_dir.resolve()
    metadata: dict[str, dict[str, Any]] = {}
    payloads: dict[str, bytes] = {}
    visited_css: set[str] = set()
    while pending:
        reference, referrer, kind = pending.pop(0)
        try:
            logical = _audit_logical_resource(reference, referrer)
        except RefereeError as error:
            errors.append(f"{referrer}: {error}")
            continue
        if logical is None:
            continue
        item = metadata.setdefault(logical, {
            "path": logical,
            "mime_type": None,
            "present": False,
            "bytes": None,
            "sha256": None,
            "kinds": set(),
            "referrers": set(),
        })
        item["kinds"].add(kind)
        item["referrers"].add(referrer)
        if logical in payloads:
            continue
        candidate = root.joinpath(*pathlib.PurePosixPath(logical).parts)
        try:
            resolved = candidate.resolve(strict=True)
            resolved.relative_to(root)
            if resolved != candidate or not resolved.is_file():
                raise RefereeError("symlinked or non-file dependency")
            payload = resolved.read_bytes()
        except (OSError, ValueError, RefereeError) as error:
            errors.append(
                f"{referrer}: unresolved render dependency "
                f"{reference!r} ({error})")
            continue
        payloads[logical] = payload
        mime_type = mimetypes.guess_type(logical)[0]
        if mime_type is None:
            errors.append(f"{logical}: unknown render dependency MIME type")
            continue
        item.update({
            "mime_type": mime_type,
            "present": True,
            "bytes": len(payload),
            "sha256": sha256_bytes(payload),
        })
        if logical.lower().endswith(".css") and logical not in visited_css:
            visited_css.add(logical)
            try:
                css = payload.decode("utf-8")
            except UnicodeDecodeError as error:
                errors.append(f"{logical}: CSS is not UTF-8 ({error})")
                continue
            pending.extend(
                (nested, logical, "css")
                for nested in _audit_css_urls(css)
            )
    entries = [
        {
            **{
                key: value for key, value in item.items()
                if key not in {"kinds", "referrers"}
            },
            "kinds": sorted(item["kinds"]),
            "referrers": sorted(item["referrers"]),
        }
        for _logical, item in sorted(metadata.items())
    ]
    return entries, sorted(set(errors))


def validate_audit_runtime(runtime: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(runtime, dict) or set(runtime) != {
        "python", "pymupdf", "loaded_application_files",
        "stdlib_and_system_shared_libraries_bound",
        "scope_complete", "incomplete_reason",
    }:
        return ["audit base runtime manifest schema is unsupported"]
    python = runtime["python"]
    if not isinstance(python, dict) or set(python) != {
            "implementation", "version", "cache_tag"}:
        errors.append("audit Python runtime identity is malformed")
    elif (python["implementation"] != platform.python_implementation()
          or python["version"] != platform.python_version()
          or python["cache_tag"] != sys.implementation.cache_tag):
        errors.append("audit Python runtime differs from referee runtime")
    pymupdf = runtime["pymupdf"]
    if (not isinstance(pymupdf, dict)
            or set(pymupdf) != {"package_version", "version_bind"}
            or not all(isinstance(value, str) and value
                       for value in pymupdf.values())
            or pymupdf["package_version"] != pymupdf["version_bind"]):
        errors.append("audit PyMuPDF identity is malformed")
    loaded = runtime["loaded_application_files"]
    if not isinstance(loaded, dict) or set(loaded) != {
            "algorithm", "files", "bytes", "tree_sha256", "members",
            "validated_before_after"}:
        errors.append("audit loaded-application manifest schema is malformed")
    else:
        members = loaded["members"]
        parsed: list[tuple[str, int, str]] = []
        if not isinstance(members, list):
            errors.append("audit loaded-application members are not a list")
        else:
            for index, member in enumerate(members):
                if (not isinstance(member, dict)
                        or set(member) != {"file", "bytes", "sha256"}
                        or not isinstance(member.get("file"), str)
                        or not member["file"]):
                    errors.append(
                        f"audit runtime member[{index}] is malformed")
                    continue
                try:
                    size = exact_nonnegative_int(
                        member["bytes"], f"audit runtime member[{index}] bytes")
                except RefereeError as error:
                    errors.append(str(error))
                    continue
                digest = member["sha256"]
                if (not isinstance(digest, str)
                        or re.fullmatch(r"[0-9a-f]{64}", digest) is None):
                    errors.append(
                        f"audit runtime member[{index}] hash is malformed")
                    continue
                parsed.append((member["file"], size, digest))
        if len({item[0] for item in parsed}) != len(parsed):
            errors.append("audit runtime members contain duplicate identities")
        if parsed != sorted(parsed, key=lambda item: item[0]):
            errors.append("audit runtime members are not canonical ordered")
        canonical = json.dumps(parsed, separators=(",", ":"))
        if loaded.get("algorithm") != (
                "sha256(canonical-json(logical-file,bytes,sha256))"):
            errors.append("audit runtime digest algorithm is unsupported")
        if loaded.get("files") != len(parsed):
            errors.append("audit runtime member count is false")
        if loaded.get("bytes") != sum(item[1] for item in parsed):
            errors.append("audit runtime byte total is false")
        if loaded.get("tree_sha256") != sha256_bytes(
                canonical.encode("ascii")):
            errors.append("audit runtime tree digest is false")
        if loaded.get("validated_before_after") is not True:
            errors.append("audit runtime was not validated before and after")
        executable = [
            item for item in parsed if item[0] == "python/executable"]
        python_path = pathlib.Path(sys.executable).resolve()
        if len(executable) != 1 or executable[0][1:] != (
                python_path.stat().st_size, sha256_file(python_path)):
            errors.append("audit runtime Python executable bytes are stale")
    if runtime["stdlib_and_system_shared_libraries_bound"] is not False:
        errors.append("audit base runtime overclaims system-library binding")
    if runtime["scope_complete"] is not False:
        errors.append("audit base runtime overclaims complete scope")
    if (not isinstance(runtime["incomplete_reason"], str)
            or not runtime["incomplete_reason"]):
        errors.append("audit base runtime lacks its incomplete-scope reason")
    return errors


def validate_audit_roundtrip(
        audit_record: dict[str, Any],
        entrypoint: str,
        dependency_paths: Sequence[str],
        ) -> tuple[bool | None, list[str]]:
    errors: list[str] = []
    if audit_record.get("roundtrip") == "skipped":
        if any(key in audit_record for key in (
                "roundtrip_runtime", "render_requests", "candidate_pdf")):
            errors.append("skipped audit carries partial roundtrip evidence")
        return None, errors
    runtime = audit_record.get("roundtrip_runtime")
    requests = audit_record.get("render_requests")
    candidate = audit_record.get("candidate_pdf")
    if not all(isinstance(value, dict)
               for value in (runtime, requests, candidate)):
        return None, ["audit roundtrip evidence is missing or partial"]
    required_runtime = {
        "mode", "playwright_package_version", "dependency_closure",
        "chromium", "same_resolution_session_used_for_render",
        "dependency_closure_validated_before_after",
        "system_shared_libraries_bound", "native_host_environment_bound",
        "scope", "scope_complete", "incomplete_reason",
        "live_browser_version", "explicit_executable_path_used",
        "launch_args", "service_workers", "browser_context_offline",
        "websocket_policy", "request_policy",
        "playwright_operation_timeout_ms", "hard_deadline_seconds",
        "hard_deadline_enforced_by", "deadline_cleanup_policy",
    }
    if set(runtime) != required_runtime:
        errors.append("audit roundtrip runtime schema is unsupported")
    else:
        deadline_value = runtime["hard_deadline_seconds"]
        deadline = (
            float(deadline_value)
            if (not isinstance(deadline_value, bool)
                and isinstance(deadline_value, (int, float))
                and math.isfinite(float(deadline_value)))
            else None
        )
        live_browser_version = runtime["live_browser_version"]
        if (not isinstance(runtime["playwright_package_version"], str)
                or not runtime["playwright_package_version"]
                or not isinstance(live_browser_version, str)
                or not live_browser_version
                or runtime["mode"] != "playwright-exact-executable"
                or runtime["same_resolution_session_used_for_render"] is not True
                or runtime[
                    "dependency_closure_validated_before_after"] is not True
                or runtime["explicit_executable_path_used"] is not True
                or runtime["browser_context_offline"] is not True
                or runtime["service_workers"] != "block"
                or runtime["websocket_policy"]
                != "record-and-leave-unconnected"
                or runtime["request_policy"] != "formgen-snapshot-only-v1"
                or deadline != 60.0
                or not isinstance(
                    runtime["playwright_operation_timeout_ms"], int)
                or isinstance(
                    runtime["playwright_operation_timeout_ms"], bool)
                or runtime["playwright_operation_timeout_ms"] != 120000
                or runtime["hard_deadline_enforced_by"]
                != "isolated-render-worker-process-v1"
                or runtime["deadline_cleanup_policy"]
                != "kill-worker-and-chromium-process-group"):
            errors.append("audit roundtrip execution binding is malformed")
        if (runtime["system_shared_libraries_bound"] is not False
                or runtime["native_host_environment_bound"] is not False
                or runtime["scope"] != AUDIT_ROUNDTRIP_SCOPE
                or runtime["scope_complete"] is not False
                or not isinstance(runtime["incomplete_reason"], str)
                or not runtime["incomplete_reason"]):
            errors.append("audit roundtrip runtime overclaims its scope")
        if runtime["launch_args"] != AUDIT_ROUNDTRIP_LAUNCH_ARGS:
            errors.append("audit roundtrip launch arguments are not exact")
        closure = runtime["dependency_closure"]
        if (not isinstance(closure, dict)
                or set(closure) != {
                    "logical_root", "algorithm", "files", "symlinks",
                    "bytes", "tree_sha256"}
                or closure.get("logical_root") != "playwright"
                or closure.get("algorithm") != (
                    "sha256(canonical-json(path,type,bytes,digest))")
                or not all(
                    isinstance(closure.get(key), int)
                    and not isinstance(closure.get(key), bool)
                    and closure[key] >= 0
                    for key in ("files", "symlinks", "bytes"))
                or not isinstance(closure.get("tree_sha256"), str)
                or re.fullmatch(
                    r"[0-9a-f]{64}", closure["tree_sha256"]) is None):
            errors.append("audit roundtrip dependency closure is malformed")
        chromium = runtime["chromium"]
        chromium_file = (
            chromium.get("file") if isinstance(chromium, dict) else None)
        chromium_file_canonical = bool(
            isinstance(chromium_file, str)
            and chromium_file.startswith("playwright/")
            and posixpath.normpath(chromium_file) == chromium_file
            and ".." not in pathlib.PurePosixPath(chromium_file).parts
        )
        if (not isinstance(chromium, dict)
                or set(chromium) != {
                    "file", "bytes", "sha256", "version_output"}
                or not chromium_file_canonical
                or not isinstance(chromium.get("bytes"), int)
                or isinstance(chromium.get("bytes"), bool)
                or chromium["bytes"] <= 0
                or not isinstance(chromium.get("sha256"), str)
                or re.fullmatch(
                    r"[0-9a-f]{64}", chromium["sha256"]) is None
                or not isinstance(chromium.get("version_output"), str)
                or not chromium["version_output"]
                or not isinstance(live_browser_version, str)
                or live_browser_version
                not in chromium["version_output"]):
            errors.append("audit roundtrip Chromium identity is malformed")
    if set(requests) != {
            "policy", "synthetic_origin", "fulfilled", "fulfilled_requests",
            "blocked", "blocked_requests", "blocked_websockets",
            "all_requests_from_retained_closure"}:
        errors.append("audit roundtrip request manifest is unsupported")
    else:
        fulfilled = requests["fulfilled"]
        blocked = requests["blocked"]
        websockets = requests["blocked_websockets"]
        retained_paths_valid = bool(
            isinstance(entrypoint, str)
            and entrypoint
            and isinstance(dependency_paths, Sequence)
            and all(isinstance(item, str) and item
                    for item in dependency_paths)
            and list(dependency_paths) == sorted(dependency_paths)
            and len(dependency_paths) == len(set(dependency_paths))
            and entrypoint not in dependency_paths
        )
        retained_paths = (
            {entrypoint, *dependency_paths} if retained_paths_valid else set())
        fulfilled_valid = bool(
            isinstance(fulfilled, list)
            and fulfilled
            and all(isinstance(item, str) and item for item in fulfilled)
        )
        fulfilled_exact = bool(
            fulfilled_valid
            and entrypoint in fulfilled
            and fulfilled == sorted(fulfilled)
            and len(fulfilled) == len(set(fulfilled))
            and set(fulfilled) <= retained_paths
        )
        fulfilled_count_exact = bool(
            isinstance(requests["fulfilled_requests"], int)
            and not isinstance(requests["fulfilled_requests"], bool)
            and requests["fulfilled_requests"] == (
                len(fulfilled) if fulfilled_valid else -1)
        )
        blocked_http_empty = bool(
            isinstance(blocked, list)
            and blocked == []
            and isinstance(requests["blocked_requests"], int)
            and not isinstance(requests["blocked_requests"], bool)
            and requests["blocked_requests"] == 0
        )
        blocked_websockets_empty = bool(
            isinstance(websockets, list) and websockets == [])
        derived_retained_closure = bool(
            fulfilled_exact
            and fulfilled_count_exact
            and blocked_http_empty
            and blocked_websockets_empty
        )
        if (requests["policy"] != "formgen-snapshot-only-v1"
                or requests["synthetic_origin"] != "https://formgen.invalid"
                or not retained_paths_valid
                or not derived_retained_closure
                or requests["all_requests_from_retained_closure"]
                is not derived_retained_closure):
            errors.append("audit roundtrip request closure is false")
    required_candidate = {
        "bytes", "sha256", "retained_exact_bytes",
        "chromium_returned_in_memory", "normalization", "materialization",
        "expected_sha256_passed_to_extractor",
        "validated_before_after_extraction", "candidate_ir_sha256",
        "candidate_ir_digest_scope",
    }
    if set(candidate) != required_candidate:
        errors.append("audit candidate PDF manifest is unsupported")
    else:
        if (not isinstance(candidate["bytes"], int)
                or isinstance(candidate["bytes"], bool)
                or candidate["bytes"] <= 0
                or not isinstance(candidate["sha256"], str)
                or re.fullmatch(r"[0-9a-f]{64}", candidate["sha256"]) is None
                or not isinstance(candidate["candidate_ir_sha256"], str)
                or re.fullmatch(
                    r"[0-9a-f]{64}", candidate["candidate_ir_sha256"]) is None
                or candidate["retained_exact_bytes"] is not True
                or candidate["chromium_returned_in_memory"] is not True
                or candidate[
                    "expected_sha256_passed_to_extractor"] is not True
                or candidate[
                    "validated_before_after_extraction"] is not True
                or candidate["materialization"]
                != AUDIT_CANDIDATE_MATERIALIZATION
                or candidate["candidate_ir_digest_scope"]
                != "source-and-generator-removed"):
            errors.append("audit candidate PDF provenance is malformed")
        normalization = candidate["normalization"]
        if (not isinstance(normalization, dict)
                or set(normalization) != {
                    "algorithm", "fields_normalized", "replacement",
                    "xref_offsets_preserved"}
                or normalization["algorithm"]
                != "fixed-width-creation-modification-date-v1"
                or not isinstance(normalization["fields_normalized"], int)
                or isinstance(normalization["fields_normalized"], bool)
                or normalization["fields_normalized"] != 2
                or normalization["replacement"]
                != AUDIT_PDF_NORMALIZATION_REPLACEMENT
                or normalization["xref_offsets_preserved"] is not True):
            errors.append("audit candidate PDF normalization is malformed")
    if (audit_record.get("measured") is not True
            or audit_record.get("hard_failure") is not None
            or audit_record.get("error") is not None
            or audit_record.get("status") != "ok"
            or "roundtrip_liveness" in audit_record):
        errors.append("audit roundtrip success state is malformed")
    return False, errors


def bind_audit_manifest(
        audit_record: dict[str, Any] | None,
        expected: dict[str, tuple[pathlib.Path, bool, bytes | None]],
        *,
        source_path: pathlib.Path,
        source_identity: str,
        source_root: pathlib.Path,
        source_payload: bytes,
        expected_source_sha256: str,
        html_dir: pathlib.Path,
        producer_sources: dict[str, bytes],
        ) -> dict[str, Any]:
    """Verify exact bytes while preserving intentional attestation blockers."""
    errors: list[str] = []
    blockers: list[str] = []
    if not audit_record:
        return {
            "binding_valid": False,
            "complete": False,
            "reason": "no audit record",
            "errors": ["no audit record"],
            "blockers": [],
        }
    manifest = audit_record.get("input_manifest")
    if not isinstance(manifest, dict) or set(manifest) != {
            "schema", "algorithm", "producer", "runtime",
            "inputs_complete", "attestation_complete", "enforceable",
            "complete", "missing_required", "inputs", "render"}:
        return {
            "binding_valid": False,
            "complete": False,
            "reason": "audit input manifest schema is missing or unsupported",
            "errors": [
                "audit input manifest schema is missing or unsupported"],
            "blockers": [],
        }
    if (manifest["schema"] != "formgen-audit-input-manifest-v1"
            or manifest["algorithm"] != "sha256"):
        errors.append("audit input manifest schema/algorithm is unsupported")

    expected_producer_keys = {
        "file", "bytes", "sha256", "dependencies",
        "dependency_execution_bound", "audit_execution_bound",
        "assertion_producer_bound", "roundtrip_runtime_bound_in_record",
        "standalone_attestation_complete", "incomplete_reason",
    }
    producer = manifest["producer"]
    if not isinstance(producer, dict) or set(producer) != expected_producer_keys:
        errors.append("audit producer manifest schema is unsupported")
    else:
        audit_payload = producer_sources.get(AUDIT_PRODUCER_FILE)
        if (audit_payload is None
                or sha256_bytes(audit_payload) != AUDIT_PRODUCER_SHA256
                or producer["file"] != AUDIT_PRODUCER_FILE
                or producer["bytes"] != len(audit_payload)
                or producer["sha256"] != AUDIT_PRODUCER_SHA256):
            errors.append("audit producer bytes differ from the frozen pin")
        dependencies = producer["dependencies"]
        if not isinstance(dependencies, list):
            errors.append("audit dependency manifest is not a list")
        else:
            expected_dependency_files = list(AUDIT_DEPENDENCY_SHA256)
            if [
                    item.get("file") if isinstance(item, dict) else None
                    for item in dependencies
            ] != expected_dependency_files:
                errors.append("audit dependency order or identity is false")
            for index, logical in enumerate(expected_dependency_files):
                if index >= len(dependencies):
                    break
                item = dependencies[index]
                payload = producer_sources.get(logical)
                if (not isinstance(item, dict)
                        or set(item) != {
                            "file", "bytes", "sha256", "loaded_origin",
                            "executed_from_snapshotted_source"}
                        or payload is None
                        or sha256_bytes(payload)
                        != AUDIT_DEPENDENCY_SHA256[logical]
                        or item.get("file") != logical
                        or item.get("loaded_origin") != logical
                        or item.get("bytes") != len(payload)
                        or item.get("sha256")
                        != AUDIT_DEPENDENCY_SHA256[logical]
                        or item.get(
                            "executed_from_snapshotted_source") is not True):
                    errors.append(
                        f"audit dependency bytes/binding are false: {logical}")
        expected_flags = {
            "dependency_execution_bound": True,
            "audit_execution_bound": False,
            "assertion_producer_bound": False,
            "roundtrip_runtime_bound_in_record": False,
            "standalone_attestation_complete": False,
        }
        for key, expected_value in expected_flags.items():
            if producer.get(key) is not expected_value:
                errors.append(f"audit producer flag is false: {key}")
        if (not isinstance(producer.get("incomplete_reason"), str)
                or not producer["incomplete_reason"]):
            errors.append("audit producer lacks its incomplete-scope reason")

    runtime_errors = validate_audit_runtime(manifest["runtime"])
    errors.extend(runtime_errors)
    inputs = manifest["inputs"]
    if not isinstance(inputs, dict) or set(inputs) != AUDIT_INPUT_ROLES:
        errors.append("audit input manifest roles disagree")
        inputs = {}
    if set(expected) != AUDIT_INPUT_ROLES - {"source_pdf"}:
        errors.append("referee audit input specification is incomplete")
    missing: list[str] = []
    for role in sorted(AUDIT_INPUT_ROLES - {"source_pdf"}):
        spec = expected.get(role)
        entry = inputs.get(role)
        if spec is None or not isinstance(entry, dict):
            errors.append(f"audit input entry is missing: {role}")
            continue
        path, required, payload = spec
        present = payload is not None
        expected_entry = {
            "file": path.name,
            "required": required,
            "present": present,
            "bytes": len(payload) if payload is not None else None,
            "sha256": (
                sha256_bytes(payload) if payload is not None else None),
        }
        if entry != expected_entry:
            errors.append(f"audit input bytes/metadata disagree: {role}")
        if required and not present:
            missing.append(role)
    source_entry = inputs.get("source_pdf")
    try:
        logical_source_path = source_path.relative_to(
            source_root.expanduser()).as_posix()
    except ValueError:
        logical_source_path = source_path.name
    expected_source_entry = {
        "file": source_identity.split(":", 1)[-1],
        "logical_identity": source_identity,
        "path": logical_source_path,
        "required": True,
        "present": True,
        "bytes": len(source_payload),
        "sha256": sha256_bytes(source_payload),
        "expected_sha256": expected_source_sha256,
    }
    if source_entry != expected_source_entry:
        errors.append("audit source PDF bytes/identity disagree")
    if manifest["missing_required"] != missing:
        errors.append("audit missing-required inventory is false")
    inputs_complete = not missing
    if manifest["inputs_complete"] is not inputs_complete:
        errors.append("audit inputs_complete relation is false")

    render = manifest["render"]
    html_spec = expected.get("html")
    html_payload = html_spec[2] if html_spec is not None else None
    expected_entrypoint = html_spec[0].name if html_spec is not None else None
    independent_dependencies: list[dict[str, Any]] = []
    render_errors: list[str] = []
    if isinstance(html_payload, bytes) and expected_entrypoint is not None:
        independent_dependencies, render_errors = audit_render_dependencies(
            html_payload, expected_entrypoint, html_dir)
    else:
        render_errors.append("HTML snapshot is absent")
    if not isinstance(render, dict) or set(render) != {
            "entrypoint", "dependencies", "errors", "complete",
            "network_policy"}:
        errors.append("audit render manifest schema is unsupported")
        dependency_paths: list[str] = []
    else:
        dependencies = render["dependencies"]
        dependency_paths = (
            [item.get("path") for item in dependencies]
            if isinstance(dependencies, list)
            and all(isinstance(item, dict) for item in dependencies)
            else []
        )
        if render["entrypoint"] != expected_entrypoint:
            errors.append("audit render entrypoint is false")
        if dependencies != independent_dependencies:
            errors.append("audit render dependency closure/bytes disagree")
        if render["errors"] != render_errors:
            errors.append("audit render error inventory disagrees")
        if render["complete"] is not (not render_errors):
            errors.append("audit render complete relation is false")
        if render["network_policy"] != (
                "deny-except-retained-relative-resources-and-inline-data"):
            errors.append("audit render network policy is unsupported")

    provenance = audit_record.get("provenance_validation")
    if provenance != {
            "validated_before": True,
            "validated_after": True,
            "error": None}:
        errors.append("audit provenance was not validated before and after")
    roundtrip_scope, roundtrip_errors = validate_audit_roundtrip(
        audit_record, expected_entrypoint or "", dependency_paths)
    errors.extend(roundtrip_errors)
    attestation = audit_record.get("attestation")
    if not isinstance(attestation, dict) or set(attestation) != {
            "inputs_complete", "producer_execution_bound",
            "base_runtime_scope_complete", "roundtrip_runtime_scope_complete",
            "validated_before_after", "complete", "enforceable",
            "incomplete_reasons", "future_gate_required"}:
        errors.append("audit top-level attestation schema is unsupported")
        attestation = {}
    else:
        expected_attestation = {
            "inputs_complete": inputs_complete,
            "producer_execution_bound": False,
            "base_runtime_scope_complete": False,
            "roundtrip_runtime_scope_complete": roundtrip_scope,
            "validated_before_after": True,
            "complete": False,
            "enforceable": False,
        }
        for key, expected_value in expected_attestation.items():
            if attestation.get(key) is not expected_value:
                errors.append(f"audit attestation relation is false: {key}")
        if (not isinstance(attestation["incomplete_reasons"], list)
                or not attestation["incomplete_reasons"]
                or not all(isinstance(item, str) and item
                           for item in attestation["incomplete_reasons"])
                or not isinstance(attestation["future_gate_required"], str)
                or not attestation["future_gate_required"]):
            errors.append("audit attestation blocker explanation is malformed")
    if manifest["attestation_complete"] is not False:
        errors.append("audit manifest overclaims producer attestation")
    if manifest["enforceable"] is not False:
        errors.append("audit manifest overclaims enforceability")
    if manifest["complete"] is not False:
        errors.append("audit manifest overclaims completeness")
    if manifest["attestation_complete"] is False:
        blockers.append("audit producer/runtime attestation is incomplete")
    if manifest["enforceable"] is False:
        blockers.append("audit evidence is not yet enforceable")
    if manifest["complete"] is False:
        blockers.append("audit input manifest is intentionally non-gating")
    if attestation.get("base_runtime_scope_complete") is False:
        blockers.append("audit base runtime scope is incomplete")
    blockers.append(
        "audit PyMuPDF/application runtime closure is manifest-"
        "self-consistent only; the referee independently rehashes the "
        "Python executable but not every named module or native dependency"
    )
    if roundtrip_scope is False:
        blockers.append("audit roundtrip native runtime scope is incomplete")
        blockers.append(
            "audit Playwright/Chromium closure is manifest-schema checked "
            "but not independently rehashed by the standalone referee"
        )
    binding_valid = not errors
    complete = bool(
        binding_valid
        and manifest["complete"] is True
        and manifest["enforceable"] is True
        and attestation.get("complete") is True
        and attestation.get("enforceable") is True
    )
    reason_parts = [
        *(f"invalid: {error}" for error in errors),
        *(f"blocked: {blocker}" for blocker in blockers),
    ]
    return {
        "binding_valid": binding_valid,
        "manifest_inputs_complete": inputs_complete,
        "attestation_complete": bool(
            manifest["attestation_complete"]
            and attestation.get("complete")),
        "enforceable": bool(
            manifest["enforceable"] and attestation.get("enforceable")),
        "complete": complete,
        "reason": "; ".join(reason_parts) if reason_parts else "complete",
        "errors": errors,
        "blockers": blockers,
        "producer_sha256": producer.get("sha256") if isinstance(
            producer, dict) else None,
        "runtime_tree_sha256": (
            ((manifest.get("runtime") or {}).get(
                "loaded_application_files") or {}).get("tree_sha256")
        ),
        "runtime_manifest_self_consistent": not runtime_errors,
        "base_runtime_closure_independently_attested": False,
        "roundtrip_runtime_closure_independently_attested": False,
        "render_dependency_count": len(independent_dependencies),
        "render_dependencies": independent_dependencies,
        "roundtrip_present": roundtrip_scope is not None,
    }


def bind_audit_assertion(
        audit: dict[str, Any],
        ledger: dict[str, Any],
        slots: dict[str, dict[str, Any]],
        emission_inventory: dict[str, Any],
        ) -> dict[str, Any]:
    """Bind legacy-cell audit identities to the canonical subject ledger."""
    errors: list[str] = []
    active_order = [
        subject["cell_id"] for subject in ledger["subjects"]
        if subject["state"] in {"active_resolved", "active_unresolved"}
    ]
    active_ids = set(active_order)

    def bound_ids(key: str, label: str) -> list[str] | None:
        try:
            return string_list(audit.get(key), label)
        except RefereeError as error:
            errors.append(str(error))
            return None

    expected_ids = bound_ids(
        "expected_comb_ids", "audit expected comb IDs")
    checked_ids = bound_ids(
        "checked_comb_ids", "audit checked comb IDs")
    if (expected_ids is not None and checked_ids is not None
            and checked_ids != expected_ids):
        errors.append("audit checked IDs differ from expected IDs")
    for label, published_ids in (
            ("expected", expected_ids), ("checked", checked_ids)):
        if published_ids is None:
            continue
        published = set(published_ids)
        missing = sorted(active_ids - published)
        extra = sorted(published - active_ids)
        if missing:
            errors.append(
                f"audit {label} IDs omit active ledger IDs: "
                + ", ".join(missing[:8]))
        if extra:
            errors.append(
                f"audit {label} IDs add non-active ledger IDs: "
                + ", ".join(extra[:8]))
    emitted_ids = sorted(slots)
    if audit.get("emitted_comb_ids") != emitted_ids:
        errors.append("audit emitted inventory differs from parsed HTML")
    if audit.get("unexpected_emitted_comb_ids") != (
            emission_inventory["unexpected_emitted_cell_ids"]):
        errors.append(
            "audit unexpected-emission inventory differs from ledger binding")

    ledger_aliases = {
        subject["legacy_cell_id"]: subject for subject in ledger["subjects"]
    }
    inference_aliases = {
        inference["cell_id"]: inference for inference in ledger["inferences"]
    }

    def validated_noncomb_binding_offender(offender: Any) -> bool:
        if not isinstance(offender, dict):
            return False
        failure_kinds = offender.get("failure_kinds") or ()
        relation = offender.get("layout_relation")
        emission_relation = offender.get("emission_relation")
        return bool(
            emission_relation == "invalid"
            and (
                (relation == "cell-binding-invalid"
                 and "emitted-cell-binding-invalid" in failure_kinds)
                or (relation == "not-owned"
                    and "unowned-live-comb-markup" in failure_kinds)
            )
        )

    unknown = sorted(
        cell_id for cell_id, offender in audit.get("offenders", {}).items()
        if _CELL_RE.fullmatch(cell_id)
        and cell_id not in ledger_aliases
        and cell_id not in inference_aliases
        and not validated_noncomb_binding_offender(offender)
    )
    if unknown:
        errors.append(
            "audit offenders do not map to ledger legacy identities: "
            + ", ".join(unknown[:8]))
    for missing_id in emission_inventory["missing_active_cell_ids"]:
        subject = next(
            item for item in ledger["subjects"]
            if item["cell_id"] == missing_id)
        offender = audit.get("offenders", {}).get(
            subject["legacy_cell_id"])
        if (offender is None
                or offender.get("emission_state") != "missing-emitted-cell"):
            errors.append(
                "audit omits current missing active emission: "
                + subject["subject_key"])
    for unexpected_id in emission_inventory["unexpected_emitted_cell_ids"]:
        offender = audit.get("offenders", {}).get(unexpected_id)
        if (offender is None
                or "unexpected-emitted-comb"
                not in (offender.get("failure_kinds") or ())):
            errors.append(
                f"audit omits current unexpected emission: {unexpected_id}")
    return {
        "binding_valid": not errors,
        "reason": "complete" if not errors else "; ".join(errors),
        "errors": errors,
        "active_subject_ids": active_order,
        "emitted_ids": emitted_ids,
        "legacy_alias_count": len(ledger_aliases),
    }


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


def audit_relation_for_subject(
        subject: dict[str, Any],
        audit_complete: bool,
        audit_offender: dict[str, Any] | None,
        ) -> tuple[int | None, str]:
    """Publish audit topology only where the exhaustive audit proves it."""
    if audit_offender is not None:
        return audit_offender.get("printed"), "published-offender"
    if audit_complete and subject["state"] in {
            "active_resolved", "active_unresolved"}:
        return int(subject["topology"]["cells"]), "complete-non-offender"
    if audit_complete:
        return None, "complete-blocked-subject"
    return None, "unknown-truncated"


def comparison(cell: dict[str, Any], audit_complete: bool) -> tuple[str, str]:
    ledger_state = cell.get("ledger_state")
    if ledger_state not in {"active_resolved", "active_unresolved"}:
        return (
            "unevaluable",
            "ledger subject has no active topology for adjudication",
        )
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


def transition_decision(
        cell: dict[str, Any], comparison_status: str) -> tuple[str, str]:
    """Report review eligibility without mutating the blocking ledger."""
    ledger_state = cell.get("ledger_state")
    if ledger_state == "active_resolved":
        return "none", "active ledger subject is already resolved"
    if ledger_state == "active_unresolved":
        if comparison_status == "agree":
            return (
                "eligible-for-reviewed-resolution",
                "four-way evidence agrees; explicit review is still required",
            )
        return (
            "blocked",
            "active unresolved ledger subject remains blocking while "
            f"comparison status is {comparison_status}",
        )
    if ledger_state == "retained_unresolved":
        return (
            "explicit-transition-required",
            "retained unresolved subject has no active topology; an explicit "
            "ledger transition is required",
        )
    return (
        "blocked",
        "unknown ledger state cannot be transitioned",
    )


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
    manifest_binding = (
        form.get("audit_evidence", {}).get("manifest_binding", {}))
    render_dependencies = manifest_binding.get("render_dependencies", [])
    if isinstance(render_dependencies, list):
        for entry in render_dependencies:
            if not isinstance(entry, dict):
                changed.append("audit_render_dependency_manifest")
                continue
            logical = entry.get("path")
            expected_sha = entry.get("sha256")
            if not isinstance(logical, str) or not isinstance(expected_sha, str):
                changed.append("audit_render_dependency_manifest")
                continue
            path = args.html_dir.joinpath(
                *pathlib.PurePosixPath(logical).parts)
            try:
                actual = sha256_file(path)
            except OSError:
                changed.append(f"audit_render_dependency:{logical}")
                continue
            if actual != expected_sha:
                changed.append(f"audit_render_dependency:{logical}")
    return changed


def form_report(layout_path: pathlib.Path, args: argparse.Namespace,
                audit_by_slug: dict[str, dict[str, Any]],
                poppler: dict[str, Any]) -> dict[str, Any]:
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
    expected_combs = EXPECTED_COMBS_BY_SLUG.get(slug)
    if expected_combs is None:
        raise RefereeError(f"{slug}: form is not in the pinned referee corpus")
    ledger = validate_comb_ledger(
        slug, layout, args.lattice_producer_bytes)
    html_structure_sha256 = emitted_structure_sha256(html_bytes)
    if html_structure_sha256 != EXPECTED_HTML_STRUCTURE_SHA256.get(slug):
        raise RefereeError(
            f"{slug}: emitted HTML bytes changed from the reviewed pin")
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
    source_contract = layout.get("source")
    if (not isinstance(source_contract, dict)
            or set(source_contract) != {
                "file", "sha256", "bytes", "page_count",
            }
            or source_contract.get("bytes") != len(pdf_bytes)
            or source_contract.get("page_count") != len(layout["pages"])):
        raise RefereeError(
            f"{slug}: source PDF provenance contract is incomplete")

    emission_contract = emitted_geometry_contract(layout, guide)
    if set(emission_contract) != set(ledger["active_cell_ids"]):
        raise RefereeError(
            f"{slug}: guide/layout emission contract does not exactly bind "
            "the active subject ledger")
    slots = slot_records(html_parser, emission_contract)
    emission_inventory = validate_emission_inventory(ledger, slots)
    audit_record = audit_by_slug.get(slug)
    owner_binding = audit_owner_binding(layout_bytes, ledger)
    audit = audit_evidence(audit_record, owner_binding)
    manifest_binding = bind_audit_manifest(audit_record, {
        "ir": (ir_path, True, ir_bytes),
        "layout": (layout_path, True, layout_bytes),
        "html": (html_path, True, html_bytes),
        "guide": (guide_path, True, guide_bytes),
        "guide_html": (
            guide_html_path, False, snapshots["guide_html"]),
    },
        source_path=pdf,
        source_identity=str(source_contract["file"]),
        source_root=args.source_root,
        source_payload=pdf_bytes,
        expected_source_sha256=actual_sha,
        html_dir=args.html_dir,
        producer_sources={
            AUDIT_PRODUCER_FILE: args.audit_producer_bytes,
            **args.audit_dependency_bytes,
        },
    )
    assertion_binding = bind_audit_assertion(
        audit, ledger, slots, emission_inventory)
    audit["input_manifest_verified"] = manifest_binding["binding_valid"]
    audit["input_manifest_reason"] = manifest_binding["reason"]
    audit["manifest_binding"] = manifest_binding
    audit["ledger_binding"] = assertion_binding
    audit["evidence_published"] = bool(audit.get("assertion_valid"))
    audit["byte_and_relation_binding_valid"] = bool(
        audit.get("assertion_valid")
        and manifest_binding["binding_valid"]
        and assertion_binding["binding_valid"]
    )
    audit["runtime_closure_independently_attested"] = False
    audit["integrity_valid"] = bool(
        audit["byte_and_relation_binding_valid"]
        and manifest_binding[
            "base_runtime_closure_independently_attested"]
        and manifest_binding[
            "roundtrip_runtime_closure_independently_attested"]
    )
    audit["complete"] = bool(
        audit["integrity_valid"]
        and audit["byte_and_relation_binding_valid"]
        and manifest_binding["complete"])
    audit_reasons = [
        value for value in (
            None if audit.get("assertion_valid") else audit.get("reason"),
            None if assertion_binding["binding_valid"]
            else assertion_binding["reason"],
            None if manifest_binding["complete"]
            else manifest_binding["reason"],
        ) if value
    ]
    audit["reason"] = (
        "complete" if audit["complete"] else "; ".join(audit_reasons))
    cells: list[dict[str, Any]] = []
    page_meta: list[dict[str, Any]] = []
    subjects_by_page: dict[int, list[dict[str, Any]]] = {}
    for subject in ledger["subjects"]:
        subjects_by_page.setdefault(int(subject["page"]), []).append(subject)

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
            for subject in subjects_by_page.get(page_index, ()):
                source_cell = subject["source_cell"]
                result = classify_band(
                    source_cell, svg, ledger_state=subject["state"])
                report_cell_id = (
                    subject["cell_id"] or subject["legacy_cell_id"])
                emitted = slots.get(report_cell_id)
                audit_offender = audit["offenders"].get(
                    subject["legacy_cell_id"])
                audit_printed, audit_relation = audit_relation_for_subject(
                    subject, bool(audit["complete"]), audit_offender)
                cells.append({
                    "cell": report_cell_id,
                    "subject_key": subject["subject_key"],
                    "legacy_cell_id": subject["legacy_cell_id"],
                    "cell_id": subject["cell_id"],
                    "ledger_state": subject["state"],
                    "ledger_blocks_gate": subject["blocks_gate"],
                    "ledger_reason_codes": subject["reason_codes"],
                    "ledger_topology_sha256": subject["topology"]["sha256"],
                    "ledger_evidence": subject["ledger"],
                    "page": page_index,
                    "bbox": list(subject["legacy_bbox"]),
                    "latticed": int(subject["topology"]["cells"]),
                    "lattice_divider_x": subject["topology"]["divider_x"],
                    "emitted": emitted["count"] if emitted else None,
                    "emitted_indexes_valid": bool(emitted and emitted["valid"]),
                    "emitted_evidence": emitted,
                    "audit_printed": audit_printed,
                    "audit_relation": audit_relation,
                    "referee": result,
                })

    cell_ids = {cell["cell"] for cell in cells}
    if len(cell_ids) != len(cells) or len(cells) != expected_combs:
        raise RefereeError(
            f"{slug}: published subject identities are not exhaustive")
    if slug == "2551q-2018":
        validate_2551q_referee_golden(cells)
    for cell in cells:
        status, reason = comparison(cell, bool(audit.get("complete")))
        cell["comparison_status"] = status
        cell["comparison_reason"] = reason
        transition_status, transition_reason = transition_decision(
            cell, status)
        cell["transition_status"] = transition_status
        cell["transition_reason"] = transition_reason
        cell["four_way"] = {
            "referee": (
                int(cell["referee"]["compartments"])
                if cell["referee"].get("status") == "measured" else None
            ),
            "lattice": cell["latticed"],
            "audit": cell["audit_printed"],
            "emitted": cell["emitted"],
        }

    source_measured = [
        cell for cell in cells if cell["referee"]["status"] == "measured"]
    source_unevaluable = [
        cell for cell in cells if cell["referee"]["status"] != "measured"]
    unevaluable = [
        cell for cell in cells
        if cell["comparison_status"] == "unevaluable"]
    layout_mismatches = [
        cell for cell in source_measured
        if int(cell["referee"]["compartments"]) != int(cell["latticed"])
    ]
    position_mismatches = [
        cell for cell in source_measured
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
    if ledger["counts"]["blocking"]:
        status = "unevaluable"
        reasons.append(
            f"{ledger['counts']['blocking']} lattice-ledger blockers")
    if not emission_inventory["complete"]:
        status = "unevaluable"
        reasons.append(
            f"emission inventory incomplete: {emission_inventory['reason']}")
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
            "bytes": len(pdf_bytes),
            "page_count": len(layout["pages"]),
            "layout_pin": dict(source_contract),
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
        "lattice_evidence": ledger["lattice"],
        "poppler": poppler,
        "pages": page_meta,
        "audit_evidence": {
            key: value for key, value in audit.items() if key != "offenders"
        },
        "emission_inventory": emission_inventory,
        "emission_binding_errors": html_parser.invalid_bindings,
        "counts": {
            "combs": len(cells),
            "subjects": ledger["counts"]["subjects"],
            "subjects_active": ledger["counts"]["active"],
            "subjects_active_resolved": ledger["counts"]["active_resolved"],
            "subjects_active_unresolved": ledger["counts"]["active_unresolved"],
            "subjects_retained_unresolved": (
                ledger["counts"]["retained_unresolved"]),
            "inferences_suppressed": (
                ledger["counts"]["inferences_suppressed"]),
            "ledger_blocking": ledger["counts"]["blocking"],
            "measured": len(source_measured),
            "source_unevaluable": len(source_unevaluable),
            "unevaluable": len(unevaluable),
            "referee_layout_mismatches": len(layout_mismatches),
            "referee_layout_position_mismatches": len(position_mismatches),
            "emission_layout_mismatches": len(emission_mismatches),
            "comparisons": comparison_counts,
        },
        "inferences": [
            {
                "page": inference["page"],
                "subject_key": inference["subject_key"],
                "cell_id": inference["cell_id"],
                "state": inference["state"],
                "blocks_gate": inference["blocks_gate"],
                "reason_codes": inference["reason_codes"],
                "bbox": inference["bbox"],
                "topology_sha256": inference["topology"]["sha256"],
                "ledger_evidence": inference["ledger"],
                "emitted_evidence": slots.get(inference["cell_id"]),
            }
            for inference in ledger["inferences"]
        ],
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

    bounded = run_bounded_subprocess(
        [sys.executable, "-c", "print('bounded-self-test')"],
        timeout_seconds=2.0,
        label="bounded self-test",
    )
    assert bounded.returncode == 0
    assert bounded.stdout.strip() == "bounded-self-test"
    try:
        run_bounded_subprocess(
            [
                sys.executable,
                "-c",
                (
                    "import subprocess,sys,time;"
                    "subprocess.Popen([sys.executable,'-c',"
                    "\"import time;time.sleep(10)\"]);"
                    "time.sleep(10)"
                ),
            ],
            timeout_seconds=0.1,
            label="bounded timeout self-test",
        )
    except RefereeError as error:
        assert "fixed 0.1-second deadline" in str(error)
        assert SUBPROCESS_CLEANUP_POLICY in str(error)
    else:
        raise AssertionError("bounded subprocess timeout was not enforced")

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

    # A source guide band may live immediately outside one cell edge because
    # the shared horizontal owns the partition.  The whole attached band is
    # evidence; clipping it to the cell would discard it as <=0.25pt noise.
    attached_above = {
        **cell,
        "y0": 20.0,
        "y1": 30.0,
        "comb": {**cell["comb"], "y0": 13.76, "y1": 19.76},
    }
    attached_above_paints = [
        paint(10, 13.76, 19.76),
        paint(20, 13.76, 19.76),
        paint(30, 13.76, 19.76),
    ]
    result = classify_band(attached_above, SvgPage(
        100, 100, attached_above_paints, [], "x"))
    assert result["status"] == "measured", result
    assert result["compartments"] == 4, result

    # The attached-band retry must not borrow a different rectangle in the
    # original cell to justify an empty multi-pitch gap in the external band.
    # Only the two anchors are painted in that band; the missing midpoint is
    # safe solely when the same evaluation window has its own single-frame
    # subject proof.
    unrelated_cell_frame = [
        Paint(-0.1, 19.9, 40.1, 20.1, 0.0, 10,
              "stroke", "unrelated-cell-frame"),
        Paint(-0.1, 29.9, 40.1, 30.1, 0.0, 11,
              "stroke", "unrelated-cell-frame"),
        Paint(-0.1, 19.9, 0.1, 30.1, 0.0, 12,
              "stroke", "unrelated-cell-frame"),
        Paint(39.9, 19.9, 40.1, 30.1, 0.0, 13,
              "stroke", "unrelated-cell-frame"),
    ]
    attached_gap_wrong_frame = classify_band(attached_above, SvgPage(
        100, 100,
        [paint(10, 13.76, 19.76), paint(30, 13.76, 19.76),
         *unrelated_cell_frame],
        [], "x"))
    assert attached_gap_wrong_frame["status"] == "unevaluable", (
        attached_gap_wrong_frame)
    assert attached_gap_wrong_frame["reason"] == (
        "chosen source topology lacks a clean single-frame subject proof"
    ), attached_gap_wrong_frame
    assert any(
        band.get("unproven_subject_gaps")
        for band in attached_gap_wrong_frame["bands"]
    ), attached_gap_wrong_frame

    attached_below = {
        **cell,
        "comb": {**cell["comb"], "y0": 10.24, "y1": 16.24},
    }
    result = classify_band(attached_below, SvgPage(
        100, 100,
        [paint(10, 10.24, 16.24),
         paint(20, 10.24, 16.24),
         paint(30, 10.24, 16.24)],
        [], "x"))
    assert result["status"] == "measured", result
    assert result["compartments"] == 4, result

    detached = {
        **cell,
        "comb": {**cell["comb"], "y0": 10.26, "y1": 16.26},
    }
    result = classify_band(detached, SvgPage(
        100, 100,
        [paint(10, 10.26, 16.26),
         paint(20, 10.26, 16.26),
         paint(30, 10.26, 16.26)],
        [], "x"))
    assert result["status"] == "unevaluable", result
    assert result["reason"] == (
        "no common Poppler band contains every recognised divider"), result
    detached_no_retry = classify_band(
        detached,
        SvgPage(
            100, 100,
            [paint(10, 10.26, 16.26),
             paint(20, 10.26, 16.26),
             paint(30, 10.26, 16.26)],
            [], "x"),
        _evaluation_window=(0.0, 10.0),
    )
    assert result == detached_no_retry

    enveloping = {
        **cell,
        "comb": {**cell["comb"], "y0": -1.0, "y1": 11.0},
    }
    result = classify_band(enveloping, SvgPage(
        100, 100,
        [paint(10, -1, 11), paint(20, -1, 11), paint(30, -1, 11)],
        [], "x"))
    assert result["status"] == "measured", result
    assert result["compartments"] == 4, result

    # An attached band whose original cell-clipped verdict is already
    # ambiguous must not invoke the fallback.
    crossing = {
        **cell,
        "y0": 20.0,
        "y1": 30.0,
        "comb": {**cell["comb"], "y0": 19.76, "y1": 25.76},
    }
    crossing_ambiguous_page = SvgPage(
        100, 100,
        [paint(10, 19.76, 25.76),
         paint(20, 19.76, 22.0),
         paint(30, 19.76, 25.76)],
        [], "x")
    crossing_ambiguous = classify_band(crossing, crossing_ambiguous_page)
    crossing_ambiguous_no_retry = classify_band(
        crossing, crossing_ambiguous_page,
        _evaluation_window=(20.0, 30.0))
    assert crossing_ambiguous["status"] == "unevaluable", crossing_ambiguous
    assert crossing_ambiguous == crossing_ambiguous_no_retry

    crossing_minority_page = SvgPage(
        100, 100,
        [paint(10, 20.0, 22.0), paint(30, 20.0, 22.0)], [], "x")
    crossing_minority = classify_band(crossing, crossing_minority_page)
    crossing_minority_no_retry = classify_band(
        crossing, crossing_minority_page,
        _evaluation_window=(20.0, 30.0))
    assert crossing_minority["status"] == "unevaluable", crossing_minority
    assert "strict majority" in crossing_minority["reason"], crossing_minority
    assert crossing_minority == crossing_minority_no_retry

    off_band_decoy = classify_band(attached_above, SvgPage(
        100, 100,
        [*attached_above_paints, paint(15, 22, 28)], [], "x"))
    assert off_band_decoy["status"] == "measured", off_band_decoy
    assert off_band_decoy["compartments"] == 4, off_band_decoy
    assert 15.0 not in off_band_decoy["source_divider_x"], off_band_decoy

    attached_partial = classify_band(attached_above, SvgPage(
        100, 100,
        [paint(10, 13.76, 19.76),
         paint(20, 13.76, 16.66),
         paint(30, 13.76, 19.76)],
        [], "x"))
    assert attached_partial["status"] == "unevaluable", attached_partial

    attached_clipped = classify_band(attached_above, SvgPage(
        100, 100,
        [*attached_above_paints,
         Paint(24.9, 13.76, 25.1, 19.76, 0.0, 5,
               "test", "attached-clipped", True)],
        [], "x"))
    assert attached_clipped["status"] == "unevaluable", attached_clipped

    attached_outward = {
        **attached_above,
        "comb": {
            **attached_above["comb"],
            "divider_x": [20.0, 30.0],
        },
    }
    attached_off_pitch = classify_band(attached_outward, SvgPage(
        100, 100,
        [paint(5, 13.76, 19.76), paint(10, 13.76, 19.76),
         paint(20, 13.76, 19.76), paint(30, 13.76, 19.76)],
        [], "x"))
    assert attached_off_pitch["status"] == "unevaluable", attached_off_pitch

    attached_unsupported = classify_band(attached_above, SvgPage(
        100, 100, attached_above_paints,
        [UnsupportedRegion(
            5, 13.76, 35, 19.76,
            "unsupported attached overlay", "attached-overlay")],
        "x"))
    assert attached_unsupported["status"] == "unevaluable", attached_unsupported

    attached_non_majority = classify_band(attached_above, SvgPage(
        100, 100,
        [paint(30, 13.76, 14.76), paint(20, 13.76, 14.36)],
        [], "x"))
    assert attached_non_majority["status"] == "unevaluable", (
        attached_non_majority)

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

    # A declared anchor does not make the inner bar of a composite frame into
    # a writable divider; anchor matching itself uses frame-distinct groups.
    composite_anchor = {
        **cell,
        "comb": {
            "cells": 2,
            "divider_x": [1.5],
            "pitch_pt": 3.0,
            "divider_gray": 0.0,
            "y0": 2.0,
            "y1": 8.0,
        },
    }
    composite_anchor_result = classify_band(
        composite_anchor,
        SvgPage(100, 100, [
            *source_frame(),
            Paint(1.0, 2, 2.0, 8, 0.0, 14,
                  "stroke", "declared-inner-frame-bar"),
        ], [], "x"),
    )
    assert composite_anchor_result["status"] == "unevaluable", (
        composite_anchor_result)

    broad_frame_result = classify_band(
        {
            **cell,
            "comb": {
                "cells": 2,
                "divider_x": [2.5],
                "pitch_pt": 3.0,
                "divider_gray": 0.0,
                "y0": 2.0,
                "y1": 8.0,
            },
        },
        SvgPage(100, 100, [
            Paint(-1.0, 2, 1.0, 8, 0.0, 0,
                  "stroke", "broad-outer-frame"),
            Paint(2.0, 2, 3.0, 8, 0.0, 1,
                  "stroke", "narrow-inner-frame"),
        ], [], "x"),
    )
    assert broad_frame_result["status"] == "unevaluable", broad_frame_result

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
    erased_stale_page = SvgPage(100, 100, [
        paint(20, order=0),
        paint(23, order=1),
        Paint(22.7, 2, 23.3, 8, 1.0, 2,
              "fill", "supported-white-erasure"),
    ], [], "x")
    result = classify_band(stale, erased_stale_page)
    assert result["status"] == "unevaluable", result

    # A missing lattice anchor is independently measurable only for an
    # already-active unresolved subject whose one observed topology occupies
    # the complete open band.  The source count comes from observed final ink;
    # the retained path remains closed so an ordinary table rail cannot become
    # a newly discovered comb.
    active_partial = classify_band(
        stale, erased_stale_page, ledger_state="active_unresolved")
    assert active_partial["status"] == "measured", active_partial
    assert active_partial["compartments"] == 2, active_partial
    assert active_partial["anchors_complete"] is False, active_partial
    assert active_partial["positions_match"] is False, active_partial
    assert active_partial["missing_anchor_x"] == [23.0], active_partial
    partial_certificate = active_partial.get(
        "active_partial_anchor_certificate")
    assert isinstance(partial_certificate, dict), active_partial
    assert partial_certificate == {
        "criterion": ACTIVE_PARTIAL_ANCHOR_CRITERION,
        "valid": True,
        "ledger_state": "active_unresolved",
        "subject_ownership_basis": "active_unresolved lattice ledger",
        "independent_source_enclosure_proven": False,
        "divider_count_basis": "final-composited Poppler vector topology",
        "missing_anchor_basis": (
            "raw target-tone rail exhaustively replaced by one supported "
            "unclipped non-target final owner"
        ),
        "anchor_corridor_clipped_paint_elements": [],
        "anchor_corridor_unsupported_region_elements": [],
        "open_y0": 2.0,
        "open_y1": 8.0,
        "coverage_pt": 6.0,
        "source_divider_x": [20.0],
        "observed_anchor_x": [20.0],
        "missing_anchor_x": [23.0],
        "missing_anchor_proofs": [{
            "layout_x": 23.0,
            "corridor_x0": 22.75,
            "corridor_x1": 23.25,
            "proof_x0": 22.75,
            "proof_x1": 23.25,
            "open_y0": 2.0,
            "open_y1": 8.0,
            "raw_anchor_rails": [{
                "element": "x23-o1",
                "order": 1,
                "kind": "test",
                "x0": 22.9,
                "x1": 23.1,
                "center_x": 23.0,
                "delta_pt": 0.0,
                "y0": 2,
                "y1": 8,
                "tone": 0.0,
                "clipped": False,
            }],
            "raw_rail_identity_valid": True,
            "proof_top_role_ambiguities": [],
            "erasure_slabs": [{
                "y0": 2.0,
                "y1": 8.0,
                "sample_y": 5.0,
                "raw_rail_elements": ["x23-o1"],
                "raw_intervals": [[22.9, 23.1]],
                "final_owner_segments": [{
                    "x0": 22.9,
                    "x1": 23.1,
                    "element": "supported-white-erasure",
                    "order": 2,
                    "kind": "fill",
                    "tone": 1.0,
                    "clipped": False,
                }],
                "ambiguous_top_roles": [],
            }],
            "erasure_owner_roles": [{
                "element": "supported-white-erasure",
                "order": 2,
                "kind": "fill",
                "tone": 1.0,
            }],
            "clipped_paint_elements": [],
            "final_target_tone_segments": [],
            "unsupported_region_elements": [],
        }],
    }, partial_certificate
    for ineligible_state in (None, "active_resolved", "retained_unresolved"):
        ineligible = classify_band(
            stale, erased_stale_page, ledger_state=ineligible_state)
        assert ineligible["status"] == "unevaluable", (
            ineligible_state, ineligible)

    # Ledger ownership is necessary but not sufficient: a lone active rail
    # with no erased source rail at the declared missing anchor stays closed.
    lone_active_rail = classify_band(
        stale,
        SvgPage(100, 100, [paint(20)], [], "x"),
        ledger_state="active_unresolved",
    )
    assert lone_active_rail["status"] == "unevaluable", lone_active_rail

    # The partial path applies the same paper-versus-ink proximity test as the
    # complete path.  An inner bar separated from a frame edge by less paper
    # than their combined weights is not a writable compartment boundary.
    composite_partial = {
        **cell,
        "comb": {
            "cells": 3,
            "divider_x": [1.5, 4.5],
            "pitch_pt": 3.0,
            "divider_gray": 0.0,
            "y0": 2.0,
            "y1": 8.0,
        },
    }
    composite_partial_result = classify_band(
        composite_partial,
        SvgPage(100, 100, [
            *source_frame(),
            Paint(1.0, 2, 2.0, 8, 0.0, 14,
                  "stroke", "inner-frame-bar"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert composite_partial_result["status"] == "unevaluable", (
        composite_partial_result)

    # Every certificate condition is fail closed: incomplete band coverage,
    # competing topology, an inexact or incomplete raw rail, incomplete or
    # mixed final erasure, clipping, unsupported/glyph/raster geometry, or
    # surviving target-tone ink prevents the active-only exception.
    partial_coverage = classify_band(
        stale,
        SvgPage(100, 100, [
            paint(20, a=2, b=7, order=0),
            paint(23, order=1),
            Paint(22.7, 2, 23.3, 8, 1.0, 2,
                  "fill", "supported-white-erasure"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert partial_coverage["status"] == "unevaluable", partial_coverage

    competing_partial = {
        **stale,
        "comb": {**stale["comb"],
                 "cells": 4, "divider_x": [20.0, 23.0, 26.0]},
    }
    competing_topologies = classify_band(
        competing_partial,
        SvgPage(100, 100, [
            paint(20, order=0),
            paint(26, a=4, b=8, order=1),
            paint(23, order=2),
            Paint(22.7, 2, 23.3, 8, 1.0, 3,
                  "fill", "supported-white-erasure"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert competing_topologies["status"] == "unevaluable", (
        competing_topologies)

    clipped_observed_anchor = classify_band(
        stale,
        SvgPage(100, 100, [
            Paint(19.9, 2, 20.1, 8, 0.0, 2,
                  "test", "observed-anchor-clip", True),
            paint(23, order=3),
            Paint(22.7, 2, 23.3, 8, 1.0, 4,
                  "fill", "supported-white-erasure"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert clipped_observed_anchor["status"] == "unevaluable", (
        clipped_observed_anchor)

    incomplete_raw_rail = classify_band(
        stale,
        SvgPage(100, 100, [
            paint(20, order=0),
            paint(23, a=2, b=7, order=1),
            Paint(22.7, 2, 23.3, 8, 1.0, 2,
                  "fill", "supported-white-erasure"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert incomplete_raw_rail["status"] == "unevaluable", (
        incomplete_raw_rail)

    shifted_raw_fragments = classify_band(
        stale,
        SvgPage(100, 100, [
            paint(20, order=0),
            paint(22.9, a=2, b=5, order=1),
            paint(23.1, a=5, b=8, order=2),
            Paint(22.6, 2, 23.4, 8, 1.0, 3,
                  "fill", "supported-white-erasure"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert shifted_raw_fragments["status"] == "unevaluable", (
        shifted_raw_fragments)

    inexact_raw_rail = classify_band(
        stale,
        SvgPage(100, 100, [
            paint(20, order=0),
            paint(23.3, order=1),
            Paint(22.7, 2, 23.5, 8, 1.0, 2,
                  "fill", "supported-white-erasure"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert inexact_raw_rail["status"] == "unevaluable", inexact_raw_rail

    overlapping_missing_anchors = {
        **stale,
        "comb": {
            **stale["comb"],
            "cells": 4,
            "divider_x": [20.0, 23.0, 23.3],
            "pitch_pt": 0.3,
        },
    }
    shared_raw_rail = classify_band(
        overlapping_missing_anchors,
        SvgPage(100, 100, [
            Paint(19.95, 2, 20.05, 8, 0.0, 0,
                  "stroke", "observed-rail"),
            Paint(23.1, 2, 23.2, 8, 0.0, 1,
                  "stroke", "ambiguous-raw-rail"),
            Paint(22.8, 2, 23.5, 8, 1.0, 2,
                  "fill", "supported-white-erasure"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert shared_raw_rail["status"] == "unevaluable", shared_raw_rail

    close_observed_and_missing = {
        **stale,
        "comb": {
            **stale["comb"],
            "cells": 3,
            "divider_x": [20.0, 20.3],
            "pitch_pt": 0.3,
        },
    }
    raw_near_observed_anchor = classify_band(
        close_observed_and_missing,
        SvgPage(100, 100, [
            Paint(19.95, 2, 20.05, 8, 0.0, 0,
                  "stroke", "observed-rail"),
            Paint(20.1, 2, 20.2, 8, 0.0, 1,
                  "stroke", "ambiguous-raw-rail"),
            Paint(20.075, 2, 20.225, 8, 1.0, 2,
                  "fill", "supported-white-erasure"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert raw_near_observed_anchor["status"] == "unevaluable", (
        raw_near_observed_anchor)

    incomplete_erasure = classify_band(
        stale,
        SvgPage(100, 100, [
            paint(20, order=0),
            paint(23, order=1),
            Paint(22.7, 2, 23.3, 7, 1.0, 2,
                  "fill", "incomplete-white-erasure"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert incomplete_erasure["status"] == "unevaluable", (
        incomplete_erasure)

    mixed_erasure = classify_band(
        stale,
        SvgPage(100, 100, [
            paint(20, order=0),
            paint(23, order=1),
            Paint(22.7, 2, 23.0, 8, 1.0, 2,
                  "fill", "left-white-erasure"),
            Paint(23.0, 2, 23.3, 8, 1.0, 3,
                  "fill", "right-white-erasure"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert mixed_erasure["status"] == "unevaluable", mixed_erasure

    broad_raw_rail = {
        **stale,
        "comb": {**stale["comb"], "pitch_pt": 4.0},
    }
    split_wide_erasure = classify_band(
        broad_raw_rail,
        SvgPage(100, 100, [
            paint(20, order=0),
            Paint(22.0, 2, 24.0, 8, 0.0, 1,
                  "stroke", "wide-raw-anchor-rail"),
            Paint(21.9, 2, 22.75, 8, 1.0, 2,
                  "fill", "wide-erasure-left"),
            Paint(22.75, 2, 23.25, 8, 1.0, 3,
                  "fill", "wide-erasure-core"),
            Paint(23.25, 2, 24.1, 8, 1.0, 4,
                  "fill", "wide-erasure-right"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert split_wide_erasure["status"] == "unevaluable", (
        split_wide_erasure)

    ambiguous_erasure = classify_band(
        stale,
        SvgPage(100, 100, [
            paint(20, order=0),
            paint(23, order=1),
            Paint(22.7, 2, 23.3, 8, 1.0, 2,
                  "fill", "white-erasure-a"),
            Paint(22.7, 2, 23.3, 8, 1.0, 2,
                  "fill", "white-erasure-b"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert ambiguous_erasure["status"] == "unevaluable", (
        ambiguous_erasure)

    outside_raw_tie = classify_band(
        stale,
        SvgPage(100, 100, [
            *erased_stale_page.paints,
            Paint(23.15, 2, 23.2, 8, 0.5, 3,
                  "fill", "outside-raw-owner-a"),
            Paint(23.15, 2, 23.2, 8, 0.75, 3,
                  "fill", "outside-raw-owner-b"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert outside_raw_tie["status"] == "unevaluable", outside_raw_tie

    clipped_missing_anchor = classify_band(
        stale,
        SvgPage(100, 100, [
            paint(20, order=0), paint(23, order=1),
            Paint(22.7, 2, 23.3, 8, 1.0, 2,
                  "fill", "missing-anchor-clip", True),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert clipped_missing_anchor["status"] == "unevaluable", (
        clipped_missing_anchor)

    for unsupported_reason, unsupported_element in (
        ("glyph use may occlude geometry: #glyph-missing",
         "missing-anchor-glyph"),
        ("embedded raster image intersects source geometry",
         "missing-anchor-raster"),
        ("unsupported source overlay", "missing-anchor-unsupported"),
    ):
        unsupported_missing_anchor = classify_band(
            stale,
            SvgPage(
                100, 100, list(erased_stale_page.paints),
                [UnsupportedRegion(
                    22.9, 2, 23.1, 8,
                    unsupported_reason, unsupported_element,
                    1.0, 3, False)],
                "x",
            ),
            ledger_state="active_unresolved",
        )
        assert unsupported_missing_anchor["status"] == "unevaluable", (
            unsupported_reason, unsupported_missing_anchor)

    thin_unsupported = classify_band(
        stale,
        SvgPage(
            100, 100, list(erased_stale_page.paints),
            [UnsupportedRegion(
                22.9, 5.0, 23.1, 5.1,
                "thin unsupported source overlay",
                "thin-missing-anchor-unsupported",
                1.0, 3, False)],
            "x",
        ),
        ledger_state="active_unresolved",
    )
    assert thin_unsupported["status"] == "unevaluable", thin_unsupported

    thin_observed_unsupported = classify_band(
        stale,
        SvgPage(
            100, 100, list(erased_stale_page.paints),
            [UnsupportedRegion(
                19.9, 5.0, 20.1, 5.1,
                "thin raster over observed divider",
                "thin-observed-anchor-raster",
                1.0, 3, False)],
            "x",
        ),
        ledger_state="active_unresolved",
    )
    assert thin_observed_unsupported["status"] == "unevaluable", (
        thin_observed_unsupported)

    broad_target_at_missing_anchor = classify_band(
        stale,
        SvgPage(100, 100, [
            *erased_stale_page.paints,
            Paint(21, 2, 25, 8, 0.0, 3,
                  "fill", "broad-missing-anchor-ink"),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert broad_target_at_missing_anchor["status"] == "unevaluable", (
        broad_target_at_missing_anchor)

    unexplained_missing_anchor = classify_band(
        stale,
        SvgPage(100, 100, [
            *erased_stale_page.paints,
            paint(21.5, order=3),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert unexplained_missing_anchor["status"] == "unevaluable", (
        unexplained_missing_anchor)

    complete_active = classify_band(
        stale,
        SvgPage(100, 100, [
            paint(20), paint(23),
        ], [], "x"),
        ledger_state="active_unresolved",
    )
    assert complete_active["status"] == "measured", complete_active
    assert "active_partial_anchor_certificate" not in complete_active, (
        complete_active)

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

    # Poppler emits small arrowheads as simple closed path fills.  Preserve the
    # unsupported provenance, but let a squat unclipped bound that cannot be a
    # divider proceed through the same conservative occlusion check as an
    # outlined glyph.  Crossing a real divider, looking divider-like, or being
    # clipped remains unevaluable.
    squat_fill_inside = UnsupportedRegion(
        12, 3, 18, 5, "non-rectangular closed SVG fill",
        "arrow-inside", 0.0, 4, False)
    result = classify_band(cell, SvgPage(
        100, 100, [paint(10), paint(20), paint(30)],
        [squat_fill_inside], "x"))
    assert result["status"] == "measured", result

    squat_fill_crossing = UnsupportedRegion(
        18, 3, 22, 5, "non-rectangular closed SVG fill",
        "arrow-crossing", 0.0, 4, False)
    assert classify_band(cell, SvgPage(
        100, 100, [paint(10), paint(20), paint(30)],
        [squat_fill_crossing], "x"))["status"] == "unevaluable"

    divider_like_fill = UnsupportedRegion(
        14.9, 2, 15.1, 8, "non-rectangular closed SVG fill",
        "divider-like-fill", 0.0, 4, False)
    assert classify_band(cell, SvgPage(
        100, 100, [paint(10), paint(20), paint(30)],
        [divider_like_fill], "x"))["status"] == "unevaluable"

    clipped_squat_fill = dataclasses.replace(
        squat_fill_inside, element="clipped-arrow", clipped=True)
    assert classify_band(cell, SvgPage(
        100, 100, [paint(10), paint(20), paint(30)],
        [clipped_squat_fill], "x"))["status"] == "unevaluable"

    embedded_raster = UnsupportedRegion(
        12, 2, 28, 8, "embedded raster use: #source-test",
        "embedded-raster", None, 4, True)
    assert classify_band(cell, SvgPage(
        100, 100, [paint(10), paint(20), paint(30)],
        [embedded_raster], "x"))["status"] == "unevaluable"

    no_anchor = {**cell, "comb": {**cell["comb"], "cells": 1, "divider_x": []}}
    assert classify_band(no_anchor, page)["status"] == "unevaluable"

    minimal_style = (
        "<style>"
        ".page{position:relative;overflow:hidden}"
        ".c{position:absolute}"
        ".s{position:absolute}"
        "</style>"
    )
    valid_font_preload = (
        '<link rel="preload" href="fonts/tinos-latin-400-normal.woff2" '
        'as="font" type="font/woff2" crossorigin>'
    )
    valid_preload_parser = SlotParser(require_runtime_contract=False)
    valid_preload_parser.feed(
        "<html><head>" + valid_font_preload
        + "</head><body></body></html>"
    )
    valid_preload_parser.close()
    assert not valid_preload_parser.invalid_bindings
    for hostile_preload in (
            valid_font_preload.replace(
                "fonts/tinos-latin-400-normal.woff2",
                "https://example.test/font.woff2"),
            valid_font_preload.replace(
                'rel="preload"', 'rel="stylesheet"'),
            valid_font_preload.replace(
                'type="font/woff2"', 'type="text/css"'),
            valid_font_preload.replace(
                " crossorigin>", ' crossorigin="anonymous">'),
            valid_font_preload.replace(
                "fonts/tinos-latin-400-normal.woff2",
                "fonts/../assets/foreign.woff2"),
            ):
        hostile_parser = SlotParser(require_runtime_contract=False)
        hostile_parser.feed(
            "<html><head>" + hostile_preload
            + "</head><body></body></html>"
        )
        hostile_parser.close()
        assert hostile_parser.invalid_bindings, hostile_preload
    body_preload_parser = SlotParser(require_runtime_contract=False)
    body_preload_parser.feed(
        "<html><head></head><body>" + valid_font_preload
        + "</body></html>"
    )
    body_preload_parser.close()
    assert any(
        "outside the document head" in error
        for error in body_preload_parser.invalid_bindings
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

    def emitted_slot_fixture(slot_markup: str) -> SlotParser:
        fixture_parser = SlotParser(require_runtime_contract=False)
        fixture_parser.feed(
            '<html>' + minimal_style + '<body>'
            '<div class="page page-1" id="page-1" '
            'style="width:100pt;height:100pt">'
            '<div class="layer-cells">'
            '<div id="p1c1" data-field-kind="comb" '
            'data-field-name="p1c1" class="c f" '
            'data-cell-kind="field" data-row="0" data-col="0" '
            'data-comb-pitch="10" data-comb-capacity="2" '
            'data-comb-slots="2" '
            'style="left:0pt;top:0pt;width:20pt;height:10pt">'
            + slot_markup
            + '</div></div></div></body></html>'
        )
        fixture_parser.close()
        return fixture_parser

    missing_emitted_slot = emitted_slot_fixture(
        '<div class="s" data-slot="0" '
        'style="left:0pt;top:0pt;width:10pt;height:10pt">'
        '<input type="text" class="fi fh0 fc" id="p1c1-s0" '
        'name="p1c1" data-slot-index="0" maxlength="1" '
        'autocomplete="off" spellcheck="false"></div>'
    )
    assert not slot_records(
        missing_emitted_slot, parser_expected)["p1c1"]["valid"]
    duplicate_emitted_slot = emitted_slot_fixture(
        '<div class="s" data-slot="0" '
        'style="left:0pt;top:0pt;width:10pt;height:10pt">'
        '<input type="text" class="fi fh0 fc" id="p1c1-s0" '
        'name="p1c1" data-slot-index="0" maxlength="1" '
        'autocomplete="off" spellcheck="false"></div>'
        '<div class="s" data-slot="0" '
        'style="left:10pt;top:0pt;width:10pt;height:10pt">'
        '<input type="text" class="fi fh0 fc" id="p1c1-s0" '
        'name="p1c1" data-slot-index="0" maxlength="1" '
        'autocomplete="off" spellcheck="false"></div>'
    )
    assert not slot_records(
        duplicate_emitted_slot, parser_expected)["p1c1"]["valid"]

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

    def synthetic_ledger_comb(
            x0: float, x1: float, status: str = "resolved",
            reason_codes: list[str] | None = None,
            ) -> dict[str, Any]:
        midpoint = (x0 + x1) / 2
        reasons = list(reason_codes or ())
        return {
            "cells": 2,
            "divider_count": 1,
            "pitch_pt": midpoint - x0,
            "pitch_min_pt": midpoint - x0,
            "pitch_max_pt": x1 - midpoint,
            "slot_x": [x0, midpoint, x1],
            "divider_x": [midpoint],
            "divider_thickness_pt": 0.2,
            "divider_thicknesses_pt": [0.2],
            "divider_gray": 0.0,
            "divider_paint_seq": [1],
            "divider_paint_ranges": [[1, 1]],
            "y0": 1.0,
            "y1": 9.0,
            "height_pt": 8.0,
            "resolution": {
                "status": status,
                "method": "self-test",
                "reason_codes": reasons,
            },
        }

    def refresh_ledger_stats(layout_value: dict[str, Any]) -> None:
        page_value = layout_value["pages"][0]
        subjects_value = page_value["comb_subjects"]
        inferences_value = page_value["comb_inferences"]
        active_resolved_value = sum(
            item["state"] == "active_resolved" for item in subjects_value)
        active_unresolved_value = sum(
            item["state"] == "active_unresolved" for item in subjects_value)
        retained_value = sum(
            item["state"] == "retained_unresolved" for item in subjects_value)
        subject_blockers_value = sum(
            item.get("blocks_gate") is True for item in subjects_value)
        inference_blockers_value = sum(
            item.get("blocks_gate") is True for item in inferences_value)
        comb_cells_value = [
            item for item in page_value["cells"] if "comb" in item
        ]
        page_value["stats"] = {
            "comb_cells": len(comb_cells_value),
            "comb_subjects": len(subjects_value),
            "comb_subjects_active": (
                active_resolved_value + active_unresolved_value),
            "comb_subjects_active_resolved": active_resolved_value,
            "comb_subjects_active_unresolved": active_unresolved_value,
            "comb_subjects_retained_unresolved": retained_value,
            "comb_subjects_retired": 0,
            "comb_subjects_blocking": subject_blockers_value,
            "comb_inferences_suppressed": len(inferences_value),
            "comb_inferences_blocking": inference_blockers_value,
            "comb_evidence_blocking": (
                subject_blockers_value + inference_blockers_value),
            "comb_slots": sum(
                int(item["comb"]["cells"]) for item in comb_cells_value),
        }

    def synthetic_ledger_layout() -> dict[str, Any]:
        cells_value: list[dict[str, Any]] = []
        subjects_value: list[dict[str, Any]] = []
        for index in range(EXPECTED_COMBS_BY_SLUG["0605-1999"]):
            x0 = float(index * 3)
            x1 = x0 + 2.0
            bbox_value = [x0, 0.0, x1, 10.0]
            subject_key = (
                f"p1@{x0:.2f},0.00,{x1:.2f},10.00")
            cell_id = f"p1c{index}"
            comb_value = synthetic_ledger_comb(x0, x1)
            cells_value.append({
                "id": cell_id,
                "subject_key": subject_key,
                "x0": x0,
                "y0": 0.0,
                "x1": x1,
                "y1": 10.0,
                "comb": comb_value,
            })
            subjects_value.append({
                "subject_key": subject_key,
                "legacy_cell_id": cell_id,
                "legacy_bbox": bbox_value,
                "cell_id": cell_id,
                "mapped_partition_cell_ids": [cell_id],
                "state": "active_resolved",
                "reason_codes": [],
                "cells": 2,
                "blocks_gate": False,
            })
        value = {
            "generator": dict(LATTICE_GENERATOR_CONTRACT),
            "pages": [{
                "index": 1,
                "cells": cells_value,
                "comb_subjects": subjects_value,
                "comb_inferences": [],
            }],
        }
        refresh_ledger_stats(value)
        return value

    def clone(value: Any) -> Any:
        return json.loads(json.dumps(value))

    lattice_producer_bytes = (HERE / "lattice.py").read_bytes()
    ledger_fixture = synthetic_ledger_layout()
    ledger_result = validate_comb_ledger(
        "0605-1999", ledger_fixture, lattice_producer_bytes)
    assert ledger_result["counts"] == {
        "subjects": 21,
        "active": 21,
        "active_resolved": 21,
        "active_unresolved": 0,
        "retained_unresolved": 0,
        "inferences_suppressed": 0,
        "blocking": 0,
    }

    for name, mutate in (
        (
            "missing-ledger",
            lambda value: value["pages"][0].pop("comb_subjects"),
        ),
        (
            "missing-inference-ledger",
            lambda value: value["pages"][0].pop("comb_inferences"),
        ),
        (
            "empty-ledger",
            lambda value: value["pages"][0].__setitem__(
                "comb_subjects", []),
        ),
        (
            "duplicate-subject-key",
            lambda value: value["pages"][0]["comb_subjects"][1].__setitem__(
                "subject_key",
                value["pages"][0]["comb_subjects"][0]["subject_key"]),
        ),
        (
            "duplicate-legacy-id",
            lambda value: value["pages"][0]["comb_subjects"][1].__setitem__(
                "legacy_cell_id",
                value["pages"][0]["comb_subjects"][0]["legacy_cell_id"]),
        ),
        (
            "active-cell-mismatch",
            lambda value: (
                value["pages"][0]["comb_subjects"][0].__setitem__(
                    "cell_id", "p1c999"),
                value["pages"][0]["comb_subjects"][0].__setitem__(
                    "mapped_partition_cell_ids", ["p1c999"]),
            ),
        ),
        (
            "retired-state",
            lambda value: value["pages"][0]["comb_subjects"][0].__setitem__(
                "state", "retired_proven_false"),
        ),
        (
            "unknown-state",
            lambda value: value["pages"][0]["comb_subjects"][0].__setitem__(
                "state", "mystery"),
        ),
    ):
        broken_ledger = clone(ledger_fixture)
        mutate(broken_ledger)
        try:
            validate_comb_ledger(
                "0605-1999", broken_ledger, lattice_producer_bytes)
        except RefereeError:
            pass
        else:
            raise AssertionError(f"invalid comb ledger passed: {name}")
    try:
        validate_comb_ledger("0605-1999", ledger_fixture, b"stale lattice")
    except RefereeError:
        pass
    else:
        raise AssertionError("stale lattice producer bytes were accepted")

    reverse_mismatch = clone(ledger_fixture)
    reverse_mismatch["pages"][0]["cells"].append({
        "id": "p1c999",
        "subject_key": "p1@90.00,0.00,92.00,10.00",
        "x0": 90.0, "y0": 0.0, "x1": 92.0, "y1": 10.0,
        "comb": synthetic_ledger_comb(90.0, 92.0),
    })
    refresh_ledger_stats(reverse_mismatch)
    try:
        validate_comb_ledger(
            "0605-1999", reverse_mismatch, lattice_producer_bytes)
    except RefereeError:
        pass
    else:
        raise AssertionError("unledgered active comb passed reverse mapping")

    unresolved_ledger = clone(ledger_fixture)
    unresolved_subject = unresolved_ledger["pages"][0]["comb_subjects"][0]
    unresolved_cell = unresolved_ledger["pages"][0]["cells"][0]
    unresolved_subject.update({
        "state": "active_unresolved",
        "reason_codes": ["self-test-unresolved"],
        "blocks_gate": True,
    })
    unresolved_cell["comb"]["resolution"].update({
        "status": "unresolved",
        "reason_codes": ["self-test-unresolved"],
    })
    refresh_ledger_stats(unresolved_ledger)
    unresolved_result = validate_comb_ledger(
        "0605-1999", unresolved_ledger, lattice_producer_bytes)
    assert unresolved_result["counts"]["active_unresolved"] == 1
    assert unresolved_result["counts"]["blocking"] == 1

    inference_ledger = clone(ledger_fixture)
    inferred_cell = {
        "id": "p1c21",
        "subject_key": "p1@63.00,0.00,65.00,10.00",
        "x0": 63.0, "y0": 0.0, "x1": 65.0, "y1": 10.0,
    }
    inference_ledger["pages"][0]["cells"].append(inferred_cell)
    inference_ledger["pages"][0]["comb_inferences"].append({
        "subject_key": inferred_cell["subject_key"],
        "cell_id": inferred_cell["id"],
        "bbox": [63.0, 0.0, 65.0, 10.0],
        "state": COMB_INFERENCE_STATE,
        "reason_codes": ["no-legacy-subject"],
        "inferred_comb": synthetic_ledger_comb(
            63.0, 65.0, "unresolved", ["no-legacy-subject"]),
        "requires_independent_evidence": True,
        "permitted_transitions": ["active_reviewed"],
        "blocks_gate": True,
    })
    refresh_ledger_stats(inference_ledger)
    inference_result = validate_comb_ledger(
        "0605-1999", inference_ledger, lattice_producer_bytes)
    assert inference_result["counts"]["inferences_suppressed"] == 1
    assert inference_result["counts"]["blocking"] == 1

    retained_ledger = clone(ledger_fixture)
    retained_subject = retained_ledger["pages"][0]["comb_subjects"][0]
    retained_cell = retained_ledger["pages"][0]["cells"][0]
    retained_comb = retained_cell.pop("comb")
    retained_comb["resolution"].update({
        "status": "unresolved",
        "reason_codes": ["legacy-continuity-only"],
    })
    retained_subject.clear()
    retained_subject.update({
        "subject_key": retained_cell["subject_key"],
        "legacy_cell_id": retained_cell["id"],
        "legacy_bbox": [
            retained_cell["x0"], retained_cell["y0"],
            retained_cell["x1"], retained_cell["y1"],
        ],
        "cell_id": None,
        "mapped_partition_cell_ids": [retained_cell["id"]],
        "mapped_partition_subject_keys": [retained_cell["subject_key"]],
        "state": "retained_unresolved",
        "emission": "suppressed",
        "reason_codes": ["emission-suppressed-no-final-visible-band"],
        "legacy_comb": retained_comb,
        "requires_independent_evidence": True,
        "permitted_transitions": [
            "active_composite", "retired_proven_false",
        ],
        "blocks_gate": True,
    })
    refresh_ledger_stats(retained_ledger)
    retained_result = validate_comb_ledger(
        "0605-1999", retained_ledger, lattice_producer_bytes)
    assert retained_result["counts"]["retained_unresolved"] == 1
    retained_emission = {
        subject["cell_id"]: {"valid": True}
        for subject in retained_result["subjects"]
        if subject["cell_id"] is not None
    }
    retained_emission[retained_cell["id"]] = {"valid": True}
    retained_inventory = validate_emission_inventory(
        retained_result, retained_emission)
    assert not retained_inventory["complete"]
    assert retained_inventory["retained_emitted_cell_ids"] == [
        retained_cell["id"]]

    def unavailable_position(*, outer: bool) -> dict[str, Any]:
        axis = "outer" if outer else "internal"
        return {
            "comparable": False,
            "tolerance_pt": HTML_GEOMETRY_EPSILON_PT,
            f"actual_{axis}_edges_x": [1.0, 2.0],
            f"expected_{axis}_edges_x": None,
            "count_matches": None,
            "deltas_pt": None,
            "matches": None,
            "unavailable_reason": "self-test source topology is unavailable",
        }

    self_audit_layout_sha = "a" * 64

    def self_owner_certificate(cell_id: str) -> dict[str, Any]:
        return {
            "criterion": AUDIT_OWNER_CERTIFICATE_CRITERION,
            "valid": True,
            "layout_sha256": self_audit_layout_sha,
            "page": 1,
            "cell_id": cell_id,
            "legacy_cell_id": cell_id,
            "subject_key": "p1@0,0,2,1",
            "legacy_bbox": ["0", "0", "2", "1"],
            "bbox_number_format": "canonical-decimal-string-v1",
            "state": "active_resolved",
            "supplies_topology": False,
        }

    def self_owner_binding(cell_ids: Sequence[str]) -> dict[str, Any]:
        return {
            "layout_sha256": self_audit_layout_sha,
            "cells": {
                cell_id: self_owner_certificate(cell_id)
                for cell_id in cell_ids
            },
        }

    def self_invalid_owner(reason: str = "self-test invalid owner"
                           ) -> dict[str, Any]:
        return {
            "criterion": AUDIT_OWNER_CERTIFICATE_CRITERION,
            "valid": False,
            "reason": reason,
            "supplies_topology": False,
        }

    def source_unevaluable_offender(cell_id: str) -> dict[str, Any]:
        item: dict[str, Any] = {
            "cell": cell_id,
            "page": 1,
            "slots": 2,
            "latticed": 2,
            "printed": None,
            "printed_divider_x": [],
            "emission_state": "physical-slots",
            "physical_slots": 2,
            "declared_slots": 2,
            "emitted_occurrences": 1,
            "slot_indexes": [0, 1],
            "input_slot_indexes": [[0], [1]],
            "slot_geometry": [],
            "emission_container_binding": {
                "expected_page": 1,
                "emitted_id_page": 1,
                "emitted_dom_page": 1,
                "page_matches": True,
                "expected_rect": [0.0, 0.0, 2.0, 1.0],
                "actual_rect": [0.0, 0.0, 2.0, 1.0],
                "rect_deltas_pt": [0.0, 0.0, 0.0, 0.0],
                "rect_matches": True,
                "tolerance_pt": HTML_GEOMETRY_EPSILON_PT,
            },
            "source_owner_certificate": self_owner_certificate(cell_id),
            "layout_relation": "unevaluable",
            "emission_relation": "match",
            "failure_kinds": ["source-topology-unevaluable"],
            "why": "self-test source topology is unavailable",
        }
        for field, (_kind, outer) in AUDIT_POSITION_FIELDS.items():
            item[field] = unavailable_position(outer=outer)
        item["effective_emission_state"] = "physical-slots"
        return item

    def layout_mismatch_offender(cell_id: str) -> dict[str, Any]:
        item = source_unevaluable_offender(cell_id)
        item.update({
            "printed": 1,
            "layout_relation": "mismatch",
            "emission_relation": "mismatch-printed",
            "failure_kinds": [
                "layout-printed-mismatch", "emission-printed-mismatch"],
            "why": (
                "self-test layout has two slots but source prints one "
                "compartment"),
            "source_frame_geometry": {
                "left_rail": {"center_x": 0.0},
                "right_rail": {"center_x": 2.0},
            },
        })
        item["emission_layout_position"] = {
            "comparable": True,
            "tolerance_pt": HTML_GEOMETRY_EPSILON_PT,
            "actual_internal_edges_x": [1.0],
            "expected_internal_edges_x": [1.0],
            "count_matches": True,
            "deltas_pt": [0.0],
            "matches": True,
        }
        item["emission_layout_outer_position"] = {
            "comparable": True,
            "tolerance_pt": HTML_GEOMETRY_EPSILON_PT,
            "actual_outer_edges_x": [0.0, 2.0],
            "expected_outer_edges_x": [0.0, 2.0],
            "count_matches": True,
            "deltas_pt": [0.0, 0.0],
            "matches": True,
        }
        item["emission_source_position"] = {
            "comparable": False,
            "tolerance_pt": HTML_GEOMETRY_EPSILON_PT,
            "actual_internal_edges_x": [1.0],
            "expected_internal_edges_x": [],
            "count_matches": None,
            "deltas_pt": None,
            "matches": None,
            "unavailable_reason": "emitted/source slot counts differ",
        }
        item["emission_source_outer_position"] = {
            "comparable": False,
            "tolerance_pt": HTML_GEOMETRY_EPSILON_PT,
            "actual_outer_edges_x": [0.0, 2.0],
            "expected_outer_edges_x": [0.0, 2.0],
            "count_matches": None,
            "deltas_pt": None,
            "matches": None,
            "unavailable_reason": "emitted/source slot counts differ",
        }
        item["layout_source_outer_position"] = {
            "comparable": False,
            "tolerance_pt": HTML_GEOMETRY_EPSILON_PT,
            "actual_outer_edges_x": [0.0, 2.0],
            "expected_outer_edges_x": [0.0, 2.0],
            "count_matches": None,
            "deltas_pt": None,
            "matches": None,
            "unavailable_reason": "layout/source slot counts differ",
        }
        return item

    def noncomb_binding_offender(
            cell_id: str, failure_kind: str) -> dict[str, Any]:
        if failure_kind == "emitted-cell-binding-invalid":
            layout_relation = "cell-binding-invalid"
            emission_state = "cell-binding-invalid"
        elif failure_kind == "unowned-live-comb-markup":
            layout_relation = "not-owned"
            emission_state = "raw-live-comb-markup"
        else:
            raise AssertionError("unknown self-test non-comb failure kind")
        return {
            "cell": cell_id,
            "page": 1,
            "slots": None,
            "latticed": None,
            "printed": None,
            "printed_divider_x": [],
            "emission_state": emission_state,
            "physical_slots": None,
            "declared_slots": None,
            "emitted_occurrences": 1,
            "layout_relation": layout_relation,
            "emission_relation": "invalid",
            "failure_kinds": [failure_kind],
            "why": "self-test canonical-looking non-comb binding offender",
        }

    def comb_assertion(
            offenders: list[dict[str, Any]],
            *,
            expected_ids: list[str],
            emitted_ids: list[str] | None = None,
            ) -> dict[str, Any]:
        emitted = list(expected_ids if emitted_ids is None else emitted_ids)
        mismatch_count = sum(
            item.get("layout_relation") == "mismatch"
            for item in offenders)
        unevaluable_count = sum(
            item.get("layout_relation") in {
                "unevaluable", "duplicate-subject", "inventory-invalid"}
            for item in offenders)
        behind_count = sum(
            audit_offender_dimensions(item)[
                "dimensions"]["emission_behind"]
            for item in offenders)
        invalid_count = sum(
            audit_offender_dimensions(item)[
                "dimensions"]["emission_invalid"]
            for item in offenders)
        unexpected = sorted(set(emitted) - set(expected_ids))
        assertion = {
            "holds": not offenders,
            "reason": (
                "" if not offenders
                else f"{len(offenders)} self-test offender(s)"),
            "offenders": offenders,
            "combs_expected": len(expected_ids),
            "combs_checked": len(expected_ids),
            "expected_comb_ids": expected_ids,
            "checked_comb_ids": list(expected_ids),
            "emitted_comb_ids": sorted(emitted),
            "unexpected_emitted_comb_ids": unexpected,
            "duplicate_layout_comb_ids": [],
            "duplicate_emitted_cell_ids": [],
            "raw_live_comb_issues": 0,
            "emitted_cell_binding_issues": len(unexpected),
            "inventory_complete": not unexpected,
            "layout_mismatches": mismatch_count,
            "layout_unevaluable": unevaluable_count,
            "owner_certificates_valid": len(expected_ids),
            "owner_certificates_invalid": 0,
            "source_u_frame_evaluable": sum(
                item.get("cell") in expected_ids
                and item.get("printed") is not None
                and item.get("source_frame_geometry") is not None
                for item in offenders
            ),
            "source_certified_unframed_evaluable": 0,
            "emission_behind_layout": behind_count,
            "emission_invalid": invalid_count,
        }
        invalid_owner_ids = {
            item.get("cell") for item in offenders
            if item.get("cell") in expected_ids
            and isinstance(item.get("source_owner_certificate"), dict)
            and item["source_owner_certificate"].get("valid") is False
        }
        assertion["owner_certificates_invalid"] = len(invalid_owner_ids)
        assertion["owner_certificates_valid"] = (
            len(expected_ids) - len(invalid_owner_ids))
        checked_source_unevaluable = {
            item.get("cell") for item in offenders
            if item.get("cell") in expected_ids
            and item.get("layout_relation") in {
                "unevaluable", "duplicate-subject"}
        }
        assertion["source_certified_unframed_evaluable"] = (
            len(expected_ids)
            - len(checked_source_unevaluable)
            - assertion["source_u_frame_evaluable"]
        )
        if any(
                item.get("layout_relation") in {
                    "duplicate-subject", "inventory-invalid",
                    "registry-invalid",
                }
                for item in offenders):
            assertion["inventory_complete"] = False
        if offenders:
            assertion.update({
                "offender_count": len(offenders),
                "offenders_published": len(offenders),
                "offenders_omitted": 0,
                "offenders_complete": True,
            })
        return assertion

    held_assertion = comb_assertion([], expected_ids=["p1c1"])
    audit_pass = audit_evidence({
        "comb_slots_match_printed": True,
        "assertions": {"comb_slots_match_printed": held_assertion},
    }, self_owner_binding(["p1c1"]))
    assert audit_pass["assertion_valid"]
    assert not audit_pass["complete"] and audit_pass["offender_count"] == 0
    one_offender = source_unevaluable_offender("p1c1")
    broken_assertion = comb_assertion(
        [one_offender], expected_ids=["p1c1"])
    audit_broken = audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {"comb_slots_match_printed": broken_assertion},
    }, self_owner_binding(["p1c1"]))
    assert audit_broken["assertion_valid"]
    assert audit_broken["layout_unevaluable"] == 1
    independent_relations = comb_assertion(
        [one_offender, layout_mismatch_offender("p1c2")],
        expected_ids=["p1c1", "p1c2"],
    )
    independent_audit = audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {"comb_slots_match_printed": independent_relations},
    }, self_owner_binding(["p1c1", "p1c2"]))
    assert independent_audit["assertion_valid"]
    assert independent_audit["offender_count"] == 2
    assert independent_audit["layout_mismatches"] == 1
    assert independent_audit["layout_unevaluable"] == 1

    invalid_geometry = layout_mismatch_offender("p1c1")
    invalid_geometry.update({
        "emission_state": "invalid-slot-geometry",
        "effective_emission_state": "invalid-slot-geometry",
        "emission_relation": "invalid",
        "failure_kinds": [
            "layout-printed-mismatch", "invalid-emission"],
        "why": (
            "self-test source disagrees while physical emission geometry "
            "is independently invalid"),
    })
    invalid_geometry_assertion = comb_assertion(
        [invalid_geometry], expected_ids=["p1c1"])
    invalid_geometry_audit = audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {
            "comb_slots_match_printed": invalid_geometry_assertion},
    }, self_owner_binding(["p1c1"]))
    assert invalid_geometry_audit["assertion_valid"], invalid_geometry_audit
    false_invalid_count_relation = clone(invalid_geometry_assertion)
    false_invalid_count_relation["offenders"][0][
        "failure_kinds"].append("emission-printed-mismatch")
    assert not audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {
            "comb_slots_match_printed": false_invalid_count_relation},
    }, self_owner_binding(["p1c1"]))["assertion_valid"]

    # Valid owner certificates are exact identity-only records.  They bind to
    # the retained layout SHA, page/cell/subject/state and canonical Decimal
    # bbox.  No individual field may drift while the topology relation passes.
    for label, mutate in (
        ("layout-sha", lambda cert: cert.__setitem__(
            "layout_sha256", "b" * 64)),
        ("subject", lambda cert: cert.__setitem__(
            "subject_key", "p1@0,0,3,1")),
        ("noncanonical-bbox", lambda cert: cert.__setitem__(
            "legacy_bbox", ["0", "0", "2.0", "1"])),
        ("state", lambda cert: cert.__setitem__(
            "state", "active_unresolved")),
        ("topology-claim", lambda cert: cert.__setitem__(
            "supplies_topology", True)),
        ("extra-key", lambda cert: cert.__setitem__("extra", False)),
    ):
        mutated_assertion = clone(broken_assertion)
        mutate(mutated_assertion["offenders"][0][
            "source_owner_certificate"])
        mutated_audit = audit_evidence({
            "comb_slots_match_printed": False,
            "assertions": {
                "comb_slots_match_printed": mutated_assertion},
        }, self_owner_binding(["p1c1"]))
        assert not mutated_audit["assertion_valid"], (
            label, mutated_audit)

    nested_owner_assertion = clone(broken_assertion)
    nested_owner_assertion["offenders"][0]["source_topology_evidence"] = {
        "criterion": "unanimous-source-derived-topology-required",
        "owner_certificate": clone(
            nested_owner_assertion["offenders"][0][
                "source_owner_certificate"]),
    }
    nested_owner_audit = audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {
            "comb_slots_match_printed": nested_owner_assertion},
    }, self_owner_binding(["p1c1"]))
    assert nested_owner_audit["assertion_valid"], nested_owner_audit
    unequal_nested_assertion = clone(nested_owner_assertion)
    unequal_nested_assertion["offenders"][0][
        "source_topology_evidence"]["owner_certificate"][
            "layout_sha256"] = "b" * 64
    unequal_nested_audit = audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {
            "comb_slots_match_printed": unequal_nested_assertion},
    }, self_owner_binding(["p1c1"]))
    assert not unequal_nested_audit["assertion_valid"], unequal_nested_audit

    invalid_owner_offender = source_unevaluable_offender("p1c1")
    invalid_owner_offender["source_owner_certificate"] = self_invalid_owner()
    invalid_owner_offender["source_topology_evidence"] = {
        "criterion": AUDIT_OWNER_CERTIFICATE_CRITERION,
        "owner_certificate": clone(
            invalid_owner_offender["source_owner_certificate"]),
    }
    invalid_owner_assertion = comb_assertion(
        [invalid_owner_offender], expected_ids=["p1c1"])
    invalid_owner_audit = audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {
            "comb_slots_match_printed": invalid_owner_assertion},
    }, self_owner_binding(["p1c1"]))
    assert invalid_owner_audit["assertion_valid"], invalid_owner_audit
    assert invalid_owner_audit["owner_certificates_valid"] == 0
    assert invalid_owner_audit["owner_certificates_invalid"] == 1
    assert invalid_owner_audit["source_u_frame_evaluable"] == 0
    assert invalid_owner_audit[
        "source_certified_unframed_evaluable"] == 0
    invalid_owner_extra_topology = clone(invalid_owner_assertion)
    invalid_owner_extra_topology["offenders"][0][
        "source_topology_evidence"]["divider_x"] = [1.0]
    assert not audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {
            "comb_slots_match_printed": invalid_owner_extra_topology},
    }, self_owner_binding(["p1c1"]))["assertion_valid"]

    invalid_owner_with_topology = layout_mismatch_offender("p1c1")
    invalid_owner_with_topology["source_owner_certificate"] = (
        self_invalid_owner())
    invalid_owner_with_topology["source_topology_evidence"] = {
        "criterion": AUDIT_OWNER_CERTIFICATE_CRITERION,
        "owner_certificate": clone(
            invalid_owner_with_topology["source_owner_certificate"]),
    }
    try:
        audit_offender_dimensions(
            invalid_owner_with_topology,
            self_owner_certificate("p1c1"))
    except RefereeError:
        pass
    else:
        raise AssertionError(
            "invalid owner certificate supplied a measured topology")

    duplicate_subject_offender = {
        "cell": "p1c1",
        "page": 1,
        "slots": 2,
        "latticed": None,
        "printed": None,
        "printed_divider_x": [],
        "emission_state": "physical-slots",
        "physical_slots": 2,
        "declared_slots": 2,
        "emitted_occurrences": 1,
        "source_owner_certificate": self_invalid_owner(
            "self-test duplicate layout owner"),
        "layout_relation": "duplicate-subject",
        "emission_relation": "unbound",
        "failure_kinds": ["duplicate-layout-subject"],
        "why": "self-test layout has two subjects with this id",
    }
    duplicate_subject_assertion = comb_assertion(
        [duplicate_subject_offender], expected_ids=["p1c1"])
    duplicate_subject_assertion["duplicate_layout_comb_ids"] = ["p1c1"]
    duplicate_subject_audit = audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {
            "comb_slots_match_printed": duplicate_subject_assertion},
    }, self_owner_binding(["p1c1"]))
    assert duplicate_subject_audit["assertion_valid"], (
        duplicate_subject_audit)
    assert duplicate_subject_audit["owner_certificates_invalid"] == 1
    assert duplicate_subject_audit["owner_certificates_valid"] == 0
    for invented_topology in (
        {
            "criterion": "invented-duplicate-subject-topology",
            "divider_x": [1.0],
        },
        {
            "printed_compartments": 2,
            "owner_certificate": clone(
                duplicate_subject_offender[
                    "source_owner_certificate"]),
        },
    ):
        invented_duplicate = clone(duplicate_subject_assertion)
        invented_duplicate["offenders"][0][
            "source_topology_evidence"] = invented_topology
        invented_duplicate_audit = audit_evidence({
            "comb_slots_match_printed": False,
            "assertions": {
                "comb_slots_match_printed": invented_duplicate},
        }, self_owner_binding(["p1c1"]))
        assert not invented_duplicate_audit["assertion_valid"], (
            invented_duplicate_audit)

    registry_offender = {
        "cell": "<comb-owner-registry>",
        "page": None,
        "slots": None,
        "latticed": None,
        "printed": None,
        "printed_divider_x": [],
        "emission_state": "not-evaluated",
        "effective_emission_state": "not-evaluated",
        "physical_slots": None,
        "declared_slots": None,
        "emitted_occurrences": 0,
        "source_owner_certificate": self_invalid_owner(
            "self-test global owner registry failure"),
        "layout_relation": "registry-invalid",
        "emission_relation": "not-evaluated",
        "failure_kinds": ["comb-owner-registry-invalid"],
        "why": "self-test global owner registry failure",
    }
    registry_assertion = comb_assertion(
        [registry_offender, invalid_owner_offender],
        expected_ids=["p1c1"],
    )
    registry_audit = audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {"comb_slots_match_printed": registry_assertion},
    }, self_owner_binding(["p1c1"]))
    assert registry_audit["assertion_valid"], registry_audit
    assert registry_audit["owner_certificates_invalid"] == 1
    assert registry_audit["owner_certificates_valid"] == 0
    assert registry_audit["combs_checked"] == 1
    assert registry_audit["offender_dimensions"][
        "<comb-owner-registry>"]["source_owner_certificate"][
            "valid"] is False

    audit_truncated_record = {
        "comb_slots_match_printed": False,
        "assertions": {"comb_slots_match_printed": clone(broken_assertion)},
    }
    audit_truncated_record["assertions"][
        "comb_slots_match_printed"]["offender_count"] = 2
    audit_truncated_record["assertions"][
        "comb_slots_match_printed"]["offenders_omitted"] = 1
    audit_truncated_record["assertions"][
        "comb_slots_match_printed"]["offenders_complete"] = False
    assert not audit_evidence(audit_truncated_record)["assertion_valid"]

    duplicate_offender_record = {
        "comb_slots_match_printed": False,
        "assertions": {"comb_slots_match_printed": comb_assertion(
            [one_offender, clone(one_offender)],
            expected_ids=["p1c1"],
        )},
    }
    assert not audit_evidence(
        duplicate_offender_record)["assertion_valid"]

    malformed_relation = clone(one_offender)
    malformed_relation["failure_kinds"] = ["layout-printed-mismatch"]
    malformed_relation["layout_relation"] = "mismatch"
    malformed_assertion = clone(broken_assertion)
    malformed_assertion["offenders"] = [malformed_relation]
    assert not audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {"comb_slots_match_printed": malformed_assertion},
    })["assertion_valid"]

    bogus_failure = clone(one_offender)
    bogus_failure["failure_kinds"].append("invented-self-test-failure")
    bogus_assertion = clone(broken_assertion)
    bogus_assertion["offenders"] = [bogus_failure]
    assert not audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {"comb_slots_match_printed": bogus_assertion},
    })["assertion_valid"]

    false_position = clone(one_offender)
    false_position["emission_layout_position"] = {
        "comparable": True,
        "tolerance_pt": HTML_GEOMETRY_EPSILON_PT,
        "actual_internal_edges_x": [1.0],
        "expected_internal_edges_x": [2.0],
        "count_matches": True,
        "deltas_pt": [-1.0],
        "matches": False,
    }
    false_position_assertion = clone(broken_assertion)
    false_position_assertion["offenders"] = [false_position]
    assert not audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {
            "comb_slots_match_printed": false_position_assertion},
    })["assertion_valid"]

    missing_without_offender = comb_assertion(
        [], expected_ids=["p1c1"], emitted_ids=[])
    assert not audit_evidence({
        "comb_slots_match_printed": True,
        "assertions": {
            "comb_slots_match_printed": missing_without_offender},
    })["assertion_valid"]

    active_order = [
        subject["cell_id"] for subject in ledger_result["subjects"]]
    active_slots = {
        cell_id: {"valid": True} for cell_id in active_order}
    active_inventory = validate_emission_inventory(
        ledger_result, active_slots)
    bound_assertion = {
        "expected_comb_ids": active_order,
        "checked_comb_ids": active_order,
        "emitted_comb_ids": sorted(active_slots),
        "unexpected_emitted_comb_ids": [],
        "offenders": {},
    }
    assert bind_audit_assertion(
        bound_assertion, ledger_result, active_slots,
        active_inventory)["binding_valid"]
    permuted_assertion = clone(bound_assertion)
    permuted_assertion["expected_comb_ids"] = list(reversed(active_order))
    permuted_assertion["checked_comb_ids"] = list(reversed(active_order))
    assert bind_audit_assertion(
        permuted_assertion, ledger_result, active_slots,
        active_inventory)["binding_valid"]
    duplicate_assertion = clone(bound_assertion)
    duplicate_assertion["expected_comb_ids"].append(active_order[0])
    duplicate_assertion["checked_comb_ids"].append(active_order[0])
    assert not bind_audit_assertion(
        duplicate_assertion, ledger_result, active_slots,
        active_inventory)["binding_valid"]
    missing_assertion = clone(bound_assertion)
    missing_assertion["expected_comb_ids"] = active_order[:-1]
    missing_assertion["checked_comb_ids"] = active_order[:-1]
    assert not bind_audit_assertion(
        missing_assertion, ledger_result, active_slots,
        active_inventory)["binding_valid"]
    extra_assertion = clone(bound_assertion)
    extra_assertion["expected_comb_ids"].append("p1c999")
    extra_assertion["checked_comb_ids"].append("p1c999")
    assert not bind_audit_assertion(
        extra_assertion, ledger_result, active_slots,
        active_inventory)["binding_valid"]
    checked_mismatch = clone(bound_assertion)
    checked_mismatch["checked_comb_ids"] = list(reversed(active_order))
    assert not bind_audit_assertion(
        checked_mismatch, ledger_result, active_slots,
        active_inventory)["binding_valid"]

    noncomb_offenders = [
        noncomb_binding_offender(
            "p1c998", "emitted-cell-binding-invalid"),
        noncomb_binding_offender(
            "p1c999", "unowned-live-comb-markup"),
    ]
    noncomb_assertion = comb_assertion(
        noncomb_offenders, expected_ids=active_order)
    noncomb_assertion["raw_live_comb_issues"] = 1
    noncomb_assertion["emitted_cell_binding_issues"] = 1
    noncomb_assertion["inventory_complete"] = False
    noncomb_audit = audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {"comb_slots_match_printed": noncomb_assertion},
    })
    assert noncomb_audit["assertion_valid"], noncomb_audit["errors"]
    assert bind_audit_assertion(
        noncomb_audit, ledger_result, active_slots,
        active_inventory)["binding_valid"]

    mixed_binding = source_unevaluable_offender("p1c996")
    mixed_binding["failure_kinds"].append(
        "emitted-cell-binding-invalid")
    mixed_unowned = source_unevaluable_offender("p1c997")
    mixed_unowned["failure_kinds"].append(
        "unowned-live-comb-markup")
    mixed_assertion = comb_assertion(
        [mixed_binding, mixed_unowned], expected_ids=active_order)
    mixed_assertion["raw_live_comb_issues"] = 1
    mixed_assertion["emitted_cell_binding_issues"] = 1
    mixed_assertion["inventory_complete"] = False
    mixed_audit = audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {"comb_slots_match_printed": mixed_assertion},
    })
    assert mixed_audit["assertion_valid"], mixed_audit["errors"]
    mixed_binding_result = bind_audit_assertion(
        mixed_audit, ledger_result, active_slots, active_inventory)
    assert not mixed_binding_result["binding_valid"]
    assert all(
        cell_id in mixed_binding_result["reason"]
        for cell_id in ("p1c996", "p1c997"))

    unknown_assertion = comb_assertion(
        [source_unevaluable_offender("p1c995")],
        expected_ids=active_order)
    unknown_audit = audit_evidence({
        "comb_slots_match_printed": False,
        "assertions": {"comb_slots_match_printed": unknown_assertion},
    })
    assert not unknown_audit["assertion_valid"]
    assert not bind_audit_assertion(
        unknown_audit, ledger_result, active_slots,
        active_inventory)["binding_valid"]

    roundtrip_fixture = {
        "status": "ok",
        "measured": True,
        "hard_failure": None,
        "error": None,
        "roundtrip_runtime": {
            "mode": "playwright-exact-executable",
            "playwright_package_version": "self-test",
            "dependency_closure": {
                "logical_root": "playwright",
                "algorithm": (
                    "sha256(canonical-json(path,type,bytes,digest))"),
                "files": 1,
                "symlinks": 0,
                "bytes": 1,
                "tree_sha256": "1" * 64,
            },
            "chromium": {
                "file": "playwright/chromium",
                "bytes": 1,
                "sha256": "2" * 64,
                "version_output": "Chrome self-test",
            },
            "same_resolution_session_used_for_render": True,
            "dependency_closure_validated_before_after": True,
            "system_shared_libraries_bound": False,
            "native_host_environment_bound": False,
            "scope": AUDIT_ROUNDTRIP_SCOPE,
            "scope_complete": False,
            "incomplete_reason": "self-test native scope is incomplete",
            "live_browser_version": "self-test",
            "explicit_executable_path_used": True,
            "launch_args": list(AUDIT_ROUNDTRIP_LAUNCH_ARGS),
            "service_workers": "block",
            "browser_context_offline": True,
            "websocket_policy": "record-and-leave-unconnected",
            "request_policy": "formgen-snapshot-only-v1",
            "playwright_operation_timeout_ms": 120000,
            "hard_deadline_seconds": 60.0,
            "hard_deadline_enforced_by": (
                "isolated-render-worker-process-v1"),
            "deadline_cleanup_policy": (
                "kill-worker-and-chromium-process-group"),
        },
        "render_requests": {
            "policy": "formgen-snapshot-only-v1",
            "synthetic_origin": "https://formgen.invalid",
            "fulfilled": ["asset.png", "x.html"],
            "fulfilled_requests": 2,
            "blocked": [],
            "blocked_requests": 0,
            "blocked_websockets": [],
            "all_requests_from_retained_closure": True,
        },
        "candidate_pdf": {
            "bytes": 1,
            "sha256": "3" * 64,
            "retained_exact_bytes": True,
            "chromium_returned_in_memory": True,
            "normalization": {
                "algorithm": "fixed-width-creation-modification-date-v1",
                "fields_normalized": 2,
                "replacement": AUDIT_PDF_NORMALIZATION_REPLACEMENT,
                "xref_offsets_preserved": True,
            },
            "materialization": AUDIT_CANDIDATE_MATERIALIZATION,
            "expected_sha256_passed_to_extractor": True,
            "validated_before_after_extraction": True,
            "candidate_ir_sha256": "4" * 64,
            "candidate_ir_digest_scope": "source-and-generator-removed",
        },
    }
    roundtrip_scope, roundtrip_errors = validate_audit_roundtrip(
        roundtrip_fixture, "x.html", ["asset.png"])
    assert roundtrip_scope is False and not roundtrip_errors

    blocked_http_roundtrip = clone(roundtrip_fixture)
    blocked_http_roundtrip["render_requests"].update({
        "blocked": [{
            "url": "https://outside.invalid/data",
            "reason": "absent from retained closure",
        }],
        "blocked_requests": 1,
        # The producer aggregate is deliberately left forged true.
        "all_requests_from_retained_closure": True,
    })
    assert validate_audit_roundtrip(
        blocked_http_roundtrip, "x.html", ["asset.png"])[1]

    bad_request_count_roundtrip = clone(roundtrip_fixture)
    bad_request_count_roundtrip[
        "render_requests"]["fulfilled_requests"] = 3
    assert validate_audit_roundtrip(
        bad_request_count_roundtrip, "x.html", ["asset.png"])[1]

    bad_blocked_count_roundtrip = clone(roundtrip_fixture)
    bad_blocked_count_roundtrip[
        "render_requests"]["blocked_requests"] = 1
    assert validate_audit_roundtrip(
        bad_blocked_count_roundtrip, "x.html", ["asset.png"])[1]

    blocked_websocket_roundtrip = clone(roundtrip_fixture)
    blocked_websocket_roundtrip["render_requests"].update({
        "blocked_websockets": ["wss://outside.invalid/socket"],
        "all_requests_from_retained_closure": True,
    })
    assert validate_audit_roundtrip(
        blocked_websocket_roundtrip, "x.html", ["asset.png"])[1]

    unknown_request_roundtrip = clone(roundtrip_fixture)
    unknown_request_roundtrip["render_requests"].update({
        "fulfilled": ["asset.png", "unknown.png", "x.html"],
        "fulfilled_requests": 3,
        "all_requests_from_retained_closure": True,
    })
    assert validate_audit_roundtrip(
        unknown_request_roundtrip, "x.html", ["asset.png"])[1]

    boolean_count_roundtrip = clone(roundtrip_fixture)
    boolean_count_roundtrip[
        "render_requests"]["fulfilled_requests"] = True
    assert validate_audit_roundtrip(
        boolean_count_roundtrip, "x.html", ["asset.png"])[1]

    malformed_request_list_roundtrip = clone(roundtrip_fixture)
    malformed_request_list_roundtrip[
        "render_requests"]["fulfilled"] = ["asset.png", 7, "x.html"]
    assert validate_audit_roundtrip(
        malformed_request_list_roundtrip, "x.html", ["asset.png"])[1]

    reordered_launch_args_roundtrip = clone(roundtrip_fixture)
    reordered_launch_args_roundtrip[
        "roundtrip_runtime"]["launch_args"].reverse()
    assert validate_audit_roundtrip(
        reordered_launch_args_roundtrip, "x.html", ["asset.png"])[1]

    wrong_scope_roundtrip = clone(roundtrip_fixture)
    wrong_scope_roundtrip[
        "roundtrip_runtime"]["scope"] = "playwright-only"
    assert validate_audit_roundtrip(
        wrong_scope_roundtrip, "x.html", ["asset.png"])[1]

    wrong_materialization_roundtrip = clone(roundtrip_fixture)
    wrong_materialization_roundtrip[
        "candidate_pdf"]["materialization"] = "ordinary-temp-file"
    assert validate_audit_roundtrip(
        wrong_materialization_roundtrip, "x.html", ["asset.png"])[1]

    wrong_normalization_roundtrip = clone(roundtrip_fixture)
    wrong_normalization_roundtrip["candidate_pdf"][
        "normalization"]["replacement"] = "D:20000101000000+00'00'"
    assert validate_audit_roundtrip(
        wrong_normalization_roundtrip, "x.html", ["asset.png"])[1]

    with tempfile.TemporaryDirectory(prefix="comb-referee-audit-bind-") as temp:
        root = pathlib.Path(temp)
        html_dir = root / "html"
        source_root = root / "source"
        html_dir.mkdir()
        source_root.mkdir()
        payloads = {
            "ir": b'{"self_test":"ir"}',
            "layout": b'{"self_test":"layout"}',
            "html": b"<!doctype html><html></html>",
            "guide": b'{"self_test":"guide"}',
            "guide_html": None,
        }
        paths = {
            "ir": root / "x.ir.json",
            "layout": root / "x.layout.json",
            "html": html_dir / "x.html",
            "guide": root / "x.guide.json",
            "guide_html": html_dir / "x.guide.html",
        }
        for role, payload in payloads.items():
            if payload is not None:
                paths[role].write_bytes(payload)
        source_payload = b"%PDF-self-test"
        source_path = source_root / "test.pdf"
        source_path.write_bytes(source_payload)
        expected = {
            role: (paths[role], role != "guide_html", payload)
            for role, payload in payloads.items()
        }
        audit_producer_bytes = (HERE / "audit.py").read_bytes()
        self_test_audit_sha = sha256_bytes(audit_producer_bytes)
        dependency_sources = {
            logical: (REPO / logical).read_bytes()
            for logical in AUDIT_DEPENDENCY_SHA256
        }
        producer_sources = {
            AUDIT_PRODUCER_FILE: audit_producer_bytes,
            **dependency_sources,
        }
        python_path = pathlib.Path(sys.executable).resolve()
        runtime_members = [(
            "python/executable",
            python_path.stat().st_size,
            sha256_file(python_path),
        )]
        runtime_canonical = json.dumps(
            runtime_members, separators=(",", ":"))
        runtime = {
            "python": {
                "implementation": platform.python_implementation(),
                "version": platform.python_version(),
                "cache_tag": sys.implementation.cache_tag,
            },
            "pymupdf": {
                "package_version": "self-test",
                "version_bind": "self-test",
            },
            "loaded_application_files": {
                "algorithm": (
                    "sha256(canonical-json(logical-file,bytes,sha256))"),
                "files": 1,
                "bytes": runtime_members[0][1],
                "tree_sha256": sha256_bytes(
                    runtime_canonical.encode("ascii")),
                "members": [{
                    "file": runtime_members[0][0],
                    "bytes": runtime_members[0][1],
                    "sha256": runtime_members[0][2],
                }],
                "validated_before_after": True,
            },
            "stdlib_and_system_shared_libraries_bound": False,
            "scope_complete": False,
            "incomplete_reason": "self-test intentionally incomplete scope",
        }
        producer = {
            "file": AUDIT_PRODUCER_FILE,
            "bytes": len(audit_producer_bytes),
            "sha256": self_test_audit_sha,
            "dependencies": [
                {
                    "file": logical,
                    "bytes": len(dependency_sources[logical]),
                    "sha256": expected_sha,
                    "loaded_origin": logical,
                    "executed_from_snapshotted_source": True,
                }
                for logical, expected_sha
                in AUDIT_DEPENDENCY_SHA256.items()
            ],
            "dependency_execution_bound": True,
            "audit_execution_bound": False,
            "assertion_producer_bound": False,
            "roundtrip_runtime_bound_in_record": False,
            "standalone_attestation_complete": False,
            "incomplete_reason": "self-test bootstrap is intentionally open",
        }
        input_entries = {
            role: {
                "file": paths[role].name,
                "required": role != "guide_html",
                "present": payload is not None,
                "bytes": len(payload) if payload is not None else None,
                "sha256": (
                    sha256_bytes(payload) if payload is not None else None),
            }
            for role, payload in payloads.items()
        }
        input_entries["source_pdf"] = {
            "file": "test.pdf",
            "logical_identity": "external:test.pdf",
            "path": "test.pdf",
            "required": True,
            "present": True,
            "bytes": len(source_payload),
            "sha256": sha256_bytes(source_payload),
            "expected_sha256": sha256_bytes(source_payload),
        }
        audit_record = {
            "roundtrip": "skipped",
            "provenance_validation": {
                "validated_before": True,
                "validated_after": True,
                "error": None,
            },
            "attestation": {
                "inputs_complete": True,
                "producer_execution_bound": False,
                "base_runtime_scope_complete": False,
                "roundtrip_runtime_scope_complete": None,
                "validated_before_after": True,
                "complete": False,
                "enforceable": False,
                "incomplete_reasons": [
                    "self-test producer/runtime scope is intentionally open"],
                "future_gate_required": "self-test trusted gate",
            },
            "input_manifest": {
                "schema": "formgen-audit-input-manifest-v1",
                "algorithm": "sha256",
                "producer": producer,
                "runtime": runtime,
                "inputs_complete": True,
                "attestation_complete": False,
                "enforceable": False,
                "complete": False,
                "missing_required": [],
                "inputs": input_entries,
                "render": {
                    "entrypoint": "x.html",
                    "dependencies": [],
                    "errors": [],
                    "complete": True,
                    "network_policy": (
                        "deny-except-retained-relative-resources-and-inline-data"),
                },
            },
        }
        assert self_test_audit_sha == AUDIT_PRODUCER_SHA256
        binding = bind_audit_manifest(
            audit_record,
            expected,
            source_path=source_path,
            source_identity="external:test.pdf",
            source_root=source_root,
            source_payload=source_payload,
            expected_source_sha256=sha256_bytes(source_payload),
            html_dir=html_dir,
            producer_sources=producer_sources,
        )
        assert binding["binding_valid"] and not binding["complete"], binding
        assert binding["blockers"]
        stale_expected = {
            **expected,
            "ir": (paths["ir"], True, b"changed"),
        }
        assert not bind_audit_manifest(
            audit_record,
            stale_expected,
            source_path=source_path,
            source_identity="external:test.pdf",
            source_root=source_root,
            source_payload=source_payload,
            expected_source_sha256=sha256_bytes(source_payload),
            html_dir=html_dir,
            producer_sources=producer_sources,
        )["binding_valid"]
        stale_manifest = clone(audit_record)
        stale_manifest["input_manifest"]["producer"]["sha256"] = "0" * 64
        assert not bind_audit_manifest(
            stale_manifest,
            expected,
            source_path=source_path,
            source_identity="external:test.pdf",
            source_root=source_root,
            source_payload=source_payload,
            expected_source_sha256=sha256_bytes(source_payload),
            html_dir=html_dir,
            producer_sources=producer_sources,
        )["binding_valid"]
        stale_render = clone(audit_record)
        stale_render["input_manifest"]["render"]["dependencies"] = [{
            "path": "invented-self-test.bin",
            "mime_type": "application/octet-stream",
            "present": True,
            "bytes": 1,
            "sha256": "0" * 64,
            "kinds": ["img"],
            "referrers": ["x.html"],
        }]
        assert not bind_audit_manifest(
            stale_render,
            expected,
            source_path=source_path,
            source_identity="external:test.pdf",
            source_root=source_root,
            source_payload=source_payload,
            expected_source_sha256=sha256_bytes(source_payload),
            html_dir=html_dir,
            producer_sources=producer_sources,
        )["binding_valid"]
        overclaimed = clone(audit_record)
        overclaimed["input_manifest"]["complete"] = True
        assert not bind_audit_manifest(
            overclaimed,
            expected,
            source_path=source_path,
            source_identity="external:test.pdf",
            source_root=source_root,
            source_payload=source_payload,
            expected_source_sha256=sha256_bytes(source_payload),
            html_dir=html_dir,
            producer_sources=producer_sources,
        )["binding_valid"]

    assert audit_relation_for_subject(
        ledger_result["subjects"][0], True, None
    ) == (2, "complete-non-offender")
    assert audit_relation_for_subject(
        unresolved_result["subjects"][0], True, None
    ) == (2, "complete-non-offender")
    assert audit_relation_for_subject(
        retained_result["subjects"][0], True, None
    ) == (None, "complete-blocked-subject")

    unresolved_compared = {
        "ledger_state": "active_unresolved",
        "ledger_blocks_gate": True,
        "latticed": 3,
        "emitted": 3,
        "emitted_indexes_valid": True,
        "audit_printed": 3,
        "referee": {
            "status": "measured",
            "compartments": 3,
            "positions_match": True,
        },
    }
    comparison_cases = [
        ("agree", True, {}, {}),
        ("repair-lattice", True, {"audit_printed": 4},
         {"compartments": 4}),
        ("repair-audit", True, {"audit_printed": 4},
         {"compartments": 3}),
        ("stop", True, {"audit_printed": 3}, {"compartments": 5}),
        ("stale-generation", True, {"emitted": 2}, {}),
        ("unevaluable", False, {}, {}),
    ]
    for expected_status, audit_complete, updates, referee_updates in (
            comparison_cases):
        compared = clone(unresolved_compared)
        compared.update(updates)
        compared["referee"].update(referee_updates)
        before = clone(compared)
        status, _reason = comparison(compared, audit_complete)
        assert status == expected_status, (expected_status, status)
        transition_status, _transition_reason = transition_decision(
            compared, status)
        assert transition_status == (
            "eligible-for-reviewed-resolution"
            if status == "agree" else "blocked"
        )
        assert compared == before
        assert compared["ledger_state"] == "active_unresolved"
        assert compared["ledger_blocks_gate"] is True

    resolved_compared = clone(unresolved_compared)
    resolved_compared.update({
        "ledger_state": "active_resolved",
        "ledger_blocks_gate": False,
    })
    resolved_status, _ = comparison(resolved_compared, True)
    assert resolved_status == "agree"
    assert transition_decision(
        resolved_compared, resolved_status)[0] == "none"

    retained_compared = clone(unresolved_compared)
    retained_compared["ledger_state"] = "retained_unresolved"
    retained_status, _ = comparison(retained_compared, True)
    assert retained_status == "unevaluable"
    assert transition_decision(
        retained_compared, retained_status)[0] == (
            "explicit-transition-required")

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
    digest_report = {"schema_version": REPORT_VERSION, "forms": [], "status": "ok"}
    attach_report_digest(digest_report)
    assert report_digest_valid(digest_report)
    assert report_bytes(digest_report) == report_bytes(clone(digest_report))
    changed_digest_report = clone(digest_report)
    changed_digest_report["status"] = "unevaluable"
    assert not report_digest_valid(changed_digest_report)
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
    standalone_attestation = referee_attestation()
    assert standalone_attestation["scope_complete"] is False
    assert standalone_attestation["complete"] is False
    assert standalone_attestation["enforceable"] is False
    assert standalone_attestation["incomplete_reasons"]
    assert standalone_attestation[
        "poppler_invocations_have_hard_deadlines"] is True

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


def referee_attestation() -> dict[str, Any]:
    """State the exact boundary this standalone process does not attest."""
    return {
        "schema": "comb-referee-runtime-attestation-v1",
        "producer_and_declared_dependency_bytes_bound": True,
        "published_form_input_bytes_bound_before_after": True,
        "python_executable_fingerprinted": True,
        "python_executable_validated_before_after": False,
        "poppler_executable_bound_before_after": True,
        "poppler_invocations_have_hard_deadlines": True,
        "poppler_timeout_cleanup_policy": SUBPROCESS_CLEANUP_POLICY,
        "clean_source_revision_bound": False,
        "python_stdlib_closure_bound": False,
        "python_dynamic_libraries_bound": False,
        "poppler_dynamic_libraries_bound": False,
        "operating_system_and_host_services_bound": False,
        "scope_complete": False,
        "complete": False,
        "enforceable": False,
        "incomplete_reasons": [
            (
                "the standalone referee hashes its source and declared local "
                "dependencies but is not bound to a reviewed clean source "
                "revision"
            ),
            (
                "the Python standard library, Python dynamic libraries, "
                "Poppler dynamic libraries, and operating-system services "
                "are outside the independently rehashed application closure"
            ),
            (
                "the Python executable is fingerprinted for reporting but "
                "is not independently snapshotted and revalidated before "
                "and after the run"
            ),
        ],
        "future_gate_required": (
            "trusted clean-source and host/runtime closure binding"),
    }


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
        lattice_producer_bytes = (HERE / "lattice.py").read_bytes()
        if sha256_bytes(lattice_producer_bytes) != LATTICE_PRODUCER_SHA256:
            raise RefereeError(
                "lattice producer changed from committed SHA "
                + LATTICE_PRODUCER_SHA256)
        audit_producer_bytes = (HERE / "audit.py").read_bytes()
        if sha256_bytes(audit_producer_bytes) != AUDIT_PRODUCER_SHA256:
            raise RefereeError(
                "audit producer changed from committed SHA "
                + AUDIT_PRODUCER_SHA256)
        audit_dependency_bytes = {
            logical: (REPO / logical).read_bytes()
            for logical in AUDIT_DEPENDENCY_SHA256
        }
        for logical, expected_sha in AUDIT_DEPENDENCY_SHA256.items():
            if sha256_bytes(audit_dependency_bytes[logical]) != expected_sha:
                raise RefereeError(
                    f"audit dependency changed from committed SHA: {logical}")
        audit_bytes = args.audit.read_bytes()
        args.lattice_producer_bytes = lattice_producer_bytes
        args.audit_producer_bytes = audit_producer_bytes
        args.audit_dependency_bytes = audit_dependency_bytes
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
                or (HERE / "audit.py").read_bytes() != audit_producer_bytes
                or (HERE / "lattice.py").read_bytes()
                != lattice_producer_bytes
                or any(
                    (REPO / logical).read_bytes() != payload
                    for logical, payload in audit_dependency_bytes.items())):
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
        source_unevaluable = sum(
            form["counts"]["source_unevaluable"] for form in forms)
        active = sum(
            form["counts"]["subjects_active"] for form in forms)
        active_resolved = sum(
            form["counts"]["subjects_active_resolved"] for form in forms)
        active_unresolved = sum(
            form["counts"]["subjects_active_unresolved"] for form in forms)
        retained_unresolved = sum(
            form["counts"]["subjects_retained_unresolved"] for form in forms)
        inferences_suppressed = sum(
            form["counts"]["inferences_suppressed"] for form in forms)
        ledger_blocking = sum(
            form["counts"]["ledger_blocking"] for form in forms)
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
        status_reasons: list[str] = []
        if (not coverage_ok
                or any(form["status"] == "unevaluable" for form in forms)):
            corpus_status = "unevaluable"
            status_reasons.append(
                "corpus coverage or one or more forms are unevaluable")
        elif any(form["status"] == "disagreement" for form in forms):
            corpus_status = "disagreement"
            status_reasons.append(
                "one or more four-way form comparisons disagree")
        else:
            corpus_status = "ok"
        runtime_attestation = referee_attestation()
        if not runtime_attestation["complete"]:
            corpus_status = "unevaluable"
            status_reasons.append(
                "standalone referee runtime/application attestation "
                "is incomplete and non-enforceable")
        python_binary = pathlib.Path(sys.executable).resolve()
        report: dict[str, Any] = {
            "schema_version": REPORT_VERSION,
            "producer": "tools/formgen/comb_referee.py",
            "producer_sha256": sha256_bytes(producer_bytes),
            "python_version": sys.version.split()[0],
            "provenance": {
                "producer": {
                    "file": "tools/formgen/comb_referee.py",
                    "bytes": len(producer_bytes),
                    "sha256": sha256_bytes(producer_bytes),
                },
                "dependencies": {
                    "audit": {
                        "file": AUDIT_PRODUCER_FILE,
                        "bytes": len(audit_producer_bytes),
                        "sha256": sha256_bytes(audit_producer_bytes),
                        "expected_sha256": AUDIT_PRODUCER_SHA256,
                        "dependencies": [
                            {
                                "file": logical,
                                "bytes": len(audit_dependency_bytes[logical]),
                                "sha256": sha256_bytes(
                                    audit_dependency_bytes[logical]),
                                "expected_sha256": expected_sha,
                            }
                            for logical, expected_sha
                            in AUDIT_DEPENDENCY_SHA256.items()
                        ],
                    },
                    "lattice": {
                        "file": LATTICE_PRODUCER_FILE,
                        "bytes": len(lattice_producer_bytes),
                        "sha256": sha256_bytes(lattice_producer_bytes),
                        "expected_sha256": LATTICE_PRODUCER_SHA256,
                    },
                },
                "runtime": {
                    "python_implementation": sys.implementation.name,
                    "python_version": sys.version.split()[0],
                    "python_executable": str(python_binary),
                    "python_executable_sha256": sha256_file(python_binary),
                    "poppler": poppler,
                },
            },
            "status": corpus_status,
            "status_reasons": status_reasons,
            "attestation": runtime_attestation,
            "poppler": poppler,
            "inputs": {
                "audit_sha256": sha256_bytes(audit_bytes),
                "audit_bytes": len(audit_bytes),
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
                "combs_source_unevaluable": source_unevaluable,
                "subjects_active": active,
                "subjects_active_resolved": active_resolved,
                "subjects_active_unresolved": active_unresolved,
                "subjects_retained_unresolved": retained_unresolved,
                "inferences_suppressed": inferences_suppressed,
                "ledger_blocking": ledger_blocking,
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
                "referee_attestation_complete": (
                    runtime_attestation["complete"]),
                "referee_enforceable": runtime_attestation["enforceable"],
            },
            "errors": errors,
            "forms": sorted(forms, key=lambda item: item["slug"]),
        }
        attach_report_digest(report)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_bytes(report_bytes(report))
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
