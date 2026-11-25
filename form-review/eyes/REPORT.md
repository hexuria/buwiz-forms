# Visual Print Audit — All 43 Forms

- **Branch**: `try/forms-eyes` (no code changes)
- **Base commit**: `origin/main` @ `51609c0b`
- **Taxpayer**: Demo taxpayer (`SANTOS, JUANA MARIA DELA CRUZ-VILLANUEVA`, TIN `000-123-453-00000`, RDO `008`)
- **Method**: Opened each form in the NSH Linux desktop app (`just nsh-run-agent` with scratch DB), populated a realistic return through the form page (reusing official samples), saved draft, opened Print Preview (`fp-print`), saved PDF via headless Chrome, and compared every rendered page side-by-side (`compare-pN.png`: official BIR sheet on the left, our printed PDF on the right).
- **Grand Totals (43 Forms)**: **`6 clean`**, **`12 tweak`**, **`25 broken`**.
  - **Clean (6)**: `0619E`, `1700`, `1602Q`, `1603Q`, `1707A`, `2000OT`
  - **Tweak (12)**: `2551Q`, `1701A`, `2550Q`, `1601C`, `1601EQ`, `0619F`, `1601FQ`, `1604F`, `1706`, `1707`, `2552`, `2200AN`
  - **Broken (25)**: `2550M`, `1701`, `1701Q`, `0605`, `1701MS`, `1702Q`, `1702RT`, `1702EX`, `1702MX`, `1600WP`, `1600PT`, `1600VT`, `1604C`, `1604E`, `1606`, `1800`, `1801`, `2000`, `2553`, `2200A`, `2200C`, `2200M`, `2200P`, `2200S`, `2200T`

---

## Batch 1: First 10 Priority Forms

| # | Form | Version | Pages (Ours / Official) | Verdict | Highest Harm | Summary |
|---|---|---|---|---|---|---|
| 1 | **2551Q** | Jan 2018 (ENCS) | 2 / 2 | **tweak** | Looks wrong on paper | Page 2 omits Table 1 (ATC table); unused Items 16–17 print `0.00`. |
| 2 | **1701Q** | Jan 2018 (ENCS) | 2 / 2 | **broken** | Looks wrong on paper | Page 2 TIN crushed on far left with empty `00000` beside it; Page 2 omits Tax Rate Tables 1 & 2; unused Sched II & Spouse boxes print `0`. |
| 3 | **1701** | Jan 2018 (ENCS) | 4 / 4 | **broken** | Return cannot be completed / Wrong number filed | On `origin/main` (`51609c0b`), 31 Sched 3.A/4/6.A.2 boxes remain locked; Page 3 Sched 6 Item 8 total is vertically clipped/unreadable; Sched 6 prints `0.00` with centavos; Page 4 omits Tax Rate Tables 1 & 2. |
| 4 | **1701A** | Jan 2018 (ENCS) | 2 / 2 | **tweak** | Looks wrong on paper | Page 2 omits Tax Rate Tables 1 & 2; TIN branch `00000` slightly bold/doubled over preprint; otherwise crisp. |
| 5 | **2550Q** | Apr 2024 (ENCS) | 2 / 2 | **tweak** | Looks wrong on paper | Scaled to Letter with bottom whitespace vs Folio/Legal official sheet; unused Part V Schedules 1–4 cells print left-aligned `0.00`. |
| 6 | **2550M** | Feb 2007 (ENCS) | 2 / 2 (frozen HTML ref) | **broken** | Wrong number filed | On `origin/main` (`51609c0b`), Item 13B & 18F/H/J/L/O stay `0.00` (wrong totals in 16B/19/22/26); Item 9 address clipped at right edge; Sched 4 prints `5.00` when untouched; amounts left-aligned. |
| 7 | **1601C** | Jan 2018 (ENCS) | 2 / 2 | **tweak** | Looks wrong on paper | Page 2 omits Guidelines and Instructions (~70% blank page); unused Part II boxes print `0.00`. |
| 8 | **0619E** | Jan 2018 (ENCS) | 1 / 1 | **clean** | Cosmetic | Exact 1-page match; all boxes, totals, and penalties land cleanly (only unused Item 15 prints `0.00`). |
| 9 | **1601EQ** | Jan 2019 (ENCS) | 2 / 2 | **tweak** | Looks wrong on paper | Page 2 omits the entire Schedule of ATCs (prints only 1-inch header over a blank page); unused ATC rows 13–18 print `0.00` in Tax Withheld. |
| 10 | **0605** | Jul 1999 (ENCS) | 1 / 2 | **broken** | Looks wrong on paper | Item 15 address truncated/clipped at right edge (`...METRO M`); Page 2 (ATC/Tax Type tables & Guidelines) missing; Part II amounts left-aligned against item labels `20A`–`20D`. |

