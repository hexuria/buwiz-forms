driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent b0c1a5f2e0a23d9b37a8c3059fac52c0c9feae0b (app b3bddd4d)


> **Environment-suspect (the wrapper pointed at another checkout), not evidence.**
# pin b0c1a5f2 with #507's whole recipe set

Part 2 evidence. The worktree was clean at `b3bddd4d252d0daac05b322f18ce6e45e6c1fa18`; all of `agent-recipes/e2e`, including `smoke.json`, was overlaid uncommitted from `74e3a8a2246fa8876d0fe86aceda35e4e1b83cdc`. In that overlay only, step `s13p` was deleted from `money-boxes-paced-phone.json` and `"may_scroll": {"phone": ["s14"]}` was added to the money-boxes entry of the overlaid `smoke.json`. `just e2e check smoke` passed: `smoke.json: 12 recipes, 37 files validated by gpui-agent-cli 0.5.0, 0 problem(s)`.

Safety env was `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; `--allow-send` was never passed. Copied text has home prefixes written as `$HOME`.

## Linux report

`report.json` has no `scrolled` field on any recipe.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---:|---|---|
| 1 | setup-start | pass | 568 |  |  |
| 2 | sign-up | pass | 11307 |  |  |
| 3 | tax-year | pass | 6454 |  |  |
| 4 | navigate | pass | 1868 |  |  |
| 5 | open-2551q | pass | 1804 |  |  |
| 6 | money-boxes | pass | 13650 |  |  |
| 7 | sheet-press | skipped | 0 |  | no desktop variant |
| 8 | delete-row | setup | 27 |  | `$table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1}` |
| 9 | queue | fail | 17200 | s27a | `wait_until timed out: node \`fp-pill-count\` exists but should be absent` |
| 10 | queue-send | refused | 0 |  | presses a send control: run with --allow-send |
| 11 | birth-date | pass | 6034 |  |  |
| 12 | sign-out | known gap | 11790 | s4 | `wait_until timed out: node \`locked-screen\` not found` |

Linux steps whose result shows `scrolled`: none. No `error_code: scrolled` failure occurred.

## Android report

`report.json` has no `scrolled` field on any recipe.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---:|---|---|
| 1 | setup-start | pass | 3302 |  |  |
| 2 | sign-up | fail | 8918 | s8 | `occluded_by: software keyboard (a press on \`node 300\` at (205.90475, 742.2857) lands on a key; hide the keyboard or scroll the target above it first)` |
| 3 | tax-year | not run | 0 |  | sign-up failed and the suite needs it |
| 4 | navigate | not run | 0 |  | sign-up failed and the suite needs it |
| 5 | open-2551q | not run | 0 |  | sign-up failed and the suite needs it |
| 6 | money-boxes | not run | 0 |  | sign-up failed and the suite needs it |
| 7 | sheet-press | not run | 0 |  | sign-up failed and the suite needs it |
| 8 | delete-row | not run | 0 |  | sign-up failed and the suite needs it |
| 9 | queue | not run | 0 |  | sign-up failed and the suite needs it |
| 10 | queue-send | not run | 0 |  | sign-up failed and the suite needs it |
| 11 | birth-date | not run | 0 |  | sign-up failed and the suite needs it |
| 12 | sign-out | not run | 0 |  | sign-up failed and the suite needs it |

Android steps whose result shows `scrolled`: none. No `error_code: scrolled` failure occurred. Money-boxes `s14` was not reached, so the expected `scrolled: 1` measurement is absent.

The blocker is unchanged with #507's whole recipe set: its `smoke.json` still gives sign-up an `android` variant, and the runner chooses that target-specific `sign-up-android.wants` before the phone variant. `sign-up-android.wants` is the same file as at #584's head and has no keyboard-hide step before `s8`.
