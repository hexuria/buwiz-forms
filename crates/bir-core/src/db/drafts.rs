//! Form drafts repository — save, load, and list tax form drafts.

use rusqlite::params;

use super::{Database, DbError};
use crate::forms::form_2551q::Form2551QDraft;
use crate::forms::{
    FilingFrequency, FilingPeriod, FilingStatus, FormDraftSummary, FormFilingProgress,
    QuarterState, find_form,
};

fn filing_status_to_db(status: &FilingStatus) -> &'static str {
    match status {
        FilingStatus::Draft => "Draft",
        FilingStatus::Queued => "Queued",
        FilingStatus::Submitted => "Submitted",
        FilingStatus::Confirmed => "Confirmed",
        FilingStatus::Paid => "Paid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestDraft {
        value: i32,
    }

    fn test_db() -> Database {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::migrate_database(&conn).unwrap();
        Database { conn }
    }

    #[test]
    fn period_key_upsert_updates_annual_draft_in_place() {
        let db = test_db();
        let period = FilingPeriod::Annual;
        let first_id = db
            .save_form_draft_v2(
                "123456789000",
                "1702MX",
                2026,
                &period,
                &FilingStatus::Draft,
                &TestDraft { value: 1 },
            )
            .unwrap();
        let second_id = db
            .save_form_draft_v2(
                "123456789000",
                "1702MX",
                2026,
                &period,
                &FilingStatus::Draft,
                &TestDraft { value: 2 },
            )
            .unwrap();

        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM form_drafts
                 WHERE tin = '123456789000' AND form_code = '1702MX' AND taxable_year = 2026",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let loaded: TestDraft = db
            .get_form_draft_v2("123456789000", "1702MX", 2026, &period)
            .unwrap()
            .unwrap();

        assert_eq!(first_id, second_id);
        assert_eq!(count, 1);
        assert_eq!(loaded, TestDraft { value: 2 });
    }

    #[test]
    fn scaffold_forms_reject_queue_persistence() {
        let db = test_db();
        let result = db.save_form_draft_v2(
            "123456789000",
            "1702RT",
            2026,
            &FilingPeriod::Annual,
            &FilingStatus::Queued,
            &TestDraft { value: 1 },
        );

        assert!(result.is_err());
        let summaries = db.list_draft_summaries("123456789000", 2026).unwrap();
        assert!(summaries.is_empty());
        assert!(db.list_all_queued_submissions().unwrap().is_empty());
    }

    #[test]
    fn annual_and_open_ended_progress_use_period_key_semantics() {
        let db = test_db();
        db.save_form_draft_v2(
            "123456789000",
            "1701",
            2026,
            &FilingPeriod::Annual,
            &FilingStatus::Submitted,
            &TestDraft { value: 1 },
        )
        .unwrap();
        db.save_form_draft_v2(
            "123456789000",
            "0605",
            2026,
            &FilingPeriod::OpenEnded(1),
            &FilingStatus::Submitted,
            &TestDraft { value: 1 },
        )
        .unwrap();
        db.save_form_draft_v2(
            "123456789000",
            "0605",
            2026,
            &FilingPeriod::OpenEnded(2),
            &FilingStatus::Draft,
            &TestDraft { value: 2 },
        )
        .unwrap();

        let annual = db
            .get_form_filing_progress("123456789000", "1701", 2026)
            .unwrap();
        let open_ended = db
            .get_form_filing_progress("123456789000", "0605", 2026)
            .unwrap();

        assert_eq!(annual.annual_status, QuarterState::Submitted);
        assert_eq!(open_ended.open_ended_count, 1);
    }

    #[test]
    fn monthly_and_quarterly_summaries_follow_period_keys() {
        let db = test_db();
        db.save_form_draft_v2(
            "123456789000",
            "1601C",
            2026,
            &FilingPeriod::Monthly(12),
            &FilingStatus::Queued,
            &TestDraft { value: 1 },
        )
        .unwrap();
        db.save_form_draft_v2(
            "123456789000",
            "2551Q",
            2026,
            &FilingPeriod::Quarterly(4),
            &FilingStatus::Queued,
            &TestDraft { value: 2 },
        )
        .unwrap();

        let mut summaries = db.list_draft_summaries("123456789000", 2026).unwrap();
        summaries.sort_by(|a, b| a.form_code.cmp(&b.form_code));

        assert_eq!(summaries[0].form_code, "1601C");
        assert_eq!(summaries[0].month, Some(12));
        assert_eq!(summaries[0].quarter, None);
        assert_eq!(summaries[1].form_code, "2551Q");
        assert_eq!(summaries[1].quarter, Some(4));
        assert_eq!(summaries[1].month, None);
    }
}

