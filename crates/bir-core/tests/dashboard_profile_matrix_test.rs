use bir_core::calendar_rules::{DeadlineOverride, DeadlinePeriod, DeadlineResolver};
use bir_core::forms::{FormSetSource, PerYearFormsSet};
use bir_core::integration::recurring_obligation_forms_for_profile_and_year;
use bir_core::integration::{
    deadline_applies_to_profile, profile_deadline_overrides_for_year,
    resolve_profile_obligations_for_year_with_global_overrides,
};
use bir_core::naming::Tin;
use bir_core::profile::{
    ComplianceSourceMode, EoptTier, ExciseTaxCategory, IncomeTaxElection, ManualObligationOverride,
    ManualObligationOverrideAction, ProfileDeadlineOverride, ProfileYearFacts, RegisteredTaxType,
    RegistrationActivityStatus, TaxClassification, TaxElectionHistory, TaxProfileVersion,
    TaxProfileResolutionIssueKind, TaxProfileVersionStatus, TaxpayerProfile, TaxpayerType,
};
use bir_core::validation::validate_profile;
use chrono::NaiveDate;
use std::collections::BTreeSet;

const TAXABLE_YEAR: u16 = 2026;

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
        business_start_date: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
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
        birth_date: None,
        compliance_source_mode: Default::default(),
        per_year_forms: Default::default(),
        profile_years: Default::default(),
    }
}

fn forms_for(profile: &TaxpayerProfile) -> BTreeSet<String> {
    forms_for_year(profile, TAXABLE_YEAR)
}

fn forms_for_year(profile: &TaxpayerProfile, year: u16) -> BTreeSet<String> {
    recurring_obligation_forms_for_profile_and_year(profile, year)
        .into_iter()
        .collect()
}

fn with_manual_forms(mut profile: TaxpayerProfile, year: u16, codes: &[&str]) -> TaxpayerProfile {
    profile.per_year_forms.insert(
        year,
        PerYearFormsSet::from_codes(year, codes.iter().copied(), FormSetSource::Manual),
    );
    profile
}

fn expected(codes: &[&str]) -> BTreeSet<String> {
    codes.iter().map(|code| code.to_string()).collect()
}

