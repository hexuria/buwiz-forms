driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent db0e4ac6 (app f030fbfd)

# gpui-agent 0.5.0 verification

Evidence only; no app source, recipe, kit or plug change is in this branch. Every app run used `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, a scratch database and no `--allow-send`. Android was driven only by gpui-agent recipes; adb was used only by the runner/runtime for install, forward, close and screenrecord.

## Toolchain and pins

| fact | value |
|---|---|
| app commit | `f030fbfd63a47cf2b322ffec6598122ad9fd6936` |
| sibling `../nsh` / NSH pin | `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` |
| `nsh --version` | `nsh 0.6.1 (8c78cfd)` |
| Android runtime classes.dex sha256 | `a53652883f334dbb2844bd2844fa5246097d2f49acddf068341c43702c46943c` |
| cargo used by the recipes | `cargo 1.99.0 (5f94df478 2026-08-27)` at `/home/ubuntu/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/bin/cargo` |
| plug rev in `nsh/Cargo.lock` | `db0e4ac6127078a05751149f0ec6bed68204f911` |
| plug rev in build dep-info | `db0e4ac` checkout paths in `plug-depinfo.txt` |

The host is Ubuntu 22.04 without GTK4, so the Linux commands ran through the repo's Ubuntu 24.04 `nsh2404.sh` wrapper. The job-local `nsh` shim used the real pinned binary directly inside that container and the wrapper outside it; this was machine setup only, not a source change. `just nsh-test` was run inside the wrapper with `XDG_RUNTIME_DIR=/tmp` because the container's `/run/user/1000` is root-owned.

## gpui-agent-cli 0.5.0 install

| fact | value |
|---|---|
| setup route | `scripts/setup-linux.sh` → `scripts/lib/prebuilt.sh` → prebuilt download |
| archive | `gpui-agent-cli-x86_64-unknown-linux-gnu-v0.5.0.tgz` |
| pinned archive sha256 | `622400b4236ebc9313c18b3a9c6946aa5aba873b91873ffe240060bcd5730505` |
| digest check | archive sha256 matched the pin; no source-build fallback |
| installed binary sha256 stamp | `68da5f4024df300aeabb69c7535adf2873ceb982abbd57c1dbf0d3442a3b8fc7` |
| wall time | 0 s by whole-second timestamps (under 1 s) |
| `gpui-agent --version` | `gpui-agent-cli 0.5.0` |

On Ubuntu 22.04 the prebuilt binary first prints the nonfatal weak-version line `/lib/x86_64-linux-gnu/libc.so.6: weak version 'GLIBC_2.39' not found (required by gpui-agent)`, then answers 0.5.0 and exits 0.

## Setup/unit checks

| command | result |
|---|---|
| `python3 scripts/tests/test_prebuilt_setup.py` | 34 tests, OK, 6 skipped, 0.888 s |
| `python3 scripts/tests/test_e2e.py` | 36 tests, OK, 0 skipped, 0.625 s |
| `just e2e check smoke` | `smoke.json: 11 recipes, 30 files validated by gpui-agent-cli 0.5.0, 0 problem(s)` |

No recipe in either Linux or Android smoke was skipped for `needs_cli`; both recipes with `needs_cli: 0.4.1` ran or were otherwise reported on their merits.

## `just nsh-check`

Linux exit 0, wall time 225 s:

```text
✓ checked nsh-app-ebirforms (1m 16s, 1 warning: -v shows it)
✓ built the Rust crate nsh-app-ebirforms (2m 20s, 1 warning: -v shows it)
```

`nsh check --help` at this head has no `--target` option, so there is no Android-target variant of this recipe to run (`nsh-check-target-note.txt`).

## `just nsh-test`

Exit 0, wall time 3,518 s. Final line: `nsh-test: every test file passed (nsh/verify left out)`.

| part | files/summaries | passed | failed | ignored |
|---|---:|---:|---:|---:|
| Rust `cargo test` | 9 result summaries | 168 | 0 | 0 |
| TS/TSX files | 212 files | 2,164 | 0 | 0 |
| combined | — | 2,332 | 0 | 0 |

No failing test name: the recipe's failure list was empty.

## Linux smoke (`linux/report.json`, `linux/report.md`)

Run header: app `f030fbfd`, gpui-agent-cli 0.5.0, host desktop, features `press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline`. Wall time 209 s. The `just` exit was 1 because the report contains a known gap/refusal/setup row.

| recipe | job 60 result | exact step/error or note | job 58 at b559e40d/CLI 0.4.1 | change |
|---|---|---|---|---|
| setup-start | pass | | pass | none |
| sign-up | pass | | pass | none |
| tax-year | pass | | pass | none |
| navigate | pass | | pass | none |
| open-2551q | pass | | pass | none |
| money-boxes | pass | | pass | none |
| sheet-press | skipped | no desktop variant | skipped, no desktop variant | none |
| delete-row | setup | `$table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1}` | pass | regression: resolver never finds the Schedule 1 table in the unchanged recipe |
| queue | refused | `presses a send control: run with --allow-send` | refused, same | none; nothing queued or sent |
| birth-date | pass | | pass | none |
| sign-out | known gap | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot`: `wait_until timed out: node \`locked-screen\` not found` | known gap, same error | none in outcome; the suite's Linux known-gap text still talks about a birth-date sheet even though birth-date passed |

