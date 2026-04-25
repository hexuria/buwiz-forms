//! Encrypted SQLite database for taxpayer data.
//!
//! Stores profiles, form data, submission history.
//! Data is transparently AES-256 encrypted using SQLCipher.
//! Master key stored in OS keychain.

use keyring::Entry;
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
    pub created_at: Option<String>,
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
}

pub struct Database {
    conn: Connection,
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
                    tracing::warn!(
                        "Keyring backend appears to be a mock store! \
                         Encryption keys will NOT persist across restarts. \
                         Enable the 'apple-native' feature on the keyring crate."
                    );
                }
            }
        }

        let key_hex = match entry.get_password() {
            Ok(hex_key) => {
                info!("Loaded existing master key from keychain");
                hex_key
            }
            Err(_) => {
                info!("Generating new master key and storing in keychain");
                let key: [u8; 32] = rand::random();
                let hex_key = hex::encode(key);
                entry.set_password(&hex_key)?;
                hex_key
            }
        };

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
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

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

    /// Delete a profile by TIN.
    pub fn delete_profile(&self, tin: &str) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM profiles WHERE tin = ?1", params![tin])?;
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
            FilingStatus::Submitted => "Submitted",
            FilingStatus::Confirmed => "Confirmed",
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
                "Draft" => QuarterState::Draft,
                _ => QuarterState::Draft,
            };
            if let Some(q) = quarter_opt {
                let idx = (q - 1) as usize;
                if idx < 4 {
                    progress.quarters[idx] = state;
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
    ) -> Result<SubmissionReceipt, DbError> {
        let (tin, form_type, period) = split_bir_filename(&receipt.filename)
            .unwrap_or_else(|| ("".to_string(), "".to_string(), "".to_string()));

        self.conn.execute(
            "INSERT INTO submission_receipts
                (filename, tin, form_type, period, received_date, received_time, source_from, raw_text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(filename) DO UPDATE SET
                tin = excluded.tin,
                form_type = excluded.form_type,
                period = excluded.period,
                received_date = excluded.received_date,
                received_time = excluded.received_time,
                source_from = excluded.source_from,
                raw_text = excluded.raw_text",
            params![
                receipt.filename,
                tin,
                form_type,
                period,
                receipt.date_received.to_string(),
                receipt.time_received.format("%H:%M:%S").to_string(),
                receipt.source_from,
                receipt.raw_text,
            ],
        )?;

        let saved = self
            .get_submission_receipt_by_filename(&receipt.filename)?
            .expect("receipt should exist after save");
        self.confirm_2551q_from_receipt(&saved)?;
        Ok(saved)
    }

    pub fn get_submission_receipt_by_filename(
        &self,
        filename: &str,
    ) -> Result<Option<SubmissionReceipt>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, filename, tin, form_type, period, received_date, received_time,
                    source_from, raw_text, created_at
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
                created_at: row.get(9)?,
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
}

fn parse_2551q_period(period: &str) -> Option<(u16, u8)> {
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
