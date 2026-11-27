# Rule 6 strips keep the frame lines they cross

The strip the print path inserts under a row for a Rule 6 band opens new
vertical space at a row boundary. The sheet's frame lines (column
dividers, table edges) are drawn in per-row segments that exactly abut at
that boundary: one segment ends there, the next row's segment starts
there. Before the fix the insertion pushed the lower rows down whole, so
every one of those lines broke at the strip; to the right of a narrowed
band the strip printed as blank white inside the table grid.

The official sheet prints continuous frame lines through that area
(`official-1701ms-same-area.png`: empty fill — the sheet as it is drawn,
same crop): the amount columns' dividers and the rows' comb ticks run
down without interruption. So the gap is a print fault.

The fix: a vertical line segment that ends exactly at the insertion cut
and has a continuation segment starting exactly at the cut at the same x
(a continuous frame line) is stretched through the strip. Comb ticks end
inside their own row and abut nothing below the boundary, so they are
never extended into the strip.

* `before-1701ms-item9-strip.png` — main `97a96fb7` (pre-fix): 1701MS
  page 2, the strip under item 9 (`p2c179`, band narrowed for the
  amount cells of the same row) is blank white to the right of the band;
  the amount-column dividers stop at its top.
* `after-1701ms-item9-strip.png` — this branch: the dividers continue
  through the strip, the comb rows above and below keep their own ticks,
  nothing else moves.

Renders through the app's own print path
(`staticize(prepare_html_for(slug, filled_document(..), true))`), same
fill in both: every join `1000`, the specify/description joins
`words_of(200)`, cropped identically from the Chrome-printed PDF of the
prepared page.

sha256:

```
7669139576a7b67dc8e9912f5935c2f4a52728482cac902e4553bece1d8b974e  before-1701ms-item9-strip.png
f16331500787162c51ef18a46b506ac44e72155b05137f2fd22a8c89cfadee6e  after-1701ms-item9-strip.png
d244d92e66feb13d4053b9736df9d486c09a5e289c0ee8323582df0a3a0a5d8d  official-1701ms-same-area.png
```

Golden: `overflow-goldens.json` moves on `1701ms-2024` only — the one
sheet whose long-text probe inserts a narrowed band over abutting frame
lines; every other sheet's hash is unchanged.
