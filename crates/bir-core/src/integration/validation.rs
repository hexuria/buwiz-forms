//! Integration payload validation — pre-flight checks before mapping.
//!
//! Validates the `UniversalTaxPayload` structure, period consistency,
//! and profile compatibility before the mapper engine runs.

use crate::forms::registry::{find_form, forms_for_taxpayer};
use crate::integration::models::UniversalTaxPayload;
use crate::profile::{TaxClassification, TaxpayerProfile};
use chrono::Datelike;

/// A single validation issue found in a payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadValidationError {
    pub field: String,
    pub message: String,
}

impl PayloadValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Validate a payload's structural integrity (independent of any profile).
pub fn validate_payload(payload: &UniversalTaxPayload) -> Vec<PayloadValidationError> {
    let mut errors = Vec::new();

    // TIN format
    let tin_len = payload.tin.len();
    if tin_len != 12 && tin_len != 13 {
        errors.push(PayloadValidationError::new(
            "tin",
            format!("TIN must be 12 or 13 digits, got {tin_len}"),
        ));
    }
    if !payload.tin.chars().all(|c| c.is_ascii_digit()) {
        errors.push(PayloadValidationError::new(
            "tin",
            "TIN must contain only digits",
        ));
    }

    // Period consistency
    if payload.period_end < payload.period_start {
        errors.push(PayloadValidationError::new(
            "period_end",
            "period_end must be on or after period_start",
        ));
    }

    // Period must not span more than 1 year
    let year_diff = payload.period_end.year() - payload.period_start.year();
    if year_diff > 1
        || (year_diff == 1 && payload.period_end.month() > payload.period_start.month())
    {
        errors.push(PayloadValidationError::new(
            "period_end",
            "Period must not span more than 12 months",
        ));
    }

    // amended-only fields
    if !payload.is_amended && payload.previous_tax_paid > 0.0 {
        errors.push(PayloadValidationError::new(
            "previous_tax_paid",
            "previous_tax_paid should be 0 for non-amended returns",
        ));
    }

    // Income source validation
    for (i, source) in payload.income_sources.iter().enumerate() {
        if source.gross_amount < 0.0 {
            errors.push(PayloadValidationError::new(
                format!("income_sources[{i}].gross_amount"),
                "Gross amount must be non-negative",
            ));
        }
        if let Some(rate) = source.tax_rate_override
            && (!(0.0..=1.0).contains(&rate))
        {
            errors.push(PayloadValidationError::new(
                format!("income_sources[{i}].tax_rate_override"),
                "Tax rate override must be between 0.0 and 1.0",
            ));
        }
    }

    if payload.creditable_withholdings < 0.0 {
        errors.push(PayloadValidationError::new(
            "creditable_withholdings",
            "Creditable withholdings must be non-negative",
        ));
    }

    errors
}

/// Validate that a target form is applicable for the given profile.
///
/// Uses both `TaxpayerType` (entity kind) and `TaxClassification` (filing behavior)
/// to determine eligibility.
pub fn validate_form_applicability(
    form_code: &str,
    profile: &TaxpayerProfile,
) -> Option<PayloadValidationError> {
    // Check if the form exists in the registry
    let form_def = match find_form(form_code) {
        Some(def) => def,
        None => {
            return Some(PayloadValidationError::new(
                "target_form",
                format!("Unknown form code: {form_code}"),
            ));
        }
    };

    // Check entity-level eligibility (TaxpayerType)
    let eligible_by_type = form_def.taxpayer_types.contains(&profile.taxpayer_type);
    if !eligible_by_type {
        return Some(PayloadValidationError::new(
            "target_form",
            format!(
                "Form {} is not applicable for {:?} taxpayers",
                form_code, profile.taxpayer_type
            ),
        ));
    }

    // If the profile has a TaxClassification, apply refined rules
    if let Some(ref classification) = profile.tax_classification {
        let applicable = is_form_applicable_for_classification(form_code, classification);
        if !applicable {
            return Some(PayloadValidationError::new(
                "target_form",
                format!(
                    "Form {} is not applicable for {:?} classification",
                    form_code, classification
                ),
            ));
        }
    }

    None
}

/// Determines if a specific form is applicable given a tax classification.
///
/// This encodes the BIR filing rules:
/// - `PurelyCompensation` → only income tax (1701/1701Q), no percentage/VAT tax
/// - `SoleProprietorNonVat` → percentage tax (2551Q), income tax (1701Q/1701)
/// - `SoleProprietorVat` → VAT (2550M), income tax (1701Q/1701), NOT percentage tax
/// - `ProfessionalOrFreelancer` → same as SoleProprietorNonVat or Vat (depends on VAT registration)
/// - `MixedIncome` → all individual forms
/// - `Corporation` → corporate forms (1702Q/1702RT), percentage tax or VAT
fn is_form_applicable_for_classification(
    form_code: &str,
    classification: &TaxClassification,
) -> bool {
    match classification {
        TaxClassification::PurelyCompensation => {
            // Only individual income tax forms
            matches!(form_code, "1701" | "1701Q")
        }
        TaxClassification::SoleProprietorNonVat | TaxClassification::ProfessionalOrFreelancer => {
            // Percentage tax + income tax (no VAT)
            matches!(form_code, "2551Q" | "1701Q" | "1701")
        }
        TaxClassification::SoleProprietorVat => {
            // VAT + income tax (no percentage tax)
            matches!(form_code, "2550M" | "1701Q" | "1701")
        }
        TaxClassification::MixedIncome => {
            // All individual forms
            matches!(form_code, "2551Q" | "2550M" | "1701Q" | "1701")
        }
        TaxClassification::Corporation => {
            // Corporate forms + percentage tax or VAT
            matches!(form_code, "1702Q" | "1702RT" | "2551Q" | "2550M")
        }
    }
}

