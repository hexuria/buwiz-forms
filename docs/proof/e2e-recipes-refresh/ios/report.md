# e2e smoke on ios

App 73302a0c, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | sign-up-seed | pass | 211 |  |  |
| 2 | setup-start | pass | 154 |  |  |
| 3 | sign-up | skipped | 0 |  | no phone variant |
| 4 | tax-year | pass | 12724 |  |  |
| 5 | navigate | pass | 2843 |  |  |
| 6 | open-2551q | pass | 3334 |  |  |
| 7 | money-boxes | pass | 13251 |  |  |
| 8 | sheet-press | skipped | 0 |  | no phone variant |
| 9 | delete-row | setup | 54 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 10 | queue | pass | 21442 |  |  |
| 11 | birth-date | pass | 8452 |  |  |
| 12 | sign-out | known gap | 12380 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: issue #450: BIR_AUTH_BYPASS=1 leaves no lock screen after Log out) [screenshot](12-sign-out/failure.png) |
