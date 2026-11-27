# iPad portrait main repro — 4512a45a

App commit `4512a45a2fabf6ed98c24602b6fd3ee4b3194a74`; NSH pin verified as `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; CLI `nsh 0.6.1 (8c78cfd)`; target `~/BuwizNsh/target-4512a45a`.

Safety: scratch database; `SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN=1 SIMCTL_CHILD_EBIR_TEST_ENV=1 SIMCTL_CHILD_BIR_AUTH_BYPASS=1 SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1`; `BIR_SFTP_LIVE` never set; Queue never pressed; real Simulator input only; Connect Hardware Keyboard OFF.

## Results

| case | exact field text | rows |
|---|---|---|
| Portrait ZIP: type `12`, pick, reopen, letters + `makati` | exactly lowercase `makati` | YES — `1200`, `1224` |
| Portrait ZIP: fresh, letters first + `makati` | exactly lowercase `makati` | YES — `1200`, `1224` |
| Landscape ZIP: type `12`, pick, reopen, letters + `makati` | exactly lowercase `makati` | YES — `1200`, `1224` |
| Landscape ZIP: fresh, letters first + `makati` | exactly lowercase `makati` | YES — `1200`, `1224` |

No case showed `12makati`, `Makati`, stray spacing/punctuation, or autocorrect. The portrait `makati` matching failure was not reproduced.

RDO portrait clipping: five openings, no typing. Search field visible at approximately 0.5s, 1s and 3s in all five; no typing needed. Clipped openings: 0/5. Persistent off-screen search: 0/5. Opening clipping was not reproduced.

Evidence: `rdo-five-openings.mp4`, exact zoomed search-field screenshots, full orientation screenshots, and `notes.md`.
