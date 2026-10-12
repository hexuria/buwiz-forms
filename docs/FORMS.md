# Forms and parity report

This file lists the 43 forms in eBIRForms 7.9.6.2.1 that the app knows, and
how far each one is proven. The source of truth for the form list is
`FORM_CAPABILITY_REGISTRY` in
[`support_level.rs`](../crates/bir-core/src/forms/support_level.rs). If this
page and that file disagree, the code wins.

Counts: 43 forms, 39 can be queued for submission, 4 cannot, 78 official
samples in total, 0 forms marked `release_ready`.

## How parity is proven

- The official runtime in [`tools/official-xml`](../tools/official-xml/README.md)
  runs each official form page (the HTA) in jsdom and writes what the official
  app would upload. These files are `crates/bir-core/tests/sample-payloads/*.official.xml`.
  `sample_payloads_test` fails if our plaintext differs by one byte.
- The official `Encrypt.exe` (sha256 `429337f4...4d2c`, same from 7.9.6.0
  to 7.9.6.2.1) was run under emulation on each `*.plain.xml`. The test pins the
  SHA-256 of that output and requires our `compress_and_encrypt` to match it.
  The helper that computes the pin, `handover/tools/official_encrypt_sha.py`
  (a unicorn emulation of `Encrypt.exe`), lives on the `handover/forms-push-2026-10-11`
  branch together with the official package files it needs; it isn't shipped here
  because it runs BIR's own executable.

"Official samples" counts the `*.official.xml` files per form, grouped by the
text before the first `-` in the file name. One sample is one filled-in
return that was replayed through the official page.

## The 43 forms

Columns:

- **Revision** and **Form type** come from the registry (`revision`, `form_id`).
  The form type is the official `formType` and PROD SFTP folder name.
- **Queue** is the registry's `queue_submission` flag. "no" means the form
  cannot be queued (see below).
- **Printed sheet** is the folder under `html-frozen/` that print preview fills.

