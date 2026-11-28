driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent db0e4ac6 (app f030fbfd, job58 recipes)

# Job 58 recipes on gpui-agent 0.5.0

Evidence only. The app source and `scripts/e2e/runner.py` remained exactly `f030fbfd63a47cf2b322ffec6598122ad9fd6936`; only `agent-recipes/e2e` was overlaid locally from job 58 commit `6117e181bc87c7906d128cf84555c84fe204c5b9`. Those recipe edits were not committed. To keep the runner's app header exact, the runs used a detached worktree at `f030fbfd`; the build line says `(with uncommitted changes)` solely because of that local recipe overlay. The sibling NSH, CLI, plug, cargo, runtime and safety settings were the same as the parent proof.

The overlay adds `delete-row.json`, `sign-up-android.json` and `tax-year-android.json`, changes Android smoke to those JSON variants, updates `money-boxes-paced-phone.json` with the pill-clearing scroll, removes delete-row's resolver, and corrects stale known-gap text. `sign-up-android.json` hides the keyboard before the former failing step: `s7h` is `system keyboard_hide` immediately before `s8 set_value new-tp-registered_address $address` (and `s6h`/`s8h`/`s9` are also keyboard hides). Thus the #24/#27 keyboard-occlusion refusal did not recur.

`just e2e check smoke`: `smoke.json: 11 recipes, 30 files validated by gpui-agent-cli 0.5.0, 0 problem(s)`. The runner did not refuse any overlaid recipe for syntax, `needs_cli` or host features. The only refusal in the runs is Queue's required send-control refusal because `--allow-send` was deliberately absent.

## CLI version source

`E2E_GPUI_AGENT_CLI_VERSION` was not set in either smoke run. Exact command `cargo install --list | grep gpui-agent-cli` produced empty stdout and exit status 1 on this machine. The prebuilt installer copies the binary without registering it as a cargo source install, so `installed_cli_version()` fell through to `pinned_gpui_agent_version()` in the app `Cargo.lock`, where `gpui-agent` is `0.5.0`. See `cli-version-source.txt`.

## Linux table

Run header: app `f030fbfd`, gpui-agent-cli 0.5.0, host desktop, features `press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline`. Wall time 71 s with the already-populated target cache; compile lines were Rust crate 1.3 s, App.tsx 220 ms, Linux host 1.0 s. Totals: 8 pass, 1 known gap, 1 refused, 1 skipped — exactly job 58's final Linux rows.

| recipe | 0.5.0/db0e4ac6 result | job 58 final | difference |
|---|---|---|---|
| setup-start | pass | pass | none |
| sign-up | pass | pass | none |
| tax-year | pass | pass | none |
| navigate | pass | pass | none |
| open-2551q | pass | pass | none |
| money-boxes | pass | pass | none |
| sheet-press | skipped: no desktop variant | skipped: no desktop variant | none |
| delete-row | pass | pass | none |
| queue | refused: `presses a send control: run with --allow-send` | refused, same | none; nothing queued or sent |
| birth-date | pass | pass | none |
| sign-out | known gap, s4 `wait_until timed out: node \`locked-screen\` not found` | known gap, same | none |

## Android table

Run header: app `f030fbfd`, gpui-agent-cli 0.5.0, host mobile/phone, features `press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline`. Wall time 565 s including Android compile/install; compile lines were Rust crate 2m31s, App.tsx 268ms, x86_64 1m34s, armeabi-v7a 1m35s, arm64-v8a 1m29s, host 10.7s, install 7.4s. Totals: 8 pass, 1 known gap, 1 refused, 1 skipped — exactly job 58's final Android rows.

| recipe | 0.5.0/db0e4ac6 result | job 58 final | difference |
|---|---|---|---|
| setup-start | pass | pass | none |
| sign-up | pass | pass | none |
| tax-year | pass | pass | none |
| navigate | pass | pass | none |
| open-2551q | pass | pass | none |
| money-boxes | pass | pass | none |
| sheet-press | skipped: no phone variant | skipped: no phone variant | none |
| delete-row | pass | pass | none |
| queue | refused: `presses a send control: run with --allow-send` | refused, same | none; nothing queued or sent |
| birth-date | pass | pass | none |
| sign-out | known gap, s4 `wait_until timed out: node \`locked-screen\` not found` | known gap, same | none |

No row differs from job 58's final rows. Therefore there is no row-level difference to attribute to the 0.5.0 header or the plug. The visible driver difference is only in the run header: `press_within` and `assert_states` are now advertised on both targets. The keyboard-occlusion refusal from main's recipes did not occur because job 58's Android recipes hide the keyboard before the vulnerable low-field `set_value` steps.

## Signed-in empty Home

Android sign-up passed. I searched the saved receipts and tree JSON files without adding any capture step. None contains `welcome-add-taxpayer`; passing receipts contain step delivery records and screenshot references, not full trees, and the only saved trees in this run are failure trees. Therefore there is no existing signed-in empty-Home tree to preserve, and no basis for checking whether the gate's sign-in button id is absent there.

## Files

- `linux/`: report, run/app logs, receipts, failure evidence.
- `android/`: report, run/app logs, receipts, failure evidence and nine per-recipe `recording.mp4` files.
- `check-smoke.log`, `linux-just.log`, `android-just.log`, `cli-version-source.txt`.