fn assert_forms(label: &str, profile: &TaxpayerProfile, codes: &[&str]) {
    assert_eq!(forms_for(profile), expected(codes), "{label}");
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

fn configure_forms_set_for_year(profile: &mut TaxpayerProfile, year: u16) {
    profile
        .per_year_forms
        .entry(year)
        .or_insert_with(|| PerYearFormsSet::new(year));
}

#[test]
fn dashboard_profile_matrix_base_forms_for_2026() {
    let empty = self_employed_profile(false, None, false);
    assert_forms(
        "no manual set means no dashboard forms, even for self-employed non-VAT",
        &empty,
        &[],
    );

    let vat = self_employed_profile(true, None, false);
    assert_forms(
        "VAT-registered still has no dashboard forms until the user picks them",
        &vat,
        &[],
    );

    let chosen = with_manual_forms(
        self_employed_profile(true, None, false),
        TAXABLE_YEAR,
        &["2551Q", "1601C"],
    );
    assert_forms(
        "manual 2551Q+1601C for 2026 even if VAT would have inferred 2550Q",
        &chosen,
        &["1601C", "2551Q"],
    );
}

#[test]
fn dashboard_profile_matrix_withholding_modifiers() {
    let mut withholding = self_employed_profile(false, None, false);
    withholding.withholds_compensation = true;
    withholding.withholds_expanded = true;
    withholding.withholds_final = true;
    assert_forms(
        "withholding flags do not infer a forms set",
        &withholding,
        &[],
    );

    let chosen = with_manual_forms(
        withholding,
        TAXABLE_YEAR,
        &["2551Q", "1601C", "0619E"],
    );
    assert_forms(
        "manual set is the dashboard list",
        &chosen,
        &["0619E", "1601C", "2551Q"],
    );
}

#[test]
fn dashboard_profile_matrix_registration_status_modifiers() {
    let chosen = with_manual_forms(
        self_employed_profile(false, None, false),
        TAXABLE_YEAR,
        &["2551Q", "1601C"],
    );
    assert_forms(
        "manual set is the dashboard list for an active registration",
        &chosen,
        &["1601C", "2551Q"],
    );

    let mut dormant_operational = chosen.clone();
    dormant_operational.registration_activity_status =
        RegistrationActivityStatus::DormantOperational;
    assert_forms(
        "dormant operational does not rewrite a Manual Forms Set",
        &dormant_operational,
        &["1601C", "2551Q"],
    );

    let mut officially_closed = chosen;
    officially_closed.registration_activity_status = RegistrationActivityStatus::OfficiallyClosed;
    assert_forms(
        "officially closed does not drop a Manual Forms Set",
        &officially_closed,
        &["1601C", "2551Q"],
    );
}

#[test]
fn dashboard_profile_matrix_excise_modifiers() {
    let mut profile = self_employed_profile(false, None, false);
    profile.excise_tax_categories = vec![ExciseTaxCategory::Alcohol];
    assert_forms(
        "excise flags do not infer a forms set",
        &profile,
        &[],
    );

    let chosen = with_manual_forms(profile, TAXABLE_YEAR, &["2200A", "2551Q"]);
    assert_forms(
        "manual excise form is the dashboard list",
        &chosen,
        &["2200A", "2551Q"],
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
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;
    profile.per_year_forms.insert(
        2025,
        PerYearFormsSet::from_codes(2025, ["2551Q"].iter().copied(), FormSetSource::Manual),
    );
    profile.per_year_forms.insert(
        2026,
        PerYearFormsSet::from_codes(2026, ["2550Q"].iter().copied(), FormSetSource::Manual),
    );

    let forms_2025 = forms_for_year(&profile, 2025);
    let forms_2026 = forms_for(&profile);

    assert!(forms_2025.contains("2551Q"));
    assert!(!forms_2025.contains("2550Q"));
    assert!(forms_2026.contains("2550Q"));
    assert!(!forms_2026.contains("2551Q"));
}

#[test]
fn missing_profile_year_is_not_a_profile_versions_blocker() {
    let mut profile = self_employed_profile(false, None, false);
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

    assert!(
        validate_profile(&profile)
            .iter()
            .all(|error| error.field != "profile_versions"),
        "COR confirmed versions are not a V1 save gate"
    );
    let resolved = profile.resolve_tax_profile_for_year(TAXABLE_YEAR);
    assert!(resolved.effective_segments.is_empty());
    assert!(resolved.issues.iter().any(|issue| {
        issue.kind == TaxProfileResolutionIssueKind::NoProfileYear
            && issue.message == "no 2026 profile"
    }));
}

#[test]
fn confirming_new_cor_version_auto_closes_previous_version() {
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
    let mut draft = confirmed_version(
        &profile,
        "draft-vat",
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
    draft.status = TaxProfileVersionStatus::Draft;
    profile.profile_versions = vec![current, draft];

    assert!(
        profile.set_profile_version_confirmed(
            "draft-vat",
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
        )
    );

    let previous = profile
        .profile_versions
        .iter()
        .find(|version| version.id == "current")
        .unwrap();
    let confirmed = profile
        .profile_versions
        .iter()
        .find(|version| version.id == "draft-vat")
        .unwrap();

    assert_eq!(
        previous.effective_until,
        Some(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap())
    );
    assert_eq!(confirmed.status, TaxProfileVersionStatus::Confirmed);
    assert_eq!(
        profile.compliance_source_mode,
        ComplianceSourceMode::CorVersioned
    );
}

#[test]
fn missing_forms_set_blocks_deadlines_even_when_profile_version_is_effective() {
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
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

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
    assert!(!deadline_applies_to_profile(&profile, q2_1701q));
}

#[test]
fn stored_forms_set_controls_deadline_applicability() {
    let mut profile = self_employed_profile(false, None, false);
    profile.per_year_forms.insert(
        TAXABLE_YEAR,
        bir_core::forms::PerYearFormsSet::from_codes(
            TAXABLE_YEAR,
            ["1701Q"],
            bir_core::forms::FormSetSource::Manual,
        ),
    );
    let deadlines = DeadlineResolver::resolve_taxable_year(TAXABLE_YEAR as i32);
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

    assert!(deadline_applies_to_profile(&profile, q1_1701q));
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
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

    let forms = forms_for(&profile);
    assert!(
        forms.is_empty(),
        "COR draft/confirmed versions do not invent a Forms Set"
    );
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
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

    assert!(
        validate_profile(&profile)
            .iter()
            .all(|error| error.field != "profile_versions"),
        "overlapping COR ledger dates are not a V1 filing blocker"
    );
}

#[test]
fn yearly_profile_resolution_excludes_undated_confirmed_version() {
    let profile = self_employed_profile(false, None, false);
    let resolved = profile.resolve_tax_profile_for_year(TAXABLE_YEAR);
    assert!(resolved.effective_segments.is_empty());
    assert!(
        resolved
            .issues
            .iter()
            .any(|issue| issue.kind == bir_core::profile::TaxProfileResolutionIssueKind::NoProfileYear)
    );
}

#[test]
fn yearly_profile_resolution_returns_the_year_clone() {
    let mut profile = self_employed_profile(false, None, false);
    profile.full_name = "2026 clone".into();
    profile.capture_current_as_year(TAXABLE_YEAR).unwrap();
    profile.full_name = "2025 clone".into();
    profile.capture_current_as_year(TAXABLE_YEAR - 1).unwrap();

    let resolved = profile.resolve_tax_profile_for_year(TAXABLE_YEAR);

    assert_eq!(resolved.effective_segments.len(), 1);
    assert!(resolved.issues.is_empty());
    assert_eq!(
        resolved.effective_segments[0].cor.registered_name,
        "2026 clone"
    );
}

#[test]
fn compensation_withholding_does_not_suggest_form_1600() {
    let mut profile = self_employed_profile(false, None, false);
    profile.withholds_compensation = true;
    let version = confirmed_version(
        &profile,
        "compensation",
        "Compensation withholding",
        Some((2026, 1, 1)),
        None,
        vec![RegisteredTaxType::WithholdingCompensation],
        false,
    );
    profile.profile_versions = vec![version];
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

    let forms = forms_for(&profile);

    assert!(!forms.contains("1600"));
}

#[test]
fn vat_percentage_withholding_tax_type_does_not_infer_form_1600() {
    let mut profile = self_employed_profile(false, None, false);
    let version = confirmed_version(
        &profile,
        "vat-percentage-withholding",
        "VAT and percentage withholding",
        Some((2026, 1, 1)),
        None,
        vec![RegisteredTaxType::WithholdingVatAndPercentage],
        false,
    );
    profile.profile_versions = vec![version];
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

    let forms = forms_for(&profile);
    assert!(
        !forms.contains("1600"),
        "withholding tax types do not infer a Forms Set"
    );

    let chosen = with_manual_forms(profile, TAXABLE_YEAR, &["1600"]);
    assert!(forms_for(&chosen).contains("1600"));
}

#[test]
fn profile_scoped_deadline_override_on_cor_ledger_is_not_a_filing_lookup() {
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
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

    let overrides = profile_deadline_overrides_for_year(&profile, 2026);
    assert!(
        overrides.is_empty(),
        "COR-ledger deadline overrides are not a V1 filing lookup"
    );
}

#[test]
fn profile_global_deadline_override_conflict_is_not_reported_from_cor_ledger() {
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
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

    let global_overrides = vec![DeadlineOverride {
        id: "global-q1-extension".into(),
        title: "Global Q1 extension".into(),
        source_reference: "BIR advisory".into(),
        affected_form_codes: vec!["1701Q".into()],
        original_deadline: NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        adjusted_deadline: NaiveDate::from_ymd_opt(2026, 5, 19).unwrap(),
        affected_regions: vec![],
        affected_taxpayer_types: vec![],
        effective_from: None,
        effective_until: None,
        expires_at: None,
    }];

    let preview = resolve_profile_obligations_for_year_with_global_overrides(
        &profile,
        2026,
        &global_overrides,
    );
    assert!(
        preview
            .consistency_report
            .issues
            .iter()
            .all(|issue| issue.code != "PROFILE_GLOBAL_DEADLINE_OVERRIDE_CONFLICT"),
        "COR-ledger deadline overrides must not conflict with global calendar rules"
    );
}

#[test]
fn ledger_override_hygiene_is_not_a_save_gate() {
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
        ],
        false,
    );
    version.obligation_overrides.push(ManualObligationOverride {
        form_code: "2551Q".into(),
        action: ManualObligationOverrideAction::Exclude,
        reason: String::new(),
        source_reference: None,
    });
    version.deadline_overrides.push(ProfileDeadlineOverride {
        id: "missing-source".into(),
        title: String::new(),
        source_reference: String::new(),
        affected_form_codes: vec![],
        original_deadline: NaiveDate::from_ymd_opt(2026, 4, 25).unwrap(),
        adjusted_deadline: NaiveDate::from_ymd_opt(2026, 4, 28).unwrap(),
        reason: None,
    });
    profile.profile_versions = vec![version];
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

    assert!(
        validate_profile(&profile)
            .iter()
            .all(|error| error.field != "profile_versions"),
        "ledger override hygiene is not a V1 save gate"
    );
}

#[test]
fn cor_consistency_report_explains_8_percent_percentage_tax_suppression() {
    let mut profile = self_employed_profile(false, None, true);
    let version = confirmed_version(
        &profile,
        "cor-2026",
        "2026 COR",
        Some((2026, 1, 1)),
        None,
        vec![
            RegisteredTaxType::IncomeTax,
            RegisteredTaxType::PercentageTax,
        ],
        false,
    );
    profile.profile_versions = vec![version];
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;
    configure_forms_set_for_year(&mut profile, 2026);

    let preview = profile.preview_obligations_for_year(2026);
    let issue = preview
        .consistency_report
        .issues
        .iter()
        .find(|issue| issue.code == "PERCENTAGE_TAX_SUPPRESSED_BY_8_PERCENT")
        .expect("8% suppression diagnostic");

    assert_eq!(
        issue.severity,
        bir_core::integration::ProfileConsistencySeverity::Info
    );
    assert_eq!(issue.version_id.as_deref(), Some("cor-2026"));
    assert_eq!(issue.source.as_deref(), Some("Income tax election + TTCE"));
    assert!(
        issue
            .fix_hint
            .as_deref()
            .unwrap_or_default()
            .contains("income tax election ledger")
    );
    assert!(!preview.form_codes.iter().any(|code| code == "2551Q"));
}

#[test]
fn eight_percent_election_preserves_non_pt010_percentage_tax_obligation() {
    let mut profile = self_employed_profile(false, None, true);
    profile.atc_codes = vec!["PT010".into(), "PT040".into()];
    let version = confirmed_version(
        &profile,
        "cor-2026",
        "2026 COR",
        Some((2026, 1, 1)),
        None,
        vec![
            RegisteredTaxType::IncomeTax,
            RegisteredTaxType::PercentageTax,
        ],
        false,
    );
    profile.profile_versions = vec![version];
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;
    profile.per_year_forms.insert(
        2026,
        PerYearFormsSet::from_codes(2026, ["2551Q"].iter().copied(), FormSetSource::Manual),
    );

    let preview = profile.preview_obligations_for_year(2026);

    assert!(preview.form_codes.iter().any(|code| code == "2551Q"));
    assert!(
        preview
            .consistency_report
            .issues
            .iter()
            .all(|issue| issue.code != "PERCENTAGE_TAX_SUPPRESSED_BY_8_PERCENT")
    );
}

#[test]
fn cor_consistency_report_flags_cor_tin_mismatch() {
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
        ],
        false,
    );
    version.cor.tin = Some("999-888-777-00000".into());
    profile.profile_versions = vec![version];
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;
    configure_forms_set_for_year(&mut profile, 2026);

    let preview = profile.preview_obligations_for_year(2026);
    let issue = preview
        .consistency_report
        .issues
        .iter()
        .find(|issue| issue.code == "COR_TIN_MISMATCH")
        .expect("COR TIN mismatch diagnostic");

    assert_eq!(
        issue.severity,
        bir_core::integration::ProfileConsistencySeverity::NeedsReview
    );
    assert_eq!(issue.version_id.as_deref(), Some("cor-2026"));
    assert_eq!(issue.source.as_deref(), Some("COR/profile version"));
    assert!(
        issue
            .fix_hint
            .as_deref()
            .unwrap_or_default()
            .contains("Verify the uploaded COR")
    );
}

