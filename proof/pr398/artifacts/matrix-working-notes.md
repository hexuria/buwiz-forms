# PR398 full matrix working notes (in progress)

iPhone hardware C:
- C1 fast 12345: PASS ordered (accessibility value exact).
- C2 click middle then type 9 into 1234: FAIL end (value 12349). artifact fail-C2-iphone-hardware-end.png.
- C3 shift-select last 2 of 12349 + Cmd-X: PASS exact; field 123, simulator clipboard 49.
- C4 Cmd+A/Backspace then Cmd+V 98765: PASS exact.
- C5 Left x2 then Backspace in 98765: PASS exact; value 9865.

Other cells not yet tested.
iPhone hardware B:
- B1 fast correct admin PIN 1234 + Open account: PASS; reached Home.
- B2 baseline 124, click middle, type 3, Open account: FAIL; Wrong admin PIN, so insertion landed wrong/end. artifact fail-B2-iphone-hardware-end.png.
- B3 shift-select last two of masked 123 + Cmd-X: n/a secure; bullets unchanged and clipboard remained stale 98765, so secure field exposes no Cut path.
- B4 paste correct 1234 with Cmd+V: PASS; signed in/reached Home.
- B5 controlled reconstruction 124 + Left + Backspace + type 23: PASS; reached Home, proving middle Backspace deleted 2 and close-up/reinsertion produced 1234.
iPhone hardware A:
- A1 fast 987654321: PASS exact. box image iphone-box-A-hardware.png.
- A2 baseline 12345678, click middle, type 9: FAIL end; exact value 123456789. artifact fail-A2-iphone-hardware-end.png.
- A3 shift-select last two of 123456789 + Cmd-X: PASS exact; field 1234567, clipboard 89.
- A4 Cmd+A/Backspace then Cmd+V 987654321: PASS exact.
- A5 Left x4 then Backspace in 987654321: PASS exact; value 98764321.

iPhone hardware column complete: A PASS/FAIL/PASS/PASS/PASS; B PASS/FAIL/n-a secure/PASS/PASS; C PASS/FAIL/PASS/PASS/PASS.
iPhone on-screen A:
- A1 fast keypad 123456789 through accessibility-bridge key presses: PASS exact. box image iphone-box-A-onscreen.png. Setup reset used hardware shortcuts only, not evidence input.
- A2 delete one to 12345678, click middle, keypad 9: FAIL end; exact 123456789. artifact fail-A2-iphone-onscreen-end.png.
- A3 long-press > Select All > Cut: PASS exact; field emptied and clipboard 123456789 (selection command was Select All).
- A4 long-press > Paste of 123456789: PASS exact after the paste turn; initial immediate read was empty due lag.
- A5 click middle then keypad Delete in 123456789: FAIL end; exact 12345678. artifact fail-A5-iphone-onscreen-end.png.

iPhone on-screen A complete: PASS/FAIL/PASS/PASS/FAIL.
iPhone on-screen C:
- C1 fast keypad 12345: PASS exact. box image iphone-box-C-onscreen.png.
- C2 click middle then keypad 9 into 1234: FAIL end; exact 12349. artifact fail-C2-iphone-onscreen-end.png.
- C3 long-press with nonempty field and nonempty pasteboard: FAIL; no edit/select/Cut menu. artifact fail-C3-iphone-onscreen-no-menu.png.
- C4 same long-press path: FAIL; no Paste menu. artifact fail-C4-iphone-onscreen-no-menu.png.
- C5 click middle then keypad Delete in 12349: FAIL end; exact 1234. artifact fail-C5-iphone-onscreen-end.png.

