# Form 1707A Print Missing Values Proof (`feat/forms-print-missing-values`)

Saved return (demo taxpayer `SANTOS, JUANA`, TIN `000-123-453-00000`, 2025):
- Item 12 = Yes, Item 12A (`frm1707Av2021:txtI11ASpecify`) = `PH-US TAX TREATY`.
- Schedule 1 gain row (`250,123,456.78` selling price / `120,000,000.00` cost / `19,518,518.52` tax paid) and Schedule 2 loss row (`100,000,000.00` / `110,234,567.89`).

## Files
- `before-return.pdf`, `before-page-{1,2}.png`: Saved PDF and 150 DPI page renders on `origin/main` (`frm1707Av2021:txtI11ASpecify` unjoined; Item 12A blank).
- `after-return.pdf`, `after-page-{1,2}.png`: Same return after joining `frm1707Av2021:txtI11ASpecify` -> `p1c207` in `html-frozen/1707a-2021/writer-cells.json`.
- `crop-item12a-tax-relief.png`: Top = before (blank), bottom = after (`PH-US TAX TREATY` right inside the 25-slot comb).
- `recording.mp4`: Headless proof reel stepping through the rendered after pages.

## What these files do not show
- No live BIR submission or Queue press was performed (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, `BIR_SFTP_LIVE` unset).
- `recording.mp4` is assembled from Chrome-rendered PDF pages rather than a live desktop capture.
