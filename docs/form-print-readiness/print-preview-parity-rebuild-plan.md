# Print Preview Parity Rebuild Plan

Status: In progress; safety and semantic slices implemented, promotion blocked
Date: 2026-07-15  
First production target: BIR Form 2551Q, January 2018 ENCS  
Reference implementation donor: commit 5b57aff on codex/html-renderer-form-refactor  
Clean integration base: local main at d07d4ce

## Execution checkpoint (2026-07-15)

The donor is archived, the clean parity branch is active, and normal Print
Preview still uses the legacy renderer. The branch now contains a 2551Q-only
Rust render contract, deterministic fixture matrix, semantic two-page HTML
document, Rust-owned continuation subtotal, 12-row continuation pages,
calibration/visual tooling, source-bundle offline and migration audits, and an
explicitly labelled development-only native host with local-only loading and
fail-closed geometry checks.

Rust now owns the return's taxpayer-type and annual-election snapshots, exact
Item 13 applicability/election matrix, exact calendar-quarter import bounds,
cent-precise monetary inputs, finite non-negative penalties, the canonical
22-entry January 2018 ATC registry, and a queue-time SHA-256 binding over every
submission field and calculation input. A queued initial-quarter 8% election
(Q1 for an existing taxpayer, or the business-commencement quarter for a new
registrant) is committed atomically with the draft and recorded in the
taxpayer's annual election ledger; conflicting elections roll the transaction
back. Before network/XML submission, the queued return is rebound to the
current profile, recomputed,
and compared with the reviewed fingerprint. Any changed field, annual
election, rate, credit split, or automatic penalty returns the filing to Draft.
The 8% rule blocks only PT010/Section 116 amounts and preserves independently
taxable ATCs such as PT040.

Immediately before FTP submission, the worker takes an atomic SQLite claim on
the exact reviewed queue generation. Any process crash or transport error after
that boundary is an unknown outcome: the durable claim is never lease-expired,
canceled, overwritten, or automatically retried because BIR may already have
received the return. The desktop removes the cancel action and shows a
support-assisted reconciliation warning. There is intentionally no in-app
claim-release workflow in this slice; reconciliation against authoritative BIR
confirmation or receipt data remains an operational gap.

The native host now uses a nonpersistent WebView, blocks workers, WebRTC, media
and device APIs, emits restrictive response headers, waits for fonts, and
requires a fresh nonce-bound measurement of the explicit print-mode CSS before
system print. Script printing, browser print shortcuts, and context menus are
blocked; unvalidated print media renders no form tree. On Windows, where Wry's
print helper itself calls `window.print()`, a per-preview UUID and nonce grant
one immutable wrapper invocation only after the validated host action. Linux
resource lookup includes the installed `/usr/share/ebirforms` layout. Static offline verification
rejects unauthorized raster payloads by magic bytes, requires complete bundle
reachability, and disallows data images. Migration evidence and public release
workflows now fail closed: positive visual/platform/rollback claims have no
trusted producer until attested CI and packaged drivers exist, dirty-source
visual runs are diagnostic-only, releases retest the exact tagged SHA, and
unsigned macOS/Windows artifacts cannot be published.

Public packaging is intentionally narrower than the set of developer packaging
commands. GitHub releases publish the notarized DMG, Debian package and Linux
tarball, and Authenticode-signed Windows Setup EXE and MSI. They do not build,
sign, or upload MSIX. `just msix` produces a Store-only candidate and is not a
release path. Store promotion remains blocked until correctly sized manifest
artwork and packaged MSVC runtime behavior pass independent Windows
certification. A local `just sign-dev` signature is only for sideload testing
and cannot satisfy that promotion gate. Release preflight also requires tracked
Cargo/npm lockfiles and an exact `vMAJOR.MINOR.PATCH` tag matching the Cargo
workspace version.

Final integrated Chromium comparison at 1224 x 1872 pixels. The relaxed run
passes 5/5 to collect diagnostics. The strict run passes the other four tests
but both pages fail the independent 1% release ceiling:

| Page | Donor baseline | Current branch | Release ceiling |
| --- | ---: | ---: | ---: |
| 2551Q page 1 | approximately 31.29% | 11.942157561030111% (273,634/2,291,328) | 1% |
| 2551Q page 2 | approximately 19.07% | 9.670461845706944% (221,582/2,291,328) | 1% |

The strict gate is red by design. The semantic document still uses a
placeholder government seal and synthetic barcode treatment, and typography
and spacing remain above the visual ceiling. The native host can request the
system print dialog experimentally, but producer-bound packaged macOS/Windows
print evidence does not exist and direct HTML PDF export is not implemented.
An authorized seal, production barcode, licensed deterministic font,
declaration-side taxpayer-type mapping, remaining signatory/payment fields,
and the separate promotion change are also incomplete. Every
completion/promotion flag remains false; `html_enabled` only exposes the
explicit development action.

## 1. Executive Decision

Build the accurate owned HTML renderer on a new, reviewable branch created from
main. Preserve 5b57aff as a donor and archive; do not reset, delete, or continue
stacking production fixes directly on its 137-file commit.

