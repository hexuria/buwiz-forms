free disk: 2 GB

# e2e 507 machine proof

Driver: gpui-agent-cli 0.5.0. Pull request head throughout: `16dd1a3071080c329cb0d8e24005eb207304796f`; every built app report showed app commit `16dd1a30`. Plug pin: `db0e4ac6127078a05751149f0ec6bed68204f911`. Sibling NSH: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`. Before the runs the shared wrapper resolved `app_dir=$HOME/repos/j507/app/nsh`.

Safety env for every run: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; no release build and no live send. `--allow-send` was used only for the dev dry-run rehearsal.

Checks with gpui-agent-cli 0.5.0, all zero problems: `smoke.json` 12 recipes / 38 files; `mbprobe.json` 5 / 8; `rdo-row-press.json` 4 / 5; `sign-out-mock.json` 2 / 3.


### Linux run 1

| recipe | status | ms | first failing step and error | scrolled map |
|---|---|---:|---|---|
| setup-start | pass | 575 |  | — |
| sign-up | pass | 10006 |  | — |
| tax-year | pass | 6290 |  | — |
| navigate | pass | 1960 |  | — |
| open-2551q | pass | 1761 |  | — |
| money-boxes | pass | 13631 |  | — |
| sheet-press | skipped | 0 | no desktop variant | — |
| delete-row | setup | 34 | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} | — |
| queue | fail | 17055 | s27a — wait_until timed out: node `fp-pill-count` exists but should be absent | — |
| queue-send | skipped (needs --allow-send) | 0 | needs --allow-send | — |
| birth-date | pass | 5972 |  | — |
| sign-out | known gap | 11796 | s4 — wait_until timed out: node `locked-screen` not found | — |

### Linux run 2

| recipe | status | ms | first failing step and error | scrolled map |
|---|---|---:|---|---|
| setup-start | pass | 571 |  | — |
| sign-up | pass | 9743 |  | — |
| tax-year | pass | 6312 |  | — |
| navigate | pass | 2038 |  | — |
| open-2551q | pass | 1762 |  | — |
| money-boxes | pass | 13613 |  | — |
| sheet-press | skipped | 0 | no desktop variant | — |
| delete-row | setup | 38 | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} | — |
| queue | fail | 17175 | s27a — wait_until timed out: node `fp-pill-count` exists but should be absent | — |
| queue-send | skipped (needs --allow-send) | 0 | needs --allow-send | — |
| birth-date | pass | 6078 |  | — |
| sign-out | known gap | 11795 | s4 — wait_until timed out: node `locked-screen` not found | — |

### Android run 1

| recipe | status | ms | first failing step and error | scrolled map |
|---|---|---:|---|---|
| setup-start | pass | 3675 |  | — |
| sign-up | pass | 14105 |  | — |
| tax-year | pass | 12091 |  | — |
| navigate | pass | 5298 |  | — |
| open-2551q | pass | 5032 |  | — |
| money-boxes | pass | 25984 |  | — |
| sheet-press | skipped | 0 | no phone variant | — |
| delete-row | setup | 94 | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} | — |
| queue | fail | 31732 | s24 — occluded_by: software keyboard (a press on `node 10964` at (205.7143, 710.4762) lands on a key; hide the keyboard or scroll the target above it first) | — |
| queue-send | skipped (needs --allow-send) | 0 | needs --allow-send | — |
| birth-date | pass | 17524 |  | — |
| sign-out | known gap | 14289 | s4 — wait_until timed out: node `locked-screen` not found | — |

### Android run 2

| recipe | status | ms | first failing step and error | scrolled map |
|---|---|---:|---|---|
| setup-start | pass | 3196 |  | — |
| sign-up | pass | 13929 |  | — |
| tax-year | pass | 12214 |  | — |
| navigate | pass | 5354 |  | — |
| open-2551q | pass | 5139 |  | — |
| money-boxes | pass | 26347 |  | — |
| sheet-press | skipped | 0 | no phone variant | — |
| delete-row | setup | 93 | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} | — |
| queue | fail | 31193 | s24 — occluded_by: software keyboard (a press on `node 10964` at (205.7143, 710.4762) lands on a key; hide the keyboard or scroll the target above it first) | — |
| queue-send | skipped (needs --allow-send) | 0 | needs --allow-send | — |
| birth-date | pass | 17698 |  | — |
| sign-out | known gap | 14353 | s4 — wait_until timed out: node `locked-screen` not found | — |

## Queue findings

Linux queue with `--allow-send` did not pass. It failed at `s27a` after 1,012 ms: `wait_until timed out: node \`fp-pill-count\` exists but should be absent`. The failure screenshot and tree are in `linux-allow-send/09-queue/`. The tree node is not stale: `fp-pill-count` is enabled, visible at x=315 y=719 w=22 h=22, named `1 item needs attention`, with child text `1`; its parent is named `All sections · 1 to do · Page 1 · Return details · 1 item needs attention`. The screenshot visually shows the black banner `1 item needs attention.` and a red circular `1` in the bottom pill, so the page really shows a validation/attention count at that moment. Either the recipe expectation or the page state is wrong; the node is not stale.

The queue-send rehearsal failed. Step `s4` waited 20,043 ms and timed out: `wait_until timed out: node \`fp-status\` not found`. The screenshot remains on Form 2551Q with the same `1 item needs attention` state rather than showing `Dry run · read-only`.

Android sign-up passed in both runs; there was no s8 software-keyboard refusal. Android queue failed twice at s24 with the exact software-keyboard occlusion shown in the tables.

## Videos

`videos/android-queue-run1.mp4` is the gpui-agent runner's Android screen recording for the queue failure. `videos/linux-smoke-x11.mp4` is an observational X11 screen capture of a supplementary Linux smoke run; the runner's `--record` implementation creates recordings only for iOS/Android even though its comment mentions desktop recording. It was not used to drive or read the app.
