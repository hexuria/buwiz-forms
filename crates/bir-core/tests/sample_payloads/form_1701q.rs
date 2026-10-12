use crate::{dummy_profile, samples_dir};
use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt};
use bir_core::forms::form_1701q::{
    Form1701QAtc, Form1701QDeductionMethod, Form1701QDraft, Form1701QFilerType, Form1701QParty,
    Form1701QSpouseType,
};
use bir_core::official_xml;
use sha2::{Digest, Sha256};

/// A professional on graduated rates (itemized) with a spouse on the 8%
/// rate, an amended return, a loss on Item 42 and centavos to round.
fn sample_1701q() -> Form1701QDraft {
    let mut draft =
        Form1701QDraft::new_from_profile(&dummy_profile("1701Qv2018", "Individual"), 2025, 2);
    let t = Form1701QParty::Taxpayer;
    let s = Form1701QParty::Spouse;
    draft.is_amended = true;
    draft.filer_type = Some(Form1701QFilerType::Professional);
    draft.atc = Some(Form1701QAtc::Ii014);
    draft.deduction_method = Some(Form1701QDeductionMethod::Itemized);
    draft.date_of_birth = "05/17/1985".into();
    draft.citizenship = "Filipino".into();
    draft.claims_foreign_tax_credits = Some(false);
    draft.has_spouse = true;
    draft.spouse_tin = "987-654-321-00000".into();
    draft.spouse_rdo_code = "040".into();
    draft.spouse_type = Some(Form1701QSpouseType::SingleProprietor);
    draft.spouse_atc = Some(Form1701QAtc::Ii015);
    draft.spouse_name = "Dummy Spouse Sample".into();
    draft.spouse_citizenship = "Filipino".into();
    draft.spouse_claims_foreign_tax_credits = Some(true);
    draft.spouse_foreign_tax_number = "x12345".into();
    draft.item_43_non_operating_income_description = "Rental income".into();
    draft.item_48_non_operating_income_description = "Interest".into();
    draft.item_61_other_tax_credit_description = "Other credit".into();
    for (item, party, value) in [
        (36, t, 1_234_567.0),
        (37, t, 345_678.0),
        (39, t, 123_456.0),
        (42, t, -1_000.505),
        (43, t, 50_000.0),
        (44, t, 7_000.0),
        (55, t, 10_000.0),
        (59, t, 2_500.0),
        (61, t, 300.0),
        (64, t, 1_000.0),
        (65, t, 250.0),
        (47, s, 900_000.0),
        (48, s, 12_345.0),
        (50, s, 15_000.505),
        (52, s, 250_000.0),
        (56, s, 4_000.0),
        (66, s, 500.0),
    ] {
        draft.set_amount(item, party, Some(value));
    }
    draft.recompute();
    draft
}

/// Checks the 1701Q sample against the official upload.
#[test]
fn form_1701q_sample_payload_is_current() {
    const STEM: &str = "1701Q-2025Q2";
    let payload = sample_1701q()
        .to_official_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1701Q must validate: {errors:?}"));
    let layout = official_xml::layout("1701q-v2018").unwrap();
    official_xml::read(layout, &payload).expect("the sample follows the official layout");

    let encrypted = compress_and_encrypt(payload.as_bytes(), BIR_IAF_PASSPHRASE).unwrap();
    let dir = samples_dir();
    if std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some() {
        std::fs::write(dir.join(format!("{STEM}.plain.xml")), &payload).unwrap();
        std::fs::write(dir.join(format!("{STEM}.iaf.xml")), &encrypted).unwrap();
    }
    let official = std::fs::read_to_string(dir.join(format!("{STEM}.official.xml"))).unwrap();
    assert_eq!(
        payload, official,
        "{STEM}: differs from the official saveXMLsubmit() output"
    );
    let digest: String = Sha256::digest(&encrypted)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest, "7a1baccf54e3c9e98b9e8ce172fc47f8e71deb7ed9a8d0e58fb48dcc8dee7a12",
        "{STEM}: official Encrypt.exe hash"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join(format!("{STEM}.plain.xml"))).unwrap(),
        payload
    );
    assert_eq!(
        std::fs::read(dir.join(format!("{STEM}.iaf.xml"))).unwrap(),
        encrypted
    );
}