fn filing_status_from_db(status: &str) -> FilingStatus {
    match status {
        "Confirmed" => FilingStatus::Confirmed,
        "Submitted" | "Filed" => FilingStatus::Submitted,
        "Paid" => FilingStatus::Paid,
        "Queued" => FilingStatus::Queued,
        _ => FilingStatus::Draft,
    }
}

fn quarter_state_from_db(status: &str) -> QuarterState {
    match status {
        "Confirmed" => QuarterState::Confirmed,
        "Submitted" | "Filed" => QuarterState::Submitted,
        "Paid" => QuarterState::Paid,
        "Queued" => QuarterState::Queued,
        "Draft" => QuarterState::Draft,
        _ => QuarterState::Draft,
    }
}

fn counts_as_started_or_filed(state: &QuarterState) -> bool {
    matches!(
        state,
        QuarterState::Queued
            | QuarterState::Submitted
            | QuarterState::Confirmed
            | QuarterState::Paid
    )
}

fn frequency_for_form(form_code: &str) -> FilingFrequency {
    find_form(form_code)
        .map(|form| form.frequency.clone())
        .unwrap_or_else(|| match form_code {
            "0619E" | "0619F" | "1601C" => FilingFrequency::Monthly,
            "2551Q" | "2550Q" | "1701Q" => FilingFrequency::Quarterly,
            "1701" | "1702RT" | "1702MX" => FilingFrequency::Annual,
            "0605" => FilingFrequency::OpenEnded,
            _ => FilingFrequency::Quarterly,
        })
}

fn normalize_month(slot: Option<u8>) -> u8 {
    slot.unwrap_or(1).clamp(1, 12)
}

fn normalize_quarter(slot: Option<u8>) -> u8 {
    slot.unwrap_or(1).clamp(1, 4)
}

fn slot_from_i64(slot: Option<i64>) -> Option<u8> {
    slot.and_then(|value| u8::try_from(value).ok())
}

fn period_from_legacy_slot(
    form_code: &str,
    slot: Option<u8>,
    default_open_ended_key: u32,
) -> FilingPeriod {
    match frequency_for_form(form_code) {
        FilingFrequency::Monthly => FilingPeriod::Monthly(normalize_month(slot)),
        FilingFrequency::Quarterly => FilingPeriod::Quarterly(normalize_quarter(slot)),
        FilingFrequency::Annual => FilingPeriod::Annual,
        FilingFrequency::OpenEnded => {
            let key = slot
                .map(u32::from)
                .filter(|value| *value > 0)
                .unwrap_or(default_open_ended_key);
            FilingPeriod::OpenEnded(key)
        }
    }
}

fn period_from_row(
    form_code: &str,
    legacy_slot: Option<i64>,
    period_key: Option<&str>,
) -> FilingPeriod {
    if let Some(period) = period_key.and_then(FilingPeriod::from_period_key) {
        return period;
    }
    period_from_legacy_slot(form_code, slot_from_i64(legacy_slot), 1)
}

fn legacy_slot_for_period(period: &FilingPeriod) -> Option<i64> {
    match period {
        FilingPeriod::Monthly(month) => Some(i64::from(*month)),
        FilingPeriod::Quarterly(quarter) => Some(i64::from(*quarter)),
        FilingPeriod::Annual | FilingPeriod::OpenEnded(_) => None,
    }
}

fn summary_period_fields(period: &FilingPeriod) -> (Option<u8>, Option<u8>) {
    match period {
        FilingPeriod::Monthly(month) => (None, Some(*month)),
        FilingPeriod::Quarterly(quarter) => (Some(*quarter), None),
        FilingPeriod::Annual | FilingPeriod::OpenEnded(_) => (None, None),
    }
}

