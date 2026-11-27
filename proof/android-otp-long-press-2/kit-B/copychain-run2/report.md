# e2e otp on android

App dd77c820, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | prep | pass | 3028 |  |  |
| 2 | copychain-select | fail | 14308 | c1w | wait_until timed out: node `system-text-menu` not found [screenshot](09-copychain-select/failure.png) |
| 3 | copychain-paste | fail | 2091 | p1 | no text menu shows [screenshot](10-copychain-paste/failure.png) |
