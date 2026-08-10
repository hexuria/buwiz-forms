#!/usr/bin/env python3
"""Break the fixture corpus at the source and watch extract.py's checks trip.

A fixture that cannot fail is a decoration. `extract.py --self-test` already
proves each check can fail by mutating the *evidence* -- the extracted IR, in
memory. That answers "is this check wired to its subject" but not "does the
fixture actually contain the structure the check is about": a corpus missing the
structure entirely would extract to nothing, the check would find nothing to
disagree with, and it would pass.

So this mutates the fixture PDFs instead. For each case it patches one primitive
in make_fixtures.py, rebuilds the whole corpus into a temporary directory,
re-pins it to its own new digests (so the sha256 mismatch cannot stand in for
the real result), and runs every check. Exactly one check must trip, and it must
be the expected one -- a mutation that trips two means a check is standing in
for another, and a mutation that trips none means the fixture never carried the
property.

Six cases reach past make_fixtures.py, and say so. Their subjects are not forms
but four pages extract.py writes itself, one per question no PDF in either
corpus states: ten painting ops with three of them drawn outside their own
scissor; seven strokes stating every case of the `J` operator at once, because
no fixture draws a round or projecting cap; a page of underscore runs, because
the corpus carries no two-underscore punctuation beside a blank and no
unresolvable face; and a page whose text operators put two baselines inside one
rawdict span. Nothing in the corpus feeds those checks, so no fixture can break
them -- and a fixture that carried a clip, a cap, a sub-floor blank or a
doubled baseline would be ink no check reads, which is the decoration this file
exists to catch. Those cases patch the probe pages' own source instead. Each is
still a mutation of the PDF a check measures rather than of the evidence that
PDF produced, which is the property the rest of this file rests on.

Five of extract.py's checks are deliberately NOT reachable this way; see
CONTRACT_ONLY. They are statements about the extractor's own output contract
rather than about corpus content, and no PDF can violate them. Naming them here
is what stops this file from looking like complete coverage: the case table and
that list must between them account for every check extract.py declares, and
this exits non-zero if they do not.

Usage:
    python3 tools/formgen/fixtures/prove_fixtures_fail.py
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import sys
import tempfile
from typing import Any, Callable, Sequence

FIXTURE_ROOT = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(FIXTURE_ROOT.parent))

try:
    import fitz  # PyMuPDF  # noqa: F401 - imported for the same guard as below
except ImportError:  # pragma: no cover - environment guard
    sys.exit("PyMuPDF is required: pip install pymupdf")

import extract  # noqa: E402 - the path has to be set up first
import make_fixtures as fixtures  # noqa: E402

# Checks whose subject is the extractor's output contract, not the corpus. No
# PDF can make a rule forget its own paint_seq or make two extractions of one
# file differ, so these have no source-level mutation and are proven only by
# extract.py's own in-memory probes.
CONTRACT_ONLY = {
    "determinism": "two extractions of one file; no PDF can make them differ",
    "paint-seq": "every emitted item carries an ordinal, by construction",
    "paint-spans": "the contributor contract is emitted, not read from the PDF",
    "interval-provenance": "measured on a synthetic interval list, not a file",
    "paint-order-reconciliation": "probes a deliberately desynced argument",
}


# ---------------------------------------------------------------------------
# The source-level mutations
# ---------------------------------------------------------------------------


def mutate_paper() -> None:
    """Print every fixture on Letter instead of folio."""
    fixtures.PAGE_HEIGHT_PT = 792.0


def mutate_paths() -> None:
    """Never draw the first "write here" marker, so one triangle is missing."""
    original = fixtures.right_triangle
    drawn = [0]

    def one_fewer(*args: Any, **kwargs: Any) -> None:
        drawn[0] += 1
        if drawn[0] == 1:
            return
        original(*args, **kwargs)

    fixtures.right_triangle = one_fewer


def mutate_soft_masks() -> None:
    """Build every fixture image without alpha, so no soft mask is written."""
    original = fixtures.checkerboard

    def flattened(width: int, height: int, rgb: tuple[int, int, int],
                  alpha: Callable[[int, int], int]) -> Any:
        return fitz.Pixmap(original(width, height, rgb, alpha), 0)

    fixtures.checkerboard = flattened


def mutate_transforms() -> None:
    """Leave the seal on insert_image()'s own positive-diagonal placement."""
    fixtures.flip_placement = lambda doc, page, box: None


