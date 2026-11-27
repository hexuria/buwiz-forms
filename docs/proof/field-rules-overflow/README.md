# Overflow golden: what moved between bb066224 and the TIN/ATC change (#355)

`crates/bir-print/src/fixtures/overflow-goldens.json` pins, per sheet, the
SHA-256 of the documents `long_text_prints_as_pinned_on_every_sheet` prints
when every text join of the sheet is filled with 200, 90 and 12 characters of
words. It moved on 41 of the 43 sheets when TINs and ATCs became identifiers
that are never shortened. This folder shows what moved on three of them:
1601C (`1601c-2018`), 2551Q (`2551q-2018`) and 2200P (`2200p-2020`).

- `before/<sheet>.html.gz`: the probe's three documents at bb066224.
- `after/<sheet>.html.gz`: the same at the TIN/ATC change, the head this
  folder is committed with.
- `<sheet>.diff`: `diff -u before after`.

The documents are written by the ignored test
`frozen_html::tests::write_overflow_probe_documents`
(`OVERFLOW_PROBE_OUT=<dir> OVERFLOW_PROBE_SHEETS=1601c-2018,2551q-2018,2200p-2020
cargo test -p bir-print --lib -- --ignored write_overflow_probe_documents`).
For bb066224 the same test was added to that commit's bir-print in a scratch
checkout. Each document is one tag per line, with the inlined fonts and images
written as `data:(inlined)`. They are gzipped because each is about 2 MB of
sheet; the diffs are plain text.

## What the diffs show

Only identifier cells moved. Before, each was cut ("..." or a forced second
line, `text-truncated`, the whole only on the additional sheet). After, each
prints whole on one line, shrunk to its cell with no floor, and, being below
5 pt, is marked `identifier-small` so the additional sheet carries it whole
and the preview warns:

| Sheet | Cells | What they are |
|---|---|---|
| 1601C | p2c5 (200 and 90 characters) | page 2 TIN header (`derived:tin_digits`) |
| 2551Q | p2c5; p2c14, p2c25, p2c36, p2c47, p2c58, p2c69 (200 and 90) | page 2 TIN header; Schedule 1 ATC, rows 1-6 (`derived:sched1_atc1..6`) |
| 2200P | p1c16, p1c18, p1c20, p1c22 (200, 90; p1c16-p1c20 also 12); p2c376, p2c390 (200, 90, 12) | page 1 TIN parts and branch code (`txtPg1TIN1`-`txtPg1TIN4`); Schedule 1 Others ATCs (`txtSched1ATC26`, `txtSched1ATC27`) |

At 12 characters, 2200P's p1c16-p1c20 TIN parts were split over two lines at
7.51 pt (`UNIT 1203` / `TW`); they now print `UNIT 1203 TW` on one line at
6.1 pt, unmarked because that is above 5 pt.

Nothing else moved: no long free text, no amount, and no comb printed
differently. (The probe fills only writer-cells text joins, so there is no
stamped comb in it whose value is longer than its boxes.)
