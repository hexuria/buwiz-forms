# Semantic HTML Form Renderer

Last verified: **July 19, 2026**.

The application has one printable-document architecture:

1. Rust owns typed form data, calculations, validation, applicability, XML,
   persistence, queue behavior, and printable values.
2. A Rust provider serializes an immutable `RenderEnvelopeV1` for one exact
   form revision.
3. React and form-scoped CSS render the official semantic structure.
4. An exact-revision specification owns paper size and pagination.
5. The same offline HTML document powers preview, system print, and direct PDF
   export.

Official PDF rasters are calibration-only evidence. They are never printable
runtime backgrounds, and rendering failure never switches to another document
implementation.

## Machine-readable status

Read current status from:

- `crates/bir-core/src/forms/support_level.rs` — Rust capability registry;
- `packages/form-specs/form-migration-status.json` — schema v3 route,
  capability, support-level, and release status;
- `packages/form-specs/form-release-evidence.json` — reviewed visual, native,
  and packaged evidence;
- `packages/form-renderer/references/manifest.json` — official reference
  sources, hashes, page geometry, and discrete artwork provenance.

Current summary:

| Exact revisions | Route | Support level | Release status |
| --- | --- | --- | --- |
| `2551Q:2018` | `html_only` | `ScaffoldOnly` | Not release-ready; full-page visual and signed platform/package evidence are incomplete |
| `1601C:2018` | `experimental` | `ScaffoldOnly` | Queue and exact XML round-trip capabilities are proven; visual, native output, and packaged-offline evidence remain incomplete |
| `1701Q:2018` | `experimental` | `ScaffoldOnly` | Exact XML round trip is proven; queue/submission, visual, native output, and packaged-offline evidence remain incomplete |
| `0605:1999`, `0619E:2018`, `0619F:2018`, `2550Q:2024`, `1701:2018`, `1702RT:2018C`, `1702MX:2018C` | `experimental` | `ScaffoldOnly` | Calibration only; queue and release evidence are incomplete |

All listed revisions have providers, fixtures, components, specifications, and
pagination. None currently has `visual_parity: true` or `release_ready: true`.
The existence of a committed reference page proves provenance, not parity.

## Contract and renderer registry

`RenderEnvelopeV1` is generated from Rust and consumed as a versioned contract.
React must not calculate tax, infer missing selections, repair invalid input, or
invent totals.

Generate and check capabilities, fixtures, schema, and TypeScript types:

```sh
npm run contracts:check
```

Exact-revision dispatch is registered in
`packages/form-renderer/src/forms/registry.ts`. Paper and pagination data live
under `packages/form-specs/`. Each provider should include minimum, normal,
long-value, validation-edge, and maximum-schedule fixtures.

Inspect a fixture first when a rendered value is wrong:

- wrong in JSON: fix the Rust draft or provider;
- correct in JSON but absent: fix React binding;
- correct but misplaced: fix form-scoped TSX/CSS;
- clipped or on the wrong page: fix form specification or pagination;
- print/export mismatch: fix the shared native output lifecycle.

## Official references

The reference manifest pins, for every exact revision:

- official PDF source and SHA-256;
- revision, physical page count, and point geometry;
- 144 DPI page PNG paths, dimensions, and hashes;
- calibration-only provenance;
- exact embedded seal/logo objects with native dimensions and source hashes;
- decoded PDF417/QR payloads, zero-difference logical matrices, inline vectors,
  and bundled-font live captions/static text, or an audited explicit no-symbol result.

Reference PNGs are rendered directly from pinned official PDFs. Do not replace
a page from an unverified download or use a full official page inside the
runtime bundle.

Runtime artwork must never come from a rendered-page crop or downloaded/generic
substitute. Preserve native embedded objects losslessly; do not threshold,
resample, resize, recolor, or sharpen them. A form such as `0605:1999` that has
no machine-readable symbol must record that audited absence and render no code.

To prepare a new exact revision, use the repository conversion workflow:

```sh
python3 .codex/skills/ebirforms-convert-form-to-html/scripts/prepare_official_reference.py \
  --repo . --form-code 1601C --revision 2018 \
  --pdf /absolute/path/1601Cv2018.pdf \
  --expected-sha256 <sha256> --source-url <official-url>
```

## Calibration viewer

Install dependencies once and run the viewer from the repository root:

```sh
npm ci
npm run dev:calibration
```

