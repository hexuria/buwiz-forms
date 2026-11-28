# e2e smoke on linux

App 16dd1a30, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 575 |  |  |
| 2 | sign-up | pass | 10006 |  |  |
| 3 | tax-year | pass | 6290 |  |  |
| 4 | navigate | pass | 1960 |  |  |
| 5 | open-2551q | pass | 1761 |  |  |
| 6 | money-boxes | pass | 13631 |  |  |
| 7 | sheet-press | skipped | 0 |  | no desktop variant |
| 8 | delete-row | setup | 34 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | queue | fail | 17055 | s27a | wait_until timed out: node `fp-pill-count` exists but should be absent [screenshot](09-queue/failure.png) |
| 10 | queue-send | skipped (needs --allow-send) | 0 |  | needs --allow-send |
| 11 | birth-date | pass | 5972 |  |  |
| 12 | sign-out | known gap | 11796 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: gap 1 in the Linux window's narrow layout: the date sheet birth-date left open (its Done press is refused offscreen) occludes the account menu) [screenshot](12-sign-out/failure.png) |
