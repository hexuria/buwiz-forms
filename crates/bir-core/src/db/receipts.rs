//! Receipt repository — save and query submission receipts.

use rusqlite::params;

use super::{Database, DbError, SubmissionReceipt, parse_2551q_period};
use crate::forms::FilingStatus;
use crate::receipt::{BirReceiptConfirmation, split_bir_filename};

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

        // Only confirm forms that are currently in Submitted status
        if !matches!(draft.status, FilingStatus::Submitted) {
            return Ok(());
        }

        if let Some(submitted_at) = &draft.submitted_at
            && let Ok(submitted_dt) = chrono::DateTime::parse_from_rfc3339(submitted_at)
        {
            let date_str = format!("{}T{}", receipt.received_date, receipt.received_time);
            if let Ok(receipt_naive) =
                chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S")
                && let Some(offset) = chrono::FixedOffset::east_opt(8 * 3600)
            {
                use chrono::TimeZone;
                if let chrono::LocalResult::Single(receipt_dt) =
                    offset.from_local_datetime(&receipt_naive)
                    && receipt_dt + chrono::Duration::minutes(5) < submitted_dt
                {
                    tracing::info!(
                        "Ignoring old receipt {} for draft submitted at {}",
                        receipt.filename,
                        submitted_dt
                    );
                    return Ok(());
                }
            }
        }

        draft.transition_to_confirmed(
            format!("{}T{}", receipt.received_date, receipt.received_time),
            receipt.id,
            Some(receipt.filename.clone()),
        );
        self.save_2551q_draft(&draft)?;
        Ok(())
    }
}
