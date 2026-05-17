use bir_core::calendar_rules::{DeadlineKind, DeadlinePeriod, DeadlineResolver};
use bir_core::integration::recurring_obligation_forms_for_profile_and_year;
use bir_core::integration::{deadline_applies_to_profile, profile_deadline_overrides_for_year};
use bir_core::naming::Tin;
use bir_core::profile::{
    EoptTier, ExciseTaxCategory, IncomeTaxElection, ProfileDeadlineOverride, RegisteredTaxType,
    RegistrationActivityStatus, TaxClassification, TaxElectionHistory, TaxProfileVersion,
    TaxProfileVersionStatus, TaxpayerProfile, TaxpayerType,
};
use bir_core::validation::validate_profile;
use chrono::NaiveDate;
use std::collections::BTreeSet;

const TAXABLE_YEAR: u16 = 2026;
const SELF_EMPLOYED_NON_VAT: &[&str] = &["1701", "1701Q", "2551Q", "1701A"];
const SELF_EMPLOYED_NON_VAT_MICRO_SMALL: &[&str] = &["1701MS", "1701", "1701Q", "2551Q", "1701A"];
const SELF_EMPLOYED_NON_VAT_8_PERCENT: &[&str] = &["1701A", "1701Q", "1701"];
const SELF_EMPLOYED_NON_VAT_MICRO_SMALL_8_PERCENT: &[&str] = &["1701MS", "1701A", "1701Q", "1701"];
const SELF_EMPLOYED_VAT: &[&str] = &["1701", "1701Q", "2550DS", "2550Q", "1701A", "2550M"];
const SELF_EMPLOYED_VAT_MICRO_SMALL: &[&str] = &[
    "1701MS", "1701", "1701Q", "2550DS", "2550Q", "1701A", "2550M",
];
const SELF_EMPLOYED_VAT_8_PERCENT: &[&str] =
    &["1701A", "1701Q", "2550DS", "2550Q", "1701", "2550M"];
const SELF_EMPLOYED_VAT_MICRO_SMALL_8_PERCENT: &[&str] = &[
    "1701MS", "1701A", "1701Q", "2550DS", "2550Q", "1701", "2550M",
];
const MIXED_NON_VAT: &[&str] = &["1701", "1701Q", "2551Q"];
const MIXED_NON_VAT_8_PERCENT: &[&str] = &["1701", "1701Q"];
const MIXED_VAT: &[&str] = &["1701", "1701Q", "2550DS", "2550Q", "2550M"];
const COMPENSATION_WITHHOLDING: &[&str] = &["0620", "1600", "1601C", "1604CF", "2316"];
const EXPANDED_WITHHOLDING: &[&str] = &["0619E", "1601EQ", "1604E", "1606", "1621"];
const FINAL_WITHHOLDING: &[&str] = &["0619F", "1600WP", "1601F", "1601FQ", "1602", "1603"];

fn base_profile(
    taxpayer_type: TaxpayerType,
    classification: Option<TaxClassification>,
) -> TaxpayerProfile {
    TaxpayerProfile {
        id: Some(1),
        full_name: "Matrix Test".into(),
        tin: Tin {
            segment1: "010".into(),
            segment2: "558".into(),
            segment3: "054".into(),
            branch: "000".into(),
        },
        rdo_code: "039".into(),
        line_of_business: "Matrix".into(),
        registered_address: "QC".into(),
        zip_code: "1100".into(),
        phone: "09156837000".into(),
        email: "matrix@example.com".into(),
        default_form_type: "2551Q".into(),
        taxpayer_type,
        is_vat_registered: false,
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
    }
}

fn forms_for(profile: &TaxpayerProfile) -> BTreeSet<String> {
    recurring_obligation_forms_for_profile_and_year(profile, TAXABLE_YEAR)
        .into_iter()
        .collect()
}

fn expected(codes: &[&str]) -> BTreeSet<String> {
    codes.iter().map(|code| code.to_string()).collect()
}

fn assert_forms(label: &str, profile: &TaxpayerProfile, codes: &[&str]) {
    assert_eq!(forms_for(profile), expected(codes), "{label}");
}

