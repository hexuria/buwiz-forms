# e2e smoke on android

App b983b2a4, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 1067 |  |  |
| 2 | sign-up | fail | 9256 | s13 `click tab-home` | occluded_by: software keyboard (a press on `node 894` at (51.42857, 838.2857) lands on a key; hide the keyboard or scroll the target above it first) [screenshot](02-sign-up/failure.png) |
