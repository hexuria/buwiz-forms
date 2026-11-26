# iOS paste bisect proof

- app commit for endpoint/candidate bisect: `219da036370d46afcc4e0814a653912436d066d0`
- PR398/bf52 comparison app commit: `19e4b0e84cfc2f7b84a504a7729b363eecfcee59`
- kit commits tested: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`, `9fb1f7f8047c248b82fdcb94138c2a41743a3650`, `150aa172228362dbbb0bd92a6219836088e34499`; PR398 comparison used `bf52b6fe79f7abf0139cec81d760f0d0dca7949e`
- `07313d52` not run: not required after parent `9fb1f7f8` passed the comparable rows
- `nsh --version`: `nsh 0.6.1 (8c78cfd)` for 8c78; `nsh 0.6.1 (150aa17)` for 150aa; `nsh 0.6.1 (bf52b6f)` for bf52/PR398; `nsh -V` reported `(unknown)` for exact archived 9fb1 source
- `cargo --version`: `cargo 1.99.0 (5f94df478 2026-08-27)`
- build folders: `~/BuwizNsh/target-8c78cfd6`, `~/BuwizNsh/target-9fb1f7f8`, `~/BuwizNsh/target-150aa172` (each deleted after its kit); `~/BuwizNsh/target-bf52b6fe` retained for possible trace work

Safety: scratch databases only; local mock auth only for email-code rows; `BIR_SFTP_LIVE` never set; Queue never pressed; no real sign-in/service; real Simulator input only. `xcrun simctl pbcopy` only seeded pasteboards.

`REPORT.md` is the evidence table and detailed result. The crucial outcome is that parent `9fb1f7f8` passes all comparable P1-P5 rows; bf52 remains the first established failing candidate only for the PR398-observed P2/P5 on-screen paths. Endpoint `150aa172` unexpectedly passes all comparable rows on main. The requested P1 failing/passing recordings could not be produced because P1 did not fail at the first-bad candidate; see the report.
