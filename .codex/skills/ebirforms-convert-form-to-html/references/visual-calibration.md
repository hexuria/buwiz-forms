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
- Satisfy `official-fidelity-v1`'s components, NOT the 1% complete-page
  threshold. That threshold is unreachable and chasing it is the documented
  failure mode of this project: the source PDFs do not embed their fonts, so
  the reference carries Poppler's substituted glyph outlines, glyph shape is
  ~57% of the residual, and every rendering-side lever was measured and
  refuted. Text correctness is proven by `static-text-exhaustive-v1` instead.
- Direct calibration effort at STRUCTURE, which is winnable, using
  `playwright.structural-defects.config.ts` (which localizes each defect and
  says whether it is weight, displacement, or missing structure) and
  `playwright.comb-capacity.config.ts` (which checks declared cell counts
  against the official form's dividers — a capacity error is invisible to every
  pixel metric).
- Verify each change against BOTH rasterizers. Improving the chromium gate
  while worsening the Poppler diagnostic is overfitting to one reference.
- Record complete-page, critical-region, expected-ink-recall, and unexpected-ink
  metrics separately. Never present a sparse diagnostic as “percent identical.”
  `official-fidelity-v1` certifies "no worse than a reviewed baseline" and can
  never certify "matches the official form".
- Record an incomplete result honestly; never adjust masks, denominators, or
  tolerances to hide application content defects.

## Measurement traps that cost real time here

- **Thresholded pixel counts misreport sub-pixel geometry.** Counting rows
  below a tone cut made a correct 3-device-px stroke look like 4. Read tone
  profiles.
- **An offset search reports a too-thin stroke as "displaced".** Shifting it
  does land on ink, so recovery reads 100% while the real fault is weight.
  Acting on the offset moves correctly-placed rules.
- **Official guides are mid-grey (tone 83–153), not black** — sub-pixel black
  ink cannot fill a pixel. A detector keyed to near-black misses nearly all of
  them.
- **A thicker border moves everything inside it** unless the padding or track
  size is compensated by the same amount.
