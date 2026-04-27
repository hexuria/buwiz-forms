//! Encrypted SQLite database for taxpayer data.
//!
//! Stores profiles, form data, submission history.
//! Data is transparently AES-256 encrypted using SQLCipher.
//! Master key stored in OS keychain.

use rusqlite::{Connection, ErrorCode, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

use crate::forms::form_2551q::{
    FilingStatus, Form2551QDraft, FormDraftSummary, FormFilingProgress, QuarterState,
};
use crate::profile::TaxpayerProfile;
use crate::receipt::{BirReceiptConfirmation, split_bir_filename};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Option<i64>,
    pub name: String,
    pub job_type: String, // "System" or "Custom"
    pub cron_expr: Option<String>,
    pub command: Option<String>,
    pub status: String,
    pub retries: i64,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub created_at: String,
    pub output_log: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission {
    pub id: Option<i64>,
    pub tin: String,
    pub form_type: String,
    pub period: String,
    pub status: String,
    pub form_data: BTreeMap<String, String>,
    pub submitted_at: Option<String>,
    pub filename: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionReceipt {
    pub id: Option<i64>,
    pub filename: String,
    pub tin: String,
    pub form_type: String,
    pub period: String,
    pub received_date: String,
    pub received_time: String,
    pub source_from: Option<String>,
    pub raw_text: String,
    pub raw_html: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxDeadline {
    pub id: Option<i64>,
    pub form_type: String,
    pub due_date: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announcement {
    pub id: Option<i64>,
    pub source: String,
    pub title: String,
    pub content: String,
    pub published_at: String,
    pub read_status: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BirNotice {
    pub id: Option<i64>,
    pub external_id: String,
    pub source: String,
    pub source_kind: NoticeSourceKind,
    pub source_url: Option<String>,
    pub title: String,
    pub body: String,
    pub notice_type: NoticeType,
    pub rdo_code: Option<String>,
    pub form_code: Option<String>,
    pub deadline: Option<String>, // NaiveDate format YYYY-MM-DD
    pub image_url: Option<String>,
    pub posted_at: Option<String>,
    pub fetched_at: String,
    pub raw_json: Option<String>,
    pub read_status: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NoticeSourceKind {
    BirCms,
    Rss,
    Manual,
    FacebookGraph,
}

impl NoticeSourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NoticeSourceKind::BirCms => "BirCms",
            NoticeSourceKind::Rss => "Rss",
            NoticeSourceKind::Manual => "Manual",
            NoticeSourceKind::FacebookGraph => "FacebookGraph",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "BirCms" => NoticeSourceKind::BirCms,
            "Manual" => NoticeSourceKind::Manual,
            "FacebookGraph" => NoticeSourceKind::FacebookGraph,
            _ => NoticeSourceKind::Rss,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NoticeType {
    EbirFormsVersion,
    Deadline,
    TaxCalendar,
    RdoAdvisory,
    SystemAdvisory,
    General,
}

impl NoticeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NoticeType::EbirFormsVersion => "EbirFormsVersion",
            NoticeType::Deadline => "Deadline",
            NoticeType::TaxCalendar => "TaxCalendar",
            NoticeType::RdoAdvisory => "RdoAdvisory",
            NoticeType::SystemAdvisory => "SystemAdvisory",
            NoticeType::General => "General",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "EbirFormsVersion" => NoticeType::EbirFormsVersion,
            "Deadline" => NoticeType::Deadline,
            "TaxCalendar" => NoticeType::TaxCalendar,
            "RdoAdvisory" => NoticeType::RdoAdvisory,
            "SystemAdvisory" => NoticeType::SystemAdvisory,
            _ => NoticeType::General,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenaltyCache {
    pub id: Option<i64>,
    pub tin: String,
    pub form_type: String,
    pub period: String,
    pub penalty_amount: f64,
    pub reason: String,
    pub is_high_risk: bool,
    pub calculated_at: String,
}

#[derive(thiserror::Error, Debug)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Keychain error: {0}")]
    Keychain(#[from] keyring::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Encryption key invalid or database corrupted")]
    Encryption,
    #[error("Keychain CLI error: {0}")]
    KeychainCli(String),
    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Other error: {0}")]
    Other(String),
}

pub struct Database {
    pub conn: Connection,
}

pub fn default_database_path() -> std::path::PathBuf {
    crate::platform::data_dir().join("bir_data.db")
}

impl Database {
    /// Opens the database, quarantining an unreadable existing file and
    /// recreating it when the on-disk contents are not a usable SQLCipher DB.
    pub fn open_or_recreate<P: AsRef<Path>>(path: P) -> Result<(Self, Option<PathBuf>), DbError> {
        let path = path.as_ref();

        match Self::open(path) {
            Ok(db) => Ok((db, None)),
            Err(err) if path.exists() && Self::should_recreate_after_open_error(&err) => {
                let backup_path = Self::quarantine_database_file(path)?;
                Self::cleanup_old_backups(path, 3);
                let db = Self::open(path)?;
                Ok((db, Some(backup_path)))
            }
            Err(err) => Err(err),
        }
    }

    /// Opens the database and initializes SQLCipher using the OS keychain.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;

        let key_hex = Self::get_or_create_master_key()?;

        // Initialize SQLCipher transparent encryption
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key_hex))?;

        // Improve concurrency and wait for locks instead of failing immediately (fixes UI freezes)
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;

        // Verify the key by trying to read from sqlite_master
        {
            let mut check_stmt = conn.prepare("SELECT count(*) FROM sqlite_master")?;
            let _: i64 = check_stmt.query_row([], |r| r.get(0)).map_err(|e| {
                if let rusqlite::Error::SqliteFailure(err, _) = &e {
                    // Only treat it as an encryption error if it's explicitly NotADatabase
                    if err.code == ErrorCode::NotADatabase {
                        return DbError::Encryption;
                    }
                }
                DbError::Sqlite(e)
            })?;
        }

        // Initialize Schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tax_deadlines (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                form_type TEXT NOT NULL,
                due_date TEXT NOT NULL,
                description TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS announcements (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source TEXT NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                published_at TEXT NOT NULL,
                read_status BOOLEAN NOT NULL DEFAULT 0
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS bir_notices (
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
            )",
            [],
        )?;

        conn.execute_batch("
            INSERT INTO bir_notices (external_id, source, source_kind, title, body, notice_type, posted_at, read_status)
            SELECT
                'legacy-' || id,
                source,
                'Rss',
                title,
                content,
                'General',
                published_at,
                read_status
            FROM announcements
            WHERE NOT EXISTS (SELECT 1 FROM bir_notices WHERE external_id = 'legacy-' || announcements.id);

            CREATE INDEX IF NOT EXISTS idx_bir_notices_posted_at ON bir_notices(posted_at);
            CREATE INDEX IF NOT EXISTS idx_bir_notices_deadline ON bir_notices(deadline);
            CREATE INDEX IF NOT EXISTS idx_bir_notices_form_code ON bir_notices(form_code);
            CREATE INDEX IF NOT EXISTS idx_bir_notices_rdo_code ON bir_notices(rdo_code);
        ")?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS penalties_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT NOT NULL,
                form_type TEXT NOT NULL,
                period TEXT NOT NULL,
                penalty_amount REAL NOT NULL,
                reason TEXT NOT NULL,
                is_high_risk BOOLEAN NOT NULL DEFAULT 0,
                calculated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (tin) REFERENCES profiles(tin)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT UNIQUE NOT NULL,
                data_json TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS submissions (
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
            )",
            [],
        )?;

        // Form drafts table — one row per (tin, form_code, year, quarter) slot
        conn.execute(
            "CREATE TABLE IF NOT EXISTS form_drafts (
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
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_form_drafts_tin_year
             ON form_drafts(tin, form_code, taxable_year)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS submission_receipts (
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
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS job_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                job_type TEXT NOT NULL DEFAULT 'Custom',
                cron_expr TEXT,
                command TEXT,
                status TEXT NOT NULL DEFAULT 'Queued',
                retries INTEGER NOT NULL DEFAULT 0,
                last_run_at TEXT,
                next_run_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        // Silent migration for existing users
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE job_queue ADD COLUMN job_type TEXT NOT NULL DEFAULT 'Custom'",
            [],
        );
        let _ = conn.execute("ALTER TABLE job_queue ADD COLUMN output_log TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE submission_receipts ADD COLUMN raw_html TEXT",
            [],
        );

        Ok(Self { conn })
    }

    fn should_recreate_after_open_error(err: &DbError) -> bool {
        match err {
            DbError::Encryption => true,
            DbError::Sqlite(rusqlite::Error::SqliteFailure(sql_err, _)) => {
                sql_err.code == ErrorCode::NotADatabase
            }
            _ => false,
        }
    }

    pub fn get_or_create_master_key() -> Result<String, DbError> {
        use keyring::Entry;
        use tracing::{info, warn};

        // By using `keyring` on macOS with the `apple-native` feature, this calls `SecItemAdd` and `SecItemCopyMatching` natively.
        // In a sandboxed App Store environment, this will automatically use the app's Keychain Access Group entitlement.
        let entry = Entry::new("com.ebir.rust", "sqlcipher_master_key")?;

        // Verify we have a real credential store, not the in-memory mock
        #[cfg(debug_assertions)]
        {
            let test_entry = Entry::new("com.ebir.rust", "__keyring_test__")?;
            let _ = test_entry.set_password("test");
            match test_entry.get_password() {
                Ok(v) if v == "test" => {
                    let _ = test_entry.delete_credential();
                    info!("Keyring backend: native credential store confirmed");
                }
                _ => {
                    warn!(
                        "Keyring backend appears to be a mock store! \
                         Encryption keys will NOT persist across restarts."
                    );
                }
            }
        }

        match entry.get_password() {
            Ok(hex_key) => {
                info!("Loaded existing master key from native keychain");
                Ok(hex_key)
            }
            Err(_) => {
                info!("Generating new master key and storing in native keychain");
                let key: [u8; 32] = rand::random();
                let hex_key = hex::encode(key);
                entry.set_password(&hex_key)?;
                Ok(hex_key)
            }
        }
    }

    fn quarantine_database_file(path: &Path) -> Result<PathBuf, DbError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "bir_data.db".to_string());
        let backup_path = path.with_file_name(format!("{file_name}.corrupt-{timestamp}.bak"));

        std::fs::rename(path, &backup_path)?;

        Ok(backup_path)
    }

    /// Checkpoint WAL without consuming self.
    /// Flushes all data from the WAL file into the main database file.
    pub fn checkpoint(&self) -> Result<(), DbError> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    /// Consume self and close cleanly after a WAL checkpoint.
    pub fn close(self) -> Result<(), DbError> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        self.conn.close().map_err(|(_, e)| DbError::Sqlite(e))
    }

    /// Remove old `.corrupt-*.bak` files, keeping only the most recent `keep`.
    fn cleanup_old_backups(db_path: &Path, keep: usize) {
        let Some(dir) = db_path.parent() else {
            return;
        };
        let db_name = db_path.file_name().unwrap_or_default().to_string_lossy();
        let prefix = format!("{}.corrupt-", db_name);

        let mut backups: Vec<_> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
            .collect();

        // Sort by name (timestamp is embedded) — oldest first
        backups.sort_by_key(|e| e.file_name());

        if backups.len() > keep {
            for entry in &backups[..backups.len() - keep] {
                let _ = std::fs::remove_file(entry.path());
                info!("Cleaned up old backup: {}", entry.path().display());
            }
        }
    }

    /// Factory Reset: Deletes all data from the database.
    pub fn factory_reset(&self) -> Result<(), DbError> {
        self.conn.execute_batch(
            "DELETE FROM tax_deadlines;
             DELETE FROM announcements;
             DELETE FROM bir_notices;
             DELETE FROM penalties_cache;
             DELETE FROM profiles;
             DELETE FROM submissions;
             DELETE FROM form_drafts;
             DELETE FROM submission_receipts;
             DELETE FROM job_queue;
             DELETE FROM settings;",
        )?;
        // Also vacuum to reclaim space and shrink file
        self.conn.execute_batch("VACUUM;")?;

        // Clean up temporary PDF directory
        let temp_pdf_dir = std::env::temp_dir().join("taxman-ebir-pdf");
        if temp_pdf_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_pdf_dir);
        }

        Ok(())
    }

    /// Retrieve a global setting value by key.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    /// Set a global setting value.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    /// Saves a taxpayer profile (insert or replace).
    pub fn save_profile(&self, mut profile: TaxpayerProfile) -> Result<TaxpayerProfile, DbError> {
        let json_data = serde_json::to_string(&profile)?;
        let tin = profile.tin.full();

        if let Some(id) = profile.id {
            let updated = self.conn.execute(
                "UPDATE profiles SET tin = ?1, data_json = ?2 WHERE id = ?3",
                params![tin, json_data, id],
            )?;
            if updated == 0 {
                self.conn.execute(
                    "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
                    params![tin, json_data],
                )?;
                profile.id = Some(self.conn.last_insert_rowid());
            }
        } else if let Some(existing) = self.get_profile(&tin)? {
            profile.id = existing.id;
            let json_data = serde_json::to_string(&profile)?;
            self.conn.execute(
                "UPDATE profiles SET data_json = ?1 WHERE tin = ?2",
                params![json_data, tin],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
                params![tin, json_data],
            )?;
            profile.id = Some(self.conn.last_insert_rowid());
        }

        Ok(profile)
    }

    /// Get a profile by TIN.
    pub fn get_profile(&self, tin: &str) -> Result<Option<TaxpayerProfile>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json, id FROM profiles WHERE tin = ?1")?;
        let mut rows = stmt.query(params![tin])?;

        if let Some(row) = rows.next()? {
            let json_data: String = row.get(0)?;
            let mut profile: TaxpayerProfile = serde_json::from_str(&json_data)?;
            profile.id = row.get(1).ok();
            Ok(Some(profile))
        } else {
            Ok(None)
        }
    }

    /// List all profiles.
    pub fn list_profiles(&self) -> Result<Vec<TaxpayerProfile>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, data_json FROM profiles ORDER BY id ASC")?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let json_data: String = row.get(1)?;
            Ok((id, json_data))
        })?;

        let mut profiles = Vec::new();
        for row_result in rows {
            let (id, json_data) = row_result?;
            let mut profile: TaxpayerProfile = serde_json::from_str(&json_data)?;
            profile.id = Some(id);
            profiles.push(profile);
        }

        Ok(profiles)
    }

    /// Get a single profile by its TIN.
    pub fn get_profile_by_tin(&self, tin: &str) -> Result<Option<TaxpayerProfile>, DbError> {
        let profiles = self.list_profiles()?;
        Ok(profiles.into_iter().find(|p| p.tin.full() == tin))
    }

    /// Delete a profile by TIN.
    pub fn delete_profile(&self, tin: &str) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM profiles WHERE tin = ?1", params![tin])?;
        Ok(())
    }

    // =========================================================================
    // Job Queue
    // =========================================================================

    pub fn save_job(&self, mut job: Job) -> Result<Job, DbError> {
        if let Some(id) = job.id {
            self.conn.execute(
                "UPDATE job_queue SET name = ?1, job_type = ?2, cron_expr = ?3, command = ?4, status = ?5, retries = ?6, last_run_at = ?7, next_run_at = ?8, output_log = ?9 WHERE id = ?10",
                params![job.name, job.job_type, job.cron_expr, job.command, job.status, job.retries, job.last_run_at, job.next_run_at, job.output_log, id],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO job_queue (name, job_type, cron_expr, command, status, retries, last_run_at, next_run_at, output_log) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![job.name, job.job_type, job.cron_expr, job.command, job.status, job.retries, job.last_run_at, job.next_run_at, job.output_log],
            )?;
            job.id = Some(self.conn.last_insert_rowid());

            // Re-fetch to get the created_at timestamp
            let mut stmt = self
                .conn
                .prepare("SELECT created_at FROM job_queue WHERE id = ?1")?;
            if let Ok(mut rows) = stmt.query(params![job.id]) {
                if let Ok(Some(row)) = rows.next() {
                    job.created_at = row.get(0).unwrap_or_default();
                }
            }
        }
        Ok(job)
    }

    pub fn list_jobs(&self) -> Result<Vec<Job>, DbError> {
        let mut stmt = self.conn.prepare("SELECT id, name, job_type, cron_expr, command, status, retries, last_run_at, next_run_at, created_at, output_log FROM job_queue ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Job {
                id: row.get(0)?,
                name: row.get(1)?,
                job_type: row.get(2).unwrap_or_else(|_| "Custom".to_string()),
                cron_expr: row.get(3)?,
                command: row.get(4)?,
                status: row.get(5)?,
                retries: row.get(6)?,
                last_run_at: row.get(7)?,
                next_run_at: row.get(8)?,
                created_at: row.get(9)?,
                output_log: row.get(10).unwrap_or(None),
            })
        })?;
        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }

    pub fn delete_job(&self, id: i64) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM job_queue WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_archived_jobs(&self) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM job_queue WHERE status = 'Archived'", [])?;
        Ok(())
    }

    /// Save a submission.
    pub fn save_submission(&self, mut sub: Submission) -> Result<Submission, DbError> {
        let json_data = serde_json::to_string(&sub.form_data)?;

        if let Some(id) = sub.id {
            self.conn.execute(
                "UPDATE submissions SET form_type = ?1, period = ?2, status = ?3, form_data = ?4, submitted_at = ?5, filename = ?6, updated_at = datetime('now') WHERE id = ?7",
                params![sub.form_type, sub.period, sub.status, json_data, sub.submitted_at, sub.filename, id],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO submissions (tin, form_type, period, status, form_data, submitted_at, filename) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![sub.tin, sub.form_type, sub.period, sub.status, json_data, sub.submitted_at, sub.filename],
            )?;
            sub.id = Some(self.conn.last_insert_rowid());
        }

        Ok(sub)
    }

    /// List submissions for a specific TIN.
    pub fn list_submissions_for_tin(&self, tin: &str) -> Result<Vec<Submission>, DbError> {
        let mut stmt = self.conn.prepare("SELECT id, tin, form_type, period, status, form_data, submitted_at, filename, created_at, updated_at FROM submissions WHERE tin = ?1 ORDER BY created_at DESC")?;
        let rows = stmt.query_map(params![tin], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        })?;

        let mut submissions = Vec::new();
        for row_result in rows {
            let (
                id,
                tin,
                form_type,
                period,
                status,
                json_data,
                submitted_at,
                filename,
                created_at,
                updated_at,
            ) = row_result?;
            let form_data: BTreeMap<String, String> = serde_json::from_str(&json_data)?;

            submissions.push(Submission {
                id: Some(id),
                tin,
                form_type,
                period,
                status,
                form_data,
                submitted_at,
                filename,
                created_at,
                updated_at,
            });
        }
        Ok(submissions)
    }

    // =========================================================================
    // Form Drafts
    // =========================================================================

    /// Save or update a Form 2551Q draft.
    /// Uses UPSERT on (tin, form_code, taxable_year, quarter).
    pub fn save_2551q_draft(&self, draft: &Form2551QDraft) -> Result<i64, DbError> {
        let json = serde_json::to_string(draft)?;
        let status = match draft.status {
            FilingStatus::Draft => "Draft",
            FilingStatus::Queued => "Queued",
            FilingStatus::Submitted => "Submitted",
            FilingStatus::Confirmed => "Confirmed",
            FilingStatus::Paid => "Paid",
        };
        let quarter = draft.quarter as i64;

        self.conn.execute(
            "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, status, data_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(tin, form_code, taxable_year, quarter)
             DO UPDATE SET status = excluded.status,
                           data_json = excluded.data_json,
                           updated_at = datetime('now')",
            params![
                draft.tin,
                "2551Q",
                draft.taxable_year as i64,
                quarter,
                status,
                json
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        Ok(id)
    }

    /// Load a 2551Q draft for a specific (tin, year, quarter).
    /// Returns None if no draft exists for that slot.
    pub fn get_2551q_draft(
        &self,
        tin: &str,
        year: u16,
        quarter: u8,
    ) -> Result<Option<Form2551QDraft>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT data_json FROM form_drafts
             WHERE tin = ?1 AND form_code = '2551Q'
               AND taxable_year = ?2 AND quarter = ?3",
        )?;
        let mut rows = stmt.query(params![tin, year as i64, quarter as i64])?;
        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            let draft: Form2551QDraft = serde_json::from_str(&json)?;
            Ok(Some(draft))
        } else {
            Ok(None)
        }
    }

    /// Mark a 2551Q draft as Filed.
    pub fn mark_2551q_filed(&self, tin: &str, year: u16, quarter: u8) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE form_drafts SET status = 'Submitted', updated_at = datetime('now')
             WHERE tin = ?1 AND form_code = '2551Q'
               AND taxable_year = ?2 AND quarter = ?3",
            params![tin, year as i64, quarter as i64],
        )?;
        Ok(())
    }

    /// Save an imported form directly to the form_drafts table to show up in Dashboard.
    pub fn save_imported_form(
        &self,
        tin: &str,
        form_code: &str,
        year: u16,
        quarter: Option<u8>,
        month: Option<u8>,
    ) -> Result<i64, DbError> {
        // We multiplex the `quarter` column to hold either month or quarter depending on form frequency.
        // For Dashboard compatibility, the UI just filters based on form frequency.
        let q_or_m = quarter.or(month).map(|v| v as i64);

        self.conn.execute(
            "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, status, data_json)
             VALUES (?1, ?2, ?3, ?4, 'Submitted', '{}')
             ON CONFLICT(tin, form_code, taxable_year, quarter)
             DO UPDATE SET status = 'Submitted', updated_at = datetime('now')",
            params![tin, form_code, year as i64, q_or_m],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Get filing progress for a form in a given year.
    /// Returns a FormFilingProgress with per-quarter states.
    pub fn get_form_filing_progress(
        &self,
        tin: &str,
        form_code: &str,
        year: u16,
    ) -> Result<FormFilingProgress, DbError> {
        let mut progress = FormFilingProgress::new_empty(form_code, year);

        let mut stmt = self.conn.prepare(
            "SELECT quarter, status FROM form_drafts
             WHERE tin = ?1 AND form_code = ?2 AND taxable_year = ?3",
        )?;
        let rows = stmt.query_map(params![tin, form_code, year as i64], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, String>(1)?))
        })?;

        for row in rows {
            let (quarter_opt, status_str) = row?;
            let state = match status_str.as_str() {
                "Confirmed" => QuarterState::Confirmed,
                "Submitted" | "Filed" => QuarterState::Submitted,
                "Paid" => QuarterState::Paid,
                "Queued" => QuarterState::Queued,
                "Draft" => QuarterState::Draft,
                _ => QuarterState::Draft,
            };
            if let Some(q) = quarter_opt {
                let idx = (q - 1) as usize;
                if idx < 4 {
                    progress.quarters[idx] = state.clone();
                }
                if idx < 12 {
                    progress.months[idx] = state;
                }
            } else {
                progress.annual_status = state;
            }
        }

        Ok(progress)
    }

    /// List all form draft summaries for a TIN in a given year (all form types).
    pub fn list_draft_summaries(
        &self,
        tin: &str,
        year: u16,
    ) -> Result<Vec<FormDraftSummary>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tin, form_code, taxable_year, quarter, status, updated_at
             FROM form_drafts WHERE tin = ?1 AND taxable_year = ?2",
        )?;
        let rows = stmt.query_map(params![tin, year as i64], |row| {
            Ok(FormDraftSummary {
                id: row.get(0)?,
                tin: row.get(1)?,
                form_code: row.get(2)?,
                taxable_year: row.get::<_, i64>(3)? as u16,
                quarter: row.get::<_, Option<i64>>(4)?.map(|q| q as u8),
                status: match row.get::<_, String>(5)?.as_str() {
                    "Confirmed" => FilingStatus::Confirmed,
                    "Submitted" | "Filed" => FilingStatus::Submitted,
                    "Paid" => FilingStatus::Paid,
                    "Queued" => FilingStatus::Queued,
                    _ => FilingStatus::Draft,
                },
                updated_at: row.get(6)?,
            })
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }

    pub fn list_all_queued_submissions(&self) -> Result<Vec<FormDraftSummary>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tin, form_code, taxable_year, quarter, status, updated_at
             FROM form_drafts WHERE status = 'Queued' OR status = 'Submitted'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(FormDraftSummary {
                id: row.get(0)?,
                tin: row.get(1)?,
                form_code: row.get(2)?,
                taxable_year: row.get::<_, i64>(3)? as u16,
                quarter: row.get::<_, Option<i64>>(4)?.map(|q| q as u8),
                status: match row.get::<_, String>(5)?.as_str() {
                    "Confirmed" => FilingStatus::Confirmed,
                    "Submitted" | "Filed" => FilingStatus::Submitted,
                    "Paid" => FilingStatus::Paid,
                    "Queued" => FilingStatus::Queued,
                    _ => FilingStatus::Draft,
                },
                updated_at: row.get(6)?,
            })
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }

    pub fn save_submission_receipt(
        &self,
        receipt: &BirReceiptConfirmation,
    ) -> Result<(SubmissionReceipt, bool), DbError> {
        let (tin, form_type, period) = split_bir_filename(&receipt.filename)
            .unwrap_or_else(|| ("".to_string(), "".to_string(), "".to_string()));

        let received_date_str = receipt.date_received.to_string();
        let received_time_str = receipt.time_received.format("%H:%M:%S").to_string();

        if let Some(existing) = self.get_submission_receipt_by_filename(&receipt.filename)? {
            if existing.received_date == received_date_str
                && existing.received_time == received_time_str
            {
                // It's the exact same receipt we already processed. Return false for is_new.
                return Ok((existing, false));
            }
        }

        self.conn.execute(
            "INSERT INTO submission_receipts
                (filename, tin, form_type, period, received_date, received_time, source_from, raw_text, raw_html)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(filename) DO UPDATE SET
                tin = excluded.tin,
                form_type = excluded.form_type,
                period = excluded.period,
                received_date = excluded.received_date,
                received_time = excluded.received_time,
                source_from = excluded.source_from,
                raw_text = excluded.raw_text,
                raw_html = excluded.raw_html",
            params![
                receipt.filename,
                tin,
                form_type,
                period,
                receipt.date_received.to_string(),
                receipt.time_received.format("%H:%M:%S").to_string(),
                receipt.source_from,
                receipt.raw_text,
                receipt.raw_html,
            ],
        )?;

        let saved = self
            .get_submission_receipt_by_filename(&receipt.filename)?
            .expect("receipt should exist after save");
        Ok((saved, true))
    }

    pub fn get_submission_receipt_by_id(
        &self,
        id: i64,
    ) -> Result<Option<SubmissionReceipt>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, filename, tin, form_type, period, received_date, received_time,
                    source_from, raw_text, raw_html, created_at
             FROM submission_receipts WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(SubmissionReceipt {
                id: row.get(0)?,
                filename: row.get(1)?,
                tin: row.get(2)?,
                form_type: row.get(3)?,
                period: row.get(4)?,
                received_date: row.get(5)?,
                received_time: row.get(6)?,
                source_from: row.get(7)?,
                raw_text: row.get(8)?,
                raw_html: row.get(9)?,
                created_at: row.get(10)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_submission_receipt_by_filename(
        &self,
        filename: &str,
    ) -> Result<Option<SubmissionReceipt>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, filename, tin, form_type, period, received_date, received_time,
                    source_from, raw_text, raw_html, created_at
             FROM submission_receipts WHERE filename = ?1",
        )?;
        let mut rows = stmt.query(params![filename])?;
        if let Some(row) = rows.next()? {
            Ok(Some(SubmissionReceipt {
                id: Some(row.get(0)?),
                filename: row.get(1)?,
                tin: row.get(2)?,
                form_type: row.get(3)?,
                period: row.get(4)?,
                received_date: row.get(5)?,
                received_time: row.get(6)?,
                source_from: row.get(7)?,
                raw_text: row.get(8)?,
                raw_html: row.get(9)?,
                created_at: row.get(10)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn confirm_2551q_from_receipt(&self, receipt: &SubmissionReceipt) -> Result<(), DbError> {
        if receipt.form_type != "2551Qv2018" {
            return Ok(());
        }

        let Some((year, quarter)) = parse_2551q_period(&receipt.period) else {
            return Ok(());
        };

        let mut draft = match self.get_2551q_draft(&receipt.tin, year, quarter)? {
            Some(draft) => draft,
            None => return Ok(()),
        };

        if let Some(submitted_at) = &draft.submitted_at {
            if let Ok(submitted_dt) = chrono::DateTime::parse_from_rfc3339(submitted_at) {
                let date_str = format!("{}T{}", receipt.received_date, receipt.received_time);
                if let Ok(receipt_naive) =
                    chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S")
                {
                    if let Some(offset) = chrono::FixedOffset::east_opt(8 * 3600) {
                        use chrono::TimeZone;
                        if let chrono::LocalResult::Single(receipt_dt) =
                            offset.from_local_datetime(&receipt_naive)
                        {
                            if receipt_dt + chrono::Duration::minutes(5) < submitted_dt {
                                tracing::info!(
                                    "Ignoring old receipt {} for draft submitted at {}",
                                    receipt.filename,
                                    submitted_dt
                                );
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        draft.status = FilingStatus::Confirmed;
        draft.confirmed_at = Some(format!(
            "{}T{}",
            receipt.received_date, receipt.received_time
        ));
        draft.submission_filename = Some(receipt.filename.clone());
        draft.receipt_id = receipt.id;
        self.save_2551q_draft(&draft)?;
        Ok(())
    }

    // =========================================================================
    // Tax Deadlines
    // =========================================================================
    pub fn save_tax_deadline(&self, deadline: &TaxDeadline) -> Result<i64, DbError> {
        self.conn.execute(
            "INSERT INTO tax_deadlines (form_type, due_date, description) VALUES (?1, ?2, ?3)",
            params![deadline.form_type, deadline.due_date, deadline.description],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_tax_deadlines(&self) -> Result<Vec<TaxDeadline>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, form_type, due_date, description FROM tax_deadlines ORDER BY due_date ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TaxDeadline {
                id: row.get(0)?,
                form_type: row.get(1)?,
                due_date: row.get(2)?,
                description: row.get(3)?,
            })
        })?;
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    // =========================================================================
    // Notices and Announcements
    // =========================================================================
    pub fn save_bir_notice(&self, notice: &BirNotice) -> Result<i64, DbError> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO bir_notices (external_id, source, source_kind, source_url, title, body, notice_type, rdo_code, form_code, deadline, image_url, posted_at, raw_json, read_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(source_kind, external_id) DO UPDATE SET
                 title=excluded.title,
                 body=excluded.body,
                 source_url=excluded.source_url,
                 notice_type=excluded.notice_type,
                 posted_at=excluded.posted_at,
                 raw_json=excluded.raw_json,
                 fetched_at=datetime('now')"
        )?;

        let read_status = if notice.read_status { 1 } else { 0 };
        stmt.execute(params![
            notice.external_id,
            notice.source,
            notice.source_kind.as_str(),
            notice.source_url,
            notice.title,
            notice.body,
            notice.notice_type.as_str(),
            notice.rdo_code,
            notice.form_code,
            notice.deadline,
            notice.image_url,
            notice.posted_at,
            notice.raw_json,
            read_status
        ])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_bir_notices(&self) -> Result<Vec<BirNotice>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, external_id, source, source_kind, source_url, title, body, notice_type, rdo_code, form_code, deadline, image_url, posted_at, fetched_at, raw_json, read_status
             FROM bir_notices ORDER BY posted_at DESC, id DESC LIMIT 50",
        )?;

        let notices = stmt
            .query_map([], |row| {
                let kind_str: String = row.get(3)?;
                let type_str: String = row.get(7)?;
                Ok(BirNotice {
                    id: row.get(0)?,
                    external_id: row.get(1)?,
                    source: row.get(2)?,
                    source_kind: NoticeSourceKind::from_str(&kind_str),
                    source_url: row.get(4)?,
                    title: row.get(5)?,
                    body: row.get(6)?,
                    notice_type: NoticeType::from_str(&type_str),
                    rdo_code: row.get(8)?,
                    form_code: row.get(9)?,
                    deadline: row.get(10)?,
                    image_url: row.get(11)?,
                    posted_at: row.get(12)?,
                    fetched_at: row.get(13)?,
                    raw_json: row.get(14)?,
                    read_status: row.get::<_, i32>(15)? != 0,
                })
            })?
            .filter_map(Result::ok)
            .collect();

        Ok(notices)
    }

    pub fn save_announcement(&self, ann: &Announcement) -> Result<i64, DbError> {
        self.save_bir_notice(&BirNotice {
            id: None,
            external_id: format!(
                "legacy-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_micros()
            ),
            source: ann.source.clone(),
            source_kind: NoticeSourceKind::Rss,
            source_url: None,
            title: ann.title.clone(),
            body: ann.content.clone(),
            notice_type: NoticeType::General,
            rdo_code: None,
            form_code: None,
            deadline: None,
            image_url: None,
            posted_at: Some(ann.published_at.clone()),
            fetched_at: "now".to_string(),
            raw_json: None,
            read_status: ann.read_status,
        })
    }

    pub fn list_announcements(&self) -> Result<Vec<Announcement>, DbError> {
        let notices = self.list_bir_notices()?;
        Ok(notices
            .into_iter()
            .map(|n| Announcement {
                id: n.id,
                source: n.source,
                title: n.title,
                content: n.body,
                published_at: n.posted_at.unwrap_or_default(),
                read_status: n.read_status,
            })
            .collect())
    }

    // =========================================================================
    // Penalties Cache
    // =========================================================================
    pub fn save_penalty_cache(&self, cache: &PenaltyCache) -> Result<i64, DbError> {
        self.conn.execute(
            "INSERT INTO penalties_cache (tin, form_type, period, penalty_amount, reason, is_high_risk) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![cache.tin, cache.form_type, cache.period, cache.penalty_amount, cache.reason, cache.is_high_risk],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_penalties_cache(&self, tin: &str) -> Result<Vec<PenaltyCache>, DbError> {
        let mut stmt = self.conn.prepare("SELECT id, tin, form_type, period, penalty_amount, reason, is_high_risk, calculated_at FROM penalties_cache WHERE tin = ?1 ORDER BY calculated_at DESC")?;
        let rows = stmt.query_map(params![tin], |row| {
            Ok(PenaltyCache {
                id: row.get(0)?,
                tin: row.get(1)?,
                form_type: row.get(2)?,
                period: row.get(3)?,
                penalty_amount: row.get(4)?,
                reason: row.get(5)?,
                is_high_risk: row.get(6)?,
                calculated_at: row.get(7)?,
            })
        })?;
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }
}

pub fn parse_2551q_period(period: &str) -> Option<(u16, u8)> {
    let q_pos = period.rfind('Q')?;
    let quarter = period[q_pos + 1..].parse::<u8>().ok()?;
    let before_q = &period[..q_pos];
    let year_str = before_q.get(before_q.len().saturating_sub(4)..)?;
    let year = year_str.parse::<u16>().ok()?;
    Some((year, quarter))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // A mock keyring can be tricky in CI, so we might encounter issues depending on the runner.
    // Assuming macOS with user session or a mock environment.
    #[test]
    fn test_profile_crud() {
        let db_file = NamedTempFile::new().unwrap();
        // Since keyring depends on OS, this might fail in headless environments without a keychain.
        // For the sake of the test, we'll try to open it and skip if the OS keychain isn't available.
        let db = match Database::open(db_file.path()) {
            Ok(db) => db,
            Err(e) => {
                println!("Skipping test due to keyring unavailability: {:?}", e);
                return;
            }
        };

        let profile = TaxpayerProfile {
            id: None,
            full_name: "Code It Like Miley".into(),
            tin: Tin {
                segment1: "010".into(),
                segment2: "558".into(),
                segment3: "054".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: "Software".into(),
            registered_address: "QC".into(),
            is_archived: false,
            email_tracking_enabled: false,
            email_auth_method: crate::profile::EmailAuthMethod::AppPassword,
            imap_email: None,
            imap_host: None,
            _imap_enabled_compat: None,
            background_cron_enabled: true,
            test_notification_enabled: false,
            error_telemetry_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            zip_code: "1103".into(),
            phone: "0999".into(),
            email: "miley@example.com".into(),
            default_form_type: "2551Qv2018".into(),
            taxpayer_type: crate::profile::TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
        };

        db.save_profile(profile).expect("Failed to save profile");

        let retrieved = db
            .get_profile("010558054000")
            .expect("Failed to query")
            .expect("Profile not found");
        assert_eq!(retrieved.full_name, "Code It Like Miley");

        let list = db.list_profiles().expect("Failed to list");
        assert_eq!(list.len(), 1);

        db.delete_profile("010558054000").expect("Failed to delete");
        let list_after = db.list_profiles().expect("Failed to list");
        assert_eq!(list_after.len(), 0);
    }

    #[test]
    fn test_open_or_recreate_quarantines_unreadable_db() {
        let mut db_file = NamedTempFile::new().unwrap();
        db_file.write_all(b"not-a-sqlite-database").unwrap();

        let (db, backup_path) = match Database::open_or_recreate(db_file.path()) {
            Ok(result) => result,
            Err(e) => {
                println!("Skipping test due to keyring unavailability: {:?}", e);
                return;
            }
        };

        let backup_path = backup_path.expect("expected unreadable db to be quarantined");
        assert!(backup_path.exists(), "backup file should exist");
        assert!(db_file.path().exists(), "database file should be recreated");
        assert!(
            db.list_profiles().is_ok(),
            "recreated database should be usable"
        );
    }
}
