# e2e tin-status on macos

App 2db32ac5, gpui-agent-cli 0.4.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 727 |  |  |
| 2 | sign-up | pass | 7660 |  |  |
| 3 | buwiz-sign-in | fail | 4886 | s15 `key gate-code-page 246810` | node 2954 cannot receive native focus [screenshot](03-buwiz-sign-in/failure.png) |
| 4 | tin-chips | not run | 0 |  | buwiz-sign-in failed and the suite needs it |
| 5 | tin-queue-sheet | not run | 0 |  | buwiz-sign-in failed and the suite needs it |
