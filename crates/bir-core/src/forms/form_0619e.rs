//! BIR Form 0619E — Typed draft struct and computation logic.
//!
//! Generated from savefile: 00000000000000-0619E-042026.xml
//! Total BIR fields: 58
//! Form-specific fields: 36
//!
//! ⚠️ ScaffoldOnly — formula evidence not yet verified

use crate::forms::{FilingStatus, FormValidator};
use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};

/// Complete draft for Form 0619E.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form0619EDraft {
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
    /// BIR: `frm0619E:optAmend:N` (sample: `true`)
    pub opt_amend_n: bool,
    /// BIR: `frm0619E:optAmend:Y` (sample: `false`)
    pub opt_amend_y: bool,

    // === optCategory ===
    /// BIR: `frm0619E:optCategory:G` (sample: `false`)
    pub opt_category_g: bool,
    /// BIR: `frm0619E:optCategory:P` (sample: `true`)
    pub opt_category_p: bool,

    // === optWithheld ===
    /// BIR: `frm0619E:optWithheld:N` (sample: `false`)
    pub opt_withheld_n: bool,
    /// BIR: `frm0619E:optWithheld:Y` (sample: `true`)
    pub opt_withheld_y: bool,

    // === shared_text ===
    /// BIR: `txtAgency19` (sample: ``)
    pub txt_agency19: String,
    /// BIR: `txtAgency20` (sample: ``)
    pub txt_agency20: String,
    /// BIR: `txtAgency21` (sample: ``)
    pub txt_agency21: String,
    /// BIR: `txtAgency22` (sample: ``)
    pub txt_agency22: String,
    /// BIR: `txtAmount19` (sample: ``)
    pub txt_amount19: String,
    /// BIR: `txtAmount20` (sample: ``)
    pub txt_amount20: String,
    /// BIR: `txtAmount21` (sample: ``)
    pub txt_amount21: String,
    /// BIR: `txtAmount22` (sample: ``)
    pub txt_amount22: String,
    /// BIR: `txtDate19` (sample: ``)
    pub txt_date19: String,
    /// BIR: `txtDate20` (sample: ``)
    pub txt_date20: String,
    /// BIR: `txtDate21` (sample: ``)
    pub txt_date21: String,
    /// BIR: `txtDate22` (sample: ``)
    pub txt_date22: String,
    /// BIR: `txtNumber19` (sample: ``)
    pub txt_number19: String,
    /// BIR: `txtNumber20` (sample: ``)
    pub txt_number20: String,
    /// BIR: `txtNumber21` (sample: ``)
    pub txt_number21: String,
    /// BIR: `txtNumber22` (sample: ``)
    pub txt_number22: String,
    /// BIR: `txtParticular22` (sample: ``)
    pub txt_particular22: String,

    // === text_fields ===
    /// BIR: `frm0619E:txtAddress` (sample: `OLONGAPO`)
    pub txt_address: String,
    /// BIR: `frm0619E:txtAtc` (sample: `WME10`)
    pub txt_atc: String,
    /// BIR: `frm0619E:txtDueDay` (sample: `10`)
    pub txt_due_day: u32,
    /// BIR: `frm0619E:txtLineBus` (sample: `SOFTWARE%20DEVELOPMENT`)
    pub txt_line_bus: String,
    /// BIR: `frm0619E:txtTax14` (sample: `1,000.00`)
    pub txt_tax14: f64,
    /// BIR: `frm0619E:txtTax15` (sample: `0.00`)
    pub txt_tax15: f64,
    /// BIR: `frm0619E:txtTax16` (sample: `1,000.00`)
    pub txt_tax16: f64,
    /// BIR: `frm0619E:txtTax17A` (sample: `100.00`)
    pub txt_tax17a: f64,
    /// BIR: `frm0619E:txtTax17B` (sample: `30.00`)
    pub txt_tax17b: f64,
    /// BIR: `frm0619E:txtTax17C` (sample: `100.00`)
    pub txt_tax17c: f64,
    /// BIR: `frm0619E:txtTax17D` (sample: `230.00`)
    pub txt_tax17d: f64,
    /// BIR: `frm0619E:txtTax18` (sample: `1,230.00`)
    pub txt_tax18: f64,
    /// BIR: `frm0619E:txtTaxTypeCode` (sample: `WE`)
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

