# PR #398 Apple input proof

- app commit: `19e4b0e84cfc2f7b84a504a7729b363eecfcee59` (`hexuria/buwiz-forms` PR #398 head)
- nsh/kit commit: `bf52b6fe79f7abf0139cec81d760f0d0dca7949e`
- `nsh --version`: `nsh 0.6.1 (bf52b6f)`
- `cargo --version`: `cargo 1.99.0 (5f94df478 2026-08-27)`
- build folder: `/Users/devin/BuwizNsh/target-bf52b6fe`
- scratch CLI folder: `/Users/devin/BuwizNsh/tools-bf52b6fe`

Safety: scratch databases only; `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1` on every run; `BIR_AUTH_BYPASS=1` except the local mock email flow where it was intentionally unset; `SIMCTL_CHILD_*` used on Simulators; `BIR_SFTP_LIVE` never set; Queue never pressed; no real sign-in/service.

Input paths: iPhone software-keyboard taps through the accessibility bridge; iPad software-keyboard taps by real mouse in the device window; hardware columns by real keyboard events into focused Simulator windows; Mac by real mouse/keyboard. `xcrun simctl pbcopy` only prepared pasteboard fixtures.

Files:
- `interim-report.md`: first P1-P5 checkpoint.
- `full-report.md`: complete 15-row matrix and Mac panel results.
- `artifacts/`: failure and representative box/panel images plus matrix notes and local mock log.
- `pdf-out/print-preview-2551q.pdf`: nonzero scratch print-preview PDF.

Coverage caveat carried in the reports: `new` classification follows the owner's simplified rule; main was not directly rerun in this PR398 job. `B5` secure-PIN paths are n/a where no exact middle-caret real-input path exists.
