# Job 19 — #363 engine lifecycle on Android, option (c): no persistent database

**lock and double-send not covered on Android**

**App commit:** `38f475f9bf7e13fb42bee5beca6dc3b15dc6d239` (feat/relay-filing-lifecycle, review-1 fixes). **Sibling NSH:** `8c78cfd6` (real worktree `~/j19/nsh`; the pin per `NSH_PINNED_REF` in that commit's nsh.yml). **Tool:** `nsh 0.6.1 (8c78cfd)` (`tools-8c78cfd-bak`). **Runtime classes.dex sha256 from the APK:** `a53652883f334dbb2844bd2844fa5246097d2f49acddf068341c43702c46943c` (re-hashed after packaging). **CARGO_TARGET_DIR:** `~/BuwizNsh/target-38f475f9`. Baked: `EBIR_TEST_ENV=1;BIR_AUTH_BYPASS=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1` (scratch database per engine — option (c), no persistent database). Phone emulator API 35, gesture mode, 150 ms hold-taps.

## Crash-signal watch — all zero

Across the install, initial setup and all six Back-exit/relaunch cycles (`logcat-full.log`, 16 431 lines):

| signal | count |
|---|---|
| SIGABRT | 0 |
| SIGTRAP | 0 |
| "aborting" | 0 |
| "panicked" (incl. "panicked while processing panic") | 0 |
| "Fatal signal" / FATAL EXCEPTION | 0 |
| `am_proc_died` / `am_kill` for dev.goldcoders.bir | 0 |

The process was never killed and never restarted: `pidof dev.goldcoders.bir` read **6819** after every Back-exit and after every relaunch, unchanged from first launch to the end of the session.

## Per-cycle table

| cycle | Back presses to launcher | pid after Back | pid after relaunch | screen on relaunch |
|---|---|---|---|---|
| 1 | 4 (first cycle backed out through the in-app stack) | 6819 | 6819 | first-run question, NOT Home |
| 2 | 1 | 6819 | 6819 | first-run question |
| 3 | 1 | 6819 | 6819 | first-run question |
| 4 | 1 | 6819 | 6819 | first-run question |
| 5 | 1 | 6819 | 6819 | first-run question |
| 6 | 1 | 6819 | 6819 | first-run question |

**What the cycles show, stated plainly:** the process survives every Back-exit (pid constant, zero deaths) — but every relaunch lands on the first-run question "Who will use eBIRForms on this device?", never on Home, even though setup (first-run choice + a created taxpayer, MariaClara 123-456-789) was completed before cycle 1. Under option (c)'s scratch-per-engine databases this is exactly the blind spot the label warns about: a relaunch boots a fresh engine with a fresh empty scratch database, so first-run is what a correct lifecycle *looks like* here; whether the engine itself restarted or was never re-entered cannot be told apart without a persistent database. That distinction is deferred to the later dedicated job, as ordered.

**The app answers normally after every relaunch:** each relaunch rendered the first-run page fresh and responsive; after the final cycle a tap on "Just me or my business" advanced to the New taxpayer form (`final-answers.png`). Tab-tap evidence per the original script does not exist for cycles 2–6 because the first-run page has no tabs — the relaunch never reached a page that has any.

## Method notes

- Cycle 1 needed 4 Back presses because the first Back presses popped in-app pages (taxpayer Forms page etc.) before the root was reached; cycles 2–6 exited with a single press. A dedicated single clean cycle (Back out from the New taxpayer page → launcher → relaunch) is the video `j19-cycle.mp4`; `j19.mp4` is the longer screenrecord covering the first cycles (3-minute cap).
- `logcat-excerpt.txt` is the tail of the full log with noise packages filtered; the full log is alongside.
- The first-run choice at this build bypasses sign-in entirely (BIR_AUTH_BYPASS=1): the welcome gate never appeared, the app opened directly at "Who will use eBIRForms".

## Evidence

`cycle0-home.png` (Home with the created taxpayer before the cycles), `c1..c6-relaunch.png`, `c1..c6-tab.png` (cycle-1-era tab interactions), `final-answers.png`, `final-cycle-relaunch.png`, `j19-cycle.mp4` (one clean Back-exit + relaunch), `j19.mp4`, `logcat-full.log`, `logcat-excerpt.txt`.