fn assert_forms_union(label: &str, profile: &TaxpayerProfile, groups: &[&[&str]]) {
    let mut codes = BTreeSet::new();
    for group in groups {
        codes.extend(expected(group));
    }

    assert_eq!(forms_for(profile), codes, "{label}");
}

fn add_eight_percent_election(profile: &mut TaxpayerProfile, year: u16) {
    profile.tax_elections.push(TaxElectionHistory {
        taxable_year: year,
        election: IncomeTaxElection::EightPercent,
        elected_at: chrono::NaiveDateTime::default(),
        source_form: "matrix".into(),
    });
}

fn self_employed_profile(
    vat: bool,
    tier: Option<EoptTier>,
    eight_percent: bool,
) -> TaxpayerProfile {
    let mut profile = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::SelfEmployed),
    );
    profile.is_vat_registered = vat;
    profile.eopt_tier = tier;

    if eight_percent {
        add_eight_percent_election(&mut profile, TAXABLE_YEAR);
    }

    profile
}

fn confirmed_version(
    profile: &TaxpayerProfile,
    id: &str,
    label: &str,
    from: Option<(i32, u32, u32)>,
    until: Option<(i32, u32, u32)>,
    tax_types: Vec<RegisteredTaxType>,
    vat: bool,
) -> TaxProfileVersion {
    let mut version = TaxProfileVersion::from_profile_backfill(profile);
    version.id = id.to_string();
    version.label = label.to_string();
    version.status = TaxProfileVersionStatus::Confirmed;
    version.effective_from = from.map(|(y, m, d)| NaiveDate::from_ymd_opt(y, m, d).unwrap());
    version.effective_until = until.map(|(y, m, d)| NaiveDate::from_ymd_opt(y, m, d).unwrap());
    version.needs_effective_date_review = version.effective_from.is_none();
    version.registered_tax_types = tax_types;
    version.is_vat_registered = vat;
    version
}

