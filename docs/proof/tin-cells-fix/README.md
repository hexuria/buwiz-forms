# TIN-cells fix proof — before (pinned kit ca39c0e) vs after (hexuria/nsh fix/otp-input-burst @ 5fa33438)

Rig: real desktop windows (`just nsh-run-agent macos`, scratch databases, safety vars
BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1), keys injected as real OS key events
(CGEvent keyDown+keyUp per digit, fixed gaps), values read from the gpui-agent view tree. Run by the machine's
testing agent; the same harness as docs/proof/tin-cells-check. What it does NOT show: Android/iOS hardware
(the keyboard-layout-change IME restart race is confirmed on emulators by other machines), and the forced
slow-commit dropped-key shape (macOS commit latency stays ~20-30ms even saturated; the instrumented-build
run is separate). No file here shows a live send: Queue was never pressed.

- before-matrix.txt / before-raw.txt — pinned kit: 0/10 correct at 10/20/30ms (20ms deterministically
  001234530 for 000123453), 10/10 at 50/70ms, loaded-machine case same permutation, never a digit lost.
- after-matrix.txt / after-raw.txt — fixed kit: 60/60 correct at all gaps including the loaded case.
- before-fast-20ms.mp4 / before-edge-70ms.mp4 — a failing fast burst and a correct edge-speed burst, before.
- after-fast-20ms.mp4 / after-edge-70ms.mp4 — the same two classes after: both correct.
- component-users-before.txt / component-users-after.txt — every user of the component at a 20ms burst:
  New taxpayer TIN, tax profile TIN edit, New branch branch code, sign-in TIN and PIN cells (PIN proven
  black-box through sign-in success/failure with the correct PIN). Before: all wrong. After: all correct.
- sixbox-345.txt + sixbox-after-burst.png — the six-box sign-in code on feat/relay-filing-signin @ 9fe51f23
  (PR #345) built against the fixed kit: the correct test code 246810 at a 20ms burst signs in. Sign-in path:
  the app's own test environment — EBIR_TEST_ENV dev build against `just mock-account` (loopback mock server,
  juana@example.com), never a real account.
- paste-before.txt / paste-after.txt — paste in all four formats (raw, dashed, spaced, 14-digit with branch
  code): correct before and after (paste was never broken; proof it stayed right).
