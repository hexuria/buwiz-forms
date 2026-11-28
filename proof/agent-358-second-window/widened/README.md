# Widened measurement of the kit fix hexuria/nsh#60 (hexuria/buwiz-forms#358)

driver: gpui-agent-cli 0.4.1

## Build lines

- Crate tests: hexuria/nsh branch fix/second-window-input at 32944c3c8e003072db1a699e7b3c8f667d883b82 (base buwiz-next 6f2afaa8), `cargo test -p nsh-platform-windows` with and without `--features dev`, one fresh CARGO_TARGET_DIR.
- App run: hexuria/buwiz-forms main 1376e150eb964d3cae8aa957bcc970b0fb9e6dcb, plug db0e4ac6 as on main (no rev change), kit hexuria/nsh at the app's pin 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf plus the patch `../358-second-window-input.patch` applied in a scratch clone, cargo 1.99.0, one fresh CARGO_TARGET_DIR inside the scratch checkout. App launched with `just nsh-run-agent windows <scratch db>` under BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1; fresh scratch database; demo taxpayer Juana Santos only (TIN 000-123-453, branch 00000, Individual, RDO 039, ZIP 1100). Setup ran through the repo's own recipes (setup-start, sign-up, tax-year, open-2551q-windows) via `python scripts/e2e windows juana-setup --no-launch`, all pass.

## What the run was

One app run. The print preview of a 2551Q draft was opened once. Then `print-save-pdf` was pressed 10 times in a row through gpui-agent only; before each press the preview was brought forward the way gpui-agent's CLI does it (a press on the preview's `print-zoom-fit` control). After each press the driver polled up to 30 s for the save panel — a `#32770` window titled "Save as PDF" owned by the app process — recorded the gpui-agent answer's `changed`, the panel, its hwnd and its GW_OWNER, and the foreground window, then cancelled the panel (it is the OS dialog, not the app: it was foregrounded and sent Escape with scan code 0x01; gpui-agent has no mechanism that reaches it, and no app control was touched by this). Then one press on `sb-new` (the sidebar's New-return row) in the MAIN window through gpui-agent.

## Results

- Preview presses: 10 of 10 `changed: true`, panel appeared 10 of 10.
- Main-window press: answer `ok: true, delivered_unverified, after.changed: false` — the pressed sidebar row itself does not change state; the visible effect did happen: screen-press11-after.png shows the "Start a return" picker open. `changed` here measures the target node, not the screen.
- Save panel GW_OWNER: in every row the panel's owner is the MAIN window (eBIRForms), never the print-preview window it was invoked from — see press-table.json (`owner` = the hwnd of the `eBIRForms` NshWindow, not the `BIR Form 2551Q …` one). This feeds hexuria/buwiz-forms#628.

## What each file shows — and does not

- `press-table.json` — the 11 rows (press #, window, changed, panel, panel hwnd + owner, foreground) plus the app's own top-level windows with their hwnds at the end of the run. It does not prove delivery beyond what the agent reports; it records answers, not intent.
- `answer-press<N>.json`, `answer-press<N>-raise.json`, `answer-open-preview.json`, `answer-press11*.json` — the raw gpui-agent answers. `delivered_unverified` means the host accepted the input but the read-back saw no target-node change; it is not a delivery guarantee.
- `windows-press<N>.json` — full visible-window enumeration after each press (hwnd, class, title, pid, GW_OWNER, foreground). These are the source for the panel-owner fact. They do not include invisible or owned-but-hidden windows.
- `screen-press<N>-before/after.png` — full-screen captures around each press (gpui-agent has no Windows window screenshots; these are the machine's own desktop captures). They show what a user would have seen; they do not prove which process produced the pixels.
- `run.mp4` — one ffmpeg gdigrab recording of the whole press sequence. Same caveat: it is the desktop, not a per-window capture.
- `test-plain.log`, `test-dev.log` — the two crate-test runs at the PR head. Plain: lib 342 passed / 0 failed / 1 ignored, doctests 2 passed. `--features dev`: lib 353 passed / 0 failed / 1 ignored (includes `agent::tests::a_presss_down_and_up_are_one_main_thread_turn`), doctests 2 passed. Both exit 0.
- `snapshot-before-main-press.json`, `snapshot-ids.json`, `recipe-*.log` — pre-press UI snapshot and setup-recipe logs, when present. The snapshot is the merged semantic tree as the agent reports it; it is not a complete view hierarchy.
