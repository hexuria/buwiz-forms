use crate::{check_sample, dummy_profile};
use bir_core::forms::form_2000ot::{
    Form2000OTAtc, Form2000OTDraft, Form2000OTNature, Form2000OTOtherParty, Form2000OTPropertyRow,
};

fn sample_2000ot() -> Form2000OTDraft {
    let mut draft =
        Form2000OTDraft::new_from_profile(&dummy_profile("2000OTv2018", "Corporation"), 2025);
    draft.transaction_date = "06/15/2025".into();
    draft.is_amended = true;
    draft.tax_paid_previous = 1_000.0;
    draft.atc = Form2000OTAtc::Do122;
    draft.other_party = Form2000OTOtherParty::Creditor;
    draft.other_party_name = "Sample Buyer Corp".into();
    draft.other_party_tin = "987654321000".into();
    draft.nature = Form2000OTNature::RealPropertyCapitalAsset;
    draft.real_property_location = "Lot 1 Block 2, Sample Subdivision, Quezon City".into();
    draft.properties = vec![
        Form2000OTPropertyRow {
            title_number: "Tct-12345".into(),
            tax_declaration_number: "Td-0001".into(),
            location: "Barangay Example".into(),
            lot: "Lot 1".into(),
            classification: "Residential".into(),
            area: "250".into(),
            fmv_per_td: 1_250_000.005,
            fmv_zonal: 1_300_000.0,
            fmv: 0.0,
        },
        Form2000OTPropertyRow {
            title_number: "Tct-12346".into(),
            tax_declaration_number: "Td-0002".into(),
            location: "Barangay Example".into(),
            lot: "Improvement".into(),
            classification: "Residential".into(),
            area: "120".into(),
            fmv_per_td: 800_000.0,
            fmv_zonal: 750_000.5,
            fmv: 0.0,
        },
    ];
    draft.gross_selling_price = 2_750_000.005;
    draft.surcharge = 25.5;
    draft.recompute();
    draft
}

#[test]
fn form_2000ot_sample_payload_is_current() {
    let draft = sample_2000ot();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2000-OT must validate: {errors:?}"));
    check_sample(
        "2000OT-06152025",
        "2000ot-v2018",
        &payload,
        "44697e2badeb947d10af41557dafe492f0d9da3ece920c193de9a335a40ce21c",
    );
}
