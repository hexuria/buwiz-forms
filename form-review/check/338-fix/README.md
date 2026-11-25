# PR #338 (`feat/store-launch-agent-fill-gaps`) — F2 + F3 Before/After Proof

- **Before (`before/`):** PR #338 head `75dcab06` (before F2 and F3 fixes).
  - 2552: `form.fill` accepted `frm2552:txtPg1Pt1I12TotShare` on a fresh return and under `LSE`, and accepted `frm2552:txtPg2P4I2Amount` before Part IV Row 1 was filled.
  - 1702EX: `form.fill` accepted `frm1702EX:txtPg3Pt6S2I4LegalBasis` and refused Schedule 1 Item `17i` (`Desc`/`Amt`) and Schedule 2 Item 4 (`Description`/`Amount`) only as `"computed by BIR's own formulas"`.
- **After (`after/`):** PR #338 head `afe7bc34` (commits `02557574` for F2 + `afe7bc34` for F3; 1702MX commit left untouched per plan until rebase onto `main` after #326 merges — F1 proof is in `../326-nolco/`).
  - 2552: `form.fill` refuses `txtPg1Pt1I12TotShare` on a fresh return and under `LSE` with `"Needs item 11 = Initial Public Offering (Primary or Secondary)"`, refuses `txtPg2P4I2Amount` before Row 1 with `"Needs Part IV row 1"`, and accepts them once opened (even in the same batch).
  - 1702EX: `form.fill` refuses all 5 boxes of Schedule 1 Item `17i` and Schedule 2 Item 4 (including `LegalBasis`) by naming `"Schedule 1 Item 17i (more...) attached schedule"` / `"Schedule 2 Item 4 (more...) attached schedule"`.

## Files in `before/` and `after/`

- `form-fill-replies.json` — Exact JSON-RPC `form.fill` replies from the running app for every accepted and refused box on 2552, 1702EX, and 1702MX.
- `form-2552-part1.png`, `form-2552-part4.png`, `form-1702ex-part4.png`, `form-1702ex-sch1.png`, `form-1702mx-sch7.png`, `form-1702mx-sch8.png` — Live Linux app screenshots after `form.fill`.
- `nolco-xml-excerpt.txt` — Official BIR XML excerpt for all 24 1702MX NOLCO boxes (F1 fixed on #326 `13e6abe4`, see `../326-nolco/after/nolco-xml-excerpt.txt`).
- `saved-2552.pdf`, `saved-1702ex.pdf`, `saved-1702mx.pdf` — Saved PDFs rendered from each form's Print Preview `sheet.html`.
- `preview/{2552,1702ex,1702mx}-p*.png` and `pdf/{2552,1702ex,1702mx}-p*.png` — Paired page renders of every page of all three forms (~1364 px wide).
- `recording-2552.mp4`, `recording-1702ex.mp4`, `recording-1702mx.mp4` — App-window-only recordings (1430×890, under 2 minutes each) showing `form.fill`, navigating the form sections, and clicking Print Preview.

## What this does not show

- Clicking **Print preview** opens a separate Print Preview window (confirmed by the toast *"Print preview opened. It follows your edits live."*), while `gpui-agent screenshot` captures only the main app window; the actual rendered print preview pages are shown in `preview/*.png` and `pdf/*.png`.
- 1702MX F1 fix (`WholePesoInput.raw` sync) lives on PR #326 (`13e6abe4`, proof in `../326-nolco/`), not on #338 (`afe7bc34`), per the no-merge plan.
