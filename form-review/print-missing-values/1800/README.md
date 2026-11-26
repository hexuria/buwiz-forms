# Form 1800 — Missing Printed Values Proof

- **Cause**: `html-frozen/1800-2018/writer-cells.json` listed `frm1800v2018:TotalNetGifts` under `not_joined` because Item 14's centavo cell `p1c64` uses two plain inputs (`p1c64-i0`, `p1c64-i1`) beside peso comb `p1c62` (10 slots).
- **Fix**: Added `frm1800v2018:TotalNetGifts` (`p1c62` / `p1c64`) to `money_joins` in `html-frozen/1800-2018/writer-cells.json` and removed it from `not_joined` / `not_joined_cells`. No change to `index.html`, upload XML, or `bir-core`.
- **Same-kind audit**: Checked all other `not_joined` entries in `1800-2018/writer-cells.json`; `txtZipCode` (`p1c26`) and `TaxTreaty` (`p1c57`) are already handled by `DONOR_ESTATE_JOINS` in `crates/bir-print/src/frozen_html.rs`.
- **Regression test**: `form_1800_prints_item_14_total_net_gifts` in `crates/bir-print/tests/print_missing_values.rs`.

## Files (same synthetic Juana Santos return, `origin/main` before vs. branch head after)
- `before-return.pdf`, `before-page-1.png`, `before-page-2.png`: `origin/main` print (Item 14 blank above Item 16 `14,005,747.38`).
- `after-return.pdf`, `after-page-1.png`, `after-page-2.png`: branch head print (Item 14 prints `233,429,122.95` matching Page 2 Item 38).
- `crop-item14-total-net-gifts.png`: stacked before (top) vs. after (bottom) crop of Part II Items 14–16.
- `recording.mp4`: 10s proof reel showing the before/after crop and full rendered pages. (Does not show live GPUI desktop window interaction.)

## Other observations on the pages (pre-existing on `origin/main`)
- Page 1 Item 14 centavo digits (`9 5` in `p1c64`) sit slightly left-aligned in their two sub-boxes (`fi fh15` without `fc`), though clean and unclipped.
- Page 2 Schedule A/B amounts are left-aligned and untouched rows 2–5 print `0.00`.