impl FormValidator for Form0619EDraft {
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

impl Form0619EDraft {
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
            txt_agency19: String::new(),
            txt_agency20: String::new(),
            txt_agency21: String::new(),
            txt_agency22: String::new(),
            txt_amount19: String::new(),
            txt_amount20: String::new(),
            txt_amount21: String::new(),
            txt_amount22: String::new(),
            txt_date19: String::new(),
            txt_date20: String::new(),
            txt_date21: String::new(),
            txt_date22: String::new(),
            txt_number19: String::new(),
            txt_number20: String::new(),
            txt_number21: String::new(),
            txt_number22: String::new(),
            txt_particular22: String::new(),
            txt_address: String::new(),
            txt_atc: String::new(),
            txt_due_day: 0,
            txt_line_bus: String::new(),
            txt_tax14: 0.0,
            txt_tax15: 0.0,
            txt_tax16: 0.0,
            txt_tax17a: 0.0,
            txt_tax17b: 0.0,
            txt_tax17c: 0.0,
            txt_tax17d: 0.0,
            txt_tax18: 0.0,
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

    /// Recompute all derived fields per BIR 0619E computation rules.
    ///
    /// Computation flow (per official BIR form instructions):
    /// - Item 16 = Item 14 − Item 15  (Tax Still Due = Total Withheld − Adjustment)
    /// - Item 17D = 17A + 17B + 17C   (Total Penalties = Surcharge + Interest + Compromise)
    /// - Item 18 = Item 16 + Item 17D  (Total Amount Due = Tax Still Due + Total Penalties)
    ///
    /// Items 14, 15, 17A, 17B, 17C are user-entered. Items 16, 17D, 18 are derived.
    pub fn recompute(&mut self) {
        // Item 16: Tax Still Due (cannot be negative per BIR)
        self.txt_tax16 = (self.txt_tax14 - self.txt_tax15).max(0.0);

        // Item 17D: Total Penalties
        self.txt_tax17d = self.txt_tax17a + self.txt_tax17b + self.txt_tax17c;

        // Item 18: Total Amount Due
        self.txt_tax18 = self.txt_tax16 + self.txt_tax17d;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;

    fn test_profile() -> TaxpayerProfile {
        TaxpayerProfile {
            id: Some(1),
            full_name: "Test".into(),
            tin: Tin {
                segment1: "000".into(),
                segment2: "000".into(),
                segment3: "000".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: String::new(),
            registered_address: "Test".into(),
            zip_code: "1100".into(),
            phone: "09170000000".into(),
            email: "t@t.com".into(),
            default_form_type: "0619E".into(),
            taxpayer_type: Default::default(),
            is_vat_registered: false,
            business_start_date: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
        }
    }

    #[test]
    fn test_recompute_basic() {
        let mut draft = Form0619EDraft::new_from_profile(&test_profile(), 2024, 4);
        draft.txt_tax14 = 10_000.0; // Total withheld
        draft.txt_tax15 = 2_000.0; // Adjustment
        draft.txt_tax17a = 250.0; // Surcharge
        draft.txt_tax17b = 100.0; // Interest
        draft.txt_tax17c = 50.0; // Compromise

        draft.recompute();

        assert_eq!(draft.txt_tax16, 8_000.0); // Tax still due
        assert_eq!(draft.txt_tax17d, 400.0); // Total penalties
        assert_eq!(draft.txt_tax18, 8_400.0); // Total amount due
    }

    #[test]
    fn test_recompute_zero_adjustments() {
        let mut draft = Form0619EDraft::new_from_profile(&test_profile(), 2024, 4);
        draft.txt_tax14 = 5_000.0;
        // All other fields default to 0.0

        draft.recompute();

        assert_eq!(draft.txt_tax16, 5_000.0);
        assert_eq!(draft.txt_tax17d, 0.0);
        assert_eq!(draft.txt_tax18, 5_000.0);
    }

    #[test]
    fn test_recompute_negative_guard() {
        let mut draft = Form0619EDraft::new_from_profile(&test_profile(), 2024, 4);
        draft.txt_tax14 = 1_000.0;
        draft.txt_tax15 = 5_000.0; // Over-adjustment exceeds withheld

        draft.recompute();

        assert_eq!(draft.txt_tax16, 0.0, "Tax still due cannot be negative");
        assert_eq!(draft.txt_tax18, 0.0);
    }
}
