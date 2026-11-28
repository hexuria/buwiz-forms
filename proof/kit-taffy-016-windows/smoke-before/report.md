# e2e smoke on windows

App b983b2a4, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 653 |  |  |
| 2 | sign-up | pass | 6711 |  |  |
| 3 | tax-year | pass | 4068 |  |  |
| 4 | navigate | pass | 9265 |  |  |
| 5 | open-2551q | pass | 9131 |  |  |
| 6 | money-boxes | pass | 17214 |  |  |
| 7 | sheet-press | pass | 11431 |  |  |
| 8 | delete-row | pass | 4385 |  |  |
| 9 | queue | pass | 13919 |  |  |
| 10 | birth-date | pass | 3865 |  |  |
| 11 | sign-out | known gap | 11128 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: Log out is refused on a fresh database: endSession requires an admin PIN (or TOTP) for an admin session, and the suite safety rules forbid typing into a PIN box, so no run of this suite can sign out) [screenshot](11-sign-out/failure.png) |
