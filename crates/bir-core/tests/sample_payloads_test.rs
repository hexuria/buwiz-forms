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
use bir_core::official_xml;
use bir_core::profile::TaxpayerProfile;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// One module per form: its dummy draft and its sample test.
mod sample_payloads;

pub(crate) fn dummy_profile(default_form_type: &str, taxpayer_type: &str) -> TaxpayerProfile {
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

pub(crate) fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sample-payloads")
}

/// `official_encrypt_sha256` is the SHA-256 of `<stem>.plain.xml` encrypted by the
/// official eBIRForms `Encrypt.exe` (sha256 `429337f4…4d2c`, unchanged from
/// 7.9.6.0 through 7.9.6.2.1) run under emulation; our `compress_and_encrypt`
/// must match it byte for byte. Regenerating samples can never change it.
pub(crate) fn check_sample(
    stem: &str,
    form_id: &str,
    plaintext: &str,
    official_encrypt_sha256: &str,
) {
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