def mutate_codepoints() -> None:
    """Emit no unmappable glyph, so the honesty field has nothing to record."""
    fixtures.insert_unmappable_glyph = lambda doc, page: None


def mutate_tone() -> None:
    """Paint both decorative greys black -- the documented past failure."""
    fixtures.GREY_LIGHT = 0.0
    fixtures.GREY_MID = 0.0


def mutate_bar_like() -> None:
    """Draw the separators exactly vertical, so none of them leans at all."""
    fixtures.LEAN_OFFSET_PT = 0.0


def mutate_clips() -> None:
    """Build the probe page with no scissor: every `W n` becomes a plain `n`.

    The path is still constructed and still discarded, so the page paints the
    same ten ops in the same order; only the clipping is gone. The three ops it
    draws outside their scissors are then ink like any other, which is exactly
    what the extractor did before it read the clip stack at all -- 1701 page 1's
    fill at x 602.16, beyond a clip ending at 598.32, printed as a black bar
    where the official sheet is blank.

    Counted rather than replaced blind: a pattern that stopped matching would
    mutate nothing, and a case that mutates nothing proves nothing.
    """
    stream = extract.CLIP_PROBE_STREAM
    found = stream.count(b" re W n")
    if found != PROBE_SCISSORS:
        raise SystemExit(f"the clip probe states {found} scissors, "
                         f"expected {PROBE_SCISSORS}")
    extract.CLIP_PROBE_STREAM = stream.replace(b" re W n", b" re n")


# The probe page's two nested scissors. Named so the mutation above fails loudly
# if the page is rewritten, rather than quietly patching a stream it no longer
# understands.
PROBE_SCISSORS = 2


def mutate_stroke_caps() -> None:
    """Build the cap probe butt-capped: every `1 J` and `2 J` becomes `0 J`.

    The same seven strokes are painted in the same order at the same declared
    endpoints; only the cap style is gone. The extractor then reads every bar
    as stopping exactly at its endpoints and measures every extension as 0.0,
    which is precisely the pre-cap-model reading -- the one that filed 2550M's
    round-capped comb ticks short of their rail, so lattice.split_verticals
    took them for box borders and four year boxes reached the taxpayer as one
    wide input. The check must refuse it on both fronts: the published
    geometry no longer matches the table, and the measured extensions are 0.0
    where the page's `J` operators say 0.6.

    Counted rather than replaced blind, for the same reason as mutate_clips: a
    pattern that stopped matching would mutate nothing, and a case that
    mutates nothing proves nothing.
    """
    stream = extract.CAP_PROBE_STREAM
    found = (stream.count(b"\n1 J\n"), stream.count(b"\n2 J\n"))
    if found != (PROBE_ROUND_CAPS, PROBE_PROJECTING_CAPS):
        raise SystemExit(
            f"the cap probe states {found[0]} round and {found[1]} projecting "
            f"caps, expected {PROBE_ROUND_CAPS} and {PROBE_PROJECTING_CAPS}")
    extract.CAP_PROBE_STREAM = (stream
                                .replace(b"\n1 J\n", b"\n0 J\n")
                                .replace(b"\n2 J\n", b"\n0 J\n"))


# The cap probe's non-butt cap operators: five round (`1 J`), one projecting
# (`2 J`). Named for the same reason as PROBE_SCISSORS -- a rewritten probe
# page must fail this case loudly, not be half-patched in silence.
PROBE_ROUND_CAPS = 5
PROBE_PROJECTING_CAPS = 1


def _one_operator(stream: bytes, operator: bytes, what: str) -> None:
    """Refuse to patch a probe stream that does not state `operator` exactly once.

    Same reason mutate_clips counts its scissors: a pattern that stopped
    matching would mutate nothing, and a case that mutates nothing proves
    nothing. A pattern that started matching twice is worse -- it would mutate
    a second operator nobody described.
    """
    found = stream.count(operator)
    if found != 1:
        raise SystemExit(f"the {what} states {operator!r} {found} time(s), "
                         f"expected exactly 1")


def _text_op(text: str) -> bytes:
    """The probe stream's own way of writing one literal-string show operator."""
    return b"(" + text.encode("ascii") + b") Tj"


