# e2e tour on ios

App 8f825621, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 760 |  |  |
| 2 | setup-cards | pass | 182 |  |  |
| 3 | sign-up | pass | 22110 |  |  |
| 4 | tax-year | pass | 12811 |  |  |
| 5 | home | pass | 622 |  |  |
| 6 | navigation | pass | 2436 |  |  |
| 7 | new-return | pass | 2898 |  |  |
| 8 | form-top | pass | 192 |  |  |
| 9 | form-middle | pass | 1921 |  |  |
| 10 | form-end | pass | 14235 |  |  |
| 11 | schedule-rows | pass | 23690 |  |  |
| 12 | sheet-pick | fail | 10135 | s11 | target_gone: fp-drpATC1-opt-1 [screenshot](12-sheet-pick/failure.png) |
| 13 | sheet-calendar | pass | 10480 |  |  |
