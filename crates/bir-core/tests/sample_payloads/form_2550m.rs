use crate::{check_sample, dummy_profile, samples_dir};
use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt, decrypt_and_decompress};
use bir_core::forms::form_2550m::{Form2550MAllocation, Form2550MDraft, Form2550MPurchase};
use bir_core::official_xml;
use sha2::{Digest, Sha256};

/// No Schedule 1 rows: the plaintext is exactly the fixed official layout.
fn sample_2550m() -> Form2550MDraft {
    let mut draft =
        Form2550MDraft::new_from_profile(&dummy_profile("2550M", "Corporation"), 2022, 6);
    draft.is_amended = true;
    draft.set_sales_to_government(250_000.005);
    draft.zero_rated_sales = 40_000.0;
    draft.exempt_sales = 12_345.67;
    draft.input_tax_carried_over = 1_500.25;
    draft.transitional_input_tax = 99.99;
    draft.set_purchase(Form2550MPurchase::DomesticGoods, 20_000.0);
    draft.set_purchase(Form2550MPurchase::ImportedGoods, 8_888.88);
    draft.set_purchase(Form2550MPurchase::DomesticServices, 3_333.33);
    draft.set_purchase(Form2550MPurchase::NonResidentServices, 1_000.0);
    draft.purchases_not_qualified = 500.0;
    draft.set_purchase(Form2550MPurchase::Others, 777.77);
    draft.schedule_4 = Some(Form2550MAllocation {
        direct_input_tax: 18_000.0,
        not_direct_input_tax: 1_200.0,
    });
    draft.schedule_5 = Some(Form2550MAllocation {
        direct_input_tax: 100.0,
        not_direct_input_tax: 300.0,
    });
    draft.vat_refund_claimed = 10.0;
    draft.vat_paid_previous = 1_000.0;
    draft.advance_payments = 200.0;
    draft.surcharge = 25.5;
    draft.interest = 10.0;
    draft.recompute();
    draft
}

/// Schedule 1 rows (vatable sales per ATC), spliced ahead of the Schedule 1 totals.
fn sample_2550m_schedule_1() -> Form2550MDraft {
    let mut draft =
        Form2550MDraft::new_from_profile(&dummy_profile("2550M", "Corporation"), 2022, 7);
    draft.add_sales_atc("VT010").unwrap();
    draft.add_sales_atc("VB010").unwrap();
    draft.add_sales_atc("VS062").unwrap();
    // Popup order: VB010, VT010, VS062.
    draft.sales_schedule[0].amount = 2_345.67;
    draft.sales_schedule[1].amount = 100_000.005;
    draft.sales_schedule[2].amount = 55_555.55;
    draft.zero_rated_sales = 1_000.0;
    draft.input_tax_carried_over = 300.0;
    draft.set_purchase(Form2550MPurchase::DomesticGoods, 15_000.0);
    draft.other_credits = 50.0;
    draft.recompute();
    draft
}

#[test]
fn form_2550m_sample_payload_is_current() {
    let payload = sample_2550m()
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2550M must validate: {errors:?}"));
    check_sample(
        "2550M-062022",
        "2550m-v2007",
        &payload,
        "5d6d66d8005aa20bf2709d317204c5b9d0b508352dd31ffbf98fa94ab1f04102",
    );
}

/// `check_sample` for a plaintext with Schedule 1 rows: the rows are not part
/// of the fixed layout, so the layout is checked with them taken out.
#[test]
fn form_2550m_schedule_1_sample_payload_is_current() {
    let stem = "2550M-072022";
    let plaintext = sample_2550m_schedule_1()
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2550M must validate: {errors:?}"));
    let layout = official_xml::layout("2550m-v2007").unwrap();
    let mut fixed = plaintext.clone();
    for n in 1..=3 {
        for id in ["txtAtcCde", "txtAmountSales", "txtOutputTax"] {
            let key = format!("frm2550m:{id}{n}");
            let start = fixed.find(&format!("<div>{key}=")).expect("row control");
            let end = start + fixed[start..].find("</div>").unwrap() + "</div>".len();
            fixed.replace_range(start..end + layout.lead.len(), "");
        }
    }
    assert!(!fixed.contains("txtAtcCde"));
    official_xml::read(layout, &fixed).expect("the rest follows the official layout");

    let encrypted = compress_and_encrypt(plaintext.as_bytes(), BIR_IAF_PASSPHRASE).unwrap();
    assert_eq!(
        decrypt_and_decompress(&encrypted, BIR_IAF_PASSPHRASE).unwrap(),
        plaintext.as_bytes()
    );
    let dir = samples_dir();
    let plain_path = dir.join(format!("{stem}.plain.xml"));
    let iaf_path = dir.join(format!("{stem}.iaf.xml"));
    let updating = std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some();
    if updating {
        std::fs::write(&plain_path, &plaintext).unwrap();
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
        digest, "537d9efdc32b81d56f13f292397a868253a169312a86b2499dfdb699d0ec0f17",
        "{stem}: compress_and_encrypt no longer matches the official Encrypt.exe output"
    );
    if !updating {
        assert_eq!(std::fs::read_to_string(&plain_path).unwrap(), plaintext);
        assert_eq!(std::fs::read(&iaf_path).unwrap(), encrypted);
    }
}
