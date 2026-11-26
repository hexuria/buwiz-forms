# PR #398 Apple input matrix report

Build under test: buwiz-forms `19e4b0e84cfc2f7b84a504a7729b363eecfcee59` with sibling nsh `bf52b6fe79f7abf0139cec81d760f0d0dca7949e`; scratch CLI/target/database only. `BIR_SFTP_LIVE` was never set, Queue was never used, and no real service/sign-in was used. Input was real platform input: iPhone software keys through the accessibility bridge, iPad software keys by real mouse clicks/long-presses, hardware columns by real keyboard events into focused Simulator windows, and Mac by real mouse/keyboard. `simctl pbcopy` was used only to set pasteboard fixtures; read-only pasteboard checks verified Cut/Paste outcomes.

## Complete matrix

`new` follows the user's simplified rule for failing cells outside the known-old main list; main was not directly rerun in this job. `n/a` means the platform/field did not expose the exact input path, not a product pass.

| Row | iPhone on-screen | iPhone hardware | iPad on-screen | iPad hardware | Mac |
|---|---|---|---|---|---|
| A1 fast typing | PASS | PASS | PASS | PASS | PASS |
| A2 middle caret | FAIL new | FAIL new | FAIL old | FAIL old | FAIL old |
| A3 select+Cut | PASS | PASS | PASS | PASS | PASS |
| A4 Paste | PASS | PASS | PASS | PASS | PASS |
| A5 middle Backspace | FAIL new | PASS | FAIL old | PASS | PASS |
| B1 fast typing | PASS | PASS | PASS | PASS | PASS |
| B2 middle caret | FAIL new | FAIL old | FAIL new | FAIL old | FAIL new |
| B3 select+Cut | n/a secure | n/a secure | n/a secure | n/a secure | n/a secure |
| B4 Paste | PASS | PASS | PASS | PASS | PASS |
| B5 middle Backspace | n/a unknown-main | PASS | n/a no-path | PASS | PASS |
| C1 fast typing | PASS | PASS | PASS | PASS | PASS |
| C2 middle caret | FAIL new | FAIL new | FAIL old | FAIL old | FAIL old |
| C3 select+Cut | FAIL new | PASS | FAIL old | PASS | PASS |
| C4 Paste | FAIL new | PASS | PASS | PASS | PASS |
| C5 middle Backspace | FAIL new | PASS | FAIL old | PASS | PASS |

## Mac panels (D)

- Save/export: PASS. Real save panel opened attached; scratch zip was saved nonzero (2,344 bytes). Evidence: `artifacts/mac-save-panel-export.png`, `artifacts/mac-export-success.png`.
- Open/import: PASS. Real Open panel opened attached; scratch zip `import-scratch.zip` was chosen and import completed with “Taxpayer data restored.” Evidence: `artifacts/mac-open-panel-import.png`, `artifacts/mac-import-result.png`.
- Print preview Save as PDF: PASS. 2551Q print preview opened in a second window; Save as PDF panel opened and `/Users/devin/ios-paste-bisect/pr398/pdf-out/BIR Form 2551Q - Juana Santos.pdf` is nonzero (344,656 bytes). Evidence: `artifacts/mac-print-pdf-saved.png`.

## iPad evidence details

### iPad hardware

- A: fast PASS exact; middle caret FAIL end (`987654329`); Cut PASS exact (field `9876543`, clipboard `29`); Paste PASS exact; middle Backspace PASS exact (`98764321`).
- B: fast correct PIN PASS; middle caret FAIL (wrong-PIN submission); secure Cut n/a; paste correct PIN PASS; controlled middle-Backspace reconstruction PASS.
- C: fast PASS exact; middle caret FAIL end (`12349`); Cut PASS exact (field `123`, clipboard `49`); Paste PASS exact; middle Backspace PASS exact (`9865`).

### iPad on-screen

- A: fast mouse-tapped keys `123456789` PASS exact; middle caret FAIL end; long-press > Select All > Cut PASS exact (clipboard `123456789`); long-press > Paste PASS exact; middle Backspace FAIL end (`12345678`).
- B: fast correct PIN PASS; middle caret FAIL (wrong-PIN submission); secure Cut n/a because Select All did not expose Cut; Paste PASS; middle Backspace n/a because tapping the middle opened Paste/Select All/AutoFill rather than exposing a middle-caret path.
- C: fast numeric-keypad taps PASS exact; middle caret FAIL because tapping the middle dismissed the numeric keypad and no digit inserted; Cut FAIL because repeated long-presses on the nonempty field exposed no edit/select/Cut menu; Paste PASS exact after clearing and long-pressing the empty field; middle Backspace FAIL end (`1234`).

