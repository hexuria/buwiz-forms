# e2e smoke on macos

App 33d82b40, gpui-agent-cli 0.4.1, host desktop (desktop), features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 737 |  |  |
| 2 | sign-up | pass | 7440 |  |  |
| 3 | tax-year | pass | 6265 |  |  |
| 4 | navigate | pass | 8983 |  |  |
| 5 | open-2551q | pass | 1977 |  |  |
| 6 | money-boxes | known gap | 8014 | s6 | node `fp-frm2551Qv2018:txt16` value: expected "500.25", got Some("0.25") (known gap: gap 2: under the old in-process macOS listener the money box refused agent text; this branch moves macOS to the strict adapter, not yet proven on a Mac) [screenshot](06-money-boxes/failure.png) |
| 7 | queue | fail | 14176 | s2 `wait_until queue-continue --timeout-ms 10000 --screenshot` | wait_until timed out: node `queue-continue` not found [screenshot](07-queue/failure.png) |
| 8 | delete-row | setup | 194 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | birth-date | pass | 4353 |  |  |
| 10 | sign-out | fail | 12165 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found [screenshot](10-sign-out/failure.png) |
