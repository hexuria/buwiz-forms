---
name: ebirforms-convert-form-to-html
description: Convert an exact BIR/eBIRForms form revision into this repository's semantic React HTML print renderer with Rust-owned tax data, generated render contracts, pinned official references, deterministic pagination, native print/PDF evidence, and migration gates. Use when adding a missing HTML renderer, migrating a form away from Typst/formtypes/SVG backgrounds, onboarding a new BIR form revision, auditing whether a form conversion is safe to promote, or evaluating a request to generate a fileable form from XML/savefile samples so insufficient evidence fails closed. Do not use for a layout-only fix to an already HTML-enabled form; use ebirforms-print-preview instead.
---

# Convert an eBIRForms Form to HTML

Convert one exact form revision at a time. Treat conversion as a tax-data and release-readiness task, not as PDF tracing.

> **Strategy amendment (2026-07-20):** read
> `docs/form-print-readiness/conversion-strategy-v2.md` first. It reorders
> this workflow around measured findings: geometry is generated from PDF
> ground-truth extraction and then reviewed (not hand-measured); calibration
> targets STRUCTURE pixels only (text-neutralized comparison); text
> correctness is proven by exhaustive static-text content assertions, never
> by chasing text pixels (all 35 source PDFs carry substituted fonts, making
> text-pixel parity unreachable by adversarially-verified proof). Every
> fail-closed rule below remains in force.

**The 1% complete-page gate is unreachable and must not be chased.** Every one
of the 35 source PDFs has non-embedded fonts, so the reference encodes
Poppler's substituted glyph outlines; glyph shape is ~57% of the residual and
every rendering-side fix was measured and refuted. Text correctness is proven
by `static-text-exhaustive-v1` content assertions, never by pixels. Spend
calibration effort on STRUCTURE, which is winnable.

The release criterion is `official-fidelity-v1`, a composite of six bound
components, currently in reporting mode and not gating. It is a
**non-regression** criterion: it certifies "no worse than a reviewed baseline",
never "matches the official form". The complete-page percentage remains
mandatory and reported alongside the Poppler diagnostic and pinned noise floor;
a masked, structure-only or text-excluded number is never parity. See
`docs/form-print-readiness/official-fidelity-criterion-v1.md` and CLAUDE.md.

## Non-negotiable boundaries

- Keep Rust authoritative for models, formulas, validation, XML, persistence, carry-over, and every value in `RenderEnvelopeV1`.
- Keep React authoritative only for semantic document structure, styling, deterministic pagination, and readiness measurement.
- Never infer tax formulas or field meaning from one saved payload. Require primary-source or corroborated implementation evidence.
- Never create Typst templates, formtype JSON, coordinate overlays, or full-page runtime PDF/SVG/raster backgrounds.
- Preserve adaptive character guides field by field. Measure each field's
  exact guide capacity and any blank, merged, or non-applicable cells from the
  pinned revision; never reuse one generic count across a table. A field with
  no guides in the pinned revision is a plain field: render no comb cells,
  guide ticks, or repeating guide background there. Absence of guides is
  reviewed geometry evidence, not permission to infer them from another form.
  Spaces and
  punctuation each consume one character position. Empty, short, and
  exact-capacity values retain every official guide. Only a valid value longer
  than that field's official capacity switches to one untruncated plain text
  box in the same footprint. Measure the real rendered field after the bundled
  font loads, start at its field-specific reviewed normal maximum so available
  width produces readable type, and reduce only in 0.5px steps to a reviewed
  readable floor. A wrapped fallback is allowed only with exact-revision proof
  that the fixed row and page geometry remain unchanged and unclipped. If it
  still cannot fit, block preview/print/export; never derive font size from a
  character-count ratio.
  Gray or merged non-applicable cells receive no guides.
- Use official pages only as pinned calibration references. Runtime artwork must
  come from the exact embedded image/XObject or vector object in that pinned PDF,
  at its native dimensions. Never use a rendered-page crop, downloaded/generic
  substitute, thresholding, resampling, resizing, recoloring, or sharpening.
  Machine-readable symbols and government identity artwork must follow
  [discrete-artwork.md](references/discrete-artwork.md).
