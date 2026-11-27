# e2e sheet-repro-suite on windows

App 43defa62, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 2081 |  |  |
| 2 | sign-up | pass | 8926 |  |  |
| 3 | tax-year | pass | 4108 |  |  |
| 4 | open-2551q | pass | 9392 |  |  |
| 5 | sheet-repro | fail | 4980 | s8 | `node 9322` is offscreen [screenshot](05-sheet-repro/failure.png) |
