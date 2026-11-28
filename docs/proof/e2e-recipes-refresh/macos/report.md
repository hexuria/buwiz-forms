# e2e smoke on macos

App d0025c42, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 753 |  |  |
| 2 | sign-up | pass | 8356 |  |  |
| 3 | tax-year | pass | 6062 |  |  |
| 4 | navigate | pass | 8623 |  |  |
| 5 | open-2551q | pass | 1620 |  |  |
| 6 | money-boxes | pass | 17839 |  |  |
| 7 | sheet-press | skipped | 0 |  | no desktop variant |
| 8 | delete-row | setup | 314 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | queue | pass | 19750 |  |  |
| 10 | queue-send | refused | 0 |  | presses a send control: run with --allow-send |
| 11 | birth-date | pass | 4572 |  |  |
| 12 | sign-out | known gap | 12262 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: issue #450: BIR_AUTH_BYPASS=1 leaves no lock screen after Log out) [screenshot](12-sign-out/failure.png) |
