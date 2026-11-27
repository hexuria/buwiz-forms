# e2e otp on android

App dd77c820, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | prep | pass | 3050 |  |  |
| 2 | foreign-paste | fail | 5248 | f3 | the clipboard was not set by this run: Paste would put in what a person or another app copied (cut or copy through the text menu first) [screenshot](12-foreign-paste/failure.png) |