The production application must continue to open the accurate legacy
Typst/SVG-backed preview until a form-specific HTML renderer passes every
release gate. The HTML renderer remains experimental and explicitly invoked
during development.

The implementation order is:

1. Preserve the current dirty checkout and create an adjacent clean worktree.
2. Keep legacy preview as the user-facing default.
3. Transplant only the renderer foundation needed for 2551Q.
4. Complete the Rust-to-renderer contract for every printable 2551Q value.
5. Rebuild 2551Q as an exact form-specific 612 x 936 point HTML document.
6. Add deterministic continuation pages without changing the official two-page
   base form.
7. Pass visual, native PDF, print, offline, and packaged-runtime gates.
8. Promote 2551Q only after reviewed evidence is committed.
9. Repeat the proven workflow for 1601C.

This is not a return to the legacy renderer as the final architecture. It is a
safe migration in which the accurate legacy result remains authoritative until
the replacement is demonstrably better.

## 2. Why This Plan Exists

The donor HTML renderer was structurally incomplete rather than merely
uncalibrated.

Measured against the committed canonical references:

| Form and page | Donor changed pixels | Release threshold |
| --- | ---: | ---: |
| 2551Q page 1 | approximately 31.29% | at most 1% |
| 2551Q page 2 | approximately 19.07% | at most 1% |
| 1601C page 1 | approximately 31.44% | at most 1% |

The donor's 2551Q component used generic stacked rows and a universal 62/38
label/value split. It omitted official fields and legal blocks, used a
synthetic barcode, had no form-specific geometry, and did not reproduce the
official page-two schedule.

That donor branch also mixed renderer work with thousands of lines of unrelated
form models, XML, persistence, carry-over, support-level, and scaffold changes.
Those changes make the branch difficult to review, bisect, roll back, and
release safely.

The valuable work in 5b57aff is the renderer foundation:

- versioned Rust render envelope;
- generated TypeScript contract;
- canonical fixtures;
- Vite/React renderer workspace;
- calibration application;
- Playwright pixel-diff harness;
- native WebView host;
- offline asset verifier;
- packaged-runtime smoke framework;
- per-form migration and evidence manifests.

The plan preserves those ideas while reducing scope to one complete form.

### Current code anchors and integration status

| Concern | Current anchor | Integration status |
| --- | --- | --- |
| Form-specific 2551Q document | packages/form-renderer/src/forms/Form2551Q.tsx | Explicit official Page 1/Page 2 and continuation regions implemented; visual calibration remains |
| Shared renderer primitives | packages/form-renderer/src/components.tsx and print.css | Used only for low-level page, comb, checkbox, and amount behavior; form geometry remains 2551Q-specific |
| Rust render adapter | crates/bir-print/src/html.rs | Owned 2551Q values, validation messages, schedule rows, and continuation subtotal mapped; unowned legal fields remain explicit gaps |
| Native WebView host | crates/bir-desktop/src/views/html_form_preview.rs | Local-only experimental preview and system-print dispatch implemented with fail-closed readiness/fallback; direct PDF and platform proof remain open |
| Runtime selection | crates/bir-desktop/src/views/form_2551q_view.rs | Legacy remains normal traffic; explicit HTML action is development-only while `release_ready` is false |
| Accurate legacy path | crates/bir-print/src/lib.rs and pdf_viewer.rs | Preserve as production default/fallback |
| Dynamic anchors | formtypes/2551Qv2018/formtype.json | Legacy Schedule 1 rows 2-6 reconciled for deterministic calibration; not used as an HTML runtime background |
| Extracted geometry | formtypes/2551Qv2018/form_structure.json | Development-time normalized input only; runtime output is semantic HTML/CSS |
| Golden comparison | packages/form-renderer/visual/form-parity.spec.ts | Per-page diagnostics retained even when the aggregate strict run fails |
| Migration truth | packages/form-specs/form-migration-status.json | Conservative flags audited; all completion/promotion gates remain false |
| Evidence truth | packages/form-specs/form-release-evidence.json | Still empty for strict visual, native platform, and packaged-offline promotion evidence |

## 3. Non-Negotiable Architecture

### 3.1 Ownership

- bir-core owns draft state, calculations, validation, carry-over, persistence,
  XML, submission, and queue eligibility.
- bir-print maps Rust-owned draft values, derived amounts, schedules, and
  validation results into RenderEnvelopeV1.
- React receives a read-only envelope. It owns only document layout,
  pagination, preview rendering, and print styling.
- React must never recalculate tax, infer filing eligibility, or write to the
  database.
- Preview, system print, and direct PDF export must use the same React document
  tree and print stylesheet. The current slice proves preview and experimental
  system-print dispatch from that tree; direct HTML PDF export remains a
  release blocker rather than silently falling back to a second HTML layout.

### 3.2 Runtime artwork

- Official PDF, full-page SVG, and reference PNG files are calibration and
  legacy-fallback inputs only.
- The owned HTML renderer must not load or draw a full official page as its
  runtime background.
- Discrete assets such as an authorized seal, a deterministic barcode, or a
  licensed font may be bundled separately when their provenance and hashes are
  recorded.
- Raw extraction output must not be shipped as a disguised snapshot. Extracted
  geometry must be normalized into semantic regions, rules, labels, cells, and
  field anchors.

