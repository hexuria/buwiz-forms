# Android money box after `fix/money-box-keep-text-on-focus`

Commit under test: `4e29caf043a19ed608168101e5ba0617d0c2ae18`.

Device: `buwiz` x86_64 Android emulator, `emulator-5554`. Target: 2551Q, Schedule 1, row 1, Taxable Amount, demo taxpayer only. The guarded Android test database was `/data/data/com.longbridge.nsh.host/files/nsh/dev.goldcoders.bir/test-data/bir_data.db`. No Queue button was pressed and no live sending was enabled.

Input method: real hold-taps on the emulator's on-screen numeric keypad (`adb shell input swipe x y x y 150`). Digits, `.` and every Backspace were keypad taps; `adb shell input text` and hardware key events were not used. Taps inside the amount box were used only to place the caret. The final runner waits about 0.58 s after each digit because a first, faster harness attempt under the loaded host once ordered `4` after the decimal point; that attempt was discarded before recording. `show_touches` was on for the recording.

## Harness and commands

The production checkout was the commit above. For the proof run only, `nsh/App.tsx` was temporarily pointed at the existing `screens/form/FormPageAny` fixture and that fixture was told to `jumpTo(4, "4:0")`, so the emulator opened directly at Schedule 1 row 1 with the demo taxpayer. The harness did not change money mask, form, or platform behavior. Separately, after the screenshots were captured, one temporary `console.log` line was added in `MaskedField.onSelectionChange` only to record the caret-0 note in `caret-selection-log.txt`. All three temporary file edits were restored after the run and the worktree has no production-file modification.

Build/dev server, from `/home/ubuntu/forms-validation/buwiz-keep/nsh`:

```sh
HOME=/home/ubuntu/forms-validation/home RUSTC_WRAPPER= /home/ubuntu/forms-validation/nsh2404-keep.sh nsh run --target android --port 8792
```

Activity launch used by the runner:

```sh
adb -s emulator-5554 shell am start -S \
  -n com.longbridge.nsh.host/com.longbridge.nsh.DevActivity \
  --es url ws://127.0.0.1:8792/ws \
  --es nsh.env.EBIR_TEST_ENV 1 \
  --es nsh.env.BIR_AUTH_BYPASS 1 \
  --es nsh.env.BUWIZ_NO_KEYCHAIN 1 \
  --es nsh.env.BIR_SFTP_DRY_RUN 1 \
  --es nsh.env.BIR_DATABASE_PATH /data/data/com.longbridge.nsh.host/files/nsh/dev.goldcoders.bir/test-data/bir_data.db
```

Runner:

```sh
python3 /home/ubuntu/forms-validation/android_keep_text_matrix.py
```

A copy is kept here as `android_keep_text_matrix.py`. The machine-readable run log is `android-keep-text-proof.log`; its final line is `ANDROID_KEEP_TEXT_PASS`.

## Result: PASS

### Filled `1,234.56`, tap, digit, `.`, Backspace

The amount was first entered as `1234.56`, saved, and shown formatted as `1,234.56`.

- Before the tap, the filled box showed exactly `1,234.56`.
- A tap at x=900 placed the visible caret after the comma, before `2`. The text after focus was still exactly `1,234.56`; it was not rewritten to ungrouped `1234.56`.
- Keypad digit `7` produced exactly `1,7234.56`. This insertion point proves the tap caret was after the comma: caret 0 would have produced `71,234.56`, while an end caret would have produced `1,234.567`.
- Keypad `.` was a no-op because the amount already had a decimal point: the text stayed `1,7234.56`.
- Keypad Backspace removed the inserted digit and restored exactly `1,234.56`.

Android caret-0 note: **Android does transiently report caret 0 after tapping a filled box, then reports the tap's real caret.** UiAutomator does not expose the numeric selection in its XML dump, so the uninstrumented proof verified the effective caret visually (the caret/handle screenshots) and operationally (the `7` landed after the comma rather than at the start). A supplementary temporary `onSelectionChange` instrumentation run, restored afterward and not committed, showed three `0,0` reports before the correct tap caret on filled grouped amounts (`caret-selection-log.txt`). In the uninstrumented `1,234.56` proof, the effective caret at the digit keypress was the tap caret: `7` produced `1,7234.56`, not `71,234.56`.

Primary screenshots:

- `primary-before-focus.png`
- `primary-after-focus.png` and enlarged `primary-after-focus-crop.png`
- `primary-after-digit.png` and enlarged `primary-after-digit-crop.png`
- `primary-after-dot.png`
- `primary-after-backspace.png` and enlarged `primary-after-backspace-crop.png`
- `primary-done.png`
- `primary-sequence-composite.png` (the three crop images together)

### Earlier value list

Each value was typed through the on-screen keypad after clearing the row with keypad Backspace, then Done was tapped. The saved row and total were visible in the corresponding `case-*-saved.png` screenshot.

| Typed | Focused text after typing | Saved row amount | Saved tax due | Result |
|---|---:|---:|---:|---|
| `0.50` | `0.50` | `0.50` | `0.02` | PASS |
| `1234.56` | `1234.56` | `1,234.56` | `37.04` | PASS |
| `99999.99` | `99999.99` | `99,999.99` | `3,000.00` | PASS |
| `100000` | `100000` | `100,000.00` | `3,000.00` | PASS |
| `1000000.01` | `1000000.01` | `1,000,000.01` | `30,000.00` | PASS |

The exact row assertions were:

- `PT010 · 0.50 · 3.0 · 0.02`
- `PT010 · 1,234.56 · 3.0 · 37.04`
- `PT010 · 99,999.99 · 3.0 · 3,000.00`
- `PT010 · 100,000.00 · 3.0 · 3,000.00`
- `PT010 · 1,000,000.01 · 3.0 · 30,000.00`

After the last case the app process was stopped and reopened against the same guarded database. The row still showed `PT010 · 1,000,000.01 · 3.0 · 30,000.00`; see `reopened-persistent.png`.

### Recording

`keep-text-proof.mp4` is the screen recording with taps shown. Android `screenrecord` stops at its 180 second maximum; the 179.4 second video covers the seeding, the complete filled-`1,234.56` tap/digit/dot/Backspace sequence, and the first matrix cases through the start of `99999.99`. The remaining matrix cases and persistence reopen are covered by the screenshots and exact assertions in `android-keep-text-proof.log`.
