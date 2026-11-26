# PR #398 P1-P5 interim report

Free disk first: **7.4 GiB free** (`df -H /System/Volumes/Data`: 7,579 MiB available) at the required stop point.

Build/setup: app `19e4b0e84cfc2f7b84a504a7729b363eecfcee59`; sibling NSH `bf52b6fe79f7abf0139cec81d760f0d0dca7949e`; scratch CLI `nsh 0.1 (bf52b6f)`; fresh target `/Users/devin/BuwizNsh/target-bf52b6fe`; scratch databases only. No product edits, no push, no Queue, no real sign-in/service, never `BIR_SFTP_LIVE`.

| Row | Result | Brief note |
|---|---|---|
| P1 A4 TIN digit boxes, iPad on-screen Paste | **PASS** | Real mouse long-press opened Paste; pasted TIN landed in order. |
| P2 C4 authenticator-code boxes, iPad on-screen Paste | **FAIL** | Repeated real mouse long-presses did not open an edit/Paste menu. |
| P3 B4 admin PIN boxes, iPad hardware Cmd+V | **PASS** | Real Cmd+V pasted the PIN digits and the field completed/submitted as expected. |
| P4 A4 TIN digit boxes, iPhone on-screen Paste | **PASS** | Real mouse long-press opened Paste; pasted TIN landed in order. |
| P5 iPad email-code boxes Paste, local mock | **PASS (hardware Cmd+V)** | Paste completed and verification advanced. However the on-screen long-press Paste menu did not appear. |
| P5 iPhone email-code boxes Paste, local mock | **PASS (hardware Cmd+V)** | Paste completed and verification returned to Account signed in as `juana@example.com`. However the on-screen long-press Paste menu did not appear. |

Failure/caveat pictures:

- P2 no Paste menu: `/Users/devin/ios-paste-bisect/pr398/artifacts/fail-P2-ipad-c4-no-paste-menu.png`
- P5 iPad no on-screen Paste menu: `/Users/devin/ios-paste-bisect/pr398/artifacts/fail-P5-ipad-no-paste-menu.png`
- P5 iPhone no on-screen Paste menu: `/Users/devin/ios-paste-bisect/pr398/artifacts/fail-P5-iphone-no-paste-menu.png`

Pass evidence additionally copied for P4/P5 where available:

- `/Users/devin/ios-paste-bisect/pr398/artifacts/pass-P4-iphone-tin-paste.png`
- `/Users/devin/ios-paste-bisect/pr398/artifacts/pass-P5-ipad-hardware-paste.png`
- `/Users/devin/ios-paste-bisect/pr398/artifacts/pass-P5-iphone-hardware-paste.png`

Mock log: `/Users/devin/ios-paste-bisect/pr398/artifacts/logs/mock-account.log`. The `/auth/email/*` proof lines are exactly two starts and two verifies:

```text
[mock-account] POST /auth/email/start
[mock-account] POST /auth/email/verify
[mock-account] POST /auth/email/start
[mock-account] POST /auth/email/verify
```

Important literal-compliance caveat: the complete mock log also contains local mock `GET /me`, `POST /api/devices`, and `POST /api/auth/logout` lines caused by successful sign-in/session/device behavior. Thus it is not literally true that only `/auth/email/*` lines appear. No non-local/real service was used.
