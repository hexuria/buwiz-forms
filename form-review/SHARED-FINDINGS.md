# Shared Print Findings: Growable Rows & Silent Print Omission (3e)

## Summary

- **23 of 43** frozen forms under `html-frozen/` declare `growables` in `provenance.json` (and embed matching `<script id="formgen-bands">` band geometry in `index.html`).
- **Print behavior today:** `crates/bir-print/src/frozen_html.rs` never reads `formgen-bands` and only fills the fixed rows statically mapped in each form's `writer-cells.json`. Any rows entered past the joined row count are **silently omitted** from both Print Preview and PDF export.
- **Upload behavior today:** `crates/bir-core/src/forms/official_inputs.rs::extend_layout` (used by 14 form writers: `1600PT`, `1600VT`, `1700`, `1701A`, `1702EX`, `1702MX`, `1702RT`, `1707`, `1707A`, `1800`, `1801`, `2200M`, `2200P`, `2200S`) and dynamic schedule writers **do serialize and upload all extra rows** in the BIR XML payload. As a result, the uploaded return and the printed/archived PDF silently disagree when a user exceeds the sheet's fixed rows.
- **Warnings today:** **None.** `bir_print::frozen_html::unprinted_keys()` exists only in unit/audit tests (`crates/bir-print/src/frozen_html.rs`, `crates/bir-app-api/src/screen_form_audit.rs`) and is never called at runtime by `bir-app-api`, `nsh/src/print.rs`, or `nsh/screens/form`.
- **Screen entry limits today:** In `nsh/screens/form`, repeatable schedule tables (e.g., `1800` Schedules A/B, `1801` Schedules 1–6, `1600WP` Schedule 1.1, `2552` Schedules 1–3, `1600PT/VT`, `1700`, `1702*`, `2550M`) allow adding rows dynamically beyond the sheet's fixed printed capacity without any print-capacity cap or notice.

## All 23 Forms with `growables` in `provenance.json`

| Frozen Slug | Growable Groups | Page & Printed Capacity (`provenance.json`) | Notes on Current Print & Upload |
|---|---:|---|---|
| `1600-pt-2018` | 2 | p1: 5 rows, p2: 4 rows | 58 joins, 7 not_joined entries; extra rows omitted silently on print |
| `1600-vt-2018` | 2 | p1: 5 rows, p2: 4 rows | 58 joins, 7 not_joined entries; extra rows omitted silently on print |
| `1601-fq-2020` | 2 | p1: 6 rows, p2: 5 rows | 76 joins, 9 not_joined entries; extra rows omitted silently on print |
| `1601c-2018` | 2 | p2: 3 rows, p2: 3 rows | 32 joins, 7 not_joined entries; extra rows omitted silently on print |
| `1601eq-2019` | 1 | p1: 6 rows | 35 joins, 5 not_joined entries; extra rows omitted silently on print |
| `1602q-2019` | 2 | p2: 3 rows, p2: 3 rows | 27 joins, 5 not_joined entries; extra rows omitted silently on print |
| `1604e-2018` | 2 | p2: 3 rows, p2: 3 rows | 117 joins, 4 not_joined entries; extra rows omitted silently on print |
| `1604f-2018` | 3 | p2: 3 rows, p2: 3 rows, p2: 3 rows | 84 joins, 6 not_joined entries; extra rows omitted silently on print |
| `1606-2018` | 2 | p2: 4 rows, p2: 5 rows | 32 joins, 12 not_joined entries; extra rows omitted silently on print |
| `1700-2018` | 1 | p2: 3 rows | 118 joins, 11 not_joined entries; extra rows omitted silently on print |
| `1701-2018` | 3 | p3: 4 rows, p4: 4 rows, p4: 3 rows | 99 joins, 6 not_joined entries; extra rows omitted silently on print |
| `1702ex-2018` | 1 | p3: 4 rows | 126 joins, 17 not_joined entries; extra rows omitted silently on print |
| `1702mx-2018c` | 4 | p3: 4 rows, p4: 4 rows, p4: 4 rows, p4: 3 rows | 403 joins, 9 not_joined entries; extra rows omitted silently on print |
| `1702rt-2018c` | 4 | p3: 4 rows, p4: 3 rows, p4: 4 rows, p4: 3 rows | 64 joins, 7 not_joined entries; extra rows omitted silently on print |
| `1706-2018` | 2 | p2: 4 rows, p2: 5 rows | 32 joins, 14 not_joined entries; extra rows omitted silently on print |
| `1707a-2021` | 2 | p2: 3 rows, p2: 3 rows | 57 joins, 57 not_joined entries; extra rows omitted silently on print |
| `1800-2018` | 4 | p2: 5 rows, p2: 7 rows, p2: 8 rows, p2: 9 rows | 101 joins, 32 not_joined entries; extra rows omitted silently on print |
| `2000-dst-2018` | 4 | p2: 3 rows, p2: 3 rows, p2: 3 rows, p2: 3 rows | 52 joins, 8 not_joined entries; extra rows omitted silently on print |
| `2000-ot-2018` | 2 | p2: 4 rows, p2: 5 rows | 80 joins, 6 not_joined entries; extra rows omitted silently on print |
| `2200c-2018` | 1 | p2: 10 rows | 138 joins, 7 not_joined entries; extra rows omitted silently on print |
| `2550m-2007` | 7 | p2: 4 rows, p2: 4 rows, p2: 4 rows, p2: 4 rows, p2: 5 rows, p2: 5 rows, p2: 5 rows | 256 joins, 13 not_joined entries; extra rows omitted silently on print |
| `2551q-2018` | 1 | p2: 6 rows | 29 joins, 10 not_joined entries; extra rows omitted silently on print |
| `2552-2018` | 7 | p2: 4 rows, p2: 5 rows, p2: 4 rows, p2: 5 rows, p2: 4 rows, p2: 5 rows, p2: 4 rows | 176 joins, 5 not_joined entries; extra rows omitted silently on print |

