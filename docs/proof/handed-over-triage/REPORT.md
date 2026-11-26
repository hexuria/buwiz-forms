# Handed-over branch triage

Report only. No handed-over branch was changed. Main for this triage is `a1a86dedea76e62f839093f93c62d0a26827563b` (it moved after the hand-over counts were written). The compared lines are PR #355 `a067f980`, PR #360 `71a5ae77`, PR #362 `40c96fea`, PR #370 `a70b3edd`, PR #371 `6b602402`, and `try/forms-band-flow` `31d3b7d7`.

Trial merges were done only in scratch clones. “Own tests” below means the tests the branch itself added, run on the merged scratch tree where a valid merged tree existed. A conflicted trial merge has no valid tree; resolving it is already part of the porting decision, so whole-branch test results are not claimed for unresolved conflicts.

## 1. `try/print-footer-draft` `d8d44c12` — verdict: still needed, but only the footer/DRAFT half

**What it changes for a filer.** Every printed page gains a small footer outside the official drawing: form number, dashed TIN, period or transaction date, filing state where known, and “Page i of n” including receipt/continuation pages. An unfiled print also carries a light diagonal `DRAFT – not filed`; a dry run keeps that mark and says it was not sent to BIR; a filed return has no mark and the footer adds filing date/reference. The first commit also changes A4 fitting from `zoom` to `transform`.

**Overlap.** The scaling half is superseded by main: `crates/bir-app-api/src/print_sheet.rs` now chooses `transform:scale` on WebKit and `zoom` on Chromium, and `nsh/src/print.rs` understands both. Neither main nor any compared PR/engine ref has `SheetFurniture`, `DRAFT – not filed`, or lifecycle footer text. `try/forms-band-flow` overlaps structurally in `crates/bir-print/src/band_flow.rs` (footer reserve/continuation flow) and `crates/bir-app-api/src/static_html.rs`/`.sheet-flow`, but it does not implement this feature. #355 touches the same shared print pipeline (`print_sheet.rs`, `static_html.rs`), so it is the open PR this must follow.

**Trial merge and tests.** Trial merge conflicts in `crates/bir-app-api/src/print_sheet.rs` and `nsh/src/print.rs`, both caused by the superseded scaling half. In the scratch tree I resolved them by keeping main’s engine-dependent scaling and adding only the footer/DRAFT CSS; that resolution is evidence for feasibility, not a proposed exact patch. Focused Rust tests pass on that scratch resolution in a checkout-specific target dir: five `footer_state*` tests 5/5 and three static-HTML furniture tests 3/3. This proves the branch logic still compiles and runs when its database helper and scaling collision are ported; it is not proof of an untouched mechanical rebase.

**Smallest remaining PR.** Drop the scaling commit entirely. Port only `static_html::SheetFurniture`/footer injection, the footer/DRAFT CSS, lifecycle lookup in `bir_module.rs`/`db/drafts.rs`, and the focused unit tests/proof. It must follow #355 and be rechecked against `try/forms-band-flow` because continuation pages are exactly where footer placement can regress.

## 2. `feat/forms-print-fixes` `eb0916c7` — verdict: still needed only as a small salvage, never as this branch

**What it changes for a filer.** Printed paper becomes closer to the official sheets across many forms: negative/overpayment amounts print in the sheet’s wording, amount digits align with preprinted decimal marks, blank zero date parts stay blank, quantities/counts/TINs print as numbers rather than money, names/spouses/classifications and 1600WP special-law values print correctly, forced font overrides are removed, 1702EX prints tax due/rate/schedule totals, 1702MX prints schedules/Part II/deduction checkbox, 2200S prints all Schedule 1 rows and a six-digit PSIC, several excise forms print place names rather than codes, 2200T routes novel-tobacco rows, and 2200AN/2200M/2200P correct other-row ATC/rate/unit presentation.

**Overlap.** Most of the branch is already represented elsewhere:

- `try/forms-band-flow` already contains or patch-supersedes the first 24 shared-print commits; `git cherry` against it leaves only the later 1702EX/1702MX/excise commits and the golden regeneration.
- #355 overlaps `crates/bir-print/src/frozen_html.rs` and implements the same preprinted-decimal-mark alignment family (`preprinted_decimal_mark`) plus the shared print rules that cover the C1/C11/C12/C13/C14-style work.
- #360 overlaps `crates/bir-core/src/forms/form_1702mx.rs`, `html-frozen/1702mx-2018c/writer-cells.json`, `forms/print-cells.json`, and `html-frozen/2200s-2018/writer-cells.json`; its commits explicitly build the 1702MX print map/overlay all schedules and join 2200S Schedule 1 rows.
- #362 replaces `2200t-2022` with the January-2020 sheet, so this branch’s `html-frozen/2200t-2022/writer-cells.json` work is obsolete as written; the underlying novel-tobacco routing question must be re-proven on the 2020 sheet.
- #370 is unrelated except for overlapping broad files; #371 is unrelated screen-test repair.

