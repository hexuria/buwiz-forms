# PR #355 Independent-Check Fixes Proof (Faults 1–5)

Before commit: `3e8b1212` (built in `/home/ubuntu/BuwizNsh/target-355-before`, `RUSTC_WRAPPER=""`).
After commit: head on `feat/store-launch-field-rules` (built in `/home/ubuntu/BuwizNsh/target-355-after`, `RUSTC_WRAPPER=""`).
Every app run used the running NSH desktop app with `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, a fresh scratch SQLite database, demo taxpayer `00012345300000`, and invented values (never `BIR_SFTP_LIVE`, never Queue).

## Recording
- `fixes-1-to-5.mp4` (25.0s): running NSH app opening, filling, saving, and printing all six forms (`1601C`, `0605`, `1701`, `2550Q`, `2551Q`, `2000OT`) at the fixed head.

## Fault-by-Fault Proof

### 1. `1601C/` — Item 20 "Other Non-Taxable Compensation (specify)"
- `before.pdf`, `before-p1..3.png`, `crop-item20-before.png`: at `3e8b1212`, long specify text wrapped onto 2 lines inside the 9.21pt box (`inner_height = 7.71pt`), cutting through line 1 and showing only a fragment of line 2.
- `after.pdf`, `after-p1..3.png`, `crop-item20-after.png`: embeds Tinos woff2, removes `white-space:pre-line`, measures inner input height, and applies Rule 6 (`formgen-page-layout` `rule6_fields`) to place the full 89-character text in a clean full-width rectangle on a new line below item 20 while keeping the item 20 amount (`250,000.00`) level with item 20.

### 2. `0605/` — Taxpayer Name (137-char long-word value)
- `before.pdf`, `before-p1.png`, `crop-name-before.png`: at `3e8b1212`, missing Tinos font embedding + `white-space:pre-line` + un-inset cell-height measurement caused the checker's 137-char long-word name to soft-wrap onto 3 lines, cutting lines 1 and 3.
- `after.pdf`, `after-p1.png`, `crop-name-after.png`: fits cleanly on 2 lines inside item 13's box (`>= 7.5pt`, `line-height: 1.04`, `Tinos-Regular` embedded), never 3 lines, never cut.

### 3. `1701/` — Additional Sheet TIN Header (all 43 forms audited)
- `before.pdf`, `before-p1..5.png`, `crop-additional-tin-before.png`: at `3e8b1212`, Additional Sheet header prints `TIN: —`.
- `after.pdf`, `after-p1..5.png`, `crop-additional-tin-after.png`: Additional Sheet header prints `TIN: 000-123-453-00000`.
- `additional-sheet-tin-audit.txt`: programmatic 43-form audit (`20` forms affected at `3e8b1212` — 19 missing TIN, 1 missing branch code; `43/43` pass after generalizing `extract_tin` in `crates/bir-print/src/continuation.rs`).

### 4. `2550Q/` and `2551Q/` — Rule 6 Named Specify Boxes & Full Audit
- `2550Q/before.pdf`, `before-p1..3.png`, `crop-item19-before.png` vs `after.pdf`, `after-p1..2.png`, `crop-item19-after.png`: item 19 (`p1c205`) moves from truncated `"..."` inside the tiny box to a full-width Rule 6 rectangle on a new line below item 19, displaying the full 89-character value.
- `2551Q/before.pdf`, `before-p1..3.png`, `crop-item17-before.png` vs `after.pdf`, `after-p1..2.png`, `crop-item17-after.png`: item 17 (`p1c187`) moves from truncated `"..."` inside the tiny box to a full-width Rule 6 rectangle on a new line below item 17, displaying the full 89-character value.
- `2550Q/specify-boxes-audit.txt`: full 43-form audit of all 95 specify-related cells (`31` covered by Rule 6 metadata in `formgen-page-layout`, `50` wide comb fields under Rule 1/3, `14` plain text fields under Rule 3; any remaining ellipsized value emits `data-overflow-warning="text-truncated"` + `data-writer-value` and prints in full on the Additional Sheet).

### 5. `2000OT/` — Page 2 & Continuation (`Add'l Sheet`) Taxpayer-Name Header
- `before.pdf`, `before-p1..5.png`, `crop-p2-header-before.png`, `crop-p3-header-before.png`: at `3e8b1212`, `p2c6` and its cloned `Add'l Sheet` continuation header wrapped onto 3 cut lines (same root cause as Fault 2).
- `after.pdf`, `after-p1..5.png`, `crop-p2-header-after.png`, `crop-p3-header-after.png`: fixed by the same Fault 2 shared fix — 2 clean lines inside the box on both Page 2 and Page 3 (`Add'l Sheet`).