iPhone on-screen C complete: PASS/FAIL/FAIL/FAIL/FAIL.
iPhone on-screen B:
- B1 fast keypad correct 1234: PASS; reached Home.
- B2 baseline masked 124, exact click middle box, keypad 3: FAIL; field collapsed to one bullet rather than completing correct 1234. artifact fail-B2-iphone-onscreen-middle.png.
- B4 long-press > Paste with clipboard 123456789 (first four correct digits): PASS; signed in/reached Home.
- B3 long-press > Select All in masked 123: n/a secure; menu never exposed Cut, selection did not persist as a cuttable selection.
- B5 exact middle tap in masked 124: n/a no-path/unknown-main; tap blurred field and dismissed numeric keyboard, so no on-screen middle Backspace path exists. image fail-B2-iphone-onscreen-blur.png also documents blur.

iPhone on-screen B complete: PASS/FAIL/n-a secure/PASS/n-a no-path.
iPhone on-screen column complete: A PASS/FAIL/PASS/PASS/FAIL; B PASS/FAIL/n-a/PASS/n-a; C PASS/FAIL/FAIL/FAIL/FAIL.
Mac C:
- C1 fast typing 12345: PASS exact.
- C2 click middle then type 9 into 1234: FAIL end; exact 12349. artifact fail-C2-mac-end.png.
- C3 shift-select last two of 12349 + Cmd-X: PASS exact; field 123, clipboard 49.
- C4 Cmd+V 98765 after clearing: PASS exact.
- C5 Left x2 then Backspace in 98765: PASS exact; value 9865.

Mac C complete: PASS/FAIL/PASS/PASS/PASS. box image mac-box-C-fast.png (captured after later sequence state because single-column evidence image).
Mac B:
- B2 baseline 124, click middle, type 3: FAIL; wrong PIN auto-submit, so insertion did not reconstruct correct 1234. artifact fail-B2-mac-end.png.
- B3 shift-select last two of masked 123 + Cmd-X: n/a secure; bullets unchanged, clipboard remained stale 98765.
- B4 Cmd+V correct 1234 after reset: PASS; reached Home.
Mac A:
- A1 fast 987654321: PASS exact. box image mac-box-A-fast.png.
- A2 delete end to 98765432, click middle, type 9: FAIL end; exact 987654329. artifact fail-A2-mac-end.png.
- A3 shift-select last two of 987654329 + Cmd-X: PASS exact; field 9876543, clipboard 29.
- A4 Cmd+V 987654321 after clearing: PASS exact.
- A5 Left x4 then Backspace in 987654321: PASS exact; value 98764321.

Mac A complete: PASS/FAIL/PASS/PASS/PASS.
Mac D:
- Export save panel opened attached; scratch zip saved nonzero (2344 bytes). artifacts mac-save-panel-export.png mac-export-success.png.
- 2551Q print preview opened in second window; Save as PDF panel opened attached; scratch PDF saved nonzero. artifact mac-print-pdf-saved.png.
- Open/import panel opened attached; scratch zip chosen and import completed "Taxpayer data restored." artifacts mac-open-panel-import.png mac-import-result.png.
- B1 fast correct admin PIN 1234: PASS; reached Home.
- B5 controlled reconstruction 124 + Left + Backspace + type23: PASS; reached Home, proving middle Backspace deleted 2 and close-up/reinsertion produced 1234.

Mac column complete: A PASS/FAIL/PASS/PASS/PASS; B PASS/FAIL/n-a secure/PASS/PASS; C PASS/FAIL/PASS/PASS/PASS.
iPad hardware C:
- C1 fast hardware 12345: PASS exact. box image ipad-box-C-hardware-fast.png.
- C2 reset 1234, click middle, hardware 9: FAIL end; exact 12349. artifact fail-C2-ipad-hardware-end.png.
- C3 shift-select last two of 12349 + Cmd-X: PASS exact; field 123, simulator clipboard 49.
- C4 Cmd+V 98765 after clearing: PASS exact.
- C5 Left x2 then Backspace in 98765: PASS exact; value 9865.

