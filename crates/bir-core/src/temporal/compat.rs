//! Backward compatibility adapter.
//!
//! Wraps the new `TemporalEngine` behind the old `applicable_forms_for_profile()` API
//! so existing dashboard code works unchanged.

use crate::profile::TaxpayerProfile;
use crate::temporal::context::TemporalContext;
use crate::temporal::eligibility::FormDecision;
use crate::temporal::engine::TemporalEngine;

/// Primary temporal API: evaluate forms for a specific year.
///
/// Use this instead of the old `applicable_forms_temporal` (now removed).
pub fn applicable_forms_for_year(profile: &TaxpayerProfile, year: u16) -> Vec<String> {
    let engine = TemporalEngine::default();
    let context = TemporalContext::current_compliance(year);
    engine.visible_form_codes_for_context(profile, &context)
}

/// Full temporal evaluation: returns all decisions with audit logs.
///
/// Use this for the dashboard where you need eligibility state, badges, and explanations.
pub fn evaluate_forms_temporal(profile: &TaxpayerProfile, year: u16) -> Vec<FormDecision> {
    let engine = TemporalEngine::default();
    let context = TemporalContext::current_compliance(year);
    engine.evaluate_with_context(profile, &context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;
    use crate::profile::{TaxClassification, TaxpayerType};

    fn test_profile() -> TaxpayerProfile {
        TaxpayerProfile {
            id: Some(1),
            full_name: "Test".into(),
            tin: Tin {
                segment1: "010".into(),
                segment2: "558".into(),
                segment3: "054".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: "Test".into(),
            registered_address: "QC".into(),
            zip_code: "1100".into(),
            phone: "09156837000".into(),
            email: "test@example.com".into(),
            default_form_type: "2551Q".into(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            tax_classification: Some(TaxClassification::SelfEmployed),
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
    fn test_year_api_returns_forms() {
        let profile = test_profile();
        let forms = applicable_forms_for_year(&profile, 2024);
        assert!(!forms.is_empty());
        assert!(forms.contains(&"2551Q".to_string()));
    }

    #[test]
    fn test_evaluate_returns_decisions() {
        let profile = test_profile();
        let decisions = evaluate_forms_temporal(&profile, 2024);
        assert!(!decisions.is_empty());
    }

    #[test]
    fn test_historical_year_2017() {
        let profile = test_profile();
        let forms_2017 = applicable_forms_for_year(&profile, 2017);
        let forms_2020 = applicable_forms_for_year(&profile, 2020);

        // 2551M visible in 2017, hidden in 2020
        assert!(forms_2017.contains(&"2551M".to_string()));
        assert!(!forms_2020.contains(&"2551M".to_string()));
    }
}
