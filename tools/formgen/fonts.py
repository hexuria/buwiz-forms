#!/usr/bin/env python3
"""Resolve every PDF font in the IR to a shipped CSS face, and prove the metrics.

WHY THIS EXISTS
---------------
The old pipeline rasterised the official PDF and pixel-diffed it. Because the
source PDFs do not embed all their primary faces, Poppler substituted glyph
outlines and ~57% of the residual became outline *shape* -- unwinnable. This
module replaces that test with the one that actually governs layout:
**advance width equality**. If our CSS face advances the pen by the same
distance the PDF did, for every glyph at every size, then every line breaks at
the same place, every column lands on the same rule, and the only thing left
differing is outline shape -- which nothing in this pipeline measures and
nothing needs to.

Advance equality is checkable exactly, with no raster anywhere.

WHAT IT MEASURES, AND AGAINST WHAT
----------------------------------
The IR gives two different widths per run and they answer different questions:

  * `char_widths_pt[i]` is MuPDF's per-glyph advance box: the *source font's own*
    advance for that glyph at that size, straight out of the PDF's /Widths (or
    the embedded hmtx). It is untouched by Tc/TJ. Comparing our CSS face against
    this is the true face-identity test, so it is what `metric_check` reports.
  * `measured_advance_pt` is the distance the pen actually travelled, which also
    contains the generator's tracking (this PDF uses both `Tc` operators and
    per-glyph `TJ` adjustments). Comparing a face against *that* would report a
    34pt "error" for a face that is glyph-for-glyph identical. Tracking is
    carried into CSS `letter-spacing` instead, and `tracking_check` reports the
    residual once it has been.

The IR's own `letter_spacing_pt` / `natural_advance_pt` are null for all 310
runs of 2551Q: extract.py calls `fitz.Font(fontname="Arial,Bold")`, which MuPDF
cannot resolve, so it silently gives up. This module recomputes both from the
real shipped face, which is the only face whose numbers matter anyway.

ARIAL NARROW IS ARIAL, HORIZONTALLY SCALED
------------------------------------------
Arial Narrow used to be mapped to Roboto Condensed, and the plan honestly
reported it as NOT metric-compatible: max per-glyph delta 1.358pt (0.1435em),
mean 0.263pt, 0 of 800 samples exact at PDF precision, ~9.9% too wide. That is a
real defect, not a rounding artefact -- Roboto Condensed is an independently
drawn design that happens to be condensed.

Measuring the two real faces (`/System/Library/Fonts/Supplemental/Arial.ttf` and
`Arial Narrow.ttf`) shows Arial Narrow is not an independent design at all where
advances are concerned: **every** glyph advance is the same constant multiple of
Arial's. Over the 51 distinct glyphs 2551Q actually sets in Arial Narrow the
ratio spans 0.819322..0.820738 about a mean of 0.820054 -- a maximum deviation of
0.00073, i.e. 0.007pt at the 9.48pt this document uses, the same order as the
Arimo-vs-Arial residual and far inside the 0.10pt advance tolerance.

So Arial Narrow resolves to the **same Arimo face** as Arial, with a horizontal
scale factor. This is exactly what the PDF's own `Tz` (horizontal scaling)
operator does, and it collapses the per-glyph delta from 1.358pt to 0.0058pt.

Outline *shape* is not preserved by this: a linear condensation of Arial is not
the same drawing as Arial Narrow's own. That is deliberate and is the same
trade this module already states at the top -- nothing in this pipeline measures
outline shape, and advance equality is what governs layout.

Two CSS consequences that are easy to get wrong, both encoded in the emitted
`css` block rather than left to the caller:

  * `transform` does not apply to non-replaced **inline** boxes. A scaled run
    must be `inline-block` (or absolutely positioned / block) or the browser
    silently drops the transform and the run renders 22% too wide.
  * `letter-spacing` inside a scaled box is scaled too. The authored value is
    therefore pre-divided by the scale; `css["letter-spacing"]` is what to write
    into the stylesheet, `letter_spacing_pt` is what it comes out as on paper.

THE SERIF: TIMES NEW ROMAN RESOLVES TO TINOS
--------------------------------------------
Times New Roman is not incidental in this corpus -- it sets 69,578 characters
across 1,349 runs under that spelling and 15,923 more under the /BaseFont
spelling "TimesNewRoman", which is the same design. It used to be left
UNRESOLVED because no serif was bundled, so all of that text fell through to
whatever serif the browser had and no advance in the plan described it.

It now maps to Tinos, the metric clone of Times New Roman from the same Chrome
OS core-fonts commission as Arimo, on the same Apache-2.0 terms. The mapping is
subject to the same per-glyph proof as Arial: the PDF's own /Widths for Times
New Roman are the reference, and the self-test refuses any used face whose worst
glyph advance exceeds the tolerance. Nothing is assumed from the name.

Tinos is the case that forced packages to be described by candidate paths rather
than one path: upstream Tinos is four static faces, not a variable one, so it
arrives as `@fontsource/tinos` with a file per weight instead of
`@fontsource-variable/<name>`. Static and variable then differ in two places
that are silent if got wrong -- `font-variation-settings` is meaningless without
an `fvar`, and an `@font-face` that claims `100 900` for a static regular makes
the browser synthesise the bold. Both are decided from the loaded face's own
`fvar`, never from the file name.

HOW WE GET THE SHIPPED FACE'S METRICS
-------------------------------------
The repo ships Arimo and Roboto Condensed as variable WOFF2, and MuPDF
cannot open WOFF2 ("unknown file format"), so `fitz.Font` is not usable for the
CSS side. We therefore read the WOFF2 directly: the table directory is plain,
and `hmtx`/`cmap`/`head`/`hhea`/`fvar`/`avar`/`HVAR` are all stored
*untransformed* (only `glyf`/`loca` carry the WOFF2 transform, and we never need
outlines). That gives exact advances for any weight on the `wght` axis, which is
what a browser would use. The brotli step has no stdlib equivalent, so it is
tried through four independent local providers and the plan records which one
answered; if none does, the plan is emitted with the metric proof explicitly
marked unavailable rather than silently assumed.

Usage:
    python3 tools/formgen/fonts.py --ir build/ir/2551q-2018.ir.json \
        --out build/fonts/2551q-2018.fontplan.json --summary
    python3 tools/formgen/fonts.py --self-test
"""

from __future__ import annotations

import argparse
import collections
import ctypes
import ctypes.util
import json
import pathlib
import struct
import subprocess
import sys
from typing import Any, Iterable, Sequence

SCHEMA_VERSION = 1

# The IR quantises every coordinate to 2dp, so a per-glyph advance read back out
# of it carries up to half a unit in the last place of intrinsic noise.
IR_QUANTISATION_PT = 0.005

# A PDF font's width table is integer thousandths of an em (/Widths, or /W for
# the Type0 faces here), so the PDF cannot express an advance more finely than
# half a per-mille of the point size. Arial's real 't' is 277.832/1000 em and
# the PDF stores 278; Arimo's is 277.832 too. The residual this module measures
# is therefore the PDF's own precision, not a difference between the faces --
# so the tolerance has to scale with size, not be a flat number.
PDF_WIDTH_HALF_PER_MILLE = 0.0005

# Emit CSS letter-spacing when a gap moves by this much...
LETTER_SPACING_EPSILON_PT = 0.01
# ...or when spacing below that floor still accumulates to this across the run.
# A 131-glyph run at 0.009pt of tracking per gap drifts 1.2pt if it is dropped,
# which is a whole character of error at 8pt.
LETTER_SPACING_ACCUMULATED_PT = 0.05

# Tracking tighter than this is condensed type, not incidental kerning. It is
# worth flagging because the usual cause is a wrongly chosen (too wide) face --
# though on 2551Q the per-glyph check clears the face, so it is genuine.
CONDENSED_TRACKING_EM = -0.05

# A face passes metric compatibility only if no single glyph advance differs by
# more than this. Set just above IR quantisation: nothing but rounding fits.
METRIC_COMPATIBLE_MAX_DELTA_PT = 0.05

# The self-test holds every *used* face to a tighter bar than the plan's own
# verdict threshold. With the Narrow scale in place nothing on 2551Q comes
# within a factor of 1.7 of it, so a regression shows up as a failure and not as
# a quietly-widened tolerance.
SELF_TEST_MAX_DELTA_PT = 0.01

# Arial Narrow's advances are a constant multiple of Arial's. Measured with
# fitz.Font on the macOS system faces as the width-weighted total-advance ratio
# over printable ASCII (see derive_horizontal_scale); pinned here so the
# pipeline produces byte-identical output on machines that have no Arial at all.
# Runtime derivation on this machine agrees to 5e-6, which is 5e-5 pt at 9.48pt.
ARIAL_NARROW_HORIZONTAL_SCALE = 0.820047

# Fixed, order-stable glyph set for runtime derivation. Printable ASCII only:
# it is what the form sets, and freezing it keeps the derived number reproducible.
SCALE_DERIVATION_CHARS = tuple(chr(code) for code in range(0x20, 0x7F))

# Derived scales are rounded here before use. Six places is ~1e-5 pt at these
# sizes -- below every tolerance in this module -- and makes the value printable
# and diffable rather than a float tail that shifts with the fitz build.
SCALE_DECIMALS = 6

MACOS_ARIAL = pathlib.Path("/System/Library/Fonts/Supplemental/Arial.ttf")
MACOS_ARIAL_NARROW = pathlib.Path("/System/Library/Fonts/Supplemental/Arial Narrow.ttf")


# ---------------------------------------------------------------------------
# brotli, without a brotli module
# ---------------------------------------------------------------------------


def _brotli_via_module(payload: bytes) -> bytes | None:
    try:
        import brotli  # type: ignore
    except ImportError:
        return None
    return brotli.decompress(payload)


