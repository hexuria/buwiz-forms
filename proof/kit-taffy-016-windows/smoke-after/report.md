# e2e smoke on windows

App 8f825621, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 563 |  |  |
| 2 | sign-up | pass | 6398 |  |  |
| 3 | tax-year | pass | 3568 |  |  |
| 4 | navigate | pass | 8315 |  |  |
| 5 | open-2551q | pass | 8718 |  |  |
| 6 | money-boxes | pass | 16927 |  |  |
| 7 | sheet-press | pass | 10813 |  |  |
| 8 | delete-row | pass | 4262 |  |  |
| 9 | queue | pass | 13539 |  |  |
| 10 | birth-date | pass | 3517 |  |  |
| 11 | sign-out | known gap | 11029 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: Log out is refused on a fresh database: endSession requires an admin PIN (or TOTP) for an admin session, and the suite safety rules forbid typing into a PIN box, so no run of this suite can sign out) [screenshot](11-sign-out/failure.png) |
