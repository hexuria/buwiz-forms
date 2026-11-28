# Print uniform amount sections (#477) — proof

`GROUPS.md` is the validation table: every amount-comb group found on each of
the 43 frozen sheets, generated from the same section-band grouping the fill
uses (`cargo test -p bir-print uniform_amount_group_table -- --ignored` with
GROUP_TABLE_OUT pointing at the file). The grouping source is the sheet's own
section bands: the `<div class="t">` headings whose text starts with "Part" or
"Schedule"; a group is one section's column, from the section's heading to the
next heading on that page. `uniform_amount_group_table_matches_the_committed_file`
fails if any sheet's table moves without GROUPS.md being regenerated.

The renders are print proofs through the real print path,
`staticize(prepare_html_for(slug, &filled_document(..), true))`. "before" is
the pre-#477 behaviour (per-cell conversion only), "after" the branch.

## HTML renders

### 1702Q Schedule 4, item 2 overflows (12,345,678,901,234)

| | sha256 |
|---|---|
| before-1702q-sched4-overflow.html | `a240191257a6e7a57d764ee5c1253557ee2b1aa70cb616e684d3d573ffcda540` |
| after-1702q-sched4-overflow.html | `2e900ae3c216965efc74773fd21633ab1683ed42067939e08ffb1bf97e38e0ee` |

before/after DIFFER — the whole section column prints as uniform right-aligned lines after.

### 1701 page 2 Schedule 3, item 10 overflows (12,345,678,901,234)

| | sha256 |
|---|---|
| before-1701-part4-overflow.html | `6918ec8e796fb26e77408fa53a926718545aeb4beef44cb605630200d43f125a` |
| after-1701-part4-overflow.html | `c0c7d377ea8f6c15afa52eccc90cb87b82cc9349f234c820e3e9c1014646c1d1` |

before/after DIFFER — the whole section column prints as uniform right-aligned lines after.

## Pictures (PNG, headless-Chrome rasters of the staticize renders above)

| 1702q-sched4-overflow | sha256 |
|---|---|
| before | `4d22abd55598f4418750b715f0098067ceb1ac83158838755c104857cbb7579f` |
| after | `7b5bcb3dc16643ce5cfd88a51591ef2eab8124894f5936c8e89aed2121d06e0c` |

| 1701-part4-overflow | sha256 |
|---|---|
| before | `e4a0f02208ee87b116689d6629a93f720848d9e45894a70b27ca4156f897afe3` |
| after | `513aeeb6e37109909ac2700f1b549be66d1ea434db1b3f17972a09e29323639a` |

before-1702q-sched4-overflow.png: Schedule 4 item 2 prints as a lone plain
line while items 1, 3-6, 6a, 6b and the Total keep their comb boxes — the
mixing the owner forbade. after-: the whole column prints as uniform
right-aligned lines, Schedule 3 above untouched.

before-1701-part4-overflow.png / after-: Schedule 3 items 8-18, column A.
After, every row of the converted column is a right-aligned line; the blank
rows print blank without comb boxes; the spouse column B keeps its boxes
throughout (independent column); items 19-24 of column A are blank and
blanked.
