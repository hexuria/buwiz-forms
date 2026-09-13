# V1 forms — ordered build plan

Written from `crates/bir-desktop/docs/eli5/01-what-v1-becomes.html` and
`PLAN-v1-forms-eli5-brief.md` §1.B–§1.C (Phase 0 decided). This file is the
Phase 2 artifact. It does **not** start product implementation (Phase 3).

Ground rules that survive into Phase 3:

- Do not touch the live BIR database. Do not set `BIR_SFTP_*`.
- Refer to the maintainer in the third person; no personal names in files.
- Ambiguous behaviour stays a `?` — do not invent.
- ELI5 field pages (`crates/bir-desktop/docs/eli5/forms/<CODE>.html`) plus
  `rules/forms/<id>/fields.json` are the field/section spec. Do not copy the
  current Rust views (`form_*_view.rs`) as the layout source.
- Queue, XML writers, SFTP cron, receipts, print preview, and the agent stay.

Verify ELI5 inventory (already green on this branch):

```bash
python3 crates/bir-desktop/docs/eli5/tools/test_form_pages.py
```

---

## Open questions the plan does not invent

These are still `?` unless marked **Decided**. Phase 3 must not guess.

1. **Editor scope.** **Decided** (Phase 3, maintainer): inventory-driven editor
   for **all 43** rules bundles. Typed models remain for the ten that have them;
   other codes use a JSON/map-backed draft. Queue, XML writers, and live BIR
   submit stay behind `can_queue_for_submission` / `support_level` — do not
   widen live BIR submit. ELI5 pages exist for all 43 rules bundles. Typed
   models and XML writers exist for ten forms; `support_level.rs` currently
   allows queue only for `2551Q` and `1601C`.
2. **Frozen HTML year ≠ rules year** (stem-paired, not verified equal): 0605
   (`v2003` / `0605-1999`), 1601EQ (`v2018` / `2019`), 1601FQ (`v2018` /
   `1601-fq-2020`), 1602Q (`v2018` / `2019`), 1700 (`v2013` / `2018`), 2200T
   (`v2020` / `2022`). See `forms/README.md`.
3. **Filing frequency `?`** on 1600WP, 1706, 1707, 1801, 2200A, 2200C, 2200M,
   2200P, 2200T, 2552. Deadline calendar must not invent monthly/quarterly/
   annual for those codes.

---

## Work item 1 — validation gating

**Picture:** 01 picture 4. Pristine page shows nothing red. After a field is
touched, that field validates. After the draft has been saved once, all
fields validate on change. Submit still validates everything.

### Files touched

- `crates/bir-desktop/src/components/form_validation/state.rs` — add
  `touched: BTreeSet<String>` and `saved_once: bool`; a
  `visible_errors(field)` helper that applies the gate. Keep evaluator
  stamping as it is.
- `crates/bir-desktop/src/views/form_2551q_view.rs` — **stop** assigning
  `view.validation_errors = view.validate_for_submit(cx)` inside `new()`
  (today: line 557). Paint errors from the gated helper. Mark a field touched
  on change/blur. Set `saved_once` from a successful `save_draft`.
- The nine old views also validate in `new()` / constructors
  (`form_1601c_view.rs`, `form_0619e_view.rs`, `form_0619f_view.rs`,
  `form_0605_view.rs`, `form_1701q_view.rs`, `form_1701_view.rs`,
  `form_1702rt_view.rs`, `form_1702mx_view.rs`, `form_2550q_view.rs`). Gate
  them the same way until work item 7 deletes them.
- `crates/bir-core/src/db/drafts.rs` — optional: persist `saved_once` if a
  reload of an existing draft must keep showing errors. **Phase 3:** newly
  opened drafts are often persisted immediately (they already have a row
  id), so a database id is not a reliable "the user saved" bit. `saved_once`
  stays in the view session only (`?` until a dedicated flag exists).

### Tests that prove it

- Unit: `form_validation/state.rs` `mod tests` — pristine snapshot has zero
  visible blocking issues; after `touch("txtYear")` only that field's
  issues; after `mark_saved()` every blocking issue is visible.
