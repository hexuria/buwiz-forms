//! Integration payload validation — pre-flight checks before mapping.
//!
//! Validates the `UniversalTaxPayload` structure, period consistency,
//! and profile compatibility before the mapper engine runs.
//!
//! **Form eligibility is evaluated exclusively through the temporal engine.**
//! There is no separate hardcoded eligibility matrix in this module.

use crate::integration::models::UniversalTaxPayload;
use crate::profile::TaxpayerProfile;
use crate::temporal::snapshot_loader::compiled_snapshot;
use chrono::Datelike;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSuggestionDecision {
    pub form_code: String,
    pub is_suggested: bool,
    pub reason: String,
    pub legal_authority_citation: String,
}

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

/// Validate that a target form is applicable for the given profile and year.
///
/// Delegates to the temporal engine so there is a single source of truth
/// for form eligibility. No separate hardcoded matrix.
pub fn validate_form_applicability(
    form_code: &str,
    profile: &TaxpayerProfile,
    taxable_year: u16,
) -> Option<PayloadValidationError> {
    // Check if the form exists in the compiled temporal snapshot.
    if !compiled_snapshot().has_form_code(form_code) {
        return Some(PayloadValidationError::new(
            "target_form",
            format!("Unknown form code: {form_code}"),
        ));
    }

    // Delegate to the temporal engine for the explicit year
    let engine = crate::temporal::TemporalEngine::default();
    let visible = engine.visible_form_codes(profile, taxable_year);

    if !visible.iter().any(|c| c == form_code) {
        return Some(PayloadValidationError::new(
            "target_form",
            format!(
                "Form {} is not applicable for this taxpayer profile in year {}",
                form_code, taxable_year
            ),
        ));
    }

    None
}

/// Evaluates a taxpayer profile against the form registry using the temporal engine.
/// Returns detailed decisions with reasons and legal authorities for all applicable forms.
///
/// This is the single canonical form suggestion path. It delegates to the temporal
/// engine rather than maintaining a separate hardcoded eligibility matrix.
#[allow(clippy::collapsible_if)]
pub fn evaluate_forms(profile: &TaxpayerProfile) -> Vec<FormSuggestionDecision> {
    let current_year = chrono::Local::now().year() as u16;
    evaluate_forms_for_year(profile, current_year)
}

/// Year-explicit variant of evaluate_forms for historical/amended filing.
#[allow(clippy::collapsible_if)]
pub fn evaluate_forms_for_year(
    profile: &TaxpayerProfile,
    taxable_year: u16,
) -> Vec<FormSuggestionDecision> {
    let engine = crate::temporal::TemporalEngine::default();
    let context = crate::temporal::TemporalContext::current_compliance(taxable_year);
    let decisions = engine.evaluate_with_context(profile, &context);

    decisions
        .into_iter()
        .map(|d| {
            let legal_authority_citation = if d.legal_citations.is_empty() {
                "Standard BIR Filing Rules".to_string()
            } else {
                d.legal_citations
                    .iter()
                    .map(|c| format!("{} {}", c.number, c.section))
                    .collect::<Vec<_>>()
                    .join("; ")
            };

            let reason = if let Some(r) = d.eligibility.reason() {
                r.to_string()
            } else if d.eligibility.is_visible() {
                "Applicable based on taxpayer classification".to_string()
            } else {
                "Not applicable".to_string()
            };

            FormSuggestionDecision {
                form_code: d.form_code,
                is_suggested: d.eligibility.is_visible(),
                reason,
                legal_authority_citation,
            }
        })
        .collect()
}

/// Returns the list of applicable form codes for a profile, considering
/// both `TaxpayerType` and `TaxClassification`.
///
/// Delegates to the temporal engine for the current year.
pub fn applicable_forms_for_profile(profile: &TaxpayerProfile) -> Vec<String> {
    let current_year = chrono::Local::now().year() as u16;
    applicable_forms_for_profile_and_year(profile, current_year)
}

/// Returns the list of applicable form codes for a profile in a specific year.
///
/// Uses the temporal engine for era-aware evaluation. This is the preferred API
/// for the dashboard where the user selects a tax year.
pub fn applicable_forms_for_profile_and_year(profile: &TaxpayerProfile, year: u16) -> Vec<String> {
    let engine = crate::temporal::TemporalEngine::default();
    engine.visible_form_codes(profile, year)
}

/// Returns all form codes known by the compiled temporal snapshot.
pub fn all_form_codes() -> Vec<String> {
    compiled_snapshot().form_codes()
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
            tax_classification: Some(crate::profile::TaxClassification::PurelyCompensation),
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            _opted_for_8_percent_flat_rate_compat: None,
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
            _imap_enabled_compat: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
        };

        // PurelyCompensation should NOT be allowed to file 2551Q
        let current_year = chrono::Local::now().year() as u16;
        let err = validate_form_applicability("2551Q", &profile, current_year);
        assert!(err.is_some());

        // But should be allowed 1700 (annual ITR for compensation)
        let ok = validate_form_applicability("1700", &profile, current_year);
        assert!(ok.is_none(), "1700 should be valid for PurelyCompensation");
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
            tax_classification: Some(crate::profile::TaxClassification::SelfEmployed),
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            _opted_for_8_percent_flat_rate_compat: None,
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
            _imap_enabled_compat: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
        };

        let forms = applicable_forms_for_profile(&profile);
        assert!(forms.iter().any(|code| code == "2551Q"));
        assert!(forms.iter().any(|code| code == "1701Q"));
        assert!(forms.iter().any(|code| code == "1701"));
        // Should NOT have VAT form
        assert!(!forms.iter().any(|code| code == "2550M"));
    }
}
