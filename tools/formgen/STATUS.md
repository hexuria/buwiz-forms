# STATUS — formgen, measured state

**Update rule: any commit that moves a number below updates this file in the
same commit.** This is the only formgen document allowed to hold measured
status numbers (`GOAL.md` owns that rule; `README.md` owns the process).

Measured 2026-08-06 at HEAD `9d44e2a`, over the 51-form corpus regenerated at
`768dacc`. Assertion counts are from a corpus-wide `audit.py
--assertions-only` run at these exact producer bytes; the findings tally is
recomputed from `review-findings.json`; per-check gate verdicts are from the
three full gate runs described below, none of which ran to a complete
verdict at this HEAD. Corpus-expansion work (1604-CF, 2200AN; see `8161a82`)
was in flight in this worktree when these numbers were taken and will move
them; per the update rule, that work updates this file when it lands.

## Gate — no complete clean-tree verdict at this HEAD yet

Three full runs on 2026-08-06, each stopped by a different, real thing:

1. **r1 (dirty tree)** — the audit application scope demands a clean
   worktree; the fix-phase edits were uncommitted. Committed as
   `125cf7e`..`768dacc` and re-run.
2. **r2 (clean tree at `768dacc`)** — audit-refresh failed with "layout cell
   and reviewed-subject owner registries differ": the gate's owner-registry
   projection re-derived subject order from cell numerals while the
   restored 2550M comb owner `p1c193` sits mid-stream. A real integration
   fault, fixed in `19d4460` (canonical order is the layout cell stream;
   the referee now proves ledger order instead of assuming it).
3. **r3** — invalidated mid-run by concurrent commits (`13b4cbd`,
   `19d4460`, `9d44e2a` landed between its two generations), so generation
   attestation correctly refused to score it. Not a defect: the attestation
   exists precisely to catch a moving tree.

Checks that did evaluate, at the run that evaluated them:

| Check | Verdict | Run | Detail |
| --- | --- | --- | --- |
| self-tests | PASS | r2 | all 10 modules at `768dacc`; at `9d44e2a` + in-flight corpus expansion, `guides` fails under the concurrently rewritten `build/` — re-measure once the expansion lands |
| conversion | PASS | r2 | 51/51 unique tracked forms converted |
| findings | FAIL | r3 | 43/84 blocker+major unresolved (worst: 1801-2018 6, 1707-2021 5, 1800-2018 4, 2316-2021 3) |
| tracked-files | PASS | r1–r3 | no tracked deletion |
| determinism | PASS | r1, r2 | two regenerations byte-identical (`c928b792f9d8`) |
| rules/paper/artwork/text/assertions/audit-refresh/comb-referee | UNEVALUABLE | all | blocked by the per-run causes above; next quiescent clean-tree run scores them |

## Failing assertions (corpus-wide `--assertions-only` at `9d44e2a` bytes)

| Assertion | Forms | Offenders | Movement from `5072249` |
| --- | --- | --- | --- |
| `inputs_over_printed_text` | 38 | 232 | was 48 forms / 336 audit-visible (sliver B2 fix + binding false positives cleared; populations A, B1, C1, C2 remain per the 2026-08 triage) |
| `comb_slots_match_printed` | 21 | 181 | was 51 forms / 825 (POSITION_TOL_PT alignment + dominant-topology rule; residual is genuine defects plus the still-refused U-frame-crop / corridor-absorb topologies) |
| `money_boxes_have_inputs` | 0 | 0 | was 6 forms / 9 (measured-ink `boxes_preprinted` exclusion + clipped-straddler binding fix) |
| `reflow_rate_without_description` | 0 | 0 | was 1 (2551m-2002; multi-section `data-flow` + content-shaped row hazard scan, rows now actually checked) |
| `image_transform_applied` | 0 | 0 | was 1 (1702q-2018; guide-plan `relocated_placements` subtraction, published) |

