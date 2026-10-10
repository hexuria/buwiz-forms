use crate::{check_sample, dummy_profile};
use bir_core::forms::form_2200m::{Form2200MDraft, Form2200MPayment, Form2200MPlace};

fn sample_2200m() -> Form2200MDraft {
    let mut draft =
        Form2200MDraft::new_from_profile(&dummy_profile("2200M", "Corporation"), 2025, 3);
    draft.day = 14;
    draft.number_of_attached_sheets = 2;
    draft.place_of_production = Form2200MPlace {
        region: "130000000".into(),
        province: "137400000".into(),
        city: "137403000".into(),
    };
    draft.place_of_removal = Form2200MPlace {
        region: "040000000".into(),
        province: "042100000".into(),
        city: "042103000".into(),
    };
    draft.tax_relief = Some(true);
    draft.tax_relief_specify = "Sample relief law".into();
    draft.manner_of_payment = Form2200MPayment::OtherScheme;
    draft.other_scheme_description = "Sample scheme".into();

    let row = draft.row_mut(0).unwrap();
    row.place_of_removal = "Pasig yard".into();
    row.volume_taxable = Some(1_234.567);
    row.volume_exempt = Some(10.0);
    row.local_value_taxable = Some(250_000.0);
    row.local_tax_due = Some(61_728.355);
    row.total_tax_due = Some(61_728.36);
    let row = draft.row_mut(3).unwrap();
    row.place_of_removal = "Rizal quarry".into();
    row.local_value_taxable = Some(999_999.995);
    row.local_value_exempt = Some(0.0);
    row.local_tax_due = Some(40_000.0);
    row.imported_taxable = Some(12_345.6);
    row.imported_exempt = Some(1.0);
    row.imported_tax_due = Some(493.82);
    row.adjustment = Some(0.5);
    row.total_tax_due = Some(40_494.32);
    let row = draft.row_mut(8).unwrap();
    row.description = "Sand and gravel".into();
    row.place_of_removal = "Laguna site".into();
    row.local_rate = Some(4.0);
    row.local_tax_due = Some(5.005);
    row.imported_rate = Some(2.5);
    let row = draft.row_mut(9).unwrap();
    row.description = "Marble".into();
    row.place_of_removal = "Romblon".into();
    row.volume_taxable = Some(3.0);
    row.local_tax_due = Some(120.0);

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
fn form_2200m_sample_payload_is_current() {
    let draft = sample_2200m();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2200M must validate: {errors:?}"));
    check_sample(
        "2200M-03142025",
        "2200m-v2018",
        &payload,
        "932bb1b6427e87b6fe49434fc44c9b31409fcc656ddfc1a51c2ad7d1043a35ea",
    );
}
