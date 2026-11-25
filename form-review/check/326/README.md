# Proof Package — Independent Check of PR #326 (`feat/store-launch-unfillable-rows`)

Checked commits:
- **Claims 1 & 2 (2551Q and the five tables) and Part C**: `13e6abe4`
- **Claim 3 (1702MX NOLCO)**: initially observed at `13e6abe4`, then re-verified at the updated branch head **`2555a2d8`**
- **Baseline**: `origin/main` (`4399ce84` for `sample_payloads_test`; `3aceafe8` for Part C 3-form diff and branch base)

All app runs used only `nsh-*` recipes, a fresh scratch DB (`/home/ubuntu/BuwizNsh/check326.db`), the synthetic demo taxpayer (`SANTOS, JUANA`, TIN `000-123-453-00000`), and `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1` (`BIR_SFTP_LIVE` unset; Queue never pressed).

## What Each File Shows (and Does Not Show)

### `REPORT.md`
- Full independent review report: tested commits, official ATC comparison, 3-way arithmetic/XML/paper tables, official sample XML box-by-box diffs, Claim 2 table audit, Part C diff reading + shared write-path audit + 3 other forms, paper audit against `RULES.md`, and ranked verdicts.

### `screenshots/` (live GPUI desktop app captures, 1280×900)
- `claim1-2551q-sched1.png`: 2551Q Schedule 1 with all 6 rows filled (`PT010`, `PT060`, `PT105`, `PT103`, `PT090`, `PT170`) and hundreds-of-millions amounts with centavos; shows calculated rates, row tax dues, and Schedule 1 total `211,111,111.11`.
- `claim1-2551q-part2.png`: 2551Q Part II showing Item 14 (`211,111,111.11`) through Item 24 (`278,985,951.11`).
- `claim1-2551q-reopened.png`: Same 2551Q return reopened from Filings after saving and navigating away, proving persistence.
- `claim2-1600wp.png`: 1600WP Schedule 1.1 Row 1 drawer showing Nature of Payment hint (`Shows the ATC's nature of payment once this row's ATC is chosen.`) and Tax Rate showing `₱ 0.00` (does not show `"Follows the ATC chosen in this row."`).
- `claim2-1601fq.png`: 1601FQ Part IV Schedule 1 Row 1 drawer showing Nature of Income Payment (D) as `-` (read-only, no explanatory note).
- `claim2-1701.png`: 1701 Part V Schedule 6 Row 4 drawer showing Column E (`Net Operating Loss (Unapplied) [(E)= A – (B+C+D)]`) as a non-editable computed `0.00` display.
- `claim2-2000.png`: 2000 Schedule 1 Row 1 drawer showing Tax Rate hint (`Shows the ATC's rate once this row's ATC is chosen.`).
- `claim2-2553-row14.png`: 2553 Part II Row 14 drawer showing `Printed: Philippine Amusement and Gaming Corp. (Pagcor), OT010, 5%`, `Printed: OT010`, and locked `0.00 %` Tax Rate.
- `claim2-2553-row18.png`: 2553 Part II Row 18 drawer showing `Shows the classification of the ATC chosen in 18B.` and editable ATC/Taxable Amount/Tax Rate.
- `claim3-1702mx-sched7.png`: 1702MX Schedule 7.1 rows 4–6 filled on the form page with 12 NOLCO amounts, showing computed Col E (`150,000,000`, `140,000,000`, `90,000,000`) and Row 8 total (`185,000,000`).
- `claim3-1702mx-sched8.png`: 1702MX Schedule 8.1 rows 4–6 filled on the form page with 12 NOLCO amounts, showing computed Col E (`225,000,000`, `183,000,000`, `145,000,000`) and Row 8 total (`180,000,000`).
- `claim3-1702mx-reopened.png`: Same 1702MX return reopened from Filings after saving and navigating away, proving persistence.

