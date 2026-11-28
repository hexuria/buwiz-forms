# e2e smoke on android

App 16dd1a30, gpui-agent-cli 0.5.0, host mobile (phone), features: press, scroll, drag, swipe, long_press, system.back, software_keyboard, paste, system.rotate, system.keyboard_show, system.keyboard_hide, press_within, assert_states, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

| # | recipe | result | ms | failing step | error / note |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 3196 |  |  |
| 2 | sign-up | pass | 13929 |  |  |
| 3 | tax-year | pass | 12214 |  |  |
| 4 | navigate | pass | 5354 |  |  |
| 5 | open-2551q | pass | 5139 |  |  |
| 6 | money-boxes | pass | 26347 |  |  |
| 7 | sheet-press | skipped | 0 |  | no phone variant |
| 8 | delete-row | setup | 93 |  | $table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1} |
| 9 | queue | fail | 31193 | s24 | occluded_by: software keyboard (a press on `node 10964` at (205.7143, 710.4762) lands on a key; hide the keyboard or scroll the target above it first) [screenshot](09-queue/failure.png) |
| 10 | queue-send | skipped (needs --allow-send) | 0 |  | needs --allow-send |
| 11 | birth-date | pass | 17698 |  |  |
| 12 | sign-out | known gap | 14353 | s4 `wait_until locked-screen --timeout-ms 10000 --screenshot` | wait_until timed out: node `locked-screen` not found (known gap: issue #450: BIR_AUTH_BYPASS=1 leaves no lock screen after Log out) [screenshot](12-sign-out/failure.png) |