Artwork: the 1701MS digest-provenance fix landed (`125cf7e`); regeneration
moved exactly one pixel digest corpus-wide (the page-1 seal, `9623eb6e…` →
`e4c70b62…`, asset bytes unchanged), so `verify.diff_images` can pair the
seal. `images_missing=0` still needs a completed audit refresh to be a gate
verdict.

Comb referee: the last complete ledger (pre-fix, 2026-08-06 00:22) carried
19 `emission_layout_mismatches`; the lattice restoration of the two
source-proven suppressed combs (2000-DST p1c4, 2550M p1c99-era subject) and
the referee's new stream-order proof await the next complete run for a
measured count.

## Findings ledger (`review-findings.json`, 138 findings)

| Severity | Open | Fixed |
| --- | --- | --- |
| blocker | 14 | 7 |
| major | 29 | 34 |
| minor | 36 | 4 |
| cosmetic | 12 | 2 |

The gate counts blocker+major only: 43 open of 84 (was 52). Nine resolved at
`9d44e2a` with measured evidence: eight stale dropped-SMask findings (F020,
F021, F022, F037, F048, F053, F057, F061 — shipped assets decode with the
masked pixels transparent, re-inspected visually over white) and F127 (the
2551M guide is a 19-row paired gl-table, no prose flattening). Worst forms:
1801-2018 (6), 1707-2021 (5), 1800-2018 (4), 2316-2021 (3).

## CI

The formgen job went green for the first time on 2026-08-05 (run 31040386488,
2m10s), after `210044a` + `99b63ed` + `5072249`. The commits above have not
yet been pushed through a full CI cycle at the time of this measurement.

## Open issues, diagnosed

One row per issue; root causes from the 2026-08 triage, updated for what
landed. "Owner" is the file the fix belongs in.

| Issue | State | Root cause / residual | Owner |
| --- | --- | --- | --- |
| artwork: 1701ms-2024 seal reported missing | fix landed (`125cf7e`), verdict pending a completed audit refresh | digest-provenance: IR pinned the in-memory pixmap hash; the bundle ships a non-bit-faithful PNG re-encode. `shipped_pixel_sha256` now hashes the decoded samples of the exact shipped bytes. | `extract.py` (done) |
| `image_transform_applied` 1702q-2018 | fixed, assertion holds corpus-wide | assertion never subtracted guide-plan-claimed placements; now subtracts by the plan's `image_indices`, publishes `relocated_placements`, fails closed on a plan/source split | `audit.py` (done) |
| `inputs_over_printed_text`: 38 forms, 232 offenders | partially fixed (48→38 forms) | B2 slivers demoted in lattice; binding false positives cleared. Remaining: A-populations (own-field decoration, needs the lattice-ownership export + assertion exemption), B1 (glyph ascent/descent boxes), C1/C2 (genuine comb/caption overlaps) | `lattice.py` + `audit.py` + `emit.py`, per the triage plan |
| `comb_slots_match_printed`: 21 forms, 181 offenders | partially fixed (51→21) | tolerance + dominant-topology landed; residual: `crops a wider source U-frame` / `absorbs unframed corridors` / `competing band-tone` refusals not yet covered by the referee-aligned chooser, plus genuine geometry defects (2550m p1c103 3.12pt, p1c193 rails) | `audit.py` topology chooser / `emit.py`-`lattice.py` per subject |
| `money_boxes_have_inputs` | fixed, assertion holds corpus-wide | measured-ink exclusion (`boxes_preprinted`), clipped-straddler binding, and the sliver/kind conflict all landed | `audit.py` (done) |
| `reflow_rate_without_description` | fixed, assertion holds corpus-wide | checker now reads multi-section `data-flow` and scans gl-table rows content-wise | `audit.py` (done) |
| findings: 43 blocker+major open | in progress (52→43) | remaining families: comb capacity (referee track), inputs-over-text populations, guide-cut orphan policy (F004/F007/F090), text mis-position (F024/F070/F102), individual re-verifications | per-cause owners |
| comb-referee ledger | await next complete run | two source-proven combs restored via `source_certified_replacement_owner`; referee now proves stream order; expected 19→17 or fewer, needs the measured run | `comb_referee.py` run |
