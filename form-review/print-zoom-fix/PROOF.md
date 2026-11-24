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
