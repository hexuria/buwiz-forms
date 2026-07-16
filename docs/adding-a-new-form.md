# Adding a BIR Form

All printable forms use the Rust-contract and semantic HTML pipeline. Start
with the canonical conversion workflow:

```text
.codex/skills/ebirforms-convert-form-to-html/SKILL.md
```

## Required evidence

Lock the exact form code and revision before writing code. Record the official
source URL or reviewed local source, SHA-256, page count, and page geometry.
The reviewed source packs are under `/Users/uriah/Downloads/forms` during local
development; copied runtime page images are prohibited.

Do not infer tax formulas, applicability, or XML semantics from one saved XML
payload. A form stays `ScaffoldOnly` or manual/external until the typed model,
formula evidence, XML round trip, persistence, validation, carry-over, and
queue behavior are independently proven.

## Implementation boundary

- Rust owns data, calculations, validation, field applicability, and printable
  values.
- A provider in `crates/bir-print/src/html_forms/` converts a typed draft into
  `RenderEnvelopeV1` and supplies the required fixture matrix.
- React in `packages/form-renderer/src/forms/` owns semantic markup, scoped CSS,
  exact-revision pagination, and layout only.
- Native preview, print, and direct PDF export use the same immutable envelope
  and bundled offline renderer document.
- Official page rasters are test references only. Runtime assets may contain
  reviewed discrete artwork such as a seal or form barcode.

## Promotion gates

Run the conversion verifier from the skill, then the repository gates:

```sh
rtk npm run contracts:check
rtk npm run audit:forms:migration
rtk npm run typecheck:forms
rtk npm run test:forms
rtk npm run test:forms:visual
rtk npm run build:forms
rtk npm run verify:forms:offline
rtk npm run audit:no-legacy
rtk cargo test --locked --workspace
rtk cargo clippy --locked --workspace --all-targets -- -D warnings
```

Only set the manifest route to `html_only` and `release_ready` after contract,
visual, native output, and packaged-offline evidence are all green for the
exact revision. Missing renderers remain visible as manual/external Forms Set
entries; they do not receive a fallback renderer.
