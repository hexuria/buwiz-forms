# Form 2200A Print Missing Values Proof (`feat/forms-print-missing-values`)

Saved return (demo taxpayer `SANTOS, JUANA`, TIN `000-123-453-00000`, March 15, 2025):
- Item 25 Cash/Bank Debit Memo: `BDO`, `100201`, `03/15/2025`, `123,456,789.12`
- Item 26 Check: `BPI`, `200302`, `03/16/2025`, `234,567,890.34`
- Item 27 Tax Debit Memo: `300403`, `03/17/2025`, `345,678,901.56`
- Item 28 Others: `EPAY` / `MBTC`, `400504`, `03/18/2025`, `456,789,012.78`

## Files
- `before-return.pdf`, `before-page-{1,2,3}.png`: Saved PDF and 150 DPI page renders on `origin/main` (`p1c108`, `p1c110`, `p1c111`, `p1c114` unjoined; Part IV rows 25–28 blank except Item 28 specify `EPAY`).
- `after-return.pdf`, `after-page-{1,2,3}.png`: Same return after adding print-only `derived:part4_{cash,check,tdm,others}` in `crates/bir-core/src/forms/form_2200a.rs` and joining them to `p1c108`, `p1c110`, `p1c111`, `p1c114` in `html-frozen/2200a-2020/writer-cells.json`.
- `crop-part4-payment.png`: Top = before (blank), bottom = after (all four Part IV payment rows aligned to Drawee Bank, Number, Date MM/DD/YYYY, 11-digit right-aligned pesos, printed decimal point, and 2 centavos).
- `recording.mp4`: Headless proof reel stepping through the rendered after pages.

## What these files do not show
- No live BIR submission or Queue press was performed (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, `BIR_SFTP_LIVE` unset).
- `recording.mp4` is assembled from Chrome-rendered PDF pages rather than a live desktop capture.
