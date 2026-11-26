# Money typing on macOS — real OS keys, main 47ab5025

Rig: CGEvent-level keys and clicks (keytyped/mouseclick/keyop, evidence tools as in the TIN matrix), never the harness typing.
App: hexuria/buwiz-forms main 47ab5025, kit sibling at the pin 8c78cfd6, four safety vars, scratch db, demo taxpayer juana santos 000-123-453-00000, invented values only, Queue never pressed.
Saved values read from the scratch database (form_drafts.data_json, plain sqlite in the test environment), not the screen.

## YES/NO: YES — a person's keys can save a different amount with no error, but not the way Windows does it.

- Typing digits at the end of a money box is EXACT on macOS at 120 ms/key and in a fast burst (~10 ms/key): 99999999.99, 12345678.90, 123456789.12, 1234567.89 all save exactly, in 2551Q Schedule 1's taxable amount and item 15. The Windows corruption of end-typing does NOT reproduce.
- 1000000.005 is REFUSED visibly both speeds: the 12-character input limit validation fires ("Exceeds the official 12-character input limit..."), Done disables, nothing saves. No silent corruption.
- THE MAC'S OWN FAULT: clicking the field and typing prepends at the FRONT. The money box is right-aligned; a click in the wide empty area left of the digits puts the caret at position 0 (verified: click, no arrows, type 7 into 51234567890 -> saved 751234567890). Click + 5 left-arrows + type 5 into 1234567890 -> saved 51234567890 (money-midinsert.mp4). Backspace there no-ops (caret already at 0). The number magnitude changes with no error.
- When the caret verifiably starts at the end (click, Cmd+A, Right-arrow), mid-edits are CORRECT: leftx5 + type 5 into 1234567890 -> 12345657890 exactly; end-append correct (12345678909).

## Table (2551Q Schedule 1 row 1 taxable amount unless noted)

| keys | speed | typed context | saved | verdict |
|---|---|---|---|---|
| 99999999.99 | 120ms | end | 99999999.99 | exact |
| 12345678.90 | 120ms | end | 12345678.9 | exact |
| 123456789.12 | 120ms | end | 123456789.12 | exact |
| 1000000.005 | 120ms | end | (unchanged) | refused visibly, 12-char limit |
| 99999999.99 | burst | end | 99999999.99 | exact |
| 12345678.90 | burst | end | 12345678.9 | exact |
| 123456789.12 | burst | end | 123456789.12 | exact |
| 1234567.89 | burst | end | 1234567.89 | exact |
| 99999999.99 | burst | item 15 end | 99999999.99 | exact |
| 123456789.12 | 120ms | item 15 end | 123456789.12 | exact |
| click+7 | 120ms | after click | 751234567890 | CORRUPT (front-insert) |
| click+leftx5+5 | 120ms | mid attempt | 51234567890 | CORRUPT (front-insert, recorded) |
| click+leftx5+backspace | - | mid attempt | unchanged | no-op (caret at 0) |
| cmda+right+9 | 120ms | end verified | 12345678909 | exact |
| cmda+right+leftx5+5 | 120ms | mid verified | 12345657890 | exact |

Build line: app 47ab5025 (hexuria main) · kit 8c78cfd6 (pin) · nsh -V 0.6.1 (8c78cfd) · cargo 1.100.0 · folder /Users/devin/rig/desktopclick/target.
