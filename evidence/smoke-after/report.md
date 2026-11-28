# e2e smoke on ios

App 8f825621, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 735 |  |  |
| 2 | sign-up | pass | 21673 |  |  |
| 3 | tax-year | pass | 7232 |  |  |
| 4 | navigate | pass | 2583 |  |  |
| 5 | open-2551q | pass | 2199 |  |  |
| 6 | money-boxes | known gap | 1452 | s1 | occluded_by: 5170 (known gap: gap 3: injected paced digits still land as permutations in the money box on iOS even after #403 and even clicking the box first (500.25 read back as .52500 / .52005 / 520.05, gpui-agent-cli 0.4.1); a person's keyed input needs the software keyboard host feature (nsh-gpui-agent#11), which this build does not have) [screenshot](06-money-boxes/failure.png) |
| 7 | sheet-press | skipped | 0 |  | no phone variant |
| 8 | delete-row | setup | 29 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | queue | refused | 0 |  | presses a send control: run with --allow-send |
| 10 | birth-date | pass | 8731 |  |  |
| 11 | sign-out | known gap | 12374 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: gap 1: a sheet an earlier recipe left open (birth-date's Done press is refused) occludes the account menu; and under BIR_AUTH_BYPASS=1 no lock screen follows Log out (the macOS run of e2e-fixes-apple)) [screenshot](11-sign-out/failure.png) |
