# PR406 confirming owner-case proof — b4b0fc11

App commit `b4b0fc1156842e69041e721f6990288db0c0b1bf` (`a9c9fd7c` plus the commit removing normal-build sheet list scroll/layout handlers). NSH pin verified as `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; CLI `nsh 0.6.1 (8c78cfd)`; cargo `1.99.0`; target folder `~/BuwizNsh/target-b4b0fc11`.

Safety: scratch databases only; `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1` (`SIMCTL_CHILD_*` on Simulators); `BIR_SFTP_LIVE` never set; Queue never pressed; no real account or service. Real input only. Connect Hardware Keyboard was OFF for iPhone evidence.

## Result

| device | RDO owner end-scroll/pick | ZIP owner end-scroll/pick | result |
|---|---|---|---|
| iPhone 17 Pro | PASS — search stayed in view; final row `120 — RDO 120` reachable above keyboard and picked | PASS — digits-first keyboard; search stayed in view; final row `9712 — Taraka` reachable above keyboard and picked | PASS |
| iPhone SE (3rd generation) | PASS — same pinned/end/reachable/pick sequence | PASS — same pinned/end/reachable/pick sequence | PASS |
| macOS desktop | PASS — typed `0`, real scrolling reached/picked final row `120 — RDO 120` | PASS — typed `12`, real scrolling reached/picked final row `9712 — Taraka` | PASS |

Each picker has one `.mp4` recording and one end-state `.png`; exact notes are beside them in `iphone/`, `iphone-se/`, and `mac/`.
