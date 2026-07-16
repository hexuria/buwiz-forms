# BIR Tax Calendar Feature Audit

Date: 2026-05-16

Scope: current working tree audit of the static calendar migration described in the architectural review prompt. This review checks code behavior, integration points, schema cleanup, and tax-calendar correctness risks. It does not certify the legal completeness of every BIR deadline.

External references checked:

- BIR RMC No. 110-2025 announces the 2026 BIR Interactive Tax Calendar and states that it contains monthly tax deadlines for Income Tax, Withholding Tax, VAT, Percentage Tax, Excise Tax, DST, and recurring submission obligations.
- BIR RMC No. 20-2026 describes 2026 filing guidance for Annual Income Tax Returns for Calendar Year 2025, which is important for distinguishing a deadline calendar year from a taxable period year.

## Executive Finding

The static-base-rule direction is sound, but the current implementation is not yet a reliable compliance calendar. The biggest problem is not performance or the loss of CRUD. The biggest problem is that `DeadlineResolver::resolve_year(year)` mixes taxable period year and deadline calendar year, while the UI treats the same value as the selected dashboard/calendar year. This causes annual and cross-year deadlines to appear in the wrong year.

The second major issue is form-code identity. The calendar engine emits several hyphenated BIR display codes while the rest of the app filters and opens forms with canonical internal codes. Those deadlines will be silently filtered out of profile dashboards.

## Findings

### P1. `resolve_year(year)` generates taxable-period-year deadlines, but the UI uses it as a calendar-year schedule

Evidence:

- `DeadlineResolver::resolve_year(year)` passes `year` into every rule generator and sorts by `final_deadline`: `crates/bir-core/src/calendar_rules.rs:40`.
- Annual rules generate due dates in `y + 1`: `crates/bir-core/src/calendar_rules.rs:194`, `crates/bir-core/src/calendar_rules.rs:203`, `crates/bir-core/src/calendar_rules.rs:212`.
- Monthly rules generate December period due dates in January of `y + 1`: `crates/bir-core/src/calendar_rules.rs:88`, `crates/bir-core/src/calendar_rules.rs:100`, `crates/bir-core/src/calendar_rules.rs:112`.
- Global dashboard calls `DeadlineResolver::resolve_year(current_year)`: `crates/bir-desktop/src/views/global_dashboard.rs:45`.
- Profile dashboard calls `DeadlineResolver::resolve_year(selected_year)`: `crates/bir-desktop/src/views/dashboard.rs:199`.
- Admin explorer labels the output as `{selected_year} Tax Deadlines`: `crates/bir-desktop/src/views/admin_calendar_dashboard.rs:84`.

Impact:

- A 2026 dashboard will include annual ITR deadlines for taxable year 2026 due in 2027.
- It will miss annual ITR deadlines for Calendar Year 2025 that are due in 2026.
- It will include January 2027 deadlines for December 2026 periods, but miss January 2026 deadlines for December 2025 periods.

Recommendation:

- Split the API into explicit concepts:
  - `resolve_taxable_year(taxable_year)` for period-driven views.
  - `resolve_deadline_calendar_year(calendar_year)` for actual calendar UI.
- For the calendar-year API, generate adjacent taxable years and filter by `final_deadline.year() == calendar_year`.
- Add tests for January cross-year monthly deadlines and annual AITR deadlines.

### P1. Calendar form codes do not match canonical app form codes

Evidence:

- Calendar emits hyphenated codes such as `0619-E`, `0619-F`, `1601-C`, `1601-EQ`, `1701-MS`, `1702-RT`, and `2200-A`: `crates/bir-core/src/calendar_rules.rs:107`, `crates/bir-core/src/calendar_rules.rs:124`, `crates/bir-core/src/calendar_rules.rs:213`, `crates/bir-core/src/calendar_rules.rs:238`.
- The form registry and app routing use canonical non-hyphenated codes such as `0619E`, `1601C`, `1601EQ`, `1701MS`, and `1702RT`: `crates/bir-core/src/forms/registry.rs:93`, `crates/bir-core/src/forms/registry.rs:506`, `crates/bir-core/src/forms/registry.rs:520`, `crates/bir-core/src/forms/registry.rs:534`.
- Profile dashboard filters deadlines by `codes_set.contains(&d.form_no)`: `crates/bir-desktop/src/views/dashboard.rs:199`.

Impact:

- Applicable profile deadlines for those forms disappear even though the static rule exists.
- Clicking a deadline, when present, may emit a display code that app routing does not recognize.

Recommendation:

- Store both `form_code` and `display_form_no` on calendar entries.
- Use canonical `form_code` for filtering/routing and display `display_form_no` for BIR-style labels.
- Add a test that every `OfficialRule.form_nos` entry normalizes to a known temporal/form registry code or is explicitly marked external/non-applicable.

