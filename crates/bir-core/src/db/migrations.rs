//! Database schema migration engine.

use rusqlite::Connection;
use tracing::info;

use crate::db::DbError;

const CURRENT_MIGRATION_VERSION: i32 = 13;

pub(crate) fn migrate_database(conn: &Connection) -> Result<(), DbError> {
    let mut version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    if version == 0 {
        // Check if this is an existing unversioned database (legacy)
        let has_profiles: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='profiles'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if has_profiles {
            info!("Legacy database detected. Setting version to 1 to run incremental migrations.");
            // Set to v1 (not CURRENT) so v2+ migrations still execute sequentially.
            // v1 uses CREATE TABLE IF NOT EXISTS, so re-running it is safe.
            version = 1;
            conn.pragma_update(None, "user_version", version)?;
        }
    }

    let migrations = [
        // v1: The complete current schema
        "
        CREATE TABLE IF NOT EXISTS tax_deadlines (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            form_type TEXT NOT NULL,
            due_date TEXT NOT NULL,
            description TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS announcements (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            published_at TEXT NOT NULL,
            read_status BOOLEAN NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS bir_notices (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            external_id TEXT NOT NULL,
            source TEXT NOT NULL,
            source_kind TEXT NOT NULL,
            source_url TEXT,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            notice_type TEXT NOT NULL,
            rdo_code TEXT,
            form_code TEXT,
            deadline TEXT,
            image_url TEXT,
            posted_at TEXT,
            fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
            raw_json TEXT,
            read_status BOOLEAN NOT NULL DEFAULT 0,
            UNIQUE(source_kind, external_id)
        );
        CREATE INDEX IF NOT EXISTS idx_bir_notices_posted_at ON bir_notices(posted_at);
        CREATE INDEX IF NOT EXISTS idx_bir_notices_deadline ON bir_notices(deadline);
        CREATE INDEX IF NOT EXISTS idx_bir_notices_form_code ON bir_notices(form_code);
        CREATE INDEX IF NOT EXISTS idx_bir_notices_rdo_code ON bir_notices(rdo_code);

        CREATE TABLE IF NOT EXISTS penalties_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tin TEXT NOT NULL,
            form_type TEXT NOT NULL,
            period TEXT NOT NULL,
            penalty_amount REAL NOT NULL,
            reason TEXT NOT NULL,
            is_high_risk BOOLEAN NOT NULL DEFAULT 0,
            calculated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (tin) REFERENCES profiles(tin)
        );

        CREATE TABLE IF NOT EXISTS profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tin TEXT UNIQUE NOT NULL,
            data_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS submissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tin TEXT NOT NULL,
            form_type TEXT NOT NULL,
            period TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'draft',
            form_data TEXT NOT NULL,
            submitted_at TEXT,
            filename TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (tin) REFERENCES profiles(tin)
        );

        CREATE TABLE IF NOT EXISTS form_drafts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tin TEXT NOT NULL,
            form_code TEXT NOT NULL,
            taxable_year INTEGER NOT NULL,
            quarter INTEGER,
            status TEXT NOT NULL DEFAULT 'Draft',
            data_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(tin, form_code, taxable_year, quarter)
        );
        CREATE INDEX IF NOT EXISTS idx_form_drafts_tin_year ON form_drafts(tin, form_code, taxable_year);

        CREATE TABLE IF NOT EXISTS submission_receipts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            filename TEXT UNIQUE NOT NULL,
            tin TEXT NOT NULL,
            form_type TEXT NOT NULL,
            period TEXT NOT NULL,
            received_date TEXT NOT NULL,
            received_time TEXT NOT NULL,
            source_from TEXT,
            raw_text TEXT NOT NULL,
            raw_html TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS job_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            job_type TEXT NOT NULL DEFAULT 'Custom',
            cron_expr TEXT,
            command TEXT,
            status TEXT NOT NULL DEFAULT 'Queued',
            retries INTEGER NOT NULL DEFAULT 0,
            last_run_at TEXT,
            next_run_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            output_log TEXT
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
        // v2: Data Providers for integrations
        "
        CREATE TABLE IF NOT EXISTS data_providers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_tin TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            name TEXT NOT NULL,
            credentials_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (profile_tin) REFERENCES profiles(tin)
        );
        ",
        // v3: Add unique index on submissions for import deduplication.
        // Existing duplicate rows (from prior buggy imports) are cleaned up first.
        // Use COALESCE so NULL submitted_at values are treated as equal (SQLite NULL != NULL).
        "
        DELETE FROM submissions
        WHERE id NOT IN (
            SELECT MIN(id) FROM submissions
            GROUP BY tin, form_type, period, COALESCE(submitted_at, '')
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_submissions_dedup
            ON submissions(tin, form_type, period, COALESCE(submitted_at, ''));
        ",
        // v4: Promote legacy compat boolean flags from profile JSON into
        // normalized fields. This is a Rust-side data migration (no DDL change).
        // The SQL marker is a no-op that just advances the user_version.
        //
        // Handled below in `migrate_v4_compat_fields()` after the SQL loop.
        "SELECT 1; -- v4 marker: compat field promotion (Rust-side)",
        // v5: Add period_key column for unified period handling.
        // Column addition is handled in Rust (below) to be idempotent
        // since SQLite does not support ADD COLUMN IF NOT EXISTS.
        "SELECT 1; -- v5 marker: period_key column (Rust-side)",
        // v6: Static tax-calendar marker.
        //
        // The official recurring calendar now lives in `calendar_rules.rs` as
        // compiled base rules. Fresh databases should not create the removed
        // calendar CRUD tables (`tax_calendars`, `tax_deadline_rules`,
        // `tax_deadline_overrides`, `resolved_tax_deadlines`). Existing
        // databases keep those legacy tables untouched.
        "SELECT 1; -- v6 marker: static tax calendar rules",
        // v7: Per-year Forms Set — the user-owned, authoritative list of which forms a
        // taxpayer files in a given taxable year (replaces the temporal suggestion engine).
        // Populated from a COR (AI-assisted) or manually; read by the dashboard + deadlines.
        "
        CREATE TABLE IF NOT EXISTS per_year_forms (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tin TEXT NOT NULL,
            taxable_year INTEGER NOT NULL,
            form_code TEXT NOT NULL,
            frequency TEXT NOT NULL,
            active INTEGER NOT NULL DEFAULT 1,
            source TEXT NOT NULL,
            custom INTEGER NOT NULL DEFAULT 0,
            reason TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(tin, taxable_year, form_code)
        );
        CREATE INDEX IF NOT EXISTS idx_per_year_forms_tin_year
            ON per_year_forms(tin, taxable_year);
        ",
        "SELECT 1; -- v8 marker: per-year forms backfill (Rust-side)",
        // v9: Re-run per-year forms backfill with correct obligation_allowed filtering.
        // v8 only checked registered_tax_types_allow_form, missing taxpayer_type,
        // VAT, deprecation, and other checks from obligation_allowed_for_version_and_profile.
        "SELECT 1; -- v9 marker: per-year forms heal (Rust-side)",
        // v10: Google Calendar links and managed event mappings.
        "
        CREATE TABLE IF NOT EXISTS profile_calendar_links (
            profile_tin TEXT PRIMARY KEY,
            google_calendar_id TEXT NOT NULL,
            calendar_name TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            last_synced_at TEXT,
            last_error TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (profile_tin) REFERENCES profiles(tin) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS profile_calendar_events (
            profile_tin TEXT NOT NULL,
            obligation_key TEXT NOT NULL,
            google_event_id TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            taxable_year INTEGER NOT NULL,
            form_code TEXT NOT NULL,
            period_label TEXT NOT NULL,
            last_synced_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (profile_tin, obligation_key),
            FOREIGN KEY (profile_tin) REFERENCES profiles(tin) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_profile_calendar_events_tin
            ON profile_calendar_events(profile_tin);
        ",
        // v11: Preserve Forms Set evidence/effective dates and fail-closed review state.
        // Columns are added by Rust below so partially upgraded legacy databases remain safe.
        "SELECT 1; -- v11 marker: Forms Set provenance and conflict state (Rust-side)",
        // v12: Persist the effective-dated profile-version ledger once for
        // legacy profiles. Undated backfills remain review-blocked and do not
        // replace an existing user-owned Forms Set.
        "SELECT 1; -- v12 marker: durable profile-version ledger backfill (Rust-side)",
        // v13: Correct early v12 undated migration backfills that were
        // serialized as Confirmed even though they required date review.
        "SELECT 1; -- v13 marker: explicit NeedsReview profile-version status (Rust-side)",
    ];

    while version < CURRENT_MIGRATION_VERSION {
        let migration_idx = version as usize;
        if let Some(migration_sql) = migrations.get(migration_idx) {
            info!("Applying database migration v{}", version + 1);
            conn.execute_batch(migration_sql)?;
            version += 1;
            conn.pragma_update(None, "user_version", version)?;

            // v4: After the SQL marker is committed, run the Rust data migration.
            if version == 4 {
                migrate_v4_compat_fields(conn)?;
            }

            // v5: Backfill period_key from existing quarter column.
            if version == 5 {
                migrate_v5_backfill_period_key(conn)?;
            }

            // v8: Backfill per-year forms from profile versions.
            if version == 8 {
                migrate_v8_per_year_forms_backfill(conn)?;
            }

            // v9: Heal per-year forms with correct obligation filtering.
            if version == 9 {
                migrate_v9_per_year_forms_heal(conn)?;
            }

            if version == 11 {
                migrate_v11_forms_set_provenance(conn)?;
            }

            if version == 12 {
                migrate_v12_profile_version_ledger(conn)?;
            }

            if version == 13 {
                migrate_v13_profile_version_review_status(conn)?;
            }
        } else {
            break;
        }
    }

    Ok(())
}

/// Convert undated migration backfills from the early v12 `Confirmed` state
/// to the explicit fail-closed `NeedsReview` state.
///
/// This migration changes profile JSON only. Existing user-owned Forms Sets
/// are intentionally left untouched.
fn migrate_v13_profile_version_review_status(conn: &Connection) -> Result<(), DbError> {
    use crate::profile::TaxpayerProfile;

    let rows = {
        let mut stmt = conn.prepare("SELECT id, data_json FROM profiles")?;
        stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
    };

    let mut normalized = 0usize;
    for (id, data_json) in rows {
        let mut profile: TaxpayerProfile = match serde_json::from_str(&data_json) {
            Ok(profile) => profile,
            Err(error) => {
                tracing::warn!(
                    profile_id = id,
                    %error,
                    "v13 migration: profile JSON could not be normalized"
                );
                continue;
            }
        };
        if !profile.normalize_profile_version_review_statuses() {
            continue;
        }

        conn.execute(
            "UPDATE profiles SET data_json = ?1 WHERE id = ?2",
            rusqlite::params![serde_json::to_string(&profile)?, id],
        )?;
        normalized += 1;
    }

    if normalized > 0 {
        info!(
            "v13 migration: moved {} undated profile versions to NeedsReview",
            normalized
        );
    }
    Ok(())
}

/// Persist one compatibility profile-version record for legacy profile JSON.
///
/// This is deliberately a one-time migration rather than a resolver fallback.
/// A reliable business start date becomes the effective start. Otherwise the
/// version is retained as `NeedsReview` and resolution remains fail-closed.
/// Existing `per_year_forms` rows are left untouched.
fn migrate_v12_profile_version_ledger(conn: &Connection) -> Result<(), DbError> {
    use crate::profile::TaxpayerProfile;

    let rows = {
        let mut stmt = conn.prepare("SELECT id, data_json FROM profiles")?;
        stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
    };

    let mut backfilled = 0usize;
    for (id, data_json) in rows {
        let mut profile: TaxpayerProfile = match serde_json::from_str(&data_json) {
            Ok(profile) => profile,
            Err(error) => {
                tracing::warn!(
                    profile_id = id,
                    %error,
                    "v12 migration: profile JSON could not be backfilled"
                );
                continue;
            }
        };
        if !profile.profile_versions.is_empty() {
            continue;
        }

        profile.ensure_profile_version_ledger();
        conn.execute(
            "UPDATE profiles SET data_json = ?1 WHERE id = ?2",
            rusqlite::params![serde_json::to_string(&profile)?, id],
        )?;
        backfilled += 1;
    }

    if backfilled > 0 {
        info!(
            "v12 migration: persisted profile-version ledgers for {} legacy profiles",
            backfilled
        );
    }
    Ok(())
}

/// Add auditable suggestion provenance and explicit conflict state to `per_year_forms`.
///
/// Each addition is checked independently because early development builds may contain
/// only a subset of the columns. Existing rows default to `resolved`, preserving their
/// established manual/generated decision while new conflicts fail closed.
fn migrate_v11_forms_set_provenance(conn: &Connection) -> Result<(), DbError> {
    let has_per_year_forms: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='per_year_forms'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !has_per_year_forms {
        return Ok(());
    }

    let column_names = {
        let mut stmt = conn.prepare("PRAGMA table_info(per_year_forms)")?;
        stmt.query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?
    };

    for (column, definition) in [
        ("source_reference", "TEXT"),
        ("effective_from", "TEXT"),
        ("effective_until", "TEXT"),
        ("review_status", "TEXT NOT NULL DEFAULT 'resolved'"),
        ("conflict_json", "TEXT"),
    ] {
        if !column_names.contains(column) {
            conn.execute(
                &format!("ALTER TABLE per_year_forms ADD COLUMN {column} {definition}"),
                [],
            )?;
        }
    }

    info!("v11 migration: added Forms Set provenance and conflict state");
    Ok(())
}

/// v4 data migration: promote legacy compat boolean flags out of stored profile JSON.
///
/// Reads the old field names directly from the raw `serde_json::Value` because
/// the compat fields have been removed from `TaxpayerProfile`. Promotes:
/// - `opted_for_8_percent_flat_rate: true` → appends `TaxElectionHistory(EightPercent)`
/// - `imap_enabled: true` → sets `email_tracking_enabled = true`
///
/// Both transformations are idempotent.
fn migrate_v4_compat_fields(conn: &Connection) -> Result<(), DbError> {
    use crate::profile::{IncomeTaxElection, TaxElectionHistory, TaxpayerProfile};
    use chrono::Datelike as _;

    let rows: Vec<(i64, String)> = {
        let mut stmt = conn.prepare("SELECT id, data_json FROM profiles")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(DbError::Sqlite)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DbError::Sqlite)?
    };

    for (id, data_json) in rows {
        // Parse compat flags from raw JSON (the fields no longer exist on the typed struct)
        let raw: serde_json::Value = match serde_json::from_str(&data_json) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("v4 migration: failed to parse profile id={}: {}", id, e);
                continue;
            }
        };

        let opted_8pct = raw
            .get("opted_for_8_percent_flat_rate")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let imap_enabled_compat = raw
            .get("imap_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !opted_8pct && !imap_enabled_compat {
            continue;
        }

        let mut profile: TaxpayerProfile = match serde_json::from_str(&data_json) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "v4 migration: failed to deserialize profile id={}: {}",
                    id,
                    e
                );
                continue;
            }
        };

        let mut dirty = false;

        // Promote 8% compat flag
        if opted_8pct && profile.tax_elections.is_empty() {
            let retroactive_year = profile
                .business_start_date
                .map(|d| d.year() as u16)
                .unwrap_or_else(|| {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    let approx_year = (now.as_secs() / 31_557_600 + 1970) as u16;
                    approx_year.saturating_sub(1)
                });

            profile.tax_elections.push(TaxElectionHistory {
                taxable_year: retroactive_year,
                election: IncomeTaxElection::EightPercent,
                elected_at: chrono::NaiveDateTime::new(
                    chrono::NaiveDate::from_ymd_opt(retroactive_year as i32, 4, 15)
                        .unwrap_or_default(),
                    chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap_or_default(),
                ),
                source_form: "legacy_compat_migration_v4".into(),
            });
            tracing::info!(
                "v4 migration: promoted 8% election for profile id={} (year {})",
                id,
                retroactive_year
            );
            dirty = true;
        }

        // Promote imap_enabled compat flag
        if imap_enabled_compat && !profile.email_tracking_enabled {
            profile.email_tracking_enabled = true;
            tracing::info!(
                "v4 migration: promoted email_tracking_enabled for profile id={}",
                id
            );
            dirty = true;
        }

        if dirty {
            let updated_json =
                serde_json::to_string(&profile).map_err(|e| DbError::Other(e.to_string()))?;
            conn.execute(
                "UPDATE profiles SET data_json = ?1 WHERE id = ?2",
                rusqlite::params![updated_json, id],
            )
            .map_err(DbError::Sqlite)?;
        }
    }

    Ok(())
}

