# e2e smoke on windows

App 6417adbc, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 699 |  |  |
| 2 | sign-up | pass | 9782 |  |  |
| 3 | tax-year | pass | 4160 |  |  |
| 4 | navigate | pass | 9507 |  |  |
| 5 | open-2551q | pass | 9451 |  |  |
| 6 | money-boxes | pass | 17163 |  |  |
| 7 | sheet-press | pass | 11503 |  |  |
| 8 | delete-row | pass | 2607 |  |  |
| 9 | queue | pass | 10856 |  |  |
| 10 | birth-date | pass | 3714 |  |  |
| 11 | sign-out | known gap | 11193 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: Log out is refused on a fresh database: endSession requires an admin PIN (or TOTP) for an admin session, and the suite safety rules forbid typing into a PIN box, so no run of this suite can sign out) [screenshot](11-sign-out/failure.png) |
