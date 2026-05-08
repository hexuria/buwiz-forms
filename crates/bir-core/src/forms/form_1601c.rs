//! BIR Form 1601C (Monthly Remittance Return of Income Taxes Withheld on Compensation)
//!
//! Data model and auto-computation logic based on eFPS offline forms.

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

    // === Part I — pre-filled from profile ===
    pub rdo_code: String,
    pub line_of_business: String,
    pub taxpayer_name: String,
    pub contact_number: String,
    pub registered_address: String,
    pub zip_code: String,
    pub category_of_agent: String, // "P" for Private, "G" for Government

    // === Part II — Computation of Tax ===
    #[serde(default)]
    pub tax_15_total_compensation: f64,
    #[serde(default)]
    pub tax_16a_nontaxable: f64,
    #[serde(default)]
    pub tax_16b_not_subject: f64,
    #[serde(default)]
    pub tax_16c_exempt: f64,
    #[serde(default)]
    pub tax_17_regular: f64,
    #[serde(default)]
    pub tax_18_supplementary: f64,
    
    // Computed Total Taxable Compensation
    #[serde(default)]
    pub tax_19_total_taxable: f64,
    
    // User input (since tax tables are complex and vary per employee, 
    // the user provides the total withheld)
    #[serde(default)]
    pub tax_20_required_withheld: f64,

    // === Part II — Adjustments ===
    #[serde(default)]
    pub tax_21a_previous_withheld: f64,
    #[serde(default)]
    pub tax_21b_other_payments: f64,
    
    // Computed Tax Still Due
    #[serde(default)]
    pub tax_22_still_due: f64,

    // === Penalties ===
    #[serde(default = "default_true")]
    pub auto_compute_penalties: bool,
    #[serde(default)]
    pub tax_24a_surcharge: f64,
    #[serde(default)]
    pub tax_24b_interest: f64,
    #[serde(default)]
    pub tax_24c_compromise: f64,
    #[serde(default)]
    pub tax_24d_total_penalties: f64,
    
    // Computed Total Amount Payable
    #[serde(default)]
    pub tax_25_total_payable: f64,

    // === Status & Audit ===
    pub status: FilingStatus,
    pub created_at: String,
    pub updated_at: String,
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
            rdo_code: profile.rdo_code.clone(),
            line_of_business: profile.line_of_business.clone(),
            taxpayer_name: profile.full_name.clone(),
            contact_number: profile.phone.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            category_of_agent: "P".to_string(), // Default Private
            tax_15_total_compensation: 0.0,
            tax_16a_nontaxable: 0.0,
            tax_16b_not_subject: 0.0,
            tax_16c_exempt: 0.0,
            tax_17_regular: 0.0,
            tax_18_supplementary: 0.0,
            tax_19_total_taxable: 0.0,
            tax_20_required_withheld: 0.0,
            tax_21a_previous_withheld: 0.0,
            tax_21b_other_payments: 0.0,
            tax_22_still_due: 0.0,
            auto_compute_penalties: true,
            tax_24a_surcharge: 0.0,
            tax_24b_interest: 0.0,
            tax_24c_compromise: 0.0,
            tax_24d_total_penalties: 0.0,
            tax_25_total_payable: 0.0,
            status: FilingStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Recalculate computed fields based on user inputs.
    pub fn compute(&mut self) {
        // Line 19 = 15 - (16A + 16B + 16C)
        // However, according to typical BIR instructions, it's actually:
        // 17 + 18 (Taxable Compensation - Regular + Supplementary)
        // Let's implement the standard 19 = 17 + 18 as per eBIRForms logic.
        // Wait, eFPS script logic:
        // txtTax19.value = parseFloat(txtTax17.value) + parseFloat(txtTax18.value)
        // Also: txtTax15.value = parseFloat(txtTax16A.value) + 16B + 16C + 17 + 18
        // Let's set 19 based on 17 + 18.
        self.tax_19_total_taxable = ((self.tax_17_regular + self.tax_18_supplementary) * 100.0).round() / 100.0;
        
        // Line 22 = 20 - 21A - 21B
        self.tax_22_still_due = ((self.tax_20_required_withheld - self.tax_21a_previous_withheld - self.tax_21b_other_payments) * 100.0).round() / 100.0;

        // Line 24D = 24A + 24B + 24C
        self.tax_24d_total_penalties = ((self.tax_24a_surcharge + self.tax_24b_interest + self.tax_24c_compromise) * 100.0).round() / 100.0;

        // Line 25 = 22 + 24D
        self.tax_25_total_payable = ((self.tax_22_still_due + self.tax_24d_total_penalties) * 100.0).round() / 100.0;
        
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
        
        // Math verification
        let expected_19 = ((self.tax_17_regular + self.tax_18_supplementary) * 100.0).round() / 100.0;
        if (self.tax_19_total_taxable - expected_19).abs() > 0.01 {
            errors.push((
                "tax_19_total_taxable".to_string(),
                "Total Taxable Compensation must equal Regular + Supplementary".to_string(),
            ));
        }

        errors
    }
}
