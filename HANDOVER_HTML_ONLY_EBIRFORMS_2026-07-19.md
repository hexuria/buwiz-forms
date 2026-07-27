# HTML-Only eBIRForms Migration Handover

Date: 2026-07-19 (Asia/Manila)

## Executive truth

This branch contains substantial implementation work, but it is **not production-ready** and the approved migration goal is **not complete**.

- All ten target revisions have Rust render providers, committed fixtures, semantic React/CSS components, paper specifications, pagination, and pinned official references.
- Typst, runtime `formtypes`, the legacy preview route, full-page runtime form backgrounds, and packaged Node have been removed from production paths. The no-legacy audit passes.
- Native HTML preview/print/PDF code exists for macOS, Windows, Linux X11, and Linux Wayland.
- Profile/COR/Forms Set truth and the profile-manager UI received extensive implementation and hardening.
- Only `2551Q:2018` is routed `html_only`; the other nine renderers remain `experimental`.
- Every target is still `ScaffoldOnly`, every target has `release_ready: false`, and every release-evidence slot is null.
- The complete-page visual comparisons are still far above the strict 1% gate. Small structure-only percentages must not be reported as full parity.
- Only `2551Q:2018` and `1601C:2018` currently have queue/submission authority. The other eight forms cannot be presented as fileable.
- No reviewed signed package/native-output evidence exists for macOS, Windows, or Linux.

The immediate problem is no longer “build more renderer infrastructure.” The immediate problem is to prove and finish one production form—`2551Q:2018`—through honest visual, native, offline-package, and rollback evidence before spending more time polishing all nine experimental forms.

## Authoritative workspace

```text
Worktree: /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity
Branch:   codex/print-preview-parity
HEAD:     bebd59a (Correct form readiness evidence metadata)
Baseline: 267d34a
Delta:    273 commits; 529 files changed; 160,077 insertions; 735,454 deletions
```

Do not continue this work in `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir`; that checkout is still `codex/html-renderer-form-refactor` at `5b57aff`.

### Current dirty state

There is no tracked diff, but these untracked diagnostics exist:

```text
packages/form-renderer/playwright.1601c-hotspots.config.ts
test-results/                       approximately 17 MB
tmp/                                approximately 206 MB
HANDOVER_HTML_ONLY_EBIRFORMS_2026-07-19.md
```

Do not delete or commit the first three paths without inspecting them. They are diagnostic artifacts and currently make `--require-clean-source` fail. This handover file is intentional.

There are also many auxiliary Git worktrees, including final-parity, page-specific, candidate-certification, and queue-certification worktrees. Audit each for dirty files and unique commits before removal. Do not run a broad reset, clean, or worktree-prune operation.

## What was accomplished

### 1. HTML renderer architecture and ten exact revisions

The React registry contains exactly these ten revisions:

| Exact revision | Rust/provider/fixtures | HTML component/spec/pagination | Route | Queue authority |
| --- | --- | --- | --- | --- |
| `2551Q:2018` | Present | Present | `html_only` | Proven |
| `1601C:2018` | Present | Present | `experimental` | Proven |
| `0619E:2018` | Present | Present | `experimental` | Blocked |
| `0619F:2018` | Present | Present | `experimental` | Blocked |
| `0605:1999` | Present | Present | `experimental` | Blocked |
| `1701Q:2018` | Present | Present | `experimental` | Blocked |
| `2550Q:2024` | Present | Present | `experimental` | Blocked |
| `1701:2018` | Present | Present | `experimental` | Blocked |
| `1702RT:2018C` | Present | Present | `experimental` | Blocked |
| `1702MX:2018C` | Present | Present | `experimental` | Blocked |

Primary anchors:

- `packages/form-renderer/src/forms/registry.ts`
- `packages/form-specs/form-migration-status.json`
- `crates/bir-print/src/html_forms/`
- `packages/form-contracts/fixtures/`
- `packages/form-renderer/src/forms/`
- `packages/form-renderer/references/manifest.json`

The reference manifest has all 24 official pages for the ten revisions. The official rasters are calibration-only and runtime-ineligible. They are not page backgrounds.

### 2. Honest readiness and visual diagnostics

The migration manifest was corrected to remain conservative. All ten forms are `ScaffoldOnly`; none is `release_ready`.

Latest documented complete-page differences at 144 DPI:

