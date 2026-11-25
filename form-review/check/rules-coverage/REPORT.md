# Rules Coverage & Print Audit — Class `(d)` Keys (`1801`, `1707A`, and All Other Forms) at `main` (`db022086`)

- **Audited revision**: `main` at `db022086`
- **Source audit**: `try/print-audit-all` at `622d2f59` (`form-review/check/print-audit/notjoined/{classification.tsv,final-table.md}`)
- **Build & render environment**: `RUSTC_WRAPPER=""`, `/home/ubuntu/.cargo/bin/cargo`, isolated empty `CARGO_TARGET_DIR=/home/ubuntu/.cargo-targets/main-db022086-audit`, `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1` (`BIR_SFTP_LIVE` unset).

---

## Executive Finding: Why `classification.tsv` Marked 572 Keys as Class `(d)`

All **572/572** class `(d)` rows across the 8 forms match our upload XML byte-for-byte (`official.xml == plain.xml`, 0 mismatches). They were NOT missing from `rules/forms/<form>/fields.json` except in two narrow cases:

| Form | Class `(d)` rows | Literal match in `fields.json` | Matches `{index}`/`{N>=1}` template in `fields.json` | Truly absent from `fields.json` | Cause of `(d)` classification |
|---|---:|---:|---:|---:|---|
| `1801-2018` | 210 | 0 | **210** (37 templates) | 0 | `classify.py` looked up concrete `_3`..`_21` keys instead of `{index}` templates |
| `1707a-2021` | 124 | 0 | **124** (11 templates) | 0 | Same — concrete `_1`..`_11` modal keys vs `{N>=1}` templates |
| `1707-2021` | 77 | 0 | **77** (6 templates) | 0 | Same — `S2_1..12Col1..4`, `S3_1..12Col1..2` vs `{N>=1}` templates |
| `2200m-2018` | 44 | 0 | **44** (17 templates) | 0 | Same — `txtSched1*11..16` vs `{n>=11}` templates |
| `1702ex-2018` | 41 | **24** | **17** (11 templates) | 0 | Rules dir is `1702ex-v2018c` (not `1702ex-v2018`); 24 static + 17 popup keys |
| `2200p-2020` | 28 | 0 | **28** (15 templates) | 0 | Same — `txtSched1*28..31` vs `{n>=28}` templates |
| `1702mx-2018c` | 28 | 0 | **21** (5 templates) | **7** | 21 match `txtPg6Sc6_{N>=1}Col1..5`; **7** are computed `Col6` (Total) missing from `fields.json` |
| `1700-2018` | 20 | 0 | 0 | **20** | `rules/forms/` only has `1700-v2013`, not `1700-v2018` (12 static + 8 modal employer keys) |

**What IS genuinely missing for a filer on `main` (`db022086`)**:
1. **Paper print (`bir-print` on `main`)**: `main` has **no continuation / band-flow engine** (`crates/bir-print/src/` contains only `frozen_html.rs` and `html_output.rs`). Consequently, **100% of dynamically added / modal schedule rows on all 8 forms are filed in XML and summed in the sheet's `TOTAL` lines, yet printed NOWHERE on paper** (only the fixed base pages render). *(Note: `try/forms-band-flow` at `4321b79b` already implements continuation/band-flow printing for these growable tables, pending merge after #355 and #360.)*
2. **Form page row caps (`forms/schema-overrides.json` + `nsh/screens/form/controls/Schedule.tsx`)**: Our form page DOES show all these schedules and lets a filer add/edit rows via **"Add row"**, but every table is statically capped at `rows: [first, last]` in `schema-overrides.json` rather than truly unbounded like the official HTA.

---

## 1. BIR Form 1801 (`1801-2018`, 210 class `(d)` keys)

### 1.1 What the keys are & how the official HTA creates them
All 210 keys are **rows 3+** of the **6 dynamic Part V schedule tables** on Page 2 (`rules/forms/1801-v2018/fields.json` defines all 37 indexed families as `frm1801v2018:sched*_{index}`; `rules/forms/1801-v2018/workflow.json` & `evidence.md` confirm the HTA initializes 2 rows per table and an inline **"Add"** button appends rows `3, 4, ...` dynamically to the DOM):

