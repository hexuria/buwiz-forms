# e2e smoke on macos

App 3b39bd3e, gpui-agent-cli 0.4.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 744 |  |  |
| 2 | sign-up | fail | 17373 | s18 `wait_until nav-home --timeout-ms 10000` | wait_until timed out: node `nav-home` not found [screenshot](02-sign-up/failure.png) |
| 3 | navigate | not run | 0 |  | sign-up failed and the suite needs it |
| 4 | birth-date | not run | 0 |  | sign-up failed and the suite needs it |
| 5 | open-2551q | not run | 0 |  | sign-up failed and the suite needs it |
| 6 | money-boxes | not run | 0 |  | sign-up failed and the suite needs it |
| 7 | delete-row | not run | 0 |  | sign-up failed and the suite needs it |
| 8 | queue | not run | 0 |  | sign-up failed and the suite needs it |
| 9 | sign-out | not run | 0 |  | sign-up failed and the suite needs it |
