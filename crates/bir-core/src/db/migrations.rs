//! Database schema migration engine.

use rusqlite::Connection;
use tracing::info;

use crate::db::DbError;

const CURRENT_MIGRATION_VERSION: i32 = 1;

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
                "Legacy database detected. Fast-forwarding schema version to {}",
                CURRENT_MIGRATION_VERSION
            );
            version = CURRENT_MIGRATION_VERSION;
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
