# proof/win-save-pdf — Save as PDF diagnosis on Windows (devin4_win)

App commit: buwiz-forms origin/main 75bd78b8437876060196c22111566758d860da0b (dev run, `just nsh-run-agent windows`).
Kit: nsh 0.6.1 (8c78cfd), sibling repo at 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf = NSH_PINNED_REF.
Machine: buwiz-forms-win, Microsoft Windows Server 2022 Standard build 20348, EnableLUA=0,
console session (administrator, not RDP), screen 1600x1200.
Demo taxpayer only (Juana Santos 000-123-453-00000), form 2551Q Q4 2026, taxable 100,000.00 at PT010,
tax due 3,000.00. Safety variables on; Queue never pressed; no account sign-in.

## What each file shows — and does not

- recording-steps1-5.mp4 (128 s, gdigrab 5 fps full screen) — the re-run of Steps 1-5 end to end:
  preview close/reopen x3 (in front each time), real click on Save as PDF and the panel appearing,
  export save panel, import open panel, Print -> Microsoft Print to PDF -> printed2.pdf.
  It does NOT contain the first run's first pass (that recording was lost to a bad ffmpeg kill —
  honestly disclosed; every state in it was re-run and is in this video or the PNGs).
- 01-first-run.png — app at first-run setup ("Who will use eBIRForms").
- 02..05-preview-open-N-front.png — the four opens of Step 2: the preview is IN FRONT of the main
  window every time. They do not prove anything about other machines' z-order.
- 06-realclick-preparing-pdf.png — within ~1 s of the REAL click: "Preparing the PDF…" with spinner,
  Save as PDF disabled. Proves the press reaches the preview.
- 07-realclick-after-30s.png — the save panel IS UP: class #32770 "Save as PDF" at the top-left,
  file name "BIR Form 2551Q - Juana Santos", type PDF. Note the preview is no longer visible —
  the panel is owned by the MAIN window and the main window has come forward over the preview.
- 08-after-save-preview-behind.png — after saving: dialog gone, preview behind the main window.
- 09/10/12/13/13b-agentpress-*.png — the agent-press attempts: no busy state, no panel, nothing.
- 11-main-minimized-preview-visible.png — diagnostic: main window minimized by the probe to expose
  the preview (tooling artifact, not app behavior).
- 14-export-save-panel.png — Step 4: the SAME kind of save panel opens from the main window
  (Export this profile), clearly visible.
- 15-import-open-panel.png — Step 4: the open panel for Import data, clearly visible.
- 16-print-dialog.png — Step 5: the Windows print dialog, Microsoft Print to PDF selected.
- 17-after-print.png — after printing.
- windows-step3a-realclick.txt / windows-step3b-agentpress.txt — EnumWindows lists with commentary.
- logs-step6.txt + app-log-full.txt — the complete app log and what was (not) logged at each press.
- saved-from-preview.pdf — the file the panel wrote in Step 3a (272,149 bytes, 2 pages, %PDF-1.4).
- printed.pdf, printed2.pdf — Step 5 output via Microsoft Print to PDF (485,730 bytes each, 2 pages).
  NOTE: their MediaBox is 612x792 = US Letter, NOT A4 — the system print path used the printer's
  default paper size although the preview renders "A4 - 2 pages".

## Answers

1. Does the save panel appear from the preview? YES here — real click: busy state in ~1 s, panel
   visible within 30 s (at the top-left corner, owned by the main window), file written. Through
   gpui-agent the press never reaches the preview (no busy state, no panel) — see step3b notes.
2. From the main window (export/import)? YES, both panels appear plainly.
3. Does Print's dialog appear and print to PDF? YES — but the PDF is Letter-size, not A4.
4. Does the preview open behind? NO — in front on all four opens here.
5. What was logged? NOTHING at any press (no success line, no failure line); full log attached.

## Follow-up additions (second commit)

- saved-from-preview.pdf, printed.pdf, printed2.pdf are now force-added (the repository's .gitignore
  has `*.pdf`; the first commit missed them and this README, which had been written outside the tree).
- printed-page1-100dpi.png, printed-page2-100dpi.png, saved-from-preview-page1-100dpi.png — pages
  rendered at 100 dpi with PyMuPDF (neither pdftoppm nor magick is installed on this machine).
- 18-printer-preferences-paper-sizes.png — Microsoft Print to PDF Advanced Document Settings:
  Paper Size **Letter** is the selected default; the dropdown offers A3, **A4**, A5, B4 (JIS),
  B5 (JIS), Executive, Legal, Letter, Statement, Tabloid. A4 IS offered.
- Is the print cut off? NO. printed.pdf is **scaled down to fit** Letter, not clipped: its content
  spans the full 612x792 page, ~94% of the A4 original (content width 561 pt vs the A4 original
  full-bleed 595 pt — the 792/842 height ratio). Page 1's bottom (declaration, signature boxes,
  Part III table, Machine Validation lines, the privacy NOTE) is complete; page 2's bottom
  (Schedule 1 row 7 Total Tax Due) is complete. Nothing is missing compared with
  saved-from-preview.pdf; the print is simply ~6% smaller because the printer's default paper is
  Letter while the preview paginates for A4. The print dialog's Preferences were not opened during
  the recorded run; the paper-size evidence here is from the printer's own Printing Preferences
  opened afterwards (no setting was changed — cancelled out).
