# 1701 — Page 2 Schedule 1 Employer Name and TIN

## Cause
- `html-frozen/1701-2018/writer-cells.json` left Page 2 Schedule 1 employer name and TIN cells (`p2c55`, `p2c56`, `p2c58` for Row 1; `p2c60`, `p2c61`, `p2c63` for Row 2) in `not_joined_cells` because the official sheet shares one employer name (35 + 16 comb slots) and one 14-digit TIN comb per row regardless of Taxpayer/Spouse owner, whereas the XML splits `TPName`/`SName` and `TIN1`/`TIN2`/`TIN3`/`BranchCode`.

## Fix
- Added print-only derived keys `derived:sched1_row{1,2}_name_line{1,2}` and `derived:sched1_row{1,2}_tin` in `crates/bir-core/src/forms/form_1701.rs::to_print_field_map()` (upload XML `to_bir_field_map()` unchanged).
- Joined all 6 cells (`p2c55`, `p2c56`, `p2c58`, `p2c60`, `p2c61`, `p2c63`) in `html-frozen/1701-2018/writer-cells.json` and regenerated `forms/print-cells.json`.
- Same-kind check: both Row 1 (Taxpayer) and Row 2 (Spouse) employer names (lines 1–2) and 14-digit TINs are fixed; the Taxpayer/Spouse checkboxes (`p2c54`, `p2c59`) and compensation/withheld columns were already joined.

## Test
- `form_1701_prints_schedule1_employer_names_and_tins` in `crates/bir-print/tests/print_missing_values.rs` (fails before fix, passes after).

## Proof files (same synthetic joint Juana Santos + Juanito Santos return, `origin/main` before vs head after)
- `before-return.pdf` / `after-return.pdf`: full 4-page printed PDFs.
- `before-page-{1..4}.png` / `after-page-{1..4}.png`: every page rendered at ~1400 px wide.
- `crop-sched1-employers.png`: before (top, blank) vs after (bottom, both Row 1 Taxpayer and Row 2 Spouse employer names across lines 1–2 and 14-digit TINs).
- `recording.mp4`: 11-second before/crop/after proof reel (does not show live GPUI desktop interaction).

## Additional pre-existing `main` page notes
- Part IV Item 1 Spouse's TIN (`000-456-789-00000`) and 10-digit whole-peso amounts on Page 2 are intentionally left untouched per instructions (handled on `feat/store-launch-field-rules`).
- Untouched whole-peso boxes on Page 2 print `0`.
