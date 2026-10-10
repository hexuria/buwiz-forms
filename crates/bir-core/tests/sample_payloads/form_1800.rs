use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1800::{
    Form1800Donee, Form1800Draft, Form1800PersonalProperty, Form1800RealProperty,
};

fn sample_1800() -> Form1800Draft {
    let mut draft = Form1800Draft::new_from_profile(&dummy_profile("1800", "Individual"), 2025, 1);
    draft.donation_month = 3;
    draft.donation_day = 10;
    draft.donation_year = 2025;
    draft.number_of_attached_sheets = 1;
    draft.donees = vec![
        Form1800Donee {
            name: "Maria Sample-Donee".into(),
            tin: "111222333000".into(),
        },
        Form1800Donee {
            name: "Pedro Example".into(),
            tin: "444555666".into(),
        },
    ];
    draft.tax_relief = Some("Special law relief".into());
    draft.personal_properties = vec![
        Form1800PersonalProperty {
            particulars: "Sedan, 2020 model".into(),
            fair_market_value: 750_000.005,
        },
        Form1800PersonalProperty {
            particulars: "Shares of Sample Corp".into(),
            fair_market_value: 1_234_567.894,
        },
    ];
    draft.real_properties = vec![Form1800RealProperty {
        title_number: "T-98765".into(),
        tax_declaration_number: "TD-0001".into(),
        location: "Lot 1, Sample Subd., Quezon City".into(),
        lot_or_improvement: "Lot".into(),
        classification: "RR".into(),
        area: "150".into(),
        fmv_per_tax_declaration: "900000".into(),
        fmv_per_zonal_value: "1200000".into(),
        fair_market_value: 1_200_000.0,
    }];
    draft.prior_gift_payments = 1_000.0;
    draft.surcharge = 25.5;
    draft.interest = 10.005;
    draft.recompute();
    draft
}

#[test]
fn form_1800_sample_payload_is_current() {
    let draft = sample_1800();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1800 must validate: {errors:?}"));
    check_sample(
        "1800-03102025",
        "1800-v2018",
        &payload,
        "20093e87af16bb3fab96209bccabe763a6f7e4146b7c01726df28df188ce5f23",
    );
}
