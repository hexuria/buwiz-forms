---
name: ebirforms-print-preview
description: Diagnose, fix, calibrate, or review an existing eBIRForms semantic HTML print preview, visual parity issue, renderer binding, deterministic pagination, clipping report, native print, or direct PDF export. Use only when the exact form revision already has an HTML component and is enabled in the migration manifest. Use ebirforms-convert-form-to-html when the renderer is missing or a new form/revision must be migrated.
---

# Maintain an eBIRForms HTML Print Preview

Fix an already-migrated form through the owning layer. Do not create a second rendering path.

## Triage by ownership

- Fix wrong values, formulas, validation, XML, or applicability in Rust.
- Fix missing values in the Rust envelope adapter or React binding after inspecting the generated fixture.
- Fix correct values in the wrong position in form-scoped TSX/CSS.
- Fix truncation or page-count errors in the form specification and pagination policy.
- Fix readiness, print, or export failures in the native HTML host/output path.
- Fix false support claims in migration and release-evidence manifests.
- Fix blurry or incorrect barcode/QR/seal artwork by tracing the exact pinned
  official-PDF object, decoded payload, module matrix, physical geometry, and
  bundled-font caption. Never replace it with an unverified crop or generic logo.

Read [renderer-workflow.md](references/renderer-workflow.md) before changing shared primitives, native output, or release flags.

## Current repository truth

- Determine supported targets from `packages/form-specs/form-migration-status.json`; never rely on a hardcoded skill list.
- At this skill's creation, only `2551Q:2018` is HTML-enabled, and it is not release-ready.
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
