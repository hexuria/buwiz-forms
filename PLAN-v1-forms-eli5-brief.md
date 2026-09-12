# V1 forms — key points and the ELI5 brief

Written 2026-09-13 from the maintainer's review of the form pages and profile
UI, cross-checked against the code on `main` (`130ebf75`). Part 1 organises
what was said and what the code says. Part 2 is the prompt to run before
building any `/eli5` artifact.

---

## Part 1 — Key points, organised

### A. What was observed (with the code behind it)

| # | Observation | Where it comes from |
|---|---|---|
| 1 | Two generations of form page coexist. **Old** (1601-C and 8 others): title + status pipeline in a band inside the content, three buttons *Save Draft / Validate / Generate XML & Submit*. **New** (2551Q only): navbar with *Back / Save / Submit*, a pipeline band under it, collapsible sections. | `views/form_1601c_view.rs` (1 877 lines) vs `views/form_2551q_view.rs` (3 109). All ten views sit on `components/form_engine.rs`; 16 644 lines of per-form view code in total. |
| 2 | The band under the navbar (Draft → Queued → Submitted → Confirmed → Paid) is the *header band*; the old pages paint the title inside it, the new page paints only the pipeline. | old: `bg={background}` band with `render_header` + `render_status_pipeline`; new: `status_banner` with `bg={secondary}`. |
| 3 | Validation fires on load: red boxes on a pristine page. Wanted: no errors until the draft has been saved once or the user has touched a field. | `form_2551q_view.rs:557` — `view.validation_errors = view.validate_for_submit(cx)` inside `new()`. No dirty/touched gate exists. |
| 4 | 2551Q says *Filing blocked — effective taxpayer profile is unresolved* and Part I is blank (Name / RDO / Address "required") although the profile has them. | 2551Q resolves the taxpayer through **effective-dated profile versions** (`profile.rs::resolve_for_period` → `NoEffectiveVersionForPeriod`); Juan's profile has **0** versions ("Loaded 0 COR versions"), so `reconcile_with_effective_profile` fails and nothing is copied into Part I. 1601-C uses `Form1601CDraft::new_from_profile(&profile)` directly — no versions — which is why it works. Two forms, two profile paths. |
| 5 | The profile page carries fields whose only job is to decide which forms to *show*: VAT-registered, withholds compensation / expanded / final, top withholding agent, government withholding entity, GPP partner, single employer, dormant, excise categories, registration activity status. | `profile.rs` (`TaxpayerProfile` + `TaxProfileVersion`), old engine `integration::applicable_forms_for_profile[_and_year]` over `FormDefinition{taxpayer_types, requires_vat, requires_employees}` (51 registry entries), newer `forms/forms_set.rs::PerYearFormsSet` (sources Manual / CorAi / ReviewedCor / InferredTaxType / MigrationBackfill). Both engines are alive. |
| 6 | COR upload / OCR (Gemini) is heavy and, the maintainer judges, unnecessary. | `views/profile_manager/mod.rs` 5 459 lines, `tab_tax_profile.rs` 4 768, `cor_ocr.rs` 1 768, `cor_evidence.rs` 296, `document_viewer.rs`; the COR tab feeds the profile-version ledger that item 4 depends on. |
| 7 | A tax-year selector already exists (form library / tax-profile tab) and should be reused at the top of *Edit / Create Tax Profile*. | `profile_manager/tab_tax_profile.rs`, `mod.rs`. |
| 8 | Tax due is computed per form in core. | `forms/form_*.rs::recompute()` / `form_1601c.rs::compute()`; 2551Q resolves ATC rates through `forms/atc.rs` and EOPT tier (Micro/Small/Medium/Large). |
| 9 | Field truth per form is already generated: XML field inventory + HTML. | `rules/forms/<code>-v<rev>/{fields.json, calculations.json, validations.json, workflow.json}` (43 forms; 2551Q = 99 fields with `control_kind`, `required_when`, `visible_when`, constraints) and `html-frozen/<code>-<year>/{index.html, writer-cells.json}` (46 bundles). In-app typed models exist for 10 forms (`forms/support_level.rs`). |

### B. Decisions taken by the maintainer

