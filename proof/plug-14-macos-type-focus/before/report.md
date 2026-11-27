# e2e p14bare on macos

App 50798351, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 748 |  |  |
| 2 | sign-up | fail | 7468 | s19 `assert context-pill name="Tax profile: $name" --screenshot` | node `context-pill` name: expected "Tax profile: E2E SMOKE TAXPAYER", got "Tax profile: 1 TEST ST" [screenshot](02-sign-up/failure.png) |
