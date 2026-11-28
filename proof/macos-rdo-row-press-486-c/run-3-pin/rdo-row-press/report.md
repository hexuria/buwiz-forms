# e2e rdo-row-press on macos

App 2281b5ea, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 686 |  |  |
| 2 | rdo-row-press | pass | 36067 |  |  |
| 3 | sign-up | fail | 17353 | s19 `wait_until nav-home --timeout-ms 10000` | wait_until timed out: node `nav-home` not found [screenshot](03-sign-up/failure.png) |
| 4 | profile-pick-press | not run | 0 |  | sign-up failed and the suite needs it |