- Unit: `form_2551q_view.rs` / any extractable `visible_errors` helper —
  `new()` on a blank draft returns no painted errors; `validate_for_submit`
  on Submit still returns the full list (TIN, year, …).
- Existing 2551Q/1601C view tests that currently expect errors on open must be
  rewritten to expect an empty painted set, not weaker validation rules.
- `rtk cargo test -p bir-desktop --lib form_validation`
- `rtk cargo test -p bir-core form_2551q`

### What gets deleted

- The `new()` call that fills `validation_errors` before the user has typed or
  saved. No validation *rules* are deleted.

---

## Work item 2 — one page layout driven by fields.json sections

**Picture:** 01 picture 3. One navbar (Back / Save / Submit), pipeline band
the same black as the page, collapsible UX sections, every inventory field
present. Screen sections follow the ELI5 buckets (period, identity,
computation, totals, schedules, payment). Paper Parts/Schedules are
reference only (Phase 0 decision 6). Page-2 TIN/name repeats are **hidden in
the editor**, shown in print preview, still written to XML (decision 7).

### Files touched

- `crates/bir-desktop/docs/eli5/tools/form_page.py` — emit a sidecar
  `crates/bir-desktop/docs/eli5/forms/<CODE>.sections.json` (section id, title,
  field keys in draw order) so Rust does not re-implement Python bucketing.
  Keep `test_form_pages.py` asserting `drawn == field_count` and the 2551Q
  canary (identity contains TIN/RDO; print/XML contains `txtPg2*`).
- New: `crates/bir-desktop/src/views/form_inventory_view.rs` (name can
  change) — one view: load `fields.json` + sidecar, render collapsible
  sections, map `control_kind` to inputs, skip `print_xml` keys in the
  editor, still pass them to the XML writer / print preview.
- `crates/bir-desktop/src/components/form_engine.rs` — shared navbar +
  pipeline band (fix background to page black, as 01 picture 3).
- `crates/bir-desktop/src/app.rs`, `views/mod.rs`,
  `crates/bir-desktop/src/agent/host.rs` (`open_form_view`) — route 2551Q
  through the inventory view first.
- Print path stays `crates/bir-print/src/frozen_html.rs` +
  `html-frozen/<bundle>/index.html` (preview already has page-2 repeats).

### Tests that prove it

- `python3 crates/bir-desktop/docs/eli5/tools/test_form_pages.py` — still
  green; sidecar field lists equal `field_count`.
- Unit: inventory view hides keys in the `print_xml` section and still
  includes them in the BIR field map used by `form_2551q_xml.rs`.
- Unit: every 2551Q `fields.json` key is bound exactly once (no silent drop).
- `rtk cargo test -p bir-core --lib form_2551q_xml`
- Phase 3 (not this run): gpui-agent screenshot of 2551Q — pipeline band,
  collapsible identity section, no page-2 TIN row in the editor.

### What gets deleted

- Hand-placed 2551Q Part widgets that omit inventory fields, once the
  inventory view is the 2551Q editor. `form_2551q_view.rs` (3 109 lines)
  goes when the generic view covers Save / Submit / agent patches. Do not
  delete it until work item 7's checklist says the agent host compiles
  without it.

---

## Work item 3 — profile-year + tax-year selector

**Picture:** 01 picture 1. The per-year profile clone *is* the version. Lookup
by tax year. No effective-date arithmetic. Earliest year = incorporation /
registration date; for a natural person, or when that date is missing, the
date the TIN was obtained. **Business Start Date** means that date. Reuse the
existing year control (`forms_editor_year_select` in
`tab_tax_profile.rs`) and sit it by the Edit / Create Tax Profile title.

### Files touched

- `crates/bir-core/src/profile.rs` — `resolve_tax_profile_for_year` /
  `resolve_tax_profile_for_period` become “return the clone for year Y” (no
  `effective_from` / `effective_until` overlap scan). `TaxProfileVersion`
  ledger fields stop being the runtime source. Keep a year-keyed store of
  profile clones (table name is not invented here if a dedicated table does
  not already exist — `?` until Phase 3 lists the migration).
