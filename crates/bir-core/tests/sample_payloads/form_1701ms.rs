use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1701ms::{
    Form1701MsCivilStatus, Form1701MsColumn, Form1701MsDeduction, Form1701MsDraft,
    Form1701MsFiling, Form1701MsSource, Form1701MsSpouseInfo, Form1701MsTaxOption,
};

/// Married, filing jointly. Taxpayer: mixed income, graduated rates, OSD.
/// Spouse: compensation income only.
fn sample_1701ms() -> Form1701MsDraft {
    let mut draft = Form1701MsDraft::new_from_profile(&dummy_profile("1701MS", "Individual"), 2025);
    draft.civil_status = Form1701MsCivilStatus::Married;
    draft.filing = Form1701MsFiling::Jointly;
    draft.spouse_info = Form1701MsSpouseInfo {
        tin: "12345678800000".to_string(),
        rdo_code: "039".to_string(),
        name: "Sample Dummy Spouse".to_string(),
        email: "sample.spouse@example.com".to_string(),
        contact_number: "09170000001".to_string(),
    };
    draft.taxpayer = Form1701MsColumn {
        source: Form1701MsSource::Mixed,
        tax_option: Form1701MsTaxOption::Graduated,
        deduction: Form1701MsDeduction::Osd,
        gross_compensation: 900_000.5,
        non_taxable_compensation: 90_000.0,
        sales: 1_500_000.0,
        sales_returns: 10_000.0,
        quarterly_payments: 50_000.0,
        cwt_2316: 80_000.0,
        ..Form1701MsColumn::default()
    };
    draft.spouse = Form1701MsColumn {
        source: Form1701MsSource::Compensation,
        gross_compensation: 600_000.0,
        non_taxable_compensation: 90_000.0,
        cwt_2316: 44_500.0,
        ..Form1701MsColumn::default()
    };
    draft.perjury_agreed = true;
    draft.recompute();
    draft
}

#[test]
fn form_1701ms_sample_payload_is_current() {
    let draft = sample_1701ms();
    assert_eq!(draft.aggregate_amount_payable, 198_500.0);
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1701MS must validate: {errors:?}"));
    check_sample(
        "1701MS-122025",
        "1701ms-v2024",
        &payload,
        "349a7a4f6eff321bd1064710bd77a91d8496972f17ee604b1d879f757ebe7250",
    );
}
