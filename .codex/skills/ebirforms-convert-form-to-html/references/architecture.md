# HTML Form Architecture

Use one direction of data flow:

```text
profile + draft + tax rules
          |
          v
Rust typed form -> validation/XML/persistence -> RenderEnvelopeV1
                                               |
                                               v
                                    exact-revision React component
                                               |
                                               v
                              readiness -> preview / print / PDF export
```

## Ownership

- `bir-core`: domain model, formulas, validation, XML, persistence, lifecycle.
- `bir-print`: generic render envelope, per-form provider, output validation, PDF utilities.
- `packages/form-contracts`: generated schema/types and deterministic fixtures.
- `packages/form-specs`: exact-revision geometry, pagination, capability, and release evidence.
- `packages/form-renderer`: semantic React layout and shared print primitives.
- `bir-desktop`: locked-down offline WebView and native print/export orchestration.

React must not infer missing tax facts, compute totals, choose elections, or normalize invalid domain values. Rust must not encode visual coordinates.

## Registry contract

Register each exact code/revision once. The provider must expose its adapter, fixture matrix, expected page count, paper geometry, and route state (`disabled`, `experimental`, or `html_only`). Generate UI capability lists from this registry instead of maintaining parallel constants.

## Runtime boundary

Ship the compiled renderer bundle only. Node and npm are build-time dependencies. Deny network access in the WebView. Do not ship Typst, `.typ` files, `formtypes`, or official full-page backgrounds.
