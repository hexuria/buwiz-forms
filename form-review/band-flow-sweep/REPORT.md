# Band-Flow Engine Sweep Report (`try/forms-band-flow` @ `446da0e5`)

Tested every growable form at **capacity** (`cap`), **capacity + 3** (`cap_p3`), and **capacity + 40** (`cap_p40`) with our `formgen-page-layout` v1 metadata merged into a scratch copy of `origin/try/forms-band-flow` (`446da0e5`).

## Summary of Findings

### 1. Generator / Layout-Data Bugs Found and Fixed (`buwiz-forms-html` `forms-layout-fixes` @ `a2779b4`)
1. **SVG index off-by-1 (`SVG_TOKEN_RE`):** `emit_page_layout.py` counted `<image>` and `<text>` inside `<svg>`, whereas `band_flow.rs` only indexes `<rect|path|line|polygon|polyline|circle|ellipse>`. Fixed in `emit_page_layout.py` — resolved the `1707` conservation failures (`page_header svg v143 expected 1, got 2`).
2. **Page 1 `page_header` too tall + split `table_head` (`1600WP`):** Page 1 `page_header` swallowed Items 1–4 (`For the Period / Amended Return / ...`), and `1600WP` Schedule 1.1's two-row column header was split in half so `table_head` overlapped `From/To/Yes/No` on Page 2. Fixed in `emit_page_layout.py` (stop p1 `page_header` at first field slice; merge adjacent multi-row non-field header sub-bands into one `table_head`).
3. **Missing `table_head` when leading rows precede `row_template` (`1800`, `1600PT`, `1600VT`):** Fixed in `emit_page_layout.py` to walk past up to 4 leading fixed rows when tagging `table_head` (plus tightened cell-straddle tolerance from `2.0pt` to `1.5pt` for `1707A` p1 checkboxes; 90/90 pages pass strict self-check).

### 2. Engine Defects Found in `crates/bir-print/src/band_flow.rs`
1. **CRITICAL — `<div class="band">` wrapper wipes all existing & extra rows on 19 of 24 active band-flow forms** (`2000OT`, `1800`, `2552`, `2551Q`, `1601C`, `2000`, `2200C`, `2550M`, `1600PT`, `1600VT`, `1601FQ`, `1601EQ`, `1602Q`, `1700`, `1702EX`, `1702MX`, `1702RT`, `1706`, `1707A`): `parse_flow_page_dom()` only parses `<div class="t">` inside `layer-text` and direct `<div class="c">` children of `layer-cells`, skipping `<div class="band" id="band-content-...">`. Only the 5 newer flat-DOM forms (`1707`, `1801`, `1600WP`, `2550Q`, `1701A`) preserve their filled values.
2. **Borders lost above/below growth (known fault + shrunk elastic box):** Outer left/right vertical rails (`spanning_svg_ids`) are lost on blocks above and below growth across `1801`, `1600WP`, `2550Q`, `1701A`, `1707`, `1601EQ`, `1602Q`, `1702RT`, `2552`, `1800`, `2000OT`; on `1600WP` `cap_p3`, the shrunk `Machine Validation` elastic box also loses its bottom horizontal border.
3. **Band overlap & paper overflow (`2550M`):** On `2550M` `cap_p3`, `C) Total Input Tax Deferred` overlaps `Schedule 4`, and Schedule 7 vertical dividers extend to `1039.0pt` (past `content_bottom_y=990pt` and `1008pt` paper height).
4. **Extra-row amount alignment & row-number collisions (`1801`, `1600WP`, `2550Q`, `1701A`):** Cloned extra-row amounts print left-aligned instead of right-aligned (`1801`, `1600WP`, `2550Q`); on `1701A`, extra rows reuse item numbers `43` and `52` directly above existing Items `43` and `52`.
5. **Gapped `table_head` not repeated on break (`1800`):** When a schedule has leading fixed rows between `table_head` and `row_template`, `pack_page` does not repeat `table_head` on `Page 2 (continued)` even when `table_group` matches.

---

## Per-Form Results (in Requested Order)

### 1801 (`1801-2018`, `formgen-bands`: 7)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | PASS | 0 | `1801/cap-p-1.png`, `1801/cap-p-2.png` |
| Capacity + 3 | 3 | Yes | PASS | 0 | `1801/cap_p3-p-1.png`, `1801/cap_p3-p-2.png`, `1801/cap_p3-p-3.png` |
| Capacity + 40 | 6 | Yes | PASS | 0 | `1801/cap_p40-p-1.png`, `1801/cap_p40-p-2.png`, `1801/cap_p40-p-3.png` (+3 more) |

- **Verdict:** All 7 schedules grow cleanly in place and break to Page 2 (continued); 0 missing values, conservation passes.
- **Defects:** 1) Known engine fault: Part IV (above growth) and Schedule 5 (below growth) lose outer left/right borders and column dividers. 2) Engine fault: extra-row amount columns print left-aligned while original rows print right-aligned. 3) Page 6 of cap+40 holds only Schedule 5 (not a growable table), so no table head needed.

