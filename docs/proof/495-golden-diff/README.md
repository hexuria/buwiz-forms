# #495 golden diff: 6e848530 (the main merge) against the fixed head

OVERFLOW_PROBE_SHEETS dumps of every sheet whose golden moved between the
main merge `6e848530` and the fixed head, rendered by the ignored
`write_overflow_probe_documents` test (every text join filled with
200/90/12 words), one unified diff per sheet in `diffs/`. Counts of changed
lines come from these diffs, not memory.

Every changed line in every sheet is one of exactly two kinds — nothing
else changed:

- **a cut moved to a word boundary**: the printed prefix of a truncated
  value ends at a space instead of mid-word (old `...CURVE B...` → new
  `...CURVE...`). This is the whole-word truncation reaching the two-line
  path (tall plain-text cells) after the in-cell and shallow paths.
- **a strip edge**: on 1706 the strip left edge moved 0.48 pt onto the
  frame divider (20.16 → 20.64) and on 2000-OT the strip right edge
  widened 0.36 pt onto the divider; 1701MS widened 0.77 pt the same way —
  the three small jogs F9 exists to remove. Every other strip edge on every
  sheet is byte-identical to the pre-snap positions: the edges the broken
  anywhere-on-page snap had moved are back, and exact matches never move.

| sheet | changed chunks | what changed |
|---|---|---|
| 1600-pt-2018 | 37 | cuts to word boundaries |
| 1600-vt-2018 | 49 | cuts to word boundaries |
| 1600wp-2010 | 67 | cuts to word boundaries |
| 1601-fq-2020 | 77 | cuts to word boundaries |
| 1601c-2018 | 36 | cuts to word boundaries |
| 1601eq-2019 | 35 | cuts to word boundaries |
| 1602q-2019 | 24 | cuts to word boundaries |
| 1604c-2018 | 161 | cuts to word boundaries |
| 1604e-2018 | 146 | cuts to word boundaries |
| 1606-2018 | 38 | cuts to word boundaries |
| 1700-2018 | 86 | cuts to word boundaries |
| 1701-2018 | 110 | cuts to word boundaries |
| 1701a-2018 | 52 | cuts to word boundaries |
| 1701ms-2024 | 265 | cuts to word boundaries; strip edge snapped onto the frame divider |
| 1701q-2018 | 19 | cuts to word boundaries |
| 1702ex-2018 | 105 | cuts to word boundaries |
| 1702mx-2018c | 769 | cuts to word boundaries |
| 1702rt-2018c | 55 | cuts to word boundaries |
| 1706-2018 | 47 | cuts to word boundaries; strip edge snapped onto the frame divider |
| 1707-2021 | 31 | cuts to word boundaries |
| 1800-2018 | 155 | cuts to word boundaries |
| 1801-2018 | 228 | cuts to word boundaries |
| 2000-ot-2018 | 140 | cuts to word boundaries; strip edge snapped onto the frame divider |
| 2200a-2020 | 447 | cuts to word boundaries |
| 2200an-2018 | 311 | cuts to word boundaries |
| 2200c-2018 | 253 | cuts to word boundaries |
| 2200m-2018 | 178 | cuts to word boundaries |
| 2200p-2020 | 420 | cuts to word boundaries |
| 2200s-2018 | 79 | cuts to word boundaries |
| 2200t-2020 | 424 | cuts to word boundaries |
| 2550m-2007 | 448 | cuts to word boundaries |
| 2550q-2024 | 58 | cuts to word boundaries |
| 2551q-2018 | 13 | cuts to word boundaries |
| 2552-2018 | 214 | cuts to word boundaries |
| 2553-1999 | 54 | cuts to word boundaries |
| 0605-1999 | 30 | cuts to word boundaries |
| 0619e-2018 | 30 | cuts to word boundaries |
| 0619f-2018 | 30 | cuts to word boundaries |
| 1603q-2018 | 18 | cuts to word boundaries |
| 1604f-2018 | 37 | cuts to word boundaries |
| 1702q-2018 | 124 | cuts to word boundaries |
| 1707a-2021 | 75 | cuts to word boundaries |
| 2000-dst-2018 | 44 | cuts to word boundaries |

Note: the 8 sheets whose goldens did not move between `6e848530` and
`eeb28629` (0605, 0619E, 0619F, 1603Q, 1604F, 1702Q, 1707A, 2000-DST) DO
appear here: their tall plain-text cells truncate through the two-line
path, which round 4 had not yet fixed — their diffs are word-boundary cuts
only, with zero geometry lines.
