# e2e smoke on android

App 8f825621, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 1017 |  |  |
| 2 | sign-up | fail | 9125 | s13 `click tab-home` | occluded_by: software keyboard (a press on `node 894` at (51.42857, 838.2857) lands on a key; hide the keyboard or scroll the target above it first) [screenshot](02-sign-up/failure.png) |
| 3 | tax-year | not run | 0 |  | sign-up failed and the suite needs it |
| 4 | navigate | not run | 0 |  | sign-up failed and the suite needs it |
| 5 | open-2551q | not run | 0 |  | sign-up failed and the suite needs it |
| 6 | money-boxes | not run | 0 |  | sign-up failed and the suite needs it |
| 7 | sheet-press | not run | 0 |  | sign-up failed and the suite needs it |
| 8 | delete-row | not run | 0 |  | sign-up failed and the suite needs it |
| 9 | queue | not run | 0 |  | sign-up failed and the suite needs it |
| 10 | birth-date | not run | 0 |  | sign-up failed and the suite needs it |
| 11 | sign-out | not run | 0 |  | sign-up failed and the suite needs it |