### 1600WP (`1600wp-2010`, `formgen-bands`: 1)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 1 | No (static) | PASS | 0 | `1600WP/cap-p-1.png` |
| Capacity + 3 | 1 | Yes | PASS | 0 | `1600WP/cap_p3-p-1.png` |
| Capacity + 40 | 2 | Yes | PASS | 0 | `1600WP/cap_p40-p-1.png`, `1600WP/cap_p40-p-2.png` |

- **Verdict:** Grows in place on Page 1 for cap+3 (shrinking the Machine Validation elastic box cleanly) and breaks to Page 2 for cap+40; 0 missing values, conservation passes.
- **Defects:** 1) Fixed layout-data bug (`a2779b4`): p1 `page_header` previously swallowed Items 1–4 and `table_head` was cut in half, overlapping `From/To/Yes/No` on Page 2; now Page 2 repeats only the title banner + full two-row table head cleanly. 2) Engine fault: bottom horizontal border of the shrunk Machine Validation elastic box is missing on cap+3, and outer side borders are missing on Part III/Machine Validation below the table. 3) Engine fault: extra-row amounts print left-aligned instead of right-aligned.

### 2550Q (`2550q-2024`, `formgen-bands`: 3)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | PASS | 0 | `2550Q/cap-p-1.png`, `2550Q/cap-p-2.png` |
| Capacity + 3 | 3 | Yes | PASS | 0 | `2550Q/cap_p3-p-1.png`, `2550Q/cap_p3-p-2.png`, `2550Q/cap_p3-p-3.png` |
| Capacity + 40 | 4 | Yes | PASS | 0 | `2550Q/cap_p40-p-1.png`, `2550Q/cap_p40-p-2.png`, `2550Q/cap_p40-p-3.png` (+1 more) |

- **Verdict:** Schedules 1, 3, and 4 grow cleanly in place and break to Page 2 (continued) with repeated column headers; 0 missing values, conservation passes.
- **Defects:** 1) Known engine fault: Part IV (above growth) and Schedule 2 (between Schedule 1 and Schedule 3) lose outer left/right borders. 2) Engine fault: extra-row amounts print left-aligned instead of right-aligned. 3) Schedule 1 grey column-header band above `(A)..(I)` renders blank grey; Schedule 4 continuation rows 4–5 leave `(A) Period Covered` blank.

### 1701A (`1701a-2018`, `formgen-bands`: 2)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | PASS | 0 | `1701A/cap-p-1.png`, `1701A/cap-p-2.png` |
| Capacity + 3 | 3 | Yes | PASS | 0 | `1701A/cap_p3-p-1.png`, `1701A/cap_p3-p-2.png`, `1701A/cap_p3-p-3.png` |
| Capacity + 40 | 4 | Yes | PASS | 0 | `1701A/cap_p40-p-1.png`, `1701A/cap_p40-p-2.png`, `1701A/cap_p40-p-3.png` (+1 more) |

- **Verdict:** Part IV.A and IV.B Other Non-Operating Income grow in place with full comb boxes (`structural_rects`) and break Part V cleanly to Page 2 (continued); 0 missing values, conservation passes.
- **Defects:** 1) Known engine fault: outer left/right borders lost on Part IV.A, Part IV.C, and Part V (both pages). 2) Engine fault: extra rows in Part IV.A are numbered `43, 44, 45` (colliding with existing Item 43 right below) and in Part IV.B `52, 53, 54` (colliding with Item 52 Total).

### 2000OT (`2000-ot-2018`, `formgen-bands`: 2)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p2: assertion `left == right` failed: 2000-ot-2018 (cap): non-repeating tex) | 0 | `2000OT/cap-p-1.png`, `2000OT/cap-p-2.png` |
| Capacity + 3 | 3 | No (static) | FAIL (p2: assertion `left == right` failed: 2000-ot-2018 (cap_p3): non-repeating ) | 8+ (e.g. `Schedule 1.A r1 [OCT/TCT/CCT No.]=S0R1C0`) | `2000OT/cap_p3-p-1.png`, `2000OT/cap_p3-p-2.png`, `2000OT/cap_p3-p-3.png` |
| Capacity + 40 | 7 | Yes | FAIL (p2: assertion `left == right` failed: 2000-ot-2018 (cap_p40): non-repeating) | 8+ (e.g. `Schedule 1.A r1 [OCT/TCT/CCT No.]=S0R1C0`) | `2000OT/cap_p40-p-1.png`, `2000OT/cap_p40-p-2.png`, `2000OT/cap_p40-p-3.png` (+4 more) |

