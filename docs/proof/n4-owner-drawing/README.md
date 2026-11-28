# n4-owner-drawing — the owner's strip drawing in #495, rendered at its final head

Evidence renders only; no product change on this branch. Rendered at #495's head
`fix/rule6-band-uses-own-field` after the owner decision of 2026-11-28 (adopt #510's
strip drawing, keep #495's page-room budget), through the app's own print path:
`staticize(prepare_html_for(slug, filled_document(..), true))` via the committed
`print_path_render` example, printed at the sheet's native 612x936 pt (one PDF point
is one sheet point, same method as docs/proof/r6-510-render).

## Fills (the same as #510's renders)

- 1701-2018:
  - `frm1701:txtPg2IShed3_19Desc=SPECIMEN OTHER NON-OPERATING INCOME LONG DESCRIPTION WRAPPING OVER SEVERAL LINES OF PRINT`
  - `frm1701:txtPg2IShed3_20Desc=SPECIMEN ITEM TWENTY LONG DESCRIPTION ALSO WRAPPING OVER SEVERAL LINES OF PRINT`
- 1601c-2018: `frm1601c:txt20Other=SPECIMEN LONG SPECIFY TEXT FOR OTHER MANNER OF PAYMENT WITH MANY WORDS TO EXERCISE WRAPPING`

## The pictures

- `n4a-page2-items15-22.png` — 1701 PDF page 2, crop x 15..580, y 694..836, zoom 2.5
  (the same crop as #510's r1a). Item 19 fills its own line, the continuation prints
  on the strip — a white text area closed by full-width rules, the amount columns
  greyed in one continuous span, the item-number divider stopping at the rules;
  item 20 truncates in-cell on a whole word with "..."; rows 21–25 follow unshifted.
- `n4b-page2-items15-to-bottom.png` — same x, y 694 to the page bottom. Nothing is
  clipped: the largest content bottom keeps the 4 pt reserve off the 936 edge
  (unlike #510, whose content bottom is 944.04 pt).
- `n4c-page2-whole.png` — the whole of PDF page 2 at zoom 1.5.
- `n4d-item19-510-vs-495.png` — top: #510's r1a at b70e42ef; bottom: this branch's
  crop at the same coordinates, red separator. The item 19 strip draws the same in
  both; the visible difference is the page budget: #510 prints item 20's strip too
  and pushes rows 21+ down off the paper, #495 truncates item 20 whole-word and the
  rows keep their positions.
- `n4r2-1601c-item20-band.png` — 1601C item 20's strip row (40 pt above the band
  div's top to 40 below its bottom, full page width, zoom 2.5; band top 415.27 pt on
  PDF page 1, the same y as #510's R2). The grey is continuous across the narrow
  decimal separator column between the pesos and centavos boxes — the 1601C/2551Q
  exception the owner asked for.
- `n4r2d-1601c-510-vs-495.png` — top: #510's r2; bottom: this branch's. In #510 the
  separator column stayed white inside the strip; in #495 the greyed span is
  continuous from the first amount column to the right border.

## BEFORE and AFTER

The `before-*.png` set renders main at `d91df05d4` with the same fills through the
same committed print path, cropped exactly like `n4a`/`n4b`/`n4c`/`n4r2`. At main the
cell line goes EMPTY and the whole value jumps to the strip line, the strip shows the
row's comb ticks, item 20 strips too, and rows 21–25 push down off the page. The
`n4*.png` set is the AFTER at the fixed head: the cell keeps its first line, the strip
is a clean white row with plain grey amount columns (no ticks — only the columns'
divider and right border), item 20 truncates whole-word, and the rows keep their
positions. Verified by eye against #510's r1a/r2: the strips draw the same.

## Numbers at this head

- band `data-rule6-of="p2c134"` (item 19): top 822.98 pt, height 11.00 pt; the strip
  row is 12 pt (one 11 pt line + 1).
- 1701 page 2's room is 13.4 pt: item 19's 12 pt strip fits with 1.4 pt to spare and
  the 4 pt bottom reserve is untouched; item 20's own strip does not fit after it,
  so item 20 truncates and its whole value goes to the additional sheet.