| Exact revision | Complete-page difference by page | Structure-only diagnostic by page |
| --- | --- | --- |
| `2551Q:2018` | 6.652998%, 4.650054% | 0.071225%, 0.009296% |
| `0605:1999` | 7.439441%, 10.226733% | 0.257798%, 0.200626% |
| `0619E:2018` | 8.591945% | 0.175107% |
| `0619F:2018` | 8.377329% | 0.331749% |
| `1601C:2018` | 11.866917%, 12.098137% | 0.389076%, 0.202808% |
| `1701Q:2018` | 13.479781%, 9.589548% | 0.328150%, 0.095185% |
| `1701:2018` | 12.599593%, 15.788224%, 12.253942%, 15.302349% | 0.416833%, 0.446597%, 0.243440%, 0.551471% |
| `1702RT:2018C` | 12.146711%, 8.016661%, 7.996585%, 11.390949% | 0.520877%, 0.148516%, 0.169116%, 0.875999% |
| `1702MX:2018C` | 13.217793%, 19.179227%, 11.552995%, 9.224345% | 0.385322%, 0.819307%, 0.131147%, 0.259195% |
| `2550Q:2024` | 8.303993%, 9.847932% | 0.407240%, 0.344548% |

These values explain the visible mismatch reported by the user. The sub-1% structure-only figures are narrow line/geometry diagnostics; they are not full-image parity and must never be presented as such.

See `docs/form-print-readiness/priority-forms-readiness.md`.

### 3. Character guides, long values, artwork, and calibration rules

The renderer and both project skills now encode the required rules:

- Character guides are measured per field and exact form revision.
- A plain field in the official PDF stays plain; shared primitives must not invent comb guides.
- Empty, short, and exact-capacity values keep the complete official guide layout.
- A valid over-capacity value switches the whole field footprint to a plain box; it must not truncate or mix comb/plain rendering.
- Font fitting starts at the reviewed field maximum and reduces in 0.5 px steps to a reviewed floor; it must fail closed below that floor.
- Official barcode/PDF417/QR and seal artwork must come from exact pinned-PDF embedded objects with source hashes, decoded payload/matrix proof, native geometry, vector symbol output, and live bundled-font captions.
- Official page rasters remain test references only.

The relevant tracked skills are:

- `.codex/skills/ebirforms-convert-form-to-html/`
- `.codex/skills/ebirforms-print-preview/`

They include routing tests, source inventory, reference preparation, conversion verification, and fail-closed guidance.

### 4. Profile, COR, and Forms Set truth

Implemented source-level foundations include:

- `ResolvedTaxProfileForYear` with effective segments and issues.
- Confirmed/NeedsReview profile-version handling and effective-date resolution.
- Token-aware `NON-VAT` and `NOT VAT REGISTERED` handling with tests.
- Central obligation suggestions and `reconcile_forms_set_for_year`.
- Manual include/exclude precedence, conflict/NeedsReview state, and source/evidence tracking.
- `ProfileComplianceChanged` events and editable-vs-immutable draft reconciliation.
- Per-year annual income-tax-election eligibility and Item 13 propagation work.
- Registration-fee/0605 suggestion limited to the COR registration year, with tests.
- Registry-backed searchable form multiselect plus explicit custom-code handling. The static registry contains 51 form definitions.
- COR starting presets for current profile, non-VAT business, and VAT business.
- `Forms & Elections` integrated into the COR workflow rather than a separate top-level Forms Set tab.
- Shared `DateInput` for COR registration/effective dates.
- Dirty-state separation for profile-only vs Forms Set changes, no-op input suppression, discard handling, async save/session guards, and save-path integrity.
- Theme-aware warning/status colors.
- Google Calendar tab hidden unless OAuth is both built/configured and connected; local documentation replaces the inaccessible private link.
- Per-profile calendar form selection and presets.

Important caveat: this handover did not rerun the GUI against the user's local profile database. The reported TIN-specific scenario `274-476-433-00000` and every manual interaction path must be regression-tested in the running app before claiming these UI issues fixed.

Confirmed/archived COR facts remain immutable by design. The UI instructs the user to create a replacement version and confirm it through review. If the product requirement is direct mutation of a confirmed COR, that request is not implemented; the current implementation deliberately preserves an audit trail.

Primary anchors:

- `crates/bir-core/src/profile.rs`
- `crates/bir-core/src/forms/forms_set.rs`
- `crates/bir-core/src/integration/validation.rs`
- `crates/bir-desktop/src/cor_ocr.rs`
- `crates/bir-desktop/src/views/profile_manager/`
- Commits `ec7e399`, `1a53b00`, `30251c1`

### 5. Native HTML print and PDF implementation

The backend code exists:

- macOS: `NSPrintOperation` for system print; per-page `WKWebView.createPDF`; normalization, merge, validation, evidence stamping, and atomic replacement.
- Windows: WebView2 `Print` and `PrintToPdf` with explicit paper size, zero margins, backgrounds enabled, and headers/footers disabled.
- Linux: X11 Wry child plus Wayland GTK/WebKit top-level host, system print, PDF export, page setup, and validation.
- PDF utility: sibling temporary output; page count; MediaBox/CropBox; zero rotation; nonempty streams; form/revision/envelope/render-graph evidence; atomic destination replacement.

