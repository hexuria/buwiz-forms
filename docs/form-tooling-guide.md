# HTML Form Tooling Guide

The canonical agent workflows are versioned under `.codex/skills/`. New form work must use the semantic HTML renderer.

## Choose the correct skill

| Task | Skill |
|---|---|
| Convert a missing form or exact revision to HTML | `.codex/skills/ebirforms-convert-form-to-html` |
| Fix or calibrate an already HTML-enabled form | `.codex/skills/ebirforms-print-preview` |
| Infer a tax formula from one savefile | Stop; gather authoritative/corroborated evidence |
| Generate a snapshot-overlay renderer or full-page runtime background | Do not use this architecture |

Determine enabled/release-ready targets from `packages/form-specs/form-migration-status.json`; do not rely on a hardcoded form list in documentation.

## Conversion helpers

Inventory the current implementation without generating code:

```sh
rtk python3 .codex/skills/ebirforms-convert-form-to-html/scripts/inventory_form.py \
  --repo . --form-code 1601C --revision 2018 \
  --source-dir /Users/uriah/Downloads/forms/1601Cv2018 --output -
```

Pin and render an exact official PDF as calibration-only references:

```sh
rtk python3 .codex/skills/ebirforms-convert-form-to-html/scripts/prepare_official_reference.py \
  --repo . --form-code 1601C --revision 2018 \
  --pdf /absolute/path/1601Cv2018.pdf \
  --expected-sha256 <sha256> --source-url <official-url>
```

Audit the implementation before enabling preview or release routing:

```sh
rtk python3 .codex/skills/ebirforms-convert-form-to-html/scripts/verify_form_conversion.py \
  --repo . --form-code 1601C --revision 2018 --stage preview
```

Read the conversion skill for the evidence contract, fixture matrix, visual method, and native print/PDF gates.

## Architecture boundary

- Rust owns domain data, formulas, validation, XML, persistence, and printable values.
- Generated `RenderEnvelopeV1` contracts cross into the renderer.
- React owns semantic layout and deterministic pagination only.
- The same offline HTML document powers preview, print, and direct PDF export.
- Official PDFs/PNGs are calibration evidence, never runtime page backgrounds.
- Node/npm are build-time dependencies; production packages ship only compiled renderer assets.

## Retired architecture

Do not use:

- any retired snapshot-overlay form generator;
- full-page runtime image or vector backgrounds;
- filesystem-loaded layout packs;
- a second document compiler or fallback viewer;
- generated tax behavior from a single payload sample.

The retired renderer, editor, calibration views, runtime layout packs, and
packaging hooks are absent from the repository. `npm` remains a build/CI tool;
production packages contain only the compiled offline renderer assets.