- `crates/bir-core/src/forms/form_2551q.rs` —
  `reconcile_with_effective_profile` copies from the profile-year; a missing
  year is “no 20XX profile”, not `NoEffectiveVersionForPeriod`.
- `crates/bir-desktop/src/views/profile_manager/mod.rs`,
  `tab_tax_profile.rs` — year selector next to the title; clone-year action;
  earliest-year clamp using Business Start Date.
- `crates/bir-core/src/validation.rs` — drop
  `NoEffectiveVersionForPeriod` as a filing blocker.

### Tests that prove it

- `profile.rs` `mod tests` — lookup 2026 returns the 2026 clone; 2025 is a
  different clone; a year before Business Start Date is rejected.
- `form_2551q.rs` tests that today require a Confirmed `TaxProfileVersion`
  covering 2026-Q1 (`profile_resolution_error` / “No confirmed”) must
  instead pass when a 2026 profile-year exists with name/TIN/RDO, and fail
  only when that year is missing.
- `rtk cargo test -p bir-core --lib profile`
- `rtk cargo test -p bir-core --lib form_2551q`
- `crates/bir-core/tests/dashboard_profile_matrix_test.rs` — rewrite any
  case that seeds `profile_versions` overlap/undated confirmed rows.

### What gets deleted

- Runtime use of `TaxProfileVersion` effective ranges, Confirmed /
  NeedsReview as a gate on Part I, `ensure_profile_version_ledger` as the
  thing 2551Q waits on. Storage columns for the ledger wait for work item 5
  so COR removal and ledger removal land together.

---

## Work item 4 — user-chosen forms + remove inference flags

**Picture:** 01 picture 2. Per profile-year, the user picks forms.
`per_year_forms` stays, **source = Manual only** (Phase 0 decision 4).
Dashboard and deadline calendar keep reading
`active_form_codes_for_year`. Profile fields that existed only to infer forms
are removed. A flag that was really a form field (1601-C item 11 Private /
Government) becomes a form default / template field, not a profile
attribute.

### Files touched

- `crates/bir-core/src/forms/forms_set.rs` — collapse `FormSetSource` to
  `Manual` (or keep the enum but refuse to persist CorAi / ReviewedCor /
  InferredTaxType / MigrationBackfill).
- `crates/bir-core/src/db/forms_set.rs`, `db/migrations.rs` — still the
  `per_year_forms` table.
- `crates/bir-core/src/forms/registry.rs` — stop using `requires_vat`,
  `requires_employees`, `taxpayer_types` as a show/hide router.
  `forms_for_profile` is already deprecated.
- `crates/bir-core/src/integration/validation.rs` —
  `applicable_forms_for_profile_and_year` already reads the yearly set;
  remove remaining inference helpers that still consult VAT/withholding
  flags.
- `crates/bir-core/src/profile.rs` — remove (after the grep in the next
  paragraph) `is_vat_registered` as a router, `withholds_compensation`,
  `withholds_expanded`, `withholds_final`, `is_top_withholding_agent`,
  `is_government_withholding_entity`, `is_gpp_partner`,
  `has_single_employer`, `is_dormant`, `excise_tax_categories`,
  `registration_activity_status`.
- `crates/bir-desktop/src/views/profile_manager/tab_tax_profile.rs` — forms
  picker is a checklist; delete the inference toggles.
- `crates/bir-core/src/forms/form_1601c.rs` / desktop 1601-C item 11 — seed
  from template/default, not `is_government_withholding_entity`.
- `crates/bir-core/src/forms/form_0619e.rs`, `form_0619f.rs` — same for
  withholding-agent category defaults.

**Grep gate (Phase 0 decision 5) before deleting flags:**

```text
rg is_vat_registered|withholds_compensation|withholds_expanded|withholds_final|is_top_withholding_agent|is_government_withholding_entity|is_gpp_partner|has_single_employer|is_dormant|excise_tax_categories|registration_activity_status \
  crates/bir-core/src/background_cron.rs \
  crates/bir-core/src/db/drafts.rs \
  crates/bir-core/src/forms/*_xml.rs
```

