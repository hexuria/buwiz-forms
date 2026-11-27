# e2e probe-mac-a on macos

App 8aa31f3c, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 710 |  |  |
| 2 | probe-a | fail | 2598 | s8 | node `new-tp-name` value: expected "E2E SMOKE TAXPAYER", got Some("") [screenshot](02-probe-a/failure.png) |
