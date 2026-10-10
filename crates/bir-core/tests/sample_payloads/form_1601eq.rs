use crate::{dummy_profile, samples_dir};
use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt, decrypt_and_decompress};
use bir_core::forms::form_1601eq::{Form1601EqCategory, Form1601EqDraft, form_1601eq_atc_option};
use bir_core::official_xml::{self, OfficialLayout};
use sha2::{Digest, Sha256};

fn sample_1601eq() -> Form1601EqDraft {
    let mut draft =
        Form1601EqDraft::new_from_profile(&dummy_profile("1601EQ", "Corporation"), 2025, 1);
    draft.set_any_tax_withheld(true);
    draft.set_category(Form1601EqCategory::Private);
    // Eight ATCs: six on page 1 and two "Other Selected ATC" rows.
    let bases = [
        ("WC160", 75_432.1),
        ("WI010", 123_456.789),
        ("WI100", 1_000.005),
        ("WI120", 999.995),
        ("WI157", 50_000.0),
        ("WI158", 250_000.0),
        ("WI160", 12_345.675),
        ("WC010", 88_888.88),
    ];
    for (code, _) in bases {
        draft.add_atc(code).unwrap();
    }
    for (code, base) in bases {
        let row = draft
            .schedule
            .iter_mut()
            .find(|row| row.atc_code == code)
            .unwrap();
        row.tax_base = base;
    }
    draft.remittance_first_month = 1_000.0;
    draft.remittance_second_month = 2_000.505;
    draft.surcharge = 25.5;
    draft.interest = 10.0;
    draft.recompute();
    assert!(form_1601eq_atc_option("WC160").is_some());
    draft
}

/// [`crate::check_sample`] over a layout that carries the controls the
/// official page draws at run time (ATC rows and popup checkboxes).
pub(crate) fn check_runtime_sample(
    stem: &str,
    layout: &OfficialLayout,
    plaintext: &str,
    official_encrypt_sha256: &str,
) {
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
fn form_1601eq_sample_payload_is_current() {
    let draft = sample_1601eq();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1601EQ must validate: {errors:?}"));
    check_runtime_sample(
        "1601EQ-2025Q1",
        &draft.official_layout().unwrap(),
        &payload,
        "62a53602f3123d037375d02a9439fd3b1e553ff938c746ff703d1d262a84cefe",
    );
}
