# Rule 6 bands clear the row's value cells

Before/after renders from the print engine (`bir_print::frozen_html::filled_document`,
no app driving) for the owner's Rule 6 placement rule: a band runs from its cell's
left to just before the first other value cell its strip crosses (2 pt gap), and
moves below any value cell it would still cross. An amount may never be covered.

The fill is the official sample payload of each form (`bir-core/tests/sample-payloads`,
dummy data, no real taxpayer) plus one long free-text entry in the band's own cell —
the 200-character words fill the band tests use. The official samples alone trigger
no band (their values are short). Renders are Chrome `print-to-pdf` of the print
HTML, rasterized at 3x (row crops) / 2x (additional sheet page).

## 1701-2018, item 9 "Other Tax Credits/Payments (specify)" — cell p4c213, page 4

- `before-1701-item9.png`: the band ran the full sheet width (583 pt) and its
  white face covered the lower half of the row's amount combs p4c90/p4c91,
  box rules included.
- `after-1701-item9.png`: the band ends 2 pt before comb p4c90 (width 308.62 pt);
  the combs print whole, the amounts stay on the item's own line.
- `additional-sheet-1701-item9.png`: the long entry no longer fits the narrowed
  band, so it follows the print order (two lines, smaller type down to the floor,
  then an ellipsis) and the whole value prints in the additional sheet.

## 1701ms-2024, "other credits" description — cell p2c179, page 2

- `before-1701ms-other-credits.png`: the full-width band reached 0.25 pt into
  combs p2c138/p2c139 (the hairline).
- `after-1701ms-other-credits.png`: the band ends 2 pt before comb p2c138
  (width 308.70 pt); no contact with any value cell.
- `additional-sheet-1701ms-other-credits.png`: the whole long entry in the
  additional sheet.

## 2000-ot-2018, Schedule 2 row C "Others (Specify)" — cell p2c99, page 2

- `before-2000ot-sched2-rowC.png`: the full-width band (559 pt) crossed the
  row's combs p2c96/p2c98.
- `after-2000ot-sched2-rowC.png`: the band ends 2 pt before comb p2c96
  (width 388.31 pt); the combs print whole.
- `additional-sheet-2000ot-sched2-rowC.png`: the whole long entry in the
  additional sheet.
