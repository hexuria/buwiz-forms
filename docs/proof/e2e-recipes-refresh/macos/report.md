# e2e smoke on macos

App 9abbb928, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 699 |  |  |
| 2 | sign-up-seed | skipped | 0 |  | no desktop variant |
| 3 | sign-up | pass | 7645 |  |  |
| 4 | tax-year | pass | 5932 |  |  |
| 5 | navigate | pass | 9326 |  |  |
| 6 | open-2551q | pass | 1920 |  |  |
| 7 | money-boxes | pass | 17366 |  |  |
| 8 | sheet-press | skipped | 0 |  | no desktop variant |
| 9 | delete-row | setup | 287 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 10 | queue | pass | 17926 |  |  |
| 11 | birth-date | pass | 4298 |  |  |
| 12 | sign-out | known gap | 12109 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: issue #450: BIR_AUTH_BYPASS=1 leaves no lock screen after Log out) [screenshot](12-sign-out/failure.png) |
