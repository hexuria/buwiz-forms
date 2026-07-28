//! Encrypted SQLite database for taxpayer data.
//!
//! Stores profiles, form data, submission history.
//! Data is transparently AES-256 encrypted using SQLCipher.
//! Master key stored in OS keychain.
//!
//! This module is split into domain-specific sub-modules:
//! - `profiles` — Taxpayer profile CRUD
//! - `jobs` — Background job queue
//! - `submissions` — Tax form submissions
//! - `drafts` — Form draft lifecycle
//! - `receipts` — Submission receipt tracking
//! - `notices` — BIR notices, announcements, deadlines, penalties

mod drafts;
pub(crate) use drafts::{Claim1601CSubmissionResult, Claim2551QSubmissionResult};
mod form_rule_state;
pub use form_rule_state::{FormFinalCopy, FormRuleIdentity, FormRuleState, FormRuleStateError};
mod forms_set;
mod google_calendar;
mod jobs;
mod migrations;
mod notices;
mod profiles;
mod providers;
mod receipts;
pub use receipts::ReceiptConfirmationOutcome;
mod submissions;

use rusqlite::{Connection, ErrorCode, OpenFlags, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use google_calendar::{
    CalendarEventLink, PostCommitRefreshStatus, PostCommitWrite, ProfileCalendarLink,
};

// =========================================================================
// Data types
// =========================================================================

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

    pub fn from_string(s: &str) -> Self {
        match s {
            "BirCms" => NoticeSourceKind::BirCms,
            "Manual" => NoticeSourceKind::Manual,
            "FacebookGraph" => NoticeSourceKind::FacebookGraph,
            "Rss" => NoticeSourceKind::Rss,
            unknown => {
                tracing::warn!("Unknown NoticeSourceKind '{}', defaulting to Rss", unknown);
                NoticeSourceKind::Rss
            }
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

    pub fn from_string(s: &str) -> Self {
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

// =========================================================================
// Error type
// =========================================================================

/// `security(1)` exit code for "the specified item could not be found".
///
/// macOS-only: the `security` CLI, and the read-then-create dance this guards,
/// exist only on that platform.
#[cfg(target_os = "macos")]
const SECURITY_ERR_ITEM_NOT_FOUND: i32 = 44;

/// Whether a failed keychain read means the master key is genuinely absent.
///
/// This single predicate is what stands between a first run and destroying an
/// existing database. `security find-generic-password` fails for several
/// reasons, and only one of them means "no key here":
///
/// - exit 44 - the item is not in the keychain. First run; safe to create one.
/// - anything else - authorization denied, keychain locked, `securityd`
///   unavailable, killed by a signal. A key may well exist and simply not be
///   readable at this moment. Generating a replacement would overwrite it and
///   leave the encrypted database unopenable forever.
///
/// Defaults to `false` for `None` (terminated by a signal), because an unknown
/// failure must never be read as "absent".
#[cfg(target_os = "macos")]
fn keychain_miss_is_genuine(exit_code: Option<i32>) -> bool {
    exit_code == Some(SECURITY_ERR_ITEM_NOT_FOUND)
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

// =========================================================================
// Core Database struct and lifecycle
// =========================================================================

pub struct Database {
    pub(crate) conn: Connection,
}

pub fn default_database_path() -> std::path::PathBuf {
    crate::platform::data_dir().join("bir_data.db")
}

impl Database {
    #[cfg(test)]
    pub(crate) fn open_in_memory_for_tests() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        migrations::migrate_database(&conn)?;
        Ok(Self { conn })
    }

    /// Returns the current SQLite `data_version`, which increments whenever another connection
    /// commits a write to the WAL. Used by the db-watcher in `bir-desktop` to detect external
    /// database changes without requiring direct access to the raw `conn` field.
    pub fn data_version(&self) -> Option<i32> {
        self.conn
            .query_row("PRAGMA data_version;", [], |row| row.get(0))
            .ok()
    }

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

        // Apply database schema migrations
        migrations::migrate_database(&conn)?;

        // Sync legacy announcements to bir_notices table
        let _ = conn.execute_batch("
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
        ");

        Ok(Self { conn })
    }

    /// Opens an existing SQLCipher database without creating, migrating, or recovering it.
    ///
    /// This is intentionally narrower than [`Self::open`]. It exists for read-only diagnostics
    /// and backup export, where a failed audit must never mutate or replace the source database.
    /// The returned connection cannot write to the main database, although SQLite may still
    /// attach a separate output database for `sqlcipher_export`.
    pub fn open_existing_read_only<P: AsRef<Path>>(path: P) -> Result<Self, DbError> {
        let path = path.as_ref();
        if !path.is_file() {
            return Err(DbError::Other(format!(
                "Database does not exist: {}",
                path.display()
            )));
        }

        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let key_hex = Self::get_existing_master_key()?;
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key_hex))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        let mut check_stmt = conn.prepare("SELECT count(*) FROM sqlite_master")?;
        let _: i64 = check_stmt
            .query_row([], |row| row.get(0))
            .map_err(|error| {
                if let rusqlite::Error::SqliteFailure(sqlite_error, _) = &error {
                    if sqlite_error.code == ErrorCode::NotADatabase {
                        return DbError::Encryption;
                    }
                }
                DbError::Sqlite(error)
            })?;
        drop(check_stmt);

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
        #[cfg(test)]
        return Ok("0000000000000000000000000000000000000000000000000000000000000000".to_string());

        #[cfg(not(test))]
        {
            if std::env::var("EBIR_TEST_ENV").is_ok() {
                return Ok(
                    "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                );
            }

            // Platform dispatch: macOS uses the `security` CLI to avoid deadlocking
            // the GPUI main RunLoop. Linux/Windows use the `keyring` crate directly.
            #[cfg(target_os = "macos")]
            return Self::get_or_create_master_key_macos();

            #[cfg(not(target_os = "macos"))]
            return Self::get_or_create_master_key_keyring();
        }
    }

    pub(crate) fn get_existing_master_key() -> Result<String, DbError> {
        #[cfg(test)]
        return Ok("0000000000000000000000000000000000000000000000000000000000000000".to_string());

        #[cfg(not(test))]
        {
            if std::env::var("EBIR_TEST_ENV").is_ok() {
                return Ok(
                    "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                );
            }

            #[cfg(target_os = "macos")]
            return Self::get_existing_master_key_macos();

            #[cfg(not(target_os = "macos"))]
            return Self::get_existing_master_key_keyring();
        }
    }

    #[cfg(all(target_os = "macos", not(test)))]
    fn get_existing_master_key_macos() -> Result<String, DbError> {
        let output = std::process::Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                "com.ebir.rust",
                "-a",
                "sqlcipher_master_key",
                "-w",
            ])
            .output()
            .map_err(|error| {
                DbError::KeychainCli(format!("Failed to run `security` CLI: {error}"))
            })?;

        let hex_key = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if output.status.success() && hex_key.len() == 64 {
            return Ok(hex_key);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(DbError::KeychainCli(format!(
            "Existing SQLCipher master key was not available: {}",
            stderr.trim()
        )))
    }

    #[cfg(all(not(target_os = "macos"), not(test)))]
    fn get_existing_master_key_keyring() -> Result<String, DbError> {
        let keyring_error = match keyring::Entry::new("com.ebir.rust", "sqlcipher_master_key") {
            Ok(entry) => match entry.get_password() {
                Ok(hex_key) if hex_key.len() == 64 => return Ok(hex_key),
                Ok(_) => "stored key has an invalid length".to_string(),
                Err(error) => error.to_string(),
            },
            Err(error) => error.to_string(),
        };

        let key_path = crate::platform::data_dir().join("bir_key.txt");
        if let Ok(hex_key) = std::fs::read_to_string(&key_path) {
            if hex_key.len() == 64 {
                return Ok(hex_key);
            }
        }

        Err(DbError::Other(format!(
            "Existing SQLCipher master key was not available: {keyring_error}"
        )))
    }

    /// macOS: Use the `security` CLI instead of the `keyring` crate's Security.framework FFI.
    ///
    /// The `keyring` crate calls `SecKeychainFindGenericPassword` which blocks the calling
    /// thread waiting for `securityd` to respond. When called from GPUI's main dispatch queue
    /// (inside `cx.spawn` → `open_window` → `AppState::new`), the Security.framework tries
    /// to dispatch authorization UI work back onto the same main queue, causing a deadlock.
    ///
    /// The `security` CLI runs in a separate process with its own event loop, so it can show
    /// dialogs and communicate with `securityd` without blocking our RunLoop.
    ///
    /// The CLI uses the same Keychain Services storage as the `keyring` crate, so existing
    /// keys created by either method are fully interchangeable.
    #[cfg(all(target_os = "macos", not(test)))]
    fn get_or_create_master_key_macos() -> Result<String, DbError> {
        use tracing::info;

        // Try to read existing key from keychain via CLI
        let output = std::process::Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                "com.ebir.rust",
                "-a",
                "sqlcipher_master_key",
                "-w",
            ])
            .output()
            .map_err(|e| DbError::KeychainCli(format!("Failed to run `security` CLI: {e}")))?;

        if output.status.success() {
            let hex_key = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if hex_key.len() == 64 {
                info!("Loaded existing master key from native keychain (CLI)");
                return Ok(hex_key);
            }
            // A key is present but malformed. Replacing it would destroy the
            // only means of decrypting an existing database, so stop.
            return Err(DbError::KeychainCli(format!(
                "The SQLCipher master key in the keychain is {} characters, expected 64. \
                 Refusing to replace it - if it were regenerated the existing database \
                 could never be decrypted again.",
                hex_key.len()
            )));
        }

        // Only exit code 44 means the item is genuinely absent. Every other
        // failure - authorization denied, keychain locked, securityd
        // unavailable - means a key may well exist and merely be unreadable
        // right now. Treating those as "absent" and generating a replacement
        // would overwrite the real key and render the database permanently
        // undecryptable. That is not hypothetical: an unsigned local build is
        // not the signed app, so macOS prompts, and a single dismissed prompt
        // used to be enough to destroy the database.
        if !keychain_miss_is_genuine(output.status.code()) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DbError::KeychainCli(format!(
                "Could not read the existing SQLCipher master key (security exit {:?}): {}. \
                 Refusing to generate a replacement, because that would overwrite the \
                 existing key and make the database permanently unreadable. If a keychain \
                 prompt appeared, approve it and retry.",
                output.status.code(),
                stderr.trim()
            )));
        }

        // Genuinely absent: first run on this machine.
        info!("No master key in keychain; generating and storing one (CLI)");
        let key: [u8; 32] = rand::random();
        let hex_key = hex::encode(key);

        let add_output = std::process::Command::new("security")
            .args([
                "add-generic-password",
                "-s",
                "com.ebir.rust",
                "-a",
                "sqlcipher_master_key",
                "-w",
                &hex_key,
                // Deliberately NOT -U. That flag means "update if exists" and
                // was the mechanism by which a stale read could clobber a live
                // key. Without it the add fails rather than overwrites, so this
                // path cannot destroy an existing key even if it is reached in
                // error.
            ])
            .output()
            .map_err(|e| DbError::KeychainCli(format!("Failed to run `security` CLI: {e}")))?;

        let add_stderr = String::from_utf8_lossy(&add_output.stderr);
        // `security add-generic-password` reports an existing item on stderr but
        // still exits 0, so the status alone cannot be trusted here.
        if !add_output.status.success() || add_stderr.contains("already exists") {
            return Err(DbError::KeychainCli(format!(
                "Failed to store master key in keychain: {}",
                add_stderr.trim()
            )));
        }

        Ok(hex_key)
    }

    /// Linux/Windows: Use the `keyring` crate directly (no RunLoop deadlock risk).
    #[cfg(not(target_os = "macos"))]
    fn get_or_create_master_key_keyring() -> Result<String, DbError> {
        use keyring::Entry;
        use tracing::info;

        let entry = match Entry::new("com.ebir.rust", "sqlcipher_master_key") {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Failed to access OS keyring: {e}. Falling back to file-based key.");
                return Self::get_or_create_master_key_file_fallback();
            }
        };

        match entry.get_password() {
            Ok(hex_key) => {
                info!("Loaded existing master key from native keychain");
                Ok(hex_key)
            }
            Err(_) => {
                info!("Generating new master key and storing in native keychain");
                let key: [u8; 32] = rand::random();
                let hex_key = hex::encode(key);
                if let Err(e) = entry.set_password(&hex_key) {
                    tracing::warn!(
                        "Failed to save key to OS keyring: {e}. Falling back to file-based key."
                    );
                    return Self::get_or_create_master_key_file_fallback();
                }
                Ok(hex_key)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn get_or_create_master_key_file_fallback() -> Result<String, DbError> {
        use tracing::info;
        let key_path = crate::platform::data_dir().join("bir_key.txt");

        if key_path.exists() {
            match std::fs::read_to_string(&key_path) {
                Ok(hex_key) if hex_key.len() == 64 => {
                    info!("Loaded master key from fallback file");
                    return Ok(hex_key);
                }
                _ => {
                    tracing::warn!("Fallback key file invalid or unreadable. Generating new key.");
                }
            }
        }

        info!("Generating new master key and storing in fallback file");
        let key: [u8; 32] = rand::random();
        let hex_key = hex::encode(key);
        std::fs::write(&key_path, &hex_key)?;

        // Try to secure the file on Linux by restricting permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(mut perms) = std::fs::metadata(&key_path).map(|m| m.permissions()) {
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(&key_path, perms);
            }
        }

        Ok(hex_key)
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
             DELETE FROM profile_calendar_events;
             DELETE FROM profile_calendar_links;
             DELETE FROM profiles;
             DELETE FROM submissions;
             DELETE FROM form_drafts;
             DELETE FROM data_providers;
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

    /// Delete a global setting value.
    pub fn delete_setting(&self, key: &str) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    /// Retrieve deadline overrides stored in the settings table.
    pub fn get_deadline_overrides(&self) -> Vec<crate::calendar_rules::DeadlineOverride> {
        self.get_setting("deadline_overrides")
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    /// Persist deadline overrides to the settings table.
    pub fn set_deadline_overrides(
        &self,
        overrides: &[crate::calendar_rules::DeadlineOverride],
    ) -> Result<(), DbError> {
        let json = serde_json::to_string(overrides).map_err(|e| DbError::Other(e.to_string()))?;
        self.set_setting("deadline_overrides", &json)?;
        self.request_google_calendar_sync()
    }
}

// =========================================================================
// Utility functions
// =========================================================================

pub fn parse_2551q_period(period: &str) -> Option<(u16, u8)> {
    let q_pos = period.rfind('Q')?;
    let quarter = period[q_pos + 1..].parse::<u8>().ok()?;
    let before_q = &period[..q_pos];
    let year_str = before_q.get(before_q.len().saturating_sub(4)..)?;
    let year = year_str.parse::<u16>().ok()?;
    Some((year, quarter))
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(all(test, target_os = "macos"))]
mod keychain_miss_tests {
    use super::{SECURITY_ERR_ITEM_NOT_FOUND, keychain_miss_is_genuine};

    // This predicate decides whether the app may generate a new SQLCipher
    // master key. Say yes when a key actually exists but is merely unreadable
    // and the app overwrites it, making the encrypted database permanently
    // unopenable. Every case below is a real failure mode of `security(1)`.

    #[test]
    fn exit_44_is_the_only_genuine_miss() {
        assert!(keychain_miss_is_genuine(Some(SECURITY_ERR_ITEM_NOT_FOUND)));
        assert_eq!(SECURITY_ERR_ITEM_NOT_FOUND, 44);
    }

    #[test]
    fn authorization_denied_is_not_a_miss() {
        // The user dismissed or denied the keychain prompt. A key exists.
        assert!(!keychain_miss_is_genuine(Some(51)));
        assert!(!keychain_miss_is_genuine(Some(1)));
    }

    #[test]
    fn success_is_not_a_miss() {
        assert!(!keychain_miss_is_genuine(Some(0)));
    }

    #[test]
    fn killed_by_signal_is_not_a_miss() {
        // No exit code at all. An unknown failure must never read as "absent".
        assert!(!keychain_miss_is_genuine(None));
    }

    #[test]
    fn no_other_code_in_a_wide_sweep_is_treated_as_a_miss() {
        for code in -128..=255 {
            if code == SECURITY_ERR_ITEM_NOT_FOUND {
                continue;
            }
            assert!(
                !keychain_miss_is_genuine(Some(code)),
                "exit {code} must not be treated as a missing key"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;
    use crate::profile::TaxpayerProfile;
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
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: vec![],
            compliance_source_mode: Default::default(),
            per_year_forms: Default::default(),
            zip_code: "1103".into(),
            phone: "0999".into(),
            email: "miley@example.com".into(),
            default_form_type: "2551Qv2018".into(),
            taxpayer_type: crate::profile::TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            birth_date: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
            profile_pin_hash: None,
            totp_secret: None,
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

    #[test]
    fn open_existing_read_only_never_creates_a_missing_database() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database_path = directory.path().join("missing.db");

        let error = match Database::open_existing_read_only(&database_path) {
            Ok(_) => panic!("a missing source database must fail closed"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("does not exist"));
        assert!(!database_path.exists());
    }

    #[test]
    fn open_existing_read_only_rejects_source_writes() {
        let database_file = NamedTempFile::new().expect("temporary database");
        let database = Database::open(database_file.path()).expect("create encrypted database");
        database
            .set_setting("read_only_probe", "preserved")
            .expect("seed source database");
        database.close().expect("close source database");

        let read_only = Database::open_existing_read_only(database_file.path())
            .expect("open existing database read-only");
        assert_eq!(
            read_only
                .get_setting("read_only_probe")
                .expect("read setting"),
            Some("preserved".to_string())
        );
        assert!(
            read_only.set_setting("read_only_probe", "changed").is_err(),
            "the diagnostic connection must not modify the source database"
        );
    }
}
