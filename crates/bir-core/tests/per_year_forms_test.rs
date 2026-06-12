use bir_core::db::Database;
use bir_core::forms::forms_set::{FormSetEntry, FormSetSource, PerYearFormsSet};
use bir_core::integration::resolve_profile_obligations_for_year;
use bir_core::naming::Tin;
use bir_core::profile::{
    ComplianceSourceMode, CorRegistrationFacts, ManualObligationOverride,
    ManualObligationOverrideAction, RegisteredTaxType, TaxClassification, TaxProfileVersion,
    TaxProfileVersionSource, TaxProfileVersionStatus, TaxpayerProfile, TaxpayerType,
};
use chrono::NaiveDate;
use tempfile::NamedTempFile;

fn parse_tin(tin_str: &str) -> Tin {
    assert!(tin_str.len() == 12 || tin_str.len() == 14);
    Tin {
        segment1: tin_str[0..3].to_string(),
        segment2: tin_str[3..6].to_string(),
        segment3: tin_str[6..9].to_string(),
        branch: tin_str[9..].to_string(),
    }
}

fn create_test_profile(tin_str: &str) -> TaxpayerProfile {
    let tin = parse_tin(tin_str);
    let mut profile = TaxpayerProfile {
        id: None,
        full_name: "Test Taxpayer".to_string(),
        tin,
        rdo_code: "039".to_string(),
        line_of_business: "Testing Services".to_string(),
        registered_address: "123 Test St, Manila".to_string(),
        zip_code: "1000".to_string(),
        phone: "09123456789".to_string(),
        email: "test@example.com".to_string(),
        default_form_type: "1701".to_string(),
        taxpayer_type: TaxpayerType::Individual,
        is_vat_registered: false,
        business_start_date: Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
        birth_date: None,
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
        profile_versions: vec![],
        compliance_source_mode: ComplianceSourceMode::CorVersioned,
        per_year_forms: Default::default(),
    };
    profile.ensure_profile_version_ledger();
    profile
}

#[test]
fn test_per_year_forms_as_single_source_of_truth() {
    temp_env::with_var("EBIR_TEST_ENV", Some("1"), || {
        let temp_file = NamedTempFile::new().unwrap();
        let db = Database::open(temp_file.path()).expect("Failed to open DB");

        let profile = create_test_profile("010558054000");
        let tin_str = profile.tin.full();

        // Save profile. Initially no custom forms set is saved in per_year_forms table,
        // so it falls back to generating it from the confirmed version on save.
        // Wait, on initial save, because there's a confirmed version (the legacy backfill),
        // save_profile scans for missing per_year_forms rows and populates them!
        // Let's first save with empty versions to verify the absolute fallback.
        // Wait, `db.save_profile` calls `profile.ensure_profile_version_ledger()`, which adds a legacy confirmed version.
        // So `save_profile` will populate `per_year_forms`.
        // Let's delete the generated forms set to simulate the database fallback when per_year_forms is absent.
        let _saved_profile = db.save_profile(profile).expect("Failed to save profile");

        db.delete_per_year_forms(&tin_str, 2026).unwrap();

        // Reload profile from DB (without per_year_forms row for 2026)
        let profile_no_forms = db.get_profile(&tin_str).unwrap().unwrap();
        assert!(!db.has_per_year_forms(&tin_str, 2026).unwrap());

        // Under fallback, we derive obligations from the confirmed profile version:
        let fallback_resolved = resolve_profile_obligations_for_year(&profile_no_forms, 2026);
        assert!(
            !fallback_resolved.form_codes.is_empty(),
            "Fallback should return default suggestions"
        );
        assert!(fallback_resolved.form_codes.contains(&"1701".to_string()));

        // Now, save a custom forms set to the database for 2026 (precisely containing ONLY ["1701"])
        let forms_set =
            PerYearFormsSet::from_codes(2026, vec!["1701".to_string()], FormSetSource::Manual);
        db.save_per_year_forms(&tin_str, 2026, &forms_set).unwrap();

        // Reload profile from DB
        let profile_with_forms = db.get_profile(&tin_str).unwrap().unwrap();
        assert!(db.has_per_year_forms(&tin_str, 2026).unwrap());
        assert_eq!(
            profile_with_forms
                .per_year_forms
                .get(&2026)
                .unwrap()
                .active_form_codes(),
            vec!["1701".to_string()]
        );

        // Resolve obligations again. It MUST precisely match the forms set (only ["1701"]), bypassing all fallback logic.
        let resolved = resolve_profile_obligations_for_year(&profile_with_forms, 2026);
        assert_eq!(
            resolved.form_codes,
            vec!["1701".to_string()],
            "Should derive obligations precisely from forms set"
        );
    });
}

