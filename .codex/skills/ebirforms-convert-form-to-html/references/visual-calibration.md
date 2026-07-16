# Visual Calibration

## Reference preparation

1. Pin the exact official PDF SHA-256 before rendering.
2. Render every page with Poppler at 144 DPI.
3. Verify page count and point/pixel geometry.
4. Record hashes for the source and rendered pages.
5. Keep references under `packages/form-renderer/references/` and mark them calibration-only.

PDF viewer presentation is not source evidence. A viewer may show a single page,
a continuous scroll, or a two-page spread. Always compare official page N to
HTML page N as an individual raster; never combine adjacent pages or infer a
page-count change from the viewer's display mode.

Use `prepare_official_reference.py`; do not use a Typst/formtype-generated page as the official reference.
The script locks geometry from `pdfinfo` by default. Supply both optional point-dimension arguments only when independently asserting the PDF metadata; never inherit another form's paper height.

## Layout method

Rebuild the form with semantic rows, tables, labels, combs, checkboxes, signatures, and schedules. Reuse shared primitives only when they preserve exact official behavior. Keep form-specific measurements scoped to the exact revision.

Verify discrete artwork through [discrete-artwork.md](discrete-artwork.md). Never
keep a rendered-page bitmap crop for a machine-readable symbol: decode its exact
payload, prove the zero-difference logical module matrix, render that matrix as
crisp inline vector artwork, and keep its caption as bundled-font live text.
Extract the exact embedded government seal/logo object at native dimensions; do
not download, threshold, resize, resample, recolor, sharpen, or substitute it.
For a form with no code, record the audited absence and render none. Never embed
a whole official page as a runtime background.

## Acceptance

- Render at the reference DPI and inspect every page visually.
- Hide only dynamic fixture values, then compare every remaining page pixel so
  static labels, instructions, short rules, checkboxes, artwork, signatures,
  and field composition remain inside the gate.
- Compare both complete pages and named critical regions.
- Treat a long-horizontal/vertical-line mask as a geometry diagnostic only.
  Its changed-pixel percentage is not a visual-similarity score and can never
  satisfy visual parity by itself.
- Add an exact static-content inventory test for official item numbers, labels,
  row counts, date separators, signature cells, and conditional structures.
- Require no clipping, overlap, missing glyph, accidental wrap, or row loss.
- Exercise empty, normal, long-value, and maximum-row fixtures.
- Meet the repository's strict complete-page pixel-difference threshold after
  suppressing only fixture-provided values (currently at most 1%) before
  marking visual parity complete.
- Record complete-page, critical-region, expected-ink-recall, and unexpected-ink
  metrics separately. Never present a sparse diagnostic as “percent identical.”
- Record an incomplete result honestly; never adjust masks, denominators, or
  tolerances to hide application content defects.