#[test]
fn cor_consistency_report_includes_manual_override_context() {
    let mut profile = self_employed_profile(false, None, false);
    let mut version = confirmed_version(
        &profile,
        "cor-2026",
        "2026 COR",
        Some((2026, 1, 1)),
        None,
        vec![RegisteredTaxType::IncomeTax],
        false,
    );
    version.obligation_overrides.push(ManualObligationOverride {
        form_code: "9999Z".into(),
        action: ManualObligationOverrideAction::Include,
        reason: "Local exception".into(),
        source_reference: Some("RDO memo".into()),
    });
    profile.profile_versions = vec![version];
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;
    configure_forms_set_for_year(&mut profile, 2026);

    let preview = profile.preview_obligations_for_year(2026);
    let issue = preview
        .consistency_report
        .issues
        .iter()
        .find(|issue| issue.code == "MANUAL_INCLUDE_UNKNOWN_FORM")
        .expect("manual include diagnostic");

    assert_eq!(issue.version_id.as_deref(), Some("cor-2026"));
    assert_eq!(issue.form_code.as_deref(), Some("9999Z"));
    assert_eq!(issue.source.as_deref(), Some("Manual obligation override"));
    assert!(
        issue
            .fix_hint
            .as_deref()
            .unwrap_or_default()
            .contains("TTCE/calendar support")
    );
}