/// v5 data migration: add `period_key` column and backfill from the existing `quarter` column.
///
/// Uses form_code to determine the correct period format:
/// - Monthly forms (1601C, 0619E, 0619F, etc.) → `M{quarter:02}` (quarter holds month)
/// - Quarterly forms (2551Q, 1701Q, etc.) → `Q{quarter}`
/// - Annual/NULL quarter → `A`
fn migrate_v5_backfill_period_key(conn: &Connection) -> Result<(), DbError> {
    // Check if form_drafts table exists (some test scenarios skip v1)
    let has_form_drafts: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='form_drafts'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !has_form_drafts {
        return Ok(());
    }

    // Idempotent column addition: check PRAGMA table_info for `period_key`
    let has_period_key: bool = {
        let mut stmt = conn.prepare("PRAGMA table_info(form_drafts)")?;
        let col_names: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(DbError::Sqlite)?
            .filter_map(|r| r.ok())
            .collect();
        col_names.iter().any(|name| name == "period_key")
    };

    if !has_period_key {
        conn.execute_batch(
            "ALTER TABLE form_drafts ADD COLUMN period_key TEXT;
             CREATE INDEX IF NOT EXISTS idx_form_drafts_period_key
                 ON form_drafts(tin, form_code, taxable_year, period_key);",
        )?;
        info!("v5 migration: added period_key column to form_drafts");
    }

    // Monthly form codes that repurpose `quarter` as month
    let monthly_forms = [
        "1601C", "1601E", "1601F", "0619E", "0619F", "1600", "1600WP", "1602", "2550M", "2551M",
        "2200A", "2200AN", "2200M", "2200P", "2200T",
    ];
    let annual_forms = [
        "1604CF", "1604E", "1700", "1701", "1701A", "1701MS", "1702", "1702RT", "1702EX", "1702MX",
    ];
    let open_ended_forms = ["0605", "2000"];

    let rows: Vec<(i64, String, Option<i64>)> = {
        let mut stmt = conn
            .prepare("SELECT id, form_code, quarter FROM form_drafts WHERE period_key IS NULL")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(DbError::Sqlite)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DbError::Sqlite)?
    };

    let count = rows.len();

    for (id, form_code, quarter_opt) in rows {
        let period_key = match quarter_opt {
            Some(q) => {
                if open_ended_forms.iter().any(|m| *m == form_code) {
                    format!("O{}", q.max(1))
                } else if annual_forms.iter().any(|m| *m == form_code) {
                    "A".to_string()
                } else if monthly_forms.iter().any(|m| *m == form_code) {
                    format!("M{q:02}")
                } else {
                    format!("Q{}", q.clamp(1, 4))
                }
            }
            None => "A".to_string(),
        };

        conn.execute(
            "UPDATE form_drafts SET period_key = ?1 WHERE id = ?2",
            rusqlite::params![period_key, id],
        )
        .map_err(DbError::Sqlite)?;
    }

    if count > 0 {
        info!("v5 migration: backfilled period_key for {} rows", count);
    }

    Ok(())
}

