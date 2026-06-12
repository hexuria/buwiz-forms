//! Per-year Forms Set repository — CRUD for the `per_year_forms` table.
//!
//! The Forms Set is the user-owned, authoritative list of which BIR forms a taxpayer
//! files in a given taxable year (replacing the temporal suggestion engine).

use rusqlite::params;

use super::{Database, DbError};
use crate::forms::forms_set::{FormSetEntry, FormSetSource, PerYearFormsSet};
use crate::forms::registry::FilingFrequency;

pub(crate) fn frequency_to_str(f: &FilingFrequency) -> &'static str {
    match f {
        FilingFrequency::Quarterly => "quarterly",
        FilingFrequency::Annual => "annual",
        FilingFrequency::Monthly => "monthly",
        FilingFrequency::OpenEnded => "open_ended",
    }
}

fn frequency_from_str(s: &str) -> FilingFrequency {
    match s {
        "quarterly" => FilingFrequency::Quarterly,
        "annual" => FilingFrequency::Annual,
        "monthly" => FilingFrequency::Monthly,
        _ => FilingFrequency::OpenEnded,
    }
}

impl Database {
    /// Load the Forms Set for `(tin, year)`. Returns an empty set if none is stored.
    pub fn get_per_year_forms(&self, tin: &str, year: u16) -> Result<PerYearFormsSet, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT form_code, frequency, active, source, custom, reason
             FROM per_year_forms
             WHERE tin = ?1 AND taxable_year = ?2
             ORDER BY form_code ASC",
        )?;
        let rows = stmt.query_map(params![tin, year], |row| {
            let form_code: String = row.get(0)?;
            let frequency: String = row.get(1)?;
            let active: i64 = row.get(2)?;
            let source: String = row.get(3)?;
            let custom: i64 = row.get(4)?;
            let reason: Option<String> = row.get(5)?;
            Ok(FormSetEntry {
                form_code,
                frequency: frequency_from_str(&frequency),
                active: active != 0,
                source: FormSetSource::from_str_lossy(&source),
                custom: custom != 0,
                reason,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(PerYearFormsSet {
            taxable_year: year,
            entries,
        })
    }

    /// Replace the entire Forms Set for `(tin, year)` (delete + insert, atomic).
    pub fn save_per_year_forms(
        &self,
        tin: &str,
        year: u16,
        set: &PerYearFormsSet,
    ) -> Result<(), DbError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM per_year_forms WHERE tin = ?1 AND taxable_year = ?2",
            params![tin, year],
        )?;
        for entry in &set.entries {
            tx.execute(
                "INSERT INTO per_year_forms
                    (tin, taxable_year, form_code, frequency, active, source, custom, reason, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
                params![
                    tin,
                    year,
                    entry.form_code,
                    frequency_to_str(&entry.frequency),
                    entry.active as i64,
                    entry.source.as_str(),
                    entry.custom as i64,
                    entry.reason,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Years (descending) that have a stored Forms Set for this TIN.
    pub fn list_forms_set_years(&self, tin: &str) -> Result<Vec<u16>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT taxable_year FROM per_year_forms
             WHERE tin = ?1 ORDER BY taxable_year DESC",
        )?;
        let rows = stmt.query_map(params![tin], |row| row.get::<_, i64>(0))?;
        let mut years = Vec::new();
        for row in rows {
            years.push(row? as u16);
        }
        Ok(years)
    }

    /// The authoritative list of form codes a taxpayer files in `year` — the active
    /// entries of the stored Forms Set. Empty when no set has been configured yet.
    pub fn active_form_codes_for_year(&self, tin: &str, year: u16) -> Result<Vec<String>, DbError> {
        Ok(self.get_per_year_forms(tin, year)?.active_form_codes())
    }

    /// Whether any Forms Set row exists for `(tin, year)`.
    pub fn has_per_year_forms(&self, tin: &str, year: u16) -> Result<bool, DbError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM per_year_forms WHERE tin = ?1 AND taxable_year = ?2",
            params![tin, year],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    /// Delete the entire Forms Set for `(tin, year)`.
    pub fn delete_per_year_forms(&self, tin: &str, year: u16) -> Result<(), DbError> {
        self.conn.execute(
            "DELETE FROM per_year_forms WHERE tin = ?1 AND taxable_year = ?2",
            params![tin, year],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use tempfile::NamedTempFile;

    #[test]
    fn per_year_forms_round_trip() {
        let db_file = NamedTempFile::new().unwrap();
        let db = match Database::open(db_file.path()) {
            Ok(db) => db,
            Err(e) => {
                println!("Skipping (keyring unavailable): {e:?}");
                return;
            }
        };

        let tin = "010558054000";

        // Empty by default.
        assert!(db.get_per_year_forms(tin, 2026).unwrap().is_empty());
        assert!(!db.has_per_year_forms(tin, 2026).unwrap());

        // Save a set with one suppressed entry.
        let mut set =
            PerYearFormsSet::from_codes(2026, ["2551Q", "1701Q", "1701"], FormSetSource::CorAi);
        set.entries[1].active = false;
        set.entries[1].reason = Some("filed annually instead".into());
        db.save_per_year_forms(tin, 2026, &set).unwrap();

        // Round-trip.
        let loaded = db.get_per_year_forms(tin, 2026).unwrap();
        assert_eq!(loaded.entries.len(), 3);
        assert!(loaded.contains_active("2551Q"));
        assert!(loaded.contains_active("1701"));
        assert!(!loaded.contains_active("1701Q"));
        assert_eq!(
            loaded.entry("1701Q").unwrap().reason.as_deref(),
            Some("filed annually instead")
        );
        assert_eq!(
            loaded.entry("2551Q").unwrap().frequency,
            FilingFrequency::Quarterly
        );

        // Per-year isolation.
        assert!(db.get_per_year_forms(tin, 2025).unwrap().is_empty());

        // Replace semantics: saving again overwrites.
        let set_2025 = PerYearFormsSet::from_codes(2025, ["0605"], FormSetSource::Manual);
        db.save_per_year_forms(tin, 2025, &set_2025).unwrap();
        assert_eq!(db.list_forms_set_years(tin).unwrap(), vec![2026, 2025]);

        // Delete.
        db.delete_per_year_forms(tin, 2026).unwrap();
        assert!(!db.has_per_year_forms(tin, 2026).unwrap());
        assert_eq!(db.list_forms_set_years(tin).unwrap(), vec![2025]);
    }
}