impl Database {
    fn next_open_ended_period_number(
        &self,
        tin: &str,
        form_code: &str,
        year: u16,
    ) -> Result<u32, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT period_key FROM form_drafts
             WHERE tin = ?1 AND form_code = ?2 AND taxable_year = ?3
               AND period_key LIKE 'O%'",
        )?;
        let rows = stmt.query_map(params![tin, form_code, year as i64], |row| {
            row.get::<_, String>(0)
        })?;

        let mut max_key = 0;
        for row in rows {
            if let Some(FilingPeriod::OpenEnded(key)) = FilingPeriod::from_period_key(&row?) {
                max_key = max_key.max(key);
            }
        }

        Ok(max_key.saturating_add(1).max(1))
    }

    /// Save or update a generic form draft.
    /// Uses `period_key` as the real period identity and keeps the legacy
    /// `quarter` column populated only for monthly/quarterly compatibility.
    pub fn save_form_draft<T: serde::Serialize>(
        &self,
        tin: &str,
        form_code: &str,
        year: u16,
        quarter: Option<u8>,
        status: &FilingStatus,
        draft: &T,
    ) -> Result<i64, DbError> {
        let default_open_ended_key =
            if matches!(frequency_for_form(form_code), FilingFrequency::OpenEnded)
                && quarter.is_none_or(|value| value == 0)
            {
                self.next_open_ended_period_number(tin, form_code, year)?
            } else {
                1
            };
        let period = period_from_legacy_slot(form_code, quarter, default_open_ended_key);
        self.save_form_draft_v2(tin, form_code, year, &period, status, draft)
    }

    /// Load a generic form draft for a specific (tin, form_code, year, quarter).
    pub fn get_form_draft<T: serde::de::DeserializeOwned>(
        &self,
        tin: &str,
        form_code: &str,
        year: u16,
        quarter: Option<u8>,
    ) -> Result<Option<T>, DbError> {
        let period = period_from_legacy_slot(form_code, quarter, 1);
        if let Some(draft) = self.get_form_draft_v2(tin, form_code, year, &period)? {
            return Ok(Some(draft));
        }

        let mut stmt;
        let mut rows = if let Some(q) = quarter {
            stmt = self.conn.prepare(
                "SELECT data_json FROM form_drafts
                 WHERE tin = ?1 AND form_code = ?2
                   AND taxable_year = ?3 AND quarter = ?4",
            )?;
            stmt.query(params![tin, form_code, year as i64, q as i64])?
        } else {
            stmt = self.conn.prepare(
                "SELECT data_json FROM form_drafts
                 WHERE tin = ?1 AND form_code = ?2
                   AND taxable_year = ?3 AND quarter IS NULL",
            )?;
            stmt.query(params![tin, form_code, year as i64])?
        };

        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            let draft: T = serde_json::from_str(&json)?;
            Ok(Some(draft))
        } else {
            Ok(None)
        }
    }

    /// Save or update a Form 2551Q draft.
    /// Uses UPSERT on (tin, form_code, taxable_year, quarter).
    pub fn save_2551q_draft(&self, draft: &Form2551QDraft) -> Result<i64, DbError> {
        let json = serde_json::to_string(draft)?;
        let status = filing_status_to_db(&draft.status);
        let quarter = draft.quarter as i64;
        let period_key = FilingPeriod::Quarterly(draft.quarter).to_period_key();

        self.conn.execute(
            "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(tin, form_code, taxable_year, quarter)
             DO UPDATE SET status = excluded.status,
                           data_json = excluded.data_json,
                           period_key = excluded.period_key,
                           updated_at = datetime('now')",
            params![
                draft.tin,
                "2551Q",
                draft.taxable_year as i64,
                quarter,
                period_key,
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

    /// Save or update a Form 1601C draft.
    /// Uses UPSERT on (tin, form_code, taxable_year, quarter) where quarter = month.
    pub fn save_1601c_draft(
        &self,
        draft: &crate::forms::form_1601c::Form1601CDraft,
    ) -> Result<i64, DbError> {
        let json = serde_json::to_string(draft)?;
        let status = filing_status_to_db(&draft.status);
        let quarter = draft.month as i64; // Repurpose quarter column
        let period_key = FilingPeriod::Monthly(draft.month).to_period_key();

        self.conn.execute(
            "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(tin, form_code, taxable_year, quarter)
             DO UPDATE SET status = excluded.status,
                           data_json = excluded.data_json,
                           period_key = excluded.period_key,
                           updated_at = datetime('now')",
            params![
                draft.tin,
                "1601C",
                draft.taxable_year as i64,
                quarter,
                period_key,
                status,
                json
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        Ok(id)
    }

    /// Load a 1601C draft for a specific (tin, year, month).
    pub fn get_1601c_draft(
        &self,
        tin: &str,
        year: u16,
        month: u8,
    ) -> Result<Option<crate::forms::form_1601c::Form1601CDraft>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT data_json FROM form_drafts
             WHERE tin = ?1 AND form_code = '1601C'
               AND taxable_year = ?2 AND quarter = ?3",
        )?;
        let mut rows = stmt.query(params![tin, year as i64, month as i64])?;
        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            let draft: crate::forms::form_1601c::Form1601CDraft = serde_json::from_str(&json)?;
            Ok(Some(draft))
        } else {
            Ok(None)
        }
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
        let legacy_slot = match frequency_for_form(form_code) {
            FilingFrequency::Monthly => month.or(quarter),
            FilingFrequency::Quarterly => quarter.or(month),
            FilingFrequency::Annual => None,
            FilingFrequency::OpenEnded => quarter.or(month),
        };
        let default_open_ended_key =
            if matches!(frequency_for_form(form_code), FilingFrequency::OpenEnded)
                && legacy_slot.is_none_or(|value| value == 0)
            {
                self.next_open_ended_period_number(tin, form_code, year)?
            } else {
                1
            };
        let period = period_from_legacy_slot(form_code, legacy_slot, default_open_ended_key);
        let legacy_slot = legacy_slot_for_period(&period);
        let period_key = period.to_period_key();

        let rows_updated = self.conn.execute(
            "UPDATE form_drafts
             SET quarter = ?4,
                 status = 'Submitted',
                 data_json = '{}',
                 updated_at = datetime('now')
             WHERE tin = ?1
               AND form_code = ?2
               AND taxable_year = ?3
               AND period_key = ?5",
            params![tin, form_code, year as i64, legacy_slot, &period_key],
        )?;

        if rows_updated == 0 {
            self.conn.execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'Submitted', '{}')",
                params![tin, form_code, year as i64, legacy_slot, &period_key],
            )?;
        }

        let id = self.conn.query_row(
            "SELECT id FROM form_drafts
             WHERE tin = ?1 AND form_code = ?2 AND taxable_year = ?3 AND period_key = ?4
             ORDER BY updated_at DESC, id DESC
             LIMIT 1",
            params![tin, form_code, year as i64, &period_key],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(id)
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
            "SELECT quarter, period_key, status FROM form_drafts
             WHERE tin = ?1 AND form_code = ?2 AND taxable_year = ?3",
        )?;
        let rows = stmt.query_map(params![tin, form_code, year as i64], |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        for row in rows {
            let (legacy_slot, period_key, status_str) = row?;
            let state = quarter_state_from_db(&status_str);
            let period = period_from_row(form_code, legacy_slot, period_key.as_deref());
            match period {
                FilingPeriod::Monthly(month) => {
                    let idx = usize::from(month - 1);
                    progress.months[idx] = state;
                }
                FilingPeriod::Quarterly(quarter) => {
                    let idx = usize::from(quarter - 1);
                    progress.quarters[idx] = state;
                }
                FilingPeriod::Annual => {
                    progress.annual_status = state;
                }
                FilingPeriod::OpenEnded(_) => {
                    if counts_as_started_or_filed(&state) {
                        progress.open_ended_count = progress.open_ended_count.saturating_add(1);
                    }
                }
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
            "SELECT id, tin, form_code, taxable_year, quarter, period_key, status, updated_at
             FROM form_drafts WHERE tin = ?1 AND taxable_year = ?2",
        )?;
        let rows = stmt.query_map(params![tin, year as i64], |row| {
            let form_code: String = row.get(2)?;
            let period_val = row.get::<_, Option<i64>>(4)?.map(|q| q as u8);
            let legacy_slot = row.get::<_, Option<i64>>(4)?;
            let period_key = row.get::<_, Option<String>>(5)?;
            let period = period_from_row(&form_code, legacy_slot, period_key.as_deref());
            let (quarter, month) = summary_period_fields(&period);
            let frequency = frequency_for_form(&form_code);
            let quarter = quarter.or({
                if matches!(&frequency, FilingFrequency::Quarterly) {
                    period_val
                } else {
                    None
                }
            });
            let month = month.or({
                if matches!(&frequency, FilingFrequency::Monthly) {
                    period_val
                } else {
                    None
                }
            });

            Ok(FormDraftSummary {
                id: row.get(0)?,
                tin: row.get(1)?,
                form_code,
                taxable_year: row.get::<_, i64>(3)? as u16,
                quarter,
                month,
                status: filing_status_from_db(row.get::<_, String>(6)?.as_str()),
                updated_at: row.get(7)?,
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
            "SELECT id, tin, form_code, taxable_year, quarter, period_key, status, updated_at
             FROM form_drafts
             WHERE (status = 'Queued' OR status = 'Submitted')",
        )?;
        let rows = stmt.query_map([], |row| {
            let form_code: String = row.get(2)?;
            let period_val = row.get::<_, Option<i64>>(4)?.map(|q| q as u8);
            let legacy_slot = row.get::<_, Option<i64>>(4)?;
            let period_key = row.get::<_, Option<String>>(5)?;
            let period = period_from_row(&form_code, legacy_slot, period_key.as_deref());
            let (quarter, month) = summary_period_fields(&period);
            let frequency = frequency_for_form(&form_code);
            let quarter = quarter.or({
                if matches!(&frequency, FilingFrequency::Quarterly) {
                    period_val
                } else {
                    None
                }
            });
            let month = month.or({
                if matches!(&frequency, FilingFrequency::Monthly) {
                    period_val
                } else {
                    None
                }
            });

            Ok(FormDraftSummary {
                id: row.get(0)?,
                tin: row.get(1)?,
                form_code,
                taxable_year: row.get::<_, i64>(3)? as u16,
                quarter,
                month,
                status: filing_status_from_db(row.get::<_, String>(6)?.as_str()),
                updated_at: row.get(7)?,
            })
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            let summary = row?;
            if crate::temporal::can_queue_for_submission(&summary.form_code) {
                summaries.push(summary);
            }
        }
        Ok(summaries)
    }

    // ── Period-key-aware methods (v2) ──
    //
    // These use the `period_key` column added in v5 migration.
    // They complement the legacy methods above which use the raw `quarter` column.

    /// Save or update a form draft using a `period_key` for unified period handling.
    ///
    /// Updates by (tin, form_code, taxable_year, period_key), then inserts if absent.
    /// This avoids SQLite's nullable `quarter` uniqueness edge case for annual/open-ended forms.
    pub fn save_form_draft_v2<T: serde::Serialize>(
        &self,
        tin: &str,
        form_code: &str,
        year: u16,
        period: &crate::forms::FilingPeriod,
        status: &FilingStatus,
        draft: &T,
    ) -> Result<i64, DbError> {
        if matches!(status, FilingStatus::Queued)
            && !crate::temporal::can_queue_for_submission(form_code)
        {
            return Err(DbError::Other(format!(
                "Form {form_code} is scaffold-only and cannot be queued for submission"
            )));
        }

        let json = serde_json::to_string(draft)?;
        let status_str = filing_status_to_db(status);
        let period_key = period.to_period_key();

        // Also set the legacy quarter column for backward compatibility
        let quarter_val = legacy_slot_for_period(period);

        let rows_updated = self.conn.execute(
            "UPDATE form_drafts
             SET quarter = ?4,
                 status = ?5,
                 data_json = ?6,
                 updated_at = datetime('now')
             WHERE tin = ?1
               AND form_code = ?2
               AND taxable_year = ?3
               AND period_key = ?7",
            params![
                tin,
                form_code,
                year as i64,
                quarter_val,
                status_str,
                json,
                &period_key
            ],
        )?;

        if rows_updated == 0 {
            self.conn.execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    tin,
                    form_code,
                    year as i64,
                    quarter_val,
                    &period_key,
                    status_str,
                    json
                ],
            )?;
        }

        let id = self.conn.query_row(
            "SELECT id FROM form_drafts
             WHERE tin = ?1 AND form_code = ?2 AND taxable_year = ?3 AND period_key = ?4
             ORDER BY updated_at DESC, id DESC
             LIMIT 1",
            params![tin, form_code, year as i64, &period_key],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(id)
    }

    /// Load a form draft by period_key.
    pub fn get_form_draft_v2<T: serde::de::DeserializeOwned>(
        &self,
        tin: &str,
        form_code: &str,
        year: u16,
        period: &crate::forms::FilingPeriod,
    ) -> Result<Option<T>, DbError> {
        let period_key = period.to_period_key();
        let mut stmt = self.conn.prepare(
            "SELECT data_json FROM form_drafts
             WHERE tin = ?1 AND form_code = ?2
               AND taxable_year = ?3 AND period_key = ?4
             ORDER BY updated_at DESC, id DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![tin, form_code, year as i64, period_key])?;

        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            let draft: T = serde_json::from_str(&json)?;
            Ok(Some(draft))
        } else {
            Ok(None)
        }
    }
}
