use crate::dummy_profile;
use bir_core::forms::form_1601eq::{Form1601EqCategory, Form1601EqDraft, form_1601eq_atc_option};

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

#[test]
fn form_1601eq_sample_payload_is_current() {
    let draft = sample_1601eq();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1601EQ must validate: {errors:?}"));
    crate::check_sample_with_layout(
        "1601EQ-2025Q1",
        &draft.official_layout().unwrap(),
        &payload,
        "36cabf574ab41191c721f5ec77e0b3ed0252fceaa9414fa5d35a415c434a343b",
    );
}
