//! EligibilityFacts Unit Tests — validates the derived facts bridge.
//!
//! These tests ensure EligibilityFacts is correctly derived from
//! TaxpayerProfile across all entity types and classification combinations.

use bir_core::naming::Tin;
use bir_core::profile::{
    ExciseTaxCategory, IncomeTaxElection, TaxClassification, TaxElectionHistory, TaxpayerProfile,
    TaxpayerType,
};
use bir_core::temporal::eligibility_facts::{
    CooperativeTaxTreatment, EligibilityFacts, IndividualIncomeKind,
};
use chrono::Datelike;

fn test_tin() -> Tin {
    Tin {
        segment1: "010".into(),
        segment2: "558".into(),
        segment3: "054".into(),
        branch: "000".into(),
    }
}

fn base_profile(tp: TaxpayerType, tc: Option<TaxClassification>) -> TaxpayerProfile {
    TaxpayerProfile {
        id: Some(1),
        full_name: "Facts Test".into(),
        tin: test_tin(),
        rdo_code: "039".into(),
        line_of_business: "Test".into(),
        registered_address: "QC".into(),
        zip_code: "1100".into(),
        phone: "09156837000".into(),
        email: "test@example.com".into(),
        default_form_type: "2551Q".into(),
        taxpayer_type: tp,
        is_vat_registered: false,
        business_start_date: None,
        tax_classification: tc,
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
        profile_versions: vec![],
        compliance_source_mode: Default::default(),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// INDIVIDUAL FACTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn facts_individual_purely_compensation() {
    let p = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::PurelyCompensation),
    );
    let f = EligibilityFacts::from_profile(&p);
    assert_eq!(
        f.individual_income_kind,
        Some(IndividualIncomeKind::CompensationOnly)
    );
    assert!(!f.has_business_activity);
    assert!(f.cooperative_tax_treatment.is_none());
    assert_eq!(
        f.effective_classification,
        Some(TaxClassification::PurelyCompensation)
    );
}

#[test]
fn facts_individual_self_employed() {
    let p = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
    );
    let f = EligibilityFacts::from_profile(&p);
    assert_eq!(
        f.individual_income_kind,
        Some(IndividualIncomeKind::BusinessOrProfessionOnly)
    );
    assert!(f.has_business_activity);
}

#[test]
fn facts_individual_mixed_income() {
    let p = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::MixedIncome),
    );
    let f = EligibilityFacts::from_profile(&p);
    assert_eq!(
        f.individual_income_kind,
        Some(IndividualIncomeKind::MixedIncome)
    );
    assert!(f.has_business_activity);
}

