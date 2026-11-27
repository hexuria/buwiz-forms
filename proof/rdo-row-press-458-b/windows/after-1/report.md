# e2e rdo-row-press on windows

App 6e73f1a9, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 747 |  |  |
| 2 | rdo-row-press | pass | 36920 |  |  |
| 3 | sign-up | pass | 8326 |  |  |
| 4 | profile-pick-press | fail | 46234 | s90 `click tp-branch-save --screenshot` | occluded_by: 13320 [screenshot](04-profile-pick-press/failure.png) |
