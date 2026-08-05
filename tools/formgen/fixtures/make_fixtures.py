#!/usr/bin/env python3
"""Build the synthetic PDF corpus extract.py's --self-test measures in CI.

`extract.py --self-test` is the strongest check this pipeline has, and none of
it could run anywhere but this laptop: every assertion is pinned against six
official BIR PDFs that are deliberately untracked (`*.pdf` is gitignored, they
are official documents, and they are pinned by sha256 precisely so a swapped
file fails loudly). A CI job that skipped it would print a green tick having
evaluated nothing -- the exact failure this project spent seventy commits
removing from its own gate.

So this builds a second corpus that *is* trackable. Each file here is the
smallest PDF that still exercises one property the real corpus taught us about,
and the docstring on each builder names the real form it stands in for. The
fixtures do not replace the real pins: a fixture can only ever encode what this
module already believes, whereas the six official files are evidence. Both pin
tables live in extract.py and both are still runnable; `--self-test` alone reads
the real one.

Determinism is not incidental. The pins in extract.py are sha256 over these
exact bytes, so a rebuild that differed by one byte would fail extraction rather
than silently score a different corpus. MuPDF stamps no creation date once the
metadata is cleared and `no_new_id` suppresses the /ID array, so two runs of
this script produce byte-identical files. The one thing that does move them is
the MuPDF version, whose banner MuPDF writes into its own header comment; that
is why --verify exists and why it prints the version it measured.

Usage:
    python3 tools/formgen/fixtures/make_fixtures.py            # write the corpus
    python3 tools/formgen/fixtures/make_fixtures.py --verify   # rebuild + compare
    python3 tools/formgen/fixtures/make_fixtures.py --pins     # the pin table
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import sys
from typing import Any, Callable, Sequence

try:
    import fitz  # PyMuPDF
except ImportError:  # pragma: no cover - environment guard
    sys.exit("PyMuPDF is required: pip install pymupdf")

FIXTURE_ROOT = pathlib.Path(__file__).resolve().parent

# Folio, the paper 2551Q is printed on: neither A4 nor Letter, so a fixture that
# accidentally took a default would be visible in the paper assertion.
PAGE_WIDTH_PT = 612.0
PAGE_HEIGHT_PT = 936.0

# The four rule thicknesses the BIR generator actually emits. 0.24 is also the
# thickness extract.py invents for a zero-width `l` op, which is why the paths
# fixture below must carry no rule at all.
THICKNESSES_PT = (0.24, 0.48, 0.96, 1.44)

# The two decorative greys the corpus uses, and the knockout white. CLAUDE.md
# records that painting a decorative grey black is a shipped past failure, so
# tone classification has to be exercised on real values, not on placeholders.
GREY_LIGHT = 0.8509
GREY_MID = 0.6509
WHITE = 1.0
BLACK = 0.0

# A stroked separator that leans less than its own stroke width. 2316 draws
# twelve of these; an exact-alignment test demoted every one of them out of
# `rules`, taking real box sides out of lattice.py's reach. 0.17pt of lean
# across 14.5pt of run, against a 0.44pt stroke.
LEAN_COUNT = 12
LEAN_RUN_PT = 14.5
LEAN_OFFSET_PT = 0.17
LEAN_STROKE_PT = 0.44

# The unmappable glyph. A Type3 font whose one glyph has a non-standard name and
# no ToUnicode CMap: get_texttrace() answers U+FFFD honestly, while rawdict
# hands back the raw code byte -- 0xA7, which reads as a section sign and looks
# exactly like content. That is 2550M and 2553's defect, reproduced.
UNMAPPED_CODE = 0xA7
UNMAPPED_GLYPH_NAME = "gexotic"


def gray(value: float) -> tuple[float, float, float]:
    """A neutral RGB triple. to_gray() collapses it back to this exact value."""
    return (value, value, value)


# ---------------------------------------------------------------------------
# Deterministic output
# ---------------------------------------------------------------------------


def save(doc: fitz.Document, path: pathlib.Path) -> bytes:
    """Serialise one fixture with nothing time- or run-dependent in it.

    Clearing the metadata removes the creation and modification dates MuPDF
    would otherwise stamp, and `no_new_id` suppresses the /ID array, which is
    seeded from the clock. `garbage=4` also makes object numbering a function of
    the content rather than of the order the builder happened to allocate xrefs
    in, so an edit to one builder cannot renumber another fixture's objects.
    """
    doc.set_metadata({})
    doc.del_xml_metadata()
    payload = doc.tobytes(garbage=4, deflate=True, no_new_id=True,
                          preserve_metadata=0)
    path.write_bytes(payload)
    return payload


def new_document(pages: int = 1) -> tuple[fitz.Document, list[fitz.Page]]:
    """A blank fixture of `pages` folio sheets.

    The page handles are re-fetched after the last new_page(), because
    new_page() resets every outstanding page reference and drawing on a stale
    one raises rather than painting.
    """
    doc = fitz.open()
    for _ in range(pages):
        doc.new_page(width=PAGE_WIDTH_PT, height=PAGE_HEIGHT_PT)
    return doc, [doc[index] for index in range(pages)]


# ---------------------------------------------------------------------------
# Drawing helpers
# ---------------------------------------------------------------------------


def bar(page: fitz.Page, x0: float, y0: float, x1: float, y1: float,
        tone: float) -> None:
    """One filled axis-aligned bar, which is how the BIR generator draws ink."""
    page.draw_rect(fitz.Rect(x0, y0, x1, y1), color=None, fill=gray(tone),
                   width=0)


def split_run(start: float, end: float, pieces: int) -> list[tuple[float, float]]:
    """Cut a run into abutting pieces, the way a long border is really drawn.

    The pieces meet exactly, so extract.py's interval union has to join them on
    a zero gap rather than on its epsilon. Real joints are patched by corner
    squares, which `merged_box` adds on top.
    """
    step = (end - start) / pieces
    return [(round(start + i * step, 2), round(start + (i + 1) * step, 2))
            for i in range(pieces)]


def merged_box(page: fitz.Page, rect: fitz.Rect, thickness: float,
               tone: float = BLACK, pieces: int = 4) -> None:
    """A box border drawn as many short bars plus a square at each corner.

    This is how the BIR generator emits a long rule, and it is the reason
    extract_segments unions intervals at all: each edge arrives as several
    filled rects, and each corner square is thin on *both* axes so it is offered
    to the horizontal and the vertical grouping alike. The square must vanish
    into the runs it patches -- it is a duplicate contributor, not a rule.
    """
    for x0, x1 in split_run(rect.x0, rect.x1, pieces):
        bar(page, x0, rect.y0, x1, rect.y0 + thickness, tone)
        bar(page, x0, rect.y1 - thickness, x1, rect.y1, tone)
    for y0, y1 in split_run(rect.y0, rect.y1, pieces):
        bar(page, rect.x0, y0, rect.x0 + thickness, y1, tone)
        bar(page, rect.x1 - thickness, y0, rect.x1, y1, tone)
    for x in (rect.x0, rect.x1 - thickness):
        for y in (rect.y0, rect.y1 - thickness):
            bar(page, x, y, x + thickness, y + thickness, tone)


def comb_band(page: fitz.Page, rect: fitz.Rect, slots: int,
              group_after: int) -> None:
    """An enclosing box with equally spaced dividers and one heavier separator.

    A comb is where a wrong slot count puts a typed digit on top of a divider,
    so the fixture carries the two thicknesses a real comb mixes: hairline
    dividers between slots and a heavier bar between groups.
    """
    merged_box(page, rect, THICKNESSES_PT[1], BLACK, pieces=2)
    step = (rect.x1 - rect.x0) / slots
    for index in range(1, slots):
        x = round(rect.x0 + index * step, 2)
        thickness = THICKNESSES_PT[2] if index == group_after else THICKNESSES_PT[0]
        bar(page, x, rect.y0, x + thickness, rect.y1, BLACK)


def checkerboard(width: int, height: int, rgb: tuple[int, int, int],
                 alpha: Callable[[int, int], int]) -> fitz.Pixmap:
    """A tiny RGBA image. `alpha` decides the soft mask fitz will write."""
    samples = bytearray(width * height * 4)
    for y in range(height):
        for x in range(width):
            index = (y * width + x) * 4
            samples[index:index + 3] = bytes(rgb)
            samples[index + 3] = alpha(x, y)
    return fitz.Pixmap(fitz.csRGB, width, height, bytes(samples), True)


# ---------------------------------------------------------------------------
# The fixtures
# ---------------------------------------------------------------------------


def build_rules() -> fitz.Document:
    """Structure: merged runs, every thickness, both greys, a knockout, a comb.

    Stands in for 2551Q, which is the paper, determinism and comb reference and
    is almost entirely filled bars. The white-filled, black-stroked box is the
    fill+stroke case: one drawing, two paint ops, and the reconciliation in
    paint_order() has to give the interior a lower ordinal than its own border
    or the interior erases it.
    """
    doc, pages = new_document(2)
    first, second = pages

    merged_box(first, fitz.Rect(48, 60, 564, 180), THICKNESSES_PT[1], BLACK)
    merged_box(first, fitz.Rect(48, 200, 564, 260), THICKNESSES_PT[3], BLACK,
               pieces=6)
    merged_box(first, fitz.Rect(48, 280, 300, 330), THICKNESSES_PT[2], BLACK,
               pieces=3)
    # A hairline run, and the thinnest ink the corpus carries.
    for x0, x1 in split_run(48, 564, 8):
        bar(first, x0, 350, x1, 350 + THICKNESSES_PT[0], BLACK)

    # Decorative tone: near-invisible on paper, and never to be painted black.
    bar(first, 48, 380, 564, 380 + THICKNESSES_PT[1], GREY_LIGHT)
    bar(first, 48, 396, 564, 396 + THICKNESSES_PT[2], GREY_MID)
    # A tint band, which is an area fill rather than a rule: thick on both axes.
    first.draw_rect(fitz.Rect(48, 420, 564, 452), color=None,
                    fill=gray(GREY_LIGHT), width=0)

    # One drawing, two paint ops: white paper knocked out of the band above,
    # outlined in black. `fs` in get_drawings(), two entries in get_bboxlog().
    first.draw_rect(fitz.Rect(64, 428, 200, 446), color=gray(BLACK),
                    fill=gray(WHITE), width=THICKNESSES_PT[1])

    comb_band(first, fitz.Rect(48, 480, 480, 504), slots=12, group_after=4)

    # Page 2 exists so the paper assertion has more than one page to measure,
    # and carries drawings of its own so nothing on it is vacuously clean.
    merged_box(second, fitz.Rect(48, 60, 564, 140), THICKNESSES_PT[1], BLACK)
    bar(second, 48, 170, 564, 170 + THICKNESSES_PT[0], GREY_MID)
    second.draw_rect(fitz.Rect(48, 200, 300, 240), color=None,
                     fill=gray(GREY_LIGHT), width=0)
    return doc


def right_triangle(page: fitz.Page, x: float, y: float, size: float,
                   shape: Any = None) -> None:
    """A solid right-pointing "write here" marker, filled and closed.

    0605 draws thirty of these. Forced through the rule classifier each one
    became three axis-aligned hairlines with its fill discarded, so a solid
    black arrow printed as a light grey open "F".
    """
    points = [fitz.Point(x, y), fitz.Point(x + size, y + size / 2),
              fitz.Point(x, y + size), fitz.Point(x, y)]
    if shape is None:
        page.draw_polyline(points, color=None, fill=gray(BLACK), width=0)
    else:
        shape.draw_polyline(points)


def build_paths() -> fitz.Document:
    """Non-rectilinear ink: filled triangles and filled Bezier marks.

    Stands in for 0605. The decimal points are four `c` ops each, and were
    dropped outright because only `re` and `l` ops ever reached either
    classifier. This fixture deliberately contains no filled rect at all, so
    the assertion that it carries no rule at the invented 0.24pt default is a
    real statement about the classifier rather than about the drawing.
    """
    doc, (page,) = new_document()
    for row in range(6):
        for column in range(5):
            right_triangle(page, 60 + column * 90, 80 + row * 40, 6)
    # Three markers share a path with a rect, which is why the triangle test is
    # on the subpath and not on the whole path's op census.
    for index in range(3):
        shape = page.new_shape()
        shape.draw_rect(fitz.Rect(60 + index * 90, 340, 100 + index * 90, 352))
        right_triangle(page, 60 + index * 90, 356, 6, shape=shape)
        shape.finish(color=None, fill=gray(BLACK), width=0)
        shape.commit()
    for index in range(10):
        page.draw_circle(fitz.Point(80 + index * 48, 420), 0.85,
                         color=None, fill=gray(BLACK), width=0)
    return doc


def build_masks() -> fitz.Document:
    """Two soft-masked placements: one partly transparent, one fully opaque.

    Stands in for 1604E, whose masked base streams are /Matte padding -- flat
    black that the mask removes -- so painting the base puts a black block over
    a printed label. The opaque one is 2316's case and is the converse the
    assertion needed: when a declared mask hides nothing, compositing it is a
    no-op and the painted digest legitimately equals the base image's.
    """
    doc, (page,) = new_document()
    partial = checkerboard(12, 8, (0, 0, 0),
                           lambda x, y: 255 if (x + y) % 2 == 0 else 0)
    opaque = checkerboard(12, 8, (217, 217, 217), lambda x, y: 255)
    page.insert_image(fitz.Rect(48, 60, 168, 140), pixmap=partial)
    page.insert_image(fitz.Rect(200, 60, 320, 140), pixmap=opaque)
    merged_box(page, fitz.Rect(48, 180, 320, 220), THICKNESSES_PT[1], BLACK)
    return doc


def flip_placement(doc: fitz.Document, page: fitz.Page,
                   box: fitz.Rect) -> None:
    """Rewrite the page's image placement as a vertical flip: negative `d`.

    Written by hand because insert_image() offers only whole-quadrant
    rotations, and rotate=180 negates `a` as well -- a different transform,
    which would let a consumer that handles only 180deg rotation pass.

    insert_image() appends its placement as a content stream of its own, so the
    one holding the `Do` can be replaced without touching the ink around it.
    Which stream that is gets located rather than assumed: rewriting the wrong
    one would erase a drawing instead of flipping the image.

    Factored out so prove_fixtures_fail.py can disable exactly this and nothing
    else, without restating the builder around it.
    """
    name = page.get_images(full=True)[0][7]
    placements = [xref for xref in page.get_contents()
                  if f"/{name} Do".encode("latin-1") in doc.xref_stream(xref)]
    if len(placements) != 1:
        raise SystemExit(f"expected one stream placing /{name}, "
                         f"found {len(placements)}")
    top = PAGE_HEIGHT_PT - box.y0
    doc.update_stream(
        placements[0],
        f"q {box.width} 0 0 -{box.height} {box.x0} {top} cm /{name} Do Q\n"
        .encode("latin-1"))


def build_flip() -> fitz.Document:
    """One placement whose matrix has a negative `d`: a vertical flip.

    Stands in for 2550M's seal, which printed upside-down with its rim lettering
    reading bottom-to-top because the IR carried only a bounding box, and a box
    cannot express a flip.
    """
    doc, (page,) = new_document()
    image = checkerboard(12, 8, (0, 0, 0),
                         lambda x, y: 255 if y < 4 else 96)
    box = fitz.Rect(48, 60, 168, 140)
    page.insert_image(box, pixmap=image)
    merged_box(page, fitz.Rect(48, 180, 320, 220), THICKNESSES_PT[1], BLACK)
    flip_placement(doc, page, box)
    return doc


def insert_unmappable_glyph(doc: fitz.Document, page: fitz.Page) -> None:
    """Draw one glyph the file states no Unicode meaning for.

    A Type3 font whose encoding difference names a glyph nothing knows, with no
    ToUnicode CMap. MuPDF's two text views then disagree: get_texttrace()
    reports U+FFFD with the glyph id, and get_text("rawdict") hands back the
    code byte 0xA7, which renders as a section sign and is indistinguishable
    from content. That is 2550M and 2553's defect, reproduced from scratch.

    Factored out so prove_fixtures_fail.py can disable exactly this and nothing
    else, without restating the builder around it.
    """
    proc = doc.get_new_xref()
    doc.update_object(proc, "<<>>")
    doc.update_stream(proc, b"600 0 0 0 600 600 d1\n60 60 480 480 re f\n")
    font = doc.get_new_xref()
    doc.update_object(font, f"""<</Type/Font/Subtype/Type3
