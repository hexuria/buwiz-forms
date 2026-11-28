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

## How the proof renders are made (ONE committed command)

Both halves of every pair come from the same committed command, run twice with
different tags against the two revisions:

    PROOF_TAG=after  cargo test -p bir-app-api --test proof_renders -- --ignored    # on this branch
    PROOF_TAG=before cargo test -p bir-app-api --test proof_renders -- --ignored    # on main (9bf2b3e5)

`crates/bir-app-api/tests/proof_renders.rs` fills each form through
`filled_document_with_receipt` (the path the app uses; the 1701 inputs are the
draft's own `print_field_map` keys) and writes the sheet through the real
print path, `staticize(prepare_html_for(slug, &filled, true))`. The inputs are
identical in both runs; only the grouping code differs. The PNGs are
headless-Chrome rasters of the HTML files, cropped to the band that differs.

## HTML renders

### 1702Q Schedule 4, item 2 overflows (12,345,678,901,234)

| | sha256 |
|---|---|
| before-1702q-sched4-overflow.html | `49a2eefd7dc2a789723cb66b64c2cfbc1d144b8f4fb068243d128d616f6e588c` |
| after-1702q-sched4-overflow.html | `19d43eedf83c9a0df6e843effe4cd4a9cf8a678e2a1bf877b71f4319a2d8d0f0` |

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
| before | `cab6ca2ea854d58418c6fdccde5a2454ac94e5d79a6dcfdf4d3571263b92cab0` |
| after | `61f8ba11ba3b721093c4c5fbbec3e48f626ff2cb1c323a03ce4cceb5a8c1ddb1` |

| 1701-part4-overflow | sha256 |
|---|---|
| before | `357399970a206acbe26d739864f6b62b467f280bf89dcb41ced433efc8b49bd8` |
| after | `4bc671846ec243913d60a02bb8fbc06a1ba8e7b5dcd444e7fd0eb748c6796fb7` |

| 1601c-centavo-blank | sha256 |
|---|---|
| before | `399fd95f72020722246d02474958c674f668257f8fd8d8ca6f4c632ce5537729` |
| after | `f86afd33f44680b6ec96e5d089c285d980f77e89296578b8c40e16cdf54ad0ab` |

Every pair differs. The no-overflow byte-identity claim is not a picture pair;
it is pinned inside the test suite (`no_overflow_render_is_byte_identical`
in `crates/bir-print/tests/uniform_amount_sections.rs`, covering 2551Q and
1702EX).