def mutate_ruled_blank_split() -> None:
    """Set the mixed run as its blank alone: `(AB ____ CD)` becomes `(____)`.

    The blank is still drawn, by the same face at the same size on the same
    baseline, and still becomes a rule -- so the page keeps its two groups, its
    one publication and its one refusal, and the floor and fail-closed checks
    see exactly the census they pin. What the page no longer carries is the
    SPLIT: there is no text either side of the blank for the run to be cut into,
    so neither fragment the check names is on the page, and the rule the blank
    publishes runs 19.78 -> 42.46 rather than 35.9 -> 58.58, because its first
    glyph is now the run's first glyph.

    That is what the check is named for. `AB ` and ` CD` each end at their OWN
    outermost glyph rather than at the span's box, and a run that is nothing but
    its blank states neither of them. A corpus that only ever set blanks alone
    would leave the split unproven while every rule it published still landed
    correctly -- which is the reading this file exists to refuse.

    Derived from the pinned text rather than spelled out again, so a rewritten
    probe fails here loudly instead of being patched by a stale literal.
    """
    text = extract.RULED_BLANK_PROBE_MIXED_TEXT
    groups = extract.ruled_blank_groups(text)
    if len(groups) != 1:
        raise SystemExit(f"the ruled-blank probe's mixed run {text!r} holds "
                         f"{len(groups)} blank(s), expected exactly 1")
    start, end = groups[0]
    stream = extract.RULED_BLANK_PROBE_STREAM
    _one_operator(stream, _text_op(text), "ruled-blank probe")
    extract.RULED_BLANK_PROBE_STREAM = stream.replace(
        _text_op(text), _text_op(text[start:end]))


def mutate_ruled_blank_floor() -> None:
    """Punctuate the sub-floor run with hyphens: `XY __ Z` becomes `XY -- Z`.

    Two underscores are the whole structure this check is about, and this page
    is the only place either corpus states them. With hyphens in their place the
    run is still there, still text and still verbatim -- and it is no longer the
    run the check pins, which is the reading a corpus carrying no sub-floor
    blank at all would leave.

    It does not reach the group census, and cannot. RULED_BLANK_PROBE_GROUPS is
    2, and the two are the mixed run's blank and the unresolvable face's, so any
    mutation that moves the count has to add or remove a group: a group added is
    a rule the split check's table does not name, and a group removed is a
    publication or a refusal the fail-closed check counts. That assertion is
    jointly owned by all three checks, and extract.py's own in-memory probe is
    what proves it wired. What is proven here is the thing no in-memory probe
    can prove -- that the sub-floor run is in the source at all.
    """
    text = extract.RULED_BLANK_PROBE_BELOW_FLOOR
    if extract.ruled_blank_groups(text):
        raise SystemExit(f"the ruled-blank probe's sub-floor run {text!r} "
                         f"already reads as a blank")
    stream = extract.RULED_BLANK_PROBE_STREAM
    _one_operator(stream, _text_op(text), "ruled-blank probe")
    extract.RULED_BLANK_PROBE_STREAM = stream.replace(
        _text_op(text),
        _text_op(text.replace(extract.RULED_BLANK_CHARACTER, PROBE_PUNCTUATION)))


def mutate_ruled_blank_fail_closed() -> None:
    """Draw the unresolvable blank with the resolvable face instead, set 40pt.

    The probe's second font is unembedded and is not base-14, so MuPDF draws
    something and no face this module can name states that glyph's outline --
    1707's shape, and the only unresolvable face in either corpus. Setting the
    run in /F1 takes it away: the band becomes derivable, and the refusal the
    check pins does not happen.

    The size travels with the face, and has to. A resolvable blank at the
    probe's own 10pt is PUBLISHED, and a published blank is a rule the split
    check's table does not name -- that mutation trips split too, for a reason
    of split's own, and would prove nothing about either check. The probe's
    pinned blank is 0.5pt thick at 10pt, so this face's underscore is 0.05 em:
    at 40pt it draws 2.0pt, over MAX_RULE_THICKNESS_PT, and the group is refused
    on its geometry and kept as text. Same two groups, same one publication,
    same one retained run. Only the REASON moves -- from a face that cannot be
    named to a band too thick to be a rule -- and the reason is what this check
    pins.
    """
    stream = extract.RULED_BLANK_PROBE_STREAM
    _one_operator(stream, PROBE_UNRESOLVABLE_FACE_OP, "ruled-blank probe")
    extract.RULED_BLANK_PROBE_STREAM = stream.replace(
        PROBE_UNRESOLVABLE_FACE_OP, PROBE_THICK_RESOLVED_FACE_OP)


