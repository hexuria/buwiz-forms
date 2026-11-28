# e2e smoke on windows

App 669996d9, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 656 |  |  |
| 2 | sign-up | pass | 6662 |  |  |
| 3 | tax-year | pass | 4038 |  |  |
| 4 | navigate | pass | 9226 |  |  |
| 5 | open-2551q | pass | 9341 |  |  |
| 6 | money-boxes | pass | 17419 |  |  |
| 7 | sheet-press | pass | 11254 |  |  |
| 8 | delete-row | pass | 2513 |  |  |
| 9 | queue | pass | 10843 |  |  |
| 10 | birth-date | pass | 3822 |  |  |
| 11 | sign-out | known gap | 11185 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: Log out is refused on a fresh database: endSession requires an admin PIN (or TOTP) for an admin session, and the suite safety rules forbid typing into a PIN box, so no run of this suite can sign out) [screenshot](11-sign-out/failure.png) |
