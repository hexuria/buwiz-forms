# e2e tour on ios

App b983b2a4, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 609 |  |  |
| 2 | setup-cards | pass | 172 |  |  |
| 3 | sign-up | pass | 21517 |  |  |
| 4 | tax-year | pass | 12772 |  |  |
| 5 | home | pass | 647 |  |  |
| 6 | navigation | pass | 2702 |  |  |
| 7 | new-return | pass | 2838 |  |  |
| 8 | form-top | pass | 184 |  |  |
| 9 | form-middle | pass | 2008 |  |  |
| 10 | form-end | pass | 14397 |  |  |
| 11 | schedule-rows | pass | 23794 |  |  |
| 12 | sheet-pick | fail | 10333 | s11 | target_gone: fp-drpATC1-opt-1 [screenshot](12-sheet-pick/failure.png) |
| 13 | sheet-calendar | pass | 10484 |  |  |
