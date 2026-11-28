# 2551Q Schedule 1 invalid-amount probe — real Linux app at a8aeaca8

Environment: unmodified `$HOME/urgent2551/a8aeaca8`, real GTK window resized by `wmctrl` to 448x844 (the same phone-size limit the WM allowed before), fresh scratch database, manually created demo taxpayer Juana Santos. Wrapper supplied `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; the container command also had `RUSTUP_HOME=$HOME/.rustup CARGO_PROFILE_DEV_DEBUG=0 GSK_RENDERER=cairo`. Queue was never pressed and `BIR_SFTP_LIVE` was never set.

## Result

All three refused amounts are visibly refused at the box. This is **not** a case where a refused amount shows nothing and the return silently never saves: each input remains visible, gets a red border, and a red sentence appears both at the top of the row sheet and directly under the taxable-amount box. Done remains visually present/enabled, but pressing it does nothing observable: the row sheet does not close and no save of the invalid amount occurs.

The page-level save status is covered/inaccessible while the invalid row editor remains open, so whether it literally reads “Unsaved changes” during the refusal is **untested**. No save-status change or error toast appears outside the row sheet. After I changed the box back to valid `0` and pressed Done, the sheet closed and the page showed green `✓ Saved`.

## Case 1 — `2000.005`

- Screen: the box shows `2,000.005` with a red border.
- Red sentence at the row top and under the box: “Exceeds the official 12-character input limit (at most 999,999,999,999 without centavos, or 999,999,999.99 with centavos); enter a smaller amount.”
- Notably, the screen does **not** say “at most two decimal places” for this input; it shows the 12-character limit sentence instead.
- Done: pressing Done refuses; the row sheet stays open.
- Draft afterwards: row 1 remains PT010 with `taxable_amount: 0.0`, rate `.03`, tax due `0.0`.
- Screenshots: `case1-2000.005-before-done.png`, `case1-2000.005-after-done.png`.

## Case 2 — `1000000000.01`

- Screen: the box shows `1,000,000,000.01` with a red border.
- Red sentence at the row top and under the box: the same official 12-character input-limit sentence as case 1.
- Done: pressing Done refuses; the row sheet stays open.
- Draft afterwards: row 1 remains PT010 with `taxable_amount: 0.0`, rate `.03`, tax due `0.0` (`db-after-case2.txt`).
- Screenshots: `case2-1000000000.01-before-done.png`, `case2-1000000000.01-after-done.png`.

## Case 3 — `-100`

- Screen: the box shows `-100` with a red border.
- Red sentence at the row top and under the box: “Must be zero or greater; enter a non-negative amount.”
- Done: pressing Done refuses; the row sheet stays open.
- Draft afterwards: row 1 remains PT010 with `taxable_amount: 0.0`, rate `.03`, tax due `0.0` (`db-after-case3.txt`).
- Screenshots: `case3-minus100-before-done.png`, `case3-minus100-after-done.png`.

## Logs

No host refusal or validation line appears in stdout or the app file log for any case. stdout has only the build/connect lines and the harmless tray warning; the file log has startup/migrations and background-cron wake lines. Thus the refusal is entirely local/UI-visible; it is not logged as a host refusal.

## Database

The live scratch database was inspected through `/proc/1062459/root/tmp/ebirforms-door/12976-0/bir_data.db` while the app was running. After every invalid case the sole Schedule 1 row stayed:

```json
{
  "atc": "PT010",
  "taxable_amount": 0.0,
  "tax_rate": 0.03,
  "tax_due": 0.0
}
```

After resetting the amount to valid `0` and pressing Done, the database still held that zero row and the page showed `✓ Saved` (`final-after-reset-zero-saved.png`, `db-final.txt`).

Recording: `amount-cases-real-a8.mp4`.

The `*-cropped.png` files are enlarged crops of the 448x844 app window for readability; the uncropped full-screen captures are retained alongside them.
