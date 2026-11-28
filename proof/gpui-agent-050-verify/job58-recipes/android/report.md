# e2e smoke on android

App f030fbfd, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 3314 |  |  |
| 2 | sign-up | pass | 12750 |  |  |
| 3 | tax-year | pass | 11313 |  |  |
| 4 | navigate | pass | 5036 |  |  |
| 5 | open-2551q | pass | 4813 |  |  |
| 6 | money-boxes | pass | 25121 |  |  |
| 7 | sheet-press | skipped | 0 |  | no phone variant |
| 8 | delete-row | pass | 16712 |  |  |
| 9 | queue | refused | 0 |  | presses a send control: run with --allow-send |
| 10 | birth-date | pass | 15483 |  |  |
| 11 | sign-out | known gap | 14287 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: under the mandatory BIR_AUTH_BYPASS=1 there is no lock gate: Log out is pressed but locked-screen never appears) [screenshot](11-sign-out/failure.png) |
