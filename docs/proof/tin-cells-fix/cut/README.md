# Cut, delete and typing over a selection in the TIN cells — and the isolated burst pair

Two things are proven here, in one place because they share the rigs:

1. **The burst before/after pair, repeated in full isolation** (the first pair shared one cargo
   target folder; this one does not). Each side: app `92b3e201`, its own sibling kit checkout, the
   nsh CLI and the runtime all at one kit commit, one empty build folder per side, RUSTC_WRAPPER
   empty, cargo 1.100.0 (3acc79c0c 2026-11-05, the toolchain's own).
   - before side: kit `8c78cfd6` (buwiz-next), nsh 0.6.1 (8c78cfd), folder `/Users/devin/rig/before/target`
   - after side: kit `6149250d` (the fix head), nsh 0.6.1 (6149250), folder `/Users/devin/rig/after/target`
   - `before-burst-isolated.txt`: **0 of 30 correct** at 10/20/30 ms, ten runs each — the
     permutations as before (`001234530` and others), never a digit lost.
   - `after-burst-isolated.txt`: **30 of 30 correct** (run at kit `62e436ff`; the caret-pin removal
     in `6149250d` cannot affect bursts — it only stops answering caret moves — and
     `after-fixed-burst.txt` confirms 9 of 9 valid runs correct at the final head; run 1 of that
     file was contaminated by the operator's own keystroke and says so honestly by its shape `[453]`).

2. **Cut, Delete, Backspace and typing over a selection**, before and after the fix, in the real
   Mac app (New taxpayer TIN cells, real OS key events, scratch database, test environment).
   - `before-selection-matrix.txt` — kit `8c78cfd6`: selections cannot exist at all (the pin
     forced everything to the end), so Cut does nothing and every delete is a bare end-delete.
     Cut as a user operation effectively did not exist before the paste-menu fix.
   - `after-62e436ff-selection-matrix.txt` — the fault this job investigated, reproduced: the pin
     answered the arrow-key carets a selection starts with and yanked them to the end, so a
     selection meant for the first or middle digit formed at the end: cutting "the first digit"
     left `12` instead of `23`, cutting "the middle" left `12` instead of `13`. This is the same
     class of wrongness the Android emulator saw ("cutting one selected digit of three emptied all
     three boxes"); on Android the plausibly same pin hits the long-press touch-down caret before
     the drag anchors. Exact steps there are owed by the emulator machine.
   - `after-fixed-selection-matrix.txt` — kit `6149250d`: **every case correct** — cut of the
     first/middle/last digit removes exactly it (`23`/`13`/`12`), two-digit selections exact,
     select-all empties all, Delete and forward-delete the same, typing over a selection replaces
     exactly it (`153`, `5`), a bare Backspace removes the digit before the caret.
   - `after-fixed-paste.txt` — the four paste formats at the final head: raw, dashed, spaced and
     the 14-digit paste cut to nine, all `000123453`.
   - `fixed-matrix.mp4` (under two minutes) — the fixed selection-matrix run as it happened.
   - `fixed-typed-123.png` / `fixed-cut-middle-13.png` — three digits typed, then the middle digit
     cut: two cells keep `1` and `3` exactly.

## What these files do not show

- The Android gesture itself. The Mac proof uses keyboard-formed selections (the only kind a
  desktop has); Android forms them by long-press and drag. Whether the emulator's emptying was the
  pin hitting the touch-down caret or the platform's word-selection grabbing the whole digit run
  (digits are one word — cutting it correctly empties everything) is for the emulator machine at
  the new head; either way no pin now interferes, because none is ever issued.
- The before-fix screenshots: the faulty behavior is textual and is in the matrix files; the fixed
  screenshots are the same shape after. The before/after harness discipline (the four arrow-matrix
  tests fail at the unfixed component, pass at the fix) is in hexuria/nsh#39's checks.
