# MAIN stray schedule-row validation

Validated commit: `642bfc78221c0a87b318c7c3378c3f5b0b2c9359` on the real Linux desktop. Scratch invented data only; no Queue, no live BIR/relay, no tokens.

## Findings

| question | answer |
|---|---|
| form/schedule | 2551Q Schedule 1, draft JSON key `schedule_1` |
| plain Delete row 1 | blank zero row remains at index 0; row 2 remains intact; page visibly shows one row |
| close/reopen | still one visible row, former row 2 |
| print preview | retained blank row prints as a row of zeros: blank ATC, `0.00` amount, `0.00` tax due |
| generated filed payload | blank row appears as slot 1 zeros: `drpATC1=0`, `txtATCAmt1=0.00`, `txtATCRate1=0.00`, `txtATCDue1=0.00`; totals/item 14 remain `233.31` |
| 1601EQ | no blank row object remains; array compacts, but surviving row also lost former amounts in this path |
| 2550Q Schedule 1 | row object remains cleared/zero for user-entered cells, hidden/computed values survive, page still shows two rows |

## Code reading

1. `nsh/screens/form/controls/Schedule.tsx:96-100` — `remove()` calls `props.onClearRow(r)` and closes the drawer; it never splices/removes the row object.
2. `nsh/screens/form/rows.tsx:160-162` — `onClearRow` writes every visible unlocked cell of that row to `""` through `s.choose(edits)`, leaving a blank row object.

Supporting output path: `crates/bir-core/src/forms/form_2551q.rs:1241-1243` uses `to_bir_xml_payload()` for `submitted_payload`; `crates/bir-core/src/forms/form_2551q_xml.rs:212-242` writes existing schedule rows into fixed payload slots.

## Harness caveat

The generated payload was produced by a temporary local Rust example decoding the real post-delete draft, recomputing it, and invoking `Form2551QDraft::to_bir_xml_payload()` directly. It did not queue or send anything. The scratch fixture had two unrelated validity problems repaired only in memory before generation: Item 13 election set to `NotApplicable`, and PT040's blank description copied from the official description.

## Key evidence

- `results.json`, `code-references.txt`
- `2551q-before.json`, `2551q-after-delete.json`, `2551q-after-delete-draft.json`
- `2551q-print-page2.png`, `2551q-print-schedule1-crop.png`, `2551q-print-sheet.html`
- `2551q-generated.xml`, `2551q-xml-harness.txt`, `2551q-xml-totals.json`, `stray_xml.rs`
- `1601eq-*.json/png`, `2550q-*.json/png`
