# Check of #362 (302c26c4) + buwiz-forms-html #6 (00dc1894) — independent verification

Method: own invented returns (not the author's), built through each commit's own draft API, rendered with `filled_document` and printed to PDF headlessly with Chromium — the same rendering path the app's print preview uses (`open_print_preview` → `filled_document_with_receipt` → html written to a temp file). Scratch harness: `check362_prints.rs.txt`. Commits: head 302c26c4 in target-c362, main db022086 in target-c362main, RUSTC_WRAPPER empty, cargo 1.99.0. Safety env throughout; nothing queued, nothing sent. Values are invented; demo taxpayer Juana Santos 000-123-453.

In before/after crops: **top strip = main (before), bottom strip = branch (after)**.

## Verdicts per claim

1. **1801/1800 branch code — HOLDS.** Branch 00001 prints on both pages at the head; at main the page-1 TIN comb prints nothing at all and page 2 prints the 9 TIN digits followed by the sheet's pre-printed zeros. Branch 00000 prints once at the head. (1801/1800 crops + cells files.)
2. **1800 item 14 centavos — HOLDS, and main is worse than claimed.** At the head p1c64 is a two-slot comb and 05/50/99 each print in their own slots. At main p1c64 is a malformed pair of overlapping plain text inputs (no comb slots) and nothing prints at all in that cell with the same print map.
3. **2200S Others descriptions — HOLDS.** Both 41/42-character descriptions print at p2c101/p2c108; main has no input there.
4. **2200T — VALUES HOLD, but see fault F1.** On the January 2020 sheet all four rows 2020–2023 print their amounts (111.00/222.00/333.00/444.00 with my data), the Schedule 1 total prints 1,110.00 and item 16 is 1110.00 = the printed rows' sum (own arithmetic). At main on the August 2022 sheet the 2020/2021 amounts print nowhere (p2c27/32 carry 333/444 = 2022/2023). A 2024 transaction date prints normally in item 1; the paper has no 2024 row (rows end at 2023) and nothing prints for one. **The upload XML from the same draft is byte-identical at both commits (24,263 bytes)** — paper only changes.
5. **(e) other sheets — unchanged** by inventory digests and the golden/parity battery.

## FAULT F1 (top harm): the 2200T print preview is broken in the app at 302c26c4

`html-frozen/2200t-2020/index.html` is the only sheet in the corpus whose stylesheet links are `<link href="../base.css" rel="stylesheet"/>` (href first, self-closed). `inline_local_assets` in `crates/bir-print/src/frozen_html.rs` rewrites only the canonical byte string `<link rel="stylesheet" href="../base.css">`, so for this one sheet the CSS is never inlined. The filled document keeps external `../base.css`/`form.css` references; the app writes that html alone into the temp dir, where neither file exists. Printed as-is (exactly what the preview serves), the return renders as 9 mashed letter pages instead of 3 correct 612x936 pages — compare `2200T/before-main-years.pdf`-style correct rendering with what the same pipeline produces at the head without the css workaround. `2200T/after-head-years.pdf` in this proof was rendered with the css files placed beside the html to show what the sheet content should look like — the joins and geometry are correct; the pipeline fails to deliver them. The author's own proof PDFs are fine, which means their harness printed with the assets reachable, masking the defect. Fix is one of: normalize the link tags in the sheet, or make `inline_local_assets` match the tag regardless of attribute order. Note this affects only the app print path, not the upload (byte-identical) nor the cell joins (proven correct).

## Limits

Headless Chromium print-to-pdf of the filled document, not the interactive preview window; the pipeline html is the same string the preview writes. Font rendering in headless chromium is approximate. The 2200T-with-css render is proof of joins/geometry, not of what the app user sees today. 2200T page-3 source-PDF difference is by design (guidelines moved to guide.html per provenance).
