# PR425 macOS re-proof — fef911cf

App commit `fef911cf8baf979e5cb399cf0bf93a4593e214e5`; NSH pin verified as `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; CLI `nsh 0.6.1 (8c78cfd)`; target `~/BuwizNsh/target-fef911cf`.

Safety: scratch database only; `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; `BIR_SFTP_LIVE` never set; Queue never pressed; no real account or service. Evidence used real mouse/keyboard input in the macOS desktop production `App.tsx` window, initially wider than 600pt.

## Results

| check | result |
|---|---|
| RDO row click after typing `makati` | PASS 5/5 — clicked `050 — South Makati`; box showed code+name; no not-taken note |
| RDO chevron close/reopen | PASS 3/3 |
| RDO click away into Phone | PASS 3/3; note YES 3/3 — list closed, RDO reverted to `050 — South Makati`, note visible |
| Cmd+Tab away/back with `makati` typed | PASS — list closed; box showed last pick `050 — South Makati` with note |
| Narrow linked date sheet opens once | PASS — close → widen → narrow did not reopen it |

Main video: `mac/pr425-fef-mac-reproof.mp4`. See `mac/notes.md` and screenshots beside it.
