# PR #326 (`feat/store-launch-unfillable-rows`) — 1702MX NOLCO Before/After Proof

- **Before (`before/`):** PR #326 head `80d880e4` (before F1 fix). All 24 Schedule 7.1 & 8.1 rows 4–6 cols A–D were typed directly on the form page (no `form.fill`), saved, and reopened. Draft stored `amount: <typed>` with `raw: ""`, so official BIR XML serialized all 24 boxes empty and printed page 4 left all 24 boxes blank while computed Col E and Row 8 totals printed.
- **After (`after/`):** PR #326 head `13e6abe4` (after F1 fix in `crates/bir-core/src/forms/agent/mod.rs`). Same 24 boxes typed directly on the form page (no `form.fill`), saved, and reopened. Draft stores both `amount` and formatted `raw` (e.g. `300,000.00`), official BIR XML serializes all 24 boxes, and printed page 4 prints all 24 boxes.

## Files in `before/` and `after/`

- `form-sch7.png`, `form-sch8.png` — Live Linux app screenshots of Schedule 7.1 and Schedule 8.1 after typing, saving, dismissing, and reopening the 1702MX return.
- `draft.json` — Raw `form_drafts.data_json` from the scratch SQLite DB after save & reopen, showing `amount` and `raw` for all 24 boxes.
- `nolco-xml-excerpt.txt` — Official BIR XML excerpt for all 24 A–D boxes plus computed Col E and Row 8 totals (matching `form_1702mx_xml.rs::insert_nolco_table`).
- `sheet.html` — Exact HTML sheet emitted by the app's Print Preview.
- `saved-1702mx.pdf` — PDF rendered from `sheet.html`.
- `preview/p1.png` .. `preview/p4.png` and `pdf/p1.png` .. `pdf/p4.png` — Paired page renders of all 4 1702MX pages (~1364 px wide).
- `after/recording-1702mx.mp4` — 16.7 s app-window-only recording (1430×890) of opening 1702MX, typing all 6 years and 24 NOLCO values on Schedules 7.1 and 8.1, dismissing/reopening, and clicking Print Preview.

## What this does not show

- In `after/recording-1702mx.mp4`, clicking **Print preview** opens the app's separate Print Preview window (confirmed by the toast *"Print preview opened. It follows your edits live."*), while `gpui-agent screenshot` captures only the main app window; the actual rendered print preview pages are shown in `after/preview/p1.png..p4.png` and `after/pdf/p1.png..p4.png`.
- Pre-existing 1702MX print layout issues (left-aligned unformatted Col E / Row 8 totals, header name crossing comb ticks) are outside the F1 fix and tracked in the print work.
