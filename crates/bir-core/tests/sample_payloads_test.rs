//! Dummy-data sample payloads for the queueable forms.
//!
//! `tests/sample-payloads/` holds what our serializers produce for a fake
//! taxpayer: the plaintext BIR pseudo-XML and the encrypted IAF file that is
//! uploaded (`dump_xml <file>` decrypts it back). No real taxpayer data.
//!
//! Regenerate after an intentional serializer change with
//! `UPDATE_SAMPLE_PAYLOADS=1 cargo test -p bir-core --test sample_payloads_test`.

use bir_core::bir_xml::parse_bir_xml_checked;
use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt, decrypt_and_decompress};
use bir_core::forms::form_1601c::{Form1601CDraft, Form1601CSchedule1Row};
use bir_core::forms::form_2551q::{Form2551QDraft, Item13Election};
use bir_core::profile::TaxpayerProfile;
use std::path::PathBuf;

fn dummy_profile(default_form_type: &str, taxpayer_type: &str) -> TaxpayerProfile {
    // 123-456-788 passes the official check digit; every value is fictitious.
    serde_json::from_value(serde_json::json!({
        "id": null,
        "full_name": "Sample Dummy Taxpayer",
        "tin": {
            "segment1": "123",
            "segment2": "456",
            "segment3": "788",
            "branch": "00000"
        },
        "rdo_code": "039",
        "line_of_business": "Sample Consulting Services",
        "registered_address": "123 Sample Street, Barangay Example, Quezon City",
        "zip_code": "1100",
        "phone": "09170000000",
        "email": "sample.taxpayer@example.com",
        "default_form_type": default_form_type,
        "taxpayer_type": taxpayer_type,
        "business_start_date": "2020-01-15",
        "tax_elections": [{
            "taxable_year": 2025,
            "election": "GraduatedUnspecified",
            "elected_at": "2025-01-15T00:00:00",
            "source_form": "sample"
        }]
    }))
    .expect("dummy profile must deserialize")
}

fn sample_1601c() -> Form1601CDraft {
    let mut draft =
        Form1601CDraft::new_from_profile(&dummy_profile("1601Cv2018", "Corporation"), 2025, 6);
    draft.auto_compute_penalties = false;
    draft.tax_14_total_compensation = 250_000.0;
    draft.tax_15_statutory_minimum_wage = 0.0;
    draft.tax_17_13th_month_pay = 20_000.0;
    draft.tax_18_de_minimis = 5_000.0;
    draft.tax_19_sss_gsis = 12_500.0;
    draft.tax_25_total_taxes_withheld = 18_750.0;
    draft.schedule_1 = vec![Form1601CSchedule1Row {
        previous_month: "04/2025".to_string(),
        date_paid: "05/09/2025".to_string(),
        drawee_bank_code_or_agency: "SAMPLE BANK".to_string(),
        payment_number: "REF-0001".to_string(),
        tax_paid: 15_000.0,
        should_be_tax_due: 15_250.0,
        adjustment: 0.0,
    }];
    draft.compute();
    draft
}

fn sample_2551q() -> Form2551QDraft {
    let mut draft =
        Form2551QDraft::new_from_profile(&dummy_profile("2551Qv2018", "Individual"), 2025, 1);
    draft.item_13_election = Item13Election::Graduated;
    draft.auto_compute_penalties = false;
    draft.surcharge = 0.0;
    draft.interest = 0.0;
    draft.compromise = 0.0;
    draft.schedule_1[0].taxable_amount = 180_000.0;
    draft.creditable_tax_withheld = 500.0;
    draft.recompute(None);
    draft
}

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sample-payloads")
}

fn check_sample(stem: &str, plaintext: &str) {
    let fields = parse_bir_xml_checked(plaintext).expect("sample payload must parse");
    assert!(!fields.is_empty());

    let encrypted = compress_and_encrypt(plaintext.as_bytes(), BIR_IAF_PASSPHRASE)
        .expect("sample payload must encrypt");
    let decrypted =
        decrypt_and_decompress(&encrypted, BIR_IAF_PASSPHRASE).expect("sample must decrypt");
    assert_eq!(
        decrypted,
        plaintext.as_bytes(),
        "encryption must round-trip"
    );

    let dir = samples_dir();
    let plain_path = dir.join(format!("{stem}.plain.xml"));
    let iaf_path = dir.join(format!("{stem}.iaf.xml"));
    if std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some() {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&plain_path, plaintext).unwrap();
        std::fs::write(&iaf_path, &encrypted).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&plain_path)
        .unwrap_or_else(|e| panic!("missing {}: {e}", plain_path.display()));
    assert_eq!(
        committed, plaintext,
        "{stem}: serializer output changed; rerun with UPDATE_SAMPLE_PAYLOADS=1 if intended"
    );
    assert_eq!(
        std::fs::read(&iaf_path).unwrap(),
        encrypted,
        "{stem}: encrypted sample is stale"
    );
}

#[test]
fn form_1601c_sample_payload_is_current() {
    let draft = sample_1601c();
    let payload = draft
        .try_to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1601C must validate: {errors:?}"));
    check_sample("1601C-062025", &payload);
}

#[test]
fn form_2551q_sample_payload_is_current() {
    let draft = sample_2551q();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2551Q must validate: {errors:?}"));
    check_sample("2551Q-122025Q1", &payload);
}
