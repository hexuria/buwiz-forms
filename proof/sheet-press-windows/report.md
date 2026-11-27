# e2e smoke on windows

App 43defa62, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 656 |  |  |
| 2 | sign-up | pass | 6794 |  |  |
| 3 | tax-year | pass | 4105 |  |  |
| 4 | navigate | pass | 9107 |  |  |
| 5 | open-2551q | pass | 9447 |  |  |
| 6 | money-boxes | pass | 16111 |  |  |
| 7 | sheet-press | pass | 11497 |  |  |
| 8 | delete-row | pass | 2615 |  |  |
| 9 | queue | pass | 8584 |  |  |
| 10 | birth-date | pass | 3844 |  |  |
| 11 | sign-out | known gap | 11246 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: Log out is refused on a fresh database: endSession requires an admin PIN (or TOTP) for an admin session, and the suite safety rules forbid typing into a PIN box, so no run of this suite can sign out) [screenshot](11-sign-out/failure.png) |
