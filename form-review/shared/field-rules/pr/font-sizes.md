# Rendered Font-Size Audit (All 43 Frozen Forms)

Measured directly from headless Chromium (`getComputedStyle(el).fontSize` under `@media print`, `1pt = 1.3333px`) on rendered filled sheets at `feat/store-launch-field-rules` (`2d3773c1`, merged with `origin/main` `92b3e201`; `html-frozen/` and `crates/bir-print/` unchanged since `3074d666`), with every joined writer key populated (including 137-char names/addresses and overflowing amounts to exercise the shrink engine).

## Summary

- **Forms audited:** 43
- **Shrink-engine inline floor:** `7.50pt` (`10.00px`) — the engine never emits an inline `font-size` below `7.50pt`.
- **Forms with rendered filled values `< 7.50pt` (base frozen `.fh*` stylesheet class, untouched when value fits):** 8 (`1601c-2018`, `1602q-2019`, `1603q-2018`, `1604f-2018`, `1801-2018`, `2200a-2020`, `2550m-2007`, `2550q-2024`).
- **Forms with rendered filled values `< 6.00pt` (`< 8.00px`, below Android's ~8 px floor):** 1 (`2550m-2007` at `5.20pt` / `6.93px` in Schedule 6/7/8 narrow `.fh*` cells).

## 1. Smallest Rendered Font Size for Filled Values (Per Form)

| Form (`slug`) | Min Filled (`pt`) | Min Filled (`px`) | Count at Min | Sample Boxes at Min | `< 7.5pt` Count | `< 7.5pt` Boxes (Cause) |
| --- | ---: | ---: | ---: | --- | ---: | --- |
| `0605-1999` | 7.50 | 10.00 | 1 | `p1c18` | 0 | None |
| `0619e-2018` | 7.85 | 10.47 | 3 | `p1c104`, `p1c113`, `p1c131` | 0 | None |
| `0619f-2018` | 7.50 | 10.00 | 1 | `p1c20-i` | 0 | None |
| `1600-pt-2018` | 7.50 | 10.00 | 7 | `p2c17-i`, `p2c24-i`, `p2c31-i`, `p2c37-i`, `p2c38-i`, `p2c44-i` (+1 more) | 0 | None |
| `1600-vt-2018` | 7.50 | 10.00 | 7 | `p2c17-i`, `p2c24-i`, `p2c31-i`, `p2c37-i`, `p2c38-i`, `p2c44-i` (+1 more) | 0 | None |
| `1600wp-2010` | 7.50 | 10.00 | 14 | `p1c64`, `p1c67`, `p1c96-i`, `p1c98-i`, `p1c104-i`, `p1c106-i` (+6 more) | 0 | None |
| `1601-fq-2020` | 7.50 | 10.00 | 1 | `p1c52` | 0 | None |
| `1601c-2018` | 6.95 | 9.27 | 40 | `p1c9-s0`, `p1c9-s1`, `p1c10-s0`, `p1c10-s1`, `p1c10-s2`, `p1c10-s3` (+6 more) | 40 | `p1c9-s0` (6.96pt, `fi fh20 fc`), `p1c9-s1` (6.96pt, `fi fh20 fc`), `p1c10-s0` (6.96pt, `fi fh20 fc`), `p1c10-s1` (6.96pt, `fi fh20 fc`) (+36 more; base frozen CSS) |
| `1601eq-2019` | 7.75 | 10.33 | 1 | `p1c30` | 0 | None |
| `1602q-2019` | 6.96 | 9.28 | 83 | `p1c9-s0`, `p1c9-s1`, `p1c9-s2`, `p1c9-s3`, `p1c15-s0`, `p1c15-s1` (+6 more) | 83 | `p1c9-s0` (6.96pt, `fi fh25 fc`), `p1c9-s1` (6.96pt, `fi fh25 fc`), `p1c9-s2` (6.96pt, `fi fh25 fc`), `p1c9-s3` (6.96pt, `fi fh25 fc`) (+79 more; base frozen CSS) |
| `1603q-2018` | 6.00 | 8.00 | 45 | `p1c9-s0`, `p1c9-s1`, `p1c9-s2`, `p1c9-s3`, `p1c15-s0`, `p1c15-s1` (+6 more) | 45 | `p1c9-s0` (6pt, `fi fh13 fc`), `p1c9-s1` (6pt, `fi fh13 fc`), `p1c9-s2` (6pt, `fi fh13 fc`), `p1c9-s3` (6pt, `fi fh13 fc`) (+41 more; base frozen CSS) |
| `1604c-2018` | 7.50 | 10.00 | 2 | `p1c42`, `p1c50` | 0 | None |
| `1604e-2018` | 7.56 | 10.08 | 149 | `p1c7-s0`, `p1c7-s1`, `p1c12-s0`, `p1c12-s1`, `p1c12-s2`, `p1c14-s0` (+6 more) | 0 | None |
| `1604f-2018` | 6.96 | 9.28 | 91 | `p1c4-s0`, `p1c4-s1`, `p1c4-s2`, `p1c4-s3`, `p1c7-s0`, `p1c7-s1` (+6 more) | 91 | `p1c4-s0` (6.96pt, `fi fh11 fc`), `p1c4-s1` (6.96pt, `fi fh11 fc`), `p1c4-s2` (6.96pt, `fi fh11 fc`), `p1c4-s3` (6.96pt, `fi fh11 fc`) (+87 more; base frozen CSS) |
| `1606-2018` | 7.50 | 10.00 | 6 | `p1c25-i`, `p1c40-i`, `p1c63`, `p2c41-i`, `p2c48-i`, `p2c110` | 0 | None |
| `1700-2018` | 7.50 | 10.00 | 5 | `p1c118`, `p2c151-i`, `p2c153-i`, `p2c155-i`, `p2c157-i` | 0 | None |
| `1701-2018` | 7.50 | 10.00 | 13 | `p1c37`, `p1c127-i`, `p1c128-i`, `p2c31`, `p2c134-i`, `p2c139-i` (+6 more) | 0 | None |
| `1701a-2018` | 7.50 | 10.00 | 8 | `p1c114-i`, `p1c115-i`, `p2c29-i`, `p2c34-i`, `p2c62-i`, `p2c67-i` (+2 more) | 0 | None |
| `1701ms-2024` | 7.50 | 10.00 | 7 | `p1c30-i`, `p1c32-i`, `p1c33-i`, `p2c74-i`, `p2c79-i`, `p2c177-i` (+1 more) | 0 | None |
| `1701q-2018` | 7.50 | 10.00 | 3 | `p2c148-i`, `p2c150-i`, `p2c152-i` | 0 | None |
| `1702ex-2018` | 7.50 | 10.00 | 19 | `p1c19-i`, `p1c20-i`, `p1c21-i`, `p1c22-i`, `p3c56`, `p3c60` (+6 more) | 0 | None |
| `1702mx-2018c` | 7.50 | 10.00 | 14 | `p1c19-i`, `p1c20-i`, `p1c21-i`, `p1c22-i`, `p1c88-i`, `p1c92-i` (+6 more) | 0 | None |
| `1702q-2018` | 7.50 | 10.00 | 4 | `p1c20-i`, `p1c21-i`, `p1c22-i`, `p1c23-i` | 0 | None |
| `1702rt-2018c` | 7.50 | 10.00 | 6 | `p1c19-i`, `p1c20-i`, `p1c21-i`, `p1c22-i`, `p1c82-i`, `p1c83-i` | 0 | None |
| `1706-2018` | 7.50 | 10.00 | 7 | `p1c24-i`, `p1c47-i`, `p1c77`, `p2c41-i`, `p2c42-i`, `p2c48-i` (+1 more) | 0 | None |
| `1707-2021` | 7.50 | 10.00 | 7 | `p1c25-i`, `p1c29-i`, `p1c33-i`, `p1c40-i`, `p1c44-i`, `p1c48-i` (+1 more) | 0 | None |
| `1707a-2021` | 8.26 | 11.01 | 1 | `p1c32` | 0 | None |
| `1800-2018` | 7.50 | 10.00 | 16 | `p1c22`, `p1c41-i`, `p1c44-i`, `p1c47-i`, `p1c50-i`, `p1c53-i` (+6 more) | 0 | None |
| `1801-2018` | 6.33 | 8.44 | 2 | `p2c236-i`, `p2c237-i` | 71 | `p2c97-i` (6.65pt, `fi fh9`), `p2c98-i` (6.65pt, `fi fh9`), `p2c99-i` (6.65pt, `fi fh9`), `p2c100-i` (6.65pt, `fi fh9`) (+67 more; base frozen CSS) |
| `2000-dst-2018` | 7.66 | 10.21 | 4 | `p1c90`, `p1c95`, `p1c100`, `p1c106` | 0 | None |
| `2000-ot-2018` | 7.50 | 10.00 | 16 | `p1c22`, `p1c36`, `p1c58-i`, `p2c121-i`, `p2c122-i`, `p2c125-i` (+6 more) | 0 | None |
| `2200a-2020` | 7.41 | 9.88 | 3 | `p2c309-i`, `p2c310-i`, `p2c311-i` | 3 | `p2c309-i` (7.41pt, `fi fh0`), `p2c310-i` (7.41pt, `fi fh0`), `p2c311-i` (7.41pt, `fi fh0`) |
| `2200an-2018` | 7.50 | 10.00 | 120 | `p1c6`, `p1c26`, `p2c23-i`, `p2c24-i`, `p2c25-i`, `p2c26-i` (+6 more) | 0 | None |
| `2200c-2018` | 7.50 | 10.00 | 72 | `p1c28`, `p2c25-i`, `p2c26-i`, `p2c28-i`, `p2c29-i`, `p2c30-i` (+6 more) | 0 | None |
| `2200m-2018` | 7.50 | 10.00 | 12 | `p1c26`, `p1c54`, `p2c6`, `p2c24-i`, `p2c32-i`, `p2c40-i` (+6 more) | 0 | None |
| `2200p-2020` | 7.50 | 10.00 | 139 | `p1c26`, `p2c31-i`, `p2c34-i`, `p2c35-i`, `p2c37-i`, `p2c38-i` (+6 more) | 0 | None |
| `2200s-2018` | 7.50 | 10.00 | 5 | `p1c25`, `p1c50`, `p1c109`, `p1c120`, `p1c142` | 0 | None |
| `2200t-2022` | 7.50 | 10.00 | 77 | `p1c16`, `p1c18`, `p1c20`, `p1c26`, `p2c26-i`, `p2c27-i` (+6 more) | 0 | None |
| `2550m-2007` | 5.20 | 6.93 | 1 | `p2c219-i` | 139 | `p1c77-i` (7.15pt, `fi fh7`), `p2c154-i` (6.5pt, `fi fh4`), `p2c155-i` (6.5pt, `fi fh4`), `p2c156-i` (6.5pt, `fi fh4`) (+135 more; base frozen CSS) |
| `2550q-2024` | 6.71 | 8.95 | 8 | `p2c184-i`, `p2c187-i`, `p2c188-i`, `p2c191-i`, `p2c192-i`, `p2c234-i` (+2 more) | 29 | `p2c184-i` (6.71pt, `fi fh0`), `p2c187-i` (6.71pt, `fi fh0`), `p2c188-i` (6.71pt, `fi fh0`), `p2c191-i` (6.71pt, `fi fh0`) (+25 more; base frozen CSS) |
| `2551q-2018` | 7.50 | 10.00 | 2 | `p1c42`, `p1c187-i` | 0 | None |
| `2552-2018` | 7.50 | 10.00 | 5 | `p2c6-i`, `p2c8-i`, `p2c10-i`, `p2c12-i`, `p2c14-i` | 0 | None |
| `2553-1999` | 7.50 | 10.00 | 2 | `p1c17-i`, `p1c32-i` | 0 | None |

## 2. Smallest Sheet Printed Text (Labels, Captions, Notes, Preprinted Digits)

Measured across all non-filled visible text nodes inside `.page`. Proximity reports whether any filled value sits **inside** the same cell, **beside** (within 12 pt / 16 px), or **no**.

| Form (`slug`) | Min Printed Text (`pt`) | 3 Smallest Classes (`pt` / `px`, count, usage) | Filled Value Inside/Beside? |
| --- | ---: | --- | --- |
| `0605-1999` | 6.02 | `t` (6.02pt/8.03px, n=146: "(To be filled up the BIR); DLN:") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 0605; Additional sheet") · `<td>` (7.50pt/10.00px, n=6: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") | `t`=beside, `<div>`=no, `<td>`=no |
| `0619e-2018` | 5.04 | `t` (5.04pt/6.72px, n=135: "Republic of the Philippines; BCS/") | `t`=beside |
| `0619f-2018` | 5.04 | `t` (5.04pt/6.72px, n=144: "Republic of the Philippines; BCS/") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 0619F; Additional sheet") · `<td>` (7.50pt/10.00px, n=6: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") | `t`=beside, `<div>`=no, `<td>`=no |
| `1600-pt-2018` | 5.52 | `t` (5.52pt/7.36px, n=202: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=24: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1600-PT; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1600-vt-2018` | 5.52 | `t` (5.52pt/7.36px, n=203: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=24: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1600-VT; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1600wp-2010` | 5.04 | `t` (5.04pt/6.72px, n=183: "(To be filled up by the BIR); DLN:") · `<td>` (7.50pt/10.00px, n=48: "123-123-123-00001; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=12: "BIR Form No. 1600WP; Additional sheet (1 of 2)") | `t`=beside, `<td>`=no, `<div>`=no |
| `1601-fq-2020` | 5.04 | `t` (5.04pt/6.72px, n=236: "Republic of the Philippines; For BIR") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1601-FQ; Additional sheet") · `<td>` (7.50pt/10.00px, n=6: "123-123-123-00001; SANTOS, JUANA MARIE DELA CRUZ-VI") | `t`=beside, `<div>`=no, `<td>`=no |
| `1601c-2018` | 5.04 | `t` (5.04pt/6.72px, n=252: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=9: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1601C; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1601eq-2019` | 5.04 | `t` (5.04pt/6.72px, n=194: "Republic of the Philippines; For BIR") | `t`=beside |
| `1602q-2019` | 5.04 | `t` (5.04pt/6.72px, n=363: "Republic of the Philippines; For BIR") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1602Q; Additional sheet") · `<td>` (7.50pt/10.00px, n=6: "123-123-123-00001; SANTOS, JUANA MARIE DELA CRUZ-VI") | `t`=beside, `<div>`=no, `<td>`=no |
| `1603q-2018` | 5.04 | `t` (5.04pt/6.72px, n=203: "Republic of the Philippines; Department of Finance") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1603Q; Additional sheet") · `<td>` (7.50pt/10.00px, n=6: "123-123-123-00001; SANTOS, JUANA MARIE DELA CRUZ-VI") | `t`=beside, `<div>`=no, `<td>`=no |
| `1604c-2018` | 6.96 | `t` (6.96pt/9.28px, n=128: "REPUBLIC OF THE PHILIPPINES; DEPARTMENT OF FINANCE") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1604C; Additional sheet") · `<td>` (7.50pt/10.00px, n=6: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") | `t`=beside, `<div>`=no, `<td>`=no |
| `1604e-2018` | 6.48 | `t` (6.48pt/8.64px, n=182: "REPUBLIC OF THE PHILIPPINES; DEPARTMENT OF FINANCE") | `t`=beside |
| `1604f-2018` | 5.52 | `t` (5.52pt/7.36px, n=266: "Republic of the Philippines; For BIR") | `t`=beside |
| `1606-2018` | 0.96 | `t` (0.96pt/1.28px, n=308: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=18: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1606; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1700-2018` | 5.04 | `t` (5.04pt/6.72px, n=336: "Republic of the Philippines; BCS/") · `<td>` (7.50pt/10.00px, n=18: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1700; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1701-2018` | 6.00 | `t` (6.00pt/8.00px, n=676: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=45: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=12: "BIR Form No. 1701; Additional sheet (1 of 2)") | `t`=beside, `<td>`=no, `<div>`=no |
| `1701a-2018` | 5.52 | `t` (5.52pt/7.36px, n=327: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=27: "123-123-123; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1701A; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1701ms-2024` | 4.00 | `t` (4.00pt/5.33px, n=383: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=23: "—; X") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1701MS; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1701q-2018` | 6.00 | `t` (6.00pt/8.00px, n=340: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=12: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1701Q; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1702ex-2018` | 6.00 | `t` (6.00pt/8.00px, n=364: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=61: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=12: "BIR Form No. 1702EX; Additional sheet (1 of 2)") | `t`=beside, `<td>`=no, `<div>`=no |
| `1702mx-2018c` | 5.04 | `t` (5.04pt/6.72px, n=596: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=46: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=12: "BIR Form No. 1702MX; Additional sheet (1 of 2)") | `t`=beside, `<td>`=no, `<div>`=no |
| `1702q-2018` | 5.04 | `t` (5.04pt/6.72px, n=307: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=14: "123-123-123; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1702Q; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1702rt-2018c` | 6.00 | `t` (6.00pt/8.00px, n=441: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=20: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1702RT; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1706-2018` | 6.00 | `t` (6.00pt/8.00px, n=313: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=21: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1706; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1707-2021` | 5.04 | `t` (5.04pt/6.72px, n=264: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=24: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 1707; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `1707a-2021` | 5.04 | `t` (5.04pt/6.72px, n=246: "Republic of the Philippines; Department of Finance") | `t`=beside |
| `1800-2018` | 6.00 | `t` (6.00pt/8.00px, n=313: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=54: "123-123-123; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=12: "BIR Form No. 1800; Additional sheet (1 of 2)") | `t`=beside, `<td>`=no, `<div>`=no |
| `1801-2018` | 0.96 | `t` (0.96pt/1.28px, n=386: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=64: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=12: "BIR Form No. 1801; Additional sheet (1 of 2)") | `t`=beside, `<td>`=no, `<div>`=no |
| `2000-dst-2018` | 6.00 | `t` (6.00pt/8.00px, n=238: "Republic of the Philippines; BCS/") | `t`=beside |
| `2000-ot-2018` | 5.04 | `t` (5.04pt/6.72px, n=244: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=52: "000-123-453-00001; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=12: "BIR Form No. 2000-OT; Additional sheet (1 of 2)") | `t`=beside, `<td>`=no, `<div>`=no |
| `2200a-2020` | 3.96 | `t` (3.96pt/5.28px, n=383: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=24: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 2200A; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `2200an-2018` | 5.04 | `t` (5.04pt/6.72px, n=330: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=381: "123-123-123-00001; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=42: "BIR Form No. 2200AN; Additional sheet (1 of 7)") | `t`=beside, `<td>`=no, `<div>`=no |
| `2200c-2018` | 5.04 | `t` (5.04pt/6.72px, n=219: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=231: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=30: "BIR Form No. 2200C; Additional sheet (1 of 5)") | `t`=beside, `<td>`=no, `<div>`=no |
| `2200m-2018` | 5.04 | `t` (5.04pt/6.72px, n=310: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=42: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=12: "BIR Form No. 2200M; Additional sheet (1 of 2)") | `t`=beside, `<td>`=no, `<div>`=no |
| `2200p-2020` | 3.96 | `t` (3.96pt/5.28px, n=351: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=441: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=48: "BIR Form No. 2200P; Additional sheet (1 of 8)") | `t`=beside, `<td>`=no, `<div>`=no |
| `2200s-2018` | 6.00 | `t` (6.00pt/8.00px, n=279: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=18: "123-123-123-00001; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 2200S; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `2200t-2022` | 2.04 | `t` (2.04pt/2.72px, n=446: ".; Republic of the Philippines") · `<td>` (7.50pt/10.00px, n=246: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=30: "BIR Form No. 2200T; Additional sheet (1 of 5)") | `t`=beside, `<td>`=no, `<div>`=no |
| `2550m-2007` | 5.52 | `t` (5.52pt/7.36px, n=398: "(To be filled up by the BIR); DLN:") · `<td>` (7.50pt/10.00px, n=174: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=30: "BIR Form No. 2550M; Additional sheet (1 of 5)") | `t`=beside, `<td>`=no, `<div>`=no |
| `2550q-2024` | 6.00 | `t` (6.00pt/8.00px, n=403: "Republic of the Philippines; Department of Finance") · `<td>` (7.50pt/10.00px, n=49: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=12: "BIR Form No. 2550Q; Additional sheet (1 of 2)") | `t`=beside, `<td>`=no, `<div>`=no |
| `2551q-2018` | 6.96 | `t` (6.96pt/9.28px, n=216: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=9: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 2551Q; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `2552-2018` | 0.96 | `t` (0.96pt/1.28px, n=295: "Republic of the Philippines; For BIR") · `<td>` (7.50pt/10.00px, n=18: "—; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 2552; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
| `2553-1999` | 6.96 | `t` (6.96pt/9.28px, n=177: "(To be filled up by the BIR); DLN:") · `<td>` (7.50pt/10.00px, n=9: "123-123-123-00001; SANTOS, JUANA MARIE DELA CRUZ-VI") · `<div>` (7.50pt/10.00px, n=6: "BIR Form No. 2553; Additional sheet") | `t`=beside, `<td>`=no, `<div>`=no |
