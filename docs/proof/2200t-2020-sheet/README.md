# 2200T on its January 2020 sheet: value cells on their drawn boxes

2200T prints on its own January 2020 sheet (`html-frozen/2200t-2020`)
instead of the August 2022 sheet. Many of the sheet's value cells were placed
by hand, and 27 of them sat off the boxes the sheet draws, by 1 to 15 pt
(item 14's box by 65 pt across): on the Part I banner, on captions, in blank
paper. This folder shows them before and
after they were moved onto their boxes.

All positions are in points from the page's top left corner, as the sheet's
`style` gives them.

## How each cell's box was found

The drawn box is the one the sheet's SVG layer outlines: black rules, and the
grey and white fills whose edges form boxes and comb slots. Every moved cell
took the geometry of the cell the sheet's own lattice generated on that box
(unjoined, so it prints nothing), or the white box itself where there is none:

- check boxes (items 2, 12, 13, 14, 15): the generated box cell on the white,
  ruled square (`p1c12`, `p1c13`, `p1c48`, `p1c49`, `p1c52`, `p1c53`); item
  15's square has no generated cell, so its white rect (41.52, 388.68,
  13.32×11.4);
- combs: the generated comb's slots. Items 10 and 11 and the page 2 and 3
  headers are one generated comb split at the drawn dividers: x 88.46 and
  363.43 (Region 5, Province 19, City 16 boxes), and x 218.81 (TIN 14, name
  26);
- item 28's row has no generated cell beside item 28 (specify): it moves down
  to the row's drawn box, 834.84 to 848.76. That is the top of `p1c112` and
  of the white boxes at 835.08. Its old top, 823.5, is the caption line.

