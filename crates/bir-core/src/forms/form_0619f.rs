//! BIR Form 0619F — Typed draft struct and computation logic.
//!
//! Generated from savefile: 00000000000000-0619F-042026WB.xml
//! Total BIR fields: 59
//! Form-specific fields: 37
//!
//! ⚠️ ScaffoldOnly — formula evidence not yet verified

use crate::forms::{FilingStatus, FormValidator};
use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};

/// Complete draft for Form 0619F.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form0619FDraft {
    /// Database row ID (None before first save)
    pub id: Option<i64>,

    // === Filing Period ===
    pub tin: String,
    pub taxable_year: u16,
    pub month: u8,

    // === Header / Options ===
    pub is_amended: bool,

    // === Profile Fields (pre-filled) ===
    pub rdo_code: String,
    pub taxpayer_name: String,
    pub registered_address: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,

    // === optAmend ===
    /// BIR: `frm0619F:optAmend:N` (sample: `true`)
    pub opt_amend_n: bool,
    /// BIR: `frm0619F:optAmend:Y` (sample: `false`)
    pub opt_amend_y: bool,

    // === optCategory ===
    /// BIR: `frm0619F:optCategory:G` (sample: `false`)
    pub opt_category_g: bool,
    /// BIR: `frm0619F:optCategory:P` (sample: `true`)
    pub opt_category_p: bool,

    // === optWithheld ===
    /// BIR: `frm0619F:optWithheld:N` (sample: `false`)
    pub opt_withheld_n: bool,
    /// BIR: `frm0619F:optWithheld:Y` (sample: `true`)
    pub opt_withheld_y: bool,

    // === shared_text ===
    /// BIR: `txtAgency20` (sample: ``)
    pub txt_agency20: String,
    /// BIR: `txtAgency21` (sample: ``)
    pub txt_agency21: String,
    /// BIR: `txtAgency22` (sample: ``)
    pub txt_agency22: String,
    /// BIR: `txtAgency23` (sample: ``)
    pub txt_agency23: String,
    /// BIR: `txtAmount20` (sample: ``)
    pub txt_amount20: String,
    /// BIR: `txtAmount21` (sample: ``)
    pub txt_amount21: String,
    /// BIR: `txtAmount22` (sample: ``)
    pub txt_amount22: String,
    /// BIR: `txtAmount23` (sample: ``)
    pub txt_amount23: String,
    /// BIR: `txtDate20` (sample: ``)
    pub txt_date20: String,
    /// BIR: `txtDate21` (sample: ``)
    pub txt_date21: String,
    /// BIR: `txtDate22` (sample: ``)
    pub txt_date22: String,
    /// BIR: `txtDate23` (sample: ``)
    pub txt_date23: String,
    /// BIR: `txtNumber20` (sample: ``)
    pub txt_number20: String,
    /// BIR: `txtNumber21` (sample: ``)
    pub txt_number21: String,
    /// BIR: `txtNumber22` (sample: ``)
    pub txt_number22: String,
    /// BIR: `txtNumber23` (sample: ``)
    pub txt_number23: String,
    /// BIR: `txtParticular23` (sample: ``)
    pub txt_particular23: String,

    // === text_fields ===
    /// BIR: `frm0619F:txtAddress` (sample: `OLONGAPO`)
    pub txt_address: String,
    /// BIR: `frm0619F:txtDueDay` (sample: `10`)
    pub txt_due_day: u32,
    /// BIR: `frm0619F:txtLineBus` (sample: `SOFTWARE%2520DEVELOPMENT`)
    pub txt_line_bus: String,
    /// BIR: `frm0619F:txtTax13` (sample: `1,000.00`)
    pub txt_tax13: f64,
    /// BIR: `frm0619F:txtTax14` (sample: `0.00`)
    pub txt_tax14: f64,
    /// BIR: `frm0619F:txtTax15` (sample: `1,000.00`)
    pub txt_tax15: f64,
    /// BIR: `frm0619F:txtTax16` (sample: `0.00`)
    pub txt_tax16: f64,
    /// BIR: `frm0619F:txtTax17` (sample: `1,000.00`)
    pub txt_tax17: f64,
    /// BIR: `frm0619F:txtTax18A` (sample: `1,000.00`)
    pub txt_tax18a: f64,
    /// BIR: `frm0619F:txtTax18B` (sample: `1,000.00`)
    pub txt_tax18b: f64,
    /// BIR: `frm0619F:txtTax18C` (sample: `1,000.00`)
    pub txt_tax18c: f64,
    /// BIR: `frm0619F:txtTax18D` (sample: `3,000.00`)
    pub txt_tax18d: f64,
    /// BIR: `frm0619F:txtTax19` (sample: `4,000.00`)
    pub txt_tax19: f64,
    /// BIR: `frm0619F:txtTaxTypeCode` (sample: `WB`)
    pub txt_tax_type_code: String,

    // === Lifecycle ===
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
    #[serde(default)]
    pub submission_attempts: u32,
    #[serde(default)]
    pub next_retry_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl FormValidator for Form0619FDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        if self.tin.is_empty() {
            errors.push(("tin".into(), "TIN is required".into()));
        }
        if self.taxpayer_name.is_empty() {
            errors.push(("taxpayer_name".into(), "Taxpayer name is required".into()));
        }
        // TODO: Add form-specific validation rules
        errors
    }
}