- **Verdict:** CRITICAL ENGINE FAULT: `<div class="band">` wrapper inside `layer-cells` causes `parse_flow_page_dom` to drop all existing cells and `.t` row numbers (`1..4`), leaving Schedule 1.A completely blank except row number `5`.
- **Defects:** 1) Engine fault (`<div class="band">`): all filled rows 1..cap and extra rows are wiped blank; conservation fails on `p2t19` even at cap. 2) Known engine fault: outer side borders missing on Schedule 2 below growth.

### 1800 (`1800-2018`, `formgen-bands`: 4)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p2: assertion `left == right` failed: 1800-2018 (cap): non-repeating text p) | 0 | `1800/cap-p-1.png`, `1800/cap-p-2.png` |
| Capacity + 3 | 3 | Yes | FAIL (p2: assertion `left == right` failed: 1800-2018 (cap_p3): non-repeating tex) | 8+ (e.g. `Schedule A r1 [Particulars]=S0R1C0`) | `1800/cap_p3-p-1.png`, `1800/cap_p3-p-2.png`, `1800/cap_p3-p-3.png` |
| Capacity + 40 | 4 | Yes | FAIL (p2: assertion `left == right` failed: 1800-2018 (cap_p40): non-repeating te) | 8+ (e.g. `Schedule A r1 [Particulars]=S0R1C0`) | `1800/cap_p40-p-1.png`, `1800/cap_p40-p-2.png`, `1800/cap_p40-p-3.png` (+1 more) |

- **Verdict:** CRITICAL ENGINE FAULT: `<div class="band">` wrapper wipes all filled rows in Deductions, Schedule A, and Schedule B; plus Schedule B Continuation breaks to Page 2 (continued) without repeating its table head.
- **Defects:** 1) Engine fault (`<div class="band">`): all filled rows wiped blank; conservation fails on `p2t23`. 2) Fixed layout-data bug (`a2779b4`): generator now tags `table_head` for `p2g1`/`p2g2`/`p2g3` even when rows 1–3 sit above `row_template`, but engine `pack_page` still only repeats a `table_head` immediately adjacent (`i-1`) to `row_template`, so Page 2 (continued) still has no repeated table head. 3) Known engine fault: outer borders missing above/below growth.

### 2552 (`2552-2018`, `formgen-bands`: 7)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p2: assertion `left == right` failed: 2552-2018 (cap): non-repeating text p) | 0 | `2552/cap-p-1.png`, `2552/cap-p-2.png` |
| Capacity + 3 | 3 | Yes | FAIL (p2: assertion `left == right` failed: 2552-2018 (cap_p3): non-repeating tex) | 8+ (e.g. `Part VI r1 [Seller]=S3R1C1`) | `2552/cap_p3-p-1.png`, `2552/cap_p3-p-2.png`, `2552/cap_p3-p-3.png` |
| Capacity + 40 | 6 | Yes | FAIL (p2: assertion `left == right` failed: 2552-2018 (cap_p40): non-repeating te) | 8+ (e.g. `Part VI r1 [Seller]=S3R1C1`) | `2552/cap_p40-p-1.png`, `2552/cap_p40-p-2.png`, `2552/cap_p40-p-3.png` (+3 more) |

- **Verdict:** CRITICAL ENGINE FAULT: `<div class="band">` wrapper wipes all filled rows in Schedules 1–3 (only row `5` label survives outside the wrapper).
- **Defects:** 1) Engine fault (`<div class="band">`): all filled cells and row numbers 1..4 wiped blank; conservation fails on `p2t24`. 2) Known engine fault: Part IV (above growth) loses outer side borders.

### 2551Q (`2551q-2018`, `formgen-bands`: 1)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p2: assertion `left == right` failed: 2551q-2018 (cap): non-repeating text ) | 0 | `2551Q/cap-p-1.png`, `2551Q/cap-p-2.png` |
| Capacity + 3 | 2 | No (static) | FAIL (p2: assertion `left == right` failed: 2551q-2018 (cap_p3): non-repeating te) | 8+ (e.g. `Schedule 1 r1 [Taxable Amount]=10017.00`) | `2551Q/cap_p3-p-1.png`, `2551Q/cap_p3-p-2.png` |
| Capacity + 40 | 3 | Yes | FAIL (p2: assertion `left == right` failed: 2551q-2018 (cap_p40): non-repeating t) | 8+ (e.g. `Schedule 1 r1 [Taxable Amount]=10017.00`) | `2551Q/cap_p40-p-1.png`, `2551Q/cap_p40-p-2.png`, `2551Q/cap_p40-p-3.png` |

- **Verdict:** CRITICAL ENGINE FAULT: `<div class="band">` wrapper wipes all 6 original + 3/40 extra rows blank; comb geometry and continuation page header repeat cleanly.
- **Defects:** 1) Engine fault (`<div class="band">`): all filled rows wiped blank (`band_flow_rows: false`); conservation fails on `p2t20`. 2) Engine fault: extra rows' left row-number column border is missing/shifted; top border of first row on Page 2 (continued) missing.

