# e2e p26signup on ios

App f030fbfd, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 1692 |  |  |
| 2 | sign-up | fail | 3708 | s6 | a press at (185, 399) did not give node 3 the focus [screenshot](02-sign-up/failure.png) |
