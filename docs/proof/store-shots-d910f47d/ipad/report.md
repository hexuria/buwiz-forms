# e2e store-shots-ipad on ios

App d910f47d, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | store-start | pass | 604 |  |  |
| 2 | store-shots | pass | 30555 |  |  |
| 3 | store-form | fail | 18737 | back-tab-home | `node 2024` is offscreen or has no native hit [screenshot](03-store-form/failure.png) |
| 4 | store-shots-landscape | not run | 0 |  | store-form failed and the suite needs it |
