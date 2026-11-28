# e2e smoke on linux

App b3bddd4d, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 1131 |  |  |
| 2 | sign-up | pass | 12245 |  |  |
| 3 | tax-year | pass | 6315 |  |  |
| 4 | navigate | pass | 2002 |  |  |
| 5 | open-2551q | pass | 1751 |  |  |
| 6 | money-boxes | pass | 13621 |  |  |
| 7 | sheet-press | skipped | 0 |  | no desktop variant |
| 8 | delete-row | setup | 31 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | queue | refused | 0 |  | presses a send control: run with --allow-send |
| 10 | birth-date | pass | 6188 |  |  |
| 11 | sign-out | known gap | 11796 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: gap 1 in the Linux window's narrow layout: the date sheet birth-date left open (its Done press is refused offscreen) occludes the account menu) [screenshot](11-sign-out/failure.png) |
