# e2e smoke on macos

App 6b1097a0, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 742 |  |  |
| 2 | sign-up | pass | 7581 |  |  |
| 3 | tax-year | pass | 6175 |  |  |
| 4 | navigate | pass | 8697 |  |  |
| 5 | open-2551q | pass | 1930 |  |  |
| 6 | money-boxes | pass | 17213 |  |  |
| 7 | queue | fail | 48136 | s29 | wait_until timed out: node `fp-send-line` not found [screenshot](07-queue/failure.png) |
| 8 | delete-row | setup | 128 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | birth-date | pass | 4192 |  |  |
| 10 | sign-out | known gap | 11947 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: under the mandatory BIR_AUTH_BYPASS=1 there is no lock gate: Log out is pressed (nb-menu-logout) but no locked-screen ever appears, so the wait can never pass in this environment) [screenshot](10-sign-out/failure.png) |