1. **V1 = the generated HTML + the XML for submission, and a form page that shows every field of that form**, section by section (Part I, Part II, Schedules…), each section collapsible so a return can be reviewed top to bottom.
2. **All forms adopt the new page layout**: navbar *Back / Save / Submit*, header band with the status pipeline (and fix its background), collapsible sections.
3. **Validation stays automatic but is gated**: pristine page shows nothing; after a save, or once a field is touched, that field's errors show.
4. **The user chooses the forms.** Per tax profile, per tax year, the user picks the forms filed. Consequently the profile fields that existed only to infer forms — excise categories, withholding obligations, government entity, dormant, single employer, GPP partner, VAT flag as a router — and the show/hide logic behind them **are removed**. Anything that was really a *form field* (e.g. 1601-C item 11, Private/Government) becomes a form default, not a profile attribute. Tax-due logic that hung off those flags (e.g. VAT) is removed with them.
5. **Tax profile gets a tax-year dimension.** A profile can be cloned per year. Earliest allowed year: the entity's incorporation / registration date; for a natural person, or when no incorporation date exists, the date the TIN was obtained. *Business Start Date* means that date. The year selector sits above / opposite the *Edit / Create Tax Profile* title and reuses the existing component.
6. **COR tab, COR upload and Gemini OCR go away.** Whatever the profile-version ledger was providing must be replaced by the per-year profile.
7. **Form defaults as a template.** Per form (per profile), the user can save the last submission as a template; fields chosen from it pre-fill the next filing; every one stays overridable; the taxpayer profile is never duplicated into the template.
8. **Give the user the power and flexibility; clean the logic.** Less inference, fewer hidden rules.

### C. Open questions to settle before building (answer in the review)

1. Which forms are in V1's first wave? Suggested: the ten with typed models — 2551Q, 1601C, 0619E, 0619F, 0605, 1701Q, 2550Q, 1701, 1702RT, 1702MX — then the remaining 33 rules bundles.
2. When the profile-version ledger goes, what replaces "which profile applies to period X"? Proposal: the per-year profile clone *is* the version — one per tax year, no effective-date arithmetic.
3. Does a per-year profile hold the chosen forms and the per-form templates, or do templates live on the form? Proposal: profile-year → forms; template → form (shared across years unless overridden).
4. Deadline calendar and dashboard currently read `PerYearFormsSet`; keep that table as the storage of "forms chosen for the year" (source `Manual` only) or fold it into the profile-year?
5. Does the queue / SFTP / receipt pipeline need anything from the removed fields? (Believed no — the XML writer maps from the draft, not the profile flags — to be confirmed in the review.)
6. Sections in the app page: follow the HTML page's Parts / Schedules exactly, or regroup where the paper form is awkward on screen?

---

## Part 2 — The ELI5 brief (the prompt to run first)

