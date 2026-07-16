# Adding a BIR Form

This is the implementation and review checklist for one exact BIR form
revision. Filing support and printable-document readiness are separate gates.
A fixture, React component, or calibration-viewer entry does not make a form
fileable or release-ready.

The machine-readable sources of truth are:

- `crates/bir-core/src/forms/support_level.rs` for Rust-owned capabilities;
- `packages/form-specs/form-migration-status.json` for route and migration
  status;
- `packages/form-specs/form-release-evidence.json` for reviewed platform
  evidence;
- `packages/form-renderer/references/manifest.json` for pinned visual
  references.

For a guided conversion, use the canonical repository workflow at
`.codex/skills/ebirforms-convert-form-to-html/SKILL.md`. The commands below use
standard developer tools and do not require an agent-specific wrapper.

## 1. Lock the exact revision and evidence

Before writing code, record:

- form code, exact revision, official title, page count, and paper dimensions;
- official source URL or reviewed local source and its SHA-256 hash;
- all applicable main pages, schedules, and conditional attachments;
- representative plain and encrypted XML, when available;
- formula, validation, carry-over, and filing-frequency evidence;
- discrete artwork that must be cropped from the pinned source, such as the
  government seal or static form barcode.

The reviewed source packs used during local development are under
`/Users/uriah/Downloads/forms`. They are evidence inputs, not runtime layout
assets. Never use an official full-page image as the printable document
background.

Do not infer tax formulas, applicability, or XML semantics from one saved XML
payload. If evidence is incomplete, keep the exact revision `ScaffoldOnly` or
manual/external and record the missing evidence explicitly.

Inventory an exact target without generating authoritative behavior:

```sh
python3 .codex/skills/ebirforms-convert-form-to-html/scripts/inventory_form.py \
  --repo . --form-code 1601C --revision 2018 \
  --source-dir /Users/uriah/Downloads/forms/1601Cv2018 --output -
```

## 2. Implement the Rust filing surface

Rust owns all tax behavior. Add or audit the form module in
`crates/bir-core/src/forms/` and prove:

1. the typed draft covers every supported official field and repeatable row;
2. calculations and rates use reviewed evidence and deterministic decimal
   arithmetic;
3. validation reports field-specific, actionable errors;
4. applicability and annual-election rules are represented explicitly;
5. the draft persists and reopens without losing fields or rows;
6. exact XML field names and formatting round-trip representative payloads;
7. amended-return and carry-over behavior are deterministic;
8. queue submission uses the correct form type, revision, and filename.

Do not duplicate formulas in the desktop UI or React renderer. A form can have
an editor and printable scaffold while queue submission remains disabled.

## 3. Add persistence and the desktop editor

Persist the complete typed draft under the correct monthly, quarterly, or
annual key. Add migration coverage when the stored shape changes.

The desktop view should:

- use the shared form engine and common section components;
- expose every user-editable field and official repeatable-row capacity;
- reject excess imported rows instead of silently truncating them;
- display validation and applicability state from Rust;
- preserve immutable snapshots after the form leaves draft state;
- make unsupported filing actions visibly unavailable.

The Forms Set may recommend a revision that is not yet renderable or fileable.
Show that entry as manual/external rather than routing it to another renderer.

## 4. Add the Rust render provider and fixtures

Add the exact-revision provider under `crates/bir-print/src/html_forms/`. It
must convert the typed draft into `RenderEnvelopeV1` without calculating tax,
repairing missing data, or inventing values.

Register the provider and supply at least:

- minimum or mostly blank values;
- normal representative values;
- long valid names, addresses, phone numbers, emails, descriptions, and
  amounts;
- validation and applicability edges;
- maximum official schedule capacity;
- pagination and conditional-page edges.

Regenerate and verify the Rust/TypeScript contract:

```sh
npm run contracts:check
```

Inspect a generated fixture before changing React layout. If a printable value
is wrong or absent there, fix Rust or the provider rather than compensating in
CSS.

