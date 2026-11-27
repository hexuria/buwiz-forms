# e2e smoke on linux

App 85dd0097, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 481 |  |  |
| 2 | sign-up | pass | 9728 |  |  |
| 3 | navigate | pass | 2017 |  |  |
| 4 | open-2551q | pass | 7377 |  |  |
| 5 | money-boxes | fail | 969 | s2 | target_gone: fp-frm2551Qv2018:txt16 [screenshot](05-money-boxes/failure.png) |
| 6 | delete-row | setup | 22 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 7 | queue | fail | 12913 | s2 `wait_until queue-continue --timeout-ms 10000 --screenshot` | wait_until timed out: node `queue-continue` not found [screenshot](07-queue/failure.png) |
| 8 | birth-date | fail | 5131 | s14 `click tp-taxpayer-birth_date-sheet-done` | `node 9282` is offscreen [screenshot](08-birth-date/failure.png) |
| 9 | sign-out | fail | 325 | s1 `click nb-avatar` | occluded_by: 555 [screenshot](09-sign-out/failure.png) |
