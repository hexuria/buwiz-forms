# proof/win-save-panel-fix — the Save as PDF panel fix (nsh f2c0490f) on Windows

App commit: buwiz-forms origin/main 75bd78b8437876060196c22111566758d860da0b (dev run, `just nsh-run-agent windows`).
Kit: sibling ../nsh at f2c0490fc3d374c8cfaf73febd59212cf454c1ec; the `nsh` CLI rebuilt from it: `nsh 0.6.1 (f2c0490)`.
The app's host and the CLI were both built from f2c0490f; the kit CI ran in its own target dir
(BuwizNsh/target-kitci-f2c0490f), the app build in BuwizNsh/target-nsh-f2c0490f (reached through a
junction at the recipe's hard-coded target-nsh path — no file was edited).
Machine: buwiz-forms-win, Windows Server 2022 Standard build 20348, EnableLUA=0, console session, 1600x1200.
Demo taxpayer only, 2551Q Q4 2026, taxable 100,000.00 at PT010, tax due 3,000.00. Queue never pressed.

The BEFORE pictures live on branch try/win-save-pdf (commits e0407d10 / cd34283c) in proof/win-save-pdf/.

## What each file shows — and its BEFORE match

- recording-steps2-4.mp4 (170 s, 2x speed of the 339 s original): preview open, real click on
  Save as PDF, panel over the preview, save, repeat, export panel, import panel, print dialog, cancel.
- 01-realclick-preparing.png — within ~1 s of the real click: "Preparing the PDF…" (matches BEFORE 06-realclick-preparing-pdf.png).
- 02-panel-up.png — the save panel OVER THE PREVIEW (BEFORE 07-realclick-after-30s.png had it at the
  screen's top-left corner with the main window forward).
- enum-step2a.txt — the panel's owner is the PREVIEW hwnd 0x12023C, rect (164,187)-(964,787) inside the
  preview's (156,156)-(1112,1295) (BEFORE windows-step3a-realclick.txt: owner was the MAIN window, rect (0,0)).
- 03-after-save-preview-still-front.png — after saving, the preview is STILL in front of the main window,
  note "Saved saved-fixed.pdf" (BEFORE 08-after-save-preview-behind.png: the preview fell behind).
- enum-step2a-aftersave.txt — no dialog left, both NshWindows.
- 04-panel-up-second-time.png + enum-step2b.txt — Step 2b repeat: same owner, same position.
- 05-export-panel.png + enum-step2c-export.txt — export save panel owned by the MAIN window 0x2402E2
  (BEFORE 14-export-save-panel.png; same correct behavior before and after).
- 06-import-panel.png + enum-step2c-import.txt — import open panel owned by the MAIN window.
- 07-print-dialog.png + enum-step3-print.txt — the print dialog owned by the PREVIEW hwnd 0x120352
  (the preview's hwnd of this opening), over the preview (BEFORE 16-print-dialog.png).
- 08-after-print-cancel.png — after cancelling the print dialog; no printing was done this time.
- app-log-full.txt — the complete BIR_LOG_DIR log (39 lines): startup lines and one cron line only;
  NOTHING logged at any press, as before the fix.
- saved-fixed.pdf, saved-fixed2.pdf — the files written through the panel (272,149 bytes, 2 pages, %PDF-1.4).

## One-line answers

1. Panel owner from the preview: the PREVIEW window — fixed (was the main window).
2. Preview in front during and after: yes, it never falls behind now.
3. Panel owner from the main window (export/import): the main window, as expected.
4. Print dialog over the preview: yes, owned by the preview.
5. Kit CI at f2c0490f: clippy PASS; "check without optional features" FAIL (dead-code MIN_WIDTH/MIN_HEIGHT);
   test FAIL one test (an_app_without_patches_of_nsh_gets_the_same_workspace) — control results at the parent
   8c78cfd6 in the session report (these two failures look unrelated to the fix: it touches neither).