impl Form0619FDraft {
    /// Create a new draft from a taxpayer profile.
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, month: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            month,
            is_amended: false,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            opt_amend_n: true,
            opt_amend_y: false,
            opt_category_g: false,
            opt_category_p: true,
            opt_withheld_n: false,
            opt_withheld_y: true,
            txt_agency20: String::new(),
            txt_agency21: String::new(),
            txt_agency22: String::new(),
            txt_agency23: String::new(),
            txt_amount20: String::new(),
            txt_amount21: String::new(),
            txt_amount22: String::new(),
            txt_amount23: String::new(),
            txt_date20: String::new(),
            txt_date21: String::new(),
            txt_date22: String::new(),
            txt_date23: String::new(),
            txt_number20: String::new(),
            txt_number21: String::new(),
            txt_number22: String::new(),
            txt_number23: String::new(),
            txt_particular23: String::new(),
            txt_address: String::new(),
            txt_due_day: 0,
            txt_line_bus: String::new(),
            txt_tax13: 0.0,
            txt_tax14: 0.0,
            txt_tax15: 0.0,
            txt_tax16: 0.0,
            txt_tax17: 0.0,
            txt_tax18a: 0.0,
            txt_tax18b: 0.0,
            txt_tax18c: 0.0,
            txt_tax18d: 0.0,
            txt_tax19: 0.0,
            txt_tax_type_code: String::new(),
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
        }
    }

    /// Recompute all derived fields per BIR 0619F computation rules.
    ///
    /// Computation flow (per official BIR form instructions):
    /// - Item 15 = Item 13 + Item 14  (Total = Withheld + Under-remittance from prior)
    /// - Item 17 = Item 15 − Item 16  (Tax Still Due = Total − Over-remittance from prior)
    /// - Item 18D = 18A + 18B + 18C   (Total Penalties = Surcharge + Interest + Compromise)
    /// - Item 19 = Item 17 + Item 18D (Total Amount Due = Tax Still Due + Total Penalties)
    ///
    /// Items 13, 14, 16, 18A, 18B, 18C are user-entered. Items 15, 17, 18D, 19 are derived.
    pub fn recompute(&mut self) {
        // Item 15: Total (Withheld + Under-remittance)
        self.txt_tax15 = self.txt_tax13 + self.txt_tax14;

        // Item 17: Tax Still Due (cannot be negative per BIR)
        self.txt_tax17 = (self.txt_tax15 - self.txt_tax16).max(0.0);

        // Item 18D: Total Penalties
        self.txt_tax18d = self.txt_tax18a + self.txt_tax18b + self.txt_tax18c;

        // Item 19: Total Amount Due
        self.txt_tax19 = self.txt_tax17 + self.txt_tax18d;

        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    // ── State Transition Methods ──

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        assert!(matches!(self.status, FilingStatus::Draft), "Must be Draft");
        let errors = <Self as FormValidator>::validate(self);
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

    pub fn transition_to_submitted(&mut self, filename: String) {
        assert!(
            matches!(self.status, FilingStatus::Queued),
            "Must be Queued"
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

    pub fn transition_to_confirmed(
        &mut self,
        confirmed_at: String,
        receipt_id: Option<i64>,
        filename: Option<String>,
    ) {
        assert!(
            matches!(self.status, FilingStatus::Submitted),
            "Must be Submitted"
        );
        self.status = FilingStatus::Confirmed;
        self.confirmed_at = Some(confirmed_at);
        self.receipt_id = receipt_id;
        if let Some(f) = filename {
            self.submission_filename = Some(f);
        }
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn transition_to_paid(&mut self) {
        assert!(
            matches!(self.status, FilingStatus::Confirmed),
            "Must be Confirmed"
        );
        self.status = FilingStatus::Paid;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn revert_to_draft(&mut self) {
        assert!(
            !matches!(self.status, FilingStatus::Paid),
            "Cannot revert Paid"
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

    pub fn record_submission_failure(&mut self, error_msg: String) {
        assert!(
            matches!(self.status, FilingStatus::Queued),
            "Must be Queued"
        );
        self.submission_attempts += 1;
        self.last_error = Some(error_msg);
        if self.submission_attempts >= 5 {
            self.status = FilingStatus::Draft;
            self.next_retry_at = None;
        } else {
            let delay = 2i64.pow(self.submission_attempts - 1);
            let next = chrono::Utc::now() + chrono::Duration::minutes(delay);
            self.next_retry_at = Some(next.to_rfc3339());
        }
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}
