//! Profile repository — CRUD for TaxpayerProfile.

use rusqlite::params;

use super::{Database, DbError};
use crate::profile::TaxpayerProfile;

impl Database {
    fn execute_save_per_year_forms(
        conn: &rusqlite::Connection,
        tin: &str,
        year: u16,
        set: &crate::forms::forms_set::PerYearFormsSet,
    ) -> Result<(), DbError> {
        conn.execute(
            "DELETE FROM per_year_forms WHERE tin = ?1 AND taxable_year = ?2",
            params![tin, year],
        )?;
        for entry in &set.entries {
            conn.execute(
                "INSERT INTO per_year_forms
                    (tin, taxable_year, form_code, frequency, active, source, custom, reason, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
                params![
                    tin,
                    year,
                    entry.form_code,
                    super::forms_set::frequency_to_str(&entry.frequency),
                    entry.active as i64,
                    entry.source.as_str(),
                    entry.custom as i64,
                    entry.reason,
                ],
            )?;
        }
        Ok(())
    }

    fn has_per_year_forms_conn(
        conn: &rusqlite::Connection,
        tin: &str,
        year: u16,
    ) -> Result<bool, DbError> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM per_year_forms WHERE tin = ?1 AND taxable_year = ?2",
            params![tin, year],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn save_profile(&self, mut profile: TaxpayerProfile) -> Result<TaxpayerProfile, DbError> {
        profile.ensure_profile_version_ledger();
        let tin = profile.tin.full();

        // Query old confirmed IDs BEFORE starting transaction and doing any inserts/updates
        let old_confirmed_ids: std::collections::HashSet<String> = self
            .get_profile(&tin)?
            .map(|p| {
                p.confirmed_profile_versions()
                    .iter()
                    .map(|v| v.id.clone())
                    .collect()
            })
            .unwrap_or_default();

        let tx = self.conn.unchecked_transaction()?;

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

        for (year, set) in &profile.per_year_forms {
            Self::execute_save_per_year_forms(&tx, &tin, *year, set)?;
        }

        // Detect newly confirmed versions or populate missing per-year forms
        let mut years_to_update = std::collections::BTreeSet::new();
        use chrono::Datelike as _;
        let current_year = chrono::Utc::now().year() as u16;

        // 1. Scan for newly confirmed versions. OCR-backed versions with an
        // exact extracted form list are also reconciled on every save so stale
        // broad tax-type expansions are removed from older databases.
        for v in profile.confirmed_profile_versions() {
            let has_exact_cor_codes = v
                .evidence
                .iter()
                .any(|document| !document.extracted_form_codes.is_empty());
            if !old_confirmed_ids.contains(&v.id) || has_exact_cor_codes {
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
                    years_to_update.insert(y);
                }
            }
        }

        // 2. Scan for any missing per_year_forms rows
        for v in profile.confirmed_profile_versions() {
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
                if !Self::has_per_year_forms_conn(&tx, &tin, y)? {
                    years_to_update.insert(y);
                }
            }
        }

        // 3. Re-generate and save forms set for affected years
        use crate::forms::forms_set::{FormSetEntry, FormSetSource, PerYearFormsSet};
        use crate::forms::registry::canonical_form_code;
        use crate::profile::{ManualObligationOverrideAction, TaxProfileVersionSource};

        for year in years_to_update {
            let active_versions = profile.active_profile_versions_for_year(year);
            if let Some(version) = active_versions.last() {
                let source = match version.source {
                    TaxProfileVersionSource::OcrCor => FormSetSource::CorAi,
                    TaxProfileVersionSource::ManualCor | TaxProfileVersionSource::UserOverride => {
                        FormSetSource::Manual
                    }
                    TaxProfileVersionSource::MigrationBackfill => FormSetSource::MigrationBackfill,
                };

                let resolved_codes =
                    crate::integration::validation::registered_form_codes_for_version(
                        &profile, version, year,
                    );
                let mut entries_by_code = std::collections::BTreeMap::new();
                for code in resolved_codes {
                    let entry = FormSetEntry::from_code(code, source);
                    entries_by_code.insert(entry.form_code.clone(), entry);
                }

                if let Some(existing_set) = profile.per_year_forms.get(&year) {
                    for existing in &existing_set.entries {
                        if existing.source == FormSetSource::Manual || !existing.active {
                            let mut preserved = existing.clone();
                            preserved.form_code = canonical_form_code(&preserved.form_code);
                            entries_by_code.insert(preserved.form_code.clone(), preserved);
                        }
                    }
                }

                for override_rule in &version.obligation_overrides {
                    let code = canonical_form_code(&override_rule.form_code);
                    let entry = entries_by_code
                        .entry(code.clone())
                        .or_insert_with(|| FormSetEntry::from_code(code, source));
                    entry.active = matches!(
                        override_rule.action,
                        ManualObligationOverrideAction::Include
                    );
                    entry.reason = Some(override_rule.reason.clone());
                }

                let set = PerYearFormsSet {
                    taxable_year: year,
                    entries: entries_by_code.into_values().collect(),
                };
                Self::execute_save_per_year_forms(&tx, &tin, year, &set)?;
                profile.per_year_forms.insert(year, set);
            }
        }

        tx.commit()?;
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
        self.conn
            .execute("DELETE FROM profiles WHERE tin = ?1", params![tin])?;
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
