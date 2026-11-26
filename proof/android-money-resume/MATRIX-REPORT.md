# Android money Home/resume matrix

**Result: NOT REPRODUCED at e4ffb037 in valid activity-preserving Home→5s→recents returns.**

# Android money-box Home/resume Step 1 matrix

Commit: `e4ffb0370746db7ee9ec5850aa0483a653690162`  
Device: `emulator-5554`  
Package: `com.longbridge.nsh.host`  
Form/box: seeded scratch 2551Q draft, Schedule 1 row 1 Taxable Amount

## Verdict

**Home/resume money-box fault NOT reproduced in the valid activity-preserving returns.** In every focused case, the amount text before any post-resume key was unchanged, the amount `EditText` remained focused, `dumpsys input_method` still reported the keyboard shown, and after tapping Done the scratch draft held the expected amount/tax values.

## Return mechanism

Valid matrix returns used the real Android recents UI: Home key, five seconds, `KEYCODE_APP_SWITCH`, then a tap on the visible app snapshot card. The app process id was identical before and after each valid return.

A normal launcher-icon comparison was attempted separately: the `NshDev` dock icon was tapped at the Android launcher. The process id remained `4575` (no force-stop), but the development host launcher intent lacks the dev-server URL extra and the activity landed on `Connecting to ws://127.0.0.1:8790/ws…`; therefore normal launcher-icon return is not a useful activity-preserving return for this NSH development host and was excluded from the valid matrix. Evidence: `a2-home-screen.png`, `a2-after-resume-launcher-icon.png`.

## Matrix

| Case | Return | Before Home | After resume before any key | Keyboard/focus after resume | Draft after Done/leave | Result |
|---|---|---|---|---|---|---|
| A run 1 (prior valid) `1,234.56` | activity-preserving explicit `am start` without `-S` | `1,234.56`, amount focused, keyboard up | `1,234.56` | focused; keyboard up | `1234.56 / 37.04 / 37.04` | PASS/no repro |
| A run 2 `1,234.56` (`a2r`) | recents UI card | `1,234.56`, amount focused, keyboard up | `1,234.56` | focused; `mInputShown=true`, `mIsInputViewShown=true`; pid 4575→4575 | `1234.56 / 37.04 / 37.04` | PASS/no repro |
| A run 3 `1,234.56` (`a3valid`) | recents UI card | `1,234.56`, amount focused, keyboard up | `1,234.56` | focused; `mInputShown=true`, `mIsInputViewShown=true`; pid 5140→5140 | `1234.56 / 37.04 / 37.04` | PASS/no repro |
| A extra corroboration `1,234.56` (`a4valid`) | recents UI card | `1,234.56`, amount focused, keyboard up | `1,234.56` | focused; `mInputShown=true`, `mIsInputViewShown=true`; pid 5140→5140 | `1234.56 / 37.04 / 37.04` | PASS/no repro |
| B `12,345,678.90` (`bvalid`) | recents UI card | `12,345,678.90`, amount focused, keyboard up | `12,345,678.90` | focused; keyboard up; pid 5140→5140 | `12345678.9 / 370370.37 / 370370.37` | PASS/no repro |
| C fresh `1,234` (`cfresh`) | recents UI card | exact visible raw text `1234`, amount focused, keyboard up | exact visible raw text `1234` | focused; keyboard up; pid 5140→5140 | `1234.0 / 37.02 / 37.02` | PASS/no repro |
| D not-focused control (`dcontrol`) | recents UI card | row summary `PT010 · 1,234.00 · 3.0 · 37.02`; no amount `EditText` focused; visual keyboard down | same row summary; untouched | no amount focus; visual keyboard down; `mInputShown=false` (`mIsInputViewShown=true` still reported); pid 5140→5140 | `1234.0 / 37.02 / 37.02` | PASS/untouched |

Notes:
- For C, the UI did not group the in-progress digits: the exact visible text was `1234`, not `1,234`. After leaving/Done it became/saved the numeric value `1234.0`; row display used `1,234.00`.
- For C setup, the previously committed zero box was opened, its `0.00` was selected/deleted inside the edit session, then digits 1-4 were typed with real Android key events at about 120 ms and not left before Home.
- Early draft-copy attempts copied only the main SQLite file and missed SQLite WAL data. This was corrected during the run by copying `validation.db`, `validation.db-wal`, and `validation.db-shm`; corrected reads are used for B/C/D and `a4valid`. Baseline values were unchanged no-op values, so the earlier reads still matched the seeded/unchanged draft.
- No product code changes, no commits, no Queue, no live send, no token exposure.

## Key artifacts

### Consolidated
- Matrix summary JSON: `/home/ubuntu/forms-validation/android-home-proof/matrix-summary.json`
- Driver/helper evidence script: `/home/ubuntu/forms-validation/android-home-proof/matrix_driver.py`

