# Existing HTML Preview Workflow

## Establish scope

Read these sources before editing:

- `packages/form-specs/form-migration-status.json`
- `packages/form-specs/form-release-evidence.json`
- `packages/form-renderer/src/forms/registry.ts`
- `packages/form-renderer/references/manifest.json`
- The failing contract fixture and form component

If no exact-revision component exists, stop and use `$ebirforms-convert-form-to-html`.

## Diagnose

1. Serialize or inspect the Rust-generated fixture.
2. Confirm the contract value and type.
3. Reproduce the layout at the visual-test viewport/DPI.
4. Inspect readiness diagnostics for clipping, overflow, or unstable geometry.
5. Patch only the owning layer.

Treat each pinned PDF page as an independent page-indexed reference. Preview,
Chrome, and Preview.app may switch between single-page and two-page-spread
presentation; that UI choice must never alter the expected page count or pair
two official pages into one comparison target.

Never fix a wrong value with CSS or recompute it in React. Never fix layout by embedding the official page.

## Verify

Run the narrow form/unit test first. Then run contracts, migration audit, type checking, renderer tests, visual parity, production bundle, and offline verification. For native work, also compile the desktop crate and gather development plus packaged evidence on each affected platform.

Do not set `release_ready` from screenshots alone. Require exact page geometry, full fixture coverage, native print/PDF evidence, packaged-offline evidence, and a passed rollback/no-legacy policy.
