# e2e smoke on linux

App 9b786302, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 477 |  |  |
| 2 | sign-up | fail | 18957 | s18 `wait_until tp-tabs-profile --timeout-ms 10000` | wait_until timed out: node `tp-tabs-profile` not found [screenshot](02-sign-up/failure.png) |
| 3 | navigate | not run | 0 |  | sign-up failed and the suite needs it |
| 4 | birth-date | not run | 0 |  | sign-up failed and the suite needs it |
| 5 | open-2551q | not run | 0 |  | sign-up failed and the suite needs it |
| 6 | money-boxes | not run | 0 |  | sign-up failed and the suite needs it |
| 7 | delete-row | not run | 0 |  | sign-up failed and the suite needs it |
| 8 | queue | not run | 0 |  | sign-up failed and the suite needs it |
| 9 | sign-out | not run | 0 |  | sign-up failed and the suite needs it |
