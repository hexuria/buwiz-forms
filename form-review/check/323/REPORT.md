# Independent check of pull request #323 at 0ef364ec

Commit tested: `0ef364ec74c017d492fee56103d778159f75522e` (head verified; I authored the merge commit at the owner's earlier order but none of the change itself). Base for the diff reading: the merge-base with origin/main (`00a20033`). Method: the page's own write path through the agent control plane (`form.fill` — the same host locks and recompute the page uses, no test setters) on fresh scratch databases, plus the running Linux GUI in docker on Xvfb for everything a person sees. Demo taxpayer only, invented values, the four safety variables on every run, nothing sent anywhere.

## 1. Box tables — official program vs our page at 0ef364ec

### 1701MS Part IV.A (inventory-v47.json)

| box | official program | ours at 0ef364ec | agree |
|---|---|---|---|
| txtGrossa (1A) | enabled at load (L2389, dis=false) | typeable | yes |
| txtGrossb (1B) | disabled at load (L2404) — joint-gated | absent until joint filing; typeable then | yes |
| **txtLessNonTaxablea (2A)** | **enabled at load (L2426, dis=false, ro=false) — typed** | typeable; value held while Schedule A open | yes (was locked-computed before this PR) |
| **txtLessNonTaxableb (2B)** | **disabled at load (L2433) — opens on joint** | absent when single; typeable when joint | yes |
| txtTaxableCompensationIncomea/b (3A/3B) | disabled (L2449/2456) — computed = gross − non-taxable | computed, not typeable | yes |

Differing boxes in the part, whether or not the PR touched them: none found. Note the page renders the shut spouse column as **absent rows**, not greyed inputs — the official program disables but renders. Behaviorally equivalent for the filer; visually different.

### 2552 Part II (inventory-v796.json, calculations.json)

| box | official program | ours at 0ef364ec | agree |
|---|---|---|---|
| items 14/15/16 | disabled at load (L822/837/849) — computed | computed, not typeable | yes |
| **item 17** | **disabled at load (L866) — an INPUT of the calc chain producing item 19 (calculations[6]); the claim's amended condition is the port's documented reading of the runtime** | absent from the page when Amended = No; appears and accepts typing when Yes; gated in rows_as_needed.rs (`Needs Amended Return = Yes`) | yes |
| item 19 | disabled (L887) — computed 17+18 | computed | yes |
| items 20/25 | disabled (L898/958) — computed chain | computed | yes |

One mismatch in mechanism, not outcome: through the agent write path a non-amended Item 17 fill is *accepted* (`filled`, no refusal) but the form's recompute zeroes `tax_paid_previous` unless `is_amended` — the value never survives, matching the shut box behaviorally, but the fill reply does not say so. On the page itself the box is absent, so a person cannot type it at all. (The refusal-with-reason mechanism is PR #338's scope, not merged, not this PR.)

### 2200A Schedule 1 Others rows (inventory-v796.json)

| box | official program | ours at 0ef364ec | agree |
|---|---|---|---|
| others1 Rate/Taxable | enabled (L2191/2197) — typed | typed | yes |
| **others1/2/3 Due** | **enabled (L2200/2226/2252, dis=false ro=false) — typed; the official program computes nothing for these rows** | typed; stored at `/others/{0,1,2}/amounts/tax_due` | yes (absent as inputs before this PR) |
| TotalDue | disabled (L2280) — computed | computed | yes |

Rows open through the row editor (Add row), as on the page.

## 2. Upload comparison — official samples replayed through the page write path

Full key-by-key text in `upload-comparison.txt`; the replay transcripts in `probes/`.

