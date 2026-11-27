# e2e rdo-row-press on windows

App 4fa8ac8b, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 680 |  |  |
| 2 | rdo-row-press | pass | 36917 |  |  |
| 3 | sign-up | pass | 8507 |  |  |
| 4 | profile-pick-press | fail | 48873 | s89 `wait_until tp-branch-rdo_code-list --timeout-ms 3000` | wait_until timed out: node `tp-branch-rdo_code-list` not found [screenshot](04-profile-pick-press/failure.png) |
