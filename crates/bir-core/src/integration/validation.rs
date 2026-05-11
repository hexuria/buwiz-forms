//! Integration payload validation — pre-flight checks before mapping.
//!
//! Validates the `UniversalTaxPayload` structure, period consistency,
//! and profile compatibility before the mapper engine runs.

use crate::forms::registry::{find_form, forms_for_taxpayer};
use crate::integration::models::UniversalTaxPayload;
use crate::profile::{EoptTier, TaxClassification, TaxpayerProfile};
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
        let applicable = is_form_applicable_for_classification(form_def, classification);
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
/// This evaluates rules primarily for Income Tax, Percentage Tax, and Value-Added Tax.
/// Other categories (like Payment Forms, Withholding Tax, Excise Tax, etc.)
/// are passed through (`true`) to be evaluated by their respective specific rules later.
fn is_form_applicable_for_classification(
    form: &crate::forms::registry::FormDefinition,
    classification: &TaxClassification,
) -> bool {
    let form_code = form.code;
    let category = form.category;

    // Only filter the specific business/income tax categories based on Tax Classification.
    // Allow other categories (like Withholding Tax, Excise Tax, Documentary Stamp Tax, Payment Form)
    // to pass through and be evaluated by their own specific rules.
    if category != "Income Tax" && category != "Value-Added Tax" && category != "Percentage Tax" {
        return true;
    }

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
            matches!(form_code, "2550M" | "2550Q" | "1701Q" | "1701")
        }
        TaxClassification::MixedIncome => {
            // All individual forms
            matches!(form_code, "2551Q" | "2550M" | "2550Q" | "1701Q" | "1701")
        }
        TaxClassification::Corporation => {
            // Corporate forms + percentage tax or VAT
            matches!(
                form_code,
                "1702Q"
                    | "1702RT"
                    | "1702EX"
                    | "1702MX"
                    | "1704"
                    | "2551Q"
                    | "2551M"
                    | "2552"
                    | "2553"
                    | "2550M"
                    | "2550Q"
            )
        }
    }
}

