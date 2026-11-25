# Band-Flow Engine Sweep Report (Sweep 2 vs `origin/try/forms-band-flow` @ `0d3fa1fb`)

- **Engine head tested:** `origin/try/forms-band-flow` @ `0d3fa1fb` (carrying refreshed `formgen-page-layout` metadata from `feat/forms-page-layout-data` @ `ab797ea5`, generator `buwiz-forms-html` `forms-layout-fixes` @ `d7f359a`)
- **Cases tested per growable form:** `cap` (capacity), `cap_p3` (capacity + 3 rows), `cap_p40` (capacity + 40 rows)
- **Rendered evidence:** `287` PNGs across `27` active growable forms under `form-review/band-flow-sweep/<CODE>/` (all pages visually inspected; `0` conservation errors, `0` missing values, `0` paper-edge overflows across all 81 HTMLs)

---

## 1. Status of the Five Defects from Sweep 1 (`446da0e5` -> `0d3fa1fb`)

| # | Sweep 1 Defect | Sweep 2 Status | Exact Forms |
| --- | --- | --- | --- |
| 1 | `<div class="band">` wrapper wipes filled rows on 19 active forms | **GONE (19/19 fixed)** | All 19 wrapped-band forms now render filled rows with `0` missing values and `0` conservation errors. |
| 2 | Lost outer side borders + shrunk elastic-box bottom border | **MOSTLY GONE (11/12 fixed)** | Fixed on `1801`, `1600WP` (including shrunk `Machine Validation` bottom border), `2550Q`, `1701A`, `1707`, `1601EQ`, `1602Q`, `1702RT`, `2552`, `1800`, and `2000OT` (`cap+3`). **1 edge case remains:** `2000OT/cap_p40-p-3.png` bottom row (`1.B` row 1) loses outer side/bottom borders at the page break. |
| 3 | `2550M` Sch. 3C/Sch. 4 overlap + `1039pt` paper overflow | **GONE** | `2550M/cap_p3-p-2.png` breaks cleanly with `Continued on the next page`; `0` paper-edge overflows across all 81 HTMLs. |
| 4 | Left-aligned amounts + `1701A` row-number collisions | **PARTIAL** | **Amounts:** fixed on `1600WP`, `2550Q`, `2000OT`, `1800`, `2552`, `2200C`, `2550M`, `1600PT`, `1600VT`; **still left-aligned** on `1801` (`EXCLUSIVE` & `CONJUGAL/COMMUNAL`) and `1601FQ` (`Amount of Tax Withheld (G)`). **Item numbers:** fixed on `1701A` (`42a-c`, `51a-c`) and `1601C` (`3a-3an`); **still colliding/wrong** on `1601EQ` (`7, 8, 9` after `18`), `1602Q` (`1, 2, 3, 3, 4, 5` before `4 Total`), `1702RT` (`5, 6, 7` before `5 Total`), `1800` (`6..8` duplicated after `10`). |
| 5 | Gapped `table_head` + `section_title` not repeated on break | **NOT FIXED** | `1800/cap_p3-p-3.png` (`Schedule B – Continuation`) still omits `table_head` and `section_title`; `1602Q/cap_p3-p-3.png` (`Schedule 4`) breaks mid-table without header; `section_title` (`repeat_on_break: true` in metadata) is **not repeated** on any band-flow continuation page (`1600WP`, `2550Q`, `2551Q`, `1601C`, `2000`, `2200C`, `2550M`, `1707`, `1702RT`). |

## 2. Two New Visual Defects Found on `0d3fa1fb`

1. **`(continued)` stamped over column headers / vertical dividers on repeated `table_head`:**
   - **Direct text collision (broken):** `2550Q/cap_p3-p-3.png` (`(continued)` overwrites `(E) Amount Paid`), `2550M/cap_p3-p-3.png` (`(continued)` overwrites `Name of Miller`; page label also stays `Page 2` instead of `Page 2 (continued)`).
   - **Crosses column divider (tweak):** `2551Q/cap_p40-p-3.png`, `2200C/cap_p40-p-3.png`, `2000/cap_p40-p-3.png`.
2. **Row `5` pushed after extra rows (`1, 2, 3, 4, 6, 7, 8, 5`):**
   - `2000OT/cap_p3-p-2.png` (`1.A - Real Property`) and `2552/cap_p3-p-2.png` / `cap_p3-p-3.png` (`Schedules 1–3` upper halves, `Part VI`): extra rows `6, 7, 8` insert after row `4` (inside `<div class="band">`), leaving printed row `5` below row `8`.

