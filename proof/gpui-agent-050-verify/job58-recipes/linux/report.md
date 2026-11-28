# e2e smoke on linux

App f030fbfd, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 563 |  |  |
| 2 | sign-up | pass | 9769 |  |  |
| 3 | tax-year | pass | 6291 |  |  |
| 4 | navigate | pass | 1995 |  |  |
| 5 | open-2551q | pass | 1765 |  |  |
| 6 | money-boxes | pass | 13606 |  |  |
| 7 | sheet-press | skipped | 0 |  | no desktop variant |
| 8 | delete-row | pass | 7522 |  |  |
| 9 | queue | refused | 0 |  | presses a send control: run with --allow-send |
| 10 | birth-date | pass | 8885 |  |  |
| 11 | sign-out | known gap | 11785 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: under the mandatory BIR_AUTH_BYPASS=1 there is no lock gate: Log out is pressed but locked-screen never appears) [screenshot](11-sign-out/failure.png) |
