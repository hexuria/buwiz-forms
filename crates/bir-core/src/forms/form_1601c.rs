//! BIR Form 1601C (Monthly Remittance Return of Income Taxes Withheld on Compensation)
//!
//! Data model and auto-computation logic based on 1601Cv2018 ENCS offline forms.

use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};
use super::{FilingStatus, FormValidator};
use crate::validation::{validate_zip, validate_ph_phone};

fn default_true() -> bool {
    true
}

/// Complete draft or filed return for Form 1601C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form1601CDraft {
    /// Database row ID (None before first save)
    pub id: Option<i64>,

    // === Filing Period ===
    pub tin: String,
    pub taxable_year: u16,
    pub month: u8, // 1–12

    // === Header Options ===
    pub is_amended: bool,
    pub any_taxes_withheld: bool, // Line 3
    pub number_of_sheets: u32,    // Line 4
    pub atc: String,              // Line 5

    // === Part I — pre-filled from profile ===
    pub rdo_code: String,
    pub line_of_business: String,
    pub taxpayer_name: String,
    pub contact_number: String,
    pub registered_address: String,
    pub zip_code: String,
    pub category_of_agent: String, // "P" for Private, "G" for Government
    pub email_address: String,

    // === Part II — Computation of Tax ===
    
    #[serde(default)]
    pub tax_14_total_compensation: f64,
    
    // Less: Non-Taxable/Exempt Compensation
    #[serde(default)]
    pub tax_15_statutory_minimum_wage: f64,
    #[serde(default)]
    pub tax_16_holiday_pay: f64,
    #[serde(default)]
    pub tax_17_13th_month_pay: f64,
    #[serde(default)]
    pub tax_18_de_minimis: f64,
    #[serde(default)]
    pub tax_19_sss_gsis: f64,
    
    #[serde(default)]
    pub tax_20_other_name: String,
    #[serde(default)]
    pub tax_20_other_amount: f64,
    
    // Computed Total Non-Taxable Compensation
    #[serde(default)]
    pub tax_21_total_non_taxable: f64,
    
    // Computed Total Taxable Compensation
    #[serde(default)]
    pub tax_22_total_taxable: f64,
    
    #[serde(default)]
    pub tax_23_not_subject: f64,
    
    // Computed Net Taxable Compensation
    #[serde(default)]
    pub tax_24_net_taxable: f64,
    
    // Total Taxes Withheld
    #[serde(default)]
    pub tax_25_total_taxes_withheld: f64,
    
    // Add/Less: Adjustment of Taxes Withheld from Previous Months
    #[serde(default)]
    pub tax_26_adjustment: f64,
    
    // Taxes Withheld for Remittance
    #[serde(default)]
    pub tax_27_taxes_withheld_for_remittance: f64,
    
    // Less: Tax Remitted in Return Previously Filed
    #[serde(default)]
    pub tax_28_tax_remitted_previously: f64,
    
    #[serde(default)]
    pub tax_29_other_remittances_name: String,
    #[serde(default)]
    pub tax_29_other_remittances_amount: f64,
    
    // Total Tax Remittances Made
    #[serde(default)]
    pub tax_30_total_tax_remittances: f64,
    
    // Tax Still Due/(Overremittance)
    #[serde(default)]
    pub tax_31_tax_still_due: f64,

    // === Penalties ===
    #[serde(default = "default_true")]
    pub auto_compute_penalties: bool,
    #[serde(default)]
    pub tax_32_surcharge: f64,
    #[serde(default)]
    pub tax_33_interest: f64,
    #[serde(default)]
    pub tax_34_compromise: f64,
    
    // Computed Total Penalties
    #[serde(default)]
    pub tax_35_total_penalties: f64,
    
    // Computed Total Amount Payable
    #[serde(default)]
    pub tax_36_total_amount_payable: f64,

    // === Status & Audit ===
    pub status: FilingStatus,
    pub created_at: String,
    pub updated_at: String,

    #[serde(default)]
    pub submission_attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_retry_at: Option<String>,
}

impl Form1601CDraft {
    /// Create a new draft pre-filled from a profile.
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, month: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            month,
            is_amended: false,
            any_taxes_withheld: true,
            number_of_sheets: 0,
            atc: "WW010".to_string(),
            rdo_code: profile.rdo_code.clone(),
            line_of_business: profile.line_of_business.clone(),
            taxpayer_name: profile.full_name.clone(),
            contact_number: profile.phone.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            category_of_agent: "P".to_string(), // Default Private
            email_address: profile.email.clone(),

            tax_14_total_compensation: 0.0,
            tax_15_statutory_minimum_wage: 0.0,
            tax_16_holiday_pay: 0.0,
            tax_17_13th_month_pay: 0.0,
            tax_18_de_minimis: 0.0,
            tax_19_sss_gsis: 0.0,
            tax_20_other_name: String::new(),
            tax_20_other_amount: 0.0,
            tax_21_total_non_taxable: 0.0,
            tax_22_total_taxable: 0.0,
            tax_23_not_subject: 0.0,
            tax_24_net_taxable: 0.0,
            tax_25_total_taxes_withheld: 0.0,
            tax_26_adjustment: 0.0,
            tax_27_taxes_withheld_for_remittance: 0.0,
            tax_28_tax_remitted_previously: 0.0,
            tax_29_other_remittances_name: String::new(),
            tax_29_other_remittances_amount: 0.0,
            tax_30_total_tax_remittances: 0.0,
            tax_31_tax_still_due: 0.0,
            
