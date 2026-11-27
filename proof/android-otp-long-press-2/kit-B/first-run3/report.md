# e2e otp on android

App dd77c820, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | prep | pass | 3070 |  |  |
| 2 | tin-first | pass | 10446 |  |  |
| 3 | lp-first | fail | 7983 | s7w | wait_until timed out: node `system-text-menu` not found [screenshot](03-lp-first/failure.png) |