**Trial merge and tests.** Trial merge conflicts in five print-golden JSON files, `crates/bir-print/src/frozen_html.rs`, and `html-frozen/1600wp-2010/index.html`. Because those conflicts are exactly the porting decision (which engine/golden generation wins), no valid whole-branch merged tree exists and its tests were not run. This is not a clean rebase candidate.

**Smallest remaining PR.** Do not revive the 35-commit branch. After #355, #360 and #362 land, audit what remains and open a small excise/1702EX print PR. The clearly still-unique candidates are: `names_comb_line`/place names for 2200A/2200C/2200P (`crates/bir-core/src/forms/excise_places.rs`, `form_2200a.rs`, `form_2200c.rs`, `form_2200p.rs`); 2200AN XG digits (`form_2200an.rs`); 2200M continuation-row rates (`form_2200m.rs`); 2200P Others unit/rate styling (`form_2200p.rs`); and the unmerged parts of 1702EX tax due/rate/true totals (`form_1702ex.rs`). Reassess 2200T novel-tobacco routing only after #362’s January-2020 sheet.

## 3. `try/penalty-computed-mark` `f92854cf` — verdict: still needed

**What it changes for a filer.** In a penalty group, every box whose component the filer did not type shows the existing “Computed from other items” lock mark; a typed component carries no mark. It does not change computed amounts, overrides, validation, or upload.

**Overlap.** Nothing in main, #355, #360, #362, #370, #371, or the engine branch implements the per-component `typed` list or per-box penalty mark. Main has the group-level Edit/Use-computed UX in `nsh/screens/form/rows.tsx` and group metadata in `crates/bir-app-api/src/screen_form_module/penalties.rs`, but not this distinction. The engine branch has no file overlap. #371 is only independent screen-test repair.

**Trial merge and tests.** Trial merge is clean. Focused Rust screen-path test `form_1701_penalty_group_says_which_components_are_typed` passes on the merged tree: 1 passed, 0 failed. The NSH test could not be run in a fresh isolated build here: the first attempt needed a sibling NSH checkout, and after supplying the pinned checkout the fresh build stopped before tests because the machine lacks the system `gtk4` pkg-config package required by `gdk4-sys`. This is an environment/setup limitation, not a test failure in the branch. The Rust screen-path test above is the completed branch-owned test evidence.

**Smallest remaining PR.** Essentially this one commit rebased: host `typed` list, `penaltyComputed` session helper, per-box mark, Rust test and `penaltyMark.test.tsx`. It does not semantically need to wait for any open PR; if it touches the same screen-test files later, follow #371’s clear-before-typing convention.

## 4. `try/form-box-counter` `7f71710d` — verdict: still needed, reduced to counter + official caps

**What it changes for a filer.** A typed text box shows how many characters it has against the paper’s character boxes, for example “48 of 54 characters”; past the boxes it says the value will print as a plain line. Money boxes warn only when the integer part exceeds the paper’s amount boxes and say it will print as a plain number. Separately, entry caps come from the official program’s `maxlength`, not the paper’s box count, so a filer can type the full official length even when the paper would overflow.

**Overlap.** Main already has part of the underlying print-cell slot plumbing in `crates/bir-app-api/src/screen_form_module/combs.rs` (`slots`/pool handling), but it has no `official-caps.json`, no `maxlen`/`amount_slots` schema fields, and no `BoxCounter` in `nsh/screens/form/controls/FieldControl.tsx`. #360 and #362 overlap `forms/print-cells.json` because they update print-cell joins; they do not implement the counter. #371 does not implement it either, but its clear-before-typing fixes are relevant to the branch’s screen tests.

**Trial merge and tests.** Trial merge conflicts only in `crates/bir-app-api/src/screen_form_module/tests/dst_pt_vat.rs`, where both sides added tests at the file end. In scratch I union-resolved that test-file conflict. Focused Rust tests all pass on that resolved merged tree: `box_counters_reach_the_schema` 1/1, `a_value_at_the_official_maximum_uploads_in_full` 1/1, and `official_maxima_reach_the_schema` 1/1. The three NSH tests were not run in this triage.

**Smallest remaining PR.** Keep only the official-cap generation/schema fields, amount digit derivation, counter UI, and focused tests. Rebase it after #360/#362 because `forms/print-cells.json` is regenerated data owned by those print-join PRs; use #371’s typing conventions for the NSH tests.

## Exact trial-merge conflicts

| Branch | Trial merge into main `a1a86ded` |
|---|---|
| `try/print-footer-draft` | conflicts: `crates/bir-app-api/src/print_sheet.rs`, `nsh/src/print.rs` |
| `feat/forms-print-fixes` | conflicts: five `crates/bir-print/src/fixtures/*goldens.json`, `crates/bir-print/src/frozen_html.rs`, `html-frozen/1600wp-2010/index.html` |
| `try/penalty-computed-mark` | clean |
| `try/form-box-counter` | conflict: `crates/bir-app-api/src/screen_form_module/tests/dst_pt_vat.rs` |