### 1601C (`1601c-2018`, `formgen-bands`: 2)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p2: assertion `left == right` failed: 1601c-2018 (cap): non-repeating text ) | 0 | `1601C/cap-p-1.png`, `1601C/cap-p-2.png` |
| Capacity + 3 | 2 | No (static) | FAIL (p2: assertion `left == right` failed: 1601c-2018 (cap_p3): non-repeating te) | 8+ (e.g. `Schedule I r1 [3 Drawee Bank/Bank Code/Agency]=S0R1C0`) | `1601C/cap_p3-p-1.png`, `1601C/cap_p3-p-2.png` |
| Capacity + 40 | 3 | Yes | FAIL (p2: assertion `left == right` failed: 1601c-2018 (cap_p40): non-repeating t) | 8+ (e.g. `Schedule I r1 [3 Drawee Bank/Bank Code/Agency]=S0R1C0`) | `1601C/cap_p40-p-1.png`, `1601C/cap_p40-p-2.png`, `1601C/cap_p40-p-3.png` |

- **Verdict:** CRITICAL ENGINE FAULT: `<div class="band">` wrapper wipes all Schedule I upper & lower half rows blank.
- **Defects:** 1) Engine fault (`<div class="band">`): all filled rows wiped blank; conservation fails on `p2t28`. 2) Engine fault: extra rows' outer left border missing on Schedule I upper half.

### 2000 (`2000-dst-2018`, `formgen-bands`: 4)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p2: assertion `left == right` failed: 2000-dst-2018 (cap): non-repeating te) | 0 | `2000/cap-p-1.png`, `2000/cap-p-2.png` |
| Capacity + 3 | 3 | No (static) | FAIL (p2: assertion `left == right` failed: 2000-dst-2018 (cap_p3): non-repeating) | 8+ (e.g. `Schedule 2 r1 [Revenue Official Receipt No.]=S0R1C0`) | `2000/cap_p3-p-1.png`, `2000/cap_p3-p-2.png`, `2000/cap_p3-p-3.png` |
| Capacity + 40 | 8 | Yes | FAIL (p2: assertion `left == right` failed: 2000-dst-2018 (cap_p40): non-repeatin) | 8+ (e.g. `Schedule 2 r1 [Revenue Official Receipt No.]=S0R1C0`) | `2000/cap_p40-p-1.png`, `2000/cap_p40-p-2.png`, `2000/cap_p40-p-3.png` (+5 more) |

- **Verdict:** CRITICAL ENGINE FAULT: `<div class="band">` wrapper wipes all Schedules 1–4 rows blank (comb geometry grows cleanly in place).
- **Defects:** 1) Engine fault (`<div class="band">`): all filled rows wiped blank across Schedules 1–4; conservation fails on `p2t15`.

### 2200M (`2200m-2018`, `formgen-bands`: 0)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | PASS | 0 | `2200M/cap-p-1.png`, `2200M/cap-p-2.png` |
| Capacity + 3 | 3 | Fallback | PASS | 0 | `2200M/cap_p3-p-1.png`, `2200M/cap_p3-p-2.png`, `2200M/cap_p3-p-3.png` |
| Capacity + 40 | 5 | Fallback | PASS | 0 | `2200M/cap_p40-p-1.png`, `2200M/cap_p40-p-2.png`, `2200M/cap_p40-p-3.png` (+2 more) |

- **Verdict:** No `formgen-bands` metadata (`fb=0`); engine does not run band-flow and falls back cleanly to legacy `ADDITIONAL SHEET` continuation pages (0 missing values).
- **Defects:** Not yet on band-flow (`formgen-bands` is empty). Legacy continuation prints all rows cleanly.

### 2200C (`2200c-2018`, `formgen-bands`: 1)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p2: 2200c-2018 (cap): repeating cell p2c10 expected 1..=1, got 0) | 0 | `2200C/cap-p-1.png`, `2200C/cap-p-2.png` |
| Capacity + 3 | 2 | No (static) | FAIL (p2: 2200c-2018 (cap_p3): repeating cell p2c10 expected 1..=1, got 0) | 8+ (e.g. `Schedule 1 r1 [column A: cosmetic procedures performed, exempt]=S0R1C0`) | `2200C/cap_p3-p-1.png`, `2200C/cap_p3-p-2.png` |
| Capacity + 40 | 3 | Yes | FAIL (p2: 2200c-2018 (cap_p40): repeating cell p2c10 expected 1..=2, got 0) | 8+ (e.g. `Schedule 1 r1 [column A: cosmetic procedures performed, exempt]=S0R1C0`) | `2200C/cap_p40-p-1.png`, `2200C/cap_p40-p-2.png`, `2200C/cap_p40-p-3.png` |

- **Verdict:** CRITICAL ENGINE FAULT: `<div class="band">` wrapper wipes all 10 original + 3/40 extra Schedule 1 rows blank.
- **Defects:** 1) Engine fault (`<div class="band">`): all filled rows wiped blank; conservation fails on `p2c10`. 2) Known engine fault: top horizontal border of Schedule 1 header missing on outer edges.