iPad hardware C complete: PASS/FAIL old/PASS/PASS/PASS.
iPad hardware B:
- B2 baseline 124, click middle, hardware 3: FAIL; wrong admin PIN auto-submit, so insertion landed end/wrong. artifact fail-B2-ipad-hardware-end.png.
- B3 shift-select in masked 123 + Cmd-X: n/a secure; bullets unchanged and clipboard remained stale 98765.
- B5 controlled reconstruction 124 + Left + Backspace + type23: PASS; reached Home.
- B4 Cmd+V correct 1234: PASS; reached Home.
- B1 fast correct 1234: PASS; reached Home. box image ipad-box-B-hardware-fast.png.

iPad hardware B complete: PASS/FAIL old/n-a secure/PASS/PASS.
iPad hardware A:
- A1 fast hardware 987654321: PASS exact. box image ipad-box-A-hardware-fast.png.
- A2 delete end to 98765432, click middle, hardware 9: FAIL end; exact 987654329. artifact fail-A2-ipad-hardware-end.png.
- A3 shift-select last two of 987654329 + Cmd-X: PASS exact; field 9876543, simulator clipboard 29.
- A4 Cmd+V 987654321 after clearing: PASS exact.
- A5 Left x4 then Backspace in 987654321: PASS exact; value 98764321.

iPad hardware A complete: PASS/FAIL old/PASS/PASS/PASS.
iPad hardware column complete: A PASS/FAIL/PASS/PASS/PASS; B PASS/FAIL/n-a/PASS/PASS; C PASS/FAIL/PASS/PASS/PASS.
iPad on-screen A:
- A1 fast mouse-tapped software keys 123456789: PASS exact. box image ipad-box-A-onscreen-fast.png.
- A2 delete end to 12345678, tap middle, software key 9: FAIL end; exact 123456789. artifact fail-A2-ipad-onscreen-end.png.
- A3 long-press > Select All > Cut: PASS exact; field emptied, simulator clipboard 123456789 (selection command was Select All).
- A4 long-press > Paste of 123456789: PASS exact.
- A5 tap middle then software Delete in 123456789: FAIL end; exact 12345678. artifact fail-A5-ipad-onscreen-end.png.

iPad on-screen A complete: PASS/FAIL old/PASS/PASS/FAIL old.
iPad on-screen C:
- C1 fast mouse-tapped numeric keypad 12345: PASS exact. box image ipad-box-C-onscreen-fast.png.
- C2 delete end to 1234, tap middle, keypad 9: FAIL; tapping middle dismissed the numeric keypad and no digit was inserted. artifact fail-C2-ipad-onscreen-no-input.png.
- C3 long-press with nonempty 1234: FAIL; no edit/select/Cut menu after repeated real long-presses. artifact fail-C3-ipad-onscreen-no-menu.png.
- C4 clear then long-press empty field > Paste of 12345: PASS exact.
- C5 tap middle then software Delete in 12345: FAIL end; exact 1234. artifact fail-C5-ipad-onscreen-end.png.

iPad on-screen C complete: PASS/FAIL old/FAIL old/PASS/FAIL old.
iPad on-screen B:
- B2 baseline masked 124, tap middle, software key 3: FAIL; wrong admin PIN, so insertion did not reconstruct correct 1234. artifact fail-B2-ipad-onscreen-end.png.
- B3 long-press > Select All in masked 123: n/a secure; the follow-up menu exposed Paste/AutoFill but no Cut.
- B5 tap middle in masked 124: n/a no-path; tapping the middle opened Paste/Select All/AutoFill instead of exposing a middle-caret Backspace path. image ipad-B5-onscreen-no-path.png.
- B4 Paste menu with clipboard whose first four digits were correct 1234: PASS; reached Home.
- B1 fast mouse-tapped software keys correct 1234: PASS; reached Home. box image ipad-box-B-onscreen-fast.png.

iPad on-screen B complete: PASS/FAIL new/n-a secure/PASS/n-a no-path.
iPad on-screen column complete: A PASS/FAIL/PASS/PASS/FAIL; B PASS/FAIL/n-a/PASS/n-a; C PASS/FAIL/FAIL/PASS/FAIL.
