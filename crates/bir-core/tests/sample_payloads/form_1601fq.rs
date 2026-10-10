use crate::dummy_profile;
use crate::sample_payloads::form_1601eq::check_runtime_sample;
use bir_core::forms::form_1601fq::{Form1601FqCategory, Form1601FqDraft, Form1601FqTaxRelief};

fn sample_1601fq() -> Form1601FqDraft {
    let mut draft =
        Form1601FqDraft::new_from_profile(&dummy_profile("1601FQ", "Corporation"), 2025, 2);
    draft.set_any_tax_withheld(true);
    draft.set_category(Form1601FqCategory::Private);
    draft.set_tax_relief(Some(Form1601FqTaxRelief::InternationalTaxTreaty));
    // Popup positions; WC212 is listed twice (30% at 15, 25% at 29).
    for index in [29, 0, 15, 2, 28] {
        draft.add_atc(index).unwrap();
    }
    for (row, base) in
        draft
            .schedule
            .iter_mut()
            .zip([123_456.789, 1_000.005, 50_000.0, 77_777.775, 999.99])
    {
        row.tax_base = base;
    }
    draft.schedule1[0].treaty_code = "US".into();
    draft.set_schedule1_atc(0, "WI330");
    draft.schedule1[0].income_payment = 100_000.005;
    draft.schedule1[0].tax_rate = 15.0;
    draft.schedule1[1].treaty_code = "JP".into();
    draft.set_schedule1_atc(1, "WC180");
    draft.schedule1[1].income_payment = 50_000.0;
    draft.remittance_first_month = 500.0;
    draft.remittance_second_month = 1_000.505;
    draft.surcharge = 25.5;
    draft.interest = 10.0;
    draft.recompute();
    draft
}

#[test]
fn form_1601fq_sample_payload_is_current() {
    let draft = sample_1601fq();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1601FQ must validate: {errors:?}"));
    check_runtime_sample(
        "1601FQ-2025Q2",
        &draft.official_layout().unwrap(),
        &payload,
        "9be19d6281ad0a61f50f576e15f43834a31cce713b651db8aafcda0b4c9724fe",
    );
}
