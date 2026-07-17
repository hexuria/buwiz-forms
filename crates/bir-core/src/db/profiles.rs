//! Profile repository — CRUD for TaxpayerProfile.

use rusqlite::{OptionalExtension, params};

use super::{Database, DbError};
use crate::profile::{
    TaxProfileVersion, TaxProfileVersionConfirmationPlan, TaxProfileVersionSource,
    TaxProfileVersionStatus, TaxpayerProfile,
};

fn same_persisted_version_except_label(
    stored: &TaxProfileVersion,
    submitted: &TaxProfileVersion,
) -> bool {
    let mut stored = stored.clone();
    let mut submitted = submitted.clone();
    stored.label.clear();
    submitted.label.clear();

    let Ok(stored) = serde_json::to_value(stored) else {
        return false;
    };
    let Ok(submitted) = serde_json::to_value(submitted) else {
        return false;
    };
    stored == submitted
}

fn is_exact_initial_migration_backfill(
    profile: &TaxpayerProfile,
    version: &TaxProfileVersion,
) -> bool {
    version.source == TaxProfileVersionSource::MigrationBackfill
        && same_persisted_version_except_label(
            &TaxProfileVersion::from_profile_backfill(profile),
            version,
        )
}

fn validate_reviewed_confirmation_plan(
    existing_profile: Option<&TaxpayerProfile>,
    submitted_profile: &TaxpayerProfile,
    reviewed_plan: Option<&TaxProfileVersionConfirmationPlan>,
) -> Result<(), DbError> {
    let Some(existing_profile) = existing_profile else {
        let confirmed = submitted_profile
            .profile_versions
            .iter()
            .filter(|version| version.status == TaxProfileVersionStatus::Confirmed)
            .collect::<Vec<_>>();
        let Some(reviewed_plan) = reviewed_plan else {
            if confirmed.is_empty()
                || (confirmed.len() == 1
                    && is_exact_initial_migration_backfill(submitted_profile, confirmed[0]))
            {
                return Ok(());
            }
            return Err(DbError::Other(
                "A new confirmed COR or manual profile version requires an explicit reviewed confirmation plan; only the exact one-time migration backfill may be generated automatically"
                    .to_string(),
            ));
        };

        if confirmed.iter().any(|version| {
            version.id != reviewed_plan.version_id
                && version.source != TaxProfileVersionSource::MigrationBackfill
        }) {
            return Err(DbError::Other(
                "The reviewed confirmation plan authorizes only one new confirmed profile version"
                    .to_string(),
            ));
        }

        let mut planning_profile = submitted_profile.clone();
        let Some(target) = planning_profile
            .profile_versions
            .iter_mut()
            .find(|version| version.id == reviewed_plan.version_id)
        else {
            return Err(DbError::Other(
                "The submitted profile does not match the reviewed confirmation plan".to_string(),
            ));
        };
        if target.status != TaxProfileVersionStatus::Confirmed
            || target.effective_from != Some(reviewed_plan.effective_from)
            || target.effective_until.is_some()
        {
            return Err(DbError::Other(
                "The submitted profile does not match the reviewed confirmation plan".to_string(),
            ));
        }
        target.status = TaxProfileVersionStatus::Draft;

        for closure in &reviewed_plan.auto_close_consequences {
            let Some(prior) = planning_profile
                .profile_versions
                .iter_mut()
                .find(|version| version.id == closure.version_id)
            else {
                return Err(DbError::Other(
                    "The submitted profile does not match the reviewed confirmation plan"
                        .to_string(),
                ));
            };
            if prior.status != TaxProfileVersionStatus::Confirmed
                || prior.effective_from != closure.effective_from
                || prior.effective_until != Some(closure.effective_until)
            {
                return Err(DbError::Other(
                    "The submitted profile does not match the reviewed confirmation plan"
                        .to_string(),
                ));
            }
            prior.effective_until = None;
        }

        let expected_plan = planning_profile
            .profile_version_confirmation_plan(
                &reviewed_plan.version_id,
                reviewed_plan.effective_from,
            )
            .ok_or_else(|| {
                DbError::Other(
                    "The submitted profile does not match the reviewed confirmation plan"
                        .to_string(),
                )
            })?;
        if &expected_plan != reviewed_plan {
            return Err(DbError::Other(
                "The submitted profile does not match the exact reviewed confirmation plan"
                    .to_string(),
            ));
        }

        for version in confirmed {
            if version.id == reviewed_plan.version_id {
                continue;
            }
            let Some(expected_backfill) = planning_profile
                .profile_versions
                .iter()
                .find(|candidate| candidate.id == version.id)
            else {
                return Err(DbError::Other(
                    "The submitted profile contains an unreviewed confirmed profile version"
                        .to_string(),
                ));
            };
            if !is_exact_initial_migration_backfill(submitted_profile, expected_backfill) {
                return Err(DbError::Other(
                    "The submitted profile contains an unreviewed confirmed profile version"
                        .to_string(),
                ));
            }
        }
        return Ok(());
    };

    let newly_confirmed = submitted_profile
        .profile_versions
        .iter()
        .filter(|version| {
            version.status == TaxProfileVersionStatus::Confirmed
                && existing_profile
                    .profile_versions
                    .iter()
                    .find(|stored| stored.id == version.id)
                    .is_none_or(|stored| stored.status != TaxProfileVersionStatus::Confirmed)
        })
        .collect::<Vec<_>>();
    if newly_confirmed.len() > 1 {
        return Err(DbError::Other(
            "Confirm profile versions one at a time so each timeline change can be reviewed"
                .to_string(),
        ));
    }

    let mut authorized_timeline = existing_profile.clone();
    let authorized_target_id = if let Some(version) = newly_confirmed.first() {
        let reviewed_plan = reviewed_plan.ok_or_else(|| {
            DbError::Other(
                "Confirming a persisted profile version: an explicit reviewed confirmation plan is required"
                    .to_string(),
            )
        })?;
        let effective_from = version.effective_from.ok_or_else(|| {
            DbError::Other(
                "A confirmed profile version must have an effective start date".to_string(),
            )
        })?;

        if let Some(stored_version) = authorized_timeline
            .profile_versions
            .iter()
            .find(|stored| stored.id == version.id)
        {
            if stored_version.status == TaxProfileVersionStatus::Archived {
                return Err(DbError::Other(
                    "Archived profile versions cannot be confirmed directly; create a replacement version through the reviewed confirmation workflow"
                        .to_string(),
                ));
            }
            if stored_version
                .effective_from
                .or(stored_version.cor.registration_date)
                != Some(effective_from)
            {
                return Err(DbError::Other(
                    "The profile timeline changed after confirmation was reviewed; review the consequence again"
                        .to_string(),
                ));
            }
        } else {
            authorized_timeline
                .profile_versions
                .push((*version).clone());
        }

        let expected_plan = authorized_timeline
            .profile_version_confirmation_plan(&version.id, effective_from)
            .ok_or_else(|| {
                DbError::Other(
                    "The reviewed profile-version confirmation plan is stale or does not match a newly confirmed version"
                        .to_string(),
                )
            })?;
        if &expected_plan != reviewed_plan {
            return Err(DbError::Other(
                "The profile timeline changed after confirmation was reviewed; review the consequence again"
                    .to_string(),
            ));
        }
        if !authorized_timeline.apply_profile_version_confirmation_plan(reviewed_plan) {
            return Err(DbError::Other(
                "The profile timeline changed after confirmation was reviewed; review the consequence again"
                    .to_string(),
            ));
        }

        Some(version.id.clone())
    } else {
        if reviewed_plan.is_some() {
            return Err(DbError::Other(
                "The reviewed profile-version confirmation plan is stale or does not match a newly confirmed version"
                    .to_string(),
            ));
        }
        None
    };

    for stored in existing_profile.profile_versions.iter().filter(|version| {
        matches!(
            version.status,
            TaxProfileVersionStatus::Confirmed | TaxProfileVersionStatus::Archived
        )
    }) {
        let authorized = authorized_timeline
            .profile_versions
            .iter()
            .find(|version| version.id == stored.id)
            .expect("the authorized timeline starts from the stored profile");
        let submitted = submitted_profile
            .profile_versions
            .iter()
            .find(|version| version.id == stored.id);
        if !submitted
            .is_some_and(|version| same_persisted_version_except_label(authorized, version))
        {
            return Err(DbError::Other(
                "Confirmed and archived profile-version facts cannot change; create a replacement version and confirm it through the reviewed workflow"
                    .to_string(),
            ));
        }
    }

    if let Some(target_id) = authorized_target_id {
        let authorized = authorized_timeline
            .profile_versions
            .iter()
            .find(|version| version.id == target_id)
            .expect("the reviewed target exists in the authorized timeline");
        let submitted = submitted_profile
            .profile_versions
            .iter()
            .find(|version| version.id == target_id);
        if !submitted
            .is_some_and(|version| same_persisted_version_except_label(authorized, version))
        {
            return Err(DbError::Other(
                "The submitted profile version contains changes outside the exact reviewed confirmation plan"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

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

    pub fn save_profile(&self, profile: TaxpayerProfile) -> Result<TaxpayerProfile, DbError> {
        self.save_profile_with_post_commit_status(profile)
            .map(super::PostCommitWrite::into_committed)
    }

    /// Save a profile while keeping the post-commit calendar/deadline refresh
    /// status distinct from transaction success.
    pub fn save_profile_with_post_commit_status(
        &self,
        profile: TaxpayerProfile,
    ) -> Result<super::PostCommitWrite<TaxpayerProfile>, DbError> {
        self.save_profile_inner(profile, None)
    }

    pub fn save_profile_with_confirmation_plan(
        &self,
        profile: TaxpayerProfile,
        reviewed_plan: &TaxProfileVersionConfirmationPlan,
    ) -> Result<TaxpayerProfile, DbError> {
        self.save_profile_with_confirmation_plan_and_post_commit_status(profile, reviewed_plan)
            .map(super::PostCommitWrite::into_committed)
    }

    /// Save a reviewed profile-version confirmation while keeping a failed
    /// post-commit refresh request from masquerading as a rolled-back save.
    pub fn save_profile_with_confirmation_plan_and_post_commit_status(
        &self,
        profile: TaxpayerProfile,
        reviewed_plan: &TaxProfileVersionConfirmationPlan,
    ) -> Result<super::PostCommitWrite<TaxpayerProfile>, DbError> {
        self.save_profile_inner(profile, Some(reviewed_plan))
    }

    fn save_profile_inner(
        &self,
        mut profile: TaxpayerProfile,
        reviewed_plan: Option<&TaxProfileVersionConfirmationPlan>,
    ) -> Result<super::PostCommitWrite<TaxpayerProfile>, DbError> {
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

        validate_reviewed_confirmation_plan(existing_profile.as_ref(), &profile, reviewed_plan)?;
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
        Ok(self.finish_post_commit_write(profile, "Taxpayer profile save"))
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
            profile.normalize_profile_version_review_statuses();
            profile.id = row.get(1).ok();
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
            profile.normalize_profile_version_review_statuses();
            profile.id = Some(id);
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
    fn profile_reads_do_not_synthesize_a_missing_version_ledger() {
        let file = NamedTempFile::new().unwrap();
        let db = Database::open(file.path()).unwrap();
        let legacy_json = serde_json::json!({
            "id": null,
            "full_name": "Post-migration legacy import",
            "tin": {
                "segment1": "444",
                "segment2": "555",
                "segment3": "666",
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
            "business_start_date": "2020-04-15",
            "compliance_source_mode": "CorVersioned",
            "profile_versions": []
        })
        .to_string();
        db.conn
            .execute(
                "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
                rusqlite::params!["444555666000", legacy_json],
            )
            .unwrap();

        let loaded = db
            .get_profile("444555666000")
            .unwrap()
            .expect("profile should deserialize");
        assert!(loaded.profile_versions.is_empty());
        assert!(loaded.confirmed_profile_versions().is_empty());

        let listed = db.list_profiles().unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].profile_versions.is_empty());
    }

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