/FontBBox[0 0 600 600]/FontMatrix[0.001 0 0 0.001 0 0]
/CharProcs<</{UNMAPPED_GLYPH_NAME} {proc} 0 R>>
/Encoding<</Type/Encoding/Differences[{UNMAPPED_CODE}/{UNMAPPED_GLYPH_NAME}]>>
/FirstChar {UNMAPPED_CODE}/LastChar {UNMAPPED_CODE}/Widths[600]
/Resources<<>>>>""")
    resources = doc.xref_get_key(page.xref, "Resources")
    doc.xref_set_key(int(resources[1].split()[0]), "Font/T3", f"{font} 0 R")

    stream = doc.get_new_xref()
    doc.update_object(stream, "<<>>")
    doc.update_stream(
        stream,
        b"BT /T3 9 Tf 48 %d Td <%02X> Tj ET\n"
        % (int(PAGE_HEIGHT_PT) - 120, UNMAPPED_CODE))
    existing = doc.xref_get_key(page.xref, "Contents")[1].strip("[] ")
    doc.xref_set_key(page.xref, "Contents", f"[{existing} {stream} 0 R]")


def build_glyphs() -> fitz.Document:
    """A glyph with no Unicode meaning, beside a question mark that has one.

    Stands in for 2550M and 2553. The literal "?" is the other half of the same
    assertion: both characters are what a mis-read symbolic glyph looks like
    when it lands as something readable, so extract.py corroborates every one of
    them against the source's own glyph log -- and a corroboration check needs a
    case that legitimately passes as well as one that must not.
    """
    doc, (page,) = new_document()
    page.insert_text(fitz.Point(48, 80), "WHAT IS THE RATE?", fontname="helv",
                     fontsize=9)
    page.insert_text(fitz.Point(48, 100), "AMOUNT OF PAYMENT", fontname="helv",
                     fontsize=9)
    merged_box(page, fitz.Rect(48, 140, 320, 180), THICKNESSES_PT[1], BLACK)
    insert_unmappable_glyph(doc, page)
    return doc


def build_lean() -> fitz.Document:
    """Twelve stroked separators that lean less than their own stroke width.

    Stands in for 2316. Each covers the same ink a bar would, and each has to
    stay in `rules` where lattice.py can find a box side. This fixture carries
    no curve and no steep segment, so the assertion that it gained no
    non-rectilinear path at all is meaningful: anything in `paths` here is a
    separator that got demoted.
    """
    doc, (page,) = new_document()
    merged_box(page, fitz.Rect(48, 60, 564, 260), THICKNESSES_PT[1], BLACK)
    for index in range(LEAN_COUNT):
        x = 80.0 + index * 40.0
        y = 80.0 + (index % 3) * 50.0
        page.draw_line(fitz.Point(x, y),
                       fitz.Point(x + LEAN_OFFSET_PT, y + LEAN_RUN_PT),
                       color=gray(BLACK), width=LEAN_STROKE_PT)
    return doc


# name -> (builder, what it is the only fixture to exercise)
BUILDERS: tuple[tuple[str, Callable[[], fitz.Document], str], ...] = (
    ("rules.pdf", build_rules,
     "paper, determinism, merged runs, four thicknesses, greys, knockout, "
     "fill+stroke, comb"),
    ("paths.pdf", build_paths, "filled triangles and Bezier marks"),
    ("masks.pdf", build_masks, "partial and fully opaque soft masks"),
    ("flip.pdf", build_flip, "a placement matrix with a negative d"),
    ("glyphs.pdf", build_glyphs, "an unmappable glyph and a real question mark"),
    ("lean.pdf", build_lean, "twelve bar-like leaning separators"),
)


def build_all(out_dir: pathlib.Path) -> dict[str, bytes]:
    out_dir.mkdir(parents=True, exist_ok=True)
    written: dict[str, bytes] = {}
    for name, builder, _why in BUILDERS:
        doc = builder()
        written[name] = save(doc, out_dir / name)
        doc.close()
    return written


def report(written: dict[str, bytes], stream: Any) -> None:
    total = sum(len(payload) for payload in written.values())
    for name, payload in written.items():
        print(f"  {name:<12} {len(payload):>7} bytes  "
              f"{hashlib.sha256(payload).hexdigest()}", file=stream)
    print(f"  {'total':<12} {total:>7} bytes over {len(written)} files",
          file=stream)


def extract_pins() -> dict[str, str]:
    """extract.py's fixture pin table, keyed by file name.

    Read back rather than duplicated here. Two spellings of one digest drift,
    and the drift would surface as an unexplained hash mismatch at extraction
    time instead of as the stale pin table it is.
    """
    sys.path.insert(0, str(FIXTURE_ROOT.parent))
    import extract  # noqa: E402 - the path has to be set up first

    return {relative: digest
            for relative, _revision, digest in extract.FIXTURE_FIXTURES.values()}


def verify(out_dir: pathlib.Path) -> int:
    """Rebuild every fixture in memory and compare it to the tracked bytes.

    A difference is a real finding either way round: either a builder changed
    and the tracked corpus is stale, or this MuPDF writes different bytes than
    the one that produced the tracked corpus. Both invalidate the sha256 pins in
    extract.py, so neither may be reported as a pass.

    The tracked bytes are also checked against those pins, which closes the
    loop: builder, file on disk and pin either all agree or this says which one
    does not.
    """
    pinned = extract_pins()
    failures: list[str] = []
    for name in sorted(set(pinned) - {name for name, _b, _w in BUILDERS}):
        failures.append(f"{name}: pinned in extract.py but no builder makes it")
    for name, builder, _why in BUILDERS:
        path = out_dir / name
        doc = builder()
        doc.set_metadata({})
        doc.del_xml_metadata()
        rebuilt = doc.tobytes(garbage=4, deflate=True, no_new_id=True,
                              preserve_metadata=0)
        doc.close()
        if not path.is_file():
            failures.append(f"{name}: not present under {out_dir}")
            continue
        tracked = path.read_bytes()
        digest = hashlib.sha256(tracked).hexdigest()
        if tracked != rebuilt:
            failures.append(
                f"{name}: tracked {len(tracked)} bytes "
                f"sha256 {digest[:16]} != "
                f"rebuilt {len(rebuilt)} bytes "
                f"sha256 {hashlib.sha256(rebuilt).hexdigest()[:16]}")
        elif pinned.get(name) != digest:
            failures.append(
                f"{name}: extract.py pins {(pinned.get(name) or 'nothing')[:16]}, "
                f"the tracked file hashes to {digest[:16]}")
        else:
            print(f"  {name:<12} identical and pinned ({len(tracked)} bytes)",
                  file=sys.stderr)
    print(f"built with PyMuPDF {fitz.VersionBind} / MuPDF {fitz.VersionFitz}",
          file=sys.stderr)
    for message in failures:
        print(f"    FAIL {message}", file=sys.stderr)
    print(f"verify: {'PASS' if not failures else f'{len(failures)} FAILURE(S)'}",
          file=sys.stderr)
    return 1 if failures else 0


def pins(out_dir: pathlib.Path) -> int:
    """Print the pin table extract.py's fixture profile holds, ready to paste."""
    for name, _builder, why in BUILDERS:
        path = out_dir / name
        if not path.is_file():
            print(f"# {name}: absent", file=sys.stdout)
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        code = f'FIXTURE-{name.removesuffix(".pdf").upper()}'
        print(f'    # {why}\n    "{code}": (\n        "{name}", "0001",\n'
              f'        "{digest}"),')
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out-dir", type=pathlib.Path, default=FIXTURE_ROOT,
                        help="Where the fixture PDFs are written or verified.")
    parser.add_argument("--verify", action="store_true",
                        help="Rebuild and compare against the tracked bytes "
                             "instead of overwriting them.")
    parser.add_argument("--pins", action="store_true",
                        help="Print the sha256 pin table for extract.py.")
    args = parser.parse_args(argv)

    if args.verify:
        return verify(args.out_dir)
    if args.pins:
        return pins(args.out_dir)
    written = build_all(args.out_dir)
    report(written, sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
