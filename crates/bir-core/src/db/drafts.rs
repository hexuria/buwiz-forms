//! Form drafts repository — save, load, and list tax form drafts.

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{Database, DbError};
use crate::forms::form_1601c::Form1601CDraft;
use crate::forms::form_2551q::{
    AnnualIncomeTaxElection, Form2551QDraft, Item13Election, annual_income_tax_election,
};
use crate::forms::{
    FilingFrequency, FilingPeriod, FilingStatus, FormDraftSummary, FormFilingProgress,
    QuarterState, find_form,
};
use crate::profile::TaxpayerProfile;

pub(crate) enum Claim2551QSubmissionResult {
    Claimed {
        draft: Form2551QDraft,
        token: String,
    },
    Rejected {
        draft: Form2551QDraft,
        errors: Vec<(String, String)>,
    },
    Superseded,
}

pub(crate) enum Claim1601CSubmissionResult {
    Claimed {
        draft: Form1601CDraft,
        token: String,
    },
    Rejected {
        draft: Form1601CDraft,
        errors: Vec<(String, String)>,
    },
    Superseded,
}

fn filing_status_to_db(status: &FilingStatus) -> &'static str {
    match status {
        FilingStatus::Draft => "Draft",
        FilingStatus::Queued => "Queued",
        FilingStatus::Submitted => "Submitted",
        FilingStatus::Confirmed => "Confirmed",
        FilingStatus::Paid => "Paid",
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
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let existing_json = tx
            .query_row(
                "SELECT data_json FROM form_drafts
                 WHERE tin = ?1 AND form_code = '2551Q'
                   AND taxable_year = ?2 AND quarter = ?3",
                params![
                    &draft.tin,
                    i64::from(draft.taxable_year),
                    i64::from(draft.quarter)
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(existing_json) = existing_json {
            let existing: Form2551QDraft = serde_json::from_str(&existing_json)?;
            if existing.submission_claim_token.is_some() {
                return Err(DbError::Other(
                    "2551Q submission has already crossed the network claim boundary and cannot be replaced by a generic draft write"
                        .to_string(),
                ));
            }
        }

        let json = serde_json::to_string(draft)?;
        let status = filing_status_to_db(&draft.status);
        let quarter = i64::from(draft.quarter);
        let period_key = FilingPeriod::Quarterly(draft.quarter).to_period_key();

        tx.execute(
            "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(tin, form_code, taxable_year, quarter)
             DO UPDATE SET status = excluded.status,
                           data_json = excluded.data_json,
                           period_key = excluded.period_key,
                           updated_at = datetime('now')",
            params![
                &draft.tin,
                "2551Q",
                i64::from(draft.taxable_year),
                quarter,
                period_key,
                status,
                json
            ],
        )?;

        let id = tx.query_row(
            "SELECT id FROM form_drafts
             WHERE tin = ?1 AND form_code = '2551Q'
               AND taxable_year = ?2 AND quarter = ?3",
            params![&draft.tin, i64::from(draft.taxable_year), quarter],
            |row| row.get::<_, i64>(0),
        )?;
        tx.commit()?;
        let _ = self.request_google_calendar_sync();
        Ok(id)
    }

    /// Reconcile only the profile-staleness audit fields on an immutable 2551Q
    /// snapshot.
    ///
    /// The caller may hold an older Queued/Submitted view while a background
    /// worker or receipt flow has already advanced the stored return. Reloading
    /// the row inside this transaction and CAS-updating its exact JSON prevents
    /// that older view from replacing newer submission, confirmation, payment,
    /// receipt, or claim data through the generic whole-row draft UPSERT.
    pub fn reconcile_immutable_2551q_profile_snapshot(
        &self,
        tin: &str,
        taxable_year: u16,
        quarter: u8,
        profile: &TaxpayerProfile,
    ) -> Result<Form2551QDraft, DbError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((raw_json, db_status)) = tx
            .query_row(
                "SELECT data_json, status FROM form_drafts
                 WHERE tin = ?1 AND form_code = '2551Q'
                   AND taxable_year = ?2 AND quarter = ?3",
                params![tin, i64::from(taxable_year), i64::from(quarter)],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        else {
            return Err(DbError::Other(format!(
                "Cannot reconcile 2551Q profile snapshot because {tin}/{taxable_year}/Q{quarter} no longer exists"
            )));
        };

        let mut current: Form2551QDraft = serde_json::from_str(&raw_json)?;
        if matches!(current.status, FilingStatus::Draft) {
            return Err(DbError::Other(
                "Editable 2551Q drafts must use the draft save path for profile reconciliation"
                    .to_string(),
            ));
        }
        if filing_status_from_db(&db_status) != current.status {
            return Err(DbError::Other(format!(
                "Stored 2551Q status column '{db_status}' does not match its immutable snapshot"
            )));
        }

        // Immutable reconciliation changes only audit/staleness fields in the
        // freshly loaded row. A resolution error is persisted in those fields
        // and returned through `profile_resolution_error` for the UI to show.
        let _ = current.reconcile_with_effective_profile(profile);
        let reconciled_json = serde_json::to_string(&current)?;
        let updated = tx.execute(
            "UPDATE form_drafts
             SET data_json = ?1, updated_at = datetime('now')
             WHERE tin = ?2 AND form_code = '2551Q'
               AND taxable_year = ?3 AND quarter = ?4
               AND status = ?5 AND data_json = ?6",
            params![
                reconciled_json,
                tin,
                i64::from(taxable_year),
                i64::from(quarter),
                db_status,
                raw_json
            ],
        )?;
        if updated != 1 {
            return Err(DbError::Other(
                "2551Q changed while its immutable profile marker was being reconciled".to_string(),
            ));
        }
        tx.commit()?;
        Ok(current)
    }

    /// Atomically persist a queued 2551Q draft and the initial-quarter Item 13
    /// election it makes.
    ///
    /// An 8% election on either an existing taxpayer's Q1 return or a new
    /// registrant's first return is part of the taxpayer's annual income-tax
    /// regime. A queued return must not become visible without the corresponding
    /// profile ledger entry. Conflicting same-year profile elections fail the
    /// entire transaction and leave both the profile and draft unchanged.
    pub fn save_queued_2551q_draft_and_election(
        &self,
        draft: &Form2551QDraft,
    ) -> Result<i64, DbError> {
        self.save_queued_2551q_draft_and_election_with_post_commit_status(draft)
            .map(super::PostCommitWrite::into_committed)
    }

    /// Atomically persist the queued draft and annual election while exposing
    /// the independent status of the post-commit calendar/deadline refresh.
    pub fn save_queued_2551q_draft_and_election_with_post_commit_status(
        &self,
        draft: &Form2551QDraft,
    ) -> Result<super::PostCommitWrite<i64>, DbError> {
        if !matches!(draft.status, FilingStatus::Queued) {
            return Err(DbError::Other(
                "Only a queued 2551Q draft can be saved through the submission path".to_string(),
            ));
        }
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        use crate::profile::{IncomeTaxElection, TaxElectionHistory, TaxpayerProfile};

        let existing_json = tx
            .query_row(
                "SELECT data_json FROM form_drafts
                 WHERE tin = ?1 AND form_code = '2551Q'
                   AND taxable_year = ?2 AND quarter = ?3",
                params![
                    &draft.tin,
                    i64::from(draft.taxable_year),
                    i64::from(draft.quarter)
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(existing_json) = existing_json {
            let existing: Form2551QDraft = serde_json::from_str(&existing_json)?;
            if existing.submission_claim_token.is_some() {
                return Err(DbError::Other(
                    "2551Q submission has already crossed the network claim boundary and cannot be requeued"
                        .to_string(),
                ));
            }
        }

        // Every queued return is bound to the authoritative profile, even if
        // Item 13 does not create an 8% ledger entry. Read it inside this
        // transaction so profile synchronization, election persistence, and
        // the draft UPSERT either all commit or all roll back.
        let profile_json = tx
            .query_row(
                "SELECT data_json FROM profiles WHERE tin = ?1",
                params![&draft.tin],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                DbError::Other(format!(
                    "Cannot queue 2551Q because taxpayer profile {} does not exist",
                    draft.tin
                ))
            })?;
        let mut profile: TaxpayerProfile = serde_json::from_str(&profile_json)?;
        let current_annual_election = annual_income_tax_election(&profile, draft.taxable_year);
        if current_annual_election == AnnualIncomeTaxElection::Conflicting {
            return Err(DbError::Other(format!(
                "Taxpayer profile records conflicting income-tax elections for {}",
                draft.taxable_year
            )));
        }

        let requests_eight_percent = draft.item_13_election == Item13Election::EightPercent;
        let requests_graduated = draft.item_13_election == Item13Election::Graduated;
        if requests_eight_percent && current_annual_election == AnnualIncomeTaxElection::Graduated {
            return Err(DbError::Other(format!(
                "Taxpayer profile already records a conflicting income-tax election for {}",
                draft.taxable_year
            )));
        }
        if requests_graduated && current_annual_election == AnnualIncomeTaxElection::EightPercent {
            return Err(DbError::Other(format!(
                "Taxpayer profile already records a conflicting income-tax election for {}",
                draft.taxable_year
            )));
        }

        let mut profile_changed = false;
        if (requests_eight_percent || requests_graduated)
            && current_annual_election == AnnualIncomeTaxElection::Unrecorded
        {
            // Keep the new election in this transaction's in-memory profile
            // until the fully synchronized queued draft validates. An invalid
            // or stale draft therefore cannot leave a partial ledger write.
            profile.tax_elections.push(TaxElectionHistory {
                taxable_year: draft.taxable_year,
                election: if requests_eight_percent {
                    IncomeTaxElection::EightPercent
                } else {
                    IncomeTaxElection::GraduatedUnspecified
                },
                elected_at: chrono::Utc::now().naive_utc(),
                source_form: "2551Qv2018".to_string(),
            });
            profile_changed = true;
        }

        let mut verified = draft.clone();
        verified
            .reconcile_with_effective_profile(&profile)
            .map_err(|error| {
                DbError::Other(format!(
                    "Cannot queue 2551Q until its exact filing period resolves to one confirmed taxpayer-profile version: {error}"
                ))
            })?;
        if let Err(validation_errors) = verified.revalidate_queued_before_submission() {
            let summary = validation_errors
                .iter()
                .map(|(field, message)| format!("{field}: {message}"))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(DbError::Other(format!(
                "Queued 2551Q draft failed current-profile and queue-fingerprint validation: {summary}"
            )));
        }

        if profile_changed {
            let resolved = profile.resolve_tax_profile_for_year(draft.taxable_year);
            if resolved.has_blocking_issues() || resolved.effective_segments.is_empty() {
                let details = resolved
                    .issues
                    .iter()
                    .map(|issue| issue.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(DbError::Other(format!(
                    "Cannot reconcile the {} Forms Set after recording Item 13 because the confirmed profile timeline is unresolved{}",
                    draft.taxable_year,
                    if details.is_empty() {
                        String::new()
                    } else {
                        format!(": {details}")
                    }
                )));
            }

            let stored_set =
                super::forms_set::query_per_year_forms(&tx, &draft.tin, draft.taxable_year)?;
            let existing_set = (!stored_set.is_empty()).then_some(&stored_set);
            let suggestions = crate::integration::validation::form_suggestions_for_profile_year(
                &profile,
                draft.taxable_year,
            );
            let reconciled = crate::forms::reconcile_forms_set_for_year(
                draft.taxable_year,
                existing_set,
                &suggestions,
            );
            if !reconciled.conflicts.is_empty() {
                tracing::warn!(
                    tin = %draft.tin,
                    taxable_year = draft.taxable_year,
                    conflicts = reconciled.conflicts.len(),
                    "Item 13 election reconciled a Forms Set that needs review"
                );
            }
            super::forms_set::execute_replace_per_year_forms(
                &tx,
                &draft.tin,
                draft.taxable_year,
                &reconciled.forms_set,
            )?;
            let updated_profile_json = serde_json::to_string(&profile)?;
            let updated = tx.execute(
                "UPDATE profiles SET data_json = ?1 WHERE tin = ?2",
                params![updated_profile_json, &draft.tin],
            )?;
            if updated != 1 {
                return Err(DbError::Other(format!(
                    "Expected one taxpayer profile for TIN {}, updated {updated}",
                    draft.tin
                )));
            }
        }

        let json = serde_json::to_string(&verified)?;
        let status = filing_status_to_db(&verified.status);
        let quarter = i64::from(verified.quarter);
        let period_key = FilingPeriod::Quarterly(verified.quarter).to_period_key();

        tx.execute(
            "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(tin, form_code, taxable_year, quarter)
             DO UPDATE SET status = excluded.status,
                           data_json = excluded.data_json,
                           period_key = excluded.period_key,
                           updated_at = datetime('now')",
            params![
                &verified.tin,
                "2551Q",
                i64::from(verified.taxable_year),
                quarter,
                &period_key,
                status,
                json
            ],
        )?;

        let id = tx.query_row(
            "SELECT id FROM form_drafts
             WHERE tin = ?1 AND form_code = '2551Q'
               AND taxable_year = ?2 AND quarter = ?3",
            params![&verified.tin, i64::from(verified.taxable_year), quarter],
            |row| row.get::<_, i64>(0),
        )?;
        tx.commit()?;

        Ok(self.finish_post_commit_write(id, "Queued 2551Q election save"))
    }

    /// Atomically claim the exact queue generation that was revalidated by the
    /// background worker. Once claimed, generic draft writes (including a stale
    /// UI cancel/requeue) are rejected until the worker finishes the claim.
    ///
    /// A claim deliberately has no lease expiry. A process crash anywhere after
    /// this transaction and before the result is finalized leaves an unknown
    /// network outcome: BIR may or may not have received the return. Automatically
    /// clearing or retrying that claim could file a duplicate return, so an
    /// abandoned claim remains fail-closed until a person reconciles it against
    /// the BIR confirmation or receipt.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn claim_queued_2551q_submission(
        &self,
        tin: &str,
        taxable_year: u16,
        quarter: u8,
        expected_fingerprint: &Option<String>,
        expected_next_retry_at: &Option<String>,
        expected_submission_attempts: u32,
    ) -> Result<Claim2551QSubmissionResult, DbError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((raw_json, db_status)) = tx
            .query_row(
                "SELECT data_json, status FROM form_drafts
                 WHERE tin = ?1 AND form_code = '2551Q'
                   AND taxable_year = ?2 AND quarter = ?3",
                params![tin, i64::from(taxable_year), i64::from(quarter)],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        else {
            return Ok(Claim2551QSubmissionResult::Superseded);
        };
        let mut draft: Form2551QDraft = serde_json::from_str(&raw_json)?;
        if db_status != "Queued"
            || !matches!(draft.status, FilingStatus::Queued)
            || draft.submission_claim_token.is_some()
            || &draft.queued_submission_fingerprint != expected_fingerprint
            || &draft.next_retry_at != expected_next_retry_at
            || draft.submission_attempts != expected_submission_attempts
        {
            return Ok(Claim2551QSubmissionResult::Superseded);
        }

        let profile_json = tx
            .query_row(
                "SELECT data_json FROM profiles WHERE tin = ?1",
                params![tin],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                DbError::Other(format!(
                    "Cannot claim 2551Q submission because taxpayer profile {tin} does not exist"
                ))
            })?;
        let profile: crate::profile::TaxpayerProfile = serde_json::from_str(&profile_json)?;
        if let Err(error) = draft.reconcile_with_effective_profile(&profile) {
            draft.revert_to_draft();
            draft.last_error = Some(format!(
                "Submission blocked because the effective taxpayer profile is unresolved: {error}"
            ));
            return Ok(Claim2551QSubmissionResult::Rejected {
                draft,
                errors: vec![("profile_resolution".to_string(), error)],
            });
        }
        if let Err(errors) = draft.revalidate_queued_before_submission() {
            return Ok(Claim2551QSubmissionResult::Rejected { draft, errors });
        }

        let token = uuid::Uuid::new_v4().to_string();
        draft.submission_claim_token = Some(token.clone());
        draft.submission_claimed_at = Some(chrono::Utc::now().to_rfc3339());
        draft.last_error = Some(
            "Submission outcome pending. Automatic retry is disabled; keep any BIR confirmation or receipt and contact support for manual reconciliation before taking another submission action."
                .to_string(),
        );
        let claimed_json = serde_json::to_string(&draft)?;
        let updated = tx.execute(
            "UPDATE form_drafts
             SET data_json = ?1, updated_at = datetime('now')
             WHERE tin = ?2 AND form_code = '2551Q'
               AND taxable_year = ?3 AND quarter = ?4
               AND status = 'Queued' AND data_json = ?5",
            params![
                claimed_json,
                tin,
                i64::from(taxable_year),
                i64::from(quarter),
                raw_json
            ],
        )?;
        if updated != 1 {
            return Ok(Claim2551QSubmissionResult::Superseded);
        }
        tx.commit()?;
        Ok(Claim2551QSubmissionResult::Claimed { draft, token })
    }

    /// Finish a claimed network attempt with either Submitted, a queued retry,
    /// or Draft after the retry limit. Only the worker holding `claim_token`
    /// may clear and replace the claimed row.
    pub(crate) fn finish_claimed_2551q_submission(
        &self,
        draft: &Form2551QDraft,
        claim_token: &str,
    ) -> Result<i64, DbError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((raw_json, db_status)) = tx
            .query_row(
                "SELECT data_json, status FROM form_drafts
                 WHERE tin = ?1 AND form_code = '2551Q'
                   AND taxable_year = ?2 AND quarter = ?3",
                params![
                    &draft.tin,
                    i64::from(draft.taxable_year),
                    i64::from(draft.quarter)
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        else {
            return Err(DbError::Other(
                "Claimed 2551Q draft disappeared before the network attempt finished".to_string(),
            ));
        };
        let existing: Form2551QDraft = serde_json::from_str(&raw_json)?;
        if db_status != "Queued" || existing.submission_claim_token.as_deref() != Some(claim_token)
        {
            return Err(DbError::Other(
                "2551Q submission claim no longer belongs to this worker".to_string(),
            ));
        }

        let mut finished = draft.clone();
        finished.submission_claim_token = None;
        finished.submission_claimed_at = None;
        let json = serde_json::to_string(&finished)?;
        let status = filing_status_to_db(&finished.status);
        let period_key = FilingPeriod::Quarterly(finished.quarter).to_period_key();
        let updated = tx.execute(
            "UPDATE form_drafts
             SET status = ?1, data_json = ?2, period_key = ?3,
                 updated_at = datetime('now')
             WHERE tin = ?4 AND form_code = '2551Q'
               AND taxable_year = ?5 AND quarter = ?6
               AND status = 'Queued' AND data_json = ?7",
            params![
                status,
                json,
                period_key,
                &finished.tin,
                i64::from(finished.taxable_year),
                i64::from(finished.quarter),
                raw_json
            ],
        )?;
        if updated != 1 {
            return Err(DbError::Other(
                "2551Q submission claim changed before completion".to_string(),
            ));
        }
        let id = tx.query_row(
            "SELECT id FROM form_drafts
             WHERE tin = ?1 AND form_code = '2551Q'
               AND taxable_year = ?2 AND quarter = ?3",
            params![
                &finished.tin,
                i64::from(finished.taxable_year),
                i64::from(finished.quarter)
            ],
            |row| row.get::<_, i64>(0),
        )?;
        tx.commit()?;
        let _ = self.request_google_calendar_sync();
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
        let _ = self.request_google_calendar_sync();
        Ok(())
    }

    /// Save or update an editable Form 1601C draft.
    ///
    /// Generic saves may never create or replace Queued and later snapshots.
    /// Queue, cancellation, claim, and completion each use a dedicated CAS path.
    pub fn save_1601c_draft(&self, draft: &Form1601CDraft) -> Result<i64, DbError> {
        if !matches!(draft.status, FilingStatus::Draft) {
            return Err(DbError::Other(
                "Only an editable Draft 1601C return may use the generic save path".to_string(),
            ));
        }

        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let json = serde_json::to_string(draft)?;
        let month = i64::from(draft.month); // Legacy quarter column stores the month.
        let period_key = FilingPeriod::Monthly(draft.month).to_period_key();
        let existing = tx
            .query_row(
                "SELECT id, status, data_json FROM form_drafts
                 WHERE tin = ?1 AND form_code = '1601C'
                   AND taxable_year = ?2 AND quarter = ?3",
                params![&draft.tin, i64::from(draft.taxable_year), month],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;

        let id = if let Some((id, db_status, raw_json)) = existing {
            if db_status != "Draft" {
                return Err(DbError::Other(
                    "An immutable queued or filed 1601C snapshot cannot be replaced by a generic draft save"
                        .to_string(),
                ));
            }
            let stored: Form1601CDraft = serde_json::from_str(&raw_json)?;
            if !matches!(stored.status, FilingStatus::Draft) {
                return Err(DbError::Other(
                    "An immutable queued or filed 1601C snapshot cannot be replaced by a generic draft save"
                        .to_string(),
                ));
            }
            let updated = tx.execute(
                "UPDATE form_drafts
                 SET data_json = ?1, period_key = ?2, updated_at = datetime('now')
                 WHERE id = ?3 AND status = 'Draft' AND data_json = ?4",
                params![json, period_key, id, raw_json],
            )?;
            if updated != 1 {
                return Err(DbError::Other(
                    "1601C draft changed before the editable save completed".to_string(),
                ));
            }
            id
        } else {
            tx.execute(
                "INSERT INTO form_drafts
                    (tin, form_code, taxable_year, quarter, period_key, status, data_json)
                 VALUES (?1, '1601C', ?2, ?3, ?4, 'Draft', ?5)",
                params![
                    &draft.tin,
                    i64::from(draft.taxable_year),
                    month,
                    period_key,
                    json
                ],
            )?;
            tx.last_insert_rowid()
        };

        tx.commit()?;
        let _ = self.request_google_calendar_sync();
        Ok(id)
    }

    /// Validate and atomically persist the exact user-reviewed queue snapshot.
    pub fn save_queued_1601c_draft(&self, draft: &Form1601CDraft) -> Result<i64, DbError> {
        if !matches!(draft.status, FilingStatus::Queued) {
            return Err(DbError::Other(
                "Only a queued 1601C draft can be saved through the submission path".to_string(),
            ));
        }

        let mut verified = draft.clone();
        if let Err(errors) = verified.revalidate_queued_before_submission() {
            let summary = errors
                .iter()
                .map(|(field, message)| format!("{field}: {message}"))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(DbError::Other(format!(
                "Queued 1601C draft failed queue-fingerprint validation: {summary}"
            )));
        }
        verified.try_to_bir_xml_payload().map_err(|errors| {
            DbError::Other(format!(
                "Queued 1601C draft failed exact XML generation: {}",
                errors
                    .iter()
                    .map(|(field, message)| format!("{field}: {message}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            ))
        })?;

        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let month = i64::from(verified.month);
        let period_key = FilingPeriod::Monthly(verified.month).to_period_key();
        let json = serde_json::to_string(&verified)?;
        let existing = tx
            .query_row(
                "SELECT id, status, data_json FROM form_drafts
                 WHERE tin = ?1 AND form_code = '1601C'
                   AND taxable_year = ?2 AND quarter = ?3",
                params![&verified.tin, i64::from(verified.taxable_year), month],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;

        let id = if let Some((id, db_status, raw_json)) = existing {
            if db_status != "Draft" {
                return Err(DbError::Other(
                    "1601C must be canceled back to Draft before a new queue snapshot can replace it"
                        .to_string(),
                ));
            }
            let stored: Form1601CDraft = serde_json::from_str(&raw_json)?;
            if !matches!(stored.status, FilingStatus::Draft) {
                return Err(DbError::Other(
                    "1601C must be canceled back to Draft before a new queue snapshot can replace it"
                        .to_string(),
                ));
            }
            let updated = tx.execute(
                "UPDATE form_drafts
                 SET status = 'Queued', data_json = ?1, period_key = ?2,
                     updated_at = datetime('now')
                 WHERE id = ?3 AND status = 'Draft' AND data_json = ?4",
                params![json, period_key, id, raw_json],
            )?;
            if updated != 1 {
                return Err(DbError::Other(
                    "1601C draft changed before its queue snapshot was persisted".to_string(),
                ));
            }
            id
        } else {
            tx.execute(
                "INSERT INTO form_drafts
                    (tin, form_code, taxable_year, quarter, period_key, status, data_json)
                 VALUES (?1, '1601C', ?2, ?3, ?4, 'Queued', ?5)",
                params![
                    &verified.tin,
                    i64::from(verified.taxable_year),
                    month,
                    period_key,
                    json
                ],
            )?;
            tx.last_insert_rowid()
        };

        tx.commit()?;
        let _ = self.request_google_calendar_sync();
        Ok(id)
    }

    /// CAS-replace an unclaimed queue generation with a retry update or a
    /// deliberate Draft rejection/cancellation. The caller must identify the
    /// exact generation it previously loaded.
    pub(crate) fn replace_unclaimed_queued_1601c_submission(
        &self,
        replacement: &Form1601CDraft,
        expected_fingerprint: &Option<String>,
        expected_next_retry_at: &Option<String>,
        expected_submission_attempts: u32,
    ) -> Result<bool, DbError> {
        let mut replacement = replacement.clone();
        if !matches!(
            replacement.status,
            FilingStatus::Draft | FilingStatus::Queued
        ) {
            return Err(DbError::Other(
                "An unclaimed 1601C queue generation may only remain Queued or return to Draft"
                    .to_string(),
            ));
        }
        if matches!(replacement.status, FilingStatus::Queued)
            && &replacement.queued_submission_fingerprint != expected_fingerprint
        {
            return Err(DbError::Other(
                "A retry update cannot change the reviewed 1601C queue fingerprint".to_string(),
            ));
        }
        if matches!(replacement.status, FilingStatus::Queued) {
            if let Err(errors) = replacement.revalidate_queued_before_submission() {
                return Err(DbError::Other(format!(
                    "A retry update cannot persist invalid 1601C submission fields: {}",
                    errors
                        .iter()
                        .map(|(field, message)| format!("{field}: {message}"))
                        .collect::<Vec<_>>()
                        .join("; ")
                )));
            }
        }
        if replacement.submission_claim_token.is_some() {
            return Err(DbError::Other(
                "An unclaimed 1601C replacement cannot carry a network claim".to_string(),
            ));
        }

        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((id, raw_json, db_status)) = tx
            .query_row(
                "SELECT id, data_json, status FROM form_drafts
                 WHERE tin = ?1 AND form_code = '1601C'
                   AND taxable_year = ?2 AND quarter = ?3",
                params![
                    &replacement.tin,
                    i64::from(replacement.taxable_year),
                    i64::from(replacement.month)
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
        else {
            return Ok(false);
        };
        let current: Form1601CDraft = serde_json::from_str(&raw_json)?;
        if db_status != "Queued"
            || !matches!(current.status, FilingStatus::Queued)
            || current.submission_claim_token.is_some()
            || &current.queued_submission_fingerprint != expected_fingerprint
            || &current.next_retry_at != expected_next_retry_at
            || current.submission_attempts != expected_submission_attempts
        {
            return Ok(false);
        }

        let json = serde_json::to_string(&replacement)?;
        let status = filing_status_to_db(&replacement.status);
        let updated = tx.execute(
            "UPDATE form_drafts
             SET status = ?1, data_json = ?2, updated_at = datetime('now')
             WHERE id = ?3 AND status = 'Queued' AND data_json = ?4",
            params![status, json, id, raw_json],
        )?;
        if updated != 1 {
            return Ok(false);
        }
        tx.commit()?;
        let _ = self.request_google_calendar_sync();
        Ok(true)
    }

    /// Cancel one exact, still-unclaimed 1601C queue generation.
    pub fn cancel_queued_1601c_submission(
        &self,
        queued: &Form1601CDraft,
    ) -> Result<Form1601CDraft, DbError> {
        if !matches!(queued.status, FilingStatus::Queued) || queued.submission_claim_token.is_some()
        {
            return Err(DbError::Other(
                "Only an unclaimed queued 1601C snapshot can be canceled".to_string(),
            ));
        }
        let mut verified = queued.clone();
        if let Err(errors) = verified.revalidate_queued_before_submission() {
            return Err(DbError::Other(format!(
                "Only the exact reviewed 1601C queue snapshot can be canceled: {}",
                errors
                    .iter()
                    .map(|(field, message)| format!("{field}: {message}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
        let expected_fingerprint = verified.queued_submission_fingerprint.clone();
        let expected_retry = verified.next_retry_at.clone();
        let expected_attempts = verified.submission_attempts;
        let mut draft = verified;
        draft.revert_to_draft();
        if !self.replace_unclaimed_queued_1601c_submission(
            &draft,
            &expected_fingerprint,
            &expected_retry,
            expected_attempts,
        )? {
            return Err(DbError::Other(
                "1601C submission has already started or the queue generation changed".to_string(),
            ));
        }
        Ok(draft)
    }

    /// Atomically revalidate and claim the exact queued 1601C generation
    /// immediately before the irreversible network boundary.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn claim_queued_1601c_submission(
        &self,
        tin: &str,
        taxable_year: u16,
        month: u8,
        expected_fingerprint: &Option<String>,
        expected_next_retry_at: &Option<String>,
        expected_submission_attempts: u32,
    ) -> Result<Claim1601CSubmissionResult, DbError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((id, raw_json, db_status)) = tx
            .query_row(
                "SELECT id, data_json, status FROM form_drafts
                 WHERE tin = ?1 AND form_code = '1601C'
                   AND taxable_year = ?2 AND quarter = ?3",
                params![tin, i64::from(taxable_year), i64::from(month)],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
        else {
            return Ok(Claim1601CSubmissionResult::Superseded);
        };
        let mut draft: Form1601CDraft = serde_json::from_str(&raw_json)?;
        if db_status != "Queued"
            || !matches!(draft.status, FilingStatus::Queued)
            || draft.submission_claim_token.is_some()
            || &draft.queued_submission_fingerprint != expected_fingerprint
            || &draft.next_retry_at != expected_next_retry_at
            || draft.submission_attempts != expected_submission_attempts
        {
            return Ok(Claim1601CSubmissionResult::Superseded);
        }

        if let Err(errors) = draft.revalidate_queued_before_submission() {
            let rejected_json = serde_json::to_string(&draft)?;
            let updated = tx.execute(
                "UPDATE form_drafts
                 SET status = 'Draft', data_json = ?1, updated_at = datetime('now')
                 WHERE id = ?2 AND status = 'Queued' AND data_json = ?3",
                params![rejected_json, id, raw_json],
            )?;
            if updated != 1 {
                return Ok(Claim1601CSubmissionResult::Superseded);
            }
            tx.commit()?;
            return Ok(Claim1601CSubmissionResult::Rejected { draft, errors });
        }

        let token = uuid::Uuid::new_v4().to_string();
        draft.submission_claim_token = Some(token.clone());
        draft.submission_claimed_at = Some(chrono::Utc::now().to_rfc3339());
        draft.submission_error = Some(
            "Submission outcome pending. Automatic retry is disabled; keep any BIR confirmation or receipt and contact support for manual reconciliation before taking another submission action."
                .to_string(),
        );
        let claimed_json = serde_json::to_string(&draft)?;
        let updated = tx.execute(
            "UPDATE form_drafts
             SET data_json = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND status = 'Queued' AND data_json = ?3",
            params![claimed_json, id, raw_json],
        )?;
        if updated != 1 {
            return Ok(Claim1601CSubmissionResult::Superseded);
        }
        tx.commit()?;
        Ok(Claim1601CSubmissionResult::Claimed { draft, token })
    }

    /// Finalize a claimed 1601C network attempt. Only the worker holding the
    /// durable token may replace the claimed snapshot.
    pub(crate) fn finish_claimed_1601c_submission(
        &self,
        draft: &Form1601CDraft,
        claim_token: &str,
    ) -> Result<i64, DbError> {
        if !matches!(draft.status, FilingStatus::Submitted) {
            return Err(DbError::Other(
                "Only a Submitted 1601C snapshot can finish a network claim".to_string(),
            ));
        }
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let Some((id, raw_json, db_status)) = tx
            .query_row(
                "SELECT id, data_json, status FROM form_drafts
                 WHERE tin = ?1 AND form_code = '1601C'
                   AND taxable_year = ?2 AND quarter = ?3",
                params![
                    &draft.tin,
                    i64::from(draft.taxable_year),
                    i64::from(draft.month)
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
        else {
            return Err(DbError::Other(
                "Claimed 1601C draft disappeared before the network attempt finished".to_string(),
            ));
        };
        let existing: Form1601CDraft = serde_json::from_str(&raw_json)?;
        if db_status != "Queued" || existing.submission_claim_token.as_deref() != Some(claim_token)
        {
            return Err(DbError::Other(
                "1601C submission claim no longer belongs to this worker".to_string(),
            ));
        }
        if draft.queued_submission_fingerprint != existing.queued_submission_fingerprint
            || draft.to_bir_field_map() != existing.to_bir_field_map()
        {
            return Err(DbError::Other(
                "Claimed 1601C submission fields changed before completion".to_string(),
            ));
        }
        let expected_filename = existing.default_submission_filename();
        if draft.submission_filename.as_deref() != Some(expected_filename.as_str())
            || draft.submitted_at.is_none()
        {
            return Err(DbError::Other(
                "Claimed 1601C submission completion did not preserve the reviewed IAF filename and timestamp"
                    .to_string(),
            ));
        }

        let mut finished = draft.clone();
        finished.submission_claim_token = None;
        finished.submission_claimed_at = None;
        let json = serde_json::to_string(&finished)?;
        let period_key = FilingPeriod::Monthly(finished.month).to_period_key();
        let updated = tx.execute(
            "UPDATE form_drafts
             SET status = 'Submitted', data_json = ?1, period_key = ?2,
                 updated_at = datetime('now')
             WHERE id = ?3 AND status = 'Queued' AND data_json = ?4",
            params![json, period_key, id, raw_json],
        )?;
        if updated != 1 {
            return Err(DbError::Other(
                "1601C submission claim changed before completion".to_string(),
            ));
        }
        tx.commit()?;
        let _ = self.request_google_calendar_sync();
        Ok(id)
    }

    /// Load a 1601C draft for a specific (tin, year, month).
    pub fn get_1601c_draft(
        &self,
        tin: &str,
        year: u16,
        month: u8,
    ) -> Result<Option<Form1601CDraft>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT data_json FROM form_drafts
             WHERE tin = ?1 AND form_code = '1601C'
               AND taxable_year = ?2 AND quarter = ?3",
        )?;
        let mut rows = stmt.query(params![tin, year as i64, month as i64])?;
        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            let draft: Form1601CDraft = serde_json::from_str(&json)?;
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

        if form_code == "1601C" {
            let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
            let existing = tx
                .query_row(
                    "SELECT id, status, data_json FROM form_drafts
                     WHERE tin = ?1 AND form_code = '1601C'
                       AND taxable_year = ?2 AND period_key = ?3",
                    params![tin, i64::from(year), &period_key],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?;

            let id = if let Some((id, status, raw_json)) = existing {
                if status != "Draft" {
                    return Err(DbError::Other(
                        "An imported 1601C return cannot replace a queued or filed snapshot"
                            .to_string(),
                    ));
                }
                let stored: Form1601CDraft = serde_json::from_str(&raw_json)?;
                if !matches!(stored.status, FilingStatus::Draft) {
                    return Err(DbError::Other(
                        "An imported 1601C return cannot replace a queued or filed snapshot"
                            .to_string(),
                    ));
                }
                let updated = tx.execute(
                    "UPDATE form_drafts
                     SET quarter = ?1, status = 'Submitted', data_json = '{}',
                         updated_at = datetime('now')
                     WHERE id = ?2 AND status = 'Draft' AND data_json = ?3",
                    params![legacy_slot, id, raw_json],
                )?;
                if updated != 1 {
                    return Err(DbError::Other(
                        "1601C draft changed before the imported return was saved".to_string(),
                    ));
                }
                id
            } else {
                tx.execute(
                    "INSERT INTO form_drafts
                        (tin, form_code, taxable_year, quarter, period_key, status, data_json)
                     VALUES (?1, '1601C', ?2, ?3, ?4, 'Submitted', '{}')",
                    params![tin, i64::from(year), legacy_slot, &period_key],
                )?;
                tx.last_insert_rowid()
            };
            tx.commit()?;
            let _ = self.request_google_calendar_sync();
            return Ok(id);
        }

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
        let _ = self.request_google_calendar_sync();
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
            if crate::forms::can_queue_for_submission(&summary.form_code) {
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
        if form_code == "1601C" {
            return Err(DbError::Other(
                "1601C must use its dedicated immutable draft or queue persistence path"
                    .to_string(),
            ));
        }

        if matches!(status, FilingStatus::Queued)
            && !crate::forms::can_queue_for_submission(form_code)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::form_2551q::{Item13Election, Schedule1Row};
    use crate::profile::{IncomeTaxElection, TaxElectionHistory, TaxpayerProfile};
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

    fn test_profile() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Test Taxpayer",
            "tin": {
                "segment1": "123",
                "segment2": "456",
                "segment3": "789",
                "branch": "000"
            },
            "rdo_code": "018",
            "line_of_business": "Retail",
            "registered_address": "Manila",
            "zip_code": "1000",
            "phone": "09123456789",
            "email": "test@example.com",
            "default_form_type": "2551Qv2018",
            "taxpayer_type": "Individual",
            "business_start_date": "2020-01-01"
        }))
        .unwrap()
    }

    fn insert_test_profile(db: &Database, profile: &TaxpayerProfile) {
        let mut persisted = profile.clone();
        persisted.ensure_profile_version_ledger();
        insert_raw_test_profile(db, &persisted);
    }

    fn insert_raw_test_profile(db: &Database, profile: &TaxpayerProfile) {
        db.conn
            .execute(
                "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
                params![profile.tin.full(), serde_json::to_string(profile).unwrap()],
            )
            .unwrap();
    }

    fn queued_eight_percent_draft(profile: &TaxpayerProfile) -> Form2551QDraft {
        let mut effective_profile = profile.clone();
        effective_profile.ensure_profile_version_ledger();
        let mut draft = Form2551QDraft::new_from_effective_profile(&effective_profile, 2026, 1);
        draft.item_13_election = Item13Election::EightPercent;
        draft
            .transition_to_queued()
            .expect("a NIL PT010 Q1 8% election should queue");
        draft
    }

    fn queued_graduated_draft(profile: &TaxpayerProfile) -> Form2551QDraft {
        let mut effective_profile = profile.clone();
        effective_profile.ensure_profile_version_ledger();
        let mut draft = Form2551QDraft::new_from_effective_profile(&effective_profile, 2026, 1);
        draft.item_13_election = Item13Election::Graduated;
        draft
            .transition_to_queued()
            .expect("a Q1 graduated election should queue");
        draft
    }

    fn editable_1601c_draft(profile: &TaxpayerProfile) -> Form1601CDraft {
        let mut draft = Form1601CDraft::new_from_profile(profile, 2026, 5);
        draft.any_taxes_withheld = false;
        draft.compute();
        draft
    }

    fn queued_1601c_draft(profile: &TaxpayerProfile) -> Form1601CDraft {
        let mut draft = editable_1601c_draft(profile);
        draft
            .transition_to_queued()
            .expect("the reviewed 1601C draft should queue");
        draft
    }

    #[test]
    fn save_and_reopen_2551q_preserves_ten_distinct_printable_schedule_rows() {
        let db = test_db();
        let profile = test_profile();
        let mut draft = Form2551QDraft::new_from_profile(&profile, 2026, 1);
        draft.schedule_1 = [
            "PT010", "PT040", "PT041", "PT060", "PT070", "PT090", "PT140", "PT150", "PT160",
            "PT170",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, code)| {
            let mut row = Schedule1Row::new(code).expect("test ATC must be canonical");
            row.taxable_amount = (index as f64 + 1.0) * 1_000.0;
            row
        })
        .collect();
        assert!(draft.ensure_required_schedule_attachment_sheets());
        draft.recompute(None);

        db.save_2551q_draft(&draft)
            .expect("ten-row printable draft should save without claiming XML fileability");
        let reopened = db
            .get_2551q_draft(&draft.tin, draft.taxable_year, draft.quarter)
            .expect("saved draft lookup should succeed")
            .expect("saved draft should exist");

        assert_eq!(
            reopened
                .schedule_1
                .iter()
                .map(|row| row.atc.as_str())
                .collect::<Vec<_>>(),
            vec![
                "PT010", "PT040", "PT041", "PT060", "PT070", "PT090", "PT140", "PT150", "PT160",
                "PT170"
            ]
        );
        assert_eq!(
            reopened
                .schedule_1
                .iter()
                .map(|row| row.taxable_amount)
                .collect::<Vec<_>>(),
            vec![
                1_000.0, 2_000.0, 3_000.0, 4_000.0, 5_000.0, 6_000.0, 7_000.0, 8_000.0, 9_000.0,
                10_000.0
            ]
        );
        assert_eq!(
            reopened
                .schedule_1
                .iter()
                .map(|row| row.tax_due)
                .collect::<Vec<_>>(),
            vec![
                30.0, 60.0, 90.0, 80.0, 150.0, 600.0, 1_260.0, 1_440.0, 900.0, 1_500.0
            ]
        );
        assert_eq!(reopened.number_of_attached_sheets, 1);
        assert_eq!(reopened.required_schedule_attachment_sheets(), 1);
        assert_eq!(reopened.total_tax_due, 6_110.0);
    }

    #[test]
    fn save_and_reopen_1601c_preserves_all_three_schedule_rows() {
        use crate::forms::form_1601c::{Form1601CDraft, Form1601CSchedule1Row};

        let db = test_db();
        let profile = test_profile();
        let mut draft = Form1601CDraft::new_from_profile(&profile, 2026, 4);
        draft.any_taxes_withheld = false;
        draft.auto_compute_penalties = false;
        draft.tax_relief = true;
        draft.tax_relief_specification = "International Tax Treaty".to_string();
        draft.schedule_1 = (1..=3)
            .map(|index| Form1601CSchedule1Row {
                previous_month: format!("{index:02}/2026"),
                date_paid: format!("{index:02}/10/2026"),
                drawee_bank_code_or_agency: format!("AAB-{index}"),
                payment_number: format!("REF-{index}"),
                tax_paid: f64::from(index) * 100.0,
                should_be_tax_due: f64::from(index) * 125.0,
                adjustment: 0.0,
            })
            .collect();
        draft.compute();

        db.save_1601c_draft(&draft)
            .expect("three-row 1601-C draft should save");
        let reopened = db
            .get_1601c_draft(&draft.tin, draft.taxable_year, draft.month)
            .expect("1601-C lookup should succeed")
            .expect("saved 1601-C draft should exist");

        assert_eq!(reopened.schedule_1, draft.schedule_1);
        assert_eq!(
            reopened.tax_relief_specification,
            "International Tax Treaty"
        );
        assert_eq!(reopened.tax_26_adjustment, 150.0);
    }

    #[test]
    fn editable_1601c_to_exact_xml_queue_claim_and_completion_is_immutable() {
        let db = test_db();
        let profile = test_profile();
        let editable = editable_1601c_draft(&profile);
        db.save_1601c_draft(&editable)
            .expect("editable 1601C should save");

        let mut queued = editable.clone();
        queued
            .transition_to_queued()
            .expect("validated editable 1601C should queue");
        let queued_fields = queued.to_bir_field_map();
        let xml = queued
            .try_to_bir_xml_payload()
            .expect("queued 1601C should produce checked XML");
        assert_eq!(
            crate::bir_xml::parse_bir_xml_checked(&xml).unwrap(),
            queued_fields
        );
        assert_eq!(
            queued.default_submission_filename(),
            "123456789000-1601Cv2018-052026#test@example.com#.xml"
        );

        db.save_queued_1601c_draft(&queued)
            .expect("reviewed queue snapshot should persist");
        let stored = db
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, FilingStatus::Queued);
        assert_eq!(stored.to_bir_field_map(), queued_fields);
        assert_eq!(
            stored.queued_submission_fingerprint,
            queued.queued_submission_fingerprint
        );
        let mut tampered_cancellation = stored.clone();
        tampered_cancellation.tax_14_total_compensation = 42.0;
        assert!(
            db.cancel_queued_1601c_submission(&tampered_cancellation)
                .is_err()
        );

        let mut stale_editable = editable.clone();
        stale_editable.tax_14_total_compensation = 99_999.0;
        assert!(db.save_1601c_draft(&stale_editable).is_err());
        assert!(
            db.save_form_draft_v2(
                &stale_editable.tin,
                "1601C",
                stale_editable.taxable_year,
                &FilingPeriod::Monthly(stale_editable.month),
                &FilingStatus::Draft,
                &stale_editable,
            )
            .is_err()
        );
        assert!(
            db.save_form_draft(
                &stale_editable.tin,
                "1601C",
                stale_editable.taxable_year,
                Some(stale_editable.month),
                &FilingStatus::Draft,
                &stale_editable,
            )
            .is_err()
        );
        assert!(
            db.save_imported_form(
                &stale_editable.tin,
                "1601C",
                stale_editable.taxable_year,
                None,
                Some(stale_editable.month),
            )
            .is_err()
        );
        assert_eq!(
            db.get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
                .unwrap()
                .unwrap()
                .to_bir_field_map(),
            queued_fields
        );

        let expected_fingerprint = stored.queued_submission_fingerprint.clone();
        let expected_retry = stored.next_retry_at.clone();
        let expected_attempts = stored.submission_attempts;
        let (mut claimed, token) = match db
            .claim_queued_1601c_submission(
                &stored.tin,
                stored.taxable_year,
                stored.month,
                &expected_fingerprint,
                &expected_retry,
                expected_attempts,
            )
            .unwrap()
        {
            Claim1601CSubmissionResult::Claimed { draft, token } => (draft, token),
            _ => panic!("the unchanged queue generation should be claimed"),
        };
        assert_eq!(claimed.to_bir_field_map(), queued_fields);
        assert_eq!(
            claimed.submission_claim_token.as_deref(),
            Some(token.as_str())
        );

        let mut stale_cancellation = queued.clone();
        stale_cancellation.revert_to_draft();
        assert!(
            !db.replace_unclaimed_queued_1601c_submission(
                &stale_cancellation,
                &expected_fingerprint,
                &expected_retry,
                expected_attempts,
            )
            .unwrap(),
            "a claimed queue generation must reject stale cancellation"
        );
        assert!(db.save_1601c_draft(&stale_editable).is_err());

        let filename = claimed.default_submission_filename();
        let mut wrong_filename = claimed.clone();
        wrong_filename.transition_to_submitted("wrong.xml".to_string());
        assert!(
            db.finish_claimed_1601c_submission(&wrong_filename, &token)
                .is_err()
        );
        claimed.transition_to_submitted(filename.clone());
        assert!(
            db.finish_claimed_1601c_submission(&claimed, "wrong-token")
                .is_err()
        );
        db.finish_claimed_1601c_submission(&claimed, &token)
            .expect("the claim owner should finalize the exact submitted snapshot");

        let submitted = db
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        assert_eq!(submitted.status, FilingStatus::Submitted);
        assert_eq!(submitted.to_bir_field_map(), queued_fields);
        assert_eq!(
            submitted.submission_filename.as_deref(),
            Some(filename.as_str())
        );
        assert!(submitted.submission_claim_token.is_none());
        assert!(db.save_1601c_draft(&stale_editable).is_err());
    }

    #[test]
    fn claimed_1601c_revalidation_atomically_rejects_tampered_queue_data() {
        let db = test_db();
        let profile = test_profile();
        let queued = queued_1601c_draft(&profile);
        db.save_queued_1601c_draft(&queued).unwrap();

        let expected_fingerprint = queued.queued_submission_fingerprint.clone();
        let expected_retry = queued.next_retry_at.clone();
        let expected_attempts = queued.submission_attempts;
        let mut tampered = db
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        tampered.tax_14_total_compensation = 42.0;
        db.conn
            .execute(
                "UPDATE form_drafts SET data_json = ?1
                 WHERE tin = ?2 AND form_code = '1601C'
                   AND taxable_year = ?3 AND quarter = ?4",
                params![
                    serde_json::to_string(&tampered).unwrap(),
                    &queued.tin,
                    i64::from(queued.taxable_year),
                    i64::from(queued.month)
                ],
            )
            .unwrap();

        match db
            .claim_queued_1601c_submission(
                &queued.tin,
                queued.taxable_year,
                queued.month,
                &expected_fingerprint,
                &expected_retry,
                expected_attempts,
            )
            .unwrap()
        {
            Claim1601CSubmissionResult::Rejected { draft, errors } => {
                assert_eq!(draft.status, FilingStatus::Draft);
                assert!(errors.iter().any(|(field, _)| {
                    field == "queued_submission_fingerprint" || field == "tax_22_total_taxable"
                }));
            }
            _ => panic!("tampered queue data must be rejected before a claim"),
        }

        let rejected = db
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        assert_eq!(rejected.status, FilingStatus::Draft);
        assert!(rejected.submission_claim_token.is_none());
        assert!(
            rejected
                .submission_error
                .as_deref()
                .is_some_and(|message| message.contains("queue revalidation"))
        );
    }

    #[test]
    fn exact_unclaimed_1601c_queue_can_be_canceled_edited_and_requeued() {
        let db = test_db();
        let profile = test_profile();
        let queued = queued_1601c_draft(&profile);
        db.save_queued_1601c_draft(&queued).unwrap();

        let mut canceled = db.cancel_queued_1601c_submission(&queued).unwrap();
        assert_eq!(canceled.status, FilingStatus::Draft);
        assert!(canceled.queued_submission_fingerprint.is_none());
        canceled.tax_14_total_compensation = 123.45;
        canceled.compute();
        db.save_1601c_draft(&canceled).unwrap();

        canceled.transition_to_queued().unwrap();
        db.save_queued_1601c_draft(&canceled).unwrap();
        let requeued = db
            .get_1601c_draft(&canceled.tin, canceled.taxable_year, canceled.month)
            .unwrap()
            .unwrap();
        assert_eq!(requeued.status, FilingStatus::Queued);
        assert_eq!(requeued.to_bir_field_map(), canceled.to_bir_field_map());
    }

    #[test]
    fn save_and_reopen_0605_preserves_independent_dates_codes_and_pdf_only_details() {
        use crate::forms::FormValidator;
        use crate::forms::form_0605::{
            Form0605Date, Form0605Draft, Form0605FilingBasis, Form0605MannerOfPayment,
            Form0605ReviewedAtc, Form0605ReviewedTaxType, Form0605TypeOfPayment,
        };

        let db = test_db();
        let profile = test_profile();
        let mut draft = Form0605Draft::new_from_profile(&profile, 2025, 1);
        draft.filing_basis = Form0605FilingBasis::Fiscal;
        draft.quarter = 1;
        draft.year_end_month = 12;
        draft.due_date = Some(Form0605Date::new(2025, 12, 31).unwrap());
        draft.return_period = Some(Form0605Date::new(2025, 12, 31).unwrap());
        draft.number_of_sheets = 10;
        draft.select_reviewed_atc(Form0605ReviewedAtc::Ii011);
        draft.select_reviewed_tax_type(Form0605ReviewedTaxType::It);
        draft.manner_of_payment = Some(Form0605MannerOfPayment::Others);
        draft.other_manner_description = "MANUAL PAYMENT".to_string();
        draft.type_of_payment = Some(Form0605TypeOfPayment::Installment);
        draft.number_of_installments = Some(10);
        draft.item_19_basic_tax_or_payment = 1_000.0;
        draft.item_20a_surcharge = 10.0;
        draft.item_20b_interest = 20.0;
        draft.item_20c_compromise = 1_000.0;
        draft.signatures.taxpayer_or_authorized_representative = "TEST TAXPAYER".to_string();
        draft.signatures.title_or_position = "OWNER".to_string();
        draft.payment_details.check.drawee_bank_or_agency = "AAB".to_string();
        draft.payment_details.check.number = "CHECK-24".to_string();
        draft.payment_details.check.date = "12/31/2025".to_string();
        draft.payment_details.check.amount = Some(2_030.0);
        draft.recompute();
        assert!(draft.validate().is_empty(), "test draft must be valid");

        db.save_form_draft(
            &draft.tin,
            "0605",
            draft.taxable_year,
            Some(draft.month),
            &draft.status,
            &draft,
        )
        .expect("0605 draft should save");
        let reopened = db
            .get_form_draft::<Form0605Draft>(
                &draft.tin,
                "0605",
                draft.taxable_year,
                Some(draft.month),
            )
            .expect("0605 lookup should succeed")
            .expect("saved 0605 draft should exist");

        assert_eq!(reopened, draft);
        assert_eq!(reopened.item_20d_total_penalties, 1_030.0);
        assert_eq!(reopened.item_21_total_amount_payable, 2_030.0);
    }

    #[test]
    fn save_and_reopen_0619e_preserves_manual_due_day_and_fixed_payment_rows() {
        use crate::forms::form_0619e::{
            Form0619EDraft, Form0619EPaymentRow, WithholdingAgentCategory,
        };

        let db = test_db();
        let profile = test_profile();
        let mut draft = Form0619EDraft::new_from_profile(&profile, 2026, 12);
        draft.due_day = Some(10);
        draft.withholding_agent_category = WithholdingAgentCategory::Government;
        draft.any_taxes_withheld = true;
        draft.item_14_amount_of_remittance = 1_000.0;
        draft.item_17a_surcharge = 100.0;
        draft.item_17b_interest = 30.0;
        draft.item_17c_compromise = 100.0;
        draft.payment_details.others = Form0619EPaymentRow {
            drawee_bank_or_agency: "AAB".to_string(),
            number: "REF-22".to_string(),
            date: "01/10/2027".to_string(),
            amount: Some(1_230.0),
        };
        draft.payment_details.others_description = "OTHER PAYMENT".to_string();
        draft.recompute();

        db.save_form_draft(
            &draft.tin,
            "0619E",
            draft.taxable_year,
            Some(draft.month),
            &draft.status,
            &draft,
        )
        .expect("0619-E draft should save");
        let reopened = db
            .get_form_draft::<Form0619EDraft>(
                &draft.tin,
                "0619E",
                draft.taxable_year,
                Some(draft.month),
            )
            .expect("0619-E lookup should succeed")
            .expect("saved 0619-E draft should exist");

        assert_eq!(reopened.due_day, Some(10));
        assert_eq!(reopened.due_month_and_year(), (1, 2027));
        assert_eq!(reopened.payment_details, draft.payment_details);
        assert_eq!(reopened.item_18_total_amount_of_remittance, 1_230.0);
    }

    #[test]
    fn save_and_reopen_0619f_preserves_manual_due_day_and_fixed_payment_rows() {
        use crate::forms::form_0619f::{
            Form0619FDraft, Form0619FPaymentRow, WithholdingAgentCategory,
        };

        let db = test_db();
        let profile = test_profile();
        let mut draft = Form0619FDraft::new_from_profile(&profile, 2026, 12);
        draft.due_day = Some(10);
        draft.withholding_agent_category = WithholdingAgentCategory::Government;
        draft.any_taxes_withheld = true;
        draft.item_13_interest_final_tax_withheld = 1_000.0;
        draft.item_18a_surcharge = 100.0;
        draft.item_18b_interest = 30.0;
        draft.item_18c_compromise = 100.0;
        draft.payment_details.others = Form0619FPaymentRow {
            drawee_bank_or_agency: "AAB".to_string(),
            number: "REF-23".to_string(),
            date: "01/10/2027".to_string(),
            amount: Some(1_230.0),
        };
        draft.payment_details.others_description = "OTHER PAYMENT".to_string();
        draft.recompute();

        db.save_form_draft(
            &draft.tin,
            "0619F",
            draft.taxable_year,
            Some(draft.month),
            &draft.status,
            &draft,
        )
        .expect("0619-F draft should save");
        let reopened = db
            .get_form_draft::<Form0619FDraft>(
                &draft.tin,
                "0619F",
                draft.taxable_year,
                Some(draft.month),
            )
            .expect("0619-F lookup should succeed")
            .expect("saved 0619-F draft should exist");

        assert_eq!(reopened.due_day, Some(10));
        assert_eq!(reopened.due_month_and_year(), (1, 2027));
        assert_eq!(reopened.payment_details, draft.payment_details);
        assert_eq!(reopened.item_19_total_amount_of_remittance, 1_230.0);
    }

    #[test]
    fn queued_q1_eight_percent_draft_atomically_records_annual_election() {
        let db = test_db();
        let profile = test_profile();
        insert_test_profile(&db, &profile);
        let draft = queued_eight_percent_draft(&profile);

        let first_id = db.save_queued_2551q_draft_and_election(&draft).unwrap();
        let second_id = db.save_queued_2551q_draft_and_election(&draft).unwrap();

        let saved_profile = db.get_profile(&draft.tin).unwrap().unwrap();
        let elections = saved_profile
            .tax_elections
            .iter()
            .filter(|entry| entry.taxable_year == 2026)
            .collect::<Vec<_>>();
        let saved_draft = db.get_2551q_draft(&draft.tin, 2026, 1).unwrap().unwrap();

        assert_eq!(first_id, second_id);
        assert_eq!(elections.len(), 1);
        assert_eq!(elections[0].election, IncomeTaxElection::EightPercent);
        assert_eq!(elections[0].source_form, "2551Qv2018");
        assert_eq!(saved_draft.status, FilingStatus::Queued);
        let saved_set = db.get_per_year_forms(&draft.tin, 2026).unwrap();
        assert!(saved_set.contains_active("1701Q"));
        assert!(saved_set.contains_active("1701"));
        assert!(!saved_set.contains_active("2551Q"));
        assert_eq!(saved_profile.per_year_forms.get(&2026), Some(&saved_set));
    }

    #[test]
    fn queued_election_reports_recorded_post_commit_refresh_request() {
        let db = test_db();
        let profile = test_profile();
        insert_test_profile(&db, &profile);
        let draft = queued_eight_percent_draft(&profile);

        let outcome = db
            .save_queued_2551q_draft_and_election_with_post_commit_status(&draft)
            .expect("the queued draft and election transaction should commit");

        assert!(outcome.refresh_status().request_recorded());
        assert_eq!(
            db.get_setting("google_calendar_sync_requested").unwrap(),
            Some("true".to_string())
        );
        assert!(db.get_2551q_draft(&draft.tin, 2026, 1).unwrap().is_some());
    }

    #[test]
    fn queued_election_refresh_failure_warns_without_rolling_back_committed_data() {
        let db = test_db();
        let profile = test_profile();
        insert_test_profile(&db, &profile);
        db.conn
            .execute_batch(
                "DELETE FROM settings WHERE key = 'google_calendar_sync_requested';
                 CREATE TRIGGER reject_calendar_refresh_insert
                 BEFORE INSERT ON settings
                 WHEN NEW.key = 'google_calendar_sync_requested'
                 BEGIN
                   SELECT RAISE(ABORT, 'forced refresh request failure');
                 END;
                 CREATE TRIGGER reject_calendar_refresh_update
                 BEFORE UPDATE ON settings
                 WHEN NEW.key = 'google_calendar_sync_requested'
                 BEGIN
                   SELECT RAISE(ABORT, 'forced refresh request failure');
                 END;",
            )
            .unwrap();
        let draft = queued_eight_percent_draft(&profile);

        let outcome = db
            .save_queued_2551q_draft_and_election_with_post_commit_status(&draft)
            .expect("refresh failure must not misreport the committed transaction as rolled back");

        assert!(matches!(
            outcome.refresh_status(),
            super::super::PostCommitRefreshStatus::Failed { warning }
                if warning.contains("committed")
                    && warning.contains("forced refresh request failure")
        ));
        let saved_profile = db.get_profile(&draft.tin).unwrap().unwrap();
        assert!(saved_profile.tax_elections.iter().any(|entry| {
            entry.taxable_year == 2026 && entry.election == IncomeTaxElection::EightPercent
        }));
        assert!(db.get_2551q_draft(&draft.tin, 2026, 1).unwrap().is_some());
    }

    #[test]
    fn queued_election_reconciliation_preserves_manual_forms_set_decisions() {
        let db = test_db();
        let profile = test_profile();
        insert_test_profile(&db, &profile);
        let mut existing = crate::forms::PerYearFormsSet::from_codes(
            2026,
            ["2551Q", "0605"],
            crate::forms::FormSetSource::CorAi,
        );
        existing
            .entries
            .iter_mut()
            .find(|entry| entry.form_code == "2551Q")
            .expect("2551Q fixture entry")
            .apply_manual_decision(true, Some("Accountant confirmed filing".into()));
        existing
            .entries
            .iter_mut()
            .find(|entry| entry.form_code == "0605")
            .expect("0605 fixture entry")
            .apply_manual_decision(false, Some("Not applicable for this year".into()));
        db.save_per_year_forms(&profile.tin.full(), 2026, &existing)
            .unwrap();

        db.save_queued_2551q_draft_and_election(&queued_eight_percent_draft(&profile))
            .expect("profile, Forms Set, and draft should commit together");

        let saved = db.get_per_year_forms(&profile.tin.full(), 2026).unwrap();
        let included = saved.entry("2551Q").expect("manual include must remain");
        assert!(included.active);
        assert_eq!(included.source, crate::forms::FormSetSource::Manual);
        let excluded = saved.entry("0605").expect("manual exclude must remain");
        assert!(!excluded.active);
        assert_eq!(excluded.source, crate::forms::FormSetSource::Manual);
    }

    #[test]
    fn forms_set_write_failure_rolls_back_queued_election_and_draft() {
        let db = test_db();
        let profile = test_profile();
        insert_test_profile(&db, &profile);
        db.conn
            .execute_batch(
                "CREATE TRIGGER reject_item13_forms_set
                 BEFORE INSERT ON per_year_forms
                 BEGIN
                   SELECT RAISE(ABORT, 'forced Forms Set failure');
                 END;",
            )
            .unwrap();
        let draft = queued_eight_percent_draft(&profile);

        let error = db
            .save_queued_2551q_draft_and_election(&draft)
            .expect_err("a Forms Set write failure must abort the transaction");

        assert!(error.to_string().contains("forced Forms Set failure"));
        let saved_profile = db.get_profile(&draft.tin).unwrap().unwrap();
        assert!(
            saved_profile
                .tax_elections
                .iter()
                .all(|entry| entry.taxable_year != 2026)
        );
        assert!(db.get_2551q_draft(&draft.tin, 2026, 1).unwrap().is_none());
        assert!(db.get_per_year_forms(&draft.tin, 2026).unwrap().is_empty());
    }

    #[test]
    fn queued_new_registrant_initial_quarter_records_eight_percent_election() {
        let db = test_db();
        let mut profile = test_profile();
        profile.business_start_date = chrono::NaiveDate::from_ymd_opt(2026, 8, 15);
        profile.ensure_profile_version_ledger();
        insert_test_profile(&db, &profile);

        let mut draft = Form2551QDraft::new_from_effective_profile(&profile, 2026, 3);
        assert_eq!(draft.item_13_is_applicable(), Some(true));
        draft.item_13_election = Item13Election::EightPercent;
        draft
            .transition_to_queued()
            .expect("a new registrant's NIL PT010 initial return should queue");

        db.save_queued_2551q_draft_and_election(&draft)
            .expect("the draft and annual election should commit atomically");

        let saved_profile = db.get_profile(&draft.tin).unwrap().unwrap();
        assert!(saved_profile.tax_elections.iter().any(|entry| {
            entry.taxable_year == 2026 && entry.election == IncomeTaxElection::EightPercent
        }));
        assert_eq!(
            db.get_2551q_draft(&draft.tin, 2026, 3)
                .unwrap()
                .unwrap()
                .status,
            FilingStatus::Queued
        );
    }

    #[test]
    fn queued_graduated_return_atomically_records_unspecified_graduated_election() {
        let db = test_db();
        let profile = test_profile();
        insert_test_profile(&db, &profile);
        let draft = queued_graduated_draft(&profile);

        db.save_queued_2551q_draft_and_election(&draft)
            .expect("an unchanged current profile should accept the queued draft");

        let saved_profile = db.get_profile(&draft.tin).unwrap().unwrap();
        let elections = saved_profile
            .tax_elections
            .iter()
            .filter(|entry| entry.taxable_year == 2026)
            .collect::<Vec<_>>();
        assert_eq!(elections.len(), 1);
        assert_eq!(
            elections[0].election,
            IncomeTaxElection::GraduatedUnspecified
        );
        assert_eq!(elections[0].source_form, "2551Qv2018");
        assert_eq!(
            db.get_2551q_draft(&draft.tin, 2026, 1)
                .unwrap()
                .unwrap()
                .status,
            FilingStatus::Queued
        );
    }

    #[test]
    fn missing_current_profile_rejects_every_queued_election() {
        let profile = test_profile();
        for draft in [
            queued_graduated_draft(&profile),
            queued_eight_percent_draft(&profile),
        ] {
            let db = test_db();
            let error = db.save_queued_2551q_draft_and_election(&draft).unwrap_err();
            assert!(error.to_string().contains("profile"));
            assert!(db.get_2551q_draft(&draft.tin, 2026, 1).unwrap().is_none());
        }
    }

    #[test]
    fn unresolved_effective_profile_ledgers_reject_queue_without_partial_writes() {
        let reviewed_profile = test_profile();
        let cases = [
            {
                let mut profile = reviewed_profile.clone();
                profile.profile_versions.clear();
                ("missing ledger", profile)
            },
            {
                let mut profile = reviewed_profile.clone();
                profile.ensure_profile_version_ledger();
                let mut overlapping = profile.profile_versions[0].clone();
                overlapping.id = "overlapping-confirmed-version".to_string();
                overlapping.label = "Overlapping confirmed version".to_string();
                overlapping.effective_from = chrono::NaiveDate::from_ymd_opt(2025, 1, 1);
                profile.profile_versions.push(overlapping);
                ("overlapping ledger", profile)
            },
            {
                let mut profile = reviewed_profile.clone();
                profile.business_start_date = None;
                profile.profile_versions.clear();
                profile.ensure_profile_version_ledger();
                ("undated ledger", profile)
            },
            {
                let mut profile = reviewed_profile.clone();
                profile.ensure_profile_version_ledger();
                profile.profile_versions[0].effective_from =
                    chrono::NaiveDate::from_ymd_opt(2027, 1, 1);
                ("out-of-period ledger", profile)
            },
        ];

        for (case, current_profile) in cases {
            let db = test_db();
            insert_raw_test_profile(&db, &current_profile);
            let draft = queued_eight_percent_draft(&reviewed_profile);

            let error = db
                .save_queued_2551q_draft_and_election(&draft)
                .expect_err(case);

            assert!(
                error.to_string().contains("exact filing period"),
                "{case} returned an unexpected error: {error}"
            );
            assert!(db.get_2551q_draft(&draft.tin, 2026, 1).unwrap().is_none());
            assert!(
                db.get_profile(&draft.tin)
                    .unwrap()
                    .unwrap()
                    .tax_elections
                    .is_empty(),
                "{case} must not persist the draft's requested election"
            );
        }
    }

    #[test]
    fn stale_profile_derived_inputs_reject_without_partial_election_write() {
        // Taxpayer type changed after review: the stale Individual draft must
        // not append an 8% election to the now-Corporation profile.
        let db = test_db();
        let reviewed_profile = test_profile();
        let draft = queued_eight_percent_draft(&reviewed_profile);
        let mut current_profile = reviewed_profile.clone();
        current_profile.taxpayer_type = crate::profile::TaxpayerType::Corporation;
        insert_test_profile(&db, &current_profile);

        assert!(db.save_queued_2551q_draft_and_election(&draft).is_err());
        let saved_profile = db.get_profile(&draft.tin).unwrap().unwrap();
        assert!(saved_profile.tax_elections.is_empty());
        assert!(db.get_2551q_draft(&draft.tin, 2026, 1).unwrap().is_none());

        // EOPT tier is calculation-relevant and fingerprinted. A change after
        // review must likewise reject a generic graduated draft.
        let db = test_db();
        let mut reviewed_profile = test_profile();
        reviewed_profile.eopt_tier = Some(crate::profile::EoptTier::Medium);
        let draft = queued_graduated_draft(&reviewed_profile);
        let mut current_profile = reviewed_profile;
        current_profile.eopt_tier = Some(crate::profile::EoptTier::Micro);
        insert_test_profile(&db, &current_profile);

        assert!(db.save_queued_2551q_draft_and_election(&draft).is_err());
        assert!(db.get_2551q_draft(&draft.tin, 2026, 1).unwrap().is_none());
    }

    #[test]
    fn network_claim_blocks_stale_cancel_and_finishes_by_token() {
        let db = test_db();
        let profile = test_profile();
        insert_test_profile(&db, &profile);
        let queued = queued_graduated_draft(&profile);
        db.save_queued_2551q_draft_and_election(&queued)
            .expect("queued draft should persist before claiming");

        let (mut claimed, token) = match db
            .claim_queued_2551q_submission(
                &queued.tin,
                queued.taxable_year,
                queued.quarter,
                &queued.queued_submission_fingerprint,
                &queued.next_retry_at,
                queued.submission_attempts,
            )
            .unwrap()
        {
            Claim2551QSubmissionResult::Claimed { draft, token } => (draft, token),
            _ => panic!("the exact queued generation should be claimed"),
        };
        assert_eq!(
            claimed.submission_claim_token.as_deref(),
            Some(token.as_str())
        );

        let mut stale_cancel = queued.clone();
        stale_cancel.revert_to_draft();
        assert!(db.save_2551q_draft(&stale_cancel).is_err());
        let still_claimed = db
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(
            still_claimed.submission_claim_token.as_deref(),
            Some(token.as_str())
        );

        claimed.transition_to_submitted("queued.xml".to_string());
        db.finish_claimed_2551q_submission(&claimed, &token)
            .expect("only the claim owner should finish the attempt");
        let submitted = db
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(submitted.status, FilingStatus::Submitted);
        assert!(submitted.submission_claim_token.is_none());
        assert!(submitted.submission_claimed_at.is_none());
    }

    #[test]
    fn immutable_profile_reconciliation_preserves_newer_filing_state_and_payload() {
        let db = test_db();
        let profile = test_profile();
        insert_test_profile(&db, &profile);
        let stale_open_view = queued_graduated_draft(&profile);
        db.save_queued_2551q_draft_and_election(&stale_open_view)
            .expect("queued draft should persist before claiming");

        let (mut claimed, token) = match db
            .claim_queued_2551q_submission(
                &stale_open_view.tin,
                stale_open_view.taxable_year,
                stale_open_view.quarter,
                &stale_open_view.queued_submission_fingerprint,
                &stale_open_view.next_retry_at,
                stale_open_view.submission_attempts,
            )
            .unwrap()
        {
            Claim2551QSubmissionResult::Claimed { draft, token } => (draft, token),
            _ => panic!("the exact queued generation should be claimed"),
        };
        claimed.transition_to_submitted("authoritative-submission.xml".to_string());
        db.finish_claimed_2551q_submission(&claimed, &token)
            .expect("the claim owner should advance the stored return");

        let mut changed_profile = profile.clone();
        changed_profile.ensure_profile_version_ledger();
        changed_profile.full_name = "Changed Taxpayer Name".to_string();
        changed_profile.profile_versions[0].cor.registered_name =
            "Changed Taxpayer Name".to_string();

        // The caller still owns the pre-claim Queued snapshot, but the narrow
        // reconciliation path reloads the authoritative Submitted row.
        assert_eq!(stale_open_view.status, FilingStatus::Queued);
        let submitted = db
            .reconcile_immutable_2551q_profile_snapshot(
                &stale_open_view.tin,
                stale_open_view.taxable_year,
                stale_open_view.quarter,
                &changed_profile,
            )
            .expect("immutable marker reconciliation should succeed");
        assert_eq!(submitted.status, FilingStatus::Submitted);
        assert_eq!(
            submitted.submission_filename.as_deref(),
            Some("authoritative-submission.xml")
        );
        assert_eq!(submitted.taxpayer_name, "Test Taxpayer");
        assert!(submitted.profile_snapshot_stale);

        let mut confirmed = submitted.clone();
        confirmed.transition_to_confirmed("2026-04-25T12:00:00Z".to_string(), Some(42), None);
        db.save_2551q_draft(&confirmed)
            .expect("confirmation transition should persist");
        let confirmed = db
            .reconcile_immutable_2551q_profile_snapshot(
                &stale_open_view.tin,
                stale_open_view.taxable_year,
                stale_open_view.quarter,
                &changed_profile,
            )
            .expect("confirmed marker reconciliation should preserve confirmation");
        assert_eq!(confirmed.status, FilingStatus::Confirmed);
        assert_eq!(confirmed.receipt_id, Some(42));
        assert_eq!(
            confirmed.submission_filename.as_deref(),
            Some("authoritative-submission.xml")
        );

        let mut paid = confirmed;
        paid.transition_to_paid();
        db.save_2551q_draft(&paid)
            .expect("payment transition should persist");
        let paid = db
            .reconcile_immutable_2551q_profile_snapshot(
                &stale_open_view.tin,
                stale_open_view.taxable_year,
                stale_open_view.quarter,
                &changed_profile,
            )
            .expect("paid marker reconciliation should preserve payment");
        assert_eq!(paid.status, FilingStatus::Paid);
        assert_eq!(paid.receipt_id, Some(42));
        assert_eq!(paid.taxpayer_name, "Test Taxpayer");

        let stored = db
            .get_2551q_draft(
                &stale_open_view.tin,
                stale_open_view.taxable_year,
                stale_open_view.quarter,
            )
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, FilingStatus::Paid);
        assert_eq!(stored.receipt_id, Some(42));
        assert_eq!(stored.taxpayer_name, "Test Taxpayer");
        assert!(stored.profile_snapshot_stale);
    }

    #[test]
    fn immutable_profile_reconciliation_refuses_editable_draft_rows() {
        let db = test_db();
        let mut profile = test_profile();
        profile.ensure_profile_version_ledger();
        let draft = Form2551QDraft::new_from_effective_profile(&profile, 2026, 1);
        db.save_2551q_draft(&draft)
            .expect("editable draft should use the generic draft save path");
        let before = serde_json::to_value(
            db.get_2551q_draft(&draft.tin, draft.taxable_year, draft.quarter)
                .unwrap()
                .unwrap(),
        )
        .unwrap();

        let error = db
            .reconcile_immutable_2551q_profile_snapshot(
                &draft.tin,
                draft.taxable_year,
                draft.quarter,
                &profile,
            )
            .expect_err("immutable reconciliation must reject an editable draft");
        assert!(error.to_string().contains("Editable 2551Q drafts"));

        let after = serde_json::to_value(
            db.get_2551q_draft(&draft.tin, draft.taxable_year, draft.quarter)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn abandoned_network_claim_stays_fail_closed_for_manual_reconciliation() {
        let db = test_db();
        let profile = test_profile();
        insert_test_profile(&db, &profile);
        let queued = queued_graduated_draft(&profile);
        db.save_queued_2551q_draft_and_election(&queued)
            .expect("queued draft should persist before claiming");

        let token = match db
            .claim_queued_2551q_submission(
                &queued.tin,
                queued.taxable_year,
                queued.quarter,
                &queued.queued_submission_fingerprint,
                &queued.next_retry_at,
                queued.submission_attempts,
            )
            .unwrap()
        {
            Claim2551QSubmissionResult::Claimed { token, .. } => token,
            _ => panic!("the exact queued generation should be claimed"),
        };

        // Simulate a process crash by abandoning the claim without calling the
        // completion method. There is no time-based reclamation because the
        // transport outcome is unknowable and an automatic retry could duplicate
        // a filing that BIR already received.
        let abandoned = db
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(abandoned.status, FilingStatus::Queued);
        assert_eq!(
            abandoned.submission_claim_token.as_deref(),
            Some(token.as_str())
        );
        assert!(abandoned.submission_claimed_at.is_some());
        assert!(
            abandoned
                .last_error
                .as_deref()
                .is_some_and(|message| message.contains("Automatic retry is disabled"))
        );
        assert!(
            abandoned
                .last_error
                .as_deref()
                .is_some_and(|message| message.contains("contact support"))
        );

        assert!(matches!(
            db.claim_queued_2551q_submission(
                &queued.tin,
                queued.taxable_year,
                queued.quarter,
                &queued.queued_submission_fingerprint,
                &queued.next_retry_at,
                queued.submission_attempts,
            )
            .unwrap(),
            Claim2551QSubmissionResult::Superseded
        ));

        let mut stale_cancel = queued;
        stale_cancel.revert_to_draft();
        assert!(db.save_2551q_draft(&stale_cancel).is_err());
    }

    #[test]
    fn network_claim_is_a_queue_revision_cas_and_revalidates_profile() {
        let db = test_db();
        let mut profile = test_profile();
        profile.eopt_tier = Some(crate::profile::EoptTier::Medium);
        profile.ensure_profile_version_ledger();
        insert_test_profile(&db, &profile);
        let queued = queued_graduated_draft(&profile);
        db.save_queued_2551q_draft_and_election(&queued)
            .expect("queued draft should persist before claiming");

        assert!(matches!(
            db.claim_queued_2551q_submission(
                &queued.tin,
                queued.taxable_year,
                queued.quarter,
                &queued.queued_submission_fingerprint,
                &Some("different queue generation".to_string()),
                queued.submission_attempts,
            )
            .unwrap(),
            Claim2551QSubmissionResult::Superseded
        ));

        profile.eopt_tier = Some(crate::profile::EoptTier::Micro);
        profile.profile_versions[0].eopt_tier = Some(crate::profile::EoptTier::Micro);
        db.conn
            .execute(
                "UPDATE profiles SET data_json = ?1 WHERE tin = ?2",
                params![serde_json::to_string(&profile).unwrap(), &queued.tin],
            )
            .unwrap();
        match db
            .claim_queued_2551q_submission(
                &queued.tin,
                queued.taxable_year,
                queued.quarter,
                &queued.queued_submission_fingerprint,
                &queued.next_retry_at,
                queued.submission_attempts,
            )
            .unwrap()
        {
            Claim2551QSubmissionResult::Rejected { draft, errors } => {
                assert_eq!(draft.status, FilingStatus::Draft);
                assert!(errors.iter().any(|(field, _)| field == "profile_snapshot"));
            }
            _ => panic!("a changed current profile must not be claimed"),
        }
        assert!(
            db.get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
                .unwrap()
                .unwrap()
                .submission_claim_token
                .is_none()
        );
    }

    #[test]
    fn network_claim_rejects_when_the_effective_profile_becomes_unresolved() {
        let db = test_db();
        let profile = test_profile();
        insert_test_profile(&db, &profile);
        let queued = queued_graduated_draft(&profile);
        db.save_queued_2551q_draft_and_election(&queued)
            .expect("queued draft should persist before claiming");

        let mut unresolved_profile = db.get_profile(&queued.tin).unwrap().unwrap();
        unresolved_profile.profile_versions.clear();
        db.conn
            .execute(
                "UPDATE profiles SET data_json = ?1 WHERE tin = ?2",
                params![
                    serde_json::to_string(&unresolved_profile).unwrap(),
                    &queued.tin
                ],
            )
            .unwrap();

        match db
            .claim_queued_2551q_submission(
                &queued.tin,
                queued.taxable_year,
                queued.quarter,
                &queued.queued_submission_fingerprint,
                &queued.next_retry_at,
                queued.submission_attempts,
            )
            .unwrap()
        {
            Claim2551QSubmissionResult::Rejected { draft, errors } => {
                assert_eq!(draft.status, FilingStatus::Draft);
                assert!(
                    draft
                        .last_error
                        .as_deref()
                        .is_some_and(|message| message.contains("effective taxpayer profile"))
                );
                assert!(errors.iter().any(|(field, message)| {
                    field == "profile_resolution" && message.contains("No confirmed")
                }));
            }
            _ => panic!("an unresolved profile must reject a network claim"),
        }

        let stored = db
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, FilingStatus::Queued);
        assert!(stored.submission_claim_token.is_none());
    }

    #[test]
    fn conflicting_annual_election_rejects_queued_draft_without_changes() {
        let db = test_db();
        let mut profile = test_profile();
        profile.tax_elections.push(TaxElectionHistory {
            taxable_year: 2026,
            election: IncomeTaxElection::GraduatedOsd,
            elected_at: chrono::Utc::now().naive_utc(),
            source_form: "1701Q".to_string(),
        });
        insert_test_profile(&db, &profile);
        let draft = queued_eight_percent_draft(&test_profile());

        let error = db.save_queued_2551q_draft_and_election(&draft).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("conflicting income-tax election")
        );
        assert!(db.get_2551q_draft(&draft.tin, 2026, 1).unwrap().is_none());
        let saved_profile = db.get_profile(&draft.tin).unwrap().unwrap();
        assert_eq!(saved_profile.tax_elections.len(), 1);
        assert_eq!(
            saved_profile.tax_elections[0].election,
            IncomeTaxElection::GraduatedOsd
        );
    }

    #[test]
    fn draft_write_failure_rolls_back_new_profile_election() {
        let db = test_db();
        let profile = test_profile();
        insert_test_profile(&db, &profile);
        db.conn
            .execute_batch(
                "CREATE TRIGGER reject_2551q_draft
                 BEFORE INSERT ON form_drafts
                 WHEN NEW.form_code = '2551Q'
                 BEGIN
                     SELECT RAISE(ABORT, 'forced draft failure');
                 END;",
            )
            .unwrap();
        let draft = queued_eight_percent_draft(&profile);

        assert!(db.save_queued_2551q_draft_and_election(&draft).is_err());
        assert!(db.get_2551q_draft(&draft.tin, 2026, 1).unwrap().is_none());
        let saved_profile = db.get_profile(&draft.tin).unwrap().unwrap();
        assert!(saved_profile.tax_elections.is_empty());
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
        // Use a form that is still ExternalOrManualOnly (1700 is not implemented in-app)
        let result = db.save_form_draft_v2(
            "123456789000",
            "1700",
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
        let profile = test_profile();
        let mut monthly = editable_1601c_draft(&profile);
        monthly.month = 12;
        monthly.compute();
        db.save_1601c_draft(&monthly).unwrap();
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
