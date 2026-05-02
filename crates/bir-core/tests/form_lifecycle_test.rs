use bir_core::db::Database;
use bir_core::forms::{FilingStatus, Form2551QDraft};
use bir_core::naming::Tin;
use bir_core::profile::TaxpayerProfile;
use tempfile::NamedTempFile;

#[test]
fn test_full_form_lifecycle() {
    unsafe {
        std::env::set_var("EBIR_TEST_ENV", "1");
    }
    // 1. Initialize temporary encrypted database
    let temp_file = NamedTempFile::new().unwrap();
    let db = Database::open(temp_file.path()).expect("Failed to open temp DB");

    // 2. Create and save profile
    let profile = TaxpayerProfile {
        id: None,
        full_name: "Integration Test User".to_string(),
        tin: Tin {
            segment1: "123".into(),
            segment2: "456".into(),
            segment3: "789".into(),
            branch: "000".into(),
        },
        rdo_code: "039".to_string(),
        line_of_business: "Test".to_string(),
        registered_address: "Manila".to_string(),
        zip_code: "1000".to_string(),
        phone: "09000000000".to_string(),
        email: "test@example.com".to_string(),
        default_form_type: "2551Q".to_string(),
        taxpayer_type: bir_core::profile::TaxpayerType::Individual,
        is_vat_registered: false,
        business_start_date: None,
        tax_classification: None,
        opted_for_8_percent_flat_rate: false,
        is_archived: false,
        profile_pin_hash: None,
            totp_secret: None,
        email_tracking_enabled: false,
        email_auth_method: bir_core::profile::EmailAuthMethod::AppPassword,
        imap_email: None,
        imap_host: None,
        _imap_enabled_compat: None,
        test_notification_enabled: false,
        imap_app_password: None,
        oauth_access_token: None,
        oauth_refresh_token: None,
    };
    let saved_profile = db.save_profile(profile).expect("Failed to save profile");

    // 3. Initialize Draft
    let mut draft = Form2551QDraft::new_from_profile(&saved_profile, 2026, 1);
    assert_eq!(draft.status, FilingStatus::Draft);
    assert_eq!(draft.rdo_code, "039");

    // 4. User edits schedule 1
    draft.schedule_1[0].taxable_amount = 100_000.0;

    // 5. Recompute
    draft.recompute();

    // Assuming PT010 rate is 3%
    assert_eq!(draft.total_tax_due, 3_000.0);
    assert_eq!(draft.tax_payable, 3_000.0);

    // 6. Save Draft
    db.save_2551q_draft(&draft).expect("Failed to save draft");

    // 7. Load Draft and Verify
    let loaded_draft = db
        .get_2551q_draft(&saved_profile.tin.full(), 2026, 1)
        .expect("DB error")
        .expect("Draft not found");

    assert_eq!(loaded_draft.status, FilingStatus::Draft);
    assert_eq!(loaded_draft.total_tax_due, 3_000.0);

    // 8. Queue for submission
    let mut final_draft = loaded_draft;
    final_draft.status = FilingStatus::Queued;
    db.save_2551q_draft(&final_draft)
        .expect("Failed to save queued draft");

    // 9. Verify Queue State
    let queued = db
        .get_2551q_draft(&saved_profile.tin.full(), 2026, 1)
        .expect("DB error")
        .expect("Draft not found");

    assert_eq!(queued.status, FilingStatus::Queued);
}