Today `background_cron.rs` does not mention those flags. XML modules still
construct `TaxpayerProfile { is_vat_registered: … }` in **tests**. Delete
flags only when writers and cron read the draft field map, not the profile
router. Test fixtures that still fill `TaxpayerProfile` can drop the fields
in the same change.

### Tests that prove it

- `crates/bir-core/tests/per_year_forms_test.rs` — save Manual 2551Q+1601C
  for 2026; dashboard list equals that set even if VAT/withholding would
  have inferred something else.
- `forms_set.rs` `mod tests` — non-Manual sources either disappear or
  cannot become `active_form_codes`.
- `registry.rs` `mod tests` — `forms_for_profile` / VAT filters unused.
- `form_1601c.rs` — item 11 round-trips on the draft without a profile
  government flag.
- `crates/bir-core/tests/dashboard_profile_matrix_test.rs` — rewrite cases
  that seed inferred sets from VAT/non-VAT profiles.
- Re-run the grep gate; CI job or a `#[test]` that `include_str!`s cron +
  xml writers and asserts the flag identifiers are absent (except comments).

### What gets deleted

- Inference seeding (`FormSetSource::CorAi`, `ReviewedCor`,
  `InferredTaxType`, `MigrationBackfill`) as producers of filing obligations.
- Profile router flags listed above, and the show/hide UI around them.
- `FormDefinition::requires_vat` / `requires_employees` as eligibility
  filters (the struct fields can remain as dead metadata until a later
  cleanup if removing them churns the whole registry in the same PR — `?`).

---

## Work item 5 — COR / OCR removal

**Picture:** 01 pictures 1 and 7. COR tab, upload, Gemini OCR, and the
profile-version ledger go. Work item 3 must already have replaced “which
profile applies to period X?”.

### Files touched (then deleted)

- Delete `crates/bir-desktop/src/cor_ocr.rs` (1 768 lines),
  `cor_evidence.rs` (296), `components/document_viewer.rs` (315).
- `crates/bir-desktop/src/views/profile_manager/mod.rs` (5 459) and
  `tab_tax_profile.rs` (4 768) — remove COR tab, upload, OCR detail,
  Gemini settings (`COR_OCR_GEMINI_ENABLED_SETTING`,
  `COR_OCR_GEMINI_MODEL_SETTING`).
- `crates/bir-core/src/profile.rs` / `db/migrations.rs` — drop the versions
  ledger once nothing reads it (work item 3).
- Settings UI that only exists to pick a Gemini model for COR.

### Tests that prove it

- Desktop crate compiles with those modules gone (`rtk cargo test -p bir-desktop --lib`).
- Profile manager tests / `dirty_state.rs` — saving a profile-year does not
  create a COR evidence row.
- Grep: `cor_ocr`, `Gemini`, `document_viewer` have no remaining production
  references.
- `dashboard_profile_matrix_test.rs` and `adversarial_stress_test.rs` —
  rewrite seeds that used `profile_versions` from COR backfill.

### What gets deleted

- COR tab, COR upload + evidence, Gemini OCR + settings, profile versions
  ledger, effective-date review UI. Print preview and queue are untouched.

---

## Work item 6 — templates

**Picture:** 01 picture 6. Per form, per profile, shared across years
(Phase 0 decision 3). User ticks which fields to keep from the last
submission. Every field stays overridable. The taxpayer profile is never
copied into the template. Default: no template until the user makes one.
Pre-fill order: **profile-year → template → typed**.

### Files touched

- New table (migration in `crates/bir-core/src/db/migrations.rs`) keyed by
  `(tin, form_code)` — not by tax year. Exact column list is Phase 3.
- New module e.g. `crates/bir-core/src/forms/templates.rs` + db repo.
- Inventory / 2551Q view — “Save as template” and a field-picker; apply on
  `new_from_profile` after profile-year fill.
- Skip keys classified `src-profile` on the ELI5 page (TIN, RDO, name,
  address, …) when writing a template.

### Tests that prove it

- Save a 2551Q template with ATC rows ticked; a new 2026-Q2 draft gets
  those ATC codes and not the 2026-Q1 TIN (TIN still comes from the 2026
  profile-year).
