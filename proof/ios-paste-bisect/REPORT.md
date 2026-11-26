# NSH paste bisect report

App under test for bisect kits: hexuria/buwiz-forms main `219da036370d46afcc4e0814a653912436d066d0`; only sibling `../nsh` changed. Scratch databases only. Real mouse/key input only; `simctl pbcopy` used only to seed pasteboards. P5 used the local mock account server bound to `127.0.0.1:3008`, `juana@example.com`, mock-generated codes, and no `BIR_AUTH_BYPASS` in the simulator child environment. Queue, real services and `BIR_SFTP_LIVE` were never used.

| cell | 8c78cfd6 | 9fb1f7f8 | bf52b6fe | 07313d52 | 150aa172 |
|---|---|---|---|---|---|
| P1 iPad TIN on-screen Paste | PASS | PASS | PASS* | – | PASS |
| P2 iPad authenticator-code on-screen Paste, nonempty-field path | PASS | PASS | FAIL* | – | PASS |
| P3 iPad PIN hardware Cmd+V | PASS | PASS | PASS* | – | PASS |
| P4 iPhone TIN on-screen Paste | PASS | PASS | PASS* | – | PASS |
| P5 iPad email-code Paste | PASS | PASS | FAIL*† | – | PASS |
| P5 iPhone email-code Paste | PASS | PASS | FAIL*† | – | PASS |
| P6 iPad one-time-code suggestion observation | no suggestion shown | – | – | – | no suggestion shown |
| P6 iPhone one-time-code suggestion observation | no suggestion shown | – | – | – | no suggestion shown |

`*` bf52b6fe values are the previously established PR398 outcomes, not results on this main app revision. They are included only as a comparison reference because the app revision differs. For P2/P5 the exact on-screen long-press Paste path failed even though hardware Cmd+V or later workarounds could succeed.

`†` For bf52b6fe P5, the on-screen long-press Paste menu did not appear in PR398; hardware Cmd+V did complete verification. In this bisect the rows are classified by the on-screen Paste path for comparability.

## Empirical bisect outcomes

### nsh 8c78cfd6

- P1 PASS: paste menu appeared; `000123453` landed in order.
- P2 PASS: starting from nonempty `12`, paste menu appeared at the active end and `345` completed `12345`.
- P3 PASS: hardware Cmd+V pasted correct PIN `4321` and sign-in reached Home.
- P4 PASS: paste menu appeared; `000123453` landed in order.
- P5 iPad/iPhone PASS: paste menu appeared in the email-code boxes; mock verify succeeded. Codes were `246810`/`246811` as printed by the mock.
- P6 iPad/iPhone: no one-time-code/AutoFill code suggestion appeared above the keyboard when focusing the email-code boxes.

### nsh 150aa172

- P1 PASS: paste menu appeared; `000123453` landed in order.
- P2 PASS: starting from nonempty `12`, paste menu appeared at the active end and `345` completed `12345`.
- P3 PASS: hardware Cmd+V pasted correct PIN `4321` and sign-in reached Home.
- P4 PASS: paste menu appeared after refocusing the field; pasted TIN landed in order.
- P5 iPad/iPhone PASS: paste menu appeared after focusing/refocusing the email-code field; mock verify succeeded. Codes were `246810`/`246811` as printed by the mock.
- P6 iPad/iPhone: no one-time-code/AutoFill code suggestion appeared above the keyboard when focusing the email-code boxes.

## Current conclusion

`9fb1f7f8` passes every required row, so the parent does **not** fail the comparable Paste paths. Therefore bf52b6fe remains the first established failing candidate in the comparable PR398 path. This supports, but does not completely prove, bf52b6fe as first bad: the bf52b6fe failures were established on PR398 and bf52b6fe itself was not rerun on this main app revision. In addition, `150aa172` did not reproduce any requested Paste failure on main. `07313d52` is not needed under the user's rule: there are no TIN/auth Paste differences left to explain, because 9fb1f7f8 and both endpoints pass the comparable Paste rows.

### nsh 9fb1f7f8

- P1 PASS: paste menu appeared; `000123453` landed in order.
- P2 PASS: using a nonempty `12` field, the active-end long-press exposed Paste and `345` completed `12345`.
- P3 PASS: hardware Cmd+V pasted correct PIN `4321` and sign-in reached Home.
- P4 PASS: paste menu appeared; pasted TIN landed in order.
- P5 iPad/iPhone PASS: paste menu appeared in the email-code boxes; mock verify succeeded. Codes were `246810`/`246811`.
- P6 was not required for this kit.
- CLI was built from exact commit `9fb1f7f8047c248b82fdcb94138c2a41743a3650`; because the bisect source is a git archive rather than a checkout, `nsh -V` reports `(unknown)`, but the source commit is exact.

## Artifacts

Evidence images are under `/Users/devin/ios-paste-bisect/bisect/artifacts/images/` and logs under `/Users/devin/ios-paste-bisect/bisect/artifacts/logs/`.

Important logs:

- `150aa-ipad-build.log` — includes earlier disk-caused interrupted build attempts before the successful build; final lines show the successful connected run.
- `mock-account-8c78-final.log`
- `mock-account-150aa.log`

The mock logs contain local `/auth/email/start` and `/auth/email/verify` proof lines, plus expected local session/device lines (`/me`, `/api/devices`, `/oauth/token`, `/api/auth/logout`) caused by sign-in/sign-out behavior. No external service was contacted.

## Build/disk caveats

- During the first 150aa build, deleting the host `debug` directory while dependent compilation was still active caused missing-crate/object errors. The build was restarted and completed successfully without product or source edits. This is a testing-environment caveat only.
- After each completed kit, its fresh target was deleted as requested.
- Free disk at report time: see final session reply (about 9.4 GiB immediately after deleting the 150aa target).