- Keep incomplete forms `ScaffoldOnly` or `disabled`. Never set promotion flags without recorded evidence.
- Preserve unrelated work and use `rtk` before shell commands in this repository.

## Workflow

1. **Lock identity and source.** Record the canonical code, exact revision, official source URL, source SHA-256, page count, and page geometry. Run `scripts/reference/inventory_form.py` before editing. Read [source-evidence.md](references/source-evidence.md) and [conversion-contract.md](references/conversion-contract.md).
2. **Prove tax behavior.** Audit or implement the Rust model, formulas, validation, XML round-trip, persistence, queue/submission behavior, carry-over, and repeatable-row limits. Stop when evidence is insufficient.
3. **Add the render provider.** Map the typed Rust draft into `RenderEnvelopeV1`. Provide minimum, normal, long-value, validation-edge, and maximum-capacity fixtures. Inventory plain fields separately from guided fields. For every adaptive character field pattern, prove empty, short, exact-capacity, and capacity-plus-one behavior; for every reviewed plain-field pattern, prove that empty and populated values render without comb cells or guide backgrounds. Use component-level fixtures only when the domain intentionally exposes no printable value for that field. Do not repair or calculate values in TypeScript.
4. **Build semantic HTML.** Add the form component, exact-revision dispatch, scoped CSS, form specification, and pagination policy. Reuse shared paper, table, comb, checkbox, and amount primitives only when their official behavior matches.
5. **Verify discrete artwork.** Inventory every physical page. Extract each exact
   seal/logo and page-specific PDF417/QR object from the pinned PDF. Decode every
   payload, prove a zero-difference logical module matrix, render it as crisp inline
   vector artwork, and keep captions/static text as live bundled-font text. When the official
   PDF contains no machine-readable symbol, record an audited explicit absence and
   render none; never fabricate one from the form identity. Read
   [discrete-artwork.md](references/discrete-artwork.md).
6. **Calibrate visually.** Use `scripts/reference/prepare_official_reference.py` to render the pinned official PDF into the Poppler raster, then repo-root `scripts/prepare_chromium_reference.mjs` to add the same-rasterizer chromium gate reference with its pinned noise floor, and pin both in the Rust provider (`npm run references:generate`). Read [visual-calibration.md](references/visual-calibration.md). Inspect both full pages and critical regions.
7. **Prove output behavior.** Verify preview, system print, direct PDF export, page count, 612 x 936 point geometry where applicable, clipping detection, offline packaging, and platform evidence. Read [native-print-export.md](references/native-print-export.md).
8. **Audit and promote.** Run `scripts/reference/verify_form_conversion.py` at `preview`, then `release` stage. Update migration/release evidence only after every named gate passes.

Use [architecture.md](references/architecture.md) when changing cross-form interfaces. Use [2551q-gold-standard.md](references/2551q-gold-standard.md) as the repository example, while respecting its current incomplete release status.

## Helper commands

```sh
rtk python3 .codex/skills/ebirforms-convert-form-to-html/scripts/quick_validate.py \
  .codex/skills/ebirforms-convert-form-to-html

rtk python3 scripts/reference/inventory_form.py \
  --repo . --form-code 1601C --revision 2018 --output -

rtk python3 scripts/reference/prepare_official_reference.py \
  --repo . --form-code 1601C --revision 2018 \
  --pdf /absolute/path/1601Cv2018.pdf --expected-sha256 <sha256> \
  --source-url <official-url>

rtk python3 scripts/reference/verify_form_conversion.py \
  --repo . --form-code 1601C --revision 2018 --stage preview

rtk python3 -m unittest discover -s .codex/skills/tests -p 'test_*.py'
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
- an exact embedded artwork object cannot be proven and substitution would be required;
- packaged output needs Node, networking, Typst, or runtime form backgrounds;
- the migration manifest claims more support than tests prove.

The conversion is complete only when the exact revision is `html_only`, all required evidence is green, and the no-legacy audit passes.
