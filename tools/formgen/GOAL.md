# Goal: PR #13 mergeable, fully documented, every known issue closed

Bring https://github.com/hexuria/buwiz-forms/pull/13 to a state where merging
it is a decision about scope, not a gamble about quality: the gate passes, CI
is green, the documentation lets a newcomer understand and safely change the
system, and every finding in the ledger is fixed or explained.

Supersedes the previous goal (integrity-only scope). The user has explicitly
pulled the defect backlog INTO scope: "fix all possible issues, gaps, edge
cases."

## Done when

```sh
python3 tools/formgen/gate.py     # exits 0 — all 12 checks
gh pr checks 13                   # every check green
```

A check that cannot be evaluated is a failure. Never edit either command to
make it pass. The gate regenerates twice, audits, and runs the comb referee;
~60 min. CI runs the no-external-input subset on every push.

## What "done" decomposes into

The gate's four failing checks, and what each needs:

1. **artwork** — 1 image missing on 1701MS. Diagnose which placement and why;
   it predates the manifest work.
2. **assertions** — 5 of 8 fail:
   - `inputs_over_printed_text` (48 forms): two known populations — an input
     over its own field's pre-printed decoration (may be legitimate; the money
     "." renders behind inputs BY DESIGN) vs genuine overlap of form labels.
     Triage first; a principled narrowing of the assertion is allowed ONLY
     with the populations separated and counted, never to make numbers move.
   - `comb_slots_match_printed` (51 forms): adjudicated by the comb referee's
     19 mismatches. Fix the producer at fault per mismatch (lattice vs emit vs
     extract), not the assertion.
   - `money_boxes_have_inputs` (6 forms), `reflow_rate_without_description`
     (2551M — its ATC band has no ruled grid), `image_transform_applied`
     (1702Q).
3. **findings** — 52 of 84 blocker+major open in `review-findings.json`. Each
   must end `fixed` or `not-a-defect` with a non-empty resolution. Round 4
   (visual re-review of the 40 forms that had findings) closes the loop:
   screenshot ours, render the official at the same size, look at both.
4. **comb-referee** — 19 source/layout/emission mismatches → 0, via the
   producer fixes in (2).

Plus CI: the formgen job has NEVER run past its install step on a runner
(PyPI playwright pin fixed in 210044a). Later steps hold unknown latent
failures — fixture byte determinism across zlib builds is the known risk.
Iterate: push, watch, fix, until green.

Plus documentation (see below) — kept current as part of every increment,
not as a final pass.

## Documentation architecture (the deliverable, not a chore)

Verdict at time of writing: the METHOD is well documented; the verification
machinery is not, and status numbers are stale in three places.

Target structure — each fact lives in exactly ONE document:

| Document | Owns | Update trigger |
| --- | --- | --- |
| `README.md` | The process end-to-end: why vector not raster, module map incl. gate/validator/fixtures/manifest, how to run everything | when the process changes |
| `STATUS.md` | ALL volatile numbers: gate output, assertion counts, findings tally, CI state. The only doc allowed to contain a measured number | same commit as any change that moves a number |
| `GOAL.md` (this file) | Objective, method, constraints, judgement calls | when the objective changes |
| `review-findings.json` | The defect ledger — scope of record | as findings resolve |
| `BLOCKER-PLAN.md`, `HANDOFF.md` | Historical records; header must say so and point here | never (frozen) |

Rule that keeps it honest: **a commit that changes a number updates STATUS.md
in the same commit.** Stale-number drift across five documents is how the
current state happened.

## Method

- Work in increments; after each, run the affected self-tests, and run the
  full gate before claiming a check moved.
- **One agent per file.** Two agents on emit.py once cost a day.
- A change to extract.py names its caller in the same increment.
- A schema change (batch record keys, provenance keys, manifest shape) is
  declared everywhere it is asserted — gate.py BATCH_RECORD_KEYS and the
  gate's own self-test fixtures — in the same commit.
- comb_referee.py pins its producers by sha256; editing audit.py, extract.py
  or lattice.py requires re-pinning as part of the same commit ritual.
- Never weaken verify.py tolerances; never special-case on form code; the
  pipeline never rasterises (humans reviewing may).
- Findings resolve in the ledger with evidence, in the same commit as the fix.

## Constraints that cannot be broken

Unchanged from the previous goal: exact tolerances (position 0.25pt,
thickness 0.05pt, advance 0.10pt, size 0.01pt); decorative greys stay grey;
deterministic byte-identical output; forms/ is hand-maintained; main stays
clean; report a cost rather than trade it. The official BIR PDFs are never
committed. Judgement calls already made (SVG rule layer, crispEdges off,
MediaBox paper, Arial Narrow via scaleX, clipped straddlers, moved 2551Q
pins) are recorded in git history at b2bd2e9 and stand unless measurement
overturns them.

## Progress

- 210044a ci: PyPI playwright pin (npm 1.58.2 does not exist on PyPI; Python
  line is 1.59.0). First honest CI execution pending.
- Everything before: see git log. Integrity increment complete — gate back to
  its 4 pre-existing failures, three prove-phase faults (f1/g/c2) now caught,
  all 10 self-tests pass, tree byte-identical.

## Blocked

Nothing. The 9 official forms with no source PDF (1600, 1601-E, 1601-F,
1602, 1603, 1604-CF, 1704, 2000, 2200AN) stay out of scope — do not download
them; ask the user.
