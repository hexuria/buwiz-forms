//! BIR Form 2551Q (January 2018 ENCS) — Quarterly Percentage Tax Return
//!
//! Data model, carry-forward logic, and auto-computation.

use super::FilingStatus;
use crate::forms::atc::find_atc;
use crate::penalties::{
    PenaltyConfig, PenaltyContext, PenaltyEngine, PenaltyProfile, TaxpayerClass,
};
use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
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
    pub eopt_tier: Option<crate::profile::EoptTier>,

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
    /// Line 17: Other Tax Credit/Payment — user-entered
    #[serde(default)]
    pub other_tax_credit: f64,
    /// Line 18: Total Tax Credits/Payments = sum of Lines 15, 16, 17
    #[serde(default)]
    pub total_tax_credits: f64,
    /// Line 19: Tax Still Payable/(Overpayment) = Line 14 Less Line 18
    /// Can be negative (overpayment)
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
            eopt_tier: profile.eopt_tier.clone(),
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
            other_tax_credit: 0.0,
            total_tax_credits: 0.0,
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

    /// Sync the draft's header fields with the current profile.
    /// This ensures that if the user updates their profile (e.g., phone number),
    /// the changes reflect in the draft return as long as it hasn't been submitted.
    pub fn sync_with_profile(&mut self, profile: &TaxpayerProfile) {
        self.tin = profile.tin.full();
        self.rdo_code = profile.rdo_code.clone();
        self.taxpayer_name = profile.full_name.clone();
        self.registered_address = profile.registered_address.clone();
        self.zip_code = profile.zip_code.clone();
        self.contact_number = profile.phone.clone();
        self.email = profile.email.clone();
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Recompute all derived values (call after any field change).
    /// `expected_sales` can be optionally provided by the ERP system to detect under-declaration fraud.
    #[allow(clippy::collapsible_if)]
    pub fn recompute(&mut self, expected_sales: Option<f64>) {
        for row in &mut self.schedule_1 {
            row.recompute();
        }
        // Line 14: Total Tax Due = sum of Schedule 1 rows
        self.total_tax_due =
            (self.schedule_1.iter().map(|r| r.tax_due).sum::<f64>() * 100.0).round() / 100.0;

        // Line 18: Total Tax Credits = Line 15 + Line 16 (if amended) + Line 17
        let previous_credit = if self.is_amended {
            self.tax_paid_previous
        } else {
            0.0
        };
        self.total_tax_credits =
            ((self.creditable_tax_withheld + previous_credit + self.other_tax_credit) * 100.0)
                .round()
                / 100.0;

        // Line 19: Tax Still Payable/(Overpayment) = Line 14 - Line 18
        // NOTE: Can be negative (overpayment). Do NOT clamp to zero.
        self.tax_payable = ((self.total_tax_due - self.total_tax_credits) * 100.0).round() / 100.0;

        // Compute deadline and penalties.
        // The penalty engine handles all three cases:
        //   1. Filed on time → all penalties = 0 (engine's own on-time check)
        //   2. Filed late, tax due → surcharge + interest + compromise
        //   3. Filed late, no tax due (overpayment) → surcharge=0, interest=0,
        //      compromise from gross_sales table (engine's unpaid_tax<=0 branch)
        //
        // We pass max(tax_payable, 0) as basic_tax_due so surcharge/interest
        // are computed on the positive amount only. Line 19 itself stays unclamped.
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

                let config = PenaltyConfig::default_rules();

                let gross_sales = self
                    .schedule_1
                    .iter()
                    .map(|r| r.taxable_amount)
                    .sum::<f64>();

                // Penalty base: clamp to 0 for surcharge/interest calc only.
                // Line 19 (self.tax_payable) is NOT clamped — it preserves overpayment.
                let penalty_tax_base = self.tax_payable.max(0.0);

                let taxpayer_class = match self.eopt_tier {
                    Some(crate::profile::EoptTier::Micro) => TaxpayerClass::Micro,
                    Some(crate::profile::EoptTier::Small) => TaxpayerClass::Small,
                    Some(crate::profile::EoptTier::Medium) => TaxpayerClass::Medium,
                    Some(crate::profile::EoptTier::Large) => TaxpayerClass::Large,
                    None => TaxpayerClass::Regular,
                };

                let mut is_fraud = false;
                if let Some(expected) = expected_sales {
                    if crate::integration::fraud::detect_under_declaration(expected, gross_sales) {
                        is_fraud = true;
                    }
                }

                let ctx = PenaltyContext {
                    form_code: "2551Qv2018".to_string(),
                    tax_type: PenaltyProfile::StandardFiling,
                    taxpayer_class,
                    taxable_period: format!("Q{} {}", self.quarter, self.taxable_year),
                    is_amended_return: self.is_amended,
                    original_was_on_time: true,
                    is_fraud_or_willful_neglect: is_fraud,
                    basic_tax_due: penalty_tax_base,
                    amount_paid_before_deadline: 0.0,
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

        // Line 23: Total Penalties
        self.total_penalties =
            ((self.surcharge + self.interest + self.compromise) * 100.0).round() / 100.0;
        // Line 24: Total Amount Payable/(Overpayment) = Line 19 + Line 23
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
    pub fn transition_to_confirmed(
        &mut self,
        confirmed_at: String,
        receipt_id: Option<i64>,
        filename: Option<String>,
    ) {
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

        if self.is_amended && self.tax_paid_previous < 0.0 {
            errors.push((
                "tax_paid_previous".to_string(),
                "Tax paid in return previously filed must be non-negative".to_string(),
            ));
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;
    use crate::profile::{TaxpayerProfile, TaxpayerType};

    fn test_profile() -> TaxpayerProfile {
        TaxpayerProfile {
            id: None,
            full_name: "Test Taxpayer".into(),
            tin: Tin {
                segment1: "123".into(),
                segment2: "456".into(),
                segment3: "789".into(),
                branch: "000".into(),
            },
            rdo_code: "018".into(),
            line_of_business: "Retail".into(),
            registered_address: "Manila".into(),
            zip_code: "1000".into(),
            phone: "09123456789".into(),
            email: "test@example.com".into(),
            default_form_type: "2551Qv2018".into(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            is_archived: false,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            profile_pin_hash: None,
            totp_secret: None,
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
        }
    }

    /// Helper: create a draft with given taxable_amount, creditable_tax_withheld,
    /// and quarter/year that determines if it's filed on time or late.
    fn make_draft(
        taxable_amount: f64,
        creditable_withheld: f64,
        year: u16,
        quarter: u8,
    ) -> Form2551QDraft {
        let mut draft = Form2551QDraft::new_from_profile(&test_profile(), year, quarter);
        draft.schedule_1[0].taxable_amount = taxable_amount;
        draft.creditable_tax_withheld = creditable_withheld;
        draft.recompute(None);
        draft
    }

    #[test]
    fn scenario_1_filed_on_time_with_overpayment() {
        // Q1 2026 deadline = 2026-04-25. If today < deadline, filed on time.
        // We use a future year to guarantee on-time filing.
        let mut draft = make_draft(50_000.0, 4_000.0, 2099, 1);
        draft.recompute(None);

        // Line 14: 50000 * 3% = 1500
        assert_eq!(draft.total_tax_due, 1500.0);
        // Line 18: 4000 (creditable) + 0 (previous) + 0 (other)
        assert_eq!(draft.total_tax_credits, 4000.0);
        // Line 19: 1500 - 4000 = -2500 (overpayment, NOT clamped)
        assert_eq!(draft.tax_payable, -2500.0);
        // Filed on time → all penalties = 0
        assert_eq!(draft.surcharge, 0.0);
        assert_eq!(draft.interest, 0.0);
        assert_eq!(draft.compromise, 0.0);
        assert_eq!(draft.total_penalties, 0.0);
        // Line 24: -2500 + 0 = -2500
        assert_eq!(draft.total_amount_payable, -2500.0);
    }

    #[test]
    fn scenario_2_filed_late_with_overpayment() {
        // Use a past quarter to guarantee late filing
        let mut draft = make_draft(50_000.0, 4_000.0, 2020, 1);
        draft.recompute(None);

        // Line 14: 1500
        assert_eq!(draft.total_tax_due, 1500.0);
        // Line 19: -2500 (overpayment)
        assert_eq!(draft.tax_payable, -2500.0);
        // Filed late but no unpaid tax → surcharge=0, interest=0
        assert_eq!(draft.surcharge, 0.0);
        assert_eq!(draft.interest, 0.0);
        // Compromise from "no amount due" tier: gross_sales=50000 ≤ 100000 → 1000
        assert_eq!(draft.compromise, 1000.0);
        assert_eq!(draft.total_penalties, 1000.0);
        // Line 24: -2500 + 1000 = -1500 (net overpayment)
        assert_eq!(draft.total_amount_payable, -1500.0);
    }

    #[test]
    fn scenario_3_filed_late_with_tax_due() {
        // Credits < tax due, past quarter
        let mut draft = make_draft(50_000.0, 400.0, 2020, 1);
        draft.recompute(None);

        // Line 14: 1500
        assert_eq!(draft.total_tax_due, 1500.0);
        // Line 18: 400
        assert_eq!(draft.total_tax_credits, 400.0);
        // Line 19: 1500 - 400 = 1100
        assert_eq!(draft.tax_payable, 1100.0);
        // Filed late with unpaid tax → surcharge, interest, and compromise apply
        assert!(
            draft.surcharge > 0.0,
            "surcharge should be positive for late filing with tax due"
        );
        assert!(
            draft.interest > 0.0,
            "interest should be positive for late filing with tax due"
        );
        assert!(
            draft.compromise > 0.0,
            "compromise should be positive for late filing with tax due"
        );
        // Line 24 = Line 19 + Line 23
        let expected_24 = ((draft.tax_payable + draft.total_penalties) * 100.0).round() / 100.0;
        assert_eq!(draft.total_amount_payable, expected_24);
        assert!(draft.total_amount_payable > draft.tax_payable);
    }

    #[test]
    fn zero_tax_filed_on_time_no_penalties() {
        let mut draft = make_draft(0.0, 0.0, 2099, 1);
        draft.recompute(None);

        assert_eq!(draft.total_tax_due, 0.0);
        assert_eq!(draft.tax_payable, 0.0);
        assert_eq!(draft.surcharge, 0.0);
        assert_eq!(draft.interest, 0.0);
        assert_eq!(draft.compromise, 0.0);
        assert_eq!(draft.total_amount_payable, 0.0);
    }

    #[test]
    fn multiple_atc_rows_sum_correctly() {
        let mut draft = Form2551QDraft::new_from_profile(&test_profile(), 2099, 1);
        // Row 1: PT010 at 3%
        draft.schedule_1[0].taxable_amount = 100_000.0;
        // Row 2: PT080 at 5%
        if let Some(row) = Schedule1Row::new("PT080") {
            draft.schedule_1.push(row);
        }
        draft.schedule_1[1].taxable_amount = 200_000.0;
        draft.recompute(None);

        // PT010: 100000 * 3% = 3000
        assert_eq!(draft.schedule_1[0].tax_due, 3000.0);
        // PT080: 200000 * 5% = 10000
        assert_eq!(draft.schedule_1[1].tax_due, 10000.0);
        // Line 14: 3000 + 10000 = 13000
        assert_eq!(draft.total_tax_due, 13000.0);
    }

    #[test]
    fn other_tax_credit_reduces_payable() {
        let mut draft = make_draft(50_000.0, 0.0, 2099, 1);
        draft.other_tax_credit = 500.0;
        draft.recompute(None);

        // Line 14: 1500
        assert_eq!(draft.total_tax_due, 1500.0);
        // Line 18: 0 + 0 + 500 = 500
        assert_eq!(draft.total_tax_credits, 500.0);
        // Line 19: 1500 - 500 = 1000
        assert_eq!(draft.tax_payable, 1000.0);
    }

    #[test]
    fn total_tax_credits_includes_all_three_sources() {
        let mut draft = make_draft(50_000.0, 1000.0, 2099, 1);
        draft.is_amended = true;
        draft.tax_paid_previous = 200.0;
        draft.other_tax_credit = 300.0;
        draft.recompute(None);

        // Line 18: 1000 + 200 + 300 = 1500
        assert_eq!(draft.total_tax_credits, 1500.0);
        // Line 19: 1500 - 1500 = 0
        assert_eq!(draft.tax_payable, 0.0);
    }

    #[test]
    fn tax_paid_previous_only_counted_when_amended() {
        let mut draft = make_draft(50_000.0, 1000.0, 2099, 1);
        draft.is_amended = false;
        draft.tax_paid_previous = 500.0; // should be ignored
        draft.recompute(None);

        // Line 18 should NOT include tax_paid_previous
        assert_eq!(draft.total_tax_credits, 1000.0);
        assert_eq!(draft.tax_payable, 500.0); // 1500 - 1000
    }
}
