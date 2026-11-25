# Job 23 — #364: the admin PIN page opens with the keyboard up

**App commit:** `2c9c16fd66b4e6a45ceb25a0291f68cb159ad822` (PR #364). **Sibling NSH:** `8c78cfd6` (real worktree `~/j23/nsh`). **Tool:** `nsh 0.6.1 (8c78cfd)` (`tools-8c78cfd-bak`, run in docker). **Runtime classes.dex sha256 from the APK:** `a53652883f334dbb2844bd2844fa5246097d2f49acddf068341c43702c46943c` (clean pin; re-hashed after packaging). **CARGO_TARGET_DIR:** `~/BuwizNsh/target-2c9c16fd`. Baked: `EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1` (scratch database, as the spec allows). Phone API 35, gesture mode, 150 ms hold-taps; sign-in is this era's device flow against the mock with `--auto-approve`.

## Results — three passes, because the first two disagreed

| pass | path | mInputShown (no tap, after wait) | digits fill the cells? |
|---|---|---|---|
| 1. first run | fresh install → sign-in → "I help other taxpayers" | **false** (~8–13 s after navigation; `pin-firstrun.png` shows no keyboard, empty cells) | no — the typed digits landed nowhere (`pin-typed.png`) |
| 2. force-stop, reopen | same flow again | **true** — numeric keyboard up in `pin-second-typed.png` | **yes — all four cells filled blind** (dots visible, keyboard up) |
| 3. force-stop, reopen again | same flow again | **true** (`pin-third-pass.png` shows the numeric keyboard: pink backspace/blue confirm keys) | — |

Honest verdict: the fix works — the keyboard rises by itself and blind typing lands in the cells — **but not every time**: one of three measured launches stayed down with the same scripted timing. Intermittent, consistent with a focus/window-timing race at mount. Where it matters: on the failure pass the person must tap the cells exactly as before #364.

Method notes: uiautomator dumps right after navigation are sometimes STALE (they showed the previous page while the keyboard was already up); screenshots and `dumpsys input_method` are the reliable evidence. The scratch database wipes on force-stop, so each pass re-walks sign-in → first-run choice; per the spec that's fine here. Video `j23.mp4` covers the passes continuously from the fresh install.
