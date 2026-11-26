# Android code-box four-hour focus watch — clean

App commit: `8a507e47d96e5799c160fe6dbabcd7077e72cdec` on `main`.
Emulator: Android 15/API 35, `sdk_gphone64_x86_64`, image `system-images/android-35/google_apis/x86_64/`, ABI `x86_64`, AVD `buwiz`, device `emulator-5554`.
Build/run environment: `BUWIZ_ANDROID_DEV_ENV='EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1'`; only the loopback mock account server; `adb reverse tcp:3008 tcp:3008` and `tcp:8792`; no real account; no cold boot during the watch.

## Verdict

CLEAN. No bad state appeared during the four-hour watch. Sixteen tap checks were logged (eight at group centre `(540,650)` and eight at box 1 centre `(204,652)`). Every check focused `android.widget.EditText`, had `inputShown=true`, and accepted the one test digit (`focus_ok=True digit_ok=True`). No force-stop, reinstall, email-first, long-press, or paste probe was needed because the bad state never occurred.

The ordered endpoint was `2026-11-26T10:15:00Z`. The last scheduled pair ran at 10:09/10:10 with emulator uptime about 3:57/3:58. A final manual pair at 10:17 with emulator uptime about 4:04/4:05 was also clean; the watcher was then stopped. It was never cold-booted.

## Uptime cadence in `watch.log`

- first watch check: emulator uptime `18 min`, clean at both coordinates
- later clean pairs: about `51 min`, `1:33`, `2:04/2:05`, `2:37/2:38`, `3:14/3:15`, `3:57/3:58`
- final manual pair: about `4:04/4:05`, clean at both coordinates

Every log line contains the full `adb shell uptime` output, focus class, EditText bounds/flags, `mInputShown`, digit result, and post-digit EditText state.

## What ran before the first bad state in the earlier probe

From the machine-setup evidence: the same emulator image had been booted with KVM, the Android development host app was built and installed, the loopback mock-account server was started, `adb reverse` was established, and the first screen was exercised/recorded. No other app installation, rotation, or deliberate background/foreground cycle is documented in the evidence before the original pre-cold failure. The user-supplied timing context was that the first bad state appeared after a little under three hours of emulator uptime. The original pre-cold/post-cold evidence remains in the parent directory.

## Files

- `watch.log` — complete uptime/focus/digit log
- `watch.py` — watcher script used for scheduled checks and ordered bad-state capture logic
- `STATUS` — final `clean`
- `final-screen.png` — final screen capture after the endpoint
- `final-group-centre-after-tap.xml`, `final-box1-centre-after-tap.xml` — final uiautomator dumps
