use crate::{dummy_profile, samples_dir};
use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt};
use bir_core::forms::form_0605::{
    Form0605ApprovalSelection, Form0605Date, Form0605Draft, Form0605MannerOfPayment,
    Form0605TypeOfPayment,
};
use bir_core::forms::form_0605_official::{form_0605_atc, form_0605_tax_type};
use bir_core::official_xml;
use sha2::{Digest, Sha256};

/// A deficiency-tax installment payment with penalties and rounding cases.
fn sample_0605() -> Form0605Draft {
    let mut draft = Form0605Draft::new_from_profile(&dummy_profile("0605", "Individual"), 2025, 1);
    // The filename carries the clock time the return was made at.
    draft.lifecycle.created_at = "2025-12-01T01:02:03+00:00".into();
    draft.quarter = 4;
    draft.due_date = Form0605Date::new(2026, 1, 25).ok();
    draft.return_period = Form0605Date::new(2025, 12, 31).ok();
    draft.number_of_sheets = 2;
    draft.tax_type = form_0605_tax_type("IT");
    draft.atc = form_0605_atc("II011");
    draft.manner_of_payment =
        Some(Form0605MannerOfPayment::PreliminaryOrFinalAssessmentOrDeficiencyTax);
    draft.approval_selection = Form0605ApprovalSelection::XmlOption1;
    draft.type_of_payment = Some(Form0605TypeOfPayment::Installment);
    draft.number_of_installments = Some(3);
    draft.item_19_basic_tax_or_payment = 12_345.675;
    draft.item_20a_surcharge = 1_234.5;
    draft.item_20b_interest = 99.995;
    draft.item_20c_compromise = 1_000.0;
    draft.recompute();
    draft
}

/// The popup radios (`AtcCode<n>`, `TaxTypeCode<n>`) are added to the page
/// at run time, so they are outside the official layout `check_sample` reads.
/// This repeats its checks with them set aside for the layout read.
fn check_0605_sample(stem: &str, plaintext: &str, official_encrypt_sha256: &str) {
    let layout = official_xml::layout("0605-v2003").expect("official layout");
    let tab = layout.lead.clone();
    let mut laid_out = String::new();
    let mut rest = plaintext;
    while let Some(start) = rest.find("<div>") {
        laid_out.push_str(&rest[..start]);
        let key = &rest[start + 5..rest[start..].find('=').map(|i| start + i).unwrap()];
        let end = rest[start..].find("=</div>").unwrap() + start + "=</div>".len();
        let popup = ["AtcCode", "TaxTypeCode"].iter().any(|prefix| {
            key.strip_prefix(prefix)
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        });
        if popup {
            assert!(rest[end..].starts_with(&tab));
            rest = &rest[end + tab.len()..];
        } else {
            laid_out.push_str(&rest[start..end]);
            rest = &rest[end..];
        }
    }
    laid_out.push_str(rest);
    official_xml::read(layout, &laid_out).expect("sample follows the official layout");

    let encrypted = compress_and_encrypt(plaintext.as_bytes(), BIR_IAF_PASSPHRASE)
        .expect("sample payload must encrypt");
    let dir = samples_dir();
    let plain_path = dir.join(format!("{stem}.plain.xml"));
    let iaf_path = dir.join(format!("{stem}.iaf.xml"));
    let updating = std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some();
    if updating {
        std::fs::write(&plain_path, plaintext).unwrap();
        std::fs::write(&iaf_path, &encrypted).unwrap();
    }
    let official =
        std::fs::read_to_string(dir.join(format!("{stem}.official.xml"))).expect("official sample");
    assert_eq!(
        plaintext, official,
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
    assert_eq!(std::fs::read_to_string(&plain_path).unwrap(), plaintext);
    assert_eq!(std::fs::read(&iaf_path).unwrap(), encrypted);
}

#[test]
fn form_0605_sample_payload_is_current() {
    let payload = sample_0605()
        .to_official_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 0605 must validate: {errors:?}"));
    check_0605_sample(
        "0605-12312025090203",
        &payload,
        "fb64e6e0472c32572841cf412428308687696df02a10d9758ad63898522948d4",
    );
}
