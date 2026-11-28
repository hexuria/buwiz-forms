# e2e smoke on windows

App 669996d9, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 741 |  |  |
| 2 | sign-up | pass | 8040 |  |  |
| 3 | tax-year | pass | 4169 |  |  |
| 4 | navigate | pass | 9888 |  |  |
| 5 | open-2551q | pass | 9480 |  |  |
| 6 | money-boxes | pass | 16733 |  |  |
| 7 | sheet-press | pass | 11393 |  |  |
| 8 | delete-row | pass | 2663 |  |  |
| 9 | queue | pass | 11090 |  |  |
| 10 | birth-date | pass | 3934 |  |  |
| 11 | sign-out | known gap | 11258 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: Log out is refused on a fresh database: endSession requires an admin PIN (or TOTP) for an admin session, and the suite safety rules forbid typing into a PIN box, so no run of this suite can sign out) [screenshot](11-sign-out/failure.png) |