### A run 1
- `/home/ubuntu/forms-validation/android-home-proof/valid1-before-home.png`
- `/home/ubuntu/forms-validation/android-home-proof/valid1-after-resume.png`
- `/home/ubuntu/forms-validation/android-home-proof/valid1-before-home.xml`
- `/home/ubuntu/forms-validation/android-home-proof/valid1-after-resume.xml`
- `/home/ubuntu/forms-validation/android-home-proof/after-valid1.db`

### A run 2
- `/home/ubuntu/forms-validation/android-home-proof/a2r-before-home.png`
- `/home/ubuntu/forms-validation/android-home-proof/a2r-after-resume-recents.png`
- `/home/ubuntu/forms-validation/android-home-proof/a2r-recents.png`
- `/home/ubuntu/forms-validation/android-home-proof/a2r-before-home.xml`
- `/home/ubuntu/forms-validation/android-home-proof/a2r-after-resume.xml`
- `/home/ubuntu/forms-validation/android-home-proof/a2r-keyboard-before.txt`
- `/home/ubuntu/forms-validation/android-home-proof/a2r-keyboard-after.txt`
- `/home/ubuntu/forms-validation/android-home-proof/a2r-draft.db`

### A run 3
- `/home/ubuntu/forms-validation/android-home-proof/a3valid-before-home.png`
- `/home/ubuntu/forms-validation/android-home-proof/a3valid-after-resume.png`
- `/home/ubuntu/forms-validation/android-home-proof/a3valid-recents.png`
- `/home/ubuntu/forms-validation/android-home-proof/a3valid-result.json`
- `/home/ubuntu/forms-validation/android-home-proof/a3valid-keyboard-before-home.txt`
- `/home/ubuntu/forms-validation/android-home-proof/a3valid-keyboard-after-resume.txt`
- `/home/ubuntu/forms-validation/android-home-proof/a3valid-draft.db`

### A extra corroboration
- `/home/ubuntu/forms-validation/android-home-proof/a4valid-before-home.png`
- `/home/ubuntu/forms-validation/android-home-proof/a4valid-after-resume.png`
- `/home/ubuntu/forms-validation/android-home-proof/a4valid-recents.png`
- `/home/ubuntu/forms-validation/android-home-proof/a4valid-result.json`
- `/home/ubuntu/forms-validation/android-home-proof/a4valid-keyboard-before-home.txt`
- `/home/ubuntu/forms-validation/android-home-proof/a4valid-keyboard-after-resume.txt`
- `/home/ubuntu/forms-validation/android-home-proof/a4valid-draft.db`

### B
- `/home/ubuntu/forms-validation/android-home-proof/bvalid-before-home.png`
- `/home/ubuntu/forms-validation/android-home-proof/bvalid-after-resume.png`
- `/home/ubuntu/forms-validation/android-home-proof/bvalid-recents.png`
- `/home/ubuntu/forms-validation/android-home-proof/bvalid-result.json`
- `/home/ubuntu/forms-validation/android-home-proof/bvalid-keyboard-before-home.txt`
- `/home/ubuntu/forms-validation/android-home-proof/bvalid-keyboard-after-resume.txt`
- `/home/ubuntu/forms-validation/android-home-proof/bvalid-draft.db`

### C
- `/home/ubuntu/forms-validation/android-home-proof/cfresh-before-home.png`
- `/home/ubuntu/forms-validation/android-home-proof/cfresh-after-resume.png`
- `/home/ubuntu/forms-validation/android-home-proof/cfresh-recents.png`
- `/home/ubuntu/forms-validation/android-home-proof/cfresh-result.json`
- `/home/ubuntu/forms-validation/android-home-proof/cfresh-keyboard-before-home.txt`
- `/home/ubuntu/forms-validation/android-home-proof/cfresh-keyboard-after-resume.txt`
- `/home/ubuntu/forms-validation/android-home-proof/cfresh-draft.db`

### D
- `/home/ubuntu/forms-validation/android-home-proof/dcontrol-before-home.png`
- `/home/ubuntu/forms-validation/android-home-proof/dcontrol-after-resume.png`
- `/home/ubuntu/forms-validation/android-home-proof/dcontrol-recents.png`
- `/home/ubuntu/forms-validation/android-home-proof/dcontrol-result.json`
- `/home/ubuntu/forms-validation/android-home-proof/dcontrol-keyboard-before-home.txt`
- `/home/ubuntu/forms-validation/android-home-proof/dcontrol-keyboard-after-resume.txt`
- `/home/ubuntu/forms-validation/android-home-proof/dcontrol-draft.db`

### Launcher-icon comparison (excluded from valid matrix)
- `/home/ubuntu/forms-validation/android-home-proof/a2-home-screen.png`
- `/home/ubuntu/forms-validation/android-home-proof/a2-after-resume-launcher-icon.png`
- `/home/ubuntu/forms-validation/android-home-proof/a2-pid-before.txt`
- `/home/ubuntu/forms-validation/android-home-proof/a2-pid-after.txt`