Linux totals: 7 pass, 1 known gap, 1 refused, 1 skipped, 1 setup. Job 58 Linux totals were 8 pass, 1 known gap, 1 refused, 1 skipped; the only row outcome change is delete-row pass → setup.

## Android smoke (`android/report.json`, `android/report.md`)

Run header: app `f030fbfd`, gpui-agent-cli 0.5.0, host mobile/phone, features `press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline`. Wall time 476 s including the Android build/install. The `just` exit was 1 because sign-up failed.

| recipe | job 60 result | exact step/error or note | job 58 at b559e40d/CLI 0.4.1 | change |
|---|---|---|---|---|
| setup-start | pass | | pass | none |
| sign-up | fail | s8 `set_value new-tp-registered_address $address`: `occluded_by: software keyboard (a press on \`node 300\` at (205.90475, 742.2857) lands on a key; hide the keyboard or scroll the target above it first)` | pass | regression/new strict refusal: the unchanged recipe does not hide the keyboard; 0.5.0/db0e4ac6 refuses the occluded press |
| tax-year | not run | sign-up failed and the suite needs it | pass | blocked by sign-up |
| navigate | not run | sign-up failed and the suite needs it | pass | blocked by sign-up |
| open-2551q | not run | sign-up failed and the suite needs it | pass | blocked by sign-up |
| money-boxes | not run | sign-up failed and the suite needs it | known gap in the original full run (`occluded_by: 5584`); pass only after job 58's recipe-only follow-up | no 0.5.0 measurement: blocked before this row |
| sheet-press | not run | sign-up failed and the suite needs it | skipped, no phone variant | blocked before its ordinary skip |
| delete-row | not run | sign-up failed and the suite needs it | pass | blocked by sign-up |
| queue | not run | sign-up failed and the suite needs it | refused without `--allow-send` | blocked before its ordinary refusal; nothing queued or sent |
| birth-date | not run | sign-up failed and the suite needs it | pass | blocked by sign-up |
| sign-out | not run | sign-up failed and the suite needs it | known gap, `locked-screen` timeout | blocked by sign-up |

Android totals: 1 pass, 1 fail, 9 not run. Job 58 Android original full-run totals were 7 pass, 2 known gap, 1 refused, 1 skipped (money-boxes later passed only after job 58's recipe-only follow-up). Because sign-up is required and failed at f030, the rows after it are not evidence about those flows at 0.5.0.

## Signed-in empty Home check

No saved Android receipt or tree in this run contains `welcome-add-taxpayer`. The run stopped in sign-up before a taxpayer was saved, so there is no existing signed-in empty Home tree to keep and no basis for checking whether the gate's sign-in button id is absent there. No capture step or recipe step was added.

## Evidence files

- `linux/`, `android/`: runner `report.json`, `report.md`, `run.log`, `app.log`, receipts, failure trees/screenshots; Android also has per-recipe `recording.mp4` files.
- `linux-just.log`, `android-just.log`: outer `just e2e` output and exit summaries.
- `check-smoke.log`: `just e2e check smoke`.
- `nsh-check-linux.log`, `nsh-check-target-note.txt`.
- `nsh-test.log`.
- `test-prebuilt.log`, `test-e2e.log`.
- `tool-facts.txt`, `plug-depinfo.txt`.
