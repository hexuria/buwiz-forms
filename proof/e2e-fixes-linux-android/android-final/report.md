# e2e smoke on android

App 9b786302, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, paste, system.rotate, system.keyboard_show, system.keyboard_hide, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 3211 |  |  |
| 2 | sign-up | pass | 11728 |  |  |
| 3 | navigate | pass | 6023 |  |  |
| 4 | birth-date | known gap | 12367 | s14 `click tp-taxpayer-birth_date-sheet-done` | `node 5204` is offscreen (known gap: gap 1: the date is set in a date sheet) [screenshot](04-birth-date/failure.png) |
| 5 | open-2551q | fail | 2402 | s1 `click nb-search` | occluded_by: 453 [screenshot](05-open-2551q/failure.png) |
| 6 | money-boxes | not run | 0 |  | open-2551q failed and the suite needs it |
| 7 | delete-row | not run | 0 |  | open-2551q failed and the suite needs it |
| 8 | queue | not run | 0 |  | open-2551q failed and the suite needs it |
| 9 | sign-out | not run | 0 |  | open-2551q failed and the suite needs it |