/// Evaluates a taxpayer profile against the form registry using a rule engine.
/// Returns detailed decisions with reasons and legal authorities for all applicable forms.
#[allow(clippy::collapsible_if)]
pub fn evaluate_forms(profile: &TaxpayerProfile) -> Vec<FormSuggestionDecision> {
    let base_forms = forms_for_taxpayer(&profile.taxpayer_type);
    let current_year = chrono::Local::now().year() as u16; // Using current year for 8% election check
    let mut decisions = Vec::new();

    for form in base_forms {
        let mut is_suggested = true;
        let mut reason = "Applicable based on taxpayer classification".to_string();
        let mut legal_authority_citation = "Standard BIR Filing Rules".to_string();

        // 1. Check Deprecation
        if form.is_deprecated {
            is_suggested = false;
            reason = "Form has been abolished and is no longer accepted".to_string();
            legal_authority_citation = "TRAIN Law / EOPT 2023".to_string();
        }

        // 2. Check Entity Level Classification Rules
        if is_suggested {
            if let Some(classification) = &profile.tax_classification {
                if !is_form_applicable_for_classification(form, classification) {
                    is_suggested = false;
                    reason = format!("Not applicable for {:?}", classification);
                }
            }
        }

        // 3. Withholding Agent Rules
        if is_suggested && form.category == "Withholding Tax" {
            if form.requires_employees
                && !profile.has_employees
                && !profile.is_expanded_withholding_agent
            {
                is_suggested = false;
                reason = "Taxpayer is not registered as a withholding agent or has no employees"
                    .to_string();
            }
        }

        // 4. EOPT Tiers
        if is_suggested && form.code == "1701" {
            if matches!(
                profile.eopt_tier,
                Some(EoptTier::Micro) | Some(EoptTier::Small)
            ) {
                is_suggested = false;
                reason = "Micro/Small taxpayers should use the simplified Form 1701-MS".to_string();
                legal_authority_citation = "EOPT 2023 Simplified Filing".to_string();
            }
        }
        if is_suggested && form.code == "1701MS" {
            if !matches!(
                profile.eopt_tier,
                Some(EoptTier::Micro) | Some(EoptTier::Small)
            ) {
                is_suggested = false;
                reason = "Form 1701-MS is exclusively for Micro/Small taxpayers".to_string();
            }
        }

        // 5. Substituted Filing
        if is_suggested && form.code == "1700" {
            if matches!(
                profile.tax_classification,
                Some(TaxClassification::PurelyCompensation)
            ) && profile.has_single_employer
            {
                is_suggested = false;
                reason = "Eligible for Substituted Filing".to_string();
                legal_authority_citation = "Substituted Filing System".to_string();
            }
        }

        // 6. 8% Flat Rate Election
        if is_suggested && profile.has_8_percent_election(current_year) {
            if matches!(form.code, "2551Q" | "1701") {
                is_suggested = false;
                reason = "Taxpayer elected 8% flat rate".to_string();
                legal_authority_citation = "TRAIN Law RA 10963".to_string();
            }
        }

        // 7. Dormant Mode
        if profile.is_dormant && is_suggested {
            reason = "Dormant / No Operations - NIL Filing Required".to_string();
        }

        // 8. VAT Registration Check
        if is_suggested {
            if let Some(requires_vat) = form.requires_vat {
                if requires_vat && !profile.is_vat_registered {
                    is_suggested = false;
                    reason = "Taxpayer is not VAT registered".to_string();
                } else if !requires_vat && profile.is_vat_registered {
                    is_suggested = false;
                    reason =
                        "Taxpayer is VAT registered, thus exempt from Percentage Tax".to_string();
                }
            }
        }

        // 9. Excise Tax Check
        if is_suggested && form.category == "Excise Tax" {
            let required_excise_category = match form.code {
                "2200A" => Some(crate::profile::ExciseTaxCategory::Alcohol),
                "2200AN" => Some(crate::profile::ExciseTaxCategory::AutomobilesAndNonEssential),
                "2200M" => Some(crate::profile::ExciseTaxCategory::Mineral),
                "2200P" => Some(crate::profile::ExciseTaxCategory::Petroleum),
                "2200T" => Some(crate::profile::ExciseTaxCategory::Tobacco),
                _ => None, // If there's an unknown excise tax form, we'll keep it visible as a fallback
            };

            if let Some(req_cat) = required_excise_category {
                if !profile.excise_tax_categories.contains(&req_cat) {
                    is_suggested = false;
                    reason = format!("Taxpayer is not liable for {:?} Excise Tax", req_cat);
                }
            }
        }

        // Filter out forms that naturally don't apply unless they are explicitly not suggested with a reason
        // Wait, if it wasn't suggested by `is_form_applicable_for_classification`, we still return the decision
        // so the UI can show "Why is this hidden?" if it wants. But we only care about `is_suggested` being true
        // for the active forms list.

        decisions.push(FormSuggestionDecision {
            form_code: form.code.to_string(),
            is_suggested,
            reason,
            legal_authority_citation,
        });
    }

    decisions
}

/// Returns the list of applicable form codes for a profile, considering
/// both `TaxpayerType` and `TaxClassification`.
pub fn applicable_forms_for_profile(profile: &TaxpayerProfile) -> Vec<&'static str> {
    // Keep it returning static strings for backward compatibility
    let decisions = evaluate_forms(profile);
    decisions
        .into_iter()
        .filter(|d| d.is_suggested)
        .filter_map(|d| crate::forms::registry::find_form(&d.form_code).map(|f| f.code))
        .collect()
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
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            tax_elections: vec![],
            _opted_for_8_percent_flat_rate_compat: None,
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
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
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            tax_elections: vec![],
            _opted_for_8_percent_flat_rate_compat: None,
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
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
