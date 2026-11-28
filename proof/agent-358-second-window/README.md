# agent-358-second-window — proof

Builds: app `1376e150eb964d3cae8aa957bcc970b0fb9e6dcb` (main), plug `db0e4ac6127078a05751149f0ec6bed68204f911` as on main unless noted, kit `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`, cargo 1.99.0 (rustup stable toolchain), each run in its own checkout with its own target dir. The fixed run builds the kit with `358-second-window-input.patch` applied. App runs used `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1` with a fresh scratch database and the demo taxpayer; nothing was sent to the BIR or any relay. The UI was driven only through gpui-agent; the one exception noted below.

## Result

A gpui-agent press on `print-save-pdf` in the print preview (the app's second window) reaches it with the kit patch: **yes — 3 of 3**; without it 0 of 10 at either plug revision on Windows.

## Cause

`WindowsNative::click` in `crates/nsh-platform-windows/src/agent.rs` dispatched the synthetic down and up in two separate main-thread turns. Activating the press's window makes Windows post a real mouse move where the physical cursor idles; delivered between the down and the up, it moved the press out of the pressed view past the press slop and cancelled it, so the up completed nothing. First-press-ever observations differ because a window that is already in front needs no re-activation, so no such move is posted.

## Files

- `358-second-window-input.patch` — the fix and its test, against kit pin 8c78cfd6; also applies cleanly to buwiz-next head 6f2afaa8e0e8041deb7533ea9ae0228dd717a62e.
- `press{1,2,3}-*.png/json`, `step1-run.mp4` — Step 1 at plug db0e4ac6, kit unpatched: the first press landed (the preview was already in front), the later two did not.
- `b0c1a5f2-press{1,2,3}-*`, `b0c1a5f2-summary.json`, `step1-b0c1a5f2.mp4` — Step 1 at plug b0c1a5f2, kit unpatched: 0 of 3. This build was made with `cargo update` and its lock downgraded `windows` 0.62.2 to 0.56.0 (see lock-diff-b0c1a5f2.txt); the clean hand-edited-lock rerun (lock-diff-b0c1a5f2-locked.txt, six plug source lines only) still gave 0 of 10, so the downgrade did not cause the failure.
- `fixed-press{1,2,3}-*`, `fixed-threepress.json`, `fixed-threepress.mp4` — the proof run with the kit patch: every press answered changed true and opened the save panel; the preview stayed in front of the main window. The save panel is an OS dialog outside the app and was cancelled with a synthesized Escape key between presses; no app control was touched except through gpui-agent.
- `tenpress/` — the ten-press measurements at db0e4ac6 and b0c1a5f2 (clean hand-edited lock, `--locked`-verified) without the kit fix: 0 of 10 each, answers and window lists per press, one recording per build. Note the panel column in these runs keyed on any #32770 window and is unreliable (the machine can show unrelated system dialogs); the changed column and the answer JSONs are the measurement.
- `test-fail-before.log`, `test-pass-after.log` — `cargo test` for the regression test `a_presss_down_and_up_are_one_main_thread_turn`: before the fix the test does not compile (the invariant it asserts did not exist — the down and up were two turns); after the fix it passes.
- `lock-diff-b0c1a5f2.txt`, `lock-diff-b0c1a5f2-locked.txt` — the two lock diffs described above.

## What these files do not show

- No file shows any real-mouse or real-keyboard interaction with the app; the single real click used during diagnosis is not part of the proof.
- The recordings do not capture cursor position (gdigrab without cursor); the window lists carry foreground state instead.
- The PNGs show the screen before/after each press of the fixed run only; the failed-run evidence is the answer JSONs, window lists and recordings.