| Cell | Item | Before | After | Slots | From |
|---|---|---|---|---|---|
| `p1c1010` | 28 Others, Drawee Bank/Agency | (118.34, 823.5) 72.6×15.12 | (118.34, 834.84) 72.6×13.92 |  | row 28 drawn box (p1c112's top) |
| `p1c1011` | 28 Others, Number | (190.94, 823.5) 86.91×15.12 | (190.94, 834.84) 86.91×13.92 |  | row 28 drawn box (p1c112's top) |
| `p1c1012` | 28 Others, Date MM | (277.85, 823.5) 29.04×15.12 | (277.85, 834.84) 29.04×13.92 |  | row 28 drawn box (p1c112's top) |
| `p1c1013` | 28 Others, Date DD | (306.89, 823.5) 28.7×15.12 | (306.89, 834.84) 28.7×13.92 |  | row 28 drawn box (p1c112's top) |
| `p1c1014` | 28 Others, Date YYYY | (335.59, 823.5) 57.72×15.12 | (335.59, 834.84) 57.72×13.92 |  | row 28 drawn box (p1c112's top) |
| `p1c1015` | 28 Others, Amount (pesos) | (393.31, 823.5) 158.19×15.12 | (393.31, 834.84) 158.19×13.92 |  | row 28 drawn box (p1c112's top) |
| `p1c1016` | 28 Others, Amount (centavos) | (566.02, 823.5) 28.82×15.12 | (566.02, 834.84) 28.82×13.92 |  | row 28 drawn box (p1c112's top) |
| `p1c1020` | 28 Others (specify) | (18, 823.5) 100.34×15.12 | (16.32, 834.84) 101.3×13.92 | 7 → 7 | p1c112[0:7] |
| `p1c1023` | 10 Place of Production, Region | (18, 272.57) 85.94×15.12 | (16.32, 273.65) 72.38×15.96 | 6 → 5 | p1c39[0:5] |
| `p1c1024` | 10 Place of Production, Province | (103.94, 272.57) 274.97×15.12 | (88.7, 273.65) 274.97×15.96 | 19 → 19 | p1c39[5:24] |
| `p1c1025` | 10 Place of Production, City | (378.91, 272.57) 215.93×15.12 | (363.67, 273.65) 231.65×15.96 | 15 → 16 | p1c39[24:40] |
| `p1c1026` | 11 Place of Removal, Region | (18, 308.57) 85.94×15.12 | (16.32, 312.53) 72.38×15.96 | 6 → 5 | p1c44[0:5] |
| `p1c1027` | 11 Place of Removal, Province | (103.94, 308.57) 274.97×15.12 | (88.7, 312.53) 274.97×15.96 | 19 → 19 | p1c44[5:24] |
| `p1c1028` | 11 Place of Removal, City | (378.91, 308.57) 215.93×15.12 | (363.67, 312.53) 231.65×15.96 | 15 → 16 | p1c44[24:40] |
| `p1c1029` | 15 Other Similar Schemes (specify) | (205.61, 376.13) 389.23×15.12 | (204.53, 386.21) 390.79×17.88 | 27 → 27 | p1c55[0:27] |
| `p1c1030` | 2 Amended Return? Yes | (353.16, 119.58) 13.44×11.46 | (350.04, 107.88) 13.58×11.46 |  | p1c12 |
| `p1c1031` | 2 Amended Return? No | (398.64, 119.58) 13.44×11.46 | (392.74, 107.88) 13.7×11.46 |  | p1c13 |
| `p1c1032` | 12 Tax relief Yes | (152.28, 334.2) 13.44×11.46 | (147.12, 340.98) 13.32×11.46 |  | p1c48 |
| `p1c1033` | 12 Tax relief No | (209.88, 334.2) 13.44×11.46 | (204.84, 340.98) 13.32×11.46 |  | p1c49 |
| `p1c1034` | 13 Payment on Actual Removal | (36.96, 362.52) 13.44×11.46 | (41.4, 373.2) 13.32×11.52 |  | p1c52 |
| `p1c1035` | 14 Prepayment/Advance Deposit | (224.4, 362.52) 13.44×11.46 | (290.16, 373.56) 13.44×11.52 |  | p1c53 |
| `p1c1036` | 15 Other Similar Schemes | (36.96, 377.28) 13.44×11.46 | (41.52, 388.68) 13.32×11.4 |  | knockout box 41.52,388.68 |
| `p1c1037` | 12A If yes, specify | (263.45, 333.65) 331.39×15.24 | (247.97, 339.89) 347.35×17.76 | 23 → 24 | p1c47[0:24] |
| `p2c950` | Page 2 TIN | (18, 90.98) 202.01×15.12 | (16.32, 99.38) 202.73×17.76 | 14 → 14 | p2c5[0:14] |
| `p2c951` | Page 2 Taxpayer's Name | (220.01, 90.98) 374.83×15.12 | (219.05, 99.38) 376.27×17.76 | 26 → 26 | p2c5[14:40] |
| `p3c950` | Page 3 TIN | (18, 92.06) 202.01×15.12 | (16.32, 106.22) 202.73×17.76 | 14 → 14 | p3c5[0:14] |
| `p3c951` | Page 3 Taxpayer's Name | (220.01, 92.06) 374.83×15.12 | (219.05, 106.22) 376.27×17.76 | 26 → 26 | p3c5[14:40] |

Item 10's and 11's Region comb had 6 slots and City 15 where the sheet draws 5
and 16; 12A had 23 where it draws 24. The Province comb started one box right of the Province box (103.94
instead of 88.7).

## The test

`crates/bir-print/tests/value_cells_on_drawn_boxes.rs`,
`every_value_cell_lies_on_its_drawn_box`, checks every value cell of every
frozen sheet. Each of its four edges must lie within 2 pt of a drawn edge
along at least half of that side. For a comb, at least 60% of its slot
boundaries must lie on a drawn tick. On the sheet before these moves it
fails with these 27 cells, all on 2200T. After, it passes.

It also fails on 29 cells of other sheets, which it lists by name (`KNOWN`)
for later fixes: the MM/DD/YYYY combs of 0605 (p1c23-29), 1600WP (p1c10-16,
p1c70-72), 2550M (p1c9-13, p1c127) and 2553 (p1c18-24) drift 4 to 15 pt past
their drawn dividers; thin strips on 1604F (p1c160), 1701A (p1c227, p2c208),
1701MS (p1c51, p1c286), 1706 (p2c183), 1800 (p1c187) and 2551Q (p1c170) sit
3 to 9 pt off the line above or below them. The test fails if any other cell
is off its box, or if a listed one is fixed and still listed.

## Files

- `before-page-1.png` to `before-page-3.png`, `before.pdf`: the sheet before
  the moves.
- `after-page-1.png` to `after-page-3.png`, `after.pdf`: after.
  Both print one invented return: every check box ticked (items 2, 12, 13, 14
  and 15, Yes and No alike), the page 2 and 3 TIN (123-456-789-00001) and
  Taxpayer's Name, items 10, 11, 12A and 15 (specify), and item 28's row.
- `page1-items-2-to-15-before-after.png`, `page1-item-28-before-after.png`,
  `headers-before-after.png`: before (top) and after (bottom) crops.

They were made by `bir_print::frozen_html::filled_document("2200t-2020", …)`,
printed to PDF by headless Chrome (612×936 pt pages) and rasterised by
`pdftoppm -r 110`.

## Why the January 2020 sheet

| Date | Source | Effect |
|---|---|---|
| 2021-03-02 | RMC 34-2021 | Prescribed manual 2200-T January 2020 (ENCS); electronic filers continued the existing Offline-package 2200-T until separately announced. |
| 2023-05-16 | RMC 54-2023 | Prescribed manual 2200-T August 2022 (ENCS); eFPS/eBIRForms users continued the package version until separately announced. |
| 2024-02-19 | RMC 26-2024 | Announced August 2022 2200-T in eFPS for mandated eFPS taxpayers. |
| 2026 package inspection | Offline eBIRForms v7.9.6.2.1 | Carries `BIR-Form2200Tv2020.hta` and `2200Tv2020` page assets, no `2200Tv2022` resource. |

Boundary: this product reproduces the Offline eBIRForms channel, so it prints the January 2020 sheet. The August 2022 sheet belongs to the manual/eFPS revision and would need its own rules and filing channel; that is deliberately out of scope here.

The January 2020 PDF provenance URL is the historic BIR URL recorded in `html-frozen/2200t-2020/provenance.json`; the migrated BIR site now returns 404 there and the bytes came from the Internet Archive capture of 2022-10-23. Its SHA-256 is `6c7ff78db4348d57139d417995832294e489cf869d807e4c254f3a00513cce4a`. The extracted package manifest and resources were decoded far enough to identify `2200Tv2020_p1..p3.png`. They are not full page images: they are small barcode strips labeled `2200-T 01/20ENCS P1/P2/P3` (SHA-256 `eb615f9a…88fe3`, `ea598ced…34534`, `16394d13…264e7e`; copies in `swap/package-2200Tv2020-p*-barcode.png`). They therefore confirm the package prints the January 2020 revision and page labels, but there are no package full-page raster images to pixel-compare with the official PDF; the sheet comparison is against the official PDF itself.

### The sheet swap's own proof (`swap/`)

- `swap/official-page1.png` through `official-page3.png`: the January 2020 official PDF pages.
- `swap/before-page*.png` and `swap/before-main.pdf`: the same invented return printed by main on the August 2022 sheet. It contains 2022 and 2023 row amounts (400.00 and 300.00) and item 16/total 1000.00, but the 2020 and 2021 amounts (100.00 and 200.00) are absent because that sheet has no rows for them.
- `swap/after-page*.png` and `swap/after-fixed.pdf`: the same return printed by this branch on the January 2020 sheet. All four amounts print, and item 16/total is 1000.00 = 100 + 200 + 400 + 300.
- `swap/2200T-schedule1-1A-years-compare.png`: official, before, and after crops over Schedule 1 item 1A rows for 2020-2023.
- `swap/2200T-item16-total-compare.png`: official, before, and after crops at Part III item 16.
- `swap/2200T-schedule1-total-compare.png`: official, before, and after crops at the Schedule 1 total row.

These files prove row presence and the arithmetic of this print fixture. Payment-detail, date, checkbox, contact, treaty, and page-header cells are joined where the sheet draws them; several of those cells are direct frozen-layout corrections because the generated lattice merged or omitted them. They are outside the visual scope of this revision-fault proof.
