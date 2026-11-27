# e2e smoke on windows

App ad7e456a, gpui-agent-cli 0.4.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 674 |  |  |
| 2 | sign-up | pass | 8288 |  |  |
| 3 | navigate | fail | 9001 | s12 `wait_until tp-profile-years --timeout-ms 5000 --screenshot` | wait_until timed out: node `tp-profile-years` not found [screenshot](03-navigate/failure.png) |
| 4 | birth-date | fail | 5475 | s2 `wait_until panel-open-profile --timeout-ms 5000` | wait_until timed out: node `panel-open-profile` not found [screenshot](04-birth-date/failure.png) |
| 5 | open-2551q | fail | 5424 | s2 `wait_until new-return-2551Q --timeout-ms 5000` | wait_until timed out: node `new-return-2551Q` not found [screenshot](05-open-2551q/failure.png) |
| 6 | money-boxes | not run | 0 |  | open-2551q failed and the suite needs it |
| 7 | delete-row | not run | 0 |  | open-2551q failed and the suite needs it |
| 8 | queue | not run | 0 |  | open-2551q failed and the suite needs it |
| 9 | sign-out | not run | 0 |  | open-2551q failed and the suite needs it |
