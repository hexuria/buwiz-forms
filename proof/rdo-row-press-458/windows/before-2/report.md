# e2e rdo-row-press on windows

App 4fa8ac8b, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 699 |  |  |
| 2 | rdo-row-press | fail | 3963 | s7 `assert new-tp-rdo_code-search value="047 — East Makati"` | node `new-tp-rdo_code-search` value: expected "047 — East Makati", got Some("") [screenshot](02-rdo-row-press/failure.png) |
| 3 | sign-up | pass | 6508 |  |  |
| 4 | profile-pick-press | fail | 6184 | s13 `assert tp-branch-rdo_code-search value="047 — East Makati"` | node `tp-branch-rdo_code-search` value: expected "047 — East Makati", got Some("040 — Cubao") [screenshot](04-profile-pick-press/failure.png) |
