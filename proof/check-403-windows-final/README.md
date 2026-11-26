# Real-input proof: hexuria/buwiz-forms#403 on Windows

build: app `d4f2852a28c323e3f6da85eae4f61d970d84ff0e` (branch fix/money-box-keep-text-on-focus), nsh `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` beside it, tool `nsh 0.6.1 (8c78cfd)`, cargo 1.99.0 (5f94df478 2026-08-27) at `C:\Users\Administrator\.cargo\bin\cargo`, build folder `C:\ci\check403\buwiz-forms\target-check403`.

Machine: Windows Server 2022 build 20348, 1600x1200, 96 DPI (100% scale; tree coordinates are screen pixels 1:1).
Every click and keystroke under test is REAL OS input (pyautogui/SendInput into the app window). gpui-agent was used ONLY for setup (adding rows, expanding parts, opening the library) and for reading back saved values. Safety env on every run: BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1. Demo taxpayer only; Queue never pressed.

## What each file shows

- `case-log.txt` — the per-run log: what was typed, what the rows/boxes held before and after, totals, whether 500 appeared anywhere. This is the text evidence per run.
- `shot-NN-*.png` — numbered screenshots in chronological order (NN by capture time). Names carry the case: `c1a/c1b/c1c` case 1 (before, focused500 = the box holding 500 still focused, predelete, postdelete), `c2*` case 2 (the print preview clearly showing 1,500.50 in item 15/18; the File menu shots proving no Save command exists), `c3*` case 3 (in-flight and settled), `c4*` case 4 (item-13 area, settled states), `c5*` case 5 (decimal comma and the malformed-comma message). `c3-probe-txt16-settled` shows the red domain validation line proving item 16 rejects values unless Amended (why txt16 was not a valid grey-zero target). Early `setup-*`/`cur*`/`rows-state` shots document the setup path.
- `recording-cases-4x.mp4` — the screen recording, sped up 4x (4.4 min). COVERAGE CAVEAT: ffmpeg's gdigrab on this VM is capture-bound (~0.5x realtime) and the first recorder died silently; THIS VIDEO COVERS case 1b-r2redo THROUGH case 4 (~22:04-22:25 wall). Cases 1a/1b-r1 are NOT in the video; their proof is the screenshots + case-log.txt. The video is a single continuous take of everything it does cover.
- `recording-early-setup-fragment.mp4` — 38 s fragment of the early setup (first recorder output before it died). Included for completeness only.

## What the files do NOT show

- No video of cases 1a and 1b-r1 (see caveat above).
- c1b-r2's post-delete screenshot is missing (driver hung and was killed); the deletion itself is verified in case-log.txt and was redone cleanly as c1b-r2redo.
- c1a-r2 is logged but INVALID as a middle-row test (only two rows existed at the start); it is disregarded. Valid 1a runs are r1, r3, r4.
- Case 2b as literally specified could not run: once the return is real there is NO explicit Save control on the page (File menu: New Tax Profile/New Return/Settings/Sign Out/Hide/Quit; toolbar: Print preview/Check/Queue; the top-right '✓ Saved' is a note, not a button). Ctrl+S was pressed as a candidate; see report.

## Rows used (case 1)

As the spec allowed: initial setup row 1 PT010 1,000 · row 2 PT040 2,000 · row 3 PT010 3,000 (total tax 180.00). Rows are consumed by deletion, so later runs used the same rotation rebuilt one row at a time; case-log.txt records the exact row contents before and after every run.
