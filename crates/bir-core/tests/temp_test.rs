use bir_core::integration::recurring_obligation_forms_for_profile_and_year;
use bir_core::profile::{TaxClassification, TaxpayerProfile};
use chrono::NaiveDate;

fn profile_for_dashboard(
    classification: TaxClassification,
    is_vat_registered: bool,
) -> TaxpayerProfile {
    use bir_core::naming::Tin;
    use bir_core::profile::TaxpayerType;

    let mut profile = TaxpayerProfile {
        id: Some(1),
        full_name: "Dashboard Test".into(),
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
        is_vat_registered,
        business_start_date: NaiveDate::from_ymd_opt(2020, 1, 1),
        birth_date: None,
        tax_classification: Some(classification),
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
        per_year_forms: Default::default(),
    };
    profile.ensure_profile_version_ledger();
    profile
}

#[test]
fn compensation_profile_resolves_only_the_annual_return() {
    let profile = profile_for_dashboard(TaxClassification::PurelyCompensation, false);
    let forms = recurring_obligation_forms_for_profile_and_year(&profile, 2026);

    assert_eq!(forms, vec!["1700".to_string()]);
}
