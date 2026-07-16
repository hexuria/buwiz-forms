//! Profile repository — CRUD for TaxpayerProfile.

use rusqlite::{OptionalExtension, params};

use super::{Database, DbError};
use crate::profile::TaxpayerProfile;

impl Database {
    fn migrate_profile_tin_references(
        conn: &rusqlite::Connection,
        old_tin: &str,
        new_tin: &str,
    ) -> Result<(), DbError> {
        if old_tin == new_tin {
            return Ok(());
        }

        for (table, column) in [
            ("penalties_cache", "tin"),
            ("submissions", "tin"),
            ("form_drafts", "tin"),
            ("submission_receipts", "tin"),
            ("data_providers", "profile_tin"),
            ("per_year_forms", "tin"),
            ("profile_calendar_events", "profile_tin"),
            ("profile_calendar_links", "profile_tin"),
        ] {
            conn.execute(
                &format!("UPDATE {table} SET {column} = ?1 WHERE {column} = ?2"),
                params![new_tin, old_tin],
            )?;
        }
        Ok(())
    }

    pub fn save_profile(&self, mut profile: TaxpayerProfile) -> Result<TaxpayerProfile, DbError> {
        profile.ensure_profile_version_ledger();
        let tin = profile.tin.full();
        let previous_tin = profile
            .id
            .map(|id| {
                self.conn
                    .query_row(
                        "SELECT tin FROM profiles WHERE id = ?1",
                        params![id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
            })
            .transpose()?
            .flatten();
        let lookup_tin = previous_tin.as_deref().unwrap_or(&tin);

        let existing_profile = self.get_profile(lookup_tin)?;
        if profile.atc_codes.is_empty()
            && let Some(existing_atc_codes) = existing_profile
                .as_ref()
                .map(|stored| &stored.atc_codes)
                .filter(|codes| !codes.is_empty())
        {
            profile.atc_codes.clone_from(existing_atc_codes);
        }

        let old_confirmed_ids = existing_profile
            .as_ref()
            .map(|stored| {
                stored
                    .confirmed_profile_versions()
                    .into_iter()
                    .map(|version| version.id)
                    .collect::<std::collections::HashSet<_>>()
            })
            .unwrap_or_default();
        let mut newly_confirmed_starts = profile
            .profile_versions
            .iter()
            .filter(|version| {
                version.status == crate::profile::TaxProfileVersionStatus::Confirmed
                    && version.source != crate::profile::TaxProfileVersionSource::MigrationBackfill
                    && !old_confirmed_ids.contains(&version.id)
            })
            .filter_map(|version| version.effective_from)
            .collect::<Vec<_>>();
        newly_confirmed_starts.sort_unstable();
        for effective_from in newly_confirmed_starts {
            profile.auto_close_previous_confirmed_version(effective_from);
        }
        profile
            .validate_confirmed_profile_timeline()
            .map_err(DbError::Other)?;

        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch("PRAGMA defer_foreign_keys = ON;")?;

        let json_data = serde_json::to_string(&profile)?;

        if let Some(id) = profile.id {
            let updated = tx.execute(
                "UPDATE profiles SET tin = ?1, data_json = ?2 WHERE id = ?3",
                params![tin, json_data, id],
            )?;
            if updated == 0 {
                tx.execute(
                    "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
                    params![tin, json_data],
                )?;
                profile.id = Some(tx.last_insert_rowid());
            }
        } else if let Some(_existing) = self.get_profile(&tin)? {
            return Err(DbError::Sqlite(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ErrorCode::ConstraintViolation as i32),
                Some(format!("A profile with TIN {} already exists", tin)),
            )));
        } else {
            tx.execute(
                "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
                params![tin, json_data],
            )?;
            profile.id = Some(tx.last_insert_rowid());
        }
        if let Some(previous_tin) = previous_tin.as_deref() {
            Self::migrate_profile_tin_references(&tx, previous_tin, &tin)?;
        }

        for (year, set) in &profile.per_year_forms {
            super::forms_set::execute_replace_per_year_forms(&tx, &tin, *year, set)?;
        }

        // Refresh the current year and every already stored year in the same
        // transaction as the profile update. Ambiguous or undated timelines
        // preserve existing Forms Sets instead of guessing.
        let mut years_to_update = std::collections::BTreeSet::new();
        use chrono::Datelike as _;
        let current_year = chrono::Utc::now().year() as u16;
        years_to_update.insert(current_year);
        years_to_update.extend(profile.per_year_forms.keys().copied());
        if let Some(stored) = &existing_profile {
            years_to_update.extend(stored.per_year_forms.keys().copied());
        }

        for year in years_to_update {
            let resolved = profile.resolve_tax_profile_for_year(year);
            if resolved.has_blocking_issues() || resolved.effective_segments.is_empty() {
                continue;
            }

            let suggestions =
                crate::integration::validation::form_suggestions_for_profile_year(&profile, year);
            let existing_set = profile.per_year_forms.get(&year).or_else(|| {
                existing_profile
                    .as_ref()
                    .and_then(|stored| stored.per_year_forms.get(&year))
            });
            let result =
                crate::forms::reconcile_forms_set_for_year(year, existing_set, &suggestions);
            if !result.conflicts.is_empty() {
                tracing::warn!(
                    tin = %tin,
                    taxable_year = year,
                    conflicts = result.conflicts.len(),
                    "Forms Set reconciliation requires review"
                );
            }
            super::forms_set::execute_replace_per_year_forms(&tx, &tin, year, &result.forms_set)?;
            profile.per_year_forms.insert(year, result.forms_set);
        }

        tx.commit()?;
        let _ = self.request_google_calendar_sync();
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
            self.hydrate_profile_forms(&mut profile)?;
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
            self.hydrate_profile_forms(&mut profile)?;
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
        if self.get_profile_calendar_link(tin)?.is_some() {
            return Err(DbError::Other(
                "Delete or unlink the profile's Google Calendar before deleting the profile"
                    .to_string(),
            ));
        }
        let tx = self.conn.unchecked_transaction()?;
        for (table, column) in [
            ("penalties_cache", "tin"),
            ("submissions", "tin"),
            ("form_drafts", "tin"),
            ("submission_receipts", "tin"),
            ("data_providers", "profile_tin"),
            ("per_year_forms", "tin"),
            ("profile_calendar_events", "profile_tin"),
        ] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE {column} = ?1"),
                params![tin],
            )?;
        }
        tx.execute("DELETE FROM profiles WHERE tin = ?1", params![tin])?;
        tx.commit()?;
        Ok(())
    }

    fn hydrate_profile_forms(&self, profile: &mut TaxpayerProfile) -> Result<(), DbError> {
        let tin = profile.tin.full();
        let years = self.list_forms_set_years(&tin)?;
        let mut per_year_forms = std::collections::BTreeMap::new();
        for y in years {
            let set = self.get_per_year_forms(&tin, y)?;
            per_year_forms.insert(y, set);
        }
        profile.per_year_forms = per_year_forms;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ProfileCalendarLink;
    use tempfile::NamedTempFile;

    #[test]
    fn tin_reference_migration_moves_calendar_and_forms_records() {
        let file = NamedTempFile::new().unwrap();
        let db = Database::open(file.path()).unwrap();
        db.conn
            .execute(
                "INSERT INTO profiles (tin, data_json) VALUES (?1, '{}')",
                ["123456789000"],
            )
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO per_year_forms
                    (tin, taxable_year, form_code, frequency, active, source, custom)
                 VALUES (?1, 2026, '0619E', 'monthly', 1, 'manual', 0)",
                ["123456789000"],
            )
            .unwrap();
        db.save_profile_calendar_link(&ProfileCalendarLink {
            profile_tin: "123456789000".into(),
            google_calendar_id: "calendar@example.com".into(),
            calendar_name: "Test Calendar".into(),
            enabled: true,
            last_synced_at: None,
            last_error: None,
        })
        .unwrap();

        let tx = db.conn.unchecked_transaction().unwrap();
        tx.execute_batch("PRAGMA defer_foreign_keys = ON;").unwrap();
        tx.execute(
            "UPDATE profiles SET tin = ?1 WHERE tin = ?2",
            params!["987654321000", "123456789000"],
        )
        .unwrap();
        Database::migrate_profile_tin_references(&tx, "123456789000", "987654321000").unwrap();
        tx.commit().unwrap();

        assert!(
            db.get_profile_calendar_link("987654321000")
                .unwrap()
                .is_some()
        );
        assert!(db.has_per_year_forms("987654321000", 2026).unwrap());
    }
}