### P1. Profile dashboard period filtering uses due-date month/quarter, not tax-period month/quarter

Evidence:

- Dashboard period chips are tax-year filters from the form dashboard context: `crates/bir-desktop/src/views/dashboard.rs:287`.
- Calendar filtering derives month and quarter from `d.final_deadline`: `crates/bir-desktop/src/views/dashboard.rs:945`.
- Deadline records already carry `period_start` and `period_end`: `crates/bir-core/src/calendar_rules.rs:13`.

Impact:

- A Q1 1701Q period due on May 15 is categorized as a Q2 deadline because May is in Q2.
- A user filtering Q1 compliance work can miss Q1 return deadlines.

Recommendation:

- Filter by period metadata for period filters.
- Offer a separate due-date filter if needed.
- Rename UI copy where needed so users know whether they are filtering by taxable period or due date.

### P1. Global dashboard no longer filters static deadlines against user profile applicability

Evidence:

- `GlobalDashboardView::new` loads all static deadlines for the current year: `crates/bir-desktop/src/views/global_dashboard.rs:42`.
- Render passes all loaded deadlines to `ComplianceCalendar`: `crates/bir-desktop/src/views/global_dashboard.rs:133`.
- `profiles` are loaded but not used to narrow the calendar deadline list: `crates/bir-desktop/src/views/global_dashboard.rs:29`.

Impact:

- The global calendar can show deadlines unrelated to any configured taxpayer profile.
- This contradicts the architectural report claim that dashboards filter the static list against applicable form codes.

Recommendation:

- Build the union of applicable form codes for all active profiles for the selected calendar/tax year.
- Filter static deadlines by canonical form code before rendering.
- Consider a profile/source badge if a deadline applies to only some profiles.

### P1. Deadline click routing cannot open the correct period

Evidence:

- `UpcomingDeadlinesList` always emits `quarter: 0`: `crates/bir-desktop/src/components/upcoming_deadlines_list.rs:77`.
- It derives the emitted year from `final_deadline.year()`, not from the taxable period: `crates/bir-desktop/src/components/upcoming_deadlines_list.rs:78`.

Impact:

- Quarterly returns opened from the deadline list can receive an invalid quarter.
- Annual and cross-year deadlines can open the due year instead of the taxable year.

Recommendation:

- Add typed period metadata to `ResolvedTaxDeadline`, such as `PeriodKey::Monthly { year, month }`, `Quarterly { year, quarter }`, `Annual { taxable_year }`, and `EventBased`.
- Emit routing parameters from that period key, not from the due date.

### P2. Weekend adjustment destroys original deadline evidence

Evidence:

- `ResolvedTaxDeadline` only stores `final_deadline`, not `original_deadline` or `adjusted_deadline`: `crates/bir-core/src/calendar_rules.rs:10`.
- Weekend adjustment overwrites `final_deadline` and sets status to `Weekend Adjusted`: `crates/bir-core/src/calendar_rules.rs:50`.
- UI displays `Originally due on: {d.final_deadline}` for non-normal statuses: `crates/bir-desktop/src/components/compliance_calendar.rs:412`, `crates/bir-desktop/src/components/upcoming_deadlines_list.rs:212`.

Impact:

- The UI cannot explain what date was moved.
- The "Originally due" copy is wrong because it repeats the adjusted deadline.
- Future emergency extensions cannot preserve auditability without changing the model again.

Recommendation:

- Restore separate `original_deadline`, `final_deadline`, `adjustment_reason`, and `source_reference` fields.
- Use a structured `DeadlineStatus` enum instead of string status labels.

### P2. Static-only rules are not enough for emergency extensions or location-specific advisories

Evidence:

- Old migration schema had `tax_deadline_overrides` with affected forms, original/adjusted deadlines, source references, affected regions, and taxpayer types: `crates/bir-core/src/db/migrations.rs:232`.
- New resolver has no input for override context or external advisory data: `crates/bir-core/src/calendar_rules.rs:40`.
- BIR notices already have fields for `notice_type`, `rdo_code`, `form_code`, and `deadline`: `crates/bir-core/src/db/mod.rs:100`.

Impact:

- Static base rules are good for deterministic baseline deadlines, but emergency or regional extensions require an app update.
- If the app later ingests advisories, the current resolver cannot apply them cleanly.

Recommendation:

- Keep base rules static.
- Add a narrow, evidence-backed override layer sourced from bundled JSON or synced BIR notices.
- Do not reinstate user CRUD for base rules.
- Require source reference, effective scope, affected canonical form codes, original deadline, adjusted deadline, and expiry.

### P2. eFPS staggered filing is not modeled by resolver context

Evidence:

- Old schema included `efps_group`: `crates/bir-core/src/db/migrations.rs:222`.
- New rules collapse withholding forms to a single non-eFPS/conservative description: `crates/bir-core/src/calendar_rules.rs:107`.
- `DeadlineResolver::resolve_year(year)` accepts only a year: `crates/bir-core/src/calendar_rules.rs:40`.