| Code | Name | Revision | Form type | Queue | Official samples | Printed sheet |
|---|---|---|---|---|---|---|
| 2551Q | Quarterly Percentage Tax Return | 2018 | `2551Qv2018` | yes | 1 | `2551q-2018` |
| 1601C | Monthly Remittance Return of Income Taxes Withheld on Compensation | 2018 | `1601Cv2018` | yes | 1 | `1601c-2018` |
| 2553 | Percentage Tax Payable Under Special Laws | 1999 | `2553` | yes | 1 | `2553-1999` |
| 1604F | Annual Information Return of Income Payments Subjected to Final Withholding Taxes | 2018 | `1604F` | yes | 1 | `1604f-2018` |
| 1604C | Annual Information Return of Income Taxes Withheld on Compensation | 2018 | `1604C` | yes | 1 | `1604c-2018` |
| 1604E | Annual Information Return of Creditable Income Taxes Withheld | 2018 | `1604Ev2018` | yes | 1 | `1604e-2018` |
| 0619E | Monthly Remittance Form for Creditable Income Taxes Withheld (Expanded) | 2018 | `0619E` | yes | 1 | `0619e-2018` |
| 0619F | Monthly Remittance Form for Final Income Taxes Withheld | 2018 | `0619F` | yes | 1 | `0619f-2018` |
| 0605 | Payment Form | 1999 | `0605` | yes | 1 | `0605-1999` |
| 1701Q | Quarterly Income Tax Return for Individuals, Estates and Trusts | 2018 | `1701Qv2018` | yes | 1 | `1701q-2018` |
| 2550Q | Quarterly Value-Added Tax Return | 2024 | `2550Qv2024` | yes | 3 | `2550q-2024` |
| 1701 | Annual Income Tax Return for Individuals, Estates and Trusts | 2018 | `1701v2018` | yes | 2 | `1701-2018` |
| 1702RT | Annual Income Tax Return — Regular Taxable | 2018C | `1702RTv2018C` | yes | 2 | `1702rt-2018c` |
| 1702MX | Annual Income Tax Return — Mixed Income | 2018C | `1702MXv2018C` | yes | 3 | `1702mx-2018c` |
| 1606 | Withholding Tax Remittance Return for Onerous Transfer of Real Property Other Than Capital Asset (Including Taxable and Exempt) | 2018 | `1606` | yes | 1 | `1606-2018` |
| 1600VT | Monthly Remittance Return of Value-Added Tax Withheld | 2018 | `1600VTv2018` | yes | 3 | `1600-vt-2018` |
| 1600PT | Monthly Remittance Return of Other Percentage Taxes Withheld | 2018 | `1600PTv2018` | yes | 3 | `1600-pt-2018` |
| 1600WP | Remittance Return of Percentage Tax on Winnings and Prizes | 2010 | `1600WP` | yes | 3 | `1600wp-2010` |
| 1601EQ | Quarterly Remittance Return of Creditable Income Taxes Withheld (Expanded) | 2018 | `1601EQ` | yes | 1 | `1601eq-2019` |
| 1601FQ | Quarterly Remittance Return of Final Income Taxes Withheld | 2018 | `1601FQ` | yes | 1 | `1601-fq-2020` |
| 1603Q | Quarterly Remittance Return of Final Income Taxes Withheld on Fringe Benefits Paid to Employees Other Than Rank and File | 2018 | `1603Qv2018` | yes | 1 | `1603q-2018` |
| 1602Q | Quarterly Remittance Return of Final Taxes Withheld on Interest Paid on Deposits and Yield on Deposit Substitutes / Trusts / Etc. | 2018 | `1602Qv2018` | yes | 1 | `1602q-2019` |
| 2552 | Percentage Tax Return on Transactions Involving Shares of Stock | 2018 | `2552v2018` | yes | 2 | `2552-2018` |
| 2550M | Monthly Value-Added Tax Declaration | 2007 | `2550M` | yes | 4 | `2550m-2007` |
| 1706 | Capital Gains Tax Return (Real Properties) | 2018 | `1706` | yes | 1 | `1706-2018` |
| 1707 | Capital Gains Tax Return (For Onerous Transfer of Shares of Stock Not Traded Through the Local Stock Exchange) | 2021 | `1707v2021` | yes | 3 | `1707-2021` |
| 1707A | Annual Capital Gains Tax Return (Shares of Stock Not Traded) | 2021 | `1707Av2021` | yes | 3 | `1707a-2021` |
| 1800 | Donor's Tax Return | 2018 | `1800v2018` | yes | 2 | `1800-2018` |
| 1801 | Estate Tax Return | 2018 | `1801v2018` | yes | 3 | `1801-2018` |
| 2000 | Documentary Stamp Tax Declaration/Return | 2018 | `2000v2018` | yes | 1 | `2000-dst-2018` |
| 2000OT | Documentary Stamp Tax Declaration/Return (One-Time Transactions) | 2018 | `2000OTv2018` | yes | 2 | `2000-ot-2018` |
| 2200S | Excise Tax Return for Sweetened Beverages | 2018 | `2200S` | yes | 2 | `2200s-2018` |
| 2200C | Excise Tax Return for Cosmetic Procedures | 2018 | `2200Cv2018` | yes | 1 | `2200c-2018` |
| 2200AN | Excise Tax Return for Automobiles and Non-Essential Goods | 2018 | `2200ANv2018` | no | 1 | `2200an-2018` |
| 2200M | Excise Tax Return for Mineral Products | 2018 | `2200Mv2018` | no | 2 | `2200m-2018` |
| 2200P | Excise Tax Return for Petroleum Products | 2020 | `2200Pv2020` | no | 2 | `2200p-2020` |
| 2200A | Excise Tax Return for Alcohol Products | 2020 | `2200Av2020` | no | 1 | `2200a-2020` |
| 2200T | Excise Tax Return for Tobacco Products | 2020 | `2200Tv2020` | yes | 1 | `2200t-2022` |
| 1701MS | Annual Income Tax Return for Micro and Small Taxpayers | 2024 | `1701MS` | yes | 2 | `1701ms-2024` |
| 1702Q | Quarterly Income Tax Return for Corporations, Partnerships and Cooperatives | 2018C | `1702Qv2018C` | yes | 1 | `1702q-2018` |
| 1701A | Annual Income Tax Return (8% / OSD) | 2018 | `1701A` | yes | 5 | `1701a-2018` |
| 1700 | Annual Income Tax Return (Purely Compensation) | 2018 | `1700v2018` | yes | 3 | `1700-2018` |
| 1702EX | Annual Income Tax Return — Tax-Exempt | 2018C | `1702EXv2018C` | yes | 2 | `1702ex-2018` |

