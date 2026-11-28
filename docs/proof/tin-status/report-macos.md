# e2e tin-status on macos

App 3cf68db4, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 676 |  |  |
| 2 | sign-up | pass | 7666 |  |  |
| 3 | buwiz-sign-in | fail | 4834 | s15 `key gate-code-page 246810` | node 2954 cannot receive native focus [screenshot](03-buwiz-sign-in/failure.png) |
| 4 | tin-chips | not run | 0 |  | buwiz-sign-in failed and the suite needs it |
| 5 | tin-queue-sheet | not run | 0 |  | buwiz-sign-in failed and the suite needs it |
