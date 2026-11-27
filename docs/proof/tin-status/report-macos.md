# e2e tin-status on macos

App 29af9d5d, gpui-agent-cli 0.4.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 684 |  |  |
| 2 | sign-up | pass | 7508 |  |  |
| 3 | buwiz-sign-in | fail | 10885 | s4 `wait_until settings-account --timeout-ms 10000` | wait_until timed out: node `settings-account` not found [screenshot](03-buwiz-sign-in/failure.png) |
| 4 | tin-chips | not run | 0 |  | buwiz-sign-in failed and the suite needs it |
| 5 | tin-queue-sheet | not run | 0 |  | buwiz-sign-in failed and the suite needs it |
