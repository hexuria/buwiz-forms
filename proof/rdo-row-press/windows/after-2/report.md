# e2e rdo-row-press on windows

App f9e543ce, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 663 |  |  |
| 2 | rdo-row-press | fail | 11343 | s10 `wait_until new-tp-rdo_code-opt-048 --timeout-ms 5000` | wait_until timed out: node `new-tp-rdo_code-opt-048` not found [screenshot](02-rdo-row-press/failure.png) |
