# Forms Set Refactor: As-Built Architecture and Completion Checklist

Date: June 12, 2026

## Status

Implementation is complete. This document records the behavior that must remain
true and the verification gates required before the change is committed.

The legacy temporal suggestion engine is removed. Taxpayer obligations are
resolved from reviewed COR/profile evidence and a user-owned per-year Forms Set.

## Source Precedence

For a selected taxable year, the resolver applies this order:

1. Select the confirmed profile versions effective during the year.
2. If a version has reviewed COR evidence with exact extracted form codes, use
   those codes as the authoritative COR list.
3. Canonicalize display/OCR variants through `canonical_form_code()`.
4. Apply hard validity filters: taxpayer type, VAT status, deprecation year,
   registration status, elections, and classification-specific ITR rules.
5. Preserve explicit manual active additions and inactive suppressions in the
   persisted Forms Set.
6. Apply version-level include/exclude overrides.
7. Only when exact COR form codes are absent, derive candidates from registered
   tax types as a compatibility/manual-profile fallback.
8. Persist the confirmed result in `per_year_forms`.

The fallback must not invent conflicting annual returns. In particular, broad
income-tax mapping selects the classification-specific corporate annual return;
generic `1702` remains valid only when explicitly present in COR evidence or
manually included.

## Core Ownership

- `crates/bir-core/src/profile.rs`
  - Holds the effective-dated `TaxProfileVersion` ledger.
  - Hydrates `per_year_forms` separately from profile JSON.
  - Reconciles exact COR evidence, persisted manual edits, and overrides.
  - Exposes the closest eligible prior year for copy behavior.
- `crates/bir-core/src/forms/forms_set.rs`
  - Defines `FormSetSource`, `FormSetEntry`, and `PerYearFormsSet`.
- `crates/bir-core/src/forms/registry.rs`
  - Owns canonical form identity, filing frequency, taxpayer compatibility, and
    year-based deprecation metadata.
- `crates/bir-core/src/integration/validation.rs`
  - Resolves library forms and recurring obligations.
  - Keeps open-ended forms in the library while excluding them from recurring
    deadline generation.
  - Reports annual ITR conflicts and missing calendar-rule coverage.
- `crates/bir-core/src/forms/support_level.rs`
  - Owns static in-app draft/fileability metadata. It is not a tax-rule engine.
- `crates/bir-core/src/calendar_rules.rs`
  - Resolves deadlines only for recurring forms that survived applicability
    filtering.

## Database Model

The current schema version is 9.

- v7 creates `per_year_forms`.
- v8 backfills existing profiles from confirmed profile versions.
- v9 heals backfilled rows using current taxpayer, VAT, deprecation, and
  applicability filters while preserving manual additions and inactive rows.

Saving a profile reconciles current confirmed COR evidence into the selected
year without silently deleting user-owned manual decisions.

## UI Behavior

- The dashboard uses the per-year Forms Set for both the tax form library and
  recurring calendar obligations.
- Open-ended forms such as `0605` and `1905` appear in the library but do not
  create recurring due dates.
- Overdue items remain in the overdue section even when they also require user
  action; overdue classification takes priority.
- The forms editor supports manual activation/deactivation and records source.
- "Copy from prior year" appears only when the destination year has no entries.
  It selects the closest earlier year containing at least one active form.
- The compliance-source selector and temporal inspector are removed.

`ComplianceSourceMode` remains in the serialized profile model for backward
compatibility and internal confirmed-version behavior. It is not exposed as a
user choice and is not a source of tax-law applicability.

## Removed Components

- `crates/bir-core/src/temporal/`
- `crates/bir-core/data/temporal/`
- temporal snapshot compilation from `crates/bir-core/build.rs`
- obsolete temporal build dependencies
- `crates/bir-desktop/src/views/temporal_inspector.rs`
- temporal regression tests that exercised the deleted engine

Static support-level metadata was relocated before deletion and all consumers
now import it from `forms`.

## Regression Coverage

The refactor includes focused coverage for:

- exact COR codes taking precedence over broad tax-type mapping
- no invented `1702RT` when COR evidence contains only generic `1702`
- classification-specific fallback without duplicate corporate annual returns
- corporation/partnership vs individual form separation
- modern VAT years excluding deprecated monthly `2550M`
- open-ended forms retained in the library
- annual ITR mutual-exclusion reporting
- closest-prior-year copy selection and destination protection
- v7-v9 migration/backfill/heal behavior
- support-level and submission-queue allowlists
- deterministic 1601C XML generation independent of current overdue penalties

## Verification Gates

Run from the workspace root:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected result: all workspace-owned checks and tests pass. A future-
incompatibility warning emitted by the third-party `imap-proto` dependency is
external to this refactor and does not represent a project clippy failure.

Verified on June 12, 2026:

- `cargo fmt --all -- --check`: passed
- `cargo check --workspace`: 0 errors; one external `imap-proto` future-
  incompatibility warning
- `cargo test --workspace`: 234 passed, 2 ignored
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 errors and 0
  workspace-owned warnings
