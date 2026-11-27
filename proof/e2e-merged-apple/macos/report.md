# e2e smoke on macos

App 7d3b77fe, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 665 |  |  |
| 2 | sign-up | pass | 7240 |  |  |
| 3 | tax-year | pass | 5885 |  |  |
| 4 | navigate | pass | 9051 |  |  |
| 5 | open-2551q | pass | 1882 |  |  |
| 6 | money-boxes | pass | 16064 |  |  |
| 7 | queue | fail | 36930 | s18 `wait_until fp-cancel-queue --timeout-ms 20000 --screenshot` | wait_until timed out: node `fp-cancel-queue` not found [screenshot](07-queue/failure.png) |
| 8 | delete-row | setup | 126 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | birth-date | pass | 4321 |  |  |
| 10 | sign-out | known gap | 11982 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: under the mandatory BIR_AUTH_BYPASS=1 there is no lock gate: Log out is pressed (nb-menu-logout) but no locked-screen ever appears, so the wait can never pass in this environment) [screenshot](10-sign-out/failure.png) |
