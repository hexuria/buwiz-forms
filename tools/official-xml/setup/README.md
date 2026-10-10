# Layout setup scripts

`gen_layout.js <hta> <form_id> setup/<form_id>.js` runs the script in the
prepared page before every pass. Use it only for controls the official page
adds inside `frmMain` after a filer's choice, so the layout matches what
`saveXMLsubmit()` really serializes. Each resulting layout is checked byte for
byte against `runtime.js` through the form's sample payload.

| Layout | Why |
|---|---|
| `1600vt-v2018-private`, `1600vt-v2018-government` | Item 10 makes `changedrpATCList` draw the ATC popup (`AtcCode1..n` checkboxes, inside the form) for that category: 10 private, 12 government. |
| `1600pt-v2018-private`, `1600pt-v2018-government` | Same popup: 6 private, 31 government. |
| `1600wp-v2010-atc0`, `-atc1`, `-atc2` | Item 7 draws both ATC popups (two ATCs per category); `getATCCode` adds one Part II row per ticked ATC (none, one or two). |

`setup/<form_id>.runtime` (any content) makes `regen_all.sh` derive that layout
with `gen_layout_runtime.js`, which loads the whole page with its own scripts,
for forms whose load handlers add controls inside `frmMain` (extra `getRdo()`
selects, 1801's default schedule rows): 1707-v2021, 1707a-v2021, 1800-v2018,
1801-v2018.
