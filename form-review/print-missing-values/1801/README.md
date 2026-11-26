# Form 1801 — Missing Printed Values Proof

- **Cause**: `html-frozen/1801-2018/writer-cells.json` listed `frm1801v2018:tinA_1`, `tinB_1`, `tinC_1`, `txtTaxableEstate`, `txtTotPayable`, `txtOtherDed_Specify`, and `sched5Others_Specify` under `not_joined` even though `html-frozen/1801-2018/index.html` already has fillable `<input>` elements in `p1c13`, `p1c14`, `p1c15`, `p1c56`/`p1c57`, `p1c112`/`p1c114`, `p2c259`, and `p2c261`.
- **Fix**: Joined those 7 keys in `html-frozen/1801-2018/writer-cells.json` (`joins` and `money_joins`) and removed them from `not_joined` / `not_joined_cells`. No change to `index.html`, upload XML, or `bir-core`.
- **Same-kind audit**: Checked all other `not_joined` entries in `1801-2018/writer-cells.json`; every other fillable cell is already handled by `DONOR_ESTATE_JOINS` in `crates/bir-print/src/frozen_html.rs` (only preprinted `ES 010`, `6.0%`, `00000` branch code, page counters, and overflow schedule rows remain unjoined).
- **Regression test**: `form_1801_prints_item5_tin_items_16_and_24_and_specify_boxes` in `crates/bir-print/tests/print_missing_values.rs`.

## Files (same synthetic Juana Santos return, `origin/main` before vs. branch head after)
- `before-return.pdf`, `before-page-1.png`, `before-page-2.png`: `origin/main` print (Items 5 TIN, 16, 24, 37C specify, and Sched 5 specify blank).
- `after-return.pdf`, `after-page-1.png`, `after-page-2.png`: branch head print (all 5 boxes populated: TIN `000-123-453`, Item 16 `292,323,706.57`, Item 24 `23,600,398.69`, Item 37C `SPECIAL TREATY ALLOWANCE`, Sched 5 `COURT-APPROVED FUNERAL CLAIMS`).
- `crop-item5-tin.png`, `crop-item16-net-taxable-estate.png`, `crop-item24-total-payable.png`, `crop-item37c-specify.png`, `crop-sched5-specify.png`: stacked before (top) vs. after (bottom) crops.
- `recording.mp4`: 17s proof reel showing the before/after crops and full rendered pages. (Does not show live GPUI desktop window interaction.)

## Other observations on the pages (pre-existing on `origin/main`)
- Page 2 Part IV Columns A/B and Schedule 1–5 plain-text amounts are left-aligned.
- Untouched Schedule 1A/2/2A/3/4 rows print `0.00` and untouched Schedule 4 RDO Code rows print `000`.
