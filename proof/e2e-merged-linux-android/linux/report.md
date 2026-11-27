# e2e smoke on linux

App 7d155e6d, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 584 |  |  |
| 2 | sign-up | pass | 9868 |  |  |
| 3 | tax-year | pass | 6343 |  |  |
| 4 | navigate | pass | 1990 |  |  |
| 5 | open-2551q | pass | 1797 |  |  |
| 6 | money-boxes | pass | 13734 |  |  |
| 7 | sheet-press | skipped | 0 |  | no desktop variant |
| 8 | delete-row | pass | 6097 |  |  |
| 9 | queue | refused | 0 |  | presses a send control: run with --allow-send |
| 10 | birth-date | pass | 6138 |  |  |
| 11 | sign-out | known gap | 11828 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: gap 1 in the Linux window's narrow layout: the date sheet birth-date left open (its Done press is refused offscreen) occludes the account menu) [screenshot](11-sign-out/failure.png) |
