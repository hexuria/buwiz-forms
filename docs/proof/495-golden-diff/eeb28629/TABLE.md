# #495 overflow-golden diff: `6e848530` → `eeb28629`

Independent OVERFLOW_PROBE_SHEETS dump. Base is `6e848530364f3b6b2309733c01c4a133fef15847`; head is `eeb2862914e21701fe96da68e3f7d00e81267892`. The probe fill is the repository's own `write_overflow_probe_documents` ignored test (200/90/12 characters in every text join). Raw dumps are in `base/` and `head/`; one zero-context unified diff per sheet is in `diffs/`.

Golden count independently re-derived: 35 changed, 8 unchanged (`0605-1999`, `0619e-2018`, `0619f-2018`, `1603q-2018`, `1604f-2018`, `1702q-2018`, `1707a-2021`, `2000-dst-2018`). Only those 35 were dumped.

Classification coverage: 4604 input differences, every one a truncation cut moving from mid-word to a whole-word boundary; strip geometry differences listed below; zero strips added or removed; zero other differences. The geometry moves are all the round-4 anywhere-on-page snap behavior and are therefore labelled `strip edge (known snap bug)`.

| sheet | truncation cuts moved to a word boundary | strip edges (known snap bug) | strips added/removed | other |
|---|---:|---|---|---|
| `1600-pt-2018` | 30 | — | none | none |
| `1600-vt-2018` | 42 | — | none | none |
| `1600wp-2010` | 52 | — | none | none |
| `1601-fq-2020` | 47 | — | none | none |
| `1601c-2018` | 19 | — | none | none |
| `1601eq-2019` | 12 | — | none | none |
| `1602q-2019` | 6 | — | none | none |
| `1604c-2018` | 152 | — | none | none |
| `1604e-2018` | 128 | — | none | none |
| `1606-2018` | 34 | left 26.28→26.64 (+0.36 pt): p2c129 | none | none |
| `1700-2018` | 5 | right 597.84→596.76 (-1.08 pt): p2c151, p2c153, p2c155, p2c157 | none | none |
| `1701-2018` | 105 | right 599.64→598.68 (-0.96 pt): p3c16 | none | none |
| `1701a-2018` | 40 | right 599.16→598.32 (-0.84 pt): p2c176, p2c29, p2c34, p2c62, p2c67 | none | none |
| `1701ms-2024` | 220 | right 599.03→598.30 (-0.73 pt): p2c179 | none | none |
| `1701q-2018` | 10 | right 597.72→596.76 (-0.96 pt): p2c148, p2c150, p2c152 | none | none |
| `1702ex-2018` | 20 | — | none | none |
| `1702mx-2018c` | 592 | — | none | none |
| `1702rt-2018c` | 4 | — | none | none |
| `1706-2018` | 38 | left 20.16→20.04 (-0.12 pt): p2c150; right 593.04→592.54 (-0.50 pt): p2c150 | none | none |
| `1707-2021` | 4 | — | none | none |
| `1800-2018` | 136 | — | none | none |
| `1801-2018` | 204 | — | none | none |
| `2000-ot-2018` | 104 | right 587.26→586.18 (-1.08 pt): p2c99 | none | none |
| `2200a-2020` | 441 | — | none | none |
| `2200an-2018` | 291 | — | none | none |
| `2200c-2018` | 239 | — | none | none |
| `2200m-2018` | 40 | — | none | none |
| `2200p-2020` | 410 | — | none | none |
| `2200s-2018` | 66 | — | none | none |
| `2200t-2020` | 404 | — | none | none |
| `2550m-2007` | 448 | — | none | none |
| `2550q-2024` | 51 | right 590.60→589.70 (-0.90 pt): p2c246, p2c248, p2c250 | none | none |
| `2551q-2018` | 0 | left 22.32→23.04 (+0.72 pt): p1c187 | none | none |
| `2552-2018` | 206 | — | none | none |
| `2553-1999` | 4 | — | none | none |