---

## 3. Per-Form Verdicts & Tables (Priority Order)

### `1801` (`1801-2018`) — Verdict: `tweak`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 2 | Band-flow (no growth) | PASS | 0 | Clean baseline (`1801/cap-p-2.png`). |
| `cap_p3` | 3 | Band-flow | PASS | 0 | Outer borders restored; `EXCLUSIVE` and `CONJUGAL/COMMUNAL` amounts are left-aligned across Sch. 1 cont., 1A, 2, 2A, 3, 4 (`1801/cap_p3-p-2.png`, `1801/cap_p3-p-3.png`). |
| `cap_p40` | 6 | Band-flow | PASS | 0 | Same left-aligned `EXCLUSIVE`/`CONJUGAL` amounts; clean breaks otherwise (`1801/cap_p40-p-2.png`). |

### `1600WP` (`1600wp-2010`) — Verdict: `clean`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 1 | Band-flow (no growth) | PASS | 0 | Clean baseline (`1600WP/cap-p-1.png`). |
| `cap_p3` | 1 | Band-flow | PASS | 0 | Grows in place cleanly; outer borders & shrunk `Machine Validation` bottom border restored; amounts right-aligned (`1600WP/cap_p3-p-1.png`). |
| `cap_p40` | 2 | Band-flow | PASS | 0 | Breaks cleanly to page 2 with `PAYEE DETAILS (continued)` header, Part III + Machine Validation below TOTAL (`1600WP/cap_p40-p-2.png`). Note: `Schedule 1.1` section title bar not repeated above `table_head`. |

### `2550Q` (`2550q-2024`) — Verdict: `broken`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 2 | Band-flow (no growth) | PASS | 0 | Clean baseline (`2550Q/cap-p-2.png`). |
| `cap_p3` | 3 | Band-flow | PASS | 0 | Page 2 clean (borders restored, amounts right-aligned); **Page 2 (continued) broken:** `(continued)` is stamped directly on top of `(E) Amount Paid` (unreadable collision) and `Schedule 4` section title is not repeated (`2550Q/cap_p3-p-2.png`, `2550Q/cap_p3-p-3.png`). |
| `cap_p40` | 4 | Band-flow | PASS | 0 | Same `(continued)` collision over repeated header on continuation pages (`2550Q/cap_p40-p-3.png`). |

### `1701A` (`1701a-2018`) — Verdict: `tweak`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 2 | Band-flow (no growth) | PASS | 0 | Clean baseline (`1701A/cap-p-2.png`). |
| `cap_p3` | 3 | Band-flow | PASS | 0 | Borders restored; row numbers fixed to `42a-c` / `51a-c` (no collision with `43`/`52`); tiny tweak: `a/b/c` slightly touches the vertical border / first char of `P0R43C0` (`1701A/cap_p3-p-2.png`, `1701A/cap_p3-p-3.png`). |
| `cap_p40` | 4 | Band-flow | PASS | 0 | Clean breaks; same tight `42a..`/`51a..` label spacing (`1701A/cap_p40-p-2.png`). |

### `2000OT` (`2000-ot-2018`) — Verdict: `broken`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 2 | Band-flow (no growth) | PASS | 0 | Clean baseline (`2000OT/cap-p-2.png`). |
| `cap_p3` | 3 | Band-flow | PASS | 0 | Rows restored (Defect 1 fixed), borders restored, amounts right-aligned; **broken:** `1.A - Real Property` rows print in order `1, 2, 3, 4, 6, 7, 8, 5` (row 5 stranded at bottom) (`2000OT/cap_p3-p-2.png`). |
| `cap_p40` | 7 | Band-flow | PASS | 0 | Same row-5 ordering bug; plus bottom row (`1.B` row 1) on `2000OT/cap_p40-p-3.png` loses outer side and bottom borders at the page break. |

### `1800` (`1800-2018`) — Verdict: `broken`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 2 | Band-flow (no growth) | PASS | 0 | Clean baseline (`1800/cap-p-2.png`). |
| `cap_p3` | 3 | Band-flow | PASS | 0 | Rows & borders restored; **broken:** (1) `Schedule B – Continuation` breaks to `Page 2 (continued)` with **no repeated `table_head` and no `section_title`** (`1800/cap_p3-p-3.png`); (2) extra rows `6, 7, 8` duplicate printed blank rows `6..10` (`1800/cap_p3-p-2.png`). |
| `cap_p40` | 4 | Band-flow | PASS | 0 | Same missing repeated header on continuation and duplicate numbering (`1800/cap_p40-p-3.png`). |

