# Form 2000 Item 15A Verification (`origin/main` `92b3e201`)

## Finding — Lead is a false positive; no code change made
- The eyes-review list (`12997d2d:form-review/eyes/round3/2000/mismatches.tsv`) flagged `frm2000:txtTax15A` (`15A Balance Carried Over from Previous Return`, `1,119.17`) as `BLANK` with cause `html-frozen/2000-dst-2018/writer-cells.json (money_joins -> )`.
- **Actual behavior on `origin/main` (`92b3e201`)**: `html-frozen/2000-dst-2018/writer-cells.json:434` already joins `frm2000:txtTax15A` via `"decimal_html_id": "p1c50"`, and `bir-print` prints `111917` (`1 1 1 9 . 1 7`) into `p1c50` cleanly. The review script only inspected `peso_html_id` and missed `decimal_html_id`, reporting an empty target.

## Files
- `main-return.pdf`, `main-page-{1,2}.png`: Saved PDF and 150 DPI page renders on `origin/main` (`92b3e201`) for an amended Form 2000 return with `balance_carried_over = 1,119.17`.
- `crop-item15a-on-main.png`: Crop of Page 1 Items 14–15D showing `1 1 1 9 . 1 7` already printed in Item 15A on `origin/main`.

## What these files do not show
- No live BIR submission or Queue press was performed (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, `BIR_SFTP_LIVE` unset).
