# 1701 Item 6 — Single Proprietor + ATC II013 (Mixed Income)

## Finding

- **Official program (`rules/forms/1701-v2018/`)**: Item 6 (`frm1701:rdoPg1I6TaxpayerTypeS/P/E/T/C`) consists of 5 checkboxes (`control_kind: "checkbox"`, `fixtures/runtime-control-inventory-v796.json:156-202`) validated by `TPFilerType` (`fixtures/validation-function-inventory-v796.json:353-366`). For a mixed-income Single Proprietor filing ATC `II013`, the official BIR eBIRForms 7.9.6 HTA marks **both** `Single Proprietor` and `Compensation Earner` (verified by `crates/bir-core/tests/sample-payloads/1701-122025.official.xml`: `rdoPg1I6TaxpayerTypeS=true`, `rdoPg1I6TaxpayerTypeC=true`, `rdoPg1I7ATC_II013=true`).
- **`main` (`db022086`) vs PR #355 (`05b9c989` / `HEAD`)**: Identical behavior on both revisions (`form_1701.rs`, `form_1701_official.rs`, `print_sheet.rs`, and `html-frozen/1701-2018/` have zero Item-6 diffs between `3e8b1212`, `db022086`, and `05b9c989`):
  - `taxpayer_also_compensation_earner = false` -> marks **only** `Single Proprietor` (`p1c23`).
  - `taxpayer_also_compensation_earner = true` -> marks **both** `Single Proprietor` (`p1c23`) and `Compensation Earner` (`p1c27`), matching the official BIR 1701 mixed-income state.
- **Conclusion**: No print regression in PR #355 or `main`. The difference between `old.pdf` (`3e8b1212`) and `new.pdf` (`05b9c989`) reflects `taxpayer_also_compensation_earner` being `false` vs `true` in the two seeded drafts, and ticking both boxes for mixed income (`II013`) is what the official BIR 1701 program requires.

## Evidence (built in isolated target dirs with `RUSTC_WRAPPER=""`)

- `main-db022086-comp-false.pdf` / `crop-item6-main-db022086-comp-false.png`
- `main-db022086-comp-true.pdf` / `crop-item6-main-db022086-comp-true.png`
- `pr355-head-comp-false.pdf` / `crop-item6-pr355-head-comp-false.png`
- `pr355-head-comp-true.pdf` / `crop-item6-pr355-head-comp-true.png`
