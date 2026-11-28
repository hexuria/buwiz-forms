# e2e smoke on macos

App 669996d9, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 666 |  |  |
| 2 | sign-up | pass | 7369 |  |  |
| 3 | tax-year | pass | 5904 |  |  |
| 4 | navigate | pass | 8251 |  |  |
| 5 | open-2551q | pass | 1876 |  |  |
| 6 | money-boxes | pass | 16534 |  |  |
| 7 | sheet-press | skipped | 0 |  | no desktop variant |
| 8 | delete-row | setup | 254 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | queue | fail | 36811 | s18 `wait_until fp-send-line --timeout-ms 20000 --screenshot` | wait_until timed out: node `fp-send-line` not found [screenshot](09-queue/failure.png) |
| 10 | birth-date | pass | 4048 |  |  |
| 11 | sign-out | known gap | 12113 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: under the mandatory BIR_AUTH_BYPASS=1 there is no lock gate: Log out is pressed (nb-menu-logout) but no locked-screen ever appears, so the wait can never pass in this environment) [screenshot](11-sign-out/failure.png) |