/// Returns the list of applicable form codes for a profile, considering
/// both `TaxpayerType` and `TaxClassification`.
pub fn applicable_forms_for_profile(profile: &TaxpayerProfile) -> Vec<&'static str> {
    let base_forms = forms_for_taxpayer(&profile.taxpayer_type);

    match &profile.tax_classification {
        Some(classification) => base_forms
            .into_iter()
            .filter(|f| is_form_applicable_for_classification(f.code, classification))
            .map(|f| f.code)
            .collect(),
        // No classification set — return all forms for the entity type
        None => base_forms.into_iter().map(|f| f.code).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::models::{IncomeCategory, IncomeSource};
    use chrono::NaiveDate;

    fn valid_payload() -> UniversalTaxPayload {
        UniversalTaxPayload {
            tin: "010558054000".to_string(),
            target_form: Some("2551Q".to_string()),
            period_start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            period_end: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
            is_amended: false,
            income_sources: vec![IncomeSource {
                category: IncomeCategory::BusinessNonVat,
                gross_amount: 100_000.0,
                is_vat_exempt: true,
                atc_code_override: None,
                tax_rate_override: None,
            }],
            creditable_withholdings: 0.0,
            previous_tax_paid: 0.0,
            metadata: Default::default(),
        }
    }

    #[test]
    fn test_valid_payload_passes() {
        let errors = validate_payload(&valid_payload());
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
    }

    #[test]
    fn test_bad_tin_length() {
        let mut payload = valid_payload();
        payload.tin = "12345".to_string();
        let errors = validate_payload(&payload);
        assert!(errors.iter().any(|e| e.field == "tin"));
    }

    #[test]
    fn test_bad_tin_chars() {
        let mut payload = valid_payload();
        payload.tin = "01055805400A".to_string();
        let errors = validate_payload(&payload);
        assert!(
            errors
                .iter()
                .any(|e| e.field == "tin" && e.message.contains("digits"))
        );
    }

    #[test]
    fn test_period_end_before_start() {
        let mut payload = valid_payload();
        payload.period_start = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        payload.period_end = NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
        let errors = validate_payload(&payload);
        assert!(errors.iter().any(|e| e.field == "period_end"));
    }

    #[test]
    fn test_previous_tax_on_non_amended() {
        let mut payload = valid_payload();
        payload.is_amended = false;
        payload.previous_tax_paid = 5_000.0;
        let errors = validate_payload(&payload);
        assert!(errors.iter().any(|e| e.field == "previous_tax_paid"));
    }

    #[test]
    fn test_negative_gross_amount() {
        let mut payload = valid_payload();
        payload.income_sources[0].gross_amount = -100.0;
        let errors = validate_payload(&payload);
        assert!(errors.iter().any(|e| e.field.contains("gross_amount")));
    }

    #[test]
    fn test_invalid_rate_override() {
        let mut payload = valid_payload();
        payload.income_sources[0].tax_rate_override = Some(1.5); // 150%
        let errors = validate_payload(&payload);
        assert!(errors.iter().any(|e| e.field.contains("tax_rate_override")));
    }

    #[test]
    fn test_purely_compensation_cannot_file_2551q() {
        use crate::naming::Tin;
        use crate::profile::TaxpayerType;

        let profile = TaxpayerProfile {
            id: Some(1),
            full_name: "Test".into(),
            tin: Tin {
                segment1: "010".into(),
                segment2: "558".into(),
                segment3: "054".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: "Employment".into(),
            registered_address: "QC".into(),
            zip_code: "1100".into(),
            phone: "09156837000".into(),
            email: "test@example.com".into(),
            default_form_type: "1701".into(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            tax_classification: Some(TaxClassification::PurelyCompensation),
            opted_for_8_percent_flat_rate: false,
            has_employees: false,
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            _imap_enabled_compat: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
        };

        // PurelyCompensation should NOT be allowed to file 2551Q
        let err = validate_form_applicability("2551Q", &profile);
        assert!(err.is_some());

        // But should be allowed 1701Q
        let ok = validate_form_applicability("1701Q", &profile);
        assert!(ok.is_none());
    }

    #[test]
    fn test_applicable_forms_sole_proprietor_non_vat() {
        use crate::naming::Tin;
        use crate::profile::TaxpayerType;

        let profile = TaxpayerProfile {
            id: Some(1),
            full_name: "Test".into(),
            tin: Tin {
                segment1: "010".into(),
                segment2: "558".into(),
                segment3: "054".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: "Consulting".into(),
            registered_address: "QC".into(),
            zip_code: "1100".into(),
            phone: "09156837000".into(),
            email: "test@example.com".into(),
            default_form_type: "2551Q".into(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            tax_classification: Some(TaxClassification::SoleProprietorNonVat),
            opted_for_8_percent_flat_rate: false,
            has_employees: false,
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            _imap_enabled_compat: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
        };

        let forms = applicable_forms_for_profile(&profile);
        assert!(forms.contains(&"2551Q"));
        assert!(forms.contains(&"1701Q"));
        assert!(forms.contains(&"1701"));
        // Should NOT have VAT form
        assert!(!forms.contains(&"2550M"));
    }
}
