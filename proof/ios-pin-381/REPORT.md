# Apple UI evidence run — buwiz-forms PR #381 @ 5770b54a

Date: 2026-11-25. Scratch DB `/Users/devin/ios-pin-381/scratch-juana.db`; no product edits, no push, no real sign-in, Queue never pressed, `BIR_SFTP_LIVE` never set.

## Required matrix

Rows are box × sequence. Columns are input surfaces. One short word follows PASS/FAIL/n/a.

| row | iPhone on-screen | iPhone hardware | iPad on-screen | iPad hardware | Mac |
|---|---|---|---|---|---|
| A1 fast typing | PASS ordered | PASS exact | PASS ordered | PASS exact | PASS exact |
| A2 middle caret | PASS moved | PASS moved | FAIL end | FAIL end | FAIL end |
| A3 select + Cut | FAIL no-menu | PASS exact | FAIL no-menu | PASS exact | PASS exact |
| A4 Paste | FAIL no-insert | PASS exact | FAIL no-insert | PASS exact | PASS exact |
| A5 middle Backspace | PASS closes | PASS exact | FAIL wrong/end | FAIL wrong-digit | PASS exact |
| B1 fast typing | PASS bullets | PASS bullets | PASS bullets | PASS bullets | PASS bullets |
| B2 middle caret | PASS moved | FAIL end | PASS moved | FAIL end | PASS moved |
| B3 select + Cut | n/a secure | n/a secure | n/a secure | n/a secure | n/a secure |
| B4 Paste | PASS pasted | PASS pasted | PASS completed | FAIL no-effect | PASS pasted |
| B5 middle Backspace | FAIL cleared | FAIL no-effect | PASS one-less | FAIL no-effect | PASS one-less |
| C1 fast typing | PASS ordered | PASS ordered | PASS ordered | PASS ordered | PASS ordered |
| C2 middle caret | PASS moved | PASS moved | FAIL end | FAIL end | FAIL end |
| C3 select + Cut | PASS exact | PASS exact | FAIL no-menu | PASS exact | PASS exact |
| C4 Paste | PASS ordered | PASS ordered | FAIL no-insert | PASS exact | PASS exact |
| C5 middle Backspace | PASS closes | PASS exact | FAIL wrong/end | PASS exact | PASS exact |

### Matrix confidence notes

The iPhone cells are carried forward from the prior baseline run. The iPad and Mac cells were tested in this continuation except where explicitly described below. Mac C was independently tested on the sign-in authenticator-code PinCells with real mouse/keyboard. Fast typing `12345` landed in order (`mac-box-C-fast.png`); clicking the middle left the active cell at the end (`fail-mac-C-middle-caret.png`); Cmd+A/Cmd+X emptied the field and macOS `pbpaste` held `12345`; Cmd+V restored the digits; Left×2 then Backspace changed `12345` to `1245` (`mac-box-C-cut-paste-backspace.png`).

## Input paths used

- iPhone on-screen: accessibility-bridge taps on visible software-keyboard keys. The bridge could observe them.
- iPhone hardware: real computer keyboard events into the focused Simulator window with Connect Hardware Keyboard enabled.
- iPad on-screen: real mouse clicks on visible software-keyboard keys in the Simulator/DeviceHub window. No bridge typing.
- iPad hardware: real computer keyboard events into the focused Simulator window with Connect Hardware Keyboard enabled.
- Mac: real computer mouse and keyboard events in the eBIRForms window. Clipboard contents were inspected with `pbpaste`; simulator clipboard setup used `simctl pbcopy` only as test fixture setup, never as the input being tested.

## Detailed findings

### iPad on-screen

- A fast typing in the tax-profile TIN passed: `987-654-321` landed in order (`ipad-box-A-onscreen-fast.png`).
- A middle tap repeatedly left the active cell at the end (`fail-ipad-A-onscreen-middle-caret.png`).
- A long-press did not expose a usable selection/edit menu; Cut could not be performed.
- A paste through the visible keyboard paste control produced the iOS paste-permission prompt, but after Allow Paste the digits were not inserted (`fail-ipad-A-onscreen-paste.png`).
- B admin PIN fast typing produced ordered bullets (`ipad-box-B-onscreen-fast.png`). Tapping the middle moved the active position enough for a middle Backspace to reduce three bullets to two (`ipad-box-B-onscreen-caret.png`).
- B secure Cut is n/a: long-press exposes no Cut/selection UI. This is not proof of a broken input path by itself because secure iOS fields suppress copy/cut; however the same secure field did accept paste.
- C fast taps `12345` landed in order (`ipad-box-C-onscreen-fast12345.png`); tapping the sixth digit triggered completion.
- C middle tap stayed at the end and the next digit completed/submitted instead of entering in the middle (`fail-ipad-C-onscreen-middle-caret.png`).
- C unmasked long-press also did not open selection/edit UI in the tested field, unlike the empty TIN field where a Paste/Autofill menu did appear once. Thus the no-menu result is not merely because secure fields hide Cut.

### iPad hardware