## Forms that cannot be queued

2200AN, 2200M, 2200P and 2200A cannot be queued for submission. BIR has no
PROD upload folder for the four 2200 excise forms (issue #87). The editor,
validation and print preview still work. The header hides Submit for them.
They still have official samples, so the plaintext layout is proven; only the
upload target is missing. 2200T, 2200S and 2200C can be queued.

## Notes on printed sheets

- **1601EQ** (code revision 2018) prints on the January 2019 sheet
  (`1601eq-2019`). No January 2018 sheet is available. The form's preview
  message says so.
- **1601FQ** (code revision 2018) prints on the September 2020 sheet
  (`1601-fq-2020`). No January 2018 sheet is available. The form's preview
  message says so.
- **2200T** (registry revision 2020) prints on the 2022 sheet (`2200t-2022`,
  source "2200-T Aug 2022").
- The other folder names match the registry revision, with these spelling
  differences: `1600-pt-2018` and `1600-vt-2018` (1600PT, 1600VT),
  `2000-dst-2018` (2000), `2000-ot-2018` (2000OT), and `1602q-2019` (1602Q,
  registry revision 2018).
- 2550M uses the 2007 sheet (`2550m-2007`), 1600WP the 2010 sheet, 2553 and
  0605 the 1999 sheet.

## Implementation status

Last checked against the code on 12 October 2026 (branch `forms/integration`,
commit `fe86f57d`).

- All 43 forms have these registry flags set: typed model, XML round trip,
  formula evidence, persistence, editor, render contract, HTML component, HTML
  spec and pagination. 39 also have `queue_submission`.
- No form is `release_ready`. The release gate also needs `visual_parity`,
  `native_preview`, `native_print`, `pdf_export` and `packaged_offline`, and
  those flags are `false` for every form. So every form reports `ScaffoldOnly`
  as its support level. Queue authority is separate and does not need those
  flags.
- There is no fallback renderer. A form that cannot print stays blocked with a
  diagnostic instead of switching to another document engine.
- `packages/form-specs/form-migration-status.json` and
  `form-release-evidence.json` are older than the registry. The migration file
  lists 10 forms and was last changed on 19 July 2026. Do not read them as the
  current list.

## Frozen HTML preview

Print preview uses the committed freeze sheets in `html-frozen/`. There is one
folder per form, with `index.html`, `form.css`, `guide.html`, `provenance.json`
and `writer-cells.json`. `html-frozen/inventory.json` lists the 43 bundles.

- The app fills catalog-stamped TIN and branch `name=` inputs from
  `to_bir_field_map()`.
- Identity leftovers (name, address, RDO, ZIP, telephone, email) and demo tax
  money rows fill onto cell ids through `writer-cells.json`. Letter combs are
  upper-cased on the fill path only. Profile and database values stay as
  stored.
- 2551Q also fills Year Ended (MM/YYYY) and the quarter boxes. 1601C fills
  For the Month (MM/YYYY) and the Amended Return and Any Taxes Withheld boxes.
- Opening output fills one frozen HTML document. Preview and print reuse that
  document. They never change the draft's status.
- CI runs `freeze_html.py --verify` and `stamp_frozen_names.py --check-all`
  against `hexuria/buwiz-forms-html`.
- Official PDFs are calibration evidence only. They are not used to print.
- What the preview window looks like depends on the platform. See
  [macOS](platforms/macos.md), [Windows](platforms/windows.md) and
  [Linux](platforms/linux.md).

## Unverified

- Form names come from `crates/bir-core/src/forms/registry.rs`. They were not
  compared one by one with the title printed on each official form.
- The issue number #87 for the 2200 forms is as given by the project owner.
  It is not recorded in the code.
