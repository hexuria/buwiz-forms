# Candidate (keep-text-on-focus) — Part 1 results on Windows, warm 2bc1e89d + fields.tsx.diff
Real OS key events, 2551Q Schedule 1 row 1 taxable amount. cand-rec.mp4: Tab into 1,234.56 + type 15000.50, then a 99999999.99 run.

- P1 burst 15000.50 into 0.00 (no delay): RIGHT — 15,000.50.
- P2 99999999.99 two more runs: RIGHT both — 99,999,999.99 (total 3/3 at this head).
- P3 type 1234, Alt+Tab away and back, type 5: RIGHT — on return the box shows 1234 intact, still focused, plain digits; 5 appends; after leaving 12,345.00.
- P4 click 1,234.56, Ctrl+A, type 15000.50: RIGHT — 15,000.50.
- P5 click, Ctrl+A, Backspace, type: RIGHT — 15,000.50.
- P6 EDITING ONE DIGIT, click 1,234.56 then End Left Left Backspace 9: the selection collapses with End as expected
  (caret 8, Left 7, Left 6 in the GROUPED text "1,234.56"), but Backspace there deletes the DECIMAL POINT, not a digit
  (trace: text becomes "1,23456"), and the 9 then gives "1,234956" = saved 1,234,956.00. The same with Home Right Right 9:
  the comma eats one Right, so the 9 lands after the '1,' giving 19,234.56 (plain-text muscle memory would give 12,934.56).
  Both are visible on screen, but the separators in the focused text shift every position by the comma count — editing with
  arrows in the grouped text is hazardous, exactly the class of fault the no-live-format design was meant to avoid. NOTE:
  the official program has the same hazard (it too shows separators once formatted); this is intrinsic to editing
  separator-laden text, not a regression vs official.
- P7 visuals: after one click the whole amount is highlighted blue, no caret anywhere. A SECOND click on the already
  focused box changes nothing: the highlight stays, no caret appears where clicked (verified by zoom screenshot).
  Typing 9 replaces the whole selection. No SelectionChange event for any click caret ever reaches the page.