# The character the sub-floor run's underscores are punctuated with. Anything
# RULED_BLANK_CHARACTER is not would do; a hyphen is the fill character the
# floor exists to spare, so the mutated run stays a run the check would have to
# leave alone for a reason of its own.
PROBE_PUNCTUATION = "-"

# The ruled-blank probe's third text operator, and what mutate_ruled_blank_
# fail_closed puts in its place. /F2 is the unembedded non-base-14 face; /F1 is
# the resolvable one, and 40pt is the size at which its underscores draw a
# 2.0pt band -- over extract.MAX_RULE_THICKNESS_PT, so the blank is refused
# rather than published. Named here for the same reason as PROBE_SCISSORS: a
# probe page rewritten around a different font or size must fail this case
# loudly rather than be patched in silence.
PROBE_UNRESOLVABLE_FACE_OP = b"/F2 10 Tf"
PROBE_THICK_RESOLVED_FACE_OP = b"/F1 40 Tf"


def mutate_baseline_split() -> None:
    """Put each second text operator on its partner's own Td baseline.

    `Z` is set 1pt under `XY`, and `PQ` 4pt under the space that positions it.
    Those two drops are the only reason MuPDF's line builder hands this module a
    span carrying two baselines. Levelled, the page still paints the same five
    operators with the same glyphs at the same x, in the same order, at the same
    size -- and carries no two-baseline span at all.

    BASELINE_PROBE_SPANS then trips first, which is exactly what that assertion
    is for: it is asserted ahead of the run table so that a page which stopped
    provoking the merge -- a reader change, a different line tolerance, or this
    mutation -- fails loudly instead of passing a run table that measures
    nothing. Both pairs are levelled because levelling one leaves the other
    still provoking it, and one two-baseline span is all the check needs.
    """
    stream = extract.BASELINE_PROBE_STREAM
    for placed, levelled in PROBE_SECOND_BASELINES:
        _one_operator(stream, placed, "baseline probe")
        stream = stream.replace(placed, levelled)
    extract.BASELINE_PROBE_STREAM = stream


# The baseline probe's two second operators, as placed and as levelled onto the
# baseline of the operator each shares a span with. Named rather than computed
# so a rewritten probe page fails this case loudly instead of being levelled
# onto coordinates nobody stated.
PROBE_SECOND_BASELINES = ((b"33.34 159 Td", b"33.34 160 Td"),
                          (b"26 126 Td", b"26 130 Td"))


# (the check that must trip, what was done to the source, how)
CASES: tuple[tuple[str, str, Callable[[], None]], ...] = (
    ("paper", "every sheet is built Letter-height", mutate_paper),
    ("paths", "one filled triangle is never drawn", mutate_paths),
    ("soft-masks", "the images are built without alpha", mutate_soft_masks),
    ("transforms", "the seal is placed unflipped", mutate_transforms),
    ("codepoints", "the unmappable glyph is not emitted", mutate_codepoints),
    ("tone", "both decorative greys are painted black", mutate_tone),
    ("is-bar-like", "the separators are drawn exactly vertical", mutate_bar_like),
    ("clips", "the probe page's scissors are never established", mutate_clips),
    ("stroke-caps", "the cap probe's every stroke is butt-capped",
     mutate_stroke_caps),
    ("ruled-blank-split", "the blank probe's mixed run is set as its blank alone",
     mutate_ruled_blank_split),
    ("ruled-blank-floor", "the blank probe's sub-floor run is punctuated with "
     "hyphens", mutate_ruled_blank_floor),
    ("ruled-blank-fail-closed", "the blank probe's unresolvable face is the "
     "resolvable one at 40pt", mutate_ruled_blank_fail_closed),
    ("baseline-split", "the baseline probe's every span is levelled onto one "
     "baseline", mutate_baseline_split),
)

