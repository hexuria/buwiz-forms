# e2e smoke on android

App 85dd0097, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, paste, system.rotate, system.keyboard_show, system.keyboard_hide, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 3076 |  |  |
| 2 | sign-up | pass | 11016 |  |  |
| 3 | navigate | pass | 5798 |  |  |
| 4 | open-2551q | pass | 14351 |  |  |
| 5 | money-boxes | fail | 3962 | s2 `type fp-frm2551Qv2018:txt16 $prior` | target_gone: fp-frm2551Qv2018:txt16 [screenshot](05-money-boxes/failure.png) |
| 6 | delete-row | setup | 62 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 7 | queue | fail | 15554 | s2 `wait_until queue-continue --timeout-ms 10000 --screenshot` | wait_until timed out: node `queue-continue` not found [screenshot](07-queue/failure.png) |
| 8 | birth-date | known gap | 12200 | s14 `click tp-taxpayer-birth_date-sheet-done` | `node 8258` is offscreen (known gap: gap 1: the date is set in a date sheet) [screenshot](08-birth-date/failure.png) |
| 9 | sign-out | known gap | 2401 | s1 `click nb-avatar` | occluded_by: 583 (known gap: gap 1: Log out is inside the tax profile sheet on a phone) [screenshot](09-sign-out/failure.png) |
