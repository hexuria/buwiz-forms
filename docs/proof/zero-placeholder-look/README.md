# Zero-placeholder look — proof pictures

Commit of the build in every picture: `30ada22f7071b0e243e29ec5e7f17486e58c0eb9`
(pull request #403's branch fix/money-box-keep-text-on-focus), form 2551Q, the demo
taxpayer, a fresh return with Item 15 = 1,234.56 and one Schedule 1 row = 50,000.00.
Real windows on macOS, desktop window 1440 by 900, phone layout 390 by 844.
Nothing focused unless the file name says otherwise.

The AFTER pictures use a local, uncommitted mock-up only — the exact change is in
`zero-placeholder-prototype.diff` (NOT applied to any branch). The mock: in
MoneyInput/PercentInput/IntInput, when a box's value is a zero amount (not blank)
and its text is the untouched zero rendering, the box shows an empty text and the
zero rendering (`0.00` / `0`) as its grey placeholder; a blank value keeps no
placeholder. Nothing else changed.

## Files

- `before-desktop.png` / `after-desktop.png` — Part II, Items 14 to 28 area, desktop.
- `before-phone.png` / `after-phone.png` — the same part, phone layout 390x844.
- `closeup-before.png` / `closeup-after.png` — close-ups cropped from the desktop
  pair: item 14 locked zero, item 15 filled, items 16/17 editable zeros.
- `after-zero-focused.png` — the zero box (Item 16) FOCUSED in the after look:
  the caret sits in an empty box and the grey `0.00` placeholder still shows.
- `after-zero-typed-5.png` — the same box after typing `5`: the placeholder is
  gone, black `5` in the box.
- `before-desktop-lower.png` / `after-desktop-lower.png` — the lower Part II rows
  and Schedule 1 with its filled 50,000.00 row.
- `after-desktop-allzero.png` — a completely fresh return in the after look, for
  the computed/locked-zero check (item 14/18/19 black locked zeros beside grey
  editable zero placeholders).
- `zero-placeholder-prototype.diff` — the uncommitted mock-up, for reference only.
