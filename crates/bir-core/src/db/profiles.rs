//! Profile repository — CRUD for TaxpayerProfile.

use rusqlite::params;

use super::{Database, DbError};
use crate::profile::TaxpayerProfile;

impl Database {
    pub fn save_profile(&self, mut profile: TaxpayerProfile) -> Result<TaxpayerProfile, DbError> {
        profile.ensure_profile_version_ledger();
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
        } else if let Some(_existing) = self.get_profile(&tin)? {
            return Err(DbError::Sqlite(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ErrorCode::ConstraintViolation as i32),
                Some(format!("A profile with TIN {} already exists", tin)),
            )));
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
            profile.ensure_profile_version_ledger();
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
            profile.ensure_profile_version_ledger();
            profiles.push(profile);
        }

        Ok(profiles)
    }

    /// Get a single profile by its TIN.
    pub fn get_profile_by_tin(&self, tin: &str) -> Result<Option<TaxpayerProfile>, DbError> {
        self.get_profile(tin)
    }

    /// Delete a profile by TIN.
    pub fn delete_profile(&self, tin: &str) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM profiles WHERE tin = ?1", params![tin])?;
        Ok(())
    }
}