- **Schedule 1 — Details of Property (REAL PROPERTIES)** (`Part V`, Page 2): `sched1Oct_{n}` (OCT/TCT/CCT No.), `sched1Td_{n}` (Tax Declaration No.), `sched1Loc_{n}` (Location), `sched1Lot_{n}` (Lot/Improvement), `sched1Area_{n}` (Area), `sched1Class_{n}'` (Classification), `sched1Fmv_{n}` (FMV per TD), `sched1Zonal_{n}` (FMV per BIR Zonal), `sched1Exc_{n}` (Exclusive), `sched1Conj_{n}` (Conjugal/Communal).
- **Schedule 1A — Details of Family Home** (`Part V`, Page 2): `sched1AOct_{n}`, `sched1ATd_{n}`, `sched1ALoc_{n}`, `sched1AArea_{n}`, `sched1AClass_{n}'`, `sched1AFmv_{n}`, `sched1AZonal_{n}`, `sched1AExc_{n}`, `sched1AConj_{n}`.
- **Schedule 2 — Personal Properties (SHARES OF STOCK)** (`Part V`, Page 2): `sched2Corp_{n}`, `sched2Class_{n}`, `sched2Stock_{n}`, `sched2Shares_{n}`, `sched2FmvPerShare_{n}`, `sched2Exc_{n}`, `sched2Conj_{n}`.
- **Schedule 2A — Other Personal Properties** (`Part V`, Page 2): `sched2AParticulars_{n}`, `sched2AExc_{n}`, `sched2AConj_{n}`.
- **Schedule 3 — Taxable Transfers** (`Part V`, Page 2): `sched3Particulars_{n}`, `sched3Exc_{n}`, `sched3Conj_{n}`.
- **Schedule 4 — Business Interest** (`Part V`, Page 2): `sched4Name_{n}`, `sched4Address_{n}`, `sched4Rdo_{n}`, `sched4Exc_{n}`, `sched4Conj_{n}`.

### 1.2 Upload, Form Page, and Print at `main` (`db022086`)
- **Upload (`to_bir_xml_payload`)**: **YES — 210/210 exact match** (`1801-09152025`: 6/6; `1801-09162025`: 204/204). Written by `Form1801Draft::to_bir_field_map()` (`crates/bir-core/src/forms/form_1801.rs:731-801`) and spliced into the XML order by `Form1801Draft::official_layout()` (`form_1801.rs:845-864`). Sample confirmation (5 keys per major family, `official.xml == plain.xml`):
  - `sched1Oct`: `sched1Oct_3='T-33333'` (`0915`), `sched1Oct_3='T-00003'`, `sched1Oct_4='T-00004'`, `sched1Oct_5='T-00005'`, `sched1Oct_6='T-00006'` (`0916`)
  - `sched1Td`: `sched1Td_3='TD-004'` (`0915`), `sched1Td_3='TD-103'`, `sched1Td_4='TD-104'`, `sched1Td_5='TD-105'`, `sched1Td_6='TD-106'` (`0916`)
  - `sched1Loc`: `sched1Loc_3='DAVAO CITY'` (`0915`), `sched1Loc_3='LOT 3, CEBU CITY'`, `sched1Loc_4='LOT 4, CEBU CITY'`, `sched1Loc_5='LOT 5, CEBU CITY'`, `sched1Loc_6='LOT 6, CEBU CITY'` (`0916`)
  - `sched1Exc`: `sched1Exc_6='600,000.75'`, `sched1Exc_7='700,000.88'`, `sched1Exc_8='800,001.00'`, `sched1Exc_9='900,001.13'`, `sched1Exc_11='1,100,001.38'` (`0916`)
  - `sched1AOct`: `sched1AOct_3='F-003'`, `sched1AOct_4='F-004'`, `sched1AOct_5='F-005'`, `sched1AOct_6='F-006'`, `sched1AOct_7='F-007'` (`0916`)
  - `sched2Corp`: `sched2Corp_3='CORP 3'`, `sched2Corp_4='CORP 4'`, `sched2Corp_5='CORP 5'`, `sched2Corp_6='CORP 6'`, `sched2Corp_7='CORP 7'` (`0916`)
  - `sched2AParticulars`: `sched2AParticulars_3='PAINTINGS'` (`0915`), `sched2AParticulars_3='ITEM 3'`, `sched2AParticulars_4='ITEM 4'`, `sched2AParticulars_5='ITEM 5'`, `sched2AParticulars_6='ITEM 6'` (`0916`)
  - `sched3Particulars`: `sched3Particulars_3='TRANSFER 3'`, `sched3Particulars_4='TRANSFER 4'`, `sched3Particulars_5='TRANSFER 5'`, `sched3Particulars_6='TRANSFER 6'`, `sched3Particulars_7='TRANSFER 7'` (`0916`)
  - `sched4Name`: `sched4Name_3='SHOP 3'`, `sched4Name_4='SHOP 4'`, `sched4Name_5='SHOP 5'`, `sched4Name_6='SHOP 6'`, `sched4Name_7='SHOP 7'` (`0916`)
