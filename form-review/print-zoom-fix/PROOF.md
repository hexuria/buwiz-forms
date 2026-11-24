# print-zoom-fix proof — saved-PDF vs preview on macOS

Branch under test: try/print-zoom-fix (origin/main + the `.sheet>.page` zoom→transform fix in
crates/bir-app-api/src/print_sheet.rs and the page_info parser in nsh/src/print.rs).

Same saved returns as the earlier mac check. "before" PDFs were saved from the same returns on the
neon fix-branches (older app code, `.page{zoom:0.878}`); "after" PDFs saved from the fixed build.
All after-PDFs are 2-page A4 (594.75 x 841.5 pt) — same page count and size as before.

## Same-line overlapping text runs (pdftotext -bbox, pairs sharing a line with x-overlap > 1.5pt)

| form | before | after |
|------|--------|-------|
| 2550Q p1 | 5 | 0 |
| 2550Q p2 | 6 | 0 |
| 1706 p1 | 1 | 0 |
| 1606 p1 | 2 | 0 |

2550Q before overlaps include 'Bank)'/'Stamp', 'Agent'/'(Indicate', 'Goods'/'(Attach',
'Withheld'/'(Attach', 'Payment'/'(Attach' — the italic-over-bold defects. All gone in after.

## Clipped names now fit (as in the preview)

- 1706 6A: before "SANTOS, JUANA MARIE DELA CRUZ-VILLANU" → after "…CRUZ-VILLANUEVA" (crop: 1706-6A-clip-crop.png)
- 1706 7A / Location: full "METRO MANILA" prints (was "…METRO MAN")
- 1606: buyer name and Schedule-1 "(whichever is higher between Columns 1 & 2)" header print uncut

## Note on the 1606 re-fill

Reopening the saved draft applied the "updated from your tax profile" clobbering and emptied the
Schedule-1 rows; the after PDF was re-taken after re-filling via the agent fill path (same values as
the earlier run). The classification cell prints the letter for property_class=condominium_commercial
in the after draft (the earlier run used agricultural → "A"); a value difference, not a layout defect.

## Baseline caveat

The "before" PDFs came from the neon branches' fixed sheets; try/print-zoom-fix is on origin/main, so
small differences unrelated to the zoom (e.g. schedule amount alignment fixed on neon-3) show as
baseline noise here, not as regressions of this change. The 43-form sweep compares main-build vs
fix-build PDFs on identical sheets.

## Update: paper path, iOS, preview zooms, 43-form sweep (fix commit e45bd5fa54)

- **Paper path**: system print dialog → "Open in Preview" on 1706 shows the corrected 2-page A4 (`paper-open-in-preview.png`) — paper prints carry the fix by construction (PDFKit prints the same PDF).
- **Preview parity**: the preview window at "Fit width" (`prev-fit.png`) and at "100%" (`prev-100.png`) shows the same corrected layout as the saved PDF — after the fix the sheet lays out at zoom 1 and the transform scales paint only, so all zoom levels render identically.
- **iOS**: `ios1706-after.pdf` (iPhone 17 sim, Save to Files; `ios1706-after-p1/p2.png`): A4, 2 pages, 0 same-line overlaps; 6A/7A names fit. Same `mod mac` capture path as macOS — fix covers iOS. Before: `ios1706.pdf` + `ios1706-1/6a/sig.png`.
- **43-form sweep** (`sweep-manifest.json`, `sweep-pagediff.txt`): every sheet folder under html-frozen/ (44 bundles) rendered twice through the same WKWebView createPDF capture — once with the old `.sheet>.page{zoom:0.878}` fit_css, once with the new transform fit_css — and compared per page at 100 dpi. Result: identical page counts for all 44 bundles; per-page pixel diffs are 0.008%–0.57% (sub-pixel anti-aliasing at the smallest text sizes). No bundle differs by more than small-size text reflow; no contact sheets needed.