Impact:

- Moving base rules to static code does not inherently block eFPS support, but the current API does.
- The app cannot ask "what are deadlines for this taxpayer profile, channel, and eFPS group?"

Recommendation:

- Introduce `DeadlineContext` with taxpayer filing channel, optional eFPS group, region/RDO, holidays, and desired calendar mode.
- Make static rules emit variants or evaluate against that context.

### P2. Event-based forms are mixed into chronological deadlines with dummy dates

Evidence:

- Event-based forms are assigned `final_deadline: "{year}-12-31"`: `crates/bir-core/src/calendar_rules.rs:237`.
- Upcoming list computes urgency from `final_deadline`: `crates/bir-desktop/src/components/upcoming_deadlines_list.rs:66`.
- Calendar list renders them as dated entries: `crates/bir-desktop/src/views/admin_calendar_dashboard.rs:136`.

Impact:

- Estate tax, donor tax, capital gains, payment forms, and other transaction-triggered forms look like December 31 obligations.
- Urgency coloring and sorting are misleading.

Recommendation:

- Do not represent event-based obligations with fake dates.
- Model them as `DeadlineKind::EventBased { trigger, statutory_window }` and show them in a separate "Required Actions" or "Event-Based Forms" module.

### P2. Database cleanup is incomplete

Evidence:

- `TaxCalendar`, `TaxDeadlineRule`, `TaxDeadlineOverride`, and DB-backed `ResolvedTaxDeadline` Rust structs were removed from `db::mod`.
- However migration v6 still creates `tax_calendars`, `tax_forms`, `tax_deadline_rules`, `tax_deadline_overrides`, and `resolved_tax_deadlines`: `crates/bir-core/src/db/migrations.rs:188`.
- Legacy `tax_deadlines` CRUD still exists in notices repository: `crates/bir-core/src/db/notices.rs:14`.

Impact:

- New installs still get tables the app no longer uses.
- The schema communicates a database-backed calendar architecture that the code no longer supports.

Recommendation:

- Decide whether to keep a narrow override table.
- If not, remove unused calendar management tables from future fresh schema migrations, while preserving existing user databases safely.
- Document the legacy `tax_deadlines` table separately if it remains for notices/reminders.

### P2. Calendar rules have no regression tests

Evidence:

- No tests reference `DeadlineResolver`, `resolve_year`, or `OfficialRule`.

Impact:

- High-risk tax deadline behavior has no guardrails.
- Code-format mismatches and cross-year errors can recur silently.

Recommendation:

- Add unit tests for:
  - canonical form-code normalization.
  - annual AITR for Calendar Year 2025 appearing in the 2026 deadline calendar.
  - January deadlines for prior December periods.
  - 1701Q Q1/Q2/Q3 period-to-due-date mapping.
  - weekend adjustment preserving original deadline.
  - event-based forms not entering date-sorted deadline arrays.

## Answers To Review Questions

### 1. Handling ad-hoc overrides

Do not bring back user-editable CRUD for base rules. Do add a lightweight override layer. The right architecture is static base rules plus evidence-backed overrides with source references and constrained scope. BIR advisories, regional suspensions, holidays, and platform outages are exactly the cases where a desktop app should be able to update data without changing compiled code.

### 2. eFPS vs non-eFPS staggered filing

The static engine does not block eFPS support, but the current resolver signature does. The fix is not a DB-first calendar. The fix is a context-aware static rule engine that can resolve variants by taxpayer profile, eFPS group, filing channel, and region.

### 3. Performance

Synchronous generation of this small in-memory schedule is acceptable. The current risk is correctness, not CPU time. Memoization can be added later if render churn becomes measurable, but it should not be prioritized over model fixes and tests.

### 4. Event-based forms strategy

The dummy-date strategy is an anti-pattern. Event-based forms should bypass chronological deadline arrays unless the user creates or imports a real triggering event. The calendar can show a separate undated section, but the date-sorted deadline engine should not invent December 31 obligations.

## Recommended Next Implementation Order

1. Define a canonical deadline model with `form_code`, `display_form_no`, typed period metadata, original/final dates, status enum, and optional source reference.
2. Split resolver APIs into taxable-year and deadline-calendar-year modes.
3. Normalize all rule form codes against the temporal registry.
4. Fix dashboard filtering and click routing to use typed period metadata.
5. Move event-based forms out of dated schedule generation.
6. Add focused regression tests before expanding rule coverage.
7. Decide and document the narrow override mechanism.
8. Clean schema migrations after the override decision.

## Verification

- `cargo check -p bir-core` passed.
- `cargo check -p bir-desktop` passed.
- The working tree has mixed staged and unstaged calendar edits, so this report reflects the current working tree rather than only staged changes.