#[test]
fn dashboard_profile_matrix_base_forms_for_2026() {
    let pure_comp = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::PurelyCompensation),
    );
    assert_forms("pure compensation", &pure_comp, &["1700"]);

    let mut pure_comp_single = pure_comp.clone();
    pure_comp_single.has_single_employer = true;
    assert_forms("pure compensation single employer", &pure_comp_single, &[]);

    assert_forms(
        "self-employed non-vat no tier",
        &self_employed_profile(false, None, false),
        SELF_EMPLOYED_NON_VAT,
    );
    assert_forms(
        "self-employed non-vat micro",
        &self_employed_profile(false, Some(EoptTier::Micro), false),
        SELF_EMPLOYED_NON_VAT_MICRO_SMALL,
    );
    assert_forms(
        "self-employed non-vat small",
        &self_employed_profile(false, Some(EoptTier::Small), false),
        SELF_EMPLOYED_NON_VAT_MICRO_SMALL,
    );
    assert_forms(
        "self-employed non-vat medium",
        &self_employed_profile(false, Some(EoptTier::Medium), false),
        SELF_EMPLOYED_NON_VAT,
    );
    assert_forms(
        "self-employed non-vat large",
        &self_employed_profile(false, Some(EoptTier::Large), false),
        SELF_EMPLOYED_NON_VAT,
    );
    assert_forms(
        "self-employed non-vat 8 percent",
        &self_employed_profile(false, None, true),
        SELF_EMPLOYED_NON_VAT_8_PERCENT,
    );
    assert_forms(
        "self-employed non-vat micro 8 percent",
        &self_employed_profile(false, Some(EoptTier::Micro), true),
        SELF_EMPLOYED_NON_VAT_MICRO_SMALL_8_PERCENT,
    );
    assert_forms(
        "self-employed non-vat small 8 percent",
        &self_employed_profile(false, Some(EoptTier::Small), true),
        SELF_EMPLOYED_NON_VAT_MICRO_SMALL_8_PERCENT,
    );
    assert_forms(
        "self-employed non-vat medium 8 percent",
        &self_employed_profile(false, Some(EoptTier::Medium), true),
        SELF_EMPLOYED_NON_VAT_8_PERCENT,
    );
    assert_forms(
        "self-employed non-vat large 8 percent",
        &self_employed_profile(false, Some(EoptTier::Large), true),
        SELF_EMPLOYED_NON_VAT_8_PERCENT,
    );

    assert_forms(
        "self-employed vat no tier",
        &self_employed_profile(true, None, false),
        SELF_EMPLOYED_VAT,
    );
    assert_forms(
        "self-employed vat micro",
        &self_employed_profile(true, Some(EoptTier::Micro), false),
        SELF_EMPLOYED_VAT_MICRO_SMALL,
    );
    assert_forms(
        "self-employed vat small",
        &self_employed_profile(true, Some(EoptTier::Small), false),
        SELF_EMPLOYED_VAT_MICRO_SMALL,
    );
    assert_forms(
        "self-employed vat medium",
        &self_employed_profile(true, Some(EoptTier::Medium), false),
        SELF_EMPLOYED_VAT,
    );
    assert_forms(
        "self-employed vat large",
        &self_employed_profile(true, Some(EoptTier::Large), false),
        SELF_EMPLOYED_VAT,
    );
    assert_forms(
        "self-employed vat 8 percent",
        &self_employed_profile(true, None, true),
        SELF_EMPLOYED_VAT_8_PERCENT,
    );
    assert_forms(
        "self-employed vat micro 8 percent",
        &self_employed_profile(true, Some(EoptTier::Micro), true),
        SELF_EMPLOYED_VAT_MICRO_SMALL_8_PERCENT,
    );
    assert_forms(
        "self-employed vat small 8 percent",
        &self_employed_profile(true, Some(EoptTier::Small), true),
        SELF_EMPLOYED_VAT_MICRO_SMALL_8_PERCENT,
    );
    assert_forms(
        "self-employed vat medium 8 percent",
        &self_employed_profile(true, Some(EoptTier::Medium), true),
        SELF_EMPLOYED_VAT_8_PERCENT,
    );
    assert_forms(
        "self-employed vat large 8 percent",
        &self_employed_profile(true, Some(EoptTier::Large), true),
        SELF_EMPLOYED_VAT_8_PERCENT,
    );

    let mut mixed_non_vat = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::MixedIncome),
    );
    assert_forms("mixed income non-vat", &mixed_non_vat, MIXED_NON_VAT);
    mixed_non_vat.eopt_tier = Some(EoptTier::Micro);
    assert_forms(
        "mixed income non-vat micro tier",
        &mixed_non_vat,
        MIXED_NON_VAT,
    );
    add_eight_percent_election(&mut mixed_non_vat, TAXABLE_YEAR);
    assert_forms(
        "mixed income non-vat 8 percent",
        &mixed_non_vat,
        MIXED_NON_VAT_8_PERCENT,
    );
    mixed_non_vat.is_vat_registered = true;
    assert_forms("mixed income vat", &mixed_non_vat, MIXED_VAT);

    let corp_non_vat = base_profile(TaxpayerType::Corporation, None);
    assert_forms(
        "corporation non-vat",
        &corp_non_vat,
        &["1702Q", "1702RT", "1704", "2551Q"],
    );
    let mut corp_vat = corp_non_vat.clone();
    corp_vat.is_vat_registered = true;
    assert_forms(
        "corporation vat",
        &corp_vat,
        &["1702Q", "1702RT", "1704", "2550DS", "2550Q", "2550M"],
    );

    let partnership_non_vat = base_profile(TaxpayerType::Partnership, None);
    assert_forms(
        "partnership non-vat",
        &partnership_non_vat,
        &["1702Q", "1702RT", "2551Q"],
    );
    let mut partnership_vat = partnership_non_vat.clone();
    partnership_vat.is_vat_registered = true;
    assert_forms(
        "partnership vat",
        &partnership_vat,
        &["1702Q", "1702RT", "2550DS", "2550Q", "2550M"],
    );

    assert_forms(
        "cooperative exempt",
        &base_profile(
            TaxpayerType::Cooperative,
            Some(TaxClassification::CooperativeExempt),
        ),
        &["1702EX", "1702Q"],
    );
    assert_forms(
        "cooperative taxable",
        &base_profile(
            TaxpayerType::Cooperative,
            Some(TaxClassification::CooperativeTaxable),
        ),
        &["1702Q", "1702RT"],
    );
    assert_forms(
        "cooperative mixed",
        &base_profile(
            TaxpayerType::Cooperative,
            Some(TaxClassification::CooperativeMixed),
        ),
        &["1702MX", "1702Q"],
    );
    assert_forms(
        "cooperative default treatment",
        &base_profile(TaxpayerType::Cooperative, None),
        &["1702Q", "1702RT"],
    );

    assert_forms(
        "estate",
        &base_profile(TaxpayerType::Estate, None),
        &["1701", "1701Q"],
    );
    assert_forms(
        "trust",
        &base_profile(TaxpayerType::Trust, None),
        &["1701", "1701Q"],
    );
}

