# e2e tour on windows

App b983b2a4, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 634 |  |  |
| 2 | sign-up | pass | 6680 |  |  |
| 3 | tax-year | pass | 4001 |  |  |
| 4 | tour-screens | pass | 12370 |  |  |
| 5 | open-2551q | pass | 9430 |  |  |
| 6 | tour-form | pass | 16537 |  |  |
| 7 | tour-extras | fail | 146 | s1 `click fp-set-profile-business_start_date` | target_gone: fp-set-profile-business_start_date [screenshot](07-tour-extras/failure.png) |
| 8 | tour-preview | pass | 7030 |  |  |
