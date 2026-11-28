# Print uniform amount sections (#477) — proof

`GROUPS.md` is the validation table: every amount-comb group found on each of
the 43 frozen sheets, generated from the same grouping the fill uses
(`cargo test -p bir-print uniform_amount_group_table -- --ignored` with
GROUP_TABLE_OUT pointing at the file). The grouping source is the sheet's own
section bands: the `<div class="t">` headings whose text is `Part`/`Schedule`,
a word boundary and a numeral; a group is one section's column, heading to
heading on that page, split further where a sub-table's column headers cross
the column, with hidden `<template>` rows never deciding geometry.
`uniform_amount_group_table_matches_the_committed_file` fails if any sheet's
table moves without GROUPS.md being regenerated.

## How the proof renders are made (exact steps)

The render test `crates/bir-app-api/tests/proof_renders.rs` exists only on
this branch, so the two halves are made in two checkouts:

1. Branch checkout (this branch): `PROOF_TAG=after cargo test -p bir-app-api --test proof_renders -- --ignored`
   writes the `after-*.html` files here.
2. Main checkout (main, which has no such test): copy
   `crates/bir-app-api/tests/proof_renders.rs` from the branch into the main
   checkout's `crates/bir-app-api/tests/` (uncommitted there; it is a probe,
   never part of main), then run
   `PROOF_TAG=before cargo test -p bir-app-api --test proof_renders -- --ignored`
   and copy the resulting `before-*.html` files into this folder.
3. PNGs: rasterize each html with headless Chrome
   (`chrome --headless --disable-gpu --force-device-scale-factor=2 --screenshot=<out.png> --window-size=900,<full-height> --hide-scrollbars file:///<html>`,
   forward slashes in the screenshot path), then crop both halves of a pair
   to the same band around the pixels that differ (the diff bbox of the two
   rasters), so the pair shows exactly the region the change touches.

The test fills each form through `filled_document_with_receipt` (the path the
app uses) and writes the sheet through the real print path,
`staticize(prepare_html_for(slug, &filled, true))`. The 1701 inputs are the
draft's own `print_field_map` keys and values; the 1702Q inputs are the
`derived:Sched4:` print keys with values formatted exactly as the draft emits
them (`1,000.00` — a raw `1000` spreads from box 1, which the app never
prints). The inputs are identical in both runs; only the grouping code differs.

## HTML renders

### 1702Q Schedule 4, item 2 overflows (12,345,678,901,234)

| | sha256 |
|---|---|
| before-1702q-sched4-overflow.html | `7cfbc72ed82c937967357973bbaaa8245dc177e7eb99fb0df4aad71549a72f80` |
| after-1702q-sched4-overflow.html | `fc39eb398da20b1fd3b28d19c852d89687d2ac9acda6b5985d4466d89617086f` |

before/after DIFFER — before, item 2 alone prints as a line beside boxed rows;
after, the whole Schedule 4 column prints as uniform right-aligned lines.

### 1701 Part VII, item 2 = 1,234,567,890 in the 9-box combs

| | sha256 |
|---|---|
| before-1701-part4-overflow.html | `4ff6be1c8c94ad124c6eaa2cba0603cc1f79e9581d84e8cf59148bc91b6f41f5` |
| after-1701-part4-overflow.html | `108bbdd4e1c88a78dfcbb4533e8651e845cc7e99cdf8306147506b2630c9501d` |

before/after DIFFER — before, item 2 alone is a line among comb-filled rows;
after, all ten column-A amounts print as uniform right-aligned lines while
the spouse column B keeps its combs.

### 1601C Part II, item 14 overflows: the centavo proof

| | sha256 |
|---|---|
| before-1601c-centavo-blank.html | `73dadc34b062b9de116ca8b404178c468de7f85f40a7c552ed91d9275894faa6` |
| after-1601c-centavo-blank.html | `2f3695e2cde308a251c237554b748015c2ed8bb61ae6b67943f6504ed723c8ea` |

before/after DIFFER — after, the empty rows of the converted column lose the
peso AND the paired centavo comb boxes and the printed point; before, item 14
alone prints as a line beside boxed rows.

## Pictures (PNG, headless-Chrome rasters of the renders above)

| 1702q-sched4-overflow | sha256 |
|---|---|
| before | `f64cc95ae1061feb6a7e58396f70a38566465e3afad6e596327232038129c59d` |
| after | `2e2275ba0106f4b48231aec1929489ddf2eb023c5c08db7b9ef0990cb7f24bd3` |

| 1701-part4-overflow | sha256 |
|---|---|
| before | `4865cb91c3b122538989653c0b1239f8ac10155ab6d3a9d854ee615c5220af2b` |
| after | `29bcd216dda0baf7e295811f834d68ba75a85ae370675b167e8925d85ef94b9c` |

| 1601c-centavo-blank | sha256 |
|---|---|
| before | `68431c58fda3e6c9dc3e2f67d29dc01d1c62f0fad56b63341e728d5e94b8db8b` |
| after | `21a1517a7e967a011cd6fce0fb666f1ff31371ef1f3ed51ee8dbf943da019212` |

Every pair differs. The no-overflow byte-identity claim is not a picture pair;
it is pinned inside the test suite (`no_overflow_documents_are_byte_identical_on_more_sheets`
in `crates/bir-print/tests/uniform_amount_sections.rs`, covering 2551Q and
1702EX).
