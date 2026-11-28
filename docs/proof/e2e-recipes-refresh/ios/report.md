# e2e smoke on ios

App d0025c42, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 1650 |  |  |
| 2 | sign-up | fail | 17670 | s9a | wait_until timed out: node `new-tp-zip_code-search` not found [screenshot](02-sign-up/failure.png) |
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
