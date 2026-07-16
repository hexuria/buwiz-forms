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
- The renderer uses semantic tables/rows and discrete reviewed seal/barcode assets.
- Schedule 1 exercises deterministic multi-page behavior and capacity fixtures.
- Readiness measures geometry and reports overflow before native output.
- Official pages are calibration-only.

## Do not copy blindly

At this skill's creation, 2551Q is HTML-enabled but `release_ready` is false. Its reference manifest still records legacy provenance that must be replaced by direct official-PDF provenance. Its hardcoded provider/dispatch/generator shape must be generalized before treating it as a multi-form template.
