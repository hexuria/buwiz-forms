# Desktop selection matrix at the Android-selection head

The macOS rig's Cut, Delete, Backspace, typing-over and Paste cases at
hexuria/nsh `fix/otp-input-android-selection` @ `151dcf3d` (the Android
selection fix branch, on buwiz-next @ bf52b6fe).

## Build line

- app: hexuria/buwiz-forms `92b3e201` (main)
- kit sibling, nsh CLI and runtime: `151dcf3d` (`nsh -V` = 0.6.1 (151dcf3))
- cargo 1.100.0 (3acc79c0c 2026-11-05), the toolchain's own; RUSTC_WRAPPER empty
- one build folder: `/Users/devin/rig/selcheck/target`, empty before, nothing else writes it
- four safety variables, scratch database, invented values; mock account server on localhost:3008

## Files

- `matrix.txt` — the selection matrix transcript: cut first/middle/last digit exact
  (23/13/12), select-all delete empties, bare backspace at the end removes the last
  digit, typing over the middle digit replaces exactly it, and all four paste formats
  (raw, dashed, spaced, 14 digits cut to nine) land as 000123453. Every case correct.
- `extended.txt` — two-digit cuts exact (first two leave 3, last two leave 1),
  forward-delete at the start removes the first digit, forward-delete over a
  selection removes exactly it.
- `field.png` — the New taxpayer TIN cells in the running app.
- `tapcell-note.txt` — the one desktop case that is not a keyboard case, and why
  it reads as it does.

## What this does not show

- Mouse-click caret placement on macOS: a click on the cells is delivered to the
  hosted AppKit text field, not to the core's touch path, so the component's
  computed tap-to-caret (which fixes Android) never runs on macOS — see
  tapcell-note.txt. This is unchanged from buwiz-next and was the same before
  this branch; it is stated here so the desktop picture is honest.
- Android behavior: the emulator machine verifies at this head.
