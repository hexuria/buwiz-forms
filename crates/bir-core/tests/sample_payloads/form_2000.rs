use crate::dummy_profile;
use bir_core::forms::form_2000::{
    Form2000AffixtureMode, Form2000Draft, Form2000OtherParty, Form2000PaymentRow,
    Form2000RemittanceRow, form_2000_atc_option,
};

fn sample_2000() -> Form2000Draft {
    let mut draft =
        Form2000Draft::new_from_profile(&dummy_profile("2000v2018", "Corporation"), 2025, 6);
    // A creditor with a mixed-case name: Item 11's blur runs capital().
    draft.other_party = Form2000OtherParty::Creditor;
    draft.other_party_name = "Sample Creditor Corp".into();
    draft.other_party_tin = "987654321000".into();
    draft.affixture_mode = Form2000AffixtureMode::Constructive;
    let pick = |code, description| form_2000_atc_option(code, description).unwrap();
    draft
        .set_atc(0, pick("DS101", "ORIGINAL ISSUE OF SHARES OF STOCKS"))
        .unwrap();
    draft
        .set_atc(1, pick("DS112", "ON PRE-NEED PLANS"))
        .unwrap();
    draft
        .set_atc(2, pick("DS106", "ORIGINAL ISSUE OF ALL DEBT INSTRUMENTS"))
        .unwrap();
    draft.schedule1[0].tax_base = 123_456.789;
    draft.schedule1[1].tax_base = 999.995;
    draft.schedule1[2].tax_base = 2_500_000.0;
    draft.ds106_term_under_a_year = Some(true);
    draft.ds106_term_days = 90;
    draft.schedule2 = vec![
        Form2000PaymentRow {
            date: "06/05/2025".into(),
            receipt_number: "Rcpt1001".into(),
            amount: 1_000.5,
        },
        Form2000PaymentRow {
            date: "06/20/2025".into(),
            receipt_number: "Rcpt1002".into(),
            amount: 2_000.005,
        },
    ];
    draft.schedule4 = vec![Form2000RemittanceRow {
        rco_code: "Rco01".into(),
        date: "06/25/2025".into(),
        bank: "Sample Bank".into(),
        amount: 5_000.0,
        number_from: "1001".into(),
        number_to: "1100".into(),
    }];
    draft.surcharge = 25.5;
    draft.interest = 10.0;
    draft.recompute();
    draft
}

#[test]
fn form_2000_sample_payload_is_current() {
    let draft = sample_2000();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2000 must validate: {errors:?}"));
    check_sample_with_mod_labels(
        "2000-062025",
        &payload,
        "9f3340aa0de65ee6dfdcd95912d2538aaeedb93f190045c5e9c84d14611687dd",
    );
}

/// `check_sample` for this page. It has four read-only `frm2000:modLabel`
/// boxes with different texts under one id, which `official_xml::read`
/// rejects as one control written twice with different values; the layout
/// check runs on a copy with those four values equalized, and everything
/// else (official parity, IAF crypto, committed files) is unchanged.
fn check_sample_with_mod_labels(stem: &str, plaintext: &str, official_encrypt_sha256: &str) {
    use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt};
    use sha2::{Digest, Sha256};

    let open = "<div>frm2000:modLabel=";
    let close = "frm2000:modLabel=</div>";
    let mut normalized = String::new();
    let mut rest = plaintext;
    while let Some(start) = rest.find(open) {
        let end = start + rest[start..].find(close).expect("closed modLabel");
        normalized.push_str(&rest[..start + open.len()]);
        normalized.push_str("LABEL");
        rest = &rest[end..];
    }
    normalized.push_str(rest);
    let layout = bir_core::official_xml::layout("2000-v2018").expect("official layout");
    bir_core::official_xml::read(layout, &normalized).expect("sample follows the official layout");

    let dir = crate::samples_dir();
    let encrypted = compress_and_encrypt(plaintext.as_bytes(), BIR_IAF_PASSPHRASE).unwrap();
    let plain_path = dir.join(format!("{stem}.plain.xml"));
    let iaf_path = dir.join(format!("{stem}.iaf.xml"));
    let updating = std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some();
    if updating {
        std::fs::write(&plain_path, plaintext).unwrap();
        std::fs::write(&iaf_path, &encrypted).unwrap();
    }
    let official = std::fs::read_to_string(dir.join(format!("{stem}.official.xml"))).unwrap();
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
