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

The renders are print proofs through the real print path,
`staticize(prepare_html_for(slug, &filled_document(..), true))` (written by
`cargo test -p bir-app-api --test proof_renders -- --ignored`). "before" is
the pre-#477 behaviour on main (per-cell conversion only), "after" the branch.

## HTML renders

### 1702Q Schedule 4, item 2 overflows (12,345,678,901,234)

| | sha256 |
|---|---|
| before-1702q-sched4-overflow.html | `a240191257a6e7a57d764ee5c1253557ee2b1aa70cb616e684d3d573ffcda540` |
| after-1702q-sched4-overflow.html | `19d43eedf83c9a0df6e843effe4cd4a9cf8a678e2a1bf877b71f4319a2d8d0f0` |

before/after DIFFER — the whole section column prints as uniform right-aligned lines after.

### 1701 page 2 Schedule 3 / Part VII, ten digits in the 9-box combs

| | sha256 |
|---|---|
| before-1701-part4-overflow.html | `6918ec8e796fb26e77408fa53a926718545aeb4beef44cb605630200d43f125a` |
| after-1701-part4-overflow.html | `989febdec4b65df0cc6c6c567774735df0884bf89441c29e2ba04336b3571df7` |

before/after DIFFER — the whole section column prints as uniform right-aligned lines after.

### 1601C Part II, item 14 overflows: the centavo proof

| | sha256 |
|---|---|
| before-1601c-centavo-blank.html | `73dadc34b062b9de116ca8b404178c468de7f85f40a7c552ed91d9275894faa6` |
| after-1601c-centavo-blank.html | `2f3695e2cde308a251c237554b748015c2ed8bb61ae6b67943f6504ed723c8ea` |

before/after DIFFER — after, the empty rows of the converted column lose the
peso AND the paired centavo comb boxes and the printed point; before, item 14
alone prints as a line beside boxed rows.

## Pictures (PNG, headless-Chrome rasters of the staticize renders above)

| 1702q-sched4-overflow | sha256 |
|---|---|
| before | `4d22abd55598f4418750b715f0098067ceb1ac83158838755c104857cbb7579f` |
| after | `7b5bcb3dc16643ce5cfd88a51591ef2eab8124894f5936c8e89aed2121d06e0c` |

| 1701-part4-overflow | sha256 |
|---|---|
| before | `e4a0f02208ee87b116689d6629a93f720848d9e45894a70b27ca4156f897afe3` |
| after | `513aeeb6e37109909ac2700f1b549be66d1ea434db1b3f17972a09e29323639a` |

| 1601c-centavo-blank | sha256 |
|---|---|
| before | `23ab882b18a0d283782317537d8e188682cea81db6efa7a8c15419e1a7c3d586` |
| after | `7d917b63f0ca5e35ab5a91de44c283ff1d0eee16e76a0c07751b7bbdcd7f3dc4` |
