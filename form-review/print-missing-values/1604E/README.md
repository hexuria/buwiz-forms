# 1604E Print Missing Values Proof (`feat/forms-print-missing-values`)

## Cause & Fix
- **Cause**: On `html-frozen/1604e-2018/index.html`, Item 1 (`p1c4`, *For the Year (20YY)*) preprints `2 0` in slots 0–1 and provides only two input slots (`2` and `3`). `Form1604EDraft::to_print_field_map()` previously passed the 4-digit upload value (`"2025"`), causing the comb renderer to overprint `"2025"` onto the preprinted `"2"` and read `"225 0"`.
- **Fix**: `Form1604EDraft::to_print_field_map()` in `crates/bir-core/src/forms/form_1604e.rs` now formats `frm1604e:txtYear` as the 2-digit year (`"25"`) for print only; `to_bir_field_map()` / upload XML still emits `"2025"`.
- **Same-kind audit**: Checked every other year box on 1604E (Schedule 1/2 dates use full `MM/DD/YYYY`; `1604F` Item 1 is a full 4-slot year box with no preprinted `20`).
- **Regression test**: `form_1604e_prints_two_digit_year_after_preprinted_20` in `crates/bir-print/tests/print_missing_values.rs`.

## Files
- `before-return.pdf` / `after-return.pdf`: Same synthetic 1604E return (`taxable_year = 2025`, `210,345,678.90` taxes withheld, `2,345.10` penalties) rendered at `origin/main` vs this branch.
- `before-page-1.png`, `before-page-2.png` / `after-page-1.png`, `after-page-2.png`: Full-page PNGs (1403 px wide) of Pages 1–2 before vs after.
- `crop-item1-year.png`: Side-by-side crop of Item 1 (`before` left showing overprinted `2025` on `2`, `after` right showing `2 | 0 | 2 | 5`).
- `recording.mp4`: Walkthrough reel comparing before/crop/after.

## What this does not show
- `recording.mp4` is assembled from the rendered before/crop/after pages rather than a live desktop GPUI screen capture.

## Other Page Notes (Pre-existing on `main`)
- Part II Schedule 1 & Schedule 2 remittance amounts are left-aligned, and untouched quarters/months print `0.00`.
- Item 10 Email Address prints lowercase.
