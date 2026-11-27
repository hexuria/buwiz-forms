# Rule 6 band floor and whole-word truncation — before/after renders

Print-engine renders (no app driving) of the three sheets whose Rule 6 bands
narrow under the placement rule: the printable html of each form is filled
with a long entry in the band's own box, printed to PDF with Chrome headless
(`--print-to-pdf --no-pdf-header-footer`, `@page` 612x936 pt margin 0) and
rendered at 3x around the band row.

- **before-*.png** — rendered from commit `3c57ed74` (the PR #355 head:
  narrowing without the whole-word truncation). Each band's text runs to the
  clip edge and is cut **mid-word**: "…BONIFACIO AAAA" clipped inside the As.
- **after-*.png** — rendered from this branch's commit (band floor, one width
  for fit and drawing, whole-word truncation). The same fill ends on the
  **whole word** followed by "…" inside the band — "…BONIFACIO..." — with the
  whole value carried to the additional sheet.

## Fills

In print the band input renders its value as one clipped line, so the visible
window is the first line plus a few characters of wrap slack. The fills are
tuned to place the truncation backoff inside that slack: a padding word
positions the wrap so that the line-2 remainder is one short whole word plus
one long unbreakable word. Before the fix the cut lands mid-word inside the
long word; after the fix the long word is backed off whole and the band ends
on the whole word before it. Verbatim (python repr; \u2003 is an em-space):

```
1701 + 1701ms: 'UNIT 1203 TWELFTH FLOOR CURVE BUILDING PiPiPiPiPiPiPiPiPiPiPi\u2003\u2003BONIFACIO AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
2000ot:        'UNIT 1203 TWELFTH FLOOR CURVE BUILDING PiPiPiPiPiPiPiPiPiPiPiPiPiPiPiPiPiPiPiPiPiPiPi\u2003\u2003BONIFACIO AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
```

Every before/after pair differs (sha256 values in the handover report); the
truncation itself is asserted by `a_truncated_rule6_band_ends_on_a_whole_word`
in `crates/bir-print/src/frozen_html.rs`.
