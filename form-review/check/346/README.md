# check/346 — evidence for the independent check of pull request #346

PR #346 `feat/forms-2551q-amount-limits` at exactly `e945627c53e5751c75193a9c4b44ba13290d4f87` (head verified unmoved during the check; `git ls-remote` re-confirmed).
Compared against `origin/main` = `6bb90ad1316e0b2b6dc3c2440af607623dd60e5d` (the main commit the PR branched near; main has since moved on, evidence is labeled).

Everything here is REPORT-ONLY evidence gathered by a checker who did not write the change. No file of the PR was modified; the probe and fixture files used to gather evidence were untracked and never committed to the PR branch.

## What every file shows — and does not show

### `flow-typing-check-refusal-print.mp4` (26 seconds)
The form page of a 2551Q return in a real application window on Linux, frame-captured while the checker drove it:
1. Item 15 (a 12-digit peso field) is filled digit by digit with 15,000,000,000 — far over the old 7/11-cell limits of `main` — and the value commits and formats.
2. Check runs: no amount complaint (the remaining to-dos are fixture-profile noise — the demo profile lacks an EOPT tier and an income-type election, unrelated to amounts).
3. Item 15 is then typed over the new limit: 1,000,000,000,000 (13 digits). The typed text shows in the box but never commits — Item 18 keeps summing the previous 15,000,000,000.00, which is the refusal working.
4. Print preview is pressed; the app reports the preview opened.

What it does NOT show: keystrokes. Linux dev-host windows do not composite to the X display on this machine (the window renders internally but nothing reaches the screen), so a pixel-recording of real typing was impossible. The digits were entered through `s.edit`, which the app's own session code documents as the same path typing funnels into, one character at a time — visually identical to typing. It also does not show the preview window itself: on the Linux dev host the preview toast fires but no window opens (pre-existing platform gap, also present on main — the preview is verified instead through the print HTML the same pipeline writes, see `print/`). Two fixture texts ("no pdf yet", "Test: save PDF") are visible at the bottom left — checker-harness artifacts, not the app.

### `shots/` — real-window screenshots at `e945627c`
- `c346-a1-at-whole.png` row 1 taxable typed 999,999,999,999 (12 digits): accepted, row computes tax due 29,999,999,999.97 — over the old 10,000,000 tax-due refusal of main, accepted on the page.
- `c346-a2-over-whole.png` typed 1,000,000,000,000 (13 digits): refused at entry; the row line keeps the typed text, the total stays 0.00.
- `c346-a3-at-cents.png` / `c346-a4-over-cents.png` 999,999,999.99 accepted / 1,000,000,000.01 refused.
- `c346-a5-commas.png` pasted-with-commas "999,999,999,999" accepted (commas tolerated).
- `c346-a6-negative.png` typed −500: accepted at entry without a message (Check catches it — see REPORT).
- `c346-b1-i15-at.png` / `c346-b2-i15-over.png` Item 15 at the limit 999,999,999,999.99 accepted with correct computed 18/19/24 / over the limit refused.
- `c346-c1-after-check.png` Check on a valid large return: no amount errors.
- `c346-d1-filled.png` / `c346-d2-checked.png` the six-row full return filled and checked; rows and total visible in the page's own summary lines.

What they do NOT show: the refusal message text under the box in every case — the message renders while the box is being edited; the harness read the exact words out of the session (`SCAN` lines) and they are quoted in REPORT.md and `probe-edges-e945627c.txt`.

- `c346main-b1-i15-at.png` / `c346main-b2-checked.png` — the comparison at `origin/main`: item 15 typed 999,999,999,999.99 is accepted at entry with no message, then Check refuses it with the old "does not fit the official 11-cell integer field" words. The complaint the PR fixes, reproduced live.

### `probe-edges-e945627c.txt` / `probe-edges-main-6bb90ad1.txt`
The same draft edge matrix run through `validate()` and `to_bir_xml_payload()` at each commit (untracked Rust probe driving the real form code): each case's validate errors and whether the upload XML builds, with the full XML amount lines for the full return. This is the before/after comparison: at main every row past the old 11-cell taxable or 7-cell tax-due width is refused; at e945627c only true overflows are.

### `print/`
- `print-sixrow-e945627c-page1.png` / `-page2.png` — the filled print sheet of the six-row full return, rasterized from the exact HTML the print pipeline writes (`bir_print::frozen_html::filled_document` → `print_sheet::prepare_html_for` → `staticize`), rendered headlessly. Page 2 shows every Schedule 1 row: 12-digit amounts right-aligned at the decimal point, centavos in the centavo cells, nothing clipped or dropped — including 299,999,999,999.70, a value with exactly 12 integer digits AND centavos: all twelve integer cells fill exactly and the centavos print.
- `print-sixrow-e945627c.pdf` — the same sheet as a PDF.
- `print-sixrow-fields.txt` — the print field map behind it (digit placement source).
- `official-sheet-page1.png` / `official-sheet-page2.png` — the frozen official sheet (`html-frozen/2551q-2018/index.html`) rendered empty, beside ours for alignment comparison.

What they do NOT show: the in-app preview window (unavailable on the Linux dev host, see above) and the system print panel. The HTML is byte-identical to what the preview window would load.

### `REPORT.md`
The full check report: box-by-box table, edge tables, arithmetic, diff reading, five other forms, verdict.