- Changing the 2026 profile name updates Part I even when a template
  exists.
- No template → blank typed fields, profile-year still fills identity.
- `rtk cargo test -p bir-core templates` (name TBD with the module).

### What gets deleted

- Nothing large. Do not store a second copy of the profile on the template
  row.

---

## Work item 7 — delete the nine old views

**Picture:** 01 picture 7. After the inventory view (work item 2) is the
editor for every form that still has an in-app page, delete the old-layout
views. Suggested nine (old layout / non-2551Q):

| file | lines (this branch) |
|---|---|
| `form_1601c_view.rs` | 1 877 |
| `form_0619e_view.rs` | 975 |
| `form_0619f_view.rs` | 1 031 |
| `form_0605_view.rs` | 1 396 |
| `form_1701q_view.rs` | 1 586 |
| `form_1701_view.rs` | 1 890 |
| `form_1702rt_view.rs` | 515 |
| `form_1702mx_view.rs` | 807 |
| `form_2550q_view.rs` | 3 458 |

`form_2551q_view.rs` (3 109) is deleted in the same item if work item 2 has
already replaced it; otherwise it is the last deletion.

### Files touched

- `crates/bir-desktop/src/views/mod.rs` — drop `pub mod form_*` for the nine
  (and 2551Q if gone).
- `crates/bir-desktop/src/app.rs` — `form_2551q_view` / `form_1601c_view`
  entities become the inventory view.
- `crates/bir-desktop/src/agent/host.rs`, `agent/drain.rs` — form open /
  fill / validate / save go through the inventory view. 1601-C
  `Agent1601CHostPatch` and 2551Q patches become generic field-key patches
  or stay as thin adapters.
- `components/form_engine.rs` — keep the trait if the inventory view still
  uses it; otherwise shrink to the pipeline widget.

### Tests that prove it

- `rtk cargo test -p bir-desktop --lib` — no references to deleted types.
- Agent host: `open_form("1601C" | "2551Q" | …)` still returns a dispatch
  result; fill-by-key still sets inventory fields.
- Phase 3: gpui-agent screenshots, one per remaining typed-model form —
  Back / Save / Submit, black pipeline, collapsible sections, no validation
  on a pristine page.
- XML round-trips unchanged: `rtk cargo test -p bir-core --lib form_1601c_xml`
  (and siblings). Deleting a view must not change `*_xml.rs`.

### What gets deleted

- The nine old-layout views (~13 535 lines) plus `form_2551q_view.rs` once
  redundant (~16 644 lines of per-form view code in total, as 01 picture 7).
- Per-view `mod tests` that only existed to parse old widgets — replace with
  inventory-view tests, do not drop coverage of period parsing / category
  codes (1601-C item 11).

---

## Suggested Phase 3 sequencing (implementation, not this run)

Do not start these in the ELI5/docs PR.

1. Work item 1 (gating) on 2551Q only — smallest user-visible fix.
2. Work item 2 sidecar + inventory view behind 2551Q.
3. Work item 3 profile-year (unblocks 2551Q Part I without COR).
4. Work item 4 forms picker + grep gate, then delete inference flags.
5. Work item 5 COR/OCR + ledger deletion.
6. Work item 6 templates.
7. Work item 7 delete old views; expand inventory view to 1601C then the
   other eight typed models. All 43 editors only if open question 1 is
   answered “yes”.

Each Phase 3 PR: conventional commits, tests listed above, gpui-agent
screenshots for UI, no `BIR_SFTP_*`, no live DB.

---

## What this plan does not change

- `background_cron.rs`, SFTP, receipts, claim tokens, `form_drafts`.
- `html-frozen/**` print preview.
- `rules/forms/**` as field truth.
- Agent control plane (except routing open/fill to the new view).
- EOPT tier and ATC rate tables as **visible form-period facts** (01
  picture 5). They are not profile inference. Whether they stay on the
  profile-year or become plain form inputs is `?` if the code still copies
  `eopt_tier` from `TaxpayerProfile` into the draft (`form_2551q.rs`
  `new_from_profile`).