### `2552` (`2552-2018`) — Verdict: `broken`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 2 | Band-flow (no growth) | PASS | 0 | Clean baseline (`2552/cap-p-2.png`). |
| `cap_p3` | 3 | Band-flow | PASS | 0 | Rows & Part IV borders restored, amounts right-aligned; **broken:** Schedules 1–3 upper halves and Part VI print rows out of order `1, 2, 3, 4, 6, 7, 8, 5` (`2552/cap_p3-p-2.png`, `2552/cap_p3-p-3.png`). |
| `cap_p40` | 6 | Band-flow | PASS | 0 | Same row-5 out-of-order defect (`2552/cap_p40-p-2.png`). |

### `2551Q` (`2551q-2018`) — Verdict: `tweak`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 2 | Band-flow (no growth) | PASS | 0 | Clean baseline (`2551Q/cap-p-2.png`). |
| `cap_p3` | 2 | Band-flow | PASS | 0 | Clean in-place growth (`1..9`), comb digits & borders intact (`2551Q/cap_p3-p-2.png`). |
| `cap_p40` | 3 | Band-flow | PASS | 0 | Tweak: `Schedule 1` section title not repeated on `Page 2 (continued)`, and `(continued)` spills across the vertical divider between `ATC` and `Taxable Amount` (`2551Q/cap_p40-p-3.png`). |

### `1601C` (`1601c-2018`) — Verdict: `tweak`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 2 | Band-flow (no growth) | PASS | 0 | Clean baseline (`1601C/cap-p-2.png`). |
| `cap_p3` | 2 | Band-flow | PASS | 0 | Clean in-place growth (`1..3, 3a-3c`), comb digits & borders intact (`1601C/cap_p3-p-2.png`). |
| `cap_p40` | 3 | Band-flow | PASS | 0 | Very clean break (`1..3, 3a..3an`); tiny tweak: 3-char labels `3aa..3an` slightly touch the vertical divider and `Schedule I` section title does not repeat (`1601C/cap_p40-p-3.png`). |

### `2000` (`2000-dst-2018`) — Verdict: `tweak`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 2 | Band-flow (no growth) | PASS | 0 | Clean baseline (`2000/cap-p-2.png`). |
| `cap_p3` | 3 | Band-flow | PASS | 0 | Clean in-place growth on Schedules 1–4 (`2000/cap_p3-p-2.png`). |
| `cap_p40` | 8 | Band-flow | PASS | 0 | Tweak: `Schedule 1` section title not repeated on `Page 2 (continued)` and `ATC (continued)` crosses the vertical column divider into `Tax Base` (`2000/cap_p40-p-3.png`). |

