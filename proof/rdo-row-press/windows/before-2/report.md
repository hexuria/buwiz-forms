# e2e rdo-row-press on windows

App f9e543ce, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 690 |  |  |
| 2 | rdo-row-press | fail | 3458 | s6 `assert new-tp-rdo_code-search value="047 — East Makati"` | node `new-tp-rdo_code-search` value: expected "047 — East Makati", got Some("") [screenshot](02-rdo-row-press/failure.png) |
