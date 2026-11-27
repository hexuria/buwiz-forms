# e2e otp-diag on android

App 7d155e6d, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | prep | pass | 3075 |  |  |
| 2 | type | pass | 10918 |  |  |
| 3 | hold-stock-p1 | fail | 3608 | h1 | target_gone: coordinate has no semantic target [screenshot](03-hold-stock-p1/failure.png) |
