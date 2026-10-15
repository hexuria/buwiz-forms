# PR #141 integration review against PR #136

Reviewed 2026-10-16. This is a local integration experiment, not merge approval,
a filing-readiness certificate, or official-form parity evidence.

## Pinned inputs and test branch

- PR #136: https://github.com/hexuria/buwiz-forms/pull/136
  - Head: 2530373521ba08a656ca393f900f6dc7443786fc
- PR #141: https://github.com/hexuria/buwiz-forms/pull/141
  - Head: df62331f69468571c2135ef9f72d24fafbc01145
  - Original base: ec555390154d5d3aff82b8ec96046698709c47ad
  - Five commits, in dependency order, below.
- Test branch: codex/pr141-preview-integration
- Isolated checkout: /Users/uriah/.codex/worktrees/pr141-preview-review/bir
- The main checkout and its other-session edits were preserved.
- GitHub returned no issue comments, inline review comments, reviews, or checks
  for #141 at inspection. The cherry-pick instructions are in the PR body.
- The two independent review agents could not start due to provider rate limits;
  both review axes below were inspected locally.

| Original commit | Test-branch commit | Purpose |
| --- | --- | --- |
| 3c411235 | 6d4814d1 | Decimal comb alignment |
| 6c094f46 | 4d4fd414 | A4 layout and static HTML |
| 3f345b1c | c3429203 | CSS override loader, 43 stylesheets, preview script |
| 8910f241 | ce6a03a6 | Blitz window and WebKit PDF |
| df62331f | 457cc3f6 | Live form-to-preview updates |

Local follow-up 18f2876 repairs engine compatibility, merged-lockfile references,
the reproduced overflow-text update bug, and incoming Rust formatting.

The first three cherry-picked cleanly. Commit four conflicted only in
crates/bir-shell/Cargo.toml: both branches added development profiles.
Resolution retained the existing GPUI/QuickJS optimizations and all Blitz
optimizations, with the new macOS dependencies in their proper table. Commit
five then applied cleanly. Cargo repaired four ambiguous dependency references
in the merged lockfile; no versions were upgraded by that repair.

## Standards review

### S1 — Runtime overrides depend on a developer checkout (P2, open)

Original #141 crates/bir-shell/src/print_sheet.rs:77–98 resolves the default
override directory through compile-time CARGO_MANIFEST_DIR and silently skips a
missing stylesheet. Copy the built app to a machine without that source checkout
and its preview/PDF loses the layout changes. A modified checkout can also
change output from an unchanged binary. This conflicts with the repository's
packaged-offline/reproducible-output requirements.

A direct probe of the actual module included the stylesheet with the checkout
present and omitted it with an absent BIR_PRINT_OVERRIDES directory. Bundle the
reviewed overrides as assets or include_str data, and keep hot overrides an
explicit development mode. Missing required assets should be reported.

### S2 — Mass caption fitting lacks the required evidence (P2, open)

Original #141 print-overrides/1700-2018.css:7 onward (and the other 42 stylesheets)
condenses/repositions static captions based on font-width measurements. For
example, Items 12 and 37 receive scaleX changes; some filled-field overrides
elsewhere use fixed sizes as small as 6.9px. AGENTS.md says not to chase text
pixels; custom_form_styling.md §8 requires contract confirmation, clipping
checks and fresh visual evidence. The cited print-audit generator, approved
measurement artifacts, and both-rasterizer checks are not delivered in this PR.
The comments saying “approved” are not proof available to this review.

Keep the override mechanism separable from these bulk adjustments. Accept
individual corrections with source-bound geometry, clipping tests, and readable
overflow behavior. Do not use the 43 CSS files as evidence of 43 verified forms.

### Integration compatibility repaired on the test branch

The host in #136 selects its engine through gpui_kit, but #141's new window
imports gpui directly. A local module alias now routes all preview types through
gpui_kit, preserving the host's gpui-fast feature choice. The test branch also
normalizes the incoming Rust formatting; the original additions failed cargo
fmt --check.