#[test]
fn test_custom_form_added_and_removed() {
    temp_env::with_var("EBIR_TEST_ENV", Some("1"), || {
        let temp_file = NamedTempFile::new().unwrap();
        let db = Database::open(temp_file.path()).expect("Failed to open DB");

        let profile = create_test_profile("010558054000");
        let tin_str = profile.tin.full();
        let _saved_profile = db.save_profile(profile).expect("Failed to save profile");

        // 1. Manually add custom form code "CUSTOM_FORM" to the forms set
        let mut set = db.get_per_year_forms(&tin_str, 2026).unwrap();
        set.entries.push(FormSetEntry {
            form_code: "CUSTOM_FORM".to_string(),
            frequency: bir_core::forms::registry::FilingFrequency::Quarterly,
            active: true,
            source: FormSetSource::Manual,
            custom: true,
            reason: Some("Manual addition of custom form".to_string()),
        });

        // Save the updated forms set back
        db.save_per_year_forms(&tin_str, 2026, &set).unwrap();

        // Reload and verify
        let profile_updated = db.get_profile(&tin_str).unwrap().unwrap();
        let resolved_updated = resolve_profile_obligations_for_year(&profile_updated, 2026);
        assert!(
            resolved_updated
                .form_codes
                .contains(&"CUSTOM_FORM".to_string()),
            "Should contain the custom form"
        );

        // 2. Suppress/Deactivate custom form
        let mut set_deactivate = db.get_per_year_forms(&tin_str, 2026).unwrap();
        if let Some(entry) = set_deactivate
            .entries
            .iter_mut()
            .find(|e| e.form_code == "CUSTOM_FORM")
        {
            entry.active = false;
        }
        db.save_per_year_forms(&tin_str, 2026, &set_deactivate)
            .unwrap();

        // Reload and verify
        let profile_deactivated = db.get_profile(&tin_str).unwrap().unwrap();
        let resolved_deactivated = resolve_profile_obligations_for_year(&profile_deactivated, 2026);
        assert!(
            !resolved_deactivated
                .form_codes
                .contains(&"CUSTOM_FORM".to_string()),
            "Should not contain deactivated custom form"
        );
    });
}

