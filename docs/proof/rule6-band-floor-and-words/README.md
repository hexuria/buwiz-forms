# Rule 6 band floor and whole-word truncation — before/after renders

Print-engine renders (no app driving) of the three sheets whose Rule 6 bands
narrow under the placement rule, rendered **through the app's own print
path** — `staticize(prepare_html_for(slug, html, true))`, which turns each
filled input into a static span, so the band's second line prints as a second
line — then Chrome headless `--print-to-pdf` and a 3x render cropped at the
band row. The fill is the natural `words_of(200)` (the golden fills' own
12 words cycled to 200 chars) in the band's box, nothing else.

- **before-*.png** — rendered from main `0265dc61` (without the whole-word
  truncation). The truncated second line ends mid-word: "…BUILDING BONIFACIO
  GLOB..." — the fragment GLOB prints as if it were a word of its own.
- **after-*.png** — rendered from this branch. The same fill ends on the
  whole word, then "…": "…BUILDING BONIFACIO...", the whole value carried to
  the additional sheet.

1701 item 9 and 1701MS other credits show the change side by side. The
2000-OT Schedule 2 row C pair is byte-identical: on that wider band the
natural fill's cut lands exactly on a word boundary even without the fix
(the same reason its golden never moved), so there is nothing to see. The
truncation itself is asserted by
`a_truncated_rule6_band_ends_on_a_whole_word` in
`crates/bir-print/src/frozen_html.rs`, which fails on the pre-fix code path
with exactly "…BONIFACIO GLOB...".