# Everything a mutation is allowed to reach into, as (module, attribute),
# captured before the first one runs and restored before each. Patching a module
# global and forgetting to put it back would leak into the next case and
# misattribute its result -- and three cases patch one stream, so a leak here
# would read as a check standing in for another. The probe streams are on this
# list for that reason and no other: they are the four subjects that do not live
# in the corpus.
PATCHABLE = ((fixtures, "PAGE_HEIGHT_PT"), (fixtures, "LEAN_OFFSET_PT"),
             (fixtures, "GREY_LIGHT"), (fixtures, "GREY_MID"),
             (fixtures, "right_triangle"), (fixtures, "checkerboard"),
             (fixtures, "flip_placement"), (fixtures, "insert_unmappable_glyph"),
             (extract, "CLIP_PROBE_STREAM"), (extract, "CAP_PROBE_STREAM"),
             (extract, "RULED_BLANK_PROBE_STREAM"),
             (extract, "BASELINE_PROBE_STREAM"))


def profile_over(root: pathlib.Path) -> extract.SelfTestProfile:
    """The fixture profile, re-pinned to whatever now sits under `root`.

    Re-pinning is the point. Leaving the tracked digests in place would make
    every mutation fail at the hash check, and a hash failure is not evidence
    that the check under test noticed anything.
    """
    base = extract.FIXTURE_PROFILE
    fixtures_table = {
        code: (relative, revision,
               hashlib.sha256((root / relative).read_bytes()).hexdigest())
        for code, (relative, revision, _digest) in base.fixtures.items()
    }
    return extract.SelfTestProfile(
        name="mutated fixtures", source_root=root, fixtures=fixtures_table,
        paper=base.paper, determinism_form=base.determinism_form,
        masked=base.masked, flipped=base.flipped, paths_form=base.paths_form,
        triangles=base.triangles, decimal_points=base.decimal_points,
        tones=base.tones, retexted_glyphs=base.retexted_glyphs,
        retexted_glyph_id=base.retexted_glyph_id,
        retexted_rawdict_codepoint=base.retexted_rawdict_codepoint,
        bar_like_form=base.bar_like_form, leaning_bars=base.leaning_bars,
        is_evidence=False)


def tripped_checks(root: pathlib.Path) -> list[str]:
    """Which of extract.py's checks disagree with the corpus under `root`."""
    profile = profile_over(root)
    evidence = extract.gather_evidence(profile, root)
    return sorted(name for name, check in extract.SELF_TEST_CHECKS
                  if check(evidence))


def prove(stream: Any) -> int:
    declared = {name for name, _check in extract.SELF_TEST_CHECKS}
    accounted = {name for name, _why, _mutate in CASES} | set(CONTRACT_ONLY)
    failures: list[str] = []
    if accounted != declared:
        failures.append(
            f"every check needs a source-level mutation or a stated reason it "
            f"cannot have one; unaccounted={sorted(declared - accounted)} "
            f"invented={sorted(accounted - declared)}")

    pristine = [(module, name, getattr(module, name))
                for module, name in PATCHABLE]
    with tempfile.TemporaryDirectory() as scratch:
        root = pathlib.Path(scratch)
        fixtures.build_all(root / "clean")
        clean = tripped_checks(root / "clean")
        if clean:
            failures.append(f"the unmutated corpus already trips {clean}")
        print(f"  {'unmutated':<12} {'OK' if not clean else 'BROKEN':<5} "
              f"nothing trips", file=stream)

        for expected, description, mutate in CASES:
            for module, name, value in pristine:
                setattr(module, name, value)
            mutate()
            out = root / expected
            fixtures.build_all(out)
            tripped = tripped_checks(out)
            good = tripped == [expected]
            if not good:
                failures.append(
                    f"'{description}' should have tripped exactly [{expected!r}], "
                    f"tripped {tripped}")
            print(f"  {expected:<12} {'OK' if good else 'WEAK':<5} "
                  f"{description}", file=stream)
        for module, name, value in pristine:
            setattr(module, name, value)

    for name in sorted(CONTRACT_ONLY):
        print(f"  {name:<12} n/a   not reachable from corpus content: "
              f"{CONTRACT_ONLY[name]}", file=stream)
    for message in failures:
        print(f"    FAIL {message}", file=stream)
    print(f"prove-fixtures-fail: "
          f"{'PASS' if not failures else f'{len(failures)} FAILURE(S)'} over "
          f"{len(CASES)} source-level mutations, {len(CONTRACT_ONLY)} checks "
          f"stated as contract-only", file=stream)
    return 1 if failures else 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.parse_args(argv)
    return prove(sys.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