Primary anchors:

- `crates/bir-desktop/src/views/html_form_preview.rs`
- `crates/bir-desktop/src/views/linux_html_preview.rs`
- `crates/bir-desktop/src/views/linux_html_preview/runtime.rs`
- `crates/bir-print/src/html_output.rs`
- `crates/bir-print/src/html_output_evidence.rs`
- `crates/bir-print/src/pdf_util.rs`

The implementation is not equivalent to platform certification. The evidence slots are still null and the candidate collectors are intentionally non-promotional until an external reviewed run occurs.

### 6. Typst/formtypes/legacy retirement

The source-level retirement is implemented:

- No tracked runtime `formtypes/` tree.
- No tracked `.typ` form templates.
- No Typst dependency in Cargo/npm production manifests.
- Legacy PDF viewer/calibration routes removed.
- Full-page form SVG/raster backgrounds excluded from runtime.
- Package audits reject Typst, formtypes, legacy routes, full-page backgrounds, and Node runtime payloads.
- npm remains build-time only.

`npm run audit:no-legacy` passes. This proves legacy absence, **not release readiness**.

The current diagnostic macOS package is about 36.38 MiB of regular files versus the previous approximately 106 MB uncompressed payload, but that is not signed release evidence.

See:

- `scripts/audit_no_legacy.py`
- `docs/form-print-readiness/html-only-retirement.md`
- `.github/workflows/release.yml`
- `justfile`

## Current verification result

Completed during this handover audit:

```text
python3 scripts/audit_html_form_migration.py  PASS
python3 scripts/audit_no_legacy.py            PASS
git diff --check                             PASS
```

An audit subtask also ran the two focused audit/security unittest modules: 10 tests passed.

Not completed in this handover:

- Full npm contract/typecheck/renderer/visual suite.
- Full Rust workspace tests/clippy.
- `cargo test --locked -p bir-print` was started by an audit subtask and stopped when the handover was requested. Do not report it as passed.
- Any live GUI regression run.
- Any signed native/package evidence run.

## What remains to finish the approved goal

### Blocker 1: 2551Q visual parity

`2551Q:2018` is the only production `html_only` route, but its complete-page differences are approximately 6.65% and 4.65%, not <=1%.

Do not chase only a masked/structural number. Diagnose the actual full page by region: official static copy, font metrics, line thickness, page margins, gray fills, checkbox/comb geometry, header artwork, signature/payment rows, and page-two schedule content. Preserve semantic HTML and never embed the official page.

### Blocker 2: trusted release evidence pipeline

`packages/form-specs/form-release-evidence.json` contains null for every visual, native, and offline entry for all ten revisions. The audit's trusted visual/native/offline producer sets are currently empty, so untracked screenshots cannot promote a form.

Evidence must bind:

- clean source revision;
- exact renderer bundle/tree hash;
- exact form/revision/envelope hash;
- page count and paper geometry;
- reviewed full-page/critical-region visual evidence;
- real system print and PDF export;
- packaged-offline operation;
- rollback drill;
- signed/timestamped platform artifact identity.

### Blocker 3: cross-platform candidate certification

- macOS: no reviewed signed/notarized non-dev candidate run and rollback bundle.
- Windows: current candidate workflow yields an unsigned portable ZIP; no real signed/timestamped WebView2/printer run.
- Linux: no installed final DEB/tarball evidence under both X11 and Wayland.

The native code exists. Exercise and certify it; do not add another output backend unless the existing one fails under evidence.

### Blocker 4: queue/fileability for eight forms

Only 2551Q and 1601C have queue/submission capability. Before promoting the other eight, prove exact typed model, formulas, validation, XML round trip, persistence, queue claim/idempotency, transport identifier, retries, immutable snapshot, and submission outcome.

### Blocker 5: nine experimental renderers

After 2551Q is genuinely releasable, complete each remaining exact revision one at a time. A renderer being visible in the calibration app does not mean it is fileable or release-ready.

Recommended order remains:

1. `1601C:2018`
2. `0619E:2018`
3. `0619F:2018`
4. `0605:1999`
5. `1701Q:2018`
6. `2550Q:2024`
7. `1701:2018`
8. `1702RT:2018C`
9. `1702MX:2018C`

### Blocker 6: profile/COR GUI regression and local data

Run the user's actual profile `274-476-433-00000` through:

- COR replacement/edit workflow;
- date controls;
- registration-fee-only and non-VAT suggestions;
- 51-form searchable selection;
- presets;
- annual election/Item 13 refresh;
- dirty/no-op/discard/save flows;
- dark mode;
- Google Calendar availability gating;
- Forms & Elections placement;
- save/reopen and current-year reconciliation.

