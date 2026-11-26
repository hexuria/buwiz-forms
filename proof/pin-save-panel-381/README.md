# Proof — PR #381 pin move: print-preview "Save as PDF" panel opens in front of the preview (Windows)

App commit: 5770b54a47ad6e9115dcf95f9229a07d54f669a5 (hexuria/buwiz-forms#381 head)
Kit (../nsh) commit: 150aa172228362dbbb0bd92a6219836088e34499 (9fb1f7f8 is an ancestor: yes)
nsh --version: nsh 0.6.1 (150aa17) — tool rebuilt from the pin
Build folder: ~/BuwizNsh/target-nsh (deleted first, exclusive)
Machine: Windows Server 2022 build 20348, EnableLUA=0, console session, 1600x1200, cargo 1.99.0, RUSTC_WRAPPER empty.
All clicks on the panels were REAL mouse clicks (pyautogui/SendInput), never agent presses.

## Files and what each shows

- 01-preview-open.png — print preview window open in front of the main window (2551Q Q4 2026, Juana Santos, PT010 100,000.00, tax 3,000.00). Matches before `proof/win-save-pdf/02-preview-open-1-front.png` and after `proof/win-save-panel-fix/` (no direct equivalent; the after run starts at the click).
- 02-pass-a-click-plus-1s.png — 1 s after the real click on "Save as PDF" (first pass): "Preparing the PDF..." state. Matches before `06-realclick-preparing-pdf.png`, after `01-realclick-preparing.png`.
- 03-pass-a-panel-up.png — the Save as PDF panel UP, in front of the preview (first pass). Matches after `02-panel-up.png`; the before branch has NO matching picture — there the panel never came up in front (the bug).
- 04-pass-a-after-save.png — after saving; preview in front again. Matches after `03-after-save-preview-still-front.png`, contrasts with before `08-after-save-preview-behind.png` (preview BEHIND — the bug).
- 05-pass-b-panel-up.png — second pass, panel up again in front of the preview. Matches after `04-panel-up-second-time.png`.
- 06-pass-b-after-save.png — second pass after save, preview in front.
- 07-main-export-panel.png — MAIN window Settings > Import/Export export save panel (owned by the MAIN window). Matches before `14-export-save-panel.png`, after `05-export-panel.png`.
- 08-main-import-panel.png — MAIN window import open panel. Matches before `15-import-open-panel.png`, after `06-import-panel.png`.
- 09-print-dialog-over-preview.png — Print dialog over the preview (with the no-printer-installed warning on top). Matches before `16-print-dialog.png`, after `07-print-dialog.png`.
- 10-rerun-panel-up.png / 11-rerun-after-save.png — the re-run captured on video (see note below); same states as 03/04.
- save-panel-proof.mp4 — 66 s screen recording of the re-run: preview opened, real click, panel up in front, filename typed, PDF saved, preview in front again. NOTE: the original continuous recording (started before app launch) was lost — ffmpeg was force-killed and the moov atom was never written. This video is a re-run of the same steps against the same app instance and database; the stills and window lists of the original passes (files 02-09) are the primary evidence.
- enumwin-pass-a.json / enumwin-pass-b.json — EnumWindows output while each Save-as-PDF panel was up: panel class #32770, owner = preview hwnd 1639066, panel foreground.
- enumwin-main-export.json — export Save panel owner = MAIN hwnd 131752.
- enumwin-main-import.json — import Open panel owner = MAIN hwnd 131752.
- enumwin-print.json — Print dialog hwnd 656074 owner = preview 590488 (the warning popup 394014 is owned by the print dialog).
- enumwin-rerun.json — the re-run panel (852800) owner = preview 590488, foreground.
- 2551Q-save-panel-proof-pass-a.pdf / pass-b.pdf — the saved PDFs, 272,149 bytes each, 2 pages (%PDF-1.4), BIR Form 2551Q Q4 2026 for the demo taxpayer.
- ebirforms.2026-11-26.log — the app log for the whole session (BIR_LOG_DIR). RUST_LOG=debug produced lifecycle-level lines only in this build; there are no per-click lines — what the log does NOT show: it cannot evidence z-order or panel ownership (the window lists do that).

## What this proof does not show

- It does not test agent presses (they do not reach a second window on Windows, #358 — by design of the test).
- It does not test printing to a real printer (none installed; the dialog was cancelled).
- It does not cover the macOS or Linux behavior.