def _brotli_via_ctypes(payload: bytes) -> bytes | None:
    """Call libbrotlidec directly if the shared library is installed."""
    candidates = [ctypes.util.find_library("brotlidec")]
    candidates += ["/opt/homebrew/lib/libbrotlidec.dylib",
                   "/usr/local/lib/libbrotlidec.dylib",
                   "libbrotlidec.so.1"]
    for name in candidates:
        if not name:
            continue
        try:
            lib = ctypes.CDLL(name)
        except OSError:
            continue
        decode = lib.BrotliDecoderDecompress
        decode.restype = ctypes.c_int
        # A font table set is small; grow only until the decoder stops asking.
        for capacity in (1 << 21, 1 << 24, 1 << 27):
            out = ctypes.create_string_buffer(capacity)
            size = ctypes.c_size_t(capacity)
            if decode(ctypes.c_size_t(len(payload)), payload,
                      ctypes.byref(size), out) == 1:
                return out.raw[:size.value]
        return None
    return None


def _brotli_via_cli(payload: bytes) -> bytes | None:
    try:
        done = subprocess.run(["brotli", "-d", "-c"], input=payload,
                              stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    except (OSError, ValueError):
        return None
    return done.stdout if done.returncode == 0 else None


def _brotli_via_node(payload: bytes) -> bytes | None:
    """Node's zlib has brotli built in, and node is this repo's own toolchain."""
    script = ("const c=[];process.stdin.on('data',d=>c.push(d))"
              ".on('end',()=>process.stdout.write("
              "require('zlib').brotliDecompressSync(Buffer.concat(c))));")
    try:
        done = subprocess.run(["node", "-e", script], input=payload,
                              stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    except (OSError, ValueError):
        return None
    return done.stdout if done.returncode == 0 else None


BROTLI_PROVIDERS = (
    ("python-brotli", _brotli_via_module),
    ("libbrotlidec", _brotli_via_ctypes),
    ("brotli-cli", _brotli_via_cli),
    ("node-zlib", _brotli_via_node),
)


class BrotliUnavailable(RuntimeError):
    pass


def brotli_decompress(payload: bytes) -> tuple[bytes, str]:
    """Decompress, returning the plaintext and the provider that produced it."""
    for name, provider in BROTLI_PROVIDERS:
        try:
            out = provider(payload)
        except Exception:  # noqa: BLE001 - a broken provider must not mask the next
            out = None
        if out:
            return out, name
    raise BrotliUnavailable(
        "no brotli decompressor available (tried "
        + ", ".join(n for n, _ in BROTLI_PROVIDERS) + ")")


# ---------------------------------------------------------------------------
# WOFF2 container
# ---------------------------------------------------------------------------

# Table tags addressable by 6-bit index in a WOFF2 directory, in spec order.
WOFF2_KNOWN_TAGS = (
    "cmap", "head", "hhea", "hmtx", "maxp", "name", "OS/2", "post", "cvt ",
    "fpgm", "glyf", "loca", "prep", "CFF ", "VORG", "EBDT", "EBLC", "gasp",
    "hdmx", "kern", "LTSH", "PCLT", "VDMX", "vhea", "vmtx", "BASE", "GDEF",
    "GPOS", "GSUB", "EBSC", "JSTF", "MATH", "CBDT", "CBLC", "COLR", "CPAL",
    "SVG ", "sbix", "acnt", "avar", "bdat", "bloc", "bsln", "cvar", "fdsc",
    "feat", "fmtx", "fvar", "gvar", "hsty", "just", "lcar", "mort", "morx",
    "opbd", "prop", "trak", "Zapf", "Silf", "Glat", "Gloc", "Feat", "Sill",
)


def read_uint_base128(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    for _ in range(5):
        byte = data[offset]
        offset += 1
        value = (value << 7) | (byte & 0x7F)
        if not byte & 0x80:
            return value, offset
    raise ValueError("malformed UIntBase128")


def read_woff2_tables(path: pathlib.Path) -> tuple[dict[str, bytes], str]:
    """Return {tag: bytes} for every table, plus the brotli provider used.

    Only `glyf` and `loca` are ever WOFF2-transformed in the shipped files, and
    this module never touches outlines, so the tables it does read come out
    byte-identical to the original SFNT.
    """
    blob = path.read_bytes()
    signature, _flavor, _length, num_tables, _res, _sfnt, compressed_len = (
        struct.unpack(">4sIIHHII", blob[:24]))
    if signature != b"wOF2":
        raise ValueError(f"not a WOFF2 file: {path}")

    offset = 48  # header is 48 bytes for a single (non-collection) font
    directory: list[tuple[str, int]] = []
    for _ in range(num_tables):
        flags = blob[offset]
        offset += 1
        index, transform = flags & 0x3F, (flags >> 6) & 3
        if index == 63:
            tag = blob[offset:offset + 4].decode("latin-1")
            offset += 4
        else:
            tag = WOFF2_KNOWN_TAGS[index]
        original_length, offset = read_uint_base128(blob, offset)
        stored_length = original_length
        transformed = (tag in ("glyf", "loca") and transform != 3) or (
            tag == "hmtx" and transform != 0)
        if transformed:
            stored_length, offset = read_uint_base128(blob, offset)
        directory.append((tag, stored_length))

    plain, provider = brotli_decompress(blob[offset:offset + compressed_len])
    tables: dict[str, bytes] = {}
    cursor = 0
    for tag, length in directory:
        tables[tag] = plain[cursor:cursor + length]
        cursor += length
    return tables, provider


# ---------------------------------------------------------------------------
# SFNT metrics (advance widths only -- no outlines are ever parsed)
# ---------------------------------------------------------------------------


def parse_cmap(table: bytes) -> dict[int, int]:
    """Unicode -> glyph id, from the best available subtable."""
    count = struct.unpack(">H", table[2:4])[0]
    preference = {(3, 10): 5, (0, 4): 4, (3, 1): 3, (0, 3): 2, (0, 6): 2}
    best: tuple[int, int] | None = None
    for i in range(count):
        platform, encoding, offset = struct.unpack(">HHI", table[4 + i * 8:12 + i * 8])
        score = preference.get((platform, encoding), 0)
        if best is None or score > best[0]:
            best = (score, offset)
    assert best is not None
    offset = best[1]
    fmt = struct.unpack(">H", table[offset:offset + 2])[0]
    mapping: dict[int, int] = {}

    if fmt == 4:
        seg_x2 = struct.unpack(">H", table[offset + 6:offset + 8])[0]
        segments = seg_x2 // 2
        end_at = offset + 14
        start_at = end_at + seg_x2 + 2
        delta_at = start_at + seg_x2
        range_at = delta_at + seg_x2
        for s in range(segments):
            end = struct.unpack(">H", table[end_at + s * 2:end_at + s * 2 + 2])[0]
            start = struct.unpack(">H", table[start_at + s * 2:start_at + s * 2 + 2])[0]
            delta = struct.unpack(">h", table[delta_at + s * 2:delta_at + s * 2 + 2])[0]
            range_offset = struct.unpack(">H", table[range_at + s * 2:range_at + s * 2 + 2])[0]
            if start == 0xFFFF:
                continue
            for code in range(start, end + 1):
                if range_offset == 0:
                    gid = (code + delta) & 0xFFFF
                else:
                    at = range_at + s * 2 + range_offset + (code - start) * 2
                    gid = struct.unpack(">H", table[at:at + 2])[0]
                    if gid:
                        gid = (gid + delta) & 0xFFFF
                if gid:
                    mapping[code] = gid
    elif fmt == 12:
        groups = struct.unpack(">I", table[offset + 12:offset + 16])[0]
        for g in range(groups):
            first, last, gid = struct.unpack(
                ">III", table[offset + 16 + g * 12:offset + 28 + g * 12])
            for code in range(first, last + 1):
                mapping[code] = gid + (code - first)
    else:
        raise ValueError(f"unsupported cmap format {fmt}")
    return mapping


def region_scalar(table: bytes, region_at: int, axis_count: int,
                  region: int, coords: Sequence[float]) -> float:
    """Blend factor of one variation region at a normalised design location."""
    scalar = 1.0
    base = region_at + 4 + region * axis_count * 6
    for axis in range(axis_count):
        start, peak, end = struct.unpack(">hhh", table[base + axis * 6:base + axis * 6 + 6])
        start, peak, end = start / 16384.0, peak / 16384.0, end / 16384.0
        value = coords[axis] if axis < len(coords) else 0.0
        if peak == 0.0 or value == peak:
            continue
        if value <= start or value >= end:
            return 0.0
        if value < peak:
            scalar *= (value - start) / (peak - start)
        else:
            scalar *= (end - value) / (end - peak)
    return scalar


class MetricFace:
    """Advance-width model of one variable font, read out of its WOFF2."""

    def __init__(self, path: pathlib.Path, tables: dict[str, bytes], provider: str) -> None:
        self.path = path
        self.brotli_provider = provider
        head, hhea, maxp, hmtx = tables["head"], tables["hhea"], tables["maxp"], tables["hmtx"]
        self.units_per_em = struct.unpack(">H", head[18:20])[0]
        self.num_glyphs = struct.unpack(">H", maxp[4:6])[0]
        self.hhea_ascender = struct.unpack(">h", hhea[4:6])[0] / self.units_per_em
        self.hhea_descender = struct.unpack(">h", hhea[6:8])[0] / self.units_per_em
        self.hhea_line_gap = struct.unpack(">h", hhea[8:10])[0] / self.units_per_em
        metric_count = struct.unpack(">H", hhea[34:36])[0]
        self.advances = [struct.unpack(">H", hmtx[i * 4:i * 4 + 2])[0]
                         for i in range(metric_count)]
        self.cmap = parse_cmap(tables["cmap"])
        self.axes = self._parse_fvar(tables["fvar"]) if "fvar" in tables else []
        self.avar = self._parse_avar(tables["avar"]) if "avar" in tables else []
        self.hvar = tables.get("HVAR")
        self.has_kern = self._has_feature(tables.get("GPOS"), "kern")
        self.has_liga = self._has_feature(tables.get("GSUB"), "liga")
        self._coord_cache: dict[float, tuple[float, ...]] = {}

    # -- variation model ----------------------------------------------------

    @staticmethod
    def _parse_fvar(table: bytes) -> list[tuple[str, float, float, float]]:
        axes_at, _size, axis_count, axis_size = struct.unpack(">HHHH", table[4:12])
        axes = []
        for i in range(axis_count):
            at = axes_at + i * axis_size
            tag = table[at:at + 4].decode("latin-1")
            minimum, default, maximum = struct.unpack(">iii", table[at + 4:at + 16])
            axes.append((tag, minimum / 65536.0, default / 65536.0, maximum / 65536.0))
        return axes

    @staticmethod
    def _parse_avar(table: bytes) -> list[list[tuple[float, float]]]:
        count = struct.unpack(">H", table[6:8])[0]
        maps: list[list[tuple[float, float]]] = []
        at = 8
        for _ in range(count):
            pairs_count = struct.unpack(">H", table[at:at + 2])[0]
            at += 2
            pairs = []
            for _ in range(pairs_count):
                source, target = struct.unpack(">hh", table[at:at + 4])
                at += 4
                pairs.append((source / 16384.0, target / 16384.0))
            maps.append(pairs)
        return maps

    @staticmethod
    def _has_feature(table: bytes | None, wanted: str) -> bool:
        if not table:
            return False
        feature_list = struct.unpack(">H", table[6:8])[0]
        count = struct.unpack(">H", table[feature_list:feature_list + 2])[0]
        return any(table[feature_list + 2 + i * 6:feature_list + 6 + i * 6]
                   .decode("latin-1") == wanted for i in range(count))

    @property
    def has_weight_axis(self) -> bool:
        """Whether this file answers more than one weight.

        Read off `fvar`, never assumed from the file name. It decides two things
        a static face gets wrong if variability is presumed:
        `font-variation-settings: "wght" N` is meaningless without the axis, and
        an `@font-face` that claims `100 900` for a static regular makes the
        browser synthesise the bold -- inventing advances nothing here measured.
        """
        return any(tag == "wght" for tag, _min, _default, _max in self.axes)

    def coords(self, weight: float) -> tuple[float, ...]:
        """Normalised design location for a CSS font-weight."""
        cached = self._coord_cache.get(weight)
        if cached is not None:
            return cached
        out: list[float] = []
        for index, (tag, minimum, default, maximum) in enumerate(self.axes):
            value = weight if tag == "wght" else default
            value = max(minimum, min(maximum, value))
            if value == default:
                normalised = 0.0
            elif value < default:
                normalised = (value - default) / (default - minimum)
            else:
                normalised = (value - default) / (maximum - default)
            if index < len(self.avar) and self.avar[index]:
                normalised = self._apply_avar(self.avar[index], normalised)
            out.append(normalised)
        self._coord_cache[weight] = tuple(out)
        return self._coord_cache[weight]

    @staticmethod
    def _apply_avar(pairs: Sequence[tuple[float, float]], value: float) -> float:
        for i in range(len(pairs) - 1):
            from_a, to_a = pairs[i]
            from_b, to_b = pairs[i + 1]
            if from_a <= value <= from_b:
                if from_b == from_a:
                    return to_a
                return to_a + (to_b - to_a) * (value - from_a) / (from_b - from_a)
        return value

    def _hvar_delta(self, gid: int, coords: Sequence[float]) -> float:
        table = self.hvar
        assert table is not None
        store_at, map_at = struct.unpack(">II", table[4:12])
        if map_at:
            outer, inner = self._delta_set_index(table, map_at, gid)
        else:
            outer, inner = 0, gid
        _fmt, region_rel, data_count = struct.unpack(">HIH", table[store_at:store_at + 8])
        if outer >= data_count:
            return 0.0
        region_at = store_at + region_rel
        axis_count, _region_count = struct.unpack(">HH", table[region_at:region_at + 4])
        data_at = struct.unpack(
            ">I", table[store_at + 8 + outer * 4:store_at + 12 + outer * 4])[0] + store_at
        _items, word_field, region_index_count = struct.unpack(">HHH", table[data_at:data_at + 6])
        long_words = bool(word_field & 0x8000)
        word_count = word_field & 0x7FFF
        regions = [struct.unpack(">H", table[data_at + 6 + i * 2:data_at + 8 + i * 2])[0]
                   for i in range(region_index_count)]
        wide, narrow = (4, 2) if long_words else (2, 1)
        row_length = word_count * wide + (region_index_count - word_count) * narrow
        at = data_at + 6 + region_index_count * 2 + inner * row_length
        total = 0.0
        for i in range(region_index_count):
            if i < word_count:
                delta = struct.unpack(">i" if long_words else ">h", table[at:at + wide])[0]
                at += wide
            else:
                delta = struct.unpack(">h" if long_words else ">b", table[at:at + narrow])[0]
                at += narrow
            if delta:
                total += delta * region_scalar(table, region_at, axis_count, regions[i], coords)
        return total

    @staticmethod
    def _delta_set_index(table: bytes, at: int, gid: int) -> tuple[int, int]:
        fmt, entry_format = table[at], table[at + 1]
        if fmt == 0:
            count = struct.unpack(">H", table[at + 2:at + 4])[0]
            base = at + 4
        else:
            count = struct.unpack(">I", table[at + 2:at + 6])[0]
            base = at + 6
        if count == 0:
            return 0, gid
        inner_bits = (entry_format & 0x0F) + 1
        entry_size = ((entry_format & 0x30) >> 4) + 1
        index = min(gid, count - 1)
        value = int.from_bytes(table[base + index * entry_size:
                                     base + index * entry_size + entry_size], "big")
        return value >> inner_bits, value & ((1 << inner_bits) - 1)

    # -- advances -----------------------------------------------------------

    def glyph_for(self, char: str) -> int:
        return self.cmap.get(ord(char), 0)

    def advance_units(self, gid: int, coords: Sequence[float]) -> float:
        units = float(self.advances[gid] if gid < len(self.advances) else self.advances[-1])
        if self.hvar is not None and any(coords):
            units += self._hvar_delta(gid, coords)
        return units

    def char_advance_pt(self, char: str, size_pt: float, weight: float) -> float:
        gid = self.glyph_for(char)
        return self.advance_units(gid, self.coords(weight)) * size_pt / self.units_per_em

    def text_length_pt(self, text: str, size_pt: float, weight: float) -> float:
        coords = self.coords(weight)
        units = sum(self.advance_units(self.glyph_for(c), coords) for c in text)
        return units * size_pt / self.units_per_em

    def covers(self, text: str) -> list[str]:
        return sorted({c for c in text if ord(c) not in self.cmap})


# ---------------------------------------------------------------------------
# Shipped-face registry
# ---------------------------------------------------------------------------

class ShippedPackage:
    """One licence-clean WOFF2 family the renderer is allowed to bundle.

    Nothing may be added here without the per-glyph advance proof below passing
    for it. Being a metric clone is a claim; `metric_check` is what licenses it.

    `css_stack` is the fallback chain, and a scaled run keeps the *same* stack on
    purpose: the transform supplies the condensation, so the fallback must be the
    wide face. Listing "Arial Narrow" in Arimo's stack would condense an
    already-condensed fallback and land at 0.67em.
    """

    __slots__ = ("key", "css_family", "css_stack")

    def __init__(self, key: str, css_family: str, css_stack: str) -> None:
        self.key = key
        self.css_family = css_family
        self.css_stack = css_stack

    def candidates(self, weight: int, style: str, subset: str) -> tuple[str, ...]:
        """node_modules-relative paths for one (weight, style, subset), in order.

        Two spellings, because fontsource publishes a family twice: as
        `@fontsource-variable/<name>` when upstream has a `wght` axis, and as
        `@fontsource/<name>` with one file per static weight when it does not.
        Arimo is the first case, Tinos the second (upstream Tinos is four
        statics), and which spelling a given checkout has installed is not this
        module's decision to make. Both are named, the first that is on disk
        wins, and the order is fixed -- so resolution is still reproducible for a
        given checkout, and a family that gains a variable release upstream needs
        no edit here.

        Whether the file that answered is actually variable is never assumed from
        this list; it is read off the loaded face's `fvar` (see
        `MetricFace.has_weight_axis`), which is the only honest source.
        """
        return (
            f"@fontsource-variable/{self.key}/files/"
            f"{self.key}-{subset}-wght-{style}.woff2",
            f"@fontsource/{self.key}/files/"
            f"{self.key}-{subset}-{weight}-{style}.woff2",
        )


# "latin-ext" is only reached for glyphs the latin subset does not cover; the
# corpus needs none of it, but the lookup order has to be fixed for determinism.
FONTSOURCE_SUBSETS = ("latin", "latin-ext")

PACKAGES = {
    "arimo": ShippedPackage(
        "arimo", "eBIRForms Arimo",
        '"eBIRForms Arimo", Arimo, Arial, Helvetica, sans-serif'),
    "roboto-condensed": ShippedPackage(
        "roboto-condensed", "eBIRForms Roboto Condensed",
        '"eBIRForms Roboto Condensed", "Roboto Condensed", "Arial Narrow", '
        'sans-serif'),
    "tinos": ShippedPackage(
        "tinos", "eBIRForms Tinos",
        '"eBIRForms Tinos", Tinos, "Times New Roman", Times, serif'),
}


class FamilyPlan:
    """How one PDF family is served, and how honest we are allowed to be about it.

    `horizontal_scale` is the CSS `scaleX` factor applied to runs in this family.
    1.0 means no transform is emitted at all, so the common case stays clean.
    """

    __slots__ = ("package", "metric_compatible", "reason", "horizontal_scale")

    def __init__(self, package: str | None, metric_compatible: bool, reason: str,
                 horizontal_scale: float = 1.0) -> None:
        self.package = package
        self.metric_compatible = metric_compatible
        self.reason = reason
        self.horizontal_scale = horizontal_scale


FAMILY_PLANS = {
    "Arial": FamilyPlan(
        "arimo", True,
        "Arimo is a metric clone of Arial (same units-per-em, same advance for "
        "every Latin glyph, same hhea ascender/descender), Apache-2.0, bundled "
        "offline. Verified per glyph below, not assumed."),
    "Arial Narrow": FamilyPlan(
        "arimo", True,
        "Arimo horizontally scaled to {scale:.6f}. Arial Narrow is not an "
        "independent design where advances are concerned: measured on the real "
        "macOS faces, every glyph advance is the same constant multiple of "
        "Arial's (0.819322..0.820738 across the 51 glyphs this form sets, max "
        "deviation 0.00073 = 0.007pt at 9.48pt). Since Arimo is a metric clone "
        "of Arial, Arimo times that constant is a metric clone of Arial Narrow, "
        "which the per-glyph check below confirms. This reproduces the PDF's own "
        "Tz horizontal-scaling operator. It was previously mapped to Roboto "
        "Condensed, an independently drawn condensed face, at a max per-glyph "
        "error of 1.358pt (0.1435em) with 0 of 800 samples exact -- that mapping "
        "is retired. Outline shape is a linear condensation of Arial rather than "
        "Arial Narrow's own drawing; this module measures advances, not outlines.",
        ARIAL_NARROW_HORIZONTAL_SCALE),
    "Times New Roman": FamilyPlan(
        "tinos", True,
        "Tinos is the metric clone of Times New Roman from the same Chrome OS "
        "core-fonts family as Arimo -- same commissioner, same brief, same "
        "Apache-2.0 licence posture as the Arimo this pipeline already bundles. "
        "The claim is not taken on faith: the per-glyph advance proof below runs "
        "against the PDF's own /Widths for Times New Roman exactly as it does for "
        "Arial, and the self-test refuses any used face whose worst glyph advance "
        "misses by more than the tolerance. This face resolves only when the "
        "package is actually installed (@fontsource/tinos, or "
        "@fontsource-variable/tinos if one is ever published); with neither on "
        "disk it stays UNRESOLVED, because substituting a sans face or an "
        "unpinned platform serif would be a silent lie about the typography."),
}

# Spellings of a family that the PDF's /BaseFont carries but that name the same
# design. Normalising here rather than adding FAMILY_PLANS entries keeps one
# plan per design, so a mapping cannot be changed for one spelling and missed
# for another. The IR's own spelling is preserved in every emitted record --
# only the plan lookup is normalised -- so a face record stays traceable to the
# /BaseFont it came from.
FAMILY_ALIASES = {
    "TimesNewRoman": "Times New Roman",
    "TimesNewRomanPSMT": "Times New Roman",
    # PostScript spellings. 1701MS is the one sheet in the corpus whose generator
    # wrote PostScript names throughout, and its seven ArialNarrow runs were the
    # last unresolved text in the corpus purely because of the missing space.
    # The style suffix is already stripped by split_font_name(), so only the
    # family stem needs an entry.
    "ArialNarrow": "Arial Narrow",
    "ArialMT": "Arial",
    "Arial-BoldMT": "Arial",
    "Arial-ItalicMT": "Arial",
    "Arial-BoldItalicMT": "Arial",
    "ArialNarrow-Bold": "Arial Narrow",
    "ArialNarrow-Italic": "Arial Narrow",
}


def plan_for(family: str) -> FamilyPlan | None:
    """The FamilyPlan governing an IR family name, through its alias if any."""
    return FAMILY_PLANS.get(FAMILY_ALIASES.get(family, family))

# Kept wired up and loadable. No family maps here today -- Arial Narrow was the
# only user and its advances proved to be Arial's -- but a genuinely independent
# condensed family in a future form has somewhere to go that is not a lie.
UNUSED_PACKAGES = ("roboto-condensed",)


def derive_horizontal_scale(narrow: pathlib.Path = MACOS_ARIAL_NARROW,
                            base: pathlib.Path = MACOS_ARIAL) -> float | None:
    """Total-advance ratio narrow/base over SCALE_DERIVATION_CHARS, or None.

    Width-weighted (sum of advances, not mean of ratios) because that is the
    quantity a line of text actually accumulates; a plain mean lets a 5-unit
    apostrophe swing the estimate as hard as a 60-unit 'm'. Returns None whenever
    the faces or fitz are missing, which is the normal case off macOS -- the
    caller then uses the pinned constant.
    """
    if not (narrow.is_file() and base.is_file()):
        return None
    try:
        import fitz  # type: ignore
    except ImportError:
        return None
    try:
        narrow_face = fitz.Font(fontfile=str(narrow))
        base_face = fitz.Font(fontfile=str(base))
    except Exception:  # noqa: BLE001 - an unreadable system face is not fatal
        return None
    narrow_total = base_total = 0.0
    for char in SCALE_DERIVATION_CHARS:
        base_advance = base_face.glyph_advance(ord(char))
        if base_advance <= 0:
            continue
        base_total += base_advance
        narrow_total += narrow_face.glyph_advance(ord(char))
    if base_total <= 0:
        return None
    return round(narrow_total / base_total, SCALE_DECIMALS)


def scale_overrides(derive: bool) -> tuple[dict[str, float], dict[str, Any]]:
    """Per-family scale overrides plus the provenance record for the plan.

    Derivation is opt-in, not automatic-when-available. Automatic would make the
    plan's contents depend on whether the machine happens to ship Arial, and
    byte-identical output for identical input is a hard rule of this pipeline.
    """
    pinned = {"source": "pinned-constant",
              "arial_narrow_scale": ARIAL_NARROW_HORIZONTAL_SCALE,
              "derivation_available": False,
              "derived_arial_narrow_scale": None,
              "base_font": None,
              "narrow_font": None,
              "note": "pinned so output is byte-identical on machines without "
                      "the system faces; pass --derive-narrow-scale to measure"}
    if not derive:
        return {}, pinned
    derived = derive_horizontal_scale()
    if derived is None:
        pinned["note"] = ("--derive-narrow-scale requested but the macOS system "
                          "faces or fitz were unavailable; fell back to the pin")
        return {}, pinned
    return {"Arial Narrow": derived}, {
        "source": "derived-from-system-faces",
        "arial_narrow_scale": derived,
        "derivation_available": True,
        "derived_arial_narrow_scale": derived,
        "base_font": str(MACOS_ARIAL),
        "narrow_font": str(MACOS_ARIAL_NARROW),
        "pinned_arial_narrow_scale": ARIAL_NARROW_HORIZONTAL_SCALE,
        # r4 would round this whole quantity away to -0.0; the point of the field
        # is to show how small it is, so it keeps the scale's own precision.
        "pin_delta": round(derived - ARIAL_NARROW_HORIZONTAL_SCALE, SCALE_DECIMALS + 2),
        "note": "width-weighted total-advance ratio over printable ASCII; this "
                "plan is machine-dependent and is not the pipeline default",
    }


def find_fonts_root(start: pathlib.Path) -> pathlib.Path | None:
    """Nearest ancestor holding node_modules/@fontsource-variable.

    A git worktree has no node_modules of its own, so this walks up into the
    main checkout. The search is a fixed upward walk, so it is deterministic.
    """
    for candidate in [start.resolve(), *start.resolve().parents]:
        if (candidate / "node_modules" / "@fontsource-variable").is_dir():
            return candidate
    return None


def npm_package_of(relative: str) -> str:
    """'@scope/name' from a node_modules-relative font path."""
    parts = pathlib.PurePosixPath(relative).parts
    return "/".join(parts[:2])


class FaceLibrary:
    """Lazily loads shipped faces and remembers which brotli provider answered."""

    def __init__(self, fonts_root: pathlib.Path) -> None:
        self.fonts_root = fonts_root
        self._cache: dict[tuple[str, int, str, str], MetricFace | None] = {}
        self.provider: str | None = None
        self.error: str | None = None

    def load(self, package: str, weight: int, style: str,
             extended: bool = False) -> MetricFace | None:
        """The shipped face for one (package, weight, style), or None.

        Weight is part of the key because a static family keeps each weight in
        its own file; a variable family answers every weight from one file and
        simply resolves both keys to it. Misses are cached too, so a package
        that is not installed is stat'ed once rather than once per face.
        """
        subset = FONTSOURCE_SUBSETS[1] if extended else FONTSOURCE_SUBSETS[0]
        key = (package, weight, style, subset)
        if key in self._cache:
            return self._cache[key]
        spec = PACKAGES.get(package)
        if spec is None:
            return None
        candidates = spec.candidates(weight, style, subset)
        path = next((self.fonts_root / "node_modules" / relative
                     for relative in candidates
                     if (self.fonts_root / "node_modules" / relative).is_file()), None)
        if path is None:
            self.error = (
                "no shipped font file for "
                + f"{package} {weight} {style} ({subset}); install "
                + " or ".join(sorted({npm_package_of(c) for c in candidates}))
                + " -- tried " + ", ".join(candidates))
            self._cache[key] = None
            return None
        try:
            tables, provider = read_woff2_tables(path)
        except BrotliUnavailable as exc:
            self.error = str(exc)
            return None
        self.provider = provider
        face = MetricFace(path, tables, provider)
        self._cache[key] = face
        return face

    def relative_path(self, face: MetricFace) -> str:
        return str(face.path.relative_to(self.fonts_root))

    def npm_package(self, face: MetricFace) -> str:
        return npm_package_of(
            str(face.path.relative_to(self.fonts_root / "node_modules")))


# ---------------------------------------------------------------------------
# Run classification
# ---------------------------------------------------------------------------


def used_faces(ir: dict[str, Any]) -> list[tuple[str, bool, bool]]:
    """Every (family, bold, italic) triple actually carried by a text run."""
    seen = {(run["family"], bool(run["bold"]), bool(run["italic"]))
            for page in ir["pages"] for run in page["text_runs"]}
    return sorted(seen)


def css_weight(bold: bool) -> int:
    return 700 if bold else 400


def face_key(family: str, bold: bool, italic: bool) -> str:
    return f"{family}|{css_weight(bold)}|{'italic' if italic else 'normal'}"


def format_pt(value: float) -> str:
    """Deterministic pt literal: fixed precision, trailing zeros stripped."""
    text = f"{value:.4f}".rstrip("0").rstrip(".")
    if text in ("", "-0", "0"):
        return "0pt"
    return f"{text}pt"


def r4(value: float) -> float:
    return round(float(value) + 0.0, 4)


def format_scale(value: float) -> str:
    return f"{value:.6f}".rstrip("0").rstrip(".")


def transform_css(scale: float) -> dict[str, Any]:
    """The `scaleX` block for a run, or explicit nulls when nothing is scaled.

    The keys are always present so a consumer can write the block unconditionally
    and so a diff of two plans shows a scale appearing rather than a key.

    `transform-origin` MUST resolve to the run's left edge, which is the pen
    origin the PDF started the run at. Scale about anything else (the default is
    the box centre) and every glyph in the run, including the first, shifts by
    half the width the run lost -- for a 100pt run that is 9pt of error placed
    exactly where verify.py compares glyph origins.

    The vertical component is written as the baseline for intent, but is
    genuinely immaterial: scaleX maps (x, y) to (k*x, y) whatever the origin's y
    is. Only the horizontal component can be got wrong.

    `display` is not decoration. CSS does not apply `transform` to non-replaced
    inline boxes, so a run left as plain inline silently renders unscaled -- a
    22% width error that looks like a font bug rather than a missing property.
    """
    if scale == 1.0:
        return {"transform": None, "transform-origin": None, "display": None}
    return {
        "transform": f"scaleX({format_scale(scale)})",
        "transform-origin": "0% 0%",
        "display": "inline-block",
    }


# ---------------------------------------------------------------------------
# The metric proof
# ---------------------------------------------------------------------------


def precision_bound_pt(size_pt: float) -> float:
    """Largest advance delta that is pure measurement precision, not difference."""
    return IR_QUANTISATION_PT + PDF_WIDTH_HALF_PER_MILLE * size_pt


class GlyphSample:
    """One glyph advance, ours against the PDF's."""

    __slots__ = ("delta", "char", "size_pt", "expected", "actual", "run_text", "per_mille_exact")

    def __init__(self, char: str, size_pt: float, expected: float, actual: float,
                 run_text: str) -> None:
        self.char = char
        self.size_pt = size_pt
        self.expected = expected
        self.actual = actual
        self.delta = abs(expected - actual)
        self.run_text = run_text
        # Round our advance the way a PDF width table must, then quantise the
        # way the IR does. If that reproduces the PDF's number exactly, the two
        # faces are identical to the last digit the PDF can carry.
        per_mille = round(expected / size_pt * 1000.0) if size_pt else 0.0
        self.per_mille_exact = bool(
            abs(round(per_mille / 1000.0 * size_pt, 2) - actual) < 1e-9)

    @property
    def excess(self) -> float:
        return self.delta - precision_bound_pt(self.size_pt)


class FaceEvidence:
    """Accumulates per-glyph and per-run evidence for one resolved face."""

    def __init__(self) -> None:
        self.glyphs: list[GlyphSample] = []
        self.run_natural: list[tuple[float, dict[str, Any], float]] = []
        self.run_residual: list[tuple[float, dict[str, Any]]] = []
        self.tracking_em: list[float] = []
        self.runs = 0

    def summarise_glyphs(self) -> dict[str, Any]:
        if not self.glyphs:
            return {"samples": 0, "max_advance_delta_pt": None,
                    "mean_advance_delta_pt": None, "worst": None}
        worst = max(self.glyphs, key=lambda s: (s.delta, s.char, s.size_pt))
        worst_excess = max(self.glyphs, key=lambda s: (s.excess, s.char, s.size_pt))
        total = sum(sample.delta for sample in self.glyphs)
        exact = sum(1 for sample in self.glyphs if sample.per_mille_exact)
        return {
            "basis": "per-glyph advance: CSS face vs the PDF font's own advance "
                     "(IR char_widths_pt); free of Tc/TJ tracking",
            "samples": len(self.glyphs),
            "max_advance_delta_pt": r4(worst.delta),
            "mean_advance_delta_pt": r4(total / len(self.glyphs)),
            "max_advance_delta_em": r4(max(s.delta / s.size_pt for s in self.glyphs
                                           if s.size_pt)),
            "precision_bound_pt_at_worst": r4(precision_bound_pt(worst.size_pt)),
            "max_excess_over_precision_pt": r4(worst_excess.excess),
            "within_measurement_precision": bool(worst_excess.excess <= 1e-9),
            "per_mille_exact_samples": exact,
            "per_mille_exact_fraction": r4(exact / len(self.glyphs)),
            # How much wider or narrower our face runs, glyph for glyph. 1.0 is
            # a clone; the number is the honest size of a substitution error.
            "mean_width_ratio": r4(
                sum(s.expected / s.actual for s in self.glyphs if s.actual)
                / max(1, sum(1 for s in self.glyphs if s.actual))),
            "worst": {
                "text": worst.char,
                "size_pt": r4(worst.size_pt),
                "expected_pt": r4(worst.expected),
                "actual_pt": r4(worst.actual),
                "delta_pt": r4(worst.delta),
                "in_run": worst.run_text,
            },
        }


def evaluate(ir: dict[str, Any], library: FaceLibrary,
             overrides: dict[str, float] | None = None,
             scale_provenance: dict[str, Any] | None = None) -> dict[str, Any]:
    """Build the whole font plan: face resolution, metric proof, per-run CSS."""
    overrides = overrides or {}
    if scale_provenance is None:
        _, scale_provenance = scale_overrides(False)
    warnings: list[str] = []
    evidence: dict[str, FaceEvidence] = collections.defaultdict(FaceEvidence)
    resolved: dict[tuple[str, bool, bool], MetricFace | None] = {}
    packages: dict[tuple[str, bool, bool], str | None] = {}
    scales: dict[tuple[str, bool, bool], float] = {}
    run_entries: list[dict[str, Any]] = []

    def family_scale(family: str) -> float:
        plan = plan_for(family)
        return overrides.get(family, plan.horizontal_scale if plan else 1.0)

    triples = used_faces(ir)
    for family, bold, italic in triples:
        plan = plan_for(family)
        if plan is None:
            resolved[(family, bold, italic)] = None
            packages[(family, bold, italic)] = None
            warnings.append(
                f"UNRESOLVED face {family!r} (weight {css_weight(bold)}, "
                f"{'italic' if italic else 'normal'}): no mapping is defined and no "
                f"substitute may be invented; add a pinned licence-clean face or "
                f"prove no visible text uses it")
            continue
        if plan.package is None:
            resolved[(family, bold, italic)] = None
            packages[(family, bold, italic)] = None
            warnings.append(f"UNRESOLVED face {family!r}: {plan.reason}")
            continue
        style = "italic" if italic else "normal"
        face = library.load(plan.package, css_weight(bold), style)
        packages[(family, bold, italic)] = plan.package
        resolved[(family, bold, italic)] = face
        scales[(family, bold, italic)] = family_scale(family)
        if face is None:
            # A mapping that exists but cannot be loaded is still UNRESOLVED, not
            # a silent fallback: the run below is emitted with no CSS at all
            # rather than with a family whose file we do not ship.
            detail = library.error or "the shipped face could not be read"
            warnings.append(
                f"UNRESOLVED face {family!r} (weight {css_weight(bold)}, {style}): "
                f"it maps to {plan.package} but {detail}. Until that is fixed this "
                f"text has no resolved face; no other family may be substituted.")

    # -- walk every run: metric samples, tracking, CSS ------------------------
    for page in ir["pages"]:
        for index, run in enumerate(page["text_runs"]):
            key = (run["family"], bool(run["bold"]), bool(run["italic"]))
            weight = css_weight(run["bold"])
            face = resolved.get(key)
            plan = plan_for(run["family"])
            package = packages.get(key)
            size = float(run["size_pt"])
            text = run["text"]
            fk = face_key(run["family"], run["bold"], run["italic"])
            entry: dict[str, Any] = {
                "run_index": index,
                "page": page["index"],
                "face_key": fk,
                "text_preview": text[:48],
                "chars": len(text),
            }

            if face is None:
                entry["css"] = None
                entry["unresolved"] = True
                run_entries.append(entry)
                continue

            store = evidence[fk]
            store.runs += 1
            missing = face.covers(text)
            if missing:
                warnings.append(
                    f"page {page['index']} run {index}: shipped face "
                    f"{PACKAGES[package].css_family} does not cover "
                    f"{''.join(missing)!r}; advances for those glyphs are .notdef")

            # Every advance below is in page space, i.e. already through the
            # scaleX. That keeps `natural`, the tracking and the residual
            # directly comparable to the IR's pt measurements; only the authored
            # CSS values have to be divided back out.
            scale = scales.get(key, 1.0)
            natural = 0.0
            for char, pdf_width in zip(text, run["char_widths_pt"]):
                css_width = face.char_advance_pt(char, size, weight) * scale
                natural += css_width
                store.glyphs.append(
                    GlyphSample(char, size, css_width, float(pdf_width), text[:32]))

            measured = float(run["measured_advance_pt"])
            store.run_natural.append((abs(natural - measured), run, natural))

            # Tracking: what the generator added on top of the face's own
            # advances (this PDF uses Tc plus per-glyph TJ nudges). CSS
            # letter-spacing is the only lever that reproduces it.
            gaps = len(text) - 1
            spacing = (measured - natural) / gaps if gaps > 0 else 0.0
            emitted = None
            if (abs(spacing) >= LETTER_SPACING_EPSILON_PT
                    or abs(spacing) * gaps >= LETTER_SPACING_ACCUMULATED_PT):
                # letter-spacing lives inside the scaled box and is scaled with
                # it, so author the pre-image and let the transform land it on
                # `spacing`. Round after dividing, not before.
                emitted = r4(spacing / scale)
            applied = (emitted or 0.0) * scale * gaps
            store.run_residual.append((abs(natural + applied - measured), run))
            store.tracking_em.append(spacing / size if size else 0.0)

            if spacing / size <= CONDENSED_TRACKING_EM and size:
                warnings.append(
                    f"page {page['index']} run {index}: condensed tracking "
                    f"{spacing / size:+.4f}em ({format_pt(spacing)} at "
                    f"{format_pt(size)}) on {text[:32]!r} -- verify this is real "
                    f"tracking and not a too-wide substitute face")

            css = {
                "font-family": PACKAGES[package].css_stack,
                "font-size": format_pt(size),
                "font-weight": weight,
                "font-style": "italic" if run["italic"] else "normal",
                "letter-spacing": format_pt(emitted) if emitted is not None else None,
                "line-height": format_pt(float(run["line_height_pt"])),
                # The advance model above is a plain sum of hmtx advances. Every
                # shipped family carries GPOS `kern` (and some also GSUB `liga`),
                # which Chromium applies by default and which would silently
                # invalidate every number in this plan.
                "font-kerning": "none",
                "font-variant-ligatures": "none",
                "font-feature-settings": "normal",
                # Only meaningful for a face that actually has the axis. Sent to
                # a static face it is inert at best and misleading at worst: it
                # reads as "this weight is interpolated" when the weight in fact
                # comes from a separate file.
                "font-variation-settings": (f'"wght" {weight}'
                                            if face.has_weight_axis else None),
            }
            css.update(transform_css(scale))
            entry["css"] = css
            entry["horizontal_scale"] = scale
            entry["natural_advance_pt"] = r4(natural)
            entry["measured_advance_pt"] = r4(measured)
            # Authored (pre-transform) vs what it measures on paper. Equal
            # whenever scale is 1.0, which is every run of an unscaled family.
            entry["letter_spacing_css_pt"] = emitted
            entry["letter_spacing_pt"] = r4(emitted * scale) if emitted is not None else None
            entry["letter_spacing_em"] = r4(spacing / size) if size else None
            entry["width_residual_pt"] = r4(natural + applied - measured)
            run_entries.append(entry)

    faces_out = [_face_record(family, bold, italic, resolved[(family, bold, italic)],
                             packages[(family, bold, italic)], evidence, library, ir,
                             family_scale(family))
                 for family, bold, italic in triples]

    faces_out.extend(_declared_unused_records(ir, triples, warnings))

    if library.provider is None:
        warnings.append(
            "no shipped face could be read; the metric proof in this plan is "
            "absent, not passing")

    for record in faces_out:
        check = record.get("metric_check") or {}
        if record["metric_compatible"] and check.get("max_advance_delta_pt") is not None:
            if check["max_advance_delta_pt"] > METRIC_COMPATIBLE_MAX_DELTA_PT:
                warnings.append(
                    f"REFUTED: {record['face_key']} claims metric compatibility but "
                    f"its worst glyph advance is off by "
                    f"{check['max_advance_delta_pt']}pt (limit "
                    f"{METRIC_COMPATIBLE_MAX_DELTA_PT}pt)")

    return {
        "schema_version": SCHEMA_VERSION,
        "form": ir["form"],
        "source": {"ir_sha256_of_pdf": ir["source"]["sha256"],
                   "pdf": ir["source"]["file"]},
        "generator": {
            "producer": "tools/formgen/fonts.py",
            "schema_version": SCHEMA_VERSION,
            "metric_source": "fontsource variable WOFF2 hmtx + HVAR "
                             "(no raster, no outlines)",
            "brotli_provider": library.provider,
            "fonts_root": str(library.fonts_root),
            "letter_spacing_model":
                "spacing is derived per run as (measured_advance - CSS natural "
                "advance) / (glyphs - 1), i.e. glyph-origin semantics. Blink adds "
                "letter-spacing after the final glyph too, so an inline box "
                "measures one spacing unit wider than measured_advance_pt; every "
                "glyph origin still lands where the PDF put it, which is what "
                "verify.py compares. Do not right-align a spaced run.",
            "shaping": "font-kerning and ligatures are disabled in every emitted "
                       "CSS block; both shipped families carry GPOS kern and the "
                       "advance proof is a plain sum of hmtx advances",
            "horizontal_scale": scale_provenance,
            "horizontal_scale_model":
                "a face may carry a scaleX factor: the CSS face supplies the "
                "outlines and the transform supplies the width, which is what "
                "the PDF's Tz operator does. The run element must be "
                "inline-block (transform does not apply to non-replaced inline "
                "boxes) and transform-origin must be the run's left edge, or "
                "every glyph origin in the run shifts. Advances reported here "
                "(natural_advance_pt, letter_spacing_pt, width_residual_pt) are "
                "post-transform page space; css['letter-spacing'] is the "
                "authored pre-transform value, already divided by the scale "
                "because letter-spacing inside a scaled box is scaled too.",
        },
        "faces": faces_out,
        "runs": run_entries,
        "warnings": sorted(set(warnings)),
    }


def _face_record(family: str, bold: bool, italic: bool, face: MetricFace | None,
                 package: str | None, evidence: dict[str, FaceEvidence],
                 library: FaceLibrary, ir: dict[str, Any],
                 horizontal_scale: float = 1.0) -> dict[str, Any]:
    plan = plan_for(family)
    fk = face_key(family, bold, italic)
    store = evidence.get(fk, FaceEvidence())
    basefonts = sorted({name for name, info in ir["fonts"].items()
                        if info["family"] == family
                        and bool(info["declared_bold"]) == bold
                        and bool(info["declared_italic"]) == italic})
    record: dict[str, Any] = {
        "face_key": fk,
        "basefont": basefonts,
        "family": family,
        "bold": bold,
        "italic": italic,
        "status": "resolved" if face is not None else "unresolved",
        # A face with no file must not advertise a CSS family: a consumer that
        # wrote it out would name a font the bundle does not ship, and Chromium
        # would answer with a platform face nothing here has measured.
        "css_family": PACKAGES[package].css_family if package and face else None,
        "css_family_stack": PACKAGES[package].css_stack if package and face else None,
        "css_weight": css_weight(bold),
        "css_style": "italic" if italic else "normal",
        # Which spelling of the package answered is a fact about this checkout,
        # so it is read off the file that loaded rather than assumed.
        "fontsource_package": library.npm_package(face) if face is not None else None,
        "required_packages": _required_packages(package, bold, italic),
        "font_file": library.relative_path(face) if face is not None else None,
        "embedded": _embedded_flag(ir, basefonts),
        # A statement about the face this plan *emits*, so a face with no file
        # can never carry it: the mapping's claim is worth nothing until there
        # is a font to attach it to and per-glyph evidence beside it.
        "metric_compatible": bool(plan and plan.metric_compatible and face is not None),
        "substitution_reason": (plan.reason.format(scale=horizontal_scale) if plan
                               else "no mapping defined for this family"),
        "horizontal_scale": horizontal_scale,
        "css_transform": transform_css(horizontal_scale),
        "used_runs": store.runs,
        "used_glyphs": len(store.glyphs),
    }
    if face is None:
        record["font_face"] = None
        record["metric_check"] = {"samples": 0, "max_advance_delta_pt": None,
                                  "mean_advance_delta_pt": None, "worst": None}
        record["tracking_check"] = None
        record["vertical_metrics"] = None
        return record

    record["font_face"] = _font_face_record(record, face, library)
    record["metric_check"] = store.summarise_glyphs()
    record["tracking_check"] = _tracking_record(store)
    record["vertical_metrics"] = _vertical_record(ir, family, face)
    record["shaping_features"] = {
        "gpos_kern": face.has_kern,
        "gsub_liga": face.has_liga,
        "note": "disabled in the emitted CSS; the advance proof is a plain sum "
                "of hmtx advances and shaping would invalidate it",
    }
    return record


def _required_packages(package: str | None, bold: bool, italic: bool) -> list[str]:
    """npm packages that could serve this face, whether or not one is installed.

    Emitted for unresolved faces too -- that is where it earns its keep, because
    it turns "UNRESOLVED" into an instruction. Both fontsource spellings are
    listed for the same reason `ShippedPackage.candidates` offers both.
    """
    if package is None:
        return []
    spec = PACKAGES[package]
    return sorted({npm_package_of(candidate)
                   for candidate in spec.candidates(
                       css_weight(bold), "italic" if italic else "normal",
                       FONTSOURCE_SUBSETS[0])})


def _font_face_record(record: dict[str, Any], face: MetricFace,
                      library: FaceLibrary) -> dict[str, Any]:
    """The exact `@font-face` descriptors this file may be declared with.

    The weight descriptor is the part that has to be derived rather than fixed.
    A variable file legitimately claims the whole `100 900` range; a static file
    must claim only the weight it draws, or the browser picks the one file it has
    for every weight and synthesises the rest -- emboldening outlines and
    inventing advances that no measurement in this plan covers.
    """
    return {
        "family": record["css_family"],
        "style": record["css_style"],
        "weight": "100 900" if face.has_weight_axis else str(record["css_weight"]),
        "file": library.relative_path(face),
        "variable": face.has_weight_axis,
    }


def _embedded_flag(ir: dict[str, Any], basefonts: Sequence[str]) -> bool | None:
    flags = {bool(ir["fonts"][name]["embedded"]) for name in basefonts}
    if len(flags) == 1:
        return flags.pop()
    return None


def _tracking_record(store: FaceEvidence) -> dict[str, Any]:
    if not store.run_natural:
        return {"runs": 0}
    worst_natural = max(store.run_natural, key=lambda item: item[0])
    worst_residual = max(store.run_residual, key=lambda item: item[0])
    return {
        "basis": "run advance: CSS face natural width vs IR measured_advance_pt; "
                 "the gap is the generator's Tc/TJ tracking, not a face mismatch",
        "runs": len(store.run_natural),
        "max_untracked_delta_pt": r4(worst_natural[0]),
        "mean_untracked_delta_pt": r4(
            sum(item[0] for item in store.run_natural) / len(store.run_natural)),
        "worst_untracked": {
            "text": worst_natural[1]["text"][:48],
            "size_pt": r4(worst_natural[1]["size_pt"]),
            "expected_pt": r4(worst_natural[2]),
            "actual_pt": r4(worst_natural[1]["measured_advance_pt"]),
        },
        "letter_spacing_em": {
            "min": r4(min(store.tracking_em)),
            "max": r4(max(store.tracking_em)),
            "mean": r4(sum(store.tracking_em) / len(store.tracking_em)),
        },
        "max_residual_after_letter_spacing_pt": r4(worst_residual[0]),
        "worst_residual_text": worst_residual[1]["text"][:48],
    }


def _vertical_record(ir: dict[str, Any], family: str, face: MetricFace) -> dict[str, Any]:
    """Line-box metrics: the 'custom spacing' half of the brief.

    line-height is emitted in absolute pt, so the box height is face-independent
    by construction. The ascender/descender comparison still matters because it
    decides where the baseline sits inside that box (half-leading).
    """
    samples = {(r["ascender"], r["descender"])
               for page in ir["pages"] for r in page["text_runs"]
               if r["family"] == family}
    pdf_ascender, pdf_descender = sorted(samples)[0] if samples else (None, None)
    record = {
        "pdf_ascender": pdf_ascender,
        "pdf_descender": pdf_descender,
        "css_hhea_ascender": r4(face.hhea_ascender),
        "css_hhea_descender": r4(face.hhea_descender),
        "css_hhea_line_gap": r4(face.hhea_line_gap),
        "line_height_emitted": "absolute pt per run (IR line_height_pt); never "
                               "left to the browser default, which is face-derived",
    }
    if pdf_ascender is not None:
        record["ascender_delta"] = r4(face.hhea_ascender - pdf_ascender)
        record["descender_delta"] = r4(face.hhea_descender - pdf_descender)
        record["baseline_matches"] = bool(
            abs(record["ascender_delta"]) <= 0.005
            and abs(record["descender_delta"]) <= 0.005)
    return record


def _declared_unused_records(ir: dict[str, Any],
                             triples: Sequence[tuple[str, bool, bool]],
                             warnings: list[str]) -> list[dict[str, Any]]:
    """Families present in the PDF resources that no visible run uses.

    2551Q declares Times New Roman and only ever sets it for whitespace-only
    footer artifacts, which extract.py drops. That is not a resolved face and it
    is not a failure either -- it is a fact that has to be stated, because the
    moment a revision puts real text in it the plan must fail instead of guess.
    """
    used = {family for family, _b, _i in triples}
    out: list[dict[str, Any]] = []
    for name, info in sorted(ir["fonts"].items()):
        family = info["family"]
        if family in used:
            continue
        plan = plan_for(family)
        out.append({
            "face_key": f"{family}|declared-unused",
            "basefont": [name],
            "family": family,
            "bold": bool(info["declared_bold"]),
            "italic": bool(info["declared_italic"]),
            "status": "declared-unused",
            "css_family": None,
            "css_family_stack": None,
            "css_weight": css_weight(bool(info["declared_bold"])),
            "css_style": "italic" if info["declared_italic"] else "normal",
            "fontsource_package": None,
            "required_packages": _required_packages(
                plan.package if plan else None, bool(info["declared_bold"]),
                bool(info["declared_italic"])),
            "font_file": None,
            "font_face": None,
            "embedded": bool(info["embedded"]),
            "metric_compatible": False,
            "substitution_reason": (plan.reason.format(scale=plan.horizontal_scale)
                                    if plan else
                                    "no mapping defined for this family")
                                   + " Declared in the page resources but used by "
                                     "no visible text run in this revision.",
            "used_runs": 0,
            "used_glyphs": 0,
            "metric_check": {"samples": 0, "max_advance_delta_pt": None,
                             "mean_advance_delta_pt": None, "worst": None},
            "tracking_check": None,
            "vertical_metrics": None,
        })
        warnings.append(
            f"{family!r} is declared in the PDF font resources but no visible text "
            f"run uses it; it is deliberately left UNRESOLVED. If a later revision "
            f"sets visible text in it, this plan must fail rather than substitute.")
    return out


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------


def print_face_table(plan: dict[str, Any], stream: Any = sys.stderr) -> None:
    header = (f"{'face':34} {'css face':22} {'scaleX':>8} {'runs':>5} {'glyphs':>7} "
              f"{'max Δpt':>9} {'mean Δpt':>9}  verdict")
    print(header, file=stream)
    print("-" * len(header), file=stream)
    for face in plan["faces"]:
        check = face["metric_check"]
        maximum = check["max_advance_delta_pt"]
        mean = check["mean_advance_delta_pt"]
        if face["status"] != "resolved":
            verdict = face["status"].upper()
        elif not face["metric_compatible"]:
            verdict = "SUBSTITUTE (not metric-compatible)"
        elif check["within_measurement_precision"]:
            verdict = (f"IDENTICAL ({check['per_mille_exact_fraction']:.0%} of glyphs "
                       f"exact at PDF per-mille precision)")
        elif maximum is not None and maximum <= METRIC_COMPATIBLE_MAX_DELTA_PT:
            verdict = "compatible"
        else:
            verdict = "REFUTED"
        scale = face.get("horizontal_scale", 1.0)
        print(f"{face['face_key']:34} {str(face['css_family'] or '-'):22} "
              f"{'-' if scale == 1.0 else format_scale(scale):>8} "
              f"{face['used_runs']:5} {face['used_glyphs']:7} "
              f"{'-' if maximum is None else f'{maximum:9.4f}'} "
              f"{'-' if mean is None else f'{mean:9.4f}'}  {verdict}", file=stream)

    print("", file=stream)
    for face in plan["faces"]:
        track = face.get("tracking_check")
        if not track:
            continue
        spacing = track["letter_spacing_em"]
        print(f"{face['face_key']:34} tracking {spacing['min']:+.4f}..{spacing['max']:+.4f}em "
              f"(mean {spacing['mean']:+.4f}em)  residual after letter-spacing "
              f"{track['max_residual_after_letter_spacing_pt']:.4f}pt", file=stream)


# ---------------------------------------------------------------------------
# Self-test
# ---------------------------------------------------------------------------

SELF_TEST_IR = pathlib.Path("build/ir/2551q-2018.ir.json")


def self_test(ir_path: pathlib.Path, fonts_root: pathlib.Path | None) -> int:
    if not ir_path.is_file():
        print(f"self-test needs the real IR at {ir_path}", file=sys.stderr)
        return 2
    ir = json.loads(ir_path.read_text(encoding="utf-8"))
    if fonts_root is None:
        print("self-test needs node_modules/@fontsource-variable", file=sys.stderr)
        return 2

    library = FaceLibrary(fonts_root)
    plan = evaluate(ir, library)
    failures: list[str] = []

    if plan["generator"]["brotli_provider"] is None:
        failures.append("no brotli provider: the metric proof did not run at all")

    # 1. Every used face either resolves or is explicitly warned about.
    for face in plan["faces"]:
        if face["status"] == "resolved":
            continue
        if not any(face["family"] in warning for warning in plan["warnings"]):
            failures.append(f"{face['face_key']} is unresolved with no warning")

    # 2. THE headline assertion. If Arimo's advances ever drift from Arial's,
    #    the whole substitution premise is dead -- for Arial Narrow too, which
    #    is now the same face under a transform. Delta itself is checked in 4;
    #    what is specific to Arial is that the baseline must line up as well.
    arial = [f for f in plan["faces"]
             if f["family"] == "Arial" and f["status"] == "resolved"]
    if len(arial) != 3:
        failures.append(f"expected 3 resolved Arial faces, got {len(arial)}")
    for face in arial:
        vertical = face["vertical_metrics"]
        if not vertical["baseline_matches"]:
            failures.append(
                f"{face['face_key']}: hhea ascender/descender differ from the PDF "
                f"font ({vertical['ascender_delta']}, {vertical['descender_delta']})")

    # 3. Arial Narrow now resolves to scaled Arimo, so it is held to the same
    #    bar as everything else -- and the scale must actually be emitted.
    narrow = [f for f in plan["faces"] if f["family"] == "Arial Narrow"
              and f["status"] == "resolved"]
    if len(narrow) != 2:
        failures.append(f"expected 2 resolved Arial Narrow faces, got {len(narrow)}")
    for face in narrow:
        if face["fontsource_package"] != "@fontsource-variable/arimo":
            failures.append(
                f"{face['face_key']}: Arial Narrow must resolve to Arimo plus a "
                f"scaleX, not to {face['fontsource_package']}")
        if not 0.5 < face["horizontal_scale"] < 1.0:
            failures.append(
                f"{face['face_key']}: horizontal_scale {face['horizontal_scale']} "
                f"is not a condensation")
        transform = face["css_transform"]
        if not transform["transform"] or transform["display"] != "inline-block":
            failures.append(
                f"{face['face_key']}: a scaled face must emit both a scaleX "
                f"transform and display:inline-block, got {transform}")

    # 4. EVERY used face is metric compatible within SELF_TEST_MAX_DELTA_PT.
    #    This is the assertion the Narrow fix bought; it was unmeetable while
    #    Arial Narrow sat at 1.358pt against Roboto Condensed.
    used = [f for f in plan["faces"] if f["status"] == "resolved"]
    if not used:
        failures.append("no resolved faces at all")
    for face in used:
        worst = face["metric_check"]["max_advance_delta_pt"]
        if not face["metric_compatible"]:
            failures.append(
                f"{face['face_key']} is used by {face['used_runs']} runs but is "
                f"not metric compatible")
        if worst is None or worst > SELF_TEST_MAX_DELTA_PT:
            failures.append(
                f"{face['face_key']}: worst glyph advance delta {worst}pt exceeds "
                f"{SELF_TEST_MAX_DELTA_PT}pt. Fix the face mapping -- never widen "
                f"this tolerance.")
        if not face["metric_check"]["within_measurement_precision"]:
            failures.append(
                f"{face['face_key']}: delta {worst}pt exceeds what IR quantisation "
                f"plus the PDF's per-mille width table can explain (excess "
                f"{face['metric_check']['max_excess_over_precision_pt']}pt)")

    # 4b. Whatever a resolved face claims about the wght axis has to match the
    #     file, in both directions: a variable file must be told to vary and a
    #     static one must not, and the @font-face weight descriptor must not
    #     invite the browser to synthesise a weight nothing here measured.
    for face in used:
        font_face = face.get("font_face") or {}
        variable = bool(font_face.get("variable"))
        if font_face.get("weight") != ("100 900" if variable else str(face["css_weight"])):
            failures.append(
                f"{face['face_key']}: @font-face weight {font_face.get('weight')!r} "
                f"does not match a {'variable' if variable else 'static'} file; a "
                f"static face declared over a range makes the browser synthesise "
                f"the weights it has no file for")
        for entry in plan["runs"]:
            if entry["face_key"] != face["face_key"] or not entry.get("css"):
                continue
            varies = entry["css"].get("font-variation-settings") is not None
            if varies != variable:
                failures.append(
                    f"page {entry['page']} run {entry['run_index']}: "
                    f"font-variation-settings is "
                    f"{'set' if varies else 'absent'} for a "
                    f"{'variable' if variable else 'static'} face")
                break

    # 4c. Times New Roman is the corpus's serif and it must never be served by a
    #     sans face. It resolves only to Tinos, and only with the proof attached;
    #     with the package absent it must stay unresolved and say which package
    #     would fix it, rather than quietly borrowing Arimo.
    for face in plan["faces"]:
        if plan_for(face["family"]) is not FAMILY_PLANS.get("Times New Roman"):
            continue
        if face["status"] == "resolved":
            if face["fontsource_package"] not in ("@fontsource/tinos",
                                                  "@fontsource-variable/tinos"):
                failures.append(
                    f"{face['face_key']}: Times New Roman resolved to "
                    f"{face['fontsource_package']}, which is not Tinos")
        elif "tinos" not in " ".join(face["required_packages"]):
            failures.append(
                f"{face['face_key']}: an unresolved Times New Roman face must name "
                f"the package that would resolve it, got {face['required_packages']}")

    # 5. The pinned scale is the default, and it still agrees with the faces.
    if plan["generator"]["horizontal_scale"]["source"] != "pinned-constant":
        failures.append("the default plan must use the pinned scale, not a "
                        "machine-dependent derived one")
    derived = derive_horizontal_scale()
    if derived is not None and abs(derived - ARIAL_NARROW_HORIZONTAL_SCALE) > 1e-4:
        failures.append(
            f"pinned Arial Narrow scale {ARIAL_NARROW_HORIZONTAL_SCALE} disagrees "
            f"with the system faces ({derived}); re-measure before shipping")

    # 6. Every run gets a CSS block, with an explicit line-height, and a scaled
    #    run carries the transform that makes its advances come out right.
    total_runs = sum(len(p["text_runs"]) for p in ir["pages"])
    if len(plan["runs"]) != total_runs:
        failures.append(f"plan covers {len(plan['runs'])} runs, IR has {total_runs}")
    scaled_runs = 0
    for entry in plan["runs"]:
        css = entry["css"]
        where = f"page {entry['page']} run {entry['run_index']}"
        if css is None:
            failures.append(f"{where}: no CSS")
            continue
        for required in ("font-family", "font-size", "font-weight", "font-style",
                         "line-height"):
            if not css.get(required):
                failures.append(f"{where}: missing {required}")
        if entry["horizontal_scale"] != 1.0:
            scaled_runs += 1
            if css["transform"] != f"scaleX({format_scale(entry['horizontal_scale'])})":
                failures.append(f"{where}: scaled run without a matching transform")
            if css["display"] != "inline-block":
                failures.append(
                    f"{where}: scaled run is not inline-block, so the browser "
                    f"would drop the transform entirely")
            if css["transform-origin"] != "0% 0%":
                failures.append(
                    f"{where}: transform-origin {css['transform-origin']!r} is not "
                    f"the run's left edge; every glyph origin would shift")
        elif css["transform"] is not None:
            failures.append(f"{where}: unscaled run must not emit a transform")
        # The authored letter-spacing has to survive the transform as the
        # page-space value the tracking model derived.
        authored, effective = entry["letter_spacing_css_pt"], entry["letter_spacing_pt"]
        if (authored is None) != (effective is None):
            failures.append(f"{where}: letter-spacing pair disagrees on presence")
        elif authored is not None and r4(authored * entry["horizontal_scale"]) != effective:
            # Exact on the rounded value rather than a tolerance: both fields are
            # r4'd, so the relation is reproducible to the last emitted digit.
            failures.append(
                f"{where}: authored letter-spacing {authored}pt does not scale to "
                f"the effective {effective}pt")
        if abs(entry["width_residual_pt"]) > LETTER_SPACING_ACCUMULATED_PT:
            failures.append(
                f"{where}: emitted CSS misses the measured width by "
                f"{entry['width_residual_pt']}pt")
    if scaled_runs != 10:
        failures.append(f"expected 10 scaled runs on 2551Q, got {scaled_runs}")

    # 7. Determinism: two evaluations must serialise identically.
    again = json.dumps(evaluate(ir, FaceLibrary(fonts_root)), sort_keys=False)
    if again != json.dumps(plan, sort_keys=False):
        failures.append("output is not deterministic across runs")

    print_face_table(plan, sys.stdout)
    print("", file=sys.stdout)
    print(f"warnings: {len(plan['warnings'])}", file=sys.stdout)
    for warning in plan["warnings"][:6]:
        print(f"  - {warning}", file=sys.stdout)
    if len(plan["warnings"]) > 6:
        print(f"  … {len(plan['warnings']) - 6} more", file=sys.stdout)

    print("", file=sys.stdout)
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stdout)
        return 1
    print(f"self-test OK: {len(plan['faces'])} faces, {len(plan['runs'])} runs, "
          f"brotli via {plan['generator']['brotli_provider']}", file=sys.stdout)
    return 0


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--ir", type=pathlib.Path, default=None,
                        help="IR JSON from extract.py")
    parser.add_argument("--out", type=pathlib.Path, default=None,
                        help="Write the font plan here (default: stdout).")
    parser.add_argument("--fonts-root", type=pathlib.Path, default=None,
                        help="Checkout holding node_modules/@fontsource-variable "
                             "(default: nearest ancestor that has it).")
    parser.add_argument("--summary", action="store_true",
                        help="Print the per-face advance-delta table to stderr.")
    parser.add_argument("--derive-narrow-scale", action="store_true",
                        help="Measure the Arial Narrow scaleX from the macOS "
                             "system faces instead of using the pinned constant. "
                             "Makes the plan machine-dependent; for auditing the "
                             "pin, not for building.")
    parser.add_argument("--self-test", action="store_true",
                        help="Assert the plan against the real 2551Q IR.")
    args = parser.parse_args(argv)

    here = pathlib.Path(__file__).resolve().parent
    fonts_root = args.fonts_root or find_fonts_root(here)

    if args.self_test:
        ir_path = args.ir
        if ir_path is None:
            for candidate in [here.parent.parent / SELF_TEST_IR, SELF_TEST_IR]:
                if candidate.is_file():
                    ir_path = candidate
                    break
        if ir_path is None:
            print("self-test could not locate build/ir/2551q-2018.ir.json",
                  file=sys.stderr)
            return 2
        return self_test(ir_path, fonts_root)

    if args.ir is None or not args.ir.is_file():
        print(f"no such IR: {args.ir}", file=sys.stderr)
        return 2
    if fonts_root is None:
        print("could not locate node_modules/@fontsource-variable; pass --fonts-root",
              file=sys.stderr)
        return 2

    ir = json.loads(args.ir.read_text(encoding="utf-8"))
    overrides, provenance = scale_overrides(args.derive_narrow_scale)
    plan = evaluate(ir, FaceLibrary(fonts_root), overrides, provenance)
    payload = json.dumps(plan, indent=2, ensure_ascii=False) + "\n"

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(payload, encoding="utf-8")
    else:
        sys.stdout.write(payload)

    if args.summary:
        print_face_table(plan)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
