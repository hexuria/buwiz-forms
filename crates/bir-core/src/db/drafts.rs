//! Form drafts repository — save, load, and list tax form drafts.

use rusqlite::params;

use super::{Database, DbError};
use crate::forms::{
    FilingStatus, FormDraftSummary, FormFilingProgress, QuarterState,
};
use crate::forms::form_2551q::Form2551QDraft;

impl Database {
    /// Save or update a generic form draft.
    /// Uses UPSERT on (tin, form_code, taxable_year, quarter).
    pub fn save_form_draft<T: serde::Serialize>(
        &self,
        tin: &str,
        form_code: &str,
        year: u16,
        quarter: Option<u8>,
        status: &FilingStatus,
        draft: &T,
    ) -> Result<i64, DbError> {
        let json = serde_json::to_string(draft)?;
        let status_str = match status {
            FilingStatus::Draft => "Draft",
            FilingStatus::Queued => "Queued",
            FilingStatus::Submitted => "Submitted",
            FilingStatus::Confirmed => "Confirmed",
            FilingStatus::Paid => "Paid",
        };

        // Note: SQLite treats NULL as distinct in UNIQUE constraints by default (unless configured otherwise).
        // For forms with no quarter (like annual forms), we insert NULL.
        // We need an index or conflict clause that handles NULLs properly, but our current schema has:
        // UNIQUE(tin, form_code, taxable_year, quarter)
        // Since sqlite treats NULL != NULL, we might get multiple rows if we just use ON CONFLICT.
        // For now, if quarter is None, we default to 0 for the constraint, or we need to ensure the DB schema handles it.
        // Our schema actually allows NULL quarter. If this causes duplicate issues for annual forms, we'll fix the schema in Phase 1.
        let quarter_val = quarter.map(|q| q as i64);

        self.conn.execute(
            "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, status, data_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(tin, form_code, taxable_year, quarter)
             DO UPDATE SET status = excluded.status,
                           data_json = excluded.data_json,
                           updated_at = datetime('now')",
            params![tin, form_code, year as i64, quarter_val, status_str, json],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Load a generic form draft for a specific (tin, form_code, year, quarter).
    pub fn get_form_draft<T: serde::de::DeserializeOwned>(
        &self,
        tin: &str,
        form_code: &str,
        year: u16,
        quarter: Option<u8>,
    ) -> Result<Option<T>, DbError> {
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

    /// Save or update a Form 1601C draft.
    /// Uses UPSERT on (tin, form_code, taxable_year, quarter) where quarter = month.
    pub fn save_1601c_draft(&self, draft: &crate::forms::form_1601c::Form1601CDraft) -> Result<i64, DbError> {
        let json = serde_json::to_string(draft)?;
        let status = match draft.status {
            FilingStatus::Draft => "Draft",
            FilingStatus::Queued => "Queued",
            FilingStatus::Submitted => "Submitted",
            FilingStatus::Confirmed => "Confirmed",
            FilingStatus::Paid => "Paid",
        };
        let quarter = draft.month as i64; // Repurpose quarter column

        self.conn.execute(
            "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, status, data_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(tin, form_code, taxable_year, quarter)
             DO UPDATE SET status = excluded.status,
                           data_json = excluded.data_json,
                           updated_at = datetime('now')",
            params![
                draft.tin,
                "1601C",
                draft.taxable_year as i64,
                quarter,
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
            let form_code: String = row.get(2)?;
            let is_1601c = form_code == "1601C";
            let period_val = row.get::<_, Option<i64>>(4)?.map(|q| q as u8);
            
            Ok(FormDraftSummary {
                id: row.get(0)?,
                tin: row.get(1)?,
                form_code,
                taxable_year: row.get::<_, i64>(3)? as u16,
                quarter: if is_1601c { None } else { period_val },
                month: if is_1601c { period_val } else { None },
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
            let form_code: String = row.get(2)?;
            let is_1601c = form_code == "1601C";
            let period_val = row.get::<_, Option<i64>>(4)?.map(|q| q as u8);
            
            Ok(FormDraftSummary {
                id: row.get(0)?,
                tin: row.get(1)?,
                form_code,
                taxable_year: row.get::<_, i64>(3)? as u16,
                quarter: if is_1601c { None } else { period_val },
                month: if is_1601c { period_val } else { None },
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
}
