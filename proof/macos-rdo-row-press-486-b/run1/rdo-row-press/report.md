# e2e rdo-row-press on macos

App 6417adbc, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 725 |  |  |
| 2 | rdo-row-press | pass | 36387 |  |  |
| 3 | sign-up | pass | 8592 |  |  |
| 4 | profile-pick-press | fail | 37040 | s67 `click tp-branch-name` | occluded_by: 12098 [screenshot](04-profile-pick-press/failure.png) |
