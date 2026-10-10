use crate::dummy_profile;
use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt, decrypt_and_decompress};
use bir_core::forms::form_2550q::{
    FORM_2550Q_LAYOUT_ID, Form2550QDate, Form2550QDraft, Form2550QTaxpayerClassification,
};
use bir_core::forms::queueable::QueueableForm;
use sha2::{Digest, Sha256};

fn sample_2550q() -> Form2550QDraft {
    let mut draft =
        Form2550QDraft::new_from_profile(&dummy_profile("2550Q", "Corporation"), 2025, 1);
    draft.taxpayer_classification = Some(Form2550QTaxpayerClassification::Small);
    draft.part_iv.item_31a_vatable_sales = Some(1_250_000.0);
    draft.part_iv.item_32a_zero_rated_sales = Some(200_000.0);
    draft.part_iv.item_33a_exempt_sales = Some(50_000.0);
    draft.part_iv.item_44a_domestic_purchases = Some(400_000.0);
    // `saveEncryptedProfile` stamps the PC date; the steps pin it to this day.
    draft.date_filed = Form2550QDate::new(2025, 4, 25).ok();
    draft.recompute();
    draft
}

/// [`crate::check_sample`] for 2550Q, whose upload appends
/// `<dateFiled>yyyy/mm/dd</dateFiled>\n` after the layout's trailer.
fn check_sample_2550q(stem: &str, plaintext: &str, official_encrypt_sha256: &str) {
    let (controls, date_filed) = plaintext
        .split_once("<dateFiled>")
        .expect("2550Q upload appends <dateFiled>");
    let date = date_filed
        .strip_suffix("</dateFiled>\n")
        .expect("<dateFiled> closes the upload");
    assert_eq!(date.len(), 10, "dateFiled is yyyy/mm/dd");
    let layout = bir_core::official_xml::layout(FORM_2550Q_LAYOUT_ID).expect("official layout");
    let fields =
        bir_core::official_xml::read(layout, controls).expect("sample follows the official layout");
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

    let dir = crate::samples_dir();
    let plain_path = dir.join(format!("{stem}.plain.xml"));
    let iaf_path = dir.join(format!("{stem}.iaf.xml"));
    let updating = std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some();
    if updating {
        std::fs::write(&plain_path, plaintext).unwrap();
        std::fs::write(&iaf_path, &encrypted).unwrap();
    }
    let official_path = dir.join(format!("{stem}.official.xml"));
    let official = std::fs::read_to_string(&official_path)
        .unwrap_or_else(|e| panic!("missing {}: {e}", official_path.display()));
    assert_eq!(
        plaintext, official,
        "{stem}: our plaintext differs from the official upload (saveEncryptedProfile) output"
    );
    let digest: String = Sha256::digest(&encrypted)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest, official_encrypt_sha256,
        "{stem}: compress_and_encrypt no longer matches the official Encrypt.exe output"
    );
    if updating {
        return;
    }
    assert_eq!(std::fs::read_to_string(&plain_path).unwrap(), plaintext);
    assert_eq!(std::fs::read(&iaf_path).unwrap(), encrypted);
}

#[test]
fn form_2550q_sample_payload_is_current() {
    let draft = sample_2550q();
    // The exact plaintext the generic queue worker uploads.
    let payload = draft
        .official_payload()
        .unwrap_or_else(|errors| panic!("dummy 2550Q must validate: {errors:?}"));
    check_sample_2550q(
        "2550Q-122025Q1",
        &payload,
        "bb3a8b4412d8dc50ae619bd3249f0d47de01418a4a46a358239fb51ee9dd8d3f",
    );
}