### Platform follow-up

lb-wry is unconditional, although PDF support is macOS-only. The shell-check
Linux job at .github/workflows/ci.yml:674–697 does not install GTK/WebKit
development packages. Its non-macOS PDF state also never becomes Ready after a
page load. Gate the WebKit backend and native controls by platform, or explicitly
provision and implement those paths. Linux and Windows execution was not
available for this local review; this is an integration risk, not a claimed
cross-platform test result.

## Specification and correctness review

### C1 — Long overflow text can leave the visible preview stale (P1, FIXED locally)

PR claim: live edits re-render “only the changed sheets.”

Original #141 print_preview_window.rs:191–203 considers all field text removable
from the document's shape. static_html::field_map only tracks spans with an id.
frozen_html::comb_overflow_line generates id-less whole-line inputs for long
emails (and derived overflow lines). Changing one long email to another of the
same length preserves the shape and produces no field patches. Blitz repaints
zero sheets while the HTML and subsequent WebKit PDF change.

Reproduced using the actual static_html module and the original shape function.
The local fix keeps untracked text in the shape comparison, forcing a full
rebuild when it changes. Regression tests cover an id-less overflow line,
ordinary identified text patches, and a real 2000-OT frozen-sheet email change.

### C2 — Print freshness does not cover updates still being produced (P1, open)

PR claim: “A Print click while it is stale waits for the fresh one.”

Original print_preview_window.rs:98–104 ticks and potentially prints before
consuming a pending live update. produce_live increments a sequence but does not
invalidate the PDF. apply_live:182–184 also queues a document while rendering
without invalidating it; pdf_stale becomes true only after writing HTML at
211–213. A pending print can therefore use the previous ready PDF while a newer
edit is queued or its producer is running. A freshly requested document can also
be overwritten by an older producer after window reuse, because direct
apply_live calls do not advance the producer sequence.

Track requested, rendered and PDF document revisions. Invalidate on receipt of a
new edit/request, and allow printing only when the PDF revision equals the
latest requested revision and no newer producer remains. Test delayed producers,
edits during PDF generation, and reopen while an old fill is pending.

### C3 — Reopening another taxpayer's form preserves the old title (P1, open)

PR claims one window per form and a title containing the taxpayer's name.

Original print_preview_window.rs:80–86 matches only the form code and sends new
HTML to the existing view. It never updates the cached TitleBar title or native
window title. Open 1700 for taxpayer A, then open 1700 for taxpayer B: the content
changes to B under A's title. Year/period are not part of the preview identity
either.

Key preview ownership and updates by the complete return identity (taxpayer,
branch, form, year, period/draft id), or atomically replace identity, titles and
pending-work generation when reusing the window. A two-taxpayer/period test is
essential before treating this as a reliable inspection tool.

### C4 — Share silently changes format while PDF generation is busy (P2, open)

PR claim: Share uses the vector PDF produced by the hidden WebView.

Original print_preview_window.rs:322–326 sends html_path whenever the PDF is not
Ready or is stale. The Share button stays enabled, so clicking it immediately
after opening or editing shares HTML instead of the expected PDF. Queue Share
until the matching PDF is ready (or explicitly offer a separate HTML action);
surface generation failures.

### C5 — The sample renderer does not verify form/model wiring (scope limit)

examples/print_render.rs fabricates values by iterating writer-cells.json, then
calls filled_document directly. This is useful for layout stress checks, but it
bypasses screen_form_fill, typed draft computation, persistence and XML
serialization. A missing or wrong model-to-writer mapping can still produce a
convincing fully populated sample.

The in-app path does use print_map and the existing Rust filler, which is a good
foundation. The sample renderer must remain labeled a layout aid, not a wiring
or submission-correctness test.

## Recommended decision

Do not apply the entire PR directly to #136 as-is. The test branch preserves all
five commits so the feature can be exercised alongside current form fixes.

- The decimal commit is independently cherry-pickable and passes bir-print tests.
- Commits two through five are sequential. Staticization/A4 needs fresh
  representative PDF and visual checks; source compatibility is not fidelity.