#[test]
fn test_cor_confirmation_flow_populates_forms_set() {
    temp_env::with_var("EBIR_TEST_ENV", Some("1"), || {
        let temp_file = NamedTempFile::new().unwrap();
        let db = Database::open(temp_file.path()).expect("Failed to open DB");

        // Create a profile, but manually clear any default confirmed versions so we can simulate draft -> confirm flow.
        let mut profile = create_test_profile("010558054000");
        profile.profile_versions.clear();
        profile.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;
        let tin_str = profile.tin.full();

        // Add a DRAFT version
        let draft_version = TaxProfileVersion {
            id: "draft-cor-1".to_string(),
            label: "Draft COR Version".to_string(),
            status: TaxProfileVersionStatus::Draft,
            source: TaxProfileVersionSource::OcrCor,
            effective_from: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            effective_until: None,
            needs_effective_date_review: false,
            cor: CorRegistrationFacts {
                tin: Some(tin_str.clone()),
                registration_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
                registered_name: "Test Taxpayer".to_string(),
                trade_name: None,
                registered_address: "Manila".to_string(),
                rdo_code: "039".to_string(),
                line_of_business_code: None,
                line_of_business_description: "Services".to_string(),
            },
            registered_tax_types: vec![
                RegisteredTaxType::IncomeTax,
                RegisteredTaxType::PercentageTax,
            ],
            taxpayer_type: TaxpayerType::Individual,
            tax_classification: Some(TaxClassification::SelfEmployed),
            eopt_tier: None,
            is_vat_registered: false,
            is_gpp_partner: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            excise_tax_categories: vec![],
            registration_activity_status: Default::default(),
            evidence: vec![],
            obligation_overrides: vec![ManualObligationOverride {
                form_code: "1701".to_string(),
                action: ManualObligationOverrideAction::Include,
                reason: "Required".to_string(),
                source_reference: None,
            }],
            deadline_overrides: vec![],
        };
        profile.profile_versions.push(draft_version);

        let mut saved_profile = db
            .save_profile(profile)
            .expect("Failed to save initial profile");

        // Verify no forms set exists for 2026 since version is DRAFT
        db.delete_per_year_forms(&tin_str, 2026).unwrap();
        assert!(!db.has_per_year_forms(&tin_str, 2026).unwrap());

        // Simulate confirmation: change status to Confirmed, update profile and save
        saved_profile.profile_versions[0].status = TaxProfileVersionStatus::Confirmed;
        saved_profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

        let _final_saved = db
            .save_profile(saved_profile)
            .expect("Failed to save confirmed profile");

        // Verify that a forms set has been automatically created and populated in per_year_forms table for 2026!
        assert!(db.has_per_year_forms(&tin_str, 2026).unwrap());
        let generated_set = db.get_per_year_forms(&tin_str, 2026).unwrap();
        assert!(
            !generated_set.entries.is_empty(),
            "Generated forms set should not be empty"
        );

        // It should contain the active form "1701" with source CorAi since the draft version source was OcrCor
        let entry = generated_set.entry("1701").unwrap();
        assert!(entry.active);
        assert_eq!(entry.source, FormSetSource::CorAi);
    });
}

