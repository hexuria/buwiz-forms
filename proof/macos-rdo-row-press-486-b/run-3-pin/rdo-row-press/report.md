# e2e rdo-row-press on macos

App 6417adbc, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 708 |  |  |
| 2 | rdo-row-press | pass | 34490 |  |  |
| 3 | sign-up | fail | 7546 | s20 `assert context-pill name="Tax profile: $name" --screenshot` | node `context-pill` name: expected "Tax profile: E2E SMOKE TAXPAYER", got "Tax profile: E2E SMOKE TAXPAYER1 TEST ST" [screenshot](03-sign-up/failure.png) |
| 4 | profile-pick-press | not run | 0 |  | sign-up failed and the suite needs it |
