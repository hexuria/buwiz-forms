# Value cells off their drawn boxes on other sheets: none visible

`value_cells_on_drawn_boxes` (added with the 2200T January 2020 sheet) first
listed 29 value cells on 10 sheets as off their boxes. Each was measured and
checked for whether a filer would see a value outside or across its printed
box. None is visible, so no cell moves here. The test learned two things the
sheets do:

- Hand-corrected TIN rows draw their boxes in the cells themselves: the cell
  has `background:#fff;box-shadow:inset 0 0 0 0.72pt #000` (the grey dash
  cells between the groups likewise), and each slot draws its tick as a
  gradient at its right. The SVG layer still holds the PDF's original row,
  4 to 15 pt away. What prints, and what the filer sees, is the box the cell
  draws, with the digits inside it. The test now counts those boxes and
  ticks as drawing.
- A value cell is a cell something prints into, whatever its
  `data-cell-kind` (`field`, `mixed` caption-and-boxes, even `label`): a cell
  a writer key is joined to (`bir_print::frozen_html::joined_cell_ids`, which
  includes the engine's own joins), or one holding an input stamped with a
  filing key, which the print fills by name. A cell with no input, or whose
  inputs nothing fills, prints nothing and is skipped. So the cells below
  marked "no input" or "nothing fills it" drop out by rule, not by a list.
  The rule checks 6,828 cells on the 43 sheets.

Under that rule one cell is a little off and stays listed (`KNOWN`): 1707
`p1c217`, 8A If yes, please specify (`frm1707:txtPg1I8TaxReliefSpec`). Its
top is 2.6 pt below the line under the caption, inside the white box. A
25-character value and a 58-character one both print fully inside the box
(`1707-item8a-specify.png`, the cell outlined in red).

Offsets are how far each edge is from the nearest drawn SVG edge beyond the
test's 2 pt tolerance, in points (blank: on it). "Own box" means the cell
draws its own box and slot ticks.

| Sheet | Cell | What it is | Top | Bottom | Left | Right | Visible | Why |
|---|---|---|---|---|---|---|---|---|
| 0605 | p1c23 | TIN 1-3 (frm0605:txtTIN1) | | | | 4.4 | no | own box; digits print inside it (`0605-tin-row.png`, `0605-tin-row-cells-outlined.png`) |
| 0605 | p1c25 | TIN 4-6 | | | 4.4 | 10.6 | no | own box |
| 0605 | p1c27 | TIN 7-9 | | | 1.3 | 15.2 | no | own box |
| 0605 | p1c29 | TIN branch | | | 4.9 | | no | own box |
| 1600WP | p1c10 | TIN 1-3 (txtTIN1) | | | | 2.2 | no | own box (`1600wp-tin-row.png`) |
| 1600WP | p1c12 | TIN 4-6 | | | 2.2 | 4.2 | no | own box |
| 1600WP | p1c14 | TIN 7-9 | | | 4.2 | 6.0 | no | own box |
| 1600WP | p1c16 | TIN branch | | | 6.0 | | no | own box |
| 1600WP | p1c70 | withholding agent TIN 4-6 (derived:att_tin2) | | | 1.9 | 4.5 | no | own box (`1600wp-attachment-tin-row.png`) |
| 1600WP | p1c72 | withholding agent TIN 7-9 | | | 4.5 | 6.4 | no | own box |
| 1604F | p1c160 | text strip 6.44 pt tall | 4.5 | 0.5 | | | no | nothing fills it (no join, no name stamp); skipped by rule |
| 1701A | p1c227 | strip 6.89 pt tall | 3.6 | 0.5 | | | no | no input; prints nothing |
| 1701A | p2c208 | strip 7.43 pt tall | 3.6 | 1.0 | | | no | no input; prints nothing |
| 1701MS | p1c51 | sliver 1.27 × 10.05 pt | 1.8 | 11.8 | | | no | nothing fills it; skipped by rule |
| 1701MS | p1c286 | text cell 19.54 pt tall | 9.3 | | | | no | nothing fills it; skipped by rule |
| 1706 | p2c183 | Schedule 3 Taxable Base line | 6.6 | 0.6 | | | no | not_joined_cells (no layout key); skipped by rule |
| 1800 | p1c187 | strip 7.16 pt tall | 6.4 | | | | no | no input; prints nothing |
| 2550M | p1c127 | TIN 1-3 (frm2550m:txtTIN1, Rust-side join) | | 0.4 | | 5.0 | no | own box (`2550m-tin-row.png`) |
| 2550M | p1c9 | TIN 4-6 | | | 0.2 | 9.6 | no | own box |
| 2550M | p1c11 | TIN 7-9 | | | 5.1 | 14.3 | no | own box |
| 2550M | p1c13 | TIN branch | | | 9.5 | | no | own box |
| 2551Q | p1c170 | strip 0.88 pt tall | 0.2 | | 0.3 | 5.0 | no | no input; prints nothing |
| 2553 | p1c18 | TIN 1-3 (txtTIN1) | | | | 4.9 | no | own box (`2553-tin-row.png`) |
| 2553 | p1c19 | dash between TIN groups | | | 4.9 | 0.1 | no | no input; own box |
| 2553 | p1c20 | TIN 4-6 | | | 0.1 | 9.4 | no | own box |
| 2553 | p1c21 | dash | | | 9.4 | 4.9 | no | no input; own box |
| 2553 | p1c22 | TIN 7-9 | | | 4.9 | 14.0 | no | own box |
| 2553 | p1c23 | dash | | | 14.0 | 9.1 | no | no input; own box |
| 2553 | p1c24 | TIN branch | | | 9.1 | | no | own box |

The crops are the sheets filled with TIN 123-456-789-00001 by
`bir_print::frozen_html::filled_document`, printed by headless Chrome and
rasterised at 200 dpi (the 0605 outline at 400 dpi, the cells drawn in red
over the printed row).