#[test]
fn dashboard_profile_matrix_withholding_modifiers() {
    let base = self_employed_profile(false, None, false);

    let mut compensation = base.clone();
    compensation.withholds_compensation = true;
    assert_forms_union(
        "withholding compensation",
        &compensation,
        &[SELF_EMPLOYED_NON_VAT, COMPENSATION_WITHHOLDING],
    );

    let mut legacy_compensation = self_employed_profile(false, None, false);
    legacy_compensation.has_employees = true;
    assert_forms_union(
        "legacy has employees maps to compensation withholding",
        &legacy_compensation,
        &[SELF_EMPLOYED_NON_VAT, COMPENSATION_WITHHOLDING],
    );

    let mut expanded = base.clone();
    expanded.withholds_expanded = true;
    assert_forms_union(
        "withholding expanded",
        &expanded,
        &[SELF_EMPLOYED_NON_VAT, EXPANDED_WITHHOLDING],
    );

    let mut legacy_expanded = self_employed_profile(false, None, false);
    legacy_expanded.is_expanded_withholding_agent = true;
    assert_forms_union(
        "legacy expanded withholding agent maps to expanded withholding",
        &legacy_expanded,
        &[SELF_EMPLOYED_NON_VAT, EXPANDED_WITHHOLDING],
    );

    let mut top_withholding_agent = self_employed_profile(false, None, false);
    top_withholding_agent.is_top_withholding_agent = true;
    assert_forms_union(
        "top withholding agent maps to expanded withholding",
        &top_withholding_agent,
        &[SELF_EMPLOYED_NON_VAT, EXPANDED_WITHHOLDING],
    );

    let mut government_withholding_entity = self_employed_profile(false, None, false);
    government_withholding_entity.is_government_withholding_entity = true;
    assert_forms_union(
        "government withholding entity maps to expanded withholding",
        &government_withholding_entity,
        &[SELF_EMPLOYED_NON_VAT, EXPANDED_WITHHOLDING],
    );

    let mut final_wh = base.clone();
    final_wh.withholds_final = true;
    assert_forms_union(
        "withholding final",
        &final_wh,
        &[SELF_EMPLOYED_NON_VAT, FINAL_WITHHOLDING],
    );

    let mut all = base;
    all.withholds_compensation = true;
    all.withholds_expanded = true;
    all.withholds_final = true;
    assert_forms_union(
        "all withholding",
        &all,
        &[
            SELF_EMPLOYED_NON_VAT,
            COMPENSATION_WITHHOLDING,
            EXPANDED_WITHHOLDING,
            FINAL_WITHHOLDING,
        ],
    );

    let mut single_employer = base_profile(
        TaxpayerType::Individual,
        Some(TaxClassification::PurelyCompensation),
    );
    single_employer.has_single_employer = true;
    single_employer.withholds_compensation = true;
    assert_forms(
        "pure compensation single employer with compensation withholding",
        &single_employer,
        COMPENSATION_WITHHOLDING,
    );
}

