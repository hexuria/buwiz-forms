#!/usr/bin/env python3
"""Round-trip every generated form, score it, and assert what scoring cannot see.

Two independent halves, deliberately:

**The round trip** prints each generated bundle to PDF with Chromium, re-extracts
it with the same extractor, and diffs against the source IR. Scoring is blunt --
rules and text runs recovered, as percentages -- and answers "did the browser
reproduce the geometry we told it to".

**The assertions** answer the question the round trip cannot ask. A round trip
compares our output against our own input, so anything wrong in the input is
reproduced faithfully and scores 100%. That is how this audit reported
`rules 100% on 51/51` while 137 real defects were present: a black rectangle over
a header, a seal printed upside-down, statutory tax brackets a taxpayer could
type over, money grids with no input fields at all. Every one of those survives a
perfect round trip.

The eight assertions in GOAL.md close that gap. Each publishes a boolean per form
under its own key so `gate.py` can demand it, plus a detail record naming the
offenders -- an assertion that fails without naming what failed is not
actionable. **True means the assertion holds.** Anything that cannot be evaluated
is `False` with a reason, never `True` and never absent: `gate.py` counts absence
as unevaluable and fails, which is the whole point.

The assertions read the source PDF's own drawing and text operators wherever the
question is "what does the official form actually print". That keeps them
independent of the IR schema and of the module whose output they are checking --
`comb_slots_match_printed` in particular must not be scored by the code that
produced the number under test.

Nothing here rasterises. Every measurement is a coordinate or a codepoint.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import collections
import contextlib
import copy
import dataclasses
import functools
import hashlib
import importlib.machinery
import importlib.metadata
import importlib.util
import json
import math
import mimetypes
import os
import pathlib
import platform
import posixpath
import re
import signal
import stat
import statistics
import subprocess
import sys
import sysconfig
import tempfile
import traceback
import types
import urllib.parse
from decimal import Decimal
from html.parser import HTMLParser
from typing import Any, Iterable, Sequence

HERE = pathlib.Path(__file__).resolve().parent


@dataclasses.dataclass(frozen=True)
class _TrustedSource:
    name: str
    path: pathlib.Path
    payload: bytes
    sha256: str
    module: types.ModuleType


def _stable_read(path: pathlib.Path) -> bytes:
    """Read one path-bound regular file and reject mutation during the read."""
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise RuntimeError(
            f"trusted source could not be opened without following a "
            f"symlink: {path}") from exc
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise RuntimeError(
                f"trusted source is not one regular file: {path}")
        chunks: list[bytes] = []
        while True:
            chunk = os.read(descriptor, 1 << 20)
            if not chunk:
                break
            chunks.append(chunk)
        payload = b"".join(chunks)
        after = os.fstat(descriptor)
        try:
            path_after = os.stat(path, follow_symlinks=False)
        except OSError as exc:
            raise RuntimeError(
                f"trusted source path changed while read: {path}") from exc
        stable_fields = (
            "st_dev", "st_ino", "st_mode", "st_size",
            "st_mtime_ns", "st_ctime_ns",
        )
        if (any(getattr(before, field) != getattr(after, field)
                for field in stable_fields)
                or not stat.S_ISREG(path_after.st_mode)
                or (path_after.st_dev, path_after.st_ino)
                != (after.st_dev, after.st_ino)
                or len(payload) != after.st_size):
            raise RuntimeError(
                f"trusted source changed while read: {path}")
        return payload
    finally:
        os.close(descriptor)


@contextlib.contextmanager
def _standard_importers_only() -> Iterable[None]:
    """Exclude caller-installed import hooks while trusted modules execute."""
    original = sys.meta_path[:]
    sys.meta_path[:] = [
        importlib.machinery.BuiltinImporter,
        importlib.machinery.FrozenImporter,
        importlib.machinery.PathFinder,
    ]
    try:
        yield
    finally:
        sys.meta_path[:] = original


def _execute_source_module(
        name: str,
        path: pathlib.Path,
        payload: bytes,
        bindings: dict[str, types.ModuleType],
        ) -> types.ModuleType:
    """Compile exact source bytes, bypassing bytecode and import hooks."""
    module = types.ModuleType(name)
    module.__file__ = str(path)
    module.__package__ = ""
    module.__loader__ = None
    module.__spec__ = importlib.util.spec_from_loader(
        name, loader=None, origin=str(path))
    source_sha = hashlib.sha256(payload).hexdigest()
    module.__dict__["__formgen_source_sha256__"] = source_sha
    code = compile(payload, str(path), "exec", dont_inherit=True)
    prior = {key: sys.modules.get(key) for key in (name, *bindings)}
    old_dont_write = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        sys.modules.update(bindings)
        sys.modules[name] = module
        with _standard_importers_only():
            exec(code, module.__dict__)
    finally:
        sys.dont_write_bytecode = old_dont_write
        for key, previous in prior.items():
            if previous is None:
                sys.modules.pop(key, None)
            else:
                sys.modules[key] = previous
    return module


def _load_trusted_formgen_modules(
        extract_path: pathlib.Path,
        verify_path: pathlib.Path,
        ) -> tuple[_TrustedSource, _TrustedSource]:
    """Load sibling producers from exact bytes, independent of sys.modules."""
    extract_payload = _stable_read(extract_path)
    verify_payload = _stable_read(verify_path)
    trusted_extract = _execute_source_module(
        "extract", extract_path, extract_payload, {})
    trusted_verify = _execute_source_module(
        "verify", verify_path, verify_payload,
        {"extract": trusted_extract},
    )
    if trusted_verify.__dict__.get("extract") is not trusted_extract:
        raise RuntimeError("verify did not bind the snapshotted extract module")
    return (
        _TrustedSource(
            "extract", extract_path.resolve(), extract_payload,
            hashlib.sha256(extract_payload).hexdigest(), trusted_extract),
        _TrustedSource(
            "verify", verify_path.resolve(), verify_payload,
            hashlib.sha256(verify_payload).hexdigest(), trusted_verify),
    )


_TRUSTED_EXTRACT, _TRUSTED_VERIFY = _load_trusted_formgen_modules(
    HERE / "extract.py", HERE / "verify.py")
extract = _TRUSTED_EXTRACT.module
verify = _TRUSTED_VERIFY.module
_AUDIT_SOURCE_PATH = pathlib.Path(__file__).resolve()
_AUDIT_SOURCE_PAYLOAD = _stable_read(_AUDIT_SOURCE_PATH)
# Keep the exact modules installed for any delayed sibling import, and make a
# later substitution visible to the before/after validator below.
sys.modules["extract"] = extract
sys.modules["verify"] = verify

# --------------------------------------------------------------------------
# tolerances
#
# None of these widen a verify.py tolerance; they are the thresholds the
# assertions need and each is derived from one that already exists.
# --------------------------------------------------------------------------

# Two rectangles "overlap" only if they share area in both axes. Sharing an edge
# is not an overlap: an input butted against the glyph beside it is correct.
OVERLAP_EPS_PT = 0.05

# A rule sits below the guide cut only if it clears it by more than the position
# tolerance. Exactly at the cut is exactly right.
CUT_EPS_PT = 0.25

# Shear small enough to be invisible is not a transform. 0.01pt is a 25th of the
# 0.25pt position tolerance, measured over the image's unit square, so the worst
# displacement it admits is 0.01pt. One corpus placement (0605 xref 13) carries
# b = 5.3e-05 and c = 1.7e-06 -- arithmetic noise in the producer, not a skew.
TRANSFORM_EPS = 0.01

# Comb geometry, for the printed-compartment oracle.
#   MERGE: two verticals closer than this cannot be two character compartments.
#          The tightest comb pitch in the corpus is 10.32pt, so this is a sixth
#          of the smallest real gap. It exists because the generator draws one
#          divider as two collinear pieces 0.6pt apart when the piece below the
#          band is thicker than the tick above it.
#   EDGE:  a vertical this close to the cell's own side is that side's border.
#   MINLEN: shorter ink than this is a decimal point or a dot leader, not a
#          divider. The shortest real tick measured is 2.88pt.
#   EDGE:  a divider is interior only if its *ink* clears the cell's own sides
#          by this much. Two facts set it. The cell box is not the frame's
#          centreline -- on 1701 the 1.44pt page frame's outer edge lands 0.15pt
#          inside the cell's x1, so a centre-based or hairline margin counts the
#          frame as a slot boundary and invents 46 merges across 1701 and 1701A.
#          And the 1st-percentile comb pitch in this corpus is 10.24pt, so no
#          real compartment boundary is ever within 2pt of its own cell's side.
#          The measured mismatch count is flat from 2.0 to 3.0pt, which is what
#          being past the artefact and short of real data looks like. (A handful
#          of degenerate combs report a sub-1pt pitch; those are broken combs,
#          and this margin can hide a boundary inside one.)
COMB_MERGE_PT = 1.5
COMB_EDGE_PT = 2.0
COMB_FALLBACK_HALFWIDTH_PT = 0.6   # for an `l` op that declares no stroke width
COMB_MINLEN_PT = 0.8
COMB_YSLACK_PT = 0.5
COMB_MAX_WIDTH_PT = 2.5   # 1.44pt group separators are in; column borders are not
# The reviewed comb-subject ledger may certify only that one exact layout cell
# owns one legacy subject rectangle.  Both active states are reviewed ownership
# decisions; their resolved/unresolved distinction is topology evidence and is
# deliberately not consumed by this oracle.
COMB_OWNER_REVIEWED_STATES = frozenset({
    "active_resolved",
    "active_unresolved",
})
RETAINED_COMB_SUBJECT_KEYS = frozenset({
    "subject_key",
    "legacy_cell_id",
    "legacy_bbox",
    "cell_id",
    "mapped_partition_cell_ids",
    "mapped_partition_subject_keys",
    "state",
    "emission",
    "reason_codes",
    "legacy_comb",
    "requires_independent_evidence",
    "permitted_transitions",
    "blocks_gate",
})
RETAINED_COMB_SUBJECT_OPTIONAL_KEYS = frozenset({
    "erased_edge_replacement_candidates",
})
RETAINED_COMB_TRANSITIONS = (
    "active_composite",
    "retired_proven_false",
)
RETAINED_PARTITION_REASON_CODES = (
    "emission-suppressed-no-rectangular-owner",
    "painted-edge-partition",
)
RETAINED_NO_BAND_REASON_CODES = (
    "emission-suppressed-no-final-visible-band",
)
# emit.py serialises point geometry to four decimals. Two rounded endpoints can
# differ by at most two ten-thousandths of a point.
EMITTED_GEOMETRY_EPS_PT = 0.0002
# PyMuPDF source coordinates carry float noise at roughly the same scale. A
# smaller paper seam is not promoted into a visible source corridor.
SOURCE_COORD_EPS_PT = 0.0002

# Default offender preview limit. Assertions that need exhaustive evidence may
# opt out explicitly; the comb assertion does because its full disagreement set
# is the evidence the referee needs.
MAX_OFFENDERS = 12

# The rate/reference tables. `reflow_rate_without_description` is about a
# relocated *table*, and these are the marker patterns guides.py assigns to one.
RATE_TABLE_MARKERS = frozenset({"table-n", "alphanumeric-tax-code"})

ASSERTION_KEYS = (
    "inputs_over_printed_text",
    "comb_slots_match_printed",
    "money_boxes_have_inputs",
    "rules_below_guide_cut",
    "run_colour_matches_ir",
    "reflow_rate_without_description",
    "image_transform_applied",
    "no_invented_codepoints",
)

INPUT_MANIFEST_SCHEMA = "formgen-audit-input-manifest-v1"
REQUIRED_INPUT_ROLES = ("ir", "layout", "html", "guide", "source_pdf")

Rect = tuple[float, float, float, float]


# --------------------------------------------------------------------------
# geometry
# --------------------------------------------------------------------------


def overlaps(a: Rect, b: Rect, eps: float = OVERLAP_EPS_PT) -> bool:
    return (min(a[2], b[2]) - max(a[0], b[0]) > eps
            and min(a[3], b[3]) - max(a[1], b[1]) > eps)


class InkIndex:
    """Printed glyph boxes on one page, bucketed by y so lookups stay cheap.

    A linear scan is 17s over the corpus for one assertion; two assertions need
    it, and an audit nobody runs protects nothing.
    """

    BUCKET_PT = 8.0

    def __init__(self, boxes: Iterable[tuple[Rect, Any]]) -> None:
        self.buckets: dict[int, list[tuple[Rect, Any]]] = collections.defaultdict(list)
        for box, tag in boxes:
            lo = int(math.floor(box[1] / self.BUCKET_PT))
            hi = int(math.floor(box[3] / self.BUCKET_PT))
            for key in range(lo, hi + 1):
                self.buckets[key].append((box, tag))

    def hits(self, rect: Rect) -> list[tuple[Rect, Any]]:
        lo = int(math.floor(rect[1] / self.BUCKET_PT))
        hi = int(math.floor(rect[3] / self.BUCKET_PT))
        out = []
        for key in range(lo, hi + 1):
            for box, tag in self.buckets.get(key, ()):
                if overlaps(rect, box):
                    out.append((box, tag))
        return out

    def any_hit(self, rect: Rect) -> tuple[Rect, Any] | None:
        for hit in self.hits(rect):
            return hit
        return None


def glyph_boxes(run: dict) -> list[Rect]:
    """One box per *inked* glyph in a text run.

    The run bbox is the wrong unit for "does an input sit on pre-printed text".
    A label like `'Yes            No'` is one run whose bbox spans the checkbox
    drawn in the gap between the two words; scored by bbox it reports a collision
    that is not there, and 269 of the 362 bbox collisions in this corpus are that
    artefact. A glyph's own advance box is where the ink is.
    """
    out: list[Rect] = []
    offsets = run.get("char_origin_offsets_pt") or ()
    widths = run.get("char_widths_pt") or ()
    if len(offsets) != len(run["text"]) or len(widths) != len(run["text"]):
        # Without per-glyph metrics the run bbox is all there is; say so by
        # returning it, so the assertion errs towards reporting a collision.
        return [(run["x0"], run["y0"], run["x1"], run["y1"])]
    origin = run.get("origin_x", run["x0"])
    for char, offset, width in zip(run["text"], offsets, widths):
        if not char.strip():
            continue
        x = origin + offset
        out.append((x, run["y0"], x + width, run["y1"]))
    return out


# --------------------------------------------------------------------------
# emitted-document parsing
#
# The markup, not the layout engine. emit.py writes every position it means as
# an inline `pt` value in the same coordinate space as the IR, so parsing is
# exact, deterministic and about 400x faster than driving a browser. Playwright
# would only add its own layout opinion between us and the numbers we wrote.
# --------------------------------------------------------------------------

CELL_RE = re.compile(r'<div id="(p(\d+)c\d+)" class="([^"]*)"([^>]*)>')
# The last cell on a page is followed by the page's closing tags, an inert band
# template, or the band script, so cell content has to stop there too. Template
# inputs are blueprints, not live inputs belonging to the preceding cell.
CELL_BOUNDARY_RE = re.compile(
    r'<div id="p\d+c\d+" class="|<div class="page |<template|<script')
STYLE_BOX_RE = re.compile(
    r'style="left:([-\d.]+)pt;top:([-\d.]+)pt;width:([-\d.]+)pt;height:([-\d.]+)pt"')
SLOT_RE = re.compile(
    r'<div class="s" data-slot="(\d+)" '
    r'style="left:([-\d.]+)pt;top:([-\d.]+)pt;width:([-\d.]+)pt;height:([-\d.]+)pt"\s*>'
    r'(.*?)</div>', re.S)
INPUT_RE = re.compile(r'<input\b([^>]*)>')
INSET_RE = re.compile(
    r'inset:([-\d.]+)pt ([-\d.]+)pt ([-\d.]+)pt ([-\d.]+)pt')
RUN_RE = re.compile(r'<div class="t" id="p(\d+)t(\d+)" style="([^"]*)"')
PAGE_SPLIT_RE = re.compile(r'<div class="page page-(\d+)"')
SVG_RECT_RE = re.compile(r'<rect\b([^>]*)/>')
SVG_IMAGE_RE = re.compile(r'<image\b([^>]*)/>')
ATTR_RE = re.compile(r'([-a-zA-Z0-9:]+)="([^"]*)"')
COLOR_RE = re.compile(r'(?:^|;)color:#([0-9a-fA-F]{6})')
SECTION_RE = re.compile(r'<section class="gl-page"([^>]*)>(.*?)</section>', re.S)
ROW_RE = re.compile(r'<tr>(.*?)</tr>', re.S)
TD_RE = re.compile(r'<td[^>]*>(.*?)</td>', re.S)
TAG_RE = re.compile(r'<[^>]*>')


@dataclasses.dataclass
class Cell:
    id: str
    page: int
    classes: str
    attrs: str
    rect: Rect
    inner: str
    dom_page: int | None = None
    dom_record: _DomCellRecord | None = None

    @property
    def comb_slots_attr(self) -> int | None:
        got = re.search(r'data-comb-slots="(\d+)"', self.attrs)
        return int(got.group(1)) if got else None


CANONICAL_CELL_ID_RE = re.compile(r"p(\d+)c\d+\Z")
DOM_PAGE_CLASS_RE = re.compile(r"(?:^|\s)page-(\d+)(?:\s|$)")


@dataclasses.dataclass
class _DomInputRecord:
    element_index: int
    data_slot_index: int | None
    owning_slot: tuple[int, int] | None
    inset: tuple[float, float, float, float] | None
    input_type: str
    editable: bool


@dataclasses.dataclass
class _DomSlotRecord:
    element_index: int
    index: int | None
    geometry: dict[str, float] | None
    input_indexes: list[int | None] = dataclasses.field(default_factory=list)


@dataclasses.dataclass
class _DomCellRecord:
    element_index: int
    cell_id: str | None
    dom_page: int | None
    live: bool
    comb_marked: bool
    slots: list[_DomSlotRecord] = dataclasses.field(default_factory=list)
    inputs: list[_DomInputRecord] = dataclasses.field(default_factory=list)
    unowned_slot_inputs: list[dict[str, Any]] = dataclasses.field(
        default_factory=list)

    @property
    def slot_count(self) -> int:
        return len(self.slots)


class _EmittedDomScanner(HTMLParser):
    """Track live comb ownership using real nesting, including page owners."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=False)
        self.current_page: int | None = None
        self.template_depth = 0
        self.guide_depth = 0
        self.section_stack: list[bool] = []
        self.div_stack: list[
            tuple[int | None, int | None, tuple[int, int] | None]
        ] = []
        self.cell_stack: list[int] = []
        self.slot_stack: list[tuple[int, int]] = []
        self.records: list[_DomCellRecord] = []
        self.orphan_slots: list[dict[str, Any]] = []
        self.element_index = 0

    @property
    def live(self) -> bool:
        return self.template_depth == 0 and self.guide_depth == 0

    def handle_starttag(
            self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        tag = tag.lower()
        values = {name.lower(): (value or "") for name, value in attrs}
        classes = set(values.get("class", "").split())
        if tag == "template":
            self.template_depth += 1
            return
        if tag == "section":
            is_guide = "gl-page" in classes
            self.section_stack.append(is_guide)
            self.guide_depth += int(is_guide)
            return
        if tag == "input":
            if not self.live:
                return
            marked_slot_input = "data-slot-index" in values
            owners = [
                index for index in self.cell_stack
                if self.records[index].live
            ]
            slots = [
                slot for slot in self.slot_stack
                if slot[0] in owners
            ]
            raw_index = values.get("data-slot-index")
            try:
                input_index = (
                    int(raw_index) if raw_index not in (None, "") else None)
            except ValueError:
                input_index = None
            input_type = values.get("type", "text").lower() or "text"
            style = values.get("style", "").lower().replace(" ", "")
            editable = (
                "disabled" not in values
                and "readonly" not in values
                and "hidden" not in values
                and values.get("aria-hidden", "").lower() != "true"
                and input_type not in {
                    "hidden", "button", "submit", "reset", "image"}
                and "display:none" not in style
                and "visibility:hidden" not in style
            )
            inset_match = INSET_RE.search(values.get("style", ""))
            inset = (
                tuple(float(inset_match.group(index))
                      for index in (1, 2, 3, 4))
                if inset_match is not None else None
            )
            owning_slot = slots[0] if len(slots) == 1 else None
            if len(owners) == 1:
                self.records[owners[0]].inputs.append(_DomInputRecord(
                    element_index=self.element_index,
                    data_slot_index=input_index,
                    owning_slot=owning_slot,
                    inset=inset,
                    input_type=input_type,
                    editable=editable,
                ))
            if len(owners) == 1 and len(slots) == 1 and editable:
                owner_index, slot_index = slots[0]
                self.records[owner_index].slots[
                    slot_index].input_indexes.append(input_index)
            elif marked_slot_input or (len(slots) == 1 and not editable):
                issue = {
                    "element_index": self.element_index,
                    "dom_page": self.current_page,
                    "owner_ids": [
                        self.records[index].cell_id for index in owners
                    ],
                    "data_slot_index": raw_index,
                    "reason": (
                        "comb input is not one live editable input enclosed "
                        "by exactly one live physical slot in exactly one "
                        "live cell"),
                }
                if len(owners) == 1:
                    self.records[owners[0]].unowned_slot_inputs.append(issue)
                else:
                    self.orphan_slots.append(issue)
            self.element_index += 1
            return
        if tag != "div":
            return

        prior_page = self.current_page
        page_match = DOM_PAGE_CLASS_RE.search(values.get("class", ""))
        if page_match:
            self.current_page = int(page_match.group(1))

        cell_id = values.get("id") or None
        canonical = (
            CANONICAL_CELL_ID_RE.fullmatch(cell_id)
            if cell_id is not None else None
        )
        comb_marker = (
            values.get("data-field-kind", "").lower() == "comb"
            or "data-comb-slots" in values
            or "data-comb-capacity" in values
        )
        is_slot = "s" in classes and "data-slot" in values
        is_cell = (
            "c" in classes
            or canonical is not None
            or comb_marker
        ) and not is_slot
        pushed_cell: int | None = None
        pushed_slot: tuple[int, int] | None = None
        if is_cell:
            pushed_cell = len(self.records)
            self.records.append(_DomCellRecord(
                element_index=self.element_index,
                cell_id=cell_id,
                dom_page=self.current_page,
                live=self.live,
                comb_marked=comb_marker,
            ))
            self.cell_stack.append(pushed_cell)
        if is_slot and self.live:
            owners = [
                index for index in self.cell_stack
                if self.records[index].live
            ]
            raw_slot_index = values.get("data-slot")
            try:
                slot_index = (
                    int(raw_slot_index)
                    if raw_slot_index not in (None, "") else None)
            except ValueError:
                slot_index = None
            box = STYLE_BOX_RE.search(
                f'style="{values.get("style", "")}"')
            geometry = None
            if box is not None:
                left, top, width, height = (
                    float(box.group(index)) for index in (1, 2, 3, 4))
                geometry = {
                    "index": slot_index,
                    "left": left,
                    "top": top,
                    "width": width,
                    "height": height,
                }
            if len(owners) == 1:
                owner = self.records[owners[0]]
                owner.slots.append(_DomSlotRecord(
                    element_index=self.element_index,
                    index=slot_index,
                    geometry=geometry,
                ))
                owner.comb_marked = True
                pushed_slot = (owners[0], len(owner.slots) - 1)
                self.slot_stack.append(pushed_slot)
            else:
                self.orphan_slots.append({
                    "element_index": self.element_index,
                    "dom_page": self.current_page,
                    "data_slot": raw_slot_index,
                    "owner_ids": [
                        self.records[index].cell_id for index in owners
                    ],
                    "reason": (
                        "live physical slot is not enclosed by exactly one "
                        "live cell container"),
                })
        self.div_stack.append((prior_page, pushed_cell, pushed_slot))
        self.element_index += 1

    def handle_endtag(self, tag: str) -> None:
        tag = tag.lower()
        if tag == "template":
            self.template_depth = max(0, self.template_depth - 1)
            return
        if tag == "section":
            if self.section_stack:
                self.guide_depth -= int(self.section_stack.pop())
            return
        if tag != "div" or not self.div_stack:
            return
        prior_page, pushed_cell, pushed_slot = self.div_stack.pop()
        if pushed_slot is not None:
            if self.slot_stack and self.slot_stack[-1] == pushed_slot:
                self.slot_stack.pop()
            elif pushed_slot in self.slot_stack:
                self.slot_stack.remove(pushed_slot)
        if pushed_cell is not None:
            if self.cell_stack and self.cell_stack[-1] == pushed_cell:
                self.cell_stack.pop()
            elif pushed_cell in self.cell_stack:
                self.cell_stack.remove(pushed_cell)
        self.current_page = prior_page

    def handle_startendtag(
            self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self.handle_starttag(tag, attrs)
        self.handle_endtag(tag)


@functools.lru_cache(maxsize=2)
def _scan_emitted_dom(
        html: str,
        ) -> tuple[tuple[_DomCellRecord, ...], tuple[dict[str, Any], ...]]:
    scanner = _EmittedDomScanner()
    scanner.feed(html)
    scanner.close()
    return tuple(scanner.records), tuple(scanner.orphan_slots)


def parse_cells(html: str) -> list[Cell]:
    """Every field/label cell div with its box, in document order."""
    dom_records, _orphan_slots = _scan_emitted_dom(html)
    by_id: dict[str, collections.deque[_DomCellRecord]] = (
        collections.defaultdict(collections.deque))
    for record in dom_records:
        if record.cell_id is not None:
            by_id[record.cell_id].append(record)
    starts = list(CELL_RE.finditer(html))
    cells: list[Cell] = []
    for index, match in enumerate(starts):
        dom_record = (
            by_id[match.group(1)].popleft()
            if by_id[match.group(1)] else None
        )
        if dom_record is not None and not dom_record.live:
            continue
        limit = starts[index + 1].start() if index + 1 < len(starts) else len(html)
        stop = CELL_BOUNDARY_RE.search(html, match.end(), limit)
        inner = html[match.end():stop.start() if stop else limit]
        box = STYLE_BOX_RE.search(match.group(4))
        if not box:
            continue
        left, top, width, height = (float(box.group(i)) for i in (1, 2, 3, 4))
        cells.append(Cell(id=match.group(1), page=int(match.group(2)),
                          classes=match.group(3), attrs=match.group(4),
                          rect=(left, top, left + width, top + height),
                          inner=inner,
                          dom_page=(
                              dom_record.dom_page
                              if dom_record is not None else None
                          ),
                          dom_record=dom_record))
    return cells


def live_comb_inventory_issues(
        html: str, parsed_cells: Sequence[Cell],
        ) -> list[dict[str, Any]]:
    """Every live comb marker/slot must belong to one parsed canonical cell."""
    records, orphan_slots = _scan_emitted_dom(html)
    parsed_remaining = collections.Counter(cell.id for cell in parsed_cells)
    issues: list[dict[str, Any]] = [dict(issue) for issue in orphan_slots]
    for record in records:
        for input_issue in record.unowned_slot_inputs:
            issues.append({
                **input_issue,
                "cell_id": record.cell_id,
                "slot_count": record.slot_count,
            })
        if not record.live or not (record.comb_marked or record.slot_count):
            continue
        canonical = (
            CANONICAL_CELL_ID_RE.fullmatch(record.cell_id)
            if record.cell_id is not None else None
        )
        if canonical is None:
            issues.append({
                "element_index": record.element_index,
                "cell_id": record.cell_id,
                "dom_page": record.dom_page,
                "slot_count": record.slot_count,
                "reason": (
                    "live comb container has no canonical pNcN cell id"),
            })
            continue
        if parsed_remaining[record.cell_id] <= 0:
            issues.append({
                "element_index": record.element_index,
                "cell_id": record.cell_id,
                "dom_page": record.dom_page,
                "slot_count": record.slot_count,
                "reason": (
                    "live canonical comb container was not parsed as a cell"),
            })
            continue
        parsed_remaining[record.cell_id] -= 1
    return sorted(
        issues,
        key=lambda issue: (
            int(issue.get("element_index", -1)),
            str(issue.get("cell_id") or ""),
            str(issue.get("reason") or ""),
        ),
    )


def emitted_cell_binding_issues(b: Any) -> list[dict[str, Any]]:
    """Bind every live canonical cell to one layout owner and actual DOM page."""
    if getattr(b, "layout", None) is None:
        return []
    relocated = set(getattr(b, "relocated_cells", set()))
    layout_subjects: dict[
        str, list[tuple[int, dict[str, Any]]]
    ] = collections.defaultdict(list)
    for page_index, page in sorted(getattr(b, "layout_pages", {}).items()):
        for layout_cell in page.get("cells", ()):
            cell_id = layout_cell.get("id")
            if isinstance(cell_id, str) and cell_id not in relocated:
                layout_subjects[cell_id].append((page_index, layout_cell))

    emitted: dict[str, list[Cell]] = collections.defaultdict(list)
    emitted_order: list[str] = []
    for cell in getattr(b, "cells", ()):
        if cell.id not in emitted:
            emitted_order.append(cell.id)
        emitted[cell.id].append(cell)

    issues: list[dict[str, Any]] = []
    has_real_html = isinstance(getattr(b, "form_html", None), str)
    for cell_id in emitted_order:
        cells = emitted[cell_id]
        subjects = layout_subjects.get(cell_id, ())
        kinds: list[str] = []
        reasons: list[str] = []
        if len(cells) != 1:
            kinds.append("duplicate-emitted-cell-id")
            reasons.append(
                f"emitted document contains {len(cells)} live cells with id "
                f"{cell_id}; exactly one is required")
        if len(subjects) != 1:
            kinds.append(
                "missing-layout-cell-owner"
                if not subjects else "duplicate-layout-cell-owner")
            reasons.append(
                f"layout contains {len(subjects)} non-relocated owners for "
                f"{cell_id}; exactly one is required")
        if len(cells) == 1 and len(subjects) == 1:
            emitted_cell = cells[0]
            page_index, layout_cell = subjects[0]
            dom_page = emitted_cell.dom_page
            if dom_page is None and not has_real_html:
                dom_page = emitted_cell.page
            if (emitted_cell.page != page_index
                    or dom_page != page_index):
                kinds.append("emitted-cell-page-mismatch")
                reasons.append(
                    "cell id page, enclosing DOM page, and layout page differ")
            expected_rect = (
                float(layout_cell["x0"]), float(layout_cell["y0"]),
                float(layout_cell["x1"]), float(layout_cell["y1"]),
            )
            deltas = [
                actual - expected
                for actual, expected in zip(emitted_cell.rect, expected_rect)
            ]
            if any(abs(delta) > EMITTED_GEOMETRY_EPS_PT
                   for delta in deltas):
                kinds.append("emitted-cell-geometry-mismatch")
                reasons.append(
                    "emitted cell rectangle differs from its layout owner")
            evidence = {
                "cell": cell_id,
                "emitted_occurrences": 1,
                "layout_occurrences": 1,
                "emitted_id_page": emitted_cell.page,
                "emitted_dom_page": dom_page,
                "layout_page": page_index,
                "actual_rect": list(emitted_cell.rect),
                "expected_rect": list(expected_rect),
                "rect_deltas_pt": [
                    round(delta, 6) for delta in deltas],
                "tolerance_pt": EMITTED_GEOMETRY_EPS_PT,
            }
        else:
            evidence = {
                "cell": cell_id,
                "emitted_occurrences": len(cells),
                "layout_occurrences": len(subjects),
            }
        if kinds:
            issues.append({
                **evidence,
                "failure_kinds": kinds,
                "why": "; ".join(reasons),
            })
    return issues


def slot_boxes(cell: Cell) -> list[tuple[int, Rect, bool]]:
    """Comb slots as (index, absolute box, whether it holds an input)."""
    left, top, _, _ = cell.rect
    if cell.dom_record is not None:
        live_out: list[tuple[int, Rect, bool]] = []
        for slot in cell.dom_record.slots:
            geometry = slot.geometry
            if slot.index is None or geometry is None:
                continue
            x = float(geometry["left"])
            y = float(geometry["top"])
            width = float(geometry["width"])
            height = float(geometry["height"])
            live_out.append((
                slot.index,
                (left + x, top + y, left + x + width, top + y + height),
                bool(slot.input_indexes),
            ))
        return live_out
    out = []
    for match in SLOT_RE.finditer(cell.inner):
        x, y, w, h = (float(match.group(i)) for i in (2, 3, 4, 5))
        out.append((int(match.group(1)), (left + x, top + y, left + x + w, top + y + h),
                    "<input" in match.group(6)))
    return out


def emitted_comb_evidence(cells: Sequence[Cell]) -> dict[str, Any]:
    """Physical emitted-comb state without falling back to layout metadata."""
    occurrences = len(cells)
    if occurrences == 0:
        return {
            "valid": False,
            "state": "missing-emitted-cell",
            "slots": None,
            "physical_slots": None,
            "declared_slots": None,
            "occurrences": occurrences,
            "reason": "emitted document has no matching cell",
        }
    if occurrences != 1:
        return {
            "valid": False,
            "state": "duplicate-emitted-cell",
            "slots": None,
            "physical_slots": None,
            "declared_slots": None,
            "occurrences": occurrences,
            "reason": (
                f"emitted document contains {occurrences} cells with this id; "
                "exactly one is required"),
        }

    cell = cells[0]
    matches = list(SLOT_RE.finditer(cell.inner))
    dom_slots = (
        list(cell.dom_record.slots)
        if cell.dom_record is not None else None
    )
    indexes: list[int | None] = (
        [slot.index for slot in dom_slots]
        if dom_slots is not None
        else [int(match.group(1)) for match in matches]
    )
    physical = len(indexes)
    declared = cell.comb_slots_attr
    if physical == 0:
        if declared is None:
            state = "missing-comb-markup"
            reason = "emitted cell has no comb slot markup"
            slots: int | None = None
        else:
            state = "zero-physical-slots"
            reason = (
                f"emitted comb declares {declared} slot(s) but renders zero "
                "physical slots")
            slots = 0
        return {
            "valid": False,
            "state": state,
            "slots": slots,
            "physical_slots": physical,
            "declared_slots": declared,
            "occurrences": occurrences,
            "reason": reason,
        }
    if declared is None:
        return {
            "valid": False,
            "state": "missing-declared-slot-count",
            "slots": physical,
            "physical_slots": physical,
            "declared_slots": None,
            "occurrences": occurrences,
            "slot_indexes": indexes,
            "reason": (
                "emitted physical comb slots have no data-comb-slots "
                "declaration"),
        }
    if len(set(indexes)) != physical:
        return {
            "valid": False,
            "state": "duplicate-slot-index",
            "slots": physical,
            "physical_slots": physical,
            "declared_slots": declared,
            "occurrences": occurrences,
            "slot_indexes": indexes,
            "reason": (
                "emitted comb repeats a physical data-slot index "
                f"({indexes})"),
        }
    expected_indexes = list(range(physical))
    if indexes != expected_indexes:
        return {
            "valid": False,
            "state": "invalid-slot-index-sequence",
            "slots": physical,
            "physical_slots": physical,
            "declared_slots": declared,
            "occurrences": occurrences,
            "slot_indexes": indexes,
            "reason": (
                "emitted comb data-slot indexes must be exactly ordered "
                f"0..{physical - 1}; got {indexes}"),
        }
    input_indexes: list[list[int | None]] = (
        [list(slot.input_indexes) for slot in dom_slots]
        if dom_slots is not None else []
    )
    bad_input_indexes: list[dict[str, Any]] = []
    if dom_slots is None:
        for slot_index, match in zip(indexes, matches):
            within_slot: list[int | None] = []
            for input_match in INPUT_RE.finditer(match.group(6)):
                index_match = re.search(
                    r'(?:^|\s)data-slot-index="(\d+)"(?:\s|$)',
                    input_match.group(1),
                )
                input_index = int(index_match.group(1)) if index_match else None
                within_slot.append(input_index)
            input_indexes.append(within_slot)
    for slot_index, within_slot in zip(indexes, input_indexes):
        for input_index in within_slot:
            if input_index != slot_index:
                bad_input_indexes.append({
                    "slot": slot_index,
                    "input_slot_index": input_index,
                })
        if len(within_slot) > 1:
            bad_input_indexes.append({
                "slot": slot_index,
                "input_slot_index": within_slot,
                "reason": "multiple editable inputs in one physical slot",
            })
        if "f" in cell.classes.split() and not within_slot:
            bad_input_indexes.append({
                "slot": slot_index,
                "input_slot_index": None,
                "reason": (
                    "editable comb slot has no live input element"),
            })
    nested_input_count = sum(len(items) for items in input_indexes)
    if dom_slots is None:
        marked_input_count = sum(
            "data-slot-index" in input_match.group(1)
            for input_match in INPUT_RE.finditer(cell.inner)
        )
        if marked_input_count != nested_input_count:
            bad_input_indexes.append({
                "reason": (
                    "one or more slot-indexed inputs are outside a physical "
                    "slot"),
                "nested": nested_input_count,
                "marked": marked_input_count,
            })
    elif cell.dom_record is not None and cell.dom_record.unowned_slot_inputs:
        bad_input_indexes.extend(cell.dom_record.unowned_slot_inputs)
    if bad_input_indexes:
        return {
            "valid": False,
            "state": "slot-input-index-mismatch",
            "slots": physical,
            "physical_slots": physical,
            "declared_slots": declared,
            "occurrences": occurrences,
            "slot_indexes": indexes,
            "input_slot_indexes": input_indexes,
            "reason": (
                "one or more comb inputs do not identify their owning slot: "
                f"{bad_input_indexes}"),
        }

    try:
        geometry = (
            [dict(slot.geometry) if slot.geometry is not None else {}
             for slot in dom_slots]
            if dom_slots is not None
            else [
                {
                    "index": int(match.group(1)),
                    "left": float(match.group(2)),
                    "top": float(match.group(3)),
                    "width": float(match.group(4)),
                    "height": float(match.group(5)),
                }
                for match in matches
            ]
        )
    except (TypeError, ValueError):
        geometry = []
    cell_width = cell.rect[2] - cell.rect[0]
    cell_height = cell.rect[3] - cell.rect[1]
    finite_container = (
        all(math.isfinite(value) for value in cell.rect)
        and cell_width > 0
        and cell_height > 0
    )
    geometry_valid = (
        finite_container
        and len(geometry) == physical
        and all(
            all(name in item for name in ("left", "top", "width", "height"))
            and all(math.isfinite(float(item[name]))
                for name in ("left", "top", "width", "height"))
            and float(item["width"]) > 0
            and float(item["height"]) > 0
            and float(item["left"]) >= -EMITTED_GEOMETRY_EPS_PT
            and float(item["left"]) + float(item["width"])
            <= cell_width + EMITTED_GEOMETRY_EPS_PT
            and max(0.0, float(item["top"]))
            < min(cell_height,
                  float(item["top"]) + float(item["height"]))
            for item in geometry
        )
    )
    if geometry_valid:
        geometry_valid = all(
                abs(
                    float(right["left"])
                    - (float(left["left"]) + float(left["width"]))
                ) <= EMITTED_GEOMETRY_EPS_PT
                and abs(
                    max(0.0, float(right["top"]))
                    - max(0.0, float(left["top"]))
                ) <= EMITTED_GEOMETRY_EPS_PT
                and abs(
                    min(cell_height,
                        float(right["top"]) + float(right["height"]))
                    - min(cell_height,
                          float(left["top"]) + float(left["height"]))
                ) <= EMITTED_GEOMETRY_EPS_PT
                for left, right in zip(geometry, geometry[1:])
            )
    if not geometry_valid:
        return {
            "valid": False,
            "state": "invalid-slot-geometry",
            "slots": physical,
            "physical_slots": physical,
            "declared_slots": declared,
            "occurrences": occurrences,
            "slot_indexes": indexes,
            "input_slot_indexes": input_indexes,
            "slot_geometry": geometry,
            "reason": (
                "emitted slots must be finite positive, vertically present "
                "after clipping, and form one ordered contiguous x partition "
                "within their comb container"),
        }
    if declared is not None and declared != physical:
        return {
            "valid": False,
            "state": "declared-physical-slot-mismatch",
            "slots": physical,
            "physical_slots": physical,
            "declared_slots": declared,
            "occurrences": occurrences,
            "slot_indexes": indexes,
            "input_slot_indexes": input_indexes,
            "slot_geometry": geometry,
            "reason": (
                f"emitted comb declares {declared} slot(s) but renders "
                f"{physical} physical slots"),
        }
    return {
        "valid": True,
        "state": "physical-slots",
        "slots": physical,
        "physical_slots": physical,
        "declared_slots": declared,
        "occurrences": occurrences,
        "slot_indexes": indexes,
        "input_slot_indexes": input_indexes,
        "slot_geometry": geometry,
        "reason": "",
    }


def _position_evidence(
        actual: Sequence[float] | None,
        expected: Sequence[float] | None,
        *,
        comparable: bool,
        unavailable_reason: str | None = None,
        ) -> dict[str, Any]:
    """Publish one fixed-tolerance physical-edge comparison."""
    actual_values = (
        [float(value) for value in actual] if actual is not None else None)
    expected_values = (
        [float(value) for value in expected] if expected is not None else None)
    evidence: dict[str, Any] = {
        "comparable": comparable,
        "tolerance_pt": EMITTED_GEOMETRY_EPS_PT,
        "actual_internal_edges_x": (
            [round(value, 6) for value in actual_values]
            if actual_values is not None else None),
        "expected_internal_edges_x": (
            [round(value, 6) for value in expected_values]
            if expected_values is not None else None),
    }
    if not comparable:
        evidence.update({
            "count_matches": None,
            "deltas_pt": None,
            "matches": None,
            "unavailable_reason": unavailable_reason,
        })
        return evidence
    if actual_values is None or expected_values is None:
        evidence.update({
            "count_matches": False,
            "deltas_pt": None,
            "matches": False,
            "unavailable_reason": (
                unavailable_reason
                or "required physical-edge geometry is absent"),
        })
        return evidence
    count_matches = len(actual_values) == len(expected_values)
    deltas = (
        [actual_value - expected_value
         for actual_value, expected_value
         in zip(actual_values, expected_values)]
        if count_matches else None
    )
    evidence.update({
        "count_matches": count_matches,
        "deltas_pt": (
            [round(delta, 6) for delta in deltas]
            if deltas is not None else None),
        "matches": (
            count_matches
            and all(abs(delta) <= EMITTED_GEOMETRY_EPS_PT
                    for delta in deltas or ())
        ),
    })
    return evidence


def _emitted_slot_edges(
        cell: Cell | None, emission: dict[str, Any],
        ) -> list[float] | None:
    """Absolute x positions of every physical slot boundary, including rails."""
    geometry = emission.get("slot_geometry")
    if cell is None or not isinstance(geometry, list) or not geometry:
        return None
    try:
        return [
            cell.rect[0] + float(geometry[0]["left"]),
            *(
                cell.rect[0] + float(slot["left"]) + float(slot["width"])
                for slot in geometry
            ),
        ]
    except (KeyError, TypeError, ValueError):
        return None


def _emitted_internal_edges(
        cell: Cell | None, emission: dict[str, Any],
        ) -> list[float] | None:
    edges = _emitted_slot_edges(cell, emission)
    return edges[1:-1] if edges is not None else None


def _outer_position_evidence(
        actual: Sequence[float] | None,
        expected: Sequence[float] | None,
        *,
        comparable: bool,
        unavailable_reason: str | None = None,
        ) -> dict[str, Any]:
    evidence = _position_evidence(
        actual,
        expected,
        comparable=comparable,
        unavailable_reason=unavailable_reason,
    )
    evidence["actual_outer_edges_x"] = evidence.pop(
        "actual_internal_edges_x")
    evidence["expected_outer_edges_x"] = evidence.pop(
        "expected_internal_edges_x")
    return evidence


def input_boxes(cell: Cell) -> list[Rect]:
    """Every editable box this cell renders, in page coordinates.

    Comb inputs live inside their slot div and fill it; a plain text input fills
    the cell minus its declared inset. Both are absolute numbers in the markup,
    so no layout pass is needed to know where a taxpayer can type. A comb slot
    is clipped to its parent cell because `.f` uses `overflow:hidden`; geometry
    outside that box cannot paint typed text over the page.
    """
    left, top, right, bottom = cell.rect
    out: list[Rect] = []
    for _, box, has_input in slot_boxes(cell):
        if has_input:
            clipped = (max(left, box[0]), max(top, box[1]),
                       min(right, box[2]), min(bottom, box[3]))
            if clipped[2] > clipped[0] and clipped[3] > clipped[1]:
                out.append(clipped)
    if cell.dom_record is not None:
        for input_record in cell.dom_record.inputs:
            if input_record.owning_slot is not None or not input_record.editable:
                continue
            if input_record.inset is not None:
                t, r, b, l = input_record.inset
                out.append((left + l, top + t, right - r, bottom - b))
            else:
                out.append(cell.rect)
        return out
    for match in INPUT_RE.finditer(cell.inner):
        attrs = match.group(1)
        if "data-slot-index" in attrs:
            continue    # already counted via its slot
        inset = INSET_RE.search(attrs)
        if inset:
            t, r, b, l = (float(inset.group(i)) for i in (1, 2, 3, 4))
            out.append((left + l, top + t, right - r, bottom - b))
        else:
            out.append(cell.rect)
    return out


def parse_run_styles(html: str) -> dict[tuple[int, int], str]:
    return {(int(m.group(1)), int(m.group(2))): m.group(3) for m in RUN_RE.finditer(html)}


def page_chunks(html: str) -> dict[int, str]:
    """The markup of each `.page` div, keyed by its 1-based page number."""
    parts = PAGE_SPLIT_RE.split(html)
    return {int(parts[i]): parts[i + 1] for i in range(1, len(parts), 2)}


def attrs_of(text: str) -> dict[str, str]:
    return {k: v for k, v in ATTR_RE.findall(text)}


# --------------------------------------------------------------------------
# source-PDF oracles
#
# Everything below reads the pinned official PDF, so it answers "what does the
# form print" rather than "what did we decide the form prints".
# --------------------------------------------------------------------------


@functools.lru_cache(maxsize=1)
def source_index(root: str) -> dict[str, tuple[pathlib.Path, ...]]:
    base = pathlib.Path(root).expanduser()
    index: dict[str, list[pathlib.Path]] = collections.defaultdict(list)
    if base.is_dir():
        for pdf in sorted(base.rglob("*.pdf")):
            index[pdf.name].append(pdf)
    return {name: tuple(paths) for name, paths in index.items()}


def resolve_source_payload(
        ir: dict, root: str,
        ) -> tuple[pathlib.Path, bytes] | None:
    """Read the pinned PDF once and return its confirmed immutable payload.

    The IR records only a basename (`external:bir2550m.pdf`) and the corpus has
    duplicate folders offering the same name, so the recorded sha256 is what
    decides. A near-miss is not accepted: an assertion scored against the wrong
    revision is worse than one that reports it could not be scored. The caller
    retains these exact bytes; no later assertion reopens the mutable path.
    """
    source = ir.get("source") or {}
    name = str(source.get("file", "")).split(":", 1)[-1]
    wanted = source.get("sha256")
    for candidate in source_index(root).get(name, ()):
        try:
            payload = _stable_read(candidate)
        except RuntimeError:
            continue
        if hashlib.sha256(payload).hexdigest() == wanted:
            return candidate, payload
    return None


@dataclasses.dataclass(frozen=True)
class VectorPaint:
    """One axis-aligned region belonging to one source painting operation.

    Several regions may share an operation.  They are composited once, not once
    per region: an even-odd compound fill can cancel overlapping rectangles,
    and a translucent compound fill applies its opacity once over their union.
    """

    x0: float
    y0: float
    x1: float
    y1: float
    tone: float
    opacity: float
    order: int
    kind: str
    operation: int = -1
    fill_rule: str = "union"
    winding: int = 1

    def covers(self, x: float, y: float) -> bool:
        return self.x0 <= x <= self.x1 and self.y0 <= y <= self.y1


@dataclasses.dataclass(frozen=True)
class UnsupportedVectorPaint:
    """A source paint whose final topology cannot be represented exactly."""

    rect: Rect
    order: int
    reason: str
    tone: float | None = None
    opacity: float | None = None
    trace_rects: tuple[Rect, ...] = ()


@dataclasses.dataclass(frozen=True)
class VectorPage:
    paints: tuple[VectorPaint, ...]
    unsupported: tuple[UnsupportedVectorPaint, ...]


@dataclasses.dataclass(frozen=True)
class _SourceBaselineSpan:
    """One untrimmed, final-visible horizontal source-paint lineage."""

    y: float
    y0: float
    y1: float
    left: float
    right: float
    operations: tuple[tuple[int, int], ...]
    segments: tuple[tuple[float, float, float, float], ...] = ()


class CombTopologyError(ValueError):
    """Fail-closed topology error carrying machine-checkable source evidence."""

    def __init__(self, message: str, evidence: dict[str, Any]) -> None:
        super().__init__(message)
        self.evidence = evidence


COMB_SUBJECT_KEY_RE = re.compile(
    r"p(?P<page>\d+)@"
    r"(?P<x0>-?(?:\d+(?:\.\d*)?|\.\d+)),"
    r"(?P<y0>-?(?:\d+(?:\.\d*)?|\.\d+)),"
    r"(?P<x1>-?(?:\d+(?:\.\d*)?|\.\d+)),"
    r"(?P<y1>-?(?:\d+(?:\.\d*)?|\.\d+))\Z"
)


def _canonical_decimal(value: Any) -> Decimal | None:
    """Return one exact finite JSON-number identity without float coercion."""
    if isinstance(value, bool):
        return None
    if isinstance(value, Decimal):
        return value if value.is_finite() else None
    if isinstance(value, int):
        return Decimal(value)
    if isinstance(value, float):
        if not math.isfinite(value):
            return None
        # `str` is Python's shortest round-tripping representation of the
        # parsed float.  Comparing it with the Decimal parsed from retained
        # bytes fails closed when json.loads already rounded a longer token.
        return Decimal(str(value))
    return None


def _decimal_identity(value: Decimal) -> str:
    """Stable, exact, non-exponent evidence for one Decimal identity."""
    rendered = format(value, "f")
    if "." in rendered:
        rendered = rendered.rstrip("0").rstrip(".")
    if rendered in {"", "-0"}:
        return "0"
    return rendered


def _exact_json_equal(left: Any, right: Any) -> bool:
    """Recursively compare retained JSON without lossy numeric conversion."""
    left_number = _canonical_decimal(left)
    right_number = _canonical_decimal(right)
    if left_number is not None or right_number is not None:
        return (left_number is not None
                and right_number is not None
                and left_number == right_number)
    if isinstance(left, dict) or isinstance(right, dict):
        return (
            isinstance(left, dict)
            and isinstance(right, dict)
            and set(left) == set(right)
            and all(_exact_json_equal(left[key], right[key]) for key in left)
        )
    if isinstance(left, (list, tuple)) or isinstance(right, (list, tuple)):
        return (
            isinstance(left, (list, tuple))
            and isinstance(right, (list, tuple))
            and len(left) == len(right)
            and all(_exact_json_equal(a, b) for a, b in zip(left, right))
        )
    return type(left) is type(right) and left == right


@dataclasses.dataclass(frozen=True)
class CombOwnerCertificate:
    """Hash-bound reviewed identity for one comb owner, never its topology."""

    page: int
    cell_id: str
    legacy_cell_id: str
    subject_key: str
    bbox: tuple[Decimal, Decimal, Decimal, Decimal]
    state: str
    layout_sha256: str

    def evidence(self) -> dict[str, Any]:
        return {
            "criterion": "exact-reviewed-layout-comb-subject-owner-v1",
            "valid": True,
            "layout_sha256": self.layout_sha256,
            "page": self.page,
            "cell_id": self.cell_id,
            "legacy_cell_id": self.legacy_cell_id,
            "subject_key": self.subject_key,
            "legacy_bbox": [
                _decimal_identity(value) for value in self.bbox
            ],
            "bbox_number_format": "canonical-decimal-string-v1",
            "state": self.state,
            "supplies_topology": False,
        }

    def matches(self, page_index: int, cell: dict[str, Any]) -> bool:
        try:
            cell_id = cell["id"]
            raw_bbox = tuple(
                cell[key] for key in ("x0", "y0", "x1", "y1"))
        except KeyError:
            return False
        canonical_id = (
            CANONICAL_CELL_ID_RE.fullmatch(cell_id)
            if isinstance(cell_id, str) else None
        )
        return (
            page_index == self.page
            and canonical_id is not None
            and int(canonical_id.group(1)) == page_index
            and cell_id == self.cell_id
            and cell.get("subject_key") == self.subject_key
            and _exact_number_vector(raw_bbox, self.bbox)
        )


@dataclasses.dataclass(frozen=True)
class CombOwnerRegistry:
    """Exact-byte layout binding and its identity-only owner certificates."""

    certificates: dict[tuple[int, str], CombOwnerCertificate]
    errors: dict[tuple[int, str], str]
    binding_error: str | None = None

    def resolve(
            self, page_index: int, cell: dict[str, Any],
            ) -> tuple[CombOwnerCertificate | None, str | None]:
        if self.binding_error is not None:
            return None, self.binding_error
        if isinstance(page_index, bool) or not isinstance(page_index, int):
            return None, "comb owner page index is not an integer"
        cell_id = cell.get("id")
        if not isinstance(cell_id, str):
            return None, "comb owner cell has no string id"
        key = (page_index, cell_id)
        certificate = self.certificates.get(key)
        if certificate is None:
            return None, self.errors.get(
                key,
                "no exact unique reviewed comb_subject owns this layout cell",
            )
        if not certificate.matches(page_index, cell):
            return None, (
                "reviewed comb_subject certificate is stale for the active "
                "layout cell identity or bbox"
            )
        return certificate, None


def _exact_number_vector(left: Any, right: Any) -> bool:
    """Numeric JSON equality with no geometry tolerance of any kind."""
    if (not isinstance(left, (list, tuple))
            or not isinstance(right, (list, tuple))
            or len(left) != len(right)):
        return False
    pairs = [
        (_canonical_decimal(a), _canonical_decimal(b))
        for a, b in zip(left, right)
    ]
    return all(a is not None and b is not None and a == b
               for a, b in pairs)


def reviewed_comb_owner_registry(bundle: Any) -> CombOwnerRegistry:
    """Validate the hash-bound layout ledger without reading comb topology.

    The exact retained layout bytes are the authority.  The parsed layout used
    elsewhere in the assertion must still equal those bytes, and the digest
    must be the digest recorded by the input snapshot.  Only identity, state,
    and rectangle fields are inspected below: `cells`, `comb`, `divider_x`,
    `slot_x`, band y, and grey are intentionally outside this certificate.
    """
    payload = getattr(bundle, "layout_payload", None)
    expected_sha = getattr(bundle, "layout_sha256", None)
    parsed_layout = getattr(bundle, "layout", None)
    if not isinstance(payload, bytes) or not isinstance(expected_sha, str):
        return CombOwnerRegistry(
            {}, {},
            "layout comb_subject ownership is not bound to retained bytes",
        )
    actual_sha = hashlib.sha256(payload).hexdigest()
    if expected_sha != actual_sha:
        return CombOwnerRegistry(
            {}, {},
            "retained layout bytes do not match their recorded SHA-256",
        )
    try:
        retained_layout = json.loads(
            payload.decode("utf-8"), parse_float=Decimal)
    except (UnicodeDecodeError, json.JSONDecodeError):
        return CombOwnerRegistry(
            {}, {}, "retained layout bytes are not valid UTF-8 JSON")
    if not _exact_json_equal(retained_layout, parsed_layout):
        return CombOwnerRegistry(
            {}, {},
            "parsed layout is stale relative to its retained hash-bound bytes",
        )
    pages = retained_layout.get("pages") if isinstance(retained_layout, dict) else None
    if not isinstance(pages, list) or not pages:
        return CombOwnerRegistry(
            {}, {}, "hash-bound layout has no exhaustive page list")

    layout_cells: dict[tuple[int, str], dict[str, Any]] = {}
    layout_cells_by_subject: dict[tuple[int, str], dict[str, Any]] = {}
    layout_cell_order: dict[tuple[int, str], int] = {}
    comb_cells: set[tuple[int, str]] = set()
    active_subjects: dict[tuple[int, str], dict[str, Any]] = {}
    cell_ids: set[str] = set()
    cell_subject_keys: set[str] = set()
    subject_cell_ids: set[str] = set()
    subject_keys: set[str] = set()
    legacy_cell_ids: set[str] = set()
    retained_partition_cells: set[tuple[int, str]] = set()
    retained_partition_subjects: set[tuple[int, str]] = set()

    def fail(reason: str) -> CombOwnerRegistry:
        return CombOwnerRegistry({}, {}, reason)

    def identity_bbox(
            subject_key: Any,
            page_index: int,
            bbox: Any,
            label: str,
            ) -> tuple[
                Decimal, Decimal, Decimal, Decimal
            ] | CombOwnerRegistry:
        if not _exact_number_vector(bbox, bbox) or len(bbox) != 4:
            return fail(f"{label} has no exact four-number bbox")
        canonical = tuple(_canonical_decimal(value) for value in bbox)
        if any(value is None for value in canonical):
            return fail(f"{label} has no exact four-number bbox")
        values = canonical
        if values[2] <= values[0] or values[3] <= values[1]:
            return fail(f"{label} bbox has no positive area")
        match = (
            COMB_SUBJECT_KEY_RE.fullmatch(subject_key)
            if isinstance(subject_key, str) else None
        )
        if match is None:
            return fail(f"{label} has a malformed subject_key")
        encoded = [
            Decimal(match.group(name))
            for name in ("x0", "y0", "x1", "y1")
        ]
        if (int(match.group("page")) != page_index
                or not _exact_number_vector(encoded, bbox)):
            return fail(f"{label} subject_key does not encode its exact bbox")
        return values  # type: ignore[return-value]

    # First bind the complete ordered page/cell registry. Subject mappings are
    # validated only after every reverse cell identity is available.
    for expected_page_index, page in enumerate(pages, 1):
        page_value = page.get("index") if isinstance(page, dict) else None
        if (not isinstance(page, dict)
                or isinstance(page_value, bool)
                or not isinstance(page_value, int)
                or page_value != expected_page_index):
            return fail(
                "hash-bound layout pages are not exhaustive and ordered "
                "from index 1")
        page_index = page_value
        raw_cells = page.get("cells")
        raw_subjects = page.get("comb_subjects")
        if not isinstance(raw_cells, list):
            return fail(f"layout page {page_index} has no cell list")
        if not isinstance(raw_subjects, list):
            return fail(
                f"layout page {page_index} has no reviewed comb_subject ledger")
        for cell_order, cell in enumerate(raw_cells):
            if not isinstance(cell, dict) or not isinstance(cell.get("id"), str):
                return fail(
                    f"layout page {page_index} contains a malformed cell")
            cell_id = cell["id"]
            canonical_id = CANONICAL_CELL_ID_RE.fullmatch(cell_id)
            if canonical_id is None or int(canonical_id.group(1)) != page_index:
                return fail(
                    f"layout cell {cell_id} does not identify page {page_index}")
            key = (page_index, cell_id)
            subject_key = cell.get("subject_key")
            cell_bbox = [
                cell.get(name) for name in ("x0", "y0", "x1", "y1")
            ]
            bbox_or_error = identity_bbox(
                subject_key, page_index, cell_bbox,
                f"layout cell {cell_id}")
            if isinstance(bbox_or_error, CombOwnerRegistry):
                return bbox_or_error
            if (key in layout_cells or cell_id in cell_ids
                    or subject_key in cell_subject_keys):
                return fail(
                    "hash-bound layout contains duplicate cell identity")
            layout_cells[key] = cell
            layout_cells_by_subject[(page_index, subject_key)] = cell
            layout_cell_order[key] = cell_order
            cell_ids.add(cell_id)
            cell_subject_keys.add(subject_key)
            comb_value = cell.get("comb")
            if comb_value is not None:
                if not isinstance(comb_value, dict):
                    return fail(
                        f"layout cell {cell_id} has a malformed comb marker")
                comb_cells.add(key)

    # Then validate every subject, including retained/suppressed records. One
    # malformed non-active record invalidates the complete registry; otherwise
    # a corrupt ledger tail could still certify earlier active cells.
    for page in pages:
        page_index = page["index"]
        raw_subjects = page["comb_subjects"]
        for subject in raw_subjects:
            if not isinstance(subject, dict):
                return fail(
                    f"layout page {page_index} contains a malformed comb_subject")
            state = subject.get("state")
            if state not in (*COMB_OWNER_REVIEWED_STATES, "retained_unresolved"):
                return fail(
                    f"layout page {page_index} comb_subject has unknown state")
            cell_id = subject.get("cell_id")
            subject_key = subject.get("subject_key")
            legacy_cell_id = subject.get("legacy_cell_id")
            legacy_bbox = subject.get("legacy_bbox")
            bbox_or_error = identity_bbox(
                subject_key, page_index, legacy_bbox,
                f"layout page {page_index} comb_subject")
            if isinstance(bbox_or_error, CombOwnerRegistry):
                return bbox_or_error
            if not isinstance(legacy_cell_id, str):
                return fail(
                    f"layout page {page_index} comb_subject has no legacy id")
            legacy_canonical = CANONICAL_CELL_ID_RE.fullmatch(legacy_cell_id)
            if (legacy_canonical is None
                    or int(legacy_canonical.group(1)) != page_index):
                return fail(
                    f"comb_subject legacy id does not identify page {page_index}")
            if (subject_key in subject_keys
                    or legacy_cell_id in legacy_cell_ids):
                return fail(
                    "hash-bound layout contains duplicate comb_subject identity")
            subject_keys.add(subject_key)
            legacy_cell_ids.add(legacy_cell_id)

            if state == "retained_unresolved":
                subject_key_set = set(subject)
                if (not RETAINED_COMB_SUBJECT_KEYS <= subject_key_set
                        or subject_key_set - RETAINED_COMB_SUBJECT_KEYS
                        - RETAINED_COMB_SUBJECT_OPTIONAL_KEYS):
                    return fail(
                        "retained_unresolved comb_subject schema is malformed")
                if (cell_id is not None
                        or subject.get("emission") != "suppressed"
                        or subject.get("requires_independent_evidence") is not True
                        or subject.get("blocks_gate") is not True
                        or tuple(subject.get("permitted_transitions") or ())
                        != RETAINED_COMB_TRANSITIONS
                        or not isinstance(subject.get("legacy_comb"), dict)):
                    return fail(
                        "retained_unresolved suppression/blocking/transition "
                        "evidence is incomplete")
                reason_codes_value = subject.get("reason_codes")
                if (not isinstance(reason_codes_value, list)
                        or tuple(reason_codes_value) not in {
                            RETAINED_PARTITION_REASON_CODES,
                            RETAINED_NO_BAND_REASON_CODES,
                        }):
                    return fail(
                        "retained_unresolved suppression reason evidence is "
                        "malformed")
                mapped_ids = subject.get("mapped_partition_cell_ids")
                mapped_keys = subject.get("mapped_partition_subject_keys")
                replacements = subject.get(
                    "erased_edge_replacement_candidates")
                if (not isinstance(mapped_ids, list)
                        or not isinstance(mapped_keys, list)
                        or len(mapped_ids) != len(mapped_keys)
                        or (not mapped_ids and not replacements)
                        or any(not isinstance(value, str)
                               for value in (*mapped_ids, *mapped_keys))
                        or len(mapped_ids) != len(set(mapped_ids))
                        or len(mapped_keys) != len(set(mapped_keys))):
                    return fail(
                        "retained_unresolved partition mapping is malformed")
                mapped_orders: list[int] = []
                for mapped_id, mapped_subject_key in zip(
                        mapped_ids, mapped_keys):
                    mapped_id_match = CANONICAL_CELL_ID_RE.fullmatch(mapped_id)
                    mapped_cell = layout_cells.get((page_index, mapped_id))
                    reverse_cell = layout_cells_by_subject.get(
                        (page_index, mapped_subject_key))
                    if (mapped_id_match is None
                            or int(mapped_id_match.group(1)) != page_index
                            or mapped_cell is None
                            or reverse_cell is not mapped_cell
                            or mapped_cell.get("subject_key")
                            != mapped_subject_key):
                        return fail(
                            "retained_unresolved partition mapping target or "
                            "reverse subject_key mapping is stale")
                    mapped_cell_key = (page_index, mapped_id)
                    mapped_subject_identity = (
                        page_index, mapped_subject_key)
                    if (mapped_cell_key in retained_partition_cells
                            or mapped_subject_identity
                            in retained_partition_subjects):
                        return fail(
                            "retained_unresolved partition mapping target is "
                            "owned more than once")
                    retained_partition_cells.add(mapped_cell_key)
                    retained_partition_subjects.add(mapped_subject_identity)
                    mapped_orders.append(layout_cell_order[mapped_cell_key])
                if mapped_orders != sorted(mapped_orders):
                    return fail(
                        "retained_unresolved partition mapping is not in "
                        "layout cell order")
                retained_cell = layout_cells_by_subject.get(
                    (page_index, subject_key))
                if (retained_cell is not None
                        and retained_cell.get("comb") is not None):
                    return fail(
                        "retained_unresolved subject still owns an active comb")
                if tuple(reason_codes_value) == RETAINED_NO_BAND_REASON_CODES:
                    if (mapped_ids != [legacy_cell_id]
                            or mapped_keys != [subject_key]
                            or retained_cell is None
                            or not _exact_number_vector(
                                legacy_bbox,
                                [retained_cell.get(name) for name in (
                                    "x0", "y0", "x1", "y1")])):
                        return fail(
                            "retained_unresolved no-band identity mapping is "
                            "stale")
                elif retained_cell is not None:
                    return fail(
                        "retained_unresolved partition subject still has a "
                        "layout owner")
                if replacements is not None:
                    if (not isinstance(replacements, list)
                            or not replacements
                            or any(not isinstance(item, dict)
                                   for item in replacements)):
                        return fail(
                            "retained_unresolved replacement identity evidence "
                            "is malformed")
                    for replacement in replacements:
                        candidate_id = replacement.get("cell_id")
                        candidate_key = replacement.get("new_subject_key")
                        candidate_bbox = replacement.get("new_bbox")
                        candidate_cell = (
                            layout_cells.get((page_index, candidate_id))
                            if isinstance(candidate_id, str) else None
                        )
                        if (not isinstance(candidate_key, str)
                                or replacement.get("old_subject_key")
                                != subject_key
                                or not _exact_number_vector(
                                    replacement.get("old_bbox"), legacy_bbox)
                                or replacement.get("blocks_gate") is not True
                                or not isinstance(
                                    replacement.get("activation_blockers"), list)
                                or not replacement.get("activation_blockers")
                                or any(not isinstance(value, str) for value in
                                       replacement["activation_blockers"])
                                or candidate_cell is None
                                or candidate_cell.get("subject_key")
                                != candidate_key
                                or not _exact_number_vector(
                                    candidate_bbox,
                                    [candidate_cell.get(name) for name in (
                                        "x0", "y0", "x1", "y1")])):
                            return fail(
                                "retained_unresolved replacement identity or "
                                "blocking evidence is stale")
                continue
            if not isinstance(cell_id, str):
                return fail("active comb_subject has no string cell_id")
            canonical_id = CANONICAL_CELL_ID_RE.fullmatch(cell_id)
            if canonical_id is None or int(canonical_id.group(1)) != page_index:
                return fail(
                    f"active comb_subject {cell_id} does not identify its page")
            key = (page_index, cell_id)
            if key in active_subjects or cell_id in subject_cell_ids:
                return fail(
                    "active comb_subject cell mapping is not unique")
            if subject.get("mapped_partition_cell_ids") != [cell_id]:
                return fail(
                    "active comb_subject is not a one-to-one cell mapping")
            reason_codes = subject.get("reason_codes")
            if (not isinstance(reason_codes, list)
                    or any(not isinstance(reason, str)
                           for reason in reason_codes)
                    or len(reason_codes) != len(set(reason_codes))
                    or (state == "active_resolved"
                        and (reason_codes or subject.get("blocks_gate") is not False))
                    or (state == "active_unresolved"
                        and (not reason_codes
                             or subject.get("blocks_gate") is not True))):
                return fail(
                    "active comb_subject review/blocking evidence is malformed")
            active_subjects[key] = subject
            subject_cell_ids.add(cell_id)

    orphan_active = sorted(set(active_subjects) - set(layout_cells))
    if orphan_active:
        page_index, cell_id = orphan_active[0]
        return fail(
            f"active comb_subject {cell_id} on page {page_index} is orphaned")
    active_noncomb = sorted(set(active_subjects) - comb_cells)
    if active_noncomb:
        page_index, cell_id = active_noncomb[0]
        return fail(
            f"active comb_subject {cell_id} on page {page_index} owns no comb cell")
    missing_active = sorted(comb_cells - set(active_subjects))
    if missing_active:
        page_index, cell_id = missing_active[0]
        return fail(
            f"comb cell {cell_id} on page {page_index} has no reviewed active "
            "comb_subject")

    certificates: dict[tuple[int, str], CombOwnerCertificate] = {}
    for key in sorted(comb_cells):
        page_index, cell_id = key
        cell = layout_cells[key]
        subject = active_subjects[key]
        state = subject.get("state")
        subject_key = subject.get("subject_key")
        cell_subject_key = cell.get("subject_key")
        legacy_cell_id = subject.get("legacy_cell_id")
        legacy_bbox = subject.get("legacy_bbox")
        cell_bbox = [cell.get(name) for name in ("x0", "y0", "x1", "y1")]
        if (state not in COMB_OWNER_REVIEWED_STATES
                or subject_key != cell_subject_key
                or legacy_cell_id != cell_id
                or not _exact_number_vector(legacy_bbox, cell_bbox)):
            return fail(
                f"active comb_subject {cell_id} identity/bbox is stale")
        bbox_values = tuple(_canonical_decimal(value) for value in legacy_bbox)
        if any(value is None for value in bbox_values):
            return fail(f"active comb_subject {cell_id} bbox is not exact")
        certificates[key] = CombOwnerCertificate(
            page=page_index,
            cell_id=cell_id,
            legacy_cell_id=legacy_cell_id,
            subject_key=subject_key,
            bbox=bbox_values,  # type: ignore[arg-type]
            state=state,
            layout_sha256=actual_sha,
        )
    return CombOwnerRegistry(certificates, {})


def _axis_aligned_quad_box(quad: Any) -> Rect | None:
    points = (quad.ul, quad.ur, quad.ll, quad.lr)
    xs = [float(point.x) for point in points]
    ys = [float(point.y) for point in points]
    x0, x1, y0, y1 = min(xs), max(xs), min(ys), max(ys)
    corners = {
        (round(x0, 6), round(y0, 6)),
        (round(x0, 6), round(y1, 6)),
        (round(x1, 6), round(y0, 6)),
        (round(x1, 6), round(y1, 6)),
    }
    if {(round(x, 6), round(y, 6)) for x, y in zip(xs, ys)} != corners:
        return None
    return x0, y0, x1, y1


def _rect_tuple(rect: Any) -> Rect:
    return tuple(float(value) for value in (rect.x0, rect.y0, rect.x1, rect.y1))


def _rect_intersection(left: Rect, right: Rect) -> Rect | None:
    rect = (max(left[0], right[0]), max(left[1], right[1]),
            min(left[2], right[2]), min(left[3], right[3]))
    return rect if rect[2] > rect[0] and rect[3] > rect[1] else None


def _rects_intersect(left: Rect, right: Rect) -> bool:
    return _rect_intersection(left, right) is not None


@dataclasses.dataclass(frozen=True)
class _DrawingContext:
    clip: Rect | None
    fully_clipped: bool
    unsupported: tuple[tuple[str, Rect], ...]


def _simple_clip_rect(item: dict[str, Any]) -> Rect | None:
    """Return an exact rectangle only when a clip really is one rectangle."""
    parts = item.get("items") or ()
    if len(parts) != 1:
        return None
    part = parts[0]
    if part[0] == "re":
        return _rect_tuple(part[1])
    if part[0] == "qu":
        return _axis_aligned_quad_box(part[1])
    return None


def _drawing_contexts(
        drawings: Sequence[dict[str, Any]],
        bboxlog: Sequence[tuple[str, Any]],
        ) -> dict[int, _DrawingContext]:
    """Resolve PyMuPDF's extended clip/group nesting for every path.

    Extended drawings are a depth-first stream.  A clip or group at level N
    owns following paths at deeper levels until another item at level N (or
    above) replaces it.  Simple rectangular scissors are applied exactly.
    Compound / curved clips and non-normal transparency groups are retained as
    unsupported evidence for every path they can affect.
    """
    stack: list[tuple[int, str, Rect | None, Rect | None, str | None]] = []
    contexts: dict[int, _DrawingContext] = {}

    for item in drawings:
        try:
            level = int(item.get("level", 0))
        except (TypeError, ValueError) as exc:
            raise ValueError("source drawing has a non-integral nesting level") from exc
        while stack and stack[-1][0] >= level:
            stack.pop()

        kind = str(item.get("type") or "")
        if kind == "clip":
            scissor_value = item.get("scissor")
            if scissor_value is None:
                raise ValueError("source clip has no bounded scissor")
            scissor = _rect_tuple(scissor_value)
            exact = _simple_clip_rect(item)
            reason = None if exact is not None else (
                "compound or non-rectilinear source clip")
            stack.append((level, kind, exact, scissor, reason))
            continue
        if kind == "group":
            rect_value = item.get("rect")
            rect = _rect_tuple(rect_value) if rect_value is not None else None
            opacity = float(item.get("opacity")
                            if item.get("opacity") is not None else 1.0)
            blend = str(item.get("blendmode") or "Normal")
            knockout = bool(item.get("knockout"))
            reason = None
            if opacity != 1.0 or blend != "Normal" or knockout:
                reason = "non-normal source transparency group"
            stack.append((level, kind, rect, rect, reason))
            continue

        if kind not in {"f", "s", "fs"}:
            raise ValueError(f"unsupported extended source drawing type {kind!r}")
        seqno = item.get("seqno")
        if not isinstance(seqno, int) or seqno < 0:
            raise ValueError("source drawing has no valid content-stream ordinal")
        if seqno in contexts:
            raise ValueError(f"duplicate source drawing ordinal {seqno}")

        drawing_rect_value = item.get("rect")
        if drawing_rect_value is None:
            raise ValueError("source drawing has no bounded rectangle")
        drawing_rect = _rect_tuple(drawing_rect_value)
        half = (
            float(item.get("width") or 2 * COMB_FALLBACK_HALFWIDTH_PT) / 2
            if kind in {"s", "fs"} else 0.0
        )
        painted_rect = (
            drawing_rect[0] - half,
            drawing_rect[1] - half,
            drawing_rect[2] + half,
            drawing_rect[3] + half,
        )
        bbox_ordinals = (seqno, seqno + 1) if kind == "fs" else (seqno,)
        for ordinal in bbox_ordinals:
            if 0 <= ordinal < len(bboxlog):
                bbox_rect = tuple(float(value) for value in bboxlog[ordinal][1])
                if bbox_rect[2] > bbox_rect[0] and bbox_rect[3] > bbox_rect[1]:
                    painted_rect = (
                        min(painted_rect[0], bbox_rect[0]),
                        min(painted_rect[1], bbox_rect[1]),
                        max(painted_rect[2], bbox_rect[2]),
                        max(painted_rect[3], bbox_rect[3]),
                    )
        clip: Rect | None = None
        unsupported: list[tuple[str, Rect]] = []
        fully_clipped = False
        for _container_level, container_kind, exact, scissor, reason in stack:
            if container_kind == "clip":
                if scissor is None:
                    raise ValueError("source clip lost its bounded scissor")
                affected = _rect_intersection(painted_rect, scissor)
                if affected is None:
                    fully_clipped = True
                    break
                if exact is None:
                    unsupported.append((
                        reason or "unsupported source clip",
                        affected,
                    ))
                    continue
                clip = exact if clip is None else _rect_intersection(clip, exact)
                if clip is None or _rect_intersection(painted_rect, clip) is None:
                    fully_clipped = True
                    break
            elif reason is not None:
                # The extended stream's nesting level, not positive-area
                # rectangle overlap, establishes group ownership. Line-path
                # rectangles can be zero-width/height, and opacity or blend is
                # applied to the nested paint regardless.
                affected = (
                    _rect_intersection(painted_rect, exact)
                    if exact is not None else None
                )
                unsupported.append((reason, affected or painted_rect))
        contexts[seqno] = _DrawingContext(
            clip=clip,
            fully_clipped=fully_clipped,
            unsupported=tuple(sorted(set(unsupported))),
        )

    return contexts


def ordered_vector_paints(page: Any) -> VectorPage:
    """The page's final-paint inputs in exact PDF content-stream order.

    `extended=True` is mandatory: default drawings retain paths that a nested
    clip removes completely.  The drawing `seqno` is the full bbox-log ordinal,
    so later text and images stay ordered relative to vector paint instead of
    disappearing from the compositor.
    """
    try:
        drawings = list(page.get_drawings(extended=True))
        bboxlog = list(page.get_bboxlog())
    except Exception as exc:
        raise ValueError(f"source paint stream is unevaluable: {exc}") from exc
    contexts = _drawing_contexts(drawings, bboxlog)
    try:
        texttrace = list(page.get_texttrace())
    except (AttributeError, RuntimeError, TypeError, ValueError):
        texttrace = []
    text_by_order: dict[
        int, list[tuple[Rect, float | None, float, float | None]]
    ] = (
        collections.defaultdict(list))
    for span in texttrace:
        seqno = span.get("seqno")
        if not isinstance(seqno, int) or seqno < 0:
            continue
        tone = extract.to_gray(span.get("color"))
        opacity = float(span.get("opacity")
                        if span.get("opacity") is not None else 1.0)
        linewidth_value = span.get("linewidth")
        linewidth = (float(linewidth_value)
                     if linewidth_value is not None else None)
        chars = span.get("chars") or ()
        for char in chars:
            if len(char) < 4:
                continue
            rect = tuple(float(value) for value in char[3])
            if rect[2] > rect[0] and rect[3] > rect[1]:
                text_by_order[seqno].append(
                    (rect, tone, opacity, linewidth))

    paints: list[VectorPaint] = []
    unsupported: list[UnsupportedVectorPaint] = []

    def add_rect(rect: Rect, tone: float, opacity: float, ordinal: int,
                 kind: str, operation: int, fill_rule: str = "union",
                 winding: int = 1, clip: Rect | None = None) -> None:
        x0, y0, x1, y1 = (float(value) for value in rect)
        if clip is not None:
            clipped = _rect_intersection((x0, y0, x1, y1), clip)
            if clipped is None:
                return
            x0, y0, x1, y1 = clipped
        if x1 <= x0 or y1 <= y0 or opacity <= 0:
            return
        if not all(math.isfinite(value)
                   for value in (x0, y0, x1, y1, tone, opacity)):
            raise ValueError("source vector paint has a non-finite value")
        if not 0 <= opacity <= 1:
            raise ValueError("source vector paint opacity is outside 0..1")
        paints.append(VectorPaint(
            x0, y0, x1, y1, float(tone), opacity, ordinal, kind,
            operation, fill_rule, winding))

    def add_unsupported_rect(rect: Rect, ordinal: int, reason: str,
                             pad: float = 0.0,
                             clip: Rect | None = None) -> None:
        padded = (rect[0] - pad, rect[1] - pad,
                  rect[2] + pad, rect[3] + pad)
        if clip is not None:
            clipped = _rect_intersection(padded, clip)
            if clipped is None:
                return
            padded = clipped
        if padded[2] <= padded[0] or padded[3] <= padded[1]:
            return
        if not all(math.isfinite(value) for value in padded):
            raise ValueError(f"{reason} has a non-finite source rectangle")
        unsupported.append(UnsupportedVectorPaint(padded, ordinal, reason))

    def add_unsupported(drawing: dict[str, Any], ordinal: int,
                        reason: str, pad: float = 0.0,
                        clip: Rect | None = None) -> None:
        rect_value = drawing.get("rect")
        if rect_value is None:
            raise ValueError(f"{reason} has no bounded source rectangle")
        add_unsupported_rect(_rect_tuple(rect_value), ordinal, reason, pad, clip)

    def expect_bbox_kind(ordinal: int, wanted: str) -> None:
        if not 0 <= ordinal < len(bboxlog):
            raise ValueError(
                f"source drawing ordinal {ordinal} is outside the bbox log")
        if str(bboxlog[ordinal][0]) != wanted:
            raise ValueError(
                f"source drawing ordinal {ordinal} is {bboxlog[ordinal][0]!r}, "
                f"expected {wanted!r}")

    for drawing in drawings:
        drawing_type = str(drawing.get("type") or "")
        if drawing_type in {"clip", "group"}:
            continue
        seqno = int(drawing["seqno"])
        context = contexts[seqno]
        if context.fully_clipped:
            continue

        fill_order = -1
        stroke_order = -1
        if drawing_type == "f":
            fill_order = seqno
            expect_bbox_kind(fill_order, "fill-path")
        elif drawing_type == "s":
            stroke_order = seqno
            expect_bbox_kind(stroke_order, "stroke-path")
        elif drawing_type == "fs":
            fill_order = seqno
            stroke_order = seqno + 1
            expect_bbox_kind(fill_order, "fill-path")
            expect_bbox_kind(stroke_order, "stroke-path")
        else:
            raise ValueError(
                f"unsupported extended source drawing type {drawing_type!r}")

        if context.unsupported:
            for reason, rect in context.unsupported:
                add_unsupported_rect(
                    rect, seqno, reason, clip=context.clip)
            continue

        fill_colour = drawing.get("fill")
        stroke_colour = drawing.get("color")
        fill_tone = extract.to_gray(fill_colour)
        stroke_tone = extract.to_gray(stroke_colour)
        fill_opacity = float(drawing.get("fill_opacity")
                             if drawing.get("fill_opacity") is not None else 1.0)
        stroke_opacity = float(drawing.get("stroke_opacity")
                               if drawing.get("stroke_opacity") is not None else 1.0)
        stroke_width = float(drawing.get("width") or
                             2 * COMB_FALLBACK_HALFWIDTH_PT)
        half = stroke_width / 2

        if fill_colour is not None and fill_tone is None:
            add_unsupported(
                drawing, fill_order, "chromatic vector fill", clip=context.clip)
        elif fill_order >= 0 and fill_tone is not None:
            fill_supported = True
            fill_regions: list[tuple[Rect, int]] = []
            parts = drawing.get("items") or ()
            for item in drawing["items"]:
                if item[0] == "re":
                    rect = item[1]
                    winding = int(item[2]) if len(item) > 2 else 1
                    if winding == 0:
                        fill_supported = False
                    else:
                        fill_regions.append((_rect_tuple(rect), winding))
                elif item[0] == "qu":
                    box = _axis_aligned_quad_box(item[1])
                    if box is None or len(parts) != 1:
                        fill_supported = False
                    else:
                        fill_regions.append((box, 1))
                else:
                    fill_supported = False
            if not fill_supported or not fill_regions:
                add_unsupported(drawing, fill_order,
                                "non-rectilinear or unbounded vector fill",
                                clip=context.clip)
            else:
                fill_rule = ("evenodd" if bool(drawing.get("even_odd"))
                             else "nonzero")
                for rect, winding in fill_regions:
                    add_rect(
                        rect, fill_tone, fill_opacity, fill_order,
                        "fill-region", fill_order, fill_rule, winding,
                        context.clip)

        if stroke_colour is not None and stroke_tone is None:
            add_unsupported(
                drawing, stroke_order, "chromatic vector stroke", half,
                context.clip)
        elif stroke_order >= 0 and stroke_tone is not None:
            stroke_supported = True
            stroke_regions: list[Rect] = []
            dashes = str(drawing.get("dashes") or "[] 0").strip()
            if dashes not in ("", "[] 0"):
                stroke_supported = False
            for item in drawing["items"]:
                op = item[0]
                if op == "re":
                    rect = item[1]
                    stroke_regions.extend((
                        (rect.x0 - half, rect.y0 - half,
                         rect.x1 + half, rect.y0 + half),
                        (rect.x0 - half, rect.y1 - half,
                         rect.x1 + half, rect.y1 + half),
                        (rect.x0 - half, rect.y0 + half,
                         rect.x0 + half, rect.y1 - half),
                        (rect.x1 - half, rect.y0 + half,
                         rect.x1 + half, rect.y1 - half),
                    ))
                elif op == "l":
                    p0, p1 = item[1], item[2]
                    dx, dy = abs(float(p1.x) - float(p0.x)), abs(float(p1.y) - float(p0.y))
                    if dx <= verify.DEFAULT_POSITION_TOL_PT:
                        stroke_regions.append((
                            min(p0.x, p1.x) - half,
                            min(p0.y, p1.y) - half,
                            max(p0.x, p1.x) + half,
                            max(p0.y, p1.y) + half,
                        ))
                    elif dy <= verify.DEFAULT_POSITION_TOL_PT:
                        stroke_regions.append((
                            min(p0.x, p1.x) - half,
                            min(p0.y, p1.y) - half,
                            max(p0.x, p1.x) + half,
                            max(p0.y, p1.y) + half,
                        ))
                    else:
                        stroke_supported = False
                elif op == "qu":
                    box = _axis_aligned_quad_box(item[1])
                    if box is None:
                        stroke_supported = False
                    else:
                        x0, y0, x1, y1 = box
                        stroke_regions.extend((
                            (x0 - half, y0 - half, x1 + half, y0 + half),
                            (x0 - half, y1 - half, x1 + half, y1 + half),
                            (x0 - half, y0 + half, x0 + half, y1 - half),
                            (x1 - half, y0 + half, x1 + half, y1 - half),
                        ))
                else:
                    stroke_supported = False
            if not stroke_supported:
                add_unsupported(drawing, stroke_order,
                                "non-rectilinear vector stroke", half,
                                context.clip)
            else:
                for rect in stroke_regions:
                    add_rect(
                        rect, stroke_tone, stroke_opacity, stroke_order,
                        "stroke-region", stroke_order, "union", 1,
                        context.clip)

    for ordinal, (kind, rect_value) in enumerate(bboxlog):
        if kind not in {"fill-image", "fill-text", "stroke-text"}:
            continue
        rect = tuple(float(value) for value in rect_value)
        if rect[2] <= rect[0] or rect[3] <= rect[1]:
            continue
        if kind in {"fill-text", "stroke-text"}:
            traced = text_by_order.get(ordinal) or ()
            tones = {tone for _char_rect, tone, _opacity, _width in traced}
            opacities = {
                opacity for _char_rect, _tone, opacity, _width in traced
            }
            tone = next(iter(tones)) if len(tones) == 1 else None
            opacity = (next(iter(opacities))
                       if len(opacities) == 1 else None)
            trace_rects = tuple(
                char_rect for char_rect, _tone, _opacity, _width in traced)
            if kind == "stroke-text":
                widths = {
                    width for _rect, _tone, _opacity, width in traced
                    if width is not None
                }
                half = (max(widths) / 2 if widths
                        else COMB_FALLBACK_HALFWIDTH_PT)
                rect = (rect[0] - half, rect[1] - half,
                        rect[2] + half, rect[3] + half)
                trace_rects = tuple(
                    (char_rect[0] - half, char_rect[1] - half,
                     char_rect[2] + half, char_rect[3] + half)
                    for char_rect in trace_rects
                )
            unsupported.append(UnsupportedVectorPaint(
                rect, ordinal, f"unmodeled source {kind} paint",
                tone, opacity, trace_rects))
            continue
        unsupported.append(UnsupportedVectorPaint(
            rect, ordinal, f"unmodeled source {kind} paint"))

    paints.sort(key=lambda paint: (
        paint.order, paint.operation, paint.kind,
        paint.x0, paint.y0, paint.x1, paint.y1,
        paint.fill_rule, paint.winding))
    unsupported.sort(key=lambda paint: (paint.order, paint.reason, paint.rect))
    return VectorPage(tuple(paints), tuple(unsupported))


def _same_topology(left: Sequence[float], right: Sequence[float]) -> bool:
    return (len(left) == len(right)
            and all(abs(a - b) <= COMB_MERGE_PT
                    for a, b in zip(left, right)))


def _topology_subset(left: Sequence[float], right: Sequence[float]) -> bool:
    """Strict subset under a sorted, monotone, one-to-one divider match."""
    if len(left) >= len(right):
        return False
    candidates = sorted(float(value) for value in right)
    cursor = 0
    for value in sorted(float(value) for value in left):
        while (cursor < len(candidates)
                and candidates[cursor] < value - COMB_MERGE_PT):
            cursor += 1
        if (cursor >= len(candidates)
                or candidates[cursor] > value + COMB_MERGE_PT):
            return False
        cursor += 1
    return True


def _merge_intervals(
        intervals: Sequence[tuple[float, float]], gap: float,
        ) -> list[tuple[float, float]]:
    merged: list[tuple[float, float]] = []
    for left, right in sorted(intervals):
        if merged and left <= merged[-1][1] + gap:
            merged[-1] = (merged[-1][0], max(merged[-1][1], right))
        else:
            merged.append((left, right))
    return merged


def _operation_covers(regions: Sequence[VectorPaint],
                      x: float, y: float) -> bool:
    hits = [region for region in regions if region.covers(x, y)]
    if not hits:
        return False
    rules = {region.fill_rule for region in regions}
    if len(rules) != 1:
        raise ValueError("one source paint operation has conflicting fill rules")
    rule = next(iter(rules))
    if rule == "union":
        return True
    if rule == "evenodd":
        return len(hits) % 2 == 1
    if rule == "nonzero":
        return sum(region.winding for region in hits) != 0
    raise ValueError(f"unknown source fill rule {rule!r}")


def _final_tone_and_owner(
        active: Sequence[VectorPaint], x: float, y: float,
        ) -> tuple[float, tuple[VectorPaint, ...]]:
    operations: dict[tuple[int, int], list[VectorPaint]] = (
        collections.defaultdict(list))
    for region in active:
        operations[(region.order, region.operation)].append(region)

    tone = 1.0
    owner: tuple[VectorPaint, ...] = ()
    for key in sorted(operations):
        regions = operations[key]
        if not _operation_covers(regions, x, y):
            continue
        tones = {region.tone for region in regions}
        opacities = {region.opacity for region in regions}
        if len(tones) != 1 or len(opacities) != 1:
            raise ValueError(
                "one source paint operation has conflicting tone or opacity")
        opacity = next(iter(opacities))
        tone = opacity * next(iter(tones)) + (1 - opacity) * tone
        owner = tuple(region for region in regions if region.covers(x, y))
    return tone, owner


def _final_tone(active: Sequence[VectorPaint], x: float, y: float) -> float:
    return _final_tone_and_owner(active, x, y)[0]


def _is_comb_vertical(paint: VectorPaint) -> bool:
    """A source stroke with material, not epsilon-only, vertical anisotropy."""
    width = paint.x1 - paint.x0
    height = paint.y1 - paint.y0
    return (
        width <= COMB_MAX_WIDTH_PT
        and height >= COMB_MINLEN_PT
        and height - width >= COMB_MINLEN_PT
    )


def _source_band_candidates(
        page: VectorPage, cell: Rect
        ) -> tuple[list[tuple[float, float]], int | None]:
    """Bands proposed only by narrow source paint near the cell."""
    x0, y0, x1, y1 = cell
    seeds: list[tuple[float, float, float, int]] = []
    for paint in page.paints:
        width = paint.x1 - paint.x0
        height = paint.y1 - paint.y0
        centre = (paint.x0 + paint.x1) / 2
        if (_is_comb_vertical(paint)
                and centre > x0 + COMB_EDGE_PT
                and centre < x1 - COMB_EDGE_PT
                and paint.y1 >= y0
                and paint.y0 <= y1):
            seeds.append((centre, paint.y0, paint.y1, paint.order))

    bands = {(a, b) for _x, a, b, _order in seeds
             if b - a >= COMB_MINLEN_PT}
    by_x = sorted(seeds)
    clusters: list[list[tuple[float, float, float, int]]] = []
    for seed in by_x:
        if clusters and seed[0] - clusters[-1][-1][0] <= COMB_MERGE_PT:
            clusters[-1].append(seed)
        else:
            clusters.append([seed])
    for cluster in clusters:
        intervals = sorted((a, b) for _x, a, b, _order in cluster)
        if not intervals:
            continue
        start, end = intervals[0]
        for a, b in intervals[1:]:
            if a <= end + COMB_YSLACK_PT:
                end = max(end, b)
            else:
                if end - start >= COMB_MINLEN_PT:
                    bands.add((start, end))
                start, end = a, b
        if end - start >= COMB_MINLEN_PT:
            bands.add((start, end))
    first_order = min((order for _x, _a, _b, order in seeds), default=None)
    return sorted(bands), first_order


def _band_topologies(page: VectorPage, x0: float, x1: float,
                     y0: float, y1: float
                     ) -> list[tuple[float, tuple[float, ...]]]:
    paints = [
        paint for paint in page.paints
        if paint.x1 > x0 and paint.x0 < x1
        and paint.y1 > y0 and paint.y0 < y1
    ]
    endpoints = {y0, y1}
    for paint in paints:
        endpoints.update((max(y0, paint.y0), min(y1, paint.y1)))
    ordered_y = sorted(endpoints)
    slabs: list[
        tuple[float, float, dict[float, tuple[tuple[float, float], ...]]]
    ] = []

    for a, b in zip(ordered_y, ordered_y[1:]):
        span = b - a
        if span <= verify.DEFAULT_POSITION_TOL_PT:
            continue
        mid_y = (a + b) / 2
        active = [paint for paint in paints if paint.y0 <= mid_y <= paint.y1]
        x_edges = {x0, x1}
        for paint in active:
            x_edges.update((max(x0, paint.x0), min(x1, paint.x1)))
        ordered_x = sorted(x_edges)
        intervals: list[tuple[float, float, float]] = []
        for left, right in zip(ordered_x, ordered_x[1:]):
            if right <= left:
                continue
            tone = round(_final_tone(active, (left + right) / 2, mid_y), 4)
            if intervals and intervals[-1][2] == tone:
                intervals[-1] = (intervals[-1][0], right, tone)
            else:
                intervals.append((left, right, tone))

        # A divider is a narrow final-paint contrast corridor, not necessarily
        # non-white ink. A white knockout through a grey band is just as visible
        # as a black rule through white paper. Keeping tones separate makes a
        # mixed-tone topology fail closed below rather than silently dropping
        # one of its visible boundaries.
        by_tone: dict[float, list[tuple[float, float]]] = (
            collections.defaultdict(list))
        for index, (left, right, tone) in enumerate(intervals):
            if (index == 0 or index == len(intervals) - 1
                    or right - left > COMB_MAX_WIDTH_PT
                    or left <= x0 + COMB_EDGE_PT
                    or right >= x1 - COMB_EDGE_PT):
                continue
            left_tone = intervals[index - 1][2]
            right_tone = intervals[index + 1][2]
            if tone == left_tone or tone == right_tone:
                continue
            centre = (left + right) / 2
            final_tone, owners = _final_tone_and_owner(
                active, centre, mid_y)
            if (round(final_tone, 4) != tone
                    or not any(
                        owner.covers(centre, mid_y)
                        and _is_comb_vertical(owner)
                        for owner in owners
                    )):
                # A final-tone seam with no narrow vertical source operation
                # is paper between paints, not a divider. This applies before
                # every topology verdict, including the single-choice path.
                continue
            by_tone[tone].append((
                round(centre, 6),
                right - left,
            ))
        slabs.append((
            a,
            b,
            {tone: tuple(components)
             for tone, components in sorted(by_tone.items())},
        ))

    out: list[tuple[float, tuple[float, ...]]] = []
    full_span = y1 - y0
    tones = sorted({
        tone for _a, _b, slab_tones in slabs for tone in slab_tones
    })
    for tone in tones:
        observations = [
            (centre, width, b - a)
            for a, b, slab_tones in slabs
            for centre, width in slab_tones.get(tone, ())
        ]
        centre_clusters: list[list[tuple[float, float, float]]] = []
        for observation in sorted(observations):
            if (centre_clusters
                    and observation[0] - centre_clusters[-1][-1][0]
                    <= COMB_MERGE_PT):
                centre_clusters[-1].append(observation)
            else:
                centre_clusters.append([observation])

        stable: list[float] = []
        for cluster in centre_clusters:
            weight = sum(span for _centre, _width, span in cluster)
            anchor = (
                sum(centre * span for centre, _width, span in cluster) / weight
                if weight else cluster[0][0]
            )
            longest = current = 0.0
            run_width = max_run_width = 0.0
            previous_end: float | None = None
            for a, b, slab_tones in slabs:
                matches = [
                    width for centre, width in slab_tones.get(tone, ())
                    if abs(centre - anchor) <= COMB_MERGE_PT
                ]
                if (matches
                        and (previous_end is None
                             or a - previous_end
                             <= verify.DEFAULT_POSITION_TOL_PT)):
                    current += b - a
                    run_width = max(run_width, max(matches))
                elif matches:
                    current = b - a
                    run_width = max(matches)
                else:
                    current = 0.0
                    run_width = 0.0
                if current > longest:
                    longest = current
                    max_run_width = run_width
                previous_end = b
            # Strict-majority *continuous* evidence prevents disconnected dots
            # from summing into a divider. Requiring its run to be taller than
            # its widest final component rejects a square even when the square
            # happens to occupy most of a short proposed band.
            if (longest > full_span / 2
                    and longest >= COMB_MINLEN_PT
                    and longest - max_run_width >= COMB_MINLEN_PT):
                stable.append(round(anchor, 6))

        if not stable:
            continue

        longest_common = current_common = 0.0
        previous_end = None
        for a, b, slab_tones in slabs:
            components = slab_tones.get(tone, ())
            all_present = all(
                any(abs(centre - anchor) <= COMB_MERGE_PT
                    for centre, _width in components)
                for anchor in stable
            )
            if (all_present
                    and (previous_end is None
                         or a - previous_end
                         <= verify.DEFAULT_POSITION_TOL_PT)):
                current_common += b - a
            elif all_present:
                current_common = b - a
            else:
                current_common = 0.0
            longest_common = max(longest_common, current_common)
            previous_end = b
        if (longest_common > full_span / 2
                and longest_common >= COMB_MINLEN_PT):
            out.append((tone, tuple(stable)))
    return out


def _source_paint_evidence(paint: VectorPaint) -> dict[str, Any]:
    """Serialize one final-paint owner without losing source lineage."""
    width = paint.x1 - paint.x0
    height = paint.y1 - paint.y0
    if width > height:
        orientation = "horizontal"
    elif height > width:
        orientation = "vertical"
    else:
        orientation = "square"
    return {
        "order": paint.order,
        "operation": paint.operation,
        "kind": paint.kind,
        "tone": round(paint.tone, 6),
        "opacity": round(paint.opacity, 6),
        "orientation": orientation,
        "rect": [
            round(paint.x0, 6),
            round(paint.y0, 6),
            round(paint.x1, 6),
            round(paint.y1, 6),
        ],
        "width_pt": round(width, 6),
        "height_pt": round(height, 6),
    }


def _vertical_lineage_diagnostics(
        page: VectorPage, x0: float, x1: float, y0: float, y1: float,
        ) -> list[dict[str, Any]]:
    """Explain why raw same-x source strokes lack one continuous final run.

    This is evidence only: it never promotes a topology. Each slab is owned
    only when the final source operation at that x is itself a narrow vertical.
    Later orthogonal paint therefore appears as an explicit interruption rather
    than being silently stitched into a divider.
    """
    candidates = [
        paint for paint in page.paints
        if paint.x1 > x0 and paint.x0 < x1
        and paint.y1 > y0 and paint.y0 < y1
        and _is_comb_vertical(paint)
        and (paint.x0 + paint.x1) / 2 > x0 + COMB_EDGE_PT
        and (paint.x0 + paint.x1) / 2 < x1 - COMB_EDGE_PT
    ]
    clusters: list[list[VectorPaint]] = []
    for paint in sorted(
            candidates,
            key=lambda item: (
                (item.x0 + item.x1) / 2,
                item.y0, item.y1, item.order, item.operation,
            )):
        centre = (paint.x0 + paint.x1) / 2
        prior_centre = (
            (clusters[-1][-1].x0 + clusters[-1][-1].x1) / 2
            if clusters else None
        )
        if (prior_centre is not None
                and centre - prior_centre <= COMB_MERGE_PT):
            clusters[-1].append(paint)
        else:
            clusters.append([paint])

    relevant = [
        paint for paint in page.paints
        if paint.x1 > x0 and paint.x0 < x1
        and paint.y1 > y0 and paint.y0 < y1
    ]
    endpoints = {y0, y1}
    for paint in relevant:
        endpoints.update((max(y0, paint.y0), min(y1, paint.y1)))
    ordered_y = sorted(endpoints)
    slabs = [
        (a, b) for a, b in zip(ordered_y, ordered_y[1:])
        if b - a > SOURCE_COORD_EPS_PT
    ]
    full_span = y1 - y0
    diagnostics: list[dict[str, Any]] = []

    for cluster in clusters:
        anchor_paint = min(
            cluster,
            key=lambda paint: (
                -max(0.0, min(y1, paint.y1) - max(y0, paint.y0)),
                (paint.x0 + paint.x1) / 2,
                paint.order,
                paint.operation,
            ),
        )
        anchor = (anchor_paint.x0 + anchor_paint.x1) / 2
        cluster_left = min(paint.x0 for paint in cluster)
        cluster_right = max(paint.x1 for paint in cluster)
        slab_evidence: list[dict[str, Any]] = []
        for a, b in slabs:
            mid_y = (a + b) / 2
            active = [
                paint for paint in relevant
                if paint.y0 <= mid_y <= paint.y1
            ]
            active_members = [
                paint for paint in cluster
                if paint.y0 <= mid_y <= paint.y1
            ]
            sample_xs = sorted({
                (paint.x0 + paint.x1) / 2 for paint in active_members
            } or {anchor}, key=lambda value: (abs(value - anchor), value))
            x_edges = {x0, x1}
            for paint in active:
                x_edges.update((max(x0, paint.x0), min(x1, paint.x1)))
            intervals: list[tuple[float, float, float]] = []
            for left, right in zip(sorted(x_edges), sorted(x_edges)[1:]):
                if right <= left:
                    continue
                tone = round(
                    _final_tone(active, (left + right) / 2, mid_y), 4)
                if intervals and intervals[-1][2] == tone:
                    intervals[-1] = (intervals[-1][0], right, tone)
                else:
                    intervals.append((left, right, tone))

            samples: list[dict[str, Any]] = []
            for sample_x in sample_xs:
                final_tone, owners = _final_tone_and_owner(
                    active, sample_x, mid_y)
                owned = any(
                    owner.covers(sample_x, mid_y)
                    and _is_comb_vertical(owner)
                    and abs(
                        (owner.x0 + owner.x1) / 2 - anchor
                    ) <= COMB_MERGE_PT
                    for owner in owners
                )
                containing = [
                    (index, interval)
                    for index, interval in enumerate(intervals)
                    if (interval[0] - SOURCE_COORD_EPS_PT <= sample_x
                        <= interval[1] + SOURCE_COORD_EPS_PT)
                ]
                if containing:
                    interval_index, corridor = min(
                        containing,
                        key=lambda item: (
                            abs((item[1][0] + item[1][1]) / 2 - sample_x),
                            item[1][1] - item[1][0],
                            item[0],
                        ),
                    )
                    left, right, corridor_tone = corridor
                    left_tone = (
                        intervals[interval_index - 1][2]
                        if interval_index > 0 else None
                    )
                    right_tone = (
                        intervals[interval_index + 1][2]
                        if interval_index + 1 < len(intervals) else None
                    )
                    visible = (
                        owned
                        and right - left <= COMB_MAX_WIDTH_PT
                        and left > x0 + COMB_EDGE_PT
                        and right < x1 - COMB_EDGE_PT
                        and left_tone is not None
                        and right_tone is not None
                        and corridor_tone != left_tone
                        and corridor_tone != right_tone
                    )
                    corridor_evidence: dict[str, Any] | None = {
                        "x0": round(left, 6),
                        "x1": round(right, 6),
                        "width_pt": round(right - left, 6),
                        "tone": corridor_tone,
                        "left_tone": left_tone,
                        "right_tone": right_tone,
                    }
                else:
                    visible = False
                    corridor_evidence = None
                samples.append({
                    "visible": visible,
                    "owned": owned,
                    "sample_x": sample_x,
                    "final_tone": final_tone,
                    "owners": owners,
                    "corridor": corridor_evidence,
                })
            selected = next(
                (sample for sample in samples if sample["visible"]),
                next(
                    (sample for sample in samples if sample["owned"]),
                    samples[0],
                ),
            )
            owned = bool(selected["owned"])
            visible = bool(selected["visible"])
            sample_x = float(selected["sample_x"])
            final_tone = float(selected["final_tone"])
            owners = selected["owners"]

            surrounding_samples = []
            for side, probe_x in (
                    ("left", max(
                        x0 + SOURCE_COORD_EPS_PT,
                        cluster_left - 2 * SOURCE_COORD_EPS_PT,
                    )),
                    ("right", min(
                        x1 - SOURCE_COORD_EPS_PT,
                        cluster_right + 2 * SOURCE_COORD_EPS_PT,
                    ))):
                probe_tone, probe_owners = _final_tone_and_owner(
                    active, probe_x, mid_y)
                surrounding_samples.append({
                    "side": side,
                    "x": round(probe_x, 6),
                    "final_tone": round(probe_tone, 6),
                    "last_owners": [
                        _source_paint_evidence(owner)
                        for owner in sorted(
                            probe_owners,
                            key=lambda item: (
                                item.order, item.operation, item.kind,
                                item.x0, item.y0, item.x1, item.y1,
                            ),
                        )
                    ],
                })
            orthogonal_owners: list[dict[str, Any]] = []
            seen_orthogonal: set[
                tuple[int, int, float, float, float, float]
            ] = set()
            for sample in surrounding_samples:
                if (round(float(sample["final_tone"]), 4)
                        != round(final_tone, 4)):
                    continue
                for owner_evidence in sample["last_owners"]:
                    if owner_evidence["orientation"] != "horizontal":
                        continue
                    rect = owner_evidence["rect"]
                    owner_key = (
                        int(owner_evidence["order"]),
                        int(owner_evidence["operation"]),
                        float(rect[0]), float(rect[1]),
                        float(rect[2]), float(rect[3]),
                    )
                    if owner_key not in seen_orthogonal:
                        seen_orthogonal.add(owner_key)
                        orthogonal_owners.append(owner_evidence)
            if visible:
                interruption_cause = None
            elif not owned:
                interruption_cause = "final-owner-not-narrow-vertical"
            else:
                interruption_cause = "no-narrow-final-tone-contrast-corridor"
            slab_evidence.append({
                "y0": round(a, 6),
                "y1": round(b, 6),
                "span_pt": round(b - a, 6),
                "source_present": bool(active_members),
                "owned_by_narrow_vertical": owned,
                "visible_narrow_corridor": visible,
                "interruption_cause": interruption_cause,
                "sample_x": round(sample_x, 6),
                "final_tone": round(final_tone, 6),
                "corridor": selected["corridor"],
                "last_owners": [
                    _source_paint_evidence(owner)
                    for owner in sorted(
                        owners,
                        key=lambda item: (
                            item.order, item.operation, item.kind,
                            item.x0, item.y0, item.x1, item.y1,
                        ),
                    )
                ],
                "surrounding_samples": surrounding_samples,
                "orthogonal_same_tone_owners": orthogonal_owners,
            })

        continuous_runs: list[list[float]] = []
        interruptions: list[list[float]] = []
        for slab in slab_evidence:
            target = (
                continuous_runs
                if slab["visible_narrow_corridor"] else interruptions
            )
            a = float(slab["y0"])
            b = float(slab["y1"])
            if (target
                    and abs(target[-1][1] - a) <= SOURCE_COORD_EPS_PT):
                target[-1][1] = b
            else:
                target.append([a, b])
        if not continuous_runs:
            continue
        covered = sum(b - a for a, b in continuous_runs)
        longest = max(b - a for a, b in continuous_runs)
        max_source_width = max(
            paint.x1 - paint.x0 for paint in cluster)
        diagnostics.append({
            "x": round(anchor, 6),
            "band_y0": round(y0, 6),
            "band_y1": round(y1, 6),
            "band_span_pt": round(full_span, 6),
            "source_segments": [
                _source_paint_evidence(paint)
                for paint in sorted(
                    cluster,
                    key=lambda item: (
                        item.y0, item.y1, item.order, item.operation,
                        item.x0, item.x1,
                    ),
                )
            ],
            "continuous_runs": continuous_runs,
            "interruptions": interruptions,
            "interruption_segments": [
                slab for slab in slab_evidence
                if not slab["visible_narrow_corridor"]
            ],
            "covered_pt": round(covered, 6),
            "longest_run_pt": round(longest, 6),
            "strict_majority": (
                longest > full_span / 2
                and longest >= COMB_MINLEN_PT
                and longest > max_source_width
            ),
        })
    return sorted(diagnostics, key=lambda item: item["x"])


def _stable_source_verticals(
        page: VectorPage, x0: float, x1: float, y0: float, y1: float,
        tone: float,
        ) -> list[float]:
    """Continuous final-tone verticals with an owning source operation."""
    candidates = [
        paint for paint in page.paints
        if paint.x1 > x0 and paint.x0 < x1
        and paint.y1 > y0 and paint.y0 < y1
        and _is_comb_vertical(paint)
    ]
    clusters: list[list[VectorPaint]] = []
    for paint in sorted(
            candidates,
            key=lambda item: ((item.x0 + item.x1) / 2, item.y0, item.y1)):
        centre = (paint.x0 + paint.x1) / 2
        if (clusters
                and centre - (
                    clusters[-1][-1].x0 + clusters[-1][-1].x1
                ) / 2 <= COMB_MERGE_PT):
            clusters[-1].append(paint)
        else:
            clusters.append([paint])

    relevant = [
        paint for paint in page.paints
        if paint.x1 > x0 and paint.x0 < x1
        and paint.y1 > y0 and paint.y0 < y1
    ]
    endpoints = {y0, y1}
    for paint in relevant:
        endpoints.update((max(y0, paint.y0), min(y1, paint.y1)))
    slabs = [
        (a, b) for a, b in zip(sorted(endpoints), sorted(endpoints)[1:])
        if b - a > SOURCE_COORD_EPS_PT
    ]
    full_span = y1 - y0
    wanted_tone = round(tone, 4)
    stable: list[float] = []
    for cluster in clusters:
        weights = [max(0.0, min(y1, paint.y1) - max(y0, paint.y0))
                   for paint in cluster]
        weight = sum(weights)
        anchor = (
            sum(((paint.x0 + paint.x1) / 2) * span
                for paint, span in zip(cluster, weights)) / weight
            if weight else (cluster[0].x0 + cluster[0].x1) / 2
        )
        longest = current = 0.0
        for a, b in slabs:
            mid_y = (a + b) / 2
            active = [
                paint for paint in relevant
                if paint.y0 <= mid_y <= paint.y1
            ]
            owned = False
            for member in active:
                width = member.x1 - member.x0
                height = member.y1 - member.y0
                centre = (member.x0 + member.x1) / 2
                if (not _is_comb_vertical(member)
                        or abs(centre - anchor) > COMB_MERGE_PT):
                    continue
                final_tone, owners = _final_tone_and_owner(
                    active, centre, mid_y)
                if (round(final_tone, 4) == wanted_tone
                        and any(
                            owner.covers(centre, mid_y)
                            and _is_comb_vertical(owner)
                            for owner in owners
                        )):
                    owned = True
                    break
            if owned:
                current += b - a
                longest = max(longest, current)
            else:
                current = 0.0
        if (longest >= full_span - COMB_YSLACK_PT
                and longest >= COMB_MINLEN_PT):
            stable.append(round(anchor, 6))
    return stable


def _source_vertical_ink_geometry(
        page: VectorPage,
        centre_x: float,
        band_y0: float,
        band_y1: float,
        tone: float,
        ) -> dict[str, Any] | None:
    """Raw painted x extent supporting one stable vertical lineage."""
    wanted_tone = round(tone, 4)
    members = [
        paint for paint in page.paints
        if _is_comb_vertical(paint)
        and round(paint.tone, 4) == wanted_tone
        and paint.y1 > band_y0
        and paint.y0 < band_y1
        and abs((paint.x0 + paint.x1) / 2.0 - centre_x)
        <= COMB_MERGE_PT
    ]
    if not members:
        return None
    return {
        "center_x": float(centre_x),
        "ink_x0": min(paint.x0 for paint in members),
        "ink_x1": max(paint.x1 for paint in members),
        "ink_y0": min(paint.y0 for paint in members),
        "ink_y1": max(paint.y1 for paint in members),
        "members": tuple(members),
        "paint_rects": [
            [paint.x0, paint.y0, paint.x1, paint.y1]
            for paint in sorted(
                members,
                key=lambda item: (
                    item.x0, item.y0, item.x1, item.y1,
                    item.order, item.operation),
            )
        ],
    }


def _baseline_segments(
        baseline: _SourceBaselineSpan,
        ) -> tuple[tuple[float, float, float, float], ...]:
    if baseline.segments:
        return baseline.segments
    return ((baseline.left, baseline.right, baseline.y0, baseline.y1),)


def _baseline_contact_segments(
        baseline: _SourceBaselineSpan,
        contact_x: float,
        ) -> tuple[_SourceBaselineSpan, ...]:
    """Retain the actual baseline segment levels touching one endpoint."""
    return tuple(
        _SourceBaselineSpan(
            y=(segment_y0 + segment_y1) / 2.0,
            y0=segment_y0,
            y1=segment_y1,
            left=segment_left,
            right=segment_right,
            operations=baseline.operations,
            segments=(
                (segment_left, segment_right, segment_y0, segment_y1),
            ),
        )
        for (
            segment_left, segment_right, segment_y0, segment_y1,
        ) in _baseline_segments(baseline)
        if contact_x >= segment_left - SOURCE_COORD_EPS_PT
        and contact_x <= segment_right + SOURCE_COORD_EPS_PT
    )


def _vertical_baseline_contact_intervals(
        page: VectorPage,
        tone: float,
        rail: dict[str, Any] | None,
        baseline: _SourceBaselineSpan,
        ) -> list[tuple[float, float]]:
    """Final-visible x intervals where this vertical physically meets a base.

    The aggregate ``ink_x0..ink_x1`` envelope is deliberately not evidence:
    two split strokes can straddle a baseline endpoint while leaving paper at
    the claimed contact coordinate. Each interval below is cut from an actual
    narrow source rectangle, at an exact y-overlap/touch with an actual
    baseline segment, and remains finally owned by one of those two lineages.
    """
    if rail is None:
        return []
    members = tuple(rail.get("members", ()))
    allowed_operations = {
        (paint.order, paint.operation) for paint in members
    } | set(baseline.operations)
    wanted_tone = round(tone, 4)
    intervals: list[tuple[float, float]] = []
    for member in members:
        for base_left, base_right, base_y0, base_y1 in _baseline_segments(
                baseline):
            if (member.y0 > base_y1 + SOURCE_COORD_EPS_PT
                    or member.y1 < base_y0 - SOURCE_COORD_EPS_PT
                    or member.x0 > base_right + SOURCE_COORD_EPS_PT
                    or member.x1 < base_left - SOURCE_COORD_EPS_PT):
                continue
            left = max(member.x0, base_left)
            right = min(member.x1, base_right)
            if right < left - SOURCE_COORD_EPS_PT:
                continue
            contact_y0 = max(member.y0, base_y0)
            contact_y1 = min(member.y1, base_y1)
            sample_y = (contact_y0 + contact_y1) / 2.0
            active = [
                paint for paint in page.paints
                if (paint.y0 <= sample_y + SOURCE_COORD_EPS_PT
                    and paint.y1 >= sample_y - SOURCE_COORD_EPS_PT
                    and paint.x1 >= left - SOURCE_COORD_EPS_PT
                    and paint.x0 <= right + SOURCE_COORD_EPS_PT)
            ]
            x_edges = {left, right}
            for paint in active:
                clipped_left = max(left, paint.x0)
                clipped_right = min(right, paint.x1)
                if clipped_right >= clipped_left:
                    x_edges.update((clipped_left, clipped_right))
            ordered_x = sorted(x_edges)
            if len(ordered_x) == 1:
                ordered_x.append(ordered_x[0])
            for slab_left, slab_right in zip(
                    ordered_x, ordered_x[1:]):
                sample_x = (slab_left + slab_right) / 2.0
                final_tone, owners = _final_tone_and_owner(
                    active, sample_x, sample_y)
                if (round(final_tone, 4) != wanted_tone
                        or not any(
                            (owner.order, owner.operation)
                            in allowed_operations
                            for owner in owners
                        )):
                    continue
                intervals.append((slab_left, slab_right))
    return _merge_intervals(intervals, SOURCE_COORD_EPS_PT)


def _baseline_coordinate_contacts_vertical(
        page: VectorPage,
        tone: float,
        contact_x: float,
        rail: dict[str, Any] | None,
        baseline: _SourceBaselineSpan,
        ) -> bool:
    """Require exact 2D contact with one painted vertical interval."""
    return any(
        contact_x >= left - SOURCE_COORD_EPS_PT
        and contact_x <= right + SOURCE_COORD_EPS_PT
        for left, right in _vertical_baseline_contact_intervals(
            page, tone, rail, baseline)
    )


def _connected_vertical_baseline_contact(
        page: VectorPage,
        tone: float,
        rail: dict[str, Any] | None,
        band_y0: float,
        span_y1: float,
        contact_x: float,
        baseline: _SourceBaselineSpan,
        ) -> bool:
    """Bind exact baseline contact to one uninterrupted visible rail path.

    ``_stable_source_verticals`` deliberately tolerates up to
    ``COMB_YSLACK_PT`` of missing span.  That tolerance cannot prove a U-frame
    side rail: a long stroke and a separate same-x contact fragment could
    otherwise straddle paper and jointly satisfy the stable/contact checks.
    Track final-tone ink backed by the effective compound paint operation of an
    actual vertical member slab by slab.  One run may start within the existing
    leading ``COMB_YSLACK_PT`` allowance; after it starts, only x-overlapping
    continuation can reach the exact baseline coordinate.  A later same-tone
    fill may own the final pixel while preserving a genuinely painted vertical
    operation; it cannot replace a canceled member or missing ink with an
    unrelated broad repaint.
    """
    if (rail is None
            or span_y1 <= band_y0 + SOURCE_COORD_EPS_PT):
        return False
    members = tuple(rail.get("members", ()))
    if not members:
        return False
    operation_regions: dict[
        tuple[int, int], list[VectorPaint]
    ] = collections.defaultdict(list)
    for paint in page.paints:
        operation_regions[(paint.order, paint.operation)].append(paint)

    span_left = min(paint.x0 for paint in members)
    span_right = max(paint.x1 for paint in members)
    relevant = [
        paint for paint in page.paints
        if paint.x1 >= span_left - SOURCE_COORD_EPS_PT
        and paint.x0 <= span_right + SOURCE_COORD_EPS_PT
        and paint.y1 > band_y0
        and paint.y0 < span_y1
    ]
    endpoints = {band_y0, span_y1}
    for paint in relevant:
        endpoints.update((
            max(band_y0, paint.y0),
            min(span_y1, paint.y1),
        ))
    slabs = [
        (a, b)
        for a, b in zip(sorted(endpoints), sorted(endpoints)[1:])
        if b - a > SOURCE_COORD_EPS_PT
    ]
    if not slabs:
        return False

    wanted_tone = round(tone, 4)
    start_deadline = band_y0 + COMB_YSLACK_PT
    reachable: list[tuple[float, float]] | None = None
    prior_y = band_y0
    for a, b in slabs:
        if a > prior_y + SOURCE_COORD_EPS_PT:
            if a > start_deadline + SOURCE_COORD_EPS_PT:
                return False
            reachable = None
        sample_y = (a + b) / 2.0
        active = [
            paint for paint in relevant
            if paint.y0 <= sample_y <= paint.y1
        ]
        active_members = [
            paint for paint in members
            if paint.y0 <= sample_y <= paint.y1
        ]

        x_edges = {span_left, span_right}
        for paint in active:
            clipped_left = max(span_left, paint.x0)
            clipped_right = min(span_right, paint.x1)
            if clipped_right >= clipped_left:
                x_edges.update((clipped_left, clipped_right))
        visible: list[tuple[float, float]] = []
        ordered_x = sorted(x_edges)
        if len(ordered_x) == 1:
            ordered_x.append(ordered_x[0])
        for left, right in zip(ordered_x, ordered_x[1:]):
            sample_x = (left + right) / 2.0
            final_tone = _final_tone(active, sample_x, sample_y)
            if (round(final_tone, 4) == wanted_tone
                    and any(
                        member.covers(sample_x, sample_y)
                        and _operation_covers(
                            operation_regions[
                                (member.order, member.operation)
                            ],
                            sample_x,
                            sample_y,
                        )
                        for member in active_members
                    )):
                visible.append((left, right))
        visible = _merge_intervals(visible, SOURCE_COORD_EPS_PT)
        if reachable is not None:
            connected = [
                interval for interval in visible
                if any(
                    interval[0] <= prior[1] + SOURCE_COORD_EPS_PT
                    and interval[1] >= prior[0] - SOURCE_COORD_EPS_PT
                    for prior in reachable
                )
            ]
            if connected:
                reachable = connected
            elif visible and a <= start_deadline + SOURCE_COORD_EPS_PT:
                reachable = visible
            elif b <= start_deadline + SOURCE_COORD_EPS_PT:
                reachable = None
            else:
                return False
        elif visible:
            if a > start_deadline + SOURCE_COORD_EPS_PT:
                return False
            reachable = visible
        elif b > start_deadline + SOURCE_COORD_EPS_PT:
            return False
        prior_y = b

    if (prior_y < span_y1 - SOURCE_COORD_EPS_PT
            or reachable is None
            or not any(
                contact_x >= left - SOURCE_COORD_EPS_PT
                and contact_x <= right + SOURCE_COORD_EPS_PT
                for left, right in reachable
            )):
        return False
    return _baseline_coordinate_contacts_vertical(
        page, tone, contact_x, rail, baseline)


def _vertical_has_connected_baseline_contact(
        page: VectorPage,
        tone: float,
        rail: dict[str, Any] | None,
        band_y0: float,
        contact_x: float,
        baseline: _SourceBaselineSpan,
        ) -> bool:
    """Require one actual segment-level contact on the connected rail path.

    A segmented junction may legitimately span more than one touching
    baseline segment or level.  Each is evaluated independently; an aggregate
    rail envelope or aggregate baseline component is never itself a witness.
    No spanning segment or no independently connected segment fails closed.
    """
    contacts = _baseline_contact_segments(baseline, contact_x)
    return bool(contacts) and any(
        _connected_vertical_baseline_contact(
            page,
            tone,
            rail,
            band_y0,
            contact.y0,
            contact_x,
            contact,
        )
        for contact in contacts
    )


def _published_vertical_geometry(
        page: VectorPage,
        tone: float,
        rail: dict[str, Any],
        baseline: _SourceBaselineSpan,
        ) -> dict[str, Any]:
    members = sorted(
        rail.get("members", ()),
        key=lambda item: (
            item.x0, item.y0, item.x1, item.y1,
            item.order, item.operation, item.kind,
        ),
    )
    return {
        "center_x": round(float(rail["center_x"]), 6),
        "ink_x0": round(float(rail["ink_x0"]), 6),
        "ink_x1": round(float(rail["ink_x1"]), 6),
        "ink_y0": round(float(rail["ink_y0"]), 6),
        "ink_y1": round(float(rail["ink_y1"]), 6),
        "paint_rects": [
            [
                round(paint.x0, 6), round(paint.y0, 6),
                round(paint.x1, 6), round(paint.y1, 6),
            ]
            for paint in members
        ],
        "paint_operations": [
            [paint.order, paint.operation] for paint in members
        ],
        "contact_intervals_x": [
            [round(left, 6), round(right, 6)]
            for left, right in _vertical_baseline_contact_intervals(
                page, tone, rail, baseline)
        ],
    }


def _baseline_spans(
        page: VectorPage, band_y1: float, tone: float,
        ) -> list[_SourceBaselineSpan]:
    """Untrimmed final-visible source baselines near one band bottom.

    A claimed layout box is deliberately absent from this function. Clipping a
    raw baseline to that box turns any two internal dividers into counterfeit
    frame endpoints, making a shrunk layout self-validating. Each returned run
    therefore retains its source-operation lineage and its real merged source
    endpoints.
    """
    wanted_tone = round(tone, 4)
    raw: list[VectorPaint] = []
    for paint in page.paints:
        width = paint.x1 - paint.x0
        height = paint.y1 - paint.y0
        if (width <= height
                or height > COMB_MAX_WIDTH_PT
                or round(paint.tone, 4) != wanted_tone
                or band_y1 < paint.y0 - COMB_YSLACK_PT
                or band_y1 > paint.y1 + COMB_YSLACK_PT):
            continue
        raw.append(paint)

    spans: list[_SourceBaselineSpan] = []
    for paint in sorted(
            raw,
            key=lambda item: (
                (item.y0 + item.y1) / 2.0,
                item.x0, item.x1, item.order, item.operation,
            )):
        sample_y = (paint.y0 + paint.y1) / 2.0
        raw_left, raw_right = paint.x0, paint.x1
        active = [
            candidate for candidate in page.paints
            if candidate.y0 <= sample_y <= candidate.y1
            and candidate.x1 > raw_left and candidate.x0 < raw_right
        ]
        x_edges = {raw_left, raw_right}
        for candidate in active:
            x_edges.update((
                max(raw_left, candidate.x0),
                min(raw_right, candidate.x1),
            ))
        visible: list[tuple[float, float]] = []
        ordered_x = sorted(x_edges)
        lineage_operation = (paint.order, paint.operation)
        for left, right in zip(ordered_x, ordered_x[1:]):
            final_tone, owners = _final_tone_and_owner(
                active, (left + right) / 2, sample_y)
            owned_by_baseline_or_connector = any(
                (
                    (owner.order, owner.operation) == lineage_operation
                    and owner.x1 - owner.x0 > owner.y1 - owner.y0
                )
                or _is_comb_vertical(owner)
                for owner in owners
            )
            if (right > left
                    and round(final_tone, 4) == wanted_tone
                    and owned_by_baseline_or_connector):
                visible.append((left, right))
        visible = _merge_intervals(visible, SOURCE_COORD_EPS_PT)
        if not any(
                left <= raw_left + SOURCE_COORD_EPS_PT
                and right >= raw_right - SOURCE_COORD_EPS_PT
                for left, right in visible):
            continue
        spans.append(_SourceBaselineSpan(
            y=sample_y,
            y0=paint.y0,
            y1=paint.y1,
            left=raw_left,
            right=raw_right,
            operations=(lineage_operation,),
            segments=((raw_left, raw_right, paint.y0, paint.y1),),
        ))
    return spans


def _segmented_u_frame_candidates(
        page: VectorPage,
        baselines: Sequence[_SourceBaselineSpan],
        band_y0: float,
        band_y1: float,
        tone: float,
        topology: Sequence[float],
        ) -> list[
            tuple[
                float, float, tuple[float, ...], _SourceBaselineSpan,
                tuple[float, ...], tuple[tuple[int, int], ...],
            ]
        ]:
    """Build maximal pitch-coherent frames over explicitly painted segments.

    Some official comb baselines are emitted as one horizontal operation per
    compartment. Their raw endpoints are genuine, but the full frame is the
    maximal source-owned chain, not whichever subset a claimed bbox clips out.
    A large non-comb table cell sharing that y is separated by its incompatible
    pitch, while group-separator variation remains inside a 30% source-derived
    pitch envelope.
    """
    remaining = list(sorted(
        baselines,
        key=lambda item: (
            item.left, item.right, item.y0, item.y1, item.operations,
        ),
    ))
    connected_groups: list[list[_SourceBaselineSpan]] = []
    while remaining:
        group = [remaining.pop(0)]
        changed = True
        while changed:
            changed = False
            retained: list[_SourceBaselineSpan] = []
            for candidate in remaining:
                connected = any(
                    any(
                        (
                            candidate_left
                            <= member_right + SOURCE_COORD_EPS_PT
                            and candidate_right
                            >= member_left - SOURCE_COORD_EPS_PT
                            and candidate_y0
                            <= member_y1 + SOURCE_COORD_EPS_PT
                            and candidate_y1
                            >= member_y0 - SOURCE_COORD_EPS_PT
                        )
                        for (
                            candidate_left, candidate_right,
                            candidate_y0, candidate_y1,
                        ) in _baseline_segments(candidate)
                        for (
                            member_left, member_right,
                            member_y0, member_y1,
                        ) in _baseline_segments(member)
                    )
                    for member in group
                )
                if connected:
                    group.append(candidate)
                    changed = True
                else:
                    retained.append(candidate)
            remaining = retained
        connected_groups.append(group)

    components: list[_SourceBaselineSpan] = []
    for group in connected_groups:
        segments = tuple(sorted({
            segment
            for item in group
            for segment in _baseline_segments(item)
        }))
        components.append(_SourceBaselineSpan(
            y=sum(item.y for item in group) / len(group),
            y0=min(segment[2] for segment in segments),
            y1=max(segment[3] for segment in segments),
            left=min(segment[0] for segment in segments),
            right=max(segment[1] for segment in segments),
            operations=tuple(sorted({
                operation
                for item in group
                for operation in item.operations
            })),
            segments=segments,
        ))

    candidates = []
    for component in components:
        if component.y0 <= band_y0 + SOURCE_COORD_EPS_PT:
            continue
        verticals = _stable_source_verticals(
            page,
            component.left - COMB_MAX_WIDTH_PT,
            component.right + COMB_MAX_WIDTH_PT,
            band_y0,
            component.y0,
            tone,
        )
        verticals = sorted(
            value for value in verticals
            if (value >= component.left - COMB_MAX_WIDTH_PT
                and value <= component.right + COMB_MAX_WIDTH_PT)
        )
        matched_indexes = [
            index for index, value in enumerate(verticals)
            if any(
                abs(value - divider) <= COMB_MERGE_PT
                for divider in topology
            )
        ]
        if not matched_indexes:
            continue

        if len(matched_indexes) == 1:
            index = matched_indexes[0]
            if index == 0 or index + 1 >= len(verticals):
                continue
            left_gap = verticals[index] - verticals[index - 1]
            right_gap = verticals[index + 1] - verticals[index]
            pitch = (left_gap + right_gap) / 2
            tolerance = max(COMB_MERGE_PT, 0.3 * pitch)
            if abs(left_gap - right_gap) > tolerance:
                continue
            start, end = index - 1, index + 1
        else:
            differences = sorted(
                verticals[right] - verticals[left]
                for left, right in zip(
                    matched_indexes, matched_indexes[1:])
            )
            pitch = differences[len(differences) // 2]
            tolerance = max(COMB_MERGE_PT, 0.3 * pitch)
            runs: list[list[int]] = [[matched_indexes[0]]]
            for index in matched_indexes[1:]:
                gap = verticals[index] - verticals[runs[-1][-1]]
                if abs(gap - pitch) <= tolerance:
                    runs[-1].append(index)
                else:
                    runs.append([index])
            longest = max(
                runs,
                key=lambda run: (
                    len(run), verticals[run[-1]] - verticals[run[0]],
                    -verticals[run[0]],
                ),
            )
            start, end = longest[0], longest[-1]

        while start > 0:
            gap = verticals[start] - verticals[start - 1]
            if abs(gap - pitch) > tolerance:
                break
            start -= 1
        while end + 1 < len(verticals):
            gap = verticals[end + 1] - verticals[end]
            if abs(gap - pitch) > tolerance:
                break
            end += 1
        run = verticals[start:end + 1]
        if len(run) < 3:
            continue
        left, right = run[0], run[-1]
        run_geometry = {
            source_x: _source_vertical_ink_geometry(
                page, source_x, band_y0, band_y1, tone)
            for source_x in run
        }
        contact_coordinates = (
            [(left, component.left)]
            + [(source_x, source_x) for source_x in run[1:-1]]
            + [(right, component.right)]
        )
        if any(
                not _vertical_has_connected_baseline_contact(
                    page,
                    tone,
                    run_geometry[source_x],
                    band_y0,
                    contact_x,
                    component,
                )
                for source_x, contact_x in contact_coordinates
                ):
            continue
        interior = tuple(
            divider for divider in topology
            if (divider > left + COMB_MERGE_PT
                and divider < right - COMB_MERGE_PT
                and any(
                    abs(divider - source_x) <= COMB_MERGE_PT
                    for source_x in run[1:-1]
                ))
        )
        if not interior:
            continue
        external = tuple(
            divider for divider in topology
            if (divider < left - COMB_MERGE_PT
                or divider > right + COMB_MERGE_PT)
        )
        candidates.append((
            left,
            right,
            interior,
            component,
            external,
            component.operations,
        ))
    return candidates


def _local_baseline_spans(
        page: VectorPage, x0: float, x1: float, band_y1: float,
        tone: float,
        ) -> list[tuple[float, float, float]]:
    """Local segmented-baseline evidence used only after source-first guards."""
    wanted_tone = round(tone, 4)
    raw: list[tuple[float, float, float]] = []
    for paint in page.paints:
        width = paint.x1 - paint.x0
        height = paint.y1 - paint.y0
        centre_y = (paint.y0 + paint.y1) / 2
        if (width <= height
                or height > COMB_MAX_WIDTH_PT
                or round(paint.tone, 4) != wanted_tone
                or band_y1 < paint.y0 - COMB_YSLACK_PT
                or band_y1 > paint.y1 + COMB_YSLACK_PT
                or paint.x1 <= x0 or paint.x0 >= x1):
            continue
        raw.append((
            centre_y,
            max(x0, paint.x0),
            min(x1, paint.x1),
        ))

    y_groups: list[list[tuple[float, float, float]]] = []
    for item in sorted(raw):
        if (y_groups
                and item[0] - y_groups[-1][-1][0] <= COMB_YSLACK_PT):
            y_groups[-1].append(item)
        else:
            y_groups.append([item])
    spans: list[tuple[float, float, float]] = []
    for group in y_groups:
        sample_y = sum(item[0] for item in group) / len(group)
        raw_intervals = _merge_intervals([
            (left, right) for _centre_y, left, right in group
        ], COMB_MERGE_PT)
        active = [
            paint for paint in page.paints
            if paint.y0 <= sample_y <= paint.y1
            and paint.x1 > x0 and paint.x0 < x1
        ]
        x_edges = {x0, x1}
        for paint in active:
            x_edges.update((max(x0, paint.x0), min(x1, paint.x1)))
        visible: list[tuple[float, float]] = []
        ordered_x = sorted(x_edges)
        for left, right in zip(ordered_x, ordered_x[1:]):
            if (right > left
                    and round(_final_tone(
                        active, (left + right) / 2, sample_y), 4)
                    == wanted_tone):
                visible.append((left, right))
        visible = _merge_intervals(
            visible, verify.DEFAULT_POSITION_TOL_PT)
        group_spans: list[tuple[float, float]] = []
        for raw_left, raw_right in raw_intervals:
            for visible_left, visible_right in visible:
                left = max(raw_left, visible_left)
                right = min(raw_right, visible_right)
                if right > left:
                    group_spans.append((left, right))
        spans.extend(
            (sample_y, left, right)
            for left, right in _merge_intervals(
                group_spans, verify.DEFAULT_POSITION_TOL_PT)
        )
    return spans


def _source_u_frame(
        page: VectorPage, x0: float, x1: float,
        band_y0: float, band_y1: float, tone: float,
        topology: Sequence[float],
        ) -> tuple[tuple[float, ...], dict[str, Any]] | None:
    """Resolve one maximal source U-frame before trusting the claimed bbox."""
    if not topology:
        return None
    baselines = _baseline_spans(page, band_y1, tone)
    candidates: list[
        tuple[
            float, float, tuple[float, ...], _SourceBaselineSpan,
            tuple[float, ...], tuple[tuple[int, int], ...],
        ]
    ] = []
    for baseline in baselines:
        if baseline.right <= x0 or baseline.left >= x1:
            continue
        if baseline.y0 <= band_y0 + SOURCE_COORD_EPS_PT:
            continue
        verticals = _stable_source_verticals(
            page,
            baseline.left - COMB_MAX_WIDTH_PT,
            baseline.right + COMB_MAX_WIDTH_PT,
            band_y0,
            baseline.y0,
            tone,
        )
        vertical_geometry = {
            value: _source_vertical_ink_geometry(
                page, value, band_y0, band_y1, tone)
            for value in verticals
        }
        left_matches = sorted(
            (value for value in verticals
             if _vertical_has_connected_baseline_contact(
                 page, tone, vertical_geometry[value],
                 band_y0, baseline.left, baseline)),
            key=lambda value: (abs(value - baseline.left), value),
        )
        right_matches = sorted(
            (value for value in verticals
             if _vertical_has_connected_baseline_contact(
                 page, tone, vertical_geometry[value],
                 band_y0, baseline.right, baseline)),
            key=lambda value: (abs(value - baseline.right), value),
        )
        if not left_matches or not right_matches:
            continue
        left, right = left_matches[0], right_matches[0]
        if right - left <= 2 * COMB_MERGE_PT:
            continue
        interior = tuple(
            divider for divider in topology
            if divider > left + COMB_MERGE_PT
            and divider < right - COMB_MERGE_PT
            and any(
                abs(divider - source_x) <= COMB_MERGE_PT
                and _vertical_has_connected_baseline_contact(
                    page, tone, vertical_geometry[source_x],
                    band_y0, source_x, baseline)
                for source_x in verticals
            )
        )
        if not interior:
            continue
        external = tuple(
            divider for divider in topology
            if (divider < left - COMB_MERGE_PT
                or divider > right + COMB_MERGE_PT)
        )
        candidates.append((
            left, right, interior, baseline,
            external, baseline.operations,
        ))
    candidates.extend(_segmented_u_frame_candidates(
        page,
        baselines,
        band_y0,
        band_y1,
        tone,
        topology,
    ))
    if not candidates:
        return None

    widest = max(
        right - left
        for left, right, _interior, _baseline, _external, _lineage
        in candidates
    )
    maximal = [
        candidate for candidate in candidates
        if abs((candidate[1] - candidate[0]) - widest)
        <= verify.DEFAULT_POSITION_TOL_PT
    ]
    interiors: list[tuple[float, ...]] = []
    for _left, _right, interior, _baseline, _external, _lineage in maximal:
        if not any(_same_topology(interior, seen) for seen in interiors):
            interiors.append(interior)
    if len(interiors) != 1:
        raise ValueError(
            "maximal same-tone source U-frames yield different interiors")
    closest = min(
        maximal,
        key=lambda item: (
            abs(item[3].y - band_y1),
            item[3].y, item[0], item[1]),
    )
    left, right, _interior, baseline, external, lineage = closest
    left_rail_geometry = _source_vertical_ink_geometry(
        page, left, band_y0, band_y1, tone)
    right_rail_geometry = _source_vertical_ink_geometry(
        page, right, band_y0, band_y1, tone)
    if left_rail_geometry is None or right_rail_geometry is None:
        return None
    rail_geometry = {
        "left": _published_vertical_geometry(
            page, tone, left_rail_geometry, baseline),
        "right": _published_vertical_geometry(
            page, tone, right_rail_geometry, baseline),
    }
    baseline_segments = [
        {
            "x0": round(segment[0], 6),
            "x1": round(segment[1], 6),
            "y0": round(segment[2], 6),
            "y1": round(segment[3], 6),
        }
        for segment in _baseline_segments(baseline)
    ]
    frame_evidence = {
        "left_rail": round(left, 6),
        "right_rail": round(right, 6),
        "rail_geometry": rail_geometry,
        "baseline_y": round(baseline.y, 6),
        "baseline_y0": round(baseline.y0, 6),
        "baseline_y1": round(baseline.y1, 6),
        "baseline_segments": baseline_segments,
        "baseline_operations": [
            list(operation) for operation in lineage
        ],
    }
    cropped_sides = []
    if left < x0 - COMB_MERGE_PT:
        cropped_sides.append("left")
    if right > x1 + COMB_MERGE_PT:
        cropped_sides.append("right")
    if cropped_sides:
        raise CombTopologyError(
            "claimed comb owner crops a wider source U-frame",
            {
                "criterion": "maximal-source-u-frame-owner",
                "owner_rect": [
                    round(x0, 6), round(band_y0, 6),
                    round(x1, 6), round(band_y1, 6),
                ],
                "frame": frame_evidence,
                "cropped_sides": cropped_sides,
            },
        )
    if external:
        raise CombTopologyError(
            "claimed comb owner absorbs unframed source corridors outside "
            "its complete source U-frame",
            {
                "criterion": "complete-source-u-frame-bounds",
                "owner_rect": [
                    round(x0, 6), round(band_y0, 6),
                    round(x1, 6), round(band_y1, 6),
                ],
                "frame": frame_evidence,
                "unframed_corridors": [
                    round(value, 6) for value in external
                ],
            },
        )
    return interiors[0], {
        "tone": round(tone, 4),
        "left_rail": rail_geometry["left"],
        "right_rail": rail_geometry["right"],
        "band_y0": round(band_y0, 6),
        "baseline_y": round(baseline.y, 6),
        "baseline_y0": round(baseline.y0, 6),
        "baseline_y1": round(baseline.y1, 6),
        "baseline_segments": baseline_segments,
        "baseline_operations": [
            list(operation) for operation in lineage
        ],
    }


def printed_compartments(
        page: VectorPage,
        cell: dict[str, Any],
        *,
        include_frame: bool = False,
        owner_certificate: CombOwnerCertificate | None = None,
        ) -> tuple[int, list[float]] | tuple[
            int, list[float], dict[str, Any] | None]:
    """Count the source's final visible divider topology inside one comb.

    The lattice supplies only an exact, reviewed owner identity and rectangle.
    Candidate vertical bands, tones, and every divider come from raw source
    paint within (or crossing) that cell.  No member of the lattice's `comb`
    object or subject topology is read.  A complete source U-frame owns its
    interior directly.  Without one, the reviewed certificate may establish
    only *whose* rectangle this is, and only one unanimous source-derived
    topology can be used.  Competing topology stays unevaluable.
    """
    try:
        x0, y0 = float(cell["x0"]), float(cell["y0"])
        x1, y1 = float(cell["x1"]), float(cell["y1"])
    except (KeyError, TypeError, ValueError) as exc:
        raise ValueError("comb owner geometry is incomplete") from exc
    if not all(math.isfinite(value) for value in (x0, y0, x1, y1)):
        raise ValueError("comb owner geometry is non-finite")
    if x1 <= x0 or y1 <= y0:
        raise ValueError("comb owner has no positive area")
    if (owner_certificate is not None
            and not owner_certificate.matches(
                int(owner_certificate.page), cell)):
        raise ValueError(
            "comb owner certificate does not bind this exact cell identity")

    owner = (x0, y0, x1, y1)
    bands, first_source_order = _source_band_candidates(page, owner)
    if not bands:
        relevant = sorted({
            unsupported.reason for unsupported in page.unsupported
            if _rects_intersect(owner, unsupported.rect)
        })
        suffix = f": {', '.join(relevant)}" if relevant else ""
        raise ValueError(f"no plausible source-derived comb band{suffix}")

    results: list[
        tuple[
            float, float, float, tuple[float, ...],
            dict[str, Any] | None,
        ]
    ] = []
    text_reasons = {
        "unmodeled source fill-text paint",
        "unmodeled source stroke-text paint",
    }
    image_reason = "unmodeled source fill-image paint"
    deferred_reasons = {*text_reasons, image_reason}
    image_hits = sorted(
        (
            unsupported for unsupported in page.unsupported
            if unsupported.reason == image_reason
            and any(_rects_intersect(
                (x0, band_y0, x1, band_y1), unsupported.rect)
                for band_y0, band_y1 in bands)
        ),
        key=lambda item: (item.order, item.rect),
    )
    if image_hits:
        raise CombTopologyError(
            "unmodeled source fill-image paint intersects a plausible "
            "source-derived comb band",
            {
                "criterion": "source-comb-band-image-free-required",
                "owner_rect": [
                    round(x0, 6), round(y0, 6),
                    round(x1, 6), round(y1, 6),
                ],
                "candidate_bands": [
                    [round(band_y0, 6), round(band_y1, 6)]
                    for band_y0, band_y1 in bands
                ],
                "image_paint": [
                    {
                        "order": hit.order,
                        "rect": [round(value, 6) for value in hit.rect],
                    }
                    for hit in image_hits
                ],
                **({
                    "owner_certificate": owner_certificate.evidence(),
                } if owner_certificate is not None else {}),
            },
        )
    blocked: set[str] = set()
    for band_y0, band_y1 in bands:
        subject = (x0, band_y0, x1, band_y1)
        nonforeign_hits = [
            unsupported for unsupported in page.unsupported
            if _rects_intersect(subject, unsupported.rect)
            and unsupported.reason not in deferred_reasons
        ]
        if nonforeign_hits:
            blocked.update(hit.reason for hit in nonforeign_hits)
            continue
        for tone, topology in _band_topologies(
                page, x0, x1, band_y0, band_y1):
            text_hits = [
                unsupported for unsupported in page.unsupported
                if unsupported.reason in text_reasons
                and first_source_order is not None
                and unsupported.order > first_source_order
                and _rects_intersect(subject, unsupported.rect)
                and any(unsupported.rect[0] <= divider_x <= unsupported.rect[2]
                        for divider_x in topology)
                and not (
                    unsupported.reason in {
                        "unmodeled source fill-text paint",
                        "unmodeled source stroke-text paint",
                    }
                    and unsupported.tone is not None
                    and unsupported.opacity == 1.0
                    and bool(unsupported.trace_rects)
                    and round(unsupported.tone, 4) == tone
                    and all(
                        trace_rect[2] < divider_x - COMB_MERGE_PT
                        or trace_rect[0] > divider_x + COMB_MERGE_PT
                        for trace_rect in unsupported.trace_rects
                        for divider_x in topology
                    )
                )
            ]
            if text_hits:
                blocked.update(hit.reason for hit in text_hits)
                continue
            frame = _source_u_frame(
                page, x0, x1, band_y0, band_y1, tone, topology)
            if frame is None:
                normalized = topology
                frame_key = None
            else:
                normalized, frame_key = frame
            results.append((
                band_y0, band_y1, tone, normalized, frame_key))

    if blocked:
        raise ValueError(
            "unsupported source paint intersects a plausible source-derived "
            f"comb band: {', '.join(sorted(blocked))}")
    if not results:
        raise CombTopologyError(
            "plausible source-derived bands have no strict-majority topology",
            {
                "criterion": "continuous-final-source-owner-strict-majority",
                "bands": [
                    {
                        "y0": round(band_y0, 6),
                        "y1": round(band_y1, 6),
                        "vertical_lineages": _vertical_lineage_diagnostics(
                            page, x0, x1, band_y0, band_y1),
                    }
                    for band_y0, band_y1 in bands
                ],
            },
        )

    topology_groups: list[tuple[float, ...]] = []
    for _band_y0, _band_y1, _tone, topology, _frame in sorted(
            results, key=lambda item: item[:4]):
        if not any(_same_topology(topology, seen) for seen in topology_groups):
            topology_groups.append(topology)

    # A U-frame is two continuous source-owned rails plus a same-tone source
    # baseline. It proves ownership without preferring whichever band happens
    # to have more dividers. Near-identical seed bands can rediscover the same
    # frame, so collapse only those; two distinct complete frames are ambiguous.
    frame_groups: list[
        tuple[
            dict[str, Any],
            tuple[float, ...],
        ]
    ] = []
    for _a, _b, _tone, topology, frame_key in results:
        if frame_key is None:
            continue
        matched_index = next((
            index for index, item in enumerate(frame_groups)
            if (abs(item[0]["tone"] - frame_key["tone"])
                <= SOURCE_COORD_EPS_PT
                and abs(
                    item[0]["left_rail"]["center_x"]
                    - frame_key["left_rail"]["center_x"]
                ) <= COMB_MERGE_PT
                and abs(
                    item[0]["right_rail"]["center_x"]
                    - frame_key["right_rail"]["center_x"]
                ) <= COMB_MERGE_PT
                and abs(item[0]["baseline_y"] - frame_key["baseline_y"])
                <= verify.DEFAULT_POSITION_TOL_PT)
        ), None)
        if matched_index is None:
            frame_groups.append((frame_key, topology))
            continue
        matched_key, matched_topology = frame_groups[matched_index]
        if _same_topology(matched_topology, topology):
            continue
        if _topology_subset(matched_topology, topology):
            # One seed band can see only the dividers that continue through the
            # whole composite cell. The same physical rails/baseline own every
            # source corridor that meets them, so retain the exhaustive
            # superset rather than treating omission as a second frame.
            frame_groups[matched_index] = (matched_key, topology)
        elif not _topology_subset(topology, matched_topology):
            raise ValueError(
                "one source U-frame yields incomparable interior topologies")

    if len(frame_groups) > 1:
        counts = sorted({len(topology) + 1 for topology in topology_groups})
        framed_counts = sorted({
            len(topology) + 1 for _frame, topology in frame_groups
        })
        raise ValueError(
            "multiple complete source U-frames compete "
            f"(compartment counts {counts}; U-frames {framed_counts})")
    if len(frame_groups) != 1:
        counts = sorted({len(topology) + 1 for topology in topology_groups})
        if len(topology_groups) == 1 and owner_certificate is not None:
            chosen = topology_groups[0]
            result = (len(chosen) + 1, [float(value) for value in chosen])
            if not include_frame:
                return result
            return (*result, None)
        criterion = (
            "unanimous-source-derived-topology-required"
            if owner_certificate is not None
            else "independent-complete-source-u-frame-required"
        )
        reason = (
            "reviewed comb owner has competing source-derived band/tone "
            "topologies"
            if owner_certificate is not None
            else "plausible source-derived band/tone choices disagree without "
                 "one complete source U-frame owner"
        )
        raise CombTopologyError(
            f"{reason} (compartment counts {counts}; U-frames [])",
            {
                "criterion": criterion,
                "owner_rect": [
                    round(x0, 6), round(y0, 6),
                    round(x1, 6), round(y1, 6),
                ],
                "unframed_compartment_counts": counts,
                **({
                    "owner_certificate": owner_certificate.evidence(),
                } if owner_certificate is not None else {}),
            },
        )
    frame_key, chosen = frame_groups[0]
    result = (len(chosen) + 1, [float(value) for value in chosen])
    if not include_frame:
        return result
    return (*result, copy.deepcopy(frame_key))


def drawn_codepoints(page) -> dict[tuple[float, float], set[int]]:
    """Codepoint(s) the page draws at each glyph origin.

    `get_texttrace()` reports what the font's encoding actually yields, U+FFFD
    included; `get_text("rawdict")` -- which extract.py reads -- substitutes a
    plausible character when the encoding fails. Comparing the two at the origin
    is how an invented character is caught, since the origin is the one thing
    both views agree on.
    """
    seen: dict[tuple[float, float], set[int]] = {}
    for span in page.get_texttrace():
        for char in span["chars"]:
            key = (round(char[2][0], 2), round(char[2][1], 2))
            seen.setdefault(key, set()).add(char[0])
    return seen


def transform_signature(matrix: Sequence[float]) -> tuple[int, int, bool]:
    """Orientation of a placement matrix: (x sign, y sign, sheared).

    Magnitude is already checked by the image's bbox, so what has to survive to
    the SVG is *orientation*. PyMuPDF reports the placement in the same
    y-downward space the page and our SVG use, so an upright image has d > 0 and
    2550M's flipped seal has d < 0; an SVG that reproduces it must therefore
    carry a negative y scale. Comparing signs rather than the six numbers keeps
    the assertion from dictating how emit.py decomposes the matrix.
    """
    a, b, c, d = (float(v) for v in matrix[:4])
    return (-1 if a < 0 else 1, -1 if d < 0 else 1,
            abs(b) > TRANSFORM_EPS or abs(c) > TRANSFORM_EPS)


SVG_TRANSFORM_RE = re.compile(r'(matrix|scale|translate|rotate)\(([^)]*)\)')


def svg_signature(transform: str | None) -> tuple[int, int, bool]:
    """The same orientation signature, read off an SVG transform list."""
    if not transform:
        return (1, 1, False)
    a, b, c, d = 1.0, 0.0, 0.0, 1.0
    for name, body in SVG_TRANSFORM_RE.findall(transform):
        nums = [float(v) for v in re.split(r'[\s,]+', body.strip()) if v]
        if name == "matrix" and len(nums) == 6:
            na, nb, nc, nd = nums[:4]
        elif name == "scale":
            na, nb, nc, nd = nums[0], 0.0, 0.0, (nums[1] if len(nums) > 1 else nums[0])
        elif name == "rotate" and nums:
            rad = math.radians(nums[0])
            na, nb, nc, nd = math.cos(rad), math.sin(rad), -math.sin(rad), math.cos(rad)
        else:
            continue    # translate has no linear part
        a, b, c, d = (a * na + c * nb, b * na + d * nb,
                      a * nc + c * nd, b * nc + d * nd)
    return transform_signature((a, b, c, d))


# --------------------------------------------------------------------------
# the bundle every assertion reads
# --------------------------------------------------------------------------


CSS_URL_RE = re.compile(
    r"""url\(\s*(?P<quote>["']?)(?P<url>.*?)\1\s*\)""",
    re.IGNORECASE | re.DOTALL,
)
CSS_IMPORT_RE = re.compile(
    r"""@import\s+(?:url\(\s*)?(?P<quote>["']?)
        (?P<url>[^"'\s;)]+)\1\s*\)?""",
    re.IGNORECASE | re.VERBOSE,
)


class _RenderDependencyScanner(HTMLParser):
    """Collect only resource URLs a browser can fetch while rendering."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.references: list[tuple[str, str]] = []
        self.errors: list[str] = []
        self._style_depth = 0

    def _add(self, value: str | None, kind: str) -> None:
        if value is not None and value.strip():
            self.references.append((value.strip(), kind))

    def _add_srcset(self, value: str | None, kind: str) -> None:
        if value is None:
            return
        if "data:" in value.lower():
            self.errors.append(
                "data URLs in srcset are unsupported by the closure parser")
            return
        for candidate in value.split(","):
            url = candidate.strip().split(None, 1)[0]
            if url and not url.lower().startswith("data:"):
                self._add(url, kind)

    def handle_starttag(
            self, tag: str,
            attrs: list[tuple[str, str | None]],
            ) -> None:
        values = {name.lower(): value for name, value in attrs}
        tag = tag.lower()
        if tag == "base" and values.get("href"):
            self.errors.append(
                "base href is forbidden in an isolated render snapshot")
        if (tag == "meta"
                and (values.get("http-equiv") or "").lower() == "refresh"):
            self.errors.append(
                "meta refresh is forbidden in an isolated render snapshot")
        if values.get("style"):
            self.references.extend(
                (url, "inline-style")
                for url in _css_resource_urls(values["style"] or "")
            )
        if tag == "style":
            self._style_depth += 1
        if tag == "script":
            self._add(values.get("src"), "script")
        elif tag == "link":
            rel = {
                item.lower()
                for item in (values.get("rel") or "").split()
            }
            if rel & {
                    "stylesheet", "preload", "modulepreload",
                    "icon", "manifest"}:
                self._add(values.get("href"), "link")
        elif tag in {"img", "source"}:
            self._add(values.get("src"), tag)
            self._add_srcset(values.get("srcset"), f"{tag}-srcset")
        elif tag in {"video", "audio", "track", "embed", "iframe"}:
            self._add(values.get("src"), tag)
            if tag == "video":
                self._add(values.get("poster"), "video-poster")
        elif tag == "object":
            self._add(values.get("data"), "object")
        elif tag == "input" and (values.get("type") or "").lower() == "image":
            self._add(values.get("src"), "input-image")
        elif tag == "image":
            self._add(
                values.get("href") or values.get("xlink:href"),
                "svg-image",
            )

    def handle_endtag(self, tag: str) -> None:
        if tag.lower() == "style" and self._style_depth:
            self._style_depth -= 1

    def handle_data(self, data: str) -> None:
        if self._style_depth:
            self.references.extend(
                (url, "style-block") for url in _css_resource_urls(data))


def _css_resource_urls(css: str) -> list[str]:
    imports = [match.group("url") for match in CSS_IMPORT_RE.finditer(css)]
    urls = [match.group("url") for match in CSS_URL_RE.finditer(css)]
    return imports + urls


def _logical_resource_path(reference: str, base: str) -> str | None:
    """Map one relative browser URL to a canonical isolated-tree path."""
    parsed = urllib.parse.urlsplit(reference.strip())
    scheme = parsed.scheme.lower()
    if scheme == "data":
        return None
    if (scheme or parsed.netloc or reference.startswith("//")
            or parsed.path.startswith("/")):
        raise ValueError(f"external or absolute render dependency: {reference}")
    if parsed.query:
        raise ValueError(
            f"query-bearing render dependency is ambiguous: {reference}")
    if not parsed.path:
        return None
    decoded = urllib.parse.unquote(parsed.path)
    if ("\\" in decoded
            or any(ord(character) < 32 or ord(character) == 127
                   for character in decoded)):
        raise ValueError(f"invalid render dependency path: {reference}")
    joined = posixpath.normpath(
        posixpath.join(posixpath.dirname(base), decoded))
    if (joined in {"", ".", ".."}
            or joined.startswith("../")
            or posixpath.isabs(joined)):
        raise ValueError(
            f"render dependency escapes its snapshot root: {reference}")
    return joined


def discover_render_dependencies(
        html_payload: bytes,
        html_filename: str,
        html_dir: pathlib.Path,
        ) -> tuple[
            dict[str, bytes],
            list[dict[str, Any]],
            list[str],
        ]:
    """Snapshot the recursive local dependency closure of one HTML document."""
    try:
        html_text = html_payload.decode("utf-8")
    except UnicodeDecodeError as exc:
        return {}, [], [f"HTML is not UTF-8: {exc}"]
    scanner = _RenderDependencyScanner()
    scanner.feed(html_text)
    errors = list(scanner.errors)
    root = html_dir.resolve()
    pending = [
        (reference, html_filename, kind)
        for reference, kind in scanner.references
    ]
    payloads: dict[str, bytes] = {}
    metadata: dict[str, dict[str, Any]] = {}
    visited_css: set[str] = set()
    while pending:
        reference, referrer, kind = pending.pop(0)
        try:
            logical = _logical_resource_path(reference, referrer)
        except ValueError as exc:
            errors.append(f"{referrer}: {exc}")
            continue
        if logical is None:
            continue
        item = metadata.setdefault(logical, {
            "path": logical,
            "kinds": set(),
            "referrers": set(),
            "mime_type": None,
            "present": False,
            "bytes": None,
            "sha256": None,
        })
        item["kinds"].add(kind)
        item["referrers"].add(referrer)
        if logical in payloads:
            continue
        candidate = root / pathlib.PurePosixPath(logical)
        try:
            resolved = candidate.resolve(strict=True)
            resolved.relative_to(root)
            if candidate.is_symlink() or resolved != candidate:
                raise ValueError("symlinked dependency path is forbidden")
            payload = _stable_read(resolved)
        except (FileNotFoundError, RuntimeError, ValueError) as exc:
            errors.append(
                f"{referrer}: unresolved render dependency "
                f"{reference!r} ({exc})")
            continue
        payloads[logical] = payload
        mime_type = mimetypes.guess_type(logical)[0]
        if mime_type is None:
            errors.append(
                f"{logical}: render dependency has unknown MIME type")
            continue
        item.update({
            "mime_type": mime_type,
            "present": True,
            "bytes": len(payload),
            "sha256": hashlib.sha256(payload).hexdigest(),
        })
        is_css = (
            logical.lower().endswith(".css")
            or kind in {"inline-style", "style-block", "link"}
            and logical.lower().split("?", 1)[0].endswith(".css")
        )
        if is_css and logical not in visited_css:
            visited_css.add(logical)
            try:
                css = payload.decode("utf-8")
            except UnicodeDecodeError as exc:
                errors.append(f"{logical}: CSS is not UTF-8 ({exc})")
                continue
            pending.extend(
                (nested, logical, "css")
                for nested in _css_resource_urls(css)
            )
    entries = [
        {
            **{key: value for key, value in item.items()
               if key not in {"kinds", "referrers"}},
            "kinds": sorted(item["kinds"]),
            "referrers": sorted(item["referrers"]),
        }
        for _logical, item in sorted(metadata.items())
    ]
    return payloads, entries, sorted(set(errors))


@dataclasses.dataclass(frozen=True)
class InputSnapshot:
    manifest: dict[str, Any]
    contents: dict[str, bytes | None]
    missing_required: tuple[str, ...]
    render_assets: dict[str, bytes] = dataclasses.field(default_factory=dict)
    render_entrypoint: str | None = None


def file_fingerprint(path: pathlib.Path, logical_file: str) -> dict[str, Any]:
    payload = _stable_read(path)
    return bytes_fingerprint(payload, logical_file)


def bytes_fingerprint(payload: bytes, logical_file: str) -> dict[str, Any]:
    return {
        "file": logical_file,
        "bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
    }


def validate_trusted_producer_sources() -> None:
    """Fail if loaded local code or its source path changes during one run."""
    expected = (
        (_AUDIT_SOURCE_PATH, _AUDIT_SOURCE_PAYLOAD, None, "audit"),
        (_TRUSTED_EXTRACT.path, _TRUSTED_EXTRACT.payload,
         _TRUSTED_EXTRACT, "extract"),
        (_TRUSTED_VERIFY.path, _TRUSTED_VERIFY.payload,
         _TRUSTED_VERIFY, "verify"),
    )
    for path, payload, loaded, name in expected:
        if _stable_read(path) != payload:
            raise RuntimeError(
                f"trusted producer source changed after snapshot: {name}")
        if (loaded is not None
                and loaded.module.__dict__.get(
                    "__formgen_source_sha256__") != loaded.sha256):
            raise RuntimeError(f"loaded producer source marker changed: {name}")
    if verify.__dict__.get("extract") is not extract:
        raise RuntimeError(
            "loaded verify module no longer references trusted extract")
    if (sys.modules.get("extract") is not extract
            or sys.modules.get("verify") is not verify):
        raise RuntimeError("trusted producer module binding was substituted")


@functools.lru_cache(maxsize=1)
def _producer_fingerprint_snapshot() -> dict[str, Any]:
    """Loaded dependency bytes plus an honest standalone-attestation scope."""
    validate_trusted_producer_sources()
    files = [
        bytes_fingerprint(
            _AUDIT_SOURCE_PAYLOAD, "tools/formgen/audit.py"),
        bytes_fingerprint(
            _TRUSTED_EXTRACT.payload, "tools/formgen/extract.py"),
        bytes_fingerprint(
            _TRUSTED_VERIFY.payload, "tools/formgen/verify.py"),
    ]
    files[1]["loaded_origin"] = "tools/formgen/extract.py"
    files[1]["executed_from_snapshotted_source"] = True
    files[2]["loaded_origin"] = "tools/formgen/verify.py"
    files[2]["executed_from_snapshotted_source"] = True
    return {
        **files[0],
        "dependencies": files[1:],
        "dependency_execution_bound": True,
        "audit_execution_bound": False,
        "assertion_producer_bound": False,
        "roundtrip_runtime_bound_in_record": False,
        "standalone_attestation_complete": False,
        "incomplete_reason": (
            "audit.py self-execution predates its in-process source snapshot; "
            "clean-bootstrap or clean-commit gate binding is required"
        ),
    }


def producer_fingerprint() -> dict[str, Any]:
    validate_trusted_producer_sources()
    return copy.deepcopy(_producer_fingerprint_snapshot())


def _stable_file_sha256(path: pathlib.Path) -> tuple[int, str]:
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise RuntimeError(
            "runtime closure member could not be opened without following "
            f"a symlink: {path}") from exc
    digest = hashlib.sha256()
    size = 0
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise RuntimeError(
                f"runtime closure member is not regular: {path}")
        while True:
            chunk = os.read(descriptor, 1 << 20)
            if not chunk:
                break
            size += len(chunk)
            digest.update(chunk)
        after = os.fstat(descriptor)
        try:
            path_after = os.stat(path, follow_symlinks=False)
        except OSError as exc:
            raise RuntimeError(
                f"runtime closure path changed while read: {path}") from exc
        stable_fields = (
            "st_dev", "st_ino", "st_mode", "st_size",
            "st_mtime_ns", "st_ctime_ns",
        )
        if (any(getattr(before, field) != getattr(after, field)
                for field in stable_fields)
                or not stat.S_ISREG(path_after.st_mode)
                or (path_after.st_dev, path_after.st_ino)
                != (after.st_dev, after.st_ino)
                or size != after.st_size):
            raise RuntimeError(
                f"runtime closure member changed while read: {path}")
        return size, digest.hexdigest()
    finally:
        os.close(descriptor)


@dataclasses.dataclass(frozen=True)
class _TreeClosure:
    root: pathlib.Path
    entries: tuple[tuple[str, str, int | None, str], ...]

    def manifest(self, logical_root: str) -> dict[str, Any]:
        canonical = json.dumps(
            self.entries, separators=(",", ":"), ensure_ascii=True)
        return {
            "logical_root": logical_root,
            "algorithm": "sha256(canonical-json(path,type,bytes,digest))",
            "files": sum(1 for item in self.entries if item[1] == "file"),
            "symlinks": sum(
                1 for item in self.entries if item[1] == "symlink"),
            "bytes": sum(
                int(item[2] or 0)
                for item in self.entries if item[1] == "file"),
            "tree_sha256": hashlib.sha256(
                canonical.encode("ascii")).hexdigest(),
        }


def _snapshot_tree(root: pathlib.Path) -> _TreeClosure:
    root = root.resolve(strict=True)
    entries: list[tuple[str, str, int | None, str]] = []
    for path in sorted(root.rglob("*"), key=lambda item: item.as_posix()):
        logical = path.relative_to(root).as_posix()
        if path.is_symlink():
            target = os.readlink(path)
            try:
                path.resolve(strict=True).relative_to(root)
            except (FileNotFoundError, ValueError) as exc:
                raise RuntimeError(
                    f"runtime closure symlink escapes root: "
                    f"{logical} -> {target}") from exc
            entries.append((logical, "symlink", None, target))
        elif path.is_file():
            size, digest = _stable_file_sha256(path)
            entries.append((logical, "file", size, digest))
    return _TreeClosure(root=root, entries=tuple(entries))


def _validate_tree_closure(
        closure: _TreeClosure,
        phase: str,
        ) -> None:
    observed = _snapshot_tree(closure.root)
    if observed.entries != closure.entries:
        raise RuntimeError(f"runtime dependency closure changed {phase}")


@dataclasses.dataclass(frozen=True)
class _BoundPlaywrightRuntime:
    playwright: Any
    chromium_path: pathlib.Path
    provenance: dict[str, Any]
    closure: _TreeClosure


_BOUND_PLAYWRIGHT_MODULE_IDENTITIES: dict[str, int] | None = None


def _loaded_playwright_modules() -> dict[str, types.ModuleType]:
    return {
        name: module
        for name, module in sys.modules.items()
        if (name == "playwright" or name.startswith("playwright."))
        and isinstance(module, types.ModuleType)
    }


def _validate_playwright_module_bindings(
        loaded: dict[str, types.ModuleType],
        expected: dict[str, int] | None,
        ) -> None:
    if expected is None:
        if loaded:
            raise RuntimeError(
                "Playwright was imported before its dependency closure "
                "was bound")
        return
    if set(loaded) != set(expected):
        raise RuntimeError(
            "bound Playwright module set changed between uses")
    for name, identity in expected.items():
        if id(loaded[name]) != identity:
            raise RuntimeError(
                f"bound Playwright module was substituted: {name}")


def _playwright_package_root() -> pathlib.Path:
    spec = importlib.machinery.PathFinder.find_spec("playwright", sys.path)
    if spec is None or spec.origin is None:
        raise FileNotFoundError(
            "Playwright is required for a provenance-bound round trip")
    return pathlib.Path(spec.origin).resolve(strict=True).parent


@contextlib.contextmanager
def _bound_playwright_runtime() -> Iterable[_BoundPlaywrightRuntime]:
    """Resolve, bind, use and revalidate one Playwright/Chromium closure."""
    global _BOUND_PLAYWRIGHT_MODULE_IDENTITIES
    package_root = _playwright_package_root()
    closure = _snapshot_tree(package_root)
    preloaded = _loaded_playwright_modules()
    _validate_playwright_module_bindings(
        preloaded, _BOUND_PLAYWRIGHT_MODULE_IDENTITIES)
    old_dont_write = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        with _standard_importers_only():
            import playwright
            from playwright.sync_api import sync_playwright
            origin = pathlib.Path(playwright.__file__).resolve(strict=True)
            try:
                origin.relative_to(package_root)
            except ValueError as exc:
                raise RuntimeError(
                    "loaded Playwright module is outside its bound closure"
                ) from exc
            _validate_tree_closure(closure, "before Playwright use")
            with sync_playwright() as pw:
                chromium_path = pathlib.Path(
                    pw.chromium.executable_path).resolve(strict=True)
                try:
                    chromium_logical = chromium_path.relative_to(
                        package_root).as_posix()
                except ValueError as exc:
                    raise RuntimeError(
                        "resolved Chromium executable is outside Playwright "
                        "closure") from exc
                version_result = subprocess.run(
                    [str(chromium_path), "--version"],
                    check=False,
                    capture_output=True,
                    text=True,
                    timeout=30,
                )
                if version_result.returncode != 0:
                    raise RuntimeError(
                        "could not identify bound Chromium runtime: "
                        + version_result.stderr.strip())
                chromium = file_fingerprint(
                    chromium_path, f"playwright/{chromium_logical}")
                chromium["version_output"] = version_result.stdout.strip()
                provenance = {
                    "mode": "playwright-exact-executable",
                    "playwright_package_version": importlib.metadata.version(
                        "playwright"),
                    "dependency_closure": closure.manifest("playwright"),
                    "chromium": chromium,
                    "same_resolution_session_used_for_render": True,
                    "dependency_closure_validated_before_after": True,
                    "system_shared_libraries_bound": False,
                    "native_host_environment_bound": False,
                    "scope": (
                        "playwright-package-tree-and-explicit-chromium-"
                        "executable"),
                    "scope_complete": False,
                    "incomplete_reason": (
                        "operating-system shared libraries, font services, "
                        "and other native rendering resources loaded by "
                        "Python and Chromium are outside the application-file "
                        "closure"
                    ),
                }
                runtime = _BoundPlaywrightRuntime(
                    playwright=pw,
                    chromium_path=chromium_path,
                    provenance=provenance,
                    closure=closure,
                )
                try:
                    yield runtime
                finally:
                    _validate_tree_closure(
                        closure, "after Playwright use")
                    loaded = _loaded_playwright_modules()
                    for name, module in loaded.items():
                        origin_value = getattr(module, "__file__", None)
                        if origin_value is None:
                            raise RuntimeError(
                                "loaded Playwright module has no bound "
                                f"origin: {name}")
                        module_origin = pathlib.Path(
                            origin_value).resolve(strict=True)
                        try:
                            module_origin.relative_to(package_root)
                        except ValueError as exc:
                            raise RuntimeError(
                                "loaded Playwright module escaped its "
                                f"closure: {name}") from exc
                    identities = {
                        name: id(module)
                        for name, module in loaded.items()
                    }
                    if _BOUND_PLAYWRIGHT_MODULE_IDENTITIES is None:
                        _BOUND_PLAYWRIGHT_MODULE_IDENTITIES = identities
                    else:
                        for name, identity in (
                                _BOUND_PLAYWRIGHT_MODULE_IDENTITIES.items()):
                            if identities.get(name) != identity:
                                raise RuntimeError(
                                    "bound Playwright module identity "
                                    f"changed: {name}")
                        _BOUND_PLAYWRIGHT_MODULE_IDENTITIES.update(identities)
    finally:
        sys.dont_write_bytecode = old_dont_write


def roundtrip_runtime_provenance() -> dict[str, Any]:
    """Inspect the exact closure a real round trip will resolve and reuse."""
    with _bound_playwright_runtime() as runtime:
        return copy.deepcopy(runtime.provenance)


def _runtime_bound_paths() -> dict[str, pathlib.Path]:
    paths = {"python/executable": pathlib.Path(sys.executable).resolve()}
    library = sysconfig.get_config_var("LDLIBRARY")
    library_dir = sysconfig.get_config_var("LIBDIR")
    if library and library_dir:
        candidate = pathlib.Path(library_dir) / str(library)
        if candidate.is_file():
            paths["python/runtime-library"] = candidate.resolve()
    for name, module in sorted(sys.modules.items()):
        if not (name == "fitz" or name.startswith("pymupdf")):
            continue
        origin = getattr(module, "__file__", None)
        if origin and pathlib.Path(origin).is_file():
            paths[f"module/{name}"] = pathlib.Path(origin).resolve()
    return paths


@functools.lru_cache(maxsize=1)
def _base_runtime_snapshot(
        ) -> tuple[tuple[str, pathlib.Path, int, str], ...]:
    records = []
    for logical, path in sorted(_runtime_bound_paths().items()):
        size, digest = _stable_file_sha256(path)
        records.append((logical, path, size, digest))
    return tuple(records)


def validate_base_runtime() -> None:
    snapshot = _base_runtime_snapshot()
    expected_paths = {
        logical: path for logical, path, _size, _sha in snapshot}
    if _runtime_bound_paths() != expected_paths:
        raise RuntimeError(
            "bound Python/PyMuPDF loaded-module closure changed")
    for logical, path, expected_size, expected_sha in snapshot:
        size, digest = _stable_file_sha256(path)
        if size != expected_size or digest != expected_sha:
            raise RuntimeError(
                f"bound Python/PyMuPDF runtime changed: {logical}")


@functools.lru_cache(maxsize=1)
def _runtime_provenance_snapshot() -> dict[str, Any]:
    """Interpreter/PyMuPDF application files, with scope stated honestly."""
    import fitz
    records = _base_runtime_snapshot()
    canonical = json.dumps(
        [(logical, size, digest)
         for logical, _path, size, digest in records],
        separators=(",", ":"),
    )
    return {
        "python": {
            "implementation": platform.python_implementation(),
            "version": platform.python_version(),
            "cache_tag": sys.implementation.cache_tag,
        },
        "pymupdf": {
            "package_version": str(getattr(fitz, "__version__", "")),
            "version_bind": str(getattr(fitz, "VersionBind", "")),
        },
        "loaded_application_files": {
            "algorithm": (
                "sha256(canonical-json(logical-file,bytes,sha256))"),
            "files": len(records),
            "bytes": sum(item[2] for item in records),
            "tree_sha256": hashlib.sha256(
                canonical.encode("ascii")).hexdigest(),
            "members": [
                {
                    "file": logical,
                    "bytes": size,
                    "sha256": digest,
                }
                for logical, _path, size, digest in records
            ],
            "validated_before_after": True,
        },
        "stdlib_and_system_shared_libraries_bound": False,
        "scope_complete": False,
        "incomplete_reason": (
            "standalone audit binds the interpreter executable, runtime "
            "library, and loaded PyMuPDF application modules, not every "
            "standard-library or operating-system shared library"
        ),
    }


def runtime_provenance() -> dict[str, Any]:
    validate_base_runtime()
    return copy.deepcopy(_runtime_provenance_snapshot())


def snapshot_inputs(slug: str, ir_dir: pathlib.Path, html_dir: pathlib.Path,
                    layout_dir: pathlib.Path,
                    guide_dir: pathlib.Path | None,
                    source_root: str) -> InputSnapshot:
    """Read and hash the exact bytes one form's audit will consume.

    Paths in the manifest are logical filenames rather than absolute paths, so
    the same inputs publish byte-identical evidence in another checkout.
    `guide_html` is optional because only forms with relocated guide content
    emit one; the guide plan itself is required for every form. The official
    PDF is resolved from the snapshotted IR, read once, and retained as bytes so
    a path mutation cannot change what later assertions evaluate.
    """
    validate_trusted_producer_sources()
    validate_base_runtime()
    specs = (
        ("ir", ir_dir / f"{slug}.ir.json", True),
        ("layout", layout_dir / f"{slug}.layout.json", True),
        ("html", html_dir / f"{slug}.html", True),
        ("guide", guide_dir / f"{slug}.guide.json" if guide_dir else None, True),
        ("guide_html", html_dir / f"{slug}.guide.html", False),
    )
    entries: dict[str, dict[str, Any]] = {}
    contents: dict[str, bytes | None] = {}
    missing: list[str] = []
    for role, path, required in specs:
        filename = path.name if path is not None else (
            f"{slug}.guide.json" if role == "guide" else role)
        if path is None or not path.is_file():
            entries[role] = {
                "file": filename,
                "required": required,
                "present": False,
                "bytes": None,
                "sha256": None,
            }
            contents[role] = None
            if required:
                missing.append(role)
            continue
        payload = _stable_read(path)
        entries[role] = {
            "file": filename,
            "required": required,
            "present": True,
            "bytes": len(payload),
            "sha256": hashlib.sha256(payload).hexdigest(),
        }
        contents[role] = payload

    source_identity = ""
    source_name = f"{slug}.source.pdf"
    expected_source_sha: str | None = None
    source_resolution: tuple[pathlib.Path, bytes] | None = None
    ir_payload = contents.get("ir")
    if ir_payload is not None:
        try:
            snapshotted_ir = json.loads(ir_payload.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            snapshotted_ir = None
        if isinstance(snapshotted_ir, dict):
            source = snapshotted_ir.get("source") or {}
            source_identity = str(source.get("file", ""))
            source_name = source_identity.split(":", 1)[-1] or source_name
            wanted = source.get("sha256")
            expected_source_sha = str(wanted) if wanted is not None else None
            source_resolution = resolve_source_payload(
                snapshotted_ir, source_root)

    if source_resolution is None:
        entries["source_pdf"] = {
            "file": source_name,
            "logical_identity": source_identity,
            "path": None,
            "required": True,
            "present": False,
            "bytes": None,
            "sha256": None,
            "expected_sha256": expected_source_sha,
        }
        contents["source_pdf"] = None
        missing.append("source_pdf")
    else:
        source_path, source_payload = source_resolution
        source_base = pathlib.Path(source_root).expanduser()
        try:
            logical_path = source_path.relative_to(source_base).as_posix()
        except ValueError:
            logical_path = source_path.name
        source_sha = hashlib.sha256(source_payload).hexdigest()
        entries["source_pdf"] = {
            "file": source_name,
            "logical_identity": source_identity,
            "path": logical_path,
            "required": True,
            "present": True,
            "bytes": len(source_payload),
            "sha256": source_sha,
            "expected_sha256": expected_source_sha,
        }
        contents["source_pdf"] = source_payload
    render_entrypoint = entries["html"]["file"]
    render_assets: dict[str, bytes] = {}
    render_dependencies: list[dict[str, Any]] = []
    render_errors: list[str] = []
    if contents.get("html") is not None:
        render_assets, render_dependencies, render_errors = (
            discover_render_dependencies(
                contents["html"] or b"",
                render_entrypoint,
                html_dir,
            )
        )
    if render_errors:
        missing.append("render_dependencies")
    producer = producer_fingerprint()
    manifest = {
        "schema": INPUT_MANIFEST_SCHEMA,
        "algorithm": "sha256",
        "producer": producer,
        "runtime": runtime_provenance(),
        "inputs_complete": not missing,
        "attestation_complete": bool(
            not missing and producer["standalone_attestation_complete"]),
        "enforceable": bool(
            not missing and producer["standalone_attestation_complete"]),
        "complete": bool(
            not missing and producer["standalone_attestation_complete"]),
        "missing_required": missing,
        "inputs": entries,
        "render": {
            "entrypoint": render_entrypoint,
            "dependencies": render_dependencies,
            "errors": render_errors,
            "complete": not render_errors,
            "network_policy": (
                "deny-except-retained-relative-resources-and-inline-data"),
        },
    }
    return InputSnapshot(manifest=manifest, contents=contents,
                         missing_required=tuple(missing),
                         render_assets=render_assets,
                         render_entrypoint=render_entrypoint)


def empty_input_manifest() -> dict[str, Any]:
    """Fail-closed placeholder retained even if input snapshotting raises."""
    return {
        "schema": INPUT_MANIFEST_SCHEMA,
        "algorithm": "sha256",
        "producer": producer_fingerprint(),
        "runtime": runtime_provenance(),
        "inputs_complete": False,
        "attestation_complete": False,
        "enforceable": False,
        "complete": False,
        "missing_required": list(REQUIRED_INPUT_ROLES),
        "inputs": {},
        "render": {
            "entrypoint": None,
            "dependencies": [],
            "errors": ["input snapshot did not complete"],
            "complete": False,
            "network_policy": (
                "deny-except-retained-relative-resources-and-inline-data"),
        },
    }


@dataclasses.dataclass
class Bundle:
    slug: str
    ir: dict
    layout: dict | None
    plan: dict | None
    form_html: str | None
    guide_html: str | None
    pdf: bytes | pathlib.Path | None
    form_html_bytes: bytes | None = None
    render_assets: dict[str, bytes] = dataclasses.field(default_factory=dict)
    render_entrypoint: str | None = None
    layout_payload: bytes | None = None
    layout_sha256: str | None = None

    @functools.cached_property
    def pages(self) -> dict[int, dict]:
        return {p["index"]: p for p in self.ir["pages"]}

    @functools.cached_property
    def layout_pages(self) -> dict[int, dict]:
        return {p["index"]: p for p in (self.layout or {}).get("pages", ())}

    @functools.cached_property
    def cells(self) -> list[Cell]:
        return parse_cells(self.form_html) if self.form_html else []

    @functools.cached_property
    def layout_cells(self) -> dict[str, dict]:
        return {c["id"]: c for p in self.layout_pages.values() for c in p["cells"]}

    @functools.cached_property
    def regions(self) -> list[dict]:
        return list((self.plan or {}).get("inline") or ())

    @functools.cached_property
    def relocated_cells(self) -> set[str]:
        out: set[str] = set()
        for region in self.regions:
            out.update(region.get("cell_ids") or ())
        return out

    @functools.cached_property
    def relocated_runs(self) -> set[tuple[int, int]]:
        out: set[tuple[int, int]] = set()
        for region in self.regions:
            for index in region.get("text_run_indices") or ():
                out.add((region["page"], index))
        return out

    @functools.cached_property
    def emitted_runs(self) -> dict[tuple[int, int], str]:
        return parse_run_styles(self.form_html) if self.form_html else {}

    @functools.cached_property
    def ink(self) -> dict[int, InkIndex]:
        """Printed glyph ink, per page, for the runs the form document emits."""
        boxes: dict[int, list[tuple[Rect, Any]]] = collections.defaultdict(list)
        for (page, index) in self.emitted_runs:
            run = self.pages.get(page, {}).get("text_runs", [])[index:index + 1]
            if not run:
                continue
            for box in glyph_boxes(run[0]):
                boxes[page].append((box, index))
        return {page: InkIndex(items) for page, items in boxes.items()}

    @functools.cached_property
    def doc(self):
        if self.pdf is None:
            return None
        import fitz  # local import: only the PDF oracles need it
        if isinstance(self.pdf, bytes):
            return fitz.open(stream=self.pdf, filetype="pdf")
        return fitz.open(self.pdf)

    @functools.cached_property
    def vector_pages(self) -> dict[int, VectorPage]:
        if self.doc is None:
            return {}
        return {index: ordered_vector_paints(self.doc[index - 1])
                for index in self.pages}

    def run_text(self, page: int, index: int) -> str:
        runs = self.pages.get(page, {}).get("text_runs", [])
        return runs[index]["text"] if 0 <= index < len(runs) else ""

    def close(self) -> None:
        if "doc" in self.__dict__ and self.__dict__["doc"] is not None:
            self.__dict__["doc"].close()


def held(**detail: Any) -> dict[str, Any]:
    return {"holds": True, "reason": "", "offenders": [], **detail}


def broken(reason: str, offenders: Sequence[Any] = (),
           *, offender_limit: int | None = MAX_OFFENDERS,
           **detail: Any) -> dict[str, Any]:
    all_offenders = list(offenders)
    published = (all_offenders if offender_limit is None
                 else all_offenders[:offender_limit])
    return {"holds": False, "reason": reason,
            "offender_count": len(all_offenders),
            "offenders_published": len(published),
            "offenders_omitted": len(all_offenders) - len(published),
            "offenders_complete": len(published) == len(all_offenders),
            "offenders": published, **detail}


# --------------------------------------------------------------------------
# assertion 1 -- no input over pre-printed text
# --------------------------------------------------------------------------


def check_inputs_over_printed_text(b: Bundle) -> dict[str, Any]:
    """C6's belt: a taxpayer must not be able to type over the printed form.

    The defect this catches is a statutory rate table emitted as fields -- on
    1700 page 2 the DOM really does offer an editable box over
    "Not over P 250,000". Scored against glyph ink, so the decorative `.` inside
    a money comb is the only kind of hit that is arguable, and it is reported
    rather than excused: a slot that already holds ink is a slot nothing should
    be typed into.
    """
    if b.form_html is None:
        return broken("no emitted form document to check")
    binding_issues = emitted_cell_binding_issues(b)
    if binding_issues:
        return broken(
            f"{len(binding_issues)} emitted cell binding issue(s)",
            binding_issues,
            offender_limit=None,
            cells_checked=len(b.cells),
            emitted_cell_binding_issues=len(binding_issues),
        )
    offenders = []
    for cell in b.cells:
        index = b.ink.get(cell.page)
        if index is None:
            continue
        for box in input_boxes(cell):
            hit = index.any_hit(box)
            if hit is None:
                continue
            offenders.append({
                "cell": cell.id,
                "page": cell.page,
                "run": hit[1],
                "text": b.run_text(cell.page, hit[1])[:60],
            })
            break
    if offenders:
        return broken(f"{len(offenders)} input(s) sit on printed glyph ink", offenders,
                      cells_checked=len(b.cells))
    return held(
        cells_checked=len(b.cells), emitted_cell_binding_issues=0)


# --------------------------------------------------------------------------
# assertion 2 -- comb slots equal printed compartments
# --------------------------------------------------------------------------


def check_comb_slots_match_printed(b: Bundle) -> dict[str, Any]:
    """C5: a slot count short of the printed one centres a digit on a bar.

    The printed count comes from `printed_compartments`, i.e. from the source
    PDF's drawing operators, because two existing oracles disagree about this
    and one of them is lattice.py's own comb code.

    The verdict is scored against the *emitted* slot count, because that is the
    comb a taxpayer types into. The lattice's own slot count is recorded beside
    it: the two differ whenever the emitted document predates a lattice change,
    and telling that apart from a real merge is the difference between a
    regeneration and a fix. Layout and emission relations are published
    independently; a malformed or duplicate emitted comb remains an offender
    even when the source and lattice counts agree.
    """
    if b.layout is None:
        return broken("no layout to read comb geometry from")
    if b.doc is None:
        return broken("source PDF not resolved; printed compartments unknown")
    owner_registry = reviewed_comb_owner_registry(b)
    form_html = getattr(b, "form_html", None)
    emitted_by_id: dict[str, list[Cell]] = collections.defaultdict(list)
    for emitted_cell in b.cells:
        emitted_by_id[emitted_cell.id].append(emitted_cell)
    raw_inventory_issues = (
        live_comb_inventory_issues(form_html, b.cells)
        if isinstance(form_html, str) else []
    )
    all_cell_binding_issues = emitted_cell_binding_issues(b)
    emitted_comb_ids = sorted(
        cell_id for cell_id, cells in emitted_by_id.items()
        if any(
            cell.comb_slots_attr is not None
            or SLOT_RE.search(cell.inner) is not None
            or 'data-field-kind="comb"' in cell.attrs
            or "data-comb-capacity=" in cell.attrs
            for cell in cells
        )
    )
    duplicate_emitted_ids = sorted(
        cell_id for cell_id, cells in emitted_by_id.items()
        if len(cells) != 1
    )

    layout_subjects: dict[
        str, list[tuple[int, dict[str, Any]]]
    ] = collections.defaultdict(list)
    all_layout_comb_count = 0
    reported_comb_count = 0
    reported_stats_present = False
    for page_index, page in sorted(b.layout_pages.items()):
        stats = page.get("stats")
        if isinstance(stats, dict) and "comb_cells" in stats:
            reported_stats_present = True
            try:
                reported_comb_count += int(stats["comb_cells"])
            except (TypeError, ValueError):
                reported_comb_count = -1
        for cell in page["cells"]:
            comb = cell.get("comb")
            if not comb:
                continue
            all_layout_comb_count += 1
            if cell["id"] in b.relocated_cells:
                continue
            layout_subjects[cell["id"]].append((page_index, cell))

    # Preserve page/cell document order so exhaustive offender publication is
    # inspectable in the same order as the owning layout.
    expected_ids = list(layout_subjects)
    duplicate_layout_ids = [
        cell_id for cell_id, subjects in layout_subjects.items()
        if len(subjects) != 1
    ]
    # Relocated cells belong to the non-interactive guide document. A live comb
    # with that id in the form document is stale duplicate markup, not an
    # allowed relocation.
    allowed_emitted_ids = set(expected_ids)
    unexpected_emitted_ids = sorted(
        set(emitted_comb_ids) - allowed_emitted_ids
    )
    covered_comb_ids = set(expected_ids) | set(emitted_comb_ids)
    uncovered_cell_binding_issues = [
        issue for issue in all_cell_binding_issues
        if issue["cell"] not in covered_comb_ids
    ]

    offenders: list[dict[str, Any]] = []
    checked_ids: list[str] = []
    layout_mismatches = 0
    layout_unevaluable = 0
    stale_emission = 0
    emission_invalid = 0
    owner_certificates_valid = 0
    owner_certificates_invalid = 0
    source_u_frame_evaluable = 0
    source_certified_unframed_evaluable = 0
    if owner_registry.binding_error is not None:
        # Registry integrity is assertion-wide. It must fail even when every
        # active comb is relocated, or when a malformed retained-only ledger
        # has no active comb cell to enter the per-cell loop below.
        offenders.append({
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
            "source_owner_certificate": {
                "criterion": "exact-reviewed-layout-comb-subject-owner-v1",
                "valid": False,
                "reason": owner_registry.binding_error,
                "supplies_topology": False,
            },
            "layout_relation": "registry-invalid",
            "emission_relation": "not-evaluated",
            "failure_kinds": ["comb-owner-registry-invalid"],
            "why": (
                "reviewed comb owner registry is globally invalid: "
                f"{owner_registry.binding_error}"
            ),
        })
    for cell_id in expected_ids:
        subjects = layout_subjects[cell_id]
        checked_ids.append(cell_id)
        emission = emitted_comb_evidence(emitted_by_id.get(cell_id, ()))
        slots = emission["slots"]
        base_stale_emission = (
            not emission["valid"]
            or len(subjects) != 1
            or (len(subjects) == 1
                and slots != subjects[0][1]["comb"]["cells"])
        )
        if len(subjects) != 1:
            stale_emission += int(base_stale_emission)
            emission_invalid += int(not emission["valid"])
            page_index, cell = subjects[0]
            owner_certificates_invalid += 1
            owner_certificate_reason = (
                owner_registry.binding_error
                or f"layout contains {len(subjects)} non-unique comb owners "
                   f"for {cell_id}"
            )
            offenders.append({
                "cell": cell_id,
                "page": page_index,
                "slots": slots,
                "latticed": None,
                "printed": None,
                "printed_divider_x": [],
                "emission_state": emission["state"],
                "physical_slots": emission["physical_slots"],
                "declared_slots": emission["declared_slots"],
                "emitted_occurrences": emission["occurrences"],
                "source_owner_certificate": {
                    "criterion": (
                        "exact-reviewed-layout-comb-subject-owner-v1"),
                    "valid": False,
                    "reason": owner_certificate_reason,
                    "supplies_topology": False,
                },
                "layout_relation": "duplicate-subject",
                "emission_relation": (
                    "invalid" if not emission["valid"] else "unbound"),
                "failure_kinds": ["duplicate-layout-subject"],
                "why": (
                    f"layout contains {len(subjects)} comb subjects with this "
                    "id; exactly one is required"),
            })
            layout_unevaluable += 1
            continue

        page_index, cell = subjects[0]
        emitted_cells = emitted_by_id.get(cell_id, ())
        emitted_cell = emitted_cells[0] if len(emitted_cells) == 1 else None
        emitted_dom_page = (
            emitted_cell.dom_page
            if emitted_cell is not None else None
        )
        # Direct unit fixtures predate DOM parsing. Production cells parsed
        # from real HTML must carry the actual enclosing `.page page-N`.
        if (emitted_cell is not None and emitted_dom_page is None
                and not isinstance(form_html, str)):
            emitted_dom_page = emitted_cell.page
        expected_rect = (
            float(cell["x0"]), float(cell["y0"]),
            float(cell["x1"]), float(cell["y1"]),
        )
        actual_rect = emitted_cell.rect if emitted_cell is not None else None
        rect_deltas = (
            [actual - expected
             for actual, expected in zip(actual_rect, expected_rect)]
            if actual_rect is not None else None
        )
        page_binding_matches = (
            emitted_cell is not None
            and emitted_cell.page == page_index
            and emitted_dom_page == page_index
        )
        rect_binding_matches = (
            rect_deltas is not None
            and all(abs(delta) <= EMITTED_GEOMETRY_EPS_PT
                    for delta in rect_deltas)
        )
        container_binding = {
            "expected_page": page_index,
            "emitted_id_page": (
                emitted_cell.page if emitted_cell is not None else None),
            "emitted_dom_page": emitted_dom_page,
            "page_matches": page_binding_matches,
            "expected_rect": list(expected_rect),
            "actual_rect": (
                list(actual_rect) if actual_rect is not None else None),
            "rect_deltas_pt": rect_deltas,
            "rect_matches": rect_binding_matches,
            "tolerance_pt": EMITTED_GEOMETRY_EPS_PT,
        }

        actual_slot_edges = _emitted_slot_edges(emitted_cell, emission)
        actual_internal_edges = (
            actual_slot_edges[1:-1]
            if actual_slot_edges is not None else None)
        layout_slot_x = cell["comb"].get("slot_x")
        layout_all_edges: list[float] | None = None
        layout_internal_edges: list[float] | None = None
        layout_position_reason: str | None = None
        if (isinstance(layout_slot_x, list)
                and len(layout_slot_x) == int(cell["comb"]["cells"]) + 1):
            try:
                layout_all_edges = [
                    float(value) for value in layout_slot_x]
                layout_internal_edges = layout_all_edges[1:-1]
            except (TypeError, ValueError):
                layout_position_reason = (
                    "layout comb slot_x contains a non-numeric coordinate")
        else:
            layout_position_reason = (
                "layout comb lacks the complete cells-plus-one slot_x vector")
        layout_position = _position_evidence(
            actual_internal_edges,
            layout_internal_edges,
            comparable=bool(
                emission["valid"] and slots == cell["comb"]["cells"]),
            unavailable_reason=(
                layout_position_reason
                or ("emitted/layout slot counts differ"
                    if slots != cell["comb"]["cells"]
                    else "emitted slot geometry is invalid")),
        )
        layout_outer_position = _outer_position_evidence(
            (
                [actual_slot_edges[0], actual_slot_edges[-1]]
                if actual_slot_edges is not None else None
            ),
            (
                [layout_all_edges[0], layout_all_edges[-1]]
                if layout_all_edges is not None else None
            ),
            comparable=bool(
                emission["valid"] and slots == cell["comb"]["cells"]),
            unavailable_reason=(
                layout_position_reason
                or ("emitted/layout slot counts differ"
                    if slots != cell["comb"]["cells"]
                    else "emitted slot geometry is invalid")),
        )
        vector_page = b.vector_pages.get(page_index)
        latticed = cell["comb"]["cells"]
        owner_certificate, owner_certificate_error = owner_registry.resolve(
            page_index, cell)
        if owner_certificate is None:
            owner_certificates_invalid += 1
            owner_certificate_evidence = {
                "criterion": "exact-reviewed-layout-comb-subject-owner-v1",
                "valid": False,
                "reason": owner_certificate_error,
                "supplies_topology": False,
            }
        else:
            owner_certificates_valid += 1
            owner_certificate_evidence = owner_certificate.evidence()
        evidence = {
            "cell": cell_id,
            "page": page_index,
            "slots": slots,
            "latticed": latticed,
            "emission_state": emission["state"],
            "physical_slots": emission["physical_slots"],
            "declared_slots": emission["declared_slots"],
            "emitted_occurrences": emission["occurrences"],
            "slot_indexes": emission.get("slot_indexes"),
            "input_slot_indexes": emission.get("input_slot_indexes"),
            "slot_geometry": emission.get("slot_geometry"),
            "emission_container_binding": container_binding,
            "emission_layout_position": layout_position,
            "emission_layout_outer_position": layout_outer_position,
            "source_owner_certificate": owner_certificate_evidence,
        }
        failure_kinds: list[str] = []
        reasons: list[str] = []
        source_frame: dict[str, Any] | None = None
        if owner_certificate is None:
            printed = None
            xs = []
            layout_relation = "unevaluable"
            failure_kinds.append("source-topology-unevaluable")
            reasons.append(
                "invalid reviewed source owner certificate: "
                f"{owner_certificate_error}")
            evidence["source_topology_evidence"] = {
                "criterion": "exact-reviewed-layout-comb-subject-owner-v1",
                "owner_certificate": owner_certificate_evidence,
            }
            layout_unevaluable += 1
        elif vector_page is None:
            printed = None
            xs: list[float] = []
            layout_relation = "unevaluable"
            failure_kinds.append("source-topology-unevaluable")
            reasons.append(f"page {page_index} has no source vector paint")
            layout_unevaluable += 1
        else:
            try:
                printed, xs, source_frame = printed_compartments(
                    vector_page,
                    cell,
                    include_frame=True,
                    owner_certificate=owner_certificate,
                )
            except ValueError as exc:
                printed = None
                xs = []
                layout_relation = "unevaluable"
                failure_kinds.append("source-topology-unevaluable")
                reasons.append(f"unevaluable final-paint topology: {exc}")
                if isinstance(exc, CombTopologyError):
                    evidence["source_topology_evidence"] = exc.evidence
                layout_unevaluable += 1
            else:
                if source_frame is None:
                    source_certified_unframed_evaluable += 1
                else:
                    source_u_frame_evaluable += 1
                if latticed == printed:
                    layout_relation = "match"
                else:
                    layout_relation = "mismatch"
                    layout_mismatches += 1
                    failure_kinds.append("layout-printed-mismatch")
                    reasons.append(
                        f"layout has {latticed} slots but source prints "
                        f"{printed} compartments")

        source_position = _position_evidence(
            actual_internal_edges,
            xs if printed is not None else None,
            comparable=bool(
                emission["valid"]
                and printed is not None
                and slots == printed
            ),
            unavailable_reason=(
                "source topology is unevaluable"
                if printed is None else
                "emitted/source slot counts differ"
                if slots != printed else
                "emitted slot geometry is invalid"
            ),
        )
        evidence["emission_source_position"] = source_position
        source_outer_edges = (
            [
                float(source_frame["left_rail"]["center_x"]),
                float(source_frame["right_rail"]["center_x"]),
            ]
            if source_frame is not None else None
        )
        emission_source_outer_position = _outer_position_evidence(
            (
                [actual_slot_edges[0], actual_slot_edges[-1]]
                if actual_slot_edges is not None else None
            ),
            source_outer_edges,
            comparable=bool(
                emission["valid"]
                and printed is not None
                and slots == printed
                and source_frame is not None
            ),
            unavailable_reason=(
                "source U-frame geometry is unevaluable"
                if source_frame is None else
                "emitted/source slot counts differ"
                if slots != printed else
                "emitted slot geometry is invalid"
            ),
        )
        layout_source_outer_position = _outer_position_evidence(
            (
                [layout_all_edges[0], layout_all_edges[-1]]
                if layout_all_edges is not None else None
            ),
            source_outer_edges,
            comparable=bool(
                printed is not None
                and latticed == printed
                and source_frame is not None
            ),
            unavailable_reason=(
                "source U-frame geometry is unevaluable"
                if source_frame is None else
                "layout/source slot counts differ"
                if latticed != printed else
                layout_position_reason
            ),
        )
        evidence.update({
            "source_frame_geometry": source_frame,
            "emission_source_outer_position": (
                emission_source_outer_position),
            "layout_source_outer_position": layout_source_outer_position,
        })

        binding_invalid = False
        if emitted_cell is not None and not page_binding_matches:
            binding_invalid = True
            failure_kinds.append("emission-container-page-mismatch")
            reasons.append(
                "emitted cell id page, enclosing DOM page, and layout page "
                f"must all be {page_index}; got id page "
                f"{emitted_cell.page} and DOM page {emitted_dom_page}")
        if emitted_cell is not None and not rect_binding_matches:
            binding_invalid = True
            failure_kinds.append("emission-container-geometry-mismatch")
            reasons.append(
                "emitted comb container does not occupy its layout cell "
                f"within {EMITTED_GEOMETRY_EPS_PT}pt")
        if (layout_position["comparable"]
                and not layout_position["matches"]):
            binding_invalid = True
            failure_kinds.append("emission-layout-position-mismatch")
            reasons.append(
                "emitted internal slot edges do not match layout comb.slot_x "
                f"within {EMITTED_GEOMETRY_EPS_PT}pt")
        if (layout_outer_position["comparable"]
                and not layout_outer_position["matches"]):
            binding_invalid = True
            failure_kinds.append("emission-layout-outer-position-mismatch")
            reasons.append(
                "emitted physical outer slot edges do not match layout "
                f"comb.slot_x within {EMITTED_GEOMETRY_EPS_PT}pt")
        if (source_position["comparable"]
                and not source_position["matches"]):
            binding_invalid = True
            failure_kinds.append("emission-source-position-mismatch")
            reasons.append(
                "emitted internal slot edges do not match independently "
                "measured source dividers within "
                f"{EMITTED_GEOMETRY_EPS_PT}pt")
        if (emission_source_outer_position["comparable"]
                and not emission_source_outer_position["matches"]):
            binding_invalid = True
            failure_kinds.append("emission-source-outer-position-mismatch")
            reasons.append(
                "emitted physical outer slot edges do not match source "
                f"U-frame rails within {EMITTED_GEOMETRY_EPS_PT}pt")
        if (layout_source_outer_position["comparable"]
                and not layout_source_outer_position["matches"]):
            binding_invalid = True
            failure_kinds.append("layout-source-outer-position-mismatch")
            reasons.append(
                "layout comb.slot_x outer edges do not match source U-frame "
                f"rails within {EMITTED_GEOMETRY_EPS_PT}pt")
        evidence["effective_emission_state"] = (
            "container-binding-invalid"
            if any(kind.startswith("emission-container-")
                   for kind in failure_kinds)
            else "slot-position-invalid"
            if any(kind in {
                "emission-layout-position-mismatch",
                "emission-layout-outer-position-mismatch",
                "emission-source-position-mismatch",
                "emission-source-outer-position-mismatch",
                "layout-source-outer-position-mismatch",
            } for kind in failure_kinds)
            else emission["state"]
        )

        stale_emission += int(base_stale_emission or binding_invalid)
        emission_invalid += int(not emission["valid"] or binding_invalid)
        if not emission["valid"]:
            emission_relation = "invalid"
            failure_kinds.append("invalid-emission")
            reasons.append(emission["reason"])
        else:
            emission_relations = []
            if slots != latticed:
                emission_relations.append("layout")
                failure_kinds.append("emission-layout-mismatch")
                reasons.append(
                    f"emission has {slots} slots but layout has {latticed}")
            if printed is not None and slots != printed:
                emission_relations.append("printed")
                failure_kinds.append("emission-printed-mismatch")
                reasons.append(
                    f"emission has {slots} slots but source prints {printed}")
            emission_relation = (
                "mismatch-" + "-and-".join(emission_relations)
                if emission_relations else "match"
            )
            if binding_invalid:
                emission_relation = "invalid"
        if failure_kinds:
            offenders.append({
                **evidence,
                "printed": printed,
                "printed_divider_x": [
                    round(value, 6) for value in xs],
                "layout_relation": layout_relation,
                "emission_relation": emission_relation,
                "failure_kinds": failure_kinds,
                "why": "; ".join(reasons),
            })

    for cell_id in unexpected_emitted_ids:
        emission = emitted_comb_evidence(emitted_by_id[cell_id])
        stale_emission += 1
        emission_invalid += int(not emission["valid"])
        cells = emitted_by_id[cell_id]
        offenders.append({
            "cell": cell_id,
            "page": cells[0].page,
            "slots": emission["slots"],
            "latticed": None,
            "printed": None,
            "printed_divider_x": [],
            "emission_state": emission["state"],
            "physical_slots": emission["physical_slots"],
            "declared_slots": emission["declared_slots"],
            "emitted_occurrences": emission["occurrences"],
            "slot_indexes": emission.get("slot_indexes"),
            "input_slot_indexes": emission.get("input_slot_indexes"),
            "slot_geometry": emission.get("slot_geometry"),
            "layout_relation": "not-owned",
            "emission_relation": "unexpected",
            "failure_kinds": ["unexpected-emitted-comb"],
            "why": (
                "emitted comb is not owned by a non-relocated layout subject"),
        })

    for issue in uncovered_cell_binding_issues:
        offenders.append({
            "cell": issue["cell"],
            "page": issue.get("emitted_dom_page"),
            "slots": None,
            "latticed": None,
            "printed": None,
            "printed_divider_x": [],
            "emission_state": "emitted-cell-binding-invalid",
            "effective_emission_state": "emitted-cell-binding-invalid",
            "physical_slots": None,
            "declared_slots": None,
            "emitted_occurrences": issue["emitted_occurrences"],
            "layout_relation": "cell-binding-invalid",
            "emission_relation": "invalid",
            "failure_kinds": [
                "emitted-cell-binding-invalid",
                *issue["failure_kinds"],
            ],
            "emitted_cell_binding_evidence": issue,
            "why": issue["why"],
        })

    for index, issue in enumerate(raw_inventory_issues, 1):
        stale_emission += 1
        emission_invalid += 1
        offenders.append({
            "cell": (
                issue.get("cell_id")
                or f"<live-comb-{index}>"),
            "page": issue.get("dom_page"),
            "slots": issue.get("slot_count"),
            "latticed": None,
            "printed": None,
            "printed_divider_x": [],
            "emission_state": "unowned-live-comb-markup",
            "effective_emission_state": "unowned-live-comb-markup",
            "physical_slots": issue.get("slot_count"),
            "declared_slots": None,
            "emitted_occurrences": 1,
            "layout_relation": "not-owned",
            "emission_relation": "invalid",
            "failure_kinds": ["unowned-live-comb-markup"],
            "raw_dom_evidence": issue,
            "why": issue["reason"],
        })

    inventory_failures: list[str] = []
    if (reported_stats_present
            and reported_comb_count != all_layout_comb_count):
        inventory_failures.append(
            "layout stats report "
            f"{reported_comb_count} combs but page cells contain "
            f"{all_layout_comb_count}")
    if (not expected_ids and not emitted_comb_ids
            and reported_stats_present and reported_comb_count > 0):
        inventory_failures.append(
            "comb inventory is empty despite a positive layout stats signal")
    if inventory_failures:
        offenders.append({
            "cell": "<comb-inventory>",
            "page": None,
            "slots": None,
            "latticed": None,
            "printed": None,
            "printed_divider_x": [],
            "emission_state": "inventory-invalid",
            "physical_slots": None,
            "declared_slots": None,
            "emitted_occurrences": 0,
            "layout_relation": "inventory-invalid",
            "emission_relation": "inventory-invalid",
            "failure_kinds": ["comb-inventory-mismatch"],
            "why": "; ".join(inventory_failures),
        })
        layout_unevaluable += 1

    inventory_complete = not (
        unexpected_emitted_ids
        or duplicate_layout_ids
        or any(
            cell_id in layout_subjects or cell_id in emitted_comb_ids
            for cell_id in duplicate_emitted_ids
        )
        or inventory_failures
        or owner_registry.binding_error is not None
        or raw_inventory_issues
        or uncovered_cell_binding_issues
    )
    counts = {
        "combs_expected": len(expected_ids),
        "combs_checked": len(checked_ids),
        "expected_comb_ids": expected_ids,
        "checked_comb_ids": checked_ids,
        "emitted_comb_ids": emitted_comb_ids,
        "unexpected_emitted_comb_ids": unexpected_emitted_ids,
        "duplicate_layout_comb_ids": duplicate_layout_ids,
        "duplicate_emitted_cell_ids": duplicate_emitted_ids,
        "raw_live_comb_issues": len(raw_inventory_issues),
        "emitted_cell_binding_issues": len(all_cell_binding_issues),
        "inventory_complete": inventory_complete,
        "layout_mismatches": layout_mismatches,
        "layout_unevaluable": layout_unevaluable,
        "owner_certificates_valid": owner_certificates_valid,
        "owner_certificates_invalid": owner_certificates_invalid,
        "source_u_frame_evaluable": source_u_frame_evaluable,
        "source_certified_unframed_evaluable": (
            source_certified_unframed_evaluable),
        "emission_behind_layout": stale_emission,
        "emission_invalid": emission_invalid,
    }
    if offenders:
        return broken(
            f"{len(offenders)} comb subject(s) fail source/layout/emission "
            "agreement or inventory binding",
            offenders,
            offender_limit=None,
            **counts,
        )
    return held(**counts)


# --------------------------------------------------------------------------
# assertion 3 -- every printed money box has an input
# --------------------------------------------------------------------------


def check_money_boxes_have_inputs(b: Bundle) -> dict[str, Any]:
    """C4: 2000-DST's entire page-1 money grid was unfillable.

    A box is fillable-by-construction if the source drew a comb in it -- a comb
    exists only to receive typed characters -- or if it is an enclosed empty
    rectangle. Either way it must carry an input.

    A comb slot that already holds printed ink is excluded, which is what keeps
    this from contradicting assertion 1, and a comb whose *every* slot is inked
    is not a money box at all: those are the container cells where a header ran
    across a run of ticks, and demanding inputs there would put a field over the
    heading. 49 cells in this corpus are excluded that way, and the count is
    reported so the exclusion cannot hide anything.
    """
    if b.form_html is None:
        return broken("no emitted form document to check")
    if b.layout is None:
        return broken("no layout to identify printed boxes from")
    binding_issues = emitted_cell_binding_issues(b)
    if binding_issues:
        return broken(
            f"{len(binding_issues)} emitted cell binding issue(s)",
            binding_issues,
            offender_limit=None,
            boxes_checked=0,
            combs_fully_inked=0,
            emitted_cell_binding_issues=len(binding_issues),
        )
    offenders, checked, fully_inked = [], 0, 0
    by_id = {cell.id: cell for cell in b.cells}
    for cell_id, layout_cell in b.layout_cells.items():
        if cell_id in b.relocated_cells:
            continue
        cell = by_id.get(cell_id)
        if cell is None:
            continue
        index = b.ink.get(cell.page)
        comb = layout_cell.get("comb")
        if comb:
            slots = slot_boxes(cell)
            if not slots:
                checked += 1
                offenders.append({"cell": cell_id, "page": cell.page,
                                  "why": "comb printed, no slots emitted",
                                  "printed_slots": comb["cells"]})
                continue
            free = [(i, box, has) for i, box, has in slots
                    if index is None or index.any_hit(box) is None]
            if not free:
                fully_inked += 1
                continue
            checked += 1
            missing = [i for i, _, has in free if not has]
            if missing:
                offenders.append({"cell": cell_id, "page": cell.page,
                                  "why": "comb slots with no input",
                                  "slots": len(slots), "ink_free": len(free),
                                  "without_input": missing[:8]})
            continue
        border = layout_cell.get("border") or {}
        enclosed = all(border.get(side) for side in ("top", "bottom", "left", "right"))
        if enclosed and layout_cell.get("is_empty") and layout_cell.get("rectangular"):
            checked += 1
            if not input_boxes(cell):
                offenders.append({"cell": cell_id, "page": cell.page,
                                  "why": "enclosed empty box, no input"})
    if offenders:
        return broken(f"{len(offenders)} of {checked} printed boxes are not fillable",
                      offenders, boxes_checked=checked, combs_fully_inked=fully_inked)
    return held(
        boxes_checked=checked,
        combs_fully_inked=fully_inked,
        emitted_cell_binding_issues=0,
    )


# --------------------------------------------------------------------------
# assertion 4 -- nothing form-side below the guide cut
# --------------------------------------------------------------------------


def check_rules_below_guide_cut(b: Bundle) -> dict[str, Any]:
    """C7: an orphaned frame down two-thirds of a page.

    Awarding a straddling rule to the form was chosen so a cut could never lose
    one. The cost is 1600-PT keeping `v85` and `v148`, each 1.44 x 461.33pt,
    with the table they framed now in the guide. Both the IR's form side and the
    emitted SVG are checked: the IR says what we decided to keep, the SVG says
    what a taxpayer sees.
    """
    if not b.regions:
        return held(reason="", cuts=0)
    if b.form_html is None:
        return broken("guide plan cuts pages but no form document to check")
    chunks = page_chunks(b.form_html)
    offenders, fills_below = [], 0
    for region in b.regions:
        page_index, cut = region["page"], float(region["cut_y_pt"])
        claimed = set(region.get("rule_ids") or ())
        for rule in b.pages.get(page_index, {}).get("rules", ()):
            if rule["id"] in claimed:
                continue
            if rule["y1"] > cut + CUT_EPS_PT:
                offenders.append({"page": page_index, "rule": rule["id"],
                                  "y1": rule["y1"], "cut_y": cut, "where": "ir"})
        for match in SVG_RECT_RE.finditer(chunks.get(page_index, "")):
            attrs = attrs_of(match.group(1))
            try:
                bottom = float(attrs["y"]) + float(attrs["height"])
            except (KeyError, ValueError):
                continue
            if bottom <= cut + CUT_EPS_PT:
                continue
            if "data-rule-id" in attrs:
                offenders.append({"page": page_index, "rule": attrs["data-rule-id"],
                                  "y1": round(bottom, 2), "cut_y": cut,
                                  "where": "emitted"})
            else:
                fills_below += 1
    if offenders:
        return broken(f"{len(offenders)} form-side rule(s) cross the guide cut",
                      offenders, cuts=len(b.regions), area_fills_below_cut=fills_below)
    return held(cuts=len(b.regions), area_fills_below_cut=fills_below)


# --------------------------------------------------------------------------
# assertion 5 -- emitted colour equals the IR's
# --------------------------------------------------------------------------


def check_run_colour_matches_ir(b: Bundle) -> dict[str, Any]:
    """C8: 1600-PT and 1600-VT publish 25 white runs each, in black.

    They are BIR reviewer initials, invisible on the official paper. Rendering
    them black turns something the source hid into something that reads as ATC
    data, so this is a disclosure defect and not a styling one.

    The form document is checked run by run against the IR. The guide can only be
    checked by containment -- its reflow merges runs into table cells and drops
    the run ids -- so the test there is that every non-black colour a relocated
    run carries appears somewhere in the guide's markup. That is weaker, and it
    is enough to catch a document that declares no colour at all, which is the
    defect.
    """
    if b.form_html is None:
        return broken("no emitted form document to check")
    offenders = []
    checked = 0
    for page_index, page in sorted(b.pages.items()):
        for index, run in enumerate(page.get("text_runs") or ()):
            key = (page_index, index)
            style = b.emitted_runs.get(key)
            if style is None:
                if key not in b.relocated_runs:
                    offenders.append({"page": page_index, "run": index,
                                      "why": "neither emitted nor relocated",
                                      "text": run["text"][:40]})
                continue
            checked += 1
            got = COLOR_RE.search(style)
            want = int(run.get("color") or 0)
            if got is None:
                offenders.append({"page": page_index, "run": index,
                                  "why": "no colour declared",
                                  "ir_color": f"#{want:06x}"})
            elif int(got.group(1), 16) != want:
                offenders.append({"page": page_index, "run": index,
                                  "why": "colour differs",
                                  "emitted": f"#{int(got.group(1), 16):06x}",
                                  "ir_color": f"#{want:06x}"})
    guide_colours = {int(b.pages[page]["text_runs"][index].get("color") or 0)
                     for page, index in b.relocated_runs
                     if page in b.pages
                     and index < len(b.pages[page]["text_runs"])}
    guide_colours.discard(0)
    if guide_colours:
        markup = (b.guide_html or "").lower()
        for colour in sorted(guide_colours):
            if f"#{colour:06x}" not in markup:
                offenders.append({"why": "relocated run's colour absent from guide",
                                  "ir_color": f"#{colour:06x}",
                                  "runs": sum(1 for p, i in b.relocated_runs
                                              if int(b.pages[p]["text_runs"][i]
                                                     .get("color") or 0) == colour)})
    if offenders:
        return broken(f"{len(offenders)} run colour(s) do not match the IR",
                      offenders, runs_checked=checked)
    return held(runs_checked=checked)


# --------------------------------------------------------------------------
# assertion 6 -- a relocated rate row keeps its description
# --------------------------------------------------------------------------


def check_reflow_rate_without_description(b: Bundle) -> dict[str, Any]:
    """C9: the only *correctness* hazard in the review.

    1600-PT's guide shows a two-line ATC description, then a row holding only
    "3% | WB 050". A reader can attach that rate to the wrong nature of payment,
    which on a withholding return is a wrong remittance. The signature is
    machine-checkable: an empty description cell beside a non-empty rate.

    A rate table reflowed as prose fails too. 2551M's table is flattened into
    running text, which destroys the column relationship outright -- there are no
    rows left to check, and reporting that as "no bad rows found" is exactly the
    blindness this file exists to remove.
    """
    tables = [r for r in b.regions if r.get("marker_pattern") in RATE_TABLE_MARKERS]
    if not tables:
        return held(rate_tables=0)
    if b.guide_html is None:
        return broken(f"{len(tables)} rate table(s) relocated but no guide document")
    sections = {}
    for match in SECTION_RE.finditer(b.guide_html):
        attrs = attrs_of(match.group(1))
        if "data-page" in attrs:
            sections[int(attrs["data-page"])] = (attrs.get("data-flow"), match.group(2))
    offenders, rows_checked = [], 0
    for region in tables:
        page_index = region["page"]
        section = sections.get(page_index)
        if section is None:
            offenders.append({"page": page_index, "why": "no guide section for page"})
            continue
        flow, body = section
        if flow != "table":
            offenders.append({"page": page_index, "why": f"rate table reflowed as {flow}; "
                                                         "row structure not recoverable",
                              "marker": region.get("marker", "")[:50]})
            continue
        for row in ROW_RE.finditer(body):
            cells = [TAG_RE.sub("", c).replace("&amp;", "&").strip()
                     for c in TD_RE.findall(row.group(1))]
            if len(cells) < 2:
                continue
            rows_checked += 1
            if not cells[0] and any(cells[1:]):
                offenders.append({"page": page_index, "why": "rate without description",
                                  "row": [c[:24] for c in cells]})
    if offenders:
        return broken(f"{len(offenders)} relocated rate row(s)/table(s) lost their "
                      f"description", offenders, rate_tables=len(tables),
                      rows_checked=rows_checked)
    return held(rate_tables=len(tables), rows_checked=rows_checked)


# --------------------------------------------------------------------------
# assertion 7 -- a non-upright image is emitted with its transform
# --------------------------------------------------------------------------


def check_image_transform_applied(b: Bundle) -> dict[str, Any]:
    """C3: 2550M's seal prints upside-down, rim lettering bottom-to-top.

    The placement matrix is read from the source PDF rather than from the IR, so
    the assertion holds its own evidence and stays evaluable across an IR schema
    change. Orientation signatures are compared as multisets per page: pairing
    individual placements would need a hash the emitter is free to change, while
    "this page draws one y-flipped image and the SVG flips none" is decidable
    from either document alone.
    """
    if b.doc is None:
        return broken("source PDF not resolved; placement matrices unknown")
    if b.form_html is None:
        return broken("no emitted form document to check")
    chunks = page_chunks(b.form_html)
    offenders = []
    placements = 0
    for page_index in sorted(b.pages):
        want: collections.Counter = collections.Counter()
        for info in b.doc[page_index - 1].get_image_info(xrefs=True):
            placements += 1
            want[transform_signature(info["transform"])] += 1
        got: collections.Counter = collections.Counter()
        for match in SVG_IMAGE_RE.finditer(chunks.get(page_index, "")):
            got[svg_signature(attrs_of(match.group(1)).get("transform"))] += 1
        if want == got:
            continue
        for signature in sorted(set(want) | set(got)):
            if want[signature] == got[signature]:
                continue
            offenders.append({"page": page_index,
                              "orientation": {"x_sign": signature[0],
                                              "y_sign": signature[1],
                                              "sheared": signature[2]},
                              "source_placements": want[signature],
                              "emitted": got[signature]})
    if offenders:
        return broken("emitted image orientation differs from the source's",
                      offenders, placements=placements)
    return held(placements=placements)


# --------------------------------------------------------------------------
# assertion 8 -- no invented codepoints
# --------------------------------------------------------------------------


INVENTED_SUSPECTS = "?§"


def check_no_invented_codepoints(b: Bundle) -> dict[str, Any]:
    """C1: a character that looks like content but is not in the source.

    Both known lies are covered. `?` is what a dropped glyph used to become, and
    a `?` inside a checkbox makes lattice.py classify the cell as a label, so the
    taxpayer cannot tick it. `§` is subtler and worse: on 2550M page 4 and 2553
    page 2 seven glyphs come from a Wingdings face with no ToUnicode CMap, and
    rawdict reports SECTION SIGN -- the WinAnsi meaning of a byte the font does
    not use. Nothing downstream can tell that apart from a real section sign.

    Checked per glyph origin against `get_texttrace()`, which reports U+FFFD
    rather than guessing, so this names the exact character. A `?` the source
    really does state passes: 2200S's checkbox glyph is drawn from codepoint
    U+003F and the assertion says so, which is the honest answer even though the
    glyph is a Wingdings box.
    """
    if b.doc is None:
        return broken("source PDF not resolved; drawn codepoints unknown")
    offenders, examined = [], 0
    for page_index, page in sorted(b.pages.items()):
        runs = page.get("text_runs") or ()
        suspect = [(i, r) for i, r in enumerate(runs)
                   if any(ch in r["text"] for ch in INVENTED_SUSPECTS)]
        if not suspect:
            continue
        drawn = drawn_codepoints(b.doc[page_index - 1])
        for index, run in suspect:
            offsets = run.get("char_origin_offsets_pt") or ()
            baseline = run.get("baseline_y")
            for position, char in enumerate(run["text"]):
                if char not in INVENTED_SUSPECTS:
                    continue
                examined += 1
                if baseline is None or position >= len(offsets):
                    offenders.append({"page": page_index, "run": index,
                                      "char_index": position, "char": char,
                                      "why": "run has no per-glyph origin to check"})
                    continue
                key = (round(run.get("origin_x", run["x0"]) + offsets[position], 2),
                       round(baseline, 2))
                codepoints = drawn.get(key)
                if codepoints is None:
                    offenders.append({"page": page_index, "run": index,
                                      "char_index": position, "char": char,
                                      "why": "no glyph drawn at this origin"})
                elif ord(char) not in codepoints:
                    offenders.append({"page": page_index, "run": index,
                                      "char_index": position, "char": char,
                                      "why": "source draws a different codepoint",
                                      "source_codepoints": [f"U+{c:04X}"
                                                            for c in sorted(codepoints)],
                                      "font": run.get("font"),
                                      "text": run["text"][:40]})
    if offenders:
        return broken(f"{len(offenders)} character(s) the source does not state",
                      offenders, characters_examined=examined)
    return held(characters_examined=examined)


CHECKS = {
    "inputs_over_printed_text": check_inputs_over_printed_text,
    "comb_slots_match_printed": check_comb_slots_match_printed,
    "money_boxes_have_inputs": check_money_boxes_have_inputs,
    "rules_below_guide_cut": check_rules_below_guide_cut,
    "run_colour_matches_ir": check_run_colour_matches_ir,
    "reflow_rate_without_description": check_reflow_rate_without_description,
    "image_transform_applied": check_image_transform_applied,
    "no_invented_codepoints": check_no_invented_codepoints,
}
assert tuple(CHECKS) == ASSERTION_KEYS, "GOAL.md names these eight, in this order"


def evaluate_assertions(bundle: Bundle) -> dict[str, Any]:
    """Run all eight and flatten them into the per-form record.

    A raising check is a failing check. It cannot be a passing one: an assertion
    that throws has not looked at the form, and "we did not look" is the exact
    reading of the audit that let 137 defects through.
    """
    details: dict[str, Any] = {}
    flat: dict[str, Any] = {}
    for key, check in CHECKS.items():
        try:
            detail = check(bundle)
        except Exception as exc:  # noqa: BLE001 - see docstring
            detail = broken(f"{type(exc).__name__}: {exc}",
                            trace=traceback.format_exc(limit=2))
        details[key] = detail
        flat[key] = bool(detail["holds"])
    flat["assertions"] = details
    flat["assertions_held"] = sum(1 for key in ASSERTION_KEYS if flat[key])
    return flat


def load_bundle(slug: str, ir_dir: pathlib.Path, html_dir: pathlib.Path,
                layout_dir: pathlib.Path, guide_dir: pathlib.Path | None,
                source_root: str,
                input_snapshot: InputSnapshot | None = None) -> Bundle:
    snapshot = input_snapshot or snapshot_inputs(
        slug, ir_dir, html_dir, layout_dir, guide_dir, source_root)
    if snapshot.missing_required:
        raise FileNotFoundError(
            "required audit input(s) missing: "
            + ", ".join(snapshot.missing_required))

    def text(role: str) -> str:
        payload = snapshot.contents[role]
        if payload is None:
            raise FileNotFoundError(f"required audit input missing: {role}")
        return payload.decode("utf-8")

    ir = json.loads(text("ir"))
    layout_payload = snapshot.contents["layout"]
    if layout_payload is None:
        raise FileNotFoundError("required audit input missing: layout")
    layout_sha256 = (
        snapshot.manifest.get("inputs", {}).get("layout", {}).get("sha256")
    )
    guide_html = snapshot.contents["guide_html"]
    return Bundle(
        slug=slug,
        ir=ir,
        layout=json.loads(layout_payload.decode("utf-8")),
        plan=json.loads(text("guide")),
        form_html=text("html"),
        guide_html=guide_html.decode("utf-8") if guide_html is not None else None,
        pdf=snapshot.contents["source_pdf"],
        form_html_bytes=snapshot.contents["html"],
        render_assets=dict(snapshot.render_assets),
        render_entrypoint=snapshot.render_entrypoint,
        layout_payload=layout_payload,
        layout_sha256=(
            str(layout_sha256) if layout_sha256 is not None else None),
    )


def form_side(reference: dict, plan: dict | None) -> tuple[dict, dict]:
    """Drop everything the guide plan moved out, from the reference IR.

    The form document no longer contains the guide's rules and strings, so
    scoring it against the whole source IR counts correctly-relocated content as
    missing. That is how a corpus at 100% rules came to read 42/51: nothing had
    moved on the sheet, the denominator was simply the wrong one.

    Indices are removed high-to-low so earlier removals cannot shift later ones.
    """
    if not plan or not plan.get("inline"):
        return reference, {"rules": 0, "text_runs": 0, "images": 0}

    filtered = copy.deepcopy(reference)
    removed = {"rules": 0, "text_runs": 0, "images": 0}
    by_page = {region["page"]: region for region in plan["inline"]}

    for page in filtered["pages"]:
        region = by_page.get(page["index"])
        if region is None:
            continue

        claimed_rules = set(region.get("rule_ids") or ())
        if claimed_rules:
            before = len(page["rules"])
            page["rules"] = [r for r in page["rules"] if r["id"] not in claimed_rules]
            removed["rules"] += before - len(page["rules"])

        for index in sorted(region.get("text_run_indices") or (), reverse=True):
            if 0 <= index < len(page["text_runs"]):
                del page["text_runs"][index]
                removed["text_runs"] += 1

        for index in sorted(region.get("image_indices") or (), reverse=True):
            if 0 <= index < len(page["images"]):
                del page["images"][index]
                removed["images"] += 1

        # Fills and paths are relocated too, and leaving them behind is what made
        # four pages look non-empty to the reference while emit.py correctly
        # dropped them: 0605 page 2 held 44 orphan fills and 532 orphan paths
        # after every rule, run and image on it had moved to the guide.
        for key, bucket in (("area_fill_indices", "area_fills"),
                            ("path_indices", "paths")):
            claimed = region.get(key) or ()
            items = page.get(bucket) or []
            for index in sorted(claimed, reverse=True):
                if 0 <= index < len(items):
                    del items[index]
                    removed[bucket] = removed.get(bucket, 0) + 1

        # stats are what the rule denominator is read from, so they must follow.
        page["stats"]["rules_structural"] = sum(
            1 for r in page["rules"] if r["role"] == "structural")

    # A page whose every element was relocated is not printed by the form at
    # all -- emit.py drops it rather than emitting a blank sheet. The reference
    # has to drop it too, or the page counts disagree and verify calls that a
    # paper mismatch: five forms failed exactly this way (0605, 1702Q, 2200P,
    # 2550M, 2553), all with identical page dimensions and rotations.
    kept = [p for p in filtered["pages"]
            if p["rules"] or p["text_runs"] or p["images"]
            or p.get("area_fills") or p.get("paths")]
    removed["pages"] = len(filtered["pages"]) - len(kept)
    if removed["pages"]:
        filtered["pages"] = kept
        for index, page in enumerate(kept, 1):
            page["index"] = index
        filtered["source"] = dict(filtered.get("source") or {})
        filtered["source"]["page_count"] = len(kept)

    return filtered, removed


@dataclasses.dataclass(frozen=True)
class MaterializedRenderTree:
    root: pathlib.Path
    entrypoint: pathlib.Path
    expected: dict[str, bytes]


def _validate_materialized_render_tree(
        tree: MaterializedRenderTree,
        phase: str,
        ) -> None:
    actual = {
        path.relative_to(tree.root).as_posix()
        for path in tree.root.rglob("*")
        if path.is_file() or path.is_symlink()
    }
    expected = set(tree.expected)
    if actual != expected:
        raise RuntimeError(
            f"isolated render tree changed {phase}: "
            f"missing={sorted(expected - actual)}, "
            f"unexpected={sorted(actual - expected)}")
    for logical, payload in sorted(tree.expected.items()):
        path = tree.root / pathlib.PurePosixPath(logical)
        if path.is_symlink() or not path.is_file():
            raise RuntimeError(
                f"isolated render dependency changed {phase}: {logical}")
        if _stable_read(path) != payload:
            raise RuntimeError(
                f"isolated render dependency bytes changed {phase}: "
                f"{logical}")


@contextlib.contextmanager
def materialized_form_snapshot(
        bundle: Bundle, _legacy_html_dir: pathlib.Path | None = None,
        ) -> Iterable[MaterializedRenderTree]:
    """Build a private tree containing only snapshotted browser inputs."""
    if bundle.form_html is None:
        raise FileNotFoundError("required audit input missing: html")
    html_payload = (
        bundle.form_html_bytes
        if bundle.form_html_bytes is not None
        else bundle.form_html.encode("utf-8")
    )
    entrypoint = bundle.render_entrypoint or f"{bundle.slug}.html"
    expected = {entrypoint: html_payload, **bundle.render_assets}
    with tempfile.TemporaryDirectory(
            prefix=f"formgen-{bundle.slug}-render-") as temporary:
        root = pathlib.Path(temporary)
        root.chmod(0o700)
        for logical, payload in sorted(expected.items()):
            path = root / pathlib.PurePosixPath(logical)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.parent.chmod(0o700)
            path.write_bytes(payload)
            path.chmod(0o400)
        tree = MaterializedRenderTree(
            root=root,
            entrypoint=root / pathlib.PurePosixPath(entrypoint),
            expected=expected,
        )
        _validate_materialized_render_tree(tree, "before Chromium")
        try:
            yield tree
        finally:
            _validate_materialized_render_tree(tree, "after Chromium")


SYNTHETIC_RENDER_ORIGIN = "https://formgen.invalid"
RENDER_REQUEST_POLICY = "formgen-snapshot-only-v1"
RENDER_WORKER_SCHEMA = "formgen-isolated-render-worker-v1"
RENDER_HARD_TIMEOUT_SECONDS = 60.0
RENDER_WORKER_KILL_GRACE_SECONDS = 2.0
CHROMIUM_PDF_DATE_RE = re.compile(
    rb"/(?P<key>CreationDate|ModDate)\s*"
    rb"\(D:\d{14}[+-]\d{2}'\d{2}'\)")


def _render_request_path(url: str) -> str:
    parsed = urllib.parse.urlsplit(url)
    if (parsed.scheme != "https"
            or parsed.netloc != "formgen.invalid"
            or parsed.query):
        raise ValueError("request is outside the synthetic snapshot origin")
    decoded = urllib.parse.unquote(parsed.path).lstrip("/")
    if not decoded:
        raise ValueError("request has no snapshot path")
    logical = posixpath.normpath(decoded)
    if (logical.startswith("../") or logical in {"", ".", ".."}
            or "\\" in logical or "\x00" in logical):
        raise ValueError("request path escapes the snapshot")
    return logical


def _retained_request_payload(
        url: str,
        method: str,
        expected: dict[str, bytes],
        ) -> tuple[str, bytes]:
    if method != "GET":
        raise ValueError(
            "only GET is allowed by retained snapshot policy")
    logical = _render_request_path(url)
    payload = expected.get(logical)
    if payload is None:
        raise ValueError("request is absent from retained closure")
    return logical, payload


class RenderDeadlineExceeded(RuntimeError):
    def __init__(self, deadline_seconds: float) -> None:
        self.deadline_seconds = deadline_seconds
        super().__init__(
            "Chromium render exceeded its deterministic hard deadline "
            f"of {deadline_seconds:g} seconds")


def _render_deadline_evidence(
        error: RenderDeadlineExceeded,
        ) -> dict[str, Any]:
    return {
        "measured": False,
        "hard_failure": "render-hard-deadline-exceeded",
        "roundtrip_liveness": {
            "status": "unevaluable",
            "hard_failure": "render-hard-deadline-exceeded",
            "hard_deadline_seconds": error.deadline_seconds,
            "cleanup_policy": "kill-worker-and-chromium-process-group",
        },
    }


def _render_snapshotted_tree_in_process(
        expected: dict[str, bytes],
        entrypoint: str,
        width_pt: float,
        height_pt: float,
        operation_timeout_ms: int,
        ) -> tuple[bytes, dict[str, Any], dict[str, Any]]:
    """Worker-side render through one exact, deny-by-default session."""
    requested: list[str] = []
    blocked: list[dict[str, str]] = []
    launch_args = [
        "--disable-background-networking",
        "--disable-component-update",
        "--disable-default-apps",
        "--disable-sync",
        "--metrics-recording-only",
        "--no-first-run",
    ]
    zero = {"top": "0", "bottom": "0", "left": "0", "right": "0"}
    with _bound_playwright_runtime() as runtime:
        browser = runtime.playwright.chromium.launch(
            executable_path=str(runtime.chromium_path),
            args=launch_args,
        )
        live_version = browser.version
        context = browser.new_context(
            service_workers="block",
            offline=True,
        )
        context.set_default_timeout(operation_timeout_ms)
        context.set_default_navigation_timeout(operation_timeout_ms)

        def route_request(route: Any) -> None:
            url = route.request.url
            try:
                logical, payload = _retained_request_payload(
                    url, route.request.method, expected)
            except ValueError as exc:
                blocked.append({"url": url, "reason": str(exc)})
                route.abort()
                return
            requested.append(logical)
            content_type = mimetypes.guess_type(logical)[0]
            if logical == entrypoint:
                content_type = "text/html; charset=utf-8"
            route.fulfill(
                status=200,
                body=payload,
                content_type=content_type or "application/octet-stream",
                headers={
                    "Cache-Control": "no-store",
                    "X-Content-Type-Options": "nosniff",
                },
            )

        context.route("**/*", route_request)
        blocked_websockets: list[str] = []
        if not hasattr(context, "route_web_socket"):
            raise RuntimeError(
                "bound Playwright runtime cannot enforce WebSocket policy")

        def route_websocket(websocket: Any) -> None:
            blocked_websockets.append(websocket.url)
            # Do not call a sync Playwright method from this callback.
            # `close()` re-enters the sync event loop and deadlocks. A
            # WebSocketRoute left unconnected performs no network I/O;
            # the post-load policy check rejects the render.

        context.route_web_socket("**/*", route_websocket)

        def reject_blocked_requests() -> None:
            if blocked or blocked_websockets:
                raise RuntimeError(
                    "Chromium requested resources outside the retained "
                    "snapshot: "
                    f"{json.dumps(blocked, sort_keys=True)}; "
                    "websockets="
                    f"{json.dumps(sorted(blocked_websockets))}")

        page = context.new_page()
        try:
            page.goto(
                f"{SYNTHETIC_RENDER_ORIGIN}/{entrypoint}",
                wait_until="load",
            )
            page.evaluate(
                "() => document.fonts.ready.then(() => true)")
            page.wait_for_load_state("networkidle")
            reject_blocked_requests()
            pdf_payload = page.pdf(
                width=f"{width_pt / 72.0:.6f}in",
                height=f"{height_pt / 72.0:.6f}in",
                margin=zero,
                print_background=True,
                prefer_css_page_size=False,
                scale=1.0,
            )
        finally:
            context.close()
            browser.close()
        reject_blocked_requests()
        provenance = copy.deepcopy(runtime.provenance)
        provenance.update({
            "live_browser_version": live_version,
            "explicit_executable_path_used": True,
            "launch_args": launch_args,
            "service_workers": "block",
            "browser_context_offline": True,
            "websocket_policy": "record-and-leave-unconnected",
            "request_policy": RENDER_REQUEST_POLICY,
            "playwright_operation_timeout_ms": operation_timeout_ms,
        })
    request_record = {
        "policy": RENDER_REQUEST_POLICY,
        "synthetic_origin": SYNTHETIC_RENDER_ORIGIN,
        "fulfilled": sorted(set(requested)),
        "fulfilled_requests": len(requested),
        "blocked": blocked,
        "blocked_requests": len(blocked),
        "blocked_websockets": sorted(blocked_websockets),
        "all_requests_from_retained_closure": (
            not blocked and not blocked_websockets),
    }
    return bytes(pdf_payload), provenance, request_record


def _render_worker_job(
        tree: MaterializedRenderTree,
        width_pt: float,
        height_pt: float,
        deadline_seconds: float,
        ) -> bytes:
    entrypoint = tree.entrypoint.relative_to(tree.root).as_posix()
    resources = []
    for logical, payload in sorted(tree.expected.items()):
        resources.append({
            "path": logical,
            "bytes": len(payload),
            "sha256": hashlib.sha256(payload).hexdigest(),
            "payload_base64": base64.b64encode(payload).decode("ascii"),
        })
    job = {
        "schema": RENDER_WORKER_SCHEMA,
        "producer_sha256": hashlib.sha256(
            _AUDIT_SOURCE_PAYLOAD).hexdigest(),
        "entrypoint": entrypoint,
        "width_pt": float(width_pt),
        "height_pt": float(height_pt),
        "hard_deadline_seconds": float(deadline_seconds),
        "resources": resources,
    }
    return json.dumps(
        job, sort_keys=True, separators=(",", ":"),
        ensure_ascii=True).encode("ascii")


def _decode_render_worker_job(
        payload: bytes,
        ) -> tuple[dict[str, bytes], str, float, float, float]:
    try:
        job = json.loads(payload.decode("ascii"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError("isolated render worker job is not canonical JSON") from exc
    if not isinstance(job, dict) or job.get("schema") != RENDER_WORKER_SCHEMA:
        raise ValueError("isolated render worker job has an invalid schema")
    producer_sha = hashlib.sha256(_AUDIT_SOURCE_PAYLOAD).hexdigest()
    if job.get("producer_sha256") != producer_sha:
        raise ValueError(
            "isolated render worker job names a different audit producer")
    entrypoint = job.get("entrypoint")
    resources = job.get("resources")
    if not isinstance(entrypoint, str) or not isinstance(resources, list):
        raise ValueError("isolated render worker job is incomplete")
    expected: dict[str, bytes] = {}
    observed_paths: list[str] = []
    for index, item in enumerate(resources):
        if not isinstance(item, dict):
            raise ValueError(
                f"isolated render resource {index} is not an object")
        logical = item.get("path")
        if not isinstance(logical, str):
            raise ValueError(
                f"isolated render resource {index} has no path")
        normalized = posixpath.normpath(logical)
        if (normalized != logical
                or logical in {"", ".", ".."}
                or logical.startswith("../")
                or posixpath.isabs(logical)
                or "\\" in logical
                or "\x00" in logical):
            raise ValueError(
                f"isolated render resource path is invalid: {logical!r}")
        try:
            decoded = base64.b64decode(
                item.get("payload_base64", ""), validate=True)
        except (binascii.Error, TypeError, ValueError) as exc:
            raise ValueError(
                f"isolated render resource is not base64: {logical}") from exc
        if (item.get("bytes") != len(decoded)
                or item.get("sha256")
                != hashlib.sha256(decoded).hexdigest()):
            raise ValueError(
                f"isolated render resource identity mismatch: {logical}")
        if logical in expected:
            raise ValueError(
                f"duplicate isolated render resource: {logical}")
        expected[logical] = decoded
        observed_paths.append(logical)
    if observed_paths != sorted(observed_paths):
        raise ValueError(
            "isolated render resources are not canonically ordered")
    if entrypoint not in expected:
        raise ValueError(
            "isolated render entrypoint is absent from retained resources")
    try:
        width_pt = float(job["width_pt"])
        height_pt = float(job["height_pt"])
    except (KeyError, TypeError, ValueError) as exc:
        raise ValueError(
            "isolated render worker has invalid paper dimensions") from exc
    if (not math.isfinite(width_pt) or width_pt <= 0
            or not math.isfinite(height_pt) or height_pt <= 0):
        raise ValueError(
            "isolated render worker paper dimensions must be positive")
    try:
        deadline_seconds = float(job["hard_deadline_seconds"])
    except (KeyError, TypeError, ValueError) as exc:
        raise ValueError(
            "isolated render worker has an invalid hard deadline") from exc
    if (not math.isfinite(deadline_seconds)
            or deadline_seconds <= 0):
        raise ValueError(
            "isolated render worker hard deadline must be positive")
    return expected, entrypoint, width_pt, height_pt, deadline_seconds


def _run_render_worker() -> int:
    """Execute only inside the process-isolated Chromium worker."""
    response: dict[str, Any]
    return_code = 0
    try:
        validate_trusted_producer_sources()
        validate_base_runtime()
        expected, entrypoint, width_pt, height_pt, deadline_seconds = (
            _decode_render_worker_job(sys.stdin.buffer.read()))
        operation_timeout_ms = max(
            1000, math.ceil(deadline_seconds * 2000.0))
        pdf_payload, provenance, requests = (
            _render_snapshotted_tree_in_process(
                expected, entrypoint, width_pt, height_pt,
                operation_timeout_ms))
        validate_trusted_producer_sources()
        validate_base_runtime()
        response = {
            "schema": RENDER_WORKER_SCHEMA,
            "ok": True,
            "producer_sha256": hashlib.sha256(
                _AUDIT_SOURCE_PAYLOAD).hexdigest(),
            "pdf": {
                "bytes": len(pdf_payload),
                "sha256": hashlib.sha256(pdf_payload).hexdigest(),
                "payload_base64": base64.b64encode(
                    pdf_payload).decode("ascii"),
            },
            "provenance": provenance,
            "requests": requests,
        }
    except BaseException as exc:  # noqa: BLE001 - cross-process error packet
        return_code = 1
        response = {
            "schema": RENDER_WORKER_SCHEMA,
            "ok": False,
            "producer_sha256": hashlib.sha256(
                _AUDIT_SOURCE_PAYLOAD).hexdigest(),
            "error_type": type(exc).__name__,
            "error": str(exc),
        }
    sys.stdout.write(json.dumps(
        response, sort_keys=True, separators=(",", ":"),
        ensure_ascii=True))
    sys.stdout.flush()
    return return_code


def _kill_render_worker(process: subprocess.Popen[bytes]) -> None:
    try:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGKILL)
        elif process.poll() is None:
            process.kill()
    except ProcessLookupError:
        pass
    try:
        process.communicate(timeout=RENDER_WORKER_KILL_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        try:
            process.kill()
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=RENDER_WORKER_KILL_GRACE_SECONDS)
        except subprocess.TimeoutExpired:
            # Never trade the authoritative render deadline for an unbounded
            # reap. The process group has already received SIGKILL.
            pass


def _render_snapshotted_tree(
        tree: MaterializedRenderTree,
        width_pt: float,
        height_pt: float,
        *,
        deadline_seconds: float = RENDER_HARD_TIMEOUT_SECONDS,
        ) -> tuple[bytes, dict[str, Any], dict[str, Any]]:
    """Render in a killable worker under one wall-clock hard deadline."""
    if (not math.isfinite(deadline_seconds)
            or deadline_seconds <= 0):
        raise ValueError("render hard deadline must be positive and finite")
    _validate_materialized_render_tree(
        tree, "before isolated Chromium worker")
    worker_job = _render_worker_job(
        tree, width_pt, height_pt, deadline_seconds)
    command = [
        sys.executable,
        "-E",
        "-B",
        str(_AUDIT_SOURCE_PATH),
        "--render-worker",
    ]
    environment = os.environ.copy()
    environment.pop("PYTHONPATH", None)
    environment.pop("PYTHONHOME", None)
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    process = subprocess.Popen(
        command,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment,
        start_new_session=(os.name == "posix"),
    )
    try:
        try:
            stdout, _stderr = process.communicate(
                input=worker_job,
                timeout=deadline_seconds,
            )
        except subprocess.TimeoutExpired as exc:
            _kill_render_worker(process)
            raise RenderDeadlineExceeded(deadline_seconds) from exc
    finally:
        _validate_materialized_render_tree(
            tree, "after isolated Chromium worker")
    try:
        response = json.loads(stdout.decode("ascii"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RuntimeError(
            "isolated Chromium worker returned no valid result") from exc
    if (not isinstance(response, dict)
            or response.get("schema") != RENDER_WORKER_SCHEMA
            or response.get("producer_sha256")
            != hashlib.sha256(_AUDIT_SOURCE_PAYLOAD).hexdigest()):
        raise RuntimeError(
            "isolated Chromium worker result has invalid provenance")
    if not response.get("ok"):
        error_type = str(response.get("error_type") or "RuntimeError")
        error = str(response.get("error") or "unknown render failure")
        raise RuntimeError(
            f"isolated Chromium worker failed: {error_type}: {error}")
    if process.returncode != 0:
        raise RuntimeError(
            "isolated Chromium worker exited unsuccessfully")
    pdf = response.get("pdf")
    provenance = response.get("provenance")
    requests = response.get("requests")
    if (not isinstance(pdf, dict)
            or not isinstance(provenance, dict)
            or not isinstance(requests, dict)):
        raise RuntimeError(
            "isolated Chromium worker result is incomplete")
    try:
        pdf_payload = base64.b64decode(
            pdf.get("payload_base64", ""), validate=True)
    except (binascii.Error, TypeError, ValueError) as exc:
        raise RuntimeError(
            "isolated Chromium worker PDF is not base64") from exc
    if (pdf.get("bytes") != len(pdf_payload)
            or pdf.get("sha256")
            != hashlib.sha256(pdf_payload).hexdigest()):
        raise RuntimeError(
            "isolated Chromium worker PDF identity mismatch")
    provenance = copy.deepcopy(provenance)
    provenance.update({
        "hard_deadline_seconds": deadline_seconds,
        "hard_deadline_enforced_by": (
            "isolated-render-worker-process-v1"),
        "deadline_cleanup_policy": (
            "kill-worker-and-chromium-process-group"),
    })
    return pdf_payload, provenance, copy.deepcopy(requests)


def _canonicalize_chromium_pdf(payload: bytes) -> tuple[bytes, dict[str, Any]]:
    """Replace only fixed-width volatile PDF metadata before retention."""
    replacement_date = b"D:19700101000000+00'00'"

    def replace(match: re.Match[bytes]) -> bytes:
        replacement = (
            b"/" + match.group("key") + b" (" + replacement_date + b")")
        if len(replacement) != len(match.group(0)):
            raise RuntimeError(
                "Chromium PDF date normalization would move xref offsets")
        return replacement

    canonical, count = CHROMIUM_PDF_DATE_RE.subn(replace, payload)
    if count != 2:
        raise RuntimeError(
            "Chromium PDF did not expose exactly CreationDate and ModDate "
            f"for deterministic normalization (found {count})")
    return canonical, {
        "algorithm": "fixed-width-creation-modification-date-v1",
        "fields_normalized": count,
        "replacement": replacement_date.decode("ascii"),
        "xref_offsets_preserved": len(canonical) == len(payload),
    }


def _canonical_candidate_ir_digest(candidate: dict[str, Any]) -> str:
    canonical = copy.deepcopy(candidate)
    canonical.pop("source", None)
    canonical.pop("generator", None)
    payload = json.dumps(
        canonical,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
    ).encode("ascii")
    return hashlib.sha256(payload).hexdigest()


def _extract_retained_candidate(
        payload: bytes,
        form_code: str,
        revision: str,
        *,
        extractor: Any | None = None,
        ) -> tuple[dict[str, Any], dict[str, Any]]:
    """Extract only a private rematerialization of retained candidate bytes."""
    extractor = extractor or extract.extract
    digest = hashlib.sha256(payload).hexdigest()
    with tempfile.TemporaryDirectory(
            prefix="formgen-candidate-extract-") as temporary:
        root = pathlib.Path(temporary)
        root.chmod(0o700)
        named_path = root / "candidate.pdf"
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        write_fd = os.open(named_path, flags, 0o400)
        try:
            view = memoryview(payload)
            written = 0
            while written < len(view):
                written += os.write(write_fd, view[written:])
            os.fsync(write_fd)
        finally:
            os.close(write_fd)
        read_flags = os.O_RDONLY
        if hasattr(os, "O_NOFOLLOW"):
            read_flags |= os.O_NOFOLLOW
        read_fd = os.open(named_path, read_flags)
        named_path.unlink()
        fd_path = pathlib.Path(f"/dev/fd/{read_fd}")
        if not fd_path.exists():
            os.close(read_fd)
            raise RuntimeError(
                "platform has no retained-descriptor path for extraction")

        def retained_fd_bytes() -> bytes:
            if not hasattr(os, "pread"):
                raise RuntimeError(
                    "platform cannot validate retained descriptor without "
                    "changing extractor-visible file position")
            chunks = []
            offset = 0
            while True:
                chunk = os.pread(read_fd, 1 << 20, offset)
                if not chunk:
                    break
                chunks.append(chunk)
                offset += len(chunk)
            return b"".join(chunks)

        extraction_error: BaseException | None = None
        candidate: dict[str, Any] | None = None
        try:
            if retained_fd_bytes() != payload:
                raise RuntimeError(
                    "retained candidate changed before extraction")
            try:
                candidate = extractor(
                    fd_path, form_code, revision, digest)
            except BaseException as exc:  # preserve SystemExit hash failures
                extraction_error = exc
            if retained_fd_bytes() != payload:
                raise RuntimeError(
                    "retained candidate changed during extraction")
        finally:
            os.close(read_fd)
        if extraction_error is not None:
            raise extraction_error
        if not isinstance(candidate, dict):
            raise RuntimeError("candidate extractor returned no IR object")
        candidate_source = candidate.get("source") or {}
        if (candidate_source.get("sha256") != digest
                or candidate_source.get("bytes") != len(payload)):
            raise RuntimeError(
                "candidate extractor did not publish retained PDF identity")
    return candidate, {
        "materialization": (
            "private-0700-o_excl-o_nofollow-fsynced-unlinked-read-fd"),
        "expected_sha256_passed_to_extractor": True,
        "validated_before_after_extraction": True,
        "candidate_ir_sha256": _canonical_candidate_ir_digest(candidate),
        "candidate_ir_digest_scope": "source-and-generator-removed",
    }


_ROUND_TRIP_TOTAL_KEYS = frozenset({
    "rules_missing", "rules_extra", "rules_thickness_violations",
    "text_missing", "text_extra", "text_mismatched",
    "images_missing", "images_extra", "images_placement_violations",
})
_ROUND_TRIP_PAPER_KEYS = frozenset({
    "reference", "candidate", "problems", "ok",
})


def _validated_verifier_report(
        report: Any,
        ) -> tuple[bool, str | None, dict[str, int]]:
    """Validate the exact report shape emitted by verify.diff_ir()."""
    if not isinstance(report, dict):
        raise RuntimeError("verifier returned no report object")
    paper = report.get("paper")
    if (not isinstance(paper, dict)
            or set(paper) != _ROUND_TRIP_PAPER_KEYS
            or type(paper.get("ok")) is not bool
            or not isinstance(paper.get("problems"), list)
            or any(not isinstance(item, str)
                   for item in paper.get("problems", []))):
        raise RuntimeError("verifier paper evidence is missing or malformed")
    for side in ("reference", "candidate"):
        value = paper.get(side)
        if (not isinstance(value, dict)
                or set(value) != {"width_pt", "height_pt", "page_count"}):
            raise RuntimeError(
                f"verifier paper {side} evidence is missing or malformed")
    paper_ok = paper["ok"]
    hard_failure = report.get("hard_failure")
    if (hard_failure is not None
            and (not isinstance(hard_failure, str) or not hard_failure)):
        raise RuntimeError("verifier hard-failure evidence is malformed")
    if (paper_ok and hard_failure is not None) or (
            not paper_ok and hard_failure != "paper mismatch"):
        raise RuntimeError("verifier paper/hard-failure relation is false")
    totals = report.get("totals")
    if (not isinstance(totals, dict)
            or set(totals) != _ROUND_TRIP_TOTAL_KEYS):
        raise RuntimeError("verifier totals are missing or unsupported")
    if any(type(value) is not int or value < 0
           for value in totals.values()):
        raise RuntimeError("verifier totals contain a nonnegative-int violation")
    return paper_ok, hard_failure, dict(totals)


def round_trip(bundle: Bundle, html_dir: pathlib.Path,
               work: pathlib.Path) -> dict[str, Any]:
    """Print with Chromium, re-extract, diff against the source IR."""
    reference, relocated = form_side(bundle.ir, bundle.plan)
    paper = reference["paper"]
    with materialized_form_snapshot(bundle) as tree:
        raw_pdf, runtime, requests = _render_snapshotted_tree(
            tree, paper["width_pt"], paper["height_pt"])
    retained_pdf, normalization = _canonicalize_chromium_pdf(raw_pdf)
    candidate, extraction_record = _extract_retained_candidate(
        retained_pdf,
        reference["form"]["code"],
        reference["form"]["revision"],
    )
    record: dict[str, Any] = {
        "guide_relocated": relocated,
        "roundtrip_runtime": runtime,
        "render_requests": requests,
        "candidate_pdf": {
            "bytes": len(retained_pdf),
            "sha256": hashlib.sha256(retained_pdf).hexdigest(),
            "retained_exact_bytes": True,
            "chromium_returned_in_memory": True,
            "normalization": normalization,
            **extraction_record,
        },
    }
    report = verify.diff_ir(reference, candidate, verify.Tolerances(),
                            roles=["structural"])
    paper_ok, hard_failure, totals = _validated_verifier_report(report)

    # Denominators come from the source IR, so a percentage always answers
    # "of what the official form contains, how much did we reproduce".
    rules_ref = sum(p["stats"]["rules_structural"] for p in reference["pages"])
    text_ref = sum(len(p["text_runs"]) for p in reference["pages"])
    rules_missing = totals["rules_missing"]
    text_missing = totals["text_missing"]

    # verify.py short-circuits on a paper mismatch and never walks the pages, so
    # every total comes back 0. Zero missing rules is indistinguishable from a
    # perfect form unless the record says which it is -- and reading the first
    # as the second is precisely the failure this project keeps paying for. The
    # gate treats `measured: false` as unevaluable, which counts as a failure.
    measured = hard_failure is None

    record.update({
        "measured": measured,
        "hard_failure": hard_failure,
        "paper_ok": paper_ok,
        "rules_ref": rules_ref,
        "rules_missing": totals["rules_missing"],
        "rules_extra": totals["rules_extra"],
        "rules_thickness_violations": totals["rules_thickness_violations"],
        "rules_pct": round(100.0 * (rules_ref - rules_missing) / rules_ref, 2) if rules_ref else None,
        "text_ref": text_ref,
        "text_missing": text_missing,
        "text_extra": totals["text_extra"],
        "text_pct": round(100.0 * (text_ref - text_missing) / text_ref, 2) if text_ref else None,
        "images_missing": totals["images_missing"],
        "images_placement_violations": totals["images_placement_violations"],
    })
    return record


def score(slug: str, ir_dir: pathlib.Path, html_dir: pathlib.Path,
          layout_dir: pathlib.Path, guide_dir: pathlib.Path | None,
          work: pathlib.Path, source_root: str,
          roundtrip: bool = True) -> dict:
    """One form's record: the eight assertions, then the round-trip score.

    The assertions run first and are kept whatever the round trip does. A
    Chromium failure must not also erase the checks that do not need Chromium --
    losing them is how the record would come to say nothing while looking
    complete.
    """
    record: dict = {
        "slug": slug,
        "status": "error",
        "error": None,
        "input_manifest": empty_input_manifest(),
        "provenance_validation": {
            "validated_before": False,
            "validated_after": False,
            "error": None,
        },
    }
    bundle = None
    try:
        validate_trusted_producer_sources()
        validate_base_runtime()
        record["provenance_validation"]["validated_before"] = True
        snapshot = snapshot_inputs(
            slug, ir_dir, html_dir, layout_dir, guide_dir, source_root)
        record["input_manifest"] = snapshot.manifest
        bundle = load_bundle(slug, ir_dir, html_dir, layout_dir, guide_dir,
                             source_root, input_snapshot=snapshot)
        record.update(evaluate_assertions(bundle))
    except Exception as exc:  # noqa: BLE001 - one bad form must not stop the sweep
        reason = f"{type(exc).__name__}: {exc}"
        record.update({key: False for key in ASSERTION_KEYS})
        record["assertions"] = {key: broken(reason) for key in ASSERTION_KEYS}
        record["assertions_held"] = 0
        record["error"] = reason

    try:
        if bundle is None:
            raise RuntimeError(record["error"] or "bundle not loaded")
        if not roundtrip:
            record["status"] = "ok"
            record["roundtrip"] = "skipped"
        else:
            record.update(round_trip(bundle, html_dir, work))
            record["status"] = "ok"
    except Exception as exc:  # noqa: BLE001
        record["error"] = f"{type(exc).__name__}: {exc}"
        record["trace"] = traceback.format_exc(limit=3)
        if isinstance(exc, RenderDeadlineExceeded):
            record.update(_render_deadline_evidence(exc))
    finally:
        if bundle is not None:
            bundle.close()
        try:
            validate_trusted_producer_sources()
            validate_base_runtime()
            record["provenance_validation"]["validated_after"] = True
        except Exception as exc:  # noqa: BLE001 - invalidates whole record
            reason = f"{type(exc).__name__}: {exc}"
            record["status"] = "error"
            record["error"] = reason
            record["provenance_validation"]["error"] = reason
    input_manifest = record.get("input_manifest") or {}
    reasons = []
    producer = input_manifest.get("producer") or {}
    base_runtime = input_manifest.get("runtime") or {}
    roundtrip_runtime = record.get("roundtrip_runtime") or {}
    if not producer.get("standalone_attestation_complete"):
        reasons.append(producer.get(
            "incomplete_reason", "producer execution is not fully bound"))
    if not base_runtime.get("scope_complete"):
        reasons.append(base_runtime.get(
            "incomplete_reason", "base runtime scope is incomplete"))
    if roundtrip and not roundtrip_runtime.get("scope_complete"):
        reasons.append(roundtrip_runtime.get(
            "incomplete_reason", "roundtrip runtime scope is incomplete"))
    record["attestation"] = {
        "inputs_complete": bool(input_manifest.get("inputs_complete")),
        "producer_execution_bound": bool(
            producer.get("assertion_producer_bound")),
        "base_runtime_scope_complete": bool(
            base_runtime.get("scope_complete")),
        "roundtrip_runtime_scope_complete": (
            bool(roundtrip_runtime.get("scope_complete"))
            if roundtrip else None
        ),
        "validated_before_after": bool(
            record["provenance_validation"]["validated_before"]
            and record["provenance_validation"]["validated_after"]),
        "complete": False,
        "enforceable": False,
        "incomplete_reasons": reasons,
        "future_gate_required": (
            "clean audit.py git-blob/bootstrap and native host/runtime binding"),
    }
    return record


def self_test() -> int:
    """Prove each assertion can fail, and that absence of evidence is failure.

    An assertion that cannot report a violation is decoration, so every one is
    fed a bundle it must reject. The fixtures are tiny by design: the corpus
    proves the assertions find real defects, this proves they would still find
    one if the corpus were clean.
    """
    failures: list[str] = []

    def check(name: str, condition: bool) -> None:
        if not condition:
            failures.append(name)

    verifier_totals = {
        key: 0 for key in sorted(_ROUND_TRIP_TOTAL_KEYS)
    }
    verifier_paper = {
        "reference": {"width_pt": 100.0, "height_pt": 100.0,
                      "page_count": 1},
        "candidate": {"width_pt": 100.0, "height_pt": 100.0,
                      "page_count": 1},
        "problems": [],
        "ok": True,
    }
    verifier_fixture = {
        "paper": verifier_paper,
        "totals": verifier_totals,
    }
    try:
        verifier_result = _validated_verifier_report(verifier_fixture)
    except Exception as error:  # noqa: BLE001 - the fixture must be complete
        verifier_result = None
        failures.append(
            "complete verifier report must validate: "
            f"{type(error).__name__}: {error}")
    check(
        "complete verifier report preserves paper and totals",
        verifier_result is not None
        and verifier_result[0] is True
        and verifier_result[1] is None
        and verifier_result[2] == verifier_totals,
    )
    verifier_malformed = [
        ("missing paper", lambda value: value.pop("paper")),
        ("bool paper verdict", lambda value: value["paper"].update({"ok": 1})),
        ("missing total", lambda value: value["totals"].pop("text_missing")),
        ("bool total", lambda value: value["totals"].update({"rules_missing": True})),
        ("paper failure without hard failure", lambda value: value["paper"].update({"ok": False})),
        ("paper success with hard failure", lambda value: value.update({"hard_failure": "paper mismatch"})),
    ]
    for label, mutator in verifier_malformed:
        malformed = copy.deepcopy(verifier_fixture)
        mutator(malformed)
        try:
            _validated_verifier_report(malformed)
        except RuntimeError:
            pass
        except Exception as error:  # noqa: BLE001 - malformed must not crash
            failures.append(
                f"malformed verifier {label} raised {type(error).__name__}: {error}")
        else:
            failures.append(f"malformed verifier {label} must fail closed")
    verifier_paper_failure = copy.deepcopy(verifier_fixture)
    verifier_paper_failure["paper"]["ok"] = False
    verifier_paper_failure["paper"]["problems"] = ["paper width mismatch"]
    verifier_paper_failure["hard_failure"] = "paper mismatch"
    try:
        paper_failure_result = _validated_verifier_report(verifier_paper_failure)
    except Exception as error:  # noqa: BLE001 - valid hard failure fixture
        paper_failure_result = None
        failures.append(
            "verifier paper mismatch must validate as measured=false: "
            f"{type(error).__name__}: {error}")
    check(
        "verifier paper mismatch retains complete zero totals",
        paper_failure_result is not None
        and paper_failure_result[0] is False
        and paper_failure_result[1] == "paper mismatch",
    )

    ir = {
        "form": {"code": "TEST", "revision": "0000"},
        "source": {"file": "external:none.pdf", "sha256": "0" * 64},
        "paper": {"width_pt": 100.0, "height_pt": 100.0},
        "pages": [{
            "index": 1, "width_pt": 100.0, "height_pt": 100.0, "rotation": 0,
            "rules": [{"id": "h0", "axis": "h", "x0": 0.0, "y0": 90.0, "x1": 50.0,
                       "y1": 90.24, "thickness_pt": 0.24, "role": "structural"}],
            "area_fills": [], "images": [],
            "text_runs": [{
                "text": "Rate?", "font": "Arial", "size_pt": 8.0, "color": 16777215,
                "x0": 10.0, "y0": 10.0, "x1": 30.0, "y1": 18.0, "origin_x": 10.0,
                "baseline_y": 16.0,
                "char_origin_offsets_pt": [0.0, 4.0, 8.0, 12.0, 16.0],
                "char_widths_pt": [4.0, 4.0, 4.0, 4.0, 4.0],
            }],
            "stats": {"rules_structural": 1},
        }],
    }
    html = (
        '<div class="page page-1" id="page-1" style="width:100pt;height:100pt">'
        '<svg class="rl"><rect x="0" y="90" width="50" height="0.24" '
        'fill="#000000" data-rule-id="h0"/></svg>'
        '<div class="layer-text"><div class="t" id="p1t0" style="left:10pt;top:10pt;'
        'color:#000000">Rate?</div></div>'
        '<div id="p1c0" class="c f" data-cell-kind="field" data-field-kind="text" '
        'style="left:8pt;top:8pt;width:30pt;height:12pt">'
        '<input type="text" class="fi" id="p1c0-i" name="p1c0" '
        'style="inset:0pt 0pt 0pt 0pt"></div>'
        '<div id="p1c1" class="c" data-cell-kind="mixed" data-comb-slots="2" '
        'style="left:50pt;top:50pt;width:20pt;height:10pt">'
        '<div class="s" data-slot="0" style="left:0pt;top:0pt;width:10pt;height:10pt">'
        '</div><div class="s" data-slot="1" style="left:10pt;top:0pt;width:10pt;'
        'height:10pt"></div></div>'
        '</div>')
    layout = {"pages": [{"index": 1, "cells": [
        {"id": "p1c0", "x0": 8.0, "y0": 8.0, "x1": 38.0, "y1": 20.0,
         "border": {"top": {}, "bottom": {}, "left": {}, "right": {}},
         "is_empty": False, "rectangular": True, "kind": "field", "text_run_ids": []},
        {"id": "p1c1", "x0": 50.0, "y0": 50.0, "x1": 70.0, "y1": 60.0,
         "border": {"top": {}, "bottom": {}, "left": {}, "right": {}},
         "is_empty": True, "rectangular": True, "kind": "mixed", "text_run_ids": [],
         "comb": {"cells": 2, "divider_x": [60.0], "slot_x": [50.0, 60.0, 70.0],
                  "y0": 56.0, "y1": 60.0}},
    ]}]}
    plan = {"inline": [{"page": 1, "cut_y_pt": 40.0, "rule_ids": [],
                        "text_run_indices": [0], "cell_ids": [],
                        "marker_pattern": "table-n", "marker": "Table 1"}]}
    guide_html = ('<section class="gl-page" data-page="1" data-flow="table">'
                  '<table class="gl-table"><tr><td></td><td>3%</td></tr></table>'
                  '</section>')

    b = Bundle(slug="test", ir=ir, layout=layout, plan=plan, form_html=html,
               guide_html=guide_html, pdf=None)
    results = evaluate_assertions(b)
    check("assertion evidence contains no wall-clock timing fields",
          '"seconds"' not in json.dumps(results, sort_keys=True))

    # 1: the input at 8..38 x 8..20 covers the glyphs of "Rate?" at 10..30.
    check("inputs_over_printed_text must fail on an input over glyph ink",
          results["inputs_over_printed_text"] is False)
    # 3: p1c1 prints two comb slots, neither carrying an input.
    check("money_boxes_have_inputs must fail on a comb with no inputs",
          results["money_boxes_have_inputs"] is False)
    # 4: h0 ends at y 90.24, the cut is at 40.
    check("rules_below_guide_cut must fail on a rule past the cut",
          results["rules_below_guide_cut"] is False)
    # 5: the run is white in the IR and black in the markup. It is also the
    # relocated run, so the guide-containment half must fire as well.
    check("run_colour_matches_ir must fail on a white run emitted black",
          results["run_colour_matches_ir"] is False)
    # 6: the guide's only row has an empty description and a 3% rate.
    check("reflow_rate_without_description must fail on a rate with no description",
          results["reflow_rate_without_description"] is False)
    # 2, 7, 8 need the source PDF, which this fixture deliberately lacks:
    # unevaluable must read as failure, not as a pass.
    for key in ("comb_slots_match_printed", "image_transform_applied",
                "no_invented_codepoints"):
        check(f"{key} must fail when the source PDF cannot be resolved",
              results[key] is False and "not resolved" in results["assertions"][key]["reason"])
    check("every assertion must name offenders or a reason",
          all(results["assertions"][k]["reason"] or results["assertions"][k]["offenders"]
              for k in ASSERTION_KEYS if not results[k]))

    # Every record binds the exact bytes it evaluated. Hashes are content-based
    # (not path- or timestamp-based), and a missing required guide plan prevents
    # the form from becoming an `ok` record.
    with tempfile.TemporaryDirectory(prefix="formgen-audit-inputs-") as tmp:
        root = pathlib.Path(tmp)
        ir_dir = root / "ir"
        html_dir = root / "html"
        layout_dir = root / "layout"
        guide_dir = root / "guides"
        source_dir = root / "sources"
        for directory in (
                ir_dir, html_dir, layout_dir, guide_dir, source_dir):
            directory.mkdir()

        slug = "test-bound"
        import fitz
        source_doc = fitz.open()
        source_doc.new_page(width=100.0, height=100.0)
        source_payload = source_doc.tobytes()
        source_doc.close()
        source_path = source_dir / f"{slug}.pdf"
        source_path.write_bytes(source_payload)
        bound_ir = copy.deepcopy(ir)
        bound_ir["source"] = {
            "file": f"external:{source_path.name}",
            "sha256": hashlib.sha256(source_payload).hexdigest(),
            "page_count": 1,
        }
        (ir_dir / f"{slug}.ir.json").write_text(
            json.dumps(bound_ir), encoding="utf-8")
        (layout_dir / f"{slug}.layout.json").write_text(
            json.dumps(layout), encoding="utf-8")
        (html_dir / "fonts").mkdir()
        (html_dir / "assets").mkdir()
        font_payload = b"retained test font bytes"
        image_payload = b"retained test image bytes"
        css_payload = (
            b'@font-face{font-family:"Bound";'
            b'src:url("../fonts/test.woff2")}')
        font_path = html_dir / "fonts" / "test.woff2"
        image_path = html_dir / "assets" / "test.png"
        css_path = html_dir / "assets" / "test.css"
        font_path.write_bytes(font_payload)
        image_path.write_bytes(image_payload)
        css_path.write_bytes(css_payload)
        bound_html = (
            '<link rel="stylesheet" href="assets/test.css">'
            '<img src="assets/test.png">'
            + html
        )
        html_path = html_dir / f"{slug}.html"
        html_path.write_text(bound_html, encoding="utf-8")
        guide_path = guide_dir / f"{slug}.guide.json"
        guide_path.write_text(json.dumps(plan), encoding="utf-8")
        (html_dir / f"{slug}.guide.html").write_text(
            guide_html, encoding="utf-8")

        first = snapshot_inputs(
            slug, ir_dir, html_dir, layout_dir, guide_dir, str(source_dir))
        check("input manifest binds every required byte source",
              first.manifest["inputs_complete"] is True
              and first.manifest["complete"] is False
              and first.manifest["attestation_complete"] is False
              and first.manifest["enforceable"] is False
              and first.manifest["missing_required"] == []
              and set(first.manifest["inputs"]) == {
                  "ir", "layout", "html", "guide", "guide_html",
                  "source_pdf"}
              and all(first.manifest["inputs"][role]["present"]
                      and first.manifest["inputs"][role]["sha256"]
                      for role in REQUIRED_INPUT_ROLES))
        check("render manifest retains every fetched font and image byte",
              first.render_assets == {
                  "assets/test.css": css_payload,
                  "assets/test.png": image_payload,
                  "fonts/test.woff2": font_payload,
              }
              and [
                  item["path"]
                  for item in first.manifest["render"]["dependencies"]
              ] == [
                  "assets/test.css",
                  "assets/test.png",
                  "fonts/test.woff2",
              ]
              and first.manifest["render"]["complete"] is True)
        check("source PDF manifest binds logical path and exact immutable bytes",
              first.manifest["inputs"]["source_pdf"] == {
                  "file": source_path.name,
                  "logical_identity": f"external:{source_path.name}",
                  "path": source_path.name,
                  "required": True,
                  "present": True,
                  "bytes": len(source_payload),
                  "sha256": hashlib.sha256(source_payload).hexdigest(),
                  "expected_sha256": hashlib.sha256(source_payload).hexdigest(),
              }
              and first.contents["source_pdf"] == source_payload)
        check("input manifest binds the exact audit producer bytes",
              first.manifest["producer"] == producer_fingerprint()
              and first.manifest["producer"]["dependency_execution_bound"]
              is True
              and first.manifest["producer"]["audit_execution_bound"]
              is False
              and first.manifest["producer"]["assertion_producer_bound"]
              is False)
        dependency_probe = root / "dependency-probe.py"
        dependency_probe.write_bytes(b"first dependency payload")
        dependency_before = file_fingerprint(
            dependency_probe, "dependency-probe.py")
        dependency_probe.write_bytes(b"stale dependency payload")
        dependency_after = file_fingerprint(
            dependency_probe, "dependency-probe.py")
        check("dependency fingerprint detects stale producer bytes",
              dependency_before["sha256"] != dependency_after["sha256"])
        check("input manifest publishes deterministic runtime provenance",
              first.manifest["runtime"] == runtime_provenance()
              and first.manifest["runtime"]["python"]["implementation"]
              and first.manifest["runtime"]["python"]["version"]
              and first.manifest["runtime"]["pymupdf"]["package_version"]
              and first.manifest["runtime"]["pymupdf"]["version_bind"])

        snapshotted_bundle = load_bundle(
            slug, ir_dir, html_dir, layout_dir, guide_dir, str(source_dir),
            input_snapshot=first)
        source_path.write_bytes(b"mutated after source snapshot")
        check("PDF assertions open the snapshotted bytes after path mutation",
              isinstance(snapshotted_bundle.pdf, bytes)
              and snapshotted_bundle.pdf == source_payload
              and snapshotted_bundle.doc.page_count == 1)
        snapshotted_bundle.close()
        stale_source = snapshot_inputs(
            slug, ir_dir, html_dir, layout_dir, guide_dir, str(source_dir))
        check("a newly observed stale source path fails hash resolution",
              stale_source.manifest["complete"] is False
              and stale_source.manifest["missing_required"] == ["source_pdf"]
              and stale_source.contents["source_pdf"] is None)
        source_path.write_bytes(source_payload)

        changed_html = bound_html.replace("Rate?", "Rote?", 1)
        html_path.write_text(changed_html, encoding="utf-8")
        second = snapshot_inputs(
            slug, ir_dir, html_dir, layout_dir, guide_dir, str(source_dir))
        check("input hash changes when exact bytes change",
              first.manifest["inputs"]["html"]["sha256"]
              != second.manifest["inputs"]["html"]["sha256"]
              and second.manifest["inputs"]["html"]["sha256"]
              == hashlib.sha256(changed_html.encode("utf-8")).hexdigest()
              and all(first.manifest["inputs"][role]["sha256"]
                      == second.manifest["inputs"][role]["sha256"]
                      for role in ("ir", "layout", "guide", "source_pdf"))
              and first.manifest["producer"] == second.manifest["producer"])

        html_snapshot_bundle = load_bundle(
            slug, ir_dir, html_dir, layout_dir, guide_dir, str(source_dir),
            input_snapshot=second)
        html_path.write_text("mutated after HTML snapshot", encoding="utf-8")
        font_path.write_bytes(b"mutated after dependency snapshot")
        with materialized_form_snapshot(
                html_snapshot_bundle, html_dir) as materialized:
            check("round trip prints snapshotted HTML after source path mutation",
                  materialized.entrypoint.read_bytes()
                  == second.contents["html"]
                  and materialized.entrypoint.read_bytes()
                  != html_path.read_bytes()
                  and (
                      materialized.root / "fonts" / "test.woff2"
                  ).read_bytes() == font_payload
                  and (
                      materialized.root / "fonts" / "test.woff2"
                  ).read_bytes() != font_path.read_bytes())
        render_mutation_failed = False
        try:
            with materialized_form_snapshot(
                    html_snapshot_bundle, html_dir) as materialized:
                materialized.entrypoint.chmod(0o600)
                materialized.entrypoint.write_bytes(
                    b"mutated isolated render tree")
        except RuntimeError as exc:
            render_mutation_failed = (
                "isolated render dependency bytes changed" in str(exc))
        check("isolated render tree mutation fails its after-use validation",
              render_mutation_failed)
        html_snapshot_bundle.close()
        html_path.write_text(changed_html, encoding="utf-8")
        font_path.write_bytes(font_payload)

        bound = score(slug, ir_dir, html_dir, layout_dir, guide_dir,
                      root / "work", str(source_dir), roundtrip=False)
        check("successful record publishes the exact input manifest",
              bound["status"] == "ok"
              and bound["input_manifest"] == second.manifest)

        guide_path.unlink()
        missing = score(slug, ir_dir, html_dir, layout_dir, guide_dir,
                        root / "work", str(source_dir), roundtrip=False)
        check("missing required guide input fails closed",
              missing["status"] == "error"
              and missing["input_manifest"]["complete"] is False
              and missing["input_manifest"]["missing_required"] == ["guide"]
              and all(missing.get(key) is False for key in ASSERTION_KEYS))

    with tempfile.TemporaryDirectory(
            prefix="formgen-audit-adversarial-") as tmp:
        adversarial_root = pathlib.Path(tmp)
        (adversarial_root / "present.png").write_bytes(b"present")
        _payloads, _entries, remote_errors = discover_render_dependencies(
            b'<img src="https://example.invalid/tracker.png">',
            "form.html",
            adversarial_root,
        )
        _payloads, _entries, missing_errors = discover_render_dependencies(
            b'<script src="missing.js"></script>',
            "form.html",
            adversarial_root,
        )
        _payloads, _entries, query_errors = discover_render_dependencies(
            b'<img src="present.png?v=mutable">',
            "form.html",
            adversarial_root,
        )
        symlink = adversarial_root / "linked.png"
        symlink.symlink_to(adversarial_root / "present.png")
        _payloads, _entries, symlink_errors = discover_render_dependencies(
            b'<img src="linked.png">',
            "form.html",
            adversarial_root,
        )
        check("remote render dependencies fail closed before Chromium",
              any("external or absolute" in item
                  for item in remote_errors))
        check("missing render dependencies fail closed before Chromium",
              any("unresolved render dependency" in item
                  for item in missing_errors))
        check("query-bearing render dependencies fail closed as ambiguous",
              any("query-bearing" in item for item in query_errors))
        check("symlinked render dependencies fail closed",
              any("symlinked dependency" in item
                  for item in symlink_errors))
        retained_policy = {"form.html": b"<html></html>"}
        policy_path, policy_payload = _retained_request_payload(
            f"{SYNTHETIC_RENDER_ORIGIN}/form.html",
            "GET",
            retained_policy,
        )
        check("synthetic-origin GET resolves only retained bytes",
              policy_path == "form.html"
              and policy_payload == retained_policy["form.html"])
        for label, url, method, reason in (
                ("remote", "https://example.invalid/form.html", "GET",
                 "outside the synthetic"),
                ("unknown", f"{SYNTHETIC_RENDER_ORIGIN}/missing.js", "GET",
                 "absent from retained"),
                ("write method", f"{SYNTHETIC_RENDER_ORIGIN}/form.html",
                 "POST", "only GET")):
            try:
                _retained_request_payload(
                    url, method, retained_policy)
            except ValueError as exc:
                request_failed = reason in str(exc)
            else:
                request_failed = False
            check(f"{label} browser request fails retained-byte policy",
                  request_failed)

        browser_fixture_prefix = (
            "<!doctype html><meta charset=\"utf-8\">"
            "<style>@page{size:72pt 72pt;margin:0}"
            "html,body{margin:0;width:72pt;height:72pt}</style>"
        )
        # Two budgets, because the fixtures below assert opposite things. The
        # hang fixtures must exceed their deadline, so theirs stays tight. The
        # control must finish inside its own, and it was sharing the tight one.
        #
        # That mattered because _bound_playwright_runtime SHA-256s the entire
        # ~873 MiB Playwright tree before and after every render -- roughly
        # 1.75 GiB of hashing inside the budget. Warm, it is CPU-bound off the
        # page cache and fits; cold, it is I/O-bound and does not. So on a cold
        # runner the control died with an uncaught RenderDeadlineExceeded while
        # the three hang fixtures passed VACUOUSLY: the closure hashing alone
        # exhausted the deadline they exist to prove is enforced. Three real
        # assertions quietly became no-ops, which is worse than the red one.
        hang_deadline = 8.0
        control_deadline = 60.0

        # Prime the page cache once, outside any timed section, so the first
        # render is not paying for the whole tree's first read.
        _snapshot_tree(_playwright_package_root())

        def run_browser_fixture(
                label: str,
                body: str,
                deadline: float = hang_deadline,
                ) -> tuple[bytes, dict[str, Any], dict[str, Any]]:
            fixture_root = adversarial_root / f"browser-{label}"
            fixture_root.mkdir()
            entrypoint = fixture_root / "form.html"
            payload = (browser_fixture_prefix + body).encode("utf-8")
            entrypoint.write_bytes(payload)
            entrypoint.chmod(0o400)
            tree = MaterializedRenderTree(
                root=fixture_root,
                entrypoint=entrypoint,
                expected={"form.html": payload},
            )
            return _render_snapshotted_tree(
                tree, 72.0, 72.0,
                deadline_seconds=deadline)

        # A deadline the control blows is a finding, not a traceback: it must
        # land in the failure count like every other assertion, or a cold runner
        # reports a crash where it should report a result.
        try:
            control_pdf, control_runtime, control_requests = (
                run_browser_fixture(
                    "control", "<body>bounded control</body>",
                    deadline=control_deadline))
        except RenderDeadlineExceeded as exc:
            check(f"actual-browser control completes inside its hard deadline "
                  f"({exc})", False)
            control_pdf, control_runtime, control_requests = b"", {}, {}
        check("actual-browser control completes inside its hard deadline",
              control_pdf.startswith(b"%PDF-")
              and control_runtime["hard_deadline_seconds"]
              == control_deadline
              and control_runtime["hard_deadline_enforced_by"]
              == "isolated-render-worker-process-v1"
              and control_requests["fulfilled"] == ["form.html"]
              and control_requests["blocked_requests"] == 0)

        try:
            run_browser_fixture(
                "websocket",
                "<script>try{new WebSocket("
                "\"wss://example.invalid/socket\")}catch(error){}</script>"
                "<body>websocket probe</body>",
            )
        except RuntimeError as exc:
            websocket_rejected = (
                not isinstance(exc, RenderDeadlineExceeded)
                and "websockets" in str(exc)
                and "wss://example.invalid/socket" in str(exc)
            )
        else:
            websocket_rejected = False
        check("actual-browser WebSocket is rejected without callback deadlock",
              websocket_rejected)

        def expect_browser_deadline(label: str, body: str) -> None:
            try:
                run_browser_fixture(label, body)
            except RenderDeadlineExceeded as exc:
                deadline_held = (
                    exc.deadline_seconds == hang_deadline
                    and "deterministic hard deadline" in str(exc))
            else:
                deadline_held = False
            check(
                f"actual-browser {label} cannot outlive render deadline",
                deadline_held,
            )

        expect_browser_deadline(
            "never-font",
            "<script>Object.defineProperty(document,'fonts',{"
            "configurable:true,value:{ready:new Promise(()=>{})}})</script>"
            "<body>font promise probe</body>",
        )
        expect_browser_deadline(
            "never-fetch",
            "<script>"
            "let fetchSettled=false;"
            "Object.defineProperty(globalThis,'fetch',{"
            "value:()=>new Promise(()=>{})});"
            "fetch('https://formgen.invalid/never').finally("
            "()=>{fetchSettled=true});"
            "while(!fetchSettled){}"
            "</script><body>fetch promise probe</body>",
        )
        expect_browser_deadline(
            "never-script",
            "<script>while(true){}</script>"
            "<body>blocked page script probe</body>",
        )
        deadline_evidence = _render_deadline_evidence(
            RenderDeadlineExceeded(hang_deadline))
        check("render deadline publishes explicit unevaluable evidence",
              deadline_evidence == {
                  "measured": False,
                  "hard_failure": "render-hard-deadline-exceeded",
                  "roundtrip_liveness": {
                      "status": "unevaluable",
                      "hard_failure": "render-hard-deadline-exceeded",
                      "hard_deadline_seconds": hang_deadline,
                      "cleanup_policy": (
                          "kill-worker-and-chromium-process-group"),
                  },
              })

        probe_extract = adversarial_root / "extract.py"
        probe_verify = adversarial_root / "verify.py"
        probe_extract.write_text("VALUE = 'retained'\n", encoding="utf-8")
        probe_verify.write_text(
            "import extract\nVALUE = extract.VALUE\n",
            encoding="utf-8",
        )
        decoy = types.ModuleType("extract")
        decoy.VALUE = "substituted"
        prior_extract = sys.modules.get("extract")
        sys.modules["extract"] = decoy
        try:
            loaded_extract, loaded_verify = _load_trusted_formgen_modules(
                probe_extract, probe_verify)
            binding_restored = sys.modules.get("extract") is decoy
        finally:
            if prior_extract is None:
                sys.modules.pop("extract", None)
            else:
                sys.modules["extract"] = prior_extract
        probe_extract.write_text("VALUE = 'mutated'\n", encoding="utf-8")
        check("trusted loader ignores a preseeded sys.modules substitution",
              loaded_extract.module.VALUE == "retained"
              and loaded_verify.module.VALUE == "retained"
              and loaded_verify.module.extract is loaded_extract.module
              and binding_restored)
        check("trusted loader fingerprints retained executed source bytes",
              loaded_extract.sha256
              == hashlib.sha256(loaded_extract.payload).hexdigest()
              and loaded_extract.module.VALUE == "retained"
              and _stable_read(probe_extract) != loaded_extract.payload)

        prior_extract = sys.modules.get("extract")
        sys.modules["extract"] = decoy
        try:
            try:
                validate_trusted_producer_sources()
            except RuntimeError as exc:
                substituted_binding_failed = (
                    "module binding was substituted" in str(exc))
            else:
                substituted_binding_failed = False
        finally:
            if prior_extract is None:
                sys.modules.pop("extract", None)
            else:
                sys.modules["extract"] = prior_extract
        check("post-load producer module substitution fails validation",
              substituted_binding_failed)

        runtime_tree = adversarial_root / "runtime"
        runtime_tree.mkdir()
        runtime_member = runtime_tree / "driver"
        runtime_member.write_bytes(b"retained runtime")
        runtime_closure = _snapshot_tree(runtime_tree)
        runtime_member.write_bytes(b"mutated runtime")
        try:
            _validate_tree_closure(runtime_closure, "after adversary")
        except RuntimeError as exc:
            runtime_mutation_failed = (
                "runtime dependency closure changed" in str(exc))
        else:
            runtime_mutation_failed = False
        check("runtime dependency mutation fails closure validation",
              runtime_mutation_failed)

        bound_playwright = types.ModuleType("playwright")
        expected_playwright = {"playwright": id(bound_playwright)}
        _validate_playwright_module_bindings(
            {"playwright": bound_playwright}, expected_playwright)
        for label, loaded, expected, reason in (
                (
                    "preloaded",
                    {"playwright": bound_playwright},
                    None,
                    "imported before",
                ),
                (
                    "expanded",
                    {
                        "playwright": bound_playwright,
                        "playwright.injected": types.ModuleType(
                            "playwright.injected"),
                    },
                    expected_playwright,
                    "module set changed",
                ),
                (
                    "substituted",
                    {"playwright": types.ModuleType("playwright")},
                    expected_playwright,
                    "was substituted",
                ),
        ):
            try:
                _validate_playwright_module_bindings(loaded, expected)
            except RuntimeError as exc:
                playwright_binding_failed = reason in str(exc)
            else:
                playwright_binding_failed = False
            check(
                f"{label} Playwright module binding fails closed",
                playwright_binding_failed,
            )

    synthetic_pdf = (
        b"%PDF-1.7\n"
        b"/CreationDate (D:20260731123456+00'00')\n"
        b"/ModDate (D:20260731123456+00'00')\n"
        b"%%EOF\n"
    )
    canonical_pdf, canonicalization = _canonicalize_chromium_pdf(
        synthetic_pdf)
    check("Chromium PDF volatile dates normalize without moving offsets",
          len(canonical_pdf) == len(synthetic_pdf)
          and b"20260731123456" not in canonical_pdf
          and canonicalization["fields_normalized"] == 2
          and canonicalization["xref_offsets_preserved"] is True)

    def retained_candidate_probe(
            path: pathlib.Path, code: str, revision: str,
            expected_sha: str,
            ) -> dict[str, Any]:
        payload = path.read_bytes()
        if hashlib.sha256(payload).hexdigest() != expected_sha:
            raise AssertionError("candidate hash was not passed to extractor")
        return {
            "source": {
                "file": f"external:{path.name}",
                "sha256": expected_sha,
                "bytes": len(payload),
            },
            "generator": {"producer": "probe"},
            "form": {"code": code, "revision": revision},
            "pages": [],
        }

    _candidate, candidate_record = _extract_retained_candidate(
        canonical_pdf, "TEST", "0000",
        extractor=retained_candidate_probe)
    check("candidate extraction binds retained bytes and canonical IR",
          candidate_record["expected_sha256_passed_to_extractor"] is True
          and candidate_record["validated_before_after_extraction"] is True
          and len(candidate_record["candidate_ir_sha256"]) == 64)

    mutation_attempt = {"prevented": False}

    def mutating_candidate_probe(
            path: pathlib.Path, code: str, revision: str,
            expected_sha: str,
            ) -> dict[str, Any]:
        try:
            path.chmod(0o600)
            path.write_bytes(b"mutated during extraction")
        except PermissionError:
            mutation_attempt["prevented"] = True
        return {
            "source": {
                "file": f"external:{path.name}",
                "sha256": expected_sha,
                "bytes": len(canonical_pdf),
            },
            "generator": {"producer": "probe"},
            "form": {"code": code, "revision": revision},
            "pages": [],
        }

    _extract_retained_candidate(
        canonical_pdf, "TEST", "0000",
        extractor=mutating_candidate_probe)
    check("unlinked read-only candidate descriptor prevents path mutation",
          mutation_attempt["prevented"])

    # The clean side: the same page with the input moved off the ink, the comb
    # filled, the cut below everything, the colour right and the row complete.
    clean_html = html.replace('style="left:8pt;top:8pt;width:30pt;height:12pt"',
                              'style="left:40pt;top:30pt;width:30pt;height:12pt"')
    clean_html = clean_html.replace('color:#000000', 'color:#ffffff')
    clean_html = clean_html.replace(
        '<div class="s" data-slot="0" style="left:0pt;top:0pt;width:10pt;height:10pt">'
        '</div>',
        '<div class="s" data-slot="0" style="left:0pt;top:0pt;width:10pt;height:10pt">'
        '<input type="text" class="fi fc" data-slot-index="0"></div>')
    clean_html = clean_html.replace(
        '<div class="s" data-slot="1" style="left:10pt;top:0pt;width:10pt;height:10pt">'
        '</div>',
        '<div class="s" data-slot="1" style="left:10pt;top:0pt;width:10pt;height:10pt">'
        '<input type="text" class="fi fc" data-slot-index="1"></div>')
    clean_layout = copy.deepcopy(layout)
    clean_layout["pages"][0]["cells"][0]["x0"] = 40.0
    clean_layout["pages"][0]["cells"][0]["x1"] = 70.0
    clean_layout["pages"][0]["cells"][0]["y0"] = 30.0
    clean_layout["pages"][0]["cells"][0]["y1"] = 42.0
    clean_plan = copy.deepcopy(plan)
    clean_plan["inline"][0]["cut_y_pt"] = 95.0
    clean_guide = guide_html.replace("<td></td>", "<td>Franchise tax</td>")
    clean_guide += '<span style="color:#ffffff">initials</span>'
    clean = Bundle(slug="test", ir=ir, layout=clean_layout, plan=clean_plan,
                   form_html=clean_html, guide_html=clean_guide, pdf=None)
    ok = evaluate_assertions(clean)
    for key in ("inputs_over_printed_text", "money_boxes_have_inputs",
                "rules_below_guide_cut", "run_colour_matches_ir",
                "reflow_rate_without_description"):
        check(f"{key} must hold on the corrected fixture: "
              f"{ok['assertions'][key]['reason']}", ok[key] is True)

    # A run assigned to the same lattice cell is still printed ink. Ownership
    # explains the collision; it does not make a live input over that ink safe.
    owned_layout = copy.deepcopy(layout)
    owned_layout["pages"][0]["cells"][0]["text_run_ids"] = ["p1t0"]
    owned = Bundle(slug="owned", ir=ir, layout=owned_layout, plan=None,
                   form_html=html, guide_html=None, pdf=None)
    check("an input over its own cell's printed run still fails",
          check_inputs_over_printed_text(owned)["holds"] is False)

    # The live page's last cell is immediately followed by inert band template
    # markup. An input inside that template must not be attributed to the cell.
    template_html = (
        '<div class="page page-1" id="page-1" style="width:100pt;height:100pt">'
        '<div class="t" id="p1t0" style="left:10pt;top:10pt">Rate?</div>'
        '<div id="p1c9" class="c" data-cell-kind="label" '
        'style="left:8pt;top:8pt;width:30pt;height:12pt"></div></div>'
        '<template id="band-template-p1g0"><input type="text" class="fi" '
        'style="inset:0pt 0pt 0pt 0pt"></template><script></script>')
    template_cells = parse_cells(template_html)
    template_bundle = Bundle(slug="template", ir=ir, layout=None, plan=None,
                             form_html=template_html, guide_html=None, pdf=None)
    check("template inputs do not belong to the preceding live cell",
          len(template_cells) == 1
          and not input_boxes(template_cells[0])
          and check_inputs_over_printed_text(template_bundle)["holds"] is True)

    enclosed_plain_layout = {"pages": [{"index": 1, "cells": [{
        "id": "p1c0",
        "x0": 8.0, "y0": 8.0, "x1": 38.0, "y1": 20.0,
        "border": {
            "top": {"gray": 0.0}, "bottom": {"gray": 0.0},
            "left": {"gray": 0.0}, "right": {"gray": 0.0},
        },
        "is_empty": True,
        "rectangular": True,
        "kind": "field",
        "text_run_ids": [],
    }]}]}
    plain_input = (
        '<input type="text" class="fi" '
        'style="inset:0pt 0pt 0pt 0pt">')
    for inert_label, inert_markup in (
            ("comment", f"<!--{plain_input}-->"),
            ("script", f"<script>{plain_input}</script>"),
            ("template", f"<template>{plain_input}</template>")):
        inert_plain_html = (
            '<div class="page page-1">'
            '<div id="p1c0" class="c f" data-cell-kind="field" '
            'data-field-kind="text" '
            'style="left:8pt;top:8pt;width:30pt;height:12pt">'
            + inert_markup
            + "</div></div>"
        )
        inert_plain_bundle = Bundle(
            slug=f"inert-plain-{inert_label}",
            ir=ir,
            layout=enclosed_plain_layout,
            plan=None,
            form_html=inert_plain_html,
            guide_html=None,
            pdf=None,
        )
        inert_plain_result = check_money_boxes_have_inputs(
            inert_plain_bundle)
        check(
            f"{inert_label} plain input does not make an empty box fillable",
            inert_plain_result["holds"] is False
            and inert_plain_result["offenders"][0]["why"]
            == "enclosed empty box, no input"
            and not input_boxes(inert_plain_bundle.cells[0]),
        )

    moved_plain_ir = copy.deepcopy(ir)
    moved_page = copy.deepcopy(ir["pages"][0])
    moved_page["index"] = 2
    moved_plain_ir["pages"] = [moved_page]
    moved_plain_html = (
        '<div class="page page-2">'
        '<div class="t" id="p2t0" '
        'style="left:10pt;top:10pt;color:#000000">Rate?</div>'
        '<div id="p1c0" class="c f" data-cell-kind="field" '
        'data-field-kind="text" '
        'style="left:8pt;top:8pt;width:30pt;height:12pt">'
        + plain_input
        + "</div></div>"
    )
    moved_plain_bundle = Bundle(
        slug="moved-plain",
        ir=moved_plain_ir,
        layout=enclosed_plain_layout,
        plan=None,
        form_html=moved_plain_html,
        guide_html=None,
        pdf=None,
    )
    moved_plain_overlap = check_inputs_over_printed_text(moved_plain_bundle)
    moved_plain_money = check_money_boxes_have_inputs(moved_plain_bundle)
    check(
        "plain field id page cannot substitute for its enclosing DOM page",
        moved_plain_overlap["holds"] is False
        and moved_plain_money["holds"] is False
        and moved_plain_overlap["offenders"][0]["emitted_id_page"] == 1
        and moved_plain_overlap["offenders"][0]["emitted_dom_page"] == 2
        and moved_plain_overlap["offenders"][0]["layout_page"] == 1
        and "emitted-cell-page-mismatch"
        in moved_plain_overlap["offenders"][0]["failure_kinds"],
    )

    # A malformed comb can put a slot outside its parent. The real `.f` clips
    # that slot, so glyph ink in the clipped-away area is not under an input.
    off_cell_html = (
        '<div class="page page-1" id="page-1" style="width:100pt;height:100pt">'
        '<div class="t" id="p1t0" style="left:10pt;top:10pt">Rate?</div>'
        '<div id="p1c0" class="c f" data-cell-kind="mixed" '
        'style="left:8pt;top:20pt;width:30pt;height:12pt">'
        '<div class="s" data-slot="0" '
        'style="left:0pt;top:-12pt;width:30pt;height:8pt">'
        '<input type="text" class="fi fc" data-slot-index="0"></div></div></div>')
    off_cell = Bundle(slug="off-cell", ir=ir, layout=None, plan=None,
                      form_html=off_cell_html, guide_html=None, pdf=None)
    check("comb input geometry clipped outside its parent cannot collide",
          check_inputs_over_printed_text(off_cell)["holds"] is True)

    # Most assertion details are bounded previews, and say exactly what they
    # omit. The comb assertion is the referee's evidence packet, so every
    # offender must survive publication -- including the first one beyond the
    # old twelve-item preview.
    preview = broken("preview", list(range(MAX_OFFENDERS + 1)))
    check("bounded offender previews state that one record was omitted",
          len(preview["offenders"]) == MAX_OFFENDERS
          and preview["offenders_published"] == MAX_OFFENDERS
          and preview["offenders_omitted"] == 1
          and preview["offenders_complete"] is False)

    class CombPublicationFixture:
        layout = {"pages": []}
        doc = object()
        cells: list[Cell] = []
        relocated_cells: set[str] = set()
        layout_pages = {1: {"cells": [
            {"id": f"p1c{index}", "x0": 0.0, "y0": 0.0,
             "x1": 40.0, "y1": 10.0,
             "comb": {"cells": 1, "y0": 2.0, "y1": 8.0,
                      "divider_gray": 0.0}}
            for index in range(MAX_OFFENDERS + 1)
        ]}}
        vector_pages = {1: VectorPage((
            VectorPaint(19.88, 2.0, 20.12, 8.0, 0.0, 1.0, 0, "test"),
        ), ())}

    complete = check_comb_slots_match_printed(CombPublicationFixture())
    check("comb publication keeps the offender beyond the old preview limit",
          complete["offender_count"] == MAX_OFFENDERS + 2
          and complete["offenders_published"] == MAX_OFFENDERS + 2
          and complete["offenders_omitted"] == 0
          and complete["offenders_complete"] is True
          and len(complete["offenders"]) == MAX_OFFENDERS + 2
          and complete["offenders"][0]["cell"]
          == "<comb-owner-registry>"
          and complete["offenders"][-1]["cell"] == f"p1c{MAX_OFFENDERS}")

    class CombEmissionFixture:
        layout = {"pages": []}
        doc = object()
        relocated_cells: set[str] = set()

        def _snapshot_layout(self) -> None:
            self.layout = {
                "pages": [
                    self.layout_pages[index]
                    for index in sorted(self.layout_pages)
                ],
            }
            self.layout_payload = json.dumps(
                self.layout, sort_keys=True, separators=(",", ":"),
            ).encode("utf-8")
            self.layout_sha256 = hashlib.sha256(
                self.layout_payload).hexdigest()

        def _bind_owner_registry(self) -> None:
            for page_index, page in sorted(self.layout_pages.items()):
                page["index"] = page_index
                subjects = []
                for cell in page["cells"]:
                    bbox = [
                        cell[name] for name in ("x0", "y0", "x1", "y1")
                    ]
                    decimal_bbox = [
                        _canonical_decimal(value) for value in bbox
                    ]
                    if any(value is None for value in decimal_bbox):
                        raise AssertionError("comb fixture bbox is not exact")
                    subject_key = (
                        f"p{page_index}@"
                        + ",".join(
                            _decimal_identity(value)
                            for value in decimal_bbox
                            if value is not None
                        )
                    )
                    cell["subject_key"] = subject_key
                    if not isinstance(cell.get("comb"), dict):
                        continue
                    subjects.append({
                        "subject_key": subject_key,
                        "legacy_cell_id": cell["id"],
                        "legacy_bbox": bbox,
                        "cell_id": cell["id"],
                        "mapped_partition_cell_ids": [cell["id"]],
                        "state": "active_unresolved",
                        "reason_codes": ["competing-endpoint-topologies"],
                        "cells": cell["comb"].get("cells"),
                        "blocks_gate": True,
                    })
                page["comb_subjects"] = subjects
            self._snapshot_layout()

        def __init__(self, cells: Sequence[Cell], count: int = 3) -> None:
            self.cells = list(cells)
            width = float(count * 10)
            self.layout_pages = {1: {"cells": [{
                "id": "p1c0", "x0": 0.0, "y0": 0.0,
                "x1": width, "y1": 10.0,
                "comb": {
                    "cells": count,
                    "divider_x": [
                        float(index * 10)
                        for index in range(1, count)
                    ],
                    "slot_x": [
                        float(index * 10)
                        for index in range(count + 1)
                    ],
                },
            }]}}
            dividers = [
                VectorPaint(
                    10.0 * index - 0.12, 2.0,
                    10.0 * index + 0.12, 8.0,
                    0.0, 1.0, index, "test",
                )
                for index in range(count + 1)
            ]
            dividers.append(VectorPaint(
                0.0, 7.88, width, 8.12,
                0.0, 1.0, count + 1, "test-baseline",
            ))
            self.vector_pages = {1: VectorPage(tuple(dividers), ())}
            self._bind_owner_registry()

    def emitted_comb_cell(
            cell_id: str = "p1c0",
            slot_indexes: Sequence[int] = (0, 1, 2),
            input_indexes: Sequence[int | None] | None = None,
            declared: int | None = None,
            geometry: Sequence[tuple[float, float, float, float]] | None = None,
            ) -> Cell:
        if input_indexes is None:
            input_indexes = slot_indexes
        if len(input_indexes) != len(slot_indexes):
            raise AssertionError("slot/input fixture lengths differ")
        if geometry is None:
            geometry = [
                (ordinal * 10.0, 0.0, 10.0, 10.0)
                for ordinal in range(len(slot_indexes))
            ]
        if len(geometry) != len(slot_indexes):
            raise AssertionError("slot/geometry fixture lengths differ")
        container_width = max(
            (left + width for left, _top, width, _height in geometry),
            default=10.0,
        )
        slots = []
        for slot_index, input_index, box in zip(
                slot_indexes, input_indexes, geometry):
            input_attr = (
                f' data-slot-index="{input_index}"'
                if input_index is not None else ""
            )
            left, top, width, height = box
            slots.append(
                f'<div class="s" data-slot="{slot_index}" '
                f'style="left:{left}pt;top:{top}pt;'
                f'width:{width}pt;height:{height}pt">'
                f'<input type="text" class="fi fc"{input_attr}></div>'
            )
        declared_value = len(slot_indexes) if declared is None else declared
        return Cell(
            id=cell_id,
            page=1,
            classes="c f",
            attrs=(
                f' data-comb-slots="{declared_value}" '
                f'style="left:0pt;top:0pt;'
                f'width:{container_width}pt;height:10pt"'
            ),
            rect=(0.0, 0.0, container_width, 10.0),
            inner="".join(slots),
        )

    missing_cell = check_comb_slots_match_printed(CombEmissionFixture([]))
    missing_markup_cell = Cell(
        id="p1c0", page=1, classes="c f",
        attrs=' style="left:0pt;top:0pt;width:40pt;height:10pt"',
        rect=(0.0, 0.0, 40.0, 10.0), inner="")
    missing_markup = check_comb_slots_match_printed(
        CombEmissionFixture([missing_markup_cell]))
    zero_slot_cell = dataclasses.replace(
        missing_markup_cell,
        attrs=(' data-comb-slots="0" '
               'style="left:0pt;top:0pt;width:40pt;height:10pt"'),
    )
    zero_slots = check_comb_slots_match_printed(
        CombEmissionFixture([zero_slot_cell]))
    for label, result, state, slots in (
            ("missing emitted cell", missing_cell,
             "missing-emitted-cell", None),
            ("missing emitted comb markup", missing_markup,
             "missing-comb-markup", None),
            ("zero physical emitted slots", zero_slots,
             "zero-physical-slots", 0)):
        offender = result["offenders"][0] if result["offenders"] else {}
        check(
            f"{label} fails closed without substituting the lattice count",
            result["holds"] is False
            and result["offender_count"] == 1
            and result["offenders_complete"] is True
            and result["emission_behind_layout"] == 1
            and result["layout_mismatches"] == 0
            and offender.get("printed") == 3
            and offender.get("latticed") == 3
            and offender.get("slots") == slots
            and offender.get("emission_state") == state
            and offender.get("layout_relation") == "match"
            and offender.get("emission_relation") == "invalid"
            and "invalid-emission" in offender.get("failure_kinds", ()),
        )

    valid_three = emitted_comb_cell()
    relocated_container = check_comb_slots_match_printed(
        CombEmissionFixture([
            dataclasses.replace(
                valid_three, rect=(100.0, 100.0, 130.0, 110.0))
        ]))
    relocated_offender = relocated_container["offenders"][0]
    check(
        "equal counts cannot hide a relocated emitted comb container",
        relocated_container["holds"] is False
        and relocated_container["emission_invalid"] == 1
        and relocated_offender["emission_container_binding"]["rect_matches"]
        is False
        and "emission-container-geometry-mismatch"
        in relocated_offender["failure_kinds"],
    )

    resized_container = check_comb_slots_match_printed(
        CombEmissionFixture([
            dataclasses.replace(
                valid_three, rect=(0.0, 0.0, 30.0, 11.0))
        ]))
    check(
        "equal counts cannot hide a resized emitted comb container",
        resized_container["holds"] is False
        and "emission-container-geometry-mismatch"
        in resized_container["offenders"][0]["failure_kinds"],
    )

    uneven_slots = check_comb_slots_match_printed(
        CombEmissionFixture([
            emitted_comb_cell(
                geometry=((0.0, 0.0, 1.0, 10.0),
                          (1.0, 0.0, 28.0, 10.0),
                          (29.0, 0.0, 1.0, 10.0)))
        ]))
    uneven_offender = uneven_slots["offenders"][0]
    check(
        "equal counts cannot hide physical slot edges at wrong positions",
        uneven_slots["holds"] is False
        and uneven_offender["emission_layout_position"]["matches"] is False
        and uneven_offender["emission_source_position"]["matches"] is False
        and "emission-layout-position-mismatch"
        in uneven_offender["failure_kinds"]
        and "emission-source-position-mismatch"
        in uneven_offender["failure_kinds"],
    )

    precision_fixture = CombEmissionFixture(
        [emitted_comb_cell(slot_indexes=(0, 1))], count=2)
    precision_fixture.vector_pages = {1: VectorPage((
        VectorPaint(
            -0.12, 2.0, 0.12, 8.0,
            0.0, 1.0, 0, "precision-left-rail"),
        VectorPaint(
            9.884, 2.0, 10.124, 8.0,
            0.0, 1.0, 1, "precision-adversary"),
        VectorPaint(
            19.88, 2.0, 20.12, 8.0,
            0.0, 1.0, 2, "precision-right-rail"),
        VectorPaint(
            0.0, 7.88, 20.0, 8.12,
            0.0, 1.0, 3, "precision-baseline"),
    ), ())}
    precision_result = check_comb_slots_match_printed(precision_fixture)
    precision_offender = precision_result["offenders"][0]
    check(
        "source position binding retains sub-centipoint divider precision",
        precision_result["holds"] is False
        and precision_offender["emission_layout_position"]["matches"] is True
        and precision_offender["emission_source_position"]
        ["expected_internal_edges_x"] == [10.004]
        and precision_offender["emission_source_position"]["deltas_pt"]
        == [-0.004]
        and "emission-source-position-mismatch"
        in precision_offender["failure_kinds"],
    )

    def source_frame_binding_fixture(
            slot_edges: Sequence[float],
            ) -> CombEmissionFixture:
        geometry = [
            (
                float(left), 0.0,
                float(right - left), 10.0,
            )
            for left, right in zip(slot_edges, slot_edges[1:])
        ]
        emitted = emitted_comb_cell(
            slot_indexes=tuple(range(len(geometry))),
            geometry=geometry,
        )
        emitted = dataclasses.replace(
            emitted,
            attrs=(
                f' data-comb-slots="{len(geometry)}" '
                'style="left:0pt;top:0pt;width:40pt;height:10pt"'
            ),
            rect=(0.0, 0.0, 40.0, 10.0),
        )
        fixture = CombEmissionFixture([emitted], count=len(geometry))
        fixture.layout_pages = {1: {"cells": [{
            "id": "p1c0",
            "x0": 0.0, "y0": 0.0, "x1": 40.0, "y1": 10.0,
            "comb": {
                "cells": len(geometry),
                "divider_x": [
                    float(value) for value in slot_edges[1:-1]],
                "slot_x": [float(value) for value in slot_edges],
            },
        }]}}
        fixture._bind_owner_registry()
        rail_and_dividers = [
            VectorPaint(
                value - 0.12, 2.0, value + 0.12, 8.0,
                0.0, 1.0, index, "outer-binding-frame",
            )
            for index, value in enumerate(
                (5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0))
        ]
        rail_and_dividers.append(VectorPaint(
            5.0, 7.88, 35.0, 8.12,
            0.0, 1.0, 20, "outer-binding-baseline",
        ))
        fixture.vector_pages = {
            1: VectorPage(tuple(rail_and_dividers), ())}
        return fixture

    inset_frame = check_comb_slots_match_printed(
        source_frame_binding_fixture(
            (5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0)))
    check(
        "an inset physical comb binds its outer edges to source rails",
        inset_frame["holds"] is True,
    )
    for label, edges in (
            ("blank-margin expansion",
             (0.0, 10.0, 15.0, 20.0, 25.0, 30.0, 40.0)),
            ("symmetric shrink",
             (6.0, 10.0, 15.0, 20.0, 25.0, 30.0, 34.0)),
            ("relocation",
             (6.0, 11.0, 16.0, 21.0, 26.0, 31.0, 36.0))):
        frame_binding = check_comb_slots_match_printed(
            source_frame_binding_fixture(edges))
        frame_offender = frame_binding["offenders"][0]
        left_geometry = frame_offender[
            "source_frame_geometry"]["left_rail"]
        right_geometry = frame_offender[
            "source_frame_geometry"]["right_rail"]
        check(
            f"source rail binding rejects {label}",
            frame_binding["holds"] is False
            and {
                key: left_geometry[key]
                for key in ("center_x", "ink_x0", "ink_x1")
            } == {"center_x": 5.0, "ink_x0": 4.88, "ink_x1": 5.12}
            and {
                key: right_geometry[key]
                for key in ("center_x", "ink_x0", "ink_x1")
            } == {"center_x": 35.0, "ink_x0": 34.88, "ink_x1": 35.12}
            and left_geometry["contact_intervals_x"] == [[5.0, 5.12]]
            and right_geometry["contact_intervals_x"] == [[34.88, 35.0]]
            and "emission-source-outer-position-mismatch"
            in frame_offender["failure_kinds"]
            and "layout-source-outer-position-mismatch"
            in frame_offender["failure_kinds"],
        )

    def emitted_cell_markup(cell: Cell) -> str:
        return (
            f'<div id="{cell.id}" class="{cell.classes}"{cell.attrs}>'
            f"{cell.inner}</div>")

    wrong_dom_page_html = (
        '<div class="page page-2">'
        + emitted_cell_markup(valid_three)
        + "</div>"
    )
    wrong_dom_page_fixture = CombEmissionFixture(
        parse_cells(wrong_dom_page_html))
    wrong_dom_page_fixture.form_html = wrong_dom_page_html
    wrong_dom_page = check_comb_slots_match_printed(wrong_dom_page_fixture)
    wrong_dom_page_offender = wrong_dom_page["offenders"][0]
    check(
        "cell id page cannot substitute for the actual enclosing DOM page",
        wrong_dom_page["holds"] is False
        and wrong_dom_page_offender["emission_container_binding"]
        ["emitted_id_page"] == 1
        and wrong_dom_page_offender["emission_container_binding"]
        ["emitted_dom_page"] == 2
        and "emission-container-page-mismatch"
        in wrong_dom_page_offender["failure_kinds"],
    )

    anonymous_slots = "".join(
        f'<div class="s" data-slot="{index}" '
        f'style="left:{index * 10}pt;top:0pt;width:10pt;height:10pt">'
        f'<input data-slot-index="{index}"></div>'
        for index in range(3)
    )
    anonymous_comb = (
        '<div class="c f" data-field-kind="comb" data-comb-slots="3" '
        'style="left:0pt;top:0pt;width:30pt;height:10pt">'
        + anonymous_slots
        + "</div>"
    )
    anonymous_html = (
        '<div class="page page-1">'
        + anonymous_comb
        + emitted_cell_markup(valid_three)
        + "</div>"
    )
    anonymous_fixture = CombEmissionFixture(parse_cells(anonymous_html))
    anonymous_fixture.form_html = anonymous_html
    anonymous_result = check_comb_slots_match_printed(anonymous_fixture)
    anonymous_offenders = [
        item for item in anonymous_result["offenders"]
        if "unowned-live-comb-markup" in item["failure_kinds"]
    ]
    check(
        "raw DOM inventory rejects an anonymous live comb before a valid cell",
        anonymous_result["holds"] is False
        and anonymous_result["raw_live_comb_issues"] == 1
        and anonymous_result["inventory_complete"] is False
        and len(anonymous_offenders) == 1
        and anonymous_offenders[0]["raw_dom_evidence"]["slot_count"] == 3,
    )

    scoped_html = (
        '<div class="page page-1">'
        '<template>' + anonymous_comb + "</template>"
        '<section class="gl-page"><section>'
        + anonymous_comb
        + "</section></section>"
        + emitted_cell_markup(valid_three)
        + "</div>"
    )
    scoped_fixture = CombEmissionFixture(parse_cells(scoped_html))
    scoped_fixture.form_html = scoped_html
    scoped_result = check_comb_slots_match_printed(scoped_fixture)
    check(
        "explicit template and guide combs are excluded from live inventory",
        scoped_result["holds"] is True
        and scoped_result["raw_live_comb_issues"] == 0
        and scoped_result["inventory_complete"] is True,
    )

    live_two_slots = "".join(
        f'<div class="s" data-slot="{index}" '
        f'style="left:{index * 10}pt;top:0pt;width:10pt;height:10pt">'
        f'<input data-slot-index="{index}"></div>'
        for index in range(2)
    )
    inert_third_slot = (
        '<div class="s" data-slot="2" '
        'style="left:20pt;top:0pt;width:10pt;height:10pt">'
        '<input data-slot-index="2"></div>'
    )
    for inert_label, inert_markup in (
            ("comment", f"<!--{inert_third_slot}-->"),
            ("script", f"<script>{inert_third_slot}</script>"),
            ("template", f"<template>{inert_third_slot}</template>")):
        inert_slot_cell = dataclasses.replace(
            valid_three, inner=live_two_slots + inert_markup)
        inert_slot_html = (
            '<div class="page page-1">'
            + emitted_cell_markup(inert_slot_cell)
            + "</div>"
        )
        inert_slot_fixture = CombEmissionFixture(
            parse_cells(inert_slot_html))
        inert_slot_fixture.form_html = inert_slot_html
        inert_slot_result = check_comb_slots_match_printed(
            inert_slot_fixture)
        inert_slot_offender = inert_slot_result["offenders"][0]
        check(
            f"{inert_label} slot markup is not a live physical slot",
            inert_slot_result["holds"] is False
            and inert_slot_offender["physical_slots"] == 2
            and inert_slot_offender["slots"] == 2
            and inert_slot_offender["emission_state"]
            in {"invalid-slot-geometry",
                "declared-physical-slot-mismatch"},
        )

    def inert_input_slots(wrapper: str) -> str:
        slots: list[str] = []
        for index in range(3):
            input_markup = f'<input data-slot-index="{index}">'
            if wrapper == "comment":
                input_markup = f"<!--{input_markup}-->"
            elif wrapper == "script":
                input_markup = f"<script>{input_markup}</script>"
            elif wrapper == "template":
                input_markup = f"<template>{input_markup}</template>"
            slots.append(
                f'<div class="s" data-slot="{index}" '
                f'style="left:{index * 10}pt;top:0pt;'
                f'width:10pt;height:10pt">{input_markup}</div>')
        return "".join(slots)

    for inert_label in ("comment", "script", "template"):
        inert_input_cell = dataclasses.replace(
            valid_three, inner=inert_input_slots(inert_label))
        inert_input_html = (
            '<div class="page page-1">'
            + emitted_cell_markup(inert_input_cell)
            + "</div>"
        )
        inert_input_cells = parse_cells(inert_input_html)
        inert_input_fixture = CombEmissionFixture(inert_input_cells)
        inert_input_fixture.form_html = inert_input_html
        inert_input_comb = check_comb_slots_match_printed(
            inert_input_fixture)
        layout_cell = copy.deepcopy(
            inert_input_fixture.layout_pages[1]["cells"][0])
        inert_input_bundle = Bundle(
            slug=f"inert-{inert_label}",
            ir=ir,
            layout={"pages": [{"index": 1, "cells": [layout_cell]}]},
            plan=None,
            form_html=inert_input_html,
            guide_html=None,
            pdf=None,
        )
        inert_input_money = check_money_boxes_have_inputs(
            inert_input_bundle)
        check(
            f"{inert_label} inputs do not make an editable comb live",
            inert_input_comb["holds"] is False
            and inert_input_comb["offenders"][0]["emission_state"]
            == "slot-input-index-mismatch"
            and inert_input_money["holds"] is False
            and inert_input_money["offenders"][0]["without_input"]
            == [0, 1, 2],
        )

    duplicate_cells = check_comb_slots_match_printed(
        CombEmissionFixture([valid_three, valid_three]))
    duplicate_offender = duplicate_cells["offenders"][0]
    check(
        "duplicate emitted cell ids fail even when the last copy matches",
        duplicate_cells["holds"] is False
        and duplicate_cells["layout_mismatches"] == 0
        and duplicate_cells["duplicate_emitted_cell_ids"] == ["p1c0"]
        and duplicate_offender["emission_state"] == "duplicate-emitted-cell"
        and duplicate_offender["emitted_occurrences"] == 2,
    )

    duplicate_layout_fixture = CombEmissionFixture([valid_three])
    duplicate_layout_cell = copy.deepcopy(
        duplicate_layout_fixture.layout_pages[1]["cells"][0])
    duplicate_layout_fixture.layout_pages[1]["cells"].append(
        duplicate_layout_cell)
    duplicate_layout = check_comb_slots_match_printed(
        duplicate_layout_fixture)
    duplicate_layout_offender = duplicate_layout["offenders"][0]
    check(
        "duplicate layout subjects publish and count an invalid certificate",
        duplicate_layout["holds"] is False
        and duplicate_layout["combs_checked"] == 1
        and duplicate_layout["owner_certificates_valid"] == 0
        and duplicate_layout["owner_certificates_invalid"] == 1
        and (duplicate_layout["owner_certificates_valid"]
             + duplicate_layout["owner_certificates_invalid"])
        == duplicate_layout["combs_checked"]
        and duplicate_layout_offender["source_owner_certificate"]["valid"]
        is False
        and duplicate_layout_offender["source_owner_certificate"]
        ["supplies_topology"] is False,
    )

    malformed_retained_fixture = CombEmissionFixture([valid_three])
    malformed_retained_page = malformed_retained_fixture.layout_pages[1]
    malformed_retained_page["cells"].append({
        "id": "p1c1",
        "subject_key": "p1@40,0,50,10",
        "x0": 40.0, "y0": 0.0, "x1": 50.0, "y1": 10.0,
    })
    malformed_retained_page["comb_subjects"].append({
        "subject_key": "p1@40,0,50,10",
        "legacy_cell_id": "p1c1",
        "legacy_bbox": [40.0, 0.0, 50.0, 10.0],
        "cell_id": None,
        "mapped_partition_cell_ids": ["p1c1"],
        "mapped_partition_subject_keys": ["p1@40,0,50,10"],
        "state": "retained_unresolved",
        # Deliberately corrupt: one malformed retained record must invalidate
        # the otherwise valid p1c0 owner and block its complete U-frame.
        "emission": "emitted",
        "reason_codes": ["emission-suppressed-no-final-visible-band"],
        "legacy_comb": {},
        "requires_independent_evidence": True,
        "permitted_transitions": [
            "active_composite", "retired_proven_false"],
        "blocks_gate": True,
    })
    malformed_retained_fixture._snapshot_layout()
    malformed_retained_assertion = check_comb_slots_match_printed(
        malformed_retained_fixture)
    malformed_retained_registry_offender = next(
        item for item in malformed_retained_assertion["offenders"]
        if item["cell"] == "<comb-owner-registry>")
    malformed_retained_offender = next(
        item for item in malformed_retained_assertion["offenders"]
        if item["cell"] == "p1c0")
    check(
        "global retained-ledger corruption makes every active comb offending",
        malformed_retained_assertion["holds"] is False
        and malformed_retained_assertion["combs_checked"] == 1
        and malformed_retained_assertion["owner_certificates_valid"] == 0
        and malformed_retained_assertion["owner_certificates_invalid"] == 1
        and malformed_retained_assertion["layout_unevaluable"] == 1
        and malformed_retained_assertion["source_u_frame_evaluable"] == 0
        and malformed_retained_assertion["offender_count"] == 2
        and malformed_retained_assertion["offenders_complete"] is True
        and malformed_retained_registry_offender["failure_kinds"]
        == ["comb-owner-registry-invalid"]
        and malformed_retained_offender["layout_relation"] == "unevaluable"
        and malformed_retained_offender["failure_kinds"]
        == ["source-topology-unevaluable"]
        and malformed_retained_offender["source_owner_certificate"]["valid"]
        is False
        and "invalid reviewed source owner certificate"
        in malformed_retained_offender["why"],
    )

    undeclared_slots_cell = dataclasses.replace(
        valid_three,
        attrs=re.sub(
            r'\s*data-comb-slots="\d+"', "", valid_three.attrs),
    )
    undeclared_slots = check_comb_slots_match_printed(
        CombEmissionFixture([undeclared_slots_cell]))
    check(
        "physical slots without an emitted count declaration fail closed",
        undeclared_slots["holds"] is False
        and undeclared_slots["layout_mismatches"] == 0
        and undeclared_slots["offenders"][0]["emission_state"]
        == "missing-declared-slot-count",
    )

    duplicate_slot_identity = check_comb_slots_match_printed(
        CombEmissionFixture(
            [emitted_comb_cell(
                slot_indexes=(0, 0), input_indexes=(0, 0), declared=2)],
            count=2,
        ))
    duplicate_slot_offender = duplicate_slot_identity["offenders"][0]
    check(
        "equal slot counts cannot hide duplicate div and input identities",
        duplicate_slot_identity["holds"] is False
        and duplicate_slot_identity["layout_mismatches"] == 0
        and duplicate_slot_offender["printed"] == 2
        and duplicate_slot_offender["slots"] == 2
        and duplicate_slot_offender["emission_state"] == "duplicate-slot-index",
    )

    unordered_slots = check_comb_slots_match_printed(
        CombEmissionFixture(
            [emitted_comb_cell(
                slot_indexes=(1, 0), input_indexes=(1, 0), declared=2)],
            count=2,
        ))
    check(
        "physical slot indexes must be ordered exactly zero through N minus one",
        unordered_slots["holds"] is False
        and unordered_slots["offenders"][0]["emission_state"]
        == "invalid-slot-index-sequence",
    )

    wrong_owner = check_comb_slots_match_printed(
        CombEmissionFixture(
            [emitted_comb_cell(
                slot_indexes=(0, 1), input_indexes=(0, 0), declared=2)],
            count=2,
        ))
    check(
        "each emitted input index must identify its owning physical slot",
        wrong_owner["holds"] is False
        and wrong_owner["offenders"][0]["emission_state"]
        == "slot-input-index-mismatch",
    )

    zero_width_slot = check_comb_slots_match_printed(
        CombEmissionFixture(
            [emitted_comb_cell(
                slot_indexes=(0, 1),
                geometry=((0.0, 0.0, 0.0, 10.0),
                          (0.0, 0.0, 10.0, 10.0)))],
            count=2,
        ))
    check(
        "a physical slot count cannot hide a zero-width slot",
        zero_width_slot["holds"] is False
        and zero_width_slot["layout_mismatches"] == 0
        and zero_width_slot["offenders"][0]["emission_state"]
        == "invalid-slot-geometry",
    )

    overlapping_slots = check_comb_slots_match_printed(
        CombEmissionFixture(
            [emitted_comb_cell(
                slot_indexes=(0, 1),
                geometry=((0.0, 0.0, 10.0, 10.0),
                          (0.0, 0.0, 10.0, 10.0)))],
            count=2,
        ))
    check(
        "distinct indexes cannot occupy the same physical slot box",
        overlapping_slots["holds"] is False
        and overlapping_slots["layout_mismatches"] == 0
        and overlapping_slots["offenders"][0]["emission_state"]
        == "invalid-slot-geometry",
    )

    unexpected = check_comb_slots_match_printed(
        CombEmissionFixture([
            valid_three,
            emitted_comb_cell("p1c9"),
        ]))
    unexpected_offender = next(
        item for item in unexpected["offenders"] if item["cell"] == "p1c9")
    check(
        "comb-marked emitted cells require a non-relocated layout owner",
        unexpected["holds"] is False
        and unexpected["expected_comb_ids"] == ["p1c0"]
        and unexpected["checked_comb_ids"] == ["p1c0"]
        and unexpected["emitted_comb_ids"] == ["p1c0", "p1c9"]
        and unexpected["unexpected_emitted_comb_ids"] == ["p1c9"]
        and unexpected_offender["layout_relation"] == "not-owned"
        and unexpected_offender["failure_kinds"]
        == ["unexpected-emitted-comb"],
    )

    class EmptyCombInventoryFixture:
        doc = object()
        relocated_cells: set[str] = set()
        vector_pages: dict[int, VectorPage] = {}

        def __init__(self, cells: Sequence[Cell], *,
                     stats_comb_cells: int | None = None) -> None:
            self.cells = list(cells)
            page: dict[str, Any] = {
                "index": 1,
                "cells": [],
                "comb_subjects": [],
            }
            if stats_comb_cells is not None:
                page["stats"] = {"comb_cells": stats_comb_cells}
            self.layout_pages = {1: page}
            self.layout = {"pages": [page]}
            self.layout_payload = json.dumps(
                self.layout, sort_keys=True, separators=(",", ":"),
            ).encode("utf-8")
            self.layout_sha256 = hashlib.sha256(
                self.layout_payload).hexdigest()

    valid_pure_empty = check_comb_slots_match_printed(
        EmptyCombInventoryFixture([]))
    check(
        "a valid hash-bound pure-empty comb inventory remains held",
        valid_pure_empty["holds"] is True
        and valid_pure_empty["inventory_complete"] is True
        and valid_pure_empty["combs_expected"] == 0
        and valid_pure_empty["combs_checked"] == 0
        and valid_pure_empty["owner_certificates_valid"] == 0
        and valid_pure_empty["owner_certificates_invalid"] == 0
        and valid_pure_empty["offenders"] == [],
    )

    emission_only_inventory = check_comb_slots_match_printed(
        EmptyCombInventoryFixture([emitted_comb_cell()]))
    check(
        "an empty layout inventory cannot vacuously own an emitted comb",
        emission_only_inventory["holds"] is False
        and emission_only_inventory["expected_comb_ids"] == []
        and emission_only_inventory["checked_comb_ids"] == []
        and emission_only_inventory["emitted_comb_ids"] == ["p1c0"]
        and emission_only_inventory["offenders"][0]["layout_relation"]
        == "not-owned",
    )
    stats_only_inventory = check_comb_slots_match_printed(
        EmptyCombInventoryFixture([], stats_comb_cells=1))
    check(
        "positive layout statistics make an empty comb inventory fail closed",
        stats_only_inventory["holds"] is False
        and stats_only_inventory["offender_count"] == 1
        and stats_only_inventory["offenders"][0]["cell"]
        == "<comb-inventory>"
        and stats_only_inventory["inventory_complete"] is False,
    )

    def bound_comb_inventory_fixture(
            layout: dict[str, Any],
            *,
            cells: Sequence[Cell] = (),
            relocated_cells: Iterable[str] = (),
            ) -> Any:
        payload = json.dumps(
            layout, sort_keys=True, separators=(",", ":"),
        ).encode("utf-8")
        return types.SimpleNamespace(
            layout=layout,
            layout_payload=payload,
            layout_sha256=hashlib.sha256(payload).hexdigest(),
            layout_pages={page["index"]: page for page in layout["pages"]},
            doc=object(),
            cells=list(cells),
            relocated_cells=set(relocated_cells),
            vector_pages={},
        )

    def retained_subject(*, corrupt_emission: bool) -> dict[str, Any]:
        return {
            "subject_key": "p1@40,0,50,10",
            "legacy_cell_id": "p1c1",
            "legacy_bbox": [40.0, 0.0, 50.0, 10.0],
            "cell_id": None,
            "mapped_partition_cell_ids": ["p1c1"],
            "mapped_partition_subject_keys": ["p1@40,0,50,10"],
            "state": "retained_unresolved",
            "emission": "emitted" if corrupt_emission else "suppressed",
            "reason_codes": [
                "emission-suppressed-no-final-visible-band"],
            "legacy_comb": {},
            "requires_independent_evidence": True,
            "permitted_transitions": [
                "active_composite", "retired_proven_false"],
            "blocks_gate": True,
        }

    retained_cell = {
        "id": "p1c1",
        "subject_key": "p1@40,0,50,10",
        "x0": 40.0, "y0": 0.0, "x1": 50.0, "y1": 10.0,
    }
    corrupt_pure_empty_layout = {"pages": [{
        "index": 1,
        "cells": [copy.deepcopy(retained_cell)],
        "comb_subjects": [retained_subject(corrupt_emission=True)],
    }]}
    corrupt_pure_empty = check_comb_slots_match_printed(
        bound_comb_inventory_fixture(corrupt_pure_empty_layout))
    corrupt_registry_offender = corrupt_pure_empty["offenders"][0]
    check(
        "corrupt retained-only inventory fails with complete registry evidence",
        corrupt_pure_empty["holds"] is False
        and corrupt_pure_empty["inventory_complete"] is False
        and corrupt_pure_empty["combs_expected"] == 0
        and corrupt_pure_empty["combs_checked"] == 0
        and corrupt_pure_empty["owner_certificates_valid"] == 0
        and corrupt_pure_empty["owner_certificates_invalid"] == 0
        and corrupt_pure_empty["offender_count"] == 1
        and corrupt_pure_empty["offenders_complete"] is True
        and corrupt_registry_offender["cell"] == "<comb-owner-registry>"
        and corrupt_registry_offender["failure_kinds"]
        == ["comb-owner-registry-invalid"]
        and corrupt_registry_offender["source_owner_certificate"]["valid"]
        is False
        and "suppression/blocking/transition"
        in corrupt_registry_offender["source_owner_certificate"]["reason"],
    )

    active_relocated_cell = {
            "id": "p1c0", "x0": 0.0, "y0": 0.0,
            "x1": 40.0, "y1": 10.0,
            "subject_key": "p1@0,0,40,10",
            "comb": {"cells": 3},
    }
    active_relocated_subject = {
        "subject_key": "p1@0,0,40,10",
        "legacy_cell_id": "p1c0",
        "legacy_bbox": [0.0, 0.0, 40.0, 10.0],
        "cell_id": "p1c0",
        "mapped_partition_cell_ids": ["p1c0"],
        "state": "active_unresolved",
        "reason_codes": ["competing-endpoint-topologies"],
        "cells": 3,
        "blocks_gate": True,
    }
    valid_relocated_layout = {"pages": [{
        "index": 1,
        "cells": [copy.deepcopy(active_relocated_cell)],
        "comb_subjects": [copy.deepcopy(active_relocated_subject)],
    }]}

    relocated_live = check_comb_slots_match_printed(
        bound_comb_inventory_fixture(
            valid_relocated_layout,
            cells=[valid_three],
            relocated_cells={"p1c0"},
        ))
    check(
        "a relocated comb left live in form HTML fails as stale markup",
        relocated_live["holds"] is False
        and relocated_live["expected_comb_ids"] == []
        and relocated_live["unexpected_emitted_comb_ids"] == ["p1c0"]
        and relocated_live["offenders"][0]["failure_kinds"]
        == ["unexpected-emitted-comb"],
    )

    corrupt_relocated_layout = copy.deepcopy(valid_relocated_layout)
    corrupt_relocated_page = corrupt_relocated_layout["pages"][0]
    corrupt_relocated_page["cells"].append(copy.deepcopy(retained_cell))
    corrupt_relocated_page["comb_subjects"].append(
        retained_subject(corrupt_emission=True))
    corrupt_relocated = check_comb_slots_match_printed(
        bound_comb_inventory_fixture(
            corrupt_relocated_layout,
            relocated_cells={"p1c0"},
        ))
    check(
        "relocated active comb cannot hide a corrupt retained ledger tail",
        corrupt_relocated["holds"] is False
        and corrupt_relocated["expected_comb_ids"] == []
        and corrupt_relocated["checked_comb_ids"] == []
        and corrupt_relocated["owner_certificates_valid"] == 0
        and corrupt_relocated["owner_certificates_invalid"] == 0
        and corrupt_relocated["inventory_complete"] is False
        and corrupt_relocated["offender_count"] == 1
        and corrupt_relocated["offenders"][0]["cell"]
        == "<comb-owner-registry>"
        and corrupt_relocated["offenders"][0]["failure_kinds"]
        == ["comb-owner-registry-invalid"],
    )

    # Geometry helpers, where an off-by-one epsilon would silently disable an
    # assertion rather than break it.
    check("touching edges do not overlap", not overlaps((0, 0, 10, 10), (10, 0, 20, 10)))
    check("shared area overlaps", overlaps((0, 0, 10, 10), (9, 0, 20, 10)))
    check("whitespace carries no ink",
          len(glyph_boxes({"text": "a b", "x0": 0.0, "y0": 0.0, "x1": 9.0, "y1": 8.0,
                           "origin_x": 0.0,
                           "char_origin_offsets_pt": [0.0, 3.0, 6.0],
                           "char_widths_pt": [3.0, 3.0, 3.0]})) == 2)
    check("an upright placement needs no SVG transform",
          transform_signature((10.0, 0.0, 0.0, 10.0, 0.0, 0.0)) == svg_signature(None))
    check("a y-flipped placement needs a negative y scale",
          transform_signature((10.0, 0.0, -0.0, -10.0, 0.0, 0.0))
          == svg_signature("translate(5,5) scale(1,-1)"))
    check("arithmetic noise is not a shear",
          transform_signature((41.0, 5.3e-05, 1.7e-06, 33.8, 0.0, 0.0))
          == (1, 1, False))
    check(
        "topology subset matching cannot reuse one superset divider twice",
        not _topology_subset((10.0, 10.1), (10.0, 20.0, 30.0)),
    )
    check(
        "topology subset matching accepts near-identical one-to-one dividers",
        _topology_subset((10.1, 20.1), (10.0, 20.0, 30.0)),
    )

    def source_paint(x: float, a: float = 2.0, b: float = 8.0,
                     *, width: float = 0.24, tone: float = 0.0,
                     order: int = 0) -> VectorPaint:
        return VectorPaint(x - width / 2, a, x + width / 2, b,
                           tone, 1.0, order, "test")

    def source_page(
            *paints: VectorPaint, framed: bool = True,
            ) -> VectorPage:
        paint_list = list(paints)
        if framed:
            vertical_paints = [
                paint for paint in paint_list if _is_comb_vertical(paint)]
            centers = sorted({
                round((paint.x0 + paint.x1) / 2.0, 6)
                for paint in vertical_paints
            })
            pitches = [
                right - left for left, right in zip(centers, centers[1:])
                if right - left > COMB_MERGE_PT
            ]
            pitch = statistics.median(pitches) if pitches else 10.0
            right_rail = max(
                40.0,
                (centers[-1] + 2.0 * pitch) if centers else 40.0,
            )
            tone_counts = collections.Counter(
                round(paint.tone, 6) for paint in vertical_paints)
            frame_tone = (
                min(
                    tone for tone, count in tone_counts.items()
                    if count == max(tone_counts.values()))
                if tone_counts else 0.0
            )
            frame_verticals = [
                paint for paint in vertical_paints
                if abs(paint.tone - frame_tone) <= SOURCE_COORD_EPS_PT
            ]
            longest = max(
                frame_verticals,
                key=lambda paint: paint.y1 - paint.y0,
                default=None,
            )
            frame_y0 = longest.y0 if longest is not None else 2.0
            frame_y1 = longest.y1 if longest is not None else 8.0
            last_order = max((paint.order for paint in paint_list), default=0)
            paint_list.extend((
                source_paint(
                    0.0, a=frame_y0, b=frame_y1,
                    order=last_order + 1, tone=frame_tone),
                source_paint(
                    right_rail, a=frame_y0, b=frame_y1,
                    order=last_order + 2, tone=frame_tone),
                VectorPaint(
                    0.0, frame_y1 - 0.12, right_rail, frame_y1 + 0.12,
                    frame_tone, 1.0, last_order + 3, "test-source-frame"),
            ))
        ordered = sorted(
            paint_list,
            key=lambda paint: (
                paint.order, paint.kind, paint.x0, paint.y0,
                paint.x1, paint.y1),
        )
        return VectorPage(tuple(
            dataclasses.replace(paint, operation=index)
            if paint.operation < 0 else paint
            for index, paint in enumerate(ordered)
        ), ())

    def owned_test_page(page: VectorPage) -> VectorPage:
        framed = source_page(*page.paints)
        return VectorPage(framed.paints, page.unsupported)

    class PoisonComb:
        def __getitem__(self, key: str) -> Any:
            raise AssertionError(f"source oracle read poisoned comb key {key!r}")

        def get(self, key: str, default: Any = None) -> Any:
            raise AssertionError(f"source oracle read poisoned comb key {key!r}")

    def comb_subject(*, x0: float = 0.0, x1: float = 40.0,
                     cell_y0: float = 0.0, cell_y1: float = 10.0,
                     ) -> dict[str, Any]:
        return {
            "id": "p1c0", "x0": x0, "y0": cell_y0,
            "x1": x1, "y1": cell_y1,
            "subject_key": (
                f"p1@{x0:.2f},{cell_y0:.2f},{x1:.2f},{cell_y1:.2f}"),
            # Every field -- cells, divider_x, y0, y1 and divider_gray -- is
            # poison. The source oracle must never inspect this mapping.
            "comb": PoisonComb(),
        }

    def owner_registry_fixture(
            cell: dict[str, Any] | None = None,
            *,
            subject_updates: dict[str, Any] | None = None,
            missing: bool = False,
            duplicate: bool = False,
            stale_parsed_layout: bool = False,
            stale_digest: bool = False,
            layout_mutator: Any = None,
            ) -> tuple[
                CombOwnerRegistry,
                dict[str, Any],
                CombOwnerCertificate | None,
                str | None,
            ]:
        source_cell = cell or comb_subject()
        identity_cell = {
            "id": source_cell["id"],
            "subject_key": source_cell["subject_key"],
            **{key: source_cell[key] for key in ("x0", "y0", "x1", "y1")},
            # Deliberately false topology: a valid identity certificate must
            # still let the source's two dividers, not this value, decide.
            "comb": {
                "cells": 999,
                "divider_x": [-999.0],
                "slot_x": [-1000.0, 1000.0],
                "y0": -999.0,
                "y1": 999.0,
                "divider_gray": 1.0,
            },
        }
        subject = {
            "subject_key": identity_cell["subject_key"],
            "legacy_cell_id": identity_cell["id"],
            "legacy_bbox": [
                identity_cell[key] for key in ("x0", "y0", "x1", "y1")
            ],
            "cell_id": identity_cell["id"],
            "mapped_partition_cell_ids": [identity_cell["id"]],
            "state": "active_unresolved",
            "reason_codes": ["competing-endpoint-topologies"],
            "cells": 999,
            "blocks_gate": True,
        }
        if subject_updates:
            subject.update(copy.deepcopy(subject_updates))
        subjects = [] if missing else [subject]
        if duplicate:
            subjects.append(copy.deepcopy(subject))
        layout_fixture = {
            "pages": [{
                "index": 1,
                "cells": [identity_cell],
                "comb_subjects": subjects,
            }],
        }
        if layout_mutator is not None:
            layout_mutator(layout_fixture)
        payload = json.dumps(
            layout_fixture, sort_keys=True, separators=(",", ":"),
        ).encode("utf-8")
        parsed_layout = copy.deepcopy(layout_fixture)
        if stale_parsed_layout:
            parsed_layout["pages"][0]["cells"][0]["x1"] += 1.0
        digest = (
            "0" * 64 if stale_digest else hashlib.sha256(payload).hexdigest()
        )
        registry = reviewed_comb_owner_registry(types.SimpleNamespace(
            layout=parsed_layout,
            layout_payload=payload,
            layout_sha256=digest,
        ))
        certificate, reason = registry.resolve(1, identity_cell)
        return registry, identity_cell, certificate, reason

    valid_registry, valid_owner_cell, valid_owner, valid_owner_reason = (
        owner_registry_fixture())
    check(
        "one exact reviewed hash-bound comb_subject certifies owner identity",
        valid_owner is not None
        and valid_owner_reason is None
        and valid_registry.binding_error is None,
    )
    if valid_owner is not None:
        certificate_evidence = valid_owner.evidence()
        certified_unframed = printed_compartments(
            source_page(
                source_paint(10), source_paint(20), framed=False),
            valid_owner_cell,
            include_frame=True,
            owner_certificate=valid_owner,
        )
        check(
            "reviewed ownership admits only the unanimous source topology",
            certified_unframed == (3, [10.0, 20.0], None),
        )
        check(
            "ownership certificate publishes deterministic identity-only evidence",
            certificate_evidence == valid_owner.evidence()
            and json.dumps(certificate_evidence, sort_keys=True)
            == json.dumps(valid_owner.evidence(), sort_keys=True)
            and certificate_evidence["supplies_topology"] is False
            and not ({"cells", "comb", "divider_x", "slot_x", "y0", "y1",
                      "divider_gray"} & set(certificate_evidence)),
        )

    missing_registry = owner_registry_fixture(missing=True)
    duplicate_registry = owner_registry_fixture(duplicate=True)
    bbox_registry = owner_registry_fixture(subject_updates={
        "legacy_bbox": [0.0, 0.0, 41.0, 10.0],
    })
    malformed_retained_state_registry = owner_registry_fixture(subject_updates={
        "state": "retained_unresolved",
        "cell_id": None,
    })
    stale_layout_registry = owner_registry_fixture(stale_parsed_layout=True)
    stale_digest_registry = owner_registry_fixture(stale_digest=True)
    for label, fixture, phrase in (
            ("missing", missing_registry, "no reviewed active"),
            ("duplicate", duplicate_registry, "duplicate comb_subject"),
            ("mismatched bbox", bbox_registry, "exact bbox"),
            ("mismatched state", malformed_retained_state_registry,
             "schema is malformed"),
            ("stale parsed layout", stale_layout_registry, "stale"),
            ("stale layout digest", stale_digest_registry, "SHA-256")):
        _registry, _cell, certificate, reason = fixture
        check(
            f"{label} comb_subject ownership certificate fails closed",
            certificate is None
            and reason is not None
            and phrase in reason,
        )

    def append_orphan_active(layout_fixture: dict[str, Any]) -> None:
        layout_fixture["pages"][0]["comb_subjects"].append({
            "subject_key": "p1@50.00,0.00,60.00,10.00",
            "legacy_cell_id": "p1c9",
            "legacy_bbox": [50.0, 0.0, 60.0, 10.0],
            "cell_id": "p1c9",
            "mapped_partition_cell_ids": ["p1c9"],
            "state": "active_unresolved",
            "reason_codes": ["competing-endpoint-topologies"],
            "blocks_gate": True,
        })

    def append_competing_active(layout_fixture: dict[str, Any]) -> None:
        layout_fixture["pages"][0]["comb_subjects"].append({
            "subject_key": "p1@1.00,0.00,39.00,10.00",
            "legacy_cell_id": "p1c9",
            "legacy_bbox": [1.0, 0.0, 39.0, 10.0],
            "cell_id": "p1c0",
            "mapped_partition_cell_ids": ["p1c0"],
            "state": "active_resolved",
        })

    orphan_active_registry = owner_registry_fixture(
        layout_mutator=append_orphan_active)
    competing_active_registry = owner_registry_fixture(
        layout_mutator=append_competing_active)
    unknown_state_registry = owner_registry_fixture(subject_updates={
        "state": "active_future",
    })
    bool_page_registry = owner_registry_fixture(
        layout_mutator=lambda value: value["pages"][0].__setitem__(
            "index", True))
    bool_coordinate_registry = owner_registry_fixture(
        layout_mutator=lambda value: value["pages"][0]["cells"][0].__setitem__(
            "x0", True))
    for label, fixture, phrase in (
            ("orphan active", orphan_active_registry, "orphaned"),
            ("competing active bbox/subject", competing_active_registry,
             "mapping is not unique"),
            ("unknown active state", unknown_state_registry, "unknown state"),
            ("boolean page", bool_page_registry, "exhaustive and ordered"),
            ("boolean coordinate", bool_coordinate_registry, "four-number")):
        _registry, _cell, certificate, reason = fixture
        check(
            f"{label} invalidates the exhaustive ownership registry",
            certificate is None and reason is not None and phrase in reason,
        )
    def append_valid_retained(layout_fixture: dict[str, Any]) -> None:
        page = layout_fixture["pages"][0]
        retained_cell = {
            "id": "p1c1",
            "subject_key": "p1@50.00,0.00,60.00,10.00",
            "x0": 50.0, "y0": 0.0, "x1": 60.0, "y1": 10.0,
        }
        page["cells"].append(retained_cell)
        page["comb_subjects"].append({
            "subject_key": retained_cell["subject_key"],
            "legacy_cell_id": retained_cell["id"],
            "legacy_bbox": [50.0, 0.0, 60.0, 10.0],
            "cell_id": None,
            "mapped_partition_cell_ids": [retained_cell["id"]],
            "mapped_partition_subject_keys": [retained_cell["subject_key"]],
            "state": "retained_unresolved",
            "emission": "suppressed",
            "reason_codes": [
                "emission-suppressed-no-final-visible-band"],
            # Presence is schema evidence only; the ownership registry never
            # reads retained topology.
            "legacy_comb": {},
            "requires_independent_evidence": True,
            "permitted_transitions": [
                "active_composite", "retired_proven_false"],
            "blocks_gate": True,
        })

    retained_registry_fixture = owner_registry_fixture(
        layout_mutator=append_valid_retained)
    retained_registry = retained_registry_fixture[0]
    retained_cell = {
        "id": "p1c1",
        "subject_key": "p1@50.00,0.00,60.00,10.00",
        "x0": 50.0, "y0": 0.0, "x1": 60.0, "y1": 10.0,
    }
    retained_certificate, retained_reason = retained_registry.resolve(
        1, retained_cell)
    check(
        "valid retained_unresolved evidence does not invalidate active owners",
        retained_registry_fixture[2] is not None
        and retained_registry.binding_error is None,
    )
    check(
        "retained_unresolved subject is allowed but cannot certify a cell",
        retained_certificate is None
        and retained_reason is not None
        and "no exact unique reviewed" in retained_reason,
    )

    def corrupt_retained(
            field: str, value: Any,
            ) -> Any:
        def mutate(layout_fixture: dict[str, Any]) -> None:
            append_valid_retained(layout_fixture)
            layout_fixture["pages"][0]["comb_subjects"][-1][field] = value
        return mutate

    retained_corruptions = (
        (
            "reverse partition mapping",
            owner_registry_fixture(layout_mutator=corrupt_retained(
                "mapped_partition_subject_keys",
                ["p1@0.00,0.00,40.00,10.00"])),
            "reverse subject_key mapping",
        ),
        (
            "suppression emission",
            owner_registry_fixture(layout_mutator=corrupt_retained(
                "emission", "emitted")),
            "suppression/blocking/transition",
        ),
        (
            "blocking evidence",
            owner_registry_fixture(layout_mutator=corrupt_retained(
                "blocks_gate", False)),
            "suppression/blocking/transition",
        ),
        (
            "permitted transition evidence",
            owner_registry_fixture(layout_mutator=corrupt_retained(
                "permitted_transitions",
                ["retired_proven_false", "active_composite"])),
            "suppression/blocking/transition",
        ),
    )
    for label, fixture, phrase in retained_corruptions:
        check(
            f"malformed retained {label} invalidates every active certificate",
            fixture[2] is None
            and fixture[3] is not None
            and phrase in fixture[3],
        )

    def append_noncontiguous_page(layout_fixture: dict[str, Any]) -> None:
        append_valid_retained(layout_fixture)
        layout_fixture["pages"].append({
            "index": 3, "cells": [], "comb_subjects": []})

    noncontiguous_registry = owner_registry_fixture(
        layout_mutator=append_noncontiguous_page)
    check(
        "noncontiguous retained layout pages invalidate active certificates",
        noncontiguous_registry[2] is None
        and noncontiguous_registry[3] is not None
        and "exhaustive and ordered" in noncontiguous_registry[3],
    )
    check(
        "exact ownership number equality rejects JSON booleans",
        not _exact_number_vector([True, 0.0], [1.0, 0.0])
        and not _exact_number_vector([1.0, 0.0], [True, 0.0]),
    )
    check(
        "exact ownership numbers do not collapse integers above 2^53",
        not _exact_number_vector(
            [9_007_199_254_740_993], [9_007_199_254_740_992]),
    )
    check(
        "exact ownership numbers preserve distinct decimal identities",
        not _exact_number_vector(
            [Decimal("0.1")], [Decimal("0.10000000000000001")])
        and not _exact_json_equal(
            [Decimal("0.10000000000000001")], [0.1]),
    )
    high_precision_certificate = CombOwnerCertificate(
        page=1,
        cell_id="p1c0",
        legacy_cell_id="p1c0",
        subject_key="p1@0.10000000000000001,0,1,1",
        bbox=(
            Decimal("0.10000000000000001"), Decimal("0"),
            Decimal("1"), Decimal("1"),
        ),
        state="active_unresolved",
        layout_sha256="0" * 64,
    )
    check(
        "certificate matching rejects a float-rounded decimal bbox",
        not high_precision_certificate.matches(1, {
            "id": "p1c0",
            "subject_key": "p1@0.10000000000000001,0,1,1",
            "x0": 0.1, "y0": 0, "x1": 1, "y1": 1,
        })
        and high_precision_certificate.evidence()["legacy_bbox"][0]
        == "0.10000000000000001",
    )

    if valid_owner is not None:
        try:
            printed_compartments(
                source_page(
                    source_paint(10), source_paint(20), framed=False),
                comb_subject(x0=5.0, x1=35.0),
                owner_certificate=valid_owner,
            )
        except ValueError as exc:
            arbitrary_owner_failed = "does not bind this exact cell" in str(exc)
        else:
            arbitrary_owner_failed = False
        check(
            "a reviewed certificate cannot be reused for an arbitrary bbox",
            arbitrary_owner_failed,
        )

    basic = printed_compartments(
        source_page(source_paint(10), source_paint(20)),
        comb_subject(),
    )
    check(f"two final black dividers make three compartments (got {basic})",
          basic == (3, [10.0, 20.0]))

    # White decorative rectangles in a white slot have no final tone boundary.
    # This is the 2200S p1c141 failure mechanism: seven such rectangles used to
    # be interleaved with its six real black dividers and double the count.
    decorative = printed_compartments(
        source_page(
            source_paint(10), source_paint(20),
            source_paint(15, width=2.4, tone=1.0),
        ),
        comb_subject(),
    )
    check(f"same-tone white decoration is not a divider (got {decorative})",
          decorative == (3, [10.0, 20.0]))

    # Content-stream order decides the final paper. A later white cell fill
    # erases an earlier black stub (0605); reversing the order repaints it.
    erased = printed_compartments(
        source_page(
            source_paint(10, order=0), source_paint(20, order=1),
            VectorPaint(9.0, 2.0, 11.0, 8.0, 1.0, 1.0, 2, "white-fill"),
        ),
        comb_subject(),
    )
    repainted = printed_compartments(
        source_page(
            VectorPaint(9.0, 2.0, 11.0, 8.0, 1.0, 1.0, 0, "white-fill"),
            source_paint(10, order=1), source_paint(20, order=2),
        ),
        comb_subject(),
    )
    check(f"later white overpaint erases the black divider (got {erased})",
          erased == (2, [20.0]))
    check(f"later black repaint restores the divider (got {repainted})",
          repainted == (3, [10.0, 20.0]))

    # A broad same-tone overpaint leaves black pixels, but no narrow vertical
    # boundary. It must not preserve the buried candidate merely by colour.
    broad_black = printed_compartments(
        source_page(
            source_paint(10, order=0), source_paint(20, order=1),
            VectorPaint(7.0, 2.0, 13.0, 8.0, 0.0, 1.0, 2, "broad-black"),
        ),
        comb_subject(),
    )
    check(f"broad same-tone paint removes narrow topology (got {broad_black})",
          broad_black == (2, [20.0]))

    # Never union unlike paints through y. The 2200A/C/P false positives are a
    # short black cap followed by a long white stem at the same x.
    cap_and_stem = printed_compartments(
        source_page(
            source_paint(10, a=2.0, b=2.48, order=0),
            source_paint(10, a=2.48, b=8.0, tone=1.0, order=1),
            source_paint(20, order=2),
        ),
        comb_subject(),
    )
    check(f"black cap and white stem do not stitch (got {cap_and_stem})",
          cap_and_stem == (2, [20.0]))

    try:
        printed_compartments(
            source_page(
                source_paint(10, tone=0.5), source_paint(20),
                framed=False),
            comb_subject(),
        )
    except ValueError as exc:
        grey_ambiguous = "band/tone choices disagree" in str(exc)
    else:
        grey_ambiguous = False
    check("competing source-derived grey and black tones fail closed",
          grey_ambiguous)

    # The source-painted band can cross the owning cell edge. 2550M's real
    # dividers begin at the cell's bottom edge and continue below it.
    outside_cell = printed_compartments(
        source_page(source_paint(10), source_paint(20)),
        comb_subject(cell_y0=0.0, cell_y1=2.0),
    )
    check(f"source paint crossing the cell edge owns dividers (got {outside_cell})",
          outside_cell == (3, [10.0, 20.0]))

    fragmented = printed_compartments(
        source_page(
            source_paint(10.0, a=2.0, b=5.0),
            source_paint(10.6, a=5.0, b=8.0),
            source_paint(20.0),
        ),
        comb_subject(),
    )
    check(f"two pieces within the existing merge bound count once (got {fragmented})",
          fragmented[0] == 3 and len(fragmented[1]) == 2)

    ambiguous_page = source_page(
        source_paint(10, a=2.0, b=5.0),
        source_paint(20, a=5.0, b=8.0),
        framed=False,
    )
    try:
        printed_compartments(ambiguous_page, comb_subject())
    except ValueError as exc:
        ambiguous_failed = "band/tone choices disagree" in str(exc)
    else:
        ambiguous_failed = False
    check("equal competing final-paint topologies fail closed", ambiguous_failed)
    if valid_owner is not None:
        try:
            printed_compartments(
                ambiguous_page,
                valid_owner_cell,
                owner_certificate=valid_owner,
            )
        except CombTopologyError as exc:
            certified_competition_failed = (
                exc.evidence.get("criterion")
                == "unanimous-source-derived-topology-required"
                and exc.evidence.get("unframed_compartment_counts") == [2]
                and exc.evidence.get("owner_certificate")
                == valid_owner.evidence()
            )
        else:
            certified_competition_failed = False
        check(
            "reviewed ownership never chooses between competing source topology",
            certified_competition_failed,
        )

        unsupported_owner_page = source_page(
            source_paint(10), source_paint(20), framed=False)
        unsupported_owner_page = VectorPage(
            unsupported_owner_page.paints,
            (UnsupportedVectorPaint(
                (9.0, 2.0, 11.0, 8.0),
                99,
                "unsupported test source paint",
            ),),
        )
        try:
            printed_compartments(
                unsupported_owner_page,
                valid_owner_cell,
                owner_certificate=valid_owner,
            )
        except ValueError as exc:
            certified_unsupported_failed = (
                "unsupported test source paint" in str(exc))
        else:
            certified_unsupported_failed = False
        check(
            "reviewed ownership cannot bypass unsupported source paint",
            certified_unsupported_failed,
        )

        for framed_image_owner in (False, True):
            for image_order in (-100, 100):
                between_divider_image_page = source_page(
                    source_paint(10), source_paint(20),
                    framed=framed_image_owner)
                between_divider_image_page = VectorPage(
                    between_divider_image_page.paints,
                    (UnsupportedVectorPaint(
                        (14.0, 2.0, 16.0, 8.0),
                        image_order,
                        "unmodeled source fill-image paint",
                    ),),
                )
                try:
                    printed_compartments(
                        between_divider_image_page,
                        valid_owner_cell,
                        owner_certificate=valid_owner,
                    )
                except CombTopologyError as exc:
                    between_divider_image_failed = (
                        exc.evidence.get("criterion")
                        == "source-comb-band-image-free-required"
                        and exc.evidence.get("image_paint") == [{
                            "order": image_order,
                            "rect": [14.0, 2.0, 16.0, 8.0],
                        }]
                    )
                else:
                    between_divider_image_failed = False
                check(
                    "between-divider fill-image blocks source topology for "
                    f"framed={framed_image_owner} regardless of source order "
                    f"{image_order}",
                    between_divider_image_failed,
                )

    unframed_expansion_page = source_page(
        source_paint(10), source_paint(20), source_paint(45),
        framed=False,
    )
    for left, right in ((0.0, 30.0), (0.0, 50.0), (5.0, 35.0)):
        try:
            printed_compartments(
                unframed_expansion_page,
                comb_subject(x0=left, x1=right),
            )
        except CombTopologyError as exc:
            unframed_owner_failed = (
                exc.evidence.get("criterion")
                == "independent-complete-source-u-frame-required"
            )
        else:
            unframed_owner_failed = False
        check(
            "unframed source ink cannot be owned by bbox "
            f"{left:g}..{right:g}",
            unframed_owner_failed,
        )

    maximal_frame_page = source_page(
        *(
            source_paint(x, order=index)
            for index, x in enumerate((5, 10, 15, 20, 25, 30, 35))
        ),
        VectorPaint(5.0, 7.88, 35.0, 8.12,
                    0.0, 1.0, 20, "maximal-frame-baseline"),
        framed=False,
    )
    maximal_frame = printed_compartments(
        maximal_frame_page, comb_subject())
    check(
        "an untrimmed maximal source U-frame owns its five dividers",
        maximal_frame == (6, [10.0, 15.0, 20.0, 25.0, 30.0]),
    )
    for left, right in ((10.0, 30.0), (15.0, 25.0)):
        try:
            printed_compartments(
                maximal_frame_page,
                comb_subject(x0=left, x1=right),
            )
        except CombTopologyError as exc:
            cropped_frame_failed = (
                "crops a wider source U-frame" in str(exc)
                and exc.evidence["frame"]["left_rail"] == 5.0
                and exc.evidence["frame"]["right_rail"] == 35.0
            )
        else:
            cropped_frame_failed = False
        check(
            f"a {left:g}..{right:g} bbox cannot manufacture inner frame rails",
            cropped_frame_failed,
        )
    (_cropped_registry, cropped_owner_cell, cropped_owner,
     _cropped_reason) = owner_registry_fixture(
         comb_subject(x0=10.0, x1=30.0))
    if cropped_owner is not None:
        try:
            printed_compartments(
                maximal_frame_page,
                cropped_owner_cell,
                owner_certificate=cropped_owner,
            )
        except CombTopologyError as exc:
            certified_cropped_frame_failed = (
                "crops a wider source U-frame" in str(exc)
                and exc.evidence.get("cropped_sides") == ["left", "right"]
            )
        else:
            certified_cropped_frame_failed = False
        check(
            "reviewed ownership cannot crop a wider source U-frame",
            certified_cropped_frame_failed,
        )

    disconnected_baseline_page = source_page(
        *(
            source_paint(x, order=index)
            for index, x in enumerate((5, 10, 15, 20, 25, 30, 35))
        ),
        VectorPaint(
            7.4, 7.88, 32.6, 8.12,
            0.0, 1.0, 20, "disconnected-frame-baseline"),
        framed=False,
    )
    try:
        printed_compartments(
            disconnected_baseline_page, comb_subject())
    except CombTopologyError as exc:
        disconnected_baseline_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required")
    else:
        disconnected_baseline_failed = False
    check(
        "baseline endpoints must touch actual rail ink, not nearby centres",
        disconnected_baseline_failed,
    )

    y_gap_frame_page = source_page(
        *(
            source_paint(x, a=2.0, b=7.6, order=index)
            for index, x in enumerate((5, 10, 15, 20, 25, 30, 35))
        ),
        VectorPaint(
            5.0, 7.88, 35.0, 8.12,
            0.0, 1.0, 20, "y-gap-frame-baseline"),
        framed=False,
    )
    try:
        printed_compartments(y_gap_frame_page, comb_subject())
    except CombTopologyError as exc:
        y_gap_frame_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required")
    else:
        y_gap_frame_failed = False
    check(
        "verticals separated from the baseline by paper cannot form a U-frame",
        y_gap_frame_failed,
    )

    y_touch_frame_page = source_page(
        *(
            source_paint(x, a=2.0, b=7.88, order=index)
            for index, x in enumerate((5, 10, 15, 20, 25, 30, 35))
        ),
        VectorPaint(
            5.0, 7.88, 35.0, 8.12,
            0.0, 1.0, 20, "y-touch-frame-baseline"),
        framed=False,
    )
    y_touch_frame = printed_compartments(
        y_touch_frame_page, comb_subject(), include_frame=True)
    check(
        "exact y-touch between every vertical and baseline forms a U-frame",
        y_touch_frame[:2]
        == (6, [10.0, 15.0, 20.0, 25.0, 30.0])
        and y_touch_frame[2]["left_rail"]["contact_intervals_x"]
        == [[5.0, 5.12]]
        and y_touch_frame[2]["right_rail"]["contact_intervals_x"]
        == [[34.88, 35.0]],
    )

    mixed_height_frame_page = source_page(
        source_paint(5, a=2.0, b=8.0, order=0),
        source_paint(20, a=2.0, b=8.75, order=1),
        source_paint(35, a=2.0, b=8.0, order=2),
        VectorPaint(
            5.0, 8.0, 35.0, 8.75,
            0.0, 1.0, 20, "mixed-height-frame-baseline"),
        framed=False,
    )
    mixed_height_frame = printed_compartments(
        mixed_height_frame_page,
        comb_subject(x0=5.0, x1=35.0),
        include_frame=True,
    )
    check(
        "rails ending at baseline start survive an interior divider "
        "crossing baseline thickness",
        mixed_height_frame[:2] == (2, [20.0])
        and mixed_height_frame[2]["left_rail"]["ink_y1"] == 8.0
        and mixed_height_frame[2]["right_rail"]["ink_y1"] == 8.0,
    )
    check(
        "equivalent ordinary and segmented discovery stays deterministic",
        printed_compartments(
            mixed_height_frame_page,
            comb_subject(x0=5.0, x1=35.0),
            include_frame=True,
        ) == mixed_height_frame,
    )

    late_start_frame_page = source_page(
        source_paint(5, a=2.3, b=8.0, order=0),
        source_paint(20, a=2.0, b=8.75, order=1),
        source_paint(35, a=2.3, b=8.0, order=2),
        VectorPaint(
            5.0, 8.0, 35.0, 8.75,
            0.0, 1.0, 20, "late-start-frame-baseline"),
        framed=False,
    )
    late_start_baseline = next(
        baseline for baseline in _baseline_spans(
            late_start_frame_page, 8.0, 0.0)
        if baseline.left == 5.0 and baseline.right == 35.0
    )
    late_start_left_rail = _source_vertical_ink_geometry(
        late_start_frame_page, 5.0, 2.0, 8.75, 0.0)
    check(
        "a connected ordinary rail may begin inside existing leading slack",
        5.0 in _stable_source_verticals(
            late_start_frame_page, 2.5, 37.5, 2.0, 8.0, 0.0)
        and _baseline_coordinate_contacts_vertical(
            late_start_frame_page, 0.0, 5.0,
            late_start_left_rail, late_start_baseline)
        and _connected_vertical_baseline_contact(
            late_start_frame_page, 0.0, late_start_left_rail,
            2.0, 8.0, 5.0, late_start_baseline),
    )
    late_start_frame = printed_compartments(
        late_start_frame_page,
        comb_subject(x0=5.0, x1=35.0),
    )
    check(
        "leading slack does not erase a continuous source U-frame",
        late_start_frame == (2, [20.0]),
    )

    mixed_height_gap_page = source_page(
        source_paint(5, a=2.0, b=7.7, order=0),
        source_paint(20, a=2.0, b=8.75, order=1),
        source_paint(35, a=2.0, b=7.7, order=2),
        VectorPaint(
            5.0, 8.0, 35.0, 8.75,
            0.0, 1.0, 20, "mixed-height-gap-baseline"),
        framed=False,
    )
    try:
        printed_compartments(
            mixed_height_gap_page,
            comb_subject(x0=5.0, x1=35.0),
        )
    except CombTopologyError as exc:
        mixed_height_gap_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required"
        )
    else:
        mixed_height_gap_failed = False
    check(
        "an interior divider crossing baseline thickness cannot bridge "
        "paper gaps under the side rails",
        mixed_height_gap_failed,
    )

    disconnected_contact_page = source_page(
        source_paint(5, a=2.0, b=7.5, order=0),
        source_paint(20, a=2.0, b=8.85, order=1),
        source_paint(35, a=2.0, b=7.5, order=2),
        source_paint(5, a=7.8, b=8.85, order=3),
        source_paint(35, a=7.8, b=8.85, order=4),
        VectorPaint(
            5.0, 8.0, 35.0, 8.75,
            0.0, 1.0, 20, "disconnected-contact-baseline"),
        framed=False,
    )
    disconnected_baseline = next(
        baseline for baseline in _baseline_spans(
            disconnected_contact_page, 8.0, 0.0)
        if baseline.left == 5.0 and baseline.right == 35.0
    )
    disconnected_left_rail = _source_vertical_ink_geometry(
        disconnected_contact_page, 5.0, 2.0, 8.75, 0.0)
    check(
        "disconnected ordinary fixture reaches the formerly independent "
        "stable-span and exact-contact predicates",
        5.0 in _stable_source_verticals(
            disconnected_contact_page, 2.5, 37.5, 2.0, 8.0, 0.0)
        and _baseline_coordinate_contacts_vertical(
            disconnected_contact_page, 0.0, 5.0,
            disconnected_left_rail, disconnected_baseline)
        and not _connected_vertical_baseline_contact(
            disconnected_contact_page, 0.0, disconnected_left_rail,
            2.0, 8.0, 5.0, disconnected_baseline),
    )
    try:
        printed_compartments(
            disconnected_contact_page,
            comb_subject(x0=5.0, x1=35.0),
        )
    except CombTopologyError as exc:
        disconnected_contact_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required"
        )
    else:
        disconnected_contact_failed = False
    check(
        "a separate same-x baseline-contact fragment cannot bridge "
        "0.3pt of paper in an ordinary frame",
        disconnected_contact_failed,
    )

    disconnected_interior_page = source_page(
        source_paint(5, a=2.0, b=8.0, order=0),
        source_paint(20, a=2.0, b=7.5, order=1),
        source_paint(35, a=2.0, b=8.0, order=2),
        source_paint(20, a=7.8, b=8.85, order=3),
        VectorPaint(
            5.0, 8.0, 35.0, 8.75,
            0.0, 1.0, 20, "disconnected-interior-baseline"),
        framed=False,
    )
    disconnected_interior_baseline = next(
        baseline for baseline in _baseline_spans(
            disconnected_interior_page, 8.0, 0.0)
        if baseline.left == 5.0 and baseline.right == 35.0
    )
    disconnected_interior_geometry = _source_vertical_ink_geometry(
        disconnected_interior_page, 20.0, 2.0, 8.75, 0.0)
    check(
        "disconnected ordinary interior reaches aggregate stable/contact "
        "evidence but not one segment-bound path",
        20.0 in _stable_source_verticals(
            disconnected_interior_page, 2.5, 37.5, 2.0, 8.0, 0.0)
        and _baseline_coordinate_contacts_vertical(
            disconnected_interior_page, 0.0, 20.0,
            disconnected_interior_geometry, disconnected_interior_baseline)
        and not _vertical_has_connected_baseline_contact(
            disconnected_interior_page, 0.0,
            disconnected_interior_geometry, 2.0, 20.0,
            disconnected_interior_baseline),
    )
    try:
        printed_compartments(
            disconnected_interior_page,
            comb_subject(x0=5.0, x1=35.0),
        )
    except CombTopologyError as exc:
        disconnected_interior_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required"
        )
    else:
        disconnected_interior_failed = False
    check(
        "an ordinary interior cannot borrow detached baseline-contact ink "
        "across 0.3pt of paper",
        disconnected_interior_failed,
    )

    canceled_interior_page = source_page(
        source_paint(5, a=2.0, b=8.0, order=0),
        VectorPaint(
            19.88, 2.0, 20.12, 8.0,
            0.0, 1.0, 2, "evenodd-full-vertical",
            operation=500, fill_rule="evenodd"),
        VectorPaint(
            19.88, 7.6, 20.12, 7.9,
            0.0, 1.0, 2, "evenodd-canceling-strip",
            operation=500, fill_rule="evenodd"),
        source_paint(35, a=2.0, b=8.0, order=3),
        VectorPaint(
            0.0, 7.6, 40.0, 7.9,
            0.0, 1.0, 10, "unrelated-broad-black-repaint"),
        VectorPaint(
            5.0, 8.0, 35.0, 8.75,
            0.0, 1.0, 20, "canceled-interior-baseline"),
        framed=False,
    )
    canceled_interior_baseline = next(
        baseline for baseline in _baseline_spans(
            canceled_interior_page, 8.0, 0.0)
        if baseline.left == 5.0 and baseline.right == 35.0
    )
    canceled_interior_geometry = _source_vertical_ink_geometry(
        canceled_interior_page, 20.0, 2.0, 8.75, 0.0)
    canceled_operation = [
        paint for paint in canceled_interior_page.paints
        if (paint.order, paint.operation) == (2, 500)
    ]
    check(
        "broad final black cannot back an even-odd-canceled vertical operation",
        20.0 in _stable_source_verticals(
            canceled_interior_page, 2.5, 37.5, 2.0, 8.0, 0.0)
        and _final_tone(
            [
                paint for paint in canceled_interior_page.paints
                if paint.y0 <= 7.75 <= paint.y1
            ],
            20.0,
            7.75,
        ) == 0.0
        and not _operation_covers(
            canceled_operation, 20.0, 7.75)
        and not _vertical_has_connected_baseline_contact(
            canceled_interior_page, 0.0,
            canceled_interior_geometry, 2.0, 20.0,
            canceled_interior_baseline),
    )
    try:
        printed_compartments(
            canceled_interior_page,
            comb_subject(x0=5.0, x1=35.0),
        )
    except CombTopologyError as exc:
        canceled_interior_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required"
        )
    else:
        canceled_interior_failed = False
    check(
        "ordinary U-frame rejects canceled interior ink hidden by broad repaint",
        canceled_interior_failed,
    )

    genuine_repaint_page = source_page(
        source_paint(5, a=2.0, b=8.0, order=0),
        VectorPaint(
            19.88, 2.0, 20.12, 8.0,
            0.0, 1.0, 2, "evenodd-genuine-vertical",
            operation=501, fill_rule="evenodd"),
        source_paint(35, a=2.0, b=8.0, order=3),
        VectorPaint(
            0.0, 7.6, 40.0, 7.9,
            0.0, 1.0, 10, "same-tone-broad-repaint"),
        VectorPaint(
            5.0, 8.0, 35.0, 8.75,
            0.0, 1.0, 20, "genuine-repaint-baseline"),
        framed=False,
    )
    genuine_repaint_baseline = next(
        baseline for baseline in _baseline_spans(
            genuine_repaint_page, 8.0, 0.0)
        if baseline.left == 5.0 and baseline.right == 35.0
    )
    genuine_repaint_geometry = _source_vertical_ink_geometry(
        genuine_repaint_page, 20.0, 2.0, 8.75, 0.0)
    check(
        "same-tone repaint preserves a genuinely painted vertical operation",
        _vertical_has_connected_baseline_contact(
            genuine_repaint_page, 0.0,
            genuine_repaint_geometry, 2.0, 20.0,
            genuine_repaint_baseline)
        and printed_compartments(
            genuine_repaint_page,
            comb_subject(x0=5.0, x1=35.0),
        ) == (2, [20.0]),
    )

    split_rail_frame_page = source_page(
        source_paint(4.7, order=0),
        source_paint(5.3, order=1),
        *(
            source_paint(x, order=index + 2)
            for index, x in enumerate((10, 15, 20, 25, 30))
        ),
        source_paint(34.7, order=10),
        source_paint(35.3, order=11),
        VectorPaint(
            5.0, 7.88, 35.0, 8.12,
            0.0, 1.0, 20, "split-rail-frame-baseline"),
        framed=False,
    )
    try:
        printed_compartments(split_rail_frame_page, comb_subject())
    except CombTopologyError as exc:
        split_rail_frame_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required")
    else:
        split_rail_frame_failed = False
    check(
        "a baseline endpoint in the gap between split rail paints is not contact",
        split_rail_frame_failed,
    )

    segmented_frame_page = source_page(
        *(
            source_paint(x, order=index)
            for index, x in enumerate((5, 10, 15, 20, 25, 30, 35))
        ),
        *(
            VectorPaint(
                left, 7.88, left + 5.0, 8.12,
                0.0, 1.0, 20 + index, "segmented-frame-baseline",
            )
            for index, left in enumerate((5, 10, 15, 20, 25, 30))
        ),
        framed=False,
    )
    segmented_full = printed_compartments(
        segmented_frame_page, comb_subject())
    check(
        "six explicit baseline operations form one maximal source frame",
        segmented_full == (6, [10.0, 15.0, 20.0, 25.0, 30.0]),
    )

    mixed_segmentation_page = source_page(
        *(
            source_paint(x, a=2.0, b=8.0, order=index)
            for index, x in enumerate((5, 10, 15, 20, 25, 30, 35))
        ),
        VectorPaint(
            5.0, 8.0, 20.0, 8.75,
            0.0, 1.0, 20, "mixed-segmentation-wide-baseline"),
        *(
            VectorPaint(
                left, 8.0, left + 5.0, 8.75,
                0.0, 1.0, 21 + index,
                "mixed-segmentation-short-baseline",
            )
            for index, left in enumerate((20, 25, 30))
        ),
        framed=False,
    )
    mixed_segmentation_baselines = _baseline_spans(
        mixed_segmentation_page, 8.0, 0.0)
    mixed_segmentation_short = next(
        baseline for baseline in mixed_segmentation_baselines
        if baseline.left == 5.0 and baseline.right == 20.0
    )
    mixed_segmentation_short_verticals = _stable_source_verticals(
        mixed_segmentation_page, 2.5, 22.5, 2.0, 8.0, 0.0)
    check(
        "mixed segmentation contains a valid short ordinary U-frame",
        all(
            source_x in mixed_segmentation_short_verticals
            and _vertical_has_connected_baseline_contact(
                mixed_segmentation_page,
                0.0,
                _source_vertical_ink_geometry(
                    mixed_segmentation_page,
                    source_x,
                    2.0,
                    8.0,
                    0.0,
                ),
                2.0,
                contact_x,
                mixed_segmentation_short,
            )
            for source_x, contact_x in (
                (5.0, 5.0),
                (10.0, 10.0),
                (15.0, 15.0),
                (20.0, 20.0),
            )
        ),
    )
    mixed_segmentation_wide_candidates = _segmented_u_frame_candidates(
        mixed_segmentation_page,
        mixed_segmentation_baselines,
        2.0,
        8.0,
        0.0,
        (10.0, 15.0),
    )
    check(
        "wider segmented discovery participates beside the short ordinary "
        "candidate",
        any(
            candidate[0] == 5.0
            and candidate[1] == 35.0
            and candidate[2] == (10.0, 15.0)
            for candidate in mixed_segmentation_wide_candidates
        ),
    )
    try:
        printed_compartments(
            mixed_segmentation_page,
            comb_subject(x0=5.0, x1=20.0),
        )
    except CombTopologyError as exc:
        mixed_segmentation_crop_failed = (
            exc.evidence.get("criterion")
            == "maximal-source-u-frame-owner"
            and exc.evidence["frame"]["left_rail"] == 5.0
            and exc.evidence["frame"]["right_rail"] == 35.0
            and exc.evidence["cropped_sides"] == ["right"]
        )
    else:
        mixed_segmentation_crop_failed = False
    check(
        "short ordinary owner is rejected as a crop of the wider frame",
        mixed_segmentation_crop_failed,
    )
    mixed_segmentation_full_a = printed_compartments(
        mixed_segmentation_page,
        comb_subject(x0=5.0, x1=35.0),
        include_frame=True,
    )
    mixed_segmentation_full_b = printed_compartments(
        mixed_segmentation_page,
        comb_subject(x0=5.0, x1=35.0),
        include_frame=True,
    )
    check(
        "full mixed-segmentation frame accepts six stable compartments",
        mixed_segmentation_full_a[:2]
        == (6, [10.0, 15.0, 20.0, 25.0, 30.0]),
    )
    check(
        "mixed ordinary/segmented discovery has no duplicate instability",
        mixed_segmentation_full_b == mixed_segmentation_full_a
        and len(
            mixed_segmentation_full_a[2]["baseline_operations"]
        ) == 4,
    )

    mixed_height_segmented_page = source_page(
        source_paint(5, a=2.0, b=8.0, order=0),
        *(
            source_paint(x, a=2.0, b=8.75, order=index + 1)
            for index, x in enumerate((10, 15, 20, 25, 30))
        ),
        source_paint(35, a=2.0, b=8.0, order=10),
        *(
            VectorPaint(
                left, 8.0, left + 5.0, 8.75,
                0.0, 1.0, 20 + index,
                "mixed-height-segmented-baseline",
            )
            for index, left in enumerate((5, 10, 15, 20, 25, 30))
        ),
        framed=False,
    )
    mixed_height_segmented = printed_compartments(
        mixed_height_segmented_page,
        comb_subject(x0=5.0, x1=35.0),
        include_frame=True,
    )
    check(
        "segmented baseline accepts side rails ending at its start while "
        "interior dividers cross its thickness",
        mixed_height_segmented[:2]
        == (6, [10.0, 15.0, 20.0, 25.0, 30.0])
        and len(
            mixed_height_segmented[2]["baseline_operations"]
        ) == 6,
    )

    mixed_height_segmented_gap_page = source_page(
        source_paint(5, a=2.0, b=7.7, order=0),
        *(
            source_paint(x, a=2.0, b=8.75, order=index + 1)
            for index, x in enumerate((10, 15, 20, 25, 30))
        ),
        source_paint(35, a=2.0, b=7.7, order=10),
        *(
            VectorPaint(
                left, 8.0, left + 5.0, 8.75,
                0.0, 1.0, 20 + index,
                "mixed-height-segmented-gap-baseline",
            )
            for index, left in enumerate((5, 10, 15, 20, 25, 30))
        ),
        framed=False,
    )
    try:
        printed_compartments(
            mixed_height_segmented_gap_page,
            comb_subject(x0=5.0, x1=35.0),
        )
    except CombTopologyError as exc:
        mixed_height_segmented_gap_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required"
        )
    else:
        mixed_height_segmented_gap_failed = False
    check(
        "segmented baselines cannot bridge paper gaps under their side rails",
        mixed_height_segmented_gap_failed,
    )

    disconnected_segmented_page = source_page(
        source_paint(5, a=2.0, b=7.5, order=0),
        *(
            source_paint(x, a=2.0, b=8.85, order=index + 1)
            for index, x in enumerate((10, 15, 20, 25, 30))
        ),
        source_paint(35, a=2.0, b=7.5, order=10),
        source_paint(5, a=7.8, b=8.85, order=11),
        source_paint(35, a=7.8, b=8.85, order=12),
        *(
            VectorPaint(
                left, 8.0, left + 5.0, 8.75,
                0.0, 1.0, 20 + index,
                "disconnected-segmented-baseline",
            )
            for index, left in enumerate((5, 10, 15, 20, 25, 30))
        ),
        framed=False,
    )
    disconnected_segmented_baselines = _baseline_spans(
        disconnected_segmented_page, 8.0, 0.0)
    disconnected_segmented_verticals = _stable_source_verticals(
        disconnected_segmented_page, 2.5, 37.5, 2.0, 8.0, 0.0)
    check(
        "disconnected segmented fixture reaches the former stable six-segment "
        "candidate path before connected-rail qualification",
        len(disconnected_segmented_baselines) == 6
        and all(
            source_x in disconnected_segmented_verticals
            for source_x in (5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0)
        )
        and not _segmented_u_frame_candidates(
            disconnected_segmented_page,
            disconnected_segmented_baselines,
            2.0,
            8.0,
            0.0,
            (10.0, 15.0, 20.0, 25.0, 30.0),
        ),
    )
    try:
        printed_compartments(
            disconnected_segmented_page,
            comb_subject(x0=5.0, x1=35.0),
        )
    except CombTopologyError as exc:
        disconnected_segmented_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required"
        )
    else:
        disconnected_segmented_failed = False
    check(
        "separate same-x contact fragments cannot bridge 0.3pt of paper "
        "into a segmented source frame",
        disconnected_segmented_failed,
    )

    disconnected_segmented_interior_page = source_page(
        *(
            source_paint(x, a=2.0, b=8.75, order=index)
            for index, x in enumerate((5, 10, 15))
        ),
        source_paint(20, a=2.0, b=7.5, order=4),
        *(
            source_paint(x, a=2.0, b=8.75, order=index + 5)
            for index, x in enumerate((25, 30, 35))
        ),
        source_paint(20, a=7.8, b=8.85, order=10),
        *(
            VectorPaint(
                left, 8.0, left + 5.0, 8.75,
                0.0, 1.0, 20 + index,
                "disconnected-segmented-interior-baseline",
            )
            for index, left in enumerate((5, 10, 15, 20, 25, 30))
        ),
        framed=False,
    )
    disconnected_segmented_interior_baselines = _baseline_spans(
        disconnected_segmented_interior_page, 8.0, 0.0)
    disconnected_segmented_interior_contacts = tuple(
        baseline
        for baseline in disconnected_segmented_interior_baselines
        if 20.0 >= baseline.left - SOURCE_COORD_EPS_PT
        and 20.0 <= baseline.right + SOURCE_COORD_EPS_PT
    )
    disconnected_segmented_interior_geometry = (
        _source_vertical_ink_geometry(
            disconnected_segmented_interior_page,
            20.0,
            2.0,
            8.75,
            0.0,
        )
    )
    check(
        "disconnected segmented interior reaches junction aggregate contact "
        "but no connected segment-level witness",
        20.0 in _stable_source_verticals(
            disconnected_segmented_interior_page,
            2.5,
            37.5,
            2.0,
            8.0,
            0.0,
        )
        and disconnected_segmented_interior_contacts
        and any(
            _baseline_coordinate_contacts_vertical(
                disconnected_segmented_interior_page,
                0.0,
                20.0,
                disconnected_segmented_interior_geometry,
                contact,
            )
            for contact in disconnected_segmented_interior_contacts
        )
        and all(
            not _vertical_has_connected_baseline_contact(
                disconnected_segmented_interior_page,
                0.0,
                disconnected_segmented_interior_geometry,
                2.0,
                20.0,
                contact,
            )
            for contact in disconnected_segmented_interior_contacts
        ),
    )
    check(
        "segmented candidate rejects a detached interior contact fragment",
        not _segmented_u_frame_candidates(
            disconnected_segmented_interior_page,
            disconnected_segmented_interior_baselines,
            2.0,
            8.0,
            0.0,
            (10.0, 15.0, 20.0, 25.0, 30.0),
        ),
    )
    try:
        printed_compartments(
            disconnected_segmented_interior_page,
            comb_subject(x0=5.0, x1=35.0),
        )
    except CombTopologyError as exc:
        disconnected_segmented_interior_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required"
        )
    else:
        disconnected_segmented_interior_failed = False
    check(
        "a segmented interior cannot borrow detached baseline-contact ink "
        "across 0.3pt of paper",
        disconnected_segmented_interior_failed,
    )

    canceled_segmented_interior_page = source_page(
        *(
            source_paint(x, a=2.0, b=8.75, order=index)
            for index, x in enumerate((5, 10, 15))
        ),
        VectorPaint(
            19.88, 2.0, 20.12, 8.75,
            0.0, 1.0, 4, "segmented-evenodd-full-vertical",
            operation=600, fill_rule="evenodd"),
        VectorPaint(
            19.88, 7.6, 20.12, 7.9,
            0.0, 1.0, 4, "segmented-evenodd-canceling-strip",
            operation=600, fill_rule="evenodd"),
        *(
            source_paint(x, a=2.0, b=8.75, order=index + 5)
            for index, x in enumerate((25, 30, 35))
        ),
        VectorPaint(
            0.0, 7.6, 40.0, 7.9,
            0.0, 1.0, 10, "segmented-unrelated-broad-repaint"),
        *(
            VectorPaint(
                left, 8.0, left + 5.0, 8.75,
                0.0, 1.0, 20 + index,
                "canceled-segmented-interior-baseline",
            )
            for index, left in enumerate((5, 10, 15, 20, 25, 30))
        ),
        framed=False,
    )
    canceled_segmented_baselines = _baseline_spans(
        canceled_segmented_interior_page, 8.0, 0.0)
    canceled_segmented_geometry = _source_vertical_ink_geometry(
        canceled_segmented_interior_page, 20.0, 2.0, 8.75, 0.0)
    canceled_segmented_contacts = tuple(
        contact for contact in canceled_segmented_baselines
        if abs(contact.y0 - 8.0) <= SOURCE_COORD_EPS_PT
        and contact.left >= 5.0 - SOURCE_COORD_EPS_PT
        and contact.right <= 35.0 + SOURCE_COORD_EPS_PT
        and 20.0 >= contact.left - SOURCE_COORD_EPS_PT
        and 20.0 <= contact.right + SOURCE_COORD_EPS_PT
    )
    check(
        "canceled segmented interior remains inside stable-span slack",
        20.0 in _stable_source_verticals(
            canceled_segmented_interior_page,
            2.5,
            37.5,
            2.0,
            8.0,
            0.0,
        ),
    )
    check(
        "segmented junction contacts exist but lack operation-backed paths",
        bool(canceled_segmented_contacts)
        and all(
            not _vertical_has_connected_baseline_contact(
                canceled_segmented_interior_page,
                0.0,
                canceled_segmented_geometry,
                2.0,
                20.0,
                contact,
            )
            for contact in canceled_segmented_contacts
        ),
    )
    check(
        "segmented candidate rejects the operation-canceled interior",
        not _segmented_u_frame_candidates(
            canceled_segmented_interior_page,
            canceled_segmented_baselines,
            2.0,
            8.0,
            0.0,
            (10.0, 15.0, 20.0, 25.0, 30.0),
        ),
    )
    try:
        printed_compartments(
            canceled_segmented_interior_page,
            comb_subject(x0=5.0, x1=35.0),
        )
    except CombTopologyError as exc:
        canceled_segmented_interior_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required"
        )
    else:
        canceled_segmented_interior_failed = False
    check(
        "segmented U-frame rejects canceled interior ink hidden by broad repaint",
        canceled_segmented_interior_failed,
    )

    mixed_level_disconnected_page = source_page(
        source_paint(5, a=2.0, b=8.0, order=0),
        *(
            source_paint(x, a=2.0, b=9.45, order=index + 1)
            for index, x in enumerate((10, 15, 20, 25, 30))
        ),
        source_paint(35, a=2.0, b=8.0, order=10),
        source_paint(35, a=8.4, b=9.45, order=11),
        *(
            VectorPaint(
                left,
                8.0 if index % 2 == 0 else 8.4,
                left + 5.0,
                8.4 if index % 2 == 0 else 8.8,
                0.0, 1.0, 20 + index,
                "mixed-level-disconnected-baseline",
            )
            for index, left in enumerate((5, 10, 15, 20, 25, 30))
        ),
        framed=False,
    )
    mixed_level_baselines = _baseline_spans(
        mixed_level_disconnected_page, 8.8, 0.0)
    mixed_level_right_baseline = next(
        baseline for baseline in mixed_level_baselines
        if baseline.right == 35.0
    )
    mixed_level_right_rail = _source_vertical_ink_geometry(
        mixed_level_disconnected_page, 35.0, 2.0, 8.8, 0.0)
    check(
        "mixed-level fixture isolates component-minimum over-admission from "
        "the actual right endpoint segment level",
        35.0 in _stable_source_verticals(
            mixed_level_disconnected_page,
            2.5,
            37.5,
            2.0,
            8.0,
            0.0,
        )
        and _connected_vertical_baseline_contact(
            mixed_level_disconnected_page, 0.0, mixed_level_right_rail,
            2.0, 8.0, 35.0, mixed_level_right_baseline)
        and not _connected_vertical_baseline_contact(
            mixed_level_disconnected_page, 0.0, mixed_level_right_rail,
            2.0, mixed_level_right_baseline.y0,
            35.0, mixed_level_right_baseline),
    )
    check(
        "segmented endpoint qualification rejects a detached rail at the "
        "later right baseline level",
        not _segmented_u_frame_candidates(
            mixed_level_disconnected_page,
            mixed_level_baselines,
            2.0,
            8.8,
            0.0,
            (10.0, 15.0, 20.0, 25.0, 30.0),
        ),
    )
    try:
        printed_compartments(
            mixed_level_disconnected_page,
            comb_subject(x0=5.0, x1=35.0),
        )
    except CombTopologyError as exc:
        mixed_level_disconnected_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required"
        )
    else:
        mixed_level_disconnected_failed = False
    check(
        "a later segmented endpoint cannot borrow detached contact ink "
        "above an internal paper gap",
        mixed_level_disconnected_failed,
    )

    for left, right in ((10.0, 30.0), (15.0, 25.0)):
        try:
            printed_compartments(
                segmented_frame_page,
                comb_subject(x0=left, x1=right),
            )
        except CombTopologyError as exc:
            segmented_crop_failed = (
                "crops a wider source U-frame" in str(exc)
                and exc.evidence["frame"]["left_rail"] == 5.0
                and exc.evidence["frame"]["right_rail"] == 35.0
                and len(exc.evidence["frame"]["baseline_operations"]) == 6
            )
        else:
            segmented_crop_failed = False
        check(
            f"segmented baseline rejects a {left:g}..{right:g} bbox crop",
            segmented_crop_failed,
        )

    alternating_segment_page = source_page(
        *(
            source_paint(x, a=2.0, b=8.32, order=index)
            for index, x in enumerate((5, 10, 15, 20, 25, 30, 35))
        ),
        *(
            VectorPaint(
                left,
                7.68 if index % 2 == 0 else 8.08,
                left + 5.0,
                7.92 if index % 2 == 0 else 8.32,
                0.0, 1.0, 20 + index,
                "alternating-level-segmented-baseline",
            )
            for index, left in enumerate((5, 10, 15, 20, 25, 30))
        ),
        framed=False,
    )
    try:
        printed_compartments(alternating_segment_page, comb_subject())
    except CombTopologyError as exc:
        alternating_segment_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required")
    else:
        alternating_segment_failed = False
    check(
        "alternating baseline levels separated by paper do not merge",
        alternating_segment_failed,
    )

    touching_segment_page = source_page(
        *(
            source_paint(x, a=2.0, b=8.16, order=index)
            for index, x in enumerate((5, 10, 15, 20, 25, 30, 35))
        ),
        *(
            VectorPaint(
                left,
                7.68 if index % 2 == 0 else 7.92,
                left + 5.0,
                7.92 if index % 2 == 0 else 8.16,
                0.0, 1.0, 20 + index,
                "touching-level-segmented-baseline",
            )
            for index, left in enumerate((5, 10, 15, 20, 25, 30))
        ),
        framed=False,
    )
    touching_segment = printed_compartments(
        touching_segment_page, comb_subject())
    check(
        "piecewise baseline segments that truly y-touch remain connected",
        touching_segment == (
            6, [10.0, 15.0, 20.0, 25.0, 30.0]),
    )

    buried_baseline_page = source_page(
        *(
            source_paint(x, order=index)
            for index, x in enumerate((5, 10, 15, 20, 25, 30, 35))
        ),
        VectorPaint(
            5.0, 7.88, 35.0, 8.12,
            0.0, 1.0, 20, "buried-narrow-baseline"),
        VectorPaint(
            0.0, 7.5, 40.0, 10.5,
            0.0, 1.0, 21, "later-broad-same-tone-fill"),
        framed=False,
    )
    try:
        printed_compartments(buried_baseline_page, comb_subject())
    except CombTopologyError as exc:
        buried_baseline_failed = (
            exc.evidence.get("criterion")
            == "independent-complete-source-u-frame-required")
    else:
        buried_baseline_failed = False
    check(
        "broad same-tone overpaint cannot own a buried narrow baseline",
        buried_baseline_failed,
    )

    repainted_baseline_page = source_page(
        *buried_baseline_page.paints,
        VectorPaint(
            5.0, 7.88, 35.0, 8.12,
            0.0, 1.0, 22, "final-narrow-baseline-repaint"),
        framed=False,
    )
    repainted_baseline = printed_compartments(
        repainted_baseline_page, comb_subject())
    check(
        "a final narrow baseline repaint restores source ownership",
        repainted_baseline == (
            6, [10.0, 15.0, 20.0, 25.0, 30.0]),
    )

    expanded_frame_page = source_page(
        *maximal_frame_page.paints,
        source_paint(45, order=30),
        framed=False,
    )
    try:
        printed_compartments(
            expanded_frame_page, comb_subject(x1=50.0))
    except CombTopologyError as exc:
        expanded_frame_failed = (
            "absorbs unframed source corridors" in str(exc)
            and exc.evidence["unframed_corridors"] == [45.0]
        )
    else:
        expanded_frame_failed = False
    check(
        "an expanded owner cannot absorb a corridor outside its source frame",
        expanded_frame_failed,
    )
    (_expanded_registry, expanded_owner_cell, expanded_owner,
     _expanded_reason) = owner_registry_fixture(comb_subject(x1=50.0))
    if expanded_owner is not None:
        try:
            printed_compartments(
                expanded_frame_page,
                expanded_owner_cell,
                owner_certificate=expanded_owner,
            )
        except CombTopologyError as exc:
            certified_expanded_frame_failed = (
                "absorbs unframed source corridors" in str(exc)
                and exc.evidence.get("unframed_corridors") == [45.0]
            )
        else:
            certified_expanded_frame_failed = False
        check(
            "reviewed ownership cannot absorb unframed source corridors",
            certified_expanded_frame_failed,
        )

    hanging_frame_page = source_page(
        source_paint(5, order=0),
        source_paint(10, order=1),
        source_paint(15, a=2.0, b=7.0, order=2),
        source_paint(20, order=3),
        source_paint(35, order=4),
        VectorPaint(5.0, 7.88, 35.0, 8.12,
                    0.0, 1.0, 20, "hanging-frame-baseline"),
        framed=False,
    )
    hanging_frame = printed_compartments(
        hanging_frame_page, comb_subject())
    check(
        "a majority-height divider hanging above the baseline stays unframed",
        hanging_frame == (3, [10.0, 20.0]),
    )

    dense = printed_compartments(
        source_page(
            source_paint(30, a=2.0, b=5.0),
            source_paint(35, a=2.0, b=5.0),
            source_paint(5, a=5.0, b=8.0),
            source_paint(10, a=5.0, b=8.0),
            source_paint(15, a=5.0, b=8.0),
            source_paint(20, a=5.0, b=8.0),
            source_paint(25, a=5.0, b=8.0),
            VectorPaint(5.0, 7.88, 25.0, 8.12,
                        0.0, 1.0, 10, "bottom-frame"),
            framed=False,
        ),
        comb_subject(),
    )
    check("a complete inset source U-frame owns one composite-cell band",
          dense == (4, [10.0, 15.0, 20.0]))

    continued_frame = printed_compartments(
        source_page(
            source_paint(5, a=2.0, b=8.0),
            source_paint(20, a=2.0, b=8.0),
            source_paint(25, a=2.0, b=8.0),
            source_paint(10, a=5.0, b=8.0),
            source_paint(15, a=5.0, b=8.0),
            VectorPaint(5.0, 7.88, 25.0, 8.12,
                        0.0, 1.0, 10, "bottom-frame"),
            framed=False,
        ),
        comb_subject(),
    )
    check(
        "one physical U-frame retains every source corridor meeting its baseline",
        continued_frame == (4, [10.0, 15.0, 20.0]),
    )

    try:
        printed_compartments(
            source_page(
                source_paint(30, a=2.0, b=5.0),
                source_paint(35, a=2.0, b=5.0),
                source_paint(10, a=5.0, b=8.0),
                source_paint(15, a=5.0, b=8.0),
                source_paint(20, a=5.0, b=8.0),
                source_paint(25, a=5.0, b=8.0),
                framed=False,
            ),
            comb_subject(),
        )
    except ValueError as exc:
        unframed_competition = (
            "without one complete source U-frame owner" in str(exc)
        )
    else:
        unframed_competition = False
    check(
        "a denser competing band without source frame ownership fails closed",
        unframed_competition,
    )

    dual_frame_page = source_page(
        VectorPaint(2.0, 1.0, 38.0, 4.0,
                    0.5, 1.0, 0, "upper-grey-band"),
        source_paint(4, a=1.0, b=4.0, tone=1.0, order=1),
        source_paint(15, a=1.0, b=4.0, tone=1.0, order=2),
        source_paint(30, a=1.0, b=4.0, tone=1.0, order=3),
        VectorPaint(4.0, 3.88, 30.0, 4.12,
                    1.0, 1.0, 4, "upper-white-baseline"),
        source_paint(5, a=6.0, b=9.0, order=5),
        source_paint(10, a=6.0, b=9.0, order=6),
        source_paint(20, a=6.0, b=9.0, order=7),
        source_paint(30, a=6.0, b=9.0, order=8),
        VectorPaint(5.0, 8.88, 30.0, 9.12,
                    0.0, 1.0, 9, "lower-black-baseline"),
        framed=False,
    )
    try:
        printed_compartments(dual_frame_page, comb_subject())
    except ValueError as exc:
        dual_frames_failed = "multiple complete source U-frames" in str(exc)
    else:
        dual_frames_failed = False
    check(
        "explicit white and black U-frames remain competing source owners",
        dual_frames_failed,
    )

    microscopic_seam = printed_compartments(
        source_page(
            VectorPaint(0.0, 2.0, 9.9999695, 8.0,
                        0.5, 1.0, 0, "left-grey"),
            VectorPaint(10.0000305, 2.0, 40.0, 8.0,
                        0.5, 1.0, 1, "right-grey"),
            source_paint(20, order=2),
        ),
        comb_subject(),
    )
    check(
        "a microscopic unpainted paper seam is never a sole divider corridor",
        microscopic_seam == (2, [20.0]),
    )

    interrupted_page = source_page(
        source_paint(10, a=2.0, b=8.0, order=0),
        source_paint(20, a=2.0, b=8.0, order=1),
        VectorPaint(0.0, 3.0, 40.0, 4.0,
                    1.0, 1.0, 2, "first-horizontal-interruptor"),
        VectorPaint(0.0, 5.0, 40.0, 6.0,
                    1.0, 1.0, 3, "second-horizontal-interruptor"),
        framed=False,
    )
    try:
        printed_compartments(interrupted_page, comb_subject())
    except CombTopologyError as exc:
        interrupted_evidence = exc.evidence
    else:
        interrupted_evidence = None
    interrupted_lineages = (
        interrupted_evidence["bands"][0]["vertical_lineages"]
        if interrupted_evidence else []
    )
    check(
        "orthogonally interrupted source lineages fail closed with exact runs",
        len(interrupted_lineages) == 2
        and all(
            lineage["continuous_runs"]
            == [[2.0, 3.0], [4.0, 5.0], [6.0, 8.0]]
            and lineage["interruptions"] == [[3.0, 4.0], [5.0, 6.0]]
            and lineage["strict_majority"] is False
            and all(
                segment["last_owners"][0]["orientation"] == "horizontal"
                for segment in lineage["interruption_segments"]
            )
            for lineage in interrupted_lineages
        ),
    )

    square_page = source_page(
        source_paint(10, a=2.0, b=5.0),
        source_paint(20, a=2.0, b=5.0),
        source_paint(30, a=2.25, b=4.75, width=2.5),
    )
    square_result = printed_compartments(square_page, comb_subject())
    check(
        "a strict-majority square is not promoted as a vertical divider",
        square_result == (3, [10.0, 20.0]),
    )

    near_square_page = source_page(
        source_paint(10, a=2.0, b=5.0),
        source_paint(20, a=2.0, b=5.0),
        source_paint(30, a=2.25, b=4.75, width=2.49),
    )
    near_square_result = printed_compartments(
        near_square_page, comb_subject())
    check(
        "epsilon-only aspect does not turn near-square decoration vertical",
        near_square_result == (3, [10.0, 20.0]),
    )

    white_knockout_failed = False
    try:
        white_knockout_result = printed_compartments(
            source_page(
                VectorPaint(0.0, 2.0, 40.0, 8.0,
                            0.5, 1.0, 0, "grey-band"),
                source_paint(10, order=1),
                source_paint(20, tone=1.0, order=2),
                framed=False,
            ),
            comb_subject(),
        )
    except ValueError:
        white_knockout_failed = True
        white_knockout_result = None
    check(
        "a white knockout on non-white paper is counted or fails closed",
        white_knockout_failed
        or white_knockout_result == (3, [10.0, 20.0]),
    )

    # Adversarial PDF-operator fixtures for the four fail-closed boundaries of
    # the source compositor. They deliberately exercise `ordered_vector_paints`
    # instead of constructing its output by hand.
    import fitz

    class FakeSourcePage:
        def __init__(self, drawings: Sequence[dict[str, Any]],
                     bboxlog: Sequence[tuple[str, Any]],
                     texttrace: Sequence[dict[str, Any]] = ()) -> None:
            self.drawings = list(drawings)
            self.bboxlog = list(bboxlog)
            self.texttrace = list(texttrace)

        def get_drawings(self, extended: bool = False) -> list[dict[str, Any]]:
            if not extended:
                raise AssertionError("source compositor omitted extended clips")
            return copy.deepcopy(self.drawings)

        def get_bboxlog(self) -> list[tuple[str, Any]]:
            return list(self.bboxlog)

        def get_texttrace(self) -> list[dict[str, Any]]:
            return copy.deepcopy(self.texttrace)

    def fake_fill(seqno: int, rect: Any, *,
                  colour: tuple[float, float, float] = (0.0, 0.0, 0.0),
                  opacity: float = 1.0,
                  items: Sequence[tuple[Any, ...]] | None = None,
                  even_odd: bool = True,
                  level: int = 0) -> dict[str, Any]:
        return {
            "type": "f", "seqno": seqno, "level": level,
            "items": list(items) if items is not None else [("re", rect, 1)],
            "even_odd": even_odd, "fill_opacity": opacity,
            "fill": colour, "rect": rect, "closePath": None,
            "color": None, "width": None, "lineCap": None,
            "lineJoin": None, "dashes": None, "stroke_opacity": None,
        }

    def fake_stroke(seqno: int, x: float, *,
                    a: float = 2.0, b: float = 8.0,
                    width: float = 0.24, level: int = 0
                    ) -> dict[str, Any]:
        return {
            "type": "s", "seqno": seqno, "level": level,
            "items": [("l", fitz.Point(x, a), fitz.Point(x, b))],
            "rect": fitz.Rect(x, a, x, b),
            "fill": None, "fill_opacity": None, "even_odd": None,
            "color": (0.0, 0.0, 0.0), "width": width,
            "lineCap": (0, 0, 0), "lineJoin": 0,
            "dashes": "[] 0", "stroke_opacity": 1.0,
            "closePath": False,
        }

    root_rect = fitz.Rect(0.0, 0.0, 40.0, 10.0)
    clipped_rect = fitz.Rect(9.88, 2.0, 10.12, 8.0)
    clipped_page = ordered_vector_paints(FakeSourcePage(
        [
            {
                "type": "group", "level": 0, "rect": root_rect,
                "isolated": True, "knockout": False,
                "blendmode": "Normal", "opacity": 1.0,
            },
            {
                "type": "clip", "level": 1, "even_odd": True,
                "items": [("re", root_rect, 1)], "scissor": root_rect,
            },
            {
                "type": "clip", "level": 2, "even_odd": True,
                "items": [("re", fitz.Rect(15.0, 0.0, 40.0, 10.0), 1)],
                "scissor": fitz.Rect(15.0, 0.0, 40.0, 10.0),
            },
            fake_fill(0, clipped_rect, level=3),
        ],
        [("fill-path", clipped_rect)],
    ))
    try:
        printed_compartments(clipped_page, comb_subject())
    except ValueError as exc:
        clipped_failed = "no plausible source-derived comb band" in str(exc)
    else:
        clipped_failed = False
    check("nested even-odd rectangular scissors remove a clipped-away divider",
          clipped_failed and not clipped_page.paints)

    stroked_clip_page = ordered_vector_paints(FakeSourcePage(
        [
            {
                "type": "clip", "level": 0, "even_odd": True,
                "items": [("re", root_rect, 1)], "scissor": root_rect,
            },
            fake_stroke(0, 10.0, level=1),
            fake_stroke(1, 20.0, level=0),
        ],
        [
            ("stroke-path", fitz.Rect(9.88, 1.88, 10.12, 8.12)),
            ("stroke-path", fitz.Rect(19.88, 1.88, 20.12, 8.12)),
        ],
    ))
    stroked_clip_page = owned_test_page(stroked_clip_page)
    stroked_clip = printed_compartments(
        stroked_clip_page, comb_subject())
    check(
        "zero-width line paths use stroked extent for clip inclusion",
        stroked_clip == (3, [10.0, 20.0]),
    )

    transparent_group_page = ordered_vector_paints(FakeSourcePage(
        [
            {
                "type": "group", "level": 0, "rect": root_rect,
                "isolated": True, "knockout": False,
                "blendmode": "Normal", "opacity": 0.0,
            },
            fake_stroke(0, 10.0, level=1),
            fake_stroke(1, 20.0, level=0),
        ],
        [
            ("stroke-path", fitz.Rect(9.88, 1.88, 10.12, 8.12)),
            ("stroke-path", fitz.Rect(19.88, 1.88, 20.12, 8.12)),
        ],
    ))
    try:
        printed_compartments(transparent_group_page, comb_subject())
    except ValueError as exc:
        transparent_group_failed = "transparency group" in str(exc)
    else:
        transparent_group_failed = False
    check(
        "nested zero-area line paint inherits its non-normal group",
        transparent_group_failed,
    )

    complex_clip_page = ordered_vector_paints(FakeSourcePage(
        [
            {
                "type": "clip", "level": 0, "even_odd": True,
                "items": [
                    ("re", root_rect, 1),
                    ("re", fitz.Rect(9.0, 1.0, 11.0, 9.0), 1),
                ],
                "scissor": root_rect,
            },
            fake_fill(0, clipped_rect, level=1),
        ],
        [("fill-path", clipped_rect)],
    ))
    try:
        printed_compartments(complex_clip_page, comb_subject())
    except ValueError as exc:
        complex_clip_failed = "compound or non-rectilinear source clip" in str(exc)
    else:
        complex_clip_failed = False
    check("compound even-odd clip topology is conservatively unevaluable",
          complex_clip_failed)

    divider_rect = fitz.Rect(9.88, 2.0, 10.12, 8.0)
    covering_rect = fitz.Rect(8.0, 2.0, 12.0, 8.0)
    image_page = ordered_vector_paints(FakeSourcePage(
        [fake_fill(0, divider_rect)],
        [("fill-path", divider_rect), ("fill-image", covering_rect)],
    ))
    text_page = ordered_vector_paints(FakeSourcePage(
        [fake_fill(0, divider_rect)],
        [("fill-path", divider_rect), ("fill-text", covering_rect)],
    ))
    for label, foreign_page, foreign_kind in (
            ("image", image_page, "fill-image"),
            ("text", text_page, "fill-text")):
        try:
            printed_compartments(foreign_page, comb_subject())
        except ValueError as exc:
            foreign_failed = foreign_kind in str(exc)
        else:
            foreign_failed = False
        check(f"later {label} paint intersecting the source band fails closed",
              foreign_failed)

    def fake_texttrace(
            seqno: int, rect: Any, colour: tuple[float, float, float],
            *, linewidth: float | None = None, text_type: int = 0
            ) -> dict[str, Any]:
        return {
            "seqno": seqno, "color": colour, "opacity": 1.0,
            "linewidth": linewidth, "type": text_type,
            "chars": ((65, 1, (rect.x0, rect.y1), tuple(rect)),),
        }

    same_tone_text_page = ordered_vector_paints(FakeSourcePage(
        [fake_fill(0, divider_rect)],
        [("fill-path", divider_rect), ("fill-text", covering_rect)],
        [fake_texttrace(1, covering_rect, (0.0, 0.0, 0.0))],
    ))
    try:
        printed_compartments(same_tone_text_page, comb_subject())
    except ValueError as exc:
        broad_same_tone_failed = "fill-text" in str(exc)
    else:
        broad_same_tone_failed = False
    check(
        "same-tone glyph bounds crossing a divider fail closed",
        broad_same_tone_failed,
    )

    separate_trace = fitz.Rect(3.0, 2.0, 4.0, 8.0)
    broad_text_bbox = fitz.Rect(3.0, 2.0, 12.0, 8.0)
    separate_same_tone_page = ordered_vector_paints(FakeSourcePage(
        [fake_fill(0, divider_rect)],
        [("fill-path", divider_rect), ("fill-text", broad_text_bbox)],
        [fake_texttrace(1, separate_trace, (0.0, 0.0, 0.0))],
    ))
    separate_same_tone_page = owned_test_page(separate_same_tone_page)
    separate_same_tone = printed_compartments(
        separate_same_tone_page, comb_subject())
    check(
        "correlated same-tone glyphs safely separated from dividers are allowed",
        separate_same_tone == (2, [10.0]),
    )

    erasing_text_page = ordered_vector_paints(FakeSourcePage(
        [fake_fill(0, divider_rect)],
        [("fill-path", divider_rect), ("fill-text", covering_rect)],
        [fake_texttrace(1, covering_rect, (1.0, 1.0, 1.0))],
    ))
    try:
        printed_compartments(erasing_text_page, comb_subject())
    except ValueError as exc:
        erasing_text_failed = "fill-text" in str(exc)
    else:
        erasing_text_failed = False
    check("later text of another tone crossing a divider fails closed",
          erasing_text_failed)

    # The bbox log is the conservative paint envelope. A tighter traced glyph
    # may localise same-tone text, but must never shrink different/unknown-tone
    # blocking away from a divider.
    mismatched_bbox = fitz.Rect(9.0, 2.0, 11.0, 8.0)
    mismatched_trace = fitz.Rect(8.0, 2.0, 9.9, 8.0)
    mismatched_text_page = ordered_vector_paints(FakeSourcePage(
        [fake_fill(0, divider_rect)],
        [("fill-path", divider_rect), ("fill-text", mismatched_bbox)],
        [fake_texttrace(1, mismatched_trace, (1.0, 1.0, 1.0))],
    ))
    try:
        printed_compartments(mismatched_text_page, comb_subject())
    except ValueError as exc:
        mismatched_bbox_failed = "fill-text" in str(exc)
    else:
        mismatched_bbox_failed = False
    check("different-tone text keeps its full bboxlog paint envelope",
          mismatched_bbox_failed)

    stroke_bbox = fitz.Rect(8.8, 2.0, 9.2, 8.0)
    stroke_text_page = ordered_vector_paints(FakeSourcePage(
        [fake_fill(0, divider_rect)],
        [("fill-path", divider_rect), ("stroke-text", stroke_bbox)],
        [fake_texttrace(
            1, stroke_bbox, (1.0, 1.0, 1.0),
            linewidth=2.0, text_type=1)],
    ))
    try:
        printed_compartments(stroke_text_page, comb_subject())
    except ValueError as exc:
        stroke_text_failed = "stroke-text" in str(exc)
    else:
        stroke_text_failed = False
    check("stroke-text bboxlog envelope includes its traced line width",
          stroke_text_failed)

    cancelled_rect = fitz.Rect(9.88, 2.0, 10.12, 8.0)
    surviving_rect = fitz.Rect(19.88, 2.0, 20.12, 8.0)
    cancelled_page = ordered_vector_paints(FakeSourcePage(
        [
            fake_fill(
                0, cancelled_rect, even_odd=True,
                items=[
                    ("re", cancelled_rect, 1),
                    ("re", cancelled_rect, 1),
                ],
            ),
            fake_fill(1, surviving_rect),
        ],
        [("fill-path", cancelled_rect), ("fill-path", surviving_rect)],
    ))
    cancelled_page = owned_test_page(cancelled_page)
    cancelled = printed_compartments(cancelled_page, comb_subject())
    check("overlapping regions of one even-odd fill cancel exactly",
          cancelled == (2, [20.0]))

    translucent_rect = fitz.Rect(9.88, 2.0, 10.12, 8.0)
    grey_rect = fitz.Rect(19.88, 2.0, 20.12, 8.0)
    translucent_page = ordered_vector_paints(FakeSourcePage(
        [
            fake_fill(
                0, translucent_rect, opacity=0.5, even_odd=False,
                items=[
                    ("re", translucent_rect, 1),
                    ("re", translucent_rect, 1),
                ],
            ),
            fake_fill(1, grey_rect, colour=(0.5, 0.5, 0.5)),
        ],
        [("fill-path", translucent_rect), ("fill-path", grey_rect)],
    ))
    translucent_page = owned_test_page(translucent_page)
    try:
        translucent = printed_compartments(
            translucent_page, comb_subject())
    except CombTopologyError:
        translucent = None
    check("one compound fill applies opacity once across overlapping regions",
          translucent is None or translucent == (3, [10.0, 20.0]))

    many = tuple(source_paint(float(x)) for x in range(4, 84, 4))
    many_count, many_xs = printed_compartments(
        source_page(*many), comb_subject(x1=88.0))
    check("printed divider evidence is exhaustive beyond sixteen entries",
          many_count == 21 and many_xs == [float(x) for x in range(4, 84, 4)])

    for name in failures:
        print(f"FAIL {name}", file=sys.stderr)
    print(f"audit self-test: {len(failures)} failure(s)", file=sys.stderr)
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ir-dir", type=pathlib.Path, default=pathlib.Path("build/ir"))
    parser.add_argument("--html-dir", type=pathlib.Path, default=pathlib.Path("build/html"))
    parser.add_argument("--layout-dir", type=pathlib.Path,
                        default=pathlib.Path("build/layout"),
                        help="Lattice output; the assertions read cell and comb geometry.")
    parser.add_argument("--work", type=pathlib.Path, default=pathlib.Path("build/audit"))
    parser.add_argument("--guide-dir", type=pathlib.Path, default=pathlib.Path("build/guides"),
                        help="Guide plans; content moved to guide.html leaves the form denominator.")
    parser.add_argument("--source-root", type=pathlib.Path,
                        default=pathlib.Path.home() / "Downloads/forms",
                        help="Where the pinned official PDFs live. Three assertions "
                             "read the source's own operators rather than our IR.")
    parser.add_argument("--out", type=pathlib.Path, default=pathlib.Path("build/audit.json"))
    parser.add_argument("--only", action="append", default=None)
    parser.add_argument("--assertions-only", action="store_true",
                        help="Skip the Chromium round trip. Useful while iterating on "
                             "an assertion; not the audit.")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument(
        "--render-worker", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()

    if args.render_worker:
        return _run_render_worker()
    if args.self_test:
        return self_test()

    slugs = sorted(p.name[: -len(".ir.json")] for p in args.ir_dir.glob("*.ir.json"))
    if args.only:
        wanted = {s.lower() for s in args.only}
        slugs = [s for s in slugs if any(w in s for w in wanted)]

    records = []
    for i, slug in enumerate(slugs, 1):
        html = args.html_dir / f"{slug}.html"
        if not html.is_file():
            print(f"[{i:>2}/{len(slugs)}] {slug:<26} no html", file=sys.stderr)
            continue
        record = score(slug, args.ir_dir, args.html_dir, args.layout_dir,
                       args.guide_dir if args.guide_dir.is_dir() else None,
                       args.work, str(args.source_root),
                       roundtrip=not args.assertions_only)
        records.append(record)
        failed = [k for k in ASSERTION_KEYS if not record.get(k)]
        assertions = f"assertions {record.get('assertions_held', 0)}/8"
        if record["status"] == "ok" and record.get("rules_pct") is not None:
            print(f"[{i:>2}/{len(slugs)}] {slug:<26} "
                  f"rules {record['rules_pct']:>6}%  text {record['text_pct']:>6}%  "
                  f"{assertions}  {','.join(failed)}", file=sys.stderr)
        elif record["status"] == "ok":
            print(f"[{i:>2}/{len(slugs)}] {slug:<26} {assertions}  "
                  f"{','.join(failed)}", file=sys.stderr)
        else:
            print(f"[{i:>2}/{len(slugs)}] {slug:<26} {assertions}  "
                  f"ERROR {str(record['error'])[:60]}", file=sys.stderr)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
