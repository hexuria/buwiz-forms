//! BIR Form 2551Q (January 2018 ENCS) — Quarterly Percentage Tax Return
//!
//! Data model, carry-forward logic, and auto-computation.

use crate::forms::atc::find_atc;
use crate::penalties::{
    PenaltyConfig, PenaltyContext, PenaltyEngine, PenaltyProfile, TaxpayerClass,
};
use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// Status of a form draft or filed return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FilingStatus {
    #[default]
    Draft,
    Queued,
    #[serde(alias = "Filed")]
    Submitted,
    Confirmed,
    Paid,
}

/// One row in Schedule 1 — a single ATC category with its taxable amount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule1Row {
    /// Alphanumeric Tax Code, e.g. "PT010"
    pub atc: String,
    /// Human-readable description, e.g. "Persons exempt from VAT [Sec. 116]"
    pub atc_description: String,
    /// User-entered gross receipts for this ATC category
    pub taxable_amount: f64,
    /// Tax rate, auto-filled from ATC table (e.g. 0.03 for 3%)
    pub tax_rate: f64,
    /// Computed: taxable_amount × tax_rate
    pub tax_due: f64,
}

impl Schedule1Row {
    /// Create a new row for a given ATC code. Returns None if code not in ATC table.
    pub fn new(atc_code: &str) -> Option<Self> {
        let entry = find_atc(atc_code)?;
        Some(Self {
            atc: entry.code.to_string(),
            atc_description: entry.description.to_string(),
            taxable_amount: 0.0,
            tax_rate: entry.rate,
            tax_due: 0.0,
        })
    }

    /// Create a default PT010 row.
    pub fn default_pt010() -> Self {
        Self::new("PT010").expect("PT010 must exist in ATC table")
    }

    /// Recompute tax_due from taxable_amount and tax_rate.
    pub fn recompute(&mut self) {
        self.tax_due = (self.taxable_amount * self.tax_rate * 100.0).round() / 100.0;
    }
}

/// Complete draft or filed return for Form 2551Q.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form2551QDraft {
    /// Database row ID (None before first save)
    pub id: Option<i64>,

    // === Filing Period ===
    pub tin: String,
    pub taxable_year: u16,
    pub quarter: u8, // 1–4

    // === Header Options ===
    pub is_amended: bool,
    pub tax_relief: bool,

    // === Part I — pre-filled from profile, read-only in UI ===
    pub rdo_code: String,
    pub taxpayer_name: String,
    pub registered_address: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,

    // === Schedule 1 — user editable ===
    pub schedule_1: Vec<Schedule1Row>,

    // === Part II — computed from Schedule 1 ===
    /// Sum of all schedule_1[].tax_due
    pub total_tax_due: f64,
    /// From BIR Form 2307 — user-entered
    pub creditable_tax_withheld: f64,
    /// Only applicable when is_amended = true — LOCKED otherwise
    pub tax_paid_previous: f64,
    /// = total_tax_due - creditable_tax_withheld - (tax_paid_previous if amended)
    pub tax_payable: f64,

    // === Penalties ===
    #[serde(default = "default_true")]
    pub auto_compute_penalties: bool,
    #[serde(default)]
    pub surcharge: f64,
    #[serde(default)]
    pub interest: f64,
    #[serde(default)]
    pub compromise: f64,
    #[serde(default)]
    pub total_penalties: f64,
    #[serde(default)]
    pub total_amount_payable: f64,

    // === Status & Audit ===
    pub status: FilingStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub submitted_at: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<String>,
    #[serde(default)]
    pub submission_filename: Option<String>,
    #[serde(default)]
    pub receipt_id: Option<i64>,

    // === Background Retry Logic ===
    #[serde(default)]
    pub submission_attempts: u32,
    #[serde(default)]
    pub next_retry_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,

    /// Set to true when this draft was pre-filled from a previous quarter.
    /// UI shows a "Pre-filled from Q{n} {year}" banner when true.
    pub carried_forward_from: Option<(u16, u8)>, // (year, quarter)

    // === Payment / Attachments ===
    #[serde(default)]
    pub payment_receipt_path: Option<String>,
}

