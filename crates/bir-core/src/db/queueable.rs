//! `form_drafts` storage for every [`QueueableForm`].
//!
//! Each function mirrors its 1601C counterpart in `drafts.rs`: the same
//! status rules and the same compare-and-swap on `data_json`, so two workers
//! or a worker and the user can never both win a transition.

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::drafts::{
    AbandonedClaimRelease, abandoned_claim_audit, claim_fields_partial, claim_fields_present,
    filing_status_to_db, require_abandoned_release_reason,
};
use super::receipts::ReceiptConfirmationOutcome;
use super::{Database, DbError, SubmissionReceipt};
use crate::forms::FilingStatus;
use crate::forms::queueable::QueueableForm;

/// Result of claiming one exact queued generation before PUT.
pub enum ClaimQueueableResult<F> {
    Claimed {
        draft: F,
        token: String,
    },
    Rejected {
        draft: F,
        errors: Vec<(String, String)>,
    },
    Superseded,
}

fn errors_summary(errors: &[(String, String)]) -> String {
    errors
        .iter()
        .map(|(field, message)| format!("{field}: {message}"))
        .collect::<Vec<_>>()
        .join("; ")
}

type Row = (i64, String, String);

fn select_row(
    tx: &Transaction<'_>,
    form_code: &str,
    tin: &str,
    taxable_year: u16,
    period: i64,
) -> Result<Option<Row>, DbError> {
    Ok(tx
        .query_row(
            "SELECT id, data_json, status FROM form_drafts
             WHERE tin = ?1 AND form_code = ?2 AND taxable_year = ?3 AND quarter = ?4",
            params![tin, form_code, i64::from(taxable_year), period],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?)
}

impl Database {
    pub fn get_queueable_draft<F: QueueableForm>(
        &self,
        tin: &str,
        taxable_year: u16,
        period: i64,
    ) -> Result<Option<F>, DbError> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT data_json FROM form_drafts
                 WHERE tin = ?1 AND form_code = ?2 AND taxable_year = ?3 AND quarter = ?4",
                params![tin, F::FORM_CODE, i64::from(taxable_year), period],
                |row| row.get(0),
            )
            .optional()?;
        raw.map(|json| serde_json::from_str(&json).map_err(DbError::from))
            .transpose()
    }

    /// Save an editable Draft. A queued or filed snapshot is never replaced.
    pub fn save_queueable_draft<F: QueueableForm>(&self, draft: &F) -> Result<i64, DbError> {
        if !draft.lifecycle().is_editable() {
            return Err(DbError::Other(format!(
                "Only an editable Draft {} return may use the draft save path",
                F::FORM_CODE
            )));
        }
        self.upsert_from_draft(draft, "Draft")
    }

    /// Persist the exact reviewed queue snapshot after revalidating it.
    pub fn save_queued_queueable<F: QueueableForm>(&self, draft: &F) -> Result<i64, DbError> {
        if !matches!(draft.lifecycle().status, FilingStatus::Queued) {
            return Err(DbError::Other(format!(
                "Only a queued {} return can be saved through the submission path",
                F::FORM_CODE
            )));
        }
        let mut verified = draft.clone();
        verified
            .revalidate_queued_before_submission()
            .map_err(|errors| {
                DbError::Other(format!(
                    "Queued {} return failed queue revalidation: {}",
                    F::FORM_CODE,
                    errors_summary(&errors)
                ))
            })?;
        self.upsert_from_draft(&verified, "Queued")
    }

    /// Write `draft` with `status` over an existing Draft row, or insert it.
    fn upsert_from_draft<F: QueueableForm>(&self, draft: &F, status: &str) -> Result<i64, DbError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let json = serde_json::to_string(draft)?;
        let period = draft.period_column();
        let period_key = draft.filing_period().to_period_key();
        let id = match select_row(&tx, F::FORM_CODE, draft.tin(), draft.taxable_year(), period)? {
            Some((id, raw_json, db_status)) => {
                let stored: F = serde_json::from_str(&raw_json)?;
                if db_status != "Draft" || !stored.lifecycle().is_editable() {
                    return Err(DbError::Other(format!(
                        "A queued or filed {} snapshot must be canceled back to Draft before it can be replaced",
                        F::FORM_CODE
                    )));
                }
                let updated = tx.execute(
                    "UPDATE form_drafts
                     SET status = ?1, data_json = ?2, period_key = ?3, updated_at = datetime('now')
                     WHERE id = ?4 AND status = 'Draft' AND data_json = ?5",
                    params![status, json, period_key, id, raw_json],
                )?;
                if updated != 1 {
                    return Err(DbError::Other(format!(
                        "{} draft changed before the save completed",
                        F::FORM_CODE
                    )));
                }
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO form_drafts
                        (tin, form_code, taxable_year, quarter, period_key, status, data_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        draft.tin(),
                        F::FORM_CODE,
                        i64::from(draft.taxable_year()),
                        period,
                        period_key,
                        status,
                        json
                    ],
                )?;
                tx.last_insert_rowid()
            }
        };
        tx.commit()?;
        let _ = self.request_google_calendar_sync();
        Ok(id)
    }

    /// CAS-replace an unclaimed queue generation with a retry update or a
    /// return to Draft. The caller names the exact generation it loaded.
    pub(crate) fn replace_unclaimed_queued_queueable<F: QueueableForm>(
        &self,
        replacement: &F,
        expected_fingerprint: &Option<String>,
        expected_next_retry_at: &Option<String>,
        expected_submission_attempts: u32,
    ) -> Result<bool, DbError> {
        let mut replacement = replacement.clone();
        let status = replacement.lifecycle().status.clone();
        if !matches!(status, FilingStatus::Draft | FilingStatus::Queued) {
            return Err(DbError::Other(format!(
                "An unclaimed {} queue generation may only remain Queued or return to Draft",
                F::FORM_CODE
            )));
        }
        if matches!(status, FilingStatus::Queued) {
            if &replacement.lifecycle().queued_submission_fingerprint != expected_fingerprint {
                return Err(DbError::Other(format!(
                    "A retry update cannot change the reviewed {} queue fingerprint",
                    F::FORM_CODE
                )));
            }
            replacement
                .revalidate_queued_before_submission()
                .map_err(|errors| {
                    DbError::Other(format!(
                        "A retry update cannot persist invalid {} submission fields: {}",
                        F::FORM_CODE,
                        errors_summary(&errors)
                    ))
                })?;
        }
        if !replacement.lifecycle().is_unclaimed() {
            return Err(DbError::Other(format!(
                "An unclaimed {} replacement cannot carry a network claim",
                F::FORM_CODE
            )));
        }

        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((id, raw_json, db_status)) = select_row(
            &tx,
            F::FORM_CODE,
            replacement.tin(),
            replacement.taxable_year(),
            replacement.period_column(),
        )?
        else {
            return Ok(false);
        };
        let current: F = serde_json::from_str(&raw_json)?;
        let lifecycle = current.lifecycle();
        if db_status != "Queued"
            || !matches!(lifecycle.status, FilingStatus::Queued)
            || !lifecycle.is_unclaimed()
            || &lifecycle.queued_submission_fingerprint != expected_fingerprint
            || &lifecycle.next_retry_at != expected_next_retry_at
            || lifecycle.submission_attempts != expected_submission_attempts
        {
            return Ok(false);
        }
        let json = serde_json::to_string(&replacement)?;
        let updated = tx.execute(
            "UPDATE form_drafts SET status = ?1, data_json = ?2, updated_at = datetime('now')
             WHERE id = ?3 AND status = 'Queued' AND data_json = ?4",
            params![
                filing_status_to_db(&replacement.lifecycle().status),
                json,
                id,
                raw_json
            ],
        )?;
        if updated != 1 {
            return Ok(false);
        }
        tx.commit()?;
        let _ = self.request_google_calendar_sync();
        Ok(true)
    }

    /// Cancel one exact, still-unclaimed queue generation back to Draft.
    pub fn cancel_queued_queueable<F: QueueableForm>(&self, queued: &F) -> Result<F, DbError> {
        if !matches!(queued.lifecycle().status, FilingStatus::Queued)
            || !queued.lifecycle().is_unclaimed()
        {
            return Err(DbError::Other(format!(
                "Only an unclaimed queued {} snapshot can be canceled",
                F::FORM_CODE
            )));
        }
        let mut verified = queued.clone();
        verified
            .revalidate_queued_before_submission()
            .map_err(|errors| {
                DbError::Other(format!(
                    "Only the exact reviewed {} queue snapshot can be canceled: {}",
                    F::FORM_CODE,
                    errors_summary(&errors)
                ))
            })?;
        let lifecycle = verified.lifecycle().clone();
        let mut draft = verified;
        draft.lifecycle_mut().revert_to_draft();
        if !self.replace_unclaimed_queued_queueable(
            &draft,
            &lifecycle.queued_submission_fingerprint,
            &lifecycle.next_retry_at,
            lifecycle.submission_attempts,
        )? {
            return Err(DbError::Other(format!(
                "{} submission has already started or the queue generation changed",
                F::FORM_CODE
            )));
        }
        Ok(draft)
    }

    /// Human-confirmed release of a claimed queue snapshot back to Draft.
    pub fn release_abandoned_claimed_queueable<F: QueueableForm>(
        &self,
        tin: &str,
        taxable_year: u16,
        period: i64,
        reason: &str,
    ) -> Result<AbandonedClaimRelease<F>, DbError> {
        require_abandoned_release_reason(reason)?;
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((id, raw_json, db_status)) =
            select_row(&tx, F::FORM_CODE, tin, taxable_year, period)?
        else {
            return Ok(AbandonedClaimRelease::AlreadyClear {
                previous_status: None,
                draft: None,
                reason: reason.to_string(),
            });
        };
        let current: F = serde_json::from_str(&raw_json)?;
        let lifecycle = current.lifecycle();
        let claimed = claim_fields_present(
            &lifecycle.submission_claim_token,
            &lifecycle.submission_claimed_at,
        );
        let partial = claim_fields_partial(
            &lifecycle.submission_claim_token,
            &lifecycle.submission_claimed_at,
        );
        if matches!(lifecycle.status, FilingStatus::Draft)
            && db_status == "Draft"
            && !claimed
            && !partial
        {
            return Ok(AbandonedClaimRelease::AlreadyClear {
                previous_status: Some(FilingStatus::Draft),
                draft: Some(current),
                reason: reason.to_string(),
            });
        }
        if matches!(
            lifecycle.status,
            FilingStatus::Submitted | FilingStatus::Confirmed | FilingStatus::Paid
        ) || matches!(db_status.as_str(), "Submitted" | "Confirmed" | "Paid")
        {
            return Err(DbError::Other(format!(
                "Will not release a {} claim that is already Submitted, Confirmed, or Paid",
                F::FORM_CODE
            )));
        }
        if partial {
            return Err(DbError::Other(format!(
                "{} claim metadata is incomplete; refusing to wipe an unknown filing state",
                F::FORM_CODE
            )));
        }
        if !(matches!(lifecycle.status, FilingStatus::Queued) && db_status == "Queued" && claimed) {
            return Err(DbError::Other(format!(
                "{} is not an abandoned claimed queue snapshot; unclaimed queues are canceled instead",
                F::FORM_CODE
            )));
        }
        let previous_claimed_at = lifecycle.submission_claimed_at.clone();
        let mut draft = current;
        draft.lifecycle_mut().revert_to_draft();
        draft.lifecycle_mut().submission_error = Some(abandoned_claim_audit(reason));
        let json = serde_json::to_string(&draft)?;
        let updated = tx.execute(
            "UPDATE form_drafts SET status = 'Draft', data_json = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND status = 'Queued' AND data_json = ?3",
            params![json, id, raw_json],
        )?;
        if updated != 1 {
            return Err(DbError::Other(format!(
                "{} claim changed before abandoned-claim release completed",
                F::FORM_CODE
            )));
        }
        tx.commit()?;
        let _ = self.request_google_calendar_sync();
        Ok(AbandonedClaimRelease::Released {
            previous_status: FilingStatus::Queued,
            previous_claim_present: true,
            previous_claimed_at,
            draft,
            reason: reason.to_string(),
        })
    }

    /// Atomically revalidate and claim the exact queued generation right
    /// before PUT. The worker opens the SFTP session first, so a connect or
    /// login failure stays unclaimed.
    pub fn claim_queued_queueable<F: QueueableForm>(
        &self,
        tin: &str,
        taxable_year: u16,
        period: i64,
        expected_fingerprint: &Option<String>,
        expected_next_retry_at: &Option<String>,
        expected_submission_attempts: u32,
    ) -> Result<ClaimQueueableResult<F>, DbError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((id, raw_json, db_status)) =
            select_row(&tx, F::FORM_CODE, tin, taxable_year, period)?
        else {
            return Ok(ClaimQueueableResult::Superseded);
        };
        let mut draft: F = serde_json::from_str(&raw_json)?;
        let lifecycle = draft.lifecycle();
        if db_status != "Queued"
            || !matches!(lifecycle.status, FilingStatus::Queued)
            || !lifecycle.is_unclaimed()
            || &lifecycle.queued_submission_fingerprint != expected_fingerprint
            || &lifecycle.next_retry_at != expected_next_retry_at
            || lifecycle.submission_attempts != expected_submission_attempts
        {
            return Ok(ClaimQueueableResult::Superseded);
        }

        if let Err(errors) = draft.revalidate_queued_before_submission() {
            let rejected_json = serde_json::to_string(&draft)?;
            let updated = tx.execute(
                "UPDATE form_drafts SET status = 'Draft', data_json = ?1, updated_at = datetime('now')
                 WHERE id = ?2 AND status = 'Queued' AND data_json = ?3",
                params![rejected_json, id, raw_json],
            )?;
            if updated != 1 {
                return Ok(ClaimQueueableResult::Superseded);
            }
            tx.commit()?;
            return Ok(ClaimQueueableResult::Rejected { draft, errors });
        }

        let token = uuid::Uuid::new_v4().to_string();
        draft
            .lifecycle_mut()
            .claim(token.clone(), chrono::Utc::now());
        let claimed_json = serde_json::to_string(&draft)?;
        let updated = tx.execute(
            "UPDATE form_drafts SET data_json = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND status = 'Queued' AND data_json = ?3",
            params![claimed_json, id, raw_json],
        )?;
        if updated != 1 {
            return Ok(ClaimQueueableResult::Superseded);
        }
        tx.commit()?;
        Ok(ClaimQueueableResult::Claimed { draft, token })
    }

    /// Record that PUT is about to start for the claim holder.
    pub fn mark_claimed_queueable_put_started<F: QueueableForm>(
        &self,
        tin: &str,
        taxable_year: u16,
        period: i64,
        claim_token: &str,
    ) -> Result<bool, DbError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((id, raw_json, db_status)) =
            select_row(&tx, F::FORM_CODE, tin, taxable_year, period)?
        else {
            return Ok(false);
        };
        if db_status != "Queued" {
            return Ok(false);
        }
        let mut draft: F = serde_json::from_str(&raw_json)?;
        if draft.lifecycle().submission_claim_token.as_deref() != Some(claim_token) {
            return Ok(false);
        }
        if draft.lifecycle().submission_put_started_at.is_none() {
            draft.lifecycle_mut().submission_put_started_at = Some(chrono::Utc::now().to_rfc3339());
        }
        let json = serde_json::to_string(&draft)?;
        let updated = tx.execute(
            "UPDATE form_drafts SET data_json = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND status = 'Queued' AND data_json = ?3",
            params![json, id, raw_json],
        )?;
        if updated != 1 {
            return Ok(false);
        }
        tx.commit()?;
        Ok(true)
    }

    /// Recover a claim whose lease expired before PUT started.
    pub fn recover_expired_unstarted_queueable_claim<F: QueueableForm>(
        &self,
        tin: &str,
        taxable_year: u16,
        period: i64,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, DbError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((id, raw_json, db_status)) =
            select_row(&tx, F::FORM_CODE, tin, taxable_year, period)?
        else {
            return Ok(false);
        };
        if db_status != "Queued" {
            return Ok(false);
        }
        let mut draft: F = serde_json::from_str(&raw_json)?;
        let lifecycle = draft.lifecycle();
        if !crate::filing_queue::claim_is_recoverable(
            lifecycle.submission_claimed_at.as_deref(),
            lifecycle.submission_claim_lease_until.as_deref(),
            lifecycle.submission_put_started_at.as_deref(),
            now,
        ) {
            return Ok(false);
        }
        if !draft.lifecycle_mut().release_expired_unstarted_claim(
            "Claim lease expired before PUT started; retrying the same authorized task".to_string(),
        ) {
            return Ok(false);
        }
        let json = serde_json::to_string(&draft)?;
        let updated = tx.execute(
            "UPDATE form_drafts SET status = 'Queued', data_json = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND status = 'Queued' AND data_json = ?3",
            params![json, id, raw_json],
        )?;
        if updated != 1 {
            return Ok(false);
        }
        tx.commit()?;
        crate::ipc::post_db_changed();
        Ok(true)
    }

    /// Finish a network claim as Submitted. The claim must still belong to
    /// this worker and the reviewed fields must be unchanged.
    pub(crate) fn finish_claimed_queueable_submission<F: QueueableForm>(
        &self,
        draft: &F,
        claim_token: &str,
    ) -> Result<i64, DbError> {
        if !matches!(draft.lifecycle().status, FilingStatus::Submitted) {
            return Err(DbError::Other(format!(
                "Only a Submitted {} snapshot can finish a network claim",
                F::FORM_CODE
            )));
        }
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((id, raw_json, db_status)) = select_row(
            &tx,
            F::FORM_CODE,
            draft.tin(),
            draft.taxable_year(),
            draft.period_column(),
        )?
        else {
            return Err(DbError::Other(format!(
                "Claimed {} draft disappeared before the network attempt finished",
                F::FORM_CODE
            )));
        };
        let existing: F = serde_json::from_str(&raw_json)?;
        let lifecycle = existing.lifecycle();
        if db_status != "Queued"
            || !matches!(lifecycle.status, FilingStatus::Queued)
            || lifecycle.submission_claim_token.as_deref() != Some(claim_token)
            || lifecycle.submission_claimed_at.is_none()
        {
            return Err(DbError::Other(format!(
                "{} submission claim no longer belongs to this worker",
                F::FORM_CODE
            )));
        }
        if draft.lifecycle().queued_submission_fingerprint
            != lifecycle.queued_submission_fingerprint
            || draft.field_map() != existing.field_map()
        {
            return Err(DbError::Other(format!(
                "Claimed {} submission fields changed before completion",
                F::FORM_CODE
            )));
        }
        let expected_filename = existing.submission_filename();
        if draft.lifecycle().submission_filename.as_deref() != Some(expected_filename.as_str())
            || draft.lifecycle().submitted_at.is_none()
        {
            return Err(DbError::Other(format!(
                "Claimed {} submission completion did not keep the reviewed IAF filename and timestamp",
                F::FORM_CODE
            )));
        }
        let json = serde_json::to_string(draft)?;
        let updated = tx.execute(
            "UPDATE form_drafts SET status = 'Submitted', data_json = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND status = 'Queued' AND data_json = ?3",
            params![json, id, raw_json],
        )?;
        if updated != 1 {
            return Err(DbError::Other(format!(
                "{} claim changed before submission completion was recorded",
                F::FORM_CODE
            )));
        }
        tx.commit()?;
        let _ = self.request_google_calendar_sync();
        Ok(id)
    }

    /// CAS-advance one exact Submitted snapshot to Confirmed.
    pub fn save_confirmed_queueable<F: QueueableForm>(&self, draft: &F) -> Result<i64, DbError> {
        let lifecycle = draft.lifecycle();
        if !matches!(lifecycle.status, FilingStatus::Confirmed)
            || lifecycle.confirmed_at.is_none()
            || !lifecycle.is_unclaimed()
        {
            return Err(DbError::Other(format!(
                "Only a completed {} confirmation may use the confirmation path",
                F::FORM_CODE
            )));
        }
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((id, raw_json, db_status)) = select_row(
            &tx,
            F::FORM_CODE,
            draft.tin(),
            draft.taxable_year(),
            draft.period_column(),
        )?
        else {
            return Err(DbError::Other(format!(
                "Submitted {} draft disappeared before confirmation",
                F::FORM_CODE
            )));
        };
        let existing: F = serde_json::from_str(&raw_json)?;
        if db_status != "Submitted"
            || !matches!(existing.lifecycle().status, FilingStatus::Submitted)
            || existing.field_map() != draft.field_map()
        {
            return Err(DbError::Other(format!(
                "Only the exact Submitted {} snapshot can be confirmed",
                F::FORM_CODE
            )));
        }
        let json = serde_json::to_string(draft)?;
        let updated = tx.execute(
            "UPDATE form_drafts SET status = 'Confirmed', data_json = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND status = 'Submitted' AND data_json = ?3",
            params![json, id, raw_json],
        )?;
        if updated != 1 {
            return Err(DbError::Other(format!(
                "{} changed before its confirmation was recorded",
                F::FORM_CODE
            )));
        }
        tx.commit()?;
        let _ = self.request_google_calendar_sync();
        Ok(id)
    }

    /// Confirm a Submitted return from a matching BIR receipt.
    /// The Submitted draft whose uploaded filename is the receipt's (the
    /// `#email#` part and case are ignored; BIR echoes the base name).
    fn find_submitted_queueable_by_filename<F: QueueableForm>(
        &self,
        receipt_filename: &str,
    ) -> Result<Option<F>, DbError> {
        fn base(name: &str) -> String {
            let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
            let name = name.split('#').next().unwrap_or(name);
            name.trim_end_matches(".xml").to_ascii_lowercase()
        }
        let wanted = base(receipt_filename);
        if wanted.is_empty() {
            return Ok(None);
        }
        let mut stmt = self.conn.prepare(
            "SELECT data_json FROM form_drafts WHERE form_code = ?1 AND status IN ('Submitted', 'Filed')",
        )?;
        let rows = stmt.query_map(params![F::FORM_CODE], |row| row.get::<_, String>(0))?;
        for raw in rows {
            let draft: F = serde_json::from_str(&raw?)?;
            if draft
                .lifecycle()
                .submission_filename
                .as_deref()
                .is_some_and(|name| base(name) == wanted)
            {
                return Ok(Some(draft));
            }
        }
        Ok(None)
    }

    pub fn confirm_queueable_from_receipt<F: QueueableForm>(
        &self,
        receipt: &SubmissionReceipt,
    ) -> Result<ReceiptConfirmationOutcome, DbError> {
        let by_period = match F::parse_period_code(&receipt.period) {
            Some((year, period)) => {
                let column = crate::forms::queueable::period_column(&period);
                self.get_queueable_draft::<F>(&receipt.tin, year, column)?
                    .filter(|draft| matches!(draft.lifecycle().status, FilingStatus::Submitted))
            }
            None => None,
        };
        // Event-based forms (1706, 1800, …) key drafts by the dashboard's
        // open-ended slot while the official filename carries a date (and a
        // TCT number), so the period alone can't find them. Match the exact
        // filename the worker uploaded instead.
        let draft = match by_period {
            Some(draft) => Some(draft),
            None => self.find_submitted_queueable_by_filename::<F>(&receipt.filename)?,
        };
        let Some(mut draft) = draft else {
            return Ok(ReceiptConfirmationOutcome::Ignored);
        };
        if !matches!(draft.lifecycle().status, FilingStatus::Submitted) {
            return Ok(ReceiptConfirmationOutcome::Ignored);
        }
        let receipt_id = receipt.id.ok_or_else(|| {
            DbError::Other(format!(
                "A {} receipt must be persisted before it can confirm a submission",
                F::FORM_CODE
            ))
        })?;
        let submitted_at = draft.lifecycle().submitted_at.clone().ok_or_else(|| {
            DbError::Other(format!(
                "Submitted {} draft has no submission timestamp",
                F::FORM_CODE
            ))
        })?;
        chrono::DateTime::parse_from_rfc3339(&submitted_at).map_err(|error| {
            DbError::Other(format!(
                "Submitted {} draft has an invalid submission timestamp: {error}",
                F::FORM_CODE
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
        let authorized_at = draft
            .lifecycle()
            .queue_authorization
            .as_ref()
            .map(|auth| auth.authorized_at.clone());
        if !crate::filing_queue::receipt_belongs_to_this_generation(
            receipt_dt,
            authorized_at.as_deref(),
            &submitted_at,
        ) {
            return Ok(ReceiptConfirmationOutcome::Ignored);
        }
        draft.lifecycle_mut().transition_to_confirmed(
            date_str,
            Some(receipt_id),
            Some(receipt.filename.clone()),
        );
        self.save_confirmed_queueable(&draft)?;
        Ok(ReceiptConfirmationOutcome::Confirmed)
    }
}
