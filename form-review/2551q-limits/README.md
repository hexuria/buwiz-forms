# 2551Q Amount Limits Proof (`feat/forms-2551q-amount-limits`)

Same saved return (demo taxpayer `SANTOS, JUANA`, TIN `000-123-453-00000`, Q1 2026, Graduated):
- Schedule 1 Row 1 (`PT010`, 3%): Taxable Amount `666,666,666.66` (12 typed chars with centavos) -> Tax Due `20,000,000.00` (> 10,000,000.00, refused by old 7-cell check).
- Schedule 1 Row 2 (`PT040`, 3%): Taxable Amount `999,999,999,999.00` (12 integer digits, max official `maxlength="12"` whole-peso input) -> Tax Due `29,999,999,999.97` (11 integer digits).
- Schedule 1 Total / Item 14 / Item 19: `30,019,999,999.97`; Item 24 (with late penalties): `39,637,142,054.75`.

## Files
- `before-check.txt` / `before-xml.txt`: Exact `origin/main` (`3aceafe8`) Check (`validate()`) and dry-run `to_bir_xml_payload()` output — 3 errors (7-cell tax due on rows 1 & 2, 11-cell taxable amount on row 2); XML refused.
- `after-check.txt` / `after-xml.txt`: Exact `feat/forms-2551q-amount-limits` Check (`0` errors) and dry-run XML excerpt (`txtATCAmt1`, `txtATCDue1`, `txtATCAmt2`, `txtATCDue2`, `txtTotalSched1`, `frm2551Qv2018:txt14/19/24`).
- `before-return.pdf`, `before-page-1.png`, `before-page-2.png`: Saved PDF and 1400px-wide page renders of the return on `origin/main`.
- `after-return.pdf`, `after-page-1.png`, `after-page-2.png`: Saved PDF and 1400px-wide page renders of the same return after the fix, showing every digit right-aligned inside the sheet's 12-cell integer combs + 2 centavo combs.
- `entry-and-check.png`: Summary of entry-time (`screen_form_fill`) refusal parity verified by `form_2551q_entry_check_and_queue_agree_on_amount_limits`.
- `negative-refusal.png`: Negative over-limit refusal parity (`-1,000,000,000.01` on `txtATCAmt1` and `-1,000,000,000,000` on `frm2551Qv2018:txt15`) showing entry-time refusal as typed, Check result with unified wording, post-#327 row-summary exclusion, and negative overpayment edges on Items 19 & 24 (`form_2551q_negative_amounts_agree_across_entry_check_and_queue` and `negative_edges_for_overpayments_and_non_negative_boxes`).
- `recording.mp4`: 17-second headless proof reel stepping through Before Check/XML refusal, After entry-time refusal parity, After Check pass + dry-run XML excerpt, and rendered Pages 1–2.

## What these files do not show
- No live BIR submission or Queue press was performed (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, `BIR_SFTP_LIVE` unset).
- `recording.mp4` is assembled from the exact Check/XML outputs and Chrome-rendered PDF pages rather than a live macOS interactive window capture; live entry/Check parity is exercised directly through `screen_form_fill` and `screen_form_validate` in `bir-app-api`.
