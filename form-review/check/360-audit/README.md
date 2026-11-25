# Re-check of draft PR #360 (feat/forms-print-missing-values @ 71a5ae77) — print audit

Independent re-run of the print audit from `try/print-audit-all` (622d2f59) at three commits,
compared **on the rendered sheet** (the filled frozen-html document, as paper would show it),
exact and case-sensitive, per forms-validation's job of 2026-11-26.

Build folders (one per commit, `RUSTC_WRAPPER` empty, rustup-pinned cargo 1.99.0 (5f94df478 2026-08-27)):
`~/ci/p360/target-main` (75bd78b8), `~/ci/p360/target-pr` (71a5ae77), `~/ci/p360/target-main2` (db022086), `~/ci/p360/target-merged` (merge test).

## Method, and where it differs from the original audit

The original checker's comparison script is not part of its evidence (only outputs are), so the
comparison was reimplemented. At the **print-map level** at 75bd78b8 the reproduction matches
table.md exactly on the three load-bearing columns: DIFFERENT VALUE 10, HELD BUT NOT PRINTED 316,
case-only 103. The same/blank/printed-empty columns do not reproduce exactly (normalization details
unrecoverable from the outputs); this is stated, not smoothed.

The rendered comparison: each sample draft is built with the suite's own builders (the original
scratch test's exact sample set and builder choices, including its wrong-builder mistake for
1701A-122025-joint-many, kept deliberately for reproduction), the print field map is filled into the
frozen sheet with `bir_print::frozen_html::filled_document`, and every `<input>` value in the filled
document is grouped by cell in document order (combs by slot). NOTE: `printed_cells()` is NOT
sufficient for this — it drops every plain-text (non-comb) cell. Known residual comparison artifacts,
identical across commits so they cancel in cross-commit diffs: multi-line comb wrapping (long
addresses), date zero-padding (`1/01/2025` vs `01/01/2025`), decimal-comb digit assembly.

## Files

- `per-key-rendered-{75bd78b8,71a5ae77,db022086}.tsv` — one row per sample/key with the rendered verdict.
- `per-key-printmap-75bd78b8.tsv` — the print-map-level reproduction at main.
- `table-main-vs-pr.md` — per-form rollup, rendered, both commits.
- `table-main-vs-main2.md` — per-form rollup, 75bd78b8 vs db022086.
- `notes.md` — points 3, 4, 6 and the merge test (added as finished).

## Headline results

- 1702MX rendered at main: 89 same / 312 held / 10 DIFFERENT; at 71a5ae77: 411 same / 0 / 0 — the fix is real on paper.
- Regressions (same at main, anything else at 71a5ae77): exactly 2 rows — frm1604c:txtYear and
  frm1604e:txtYear printing `25` instead of `2025`, the author's documented intentional exception
  (sheet preprints "20"). Zero unintended regressions.
- db022086 vs 75bd78b8: only 6 rows differ at all, all comma-formatting improvements on 1702MX
  schedule cells (`200000` → `200,000`), verdict unchanged. New baseline ≈ old baseline.
