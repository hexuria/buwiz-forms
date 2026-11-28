# e2e smoke on macos

App 6417adbc, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 687 |  |  |
| 2 | sign-up | pass | 7609 |  |  |
| 3 | tax-year | pass | 6050 |  |  |
| 4 | navigate | pass | 8848 |  |  |
| 5 | open-2551q | pass | 1916 |  |  |
| 6 | money-boxes | pass | 17465 |  |  |
| 7 | sheet-press | skipped | 0 |  | no desktop variant |
| 8 | delete-row | setup | 272 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | queue | fail | 36900 | s18 `wait_until fp-send-line --timeout-ms 20000 --screenshot` | wait_until timed out: node `fp-send-line` not found [screenshot](09-queue/failure.png) |
| 10 | birth-date | pass | 4140 |  |  |
| 11 | sign-out | known gap | 12183 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: under the mandatory BIR_AUTH_BYPASS=1 there is no lock gate: Log out is pressed (nb-menu-logout) but no locked-screen ever appears, so the wait can never pass in this environment) [screenshot](11-sign-out/failure.png) |
