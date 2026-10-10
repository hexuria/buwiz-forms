//! The steps answer the radio items in form order, then type amounts (see
//! `Form1701ADraft::to_bir_field_map`); profile fields are filled the way
//! `loadBGData` does.

use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1701a::{
    Form1701AAtc, Form1701ACivilStatus, Form1701AColumn, Form1701ADraft, Form1701AFilerType,
    Form1701AFilingStatus, Form1701ASpouse,
};

fn sample_1701a() -> Form1701ADraft {
    let mut draft = Form1701ADraft::new_from_profile(&dummy_profile("1701A", "Individual"), 2025);
    draft.taxpayer_name = "Dummy, Sample Taxpayer".to_string();
    draft.birth_date = "01/15/1980".to_string();
    draft.citizenship = "Filipino".to_string();
    draft.filer_type = Form1701AFilerType::SingleProprietor;
    draft.atc = Form1701AAtc::II012;
    draft.civil_status = Form1701ACivilStatus::Single;
    draft.taxpayer = Form1701AColumn {
        sales: 1_234_567.89,
        sales_returns: 10_000.5,
        other_income_41: 5_000.55,
        gpp_share: 20_000.0,
        prior_year_excess: 1_000.0,
        quarterly_payments: 30_000.0,
        cwt_q1_q3: 5_000.25,
        cwt_q4: 2_500.0,
        other_credits: 100.0,
        surcharge: 1_000.0,
        ..Form1701AColumn::default()
    };
    draft.other_income_41_description = "Interest income".to_string();
    draft.other_credits_description = "Sample credit".to_string();
    draft.recompute();
    draft
}

#[test]
fn form_1701a_sample_payload_is_current() {
    let draft = sample_1701a();
    assert_eq!(draft.aggregate_amount_payable, 56_848.0);
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1701A must validate: {errors:?}"));
    check_sample(
        "1701A-122025",
        "1701a-v2018",
        &payload,
        "99d6adb51afca6a987d6fe532f5dd782c2f42a909180c5268878e49e55518e57",
    );
}

/// Joint filing: the spouse uses the 8% rate as a professional.
#[test]
fn form_1701a_joint_sample_payload_is_current() {
    let mut draft = sample_1701a();
    draft.civil_status = Form1701ACivilStatus::Married;
    draft.spouse_has_income = Some(true);
    draft.filing_status = Form1701AFilingStatus::Joint;
    draft.spouse = Form1701ASpouse {
        tin: "12345678800000".to_string(),
        rdo_code: "039".to_string(),
        filer_type: Form1701AFilerType::Professional,
        atc: Form1701AAtc::II017,
        name: "Dummy, Sample Spouse".to_string(),
        contact_number: "09170000001".to_string(),
        citizenship: "Filipino".to_string(),
        foreign_tax_credits: Some(false),
        foreign_tax_number: String::new(),
    };
    draft.spouse_column = Form1701AColumn {
        eight_sales: 900_000.0,
        eight_reduction: 250_000.0,
        cwt_q1_q3: 4_500.5,
        ..Form1701AColumn::default()
    };
    draft.recompute();
    assert_eq!(draft.aggregate_amount_payable, 104_347.0);
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy joint 1701A must validate: {errors:?}"));
    check_sample(
        "1701A-122025-joint",
        "1701a-v2018",
        &payload,
        "6cba3b6852efd8b75b9603b328ec7b47b89bbe377004ab0d4576642184c9df6c",
    );
}