### 3.3 Form-by-form migration

- Only 2551Q is in scope until it is release-ready.
- 1601C remains on the legacy path while 2551Q is rebuilt.
- 0605, 0619E, 0619F, 2550Q, 1701Q, 1701, 1702RT, and 1702MX remain outside
  this integration branch.
- A renderer component existing in source does not make a form HTML-enabled,
  fileable, or release-ready.

## 4. Source-of-Truth Order

When sources disagree, use this order:

1. Verified official BIR source URL, revision, and SHA-256 in form metadata.
2. Captured eBIRForms/eFPS behavior and submission/XML evidence.
3. Rust draft, calculations, validation, and XML implementation.
4. RenderEnvelopeV1 and its canonical fixtures.
5. Owned per-form layout specification.
6. React page components.
7. Legacy SVG/Typst output as the visual calibration oracle.

The legacy output is not authoritative for tax semantics. It is authoritative
only for verified page geometry and appearance.

## 5. Target Renderer Shape

The 2551Q renderer must be a form-specific fixed-coordinate document rather
than a composition of universal business-form rows.

Recommended package shape:

~~~text
packages/form-specs/src/forms/2551q-2018/
  paper.ts
  page-1-layout.ts
  page-2-layout.ts
  continuation-layout.ts
  field-anchors.ts
  pagination.ts

packages/form-renderer/src/forms/2551q/
  Form2551Q.tsx
  Page1.tsx
  Page2.tsx
  ContinuationPage.tsx
  components.tsx
  2551q.css
~~~

Shared renderer components should be low-level primitives:

- fixed paper page;
- positioned text;
- border or rule;
- shaded region;
- comb cells;
- checkbox;
- amount cells;
- fixed table;
- identity strip;
- continuation header.

Shared components must not impose one row split, one masthead grid, or one
declaration layout on every form.

## 6. Geometry Strategy

Do not manually rediscover all coordinates.

Existing sources:

- formtypes/2551Qv2018/form_structure.json contains the extracted 612 x 936
  point page geometry, 310 positioned text blocks, and 5,587 rectangles.
- formtypes/2551Qv2018/formtype.json contains calibrated dynamic-field
  positions, cell counts, repeated boxes, decimal splits, and widget sizes.
- formtypes/2551Qv2018/pages/page1.svg and page2.svg remain the development
  visual oracle.
- packages/form-renderer/references/2551q-2018-page-1.png and page-2.png are the
  144 DPI golden references.

Create a maintained development-time normalization tool. Suggested path:

~~~text
scripts/build_owned_form_layout.py
~~~

The tool should:

1. Verify the form revision and official-source hash.
2. Read form_structure.json and formtype.json.
3. Group near-identical rectangle edges into stable horizontal and vertical
   rules.
4. Identify large fills and official grey regions.
5. preserve text content, position, font size, weight, and page.
6. Map calibrated dynamic anchors by BIR field key.
7. Round coordinates only within a documented tolerance, such as 0.1 point.
8. Emit a reviewable semantic intermediate representation under
   .scratch/form-layouts/2551q-2018/.
9. Include source hashes and generator version in the generated header.
10. Group intentional repeated bindings by field key and page into ordered
    fragments with explicit roles such as integer, decimal, continuation, or
    repeat.
11. Reject only undeclared overlaps, conflicting fragment order, or ambiguous
    bindings.

The generated candidate must never overwrite the curated runtime
specification. Review and simplify it into
packages/form-specs/src/forms/2551q-2018/, then validate that committed spec
against a schema and source-hash manifest. It must not emit thousands of glyph
rectangles into the runtime DOM.

Coordinate semantics:

- all stored dimensions use points;
- the page origin is top-left;
- X increases rightward and Y increases downward;
- text extraction baselines must be converted explicitly to CSS positioning;
- field/widget boxes describe the value anchor, not an inferred row boundary;
- coordinate rounding is limited to the documented 0.1 point tolerance;
- generation and validation must be deterministic in tests.

## 7. Git and Worktree Safety

### 7.1 Preserve the current checkout

The current checkout contains staged, unstaged, and untracked user-owned work.
Do not switch branches, stash, reset, clean, or amend it as part of creating the
new integration branch.

Before any Git operation, record:

~~~sh
rtk git status --short --branch
rtk git rev-parse HEAD
rtk git rev-parse main
rtk git merge-base HEAD main
rtk git diff --stat
rtk git diff --cached --stat
rtk git diff
rtk git diff --cached
~~~

Create a durable backup outside the repository. The archive branch preserves
5b57aff, but it does not preserve the current staged patch, unstaged patch, or
untracked files.

~~~sh
rtk proxy mkdir \
  /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-html-renderer-backup-20260715
rtk proxy git diff --cached --binary \
  > /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-html-renderer-backup-20260715/staged.patch
rtk proxy git diff --binary \
  > /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-html-renderer-backup-20260715/unstaged.patch
rtk proxy git ls-files --others --exclude-standard -z \
  > /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-html-renderer-backup-20260715/untracked.zlist
rtk proxy tar -czf \
  /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-html-renderer-backup-20260715/untracked.tar.gz \
  --null -T \
  /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-html-renderer-backup-20260715/untracked.zlist
