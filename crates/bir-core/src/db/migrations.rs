//! Database schema migration engine.

use rusqlite::Connection;
use tracing::info;

use crate::db::DbError;

const CURRENT_MIGRATION_VERSION: i32 = 5;

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
        } else {
            break;
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: opens an unencrypted in-memory SQLite connection for testing.
    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn
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
}