fn migrate_v8_per_year_forms_backfill(conn: &Connection) -> Result<(), DbError> {
    use crate::forms::forms_set::{FormSetEntry, FormSetSource};
    use crate::forms::registry::{FORM_REGISTRY, FilingFrequency, find_form};
    use crate::profile::{ManualObligationOverrideAction, TaxpayerProfile};
    use chrono::Datelike as _;

    fn frequency_to_str_local(f: &FilingFrequency) -> &'static str {
        match f {
            FilingFrequency::Quarterly => "quarterly",
            FilingFrequency::Annual => "annual",
            FilingFrequency::Monthly => "monthly",
            FilingFrequency::OpenEnded => "open_ended",
        }
    }

    let rows: Vec<(i64, String)> = {
        let mut stmt = conn.prepare("SELECT id, data_json FROM profiles")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(DbError::Sqlite)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DbError::Sqlite)?
    };

    let mut backfilled_count = 0;

    for (id, data_json) in rows {
        let mut profile: TaxpayerProfile = match serde_json::from_str(&data_json) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "v8 migration: failed to deserialize profile id={}: {}",
                    id,
                    e
                );
                continue;
            }
        };

        profile.ensure_profile_version_ledger();
        let tin = profile.tin.full();
        let versions = profile.confirmed_profile_versions();
        let mut years = std::collections::BTreeSet::new();
        let current_year = chrono::Utc::now().year() as u16;
        for v in &versions {
            let start_year = v
                .effective_from
                .map(|d| d.year() as u16)
                .or_else(|| profile.business_start_date.map(|d| d.year() as u16))
                .unwrap_or(2018);
            let end_year = v
                .effective_until
                .map(|d| d.year() as u16)
                .unwrap_or(current_year);
            let end_year = end_year.max(current_year);
            for y in start_year..=end_year {
                years.insert(y);
            }
        }

        for year in years {
            let active_versions = profile.active_profile_versions_for_year(year);
            if let Some(version) = active_versions.last() {
                let mut entries = Vec::new();
                for def in FORM_REGISTRY {
                    if crate::integration::validation::registered_tax_types_allow_form(
                        version, def.code,
                    ) && crate::integration::validation::obligation_allowed_for_version_and_profile(
                        def, version, &profile, year,
                    ) {
                        entries.push(FormSetEntry::from_code(
                            def.code,
                            FormSetSource::MigrationBackfill,
                        ));
                    }
                }

                for r in &version.obligation_overrides {
                    if let Some(existing) = entries.iter_mut().find(|e| e.form_code == r.form_code)
                    {
                        match r.action {
                            ManualObligationOverrideAction::Include => {
                                existing.active = true;
                                existing.reason = Some(r.reason.clone());
                            }
                            ManualObligationOverrideAction::Exclude => {
                                existing.active = false;
                                existing.reason = Some(r.reason.clone());
                            }
                        }
                    } else {
                        let custom = find_form(&r.form_code).is_none();
                        let frequency = find_form(&r.form_code)
                            .map(|d| d.frequency.clone())
                            .unwrap_or(FilingFrequency::OpenEnded);
                        let mut entry = FormSetEntry::from_code(
                            r.form_code.clone(),
                            FormSetSource::MigrationBackfill,
                        );
                        entry.frequency = frequency;
                        entry.active = matches!(r.action, ManualObligationOverrideAction::Include);
                        entry.custom = custom;
                        entry.reason = Some(r.reason.clone());
                        entries.push(entry);
                    }
                }

                conn.execute(
                    "DELETE FROM per_year_forms WHERE tin = ?1 AND taxable_year = ?2",
                    rusqlite::params![tin, year],
                )
                .map_err(DbError::Sqlite)?;

                for entry in entries {
                    conn.execute(
                        "INSERT INTO per_year_forms
                         (tin, taxable_year, form_code, frequency, active, source, custom, reason)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        rusqlite::params![
                            tin,
                            year,
                            entry.form_code,
                            frequency_to_str_local(&entry.frequency),
                            entry.active as i64,
                            entry.source.as_str(),
                            entry.custom as i64,
                            entry.reason,
                        ],
                    )
                    .map_err(DbError::Sqlite)?;
                    backfilled_count += 1;
                }
            }
        }
    }

    if backfilled_count > 0 {
        info!(
            "v8 migration: backfilled {} forms in per_year_forms",
            backfilled_count
        );
    }

    Ok(())
}

