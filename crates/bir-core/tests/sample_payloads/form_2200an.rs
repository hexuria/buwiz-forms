use crate::{check_sample, dummy_profile};
use bir_core::forms::excise_places::ExcisePlace;
use bir_core::forms::form_2200an::{
    Form2200ANDraft, Form2200ANManner, Form2200ANOtherGood, Form2200ANRow,
};

fn sample_2200an() -> Form2200ANDraft {
    let mut draft =
        Form2200ANDraft::new_from_profile(&dummy_profile("2200ANv2018", "Corporation"), 2025);
    draft.month = 6;
    draft.day = 30;
    draft.is_amended = true;
    draft.previous_payment = 1_000.0;
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
    draft.tax_relief = Some(true);
    draft.tax_relief_specify = "Sample Relief".into();
    draft.manner = Form2200ANManner::Other;
    draft.manner_other = "Sample scheme".into();
    let row = |eu: &str, tu: &str, ev: f64, tv: f64, due: f64| Form2200ANRow {
        exempt_units: eu.into(),
        taxable_units: tu.into(),
        exempt_value: ev,
        taxable_value: tv,
        tax_due: due,
    };
    draft.recompute();
    draft.automobiles[0] = row("", "2", 0.0, 1_000_000.0, 40_000.0);
    // XG033 is the seventh automobile row.
    draft.automobiles[6] = row("1", "3", 2_500_000.0, 7_500_000.0, 1_500_000.005);
    draft.goods[1] = row("", "10", 0.0, 250_000.5, 50_000.1);
    draft.other_goods = vec![Form2200ANOtherGood {
        atc: "XG130".into(),
        name: "Sample luxury item".into(),
        rate: "20%".into(),
        row: row("", "1", 0.0, 10_000.0, 2_000.0),
    }];
    draft.balance_carried_over = 500.0;
    draft.creditable_excise_tax = 250.25;
    draft.surcharge = 100.0;
    draft.interest = 12.345;
    draft.tax_payment = 1_500_000.0;
    draft.recompute();
    draft
}

#[test]
fn form_2200an_sample_payload_is_current() {
    let draft = sample_2200an();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2200-AN must validate: {errors:?}"));
    check_sample(
        "2200AN-06302025",
        "2200an-v2018",
        &payload,
        "5010bd988e28db88326cbe1d147a7108ea8a039750fa3c161c79f47989544e14",
    );
}
