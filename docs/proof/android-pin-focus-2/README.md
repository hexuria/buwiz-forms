# Job 25 — #364 PIN focus retries at a3aaa36f on Android

**App commit:** `a3aaa36faf82a6a0d6ff795d99f27683a915ef58` (PR #364 head: PIN cells ask for focus at once, then again at 0.5, 1.5 and 3 s until they have it). **Sibling NSH:** `8c78cfd6` (real worktree `~/j25/nsh`). **Tool:** `nsh 0.6.1 (8c78cfd)` (`tools-8c78cfd-bak`, docker). **Runtime classes.dex sha256 from the APK:** `a53652883f334dbb2844bd2844fa5246097d2f49acddf068341c43702c46943c` (clean pin, re-hashed from the packaged APK). **CARGO_TARGET_DIR:** `~/BuwizNsh/target-a3aaa36f`. Baked: `EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1`. Phone emulator API 35, gesture mode, 150 ms hold-taps.

## Headline result: the keyboard NEVER self-rose — in any pass, including the zero-interference control

| pass | conditions | mInputShown ~5–10 s, no tap | blind digits fill cells | focus ring |
|---|---|---|---|---|
| 1 | fresh boot (uptime 25 s, `mCurMethodId=LatinIME`, `mInputShown=false` recorded before launch), fresh install, first launch | **false — never rose in the full 10 s watch** | no — cells empty (`p1-pin-typed.png`) | none visible |
| 2 | force-stop, reopen, re-walk | **false — never in 10 s** | no | none |
| 3 | force-stop, reopen, re-walk | **false — never in 10 s** | no | none |
| control | force-stop, reopen, navigation verified to the choice row, then **zero adb interaction for 12 s** (rules out uiautomator/dumpsys interference in the 0–3 s retry window) | **false** (`control-pin-12s.png`: PIN page, empty cells, no keyboard) | — | none |

**This is worse than job 23 at 2c9c16fd**, where passes 2–3 self-rose and blind digits landed. Same emulator, same scripted timing, same hold-taps: at a3aaa36f no pass ever raised the keyboard, and blind typing landed nowhere every time. Reported plainly as measured: on this emulator the retry fix does not produce a self-rising keyboard at all. The retry schedule (0/0.5/1.5/3 s) either never fires its focus win or the window never counts as focusable here; the measurement can't separate those, but it is not a timing marginality — four passes, ten-plus seconds watched each, zero rises.

## Timing: launch to PIN page

Pass 1 wall clock from the launch intent: welcome visible at t+11.7 s; PIN page detected at t+41.1 s. The walk is scripted (hold-taps with verification dumps between), so most of the ~29 s walk is harness latency, not app latency. The continuous recording `j25.mp4` covers from before the pass-1 launch through the passes for frame-level checking.

## Extra check: Back within 1 s of arrival

- **System Back** on the PIN page exits the app to the launcher (twice reproduced: `backcheck-after-back` shows the launcher). If the choice page is expected underneath in the back stack, it is not there for the system Back — stated as measured.
- **In-app Back control** lands on the choice page correctly (`backcheck2-after-back.png`). Over the next 4 s: `mInputShown` false at 1, 2 and 4 s — **no keyboard ever appears over the choice page**, and no visible focus ring anywhere (`backcheck2-choice-final.png`; uiautomator shows one node with focused=true but nothing visible in the screenshot).

## Evidence

`p1-pin-before-type.png`, `p1-pin-typed.png`, `p1-pin-10s.png`, `p2-pin-10s.png`, `p2-pin-typed.png`, `p3-pin-10s.png`, `p3-pin-typed.png`, `control-pin-12s.png`, `backcheck-pin-before-back.png`, `backcheck-after-back.png`, `backcheck2-after-back.png`, `backcheck2-choice-final.png`, video `j25.mp4`.

## Method notes

- At this commit the welcome gate's "Sign in" navigates straight to the first-run choice in the test environment (no email step), so pass 1 stayed genuinely cold — no field was ever touched before the PIN page.
- uiautomator dumps can be stale right after navigation; all keyboard claims rest on `dumpsys input_method` plus the screenshots, not dumps.
- The 'I help other taxpayers' row sometimes needs two hold-taps; walks verified landing on the PIN page by dump, screenshots confirm.
