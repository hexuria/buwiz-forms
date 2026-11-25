# Android print text size — proof (job 9)

Measuring builds (never to be merged): BEFORE `try/android-print-ruler-before` @ 450eb6df, AFTER `try/android-print-ruler-after` @ ffce6bb0. Both pin NSH ca39c0e9103a389a31ea280799ef34796d7aac70; the `nsh` tool, both runtimes and the sibling checkout were at that pin, classes.dex sha256 `73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17`, each build in its own CARGO_TARGET_DIR (`target-450eb6d`, `target-ffce6bb`). Part 3's pair: main 00a20033 and #333's head 62490278, same pin, own target dirs.

## 1. The ruler — WORKS (the fix draws text at the sheet's own sizes)

Form 0605 prints seven ruler lines: RULER04–09 at 4–9 CSS px outside the scaled page, RULER10 at 10 px inside it. Asked sizes: 1 CSS px = 0.75 pt. Drawn sizes read from the PDFs with PyMuPDF span sizes; page scale zoom:0.8781 in both sheet.html files.

| label | asked (pt) | BEFORE drawn (pt) | AFTER drawn (pt) | AFTER ÷ scale |
|---|---|---|---|---|
| RULER04 | 3.00 | 6.00 | 3.00 | 3.00 (unscaled) |
| RULER05 | 3.75 | 6.00 | 3.75 | 3.75 |
| RULER06 | 4.50 | 6.00 | 4.50 | 4.50 |
| RULER07 | 5.25 | 6.00 | 5.25 | 5.25 |
| RULER08 | 6.00 | 6.00 | 6.00 | 6.00 |
| RULER09 | 6.75 | 6.75 | 6.75 | 6.75 |
| RULER10 | 7.50 | 6.58 | 6.58 | 7.49 |

BEFORE floors every size below 8 px at exactly 6.00 pt (the WebView 8 px minimum); AFTER draws exactly the asked sizes. RULER10 is inside the scaled page: 7.5 × 0.8781 = 6.58 in both builds — never floored even in BEFORE because 10 px > the 8 px floor; divided by the page scale it returns the asked 7.5. Crops: ruler-before-600dpi-crop.png, ruler-after-600dpi-crop.png.

## 2. 2550M before/after — BEFORE's Save as PDF is BROKEN for this document; sizes measured from the spooler's print-job file

Seeded through the app's own host APIs (`bir.screen_form_open`/`fill`/`save` from Check2550m.tsx, values exactly as desktop-2550m-note.txt = uriahg17/buwiz-forms try/check-355 @ 2a2dd9a2 note.txt; 4 sched-4/5 keys rejected, same as desktop) and printed through the real system print path (`bir.screen_form_print` → Android print preview).

**The BEFORE build cannot save this PDF at all — DOES NOT WORK.** In 4 out of 4 attempts the WebView renderer process crashes about 5 s after sheet.html loads (`aw_browser_terminator.cc(154): Renderer process crash detected (code -1)` in logcat, every attempt — see print-before-attempt4.logcat.txt). The preview renders (the spooler already holds the written print-job PDF), but tapping the print button then fails instantly with the system preview's "Sorry, that didn't work. Try again." — Retry never recovers. Attempts 1–2 on the long-running emulator, attempt 3 with fast taps, attempt 4 after a full emulator reboot: identical. The AFTER control on the same freshly rebooted emulator saved on the first try with no crash (byte-identical re-save 319,659 bytes), and the earlier AFTER run saved the same way — 2/2. The simpler one-page 0605 ruler documents saved fine in BOTH builds, so the crash is specific to this heavy two-page document in BEFORE. `2550m-before.pdf` here is the spooler's own `print_job_*.pdf` pulled by root while the preview was open — the exact bytes "Save as PDF" copies — verified 2 pages with all seeded values present (VT010, DELIVERY VAN, 1,234,567.89, the long address). This capture was attempt 5.

**Ten smallest text sizes** (PyMuPDF span sizes, points on the PDF; sheet zoom 0.8154 both, so the 6 pt sheet floor draws at 6 × 0.8154 = 4.89):

| BEFORE (floored) | AFTER (as asked) |
|---|---|
| 4.89 (Sum of Item 18A… | 2.74 PRIOR CAPITAL EQUIPMENT |
| 4.89 0 | 2.74 PRODUCTION MACHINE |
| 4.89 0.00 | 2.74 SAMPLE RICE MILL |
| 4.89 0.00 | 3.91 0 |
| 4.89 141,600.00 | 4.15 ABC CREDITABLE WITHHOLDER |
| 4.89 40,000.25 | 4.15 NATIONAL GOVERNMENT AGENCY |
| 4.89 ABC CREDITABLE WITHHOLDER | 4.24 0.00 |
| 4.89 NATIONAL GOVERNMENT AGENCY | 4.46 UNIT 27F THE VERY LONG… |
| 4.89 PRIOR CAPITAL EQUIPMENT | 4.50 (Sum of Item 18A… |
| 4.89 PRODUCTION MACHINE | 4.59 141,600.00 |

In BEFORE every size below the floor is exactly 4.89 — small text is inflated, not clipped. In AFTER each draws at its own asked size.

**Box p2c219-i** (Schedules 6–8 amount, asked 5.20 pt; the "0.00" at x≈441.5 y≈419 on page 2): BEFORE **4.89 pt** (floored), AFTER **4.24 pt** = 5.20 × 0.8154 exactly. Its row neighbors at asked sizes above the floor are identical in both (e.g. 6.72 in both one row down).

**Android beside desktop** (poppler `pdftotext -bbox` word heights in points, desktop = before.pdf @ commit 2a2dd9a2 main 92b3e201, file desktop-2550m-before.pdf; word heights, not font sizes):

| word | android BEFORE | android AFTER | desktop BEFORE |
|---|---|---|---|
| VT010 | 6.20 | 6.20 | 7.22 |
| DELIVERY | 5.87 | 5.87 | 6.83 |
| SANTOS, | 7.87 | 7.87 | 9.16 |
| PRODUCTION | 5.73 | 3.22 | 3.75 |
| UNIT | 5.73 | 5.23 | 6.08 |
| 1,234,567.89 | 7.45 | 7.45 | 8.66 |
| ABC | 5.73 | 4.87 | 5.66 |

Android sizes run ≈ 0.86 × desktop (the 0.8154 sheet zoom plus font metrics); the floor rows (PRODUCTION, UNIT, ABC) are inflated in android-BEFORE and land close to the desktop ratio in android-AFTER. Where the asked size is above the floor the two android builds are identical, as they should be.

Crops of the same caption area, before and after: 2550m-caption-600dpi-before.png / -after.png (the "(Sum of Item 18A…" caption — visibly wider in BEFORE because the floored text is bigger). bbox outputs: 2550m-before-bbox.html, 2550m-after-bbox.html. Sheet document: 2550m-before-sheet.html (zoom:0.8154). Fixture: Check2550m.tsx + check2550m-print.test.tsx (untracked scratch, as in the desktop proof).

## 3. #333's pair — WORKS

Form 0605 with the 137-character address, main 00a20033 vs #333 head 62490278: the address prints at 7.66 pt on main (the floor inflates it past the sheet's fit) and 5.26 pt with #333. Captions move too (Stamp of Receiving 5.71 vs 5.54 pt). Crops: main-0605-address-600dpi.png, 333-0605-address-600dpi.png, main-0605-captions-600dpi.png, 333-0605-captions-600dpi.png.