- A fast typing, select-all Cut, Paste, and clipboard contents passed exactly. `simctl pbpaste` returned `87654321` after Cut and Cmd+V restored the digits in order (`ipad-box-A-hardware-fast.png`).
- A middle click stayed at the end (`fail-ipad-A-hardware-middle-caret.png`). Arrow/Backspace middle editing removed the wrong digit in the grouped TIN (`fail-ipad-A-hardware-middle-backspace.png`).
- B fast typing passed (`ipad-box-B-hardware-fast.png`). Middle click stayed at the end. Cmd+A visibly selected but Cmd+X did not alter the clipboard or field (`ipad-box-B-hardware-select-no-cut.png`); secure Cut is therefore correctly n/a. Cmd+V did not complete/paste in the tested secure field.
- C fast typing passed (`ipad-box-C-hardware-fast.png`). Middle click stayed at end (`fail-ipad-C-hardware-middle-caret.png`). Cmd+A/Cmd+X emptied the field and the clipboard held `12345`; Cmd+V restored it. Left-arrow positioning plus Backspace produced `1245` from `12345`, exactly removing the middle `3`.

### Mac

- A tax-profile TIN: fast typing `987654321` passed exactly (`mac-box-A-fast.png`); clicking the middle left the active cell at the end (`fail-mac-A-middle-caret.png`); Cmd+A/Cmd+X emptied the field and macOS `pbpaste` held `987654321`; Cmd+V restored it in order; Left×4 then Backspace changed `987654321` to `98764321`, exactly removing `5`.
- B Settings > Account > Security > Admin PIN: fast typing produced bullets (`mac-box-B-fast.png`). Click/backspace evidence shows middle positioning and one-digit removal (`mac-box-B-middle-backspace.png`). Secure Cmd+X did not cut; paste did insert. Cut is therefore n/a because the secure field/platform exposes no Cut path, not because selection itself is absent.
- C sign-in authenticator code: independently tested. Fast typing passed; middle click caret failed; Cut/Paste and arrow-positioned middle Backspace passed exactly. Evidence: `mac-box-C-fast.png`, `fail-mac-C-middle-caret.png`, `mac-box-C-cut-paste-backspace.png`.

## Mac file panels

- Save panel already verified in baseline: export profile wrote `/Users/devin/ios-pin-381/artifacts/BIR_Profile_00012345300000_1795663714.zip` (2,484 bytes), and the print preview Save as PDF panel wrote `/Users/devin/ios-pin-381/pdf-out/BIR Form 2551Q - Juana Santos.pdf` (346,178 bytes). Both panels were real panels attached to their app/preview windows (`mac-save-panel-export.png`, `mac-save-panel-print.png`).
- Open/import panel: exercised as the helpdesk admin. Settings > Import / Export > Import opened a real macOS Open panel modal to/attached to the eBIRForms window (`mac-open-panel-import.png`). The panel navigated to `artifacts`, selected the harmless scratch export `BIR_Profile_00012345300000_1795663714.zip`, and Import completed with the toast "Taxpayer data restored" (`mac-import-result.png`). No real data was used.

## Artifact index

Report: `/Users/devin/ios-pin-381/artifacts/testing-report.md`  
Recording: `/Users/devin/ios-pin-381/artifacts/iphone-tin-burst-cut-paste.mov`

iPad: `ipad-box-A-onscreen-fast.png`, `ipad-box-A-hardware-fast.png`, `ipad-box-B-onscreen-fast.png`, `ipad-box-B-onscreen-caret.png`, `ipad-box-B-hardware-fast.png`, `ipad-box-B-hardware-select-no-cut.png`, `ipad-box-C-onscreen-fast12345.png`, `ipad-box-C-hardware-fast.png`, plus `fail-ipad-*` images.

Mac: `mac-box-A-fast.png`, `mac-box-C-fast.png`, `fail-mac-C-middle-caret.png`, `mac-box-C-cut-paste-backspace.png`, `mac-open-panel-import.png`, `mac-import-result.png`, `fail-mac-A-middle-caret.png`, `mac-box-B-fast.png`, `mac-box-B-middle-backspace.png`, `mac-save-panel-export.png`, `mac-export-success.png`, `mac-save-panel-print.png`, `mac-print-pdf-saved.png`.

iPhone baseline: `iphone-box-A-*`, `iphone-box-B-*`, `iphone-box-C-*`, and `fail-iphone-*` images in the same directory.

Logs: `/Users/devin/ios-pin-381/artifacts/logs/`.

## Remaining gaps / caveats

1. Mac Open/import panel and independent Mac C evidence are now complete.
2. iPad on-screen Cut/Paste failures are real UI-path failures, but selection in these custom transparent-input cells is generally unavailable; secure-field Cut is separately n/a by platform behavior.
3. Full six-digit authenticator fast-burst verification is constrained by auto-submit-on-complete; ordered evidence was captured through the first five cells plus completion behavior.


## Main regression comparison addendum

See `COMPARISON.md` and `comparison/` for the one-at-main check of regression-looking FAIL cells. Four paste cells pass at main and therefore look like #381 regressions: A4 iPhone on-screen, A4 iPad on-screen, B4 iPad hardware, and C4 iPad on-screen. Most caret/middle-edit failures reproduce at main. One comparison cell (`B5 middle Backspace — iPad hardware`) remains untested for disk-safety reasons; `B5 middle Backspace — iPhone on-screen` is not exactly testable at main.
