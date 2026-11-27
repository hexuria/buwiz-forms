# e2e smoke on windows

App 172e09d8, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 702 |  |  |
| 2 | sign-up | pass | 6202 |  |  |
| 3 | navigate | pass | 9554 |  |  |
| 4 | birth-date | pass | 3364 |  |  |
| 5 | open-2551q | pass | 13608 |  |  |
| 6 | money-boxes | pass | 18411 |  |  |
| 7 | queue | fail | 28937 | s9 `wait_until fp-cancel-queue --timeout-ms 20000 --screenshot` | wait_until timed out: node `fp-cancel-queue` not found [screenshot](07-queue/failure.png) |
| 8 | delete-row | setup | 92 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | sign-out | known gap | 11161 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: Log out is refused on a fresh database: endSession requires an admin PIN (or TOTP) for an admin session, and the suite safety rules forbid typing into a PIN box, so no run of this suite can sign out) [screenshot](09-sign-out/failure.png) |