#[test]
fn dashboard_profile_matrix_registration_status_modifiers() {
    let active = self_employed_profile(false, None, false);
    assert_forms(
        "active registration keeps recurring obligations",
        &active,
        SELF_EMPLOYED_NON_VAT,
    );

    let mut dormant_operational = active.clone();
    dormant_operational.registration_activity_status =
        RegistrationActivityStatus::DormantOperational;
    assert_forms(
        "dormant operational keeps NIL filing obligations",
        &dormant_operational,
        SELF_EMPLOYED_NON_VAT,
    );

    let mut legacy_dormant = active.clone();
    legacy_dormant.is_dormant = true;
    assert_forms(
        "legacy dormant flag keeps NIL filing obligations",
        &legacy_dormant,
        SELF_EMPLOYED_NON_VAT,
    );

    let mut temporarily_inactive = active.clone();
    temporarily_inactive.registration_activity_status =
        RegistrationActivityStatus::TemporarilyInactive;
    assert_forms(
        "temporarily inactive keeps NIL filing obligations",
        &temporarily_inactive,
        SELF_EMPLOYED_NON_VAT,
    );

    let mut officially_closed = active;
    officially_closed.registration_activity_status = RegistrationActivityStatus::OfficiallyClosed;
    assert_forms(
        "officially closed has no recurring obligations",
        &officially_closed,
        &[],
    );

    let mut closed_with_withholding = self_employed_profile(false, None, false);
    closed_with_withholding.withholds_compensation = true;
    closed_with_withholding.withholds_expanded = true;
    closed_with_withholding.withholds_final = true;
    closed_with_withholding.registration_activity_status =
        RegistrationActivityStatus::OfficiallyClosed;
    assert_forms(
        "official closure suppresses income and withholding obligations",
        &closed_with_withholding,
        &[],
    );
}

#[test]
fn dashboard_profile_matrix_excise_modifiers() {
    let cases = [
        (ExciseTaxCategory::Alcohol, "2200A"),
        (ExciseTaxCategory::AutomobilesAndNonEssential, "2200AN"),
        (ExciseTaxCategory::Mineral, "2200M"),
        (ExciseTaxCategory::Petroleum, "2200P"),
        (ExciseTaxCategory::Tobacco, "2200T"),
        (ExciseTaxCategory::SweetenedBeverages, "2200S"),
        (ExciseTaxCategory::CoalAndCoke, "2200C"),
    ];

    for (category, form_code) in &cases {
        let mut profile = self_employed_profile(false, None, false);
        profile.excise_tax_categories = vec![category.clone()];
        assert_forms(
            &format!("excise category {:?}", category),
            &profile,
            &["1701", "1701Q", *form_code, "2551Q", "1701A"],
        );
    }

    let mut all_excise = self_employed_profile(false, None, false);
    all_excise.excise_tax_categories = cases.iter().map(|(category, _)| category.clone()).collect();
    assert_forms(
        "all excise categories",
        &all_excise,
        &[
            "1701", "1701Q", "2200A", "2200AN", "2200C", "2200M", "2200P", "2200S", "2200T",
            "2551Q", "1701A",
        ],
    );
}

#[test]
fn versioned_cor_uses_the_profile_active_for_the_selected_year() {
    let mut profile = self_employed_profile(false, None, false);

    let non_vat_2025 = confirmed_version(
        &profile,
        "cor-2025",
        "2025 non-VAT COR",
        Some((2025, 1, 1)),
        Some((2025, 12, 31)),
        vec![
            RegisteredTaxType::IncomeTax,
            RegisteredTaxType::PercentageTax,
            RegisteredTaxType::RegistrationFee,
        ],
        false,
    );
    let vat_2026 = confirmed_version(
        &profile,
        "cor-2026",
        "2026 VAT COR",
        Some((2026, 1, 1)),
        None,
        vec![
            RegisteredTaxType::IncomeTax,
            RegisteredTaxType::ValueAddedTax,
            RegisteredTaxType::RegistrationFee,
        ],
        true,
    );
    profile.profile_versions = vec![non_vat_2025, vat_2026];

    let forms_2025: BTreeSet<_> = recurring_obligation_forms_for_profile_and_year(&profile, 2025)
        .into_iter()
        .collect();
    let forms_2026 = forms_for(&profile);

    assert!(forms_2025.contains("2551Q"));
    assert!(!forms_2025.contains("2550Q"));
    assert!(forms_2026.contains("2550Q"));
    assert!(!forms_2026.contains("2551Q"));
}

