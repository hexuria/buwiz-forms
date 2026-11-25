TIN-cells investigation evidence (origin/main 3aceafe8, branch try/tin-cells-check, real desktop window,
scratch db /tmp/tin-check.db, safety vars BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1).
Keys produced by keyinject: a Swift program posting REAL OS key events via CGEventCreateKeyboardEvent +
CGEventPost(.cgSessionEventTap), keyDown+keyUp per digit (8ms hold), fixed inter-key gap via usleep.

keyinject.swift — injector source (compiled binaries not committed; build: swiftc keyinject.swift -o keyinject) (CGEvent digit keys / Cmd+V / backspaces)
timing-results.txt / timing-raw.txt — threshold matrix, 7 gaps x 10 runs
timing-fail-30ms.mp4 — recording of a failing 30ms burst (result 001234530)
timing-ok-100ms.mp4 — recording of a correct 100ms burst
burst-frame-trace.txt + burst-frames-grid.png — frame-by-frame trace: mid-burst flicker, cells empty then refill rotated
paste-a.mp4..paste-e.mp4 + paste-results.txt — paste never corrupts (single atomic input event)
step4-cells-before-create.png — cells showing corrupted 001-234-530 before Create was pressed
census-results.txt + field-*.png — per-field burst/paste census (new taxpayer, profile edit, branch code)
stored-value.txt — sqlite proof: corrupted TIN is the taxpayers primary key
formpage-tin.png — form 1701 Part I: TIN auto-filled+locked from profile as 001-234-530-00000
print-tin.png — print preview: corrupted TIN prints in the official form layout
xml-excerpt.txt — draft payload excerpt carrying "tin":"00123453000000"
form-tin-escape.txt — step-4 exposure write-up
cause-analysis.txt — step-5 cause: code citations, empirical elimination, driver-vs-OS input paths, verdict on the redraw hypothesis
permute-sim.py — proof port of the end-only diff/edit logic: one mid-string caret insertion reproduces the exact observed corruption
