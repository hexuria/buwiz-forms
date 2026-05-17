# Calendar Business-Day Deadline Adjustment

This note documents how the static BIR calendar engine adjusts deadlines that
fall on weekends, holidays, and configured non-working days.

## Rule Basis

- [BIR RMC No. 65-2016](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2065-2016.pdf)
  clarifies that when filing or payment due dates fall on a Saturday, Sunday,
  or holiday, the applicable due date moves to the next business day. The
  circular discusses eFPS, Online eBIRForms, and manual/offline filers.
- [BIR RR No. 13-2024](https://bir-cdn.bir.gov.ph/BIR/pdf/RR%2013-2024%282%29.pdf)
  applies the same next-working-day rule to extended due dates that fall on a
  holiday or non-working day.

Implementation implication: the engine must not stop at "add one day". It must
keep advancing until the candidate deadline is a working day.

## Core Model

The implementation lives in
`crates/bir-core/src/calendar_rules.rs`.

`ResolvedTaxDeadline` keeps both dates:

- `original_deadline`: the statutory or base-rule date.
- `final_deadline`: the date after weekend, holiday, non-working-day, and
  extension handling.

`BusinessDayCalendar` is the adjustment input:

- Saturdays and Sundays are always non-working days.
- Holidays, special non-working days, local holidays, and closures are explicit
  `NonWorkingDay` entries.
- The resolver repeatedly advances while the date is Saturday, Sunday, or a
  configured non-working day.

`DeadlineStatus` describes why the final deadline differs:

- `Normal`: original and final deadline are the same.
- `WeekendAdjusted`: the original deadline fell on a Saturday or Sunday.
- `HolidayAdjusted`: the original deadline fell on a configured regular/local
  holiday.
- `NonWorkingDayAdjusted`: the original deadline fell on a configured special
  non-working day or closure.
- `Extended`: a source-backed deadline override changed the due date.
- `EventBased`: the obligation has no concrete deadline until a triggering
  event exists.

## Resolver Order

For dated rules, resolution order is:

1. Generate static base deadline from the official rule.
2. Apply `BusinessDayCalendar` to the base `original_deadline`.
3. Apply matching `DeadlineOverride` records.
4. Apply `BusinessDayCalendar` again to the override's `adjusted_deadline`.

The second adjustment pass is intentional. If BIR extends a deadline to a date
that is also a holiday or non-working day, the final deadline still moves to
the next working day.

When an override applies, `DeadlineStatus` stays `Extended` even if the
override date is then moved by the business-day calendar. This preserves the
source-backed extension as the primary reason shown to the user while still
returning a business-day-safe `final_deadline`.

## Current Scope

The default resolver behavior includes weekends only. Configured holiday and
non-working-day awareness is available through:

- `DeadlineResolver::resolve_taxable_year_with_calendar(...)`
- `DeadlineResolver::resolve_taxable_year_with_overrides_and_calendar(...)`
- `DeadlineResolver::resolve_deadline_calendar_year_with_calendar(...)`
- `DeadlineResolver::resolve_deadline_calendar_year_with_overrides_and_calendar(...)`

Unknown national proclamations, local/RDO-specific holidays, force majeure
closures, and emergency advisories are not guessed by core. They must be
provided as `NonWorkingDay` entries or as source-backed deadline overrides.

## Examples

- `1702Q` Q1 2026: original deadline is `2026-05-30`; because that is a
  Saturday, the final deadline is `2026-06-01`.
- If a Saturday deadline moves to Monday, and that Monday is configured as a
  holiday, the final deadline moves to Tuesday.
- If a Friday deadline is configured as a holiday and the following two days
  are Saturday and Sunday, the final deadline moves to Monday.
- If an override changes a deadline to a configured holiday, the override is
  accepted, then the final deadline moves to the next working day.

## Regression Coverage

Targeted tests live in `crates/bir-core/src/calendar_rules.rs` and cover:

- Saturday deadline to Monday.
- Sunday deadline to Monday.
- Weekend followed by configured Monday holiday.
- Friday holiday followed by weekend.
- Special non-working day status.
- Override adjusted date landing on a holiday.
- `1702Q` Q1 2026 preserving original `2026-05-30` and final `2026-06-01`.

Useful verification commands:

```bash
rtk cargo test -p bir-core calendar_rules -- --nocapture
rtk cargo check -p bir-core
rtk cargo check -p bir-desktop
```
