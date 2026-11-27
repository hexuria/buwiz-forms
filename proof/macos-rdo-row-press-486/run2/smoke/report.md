# e2e smoke on macos

App 04fa6dab, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 657 |  |  |
| 2 | sign-up | fail | 7289 | s20 `assert context-pill name="Tax profile: $name" --screenshot` | node `context-pill` name: expected "Tax profile: E2E SMOKE TAXPAYER", got "Tax profile: E2E SMOKE TAXPAYER1 TEST ST" [screenshot](02-sign-up/failure.png) |
| 3 | tax-year | not run | 0 |  | sign-up failed and the suite needs it |
| 4 | navigate | not run | 0 |  | sign-up failed and the suite needs it |
| 5 | open-2551q | not run | 0 |  | sign-up failed and the suite needs it |
| 6 | money-boxes | not run | 0 |  | sign-up failed and the suite needs it |
| 7 | sheet-press | not run | 0 |  | sign-up failed and the suite needs it |
| 8 | delete-row | not run | 0 |  | sign-up failed and the suite needs it |
| 9 | queue | not run | 0 |  | sign-up failed and the suite needs it |
| 10 | birth-date | not run | 0 |  | sign-up failed and the suite needs it |
| 11 | sign-out | not run | 0 |  | sign-up failed and the suite needs it |
