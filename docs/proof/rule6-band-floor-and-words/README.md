# Rule 6 band floor and whole-word truncation

Renders from the print engine (`bir_print::frozen_html::filled_document`, no app
driving): the official sample payload of each form (dummy data) plus the
200-character words fill in the band's own cell, Chrome `print-to-pdf` of the
print HTML rasterized at 3x. `before-*` is the behaviour at 3c57ed74 (#355 as
merged), `after-*` this follow-up.

## Whole-word truncation

- `before-1701-item9.png`: the narrowed band's second line ends mid-word
  ("...TAGUIGME"), a fragment reading as a word of its own.
- `after-1701-item9.png`: the same line ends on a whole word, then the
  ellipsis ("...BONIFACIO..."); the whole value prints in the additional
  sheet as before.
- `before/after-1701ms-other-credits.png`, `before/after-2000ot-sched2-rowC.png`:
  the same pair for the 1701MS other-credits band and the 2000-OT Schedule 2
  row C band.

## The floor

No render shows the floor: no real band comes near it (the narrowest today is
311.6 pt of the 120 pt floor), so nothing moves. The floor is pinned by the
assertion over every sheet's bands in `every_overflow_line_lies_in_its_drawn_cell`
(no band narrower than 120 pt) and by the unit test
`a_value_cell_just_right_of_the_band_cell_moves_the_band_down_not_shrinks`.
