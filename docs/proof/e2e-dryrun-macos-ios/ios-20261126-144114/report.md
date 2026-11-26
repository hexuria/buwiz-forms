# e2e smoke on ios

App 3b39bd3e, gpui-agent-cli 0.4.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, paste, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 1653 |  |  |
| 2 | sign-up | known gap | 21168 | s23 `assert home-tp-$tin --screenshot` | node `home-tp-12345678800000` not found (known gap: gap 1: the ZIP and RDO pickers are sheets on a phone, and gpui-agent presses inside sheets did not land in this week's rounds) [screenshot](02-sign-up/failure.png) |
| 3 | navigate | not run | 0 |  | sign-up failed and the suite needs it |
| 4 | birth-date | not run | 0 |  | sign-up failed and the suite needs it |
| 5 | open-2551q | not run | 0 |  | sign-up failed and the suite needs it |
| 6 | money-boxes | not run | 0 |  | sign-up failed and the suite needs it |
| 7 | delete-row | not run | 0 |  | sign-up failed and the suite needs it |
| 8 | queue | not run | 0 |  | sign-up failed and the suite needs it |
| 9 | sign-out | not run | 0 |  | sign-up failed and the suite needs it |
