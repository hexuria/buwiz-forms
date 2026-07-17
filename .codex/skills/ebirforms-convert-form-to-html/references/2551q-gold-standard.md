# 2551Q:2018 Repository Example

Use 2551Q to understand the current HTML pipeline, not as evidence that another form behaves the same.

## Current anchors

- Rust envelope: `crates/bir-print/src/html.rs`
- Rust provider: `crates/bir-print/src/html_forms/form_2551q.rs`
- Contract generator: `crates/bir-print/src/bin/generate_render_contract.rs`
- Fixtures: `packages/form-contracts/fixtures/2551q-*.json`
- Component: `packages/form-renderer/src/forms/Form2551Q.tsx`
- Exact-revision dispatch: `packages/form-renderer/src/forms/registry.ts`
- Form spec: `packages/form-specs/src/index.ts`
- References: `packages/form-renderer/references/manifest.json`
- Migration truth: `packages/form-specs/form-migration-status.json`

## Patterns worth reusing

- Rust serializes all printable values.
- The renderer uses semantic tables/rows and discrete reviewed artwork.
- Each page's PDF417 payload and logical module matrix are verified against the
  pinned official PDF, rendered as crisp inline SVG, and positioned with the
  official page-specific geometry.
- Barcode captions are bundled-font live text rather than pixels baked into a
  cropped image.
- The seal/logo remains a separate official-PDF-sourced asset with recorded
  derivation provenance; other forms must verify their own exact artwork rather
  than reuse 2551Q blindly.
- Schedule 1 exercises deterministic multi-page behavior and capacity fixtures.
- Overflow plain fields start at the largest ceiling allowed by the exact
  field height: 21px for the page-one name, address, and email rows, and
  21.5px for Item 12A and the page-two taxpayer-name row. The browser measures
  the loaded Arimo text in the final field and descends in 0.5px steps only to
  the reviewed 10.5px floor. Scroll dimensions, the field/owner bounds, and
  every DOM `Range` line rectangle must all fit. Character count decides only
  whether to leave comb mode; it never selects the font size.
- The committed long-value fixture currently resolves to 20.5px for the
  page-one name, 12.5px for the address, 17.5px for email, 12px for Item 12A,
  and 13px for the page-two name. The last value is genuinely width-limited:
  its 60-character text occupies 471.47px of a 490px field at 13px, while the
  next 13.5px candidate clips after padding. Do not enlarge it by distorting
  tracking or changing official geometry. Values that still cannot fit at
  10.5px retain their full text, report unresolved geometry, and block output.
- Readiness measures geometry and reports overflow before native output.
- Official pages are calibration-only.

## Do not copy blindly

2551Q is HTML-enabled but `release_ready` is false. Its verified PDF417 workflow
and exact native official-PDF seal extraction are artwork examples; its remaining
layout, native-platform, and package gates are still incomplete. Every other form
must independently satisfy [discrete-artwork.md](discrete-artwork.md). Its
hardcoded provider/dispatch/generator shape must be generalized before treating it
as a multi-form template.