- **Form page**: **YES (with static row cap)**. `forms/schema-overrides.json` defines `table:4:sched1` (`rows: [1, 21]`), `table:4:sched1A`, `table:4:sched2`, `table:4:sched2A`, `table:4:sched3`, `table:4:sched4` (each `rows: [1, 7]`), rendered by `nsh/screens/form/controls/Schedule.tsx` with **"Add row"** and **"Delete row"**. Covers both official samples, but a filer cannot add row 22+ of Schedule 1 or row 8+ of Schedules 1A–4.
- **Print at `main` (`db022086`)**: **NOWHERE**. Rendered `1801-09152025` (2 PDF pages) and `1801-09162025` (2 PDF pages; see `1801-09162025-p2.png`). Only rows 1–2 print on Page 2; rows 3–21 (`sched1`) and 3–7 (`sched1A`–`sched4`) appear on no page, while Page 2 `TOTAL` lines print the full sums (e.g., Schedule 1 Exclusive `24,200,028.80` vs `1,400,000.25` visible on paper).

### 1.3 Official sheet & official HTA print behavior
The official BIR paper sheet (`html-frozen/1801-2018/index.html`) draws **2 physical rows** per schedule on Page 2, with `(Attach additional sheet/s if necessary)` printed in every schedule heading. In the official eBIRForms HTA, added rows are appended as `<tr>` elements inside Page 2's HTML tables and spill across extra printed pages when printed.

---

## 2. BIR Form 1707-A (`1707a-2021`, 124 class `(d)` keys)

### 2.1 What the keys are & how the official HTA creates them
All 124 keys are **modal overflow rows** (`_1.._11` for Schedule 1, `_1.._9` for Schedule 2 — **not** spouse columns) for the two Part IV schedules on Page 2 (`rules/forms/1707a-v2021/fields.json` defines all 11 unbounded families as `frm1707Av2021:txtS{1,2}C*_{N>=1}`):
- **Schedule 1 — List of Transactions Resulting to Gain** (`Part IV`, Page 2): `txtS1C1DateOfTransaction_{k}` (Col A), `txtS1C2NameOfCorporateStock_{k}` (Col B), `txtS1C3SellingPrice_{k}` (Col C), `txtS1C4Cost_{k}` (Col D), `txtS1C5CapitalGains_{k}` (Col E), `txtS1C6TaxPaid_{k}` (Col F).
- **Schedule 2 — List of Transactions Resulting to Loss** (`Part IV`, Page 2): `txtS2C1DateOfTransaction_{k}` (Col A), `txtS2C2NameOfCorporateStock_{k}` (Col B), `txtS2C3SellingPrice_{k}` (Col C), `txtS2C4Cost_{k}` (Col D), `txtS2C5CapitalLoss_{k}` (Col E).

In the official HTA, clicking Row 4's `"OTHERS"` modal (`elemModalSched1Compute` / `elemModalSched2Compute`) opens a popup where the filer adds rows `4+` (serialized as `_1, _2, ...`), records `ModalSched1Length` / `ModalSched2Length`, and writes `"OTHERS"` plus the modal subtotals into Page 2 Row 4 (`I4`).

