# e2e tour on ios

App 8f825621, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 674 |  |  |
| 2 | setup-cards | pass | 151 |  |  |
| 3 | sign-up | pass | 21418 |  |  |
| 4 | tax-year | pass | 8445 |  |  |
| 5 | home | pass | 864 |  |  |
| 6 | navigation | pass | 2455 |  |  |
| 7 | new-return | pass | 2982 |  |  |
| 8 | form-top | pass | 180 |  |  |
| 9 | form-middle | pass | 1933 |  |  |
| 10 | form-end | pass | 14470 |  |  |
| 11 | schedule-rows | pass | 23665 |  |  |
| 12 | sheet-pick | fail | 10371 | s11 | target_gone: fp-drpATC1-opt-1 [screenshot](12-sheet-pick/failure.png) |
| 13 | sheet-calendar | pass | 10417 |  |  |