If existing local DB rows still show stale Forms Sets, inspect migrations and stored evidence rather than papering over the UI.

## Recommended continuation sequence

1. **Preserve the branch and make source identity clean.** Inspect the untracked Playwright config and diagnostics. Commit only intentional source; archive or remove generated diagnostics only after review.
2. **Run the focused 2551Q visual suite on the clean revision.** Produce a region-ranked diff report based on complete-page changed pixels, not only line masks.
3. **Fix 2551Q page 1 and page 2 to the real <=1% gate** while retaining clipping, long-value, adaptive-guide, exact copy, page-count, and critical-region tests.
4. **Build one exact non-dev macOS candidate** and run the external collector, actual preview, actual system print, direct PDF export, offline test, and rollback drill. Review the evidence before promotion.
5. **Repeat the same candidate protocol on signed Windows and Linux X11/Wayland artifacts.**
6. **Only then mark 2551Q evidence/capabilities/release-ready.** Require the audit to pass with `--require-release-ready 2551Q:2018`.
7. **Regression-test profile/COR against the user's real data.** Fix only reproduced gaps.
8. **Take 1601C through the same process**, then certify queue/fileability and parity for the remaining forms in order.

## Exact commands for the next agent

Start with read-only truth:

```sh
cd /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity
rtk git status --short --branch
rtk git log --oneline --decorate -20
rtk npm run audit:forms:migration
rtk npm run audit:no-legacy
```

After deliberately resolving the untracked source state:

```sh
rtk npm run audit:forms:migration -- --require-clean-source
rtk npm run contracts:check
rtk npm run typecheck:forms
rtk npm run test:forms
rtk env FORM_VISUAL_MAX_CHANGED_PERCENT=1 npm run test:forms:visual
rtk npm run build:forms
rtk npm run verify:forms:offline:package
rtk cargo check --locked -p bir-desktop
rtk cargo test --locked -p bir-print --lib
```

Do not run the release-ready command until evidence exists:

```sh
rtk npm run audit:forms:migration -- --require-release-ready 2551Q:2018
```

Expected current result: it fails because visual, native, offline, rollback, support-level, and release evidence are absent.

## Source material

The user's official PDFs, XML schemas/examples, and related source files are in:

```text
/Users/uriah/Downloads/forms
```

Every renderer must bind to the exact revision and source hash. Do not use a generic BIR logo, infer a machine-readable payload from its caption, or generate geometry from a different revision.

## Warnings for Claude/Fable or any successor

- Do not claim this branch is complete or production-ready.
- Do not equate `html_only` with `release_ready`.
- Do not report structural masked differences as whole-page visual parity.
- Do not promote untracked `test-results/` or `tmp/` files as trusted evidence.
- Do not turn capability flags true to make the audit green.
- Do not reintroduce Typst, formtypes, full-page runtime backgrounds, a legacy viewer, or a Node runtime.
- Do not directly mutate confirmed COR facts without an explicit product decision to abandon the audit trail.
- Do not use broad `git clean`, `git reset`, or worktree deletion; there are many auxiliary worktrees and possible unique commits.
- Do not spawn subagents with full conversation history. This task contained many screenshots and previously caused hundreds of duplicated multi-gigabyte session rollouts. Use `fork_turns: none` or a compact prompt with exact file paths.
- Keep work in `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity` unless the user explicitly changes the target.

## Copy-paste prompt for the successor model

```text
Continue the approved HTML-only eBIRForms migration in:
/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity

Branch: codex/print-preview-parity
Expected starting HEAD: bebd59a

Read HANDOVER_HTML_ONLY_EBIRFORMS_2026-07-19.md first. Treat the current Git tree, migration manifest, release-evidence manifest, and official source pack as authoritative. Do not claim completion from historical commentary.

First inspect the untracked source/diagnostic paths without deleting them. Then make the source revision clean and close the true 2551Q full-page visual gate (currently about 6.65% and 4.65%, required <=1%) while preserving semantic HTML, exact two-page geometry, long-value/no-clipping rules, field-specific character guides, and exact official artwork provenance.

After visual parity, exercise the already-implemented native output path with one exact signed non-dev macOS candidate: preview, real system print, direct PDF export, packaged offline operation, and rollback. Bind the evidence to source revision, renderer hash, envelope hash, page geometry, and artifact identity. Repeat with signed Windows and Linux X11/Wayland candidates. Only then promote 2551Q using the migration audit.

Separately regression-test the profile/COR changes against local profile 274-476-433-00000. Preserve confirmed COR immutability unless the user explicitly chooses direct mutation over replacement-version audit history.

Do not add another renderer backend, reintroduce Typst/formtypes, or spend time polishing all nine experimental forms before 2551Q has a complete production evidence chain.
```
