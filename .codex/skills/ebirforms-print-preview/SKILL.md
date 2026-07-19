---
name: ebirforms-print-preview
description: Diagnose, fix, calibrate, or review an existing eBIRForms semantic HTML print preview, visual parity issue, renderer binding, deterministic pagination, clipping report, native print, or direct PDF export. Use only when the exact form revision already has an HTML component and is enabled in the migration manifest. Use ebirforms-convert-form-to-html when the renderer is missing or a new form/revision must be migrated.
---

# Maintain an eBIRForms HTML Print Preview

Fix an already-migrated form through the owning layer. Do not create a second rendering path.

**The visual release gate is a complete-page pixel difference of at most 1%**
per official page at 144 DPI (pixelmatch threshold 0.1), compared against the
pinned gate reference (the same-rasterizer chromium raster where one exists,
otherwise the Poppler raster). Structure-only or masked percentages are
geometry diagnostics and never satisfy or substitute for this gate. Report the
raw gate number, the Poppler-raster diagnostic, and the pinned noise floor
separately and honestly.

Calibration loop: `rtk npm run test:forms:visual` writes a region-ranked diff
report (worst regions first, with cropped expected|actual|diff strips) next to
its artifacts; `rtk npm run report:visual:regions` re-ranks the last captured
run in seconds without a browser; `rtk npm run diagnose:fonts` attributes the
residual to candidate font stacks against both rasters. Fix the top-ranked
regions; do not tweak blindly against a whole-page percentage.

## Triage by ownership

- Fix wrong values, formulas, validation, XML, or applicability in Rust.
- Fix missing values in the Rust envelope adapter or React binding after inspecting the generated fixture.
- Fix correct values in the wrong position in form-scoped TSX/CSS.
- Fix truncation or page-count errors in the form specification and pagination policy.
- Fix readiness, print, or export failures in the native HTML host/output path.
- Fix false support claims in migration and release-evidence manifests.
- Fix blurry or incorrect PDF417/QR/seal artwork by tracing the exact embedded
  pinned-PDF object and native geometry. Require a decoded payload,
  zero-difference logical matrix, crisp inline vector, and live bundled-font
  caption/static text. Preserve a seal/logo losslessly at native dimensions. Never use a
  rendered-page crop, download/generic substitute, threshold, resample, resize,
  recolor, or sharpen. If the official form has no code, retain its audited
  explicit absence and never fabricate one.
- Preserve adaptive character guides field by field. Count the exact guides
  and non-applicable cells from the pinned revision; never apply one generic
  count across a table. If the pinned revision shows a plain field, preserve it
  as plain: do not add comb cells, guide ticks, or a repeating guide
  background. The absence of guides is field-specific evidence and must not be
  overridden by a shared primitive or another form's pattern. Spaces and
  punctuation each consume one official
  character position. Empty, short, and exact-capacity values retain every
  official guide. Only a valid value longer than that field's official
  capacity switches to one plain text box in the same footprint, without
  truncation. Fit that plain text against its actual rendered field after the
  bundled font loads: start at the field's reviewed normal maximum so wide
  fields use readable type rather than inheriting the smaller comb-glyph size,
  reduce only in 0.5px steps to the reviewed readable floor, and block
  preview/print/export if it still does not fit. Prefer a reviewed wrapped
  layout over crossing that floor only when the fixed row can contain it with
  unchanged geometry and zero clipping. Never derive its font size from a
  character-count ratio.
  Merged or gray non-applicable cells never receive comb guides.
  Keep explicit empty, short, exact-capacity, and capacity-plus-one tests for
  every adaptive field pattern that the renderer introduces.
  Keep explicit empty and populated tests proving that each reviewed plain-field
  pattern remains free of comb cells and guide backgrounds.

Read [renderer-workflow.md](references/renderer-workflow.md) before changing shared primitives, native output, or release flags.

## Current repository truth

- Determine supported targets from `packages/form-specs/form-migration-status.json`; never rely on a hardcoded skill list.
- Re-read the manifest and registry for every task. Do not preserve a historical
  form count or assume that a committed component is `html_only` or release-ready.
- Treat `packages/form-renderer/src/forms/registry.ts` as the exact-revision dispatch source.
- Treat official page images as calibration-only and never as runtime backgrounds.
- Route a missing component or new revision to `$ebirforms-convert-form-to-html`.

## Standard workflow

1. Identify the exact code, revision, fixture, page, and failing region.
2. Inspect the serialized `RenderEnvelopeV1` before changing layout.
3. Patch the narrowest owning layer.
4. Add a regression fixture/test for the reported value or geometry.
5. Run targeted tests, then all affected gates.
6. Leave promotion flags false unless native, visual, and packaged evidence exists.

```sh
rtk npm run contracts:check
rtk npm run audit:forms:migration
rtk npm run typecheck:forms
rtk npm run test:forms
rtk npm run test:forms:visual
rtk npm run build:forms
rtk npm run verify:forms:offline
rtk cargo check --locked -p bir-desktop
```

Accept a fix only when the envelope is authoritative, official page geometry and page count hold, no field clips or truncates, visual evidence passes or is explicitly recorded as incomplete, and preview/print/export use the same semantic document.
