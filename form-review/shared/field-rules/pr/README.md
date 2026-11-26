# Part 3 Proof — Print Field Rules (`feat/store-launch-field-rules`)

- **Before (`before/`):** `origin/main` (`92b3e201`, with `nsh` pinned at `8c78cfd6d`)
- **After (`after/`, `recordings/`):** `feat/store-launch-field-rules` (`2d3773c1`, merged with `origin/main` `92b3e201`, including Part 1a–j and 1600WP Schedule 1.1 total restoration)
- **Additional artifacts:**
  - `1600wp-restore/` — 1600WP page 1 with filled Schedule 1.1 rows 1–2 and totals (`p1c241` / `p1c243`) at `before-merge.png` (`de513578`), `after-merge.png` (`f8e8ed8a`), and `after-restore.png` (`3074d666`, preserved at `2d3773c1`).
  - `font-sizes.md` — Rendered font-size audit measured via headless Chromium (`getComputedStyle` under `@media print`) across all 43 frozen forms.

## Directory Layout & What Each File Shows

- `before/<FORM>/pdf/pN.png` and `after/<FORM>/pdf/pN.png`: Every page of the saved PDF rendered from the running desktop app (`nsh-run-agent` with `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, fresh scratch SQLite DB, demo taxpayer, invented values, Queue never pressed).
- `before/<FORM>/preview/pN.png` and `after/<FORM>/preview/pN.png`: Every print-preview page captured from the same print sheet.
- `before/<FORM>/saved-<form>.pdf` and `after/<FORM>/saved-<form>.pdf`: The saved PDF documents.
- `recordings/<FORM>.mp4`: Under-two-minute screen recording at `after` head showing the saved return reopened on the form page and paged through the print preview.

## What This Proof Does NOT Show

1. **1702MX typed NOLCO columns A–D (Part 1j):** PR #326 is not yet on `main`, so the app's 1702MX draft only populates row 7 A & E (not typed rows 4–6 A–D). Part 1j's `.00` stripping, thousands grouping, and right alignment on typed columns A–D are proven by `frozen_html::tests::form_1702mx_schedules_7_and_8_whole_peso_right_align_and_row7_default`; page 4 in `after/1702MX/pdf/p4.png` shows the live right-aligned comma-formatted row 7 and untouched zeros.
2. **1701 Part IV item 1 Spouse TIN (Part 1f):** The Part 3 harness filled Spouse Name, RDO, Contact, Citizenship, Foreign Tax No., and Schedules 3.A/4/6 spouse columns, but left Spouse TIN blank on both `before` and `after`. Full 14-digit second-person TIN + non-zero branch code printing is proven by `frozen_html::tests::second_person_tins_print_all_fourteen_digits_including_branch_code`.
3. **Preview vs. PDF page count on `1707` (6 vs. 7) and `2000OT` (8 vs. 9):** Additional-sheet overflow tables render inside one scrollable `.page-continuation` container in preview and paginate cleanly into physical pages in PDF; there are no blank trailing pages.

---

## Per-Form, Per-Page Inspection (`before/` vs `after/`)

### `0605` (1 page before / 1 page after; recording 6s)
- **Page 1:**
  - *Before:* Item 15 Registered Address shrinks onto a single tiny line; Part II items 19, 20A–20D, and 21 are left-aligned inside their boxes.
  - *After:* Item 15 wraps cleanly onto 2 lines at `>= 7.5pt`; Part II items 19, 20A–20D, and 21 are right-aligned with centavos neatly flanking the preprinted decimal dot.
  - *Still wrong on both:* Item 10 RDO Code is blank because the demo return omitted it in `0605`.

### `0605-137` (1 page before / 1 page after; recording 6s)
- **Page 1:**
  - *Before:* Full 137-character Taxpayer's Name (item 13) shrinks to tiny single-line text (~4.8pt) and 137-character Registered Address (item 15) shrinks to ~4.65pt and clips at the right edge (`...MAKATI CITY METRO`, losing `MANILA PH.`); Part II amounts left-aligned.
  - *After:* Full 137-character Taxpayer's Name (item 13) and 137-character Registered Address (item 15) wrap onto two readable lines at `>= 7.5pt` without clipping or dropping below the Android ~8 px floor; Part II amounts right-aligned on the preprinted decimal dot.

### `1601C` (2 pages before / 2 pages after; recording 10s)
- **Page 1:**
  - *Before:* Item 9 address text collides with comb divider ticks; negative items 31 & 36 (`(Over-remittance)`) print with a leading `-` inside comb boxes.
  - *After:* Item 9 prints in clean plain rectangles with comb ticks knocked out; items 31 & 36 print parenthesized `(47,541,854.87)` right-aligned in plain rectangles.
- **Page 2:**
  - *Before / After:* Schedule 1 rows and totals match between preview and PDF; header TIN/name clean.
  - *Still wrong on both:* Item 7 RDO Code blank; base `.fh20` comb slots on `1601c-2018` sit at `6.96pt` in the frozen sheet CSS (see `font-sizes.md`).

### `1701` (4 pages before / 5 pages after; joint return; recording 27s)
- **Page 1:**
  - *Before:* Negative Part II items 26, 31, and 32 print in comb boxes with leading minus signs or vanish on overflow; item 9 address hits comb ticks.
  - *After:* Joint return (`X Married`, `X Joint Filing`) with both columns populated; item 9 clean plain rectangle; negative items 26/31/32 cleanly parenthesized `(110,962,209)`, `(100,471,518)`, `(211,433,727)`.
- **Page 2:**
  - *Before:* 10-digit Schedule 3.A item 8A (`1,234,567,890`), items 13 & 16 (`1,156,942,024`), and negative items 6 & 18 VANISH completely (blank boxes).
  - *After:* Item 8A prints `1,234,567,890` right-aligned in a plain rectangle while its 9-digit neighbor 8B (`9 8 7 6 5 4 3 2 1`) retains its comb boxes; items 6, 13, 16, and 18 all print cleanly right-aligned with commas.
  - *Still wrong on both:* Part IV item 1 Spouse TIN blank in this draft (see #2 above).
- **Page 3:**
  - *Before:* Schedule 4 item 18 (`1,156,942,024`) vanishes; Schedule 6.A.1 amounts print left-aligned with `.00` decimals and no commas, and total `8D` prints at tiny `3.4pt`.
  - *After:* Schedule 4 item 18 prints `1,156,942,024` right-aligned; every Schedule 6.A.1 amount (both columns) prints whole-peso, comma-grouped, right-aligned at `>= 7.5pt`, with `8D` at `8.48pt`.
- **Page 4:**
  - *Before:* Schedule 6.A.2 (Spouse NOLCO) prints left-aligned with `.00` and no commas; Part VIII items 3/7/10 and Part IX items 5/10/11 vanish completely.
  - *After:* Schedule 6.A.2 whole-peso right-aligned with commas; Part VIII totals (`1,274,310,919`, `1,278,632,906`, `1,308,886,815`) and Part IX totals/loss (`2,695,569,396`, `(138,303,584)`) all print cleanly.
- **Page 5 (After only — Additional Sheet):**
  - Carries full text of truncated Part VII item 9 (`SAMPLE ENTRY TXTPG4IPART7_9SPECIFY`) with `Taxpayer's Name: SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA`.
  - *Still wrong on after:* Additional sheet header shows `TIN: —` on 1701.

### `1702MX` (4 pages before / 5 pages after; recording 27s)
- **Pages 1–3:**
  - *Before:* Schedule 2 & 3 whole-peso zeros left-aligned touching left border; overflowing comb amounts vanish.
  - *After:* Whole-peso zeros and amounts right-aligned with commas; overflowing amounts render as plain right-aligned rectangles.
- **Page 4:**
  - *Before:* Schedule 7.1 & 8.1 row 7 amounts (`8698066981`, `8615949228`, `0`) are left-aligned with no thousands commas; Schedule 10 zeros missing/unaligned.
  - *After:* Schedule 7.1, 8.1, and 10 amounts all right-aligned with thousands commas (`8,698,066,981`, `8,615,949,228`, `0`) in whole pesos.
- **Page 5 (After only — Additional Sheet):**
  - Carries full truncated text entries cleanly with taxpayer name and period header.

### `1702Q` (2 pages before / 2 pages after; recording 11s)
- **Page 1:**
  - *Before:* Part II items 14–25 all print `0 . 0 0` into whole-peso boxes (`DO NOT enter Centavos`); item 9 address hits comb ticks.
  - *After:* Part II items 14–25 all print clean whole-peso `0` without `.00` or doubled commas; item 9 clean plain rectangle.
- **Page 2:**
  - *Before / After:* Schedules 1–4 print cleanly with preview/PDF parity.

### `1706` (2 pages before / 3 pages after; recording 13s)
- **Pages 1–2:**
  - *Before:* Seller/Buyer long names and addresses shrink below 7.5pt or collide with combs.
  - *After:* Long fields stay at `>= 7.5pt` with ellipsis and carry full text to Page 3; Schedule 1 classification (`derived:sched1_class1/2`) maps cleanly.
- **Page 3 (After only — Additional Sheet):**
  - Prints `TIN: 000-123-453-00000`, `Taxpayer's Name: SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA` (fixing the `—` defect), `Date of Transaction: 01/01/2025`, and all 7 truncated entries. Saved PDF ends cleanly on page 3 (no blank page 4).

### `1707` (2 pages before / 7 PDF pages, 6 preview pages after; recording 20s)
- **Pages 1–2:**
  - *Before:* Long seller/buyer names and addresses crush into tiny type or clip against comb dividers.
  - *After:* Clean plain rectangles at `>= 7.5pt`, truncated entries carried to Additional Sheets.
- **Pages 3–7 (After only — Band Continuation & Additional Sheets):**
  - Band-cloned schedule rows and Additional Sheets (3 of 3) carry all overflowing entries with zero blank pages.
  - *Still wrong on after:* Additional sheet header shows `TIN: —` and repeats the buyer/corporate name string in `Taxpayer's Name` when multiple buyer name keys are populated.

### `2000OT` (2 pages before / 9 PDF pages, 8 preview pages after; recording 21s)
- **Page 1:**
  - *Before:* Item 16 Tax Rate (`P15 / P1,000`) is left-aligned; items 7, 8, 11A, and 13 collide with comb ticks.
  - *After:* Item 16 Tax Rate (`P15 / P1,000`) is centered per RULES 4; items 7, 8, 11A, and 13 render in clean plain rectangles.
- **Page 2 & Pages 3–9 (After only — Continuation & Additional Sheets):**
  - Overflowing Schedule 1.A rows and truncated Classification entries continue cleanly onto Additional Sheets (`1 of 5` through `5 of 5`) with `TIN: 000-123-453-00000` and `Taxpayer's Name: SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA`.

### `2200P` (2 pages before / 7 pages after; recording 21s)
- **Page 1:**
  - *Before / After:* Background info and Part II/III totals right-aligned; long address cleanly handled.
- **Page 2:**
  - *Before:* Header TIN runs directly into Taxpayer's Name (`00012345300000SANTOS, JUANA...`) with no gap; Schedule 1 amounts shrink down to ~3–4pt and bottom `TOTAL TAX DUE` clips at the right edge.
  - *After:* Header TIN has a clear two-space gap before Taxpayer's Name (`00012345300000  SANTOS, JUANA...`); 7.5pt floor enforced across all cells.
  - *Trade-off / still imperfect on after:* In 2200P's narrow Schedule 1 columns (`Locally Manufactured`, `Imported`, `Under Bond`, `Exports`), 12-character hundreds-of-millions amounts at the 7.5pt floor wrap mid-number onto 2 lines or ellipsize (`475,999,03...`) with full values carried to Additional Sheets (pages 3–7).
- **Pages 3–7 (After only — Additional Sheets):**
  - Carry all truncated Schedule 1 values cleanly with header TIN and Taxpayer's Name.

### `2550M` (2 pages before / 3 pages after; recording 18s)
- **Page 1:**
  - *Before:* Item 9 Registered Address shrinks tiny on one line; EVERY Part II amount (`12A`–`26`) is left-aligned.
  - *After:* Item 9 stays at `>= 7.5pt` with ellipsis (full address on Page 3); EVERY Part II amount (`12A`–`26`) is right-aligned.
- **Page 2:**
  - *Before / After:* Schedule 1–8 amounts right-aligned.
  - *Still wrong on both:* Base frozen stylesheet `.fh*` classes in narrow Schedule 6/7/8 cells on `2550m-2007` are `5.20pt`–`7.36pt` (see `font-sizes.md`).
- **Page 3 (After only — Additional Sheet):**
  - Prints `TIN: 000-123-453-00000`, `Taxpayer's Name: SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA` (fixing the `—` defect), and full Item 9 Registered Address.

### `2550Q` (2 pages before / 2 pages after; recording 10s)
- **Page 1:**
  - *Before:* Item 10 address, item 12 email, and item 14A collide with comb ticks; negative item 21 (`(Excess Credits)`) squeezes `- 1 6 9 8 8 2 7 8 1 7 . 7 3` into comb boxes.
  - *After:* Items 10, 12, and 14A render in clean plain rectangles; item 21 prints `(1,698,827,817.73)` right-aligned in a plain rectangle.
- **Page 2:**
  - *Before / After:* Part IV & V schedules clean with preview/PDF parity; base `.fh0` schedule cells sit at `6.71pt` in frozen sheet CSS (see `font-sizes.md`).

### `2551Q` (2 pages before / 2 pages after; recording 9s)
- **Page 1:**
  - *Before:* Item 9 address & item 11 email hit comb ticks; negative items 19 & 24 (`(Overpayment)`) print `- 2 7 0 7 5 5 6 6 7 . 2 5` across comb slots.
  - *After:* Items 9 & 11 clean plain rectangles; items 19 & 24 print `(270,755,667.25)` and `(270,730,667.25)` right-aligned in plain rectangles.
- **Page 2:**
  - *Before / After:* Schedule 1 rows and totals match between preview and PDF.

### `1600wp-restore/` (3 images)
- `before-merge.png` (`de513578`): Schedule 1.1 rows 1–2 and TOTAL row print `P 1,500,000.00` (`p1c241`) and `P 150,000.00` (`p1c243`).
- `after-merge.png` (`f8e8ed8a`): Taking `origin/main`'s regenerated `html-frozen/1600wp-2010/index.html` made `p1c241` and `p1c243` empty `data-cell-kind="label"` divs, leaving both TOTAL boxes blank.
- `after-restore.png` (`3074d666`, unchanged at `2d3773c1`): Restored both joins in `writer-cells.json` and added runtime promotion of explicitly joined empty `<div></div>` cells in `frozen_html.rs`; both TOTAL boxes print `P 1,500,000.00` and `P 150,000.00` identically to `before-merge.png` while `freeze_html.py --verify` (`43 bundles`) stays green.
