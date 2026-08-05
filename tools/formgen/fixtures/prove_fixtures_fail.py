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


# (the check that must trip, what was done to the corpus, how)
CASES: tuple[tuple[str, str, Callable[[], None]], ...] = (
    ("paper", "every sheet is built Letter-height", mutate_paper),
    ("paths", "one filled triangle is never drawn", mutate_paths),
    ("soft-masks", "the images are built without alpha", mutate_soft_masks),
    ("transforms", "the seal is placed unflipped", mutate_transforms),
    ("codepoints", "the unmappable glyph is not emitted", mutate_codepoints),
    ("tone", "both decorative greys are painted black", mutate_tone),
    ("is-bar-like", "the separators are drawn exactly vertical", mutate_bar_like),
)

# Everything a mutation is allowed to reach into, captured before the first one
# runs and restored before each. Patching a module global and forgetting to put
# it back would leak into the next case and misattribute its result.
PATCHABLE = ("PAGE_HEIGHT_PT", "LEAN_OFFSET_PT", "GREY_LIGHT", "GREY_MID",
             "right_triangle", "checkerboard", "flip_placement",
             "insert_unmappable_glyph")


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

    pristine = {name: getattr(fixtures, name) for name in PATCHABLE}
    with tempfile.TemporaryDirectory() as scratch:
        root = pathlib.Path(scratch)
        fixtures.build_all(root / "clean")
        clean = tripped_checks(root / "clean")
        if clean:
            failures.append(f"the unmutated corpus already trips {clean}")
        print(f"  {'unmutated':<12} {'OK' if not clean else 'BROKEN':<5} "
              f"nothing trips", file=stream)

        for expected, description, mutate in CASES:
            for name, value in pristine.items():
                setattr(fixtures, name, value)
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
        for name, value in pristine.items():
            setattr(fixtures, name, value)

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
