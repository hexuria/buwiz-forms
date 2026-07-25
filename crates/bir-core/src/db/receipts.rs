//! Receipt repository — save and query submission receipts.

use rusqlite::params;

use super::{Database, DbError, SubmissionReceipt, parse_2551q_period};
use crate::forms::FilingStatus;
use crate::receipt::{BirReceiptConfirmation, split_bir_filename};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptConfirmationOutcome {
    Confirmed,
    Ignored,
}

pub(super) fn is_audited_2551q_receipt_form_type(form_type: &str) -> bool {
    matches!(form_type, "2551Qv2018" | "2551Q")
}

impl Database {
    pub fn save_submission_receipt(
        &self,
        receipt: &BirReceiptConfirmation,
    ) -> Result<(SubmissionReceipt, bool), DbError> {
        let (tin, form_type, period) = split_bir_filename(&receipt.filename)
            .unwrap_or_else(|| ("".to_string(), "".to_string(), "".to_string()));

        let received_date_str = receipt.date_received.to_string();
        let received_time_str = receipt.time_received.format("%H:%M:%S").to_string();

        if let Some(existing) = self.get_submission_receipt_by_filename(&receipt.filename)?
            && existing.received_date == received_date_str
            && existing.received_time == received_time_str
        {
            // It's the exact same receipt we already processed. Return false for is_new.
            return Ok((existing, false));
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

    pub fn confirm_2551q_from_receipt(
        &self,
        receipt: &SubmissionReceipt,
    ) -> Result<ReceiptConfirmationOutcome, DbError> {
        if !is_audited_2551q_receipt_form_type(&receipt.form_type) {
            return Ok(ReceiptConfirmationOutcome::Ignored);
        }

        let Some((year, quarter)) = parse_2551q_period(&receipt.period) else {
            return Ok(ReceiptConfirmationOutcome::Ignored);
        };

        let mut draft = match self.get_2551q_draft(&receipt.tin, year, quarter)? {
            Some(draft) => draft,
            None => return Ok(ReceiptConfirmationOutcome::Ignored),
        };

        // Only confirm forms that are currently in Submitted status
        if !matches!(draft.status, FilingStatus::Submitted) {
            return Ok(ReceiptConfirmationOutcome::Ignored);
        }
        let receipt_id = receipt.id.ok_or_else(|| {
            DbError::Other(
                "A 2551Q receipt must be persisted before it can confirm a submission".to_string(),
            )
        })?;

        let submitted_at = draft.submitted_at.as_deref().ok_or_else(|| {
            DbError::Other("Submitted 2551Q draft has no submission timestamp".to_string())
        })?;
        let submitted_dt = chrono::DateTime::parse_from_rfc3339(submitted_at).map_err(|error| {
            DbError::Other(format!(
                "Submitted 2551Q draft has an invalid submission timestamp: {error}"
            ))
        })?;
        let date_str = format!("{}T{}", receipt.received_date, receipt.received_time);
        let receipt_naive = chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S")
            .map_err(|error| {
            DbError::Other(format!(
                "Receipt has an invalid received timestamp: {error}"
            ))
        })?;
        let offset = chrono::FixedOffset::east_opt(8 * 3600)
            .ok_or_else(|| DbError::Other("UTC+08:00 offset is unavailable".to_string()))?;
        use chrono::TimeZone;
        let receipt_dt = offset
            .from_local_datetime(&receipt_naive)
            .single()
            .ok_or_else(|| DbError::Other("Receipt received timestamp is ambiguous".to_string()))?;
        if receipt_dt < submitted_dt {
            tracing::info!(
                "Ignoring old receipt {} for draft submitted at {}",
                receipt.filename,
                submitted_dt
            );
            return Ok(ReceiptConfirmationOutcome::Ignored);
        }

        draft.transition_to_confirmed(date_str, Some(receipt_id), Some(receipt.filename.clone()));
        self.save_confirmed_2551q_draft(&draft)?;
        Ok(ReceiptConfirmationOutcome::Confirmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(form_type: &str) -> SubmissionReceipt {
        SubmissionReceipt {
            id: None,
            filename: format!("123456789000-{form_type}-122026Q1.xml"),
            tin: "123456789000".to_string(),
            form_type: form_type.to_string(),
            period: "122026Q1".to_string(),
            received_date: "2026-04-25".to_string(),
            received_time: "12:00:00".to_string(),
            source_from: None,
            raw_text: "test receipt".to_string(),
            raw_html: None,
            created_at: None,
        }
    }

    #[test]
    fn receipt_form_aliases_are_closed_and_explicit() {
        assert!(is_audited_2551q_receipt_form_type("2551Qv2018"));
        assert!(is_audited_2551q_receipt_form_type("2551Q"));
        assert!(!is_audited_2551q_receipt_form_type("2551Qv2024"));
        assert!(!is_audited_2551q_receipt_form_type("1601C"));
    }

    #[test]
    fn unrelated_or_unmatched_receipts_report_ignored() {
        let database = Database::open_in_memory_for_tests().unwrap();
        assert_eq!(
            database
                .confirm_2551q_from_receipt(&receipt("1601C"))
                .unwrap(),
            ReceiptConfirmationOutcome::Ignored
        );
        assert_eq!(
            database
                .confirm_2551q_from_receipt(&receipt("2551Q"))
                .unwrap(),
            ReceiptConfirmationOutcome::Ignored
        );
    }
}
