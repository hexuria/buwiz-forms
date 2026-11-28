# Owner print proof — #495 (Rule 6 own-field bands) + #499 (uniform amount sections) on main

Branch `try/owner-print-proof` = main `d910f47d` + fix/rule6-band-uses-own-field
`ff960f2c` + fix/print-uniform-amount-sections `3e30432308` (win-3's final sha
had not been named when this branch was built; the remote head was already at
3e30432308, past the 1e102e24 the conductor quoted — built with the remote head).

Every render is through the app's own print path,
`staticize(prepare_html_for(slug, &filled_document(..), true))` — never the raw
frozen sheet. Before = main `d910f47d`, same fill, same crop. Every pair differs
by sha256.

## The cases

### 1701 item 9 with the owner's address

Fill: `UNIT 1203 TWELFTH FLOOR CURVE BUILDING BONIFACIO GLOBAL CITY TAGUIG
METRO MANILA UNIT 1203 TWELFTH FLOOR CURVE BUILDING BONIFACIO` in item 9's
specify. Before: the original box is emptied, the whole value moves onto an
inserted white band that runs into the amount columns. After: the original
field keeps its first part, the rest wraps into greyed full-row continuation
lines — the strip IS the row, one bare input per line, the amount columns on
the added lines greyed with nothing rendered or entered there, the amount box
at its sheet height, no divider crossing text.

### 1701MS item 9

Same composition on 1701MS: the original field holds the first part, greyed
full-row continuation lines, amount right-aligned with its last digit in the
right-most slot.

### 1701 Part VII — one amount over 9 digits (12,345,678,901)

Item 2A filled with 12345678901; items 1,3-9 column A with 1000; items 1,3,4
column B with 1000. Before: the overflowing amount prints as a lone plain line
while the rest of the column keeps comb boxes — the mixing the owner forbade.
After: the WHOLE section column A prints as uniform right-aligned plain text,
blank rows print blank without comb boxes, and the independent column B keeps
its combs with its digits right-aligned in the right-most slots.

### 1701 Part VII — exactly 10 digits (1,234,567,890)

Same fill but item 2A = 1234567890, exactly at the comb capacity boundary
(capacity 9). After: the same uniform conversion — 10 digits does not fit the
9-slot comb, so the whole column prints as right-aligned plain text.

## sha256

| file | sha256 |
|---|---|
| after-1701-item9.png  |
| after-1701-part7-exact10.png  |
| after-1701-part7-over9.png  |
| after-1701ms-item9.png  |
| before-1701-item9.png  |
| before-1701-part7-exact10.png  |
| before-1701-part7-over9.png  |
| before-1701ms-item9.png  |