### `2200M` (`2200m-2018`) — Verdict: `clean` (legacy fallback)
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` / `cap_p3` / `cap_p40` | 2 / 3 / 5 | Legacy `ADDITIONAL SHEET` (`fb=0`) | PASS | 0 | Clean legacy continuation with right-aligned amounts and subtotals (`2200M/cap_p3-p-3.png`). |

### `2200C` (`2200c-2018`) — Verdict: `tweak`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 2 | Band-flow (no growth) | PASS | 0 | Clean baseline (`2200C/cap-p-2.png`). |
| `cap_p3` | 2 | Band-flow | PASS | 0 | Clean in-place growth (13 rows), borders intact, Col (J) right-aligned (`2200C/cap_p3-p-2.png`). |
| `cap_p40` | 3 | Band-flow | PASS | 0 | Tweak: `Schedule 1` section title not repeated on `Page 2 (continued)` and `COSMETIC PROCEDURES (continued)` spills across the vertical divider into `GROSS RECEIPTS` (`2200C/cap_p40-p-3.png`). |

### `2550M` (`2550m-2007`) — Verdict: `broken`
| Case | Pages | Engine | Conservation | Missing | Visual Findings & Pictures |
| --- | --- | --- | --- | --- | --- |
| `cap` | 2 | Band-flow (no growth) | PASS | 0 | Clean baseline (`2550M/cap-p-2.png`). |
| `cap_p3` | 3 | Band-flow | PASS | 0 | Page 2 fixed (all 7 schedules filled, Sch. 3C/Sch. 4 overlap gone, `1039pt` overflow gone, `Continued on the next page` clean); **Page 2 (continued) broken:** `(continued)` overwrites `Name of Miller`, `Schedule 7` section title not repeated, page label says `Page 2` instead of `Page 2 (continued)` (`2550M/cap_p3-p-2.png`, `2550M/cap_p3-p-3.png`). |
| `cap_p40` | 6 | Band-flow | PASS | 0 | Same `(continued)` collision over column headers and missing `Page 2 (continued)` label (`2550M/cap_p40-p-3.png`). |

### Remaining 14 Active Forms

| Form | Verdict | cap / +3 / +40 pgs | Engine | Visual Findings & Pictures |
| --- | --- | --- | --- | --- |
| **`1600PT`** | `clean` | 2 / 2 / 2 | Band-flow | All rows (`1..8` / `1..45`) render cleanly on Page 2, borders intact, amounts right-aligned (`1600PT/cap_p3-p-2.png`). |
| **`1600VT`** | `clean` | 2 / 2 / 2 | Band-flow | All rows (`1..8` / `1..45`) render cleanly on Page 2, borders intact, amounts right-aligned (`1600VT/cap_p3-p-2.png`). |
| **`1601FQ`** | `tweak` | 2 / 2 / 2 | Band-flow | All rows (`1..8`) and borders clean, but `Amount of Tax Withheld (G)` is **left-aligned** (`1601FQ/cap_p3-p-2.png`). |
| **`1601EQ`** | `broken` | 2 / 3 / 3 | Band-flow | Outer borders & `Machine Validation` move to Page 1 cont. are clean, but extra rows after Item `18` are numbered **`7, 8, 9`** (colliding with Part I items) (`1601EQ/cap_p3-p-1.png`, `1601EQ/cap_p3-p-2.png`). |
| **`1602Q`** | `broken` | 3 / 4 / 5 | Band-flow | Sch. 2 & 3 rows numbered **`1, 2, 3 (blank), 3, 4, 5`** (duplicate `3` + collides with `4 Total`); `Schedule 4` breaks to `Page 2 (continued)` with **no repeated title or column header** (`1602Q/cap_p3-p-2.png`, `1602Q/cap_p3-p-3.png`). |
| **`1707`** | `tweak` | 2 / 3 / 6 | Band-flow | `cap_p3` is `clean` (borders fully restored); `cap_p40` breaks cleanly with `Particulars (continued)`, only missing repeated `Schedule 3` section title (`1707/cap_p3-p-2.png`, `1707/cap_p40-p-3.png`). |
| **`1702RT`** | `broken` | 4 / 6 / 17 | Band-flow | Outer borders restored and comb rows clean, but Sch. II extra rows are numbered **`5, 6, 7`** directly above **`5 Total`** (duplicate `5`), and `Schedule II` section title not repeated on `Page 3 (continued)` (`1702RT/cap_p3-p-3.png`, `1702RT/cap_p40-p-04.png`). |
| **`1603Q`** | `clean` | 2 / 3 / 5 | Legacy (`fb=0`) | Clean `ADDITIONAL SHEET` (`1603Q/cap_p3-p-3.png`). |
| **`1700`** | `clean` | 2 / 3 / 5 | Legacy (popup) | Clean `ADDITIONAL SHEET` (`1700/cap_p3-p-3.png`). |
| **`1702EX`** | `clean` | 3 / 5 / 15 | Legacy (popup) | Clean `ADDITIONAL SHEET` (`1702EX/cap_p3-p-4.png`). |
| **`1702MX`** | `clean` | 4 / 5 / 7 | Legacy (popup) | Clean `ADDITIONAL SHEET` (`1702MX/cap_p3-p-5.png`). |
| **`1707A`** | `clean` | 2 / 3 / 7 | Legacy (popup) | Clean `ADDITIONAL SHEET` (`1707A/cap_p3-p-3.png`). |
| **`2200AN`** | `clean` | 2 / 3 / 5 | Legacy (`fb=0`) | Clean `ADDITIONAL SHEET` (`2200AN/cap_p3-p-3.png`). |
| **`2200P`** | `clean` | 2 / 3 / 5 | Legacy (`fb=0`) | Clean `ADDITIONAL SHEET` (`2200P/cap_p3-p-3.png`). |

*(16 forms without repeatable schedules in `writer-cells.json` — `0605`, `0619E`, `0619F`, `1604C`, `1604E`, `1604F`, `1606`, `1701`, `1701MS`, `1701Q`, `1702Q`, `1706`, `2200A`, `2200S`, `2200T`, `2553` — have no growable-table overflow cases.)*
