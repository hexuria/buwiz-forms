use crate::{check_sample, dummy_profile};
use bir_core::forms::form_2553::{FORM_2553_ATC_OPTIONS, Form2553Draft, Form2553TaxTreaty};

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

#[test]
fn form_2553_sample_payload_is_current() {
    let draft = sample_2553();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2553 must validate: {errors:?}"));
    check_sample(
        "2553-122025Q1",
        "2553-v1999",
        &payload,
        "4e7a1727b080bc0e3ca7e9b0c52443825ecd83f49889097b32e3bbdfff80428c",
    );
}
