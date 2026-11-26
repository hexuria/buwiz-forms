# Form 1702MX Print Missing Values Proof (`feat/forms-print-missing-values`)

Saved return (demo taxpayer `SANTOS, JUANA`, TIN `000-123-453-00000`, Calendar Year 2025):
- Page 1 Part II Items 14–21:
  - Item 14 Total Tax Due (`p1c59`): `0`
  - Item 15 Total Tax Credits (`p1c61`): from Schedule 3 (`10,000,000`)
  - Item 16 Net Tax Payable (`p1c63`): `-10,000,000`
  - Items 17–21: Surcharge `1,234`, Interest `2,345`, Compromise `3,456`, Total Penalties `7,035`, Total Amount Payable `7,035`
- Page 2 Schedule 1 Items 1–3:
  - Investment Promotion Agency (`p2c17..19`): `PEZA`, `BOI`, `SBMA`
  - Legal Basis (`p2c21..23`): `RA 7916`, `EO 226`, `RA 7227`
  - Registered Activity (`p2c25..27`): `IT BPO`, `EXPORT MFG`, `LOGISTICS`
- Page 2 Schedule 2 Item 1 Sales (`p2c70..72`): `123456789`, `234567890`, `345678901`

## Root Cause
`Form1702MXDraft::to_print_field_map()` in `crates/bir-core/src/forms/form_1702mx.rs` called the legacy `self.to_bir_field_map()` (`form_1702mx_xml.rs`) instead of `self.to_official_field_map()` (`form_1702mx_official.rs`, which is what `to_official_xml_payload()` uploads). The legacy map omitted Part II Items 14–16, left Schedule 1 Item 1 blank, and wrote Schedule 3 numbers into Schedule 1 Items 2–3 (`txtPg2Pt4I32C*` / `txtPg2Pt4I33C*`). Switching `to_print_field_map()` to `self.to_official_field_map()` fixes all of them without touching upload XML. Not a `promote_empty_joined_cells` case (all 405 joined cells in `html-frozen/1702mx-2018c/index.html` already have `<input>` elements).

## Files
- `before-return.pdf`, `before-page-{1..4}.png`: Saved PDF and 150 DPI page renders using legacy `to_bir_field_map()` (Part II Items 14–16 blank; Schedule 1 IPA blank, Legal Basis and Registered Activity showing `0` / Schedule 3 numbers).
- `after-return.pdf`, `after-page-{1..4}.png`: Same return after switching `to_print_field_map()` to `self.to_official_field_map()`.
- `crop-part2-items14-21.png`: Top = before (Items 14–16 blank), bottom = after (Items 14–21 all printed).
- `crop-sched1-and-sched2.png`: Top = before (IPA blank, Legal Basis / Registered Activity `0`), bottom = after (`PEZA`/`BOI`/`SBMA`, `RA 7916`/`EO 226`/`RA 7227`, `IT BPO`/`EXPORT MFG`/`LOGISTICS`, and Schedule 2 sales printed).
- `recording.mp4`: Headless proof reel stepping through the 4 rendered after pages.

## What these files do not show
- No live BIR submission or Queue press was performed (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, `BIR_SFTP_LIVE` unset).
- `recording.mp4` is assembled from Chrome-rendered PDF pages rather than a live desktop capture.