### 2550M (`2550m-2007`, `formgen-bands`: 7)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p2: assertion `left == right` failed: 2550m-2007 (cap): non-repeating cell ) | 0 | `2550M/cap-p-1.png`, `2550M/cap-p-2.png` |
| Capacity + 3 | 3 | Yes | FAIL (p2: 2550m-2007 (cap_p3) page idx 0: svg None [198..1038.96] outside [0, 990) | 8+ (e.g. `Schedule 1 r1 [ATC]=S0R1C0`) | `2550M/cap_p3-p-1.png`, `2550M/cap_p3-p-2.png`, `2550M/cap_p3-p-3.png` |
| Capacity + 40 | 6 | Yes | FAIL (p2: 2550m-2007 (cap_p40) page idx 1: svg None [-11.76..179.76000000000002] ) | 8+ (e.g. `Schedule 1 r1 [ATC]=S0R1C0`) | `2550M/cap_p40-p-1.png`, `2550M/cap_p40-p-2.png`, `2550M/cap_p40-p-3.png` (+3 more) |

- **Verdict:** CRITICAL ENGINE FAULTS: `<div class="band">` wipes all 7 schedules blank, Schedule 3C/Schedule 4 bands OVERLAP each other, and Schedule 7 vertical rules OVERFLOW the 1008pt paper height (`max_y=1039.0pt > 1008.0pt`).
- **Defects:** 1) Engine fault (`<div class="band">`): all filled rows wiped blank; conservation fails on `p2c153`. 2) Engine fault (overlap): `C) Total Input Tax Deferred` overlaps `Schedule 4` header and first line. 3) Engine fault (paper overflow): Schedule 7 vertical column dividers shoot past the bottom border to `1039.0pt` (past `content_bottom_y=990pt` and `1008pt` paper), colliding with `Continued on the next page`.

### 1600PT (`1600-pt-2018`, `formgen-bands`: 2)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p1: assertion `left == right` failed: 1600-pt-2018 (cap): non-repeating tex) | 0 | `1600PT/cap-p-1.png`, `1600PT/cap-p-2.png` |
| Capacity + 3 | 2 | No (static) | FAIL (p1: assertion `left == right` failed: 1600-pt-2018 (cap_p3): non-repeating ) | 8+ (e.g. `Schedule 1 r2 [TIN]=S0R2C0`) | `1600PT/cap_p3-p-1.png`, `1600PT/cap_p3-p-2.png` |
| Capacity + 40 | 2 | No (static) | FAIL (p1: assertion `left == right` failed: 1600-pt-2018 (cap_p40): non-repeating) | 8+ (e.g. `Schedule 1 r2 [TIN]=S0R2C0`) | `1600PT/cap_p40-p-1.png`, `1600PT/cap_p40-p-2.png` |

- **Verdict:** CRITICAL ENGINE FAULT (`<div class="band">` wrapper): filled rows inside `<div class="band">` are wiped blank (`8+` missing values, conservation fails on `p1: assertion `left == right` failed: 1600-pt-2018 (cap): non-repeatin`); outer side borders also lost above/below growth.

### 1600VT (`1600-vt-2018`, `formgen-bands`: 2)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p1: assertion `left == right` failed: 1600-vt-2018 (cap): non-repeating tex) | 0 | `1600VT/cap-p-1.png`, `1600VT/cap-p-2.png` |
| Capacity + 3 | 2 | No (static) | FAIL (p1: assertion `left == right` failed: 1600-vt-2018 (cap_p3): non-repeating ) | 8+ (e.g. `Schedule 1 r2 [TIN]=S0R2C0`) | `1600VT/cap_p3-p-1.png`, `1600VT/cap_p3-p-2.png` |
| Capacity + 40 | 2 | No (static) | FAIL (p1: assertion `left == right` failed: 1600-vt-2018 (cap_p40): non-repeating) | 8+ (e.g. `Schedule 1 r2 [TIN]=S0R2C0`) | `1600VT/cap_p40-p-1.png`, `1600VT/cap_p40-p-2.png` |

- **Verdict:** CRITICAL ENGINE FAULT (`<div class="band">` wrapper): filled rows inside `<div class="band">` are wiped blank (`8+` missing values, conservation fails on `p1: assertion `left == right` failed: 1600-vt-2018 (cap): non-repeatin`); outer side borders also lost above/below growth.

### 1601FQ (`1601-fq-2020`, `formgen-bands`: 2)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p1: assertion `left == right` failed: 1601-fq-2020 (cap): non-repeating tex) | 0 | `1601FQ/cap-p-1.png`, `1601FQ/cap-p-2.png` |
| Capacity + 3 | 2 | No (static) | FAIL (p1: assertion `left == right` failed: 1601-fq-2020 (cap_p3): non-repeating ) | 8+ (e.g. `Schedule 1 r1 [Nature of Income Payment]=S0R1C0`) | `1601FQ/cap_p3-p-1.png`, `1601FQ/cap_p3-p-2.png` |
| Capacity + 40 | 2 | No (static) | FAIL (p1: assertion `left == right` failed: 1601-fq-2020 (cap_p40): non-repeating) | 8+ (e.g. `Schedule 1 r1 [Nature of Income Payment]=S0R1C0`) | `1601FQ/cap_p40-p-1.png`, `1601FQ/cap_p40-p-2.png` |

