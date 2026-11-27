# 1701 Schedule 3.A / 4 / 6 gates — confirm/refute at main db022086 (step 1)

Verdict: **CONFIRMED, with the exact count the independent check reported:
61 boxes** — Schedule 3.A 8, Schedule 4 23, Schedule 6 30. Every disagreement
is in one direction only: the official program shuts the box in the state and
our page leaves it open (371 box-state rows; never the reverse). Every box #337
gated agrees with the official rule in every state, including column B single
versus joint. `gates-main.tsv` is one row per box per state.

## Method, and what the official rule is built from

The official HTA's function bodies are not in this repository; the official
rule per box is rebuilt from the three files the task names, plus the ported
compute chain that cites them line by line:

1. `rules/forms/1701-v2018/fixtures/runtime-control-inventory-v796.json` —
   which boxes exist, which start disabled (the computed ones).
2. `rules/forms/1701-v2018/fixtures/validation-function-inventory-v796.json` —
   the regime functions exist and where: `taxRateEnableDisableFields`
   L11195–L11223, `enableFieldsForGRRate` L11224–L11236,
   `disableFieldsForGRRate` L11270–L11278, `methodRate` L11302–L11379.
3. `crates/bir-core/src/forms/form_1701_official.rs` — the ported compute
   (`official_recompute_chain`, `recompute_nolco_official`) that cites those
   functions: a Schedule 3.A/4/6 input's value survives compute **only** in
   the states the official program types it in, so "open" officially means
   exactly the states where compute keeps the value:
   - Schedule 3.A items 8, 9, 19, 20, 21 (and the item 19/20 description
     boxes): typed `if graduated` — open under graduated rates with business,
     profession or mixed income (ATC II012/II013/II014), either deduction
     method.
   - Schedule 3.A item 11, all of Schedule 4 (items 1–16, 17a–17d and the
     17d description), all of Schedule 6 (summary items 1–2, NOLCO rows and
     their year boxes): typed only `if itemized` within graduated — open
     under graduated rates **with Itemized Deduction** only.
   - The remaining boxes (items 10, 12–18, 22–25 of 3.A; Schedule 4 item 18;
     Schedule 6 items 3, row E columns, 7A/7Year/12A/12Year, totals 8D/13D)
     are computed: statically disabled in the inventory and unbound in
     `data/agent-forms/1701-v2018.json`, so no write path exists for them at
     all — official and ours agree trivially.
   - Column B of any of the above additionally needs a joint return
     (the spouse column mechanism), exactly as for the rest of the form.

"What our page does" is evaluated from the `GATED_BOXES` table in
`crates/bir-app-api/src/screen_form_module/rows_annual_individual.rs` and its
`holds()`: a box absent from the table is open in every state; a gated box is
open exactly when its gate holds. States: fresh (no ATC), graduated+itemized,
graduated+OSD, 8%, compensation only; column B boxes evaluated once as a
single filer and once joint.

## The 61 disagreeing boxes at main

- **Schedule 3.A (8):** items 19A/B, 20A/B, 21A/B and the 19/20 description
  boxes — open in every state, officially open only under graduated rates
  (the same rule #337 already applied to items 8 and 9).
- **Schedule 4 (23):** items 1, 2, 4, 5, 14, 15, 16 columns A+B, rows 17a–17d
  columns A+B, and the 17d description — officially itemized-only. (#337
  gated items 3, 6–13 correctly; these were missed.)
- **Schedule 6 (30):** summary items 1A/B, 2A/B; taxpayer rows 4–7 columns
  A–D plus year (except the 6A/6B/7B three #337 gated); spouse rows 9–12
  likewise — officially itemized-only, spouse rows additionally joint.

In every disagreeing state a typed value is silently zeroed by compute, as
the check reported.

## After the fix (same branch)

`gates-branch.tsv` re-evaluates every box and state against the branch's
`GATED_BOXES` table: **0 disagreeing rows** (was 371). The fix adds no new
mechanism: the 61 boxes take #337's two gate kinds — items 19–21 of
Schedule 3.A and their description boxes `GraduatedBusiness` like items 8–9
(the compute types them in the same `if graduated` block), everything else
`GraduatedItemized`. One judgment call, documented in the code: the three
description boxes (3.A items 19/20, Schedule 4 item 17d) are a single box for
both columns, so they follow column A's gate; compute never clears them, so
the one shut-but-writable case (joint return, only the spouse graduated)
loses nothing.