### 2.2 Upload, Form Page, and Print at `main` (`db022086`)
- **Upload (`to_bir_xml_payload`)**: **YES — 124/124 exact match** (`1707A-12312023`: 108/108; `1707A-12312024`: 16/16). Written by `Form1707ADraft::to_bir_field_map()` (`crates/bir-core/src/forms/form_1707a.rs:704-760`). Sample confirmation (5 keys per family, `1707A-12312023`, `official.xml == plain.xml`):
  - `txtS1C1DateOfTransaction`: `_1='02/10/2023'`, `_2='03/10/2023'`, `_3='04/10/2023'`, `_4='05/10/2023'`, `_5='06/10/2023'`
  - `txtS1C2NameOfCorporateStock`: `_1='GAIN CO 4'`, `_2='GAIN CO 5'`, `_3='GAIN CO 6'`, `_4='GAIN CO 7'`, `_5='GAIN CO 8'`
  - `txtS1C3SellingPrice`: `_1='8,002.03'`, `_2='10,002.54'`, `_3='12,003.05'`, `_4='14,003.56'`, `_5='16,004.07'`
  - `txtS1C4Cost`: `_1='4,000.00'`, `_2='5,000.00'`, `_3='6,000.00'`, `_4='7,000.00'`, `_5='8,000.00'`
  - `txtS1C5CapitalGains`: `_1='4,002.03'`, `_2='5,002.54'`, `_3='6,003.05'`, `_4='7,003.56'`, `_5='8,004.07'`
  - `txtS1C6TaxPaid`: `_1='40.00'`, `_2='50.00'`, `_3='60.00'`, `_4='70.00'`, `_5='80.00'`
  - `txtS2C1DateOfTransaction`: `_1='03/20/2023'`, `_2='04/20/2023'`, `_3='05/20/2023'`, `_4='06/20/2023'`, `_5='07/20/2023'`
  - `txtS2C2NameOfCorporateStock`: `_1='LOSS CO 4'`, `_2='LOSS CO 5'`, `_3='LOSS CO 6'`, `_4='LOSS CO 7'`, `_5='LOSS CO 8'`
  - `txtS2C3SellingPrice`: `_1='2,000.00'`, `_2='2,500.00'`, `_4='3,500.00'`, `_5='4,000.00'`, `_6='4,500.00'`
  - `txtS2C4Cost`: `_1='3,001.00'`, `_2='3,751.25'`, `_3='4,501.50'`, `_4='5,251.75'`, `_5='6,002.00'`
  - `txtS2C5CapitalLoss`: `_1='1,001.00'`, `_2='1,251.25'`, `_3='1,501.50'`, `_4='1,751.75'`, `_5='2,002.00'`
- **Form page**: **YES (with static row cap)**. `forms/schema-overrides.json` defines `table:3:sched1more` (`rows: [1, 11]`, `start: 4`, cap 14 total rows) and `table:3:sched2more` (`rows: [1, 9]`, `start: 4`, cap 12 total rows) with **"Add row"**. Covers both official samples, but a filer cannot add row 15+ (Sched 1) or row 13+ (Sched 2).
- **Print at `main` (`db022086`)**: **NOWHERE**. Rendered `1707A-12312023` (2 PDF pages; see `1707A-12312023-p2.png`) and `1707A-12312024` (2 PDF pages). Page 2 prints rows 1–3 and Row 4 `"OTHERS"` with the modal subtotals (`198,050.34` / `99,000.00` / `99,050.34` / `990.00` for Sched 1; `36,000.00` / `54,018.00` / `18,018.00` for Sched 2); all 11 Gain and 9 Loss modal rows print on no page.

