# e2e rdo-row-press on macos

App 04fa6dab, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 726 |  |  |
| 2 | rdo-row-press | pass | 35198 |  |  |
| 3 | sign-up | pass | 7735 |  |  |
| 4 | profile-pick-press | fail | 2020 | s7 `click tp-branch-rdo_code-search` | occluded_by: 7416 [screenshot](04-profile-pick-press/failure.png) |
