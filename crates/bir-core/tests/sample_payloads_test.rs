//! Dummy-data sample payloads for the queueable forms.
//!
//! `tests/sample-payloads/` holds what our serializers produce for a fake
//! taxpayer: the plaintext BIR pseudo-XML and the encrypted IAF file that is
//! uploaded (`dump_xml <file>` decrypts it back). No real taxpayer data.
//!
//! `<stem>.official.xml` is what the official eBIRForms page submits when a
//! filer enters `<stem>.steps.json`: `tools/official-xml/runtime.js` runs the
//! official HTA with its own scripts, so the official handlers compute and
//! format every derived amount, then its `saveXMLsubmit()` loop writes the
//! plaintext. Our plaintext must equal it byte for byte, which checks our
//! calculations, value formatting and serialization together. Regenerating our
//! samples never changes it.
//!
//! Regenerate after an intentional serializer change with
//! `UPDATE_SAMPLE_PAYLOADS=1 cargo test -p bir-core --test sample_payloads_test`.

use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt, decrypt_and_decompress};
use bir_core::forms::form_1601c::{Form1601CDraft, Form1601CSchedule1Row};
use bir_core::forms::form_2551q::{Form2551QDraft, Item13Election};
use bir_core::forms::form_2553::{FORM_2553_ATC_OPTIONS, Form2553Draft, Form2553TaxTreaty};
use bir_core::official_xml;
use bir_core::profile::TaxpayerProfile;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// SHA-256 of each sample plaintext encrypted by the official eBIRForms
/// `Encrypt.exe` (sha256 `429337f4…4d2c`, unchanged from 7.9.6.0 through
/// 7.9.6.2.1), run under emulation. Our `compress_and_encrypt` must match
/// it byte for byte; regenerating samples can never change these.
const OFFICIAL_ENCRYPT_EXE_SHA256: [(&str, &str); 3] = [
    (
        "1601C-062025",
        "74731b2485fc21e3ced644988515019ed271a4f7e7b0ac5e9b07a5c82011f16d",
    ),
    (
        "2551Q-122025Q1",
        "fa902b122f7a6dafca86314914279491aeceea8a46c23815178dd7403e820bb9",
    ),
    (
        "2553-122025Q1",
        "4e7a1727b080bc0e3ca7e9b0c52443825ecd83f49889097b32e3bbdfff80428c",
    ),
];

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

fn sample_2553() -> Form2553Draft {
    let mut draft = Form2553Draft::new_from_profile(&dummy_profile("2553", "Corporation"), 2025, 1);
    draft.tax_treaty = Form2553TaxTreaty::SpecialRate;
    draft.set_atc(0, &FORM_2553_ATC_OPTIONS[0]).unwrap();
    draft.set_atc(1, &FORM_2553_ATC_OPTIONS[1]).unwrap();
    draft.set_atc(2, &FORM_2553_ATC_OPTIONS[3]).unwrap();
    draft.schedule[0].taxable_amount = 123_456.789;
    draft.schedule[1].taxable_amount = 2_500_000.0;
    draft.schedule[1].tax_rate = 3.5;
    draft.schedule[2].taxable_amount = 999.995;
    draft.creditable_tax_withheld = 1_000.0;
    draft.surcharge = 25.5;
    draft.interest = 10.0;
    draft.recompute();
    draft
}

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sample-payloads")
}

fn check_sample(stem: &str, form_id: &str, plaintext: &str) {
    let layout = official_xml::layout(form_id).expect("official layout");
    let fields = official_xml::read(layout, plaintext).expect("sample follows the official layout");
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
    let updating = std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some();
    if updating {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&plain_path, plaintext).unwrap();
        std::fs::write(&iaf_path, &encrypted).unwrap();
    }

    let official_plain_path = dir.join(format!("{stem}.official.xml"));
    let official_plain = std::fs::read_to_string(&official_plain_path)
        .unwrap_or_else(|e| panic!("missing {}: {e}", official_plain_path.display()));
    assert_eq!(
        plaintext, official_plain,
        "{stem}: our plaintext differs from the official saveXMLsubmit() output"
    );

    let official = OFFICIAL_ENCRYPT_EXE_SHA256
        .iter()
        .find(|(name, _)| *name == stem)
        .map(|(_, sha)| *sha);
    if let Some(official) = official {
        let digest: String = Sha256::digest(&encrypted)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            digest, official,
            "{stem}: compress_and_encrypt no longer matches the official Encrypt.exe output"
        );
    }

    if updating {
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
    check_sample("1601C-062025", "1601c-v2018", &payload);
}

#[test]
fn form_2551q_sample_payload_is_current() {
    let draft = sample_2551q();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2551Q must validate: {errors:?}"));
    check_sample("2551Q-122025Q1", "2551q-v2018", &payload);
}

#[test]
fn form_2553_sample_payload_is_current() {
    let draft = sample_2553();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2553 must validate: {errors:?}"));
    check_sample("2553-122025Q1", "2553-v1999", &payload);
}
