driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent b0c1a5f2e0a23d9b37a8c3059fac52c0c9feae0b (app b3bddd4d)


> **Environment-suspect (the wrapper pointed at another checkout), not evidence.**
# pin b0c1a5f2 Linux/Android proof

Evidence only; no source change is committed. The checkout was `b3bddd4d252d0daac05b322f18ce6e45e6c1fa18` with sibling NSH `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`. Safety env was `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; `--allow-send` was never passed. Text copied here has home prefixes written as `$HOME`.

## Setup

`gpui-agent --version` printed the host's nonfatal weak `GLIBC_2.39` line and then `gpui-agent-cli 0.5.0`. The setup's prebuilt installer reported `gpui-agent-cli already at 0.5.0`. The plug rev in `nsh/Cargo.lock` is `b0c1a5f2e0a23d9b37a8c3059fac52c0c9feae0b`.

`python3 scripts/tests/test_e2e.py`: 41 ran, 41 passed, 0 failed, 0 skipped.

`just e2e check smoke` before the Linux run: `smoke.json: 11 recipes, 30 files validated by gpui-agent-cli 0.5.0, 0 problem(s)`.

## Android recipe overlay

After the Linux run, recipes only were overlaid uncommitted from `74e3a8a2246fa8876d0fe86aceda35e4e1b83cdc` with `git checkout 74e3a8a2246fa8876d0fe86aceda35e4e1b83cdc -- agent-recipes/e2e`. In `money-boxes-paced-phone.json` only step `s13p` was deleted. `smoke.json` was restored from `b3bddd4d`, so money-boxes kept `"may_scroll": {"phone": ["s14"]}`. There was no smoke/recipe-file clash around that entry: the overlaid file exists and still has `s14`. The overlay check again reported `smoke.json: 11 recipes, 30 files validated by gpui-agent-cli 0.5.0, 0 problem(s)`.

The Android run nevertheless stopped in required `sign-up`: the Android-specific file selected by both smoke files is `sign-up-android.wants`, not the overlaid `sign-up-phone.json`, and `sign-up-android.wants` has no keyboard-hide step before `s8`. No recipe was edited to work around that.

## Linux report

Report files are in `linux/`. `report.json` has no `scrolled` field on any recipe.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---:|---|---|
| 1 | setup-start | pass | 1131 |  |  |
| 2 | sign-up | pass | 12245 |  |  |
| 3 | tax-year | pass | 6315 |  |  |
| 4 | navigate | pass | 2002 |  |  |
| 5 | open-2551q | pass | 1751 |  |  |
| 6 | money-boxes | pass | 13621 |  |  |
| 7 | sheet-press | skipped | 0 |  | no desktop variant |
| 8 | delete-row | setup | 31 |  | `$table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1}` |
| 9 | queue | refused | 0 |  | presses a send control: run with --allow-send |
| 10 | birth-date | pass | 6188 |  |  |
| 11 | sign-out | known gap | 11796 | s4 | `wait_until timed out: node \`locked-screen\` not found` |

Linux steps whose result shows `scrolled`: none.

## Android report

Report files and the two recordings are in `android/`. `report.json` has no `scrolled` field on any recipe.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---:|---|---|
| 1 | setup-start | pass | 3099 |  |  |
| 2 | sign-up | fail | 9065 | s8 | `occluded_by: software keyboard (a press on \`node 300\` at (205.90475, 742.2857) lands on a key; hide the keyboard or scroll the target above it first)` |
| 3 | tax-year | not run | 0 |  | sign-up failed and the suite needs it |
| 4 | navigate | not run | 0 |  | sign-up failed and the suite needs it |
| 5 | open-2551q | not run | 0 |  | sign-up failed and the suite needs it |
| 6 | money-boxes | not run | 0 |  | sign-up failed and the suite needs it |
| 7 | sheet-press | not run | 0 |  | sign-up failed and the suite needs it |
| 8 | delete-row | not run | 0 |  | sign-up failed and the suite needs it |
| 9 | queue | not run | 0 |  | sign-up failed and the suite needs it |
| 10 | birth-date | not run | 0 |  | sign-up failed and the suite needs it |
| 11 | sign-out | not run | 0 |  | sign-up failed and the suite needs it |

Android steps whose result shows `scrolled`: none. In particular, money-boxes `s14` was never reached, so the expected `scrolled: 1` measurement is absent. No step scrolled without `may_scroll` because no step scrolled at all.

## with-507-recipes

Part 2, using #507's whole recipe set including its smoke.json, is in `with-507-recipes/README.md`; it reaches the same Android sign-up keyboard blocker before money-boxes.
