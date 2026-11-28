# e2e tour on linux

App 8f825621, gpui-agent-cli 0.5.0, host desktop (desktop), features: press, scroll, hover, drag, viewport, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 731 |  |  |
| 2 | setup-cards | skipped | 0 |  | no desktop variant |
| 3 | sign-up | pass | 9385 |  |  |
| 4 | tax-year | pass | 6230 |  |  |
| 5 | shell | fail | 110 | s1 | target_gone: nav-home [screenshot](05-shell/failure.png) |
| 6 | home | skipped | 0 |  | no desktop variant |
| 7 | navigation | skipped | 0 |  | no desktop variant |
| 8 | new-return | fail | 115 | s1 | target_gone: sb-new [screenshot](08-new-return/failure.png) |
| 9 | form-top | not run | 0 |  | new-return failed and the suite needs it |
| 10 | form-middle | not run | 0 |  | new-return failed and the suite needs it |
| 11 | form-end | not run | 0 |  | new-return failed and the suite needs it |
| 12 | schedule-rows | not run | 0 |  | new-return failed and the suite needs it |
| 13 | sheet-pick | not run | 0 |  | new-return failed and the suite needs it |
| 14 | sheet-calendar | not run | 0 |  | new-return failed and the suite needs it |
| 15 | print | not run | 0 |  | new-return failed and the suite needs it |
