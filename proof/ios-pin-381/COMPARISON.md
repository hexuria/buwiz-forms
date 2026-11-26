# PR #381 Apple-input regression comparison at origin/main

Compared current `origin/main` app commit `b29e56bbfd00fcb8fa9c1bde5607260d4769e3e9` with NSH pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`, using fresh folder `/Users/devin/ios-pin-main/buwiz-forms`, fresh scratch DB `/Users/devin/ios-pin-main/scratch-main.db`, scratch CLI `/Users/devin/BuwizNsh/tools-main-8c78cfd/bin/nsh`, and scratch target `/Users/devin/BuwizNsh/target-main-b29e56b`.

No product source was edited, nothing was pushed, Queue was never pressed, no real service was used, and `BIR_SFTP_LIVE` was never set.

## Input paths

- iPad on-screen: real mouse clicks and long-presses in the Simulator window, using the visible on-screen keyboard/menu.
- iPhone on-screen keyboard digits: accessibility-bridge taps on the visible numeric keys. Field menu gestures were real mouse long-presses/clicks in the Simulator window.
- iPhone/iPad hardware: real computer-tool key events into the focused Simulator window with Connect Hardware Keyboard enabled.
- Mac: real computer-tool mouse clicks and keyboard events.

## Comparison matrix

| regression-looking #381 cell | result at main | notes |
|---|---|---|
| A2 middle caret — iPad on-screen | same-fail-old | Real mouse tap in a middle TIN cell left the active cell at the end. |
| A2 middle caret — iPad hardware | same-fail-old | Real click in a middle TIN cell left the active cell at the end. |
| A2 middle caret — Mac | same-fail-old | Real click in a middle TIN cell left the active highlight on the final cell. |
| A3 select+Cut — iPhone on-screen | same-fail-old | Main now shows an edit menu, unlike #381, but Select All did not leave a persistent selection and no Cut command appeared. The Cut sequence still could not be completed. |
| A3 select+Cut — iPad on-screen | same-fail-old | Main now shows an edit menu, unlike #381, but Select All did not leave a persistent selection and no Cut command appeared. |
| A4 Paste — iPhone on-screen | fixed-in-main | Paste inserted `000123453` exactly into the tax-profile TIN boxes. Evidence: `artifacts/main-iphone-A4-onscreen-paste-pass.png`. Because #381 failed here, this is a #381 regression. |
| A4 Paste — iPad on-screen | fixed-in-main | Paste inserted `000123453` exactly into the tax-profile TIN boxes. Evidence: `artifacts/main-ipad-A4-onscreen-paste-pass.png`. Because #381 failed here, this is a #381 regression. |
| A5 middle Backspace — iPad on-screen | same-fail-old | Visible-keyboard arrow/delete path removed the final digit (`000123453` → `00012345`), not a middle digit. |
| A5 middle Backspace — iPad hardware | same-fail-old | Left, Left, Backspace removed the final digit (`000123453` → `00012345`), not a middle digit. |
| B2 middle caret — iPhone hardware | same-fail-old | Three bullets were entered; clicking a middle PIN cell left the active position at the end. |
| B2 middle caret — iPad hardware | same-fail-old | Three bullets were entered; clicking a middle PIN cell left the active position at the end. |
| B4 Paste — iPad hardware | fixed-in-main | Cmd+V pasted enough scratch digits to complete the four-digit secure PIN and trigger the expected wrong-PIN submission, unlike #381's no-effect paste. Evidence: `artifacts/main-ipad-B4-hardware-paste-pass.png`. Because #381 failed here, this is a #381 regression. |
| B5 middle Backspace — iPhone on-screen | not-testable | Exact middle positioning could not be reached at main: tapping a middle secure-PIN cell blurred/dismissed the software keyboard, and the numeric keyboard has no arrow keys. This is a real platform/flow limitation in the main UI, not a pass. |
| B5 middle Backspace — iPhone hardware | same-fail-old | Left, Left, Backspace deleted the end digit rather than a middle digit; the required middle-delete sequence still failed. |
| B5 middle Backspace — iPad hardware | untested | The first attempt batched Backspace and Paste, so the intermediate post-Backspace state was not isolated. A focused retest was attempted, but the erased iOS simulator target had to rebuild; the rebuild was stopped when disk fell to about 2.2 GiB free, per the disk-safety instruction. |
| C2 middle caret — iPad on-screen | same-fail-old | Real mouse tap in a middle authenticator-code cell left the active cell at the end. |
| C2 middle caret — iPad hardware | same-fail-old | Real mouse click in a middle authenticator-code cell left the active cell at the end. |
| C2 middle caret — Mac | same-fail-old | After entering `12345`, clicking the third cell left the active highlight on the sixth/end cell. |
| C3 select+Cut — iPad on-screen | same-fail-old | Main now shows an edit menu, but Select All did not leave a persistent selection and no Cut command appeared. |
| C4 Paste — iPad on-screen | fixed-in-main | Paste inserted `12345` exactly into the authenticator-code boxes. Evidence: `artifacts/main-ipad-C4-onscreen-paste-pass.png`. Because #381 failed here, this is a #381 regression. |
| C5 middle Backspace — iPad on-screen | same-fail-old | Visible-keyboard arrow/delete path removed the final digit (`12345` → `1234`), not a middle digit. |

## Summary

Empirically regression-looking in PR #381:

- A4 Paste is fixed at main on iPhone on-screen and iPad on-screen.
- B4 Paste is fixed at main on iPad hardware.
- C4 Paste is fixed at main on iPad on-screen.

Most middle-caret and middle-Backspace failures reproduce at main and therefore look old, not introduced by #381. Cut is still not completable at main even though main sometimes exposes an edit menu that #381 did not; the end-to-end Cut outcome remains a failure at both revisions.

## Remaining coverage gap

`B5 middle Backspace — iPad hardware` remains untested. It requires one focused retest after enough disk space is available to rebuild the erased iOS simulator target. `B5 middle Backspace — iPhone on-screen` is not exactly testable through the main UI for the reason above.

## Artifacts

- `artifacts/main-iphone-A4-onscreen-paste-pass.png`
- `artifacts/main-ipad-A4-onscreen-paste-pass.png`
- `artifacts/main-ipad-B4-hardware-paste-pass.png`
- `artifacts/main-ipad-C4-onscreen-paste-pass.png`
- logs under `artifacts/logs/`
