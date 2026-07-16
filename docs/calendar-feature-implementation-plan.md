# BIR Tax Calendar Fix Implementation Plan

Date: 2026-05-16

> Historical implementation plan. The temporal engine referenced in some
> problem statements was removed in June 2026. Current form identity lives in
> `forms/registry.rs`, and profile applicability comes from the per-year Forms
> Set plus confirmed COR/profile versions.

Source audit: `docs/calendar-feature-audit.md`

Scope: required fixes for the static BIR tax calendar migration before the calendar can be treated as a reliable compliance surface. This plan keeps the base rules static, but corrects date semantics, form identity, routing, profile filtering, event-based forms, overrides, schema cleanup, and regression coverage.

## Guiding Decisions

1. Static base rules stay. Do not restore end-user CRUD for official recurring BIR deadlines.
2. Deadline resolution must distinguish taxable period year from deadline calendar year.
3. Internal routing and filtering must use canonical app form codes, while display can preserve BIR-style form numbers.
4. Dated calendar entries must represent real due dates only.
5. Emergency extensions and regional advisories need a narrow evidence-backed override layer, not a full calendar administration UI.
6. Tests must land with the model/API work, before expanding rule coverage.

## Phase 0: Stabilize The Worktree And Baseline

Problem solved: current calendar files are mixed staged/unstaged, so implementation needs a clean scope boundary before changing behavior.

Primary files:

- `crates/bir-core/src/calendar_rules.rs`
- `crates/bir-core/src/db/migrations.rs`
- `crates/bir-core/src/db/mod.rs`
- `crates/bir-desktop/src/views/dashboard.rs`
- `crates/bir-desktop/src/views/global_dashboard.rs`
- `crates/bir-desktop/src/views/admin_calendar_dashboard.rs`
- `crates/bir-desktop/src/components/compliance_calendar.rs`
- `crates/bir-desktop/src/components/upcoming_deadlines_list.rs`

Tasks:

1. Separate staged calendar migration work from unrelated penalty/form/print changes.
2. Capture current behavior with failing tests or ignored test sketches for the audited defects.
3. Decide whether to keep `crates/bir-core/src/db/calendar.rs` deleted or restore a narrowed override-only repository.

Acceptance criteria:

- `git status --short` has a known calendar-only implementation scope.
- Existing `cargo check -p bir-core` and `cargo check -p bir-desktop` still pass before functional edits.
- The implementation PR/commit description explicitly says this is a correctness fix, not broader rule coverage certification.

## Phase 1: Replace The Deadline Data Model

Problems solved:

- `ResolvedTaxDeadline` stores only strings and loses original deadline evidence.
- UI cannot route to the correct taxable period.
- Event-based entries are forced into fake dates.

Target design:

```rust
pub struct ResolvedTaxDeadline {
    pub form_code: String,
    pub display_form_no: String,
    pub form_name: String,
    pub period: DeadlinePeriod,
    pub deadline: DeadlineKind,
    pub status: DeadlineStatus,
    pub description: String,
    pub source_reference: Option<String>,
}

pub enum DeadlinePeriod {
    Monthly { taxable_year: i32, month: u8 },
    Quarterly { taxable_year: i32, quarter: u8 },
    Annual { taxable_year: i32 },
    DateRange { start: NaiveDate, end: NaiveDate },
    EventBased,
}

pub enum DeadlineKind {
    Dated {
        original_deadline: NaiveDate,
        final_deadline: NaiveDate,
    },
    EventBased {
        trigger: &'static str,
        statutory_window: &'static str,
    },
}

pub enum DeadlineStatus {
    Normal,
    WeekendAdjusted,
    HolidayAdjusted,
    NonWorkingDayAdjusted,
    Extended,
}
```

Tasks:

1. Move date storage from `String` to `NaiveDate` in core model.
2. Add `form_code` for internal use and `display_form_no` for UI labels.
3. Add typed `DeadlinePeriod` so dashboard clicks can derive year/month/quarter correctly.
4. Add `DeadlineKind::EventBased` so undated obligations never need fake `YYYY-12-31` dates.
5. Add `BusinessDayCalendar` so weekend and configured non-working-day adjustment
   is part of core deadline resolution, not UI rendering.
6. Add small helper methods for UI compatibility:
   - `final_deadline_date() -> Option<NaiveDate>`
   - `period_start() -> Option<NaiveDate>`
   - `period_end() -> Option<NaiveDate>`
   - `route_year_quarter() -> Option<(u16, u8)>`

Acceptance criteria:

- Weekend, holiday, and configured non-working-day adjustment preserves both
  original and final due date.
- Event-based forms cannot be sorted into dated calendar cells by accident.
- UI callers no longer slice date strings like `d.final_deadline[8..10]`.

## Phase 2: Split Resolver APIs By Date Semantics

Problems solved:

- `resolve_year(year)` currently means taxable year in core but calendar year in UI.
- Cross-year annual and December-period deadlines land in the wrong view.

Target API:

```rust
impl DeadlineResolver {
    pub fn resolve_taxable_year(taxable_year: i32, context: DeadlineContext) -> ResolvedDeadlineSet;
    pub fn resolve_deadline_calendar_year(calendar_year: i32, context: DeadlineContext) -> ResolvedDeadlineSet;
}
```

Tasks:

1. Rename the current generator path to `resolve_taxable_year`.
2. Implement `resolve_deadline_calendar_year(calendar_year)` by generating `calendar_year - 1`, `calendar_year`, and possibly `calendar_year + 1`, then filtering dated entries where `final_deadline.year() == calendar_year`.
3. Keep event-based entries out of `resolve_deadline_calendar_year` unless explicitly requested by context.
4. Update the admin explorer label to clarify whether it is showing taxable-year generation or deadline-calendar-year output.
5. Remove or deprecate ambiguous `resolve_year`.

Acceptance criteria:

- Calendar year 2026 includes Calendar Year 2025 annual ITR due dates.
- Calendar year 2026 includes January 2026 deadlines for December 2025 periods.
- Calendar year 2026 does not include January/April 2027 deadlines generated from taxable year 2026.

## Phase 3: Canonicalize Form Identity

Problems solved:

- Calendar emits `1601-C`, `0619-E`, `1701-MS`, etc.
- Registry, Forms Set storage, draft storage, and routing use `1601C`, `0619E`, `1701MS`, etc.

Tasks:

1. Add a small canonicalization helper in core, near the form registry.
2. Encode known display-to-canonical mappings:
   - `0619-E` -> `0619E`
   - `0619-F` -> `0619F`
   - `1600-WP` -> `1600WP`
   - `1601-C` -> `1601C`
   - `1601-EQ` -> `1601EQ`
   - `1601-FQ` -> `1601FQ`
   - `1604-C` / `1604-F` -> decide whether these map to `1604CF` or remain separate display-only entries
   - `1701-MS` -> `1701MS`
   - `1702-EX` -> `1702EX`
   - `1702-MX` -> `1702MX`
   - `1702-RT` -> `1702RT`
   - `2200-A` -> `2200A`
   - `2200-AN` -> `2200AN`
   - `2200-M` -> `2200M`
   - `2200-S` -> decide canonical support status
   - `2200-T` -> `2200T`
3. Assert each rule form maps to a known app code, or is explicitly marked as external/display-only.
4. Use `form_code` everywhere for filtering and event emission.
5. Use `display_form_no` only for rendered labels.

Acceptance criteria:

- Profile dashboards no longer drop withholding and annual form deadlines because of hyphen mismatch.
- Deadline clicks emit canonical form codes accepted by `AppState::open_form`.
- Tests fail if a new static rule uses an unmapped display code.

## Phase 4: Fix Dashboard And Calendar Integrations

Problems solved:

- Global dashboard shows deadlines unrelated to configured profiles.
- Profile dashboard filters period chips by due date instead of taxable period.
- Upcoming deadline clicks emit `quarter: 0` and derive year from due date.

Tasks:

1. In `GlobalDashboardView`, build the union of applicable canonical form codes across loaded profiles.
2. Filter global calendar entries by that union before passing to `ComplianceCalendar`.
3. In `DashboardView`, filter deadline period chips by `DeadlinePeriod`, not due date.
4. Add a separate due-date-month filter only if the UI intentionally needs one.
5. In `UpcomingDeadlinesList`, route clicks from `DeadlinePeriod`:
   - monthly forms: emit year and month through the existing `quarter` field only if that field is already used as month for monthly drafts.
   - quarterly forms: emit year and real quarter.
   - annual forms: emit taxable year and a stable annual sentinel compatible with current routing.
   - event-based entries: do not emit `FileForm` until a real trigger/period is selected.
6. Update `ComplianceCalendar` to render dated entries only in date cells and render event-based actions in a separate section.

Acceptance criteria:

- Q1 `1701Q` due May 15 still appears when Q1 taxable-period filter is active.
- A deadline card for `2551Q` opens the correct taxable year and quarter.
- Global dashboard calendar does not show forms that apply to no configured profile.
- Event-based items do not appear as December 31 urgent deadlines.

## Phase 5: Event-Based Forms Module

Problems solved:

- Estate tax, donor tax, capital gains, and payment forms are currently represented with fake deadlines.

Tasks:

1. Split static rules into `OfficialDeadlineRule` and `OfficialEventRule`.
2. Add an `EventBasedObligation` model with:
   - canonical form code
   - display form number
   - title
   - trigger
   - statutory window
   - optional source reference
3. Render event-based forms under an "Event-Based Forms" or "Required Actions" section.
4. Defer dated calendar entries for these forms until the app has an actual transaction/death/donation/sale event date.

Acceptance criteria:

- No generated dated deadline has a placeholder date.
- Event-based forms remain discoverable without being represented as scheduled obligations.
- Sorting and urgency color only apply to real dated deadlines.

## Phase 6: Add A Narrow Override Layer

Problems solved:

- Static-only base rules cannot absorb emergency extensions or regional advisories.
- Reinstating full calendar CRUD would recreate the original drift risk.

Target model:

```rust
pub struct DeadlineOverride {
    pub id: String,
    pub title: String,
    pub source_reference: String,
    pub affected_form_codes: Vec<String>,
    pub original_deadline: NaiveDate,
    pub adjusted_deadline: NaiveDate,
    pub affected_regions: Vec<String>,
    pub affected_taxpayer_types: Vec<String>,
    pub effective_from: Option<NaiveDate>,
    pub effective_until: Option<NaiveDate>,
    pub expires_at: Option<NaiveDate>,
}
```

Tasks:

1. Add override application after base deadline generation and business-day
   adjustment.
2. Match overrides by canonical form code plus original/final date scope.
3. Preserve source reference and original deadline in `DeadlineStatus::Extended`.
4. Run business-day adjustment again after applying an override, because an
   extended due date can also fall on a holiday or non-working day.
5. Support a bundled JSON override source first.
6. Later, optionally map synced `BirNotice` deadline advisories into candidate overrides, but require explicit trusted-source parsing.

Acceptance criteria:

- Base rules stay static and non-editable.
- An override can adjust a specific deadline without changing compiled base rules.
- Every applied override is traceable to a source reference.
- Expired or non-matching overrides do not affect output.

## Phase 7: Schema Cleanup

Problems solved:

- Migration v6 still creates old full calendar management tables.
- The old schema does not match the intended static-base architecture.

Dependency: complete Phase 6 decision first. Do not clean schema before choosing the override storage path.

Tasks:

1. If overrides are file-backed only, remove unused calendar-management table creation from fresh-schema migrations:
   - `tax_calendars`
   - `tax_forms`
   - `tax_deadline_rules`
   - `resolved_tax_deadlines`
2. If overrides are DB-backed, replace old broad tables with a narrow override/advisory table.
3. Leave legacy existing databases safe. Do not drop user tables without an explicit migration and backup plan.
4. Keep or separately document `tax_deadlines` if it remains part of notices/reminders rather than official schedule generation.
5. Update `docs/migrations_and_versioning.md`.

Acceptance criteria:

- New installs no longer create unused CRUD calendar tables unless they are intentionally retained for override storage.
- Existing databases migrate without destructive calendar data loss.
- Schema docs match runtime architecture.

## Phase 8: Regression Tests

Problems solved:

- Calendar correctness currently has no guardrails.

Core tests:

1. `resolve_deadline_calendar_year(2026)` includes annual ITR for taxable year 2025 due in 2026.
2. `resolve_deadline_calendar_year(2026)` includes January 2026 deadlines for December 2025 periods.
3. `resolve_deadline_calendar_year(2026)` excludes January/April 2027 deadlines from taxable year 2026.
4. `1701Q` Q1/Q2/Q3 entries preserve taxable quarter metadata while due dates land in May/August/October.
5. Weekend adjustment preserves `original_deadline` and changes only `final_deadline`.
6. Business-day adjustment keeps advancing across consecutive weekends and
   configured holidays/non-working days.
7. Override adjusted dates are rechecked against the business-day calendar.
8. Every static rule form maps to a canonical form code or explicit external marker.
9. Event-based rules do not produce dated deadlines.
10. Override application requires source reference and correct affected form/date scope.

Desktop-facing tests or focused component checks:

1. Dashboard Q1 filter includes Q1 period deadlines even when due date is in Q2.
2. Deadline click emits canonical code plus correct taxable year and quarter.
3. Global dashboard filters to profile-applicable canonical codes.
4. Event-based forms render outside the dated calendar grid.

Acceptance criteria:

- `cargo test -p bir-core calendar` or equivalent targeted tests pass.
- `cargo check -p bir-core` passes.
- `cargo check -p bir-desktop` passes.

## Release Gate

The calendar feature should not be described as compliance-ready until all of these are true:

1. Calendar-year and taxable-year semantics are explicit and tested.
2. Internal form identity uses canonical codes consistently.
3. UI period filters use taxable-period metadata.
4. Deadline clicks open the correct form period.
5. Event-based forms are removed from fake dated rows.
6. Weekend, configured non-working-day, and extension adjustments preserve
   source and original-date evidence.
7. Profile dashboards filter by taxpayer applicability.
8. Override strategy is implemented or explicitly deferred with a visible product limitation.
9. Schema docs and migrations match the chosen architecture.
10. Regression tests cover the P1 findings from the audit.

## Suggested Implementation Slices

Slice 1: Core model and resolver semantics

- Phases: 1, 2, 3, core parts of 8.
- Expected result: correct, test-backed core output independent of UI.

Slice 2: Desktop integration

- Phases: 4, 5, desktop parts of 8.
- Expected result: dashboards and calendar components consume the corrected model.

Slice 3: Overrides and schema

- Phases: 6, 7.
- Expected result: emergency adjustments are possible without broad user-editable calendar CRUD.

Slice 4: Final audit closeout

- Re-run targeted tests and checks.
- Update `docs/calendar-feature-audit.md` with resolved status or create a short closeout note.
- Confirm remaining legal-rule coverage gaps separately from architecture/correctness fixes.
