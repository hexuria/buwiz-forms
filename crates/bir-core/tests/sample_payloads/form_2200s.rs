use crate::{check_sample, dummy_profile};
use bir_core::forms::excise_places::ExcisePlace;
use bir_core::forms::form_2200s::{Form2200SDraft, Form2200SManner, Form2200SOtherRow};

fn sample_2200s() -> Form2200SDraft {
    let mut draft = Form2200SDraft::new_from_profile(&dummy_profile("2200S", "Corporation"), 2025);
    draft.month = 6;
    draft.day = 30;
    draft.psic = "1104".into();
    draft.place_of_production = ExcisePlace {
        region: "130000000".into(),
        province: "137400000".into(),
        city: "137404000".into(),
    };
    draft.place_of_removal = ExcisePlace {
        region: "040000000".into(),
        province: "043400000".into(),
        city: "043402000".into(),
    };
    draft.manner = Form2200SManner::ActualRemoval;
    draft.recompute();
    draft.beverages[0].sales_value = 98_765.43;
    draft.beverages[0].volume = 12_345.678;
    draft.beverages[2].volume = 10_000.509;
    draft.beverages[8].volume = 100.0;
    draft.others = Some(Form2200SOtherRow {
        atc: "xb200".into(),
        description: "Other sweetened drink".into(),
        tax_bracket: "Per Liter".into(),
        rate: 3.5,
        sales_value: 1_000.0,
        volume: 200.25,
        basic_tax_due: 0.0,
    });
    draft.balance_carried_over = 100.5;
    draft.creditable_excise_tax = 50.0;
    draft.recompute();
    draft.tax_payment = draft.amount_payable;
    draft.recompute();
    draft
}

#[test]
fn form_2200s_sample_payload_is_current() {
    let draft = sample_2200s();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2200-S must validate: {errors:?}"));
    check_sample(
        "2200S-06302025",
        "2200s-v2018",
        &payload,
        "ec1611325d60a52c206ed50cd232125e94cfd8759806964c9338357e6f7871d7",
    );
}
