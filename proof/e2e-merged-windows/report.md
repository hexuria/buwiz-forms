# e2e smoke on windows

App 7d3b77fe, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 772 |  |  |
| 2 | sign-up | pass | 6882 |  |  |
| 3 | tax-year | pass | 4020 |  |  |
| 4 | navigate | pass | 9989 |  |  |
| 5 | open-2551q | pass | 9492 |  |  |
| 6 | money-boxes | pass | 17274 |  |  |
| 7 | queue | pass | 12196 |  |  |
| 8 | delete-row | setup | 86 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | birth-date | pass | 4129 |  |  |
| 10 | sign-out | known gap | 11179 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: Log out is refused on a fresh database: endSession requires an admin PIN (or TOTP) for an admin session, and the suite safety rules forbid typing into a PIN box, so no run of this suite can sign out) [screenshot](10-sign-out/failure.png) |