#[test]
fn facts_individual_no_classification() {
    let p = base_profile(TaxpayerType::Individual, None);
    let f = EligibilityFacts::from_profile(&p);
    assert!(f.individual_income_kind.is_none());
    assert!(!f.has_business_activity);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CORPORATION FACTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn facts_corporation() {
    let p = base_profile(TaxpayerType::Corporation, None);
    let f = EligibilityFacts::from_profile(&p);
    assert!(f.individual_income_kind.is_none());
    assert!(f.cooperative_tax_treatment.is_none());
    assert!(f.has_business_activity);
    assert_eq!(
        f.effective_classification,
        Some(TaxClassification::Corporation)
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// COOPERATIVE FACTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn facts_cooperative_exempt() {
    let p = base_profile(
        TaxpayerType::Cooperative,
        Some(TaxClassification::CooperativeExempt),
    );
    let f = EligibilityFacts::from_profile(&p);
    assert_eq!(
        f.cooperative_tax_treatment,
        Some(CooperativeTaxTreatment::Exempt)
    );
    assert!(f.has_business_activity);
}

#[test]
fn facts_cooperative_taxable() {
    let p = base_profile(
        TaxpayerType::Cooperative,
        Some(TaxClassification::CooperativeTaxable),
    );
    let f = EligibilityFacts::from_profile(&p);
    assert_eq!(
        f.cooperative_tax_treatment,
        Some(CooperativeTaxTreatment::Taxable)
    );
}

#[test]
fn facts_cooperative_mixed() {
    let p = base_profile(
        TaxpayerType::Cooperative,
        Some(TaxClassification::CooperativeMixed),
    );
    let f = EligibilityFacts::from_profile(&p);
    assert_eq!(
        f.cooperative_tax_treatment,
        Some(CooperativeTaxTreatment::Mixed)
    );
}

#[test]
fn facts_cooperative_defaults_to_taxable() {
    let p = base_profile(TaxpayerType::Cooperative, None);
    let f = EligibilityFacts::from_profile(&p);
    assert_eq!(
        f.cooperative_tax_treatment,
        Some(CooperativeTaxTreatment::Taxable)
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ESTATE/TRUST FACTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn facts_estate() {
    let p = base_profile(TaxpayerType::Estate, None);
    let f = EligibilityFacts::from_profile(&p);
    assert!(!f.has_business_activity);
    assert_eq!(
        f.effective_classification,
        Some(TaxClassification::EstateOrTrust)
    );
}

#[test]
fn facts_trust() {
    let p = base_profile(TaxpayerType::Trust, None);
    let f = EligibilityFacts::from_profile(&p);
    assert!(!f.has_business_activity);
    assert_eq!(
        f.effective_classification,
        Some(TaxClassification::EstateOrTrust)
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ELECTION FACTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn facts_8_percent_election_year_specific() {
    let mut p = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
    );
    p.tax_elections.push(TaxElectionHistory {
        taxable_year: 2024,
        election: IncomeTaxElection::EightPercent,
        elected_at: chrono::NaiveDateTime::default(),
        source_form: "test".to_string(),
    });
    let f = EligibilityFacts::from_profile(&p);
    assert!(f.has_8_percent_election(2024));
    assert!(!f.has_8_percent_election(2023));
    assert!(!f.has_8_percent_election(2025));
}

#[test]
fn facts_no_elections() {
    let p = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
    );
    let f = EligibilityFacts::from_profile(&p);
    assert!(!f.has_8_percent_election(2024));
    assert!(f.tax_elections.is_empty());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// WITHHOLDING & EXCISE FACTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn facts_withholding_agent() {
    let mut p = base_profile(TaxpayerType::Corporation, None);
    p.has_employees = true;
    p.is_expanded_withholding_agent = true;
    let f = EligibilityFacts::from_profile(&p);
    assert!(f.has_employees);
    assert!(f.is_expanded_withholding_agent);
}

#[test]
fn facts_excise_categories() {
    let mut p = base_profile(TaxpayerType::Corporation, None);
    p.excise_tax_categories = vec![ExciseTaxCategory::Tobacco, ExciseTaxCategory::Alcohol];
    let f = EligibilityFacts::from_profile(&p);
    assert_eq!(f.excise_tax_categories.len(), 2);
    assert!(
        f.excise_tax_categories
            .contains(&ExciseTaxCategory::Tobacco)
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// DORMANCY & GPP FACTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn facts_dormant_taxpayer() {
    let mut p = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
    );
    p.is_dormant = true;
    let f = EligibilityFacts::from_profile(&p);
    assert!(f.is_dormant);
}

#[test]
fn facts_gpp_partner() {
    let mut p = base_profile(TaxpayerType::Individual, None);
    p.is_gpp_partner = true;
    let f = EligibilityFacts::from_profile(&p);
    assert!(f.is_gpp_partner);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// VALIDATION → ENGINE DELEGATION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn validation_delegates_to_temporal_engine() {
    // This test verifies that validation.rs no longer has its own
    // hardcoded matrix, but delegates to the temporal engine.
    use bir_core::integration::validate_form_applicability;

    let compensation_profile = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::PurelyCompensation),
    );

    // PurelyCompensation should NOT be able to file 2551Q
    let current_year = chrono::Local::now().date_naive().year() as u16;
    let err = validate_form_applicability("2551Q", &compensation_profile, current_year);
    assert!(err.is_some(), "2551Q should fail for PurelyCompensation");

    // But should be able to file 1700
    let ok = validate_form_applicability("1700", &compensation_profile, current_year);
    assert!(ok.is_none(), "1700 should pass for PurelyCompensation");
}

#[test]
fn validation_unknown_form_rejected() {
    use bir_core::integration::validate_form_applicability;
    let p = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
    );
    let current_year = chrono::Local::now().date_naive().year() as u16;
    let err = validate_form_applicability("XXXX", &p, current_year);
    assert!(err.is_some());
    assert!(err.unwrap().message.contains("Unknown form code"));
}

#[test]
fn applicable_forms_corporation_has_corporate_itr() {
    use bir_core::integration::applicable_forms_for_profile;

    let p = base_profile(TaxpayerType::Corporation, None);
    let forms = applicable_forms_for_profile(&p);
    // Corporations should see 1702Q and NOT 1701Q
    assert!(
        forms.iter().any(|code| code == "1702Q"),
        "Corp should have 1702Q"
    );
    assert!(
        !forms.iter().any(|code| code == "1701Q"),
        "Corp should NOT have 1701Q"
    );
}

#[test]
fn applicable_forms_self_employed_vat_registered() {
    use bir_core::integration::applicable_forms_for_profile;

    let mut p = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
    );
    p.is_vat_registered = true;
    let forms = applicable_forms_for_profile(&p);
    // VAT registered should see VAT forms
    assert!(
        forms.iter().any(|code| code == "2550Q"),
        "VAT should see 2550Q"
    );
    // Non-VAT percentage tax should be hidden
    assert!(
        !forms.iter().any(|code| code == "2551Q"),
        "VAT should NOT see 2551Q"
    );
}

