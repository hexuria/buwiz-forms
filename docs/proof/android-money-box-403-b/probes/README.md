# Probes on job 41's findings (forms-validation follow-ups)

Same build as job 41: `7d491c57`, still installed on the emulator (the APK file itself was cleaned after the push). Same 2551Q Schedule 1 row 1 Taxable Amount. Network still cut, never Queue.

## 1. F3 — Home → 5 s → back with a focused, filled box

Four trials, identical protocol (box holds committed 1,234.56, refocused, Home key, 5 s away, relaunch; screenshot before any key):

| trial | box after return | focused | keyboard |
|---|---|---|---|
| 1 | `1,234.56` — unchanged | yes | up |
| 2 | **`123.46`** — corrupted | yes | up |
| 3 | `1,234.56` — unchanged | yes | up |
| 4 | **`123.46`** — corrupted | yes | up |

So: intermittent, roughly half the trials. The corruption drops digits: `1,234.56` (digits 1-2-3-4-5-6) becomes `123.46` (1-2-3-**4-6** — the 5 gone, the point shifted one left); job 41's original run showed `12.35` (digits 1-2-**3-5** — the 4 and 6 gone). In every trial the box is still focused with the keyboard up on return, and the damage is present before any key is pressed (`probe1-f3-return.png`, `probe1-trial2-corrupt.png`, `probe1-trial4-corrupt.png`). Exact text as read from the EditText node: `123.46`, not `12.3456` or anything with a dangling digit.

## 3. D1 — device facts and caret probes

Device (`device.txt`): AVD `buwiz_api35`, system image `system-images/android-35/google_apis/x86_64/`, `ro.build.version.sdk` = **35**, default IME `com.google.android.inputmethod.latin` (Gboard) versionName **14.2.09.629370537-preload-x86_64**. (The money box's own keypad is the app's custom one, not Gboard.)

- **Long-press the filled 1,234.56: nothing selects.** No Cut/Copy/Paste/Select-all menu appears, no visible selection (`probe3-longpress.png`). A digit after the long-press is simply inserted at the caret — `9` produced `91,234.56` (`probe3-after9.png`). No select-all on long-press either.
- **Tap on a middle digit:** tapping inside the box at a position over the middle of the text (x=866 of a box spanning 110–1007, right where the `4` region of the right-aligned `1,234.56` sits) — the caret lands at the **start** of the text regardless. Digit `7` then prepends: `71,234.56` (`probe3-midtap2.png`). Tap position inside the box does not affect caret placement in this build.

## At main 44ccfd80 (job 43's BEFORE build), same box

### 2. F1 on main — the keypad comma

Box zeroed, keypad taps `1 5 , 5 0`, reading after each key (`probe2-after-keys.png`):

| key | box |
|---|---|
| 1 | `10.00` |
| 5 | `150.00` |
| , | `150.00` — **the comma inserts nothing** |
| 5 | `1,550.00` |
| 0 | `15,500.00` |

After Done the row summary reads `PT010 · 15,500.00 · 3.0 · 465.00` (`probe2-rowsummary.png`): the user typed 15,50-intent and the draft holds **15,500.00**, no error. On main the comma is dead exactly as at 7d491c57 — and main additionally reorders every digit (the prepend-before-zero cascade: `1` → `10.00` on the very first key), so even without the comma the amount never matches intent.

### 1b. F3 on main — Home → 5 s → back, focused box

Box holding a committed 15,500.00, refocused (keyboard up), Home key, 5 s away, relaunch — three trials, screenshot before any key (`probe1b-main-return.png`):

| trial | box after return | focused | keyboard |
|---|---|---|---|
| 1 | `15,500.00` — unchanged | yes | up |
| 2 | `15,500.00` — unchanged | yes | up |
| 3 | `15,500.00` — unchanged | yes | up |

**Main does not rewrite the amount.** The focus survives the round trip exactly as it was, three of three — so the intermittent rewrite measured at 7d491c57 (2 of 4 trials corrupted) is new in the reworked head, not inherited behavior. Raw log: `mainprobes.txt`.