Open `http://127.0.0.1:4175`.

### Fixture selection

The viewer discovers committed fixture JSON through a searchable selector. No
manual fixture upload is required. The selected entry displays exact form code,
revision, fixture, route, support level, and paper facts so an experimental
scaffold cannot be mistaken for a production form.

### Page navigation

All physical pages render in one continuous scroll region. Normal scrolling is
the primary interaction. Previous, Next, and page-number controls scroll to a
physical page and do not alter pagination.

### Comparison controls

When the reference manifest contains the matching exact revision, the viewer
loads its official page automatically:

- **HTML** shows only the semantic document;
- **Overlay** blends the official raster above the HTML page;
- **Difference** highlights visual disagreement;
- **Reference opacity** changes only the calibration layer.

The calibration layer is never present in preview, print, export, or the
packaged renderer.

## Visual acceptance

Run the verified-reference suite:

```sh
npm run test:forms:visual
```

The release assertion compares the complete page at the manifest DPI. The
strict target is at most 1% masked pixel difference plus exact critical-region
checks. Every current form still exceeds the whole-page threshold, so all
`visual_parity` flags remain false.

Line-structure, region, or mask diagnostics may guide calibration, but they
cannot replace the complete-page gate. Never report a sparse diagnostic as
whole-page similarity, weaken the threshold, or hide semantically wrong
regions behind a broad mask.

Always test:

- exact physical page count and dimensions;
- headers, form identity, barcodes, choices, totals, and signatures;
- long valid names, addresses, phone numbers, emails, descriptions, and
  amounts;
- maximum repeatable-row capacity and conditional pages;
- no clipping, overflow, hidden values, or silent truncation;
- stable geometry after fonts finish loading.

## Native preview, print, and PDF export

All outputs use the currently loaded immutable envelope. The shared preflight:

1. enters print-mode CSS;
2. waits for fonts and two identical geometry measurements;
3. validates page count, paper geometry, page rectangles, clipping, and
   overflow;
4. consumes a one-use nonce;
5. invokes the platform backend against that same WebView document.

The exported PDF is validated for page count, MediaBox/CropBox dimensions,
rotation, finite geometry, and non-empty page content before atomically
replacing the selected destination. Failure leaves an existing destination
unchanged.

Record reviewed preview, system-print, and direct-export evidence on macOS,
Windows, and Linux before setting the corresponding capabilities.

The current macOS development-only transcript verifier and its explicit trust
boundary are documented in
[macOS native-output evidence foundation](macos-native-evidence-foundation.md).
It verifies supplied artifacts but does not collect runtime evidence or promote
form readiness.

## Offline build and packaging

Build the embedded renderer and verify its locked-down asset graph:

```sh
npm run build:forms
npm run verify:forms:offline
```

Node and npm are build-time dependencies only. Production packages contain the
compiled static renderer assets, use no network resources, and contain no
alternate document compiler or runtime layout packs.

Run the no-retired-path audit:

```sh
npm run audit:no-legacy
```

## Migration audit and promotion

Audit manifest v3 after every provider, route, capability, or evidence change:

```sh
npm run audit:forms:migration
```

The independent status fields mean:

- `support_level` describes the Rust-owned production capability gate;
- `route: experimental` permits development calibration;
- `route: html_only` selects the semantic document without promising release
  readiness;
- `capabilities.visual_parity`, native output, and packaged-offline flags must
  reflect reviewed evidence;
- `release_ready` is true only when every required capability is green.

Do not change a status because a fixture loads, pagination is stable, or a
structural diagnostic looks close. Promote only the exact revision supported by
the evidence.

## Troubleshooting

- **Fixture missing:** run `npm run contracts:check` and inspect the provider
  registry and generated fixture path.
- **Reference missing:** inspect
  `packages/form-renderer/references/manifest.json`; do not upload a private
  image into the viewer as a substitute.
- **Wrong number of pages:** inspect the exact-revision specification and
  conditional-page policy before changing CSS.
- **Washed-out or colored form:** inspect form-scoped print colors and browser
  print-adjust rules; the official document should remain black, white, and
  reviewed gray values.
- **Visual suite fails:** use Overlay and Difference to fix the owning semantic
  region; keep the release flag false.
- **Native output fails:** preserve the immutable envelope and diagnostic;
  retry the same HTML path rather than switching renderers.