- **Verdict:** CRITICAL ENGINE FAULT (`<div class="band">` wrapper): filled rows inside `<div class="band">` are wiped blank (`8+` missing values, conservation fails on `p1: assertion `left == right` failed: 1601-fq-2020 (cap): non-repeatin`); outer side borders also lost above/below growth.

### 1601EQ (`1601eq-2019`, `formgen-bands`: 1)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p1: assertion `left == right` failed: 1601eq-2019 (cap): non-repeating text) | 0 | `1601EQ/cap-p-1.png`, `1601EQ/cap-p-2.png` |
| Capacity + 3 | 3 | Yes | FAIL (p1: assertion `left == right` failed: 1601eq-2019 (cap_p3): non-repeating t) | 8+ (e.g. `ATC, Part II r1 [(item 13)]=S0R1C0`) | `1601EQ/cap_p3-p-1.png`, `1601EQ/cap_p3-p-2.png`, `1601EQ/cap_p3-p-3.png` |
| Capacity + 40 | 3 | Yes | FAIL (p1: assertion `left == right` failed: 1601eq-2019 (cap_p40): non-repeating ) | 8+ (e.g. `ATC, Part II r1 [(item 13)]=S0R1C0`) | `1601EQ/cap_p40-p-1.png`, `1601EQ/cap_p40-p-2.png`, `1601EQ/cap_p40-p-3.png` |

- **Verdict:** CRITICAL ENGINE FAULT (`<div class="band">` wrapper): filled rows inside `<div class="band">` are wiped blank (`8+` missing values, conservation fails on `p1: assertion `left == right` failed: 1601eq-2019 (cap): non-repeating`); outer side borders also lost above/below growth.

### 1602Q (`1602q-2019`, `formgen-bands`: 2)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 3 | No (static) | FAIL (p2: assertion `left == right` failed: 1602q-2019 (cap): non-repeating text ) | 0 | `1602Q/cap-p-1.png`, `1602Q/cap-p-2.png`, `1602Q/cap-p-3.png` |
| Capacity + 3 | 4 | Yes | FAIL (p2: assertion `left == right` failed: 1602q-2019 (cap_p3): non-repeating te) | 8+ (e.g. `Schedule 2 r1 [Total Interest]=10017.00`) | `1602Q/cap_p3-p-1.png`, `1602Q/cap_p3-p-2.png`, `1602Q/cap_p3-p-3.png` (+1 more) |
| Capacity + 40 | 5 | Yes | FAIL (p2: assertion `left == right` failed: 1602q-2019 (cap_p40): non-repeating t) | 8+ (e.g. `Schedule 2 r1 [Total Interest]=10017.00`) | `1602Q/cap_p40-p-1.png`, `1602Q/cap_p40-p-2.png`, `1602Q/cap_p40-p-3.png` (+2 more) |

- **Verdict:** CRITICAL ENGINE FAULT (`<div class="band">` wrapper): filled rows inside `<div class="band">` are wiped blank (`8+` missing values, conservation fails on `p2: assertion `left == right` failed: 1602q-2019 (cap): non-repeating `); outer side borders also lost above/below growth.

### 1603Q (`1603q-2018`, `formgen-bands`: 0)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | PASS | 0 | `1603Q/cap-p-1.png`, `1603Q/cap-p-2.png` |
| Capacity + 3 | 3 | Fallback | PASS | 0 | `1603Q/cap_p3-p-1.png`, `1603Q/cap_p3-p-2.png`, `1603Q/cap_p3-p-3.png` |
| Capacity + 40 | 5 | Fallback | PASS | 0 | `1603Q/cap_p40-p-1.png`, `1603Q/cap_p40-p-2.png`, `1603Q/cap_p40-p-3.png` (+2 more) |

- **Verdict:** No `formgen-bands` (`fb=0`); falls back to legacy continuation sheet with 0 missing values.

### 1700 (`1700-2018`, `formgen-bands`: 1)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p2: assertion `left == right` failed: 1700-2018 (cap): non-repeating text p) | 0 | `1700/cap-p-1.png`, `1700/cap-p-2.png` |
| Capacity + 3 | 3 | No (static) | FAIL (p2: assertion `left == right` failed: 1700-2018 (cap_p3): non-repeating tex) | 0 | `1700/cap_p3-p-1.png`, `1700/cap_p3-p-2.png`, `1700/cap_p3-p-3.png` |
| Capacity + 40 | 5 | No (static) | FAIL (p2: assertion `left == right` failed: 1700-2018 (cap_p40): non-repeating te) | 0 | `1700/cap_p40-p-1.png`, `1700/cap_p40-p-2.png`, `1700/cap_p40-p-3.png` (+2 more) |

