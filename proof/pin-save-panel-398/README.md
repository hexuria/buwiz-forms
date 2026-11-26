# pin-save-panel-398 Windows real-input proof

App commit: `19e4b0e84cfc2f7b84a504a7729b363eecfcee59` (`hexuria/buwiz-forms`, branch `feat/android-store-pin-bf52`)  
NSH commit: `bf52b6fe79f7abf0139cec81d760f0d0dca7949e`; `git merge-base --is-ancestor 9fb1f7f8 HEAD`: yes  
Tool: `nsh 0.6.1 (bf52b6f)`  
Machine: Windows Server 2022 Standard 10.0.20348, EnableLUA=0, console session, 1600x1200 desktop  
Run: fresh scratch database, demo taxpayer only, `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; no Queue press and no account sign-in.

## Answers

1. From the print preview, both `Save as PDF` panels were owned by the preview hwnd `459514`.
2. During the panels the z-order was panel z=1, preview z=2, main z=3; after saving, the preview remained in front of the main window.
3. From the main window, export `Save As` and import `Open` panels were owned by the main hwnd `590418`.
4. The print dialog was over the preview: print dialog z=2, preview z=3, owner preview hwnd `590674`; the separate no-printer warning was z=1 and owned by the print dialog.

## Numbered pictures and earlier counterparts

Earlier “before” pictures are on `try/win-save-pdf` (`e0407d10`/`cd34283c`). The #381 “after” pictures are on `try/pin-save-panel-381` (`29574b5f`).

- `01-preview-open.png` — preview open in front of the maximized main window before the first real click. Before: `02-preview-open-1-front.png`; #381 after: `01-preview-open.png`.
- `02-pass-a-click-plus-1s.png` — one second after the first real SendInput click on `Save as PDF`; the PDF was already prepared enough that the native panel was up. Before: `06-realclick-preparing-pdf.png`; #381 after: `02-pass-a-click-plus-1s.png`.
- `03-pass-a-panel-up.png` — pass A native `Save as PDF` panel visibly in front of the preview and main window. Before: `07-realclick-after-30s.png`; #381 after: `03-pass-a-panel-up.png`.
- `04-pass-a-after-save.png` — after pass A saved; preview remains in front of main. Before: `08-after-save-preview-behind.png`; #381 after: `04-pass-a-after-save.png`.
- `05-pass-b-click-plus-1s.png` — one second after the pass B real click, with the second panel already up. Before: `06-realclick-preparing-pdf.png`; #381 after: no exact pass-B one-second counterpart; nearest is `05-pass-b-panel-up.png`.
- `06-pass-b-panel-up.png` — pass B native `Save as PDF` panel visibly in front of the preview and main window. Before: `07-realclick-after-30s.png`; #381 after: `05-pass-b-panel-up.png`.
- `07-pass-b-after-save.png` — after pass B saved; preview remains in front of main. Before: `08-after-save-preview-behind.png`; #381 after: `06-pass-b-after-save.png`.
- `08-main-export-panel.png` — main-window export `Save As` panel in front of and owned by the main window. Before: `14-export-save-panel.png`; #381 after: `07-main-export-panel.png`.
- `09-main-import-panel.png` — main-window import `Open` panel in front of and owned by the main window. Before: `15-import-open-panel.png`; #381 after: `08-main-import-panel.png`.
- `10-print-dialog-over-preview.png` — print dialog over the preview, with Windows' no-printer warning also visible. Before: `16-print-dialog.png`; #381 after: `09-print-dialog-over-preview.png`.

## Other files

- `save-panel-proof.mp4` — one continuous 32.1-second desktop recording of the successful proof run, including both saves, export/import panels, and print cancellation. It does not include the discarded compile-time harness attempt.
- `enumwin-pass-a.json` / `enumwin-pass-b.json` — EnumWindows captures for the two Save as PDF panels: hwnd, title, class, visibility, rect, owner, and z-order for the panels and both `NshWindow` windows.
- `enumwin-main-export.json` / `enumwin-main-import.json` — EnumWindows captures proving the export and import panels are owned by the main window.
- `enumwin-print.json` — EnumWindows capture proving the print dialog is owned by and z-ordered over the preview; it also records the no-printer warning.
- `2551Q-save-panel-proof-pass-a.pdf` / `2551Q-save-panel-proof-pass-b.pdf` — saved PDFs, each 272,094 bytes and 2 pages. They prove file creation and rendering; pypdf text extraction did not expose the amount/ATC strings because of font encoding, so those exact strings are not textually proven here. Form readback confirmed `drpATC1=1` and `txtATCAmt1=100,000.00`, and the visible preview shows tax due `3,000.00`.
- `ebirforms.2026-11-26.log` — copied app log from `BIR_LOG_DIR` for the proof run.
- `log-lines.txt` — extracted lines around `Save as PDF`/`Print`; none were emitted by the app log, so this file records that absence rather than dialog events.
- `proof-notes.txt` — machine-generated run notes: exact environment, clicks, z-order/owner summaries, PDF sizes/pages, deviations, and chronological run log.

## What this proof does not show

It does not prove real BIR submission, relay sending, printer output, account sign-in, Queue behavior, or agent-semantic button delivery. No live BIR/relay variable was set. Export/import panels were cancelled. The print dialog was cancelled after the no-printer warning was dismissed with No. The first compile-only harness recording/artifacts were discarded and overwritten; the final video and artifacts are one consistent successful run.
