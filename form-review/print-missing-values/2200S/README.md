# Form 2200S Print Missing Values Proof (`feat/forms-print-missing-values`)

Saved return (demo taxpayer `SANTOS, JUANA`, TIN `000-123-453-00000`, March 2025, Payment on Actual Removal):
- Page 1 Items 19A (`1,238.24`), 19B (`1,255.25`), 25A (`1,340.30`): **already printed on `origin/main` (`92b3e201`)** via `decimal_html_id` (`p1c62`, `p1c64`, `p1c84`); the lead's `money_joins -> ` was a script false positive.
- Page 2 Schedule 1:
  - Fixed row 0 (`XB010`): Sales Value `123,456,789.12`, Volume `10,000.50`, Basic Tax Due `60,003.00`
  - Fixed row 8 (`XB090`): Sales Value `234,567,890.34`, Volume `5,000.25`, Basic Tax Due `60,003.00`
  - Others row 1: ATC `XB010`, Tax Bracket `PER LITER`, Rate `6.00`, Sales Value `9,876,543.21`, Volume `1,500.00`, Basic Tax Due `9,000.00`
  - Grand Total: `129,006.00`

## Note on Others Description cells (`p2c101`, `p2c108`)
- `p2c101` (`frm2200S:txtsched1Desc0`) and `p2c108` (`frm2200S:txtsched1Desc1`) are drawn as empty `<div class="c" data-cell-kind="label"></div>` cells in `html-frozen/2200s-2018/index.html` (no `<input>`). Per instructions, no second promotion mechanism was added here; those two cells are covered by `promote_empty_joined_cells` on PR #355 (`feat/store-launch-field-rules`).

## Files
- `before-return.pdf`, `before-page-{1,2}.png`: Saved PDF and 150 DPI page renders on `origin/main` (all Schedule 1 product/Others rows blank above the printed grand total `129,006.00`).
- `after-return.pdf`, `after-page-{1,2}.png`: Same return after joining all 42 fillable Schedule 1 cells (30 fixed rows + 12 Others cells) in `html-frozen/2200s-2018/writer-cells.json`.
- `crop-sched1-rows.png`: Top = before (blank rows above grand total), bottom = after (fixed and Others rows printed).
- `recording.mp4`: Headless proof reel stepping through the rendered after pages.

## What these files do not show
- No live BIR submission or Queue press was performed (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, `BIR_SFTP_LIVE` unset).
- `recording.mp4` is assembled from Chrome-rendered PDF pages rather than a live desktop capture.