- **Verdict:** Popup-only schedules fall back to legacy continuation sheet (0 missing values), while conservation fails on `<div class="band">` text/cell parsing (`p2: assertion `left == right` failed: 1700-2018 (cap): non-repeating t`).

### 1702EX (`1702ex-2018`, `formgen-bands`: 1)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 3 | No (static) | FAIL (p3: assertion `left == right` failed: 1702ex-2018 (cap): non-repeating text) | 0 | `1702EX/cap-p-1.png`, `1702EX/cap-p-2.png`, `1702EX/cap-p-3.png` |
| Capacity + 3 | 5 | No (static) | FAIL (p3: assertion `left == right` failed: 1702ex-2018 (cap_p3): non-repeating t) | 0 | `1702EX/cap_p3-p-1.png`, `1702EX/cap_p3-p-2.png`, `1702EX/cap_p3-p-3.png` (+2 more) |
| Capacity + 40 | 15 | No (static) | FAIL (p3: assertion `left == right` failed: 1702ex-2018 (cap_p40): non-repeating ) | 0 | `1702EX/cap_p40-p-01.png`, `1702EX/cap_p40-p-02.png`, `1702EX/cap_p40-p-03.png` (+12 more) |

- **Verdict:** Popup-only schedules fall back to legacy continuation sheet (0 missing values), while conservation fails on `<div class="band">` text/cell parsing (`p3: assertion `left == right` failed: 1702ex-2018 (cap): non-repeating`).

### 1702MX (`1702mx-2018c`, `formgen-bands`: 4)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 4 | No (static) | FAIL (p3: assertion `left == right` failed: 1702mx-2018c (cap): non-repeating tex) | 0 | `1702MX/cap-p-1.png`, `1702MX/cap-p-2.png`, `1702MX/cap-p-3.png` (+1 more) |
| Capacity + 3 | 5 | No (static) | FAIL (p3: assertion `left == right` failed: 1702mx-2018c (cap_p3): non-repeating ) | 0 | `1702MX/cap_p3-p-1.png`, `1702MX/cap_p3-p-2.png`, `1702MX/cap_p3-p-3.png` (+2 more) |
| Capacity + 40 | 7 | No (static) | FAIL (p3: assertion `left == right` failed: 1702mx-2018c (cap_p40): non-repeating) | 0 | `1702MX/cap_p40-p-1.png`, `1702MX/cap_p40-p-2.png`, `1702MX/cap_p40-p-3.png` (+4 more) |

- **Verdict:** Popup-only schedules fall back to legacy continuation sheet (0 missing values), while conservation fails on `<div class="band">` text/cell parsing (`p3: assertion `left == right` failed: 1702mx-2018c (cap): non-repeatin`).

### 1702RT (`1702rt-2018c`, `formgen-bands`: 4)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 4 | No (static) | FAIL (p3: assertion `left == right` failed: 1702rt-2018c (cap): non-repeating tex) | 0 | `1702RT/cap-p-1.png`, `1702RT/cap-p-2.png`, `1702RT/cap-p-3.png` (+1 more) |
| Capacity + 3 | 6 | No (static) | FAIL (p3: assertion `left == right` failed: 1702rt-2018c (cap_p3): non-repeating ) | 8+ (e.g. `Schedule II r5 [Description]=P2R5C0`) | `1702RT/cap_p3-p-1.png`, `1702RT/cap_p3-p-2.png`, `1702RT/cap_p3-p-3.png` (+3 more) |
| Capacity + 40 | 17 | Yes | FAIL (p3: assertion `left == right` failed: 1702rt-2018c (cap_p40): non-repeating) | 8+ (e.g. `Schedule II r5 [Description]=P2R5C0`) | `1702RT/cap_p40-p-01.png`, `1702RT/cap_p40-p-02.png`, `1702RT/cap_p40-p-03.png` (+14 more) |

- **Verdict:** CRITICAL ENGINE FAULT (`<div class="band">` wrapper): filled rows inside `<div class="band">` are wiped blank (`8+` missing values, conservation fails on `p3: assertion `left == right` failed: 1702rt-2018c (cap): non-repeatin`); outer side borders also lost above/below growth.

### 1706 (`1706-2018`, `formgen-bands`: 2)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | — | skipped (`no_schedules` in safety test) | — | — | — |
| Capacity + 3 | — | skipped (`no_schedules` in safety test) | — | — | — |
| Capacity + 40 | — | skipped (`no_schedules` in safety test) | — | — | — |

- **Verdict:** Skipped by `discover_schedules` (no repeatable writer-cell stem on `try/forms-band-flow`; `formgen-bands=2`).

