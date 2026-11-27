# e2e smokeprobe on ios

App 58621b4f, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 1955 |  |  |
| 2 | sign-up | pass | 23279 |  |  |
| 3 | tax-year | pass | 10055 |  |  |
| 4 | navigate | pass | 3861 |  |  |
| 5 | open-2551q | pass | 3803 |  |  |
| 6 | money-boxes | known gap | 2864 | s2 | target_gone: fp-frm2551Qv2018:txt16 (known gap: gap 3: injected paced digits still land as permutations in the money box on iOS even after #403 and even clicking the box first (500.25 read back as .52500 / .52005 / 520.05, gpui-agent-cli 0.4.1); a person's keyed input needs the software keyboard host feature (nsh-gpui-agent#11), which this build does not have) [screenshot](06-money-boxes/failure.png) |
| 7 | mbprobe | fail | 9706 | s9 | node `fp-frm2551Qv2018:txt16` value: expected "500.25", got Some(".52005") [screenshot](07-mbprobe/failure.png) |
| 8 | sheet-press | skipped | 0 |  | no phone variant |
| 9 | delete-row | setup | 48 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 10 | queue | fail | 9473 | s5 `click tp-branch-business_start_date` | occluded_by: 573 [screenshot](10-queue/failure.png) |
| 11 | birth-date | known gap | 1609 | s1 | occluded_by: 465 (known gap: gap 1: the date is set in a date sheet) [screenshot](11-birth-date/failure.png) |
| 12 | sign-out | known gap | 1376 | s1 `click nb-avatar` | occluded_by: 465 (known gap: gap 1: a sheet an earlier recipe left open (birth-date's Done press is refused) occludes the account menu; and under BIR_AUTH_BYPASS=1 no lock screen follows Log out (the macOS run of e2e-fixes-apple)) [screenshot](12-sign-out/failure.png) |
