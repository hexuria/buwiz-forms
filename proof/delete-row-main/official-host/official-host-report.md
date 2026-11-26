# Official-form comparison and host row-removal inventory

MAIN `bcbaf324187137010b19814d6ef6e59db4d1f5d9`; static reading only. Full machine-readable evidence is in `official-host-inventory.json`.

## 2551Q Schedule 1
- Delete/remove control: none found; fixed six-row table
- Mechanism: No Add/Delete row builder. Choosing the zero/blank ATC in a fixed row disables and zeros amount/rate in place; no row compaction is evidenced.
- Compact/clear conclusion: clear/zero in place only; no removal control and no compaction
- Evidence:
  - `rules/forms/2551q-v2018/evidence.md:3-5` — Applying that predicate to the pinned DOM produces 98 static occurrences; `getRdo()` injects the RDO select into `frmMain`, producing 99 occurrences total. There are 98 distinct serialized keys because the DOM contains two `txtEmail` controls and the loop emits both under the same key. Six ATC rows are concrete and bounded. No Add/Delete row builder or unbounded family exists.
  - `rules/forms/2551q-v2018/validations.json:317-337` — "rule_id": "2551q-atc-row-enable" ... "condition": "An ATC row selection is zero or nonzero." ... "source_refs": ["official-hta-runtime#drpATCChanged:L2976-L2997"] ... "official_behavior": "Zero selection disables and zeros amount/rate; a selected ATC enables amount and copies the catalog rate."
  - `rules/forms/2551q-v2018/calculations.json:18-35` — "calculation_id": "2551q-atc-row-tax" ... "outputs": ["txtATCDue{1..6}"] ... "official_formula": "For row N, tax due = amount * (rate / 100)." ... "trigger": "computeATC" ... "official-hta-runtime#computeATC:L2998-L3005"

## 1601EQ ATC schedule
- Delete/remove control: no per-row Delete/Remove control found. The official UI uses the ATC checkbox popup; runtime inventory also records a popup-level Clear button (`btnClearOtherAtc`) but not a row delete button.
- Mechanism: Rows are positional projections of ticked ATC checkboxes. Rows 1-6 always exist after populateAtcPart2; rows 7-N exist only when more than six ATCs are selected. Thus unticking reduces/rebuilds the selected-row projection in popup order rather than leaving an independent row object. The exact HTA statements for unticking are not quoted in the repository artifacts, so this compaction conclusion is an evidence-backed inference from the documented positional row existence and getATCCode refs, not a literal source quote.
- Compact/clear conclusion: checkbox selection model; rows are rebuilt/compact positional slots. No evidence of a table-row clear-in-place Delete control.
- Evidence:
  - `rules/forms/1601eq-v2018/evidence.md:15-19` — The representative dummy save contains the initial six ATC rows. Source inspection proves up to 111 dynamic row positions. Because each added row contributes four serialized keys, the complete category-dependent source-reachable union is 621 fields. ... ATC row slots are positional within the selected Private/Government catalog, not permanent meanings.
  - `rules/forms/1601eq-v2018/fixtures/runtime-control-inventory-v796.json:1297-1316` — "id_pattern": "AtcCode{1..N}", "control_kind": "checkbox", "minimum_instances": 0, "maximum_instances": 111, "existence": "N is 111 for Private and 96 for Government after Item 11 category filtering." ... "id_pattern": "frm1601EQ:txtAtcCd{1..N}" ... "existence": "Rows 1-6 always exist after populateAtcPart2; rows 7-N exist when more than six ATCs are selected." ... "official-hta-runtime#populateAtcPart2:L2861-L2875" ... "official-hta-runtime#getATCCode:L2890-L3031"
  - `rules/forms/1601eq-v2018/fixtures/runtime-control-inventory-v796.json:985-997` — "id": "btnClearOtherAtc" ... "control_kind": "button" ... "source_line": 1093 ... "value": " Clear "
  - `rules/forms/1601eq-v2018/calculations.json:63-67` — "condition": "Only additional ATC rows numbered 7 and above." ... "official_formula": "AdditionalATCTotal = sum(TaxWithheld[row], row=7..(compute-table rows + other-table rows - 4))." ... "trigger": "Additional ATC row computation, selection change, clear, or year/rate refresh."
  - Port cross-check: crates/bir-core/src/forms/form_1601eq.rs:5-14 says the port is of changedrpATCList/getATCCode/populateAtcPart2 and dynamically adds popup checkboxes/rows; remove_atc at lines 874-877 retains nonmatching rows and recompute sorts rows in popup order at lines 891-895.

