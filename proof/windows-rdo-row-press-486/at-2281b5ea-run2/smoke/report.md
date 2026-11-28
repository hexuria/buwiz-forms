# e2e smoke on windows

App 2281b5ea, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 698 |  |  |
| 2 | sign-up | pass | 6775 |  |  |
| 3 | tax-year | pass | 4209 |  |  |
| 4 | navigate | pass | 9525 |  |  |
| 5 | open-2551q | pass | 9242 |  |  |
| 6 | money-boxes | pass | 17397 |  |  |
| 7 | sheet-press | pass | 11505 |  |  |
| 8 | delete-row | pass | 2683 |  |  |
| 9 | queue | pass | 11317 |  |  |
| 10 | birth-date | pass | 3880 |  |  |
| 11 | sign-out | known gap | 11157 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: Log out is refused on a fresh database: endSession requires an admin PIN (or TOTP) for an admin session, and the suite safety rules forbid typing into a PIN box, so no run of this suite can sign out) [screenshot](11-sign-out/failure.png) |
