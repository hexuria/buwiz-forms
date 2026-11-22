# TIN AFTER shots at feat/form-fields-all b3da6f03 (locked phone TIN cell padding fix)

12 PNGs, macOS windows via `just nsh-shots`, demo taxpayer, dry-run env, 1x (non-Retina display). Same recipe and names as `../head-da972662/`.

| File | Shows | Visual failure |
|---|---|---|
| tin-after-0605-locked-320x700-light.png | 0605 item 9 locked profile TIN: 000 / 123 / 453 / 00000 all in full, centred, "From the tax profile" line present | none in the TIN row; window buttons sit on the sticky part title |
| tin-after-0605-locked-320x700-dark.png | same, dark | same |
| tin-after-1801-typed-empty-320x700-light.png | 1801 item 5 locked TIN in full with its line; item 11 typed TIN, four empty boxes in one row | window buttons sit on the sticky part title |
| tin-after-1801-typed-empty-320x700-dark.png | same, dark | same |
| tin-after-1801-typed-filled-320x700-light.png | 1801 item 5 locked TIN in full; item 11 typed 000 123 453 00001 | still clipped in the TYPED (editable) boxes: the last digit of "000" and of "453" is cut at the box edge (same as da972662; the fix covers locked cells only); window buttons on the part title |
| tin-after-1801-typed-filled-320x700-dark.png | same, dark | same |
| tin-after-1706-refusal-320x700-light.png | 1706 after Check: item 6A locked seller TIN in full with its line | unchanged: the "12 items need attention" toast and the pill cover the buyer TIN row, its boxes and sentence are not readable; window buttons on the part title |
| tin-after-1706-refusal-320x700-dark.png | same, dark | same |
| tin-after-2000-onebox-320x700-light.png | 2000 item 4 locked TIN in full with its line; item 12 one-box TIN (placeholder 000-000-000-00000) | window buttons sit on the sticky part title |
| tin-after-2000-onebox-320x700-dark.png | same, dark | same |
| tin-after-0605-locked-420x900-light.png | 0605 item 9 locked TIN at 420: pixel-identical to head-da972662 | none |
| tin-after-0605-locked-420x900-dark.png | same, dark: pixel-identical to head-da972662 | none |