### Notable Non-`growables` Forms with Repeatable Schedules
Several forms have `growables: []` in `provenance.json` (no `formgen-bands` emitted) yet still have repeatable schedule tables in `forms-schema.json` / `bir-core` whose extra rows are also silently dropped on print (e.g., `1600wp-2010` Schedule 1.1: 6 rows printed; `1801-2018` Schedules 1–6: fixed printed rows; `1707-2021`, `2200m/p/s/t`, `2550q-2024`).

## Handling Options (Smallest First)

### Option 1 — Warn (and optionally block print) when extra rows exceed printed capacity
- **Behavior:** Call `unprinted_keys()` (or a dedicated overflow-row detector) before rendering print preview/PDF. If non-empty schedule rows exist past the sheet's joined rows, show a clear banner in Print Preview / Form toolbar (and optionally block final print/PDF export or stamp an "Incomplete — attach additional sheet" notice).
- **What it touches:** `crates/bir-print/src/frozen_html.rs` (expose structured unprinted-row summary), `crates/bir-app-api/src/print_module.rs` (return warning metadata with print preview), `nsh/screens/form` / print preview chrome (render banner / confirmation gate).
- **Trade-off:** Fastest to ship and eliminates silent mismatch immediately, but taxpayers with long schedules still cannot print their extra rows from the app.

### Option 2 — Append a plain continuation sheet table for overflowing rows
- **Behavior:** Keep pages 1..N of the frozen BIR sheet byte-for-byte unchanged. When any schedule has filled rows past its printed capacity, append one or more clean HTML continuation pages (`.sheet-flow`, styled like the existing receipt page) carrying the form header (Form No., TIN, Taxpayer Name, Return Period) and a plain bordered table of the overflowing rows, labelled "Additional Sheet — Schedule X (continued)".
- **What it touches:** `crates/bir-print/src/frozen_html.rs` (collect overflow schedule rows using writer key naming + append `.page-continuation` markup), `crates/bir-app-api/src/static_html.rs` (treat `.page-continuation` as `.sheet-flow` just like `.page-receipt`), plus unit tests and PDF pagination verification.
- **Trade-off:** Works for **all 43 forms** (including the repeatable forms with `growables: []`), never mutates `html-frozen/`, preserves all existing fitting/layout goldens, and directly satisfies the BIR sheet instruction *"Attach additional sheet/s, if necessary"*.

### Option 3 — Real band pagination using `formgen-bands`
- **Behavior:** Parse `<script id="formgen-bands">` in `crates/bir-print/src/frozen_html.rs` (and re-run the upstream generator for forms currently missing `formgen-bands`), clone the band's SVG rules, comb/text cells, and row numbers onto overflow pages matching the official BIR grid geometry pixel-for-pixel, and dynamically map overflow writer keys to the cloned cells.
- **What it touches:** `crates/bir-print/src/frozen_html.rs` (band parser, SVG/cell cloner, dynamic writer-key-to-band binder, page numbering), upstream `hexuria/buwiz-forms-html` generator (for repeatable forms with `growables: []`), and extensive golden/PDF tests.
- **Trade-off:** Highest visual fidelity to official BIR sheets, but highest engineering cost and requires upstream generator work for forms without `formgen-bands`.

