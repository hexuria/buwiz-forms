# Rule 6: the sheet's own field keeps the first part; the added line is greyed

Renders through the app's own print path —
`staticize(prepare_html_for(slug, &filled_document(..), true))` — never the
raw frozen sheet. Same fill in every before/after pair: every join `1000`,
the specify/description/Others joins `words_of(200)`; the pair cropped at
identical coordinates from the Chrome-printed PDF of the prepared page.

Before is main `d910f47d`; after is this branch (final head).

Continuation-line cap: the rest of the value wraps over full-row-width
added lines (spanning the row, never crossing into the greyed amount
columns) over at most 6 added lines; only when it still does not
fit, the last line ends with a whole-word "…" and the WHOLE value prints on
the additional sheet (a test asserts no text is ever lost). In each pair:

- the original specify field holds the first part of the value (before: it
  was emptied, the whole value moved onto the inserted band);
- the added strip IS the row: one bare input per line inside the strip,
  no outer + inner container. The lines span the full row width, greyed,
  drawn under the sheet's frame and divider lines — those lines still
  cross the grey, present and on top, and no divider ever crosses the
  wrapped text, which ends before the first greyed amount column;
- the amount boxes and their comb ticks keep the sheet's original height —
  the grey strip has no ticks;
- no input in the grey area, no numbers rendered or entered there.


sha256:

```
e9bab289a3204aef2d1053d672e0a12ce2d6796ac83b0d5f6a7703dcbc8cdd14  before-1701ms-2024.png
bfb20e042b7997d7870049ca2dd3dc14a24de9032751ed53cb16042c871d5b40  after-1701ms-2024.png
a6c60b8c5c932d27945a5d13daffd32d6678eb00424ec09f60101ded9ce1103d  before-1701-2018.png
20c395f7b9e4ea8110a71230b0bb7fbfd6cd61e0de8844ff3e67b0eb4228c4be  after-1701-2018.png
130ba5a4bb07f35b5500110afda759d1a1460ab5df7b08871ce54956ceaa5196  before-2000-ot-2018.png
0baf33a82fa5dcc47e9b070fead7d15e340c2b3460440568ccfcaa335e6f23b2  after-2000-ot-2018.png
```
