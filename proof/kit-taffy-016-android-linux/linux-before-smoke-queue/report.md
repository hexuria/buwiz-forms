# e2e smoke on linux

App b983b2a4, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 561 |  |  |
| 2 | sign-up | pass | 10341 |  |  |
| 3 | tax-year | pass | 6385 |  |  |
| 4 | open-2551q | pass | 1806 |  |  |
| 5 | queue | fail | 6606 | s5 `click tp-branch-business_start_date` | occluded_by: 399 [screenshot](09-queue/failure.png) |
