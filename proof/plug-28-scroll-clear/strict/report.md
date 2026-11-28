# e2e plug28-money on android

App f030fbfd, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 1013 |  |  |
| 2 | sign-up | pass | 11420 |  |  |
| 3 | tax-year | pass | 9398 |  |  |
| 4 | open-2551q | pass | 2422 |  |  |
| 5 | money-boxes-plug28 | fail | 19809 | s14 | occluded_by: fp-pill-list (its n4120) [screenshot](05-money-boxes-plug28/failure.png) |
