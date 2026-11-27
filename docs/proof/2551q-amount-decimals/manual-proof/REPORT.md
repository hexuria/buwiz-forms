# 2551Q `2000.005` real-window proof

Branch: `fix/2551q-amount-decimals`. Real Linux GTK window, 448x844 phone layout, with `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; Queue was never pressed and `BIR_SFTP_LIVE` was never set. The scratch database was seeded by `seed-decimal.test.ts` with the demo taxpayer and one saved 2551Q draft, then the running app was driven by hand.

## Result

Typed `2000.005` in Schedule 1 row 1's taxable amount. There is no refusal and no misleading 12-character sentence. The box itself shows `2,000.01`; row 1 shows `2,000.01 · 3.0 · 60.00`; Tax Due shows `60.00`. Done closes the row, and the Schedule 1 total is `60.00`.

The database afterwards holds `taxable_amount: 2000.01`, `tax_rate: 0.03`, `tax_due: 60.0`, and `total_tax_due: 60.0`. `run.stdout.log` has only the build/connect lines and the harmless tray warning: no refusal is logged.

Artifacts:

- `case1-2000.005-rounded-before-done.png` / `-enlarged.png`: rounded value visible in the open row before Done.
- `case1-2000.005-rounded-after-done.png` / `-enlarged.png`: row closed, row total and Schedule 1 total visible.
- `decimal-2000_005-real.mp4`: 93-second recording from the freshly opened window through typing, rounding, Done, and the closed row.
- `decimal-proof.db`: scratch database after the proof.
- `run.stdout.log`: application output.
