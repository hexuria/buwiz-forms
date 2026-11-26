# fix/money-box-keep-text-on-focus @ 7eacb1d2195b8dc1e1a1b65ad97728c15b50bc2f — Mac real-key rig

Build line: app exactly 7eacb1d2 (rev-parse confirmed), kit sibling 8c78cfd6 (nsh 0.6.1 (8c78cfd)), cargo 1.100.0 toolchain's own, RUSTC_WRAPPER empty, build folder /Users/devin/moneykt/target, scratch db, four safety variables (BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1), invented values only, Queue never pressed.

All typing by real CGEvent key events. Saved values read from the scratch draft database, not the screen.

## X cases (entry, clicks, comma, paste) — transcripts x1-5.log, x6-10.log, x6-8redo.log, x22-24.log, scans xscan.log/xscan4.log

X1 Tab into 1,234.56, type 15000.50: platform select-all stands; first key replaces whole; plain digits per key; leaving 15,000.50; saved 15000.5. RIGHT.
X2 Shift+Tab in: identical. RIGHT.
X3 click, Cmd+A, type: identical. RIGHT.
X4 click, Cmd+A, Delete, type: identical. RIGHT.
X5 Tab into 12,345,678.90, type: replaced whole. RIGHT.
X6 first click between the 2 and the 3, key 9: 1,2934.56 in the box; leaving 12,934.56; saved 12934.56. RIGHT — hit-testing glyph-accurate (scan xscan4.log: positions map to the drawn glyphs; the text in this build sits at screen x1305-1370).
X7 first click just after the last digit (x1371): caret reaches end (unlike main); 1,234.569; leaving 1,234.57; saved 1234.57. RIGHT.
X8 first click empty left part: 9 at the front, 91,234.56. RIGHT.
X9 zero box, click, type: box empty on focus as designed; plain digits; 15,000.50 exact. RIGHT.
X10 zero box, Tab in, type: identical. RIGHT.
X22a typed comma 1 5 , 0 0 0 . 5 0: comma echoed as typed; leaving 15,000.50; saved exact. RIGHT.
X22b comma as decimal 15000,50: box echoes as typed; leaving **1,500,050.00**; saved 1500050.0 — the comma is treated as a thousands separator, so the number visibly becomes a hundred times larger. Shown == saved (not silent), but the typed meaning is not what is kept. FLAGGED for the author.
X23 pastes into 0.00 by real Cmd+V: '₱ 15,000.50', ' 15000.50 ', '15,000.50', 'PHP 15000.50' all normalize to 15000.50, saved exact; '15000.50.25' refused entirely (box stays empty, 0.00 saved 0). RIGHT.
X24 paste '15,000.50' over a Tab-selected filled amount: replaced, 15,000.50 exact. RIGHT.
No "the box shows ..., but the draft stored ..." message in any case.

## Hunt cases — transcripts h1.log, h2.log, h4.log, h5.log, h6.log

H1 click into 0.00 + immediate zero-delay burst 15000.50, 10x, and Tab version 10x: never a leading 0.00; every run exact. RIGHT.
H2 zero-delay typos 15000..50 and 150a00.50: second point and letter never appear; both exact. RIGHT.
H3 (form focus into a filled amount): not directly reachable in 2551Q/2550M — the stepper targets a row's first invalid control (the ATC dropdown), and a filled valid amount is never a to-do. Mechanism evidence: the pill does move keyboard focus into fields; typing into a pill-focused FILLED plain field appended (021234567 -> 021234567021234567, no selection). The amount shares that focus path; only the platform's own Tab selects. Reads as: form-focus into a filled amount appends.
H4 2550M recomputed editable box (13B = 13A x 0.12): type base 10000, Tab into form-filled 1,200.00, type 999: replaced whole every time (5x 120ms, 5x burst); leaving 999.00; draft sales_to_government 10000.0, output_tax_government 999.0 exact. RIGHT.
H5 int box (txtSheets, number of attached sheets, holding 0): CLICK in, type 42 -> box '04', the 2 never lands; leaving '04'; saved **4**. FAULT (the click into zero does not select, first digit appends after the 0, the rest is refused by the mask). Tab in: correct (zero replaced, 42, saved 42). The money zero box is unaffected (it shows empty on focus); percent shares the int code path in this head (both lost the numeric select wrapper); no editable percent typing box in these two forms (2551Q's rate is a stepper group).
H6 Cmd+Z does nothing in the amount box (no undo: typed text stays; deletion stays deleted); saved == shown both ways. Noted.
H7 the mismatch message never appeared anywhere in the whole rig.
H8 just nsh-shots select: see h8.log (result in the final report).

## Recordings (app window only)
x1.mp4 — X1 (Tab in, type 15000.50) and X5.
x6-x9.mp4 — X6 redo (click between 2 and 3, key 9) and X9 (zero box, click, type).
x6-x10-original-run.mp4 — the first X6-X10 pass (superseded aims; kept for completeness).
h5-fault.mp4 — H5 fault re-enactment in 2550M txtSheets: 0, click, type 42 -> 04, saved 4.