            auto_compute_penalties: true,
            tax_32_surcharge: 0.0,
            tax_33_interest: 0.0,
            tax_34_compromise: 0.0,
            tax_35_total_penalties: 0.0,
            tax_36_total_amount_payable: 0.0,
            
            status: FilingStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            submission_attempts: 0,
            submission_error: None,
            next_retry_at: None,
        }
    }

    pub fn period_code(&self) -> String {
        format!("{:02}{:04}", self.month, self.taxable_year)
    }

    pub fn default_submission_filename(&self) -> String {
        format!(
            "{}-1601Cv2018-{}.xml",
            self.tin.replace("-", ""),
            self.period_code()
        )
    }

    pub fn transition_to_submitted(&mut self, _filename: String) {
        self.status = FilingStatus::Submitted;
        self.updated_at = chrono::Utc::now().to_rfc3339();
        self.submission_error = None;
        self.next_retry_at = None;
    }

    pub fn revert_to_draft(&mut self) {
        self.status = FilingStatus::Draft;
        self.submission_attempts = 0;
        self.submission_error = None;
        self.next_retry_at = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn record_submission_failure(&mut self, error_msg: String) {
        self.submission_attempts += 1;
        self.submission_error = Some(error_msg);
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Recalculate computed fields based on user inputs.
    pub fn compute(&mut self) {
        // Line 21 = 15 + 16 + 17 + 18 + 19 + 20
        self.tax_21_total_non_taxable = ((self.tax_15_statutory_minimum_wage 
            + self.tax_16_holiday_pay
            + self.tax_17_13th_month_pay
            + self.tax_18_de_minimis
            + self.tax_19_sss_gsis
            + self.tax_20_other_amount) * 100.0).round() / 100.0;

        // Line 22 = 14 - 21
        self.tax_22_total_taxable = ((self.tax_14_total_compensation - self.tax_21_total_non_taxable) * 100.0).round() / 100.0;

        // Line 24 = 22 - 23
        self.tax_24_net_taxable = ((self.tax_22_total_taxable - self.tax_23_not_subject) * 100.0).round() / 100.0;

        // Line 27 = 25 + 26
        self.tax_27_taxes_withheld_for_remittance = ((self.tax_25_total_taxes_withheld + self.tax_26_adjustment) * 100.0).round() / 100.0;

        // Line 30 = 28 + 29
        self.tax_30_total_tax_remittances = ((self.tax_28_tax_remitted_previously + self.tax_29_other_remittances_amount) * 100.0).round() / 100.0;

        // Line 31 = 27 - 30
        self.tax_31_tax_still_due = ((self.tax_27_taxes_withheld_for_remittance - self.tax_30_total_tax_remittances) * 100.0).round() / 100.0;

        // Line 35 = 32 + 33 + 34
        self.tax_35_total_penalties = ((self.tax_32_surcharge + self.tax_33_interest + self.tax_34_compromise) * 100.0).round() / 100.0;

        // Line 36 = 31 + 35
        self.tax_36_total_amount_payable = ((self.tax_31_tax_still_due + self.tax_35_total_penalties) * 100.0).round() / 100.0;
        
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}

impl FormValidator for Form1601CDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();

        if !(1900..=9999).contains(&self.taxable_year) {
            errors.push((
                "taxable_year".to_string(),
                "Taxable year must be a 4-digit year".to_string(),
            ));
        }

        if !(1..=12).contains(&self.month) {
            errors.push((
                "month".to_string(),
                "Month must be between 1 and 12".to_string(),
            ));
        }

        if self.tin.trim().is_empty() {
            errors.push(("tin".to_string(), "TIN is required".to_string()));
        }

        if self.rdo_code.trim().is_empty() {
            errors.push(("rdo_code".to_string(), "RDO is required".to_string()));
        }

        if self.taxpayer_name.trim().is_empty() {
            errors.push(("taxpayer_name".to_string(), "Name is required".to_string()));
        }

        if self.registered_address.trim().is_empty() {
            errors.push((
                "registered_address".to_string(),
                "Address is required".to_string(),
            ));
        }

        if !validate_zip(&self.zip_code) {
            errors.push(("zip_code".to_string(), "Valid ZIP Code required".to_string()));
        }

        if !validate_ph_phone(&self.contact_number) {
            errors.push((
                "contact_number".to_string(),
                "Valid Philippine phone number required".to_string(),
            ));
        }

        // eFPS Parity Validations
        if self.category_of_agent != "P" && self.category_of_agent != "G" {
            errors.push((
                "category_of_agent".to_string(),
                "Please select an option for Category of Withholding Agent (Item 12)".to_string(),
            ));
        }

        if self.any_taxes_withheld {
            if self.tax_14_total_compensation <= 0.0 {
                errors.push((
                    "tax_14_total_compensation".to_string(),
                    "Invalid amount in Item 14. Value must be greater than zero(0) when Any Taxes Withheld is YES.".to_string(),
                ));
            }
            if self.tax_25_total_taxes_withheld <= 0.0 {
                errors.push((
                    "tax_25_total_taxes_withheld".to_string(),
                    "Invalid amount in Item 25. Value must be greater than zero(0) when Any Taxes Withheld is YES.".to_string(),
                ));
            }
        }

        // Math verification could be added here if needed

        errors
    }
}
