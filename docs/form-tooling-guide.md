# HTML Form Tooling Guide

Use this guide to choose the owning tool and status gate for form work. The
repository has one semantic HTML document pipeline; fixture visibility and
filing authority remain independent.

All command examples use standard developer tools. Repository-local agent
workflows under `.codex/skills/` document the same evidence contract but are
not required to run these commands.

## Sources of truth

| Question | Authority |
| --- | --- |
| Can this exact revision queue a filing? | `crates/bir-core/src/forms/support_level.rs` |
| Which document route and capabilities are recorded? | `packages/form-specs/form-migration-status.json` |
| Which visual, native, and package evidence was reviewed? | `packages/form-specs/form-release-evidence.json` |
| Which official pages and hashes calibrate the renderer? | `packages/form-renderer/references/manifest.json` |
| Which React component handles the exact revision? | `packages/form-renderer/src/forms/registry.ts` |

Current manifest v3 status is deliberately conservative: every form is
`ScaffoldOnly`, only `2551Q:2018` uses the `html_only` route, the other committed
renderers are `experimental`, and no form is `release_ready`.

## Choose the workflow

| Task | Workflow |
| --- | --- |
| Convert a missing form or exact revision | `.codex/skills/ebirforms-convert-form-to-html` |
| Fix or calibrate an existing HTML component | `.codex/skills/ebirforms-print-preview` |
| Investigate wrong formulas, applicability, XML, or filing values | Rust domain/provider audit |
| Investigate correct values in the wrong place | Form-scoped React/CSS calibration |
| Investigate clipping or wrong page count | Exact-revision specification and pagination |
| Investigate print/export mismatch | Shared native HTML output lifecycle |
| Infer authoritative tax behavior from one payload | Stop and gather corroborated evidence |
| Generate a full-page runtime overlay | Unsupported architecture |

The canonical skills are versioned under `.codex/skills/`. Do not keep copied
machine-local variants that can drift from the repository.

## Inventory and source preparation

Inventory an exact target without generating authoritative behavior:

```sh
python3 .codex/skills/ebirforms-convert-form-to-html/scripts/inventory_form.py \
  --repo . --form-code 1601C --revision 2018 \
  --source-dir /Users/uriah/Downloads/forms/1601Cv2018 --output -
```

Prepare exact official PDF references and provenance:

```sh
python3 .codex/skills/ebirforms-convert-form-to-html/scripts/prepare_official_reference.py \
  --repo . --form-code 1601C --revision 2018 \
  --pdf /absolute/path/1601Cv2018.pdf \
  --expected-sha256 <sha256> --source-url <official-url>
```

The script may hash sources, render deterministic page PNGs, and record
geometry. It must not guess formulas or generate authoritative form layout.

## Discrete artwork evidence

Inventory every physical page in the pinned PDF. Extract the exact embedded
seal/logo object at native dimensions and preserve it losslessly. For every
PDF417 or QR symbol, record its PDF object/hash/CTM, decode the exact payload,
prove a zero-difference logical module matrix, render the matrix as crisp inline
vector artwork, and render caption/static text separately with a bundled offline font.

Do not use a rendered-page crop, downloaded/generic substitute, thresholding,
resampling, resizing, recoloring, or sharpening. If no code exists, record an
audited explicit absence covering every page and add a test that no symbol is
rendered. `0605:1999` is the canonical no-symbol case; never fabricate a code
from its form identity.

## Contract and fixture tools

Generate Rust capabilities, `RenderEnvelopeV1` schema, TypeScript types, and
canonical fixtures, then fail on drift:

```sh
npm run contracts:check
```

Fixtures contain form identity, taxpayer and period data, Rust-owned fields,
schedules, and validation messages. They are renderer inputs, not filing
evidence by themselves.

Use the fixture to identify ownership:

- wrong or missing value in JSON: Rust draft/provider;
- correct JSON but missing markup: React binding;
- correct markup but wrong geometry: form-scoped CSS;
- wrong page break: form specification/pagination;
- correct HTML but bad print/export: native output lifecycle.

## Calibration viewer

Install dependencies and start the committed-fixture viewer:

```sh
npm ci
npm run dev:calibration
```

Open `http://127.0.0.1:4175`. The viewer provides:

- a searchable selector for committed fixtures;
- explicit route and scaffold/release labels;
- continuous scrolling across all physical pages;
- Previous, Next, and page-number scroll shortcuts;
- automatic reference loading from the official manifest;
- HTML, Overlay, and Difference modes;
- independent reference-opacity control.

The viewer never needs a manually uploaded fixture and never places the
reference image in the printable output. A renderer entry in this viewer is not
proof of queue authority, visual parity, or release readiness.

Build the viewer without starting a server:

```sh
npm run build:calibration
```

## Visual and geometry tests

Run renderer unit tests and complete-page reference comparisons:

```sh
npm run typecheck:forms
npm run test:forms
npm run test:forms:visual
```

The visual release assertion uses the complete page at the pinned manifest
DPI. Region and line-structure diagnostics may help locate a problem but cannot
replace the full-page gate. Keep `visual_parity` false whenever a page exceeds
the threshold, a critical region is wrong, or any valid value clips.

## Build and offline verification

Build the embedded preview bundle and verify that its asset graph is offline:

```sh
npm run build:forms
npm run verify:forms:offline
```

Node and npm are build-time dependencies. A production package contains only
compiled static assets and must work with networking disabled.

Audit that retired document paths and runtime layout packs are absent:

```sh
npm run audit:no-legacy
```

## Status audits

Run the manifest v3 audit after changing a provider, fixture, specification,
route, capability, or evidence entry:

```sh
npm run audit:forms:migration
```

Audit one exact conversion before promotion:

```sh
python3 .codex/skills/ebirforms-convert-form-to-html/scripts/verify_form_conversion.py \
  --repo . --form-code 1601C --revision 2018 --stage preview
```

Status fields are independent:

- `support_level` is the Rust-owned production capability result;
- `route: experimental` allows development calibration only;
- `route: html_only` selects the semantic document and has no alternate route;
- capability booleans record reviewed evidence, not intent;
- `release_ready` requires every mandated capability and platform record.

## Command reference

| Goal | Command |
| --- | --- |
| Install JS workspace | `npm ci` |
| Start calibration viewer on port 4175 | `npm run dev:calibration` |
| Build calibration viewer | `npm run build:calibration` |
| Check generated contracts | `npm run contracts:check` |
| Type-check form workspaces | `npm run typecheck:forms` |
| Run renderer unit tests | `npm run test:forms` |
| Run official-reference comparisons | `npm run test:forms:visual` |
| Build embedded renderer | `npm run build:forms` |
| Verify offline assets | `npm run verify:forms:offline` |
| Audit manifest v3 | `npm run audit:forms:migration` |
| Audit retired paths | `npm run audit:no-legacy` |

## Promotion checklist

Before queue enablement, prove typed behavior, exact XML, formulas, validation,
persistence, carry-over, amendments, queue adapter, and editor behavior.

Before visual promotion, prove the complete official page, exact paper and page
count, long values, schedules, critical regions, zero clipping, and the strict
embedded-artwork or explicit no-symbol evidence above.

Before release promotion, record preview, system print, direct PDF export, and
packaged-offline evidence on macOS, Windows, and Linux using the same immutable
HTML document.

If a gate fails, keep the form visibly scaffold-only or experimental and report
the diagnostic. Do not hide missing evidence by switching document paths.
