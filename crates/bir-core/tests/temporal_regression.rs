//! End-to-End Temporal Engine Regression Matrix
//!
//! Validates form visibility across all three regulatory eras
//! and multiple taxpayer profiles. This is the definitive regression
//! test that locks down temporal correctness.

use bir_core::naming::Tin;
use bir_core::profile::{TaxClassification, TaxpayerProfile, TaxpayerType};
use bir_core::temporal::ComplianceState;
use bir_core::temporal::context::TemporalContext;
use bir_core::temporal::engine::TemporalEngine;

fn make_profile(
    tp_type: TaxpayerType,
    classification: Option<TaxClassification>,
    is_vat: bool,
    is_dormant: bool,
) -> TaxpayerProfile {
    TaxpayerProfile {
        id: Some(1),
        full_name: "E2E Test".into(),
        tin: Tin {
            segment1: "010".into(),
            segment2: "558".into(),
            segment3: "054".into(),
            branch: "000".into(),
        },
        rdo_code: "039".into(),
        line_of_business: "E2E".into(),
        registered_address: "QC".into(),
        zip_code: "1100".into(),
        phone: "09156837000".into(),
        email: "e2e@test.com".into(),
        default_form_type: "2551Q".into(),
        taxpayer_type: tp_type,
        is_vat_registered: is_vat,
        business_start_date: None,
        tax_classification: classification,
        eopt_tier: None,
        is_bmbe: false,
        is_gpp_partner: false,
        is_create_msme: false,
        is_expanded_withholding_agent: false,
        atc_codes: vec![],
        excise_tax_categories: vec![],
        tax_elections: vec![],
        has_employees: false,
        is_dormant,
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
    }
}

fn visible_codes(profile: &TaxpayerProfile, year: u16) -> Vec<String> {
    let engine = TemporalEngine::default();
    let ctx = TemporalContext::current_compliance(year);
    engine.visible_form_codes_for_context(profile, &ctx)
}

fn has(codes: &[String], code: &str) -> bool {
    codes.iter().any(|c| c == code)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ERA: PRE-TRAIN (≤ 2017)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn pre_train_nonvat_individual_sees_2551m() {
    let p = make_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
        false,
        false,
    );
    let codes = visible_codes(&p, 2017);
    assert!(
        has(&codes, "2551M"),
        "2551M should be visible in 2017. Got: {:?}",
        codes
    );
    // 2551Q was introduced by TRAIN Law in 2018 — it does NOT exist in 2017
    assert!(
        !has(&codes, "2551Q"),
        "2551Q should NOT be visible in pre-TRAIN 2017. Got: {:?}",
        codes
    );
}

#[test]
fn pre_train_vat_individual_sees_vat_forms() {
    let p = make_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
        true,
        false,
    );
    let codes = visible_codes(&p, 2017);
    assert!(
        has(&codes, "2550M"),
        "2550M should be visible for VAT in 2017. Got: {:?}",
        codes
    );
    assert!(
        has(&codes, "2550Q"),
        "2550Q should be visible for VAT in 2017. Got: {:?}",
        codes
    );
    // Non-VAT forms hidden
    assert!(
        !has(&codes, "2551Q"),
        "2551Q should be hidden for VAT. Got: {:?}",
        codes
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ERA: TRAIN (2018–2023)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn train_era_hides_2551m() {
    let p = make_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
        false,
        false,
    );
    let codes = visible_codes(&p, 2020);
    assert!(
        !has(&codes, "2551M"),
        "2551M should be hidden in TRAIN era. Got: {:?}",
        codes
    );
    assert!(
        has(&codes, "2551Q"),
        "2551Q should remain visible. Got: {:?}",
        codes
    );
}

#[test]
fn train_era_corporation_sees_corporate_forms() {
    let p = make_profile(
        TaxpayerType::Corporation,
        Some(TaxClassification::Corporation),
        false,
        false,
    );
    let codes = visible_codes(&p, 2020);
    assert!(
        has(&codes, "1702Q"),
        "1702Q should be visible for corps. Got: {:?}",
        codes
    );
    // Individual-only forms should not appear
    assert!(
        !has(&codes, "1701Q"),
        "1701Q should not appear for corporations. Got: {:?}",
        codes
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ERA: EOPT (2024+)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn eopt_era_nonvat_sole_prop_2024() {
    let p = make_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
        false,
        false,
    );
    let codes = visible_codes(&p, 2024);
    assert!(
        has(&codes, "2551Q"),
        "2551Q should be visible. Got: {:?}",
        codes
    );
    assert!(
        !has(&codes, "2551M"),
        "2551M should still be hidden post-TRAIN. Got: {:?}",
        codes
    );
}

#[test]
fn eopt_era_compensation_earner() {
    let p = make_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::PurelyCompensation),
        false,
        false,
    );
    let codes = visible_codes(&p, 2024);
    assert!(
        has(&codes, "1700"),
        "1700 (payment) should be visible. Got: {:?}",
        codes
    );
    // Compensation earners should not have business forms
    assert!(
        !has(&codes, "2551Q"),
        "2551Q should not appear for purely compensation. Got: {:?}",
        codes
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CROSS-ERA DETERMINISM
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn deterministic_same_input_same_output() {
    let p = make_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
        false,
        false,
    );
    let run1 = visible_codes(&p, 2024);
    let run2 = visible_codes(&p, 2024);
    assert_eq!(
        run1, run2,
        "Deterministic: same input must produce same output"
    );
}

