use crate::{check_sample, dummy_profile};
use bir_core::forms::form_2200p::{Form2200PDraft, Form2200PPayment, Form2200PPlace};

fn sample_2200p() -> Form2200PDraft {
    let mut draft =
        Form2200PDraft::new_from_profile(&dummy_profile("2200P", "Corporation"), 2025, 3);
    draft.day = 14;
    draft.number_of_attached_sheets = 2;
    draft.place_of_production = Form2200PPlace {
        region: "130000000".into(),
        province: "137400000".into(),
        city: "137403000".into(),
    };
    draft.place_of_removal = Form2200PPlace {
        region: "040000000".into(),
        province: "042100000".into(),
        city: "042103000".into(),
    };
    draft.tax_relief = Some(true);
    draft.tax_relief_specify = "Sample relief law".into();
    draft.manner_of_payment = Form2200PPayment::OtherScheme;
    draft.other_scheme_description = "Sample scheme".into();

    // Row 6 — XP060 unleaded premium gasoline.
    let row = draft.row_mut(5).unwrap();
    row.place_of_removal = "Batangas depot".into();
    row.local_manufactured = Some(10_000.005);
    row.imported = Some(2_500.0);
    row.total_taxable_removals = Some(12_500.01);
    row.basic_excise_tax_due = Some(125_000.1);
    // Row 16 — XP140 diesel.
    let row = draft.row_mut(15).unwrap();
    row.place_of_removal = "Pasig terminal".into();
    row.under_bond = Some(100.0);
    row.exports = Some(0.0);
    row.total_tax_paid = Some(100.0);
    row.basic_excise_tax_due = Some(999.995);
    // Row 26 — others.
    let row = draft.row_mut(25).unwrap();
    row.atc_digits = "999".into();
    row.description = "Other fuel".into();
    row.unit_of_measure = "Per liter".into();
    row.applicable_rate = Some(2.5);
    row.place_of_removal = "Bataan".into();
    row.imported = Some(40.0);
    row.basic_excise_tax_due = Some(100.0);

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
fn form_2200p_sample_payload_is_current() {
    let draft = sample_2200p();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2200P must validate: {errors:?}"));
    check_sample(
        "2200P-03142025",
        "2200p-v2020",
        &payload,
        "a6d040ab3d2694e713b6998691f0acded45340e1836475d149cde07af9dc1c81",
    );
}