impl Form2551QDraft {
    /// Create a new draft pre-filled from a profile.
    /// Defaults to PT010 row with zero amounts.
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, quarter: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            quarter,
            is_amended: false,
            tax_relief: false,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            schedule_1: vec![Schedule1Row::default_pt010()],
            total_tax_due: 0.0,
            creditable_tax_withheld: 0.0,
            tax_paid_previous: 0.0,
            tax_payable: 0.0,
            auto_compute_penalties: true,
            surcharge: 0.0,
            interest: 0.0,
            compromise: 0.0,
            total_penalties: 0.0,
            total_amount_payable: 0.0,
            status: FilingStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            submitted_at: None,
            confirmed_at: None,
            submission_filename: None,
            receipt_id: None,
            submission_attempts: 0,
            next_retry_at: None,
            last_error: None,
            carried_forward_from: None,
            payment_receipt_path: None,
        }
    }

    /// Carry-forward: clone previous quarter's Schedule 1 rows as editable defaults.
    /// Preserves ATCs and amounts as starting point — user adjusts them.
    pub fn with_carried_forward(mut self, previous: &Form2551QDraft) -> Self {
        self.schedule_1 = previous.schedule_1.clone();
        self.carried_forward_from = Some((previous.taxable_year, previous.quarter));
        self
    }

    /// Recompute all derived values (call after any field change).
    pub fn recompute(&mut self) {
        for row in &mut self.schedule_1 {
            row.recompute();
        }
        self.total_tax_due =
            (self.schedule_1.iter().map(|r| r.tax_due).sum::<f64>() * 100.0).round() / 100.0;

        let previous_credit = if self.is_amended {
            self.tax_paid_previous
        } else {
            0.0
        };
        let total_credits = self.creditable_tax_withheld + previous_credit;
        self.tax_payable = ((self.total_tax_due - total_credits) * 100.0).round() / 100.0;
        self.tax_payable = self.tax_payable.max(0.0);

        // Compute deadline and penalties
        if self.auto_compute_penalties && matches!(self.status, FilingStatus::Draft) {
            let deadline_month = match self.quarter {
                1 => 4,
                2 => 7,
                3 => 10,
                _ => 1,
            };
            let deadline_year = if self.quarter == 4 {
                self.taxable_year + 1
            } else {
                self.taxable_year
            };

            if let Some(deadline) =
                chrono::NaiveDate::from_ymd_opt(deadline_year as i32, deadline_month, 25)
            {
                let today = chrono::Local::now().date_naive();

                let config = PenaltyConfig::default_rules(); // Dynamic loading point in the future

                let gross_sales = self
                    .schedule_1
                    .iter()
                    .map(|r| r.taxable_amount)
                    .sum::<f64>();

                let ctx = PenaltyContext {
                    form_code: "2551Qv2018".to_string(),
                    tax_type: PenaltyProfile::StandardFiling,
                    taxpayer_class: TaxpayerClass::Regular, // Default until Profile supports classification
                    taxable_period: format!("Q{} {}", self.quarter, self.taxable_year),
                    is_amended_return: self.is_amended,
                    original_was_on_time: true, // Optimistic default for amended returns
                    is_fraud_or_willful_neglect: false,
                    basic_tax_due: self.tax_payable,
                    amount_paid_before_deadline: 0.0, // Form UI does not capture this directly yet
                    gross_sales_or_receipts: gross_sales,
                    due_date: deadline,
                    filing_date: today,
                    payment_date: None,
                };

                let penalties = PenaltyEngine::calculate(&ctx, &config);
                self.surcharge = penalties.surcharge;
                self.interest = penalties.interest;
                self.compromise = penalties.compromise;
            }
        }

        self.total_penalties =
            ((self.surcharge + self.interest + self.compromise) * 100.0).round() / 100.0;
        self.total_amount_payable =
            ((self.tax_payable + self.total_penalties) * 100.0).round() / 100.0;

        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Returns a friendly label for the carry-forward banner.
    pub fn carry_forward_label(&self) -> Option<String> {
        self.carried_forward_from
            .map(|(year, q)| format!("Pre-filled from Q{} {} - adjust amounts as needed", q, year))
    }

    pub fn period_code(&self) -> String {
        format!("12{}Q{}", self.taxable_year, self.quarter)
    }

    pub fn default_submission_filename(&self) -> String {
        format!(
            "{}-2551Qv2018-{}#{}#.xml",
            self.tin,
            self.period_code(),
            self.email
        )
    }

    // ── State Transition Methods ──
    // These centralize all status mutations with precondition checks.
    // Callers should use these instead of directly assigning `self.status`.

    /// Returns true if the form fields should be editable (only in Draft status).
    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    /// Transition: Draft → Queued.
    /// Validates the form first. Returns Err with validation errors if invalid.
    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        assert!(
            matches!(self.status, FilingStatus::Draft),
            "Cannot queue form in {:?} status — must be Draft",
            self.status
        );
        let errors = <Self as super::FormValidator>::validate(self);
        if !errors.is_empty() {
            return Err(errors);
        }
        self.status = FilingStatus::Queued;
        self.submission_attempts = 0;
        self.next_retry_at = Some(chrono::Utc::now().to_rfc3339());
        self.last_error = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }

    /// Transition: Queued → Submitted (called by background cron after successful FTP upload).
    pub fn transition_to_submitted(&mut self, filename: String) {
        assert!(
            matches!(self.status, FilingStatus::Queued),
            "Cannot submit form in {:?} status — must be Queued",
            self.status
        );
        let now = chrono::Utc::now();
        self.status = FilingStatus::Submitted;
        self.submitted_at = Some(now.to_rfc3339());
        self.submission_filename = Some(filename);
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = now.to_rfc3339();
    }

    /// Transition: Submitted → Confirmed (called when BIR confirmation email is matched).
    pub fn transition_to_confirmed(&mut self, confirmed_at: String, receipt_id: Option<i64>, filename: Option<String>) {
        assert!(
            matches!(self.status, FilingStatus::Submitted),
            "Cannot confirm form in {:?} status — must be Submitted",
            self.status
        );
        self.status = FilingStatus::Confirmed;
        self.confirmed_at = Some(confirmed_at);
        self.receipt_id = receipt_id;
        if let Some(f) = filename {
            self.submission_filename = Some(f);
        }
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Transition: Confirmed → Paid (called by user action after bank payment).
    pub fn transition_to_paid(&mut self) {
        assert!(
            matches!(self.status, FilingStatus::Confirmed),
            "Cannot mark as paid in {:?} status — must be Confirmed",
            self.status
        );
        self.status = FilingStatus::Paid;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Transition: Any non-terminal → Draft (revert). Clears submission metadata.
    pub fn revert_to_draft(&mut self) {
        assert!(
            !matches!(self.status, FilingStatus::Paid),
            "Cannot revert a Paid form to Draft"
        );
        self.status = FilingStatus::Draft;
        self.submitted_at = None;
        self.confirmed_at = None;
        self.receipt_id = None;
        self.submission_filename = None;
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Record a failed submission attempt with exponential backoff.
    /// After 5 failures, automatically reverts to Draft.
    pub fn record_submission_failure(&mut self, error_msg: String) {
        assert!(
            matches!(self.status, FilingStatus::Queued),
            "Cannot record submission failure in {:?} status — must be Queued",
            self.status
        );
        self.submission_attempts += 1;
        self.last_error = Some(error_msg);

        if self.submission_attempts >= 5 {
            self.status = FilingStatus::Draft;
            self.next_retry_at = None;
        } else {
            let delay_mins = 2i64.pow(self.submission_attempts - 1);
            let next_time = chrono::Utc::now() + chrono::Duration::minutes(delay_mins);
            self.next_retry_at = Some(next_time.to_rfc3339());
        }
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}

/// Summary record returned by database list queries (no full JSON needed).
#[derive(Debug, Clone)]
pub struct FormDraftSummary {
    pub id: i64,
    pub tin: String,
    pub form_code: String,
    pub taxable_year: u16,
    pub quarter: Option<u8>,
    pub status: FilingStatus,
    pub updated_at: String,
}

impl FormDraftSummary {
    pub fn quarter_state(&self) -> QuarterState {
        match self.status {
            FilingStatus::Draft => QuarterState::Draft,
            FilingStatus::Queued => QuarterState::Queued,
            FilingStatus::Submitted => QuarterState::Submitted,
            FilingStatus::Confirmed => QuarterState::Confirmed,
            FilingStatus::Paid => QuarterState::Paid,
        }
    }
}

/// Per-quarter filing state for a form card on the dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarterState {
    NotStarted,
    Draft,
    Queued,
    Submitted,
    Confirmed,
    Paid,
}

/// Aggregated filing progress for a single form in a given year.
#[derive(Debug, Clone)]
pub struct FormFilingProgress {
    pub form_code: String,
    pub taxable_year: u16,
    /// For quarterly forms: index 0 = Q1, 1 = Q2, 2 = Q3, 3 = Q4
    pub quarters: [QuarterState; 4],
    /// For monthly forms: index 0 = Jan, 11 = Dec
    pub months: [QuarterState; 12],
    /// For annual forms
    pub annual_status: QuarterState,
    /// For open-ended forms: total filed this year
    pub open_ended_count: u32,
}

impl FormFilingProgress {
    pub fn new_empty(form_code: &str, year: u16) -> Self {
        Self {
            form_code: form_code.to_string(),
            taxable_year: year,
            quarters: [
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
            ],
            months: [
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
            ],
            annual_status: QuarterState::NotStarted,
            open_ended_count: 0,
        }
    }

    /// How many quarters have been filed (for quarterly forms).
    pub fn filed_count(&self) -> u32 {
        self.quarters
            .iter()
            .filter(|q| {
                **q == QuarterState::Queued
                    || **q == QuarterState::Submitted
                    || **q == QuarterState::Confirmed
                    || **q == QuarterState::Paid
            })
            .count() as u32
    }

    /// Next quarter that hasn't been started yet (1-based). Returns None if all 4 submitted/confirmed.
    pub fn next_unfiled_quarter(&self) -> Option<u8> {
        self.quarters
            .iter()
            .enumerate()
            .find(|(_, s)| **s == QuarterState::NotStarted)
            .map(|(i, _)| (i + 1) as u8)
    }

    /// Next quarter that is a draft in progress (1-based).
    pub fn draft_quarter(&self) -> Option<u8> {
        self.quarters
            .iter()
            .enumerate()
            .find(|(_, s)| **s == QuarterState::Draft || **s == QuarterState::Queued)
            .map(|(i, _)| (i + 1) as u8)
    }
}

use super::FormValidator;
use crate::validation::{validate_email, validate_ph_phone, validate_zip};

impl FormValidator for Form2551QDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        if !(1900..=9999).contains(&self.taxable_year) {
            errors.push((
                "taxable_year".to_string(),
                "Taxable year must be a 4-digit year".to_string(),
            ));
        }

        if !(1..=4).contains(&self.quarter) {
            errors.push(("quarter".to_string(), "Quarter is required".to_string()));
        }

        for (key, label, value) in [
            ("tin", "TIN", self.tin.as_str()),
            ("rdo_code", "RDO Code", self.rdo_code.as_str()),
            (
                "taxpayer_name",
                "Taxpayer Name",
                self.taxpayer_name.as_str(),
            ),
            (
                "registered_address",
                "Registered Address",
                self.registered_address.as_str(),
            ),
            ("zip_code", "ZIP Code", self.zip_code.as_str()),
            (
                "contact_number",
                "Contact Number",
                self.contact_number.as_str(),
            ),
            ("email", "Email Address", self.email.as_str()),
        ] {
            if value.trim().is_empty() {
                errors.push((key.to_string(), format!("{label} is required")));
            }
        }

        if self.zip_code.trim().is_empty() {
            // Already handled by the loop above, but we keep it here if we want to separate logic
        } else if !validate_zip(&self.zip_code) {
            errors.push((
                "zip_code".to_string(),
                "Zip Code must be 4 digits".to_string(),
            ));
        }

        if self.contact_number.trim().is_empty() {
            // Already handled
        } else if !validate_ph_phone(&self.contact_number) {
            errors.push((
                "contact_number".to_string(),
                "Contact Number must be valid".to_string(),
            ));
        }

        if !self.email.trim().is_empty() && !validate_email(&self.email) {
            errors.push((
                "email".to_string(),
                "Email Address must be a valid email".to_string(),
            ));
        }

        if self.schedule_1.is_empty() {
            errors.push((
                "schedule_1".to_string(),
                "Schedule 1 requires at least one ATC row".to_string(),
            ));
        }
        for (i, row) in self.schedule_1.iter().enumerate() {
            if row.taxable_amount < 0.0 {
                errors.push((
                    format!("schedule_1_row_{}", i + 1),
                    format!(
                        "Schedule 1 row {} taxable amount must be non-negative",
                        i + 1
                    ),
                ));
            }
        }

        if self.creditable_tax_withheld < 0.0 {
            errors.push((
                "creditable_withheld".to_string(),
                "Creditable percentage tax withheld must be non-negative".to_string(),
            ));
        }

        if self.is_amended
            && self.tax_paid_previous < 0.0 {
                errors.push((
                    "tax_paid_previous".to_string(),
                    "Tax paid in return previously filed must be non-negative".to_string(),
                ));
            }

        errors
    }
}
