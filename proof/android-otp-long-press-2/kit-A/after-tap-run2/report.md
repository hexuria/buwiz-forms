# e2e otp on android

App dd77c820, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | prep | pass | 3085 |  |  |
| 2 | tin-after-tap | pass | 11996 |  |  |
| 3 | lp-after-tap | fail | 4286 | s9 | node `system-text-menu-copy` not found [screenshot](05-lp-after-tap/failure.png) |
