use crate::{check_sample, dummy_profile};
use bir_core::forms::form_2200a::{Form2200ADraft, Form2200APayment, Form2200APlace};

fn sample_2200a() -> Form2200ADraft {
    let mut draft =
        Form2200ADraft::new_from_profile(&dummy_profile("2200A", "Corporation"), 2025, 3);
    draft.day = 14;
    draft.number_of_attached_sheets = 1;
    draft.place_of_production = Form2200APlace {
        region: "130000000".into(),
        province: "137400000".into(),
        city: "137403000".into(),
    };
    draft.place_of_removal = Form2200APlace {
        region: "040000000".into(),
        province: "042100000".into(),
        city: "042103000".into(),
    };
    draft.tax_relief = true;
    draft.tax_relief_specify = "Sample relief law".into();
    draft.manner_of_payment = Form2200APayment::OtherScheme;
    draft.other_scheme_description = "Sample scheme".into();

    // Ad valorem 22% on a taxable base with a half-centavo product.
    let row = draft.row_by_key_mut("1A_2024").unwrap();
    row.export_exempt = 50.0;
    row.taxable = 10_000.125;
    // 21% from xml/taxRate.xml (the page itself prints 22%).
    draft.row_by_key_mut("1A_2020B").unwrap().taxable = 1_234.56;
    // Specific tax: 66.00 per proof liter.
    draft.row_by_key_mut("1B_2024").unwrap().taxable = 1_234.567;
    // Fermented liquors XA056 (row key 3B_2020B2 on the official page).
    draft.row_by_key_mut("3B_2020B2").unwrap().taxable = 99.99;
    let other = draft.other_mut(0).unwrap();
    other.atc_digits = "999".into();
    other.description = "Sample other product".into();
    other.bracket = "Per Liter".into();
    other.rate = "5.5".into();
    other.amounts.taxable = 2.0;
    other.amounts.tax_due = 11.0;

    draft.balance_carried_over = 1_000.0;
    draft.creditable_excise_tax = 234.565;
    draft.surcharge = 25.5;
    draft.interest = 10.0;
    draft.compromise = 1_000.0;
    draft.recompute();
    draft.tax_deposit = draft.tax_still_due + 100.0;
    draft.recompute();
    draft
}

#[test]
fn form_2200a_sample_payload_is_current() {
    let draft = sample_2200a();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2200A must validate: {errors:?}"));
    check_sample(
        "2200A-03142025",
        "2200a-v2020",
        &payload,
        "80270137fded0ec31f50299a555a077dbba2992efe37f23b58f6a446b3f6c61b",
    );
}
