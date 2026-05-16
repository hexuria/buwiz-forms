# 1702 Corporate Print Readiness Gate

This checklist is the required gate before `1702RTv2018C` or `1702MXv2018C`
can be promoted from `ScaffoldOnly` to `ImplementedInApp`.

## Scope

- Forms: `1702RTv2018C`, `1702MXv2018C`
- Current status: typed draft scaffolds only
- Required result: generic `bir-print` rendering through `PrintRequest`, not a
  2551Q-specific render path

## Provenance

For each form pack, commit `formtypes/<FormID>/metadata.json` with:

- `source_kind`: `local` or `official_bir`
- `source_path` or `source_url`
- `sha256`
- `page_count`
- `form_revision`
- `extracted_at`
- `extraction_command`

Only source blank PDFs from local files supplied for this task or from official
BIR website/CDN locations. Do not use third-party PDF mirrors.

## Pack Generation

Use the repo-local formtype extraction flow:

```sh
just generate-form <pdf> <FormID> "<title>"
```

If the `just` target is unavailable, use the equivalent
`.scripts/generate_formtype.py` command and record the exact command in
`metadata.json`.

Expected outputs per form:

- `formtypes/<FormID>/formtype.json`
- one SVG page background per PDF page
- `formtypes/<FormID>/metadata.json`
- `formtypes/<FormID>/template.typ` or a documented shared template path

## Calibration

Every printable field in `formtype.json` must be manually calibrated to the
exact key emitted by the corresponding Rust draft's `to_bir_field_map()`:

- `crates/bir-core/src/forms/form_1702rt.rs`
- `crates/bir-core/src/forms/form_1702mx.rs`

Calibration must cover:

- field key names
- x/y positions
- page number
- font size
- checkbox rendering
- comb fields and `cell_w`
- date and amount alignment
- overflow behavior for long taxpayer names and addresses

## Engine Integration

The corporate preview buttons must stay disabled until the layout packs exist
and render successfully. When ready, wire them through the generic print path:

- build a `PrintRequest` with the form type ID and BIR field map;
- call generic `render_flat_pdf(PrintRequest)`;
- do not call `render_2551q_print` or any 2551Q-only template fallback.

`bir-print` currently has a 2551Q-oriented Typst fallback, so 1702 packs must
provide or reference a non-2551Q `template.typ` before preview is enabled.

## Verification

Required before promotion:

- `load_formtype_resolved("1702RTv2018C")` succeeds from `formtypes`
- `load_formtype_resolved("1702MXv2018C")` succeeds from `formtypes`
- every layout key maps to a key emitted by `to_bir_field_map()`
- generic `render_flat_pdf(PrintRequest)` creates a non-empty PDF when Typst is
  available
- visual spot checks pass against official blank PDF geometry
- validation, formulas, XML, persistence, and submission behavior are covered

Only after all items pass may either form move from `ScaffoldOnly` to
`ImplementedInApp`.
