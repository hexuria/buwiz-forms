//! The steps answer the items in form order (Part V before the Schedule 1
//! employer rows on page 2); profile fields are filled the way `loadBGData`
//! does.

use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1700::{
    Form1700CivilStatus, Form1700Column, Form1700Draft, Form1700Employer, Form1700FilingStatus,
    Form1700Spouse, Form1700TaxpayerType,
};

fn sample_1700() -> Form1700Draft {
    let mut draft = Form1700Draft::new_from_profile(&dummy_profile("1700", "Individual"), 2025);
    draft.taxpayer_name = "Dummy, Sample Taxpayer".to_string();
    draft.taxpayer_type = Form1700TaxpayerType::Employee;
    draft.birth_date = "01/15/1980".to_string();
    draft.citizenship = "Filipino".to_string();
    draft.foreign_tax_credits = Some(false);
    draft.civil_status = Form1700CivilStatus::Single;
    draft.taxpayer = Form1700Column {
        non_taxable: 90_000.0,
        other_income: 10_000.49,
        other_credits: 100.0,
        surcharge: 1_000.0,
        ..Form1700Column::default()
    };
    draft.other_income_description = "Prize".to_string();
    draft.other_credits_description = "Sample credit".to_string();
    draft.employers = vec![Form1700Employer {
        for_spouse: false,
        name: "Sample Employer Inc".to_string(),
        name2: String::new(),
        tin: "12345678800000".to_string(),
        regular: 600_000.5,
        flat: 0.0,
        withheld: 40_000.0,
    }];
    draft.recompute();
    draft
}

#[test]
fn form_1700_sample_payload_is_current() {
    let draft = sample_1700();
    assert_eq!(draft.aggregate_amount_payable, 7_400.0);
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1700 must validate: {errors:?}"));
    check_sample(
        "1700-2025",
        "1700-v2013",
        &payload,
        "2a9cd588fcb5ce2c03ac1bd8291ce603bbff4da3beeb0d199410f7f2a5779e0d",
    );
}

/// Joint filing: the spouse is an employee with their own employer row.
#[test]
fn form_1700_joint_sample_payload_is_current() {
    let mut draft = sample_1700();
    draft.civil_status = Form1700CivilStatus::Married;
    draft.spouse_has_income = Some(true);
    draft.filing_status = Form1700FilingStatus::Joint;
    draft.spouse = Form1700Spouse {
        tin: "12345678800000".to_string(),
        rdo_code: "039".to_string(),
        taxpayer_type: Form1700TaxpayerType::Employee,
        name: "Dummy, Sample Spouse".to_string(),
        contact_number: "09170000001".to_string(),
        citizenship: "Filipino".to_string(),
        foreign_tax_credits: Some(false),
        foreign_tax_number: String::new(),
    };
    draft.employers.push(Form1700Employer {
        for_spouse: true,
        name: "Sample Spouse Employer".to_string(),
        name2: String::new(),
        tin: "12345678800000".to_string(),
        regular: 400_000.0,
        flat: 0.0,
        withheld: 20_000.0,
    });
    draft.recompute();
    assert_eq!(draft.aggregate_amount_payable, 9_900.0);
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy joint 1700 must validate: {errors:?}"));
    check_sample(
        "1700-2025-joint",
        "1700-v2013",
        &payload,
        "5ced309643ae7f884adf7152f52fef0e7ed7cffc49518ec2b473c25636ae23b0",
    );
}
