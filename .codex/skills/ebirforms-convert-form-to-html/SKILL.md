---
name: ebirforms-convert-form-to-html
description: Convert an exact BIR/eBIRForms form revision into this repository's semantic React HTML print renderer with Rust-owned tax data, generated render contracts, pinned official references, deterministic pagination, native print/PDF evidence, and migration gates. Use when adding a missing HTML renderer, migrating a form away from Typst/formtypes/SVG backgrounds, onboarding a new BIR form revision, or auditing whether a form conversion is safe to promote. Do not use for a layout-only fix to an already HTML-enabled form; use ebirforms-print-preview instead.
---

# Convert an eBIRForms Form to HTML

Convert one exact form revision at a time. Treat conversion as a tax-data and release-readiness task, not as PDF tracing.

## Non-negotiable boundaries

- Keep Rust authoritative for models, formulas, validation, XML, persistence, carry-over, and every value in `RenderEnvelopeV1`.
- Keep React authoritative only for semantic document structure, styling, deterministic pagination, and readiness measurement.
- Never infer tax formulas or field meaning from one saved payload. Require primary-source or corroborated implementation evidence.
- Never create Typst templates, formtype JSON, coordinate overlays, or full-page runtime PDF/SVG/raster backgrounds.
- Use official pages only as pinned calibration references. Permit only reviewed
  discrete assets whose exact official-PDF provenance is recorded. Machine-readable
  symbols and the government seal/logo must follow
  [discrete-artwork.md](references/discrete-artwork.md); a screenshot crop is not
  sufficient evidence.
- Keep incomplete forms `ScaffoldOnly` or `disabled`. Never set promotion flags without recorded evidence.
- Preserve unrelated work and use `rtk` before shell commands in this repository.

## Workflow

1. **Lock identity and source.** Record the canonical code, exact revision, official source URL, source SHA-256, page count, and page geometry. Run `scripts/inventory_form.py` before editing. Read [source-evidence.md](references/source-evidence.md) and [conversion-contract.md](references/conversion-contract.md).
2. **Prove tax behavior.** Audit or implement the Rust model, formulas, validation, XML round-trip, persistence, queue/submission behavior, carry-over, and repeatable-row limits. Stop when evidence is insufficient.
3. **Add the render provider.** Map the typed Rust draft into `RenderEnvelopeV1`. Provide minimum, normal, long-value, validation-edge, and maximum-capacity fixtures. Do not repair or calculate values in TypeScript.
4. **Build semantic HTML.** Add the form component, exact-revision dispatch, scoped CSS, form specification, and pagination policy. Reuse shared paper, table, comb, checkbox, and amount primitives only when their official behavior matches.
5. **Verify discrete artwork.** Extract the exact seal/logo and every page-specific
   barcode or QR symbol from the pinned official PDF. Decode machine-readable
   payloads, preserve their module matrices as crisp vector artwork, keep captions
   as live bundled-font text, and record object/hash/geometry provenance. Read
   [discrete-artwork.md](references/discrete-artwork.md).
6. **Calibrate visually.** Use `scripts/prepare_official_reference.py` to render the pinned official PDF. Read [visual-calibration.md](references/visual-calibration.md). Inspect both full pages and critical regions.
7. **Prove output behavior.** Verify preview, system print, direct PDF export, page count, 612 x 936 point geometry where applicable, clipping detection, offline packaging, and platform evidence. Read [native-print-export.md](references/native-print-export.md).
8. **Audit and promote.** Run `scripts/verify_form_conversion.py` at `preview`, then `release` stage. Update migration/release evidence only after every named gate passes.

Use [architecture.md](references/architecture.md) when changing cross-form interfaces. Use [2551q-gold-standard.md](references/2551q-gold-standard.md) as the repository example, while respecting its current incomplete release status.

## Helper commands

```sh
rtk python3 .codex/skills/ebirforms-convert-form-to-html/scripts/inventory_form.py \
  --repo . --form-code 1601C --revision 2018 --output -

rtk python3 .codex/skills/ebirforms-convert-form-to-html/scripts/prepare_official_reference.py \
  --repo . --form-code 1601C --revision 2018 \
  --pdf /absolute/path/1601Cv2018.pdf --expected-sha256 <sha256> \
  --source-url <official-url>

rtk python3 .codex/skills/ebirforms-convert-form-to-html/scripts/verify_form_conversion.py \
  --repo . --form-code 1601C --revision 2018 --stage preview
```

Run repository gates after targeted tests:

```sh
rtk npm run contracts:check
rtk npm run audit:forms:migration
rtk npm run typecheck:forms
rtk npm run test:forms
rtk npm run test:forms:visual
rtk npm run build:forms
rtk npm run verify:forms:offline
rtk cargo test --locked --workspace
rtk cargo clippy --locked --workspace --all-targets
```

## Stop conditions

Stop and report the missing evidence rather than guessing when:

- the exact official revision cannot be pinned;
- formula, applicability, or XML semantics conflict;
- the request asks for a fileable form from only one XML/savefile sample;
- imported data exceeds a verified official capacity;
- the renderer clips, overflows, or changes page count;
- preview and PDF export do not use the same immutable envelope/document;
- packaged output needs Node, networking, Typst, or runtime form backgrounds;
- the migration manifest claims more support than tests prove.

The conversion is complete only when the exact revision is `html_only`, all required evidence is green, and the no-legacy audit passes.