### 1707 (`1707-2021`, `formgen-bands`: 2)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | PASS | 0 | `1707/cap-p-1.png`, `1707/cap-p-2.png` |
| Capacity + 3 | 3 | Yes | PASS | 0 | `1707/cap_p3-p-1.png`, `1707/cap_p3-p-2.png`, `1707/cap_p3-p-3.png` |
| Capacity + 40 | 6 | Yes | PASS | 0 | `1707/cap_p40-p-1.png`, `1707/cap_p40-p-2.png`, `1707/cap_p40-p-3.png` (+3 more) |

- **Verdict:** Schedules 2 & 3 grow cleanly in place (`cap+3`) and break cleanly to `Page 2 (continued)` (`cap+40`); 0 missing values, conservation passes. Only defect: known engine fault (Schedule 1 above growth loses outer left/right borders).

### 1707A (`1707a-2021`, `formgen-bands`: 2)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | FAIL (p2: assertion `left == right` failed: 1707a-2021 (cap): non-repeating text ) | 0 | `1707A/cap-p-1.png`, `1707A/cap-p-2.png` |
| Capacity + 3 | 3 | No (static) | FAIL (p2: assertion `left == right` failed: 1707a-2021 (cap_p3): non-repeating te) | 0 | `1707A/cap_p3-p-1.png`, `1707A/cap_p3-p-2.png`, `1707A/cap_p3-p-3.png` |
| Capacity + 40 | 7 | No (static) | FAIL (p2: assertion `left == right` failed: 1707a-2021 (cap_p40): non-repeating t) | 0 | `1707A/cap_p40-p-1.png`, `1707A/cap_p40-p-2.png`, `1707A/cap_p40-p-3.png` (+4 more) |

- **Verdict:** Popup-only schedules fall back to legacy continuation sheet (0 missing values), while conservation fails on `<div class="band">` text/cell parsing (`p2: assertion `left == right` failed: 1707a-2021 (cap): non-repeating `).

### 2200A (`2200a-2020`, `formgen-bands`: 0)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | — | skipped (`no_schedules` in safety test) | — | — | — |
| Capacity + 3 | — | skipped (`no_schedules` in safety test) | — | — | — |
| Capacity + 40 | — | skipped (`no_schedules` in safety test) | — | — | — |

- **Verdict:** Skipped by `discover_schedules` (no repeatable writer-cell stem on `try/forms-band-flow`; `formgen-bands=0`).

### 2200AN (`2200an-2018`, `formgen-bands`: 0)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | PASS | 0 | `2200AN/cap-p-1.png`, `2200AN/cap-p-2.png` |
| Capacity + 3 | 3 | Fallback | PASS | 0 | `2200AN/cap_p3-p-1.png`, `2200AN/cap_p3-p-2.png`, `2200AN/cap_p3-p-3.png` |
| Capacity + 40 | 5 | Fallback | PASS | 0 | `2200AN/cap_p40-p-1.png`, `2200AN/cap_p40-p-2.png`, `2200AN/cap_p40-p-3.png` (+2 more) |

- **Verdict:** No `formgen-bands` (`fb=0`); falls back to legacy continuation sheet with 0 missing values.

### 2200P (`2200p-2020`, `formgen-bands`: 0)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | 2 | No (static) | PASS | 0 | `2200P/cap-p-1.png`, `2200P/cap-p-2.png` |
| Capacity + 3 | 3 | Fallback | PASS | 0 | `2200P/cap_p3-p-1.png`, `2200P/cap_p3-p-2.png`, `2200P/cap_p3-p-3.png` |
| Capacity + 40 | 5 | Fallback | PASS | 0 | `2200P/cap_p40-p-1.png`, `2200P/cap_p40-p-2.png`, `2200P/cap_p40-p-3.png` (+2 more) |

- **Verdict:** No `formgen-bands` (`fb=0`); falls back to legacy continuation sheet with 0 missing values.

### 2200S (`2200s-2018`, `formgen-bands`: 0)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | — | skipped (`no_schedules` in safety test) | — | — | — |
| Capacity + 3 | — | skipped (`no_schedules` in safety test) | — | — | — |
| Capacity + 40 | — | skipped (`no_schedules` in safety test) | — | — | — |

- **Verdict:** Skipped by `discover_schedules` (no repeatable writer-cell stem on `try/forms-band-flow`; `formgen-bands=0`).

### 2200T (`2200t-2022`, `formgen-bands`: 0)

| Case | Pages | Band-Flow? | Conservation | Missing Values | Picture(s) |
| --- | --- | --- | --- | --- | --- |
| Capacity | — | skipped (`no_schedules` in safety test) | — | — | — |
| Capacity + 3 | — | skipped (`no_schedules` in safety test) | — | — | — |
| Capacity + 40 | — | skipped (`no_schedules` in safety test) | — | — | — |

- **Verdict:** Skipped by `discover_schedules` (no repeatable writer-cell stem on `try/forms-band-flow`; `formgen-bands=0`).
