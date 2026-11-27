# Print uniform amount sections (#477) — proof

`GROUPS.md` is the validation table: every (section, column) amount-comb
group found on each of the 43 frozen sheets, generated from the same
geometric grouping the fill uses (`cargo test -p bir-print uniform_amount_group_table -- --ignored`).

The renders are print proofs through the real print path,
`staticize(prepare_html_for(slug, &filled_document(..), true))`:

| file pair | before sha256 | after sha256 | verdict |
|---|---|---|---|
| *-1701-part7-overflow.html | `027d2262cbbcd540259dd7cc09a562c98c3014585116c52a232982bc3b2328d0` | `1e344f089263cbe03b0347073b348927a2d734a910135a8de7637a1dbdcbb9ed` | DIFFER — item 2 overflows, so all ten rows of the column print as lines after |
| *-1701-part7-nooverflow.html | `51487cc3f96e67bcfde85f7fcae5d311a3de56dd392ef9538faaf6c56f397c2a` | `51487cc3f96e67bcfde85f7fcae5d311a3de56dd392ef9538faaf6c56f397c2a` | byte-identical — no conversion anywhere, nothing moves |
| *-1702mx-part2-overflow.html | `e8ff9a15c073d53dc0af93bbe38b40c0cdc4e585e2f1d73c1293bab289e02714` | `1757a0ba245a3c8dce71ab3ee468fdfbfb5a4bf4cf4138dc19dd079e4b3d9ac0` | DIFFER — item 15 overflows, the 8-row column prints uniform lines after |
| *-1702mx-part2-nooverflow.html | `e47893adacbbe867664cc4e771727024fc4d70f5d0745170ceb990c9238736c5` | `e47893adacbbe867664cc4e771727024fc4d70f5d0745170ceb990c9238736c5` | byte-identical |

Fields: 1701 Part VII items 1–10 column A = 1,000.00 except item 2 =
1,234,567,890.00 in the overflow render; 1702MX Part II items 14–21 =
1,000.00 except item 15 = 12,345,678,901,234.00 in the overflow render.
