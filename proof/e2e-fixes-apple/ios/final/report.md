# e2e smoke on ios

App 8aa31f3c, gpui-agent-cli 0.4.1, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, paste, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 2653 |  |  |
| 2 | sign-up | pass | 22216 |  |  |
| 3 | tax-year | pass | 7909 |  |  |
| 4 | navigate | pass | 3669 |  |  |
| 5 | open-2551q | pass | 3689 |  |  |
| 6 | money-boxes | known gap | 9600 | s7 | node `fp-frm2551Qv2018:txt16` value: expected "500.25", got Some("520.01") (known gap: gap 3: injected text cannot type into an auto-formatting money box (digits land scrambled at any pacing); a person's keyed input needs the software keyboard host feature (nsh-gpui-agent#11), which this build does not have) [screenshot](06-money-boxes/failure.png) |
| 7 | queue | fail | 15939 | s2 `wait_until queue-continue --timeout-ms 10000 --screenshot` | wait_until timed out: node `queue-continue` not found [screenshot](07-queue/failure.png) |
| 8 | delete-row | setup | 45 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | birth-date | known gap | 1182 | s1 | target_gone: tp-tabs-profile (known gap: gap 1: the date is set in a date sheet) [screenshot](09-birth-date/failure.png) |
| 10 | sign-out | known gap | 6814 | s2 `wait_until nb-menu-profile --timeout-ms 5000` | wait_until timed out: node `nb-menu-profile` not found (known gap: gap 1: Log out is inside the tax profile sheet on a phone) [screenshot](10-sign-out/failure.png) |