#[test]
fn cross_era_2551m_transition() {
    let p = make_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
        false,
        false,
    );

    // 2551M should be visible in 2017, hidden from 2018+
    assert!(
        has(&visible_codes(&p, 2017), "2551M"),
        "2551M visible in 2017"
    );
    assert!(
        !has(&visible_codes(&p, 2018), "2551M"),
        "2551M hidden in 2018"
    );
    assert!(
        !has(&visible_codes(&p, 2020), "2551M"),
        "2551M hidden in 2020"
    );
    assert!(
        !has(&visible_codes(&p, 2024), "2551M"),
        "2551M hidden in 2024"
    );
}

#[test]
fn train_era_keeps_2551m_as_deprecated_decision() {
    let p = make_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
        false,
        false,
    );
    let engine = TemporalEngine::default();
    let ctx = TemporalContext::current_compliance(2020);

    let decisions = engine.evaluate_with_context(&p, &ctx);
    let decision = decisions
        .iter()
        .find(|d| d.form_code == "2551M")
        .expect("2551M should remain in decisions for historical explanation");

    assert!(
        matches!(decision.eligibility, ComplianceState::Deprecated(_)),
        "2551M should be deprecated in 2020, got {:?}",
        decision.eligibility
    );
    assert!(
        !decision.eligibility.is_visible(),
        "Deprecated 2551M should stay hidden from primary suggestions"
    );
    assert!(
        decision
            .audit_log
            .iter()
            .any(|entry| entry.rule_name == "Timeline Window"),
        "Deprecated decision should include a timeline explanation"
    );
}

#[test]
fn validation_uses_explicit_year_for_historical_forms() {
    let p = make_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
        false,
        false,
    );

    assert!(
        bir_core::integration::validate_form_applicability("2551M", &p, 2017).is_none(),
        "2551M should validate for a 2017 filing context"
    );
    assert!(
        bir_core::integration::validate_form_applicability("2551M", &p, 2020).is_some(),
        "2551M should not validate for a 2020 filing context"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// DORMANT & EDGE CASES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn dormant_taxpayer_still_sees_forms() {
    let p = make_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
        false,
        true, // dormant
    );
    let codes = visible_codes(&p, 2024);
    // Dormant taxpayers still file NIL returns
    assert!(
        !codes.is_empty(),
        "Dormant taxpayer should still see forms (NIL). Got: {:?}",
        codes
    );
}

#[test]
fn future_year_uses_latest_era() {
    let p = make_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
        false,
        false,
    );
    let codes_2030 = visible_codes(&p, 2030);
    let codes_2024 = visible_codes(&p, 2024);
    // EOPT has no end date, so 2030 should behave like 2024
    assert_eq!(
        codes_2030, codes_2024,
        "Future year should match current EOPT era"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SNAPSHOT INTEGRITY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_has_expected_artifact_count() {
    let snapshot = bir_core::temporal::snapshot_loader::compiled_snapshot();
    assert!(
        snapshot.form_artifacts.len() >= 30,
        "Expected at least 30 form artifacts, got {}",
        snapshot.form_artifacts.len()
    );
}

#[test]
fn snapshot_has_three_eras() {
    let snapshot = bir_core::temporal::snapshot_loader::compiled_snapshot();
    let primary_eras: Vec<_> = snapshot.eras.iter().filter(|e| !e.is_overlay).collect();
    assert_eq!(
        primary_eras.len(),
        3,
        "Expected 3 primary eras (PRE_TRAIN, TRAIN, EOPT), got {}",
        primary_eras.len()
    );
}

#[test]
fn snapshot_eras_have_no_gaps() {
    let snapshot = bir_core::temporal::snapshot_loader::compiled_snapshot();
    // Every year from 1997 to 2030 should resolve to an era
    for year in 1997..=2030 {
        let era = snapshot.find_primary_era(year);
        assert!(era.is_some(), "No primary era resolves for year {}", year);
    }
}

#[test]
fn snapshot_rate_tables_resolve() {
    use bir_core::temporal::engine::resolve_rates;
    let ctx = TemporalContext::current_compliance(2024);

    let pct = resolve_rates(&ctx, "PercentageTax");
    assert!(!pct.is_empty(), "Should find PercentageTax rates for 2024");

    let cit = resolve_rates(&ctx, "CIT");
    assert!(!cit.is_empty(), "Should find CIT rates for 2024");
}