```markdown
# ELI5 brief — eBIRForms V1 forms: how it works today, what V1 becomes, one page per form

You are working in /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-smoke-2026-09-12
(bir on `main`). Read PLAN-v1-forms-eli5-brief.md first; Part 1 is the agreed
context, Part 1.C lists the open questions and their proposed answers. Do not
change product code in this task. Do not touch the live database under
~/Library/Group Containers/group.dev.goldcoders.bir/. Use the maintainer in the
third person. Use `rtk cargo …` if you need to build anything.

## What to produce
Three kinds of `/eli5` artifacts (HTML, big pictures, few words), saved under
crates/bir-desktop/docs/eli5/ with these names:

1. crates/bir-desktop/docs/eli5/00-how-it-works-today.html — the CURRENT setup, end to end.
2. crates/bir-desktop/docs/eli5/01-what-v1-becomes.html — the TARGET setup after the cleanup.
3. crates/bir-desktop/docs/eli5/forms/<code>.html — one per form, first wave in this order:
   2551Q, 1601C, 0619E, 0619F, 0605, 1701Q, 2550Q, 1701, 1702RT, 1702MX.
   Each lays out every section and every field of that form as the app page
   must show it.

Build 00 and 01 first, stop, and ask for review before starting the per-form
pages. Every artifact ends with a "Sources" strip listing the files (and line
ranges) it was drawn from, so a reader can check any picture against the code.

## Ground truth — read these, in this order, before drawing anything
- Profile and versions: crates/bir-core/src/profile.rs (TaxpayerProfile,
  TaxProfileVersion, resolve_for_period, applicable_forms_for_year,
  ensure_profile_version_ledger).
- Form choice, old and new: crates/bir-core/src/integration/mapper.rs
  (applicable_forms_for_profile*), crates/bir-core/src/forms/registry.rs
  (FormDefinition, 51 entries), crates/bir-core/src/forms/forms_set.rs
  (PerYearFormsSet, FormSetSource), and the per_year_forms table in
  crates/bir-core/src/db/.
- Support levels: crates/bir-core/src/forms/support_level.rs.
- Tax due: crates/bir-core/src/forms/form_2551q.rs (recompute, ATC rates,
  EOPT tier), forms/atc.rs, form_1601c.rs (compute), and recompute() in
  form_0605/0619e/0619f/1701q/1701/1702rt/1702mx/2550q/2307.
- Drafts, queue, XML, receipts: crates/bir-core/src/db/drafts.rs,
  forms/form_2551q_xml.rs (and the other *_xml.rs), background_cron.rs.
- The pages: crates/bir-desktop/src/views/form_2551q_view.rs (new layout),
  form_1601c_view.rs (old layout), components/form_engine.rs,
  components/form_validation/.
- The profile UI and what is to be removed: views/profile_manager/mod.rs,
  tab_tax_profile.rs, cor_ocr.rs, cor_evidence.rs, components/document_viewer.rs.
- Field truth per form: rules/forms/<code>-v<rev>/fields.json,
  calculations.json, validations.json, workflow.json; html-frozen/<code>-<year>/
  index.html and writer-cells.json.

## Artifact 00 — how it works today (one story, six pictures)
Show, as pictures with almost no text:
1. A taxpayer profile and its two faces: the plain profile (what 1601-C reads)
   and the effective-dated profile versions fed by COR/OCR (what 2551Q reads).
   Show why Juan's 2551Q is "blocked": zero confirmed versions covering
   2026-Q1, so Part I stays empty.
2. "Which forms do I file?": the old inference engine (profile flags →
   FormDefinition filters → list) and the newer PerYearFormsSet (sources:
   Manual, COR-AI, reviewed COR, inferred tax type, migration backfill), and
   that both are still alive.
3. A form page today: the two layouts side by side (1601-C old, 2551Q new),
   which buttons exist, where the status pipeline sits, which sections exist.
4. Validation today: fires in new() on the pristine page.
5. Tax due today: per-form recompute(); for 2551Q the ATC → rate → tax path
   and where EOPT tier and the VAT flag enter.
6. The road to the BIR: draft → queue → XML writer → SFTP → receipt e-mail →
   Confirmed; and where the HTML bundle is used (print preview only).
Keep each picture to one idea. Name real files under each picture.

## Artifact 01 — what V1 becomes (same six pictures, after)
1. One profile with a tax-year dimension: clone per year; earliest year =
   incorporation/registration date, or the TIN date for a person or when there
   is no incorporation date; "Business Start Date" = that date. The year
   selector reuses the existing component and sits by the page title. No
   versions ledger, no COR tab, no OCR.
2. "Which forms do I file?" becomes a choice: per profile-year, the user picks
   forms. Every profile field that existed only to infer forms is gone; the
   ones that were really form fields become form defaults.
3. One form page layout for every form: Back / Save / Submit, the status
   pipeline band (same black as the page), collapsible Part I / Part II /
   Schedules mirroring the HTML form, every field present.
4. Validation gated: nothing on a pristine page; after the first save or once
   a field is touched, that field's errors show; Submit validates everything.
5. Tax due: only the arithmetic the form itself defines (calculations.json);
   no profile-flag branches.
6. Templates: save the last submission as a per-form template, choose which
   fields it keeps, overridable at filing time, never containing the profile.
Add a seventh picture: what gets deleted (modules, tables, fields) and what
stays (queue, XML, receipts, print preview, agent).

## Per-form pages — crates/bir-desktop/docs/eli5/forms/<code>.html
For each form, derive from fields.json + the frozen HTML (not from the current
Rust view) and show:
- The form at a glance: code, revision, filing frequency, who files it.
- Sections in the order of the paper form (Part I, Part II, Schedule …), each
  as a collapsible block in the picture, with every field inside it: item
  number, label, control kind (text / money / radio / checkbox / date / select),
  required-when / visible-when / enabled-when in plain words, and whether the
  value comes from the profile, from the template, or is typed.
- Computed fields drawn as arrows from their inputs (from calculations.json).
- Validations that block Submit (from validations.json), one line each.
- A count line: N fields in fields.json, M drawn here — they must be equal.
- Sources strip with the exact rules/forms and html-frozen paths.

## Style rules for every artifact
- Vivid and compact: one idea per picture, a caption of at most two lines,
  real names (file, table, field) instead of prose.
- Before/after pairs use the same layout so the eye can diff them.
- No invented behaviour: if the code is ambiguous, draw a "?" and add it to
  the questions list at the end of the artifact.
- Third person; no personal names in artifacts.

## Definition of done
- 00 and 01 exist, each with a Sources strip, and a short questions list.
- The maintainer has reviewed 00 and 01 and answered Part 1.C.
- Ten per-form pages exist; each one's drawn field count equals its
  fields.json field_count.
- Nothing under crates/ or rules/ was modified; git status shows only
  crates/bir-desktop/docs/eli5/**.
```
