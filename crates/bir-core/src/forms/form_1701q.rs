use serde::{Deserialize, Serialize};

use crate::forms::{FilingPeriod, FilingStatus, FormValidator, TypedBirForm};

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
    pub status: FilingStatus,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
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

impl Form1701QDraft {
    pub fn new_from_profile(
        profile: &crate::profile::TaxpayerProfile,
        year: u16,
        quarter: u8,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            tin: profile.tin.full(),
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            taxable_year: year.to_string(),
            quarter: quarter.to_string(),
            status: FilingStatus::Draft,
            created_at: Some(now.clone()),
            updated_at: Some(now),
            ..Default::default()
        }
    }

    pub fn form_code(&self) -> &'static str {
        "1701Q"
    }

    pub fn form_type_id(&self) -> &'static str {
        "1701Qv2018"
    }

    pub fn taxable_year_u16(&self) -> u16 {
        self.taxable_year.parse::<u16>().unwrap_or_default()
    }

    pub fn quarter_u8(&self) -> u8 {
        self.quarter.parse::<u8>().unwrap_or(1).clamp(1, 3)
    }

    pub fn period_code(&self) -> String {
        format!("{}Q{}", self.taxable_year, self.quarter_u8())
    }

    pub fn default_submission_filename(&self) -> String {
        format!(
            "{}-1701Qv2018-{}.xml",
            self.tin.replace('-', ""),
            self.period_code()
        )
    }

    pub fn recompute(&mut self) {
        self.total_amount_payable = self.total_tax_due.max(0.0);
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        assert!(matches!(self.status, FilingStatus::Draft), "Must be Draft");
        Err(vec![(
            "support_level".to_string(),
            "Form 1701Q is scaffold-only and cannot be queued for submission yet.".to_string(),
        )])
    }

    pub fn transition_to_submitted(&mut self, filename: String) {
        assert!(
            matches!(self.status, FilingStatus::Queued),
            "Must be Queued"
        );
        let now = chrono::Utc::now().to_rfc3339();
        self.status = FilingStatus::Submitted;
        self.submitted_at = Some(now.clone());
        self.submission_filename = Some(filename);
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = Some(now);
    }

    pub fn revert_to_draft(&mut self) {
        assert!(
            !matches!(self.status, FilingStatus::Paid),
            "Cannot revert Paid"
        );
        self.status = FilingStatus::Draft;
        self.submitted_at = None;
        self.confirmed_at = None;
        self.submission_filename = None;
        self.receipt_id = None;
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
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
        if self.taxable_year.parse::<u16>().is_err() {
            errors.push((
                "taxable_year".to_string(),
                "Taxable year must be a 4-digit year".to_string(),
            ));
        }
        if !matches!(self.quarter.parse::<u8>(), Ok(1..=3)) {
            errors.push((
                "quarter".to_string(),
                "1701Q quarter must be 1, 2, or 3".to_string(),
            ));
        }

        errors
    }
}

impl TypedBirForm for Form1701QDraft {
    fn form_code(&self) -> &'static str {
        self.form_code()
    }

    fn form_type_id(&self) -> &'static str {
        self.form_type_id()
    }

    fn filing_period(&self) -> FilingPeriod {
        FilingPeriod::Quarterly(self.quarter_u8())
    }

    fn recompute(&mut self) {
        self.recompute();
    }

    fn to_bir_field_map(&self) -> std::collections::BTreeMap<String, String> {
        self.to_bir_field_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recompute_sets_amount_payable_from_tax_due() {
        let mut draft = Form1701QDraft {
            total_tax_due: 250.0,
            ..Default::default()
        };

        draft.recompute();

        assert_eq!(draft.total_amount_payable, 250.0);
        assert!(draft.updated_at.is_some());
    }

    #[test]
    fn scaffold_transition_rejects_queue() {
        let mut draft = Form1701QDraft {
            status: FilingStatus::Draft,
            ..Default::default()
        };

        let errors = draft.transition_to_queued().expect_err("must stay gated");

        assert_eq!(draft.status, FilingStatus::Draft);
        assert_eq!(errors[0].0, "support_level");
    }

    #[test]
    fn validates_year_and_quarter() {
        let draft = Form1701QDraft {
            tin: "123456789000".into(),
            rdo_code: "039".into(),
            taxable_year: "20x6".into(),
            quarter: "4".into(),
            ..Default::default()
        };

        let errors = draft.validate();

        assert!(errors.iter().any(|(field, _)| field == "taxable_year"));
        assert!(errors.iter().any(|(field, _)| field == "quarter"));
    }
}
