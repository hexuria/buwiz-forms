# e2e rdo-row-press on macos

App 2281b5ea, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 725 |  |  |
| 2 | rdo-row-press | pass | 35783 |  |  |
| 3 | sign-up | pass | 8653 |  |  |
| 4 | profile-pick-press | fail | 90075 | s163 `click tp-address-phone` | occluded_by: 18500 [screenshot](04-profile-pick-press/failure.png) |
