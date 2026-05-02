//! Database schema migration engine.

use rusqlite::Connection;
use tracing::info;

use crate::db::DbError;

const CURRENT_MIGRATION_VERSION: i32 = 3;

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
            info!(
                "Legacy database detected. Setting version to 1 to run incremental migrations."
            );
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
    ];

    while version < CURRENT_MIGRATION_VERSION {
        let migration_idx = version as usize;
        if let Some(migration_sql) = migrations.get(migration_idx) {
            info!("Applying database migration v{}", version + 1);
            conn.execute_batch(migration_sql)?;
            version += 1;
            conn.pragma_update(None, "user_version", version)?;
        } else {
            break;
        }
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
        assert!(has_dp, "data_providers table should exist after v2 migration");

        // Verify dedup index from v3
        let has_idx: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_submissions_dedup'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(has_idx, "submissions dedup index should exist after v3 migration");
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
        assert!(has_dp, "Legacy DB should get data_providers after incremental migration");
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
}
