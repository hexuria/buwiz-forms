# e2e smoke on linux

App b3bddd4d, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 557 |  |  |
| 2 | sign-up | pass | 10623 |  |  |
| 3 | tax-year | pass | 6299 |  |  |
| 4 | navigate | pass | 1977 |  |  |
| 5 | open-2551q | pass | 1753 |  |  |
| 6 | money-boxes | pass | 13621 |  |  |
| 7 | sheet-press | skipped | 0 |  | no desktop variant |
| 8 | delete-row | pass | 7518 |  |  |
| 9 | queue | refused | 0 |  | presses a send control: run with --allow-send |
| 10 | birth-date | pass | 8915 |  |  |
| 11 | sign-out | known gap | 11798 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: under the mandatory BIR_AUTH_BYPASS=1 there is no lock gate: Log out is pressed but locked-screen never appears) [screenshot](11-sign-out/failure.png) |
