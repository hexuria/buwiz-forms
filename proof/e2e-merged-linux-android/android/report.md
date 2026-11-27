# e2e smoke on android

App 7d155e6d, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 3312 |  |  |
| 2 | sign-up | pass | 13623 |  |  |
| 3 | tax-year | pass | 11119 |  |  |
| 4 | navigate | pass | 5203 |  |  |
| 5 | open-2551q | pass | 4866 |  |  |
| 6 | money-boxes | known gap | 24726 | s14 | occluded_by: 5584 (known gap: gap 3: injected paced digits still land as permutations in the money box on iOS even after #403 and even clicking the box first (500.25 read back as .52500 / .52005 / 520.05, gpui-agent-cli 0.4.1); a person's keyed input needs the software keyboard host feature (nsh-gpui-agent#11), which this build does not have) [screenshot](06-money-boxes/failure.png) |
| 7 | sheet-press | skipped | 0 |  | no phone variant |
| 8 | delete-row | pass | 11872 |  |  |
| 9 | queue | refused | 0 |  | presses a send control: run with --allow-send |
| 10 | birth-date | pass | 15963 |  |  |
| 11 | sign-out | known gap | 14384 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: gap 1: a sheet an earlier recipe left open (birth-date's Done press is refused) occludes the account menu; and under BIR_AUTH_BYPASS=1 no lock screen follows Log out (the macOS run of e2e-fixes-apple)) [screenshot](11-sign-out/failure.png) |
