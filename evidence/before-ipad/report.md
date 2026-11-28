# e2e tour on ios

App b983b2a4, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 887 |  |  |
| 2 | setup-cards | pass | 249 |  |  |
| 3 | sign-up | pass | 22449 |  |  |
| 4 | tax-year | pass | 13955 |  |  |
| 5 | home | pass | 1099 |  |  |
| 6 | navigation | pass | 3333 |  |  |
| 7 | new-return | pass | 2806 |  |  |
| 8 | form-top | pass | 462 |  |  |
| 9 | form-middle | pass | 2311 |  |  |
| 10 | form-end | pass | 14226 |  |  |
| 11 | schedule-rows | pass | 23931 |  |  |
| 12 | sheet-pick | fail | 10464 | s11 | target_gone: fp-drpATC1-opt-1 [screenshot](12-sheet-pick/failure.png) |
| 13 | sheet-calendar | pass | 10518 |  |  |