Important iPad caveat: C4 Paste passes, but only through the empty-field Paste menu. A long-press on the nonempty code field did not expose an edit menu; that is why C3 remains FAIL.

## iPhone evidence details

### iPhone hardware

- A: fast PASS; middle caret FAIL end (`123456789`); Cut PASS exact; Paste PASS exact; middle Backspace PASS exact.
- B: fast correct PIN PASS; middle caret FAIL (wrong-PIN submission); secure Cut n/a; Paste PASS; controlled middle-Backspace reconstruction PASS.
- C: fast PASS; middle caret FAIL end (`12349`); Cut PASS exact; Paste PASS exact; middle Backspace PASS exact.

### iPhone on-screen

- A: fast PASS exact; middle caret FAIL end; Select All + Cut PASS exact; Paste PASS exact; middle Backspace FAIL end.
- B: fast correct PIN PASS; middle caret FAIL/collapsed; secure Cut n/a; Paste PASS; exact middle-Backspace path n/a because tapping blurred/dismissed the numeric keyboard (`unknown-main`).
- C: fast PASS; middle caret FAIL end; Cut FAIL because no edit menu; Paste FAIL because no Paste menu; middle Backspace FAIL end.

## Mac evidence details

- A: fast PASS exact; middle caret FAIL end (`987654329`); Cut PASS exact (`29` on clipboard); Paste PASS exact; middle Backspace PASS exact (`98764321`).
- B: fast correct PIN PASS; middle caret FAIL (wrong PIN auto-submit); secure Cut n/a; Paste PASS; controlled middle-Backspace reconstruction PASS.
- C: fast PASS exact; middle caret FAIL end (`12349`); Cut PASS exact (`49` on clipboard); Paste PASS exact; middle Backspace PASS exact (`9865`).

## Artifact index

Detailed working notes: `artifacts/matrix-working-notes.md`.

Representative box images:
- `artifacts/iphone-box-A-onscreen.png`, `artifacts/iphone-box-A-hardware.png`, `artifacts/iphone-box-C-onscreen.png`
- `artifacts/ipad-box-A-onscreen-fast.png`, `artifacts/ipad-box-A-hardware-fast.png`
- `artifacts/ipad-box-B-onscreen-fast.png`, `artifacts/ipad-box-B-hardware-fast.png`
- `artifacts/ipad-box-C-onscreen-fast.png`, `artifacts/ipad-box-C-hardware-fast.png`
- `artifacts/mac-box-A-fast.png`, `artifacts/mac-box-C-fast.png`

iPad failure/path images:
- `artifacts/fail-A2-ipad-onscreen-end.png`, `artifacts/fail-A5-ipad-onscreen-end.png`
- `artifacts/fail-B2-ipad-onscreen-end.png`, `artifacts/ipad-B5-onscreen-no-path.png`
- `artifacts/fail-C2-ipad-onscreen-no-input.png`, `artifacts/fail-C3-ipad-onscreen-no-menu.png`, `artifacts/fail-C5-ipad-onscreen-end.png`
- `artifacts/fail-A2-ipad-hardware-end.png`, `artifacts/fail-B2-ipad-hardware-end.png`, `artifacts/fail-C2-ipad-hardware-end.png`

Other failure images include:
- `artifacts/fail-A2-iphone-onscreen-end.png`, `artifacts/fail-A5-iphone-onscreen-end.png`
- `artifacts/fail-B2-iphone-onscreen-middle.png`, `artifacts/fail-B2-iphone-onscreen-blur.png`
- `artifacts/fail-C2-iphone-onscreen-end.png`, `artifacts/fail-C3-iphone-onscreen-no-menu.png`, `artifacts/fail-C4-iphone-onscreen-no-menu.png`, `artifacts/fail-C5-iphone-onscreen-end.png`
- `artifacts/fail-A2-iphone-hardware-end.png`, `artifacts/fail-B2-iphone-hardware-end.png`, `artifacts/fail-C2-iphone-hardware-end.png`
- `artifacts/fail-A2-mac-end.png`, `artifacts/fail-B2-mac-end.png`, `artifacts/fail-C2-mac-end.png`

Logs: `artifacts/logs/`. Mock/interim evidence remains in `interim-report.md` and the same artifact directory.

Final free disk after completing the iPad matrix: about 14 GiB.
