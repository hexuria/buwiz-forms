# Android money box after `fix/money-box-no-live-format`

Device: `buwiz` x86_64 emulator. Input: real hold-taps on the numeric keypad (`adb shell input swipe x y x y 150`); no `input text`. Show touches was on during the recording.

Target: 2551Q, Schedule 1, row 1, Taxable Amount. Before proof is `try/android-money-box-1` (`docs/proof/android-money-box/README.md`).

Result at `1988536bfd41b22ee77d6f1e39ef29a61d4bbc87`:

- Zero amounts are empty while focused, so the first digit does not go before `0.00`.
- Keypad taps produced exactly `99999999.99`, one edit at a time, with the caret after the edit.
- Done/blur formatted and saved it as `99,999,999.99`; the row showed `PT010 · 99,999,999.99 · 3.0 · 3,000,000.00` and the page showed `✓ Saved`.

Files: `money-after.mp4`, `typed-99999999-99.png`, `blur-99999999-99.png`.

Matrix/persistence proof added at `7b5071cb21157b38eabed4d7079c7db0401a52c6`:

- `0.50` -> saved row `PT010 · 0.50 · 3.0 · 0.02`
- `1234.56` -> saved row `PT010 · 1,234.56 · 3.0 · 37.04`
- `99999.99` -> saved row `PT010 · 99,999.99 · 3.0 · 3,000.00`
- `100000` -> saved row `PT010 · 100,000.00 · 3.0 · 3,000.00`
- `1000000.01` -> saved row `PT010 · 1,000,000.01 · 3.0 · 30,000.00`
- Middle edit: keypad tap after the third digit of `1000000.01`, then `2`, produced exactly `10020000.01` and saved `10,020,000.01`.
- Persistent reopening: the app process was closed and reopened against the guarded Android test database `files/nsh/dev.goldcoders.bir/test-data/bir_data.db`; the draft showed the persisted `10,020,000.01`.
- Backspace then cleared the focused amount to empty; leaving it returned the amount to zero.

The matrix log is `android-matrix-proof.log`. `android-money-matrix.mp4` records the value matrix with taps shown. `android-money-finish.mp4` records the middle-edit correction, persistent reopening and Backspace-to-empty finish with taps shown. The screenshots named `case-*`, `reopened-persistent-draft.png` and `backspace-empty*.png` are the per-step evidence. `KEYCODE_MOVE_END` was used only to place the caret at the end before clearing; every digit, decimal point, deletion and middle insertion was a real keypad hold-tap.
