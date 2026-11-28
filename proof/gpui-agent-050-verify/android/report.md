# e2e smoke on android

App f030fbfd, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 3202 |  |  |
| 2 | sign-up | fail | 7314 | s8 `set_value new-tp-registered_address $address` | occluded_by: software keyboard (a press on `node 300` at (205.90475, 742.2857) lands on a key; hide the keyboard or scroll the target above it first) [screenshot](02-sign-up/failure.png) |
| 3 | tax-year | not run | 0 |  | sign-up failed and the suite needs it |
| 4 | navigate | not run | 0 |  | sign-up failed and the suite needs it |
| 5 | open-2551q | not run | 0 |  | sign-up failed and the suite needs it |
| 6 | money-boxes | not run | 0 |  | sign-up failed and the suite needs it |
| 7 | sheet-press | not run | 0 |  | sign-up failed and the suite needs it |
| 8 | delete-row | not run | 0 |  | sign-up failed and the suite needs it |
| 9 | queue | not run | 0 |  | sign-up failed and the suite needs it |
| 10 | birth-date | not run | 0 |  | sign-up failed and the suite needs it |
| 11 | sign-out | not run | 0 |  | sign-up failed and the suite needs it |
