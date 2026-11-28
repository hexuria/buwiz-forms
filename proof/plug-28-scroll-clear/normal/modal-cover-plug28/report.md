# e2e plug28-modal on android

App f030fbfd, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 1235 |  |  |
| 2 | sign-up | pass | 11177 |  |  |
| 3 | tax-year | pass | 9279 |  |  |
| 4 | modal-cover-plug28 | fail | 5974 | s13 | occluded_by: n395 [screenshot](04-modal-cover-plug28/failure.png) |
