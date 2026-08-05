# STATUS — formgen, measured state

**Update rule: any commit that moves a number below updates this file in the
same commit.** This is the only formgen document allowed to hold measured
status numbers (`GOAL.md` owns that rule; `README.md` owns the process).

Measured 2026-08-06 at HEAD `5072249`. Gate verdicts are from the most recent
full gate over this corpus; assertion counts, the findings tally and the
referee count were re-verified directly against `build/audit.json`,
`review-findings.json` and `build/comb-referee.json` at this HEAD. CI state is
from the workflow run logs.

## Gate — 8 of 12 checks pass

| Check | Verdict | Detail |
| --- | --- | --- |
| self-tests | PASS | all 10 module self-tests |
| conversion | PASS | 51/51 unique tracked forms converted |
| rules | PASS | |
| paper | PASS | |
| artwork | **FAIL** | `images_missing=1` on 1701ms-2024 |
| text | PASS | |
| assertions | **FAIL** | 5 of the 8 assertions fail (below) |
| findings | **FAIL** | 52/84 blocker+major unresolved |
| tracked-files | PASS | no tracked deletion |
| audit-refresh | PASS | |
| determinism | PASS | two regenerations, byte-identical |
| comb-referee | **FAIL** | 19 source/layout/emission mismatches |

## Failing assertions (`build/audit.json`)

| Assertion | Forms | Offenders |
| --- | --- | --- |
| `inputs_over_printed_text` | 48 | 336 audit-visible; true population 359 (see below) |
| `comb_slots_match_printed` | 51 | 825 |
| `money_boxes_have_inputs` | 6 | 9 |
| `reflow_rate_without_description` | 1 (2551m-2002) | 1 |
| `image_transform_applied` | 1 (1702q-2018) | 1 |

## Findings ledger (`review-findings.json`, 138 findings)

| Severity | Open | Fixed |
| --- | --- | --- |
| blocker | 16 | 5 |
| major | 36 | 27 |
| minor | 36 | 4 |
| cosmetic | 12 | 2 |

The gate counts blocker+major only: 52 open of 84. Worst forms: 1801-2018 (6),
1707-2021 (5), 1800-2018 (4), 1702q-2018 (3).

## CI

The formgen job went green for the first time on 2026-08-05 (run 31040386488,
2m10s, at this HEAD), after three fixes: `210044a` (PyPI playwright pin — npm's
1.58.2 does not exist on PyPI), `99b63ed` (`PLAYWRIGHT_BROWSERS_PATH=0` so
Chromium resolves inside the hashed Playwright closure; gate probes now name
what diverged), `5072249` (two macOS-only assumptions: Linux `Popen` uses
`posix_spawn` internally; the mutation probe now accepts detection as well as
prevention). `gh pr checks 13` was not yet fully green at the time of this
measurement — other workflows' jobs were still pending on the same push.

## Open issues, diagnosed

One row per issue; root causes from the 2026-08 triage. "Owner" is the file the
fix belongs in.

| Issue | Root cause | Owner |
| --- | --- | --- |
| artwork: 1701ms-2024 p1 BIR seal reported missing | Digest-provenance mismatch: the IR pins `pixel_sha256` from the in-memory composited pixmap (`9623eb6e…`), but the shipped asset is that pixmap after a MuPDF PNG encode/decode round-trip that is not bit-faithful (3 of 7,900 partial-alpha seal-edge pixels, red off by 1 → `e4c70b62…`). `verify.diff_images` pairs images by digest and cannot pair the two. Compositing and Chromium's re-embed are proven bit-exact; finding F037's "black rectangle" evidence is stale. | `extract.py` |
| `image_transform_applied`: 1702q-2018 p3 | 1 source placement, emitted 0, orientation mismatch per the audit offender record. Triage (task A.2) classified it on the extract/emit image path; full diagnosis was cut off in the triage report — re-derive before fixing. | `extract.py`/`emit.py` (unconfirmed) |
| `inputs_over_printed_text`: 48 forms | Three populations. (a) 2 of the 336 records are not overlaps: 1701-2018 p4c216 and 2551m-2002 p2c24 are guide-cut straddlers deliberately clipped per their guide plans, but `emitted_cell_binding_issues` compares the emitted rect against the *unclipped* layout rect, and each false positive replaces the form's real offender list (masking 18 offenders on 1701, 7 on 2551m). (b) Of the true population of 359 cells, 161 are an input over its *own* field's pre-printed decoration (comb decimal/thousands markers, pre-printed centavo zeros, rate constants, TIN dashes, century "2 0", date-format hints, own-comb ATC constants) — correct behaviour kept fillable by design, flagged by the predicate anyway. (c) The remainder are the genuine-overlap families; their breakdown was cut off in the triage report. | `audit.py` (predicate + binding comparison) |
| `comb_slots_match_printed` + comb-referee: 19 mismatches | All 19 are legacy-lattice comb subjects whose current partition has no rectangular owner cell, so emit suppresses them (`lattice.py:3785-3840`, state `retained_unresolved`, blocks gate). Referee-proven real combs wrongly suppressed: 2000-dst-2018 p1c4 (6 slots, 14.04pt pitch), 2550m-2007 p1c99 (4 slots, 11.04pt). Near-certainly real, needs referee proof: 1707-2021 p1c50 and 1707a-2021 p1c40 (25 dividers ~14.4pt), 2200c-2018 p1c3 (23 dividers, TIN-style). Phantom, suppression correct, legacy lattice wrong: 0605-1999 p1c20 (divider 0.63pt from the left rail). Remaining subjects' verdicts were cut off in the triage report — adjudicate per subject. | `lattice.py` ownership / `emit.py` suppression, per subject |
| `money_boxes_have_inputs`: 6 forms, 9 boxes | Pop A (7 boxes: 1601eq-2019 p2c14/16/40, 1801-2018 p2c91, 2200a-2020 p2c14/p3c13, 2200p-2020 p2c25): layout says empty but the box holds printed glyphs whose runs straddle the cell boundary; emit's PrePrintedInk verdict (coverage 0.61–0.91 vs 0.5) correctly refuses an input — an input there would let a taxpayer type over a printed ATC code — and the audit predicate misses the measured-ink case. Pop B (2 boxes) is the same guide-cut binding false positive as the row above. | `audit.py` |
| `reflow_rate_without_description`: 2551m-2002 p2 | The ATC band has no ruled grid, so the rate table reflowed as table+prose and row structure is not recoverable ("1 relocated rate row(s)/table(s) lost their description", marker "ALPHANUMERIC TAX CODE (ATC)"). | reflow path (`guides.py`/`emit.py`), not fully diagnosed |
| findings: 52 blocker+major open | Nine root causes per `BLOCKER-PLAN.md` (historical); each finding must end `fixed` or `not-a-defect` with evidence, in the same commit as its fix. Round 4 visual re-review closes the loop. | `review-findings.json` + per-cause owners |
