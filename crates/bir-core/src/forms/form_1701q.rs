use serde::{Deserialize, Serialize};

use crate::forms::FormValidator;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Form1701QDraft {
    pub id: Option<i64>,
    pub tin: String,
    pub rdo_code: String,
    pub taxpayer_name: String,
    pub registered_address: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,

    // Taxable Year and Quarter
    pub taxable_year: String,
    pub quarter: String,

    // Part II Computations
    pub total_tax_due: f64,
    pub total_amount_payable: f64,

    // Status and Metadata
    pub status: super::FilingStatus,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl Form1701QDraft {
    pub fn new_from_profile(
        profile: &crate::profile::TaxpayerProfile,
        year: u16,
        quarter: u8,
    ) -> Self {
        Self {
            tin: profile.tin.full(),
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            taxable_year: year.to_string(),
            quarter: quarter.to_string(),
            ..Default::default()
        }
    }
}

impl FormValidator for Form1701QDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();

        if self.tin.trim().is_empty() {
            errors.push(("tin".to_string(), "TIN is required".to_string()));
        }
        if self.rdo_code.trim().is_empty() {
            errors.push(("rdo_code".to_string(), "RDO Code is required".to_string()));
        }

        // Add additional 1701Q validation rules here

        errors
    }
}
