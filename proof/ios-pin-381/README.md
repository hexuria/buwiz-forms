# Apple input evidence for PR #381

- buwiz-forms commit: `5770b54a47ad6e9115dcf95f9229a07d54f669a5`
- `nsh --version`: `nsh 0.6.1 (150aa17)`
- `../nsh` commit: `150aa172228362dbbb0bd92a6219836088e34499`
- `cargo --version`: `cargo 1.99.0 (5f94df478 2026-08-27)`
- build folder: `/Users/devin/BuwizNsh/target-5770b54a`
- scratch CLI folder: `/Users/devin/BuwizNsh/tools-150aa172`
- scratch database: Juana Santos only, with scratch helpdesk sign-in state and one scratch 2551Q draft

Safety: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; `SIMCTL_CHILD_*` forms on the Simulators; `GOOGLE_*` unset; `BIR_SFTP_LIVE` never set; Queue never pressed; no real service signed in.

Input paths: iPhone on-screen key taps through the accessibility bridge; iPad on-screen key taps by real mouse in the device window; hardware-keyboard columns by real key events into the focused device window with Connect Hardware Keyboard enabled; Mac by real mouse and keyboard events. gpui-agent typing/paste/key synthesis was not used as evidence.

See `REPORT.md` for the full 15-row matrix and artifact index. The required iPhone TIN fast-burst/Cut/Paste recording is `iphone-tin-burst-cut-paste.mov`; its Cut/Paste portion documents that the edit menu did not appear in the taxpayer edit sheet.

## Main regression comparison

`COMPARISON.md` compares the regression-looking FAIL cells once at `origin/main` `b29e56bbfd00fcb8fa9c1bde5607260d4769e3e9` with NSH pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`. Paste failures in A4 iPhone/iPad on-screen, B4 iPad hardware and C4 iPad on-screen pass at main and therefore look like PR #381 regressions. Most middle-caret/middle-Backspace failures reproduce at main and look old. `B5 middle Backspace — iPad hardware` remains untested because the iOS target rebuild was stopped for disk safety; `B5 middle Backspace — iPhone on-screen` has no exact real-input path at main.
