# e2e dry run: macOS and iOS Simulator, smoke suite at 3b39bd3e

Devin machine buwiz-forms-macos-3, macOS. App commit 3b39bd3ec064be4c1617ae8a0311bf8b2d7b66a7 (PR #428, feat/devdrive, detached). gpui-agent-cli 0.4.0 (the version Cargo.lock pins). nsh pin 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf; cargo 1.99.0 (5f94df478 2026-08-27); build folder ~/BuwizNsh/target-nsh (the recipes' own default). Run exactly through `scripts/e2e/machines/macos.sh` and `scripts/e2e/machines/ios.sh`, no improvised input. Safety env throughout: BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1, never BIR_SFTP_LIVE, scratch databases under ~/BuwizNsh/agents/, nothing sent anywhere real. GPUI_AGENT_TOKEN was generated per shell, never printed or written down.

`just e2e check smoke` (both runs): 9 recipes, 14 files validated by gpui-agent-cli 0.4.0, 0 problems. `money-boxes-paced*.json` skipped at check: paced typing needs cli 0.4.1; the unpaced fallbacks would have run — the suite never reached them on either platform.

## macOS — macos-20261126-141133/ (1 pass, 1 fail, 7 not run; exit 1)

| # | recipe | result | ms | failing step | error |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 744 | | |
| 2 | sign-up | fail | 17373 | s18 `wait_until nav-home` | node `nav-home` not found |
| 3-9 | navigate, birth-date, open-2551q, money-boxes, delete-row, queue, sign-out | not run | 0 | | sign-up failed and the suite needs it |

What actually happened at the failure (02-sign-up/failure.png, 017-s17.png, failure-tree.json): the **registered name never landed**. Every other field is filled (TIN 123-456-788 in its segments, "1 TEST ST", ZIP 1000 chosen, RDO 040 chosen), but the name box is empty with the live validation error "Type the registered name.", so Create taxpayer did nothing and `nav-home` never appeared. `type new-tp-name` (s5) reported `delivered_unverified`; the type into the TIN box before it and the address after it both landed. Host features: press, scroll, hover, drag, viewport, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

## iOS Simulator — ios-20261126-144114/ (1 pass, 1 known gap, 7 not run; exit 1)

iPhone 17e, iOS 27.0, `--record` (recording.mp4 per recipe, both well under 50 MB).

| # | recipe | result | ms | failing step | error |
|---|---|---|---|---|---|
| 1 | setup-start | pass | 1653 | | |
| 2 | sign-up | known gap | 21168 | s23 `assert home-tp-$tin` | node `home-tp-12345678800000` not found |
| 3-9 | (same) | not run | 0 | | sign-up failed and the suite needs it |

What actually happened at the known-gap step — **not what the gap label assumes**. Gap 1 says sheet presses don't land; here every sheet step PASSED (ZIP and RDO sheets opened, their options were pressed, the sheets closed — s9-s20 all ok), and the taxpayer **was created**: failure.png shows "E2E SMOKE TAXPAYER · 123-456-788-00000" with the toast "E2E SMOKE TAXPAYER added." The failure is that after `new-tp-save` the phone lands on the taxpayer's own page (Forms tab), not on Home, so `home-tp-<tin>` is absent. s22 `wait_until tab-home` passed trivially because the bottom tab bar's Home tab exists on that page too. Host features: press, scroll, drag, swipe, long_press, system.back, paste, snapshot_since, snapshot_compact, expect_fingerprint, secret_redaction, screenshot_inline.

## Blockers along the way

1. The first iOS run never started a recipe: the disk filled during the iOS Simulator host build (16 GB target-nsh/debug + a partial 5.7 GB ios-sim object set on a small disk), the host build failed, and the runner then hit `No space left on device` writing report.json. Its app.log/run.log are in ios-run1-build-failed/. Space was freed by erasing the contents (not the devices) of two shutdown Simulators left over from earlier jobs, and the rerun then built and ran.
2. The machine's `~/BuwizNsh/tools` nsh CLI is 0.6.1 (ca39c0e), older than the pin 8c78cfd6 — the launch line in app.log reports it. The macOS run still built and ran; nothing in either failure is traceable to it, recorded here because the pin rule says the two must match.
