# e2e signin-look-lock on ios

App 517374de, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, paste, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | signin-pin | setup | 32 |  | $pin_field: no node matches {"role": ["textbox", "textfield"], "name_regex": "^PIN"} |