#[test]
fn abolished_1704_is_not_a_current_corporate_dashboard_obligation() {
    let mut profile = base_profile(
        TaxpayerType::Corporation,
        Some(TaxClassification::Corporation),
    );
    let version = confirmed_version(
        &profile,
        "corp-cor-2026",
        "Corporate COR",
        Some((2026, 1, 1)),
        None,
        vec![RegisteredTaxType::IncomeTax],
        false,
    );
    profile.profile_versions = vec![version];
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;
    configure_forms_set_for_year(&mut profile, 2026);

    let preview = profile.preview_obligations_for_year(2026);
    assert!(!preview.form_codes.iter().any(|code| code == "1704"));
    assert!(
        !preview
            .consistency_report
            .issues
            .iter()
            .any(|issue| issue.code == "OBLIGATION_WITHOUT_CALENDAR_RULE"
                && issue.form_code.as_deref() == Some("1704")),
        "abolished 1704 should not produce a missing calendar rule diagnostic for 2026"
    );
}

#[test]
fn manual_include_still_reports_missing_calendar_rule_for_1704() {
    let mut profile = base_profile(
        TaxpayerType::Corporation,
        Some(TaxClassification::Corporation),
    );
    let mut version = confirmed_version(
        &profile,
        "corp-cor-2026",
        "Corporate COR",
        Some((2026, 1, 1)),
        None,
        vec![RegisteredTaxType::IncomeTax],
        false,
    );
    version.obligation_overrides.push(ManualObligationOverride {
        form_code: "1704".into(),
        action: ManualObligationOverrideAction::Include,
        reason: "Legacy/special IAET obligation asserted by user".into(),
        source_reference: Some("Manual evidence".into()),
    });
    profile.profile_versions = vec![version];
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;
    profile.per_year_forms.insert(
        2026,
        PerYearFormsSet::from_codes(2026, ["1704"].iter().copied(), FormSetSource::Manual),
    );

    let preview = profile.preview_obligations_for_year(2026);
    let issue = preview
        .consistency_report
        .issues
        .iter()
        .find(|issue| {
            issue.code == "OBLIGATION_WITHOUT_CALENDAR_RULE"
                && issue.form_code.as_deref() == Some("1704")
        })
        .expect("missing calendar rule diagnostic for 1704");

    assert_eq!(issue.version_id.as_deref(), Some("corp-cor-2026"));
    assert_eq!(issue.source.as_deref(), Some("Calendar rules"));
    assert!(
        issue
            .fix_hint
            .as_deref()
            .unwrap_or_default()
            .contains("official calendar rule")
    );
    assert!(preview.form_codes.iter().any(|code| code == "1704"));
}

#[test]
fn profile_version_ledger_date_range_is_not_a_save_gate() {
    let mut profile = self_employed_profile(false, None, false);
    let version = confirmed_version(
        &profile,
        "bad-range",
        "Bad Range COR",
        Some((2026, 6, 1)),
        Some((2026, 5, 31)),
        vec![RegisteredTaxType::IncomeTax],
        false,
    );
    profile.profile_versions = vec![version];
    profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

    assert!(
        validate_profile(&profile).iter().all(|error| {
            !error
                .message
                .contains("effective end date before its start date")
        }),
        "COR ledger date ranges are not a V1 save gate"
    );
}

#[test]
fn validate_profile_rejects_year_before_business_start() {
    let mut profile = self_employed_profile(false, None, false);
    profile.business_start_date = Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    profile
        .profile_years
        .insert(2023, ProfileYearFacts::from_profile(&profile));

    assert!(
        validate_profile(&profile)
            .iter()
            .any(|error| error.field == "profile_years"
                && error.message.contains("before Business Start Date"))
    );
}
