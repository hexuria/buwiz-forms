# e2e smoke on linux

App 8f825621, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 562 |  |  |
| 2 | sign-up | pass | 9418 |  |  |
| 3 | tax-year | pass | 6298 |  |  |
| 4 | navigate | pass | 2014 |  |  |
| 5 | open-2551q | pass | 1757 |  |  |
| 6 | money-boxes | pass | 13612 |  |  |
| 7 | sheet-press | skipped | 0 |  | no desktop variant |
| 8 | delete-row | setup | 30 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | queue | fail | 7144 | s5 `click tp-branch-business_start_date` | occluded_by: 507 [screenshot](09-queue/failure.png) |
| 10 | birth-date | known gap | 324 | s1 | occluded_by: 399 (known gap: gap 1: in the narrow layout of the Linux window, presses inside a sheet are refused as offscreen too (linux-final birth-date s14)) [screenshot](10-birth-date/failure.png) |
| 11 | sign-out | known gap | 327 | s1 `click nb-avatar` | occluded_by: 399 (known gap: gap 1 in the Linux window's narrow layout: the date sheet birth-date left open (its Done press is refused offscreen) occludes the account menu) [screenshot](11-sign-out/failure.png) |