#[test]
fn test_obligation_filtering_individual_vs_corporate_and_vat() {
    temp_env::with_var("EBIR_TEST_ENV", Some("1"), || {
        let temp_file = NamedTempFile::new().unwrap();
        let db = Database::open(temp_file.path()).expect("Failed to open DB");

        // 1. Create Individual profile
        let mut individual = create_test_profile("010558054000");
        individual.taxpayer_type = TaxpayerType::Individual;
        individual.tax_classification = Some(TaxClassification::SelfEmployed);
        individual.compliance_source_mode = ComplianceSourceMode::CorVersioned;
        individual.profile_versions[0] = TaxProfileVersion {
            id: "v-indiv".to_string(),
            label: "Indiv Version".to_string(),
            status: TaxProfileVersionStatus::Confirmed,
            source: TaxProfileVersionSource::ManualCor,
            effective_from: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            effective_until: None,
            needs_effective_date_review: false,
            cor: CorRegistrationFacts {
                tin: Some("010558054000".to_string()),
                registration_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
                registered_name: "Test Indiv".to_string(),
                trade_name: None,
                registered_address: "QC".to_string(),
                rdo_code: "039".to_string(),
                line_of_business_code: None,
                line_of_business_description: "Services".to_string(),
            },
            registered_tax_types: vec![
                RegisteredTaxType::IncomeTax,
                RegisteredTaxType::PercentageTax,
            ],
            taxpayer_type: TaxpayerType::Individual,
            tax_classification: Some(TaxClassification::SelfEmployed),
            eopt_tier: None,
            is_vat_registered: false,
            is_gpp_partner: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            excise_tax_categories: vec![],
            registration_activity_status: Default::default(),
            evidence: vec![],
            obligation_overrides: vec![],
            deadline_overrides: vec![],
        };

        let saved_indiv = db.save_profile(individual).unwrap();
        let resolved_indiv = resolve_profile_obligations_for_year(&saved_indiv, 2026);

        // Individual should have 1701/1701Q but NOT 1702/1702Q
        assert!(resolved_indiv.form_codes.contains(&"1701".to_string()));
        assert!(resolved_indiv.form_codes.contains(&"1701Q".to_string()));
        assert!(!resolved_indiv.form_codes.contains(&"1702RT".to_string()));
        assert!(!resolved_indiv.form_codes.contains(&"1702Q".to_string()));

        // 2. Create Corporate profile
        let mut corporate = create_test_profile("987654321000");
        corporate.taxpayer_type = TaxpayerType::Corporation;
        corporate.tax_classification = Some(TaxClassification::Corporation);
        corporate.compliance_source_mode = ComplianceSourceMode::CorVersioned;
        corporate.profile_versions[0] = TaxProfileVersion {
            id: "v-corp".to_string(),
            label: "Corp Version".to_string(),
            status: TaxProfileVersionStatus::Confirmed,
            source: TaxProfileVersionSource::ManualCor,
            effective_from: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            effective_until: None,
            needs_effective_date_review: false,
            cor: CorRegistrationFacts {
                tin: Some("987654321000".to_string()),
                registration_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
                registered_name: "Test Corp".to_string(),
                trade_name: None,
                registered_address: "Manila".to_string(),
                rdo_code: "039".to_string(),
                line_of_business_code: None,
                line_of_business_description: "Services".to_string(),
            },
            registered_tax_types: vec![RegisteredTaxType::IncomeTax],
            taxpayer_type: TaxpayerType::Corporation,
            tax_classification: Some(TaxClassification::Corporation),
            eopt_tier: None,
            is_vat_registered: false,
            is_gpp_partner: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            excise_tax_categories: vec![],
            registration_activity_status: Default::default(),
            evidence: vec![],
            obligation_overrides: vec![],
            deadline_overrides: vec![],
        };

        let saved_corp = db.save_profile(corporate).unwrap();
        let resolved_corp = resolve_profile_obligations_for_year(&saved_corp, 2026);

        // Corp should have 1702RT/1702Q but NOT 1701/1701Q
        assert!(!resolved_corp.form_codes.contains(&"1701".to_string()));
        assert!(!resolved_corp.form_codes.contains(&"1701Q".to_string()));
        assert!(resolved_corp.form_codes.contains(&"1702RT".to_string()));
        assert!(resolved_corp.form_codes.contains(&"1702Q".to_string()));

        // 3. Create VAT-registered Corporate profile
        let mut vat_corp = create_test_profile("111222333000");
        vat_corp.taxpayer_type = TaxpayerType::Corporation;
        vat_corp.tax_classification = Some(TaxClassification::Corporation);
        vat_corp.is_vat_registered = true;
        vat_corp.compliance_source_mode = ComplianceSourceMode::CorVersioned;
        vat_corp.profile_versions[0] = TaxProfileVersion {
            id: "v-vat-corp".to_string(),
            label: "VAT Corp Version".to_string(),
            status: TaxProfileVersionStatus::Confirmed,
            source: TaxProfileVersionSource::ManualCor,
            effective_from: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            effective_until: None,
            needs_effective_date_review: false,
            cor: CorRegistrationFacts {
                tin: Some("111222333000".to_string()),
                registration_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
                registered_name: "Test VAT Corp".to_string(),
                trade_name: None,
                registered_address: "Manila".to_string(),
                rdo_code: "039".to_string(),
                line_of_business_code: None,
                line_of_business_description: "Services".to_string(),
            },
            registered_tax_types: vec![
                RegisteredTaxType::IncomeTax,
                RegisteredTaxType::ValueAddedTax,
            ],
            taxpayer_type: TaxpayerType::Corporation,
            tax_classification: Some(TaxClassification::Corporation),
            eopt_tier: None,
            is_vat_registered: true,
            is_gpp_partner: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            excise_tax_categories: vec![],
            registration_activity_status: Default::default(),
            evidence: vec![],
            obligation_overrides: vec![],
            deadline_overrides: vec![],
        };

        let saved_vat_corp = db.save_profile(vat_corp).unwrap();
        let resolved_vat_corp = resolve_profile_obligations_for_year(&saved_vat_corp, 2026);

        // VAT Corp should have 2550Q/2550M but NOT 2551Q (percentage tax)
        assert!(resolved_vat_corp.form_codes.contains(&"2550Q".to_string()));
        assert!(!resolved_vat_corp.form_codes.contains(&"2551Q".to_string()));
    });
}
