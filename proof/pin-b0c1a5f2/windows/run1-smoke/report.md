# e2e smoke on windows

App b3bddd4d, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 674 |  |  |
| 2 | sign-up | pass | 6877 |  |  |
| 3 | tax-year | pass | 4202 |  |  |
| 4 | navigate | pass | 9225 |  |  |
| 5 | open-2551q | pass | 9363 |  |  |
| 6 | money-boxes | pass | 17580 |  |  |
| 7 | sheet-press | pass | 11371 |  |  |
| 8 | delete-row | pass | 4640 |  |  |
| 9 | queue | pass | 14390 |  |  |
| 10 | birth-date | pass | 3860 |  |  |
| 11 | sign-out | known gap | 11312 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: Log out is refused on a fresh database: endSession requires an admin PIN (or TOTP) for an admin session, and the suite safety rules forbid typing into a PIN box, so no run of this suite can sign out) [screenshot](11-sign-out/failure.png) |