#[test]
fn cor_effective_date_filters_deadlines_by_taxable_period_not_due_month() {
    let mut profile = self_employed_profile(false, None, false);
    let version = confirmed_version(
        &profile,
        "cor-june-2026",
        "June 2026 registration",
        Some((2026, 6, 1)),
        None,
        vec![
            RegisteredTaxType::IncomeTax,
            RegisteredTaxType::PercentageTax,
            RegisteredTaxType::RegistrationFee,
        ],
        false,
    );
    profile.profile_versions = vec![version];

    let deadlines = DeadlineResolver::resolve_taxable_year(2026);
    let q1_1701q = deadlines
        .iter()
        .find(|deadline| {
            deadline.form_code == "1701Q"
                && matches!(
                    deadline.period,
                    DeadlinePeriod::Quarterly {
                        taxable_year: 2026,
                        quarter: 1
                    }
                )
        })
        .unwrap();
    let q2_1701q = deadlines
        .iter()
        .find(|deadline| {
            deadline.form_code == "1701Q"
                && matches!(
                    deadline.period,
                    DeadlinePeriod::Quarterly {
                        taxable_year: 2026,
                        quarter: 2
                    }
                )
        })
        .unwrap();

    assert!(!deadline_applies_to_profile(&profile, q1_1701q));
    assert!(deadline_applies_to_profile(&profile, q2_1701q));
}

#[test]
fn profile_version_validation_rejects_overlaps_and_ignores_draft_versions() {
    let mut profile = self_employed_profile(false, None, false);
    let current = confirmed_version(
        &profile,
        "current",
        "Current COR",
        Some((2025, 1, 1)),
        None,
        vec![
            RegisteredTaxType::IncomeTax,
            RegisteredTaxType::PercentageTax,
            RegisteredTaxType::RegistrationFee,
        ],
        false,
    );
    let mut draft_vat = confirmed_version(
        &profile,
        "draft",
        "Draft VAT COR",
        Some((2026, 1, 1)),
        None,
        vec![
            RegisteredTaxType::IncomeTax,
            RegisteredTaxType::ValueAddedTax,
            RegisteredTaxType::RegistrationFee,
        ],
        true,
    );
    draft_vat.status = TaxProfileVersionStatus::Draft;
    profile.profile_versions = vec![current.clone(), draft_vat];

    let forms = forms_for(&profile);
    assert!(forms.contains("2551Q"));
    assert!(!forms.contains("2550Q"));
    assert!(
        validate_profile(&profile)
            .iter()
            .all(|error| error.field != "profile_versions")
    );

    let mut overlapping = confirmed_version(
        &profile,
        "overlap",
        "Overlapping COR",
        Some((2026, 1, 1)),
        None,
        vec![RegisteredTaxType::IncomeTax],
        false,
    );
    overlapping.status = TaxProfileVersionStatus::Confirmed;
    profile.profile_versions = vec![current, overlapping];

    assert!(
        validate_profile(&profile)
            .iter()
            .any(|error| error.field == "profile_versions")
    );
}

#[test]
fn profile_scoped_deadline_override_applies_after_global_rules() {
    let mut profile = self_employed_profile(false, None, false);
    let mut version = confirmed_version(
        &profile,
        "cor-2026",
        "2026 COR",
        Some((2026, 1, 1)),
        None,
        vec![
            RegisteredTaxType::IncomeTax,
            RegisteredTaxType::PercentageTax,
            RegisteredTaxType::RegistrationFee,
        ],
        false,
    );
    version.deadline_overrides.push(ProfileDeadlineOverride {
        id: "profile-q1-extension".into(),
        title: "Profile-specific Q1 extension".into(),
        source_reference: "Manual COR override".into(),
        affected_form_codes: vec!["1701Q".into()],
        original_deadline: NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        adjusted_deadline: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        reason: Some("RDO-specific extension".into()),
    });
    profile.profile_versions = vec![version];

    let overrides = profile_deadline_overrides_for_year(&profile, 2026);
    let deadlines = DeadlineResolver::resolve_taxable_year_with_overrides(2026, &overrides);
    let q1_1701q = deadlines
        .iter()
        .find(|deadline| {
            deadline.form_code == "1701Q"
                && matches!(
                    deadline.period,
                    DeadlinePeriod::Quarterly {
                        taxable_year: 2026,
                        quarter: 1
                    }
                )
        })
        .unwrap();

    assert!(matches!(
        q1_1701q.deadline,
        DeadlineKind::Dated {
            final_deadline,
            ..
        } if final_deadline == NaiveDate::from_ymd_opt(2026, 5, 20).unwrap()
    ));
}
