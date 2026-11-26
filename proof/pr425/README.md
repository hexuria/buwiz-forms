# PR425 macOS click proof — c4262697

App commit `c42626973461a21a2e8ab58caca2405ff0891252`; NSH pin verified as `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; CLI `nsh 0.6.1 (8c78cfd)`; target folder `~/BuwizNsh/target-c4262697`.

Safety: scratch database only; `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; `BIR_SFTP_LIVE` never set; Queue never pressed; no real account or service. Evidence used real mouse/keyboard input in the macOS desktop production `App.tsx` window.

## Results

| check | result |
|---|---|
| RDO row click after typing `makati` | PASS 5/5 — clicked `050 — South Makati`; box showed code+name; no “not in the list”/not-taken note |
| RDO chevron close/reopen after typing `makati` | PASS 3/3 — first click closed; second reopened |
| RDO click away into Phone after typing `makati` | PASS 3/3 — list closed; RDO returned to `050 — South Makati`; note said typed text fits 4 choices / was not taken |
| Narrow linked date sheet opens once | PASS — under 600pt the form link opened Edit profile → Registered activity and date sheet once; close → widen → narrow did not reopen it |

Main video: `mac/pr425-mac-click-proof.mp4`. See `mac/notes.md` and screenshots beside it.
