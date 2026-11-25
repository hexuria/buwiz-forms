# 1702MX print check — 3f37b225 vs main 92b3e201 (part 5)

Files:
- 1702MX-head-3f37b225.pdf / 1702MX-main-92b3e201.pdf — saved print PDFs of the same filled draft at each commit (force-added).
- page1..4-head.png — head pages at ~1400px.
- main-pdftotext.txt / head-pdftotext.txt — `pdftotext -layout` of both PDFs.
- main-vs-head.diff — the complete list of boxes whose print differs.

What this shows: every differing cell is blank-on-main, prints-at-head. No value moved to a different box.
What it does NOT show: distinct computed values. 180 of 257 fill keys are GUI-editor-only (refused through the agent write path), so the computation chain inputs (Schedules A/B line items) could not be filled; all computed amounts are 0. Column-swap (exempt/special/regular) and arithmetic verification with distinct numbers was therefore impossible except in Schedule 1 rows 1-3, where text fields held distinct per-column values (122.43/123.44/124.45 etc.) and each printed in its own column.