## 5. Build semantic HTML and exact-revision CSS

Add a form component and form-scoped stylesheet under
`packages/form-renderer/src/forms/`, then register the exact revision. Reuse
shared paper, comb, checkbox, amount, table, and page primitives where their
geometry truly matches.

The component may:

- select markup for explicitly supplied applicability state;
- lay out labels and Rust-supplied values;
- paginate according to the exact form specification;
- report clipping, overflow, or invalid geometry.

It must not:

- calculate tax or infer a choice;
- silently shorten values;
- load a full-page image or coordinate layout pack at runtime;
- fetch network assets;
- substitute another document implementation when rendering fails.

Add or update the paper and pagination specification in
`packages/form-specs/`. Every normal and edge fixture must produce the official
page count and dimensions without clipping.

## 6. Prepare official references

Prepare calibration-only page rasters directly from the pinned official PDF:

```sh
python3 .codex/skills/ebirforms-convert-form-to-html/scripts/prepare_official_reference.py \
  --repo . --form-code 1601C --revision 2018 \
  --pdf /absolute/path/1601Cv2018.pdf \
  --expected-sha256 <sha256> --source-url <official-url>
```

Commit the reference manifest entry and reviewed page PNGs. Record source and
PNG hashes, page count, DPI, and point geometry. Runtime assets may include
only reviewed discrete crops with provenance and hashes.

## 7. Calibrate in the development viewer

Install the JavaScript workspace and start the viewer from the repository root:

```sh
npm ci
npm run dev:calibration
```

Open `http://127.0.0.1:4175`. The viewer:

- searches committed fixtures; it does not require manual JSON upload;
- renders all pages in one continuous scroll region;
- provides Previous, Next, and page-number jump shortcuts;
- loads a manifest-backed official reference automatically when available;
- switches among HTML, Overlay, and Difference modes;
- changes reference opacity without modifying the printable document;
- labels the renderer route separately from the filing support level.

Compare the full official page, then inspect critical regions such as headers,
identity fields, choices, totals, signatures, schedules, and page breaks. Sparse
line-structure diagnostics are useful for calibration but are not release
evidence. A reference existing in the manifest does not mean the page passes
visual parity.

Run the visual suite:

```sh
npm run test:forms:visual
```

The release gate compares the complete page at the pinned DPI. Do not weaken
the threshold, expand masks, or exclude difficult regions to make a scaffold
appear complete.

## 8. Prove native output and packaged-offline operation

Preview, system print, and direct PDF export must consume the same immutable
envelope and the same semantic document. Before output, verify fonts, stable
geometry, official page count and size, and zero clipping or overflow.

Record successful evidence for macOS, Windows, and Linux. Validate exported
PDF page count, MediaBox/CropBox geometry, rotation, and non-empty page content.
An export failure must leave an existing destination untouched.

Build and verify the offline renderer assets:

```sh
npm run build:forms
npm run verify:forms:offline
```

Production packages contain compiled static assets only. Node and npm are
build-time dependencies, and the renderer must work with networking disabled.

## 9. Audit and promote

Run the conversion verifier first:

```sh
python3 .codex/skills/ebirforms-convert-form-to-html/scripts/verify_form_conversion.py \
  --repo . --form-code 1601C --revision 2018 --stage preview
```

Then run the repository gates:

```sh
npm run contracts:check
npm run audit:forms:migration
npm run typecheck:forms
npm run test:forms
npm run test:forms:visual
npm run build:forms
npm run verify:forms:offline
npm run audit:no-legacy
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Promote independently:

- Queue authority requires the reviewed Rust filing capabilities.
- An `experimental` route permits development calibration only.
- An `html_only` route means the exact revision has no alternate renderer; it
  does not by itself make the form release-ready.
- Set `release_ready` only when every required capability and signed platform
  evidence is green.

If any gate is incomplete, leave the status honest and document the next
blocking fact. Do not route around an incomplete HTML form.
