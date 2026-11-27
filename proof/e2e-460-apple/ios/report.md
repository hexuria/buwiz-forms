# e2e smoke on ios

App 6b1097a0, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, paste, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 609 |  |  |
| 2 | sign-up | pass | 21066 |  |  |
| 3 | tax-year | pass | 7177 |  |  |
| 4 | navigate | pass | 2304 |  |  |
| 5 | open-2551q | pass | 2769 |  |  |
| 6 | money-boxes | known gap | 8085 | s9 | node `fp-frm2551Qv2018:txt16` value: expected "500.25", got Some(".25005") (known gap: gap 3: injected paced digits still land as permutations in the money box on iOS even after #403 and even clicking the box first (500.25 read back as .52500 / .52005 / 520.05, gpui-agent-cli 0.4.1); a person's keyed input needs the software keyboard host feature (nsh-gpui-agent#11), which this build does not have) [screenshot](06-money-boxes/failure.png) |
| 7 | queue | fail | 10162 | s5 `click tp-branch-business_start_date` | occluded_by: 571 [screenshot](07-queue/failure.png) |
| 8 | delete-row | setup | 40 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | birth-date | known gap | 336 | s1 | occluded_by: 463 (known gap: gap 1: the date is set in a date sheet) [screenshot](09-birth-date/failure.png) |
| 10 | sign-out | known gap | 356 | s1 `click nb-avatar` | occluded_by: 463 (known gap: gap 1: a sheet an earlier recipe left open (birth-date's Done press is refused) occludes the account menu; and under BIR_AUTH_BYPASS=1 no lock screen follows Log out (the macOS run of e2e-fixes-apple)) [screenshot](10-sign-out/failure.png) |