### `2551q/` (1400 px wide PNGs + saved PDF, tested at `13e6abe4`)
- `2551q-saved.pdf`: Saved PDF printed from the live app's generated preview HTML for the 6-row 2551Q return.
- `2551q-preview-page-{1,2}.png` and `2551q-pdf-page-{1,2}.png`: Preview and saved-PDF pages 1–2 (1400 px wide).
- `2551q-official-page-{1,2}.png`: Official blank reference pages rendered at the same 1400 px scale from `html-frozen/2551q-2018/index.html` (the raw source PDF named in `html-frozen/2551q-2018/provenance.json`, `2551Q Jan 2018 ENCS final rev 3_copy.pdf`, is not stored on this VM).
- `2551q-compare-page-{1,2}.png`: Side-by-side official page (left) beside app saved-PDF page (right) at identical scale.

### `1702mx/` (1400 px wide PNGs + saved PDFs, tested at `2555a2d8`)
- `1702mx-saved.pdf`: Saved PDF rendered from the UI-saved 24-box 1702MX return via `to_print_field_map()` at `2555a2d8`.
- `1702mx-formfill-saved.pdf`: Saved PDF rendered from the separately filled `form.fill` 24-box 1702MX return at `2555a2d8`.
- `1702mx-preview-page-{1..4}.png` and `1702mx-pdf-page-{1..4}.png`: Preview and saved-PDF pages 1–4 (1400 px wide).
- `1702mx-formfill-pdf-page-4.png`: Page 4 from the `form.fill` return (1400 px wide), proving parity with typed UI entry.
- `1702mx-official-page-{1..4}.png`: Official blank reference pages rendered at 1400 px from `html-frozen/1702mx-2018c/index.html` (`1702-MX Jan 2018 ENCS Final with OSDv2.pdf` is not stored on this VM).
- `1702mx-compare-page-{1..4}.png`: Side-by-side official page (left) beside app saved-PDF page (right) at identical scale.

### `xml/` (plain-text XML payloads and box-by-box official sample diffs)
- `2551q-6row-upload.xml.txt`: Full field-map XML for the 6-row hundreds-of-millions 2551Q return (note: `to_bir_xml_payload()` itself rejects rows 3–6 with a pre-existing `main` 7-cell validation error; see `REPORT.md`).
- `2551q-official-sample-diff.txt`: Box-by-box text diff of our 2551Q replay against `crates/bir-core/tests/sample-payloads/2551Q-122025Q1.official.xml`.
- `1702mx-ui-upload.xml.txt`: Official upload XML (`official_xml::write`) for the UI-typed 24-box 1702MX return at `2555a2d8`.
- `1702mx-formfill-upload.xml.txt`: Official upload XML for the `form.fill` 24-box 1702MX return at `2555a2d8` (0 NOLCO diffs vs UI).
- `1702mx-official-sample-diff.txt`: Box-by-box text diff of our `1702MX-1225` replay against `crates/bir-core/tests/sample-payloads/1702MX-1225.official.xml` (0 NOLCO diffs across all 50 NOLCO boxes).

### `recordings/` (`.mp4`, under 2 minutes each)
- `2551q-flow.mp4` (~25 s): Live GPUI app captures stepping through 2551Q period setup, Schedule 1 rows 1–6 (choosing ATCs, entering amounts, rates & taxes updating), Part II totals, save/reopen from Filings, followed by rendered print pages 1–2.
- `1702mx-flow.mp4` (~29 s): Live GPUI app captures stepping through 1702MX period setup, typing Schedule 7.1 rows 4–6 and Schedule 8.1 rows 4–6 (Col E and Row 8 totals updating), save/reopen from Filings, followed by rendered print pages 1–4 at `2555a2d8`.
- **What the recordings do not show**: The native Linux WebKit print-preview popup cannot be captured inside the headless X11 container, so the print step shows the rendered pages produced from the app's generated preview HTML rather than a live OS print dialog.