### 2.3 Official sheet & official HTA print behavior
The official paper sheet (`html-frozen/1707a-2021/index.html`) draws **4 physical rows** per schedule on Page 2 with `(Attach additional sheet/s if necessary)` on both headers. When > 4 rows exist, Row 4 prints `"OTHERS"` + modal totals on Page 2, and the individual overflow rows require an attached continuation sheet (the HTA only prints Page 2's `"OTHERS"` summary unless the modal/attachment is printed separately).

---

## 3. All Other Forms Where Class `(d)` > 0 (One Line Per Form)

- **`1707-2021` (77 `(d)` rows)**: Schedule 2 (Shares of Stocks, 4 cols) & Schedule 3 (Cost/Expenses, 2 cols) modal rows `4+` (`txtPg2Pt4S{2,3}_{1..12}Col*`); upload 77/77 exact (`form_1707.rs`); form page shows `s2more`/`s3more` (`rows=[1,12]`, cap 15); **`main` prints NOWHERE** (2 pages only; paper has 4 rows A–D, Row D prints modal total + `(Attach additional sheet)`).
- **`2200m-2018` (44 `(d)` rows)**: Schedule 1 added `XM060` rows `11+` (`txtSched1*11..16`, 16 cols); upload 44/44 exact (`form_2200m.rs`); form page shows `table:3:added` (`rows=[11,22]`, cap 12 added); **`main` prints NOWHERE** (2 pages only; paper has 10 fixed rows + `(Attach additional sheet)`).
- **`1702ex-2018` (41 `(d)` rows)**: 24 are static keys already in `1702ex-v2018c/fields.json` (printed via `derived:*` joins, preprinted ATC, or hidden `txtLOB`) + **17 modal rows** (`S1I17`, `S2I4`, `S3I3`, `S3I6`, `S3I8`); upload 41/41 exact (`form_1702ex.rs`); form page shows all 5 tables (`rows=[1,12]` each); **`main` prints the 17 modal rows NOWHERE** (3 pages only; paper has fixed lines + `(Attach additional sheet)`).
- **`2200p-2020` (28 `(d)` rows)**: Schedule 1 added rows `28+` (`txtSched1*28..31`, 14 cols); upload 28/28 exact (`form_2200p.rs`); form page shows `table:4:added` (`rows=[28,41]`, cap 14 added); **`main` prints NOWHERE** (2 pages only; paper has 27 fixed rows + `(Attach additional sheet)`).
- **`1702mx-2018c` (28 `(d)` rows)**: Schedule 6 Item 4 (`(Add more...)` modal rows `txtPg6Sc6_{1..7}Col1..6`, where `Col6` Total is absent from `fields.json`); upload 28/28 exact (`form_1702mx.rs`); form page shows `table:19:sc6i4` (`rows=[1,12]`, 5 input cols); **`main` prints NOWHERE** (4 pages only; paper has 3 fixed lines + Item 4 summary line).
- **`1700-2018` (20 `(d)` rows)**: 12 are static v2018 keys (no `rules/forms/1700-v2018/` dir; `txtPg1I9BirthDate`/`txtPg2ISched1c_1REG` print via `derived:*`, `II011` preprinted, `txtLOB` hidden) + **8 Schedule 1 "Others" employer modal rows** (`txtPg2I4_{1..3}PartVIEmployerName1`, `txtPg2ISched1{c,e}_4_{1..3}*`); upload 20/20 exact (`form_1700.rs`); form page shows 12 modal employer rows (`screen-1700-v2018.json`); **`main` prints the 8 modal rows NOWHERE** (2 pages only; paper has 4 employer rows).

---

## 4. Ranked List of What Must Change (by Impact on Filer's Paper / Form Page)

1. **Print all dynamic/modal overflow rows on paper with repeated headers & totals** *(Highest — affects every filer on all 8 forms; values are filed in XML and summed in totals but completely absent from paper on `main`)*:
   - **Fix**: Land the print continuation / band-flow engine (`try/forms-band-flow`, already fixed & proven at `4321b79b` after PR #355 and PR #360 merge).
   - **Size**: **Already built on `try/forms-band-flow`** (merge/rebase onto `main` after #355 + #360: **M**, ~1 session).
2. **Lift static row caps in `schema-overrides.json` / `Schedule.tsx` so "Add row" is unbounded** *(High for filers with >21 real properties on `1801`, >14 gains / >12 losses on `1707A`, >12 modal rows on `1707`/`1702EX`/`1702MX`/`1700`/`2200M`/`2200P`)*:
   - **Fix**: Allow `Schedule.tsx` / `screen_form_module.rs` to generate row keys dynamically from `templates` + `row_keys` instead of pre-materializing a fixed `rows: [first, last]` array.
   - **Size**: **M** (~0.5–1 session across `screen_form_module.rs` + `Schedule.tsx` + tests).
3. **Add `rules/forms/1700-v2018/` package & `1702mx-v2018c` `txtPg6Sc6_{N>=1}Col6` template** *(Medium — tooling/rules-parity gap; does not break filing today because `screen-1700-v2018.json` and `form_1702mx.rs` already handle them, but blocks any rules-driven checker)*:
   - **Fix**: Add `Col6` template to `rules/forms/1702mx-v2018c/fields.json` and add `rules/forms/1700-v2018/` (or align `1700` rules dir).
   - **Size**: **S** for `1702MX` `Col6` (~15 min); **M** for full `1700-v2018` rules package (~0.5 session).
4. **Update audit classifier (`classify.py`) to match `{index}`/`{N>=1}`/`{n>=...}` templates and resolve `v2018c` dirs** *(Low — audit tooling only; eliminates 545/572 false class `(d)` flags)*:
   - **Size**: **XS** (~15 min).