## 2550Q Schedule 1
- Delete/remove control: Add/Delete behavior exists for the dynamic rows, including Schedule 1; no maximum-row guard.
- Mechanism: Repository evidence proves Add/Delete and unbounded indexed families, but does not quote enough of the dynamic-row-builder implementation to establish whether official Delete compacts rows up or clears in place. Serialization metadata uses assigned stable row ids and marks serialization order unresolved, so it does not settle compaction either.
- Compact/clear conclusion: unestablished from the repository official-form artifacts; do not infer. Host runtime behavior compacts backing arrays, but that is host behavior, not proof of the official HTA mechanism.
- Evidence:
  - `rules/forms/2550q-v2024/evidence.md:20-20` — The concrete union therefore has 160 keys. Source inspection also proves 28 unbounded indexed families: nine Schedule 1 fields, five Schedule 3 fields, six Schedule 4 fields, and description/amount pairs for Items 19, 42, 47, and 56. The HTA has Add/Delete behavior and no maximum-row guard, so `fields.json` records family descriptors instead of inventing a finite row count.
  - `rules/forms/2550q-v2024/fields.json:5894-5900` — "source_refs": ["official-hta-runtime#dynamic-row-builders:L5830-L6760", "official-hta-runtime#serialization:L5160-L5820"] ... "The HTA exposes Add/Delete without a maximum-row guard; this descriptor prevents a false finite inventory claim."
  - `rules/forms/2550q-v2024/fixtures/serialization-binding-inventory-v796.json:97-110` — "group_id": "schedule-1-capital-good-row" ... "instance_identity": "assigned-stable-id", "instance_order": "legacy-zero-based-index-ascending" ... "serialization_instance_order_relation": "unresolved" ... "official-hta-runtime#dynamic-row-builders:L6089-6217"
  - `rules/forms/2550q-v2024/validations.json:789-805` — "rule_id": "2550q-s1-empty" ... "condition": "Any materialized Schedule 1 row field is blank." ... "exact_message": "Empty fields are not allowed " ... "official-hta-runtime#schedule1-validation:L5830-L6357"
  - Host contrast: The current host row module compacts emptied Schedule 1/3/4 backing rows (rows_2550q.rs:175-192), but the official Delete mechanism is not established by the quoted repository artifacts.

## Host inventory summary

Page path for every inventoried form: `Schedule.tsx remove -> rows.tsx onClearRow -> blank string writes -> screen_form_fill (session numeric-zero retry/removes confirmation semantics)`.

| form | schedules | exposed host row-delete call | typed removal/compaction method | blank-write compaction |
|---|---:|---|---|---|
| 1600PT | 3 | none | Form1600PtDraft::toggle_atc | yes for ATC rows: matrix rows 1600PT_frm1600PT_txtAtcCd1 and txtAtcCd6 compacted; host blank ATC branch removes `atc_rows[row]` in rows_monthly.rs:291-298 |
| 1600VT | 3 | none | Form1600VtDraft::toggle_atc | yes for ATC rows: matrix rows 1600VT_frm1600VT_txtAtcCd1 and txtAtcCd6 compacted; same rows_monthly.rs:291-298 mechanism |
| 1600WP | 2 | none | Form1600WpDraft::toggle_atc | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1601C | 1 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1601EQ | 2 | none | Form1601EqDraft::remove_atc | yes: matrix rows 1601EQ_frm1601EQ_txtAtcCd1 and txtAtcCd7 compacted; blank ATC calls remove_atc in catalogs.rs:516-526 and rows re-sort |
| 1601FQ | 3 | none | Form1601FqDraft::remove_atc | yes: matrix rows 1601FQ_frm1601FQ_txtAtcCode1 and txtAtcCode7 compacted; blank ATC calls remove_atc in rows_withholding.rs:373-382 |
| 1602Q | 2 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1603Q | 1 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1604C | 1 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1604E | 2 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1604F | 3 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1700 | 2 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1701 | 3 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1701A | 2 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1702EX | 5 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1702MX | 1 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1702RT | 6 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1707 | 6 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1707A | 3 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1800 | 2 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 1801 | 6 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 2000 | 4 | none | Form2000Draft::clear_atc | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 2000OT | 2 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 2200A | 11 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 2200C | 1 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 2200M | 8 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 2200P | 3 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 2200S | 1 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 2200T | 25 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 2550M | 7 | none | Form2550MDraft::remove_sales_atc | yes across the seven inventoried row tables in the runtime matrix; rows_2550m.rs:220-225 removes a fully blank/default row and later rows move up; sales ATC also has typed retain removal |
| 2550Q | 7 | none | Form2550QRawEditorState::remove_repeated_row | yes for Schedule 1/3/4 backing arrays: matrix rows txtDatePurchase10, txtDateCovered30 and txtDate40 compacted; rows_2550q.rs:175-192 removes an emptied row and then restores only the minimum trailing rows |
| 2551Q | 1 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 2552 | 8 | none | none found | no compaction detected in the Phase 2/3 runtime matrix rows for this form |
| 2553 | 2 | none | Form2553Draft::clear_atc | limited/yes for row 18 only in matrix: rows_dst_pt_vat.rs:385-390 calls clear_atc on blank ATC and form_2553.rs:347-355 pops trailing empty rows; fixed printed rows 14-17 do not compact |