- Separate the CSS loader from its bulk generated caption adjustments.
- Finish C2–C4 and packaged asset handling before relying on Print/Share.
- Keep all current migration/release readiness flags conservative.

## What would make this useful for validating wiring?

Use one synthetic return through the actual application pipeline:

1. Enter distinct marker values through screen_form_fill, then save/reopen.
2. Assert the typed draft, calculated values and official XML against the
   independently pinned official behavior/fixtures.
3. Assert print_field_map transformations separately: rounding, intentional
   blanks, spouse applicability, totals and repeating-row folding.
4. Resolve each writer key to its printed cell and assert the rendered value
   and page. Inspect the generated PDF too, because Blitz and WebKit are
   different layout engines even when they receive the same HTML.
5. Include zero, negative/overpayment, boundary lengths, Unicode, extra rows,
   amended returns, spouse branches, and maximum values. Require explicit
   reasons for fields that are intentionally absent from the printed sheet.
6. A developer-only field inspector can link a screen control to its draft
   path, submission key, print key and PDF cell. That makes discrepancies
   explainable rather than merely visible.

Server acceptance, correct XML mapping, correct print mapping and faithful PDF
layout are four separate claims. This PR improves visibility; it proves none
of them exhaustively.

## Validation

- bir-print: **94 library + 2 exporter + 3 PDF-verification tests passed**.
- bir-shell with gpui-fast: **106 passed, 3 existing ignored, 0 failed**.
  The real 2000-OT overflow test passed.
- Compile checks passed with both gpui-fast and the default upstream engine.
- Strict clippy passed for all shell targets with gpui-fast:
  cargo clippy --locked --manifest-path crates/bir-shell/Cargo.toml
  --all-targets --features gpui-fast -- -D warnings.
- Shell formatting and JavaScript syntax checks passed.
- audit:no-legacy passed.
- validate-v1 passed with the unchanged 43 / 659 / 9,592 / 2,007 / 623 /
  1,354 / 216 baseline; no rules or release-readiness manifests changed.
- The shell binary and print_render example built with gpui-fast.
  The macOS debug linker warned that its unwind table exceeded 16 MB; the
  build completed. Dependency future-incompatibility notices remain for
  block 0.1.6 and imap-proto 0.10.2.
- Native smoke: rendered the sample 0605 through print_render; launched the
  test binary with EBIR_TEST_ENV=1, an isolated smoke.db, dry-run transport,
  and BIR_PREVIEW_HTML. Both test windows were moved to workspace 9, then
  only the owned test process was stopped.
- The preview produced a parseable vector PDF: **1 page, 595.276 × 841.89 pt
  (A4), 86,642 bytes**, with the sample taxpayer name and amount present in
  extracted text. No physical printing, external sharing, or submission ran.
- Not established: all-form field completeness, native live-typing behavior,
  stale Print/Share handling, complete Blitz/WebKit visual equivalence,
  Windows/Linux runtime behavior, or packaged-offline operation.

The sample fills mutually exclusive boxes and uses synthetic stress values;
it is deliberately not a valid filing fixture.

Artifacts and original reproduction runner: tmp/pr141-review/ (ignored).
Useful files:

- native-preview.png — running GPUI Fast preview.
- render/0605-1999.pdf — WebKit-generated output.
- pdf-preview.png — raster of that PDF, inspected separately.
- shell-tests-fast.log, print-tests.log, upstream-check.log, clippy.log.
- probe.rs + patch_shape.rs — original id-less-text reproduction.

The initial original-format.log and shell-tests.log record the incoming
formatting and merged-lockfile failures, not the final repaired result.

## Review totals

Standards: two open findings (S1–S2), plus a separately noted untested platform
integration risk. The most consequential standards issue is loss of overrides
outside the source checkout.

Specification: four defects found (C1 fixed locally; C2–C4 open), plus the C5
verification-scope limitation. The most consequential remaining specification
issue is printing a previous revision while newer work is pending.
