# Data Migration & Versioning Guide

This guide explains how the eBIRForms Rust backend handles database schema upgrades and backwards-compatible JSON profile imports.

We have a robust system that ensures we never lose user data when the app updates, regardless of whether that data is stored in the local SQLite database or exported as a `.zip` profile backup.

---

## 1. SQL Database Migrations (`migrations.rs`)

The database engine uses `PRAGMA user_version` to track the current schema version. Migrations are strictly **sequential and forward-only**.

### When to add a SQL migration:
You need to add a SQL migration whenever you:
- Add a new table (e.g., `CREATE TABLE ...`)
- Add a new column to an existing table (e.g., `ALTER TABLE ... ADD COLUMN ...`)
- Add new indexes or constraints (e.g., `CREATE UNIQUE INDEX ...`)

### How to add a SQL migration:

1. Open `crates/bir-core/src/db/migrations.rs`.
2. Locate the `CURRENT_MIGRATION_VERSION` constant at the top and increment it by `1`.
3. Locate the `migrations` array inside `migrate_database()`.
4. Append your new SQL statement as the next item in the array.

**Example:**
```rust
const CURRENT_MIGRATION_VERSION: i32 = 4; // bumped from 3

// Inside `migrations` array:
let migrations = [
    // ... v1, v2, v3 ...
    // v4: Add a new column to profiles
    "ALTER TABLE profiles ADD COLUMN last_login TEXT;",
];
```

> **Note:** Do *not* edit past migrations unless you are absolutely sure it won't break existing databases. Migrations should generally only append new `ALTER` or `CREATE` statements.

### Current schema notes

- v6 is a static tax-calendar marker only. Fresh databases should not create the removed calendar CRUD tables (`tax_calendars`, `tax_forms`, `tax_deadline_rules`, `tax_deadline_overrides`, `resolved_tax_deadlines`).
- **v7 (Per-year Forms Set Table)**: Introduces the `per_year_forms` table, which serves as the user-owned, authoritative list of active tax forms for a given profile and taxable year, replacing the legacy rule-based temporal engine.
- **v8 (Per-year Forms Backfill)**: Performs a Rust-side data migration to populate the `per_year_forms` table for existing profile versions based on their registered tax types and obligation overrides.
- **v9 (Per-year Forms Heal)**: Re-runs the forms backfill to apply taxpayer type, VAT, deprecation, and other filters using `obligation_allowed_for_version_and_profile`, while preserving user-added custom forms and manual deactivations (active=0).
- Official recurring calendar rules live in `crates/bir-core/src/calendar_rules.rs`.
- Deadline adjustment is handled by the core `BusinessDayCalendar`: weekends are
  known by default, while holidays, local holidays, special non-working days,
  and closures must be supplied as configured non-working days or source-backed
  overrides. See `docs/calendar-business-day-adjustment.md`.
- Emergency or advisory deadline changes are applied through the typed override model in the calendar resolver, not through user-editable base-rule tables.
- Existing databases that already have legacy calendar tables keep them untouched; do not drop them without a dedicated backup and migration plan.

---

## 2. JSON Export & Import Versioning (`export.rs` / `import.rs`)

Because users can export their tax profiles to `.zip` files and import them later (potentially on a newer version of the app), our exported JSON files must also be versioned.

We automatically use the Cargo package version (`CARGO_PKG_VERSION`, e.g., `"0.0.1"`) as our canonical export version.

### How it works:
- **Exporting:** When a user exports a profile, `export.rs` writes a `manifest.json` file inside the ZIP. This manifest automatically records `"export_version": "0.0.1"` (matching the current Cargo version).
- **Importing:** When a user imports a profile, `import.rs` reads `manifest.json`. If the archive is older than the current app, it intercepts the raw JSON and runs it through a migration pipeline *before* trying to parse it into Rust structs.

### When to add a JSON migration:
You need to add a JSON migration whenever you:
- Rename a field in `TaxpayerProfile` or `Submission`.
- Change the data type of a field.
- Remove a field (if strict deserialization would fail).

*(Note: Simply adding a new field does **not** strictly require a JSON migration if the new field is annotated with `#[serde(default)]` in the Rust struct, as older JSON will just get the default value).*

### How to add a JSON migration:

1. Open `crates/bir-core/src/import.rs`.
2. Locate the `migrate_profile_json()` or `migrate_submission_json()` functions.
3. Add an `if from_version < (MAJOR, MINOR, PATCH)` block that manipulates the raw `serde_json::Value` to match the shape the current Rust structs expect.

**Example: Renaming a field**
Let's say in app version `0.2.0`, you renamed `rdo_code` to `rdo` in the `TaxpayerProfile` struct.

```rust
fn migrate_profile_json(
    value: &mut serde_json::Value,
    from_version: (u32, u32, u32),
) -> Result<(), DbError> {
    
    // If the archive is from an app older than 0.2.0
    if from_version < (0, 2, 0) {
        // Find the old "rdo_code" and rename it to "rdo"
        if let Some(old_val) = value.get("rdo_code").cloned() {
            value["rdo"] = old_val;
            value.as_object_mut().unwrap().remove("rdo_code");
        }
    }
    
    Ok(())
}
```

By doing this, an archive generated in v0.0.1 will cleanly load into v0.2.0, because the import logic rewrites the JSON in memory before `serde_json::from_value::<TaxpayerProfile>` is called.

---

## 3. Deduplication (Idempotent Imports)

Our import system is designed to be **idempotent**, meaning a user can import the same ZIP file 10 times and it will not create duplicate records.

- **Profiles** are deduplicated natively by the `tin` (Tax Identification Number).
- **Submissions** are deduplicated via a combination of `(tin, form_type, period, submitted_at)`.
- **Form Drafts** are deduplicated via `(tin, form_code, taxable_year, quarter)`.

If you add a new data type to the export bundle (e.g. `data_providers`), ensure the import logic uses an `INSERT ... ON CONFLICT DO UPDATE` (or a `SELECT 1` check) to prevent duplicates upon re-importing.
