# e2e signin-look on ios

App 517374de, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, paste, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | pin-error-open | pass | 3815 |  |  |
| 2 | pin-error-shot | pass | 2514 |  |  |
| 3 | newtp | pass | 5608 |  |  |
| 4 | signout-gap | known gap | 33317 | s9 `wait_until locked-screen --timeout-ms 8000` | wait_until timed out: node `locked-screen` not found (known gap: gap #13 (hexuria/nsh-gpui-agent#13): presses inside sheets answer ok and nothing happens, so Log out never fires and the account gate (the email and code pages) cannot be reached) [screenshot](04-signout-gap/failure.png) |
