//! BIR Form 2551Q (January 2018 ENCS) — Quarterly Percentage Tax Return
//!
//! Data model, carry-forward logic, and auto-computation.

use crate::forms::atc::find_atc;
use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};

/// Status of a form draft or filed return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FilingStatus {
    #[default]
    Draft,
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

    /// Set to true when this draft was pre-filled from a previous quarter.
    /// UI shows a "Pre-filled from Q{n} {year}" banner when true.
    pub carried_forward_from: Option<(u16, u8)>, // (year, quarter)
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
            status: FilingStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            submitted_at: None,
            confirmed_at: None,
            submission_filename: None,
            receipt_id: None,
            carried_forward_from: None,
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
        format!("{}-2551Qv2018-{}.xml", self.tin, self.period_code())
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
                **q == QuarterState::Submitted
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
            .find(|(_, s)| **s == QuarterState::Draft)
            .map(|(i, _)| (i + 1) as u8)
    }
}
