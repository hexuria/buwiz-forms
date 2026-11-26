# Money-box FINAL CANDIDATE real-key round — hexuria/buwiz-forms @ 542e484401494623a7ccbae9e25832985739a3a8

PR #403 fix/money-box-keep-text-on-focus, rev-parse confirmed, fresh clone, CARGO_TARGET_DIR inside the
checkout, desktop. Real window, real hardware-keycode keys and real clicks. 2551Q Schedule 1 row 1 unless
said; demo taxpayer; invented values; draft read after each case. Four safety variables; Queue never pressed.

## B1. Delete row with typing pending (the independent-reader fault) — REAL, 3/3 and 3/3

Rows built: PT010 1,000 / PT040 2,000 / PT010 3,000.

- (a) mid row ×3: open row 2, click amount, Cmd+A, type 500 (box `500`), click Delete row WITHOUT leaving:
  the page still shows three rows — `Row 1, PT010 · 1,000.00` · `Row 2, 500.00` (NO ATC) · `Row 3, PT010 ·
  3,000.00`. Draft: `[1000, 500, 3000]`, middle row `atc ""`, amount 500.0, rate/due 0. Identical 3/3.
- (b) last row ×3: same shape — `Row 3, 500.00` no ATC; draft `[1000, 2000, 500]`, blank ATC. 3/3.
- (c) control (Tab first, then delete): page shows only Row 1 and Row 3 — but the draft keeps a HOLLOW
  middle row (`atc ""`, amount 0.0): delete hollows the row, the page hides hollow rows. The uncommitted
  500 is exactly what un-hides it in (a)/(b).

Evidence: caseB1.log/json, shot-b1-*.png.

## A. Entry into an editable zero — every amount saved as typed: YES

click/Tab/Shift+Tab 15000.50 → 15,000.50 saved 15000.5; 1000000.005 → 1,000,000.01 saved 1000000.01
(half-up); burst exact; click+first-key at ~0 ms: wrong 0/10. Typing states start at `''` — focus writes
nothing. Pictures: unfocused grey 0.00 (crop-ph), focused empty box with grey 0.00 placeholder + caret
(crop-fe, fshot-focused-empty-zero), black digits while typing (crop-ty).

## B. Filled 1,234.56

Tab then 7 → replaced, 7.00/7.0 · Tab Right 9 → `1,234.569` then 1,234.57/1234.57 · click between 2|3 then
9 → `12934.56` then 12,934.56/12934.56 · Cmd+A Backspace Tab away → empty box, grey placeholder, draft 0.0,
no sentence · end caret 4×Backspace `1,23` → 123.00/123.0. All RIGHT. (B4/B5 re-run after a rig entry flaw;
first-pass noise discarded.)

## C. Commas into a zero box

`1500,50` → 1,500.50/1500.5 · `1,500` → 1,500.00/1500.0 · `15,00,50` → empty box, grey placeholder, red
border, sentence word for word (crop-c3): "Taxable Amount: 15,00,50 is not an amount. The box kept 0.00.
Use a point for centavos, and commas only between thousands."

## D. Unreadable into a zero box

`1500x` → same shape; sentence (crop-d1): "Taxable Amount: 1500x is not an amount. The box kept 0.00."
Draft stays 0.0.

## E. Typed 0 in a zero box

states `''→0` while typing; leaving returns to `''` (placeholder), saved 0.0, nothing changes.

## F. Cmd+Tab away 5 s and back, five runs each

filled 1,234.56 focused: box unchanged 5/5 (reads `1234.56` ungrouped while editing at this head); typing 7
continues at the caret — center-click had put it at the FRONT, so `71234.56` → 71,234.56 saved, 5/5
consistent. Zero box focused, nothing typed: empty stays empty `''`, saved 0.0, nothing reported, 5/5
(fshot-f2-*.png).

## G. Box that goes away — 2550M 13A `12,3 4`, fold Part II, reopen

typed `12,3 4` verbatim; node gone after fold; reopen shows the old 1,000.00 red-bordered with the sentence
word for word (crop-g2): "Item 13A: 12,3 4 is not an amount. The box kept 1,000.00. Use a point for
centavos, and commas only between thousands." Draft 1000.0.

## H. Dark mode

fshot-h-dark.png / crop-h.png: focused editable zero — blue ring, caret, dim grey `0.00` placeholder —
directly above the locked Tax Due zero `0.00` in solid light tone; the two reads are visually distinct.
