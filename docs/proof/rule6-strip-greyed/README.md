# Rule 6 strips grey out the added rows

The strip a Rule 6 band inserts is spacing, not entry room: rows added
to push the sheet down carry no boxes and nothing is rendered or entered
there. Printing it white read as a tall empty amount box; the owner's
markup asks for the added rows greyed out, like the sheet's own labels.

The strip now fills with the sheet's own label grey (`#d9d9d9`, the fill
the frozen sheets already use for caption cells) from the band's right
edge to the mirrored right margin. A full-width band leaves no room to
the right, so nothing is greyed there. The vertical frame lines the
strip crosses keep running through it (rule6-strip-keeps-lines).

* `before-1701ms-item9-strip.png` — the strip branch before greying
  (local `323271ba`): blank white strip right of the band.
* `after-1701ms-item9-strip.png` — this commit: the strip reads as
  spacing.

Both through the app's own print path
(`staticize(prepare_html_for(slug, filled_document(..), true))`), same
fill: the specify joins `words_of(200)`, the amount joins
**left-padded to their comb width exactly as the real writer pads them**
(`print_right_aligned` in `form_1701ms.rs`) — so the amounts print
right-aligned in their combs as they do for a real filer. The earlier
rule6-strip-keeps-lines renders filled plain unpadded `1000`, which the
writer never emits; that made the amounts look left-aligned. They were
a probe artifact, not a print fault.

sha256:

```
0fa92fd2e37007f78682a0c05b3417ec75650a685e4f20ad38ea6fa3edc46e89  before-1701ms-item9-strip.png
56a173bf9f9b9a3b66b719a4515097ed9a98e2b4f8c7203848486203fc728bb0  after-1701ms-item9-strip.png
```

Goldens: `overflow-goldens.json` moves on the five sheets whose probe
bands leave room to the right — 1700-2018, 1701-2018, 1701ms-2024,
1701q-2018, 2000-ot-2018; every other sheet's hash is unchanged.