#[test]
fn applicable_forms_self_employed_non_vat() {
    use bir_core::integration::applicable_forms_for_profile;

    let p = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
    );
    let forms = applicable_forms_for_profile(&p);
    // Non-VAT should see percentage tax
    assert!(
        forms.iter().any(|code| code == "2551Q"),
        "Non-VAT should see 2551Q"
    );
    // Non-VAT should NOT see VAT returns
    assert!(
        !forms.iter().any(|code| code == "2550Q"),
        "Non-VAT should NOT see 2550Q"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CROSS-ERA ENGINE MATRIX
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn cross_era_corporation_1702q_all_eras() {
    use bir_core::temporal::context::TemporalContext;
    use bir_core::temporal::engine::TemporalEngine;

    let p = base_profile(TaxpayerType::Corporation, None);
    let engine = TemporalEngine::default();

    // Pre-TRAIN (2017): Corporation should have 1702Q equivalent
    let ctx_2017 = TemporalContext::current_compliance(2017);
    let codes_2017 = engine.visible_form_codes_for_context(&p, &ctx_2017);
    assert!(
        codes_2017.iter().any(|c| c == "1702Q"),
        "Corp should have 1702Q in 2017. Got: {:?}",
        codes_2017
    );

    // TRAIN (2020): Still has 1702Q
    let ctx_2020 = TemporalContext::current_compliance(2020);
    let codes_2020 = engine.visible_form_codes_for_context(&p, &ctx_2020);
    assert!(
        codes_2020.iter().any(|c| c == "1702Q"),
        "Corp should have 1702Q in 2020. Got: {:?}",
        codes_2020
    );

    // EOPT (2024): Still has 1702Q
    let ctx_2024 = TemporalContext::current_compliance(2024);
    let codes_2024 = engine.visible_form_codes_for_context(&p, &ctx_2024);
    assert!(
        codes_2024.iter().any(|c| c == "1702Q"),
        "Corp should have 1702Q in 2024. Got: {:?}",
        codes_2024
    );
}

#[test]
fn cross_era_partnership_sees_partnership_forms() {
    use bir_core::temporal::engine::TemporalEngine;

    let p = base_profile(TaxpayerType::Partnership, None);
    let engine = TemporalEngine::default();
    let codes = engine.visible_form_codes(&p, 2024);

    // Partnership should see 1702Q (same as Corporation)
    assert!(
        codes.iter().any(|c| c == "1702Q"),
        "Partnership should have 1702Q. Got: {:?}",
        codes
    );
    // Should NOT see individual-only forms
    assert!(
        !codes.iter().any(|c| c == "1701Q"),
        "Partnership should NOT have 1701Q. Got: {:?}",
        codes
    );
}

#[test]
fn evaluate_forms_returns_decisions_for_self_employed() {
    use bir_core::integration::validation::evaluate_forms;

    let p = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
    );
    let decisions = evaluate_forms(&p);
    assert!(
        !decisions.is_empty(),
        "evaluate_forms should return decisions"
    );

    // At least one should be suggested
    assert!(
        decisions.iter().any(|d| d.is_suggested),
        "Should have at least one suggested form"
    );

    // Each decision should have a reason
    for d in &decisions {
        assert!(
            !d.reason.is_empty(),
            "Decision for {} missing reason",
            d.form_code
        );
    }
}