- **1701MS-122025** (joint): 2A 90,000 / 2B 90,000 stored exactly; computed taxable compensation 810,001 / 510,000 matches the official serialization exactly. Every compared key reproduces.
- **2200A-03142025**: row 1 Due 11.00 stored exactly; schedule total 87,451.56 matches; Part III totals consistent (total payment 140,473.68, penalties 54,156.69).
- **2552-03142025**: Item 17 stored 100.50 exactly; Item 19 computed 1,101.00 — matching the official serialization *including its whole-peso rounding* (100.50 + 1,000.25 = 1,100.75 → 1,101.00). Items 16/20/25 could not be reproduced end-to-end because the transaction radio (`rdoPg1I11TransactionLSE`) cannot be written through the agent surface at all — "written through the form's own editors" — so the tax-due side of the chain stayed 0.00 in the replay. This limitation is pre-existing on main (nothing in this PR's diff touches it) and does not affect the PR's boxes; in the GUI flow the transaction is chosen in its editor and the chain follows (see recording).

## 3. Arithmetic — large invented amounts, hundreds of millions with centavos

- **2552** amended: Item 17 = 123,456,789.01 + Item 18 = 98,765,432.99 → Item 19 = 222,222,222.00 exact; chain to 20/25 internally consistent. Oversized 99,999,999,999.99 accepted and carried (Item 19 = 100,098,765,433.0 — the chain's rounding matches the official sample's). Negative −5,000.25 accepted into the box (the official box is freeform-typed; noted, not judged a fault).
- **1701MS** joint: gross 500,000,001 − non-taxable 123,456,789 = taxable 376,543,212 exact; spouse 300,000,000 − 98,765,432 = 201,234,568 exact; tax due followed the graduated schedule (print shows 131,192,624 / 69,834,599 — centavo-less, as 1701MS paper instructs).
- **2200A**: row due 123,456,789.01 typed over rate×taxable; schedule total carries it exactly; Part III follows. On paper the printed row and the total match the draft.

## 4. What the claim does not mention (point 3)

- **Un-amending a 2552 after typing Item 17**: the stored value is zeroed (`tax_paid_previous` 0.0, page shows 0.00 after re-open); the filer is **not asked**. The official program keeps a shut box empty too, so the outcome matches; the silence is the host's standard recompute.
- **Switching a 1701MS from joint to Not Applicable after typing**: the spouse column is reset entirely (official behavior — "resets the whole spouse column off a joint return"), no confirmation. Observed additionally: both columns' compensation values were zero in the final draft — the item-5 change recomputes the income-source state and closes Schedule A. Filer not asked either way.
- **Negative and oversized values**: accepted, no complaint (see arithmetic).
- **Save, close, reopen**: values kept (verified on the 2552 and 1701MS drafts across save/reopen cycles in the GUI flow and the probe saves).

## 5. On paper

No official PDFs exist in the repository frozen folders (provenance.json names files that are not present), so the comparison is of the app's print pages themselves, alignment judged **from the pictures**:

- 2552: Item 17 prints its large value in the right box, once; totals in place; nothing clipped or over a border in the pages produced. The transaction date prints `00/00/2025` — because the transaction date keys cannot be written through the agent surface at all (pre-existing); a person types them on the page normally.
- 1701MS: large values fit the digit boxes without clipping in both columns; layout clean.
- 2200A: the Others row prints on page 3 with the typed due; total correct. Two faults visible on paper, **both pre-existing on main, neither caused by this PR**: Part IV payment details print blank even when the draft holds them (see 2200-part4-crop.png — exactly what the earlier review reported), and place codes print as numbers rather than names. ATC digits print small/cramped beside `XA` but legible.

## 6. Nothing else moved

- `cargo test --locked -p bir-core --test sample_payloads_test`: **146 passed, 1 ignored at 0ef364ec**; origin/main has the same corpus (146 — this branch adds no samples; confirmed identical directory listings).
- `git diff -w origin/main...0ef364ec` (merge-base 00a20033) file by file: forms/schema-overrides.json (computed:false + evidence on the six keys), crates/bir-core/src/forms/agent/probe.rs (4 FILER_TYPED entries), crates/bir-app-api/src/screen_form_module/rows_as_needed.rs (the Amended2552 gate only), screen_form_module.rs and tests (gated-count 10→11 for 2552, the branch's own tests), generated agent-carry.json / agent-forms entries exactly matching the unlocks. Nothing beyond the three boxes; no serializer, probe-path or unrelated-form change.

## Verdicts

- **1701MS: clean.** Official sample reproduces exactly through the page path; states match the official conditions; paper clean.
- **2552: clean**, with one observation ranked *cosmetic*: the agent write path accepts a non-amended Item 17 fill silently (the value is then zeroed by recompute) instead of refusing — the page itself is correct, and the refusal mechanism belongs to the unmerged #338, not this PR.
- **2200A: clean for what it touches.** The paper faults (blank Part IV payment block, numeric place codes) are pre-existing on main and untouched by this PR; ranked *looks wrong on paper / missing data on paper* but explicitly not caused here.

One fault worth owner attention regardless of cause, ranked by harm: 2200A's blank printed Part IV means a filed paper copy omits payment details the return actually contains — pre-existing on main, not this pull request.