rtk git bundle create \
  /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-html-renderer-backup-20260715/repository.bundle \
  main codex/html-renderer-form-refactor
rtk git bundle verify \
  /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-html-renderer-backup-20260715/repository.bundle
rtk proxy shasum -a 256 \
  /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-html-renderer-backup-20260715/*
~~~

Do not proceed until the bundle verifies and the backup hashes are retained.
Do not use stash or clean as a substitute.

### 7.2 Archive the donor and create a clean worktree

Run only after confirming both proposed branch names are unused and the
destination path does not exist:

~~~sh
rtk git branch --list \
  archive/html-renderer-5b57aff codex/print-preview-parity
rtk git worktree list --porcelain
rtk proxy ls -ld \
  /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity
rtk git rev-list --left-right --count origin/main...main
rtk git branch archive/html-renderer-5b57aff 5b57aff
rtk git worktree add \
  -b codex/print-preview-parity \
  /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity \
  d07d4ce
rtk git -C \
  /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity \
  rev-parse HEAD
rtk git -C \
  /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity \
  branch --show-current
rtk git -C \
  /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity \
  status --short --branch
~~~

The new worktree is the implementation workspace. The current checkout remains
untouched and available for comparison. If main intentionally advances before
execution, re-review its new tip and replace d07d4ce with a recorded approved
base hash; do not silently inherit a moving branch tip. At plan creation,
local main is 159 commits ahead of origin/main. Do not substitute origin/main,
fetch, or rebase implicitly.

### 7.3 Transplant, do not cherry-pick wholesale

Do not cherry-pick 5b57aff. Restore or reimplement donor files in small,
reviewable groups.

Candidate donor groups:

| Group | Candidate paths | Treatment |
| --- | --- | --- |
| Web workspace | package.json, package-lock.json, tsconfig.base.json | Transplant and reduce to 2551Q |
| Preview tools | apps/form-preview, apps/form-calibration | Transplant |
| Contract | packages/form-contracts | Keep schema and 2551Q fixtures only |
| Form specs | packages/form-specs | Keep paper, 2551Q pagination, and gates only |
| Renderer | packages/form-renderer | Keep shell, low-level primitives, 2551Q, and tests |
| Rust adapter | crates/bir-print/src/html.rs | Rebuild as 2551Q-only initial adapter |
| Native host | crates/bir-desktop/src/views/html_form_preview.rs | Transplant after host audit |
| Verification | renderer audit, offline verifier, packaged smoke | Transplant after evidence hardening |

Explicit initial exclusions:

- unrelated database migrations;
- carry-over migrations and UI work;
- bulk changes for forms other than 2551Q;
- scaffold components and large fixtures for other forms;
- broad support-level promotions;
- release workflow changes before one form passes locally;
- handoff/todo artifacts;
- unrelated OCR documents and scripts;
- the staged arbitrary PDF normalizer;
- the staged visual selector regression;
- the staged weakening of packaged artifact checks.

## 8. Logical Commit Sequence

Each commit should compile and pass the tests relevant to its layer.

Stage only explicit paths for one logical layer:

~~~sh
rtk git add <explicit-paths-for-one-layer>
rtk git diff --check
rtk git diff --cached --check
rtk git diff --cached --name-status
rtk git commit -m "<logical-layer-message>"
~~~

Never use git add . for the transplant.

1. docs: record parity rebuild decision and baseline
2. build: add minimal 2551Q web workspace
3. feat(print): add 2551Q render envelope and canonical fixtures
4. feat(renderer): add calibration shell and fixed paper primitives
5. feat(renderer): reproduce 2551Q page 1
6. feat(renderer): reproduce 2551Q page 2
7. feat(renderer): add deterministic continuation pages
8. feat(desktop): add gated HTML preview with legacy fallback
9. feat(print): add validated native PDF export
10. test(renderer): add visual, native, offline, and package evidence
11. feat(renderer): promote 2551Q after reviewed evidence

Do not combine support promotion with the layout implementation commit.

## 9. Implementation Phases

### Phase 0: Establish an immutable baseline

Tasks:

- Verify main points at the intended base commit.
- Record current legacy and donor HTML screenshots for the same draft.
- Record official source URL, revision, and SHA-256.
- Record current legacy PDF page count, dimensions, file size, render time, and
  preview time.
- Record current donor HTML page-one and page-two changed-pixel percentages.
- Record the current workspace test baseline.
- Preserve generated evidence under test-results; commit only curated evidence
  and hashes required by the promotion manifests.

Commands:

~~~sh
rtk cargo test -p bir-print
rtk cargo test -p bir-core
rtk cargo check -p bir-desktop
rtk du -sh formtypes
~~~

Run the Rust and legacy commands in the clean main worktree. Run the existing
HTML visual suite in the preserved donor checkout, where the Node workspace
exists:

~~~sh
rtk npm run test:forms:visual
~~~

Exit criteria:

- The same 2551Q revision and fixture are used by legacy and HTML comparisons.
- Baseline page geometry is exactly 612 x 936 points and two official pages.
- The source hash matches form metadata.
- Known failures are recorded rather than silently accepted.

### Phase 1: Restore safe production selection

The legacy renderer remains the normal Print Preview action.

Tasks:

- Add a Rust-side renderer support decision for form code and revision.
- Treat html_enabled as development availability only.
- Require release_ready before normal user traffic selects HTML.
- Expose experimental HTML preview only in development mode.
- Retain explicit automatic fallback for missing assets, navigation rejection,
  WebView construction failure, readiness timeout, renderer error, print error,
  and export error.
- While the legacy renderer is the default, refuse or clearly block preview for
  more than six real Schedule 1 rows unless every row is represented. Never
  silently produce a legacy preview that omits rows affecting the total.
- Observe renderer errors that arrive after the initial ready message.
- Add selection tests for release-ready, development-only, unsupported, and
  failed renderer states.

Exit criteria:

- A release build opens legacy preview while release_ready is false.
- A developer can explicitly open the experimental HTML renderer.
- Any HTML failure returns to legacy without losing the draft.
- A draft with more than six rows cannot receive a misleading truncated
  production preview.
- Runtime selection reads the same support truth that the migration audit
  validates.

### Phase 2: Add the minimal 2551Q renderer foundation

Tasks:

- Add the minimal Node/Vite/React workspace.
- Add RenderEnvelopeV1 schema generation from Rust.
- Add only the 2551Q normal and stress fixtures.
- Add the 612 x 936 point paper token.
- Add the development preview and calibration application.
- Add fixed-page, text, rule, cell, checkbox, and amount primitives.
- Add deterministic local fonts and a strict local-only CSP.
- Add the two verified 2551Q golden references and their hashes.

Exit criteria:

- npm installation is reproducible from the lockfile.
- Contract generation is deterministic.
- The preview loads with networking disabled.
- No runtime bundle reference points at official full-page PDF, SVG, or PNG.
- Renderer errors are surfaced through the native readiness bridge.

### Phase 3: Complete the 2551Q print contract

Perform a field-by-field reconciliation before layout work.

At minimum, resolve:

- full MM/YYYY period representation;
- calendar versus fiscal period;
- quarter;
- amended state;
- number of sheets attached;
- grouped TIN and RDO;
- complete taxpayer name and registered address;
- ZIP, contact number, and email;
- tax-relief yes/no and specification;
- Item 13 income-tax-rate election;
- Items 14 through 24 and every printed subtotal;
- overpayment disposition: refund or tax credit certificate;
- signatory and tax-agent metadata where the application captures them;
- payment-detail values and identifiers;
- all six official Schedule 1 rows;
- page-two identity fields and final total.

Correctness rules:

- Rust computes every amount.
- React formats and positions values but performs no tax calculation.
- If a legally printable field is not captured by the application, the
  manifest must not claim contract or XML completeness.
- Overpayment elections must be represented in the Rust model, validation,
  editor, renderer envelope, and XML together.
- Schedule totals and Item 14 must be derived from one source of truth.

Required canonical fixtures:

- blank/minimum draft;
- ordinary calendar-quarter filing;
- fiscal-period filing;
- amended return;
- tax-relief return;
- Item 13 graduated-rate election;
- Item 13 eight-percent election;
- payable return;
- overpayment-to-refund;
- overpayment-to-tax-credit-certificate;
- six Schedule 1 rows;
- ten Schedule 1 rows for continuation behavior;
- representative individual and non-individual declarations.

Commands:

~~~sh
rtk npm run contracts:generate
rtk npm run contracts:check
rtk cargo test -p bir-print
rtk cargo test -p bir-core
rtk npm run typecheck:forms
rtk npm run test:forms
~~~

Exit criteria:

- A reviewed field-coverage checklist has no unexplained omissions.
- Schema, generated TypeScript, and fixtures are clean after regeneration.
- Rust/XML tests cover every election and printed computed amount.
- React consumes only envelope values.

### Phase 4: Normalize the owned layout specification

Tasks:

- Implement the development-time geometry normalizer.
- Generate page-one and page-two semantic layout candidates.
- Review and group extracted geometry into named official sections.
- Replace raw extraction fragments with semantic labels and rules.
- Record the fixed coordinate system and rounding policy.
- Add source-hash drift checks.
- Add a deterministic font asset or an explicitly tested font stack.
- Determine the authorized source for the government seal.
- Implement the required deterministic barcode rather than a decorative stripe.

Exit criteria:

- The owned layout can be reviewed without opening the official SVG.
- Every region has a stable semantic name.
- Every dynamic field anchor maps to exactly one envelope value or documented
  repeated segment.
- The runtime bundle contains no full-page reference artwork.
- Generated layout output changes only when its source or generator changes.

### Phase 5: Rebuild 2551Q page 1

Implement explicit official regions:

1. BIR-use and BCS boxes.
2. Government seal and department header.
3. Form number, revision, page number, title, instructions, and barcode.
4. Items 1 through 5 filing-period grid.
5. Part I title and Items 6 through 13.
6. Part II title and Items 14 through 24.
7. Overpayment disposition row.
8. Full declaration text.
9. Individual and non-individual signature blocks.
10. Tax-agent accreditation, date of issue, and expiry blocks.
11. Full Part III payment table.
12. Machine-validation and receiving-office blocks.
13. Data-privacy note.

Implementation rules:

- Do not use the generic FormRow layout.
- Use point units for fixed page geometry.
- Do not stretch the page or independently scale X and Y.
- Use exact official grey fills, border weights, and row heights.
- Prevent every text block from overflowing its assigned rectangle.
- Keep value text selectable.

Calibration sequence:

1. Match page bounds and major rules.
2. Match section fills and column tracks.
3. Match static labels and typography.
4. Match dynamic comb cells and values.
5. Match seal, barcode, and legal footer.
6. Iterate with overlay and difference views.

Intermediate thresholds may guide work, but only the release threshold counts:

| Milestone | Suggested diagnostic ceiling |
| --- | ---: |
| Major geometry | 10% |
| Static labels and cells | 3% |
| Release candidate | 1% |

Exit criteria:

- Page 1 is 1224 x 1872 pixels at the pinned 144 DPI capture.
- Changed pixels are at most 1% at pixelmatch threshold 0.1.
- No clipping, scroll overflow, overlapping labels, or missing official item.
- The visual evidence report records the page even if another page fails.

### Phase 6: Rebuild 2551Q page 2

Implement:

- exact masthead and barcode;
- TIN and taxpayer-name identity row;
- official six-row Schedule 1;
- exact taxable amount, rate, and tax-due cells;
- Item 7 final total and carry destination;
- complete categorized ATC reference table;
- all official page-two notes and footer elements.

Do not repeat page-one declaration content unless the official page contains
it.

Exit criteria:

- Six real rows occupy the official six-row grid.
- Page-two total equals Rust Item 14 input.
- Page 2 is at most 1% changed pixels.
- Page-one failure does not prevent collection of page-two metrics.

### Phase 7: Add continuation pages

The official base remains two pages. Extra data uses an owned continuation
template rather than stretching the official page.

Rules:

- zero through six real rows produce the two official pages;
- seven through eighteen rows produce three pages;
- nineteen through thirty rows produce four pages;
- every additional block of twelve rows adds one continuation page;
- every real row is rendered exactly once using a stable key;
- continuation pages repeat form identity and table headers;
- the official page-two grid always has six visual slots and each continuation
  page has twelve visual slots;
- unused visual slots are explicit blank placeholders and are never counted as
  real rows;
- totals appear only on the final schedule page;
- when continuation exists, page two replaces the official final Item 7 value
  with a clearly identified subtotal carried to continuation;
- the final continuation page contains the final Item 7 total and its
  destination to Part II Item 14;
- Rust continues blocking submission above the verified six-row XML boundary
  until the attachment/submission protocol is proven;
- preview never truncates data to mimic a valid submission.

Boundary tests:

- 0, 1, 5, and 6 real rows;
- 7 rows;
- 18 rows;
- 19 rows;
- long taxpayer identity fields;
- maximum supported amount widths.

Because BIR provides no official continuation-page reference for this flow,
commit a separately reviewed functional golden for the owned continuation
template. Do not represent that golden as official visual-parity evidence.

Exit criteria:

- Expected page counts are deterministic at every boundary.
- No row is missing, duplicated, or reordered.
- No page has scroll overflow.
- Print and PDF pagination match preview pagination exactly.

### Phase 8: Harden the native host and PDF export

Tasks:

- Load only bundled local assets.
- Reject navigation outside the renderer root.
- Disable network APIs and external font/script/image access.
- Do not mark the document ready until every page has stable geometry and no
  overflow.
- Continue observing errors after ready.
- Derive expected page dimensions and count from the trusted host-side form
  specification, not from renderer self-report alone.
- Validate renderer-reported page rectangles against host expectations.
- Configure WebView2 PrintToPdf explicitly for 8.5 x 13 inch paper, zero
  margins where supported, and printed backgrounds.
- Configure WKWebView capture per expected page rectangle.
- Permit only uniform PDF normalization for a documented CSS-pixel-to-point
  conversion and reject wrong aspect ratios.
- Validate PDF signature, page count, MediaBox/CropBox, selectable text, and
  known text markers before replacing the destination.
- Keep temporary output beside the destination and preserve any existing file
  until validation succeeds.

Exit criteria:

- macOS and Windows exports are exactly 612 x 936 points per official page.
- Background fills and borders are present.
- Every page contains selectable text.
- Wrong-size, wrong-aspect, blank, raster-only, or partial PDFs are rejected.
- A late renderer error disables print/export and falls back safely.

### Phase 9: Harden evidence and CI

Visual evidence must include:

- form code and revision;
- source commit;
- fixture path and hash;
- reference path and hash;
- page number;
- expected and actual dimensions;
- changed pixels and percentage;
- threshold;
- per-page pass/fail;
- aggregate pass/fail.

Native/package evidence must include:

- form code and revision;
- source commit;
- platform and architecture;
- artifact kind;
- artifact path and hash;
- packaged renderer asset hashes;
- network-disabled runtime result;
- readiness result;
- renderer page geometry;
- distinct native_print and native_pdf_export results;
- PDF validation result;
- explicit error when PDF export was not exercised.

Rules:

- A development binary may produce diagnostic evidence but cannot satisfy the
  packaged-app gate.
- PDF validation is false or not-exercised when no PDF was requested; it must
  never default to true.
- native_print.passed and native_pdf_export.passed are separate required
  promotion facts on both macOS and Windows.
- If native print cannot be automated, a signed manual print record with
  artifact hash, operator, platform, printer or PDF driver, paper measurement,
  date, and reviewed output hash is mandatory. Native PDF export evidence
  cannot stand in for print evidence.
- Audit scripts validate evidence schema and hashes, not just a passed boolean.
- PowerShell release steps must stop immediately on every failed native
  command.
- Worker failure must not erase already measured visual pages.
- CI uploads evidence even on failure.

Commands:

~~~sh
rtk python3 scripts/audit_html_form_migration.py
rtk npm run contracts:check
rtk npm run typecheck:forms
rtk npm run test:forms
rtk npm run test:forms:visual
rtk npm run build:forms
rtk python3 scripts/verify_offline_form_renderer.py \
  --evidence-out test-results/offline/source-bundle.json
rtk cargo check -p bir-desktop
rtk cargo test -p bir-print
rtk cargo test -p bir-core
rtk cargo test --workspace
rtk cargo clippy --workspace -- -D warnings
rtk cargo fmt --all -- --check
~~~

Packaged macOS gate:

~~~sh
rtk env DEVELOPER_MODE=false DEV_MODE=false just _package-mac
rtk python3 scripts/smoke_test_packaged_renderer.py \
  target/release-artifacts/eBIRForms.app \
  --evidence-out test-results/offline/macos-packaged-runtime.json
~~~

Windows must run the equivalent packaged application and native WebView2 PDF
export gate in the Windows release job. Add a maintained driver such as
scripts/smoke_test_packaged_renderer_windows.ps1 that:

1. installs or stages the actual packaged/MSIX artifact;
2. records the artifact kind and SHA-256;
3. applies a temporary outbound firewall rule to the packaged executable;
4. launches renderer smoke for 2551Q;
5. exercises WebView2 PDF export;
6. validates page geometry and selectable text;
7. records firewall enforcement and renderer asset hashes;
8. removes the temporary firewall rule in a finally block;
9. exits nonzero for any missing or failed sub-gate.

Planned CI invocation:

~~~powershell
rtk proxy pwsh -File scripts/smoke_test_packaged_renderer_windows.ps1 -ArtifactPath target\release-artifacts\eBIRForms.msix -EvidenceOut test-results\offline\windows-packaged-runtime.json
~~~

A staged directory or development executable may be useful diagnostically but
cannot satisfy this packaged Windows command.

### Phase 10: Promote 2551Q

Promotion is a separate reviewed change.

Before promotion:

- layout_calibrated reflects actual page-one and page-two completion;
- visual_parity_complete has committed passing evidence;
- native_print_export_verified has passing macOS and Windows evidence;
- packaged_offline_verified has passing packaged evidence on both platforms;
- release_ready is still false.
- a rollback drill has passed and its evidence is committed.

Promotion change:

1. Commit reviewed evidence references.
2. Update the migration manifest.
3. Run the migration audit.
4. Enable normal HTML routing only when release_ready is true.
5. Keep the legacy fallback callable.
6. Re-run the complete release matrix from a clean checkout.

Exit criteria:

- A release build chooses HTML for 2551Q only.
- Any runtime failure still opens the accurate legacy renderer.
- 1601C and every scaffold form remain unchanged.
- test-results or curated release evidence contains a reviewed
  rollback-drill.json for the exact release candidate.

### Phase 11: Migrate 1601C

Begin only after 2551Q is promoted.

Reuse the process, not the 2551Q page structure:

- reconcile every printable field;
- normalize form-specific geometry;
- build explicit page components;
- prove dynamic adjustment pagination;
- verify no page-one clipping;
- pass visual parity per page;
- verify macOS and Windows PDF output;
- commit evidence and promote separately.

Do not generalize a shared component until at least two calibrated forms prove
the abstraction has identical semantics.

## 10. Verification Matrix

| Gate | Command or evidence | Required result |
| --- | --- | --- |
| Source provenance | migration audit and metadata hashes | Exact official revision/hash |
| Rust correctness | cargo test -p bir-core | All tests pass |
| Envelope adapter | cargo test -p bir-print | All printed values covered |
| Contract drift | npm run contracts:check | No generated diff |
| Renderer types | npm run typecheck:forms | No errors |
| Renderer unit tests | npm run test:forms | All tests pass |
| Page 1 visual | visual parity suite | At most 1% changed pixels |
| Page 2 visual | visual parity suite | At most 1% changed pixels |
| Continuation | boundary fixtures | No loss, duplication, or overflow |
| Offline source bundle | offline verifier | CSP/local/hash checks pass |
| Desktop compile | cargo check -p bir-desktop | No errors |
| macOS PDF | packaged native evidence | Correct PDF and geometry |
| Windows PDF | packaged native evidence | Correct PDF and geometry |
| Packaged offline | packaged smoke | App bundle, network denied |
| Workspace regression | cargo test --workspace | All tests pass |
| Lints | cargo clippy --workspace -- -D warnings | No warnings |
| Formatting | cargo fmt --all -- --check | Clean |
| Manual print | physical or trusted PDF inspection | Correct scale, margins, fills |

## 11. Definition of Done for 2551Q

### Contract and semantics

- Every official printable field is represented or explicitly documented as
  unavailable.
- Calculations and submission validation remain Rust-only.
- XML, editor, envelope, and print choices agree.
- Overpayment and tax-rate elections are complete.

### Visual document

- Page 1 and page 2 each pass the 1% golden threshold.
- Exact 612 x 936 point paper and two-page base count.
- Official labels, item numbers, instructions, declarations, payment regions,
  seal, barcode, and notes are present.
- No full-page runtime reference artwork.
- Text remains selectable.

### Dynamic behavior

- Six-row official schedule is exact.
- Continuation boundaries are deterministic.
- Every row is preserved.
- Totals appear only where intended.
- Unsupported submission row counts remain blocked without truncating preview.

### Native behavior

- Same React tree drives preview, print, and PDF.
- macOS and Windows exports pass page and text validation.
- Network access is unnecessary and denied in packaged smoke.
- Failure returns to legacy without data loss.

### Release behavior

- HTML is not the default before release_ready.
- Evidence paths and hashes are committed and audited.
- Scaffold forms remain disabled.
- Legacy fallback remains packaged.
- Workspace tests, lint, and formatting are green.

## 12. Rollback Plan

Rollback must never require a data migration.

- Renderer selection is controlled by the release-ready support gate.
- Setting 2551Q release_ready to false returns normal traffic to legacy.
- Keep render_2551q_print, PdfViewerView, Typst packaging, formtype.json, and
  legacy page assets until at least 2551Q and 1601C are independently proven in
  released packages.
- Do not delete legacy assets in the same release that first enables HTML.
- Native export writes validated temporary output before replacing a user
  destination.
- Each logical commit can be reverted without reverting unrelated tax logic.
- Roll back a promoted renderer with an additive revert of the standalone
  promotion commit, or with a new commit that disables the gate. Do not reset
  or force-push shared history.

~~~sh
rtk git revert --no-edit <promotion-commit-sha>
~~~

- Retain the donor branch, legacy assets, and clean worktree through at least
  one independently verified packaged release.

Before the first promotion, automate a rollback drill that proves:

- release routing selects legacy when release_ready is false;
- an explicit local kill switch selects legacy without changing form data;
- missing assets select legacy;
- renderer error, late renderer error, and readiness timeout select legacy;
- invalid renderer geometry and rejected PDF select legacy;
- an existing export destination remains byte-identical after failed export;
- no temporary PDF or renderer directory leaks after failure;
- the in-memory and persisted draft remain unchanged.

The drill writes rollback-drill.json containing the release-candidate commit,
fixture hash, cases executed, before/after destination hashes, temporary-file
audit, draft-state hash, and aggregate result. The migration audit must reject
first promotion when this evidence is absent or failed.

## 13. Risk Register

| Risk | Consequence | Mitigation |
| --- | --- | --- |
| Generic components distort official grids | Persistent visual mismatch | Form-specific fixed layout; low-level primitives only |
| Extracted geometry is noisy | Snapshot-like or unreviewable runtime DOM | Normalize and semantically review generated output |
| Cross-platform font metrics differ | macOS/Windows page drift | Bundle a deterministic licensed font and test both |
| Native WebViews print differently | PDF size/background mismatch | Explicit settings and platform evidence |
| Renderer validates its own wrong geometry | False-positive PDF gate | Host-owned canonical dimensions |
| Arbitrary PDF scaling hides errors | Distorted official output | Uniform known-ratio conversion only |
| Visual worker failure loses evidence | Misleading zero-page report | Persist metrics per page before assertions |
| Development binary satisfies package gate | False release confidence | Require packaged artifact kind and hashes |
| Extra schedule rows exceed XML protocol | Preview/submission disagreement | Render all rows; block unsupported submission |
| 5b57aff scope leaks into rebuild | Large unreviewable integration | Selective transplant and 2551Q-only commits |
| Legacy removed too early | No accurate fallback | Keep legacy through multiple proven releases |

## 14. Review Checkpoints

Stop for review at these points:

1. Clean worktree and donor inventory approved.
2. 2551Q field-coverage matrix approved.
3. Owned layout specification approved before full rendering.
4. Page 1 under 1%.
5. Page 2 under 1%.
6. Continuation boundaries green.
7. macOS native PDF evidence approved.
8. Windows native PDF evidence approved.
9. Packaged-offline evidence approved.
10. Separate release-ready promotion approved.

No later checkpoint may waive an earlier failed gate.

## 15. First Execution Slice

The first implementation slice should end with a safe application and a
reviewable page-one skeleton, not a promoted renderer.

Deliverables:

- archive branch and clean worktree;
- this plan committed;
- minimal 2551Q-only web workspace;
- legacy default plus explicit experimental HTML action;
- complete 2551Q field-coverage matrix;
- deterministic source/reference hashes;
- normalized page-one major geometry;
- visual evidence showing improvement from the approximately 31.29% baseline;
- no changes to release_ready.

This slice is successful even if visual parity is not yet complete, provided it
does not weaken production behavior or any release gate.
