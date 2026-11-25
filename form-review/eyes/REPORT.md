# Visual Print Audit — Batch 1 (First 10 Priority Forms)

- **Branch**: `try/forms-eyes` (no code changes)
- **Base commit**: `origin/main` @ `51609c0b`
- **Taxpayer**: Demo taxpayer (`SANTOS, JUANA MARIA DELA CRUZ-VILLANUEVA`, TIN `000-123-453-00000`, RDO `008`)
- **Method**: Opened each form in the NSH Linux desktop app (`just nsh-run-agent` with scratch DB), populated a realistic return through the form page, saved draft, opened Print Preview (`fp-print`), saved PDF via headless Chrome, and compared every rendered page side-by-side (`compare-pN.png`: official BIR sheet on the left, our printed PDF on the right).

---

## Summary Table (Batch 1: 10 Forms)

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