**Totals**: `1 clean`, `5 tweak`, `4 broken`.

---

## Findings Ranked by Harm

### 1. Wrong number filed
- **2550M (Page 1, Part II)**: On `origin/main` (`51609c0b`, before #315 merges), entering Sales to Government (`13A = 250,000.01`) leaves `13B` at `0.00` (expected `30,000.00`), and entering Purchases (`18E/G/I/K/N`) leaves `18F/H/J/L/O` at `0.00`, causing wrong totals in `16B` (`0.00`), `19` (`1,600.24`), `22` (`-1,590.24`), and `26` (`-1,790.24`).
- **1701 (Pages 2–3, Schedules 3.A, 4, 6.A.2)**: On `origin/main` (`51609c0b`, before `try/forms-1701-locked` merges), 31 business income/deduction/spouse-NOLCO boxes remain locked at `0.00`, misstating business tax due.

### 2. Return cannot be completed
- **1701 (Pages 2–3)**: Self-employed / mixed-income graduated-rate filers cannot enter Sales (`8A/B`), Returns (`9A/B`), Cost of Sales (`11A/B`), or 18 Schedule 4 deduction lines on `origin/main` (`51609c0b`).

### 3. Looks wrong on paper
- **1701Q (Page 2 TIN header)**: `00012345300000` prints crushed/overlapping on the far left of the TIN box while the pre-printed `0 0 0 0 0` branch-code digits sit empty to the right (`1701Q/compare-p2.png`).
- **1701 (Page 3, Schedule 6.A.1 Item 8)**: `Total NOLCO - Taxpayer/Filer` column D (`0.00`) is vertically clipped into an unreadable smudge at the top border of row 8 (`1701/compare-p3.png`).
- **2550M (Page 1, Item 9) & 0605 (Page 1, Item 15)**: Long registered address prints on a single shrunk line and truncates at the right edge (`...METRO MAN` on 2550M; `...METRO M` on 0605) instead of wrapping (`2550M/compare-p1.png`, `0605/compare-p1.png`).
- **2550M (Page 2, Schedule 4)**: Untouched `Less: Standard Input Tax to sale to government` prints `5.00` in the peso box (`2550M/compare-p2.png`).
- **1701 (Pages 3–4, Schedule 6.A.1 & 6.A.2)**: Empty NOLCO cells print left-aligned `0.00` with centavos on a whole-peso form (`DO NOT enter Centavos`).
- **2550M (Pages 1–2) & 0605 (Page 1, Part II)**: Peso amounts are left-aligned instead of right-aligned; on 0605, `20A`–`20D` values sit flush against the item labels (`0605/compare-p1.png`).
- **Missing reference tables / instructions / Page 2**:
  - **1601EQ Page 2**: Omits the entire Schedule of ATCs, producing a nearly blank Page 2 with only the top header bar (`1601EQ/compare-p2.png`).
  - **2551Q Page 2**: Omits `Table 1 – Alphanumeric Tax Code (ATC)` (~70% blank bottom) (`2551Q/compare-p2.png`).
  - **1601C Page 2**: Omits `Guidelines and Instructions for BIR Form No. 1601-C` (~70% blank bottom) (`1601C/compare-p2.png`).
  - **1701Q Page 2, 1701 Page 4, 1701A Page 2**: Omit `TABLE 1` and `TABLE 2 – Tax Rates` at the bottom of the last page.
  - **0605 Page 2**: Missing entirely (1 page printed vs 2-page official sheet containing ATC/Tax Type tables and Guidelines) (`0605/compare-p2.png`).
- **2550Q (Pages 1–2)**: Printed scaled into Letter with bottom whitespace vs Folio/Legal official sheet; unused Part V Schedules 1–4 cells print left-aligned `0.00` (`2550Q/compare-p1.png`, `2550Q/compare-p2.png`).
- **Unused amount boxes printing `0` / `0.00`**: Observed across `2551Q`, `1701Q` (including unused Schedule II & Spouse column), `1701`, `2550Q`, `2550M`, `1601C`, `0619E` (Item 15), and `1601EQ` (empty ATC rows 13–18). (`1701A` cleanly leaves unused boxes blank.)

### 4. Cosmetic
- **Comb-tick drift on long names/addresses**: `SANTOS, JUANA MARIA DELA CRUZ-VILLANUEVA` and the 2-line registered address drift slightly across comb tick marks on `2551Q`, `1701Q`, `1701`, `2550Q`, `1601C`, `0619E`, `1601EQ` (all characters legible, no clipping).
- **1701A TIN branch `00000`**: Slightly bold/doubled over the pre-printed `00000` in Item 4 and Item 66.
- **Negative-sign spacing**: Minus sign sits tight against the leading digit in comb cells on overpayments (e.g., `1601EQ` Items 26/31 `-3,000.51`).

---

## Per-Form Evidence

- **2551Q**: `2551Q/compare-p1.png`, `2551Q/compare-p2.png` (`ours-p1.png`, `ours-p2.png`)
- **1701Q**: `1701Q/compare-p1.png`, `1701Q/compare-p2.png` (`ours-p1.png`, `ours-p2.png`)
- **1701**: `1701/compare-p1.png` – `1701/compare-p4.png` (`ours-p1.png` – `ours-p4.png`)
- **1701A**: `1701A/compare-p1.png`, `1701A/compare-p2.png` (`ours-p1.png`, `ours-p2.png`)
- **2550Q**: `2550Q/compare-p1.png`, `2550Q/compare-p2.png` (`ours-p1.png`, `ours-p2.png`)
- **2550M**: `2550M/compare-p1.png`, `2550M/compare-p2.png` (`ours-p1.png`, `ours-p2.png`)
- **1601C**: `1601C/compare-p1.png`, `1601C/compare-p2.png` (`ours-p1.png`, `ours-p2.png`)
- **0619E**: `0619E/compare-p1.png` (`ours-p1.png`)
- **1601EQ**: `1601EQ/compare-p1.png`, `1601EQ/compare-p2.png` (`ours-p1.png`, `ours-p2.png`)
- **0605**: `0605/compare-p1.png`, `0605/compare-p2.png` (`ours-p1.png`)

---

## Remaining 33 Forms (Batches 2–4)

| # | Form | Pages (Ours / Official) | Verdict | Highest Harm | Summary |
|---|---|---|---|---|---|
| 11 | **1700** | 2 / 2 (frozen HTML) | **clean** | Cosmetic | Exact 2-page match; whole-peso comb values (`(100)`) crisp, unused boxes blank; TIN branch `00000` slightly bold over preprint. |
| 12 | **1701MS** | 2 / 2 (frozen HTML) | **broken** | Wrong number filed | On `origin/main` (`51609c0b`, before #323), Items 2A/2B non-taxable compensation locked → overstated tax due; print layout itself is crisp (unused Part II boxes print `0`). |
| 13 | **1702Q** | 2 / 3 (official PDF) | **broken** | Wrong number filed / Looks wrong on paper | Whole-peso Part II prints `.00` shifted into peso combs (**100× overstated**: `875,000` prints as `87,500,000`); Item 2 `2025` collides with pre-printed `/20`; Page 3 (ATC table & Guidelines) missing. |
| 14 | **1702RT** | 4 / 4 (official PDF) | **broken** | Looks wrong on paper | Page 1 Item 2 `2025` collides with pre-printed `/20`; TIN branch `00000` doubled over preprint; otherwise whole-peso values align cleanly. |
| 15 | **1702EX** | 3 / 3 (frozen HTML) | **broken** | Looks wrong on paper | Page 1 Item 2 `2025` collides with pre-printed `/20`; Item 9 address truncated at `...SAN LOREN`; Item 6 `00000` & Item 18 `000` doubled over preprints. |
| 16 | **1702MX** | 4 / 4 (frozen HTML) | **broken** | Looks wrong on paper | Page 1 Item 2 `2025` collides with pre-printed `/20`; Part II Items 14–16 print blank while Item 21 carries `-79,000`; TIN `00000` doubled. |
| 17 | **0619F** | 1 / 1 (official PDF) | **tweak** | Cosmetic | Exact 1-page match; Item 5 Tax Type Code (`WF`) is tiny; unused Part II boxes print `0.00`. |
| 18 | **1601FQ** | 2 / 2 (frozen HTML) | **tweak** | Cosmetic | Exact 2-page match; totals and comb values crisp; unused Part II rows 14–19 & penalties print `0.00`. |
| 19 | **1602Q** | 3 / 3 (frozen HTML) | **clean** | Cosmetic | Exact 3-page match; all Part II totals (`282,175.38`) and schedules align cleanly (unused boxes print `0.00`). |
| 20 | **1603Q** | 2 / 2 (frozen HTML) | **clean** | Cosmetic | Exact 2-page match; all Part II totals (`112,788.34`) and schedules align cleanly (unused boxes print `0.00`). |
| 21 | **1600WP** | 1 / 1 (frozen HTML) | **broken** | Wrong number filed | Schedule 1.1 lists `4,000.00 + 2,010.01 = 6,010.01`, yet Schedule `TOTAL` & Part II rows are blank and Item 12 = `0.00` (Item 16 = `-1,000.00`); Item 9 address microscopic & clipped (`...METRO`); Part II amounts left-aligned. |
| 22 | **1600PT** | 2 / 2 (frozen HTML) | **broken** | Wrong number filed | Page 2 Schedule 1 computes `6,005.00`, yet Page 1 Item 3 = `No`, Part II rows 13–17 blank, Item 18 = `0.00` (Item 27 = `-250.50`); Item 1 `06 2025` collides with pre-printed `2 0`; unused Sched 1 rows print `0.00`. |
| 23 | **1600VT** | 2 / 2 (frozen HTML) | **broken** | Wrong number filed | Same twin defects as 1600PT: Page 2 Schedule 1 = `6,005.00` vs Page 1 Part II = `0.00` (Item 27 = `-250.50`); Item 1 `06 2025` collides with pre-printed `2 0`. |
| 24 | **1604C** | 1 / 1 (frozen HTML) | **broken** | Looks wrong on paper | Page 1 Item 1 `For the Year (20YY)` prints `25` over pre-printed `2 0` (garbled `225 0`); Part II amounts left-aligned with unused months printing `0.00`. |
| 25 | **1604E** | 2 / 2 (frozen HTML) | **broken** | Looks wrong on paper | Page 1 Item 1 prints `25` over pre-printed `2 0` (garbled `225 0`); Item 7 address truncated at `...845 ARNAIZ AV`; Part II amounts left-aligned (`0.00` on unused rows). |
| 26 | **1604F** | 2 / 2 (frozen HTML) | **tweak** | Looks wrong on paper | Page 1 Item 7 address truncated at `...845 ARNAIZ AV` (Item 1 `2025` is clean); Part II amounts left-aligned (`0.00` on unused rows). |
| 27 | **1606** | 2 / 2 (frozen HTML) | **broken** | Wrong number filed | Page 2 Schedule 1 FMVs (`4,200,000.00 + 1,500,000.00 = 5,700,000.00`) print `TOTAL = 0.00` and Sched 3 Item 1B = `0.00`, understating Taxable Base (`5,000,000.01` vs `5,700,000.00`); Buyer address microscopic & clipped (`...METRO`); empty Seller TIN prints `--00000`. |
| 28 | **1706** | 2 / 2 (frozen HTML) | **tweak** | Looks wrong on paper | Totals (`1,800,001.00`, tax `108,000.06`) clean; Seller address microscopic & clipped (`...METR`); empty Buyer TIN prints `--00000` (`00000` on Page 2); unused boxes print `0.00`. |
| 29 | **1707** | 2 / 2 (frozen HTML) | **tweak** | Looks wrong on paper | Totals (`573,222.22`) clean; Seller/Buyer names & addresses microscopic; TINs lack dashes; unused Schedule 1 (Installment) prints `0.00` in every box. |
| 30 | **1707A** | 2 / 2 (frozen HTML) | **clean** | Cosmetic | Exact 2-page match; all Part II (`176,570.06`) & Schedule 1/2 gain/loss rows crisp (TIN `00000` slightly bold; Sched 1/2 amounts left-aligned). |
| 31 | **1800** | 2 / 2 (frozen HTML) | **broken** | Looks wrong on paper | Page 1 Item 14 (`Total Net Gifts Subject to Tax`) is **blank** while Page 2 Item 38 = `2,934,567.90` and Page 1 Item 16 Tax Due = `176,074.07`; TIN `00000` & Item 37 `250,000.00` bold over preprints; unused rows print `0.00`. |
| 32 | **1801** | 2 / 2 (frozen HTML) | **broken** | Looks wrong on paper | Page 1 Item 5 **TIN is blank** (only pre-printed `00000` shows, though Page 2 header prints `00012345300000`); Page 1 Item 16 (`Net Taxable Estate`) is blank when Page 2 Item 40 is negative (`-3,489,999.38`). |
| 33 | **2000** | 2 / 2 (frozen HTML) | **broken** | Looks wrong on paper | Page 2 header TIN `00012345300000` crushed on far left colliding into `SANTOS, JUANA...`; Page 2 Sched 1 rows show `0.00` (blank ATC/rate) yet `TOTAL TAX DUE` = `20.00`. |
| 34 | **2000OT** | 2 / 2 (frozen HTML) | **clean** | Cosmetic | Exact 2-page match; Part II (`51,500.00`) & Schedule 1.A/2 (`3,500,000.00`) crisp (Item 7 name not comb-spaced; unused Sched 1.B prints `0.00`). |
| 35 | **2552** | 2 / 2 (frozen HTML) | **tweak** | Return cannot be completed | Print layout clean on both pages (unused cells print `0.00`, Sched 1 empty rows print `6/10 OF 1%`); on `origin/main` (`51609c0b`, before #323), Item 17 is locked for amended returns. |
| 36 | **2553** | 1 / 1 (frozen HTML) | **broken** | Looks wrong on paper | Item 11 address microscopic & clipped (`...MAKATI CITY`); Part II amounts (`1,000.00`, `-1,000.00`, `0.00`) left-aligned inside decimal-dotted boxes ignoring the pre-printed `.` separator. |
| 37 | **2200A** | 3 / 3 (frozen HTML) | **broken** | Wrong number filed / Looks wrong on paper | On `origin/main` (before #323), Others row 1 tax due stays `0.00` (`87,440.56` vs `87,451.56`); Pages 2–3 header TIN crushed into Name (`00012345300000SANTOS...`); Items 10–11 print raw PSGC codes (`130000000`...) bleeding across Region→Province. |
| 38 | **2200AN** | 2 / 2 (frozen HTML) | **tweak** | Looks wrong on paper | Totals (`1,592,000.11`) & Region/Province/City names clean; Page 2 Sched 1B empty Others row 2 prints doubled `XGXG`; unused rows print `0.00`. |
| 39 | **2200C** | 2 / 2 (frozen HTML) | **broken** | Looks wrong on paper | Page 2 header TIN crushed into Name (`00012345300000SANTOS...`); Item 11 prints raw PSGC codes (`130000000`...) bleeding across Region→Province; all 100 empty Sched 1 cells print `0.00`. |
| 40 | **2200M** | 2 / 2 (frozen HTML) | **broken** | Wrong number filed / Looks wrong on paper | Page 2 Sched 1 continuation Col L rows sum to `102,222.68` (`XM060` Others 1–2 print `0.00` despite `5.01`/`120.00` in Col F), yet `TOTAL EXCISE TAX DUE` = `107,664.37` (`5,441.69` discrepancy on paper); `XM060` top descriptions microscopic. |
| 41 | **2200P** | 2 / 2 (frozen HTML) | **broken** | Looks wrong on paper | Page 2 header TIN crushed into Name (`00012345300000SANTOS...`); Items 10–11 print raw PSGC codes (`130000000`...) bleeding across Region→Province; Sched 1 text tiny; unused cells print `0.00`. |
| 42 | **2200S** | 2 / 2 (frozen HTML) | **broken** | Looks wrong on paper | Page 2 Schedule 1 **drops every single line item** (`XB010`–`XB100` & Others all blank) while printing `GRAND TOTAL EXCISE TAX DUE = 135,277.02`; Item 13 Region truncates `REGION IV-A` to `REG`. |
| 43 | **2200T** | 3 / 3 (frozen HTML) | **broken** | Looks wrong on paper | Visible Schedule 1 rows sum to `60,163.47` (`60,030.01 + 123.46 + 10.00`) vs printed `TOTAL TAX DUE = 60,413.47` (`250.00` missing from `2024 Onwards` row); Page 3 Others `XT999` overlaps `XT` prefix; unused rows print `0.00`. |

---

## Systemic Patterns Across All 43 Forms (Ranked by Harm)

1. **Wrong number filed (on `origin/main` `51609c0b`)**
   - **Whole-peso `.00` 100× shift on paper**: `1702Q` Part II (every amount prints `.00` inside whole-peso combs).
   - **Schedule → Part II / Total disconnects on paper**: `1600PT`, `1600VT`, `1600WP` (Schedule tax withheld never lands in Part II), `1606` (Schedule 1 FMV `5.7M` prints `TOTAL = 0.00`, understating Taxable Base by `700k`), `2000` (Sched 1 rows `0.00` vs Total `20.00`), `2200M` (Col L sum `102,222.68` vs Total `107,664.37`), `2200S` (all Sched 1 rows blank vs Grand Total `135,277.02`), `2200T` (visible rows sum `60,163.47` vs Total `60,413.47`).
   - **Known `origin/main` compute / locked-input bugs** (fixed on PRs #315, #323, `try/forms-1701-locked`): `2550M`, `1701`, `1701MS`, `2200A`, `2552`.
2. **Looks wrong on paper**
   - **4-digit year / `25` colliding with pre-printed `20` or `/20`** (8 forms): `1702Q`, `1702RT`, `1702EX`, `1702MX`, `1600PT`, `1600VT`, `1604C`, `1604E`.
   - **Page 2+ header TIN crushed on far left colliding into Name** (5 forms): `1701Q`, `2000`, `2200A`, `2200C`, `2200P`.
   - **Key Page 1 summary / ID boxes left blank** (3 forms): `1800` (Item 14 Net Gifts), `1801` (Item 5 TIN & Item 16 Net Taxable Estate), `1702MX` (Part II Items 14–16).
   - **Raw PSGC numeric codes bleeding across Region/Province/City** (3 forms): `2200A`, `2200C`, `2200P` (and `2200S` truncates `REGION IV-A` to `REG`).
   - **Registered address clipped or shrunk to microscopic font** (9 forms): `2550M`, `0605`, `1702EX`, `1604E`, `1604F`, `1606`, `1706`, `1600WP`, `2553`.
   - **Missing official pages / omitted reference tables** (8 forms): `0605` (1/2), `1702Q` (2/3), `1601EQ`, `2551Q`, `1601C`, `1701`, `1701A`, `1701Q`.
   - **Left-aligned amounts in decimal-dotted or right-aligned boxes**: `2550M`, `0605`, `1600WP`, `2553`, `1604C`, `1604E`, `1604F`.
3. **Cosmetic**
   - Unused amount boxes printing `0` / `0.00` (widespread; cleanly blank on `1701A`, `1700`), slightly bold `00000` over pre-printed branch codes (`1701A`, `1700`, `1707A`, `1800`, `1702*`), and minor comb-tick drift on long names.

*Evidence for all 43 forms lives under `form-review/eyes/<CODE>/compare-pN.png` and `ours-pN.png`.*