/// v9 data migration: heal per_year_forms rows that were backfilled by v8 without
/// the `obligation_allowed_for_version_and_profile` check. This re-runs the same
/// logic as v8 but with both `registered_tax_types_allow_form` AND
/// `obligation_allowed_for_version_and_profile`, ensuring taxpayer_type, VAT,
/// deprecation, and other filters are applied. Existing manual overrides and
/// custom forms are preserved.
fn migrate_v9_per_year_forms_heal(conn: &Connection) -> Result<(), DbError> {
    use crate::forms::forms_set::{FormSetEntry, FormSetSource};
    use crate::forms::registry::{FORM_REGISTRY, FilingFrequency, find_form};
    use crate::profile::{ManualObligationOverrideAction, TaxpayerProfile};
    use chrono::Datelike as _;

    fn frequency_to_str_local(f: &FilingFrequency) -> &'static str {
        match f {
            FilingFrequency::Quarterly => "quarterly",
            FilingFrequency::Annual => "annual",
            FilingFrequency::Monthly => "monthly",
            FilingFrequency::OpenEnded => "open_ended",
        }
    }

    let rows: Vec<(i64, String)> = {
        let mut stmt = conn.prepare("SELECT id, data_json FROM profiles")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(DbError::Sqlite)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DbError::Sqlite)?
    };

    let mut healed_count = 0;

    for (_id, data_json) in rows {
        let mut profile: TaxpayerProfile = match serde_json::from_str(&data_json) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("v9 migration: failed to deserialize profile: {}", e);
                continue;
            }
        };

        profile.ensure_profile_version_ledger();
        let tin = profile.tin.full();
        let versions = profile.confirmed_profile_versions();
        let mut years = std::collections::BTreeSet::new();
        let current_year = chrono::Utc::now().year() as u16;
        for v in &versions {
            let start_year = v
                .effective_from
                .map(|d| d.year() as u16)
                .or_else(|| profile.business_start_date.map(|d| d.year() as u16))
                .unwrap_or(2018);
            let end_year = v
                .effective_until
                .map(|d| d.year() as u16)
                .unwrap_or(current_year);
            let end_year = end_year.max(current_year);
            for y in start_year..=end_year {
                years.insert(y);
            }
        }

        for year in years {
            let active_versions = profile.active_profile_versions_for_year(year);
            if let Some(version) = active_versions.last() {
                let mut entries = Vec::new();
                for def in FORM_REGISTRY {
                    if crate::integration::validation::registered_tax_types_allow_form(
                        version, def.code,
                    ) && crate::integration::validation::obligation_allowed_for_version_and_profile(
                        def, version, &profile, year,
                    ) {
                        entries.push(FormSetEntry::from_code(
                            def.code,
                            FormSetSource::MigrationBackfill,
                        ));
                    }
                }

                for r in &version.obligation_overrides {
                    if let Some(existing) = entries.iter_mut().find(|e| e.form_code == r.form_code)
                    {
                        match r.action {
                            ManualObligationOverrideAction::Include => {
                                existing.active = true;
                                existing.reason = Some(r.reason.clone());
                            }
                            ManualObligationOverrideAction::Exclude => {
                                existing.active = false;
                                existing.reason = Some(r.reason.clone());
                            }
                        }
                    } else {
                        let custom = find_form(&r.form_code).is_none();
                        let frequency = find_form(&r.form_code)
                            .map(|d| d.frequency.clone())
                            .unwrap_or(FilingFrequency::OpenEnded);
                        let mut entry = FormSetEntry::from_code(
                            r.form_code.clone(),
                            FormSetSource::MigrationBackfill,
                        );
                        entry.frequency = frequency;
                        entry.active = matches!(r.action, ManualObligationOverrideAction::Include);
                        entry.custom = custom;
                        entry.reason = Some(r.reason.clone());
                        entries.push(entry);
                    }
                }

                // Also preserve any user-added custom forms and manually deactivated standard forms from existing data
                let existing_preserved: Vec<(String, String, bool, String, Option<String>, bool)> = {
                    let mut stmt = conn.prepare(
                        "SELECT form_code, frequency, active, source, reason, custom FROM per_year_forms \
                         WHERE tin = ?1 AND taxable_year = ?2 AND (custom = 1 OR active = 0)",
                    )?;
                    let rows = stmt
                        .query_map(rusqlite::params![tin, year], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, bool>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, Option<String>>(4)?,
                                row.get::<_, bool>(5)?,
                            ))
                        })
                        .map_err(DbError::Sqlite)?;
                    rows.collect::<Result<Vec<_>, _>>()
                        .map_err(DbError::Sqlite)?
                };

                for (code, freq_str, active, source_str, reason, custom) in existing_preserved {
                    if let Some(existing) = entries.iter_mut().find(|e| e.form_code == code) {
                        if !active {
                            existing.active = false;
                            existing.reason = reason;
                        }
                    } else {
                        let frequency = match freq_str.as_str() {
                            "monthly" => FilingFrequency::Monthly,
                            "quarterly" => FilingFrequency::Quarterly,
                            "annual" => FilingFrequency::Annual,
                            _ => FilingFrequency::OpenEnded,
                        };
                        let mut entry = FormSetEntry::from_code(
                            code,
                            FormSetSource::from_str_lossy(&source_str),
                        );
                        entry.frequency = frequency;
                        entry.active = active;
                        entry.custom = custom;
                        entry.reason = reason;
                        entries.push(entry);
                    }
                }

                conn.execute(
                    "DELETE FROM per_year_forms WHERE tin = ?1 AND taxable_year = ?2",
                    rusqlite::params![tin, year],
                )
                .map_err(DbError::Sqlite)?;

                for entry in entries {
                    conn.execute(
                        "INSERT INTO per_year_forms
                         (tin, taxable_year, form_code, frequency, active, source, custom, reason)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        rusqlite::params![
                            tin,
                            year,
                            entry.form_code,
                            frequency_to_str_local(&entry.frequency),
                            entry.active as i64,
                            entry.source.as_str(),
                            entry.custom as i64,
                            entry.reason,
                        ],
                    )
                    .map_err(DbError::Sqlite)?;
                    healed_count += 1;
                }
            }
        }
    }

    if healed_count > 0 {
        info!(
            "v9 migration: healed {} forms in per_year_forms with correct obligation filtering",
            healed_count
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: opens an unencrypted in-memory SQLite connection for testing.
    fn test_conn() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn test_fresh_db_migrates_to_current_version() {
        let conn = test_conn();
        migrate_database(&conn).unwrap();
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, CURRENT_MIGRATION_VERSION);
    }

    #[test]
    fn test_fresh_db_creates_all_tables() {
        let conn = test_conn();
        migrate_database(&conn).unwrap();

        let tables = [
            "profiles",
            "submissions",
            "form_drafts",
            "submission_receipts",
            "job_queue",
            "settings",
            "bir_notices",
            "data_providers",
            "per_year_forms",
            "profile_calendar_links",
            "profile_calendar_events",
        ];
        for table in tables {
            let exists: bool = conn
                .query_row(
                    &format!(
                        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='{}'",
                        table
                    ),
                    [],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            assert!(exists, "Table '{}' should exist after migration", table);
        }
    }

    #[test]
    fn test_v10_forms_set_rows_gain_resolved_provenance_defaults() {
        let conn = test_conn();
        conn.execute_batch(
            "CREATE TABLE profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT UNIQUE NOT NULL,
                data_json TEXT NOT NULL
            );
            CREATE TABLE per_year_forms (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT NOT NULL,
                taxable_year INTEGER NOT NULL,
                form_code TEXT NOT NULL,
                frequency TEXT NOT NULL,
                active INTEGER NOT NULL DEFAULT 1,
                source TEXT NOT NULL,
                custom INTEGER NOT NULL DEFAULT 0,
                reason TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(tin, taxable_year, form_code)
            );
            INSERT INTO per_year_forms
                (tin, taxable_year, form_code, frequency, active, source, custom, reason)
            VALUES
                ('123456789000', 2026, '2551Q', 'quarterly', 1, 'reviewed_cor', 0,
                 'Reviewed before provenance migration');
            PRAGMA user_version = 10;",
        )
        .unwrap();

        migrate_database(&conn).unwrap();
        let row: (String, Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT review_status, source_reference, effective_from, conflict_json
                 FROM per_year_forms WHERE form_code = '2551Q'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert_eq!(row, ("resolved".into(), None, None, None));
    }

    #[test]
    fn test_v12_persists_legacy_profile_ledgers_without_touching_forms_sets() {
        use crate::profile::{TaxProfileVersionSource, TaxProfileVersionStatus, TaxpayerProfile};
        use chrono::NaiveDate;

        fn legacy_profile_json(
            full_name: &str,
            tin_segment: &str,
            business_start_date: Option<&str>,
        ) -> String {
            serde_json::json!({
                "id": null,
                "full_name": full_name,
                "tin": {
                    "segment1": tin_segment,
                    "segment2": "456",
                    "segment3": "789",
                    "branch": "000"
                },
                "rdo_code": "039",
                "line_of_business": "Consulting",
                "registered_address": "Quezon City",
                "zip_code": "1100",
                "phone": "09156837000",
                "email": "profile@example.com",
                "default_form_type": "2551Q",
                "taxpayer_type": "Individual",
                "is_vat_registered": false,
                "business_start_date": business_start_date,
                "compliance_source_mode": "CorVersioned"
            })
            .to_string()
        }

        let conn = test_conn();
        conn.execute_batch(
            "CREATE TABLE profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT UNIQUE NOT NULL,
                data_json TEXT NOT NULL
            );
            CREATE TABLE per_year_forms (
                tin TEXT NOT NULL,
                taxable_year INTEGER NOT NULL,
                form_code TEXT NOT NULL,
                reason TEXT
            );
            INSERT INTO per_year_forms (tin, taxable_year, form_code, reason)
            VALUES ('111456789000', 2026, '2551Q', 'User-owned decision');
            PRAGMA user_version = 11;",
        )
        .unwrap();

        let dated_json = legacy_profile_json("Dated Legacy", "111", Some("2020-04-15"));
        let undated_json = legacy_profile_json("Undated Legacy", "222", None);
        let mut already_versioned: TaxpayerProfile = serde_json::from_str(&legacy_profile_json(
            "Already Versioned",
            "333",
            Some("2021-01-01"),
        ))
        .unwrap();
        already_versioned.ensure_profile_version_ledger();
        already_versioned.profile_versions[0].id = "existing-version".to_string();
        let already_versioned_json = serde_json::to_string(&already_versioned).unwrap();

        for (tin, data_json) in [
            ("111456789000", dated_json),
            ("222456789000", undated_json),
            ("333456789000", already_versioned_json.clone()),
        ] {
            conn.execute(
                "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
                rusqlite::params![tin, data_json],
            )
            .unwrap();
        }

        migrate_database(&conn).unwrap();

        let load_profile = |tin: &str| -> TaxpayerProfile {
            let json: String = conn
                .query_row(
                    "SELECT data_json FROM profiles WHERE tin = ?1",
                    [tin],
                    |row| row.get(0),
                )
                .unwrap();
            serde_json::from_str(&json).unwrap()
        };

        let dated = load_profile("111456789000");
        assert_eq!(dated.profile_versions.len(), 1);
        let dated_version = &dated.profile_versions[0];
        assert_eq!(
            dated_version.source,
            TaxProfileVersionSource::MigrationBackfill
        );
        assert_eq!(dated_version.status, TaxProfileVersionStatus::Confirmed);
        assert_eq!(
            dated_version.effective_from,
            NaiveDate::from_ymd_opt(2020, 4, 15)
        );
        assert!(!dated_version.needs_effective_date_review);

        let undated = load_profile("222456789000");
        assert_eq!(undated.profile_versions.len(), 1);
        let undated_version = &undated.profile_versions[0];
        assert_eq!(
            undated_version.source,
            TaxProfileVersionSource::MigrationBackfill
        );
        assert_eq!(undated_version.status, TaxProfileVersionStatus::NeedsReview);
        assert_eq!(undated_version.effective_from, None);
        assert!(undated_version.needs_effective_date_review);

        let stored_versioned_json: String = conn
            .query_row(
                "SELECT data_json FROM profiles WHERE tin = '333456789000'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_versioned_json, already_versioned_json);

        let forms_row: (String, i64, String, String) = conn
            .query_row(
                "SELECT tin, taxable_year, form_code, reason FROM per_year_forms",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            forms_row,
            (
                "111456789000".to_string(),
                2026,
                "2551Q".to_string(),
                "User-owned decision".to_string()
            )
        );

        let before_second_run: Vec<String> = {
            let mut statement = conn
                .prepare("SELECT data_json FROM profiles ORDER BY tin")
                .unwrap();
            statement
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        migrate_database(&conn).unwrap();
        let after_second_run: Vec<String> = {
            let mut statement = conn
                .prepare("SELECT data_json FROM profiles ORDER BY tin")
                .unwrap();
            statement
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(before_second_run, after_second_run);
        assert_eq!(dated.profile_versions[0].id, "legacy-current-profile");
        assert_eq!(undated.profile_versions[0].id, "legacy-current-profile");
    }

    #[test]
    fn test_v13_normalizes_undated_confirmed_backfill_without_touching_forms_sets() {
        use crate::profile::{TaxProfileVersionStatus, TaxpayerProfile};

        let mut profile: TaxpayerProfile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Undated Versioned Legacy",
            "tin": {
                "segment1": "444",
                "segment2": "456",
                "segment3": "789",
                "branch": "000"
            },
            "rdo_code": "039",
            "line_of_business": "Consulting",
            "registered_address": "Quezon City",
            "zip_code": "1100",
            "phone": "09156837000",
            "email": "profile@example.com",
            "default_form_type": "2551Q",
            "taxpayer_type": "Individual",
            "is_vat_registered": false,
            "business_start_date": null,
            "compliance_source_mode": "CorVersioned"
        }))
        .unwrap();
        profile.ensure_profile_version_ledger();
        profile.profile_versions[0].status = TaxProfileVersionStatus::Confirmed;
        let legacy_v12_json = serde_json::to_string(&profile).unwrap();

        let conn = test_conn();
        conn.execute_batch(
            "CREATE TABLE profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT UNIQUE NOT NULL,
                data_json TEXT NOT NULL
            );
            CREATE TABLE per_year_forms (
                tin TEXT NOT NULL,
                taxable_year INTEGER NOT NULL,
                form_code TEXT NOT NULL,
                reason TEXT
            );
            INSERT INTO per_year_forms (tin, taxable_year, form_code, reason)
            VALUES ('444456789000', 2026, '2551Q', 'Manual include survives review migration');
            PRAGMA user_version = 12;",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
            rusqlite::params!["444456789000", legacy_v12_json],
        )
        .unwrap();

        migrate_database(&conn).unwrap();

        let migrated_json: String = conn
            .query_row(
                "SELECT data_json FROM profiles WHERE tin = '444456789000'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let migrated: TaxpayerProfile = serde_json::from_str(&migrated_json).unwrap();
        assert_eq!(
            migrated.profile_versions[0].status,
            TaxProfileVersionStatus::NeedsReview
        );
        assert!(migrated.profile_versions[0].needs_effective_date_review);
        assert!(migrated.confirmed_profile_versions().is_empty());

        let forms_row: (String, i64, String, String) = conn
            .query_row(
                "SELECT tin, taxable_year, form_code, reason FROM per_year_forms",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            forms_row,
            (
                "444456789000".to_string(),
                2026,
                "2551Q".to_string(),
                "Manual include survives review migration".to_string(),
            )
        );
    }

    #[test]
    fn test_v1_db_upgrades_to_current() {
        let conn = test_conn();

        // Simulate a v1 database: run only the first migration manually
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT UNIQUE NOT NULL,
                data_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS submissions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT NOT NULL,
                form_type TEXT NOT NULL,
                period TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'draft',
                form_data TEXT NOT NULL,
                submitted_at TEXT,
                filename TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 1i32).unwrap();

        // Run migrations — should apply v2 and v3
        migrate_database(&conn).unwrap();

        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, CURRENT_MIGRATION_VERSION);

        // Verify data_providers table from v2
        let has_dp: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='data_providers'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(
            has_dp,
            "data_providers table should exist after v2 migration"
        );

        // Verify dedup index from v3
        let has_idx: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_submissions_dedup'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(
            has_idx,
            "submissions dedup index should exist after v3 migration"
        );
    }

    #[test]
    fn test_legacy_db_runs_incremental_migrations() {
        let conn = test_conn();

        // Create a legacy (unversioned) database with profiles table but no version
        conn.execute_batch(
            "CREATE TABLE profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT UNIQUE NOT NULL,
                data_json TEXT NOT NULL
            );
            CREATE TABLE submissions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT NOT NULL,
                form_type TEXT NOT NULL,
                period TEXT NOT NULL,
                status TEXT NOT NULL,
                form_data TEXT NOT NULL,
                submitted_at TEXT,
                filename TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        // user_version is 0 by default (legacy)

        migrate_database(&conn).unwrap();

        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, CURRENT_MIGRATION_VERSION);

        // data_providers should have been created by v2 migration
        let has_dp: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='data_providers'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(
            has_dp,
            "Legacy DB should get data_providers after incremental migration"
        );
    }

    #[test]
    fn test_v3_migration_deduplicates_existing_submissions() {
        let conn = test_conn();

        // Simulate v2 database with duplicate submissions
        conn.execute_batch(
            "CREATE TABLE profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT UNIQUE NOT NULL,
                data_json TEXT NOT NULL
            );
            CREATE TABLE submissions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT NOT NULL,
                form_type TEXT NOT NULL,
                period TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'draft',
                form_data TEXT NOT NULL,
                submitted_at TEXT,
                filename TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE data_providers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                profile_tin TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                name TEXT NOT NULL,
                credentials_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            INSERT INTO submissions (tin, form_type, period, status, form_data) VALUES ('123', '2551Q', '2024Q1', 'Draft', '{}');
            INSERT INTO submissions (tin, form_type, period, status, form_data) VALUES ('123', '2551Q', '2024Q1', 'Draft', '{}');
            INSERT INTO submissions (tin, form_type, period, status, form_data) VALUES ('123', '2551Q', '2024Q2', 'Draft', '{}');",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 2i32).unwrap();

        // Count before
        let count_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM submissions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count_before, 3);

        // Run v3 migration
        migrate_database(&conn).unwrap();

        // Duplicates should be removed
        let count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM submissions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count_after, 2, "Duplicate submission should be removed");
    }

    #[test]
    fn test_idempotent_migration() {
        let conn = test_conn();
        migrate_database(&conn).unwrap();
        // Running migrations again should be a no-op
        migrate_database(&conn).unwrap();
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, CURRENT_MIGRATION_VERSION);
    }

    #[test]
    fn test_v4_promotes_8pct_compat_flag() {
        use crate::profile::{IncomeTaxElection, TaxpayerProfile};

        let conn = test_conn();

        // Run v1-v3 migrations to create the profiles table
        conn.execute_batch(
            "CREATE TABLE profiles (id INTEGER PRIMARY KEY AUTOINCREMENT, tin TEXT UNIQUE NOT NULL, data_json TEXT NOT NULL);",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 3i32).unwrap();

        // Insert a legacy profile with the old compat flag set to true
        let legacy_profile_json = serde_json::json!({
            "id": 1,
            "full_name": "Test Taxpayer",
            "tin": { "segment1": "123", "segment2": "456", "segment3": "789", "branch": "000" },
            "rdo_code": "039",
            "line_of_business": "Consulting",
            "registered_address": "QC",
            "zip_code": "1100",
            "phone": "09156837000",
            "email": "test@example.com",
            "default_form_type": "2551Q",
            "opted_for_8_percent_flat_rate": true,   // old compat field name
            "imap_enabled": true,                     // old compat field name
            "email_tracking_enabled": false,
            "tax_elections": []
        })
        .to_string();

        conn.execute(
            "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
            rusqlite::params!["123-456-789-000", legacy_profile_json],
        )
        .unwrap();

        // Run v4 migration
        migrate_database(&conn).unwrap();

        // Verify the profile was updated
        let updated_json: String = conn
            .query_row(
                "SELECT data_json FROM profiles WHERE tin = '123-456-789-000'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        let updated_profile: TaxpayerProfile = serde_json::from_str(&updated_json).unwrap();

        // 8% election should have been promoted to tax_elections
        assert!(
            !updated_profile.tax_elections.is_empty(),
            "tax_elections should have been populated by v4 migration"
        );
        assert!(
            matches!(
                updated_profile.tax_elections[0].election,
                IncomeTaxElection::EightPercent
            ),
            "election should be EightPercent"
        );
        assert_eq!(
            updated_profile.tax_elections[0].source_form,
            "legacy_compat_migration_v4"
        );

        // email_tracking should have been promoted
        assert!(
            updated_profile.email_tracking_enabled,
            "email_tracking_enabled should have been set to true"
        );
    }

    #[test]
    fn test_v8_migration_backfills_per_year_forms() {
        let conn = test_conn();

        // Initialize schema as if it is at v7
        conn.execute_batch(
            "CREATE TABLE profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT UNIQUE NOT NULL,
                data_json TEXT NOT NULL
            );
            CREATE TABLE per_year_forms (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT NOT NULL,
                taxable_year INTEGER NOT NULL,
                form_code TEXT NOT NULL,
                frequency TEXT NOT NULL,
                active INTEGER NOT NULL DEFAULT 1,
                source TEXT NOT NULL,
                custom INTEGER NOT NULL DEFAULT 0,
                reason TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(tin, taxable_year, form_code)
            );",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 7i32).unwrap();

        // Let's create a TaxpayerProfile with one confirmed version active in 2026.
        let profile_json = serde_json::json!({
            "id": 1,
            "full_name": "Test Taxpayer",
            "tin": { "segment1": "123", "segment2": "456", "segment3": "789", "branch": "000" },
            "rdo_code": "039",
            "line_of_business": "Consulting",
            "registered_address": "QC",
            "zip_code": "1100",
            "phone": "09156837000",
            "email": "test@example.com",
            "default_form_type": "2551Q",
            "taxpayer_type": "Individual",
            "is_vat_registered": false,
            "compliance_source_mode": "CorVersioned",
            "profile_versions": [
                {
                    "id": "v1",
                    "label": "Version 1",
                    "status": "Confirmed",
                    "source": "ManualCor",
                    "effective_from": "2026-01-01",
                    "effective_until": null,
                    "cor": {},
                    "registered_tax_types": ["IncomeTax", "PercentageTax"],
                    "taxpayer_type": "Individual",
                    "is_vat_registered": false,
                    "obligation_overrides": [
                        {
                            "form_code": "1701",
                            "action": "Include",
                            "reason": "Required manual include"
                        },
                        {
                            "form_code": "2551Q",
                            "action": "Exclude",
                            "reason": "Not filing monthly/quarterly percentage tax"
                        }
                    ]
                }
            ]
        })
        .to_string();

        conn.execute(
            "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
            rusqlite::params!["123-456-789-000", profile_json],
        )
        .unwrap();

        // Run the v8 migration
        migrate_database(&conn).unwrap();

        // Check if user_version reaches the current schema.
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, CURRENT_MIGRATION_VERSION);

        // Check that per_year_forms has been backfilled
        let mut stmt = conn.prepare(
            "SELECT form_code, active, reason FROM per_year_forms WHERE tin = '123456789000' AND taxable_year = 2026"
        ).unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? != 0,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .unwrap();

        let mut results = std::collections::HashMap::new();
        for r in rows {
            let (form, active, reason) = r.unwrap();
            results.insert(form, (active, reason));
        }

        // According to our logic:
        // - "1701" is an override to Include, so it should be active = true.
        // - "2551Q" is an override to Exclude, so it should be active = false.
        assert!(results.contains_key("1701"));
        assert!(results.get("1701").unwrap().0);
        assert_eq!(
            results.get("1701").unwrap().1.as_deref(),
            Some("Required manual include")
        );

        assert!(results.contains_key("2551Q"));
        assert!(!results.get("2551Q").unwrap().0);
        assert_eq!(
            results.get("2551Q").unwrap().1.as_deref(),
            Some("Not filing monthly/quarterly percentage tax")
        );
    }

    #[test]
    fn test_v9_migration_preserves_deactivated_standard_forms() {
        let conn = test_conn();

        // Initialize schema as if it is at v7
        conn.execute_batch(
            "CREATE TABLE profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT UNIQUE NOT NULL,
                data_json TEXT NOT NULL
            );
            CREATE TABLE per_year_forms (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT NOT NULL,
                taxable_year INTEGER NOT NULL,
                form_code TEXT NOT NULL,
                frequency TEXT NOT NULL,
                active INTEGER NOT NULL DEFAULT 1,
                source TEXT NOT NULL,
                custom INTEGER NOT NULL DEFAULT 0,
                reason TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(tin, taxable_year, form_code)
            );",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 8i32).unwrap();

        // Store standard form 2550Q with active = 0 and custom = 0 for year 2026
        conn.execute(
            "INSERT INTO per_year_forms (tin, taxable_year, form_code, frequency, active, source, custom, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "123456789000",
                2026,
                "2550Q",
                "quarterly",
                0, // active = 0
                "migration_backfill",
                0, // custom = 0
                "Manually deactivated standard form"
            ],
        )
        .unwrap();

        // Insert a profile version for 2026 where 2550Q is normally active/suggested
        // e.g. a VAT registered profile
        let profile_json = serde_json::json!({
            "id": 1,
            "full_name": "VAT Taxpayer",
            "tin": { "segment1": "123", "segment2": "456", "segment3": "789", "branch": "000" },
            "rdo_code": "039",
            "line_of_business": "Consulting",
            "registered_address": "QC",
            "zip_code": "1100",
            "phone": "09156837000",
            "email": "test@example.com",
            "default_form_type": "2550Q",
            "taxpayer_type": "Individual",
            "is_vat_registered": true,
            "compliance_source_mode": "CorVersioned",
            "profile_versions": [
                {
                    "id": "v1",
                    "label": "Version 1",
                    "status": "Confirmed",
                    "source": "ManualCor",
                    "effective_from": "2026-01-01",
                    "effective_until": null,
                    "cor": {},
                    "registered_tax_types": ["IncomeTax", "ValueAddedTax"],
                    "taxpayer_type": "Individual",
                    "is_vat_registered": true,
                    "obligation_overrides": []
                }
            ]
        })
        .to_string();

        conn.execute(
            "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
            rusqlite::params!["123-456-789-000", profile_json],
        )
        .unwrap();

        // Run the v9 migration (will upgrade from v7 -> v8 -> v9)
        migrate_database(&conn).unwrap();

        // Check if user_version reaches the current schema.
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, CURRENT_MIGRATION_VERSION);

        // Assert that after migration v9, 2550Q is preserved and active is still false (0)
        let mut stmt = conn.prepare(
            "SELECT form_code, active, custom, reason FROM per_year_forms WHERE tin = '123456789000' AND taxable_year = 2026 AND form_code = '2550Q'"
        ).unwrap();
        let mut rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .unwrap();

        let (form, active, custom, reason) = rows.next().unwrap().unwrap();
        assert_eq!(form, "2550Q");
        assert!(!active);
        assert!(!custom);
        assert_eq!(
            reason.as_deref(),
            Some("Manually deactivated standard form")
        );
    }

    #[test]
    fn test_v9_migration_preserves_custom_forms() {
        let conn = test_conn();

        // Initialize schema as if it is at v7
        conn.execute_batch(
            "CREATE TABLE profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT UNIQUE NOT NULL,
                data_json TEXT NOT NULL
            );
            CREATE TABLE per_year_forms (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT NOT NULL,
                taxable_year INTEGER NOT NULL,
                form_code TEXT NOT NULL,
                frequency TEXT NOT NULL,
                active INTEGER NOT NULL DEFAULT 1,
                source TEXT NOT NULL,
                custom INTEGER NOT NULL DEFAULT 0,
                reason TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(tin, taxable_year, form_code)
            );",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 8i32).unwrap();

        // Store custom form "9999" with active = 1 and custom = 1 for year 2026
        conn.execute(
            "INSERT INTO per_year_forms (tin, taxable_year, form_code, frequency, active, source, custom, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "123456789000",
                2026,
                "9999",
                "open_ended",
                1, // active = 1
                "user",
                1, // custom = 1
                "My custom tax form"
            ],
        )
        .unwrap();

        // Insert a profile version for 2026 where standard forms are suggested
        let profile_json = serde_json::json!({
            "id": 1,
            "full_name": "VAT Taxpayer",
            "tin": { "segment1": "123", "segment2": "456", "segment3": "789", "branch": "000" },
            "rdo_code": "039",
            "line_of_business": "Consulting",
            "registered_address": "QC",
            "zip_code": "1100",
            "phone": "09156837000",
            "email": "test@example.com",
            "default_form_type": "2550Q",
            "taxpayer_type": "Individual",
            "is_vat_registered": true,
            "compliance_source_mode": "CorVersioned",
            "profile_versions": [
                {
                    "id": "v1",
                    "label": "Version 1",
                    "status": "Confirmed",
                    "source": "ManualCor",
                    "effective_from": "2026-01-01",
                    "effective_until": null,
                    "cor": {},
                    "registered_tax_types": ["IncomeTax", "ValueAddedTax"],
                    "taxpayer_type": "Individual",
                    "is_vat_registered": true,
                    "obligation_overrides": []
                }
            ]
        })
        .to_string();

        conn.execute(
            "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
            rusqlite::params!["123-456-789-000", profile_json],
        )
        .unwrap();

        // Run the v9 migration
        migrate_database(&conn).unwrap();

        // Assert that after migration v9, custom form "9999" is preserved and active is true, custom is true
        let mut stmt = conn.prepare(
            "SELECT form_code, active, custom, reason FROM per_year_forms WHERE tin = '123456789000' AND taxable_year = 2026 AND form_code = '9999'"
        ).unwrap();
        let mut rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .unwrap();

        let (form, active, custom, reason) = rows.next().unwrap().unwrap();
        assert_eq!(form, "9999");
        assert!(active);
        assert!(custom);
        assert_eq!(reason.as_deref(), Some("My custom tax form"));
    }
}
