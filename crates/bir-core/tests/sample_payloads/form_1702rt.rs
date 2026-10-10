use crate::{dummy_profile, samples_dir};
use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt};
use bir_core::forms::form_1702rt::{
    Form1702RTDate, Form1702RTDeductionMethod, Form1702RTDraft, Form1702RTNamedAmount, WholePeso,
};
use sha2::{Digest, Sha256};

fn named(description: &str, amount: i64) -> Form1702RTNamedAmount {
    Form1702RTNamedAmount {
        description: description.to_string(),
        amount: WholePeso(amount),
    }
}

/// Itemized deductions with Schedules I to V, NOLCO and MCIT carry-overs,
/// credits, penalties and one payment row.
fn sample_1702rt() -> Form1702RTDraft {
    let mut draft =
        Form1702RTDraft::new_from_profile(&dummy_profile("1702RTv2018C", "Corporation"), 2025, 12);
    draft.deduction_method = Form1702RTDeductionMethod::Itemized;
    draft.president_signatory_title = "President".to_string();
    draft.president_signatory_tin = "123456788000".to_string();

    let p4 = &mut draft.part_iv;
    p4.item_40_income_tax_rate_percent = 25;
    p4.item_27_sales = WholePeso(8_500_000);
    p4.item_28_sales_returns = WholePeso(125_000);
    p4.item_30_cost_of_sales_or_services = WholePeso(4_200_000);
    p4.item_32_other_taxable_income = WholePeso(75_500);
    p4.item_42_mcit_due = WholePeso(85_010);
    let credits = &mut p4.tax_credits;
    credits.item_44_prior_year_excess_credits = WholePeso(10_000);
    credits.item_46_previous_quarter_regular_payments = WholePeso(300_000);
    credits.item_48_previous_quarter_withholding = WholePeso(40_000);
    credits.item_49_fourth_quarter_withholding = WholePeso(15_500);
    credits.item_52_special_tax_credits = WholePeso(1_000);
    credits.item_53_other = named("Other credit", 2_500);
    draft.part_v.item_57_special_allowable_deductions_tax_effect = WholePeso(12_500);

    let s1 = &mut draft.schedule_1;
    s1.salaries_wages_allowances = WholePeso(1_500_000);
    s1.rental = WholePeso(360_000);
    s1.depreciation = WholePeso(120_000);
    s1.taxes_and_licenses = WholePeso(45_250);
    s1.professional_fees = WholePeso(80_000);
    s1.other[0] = named("Office supplies", 15_750);
    let row = &mut draft.schedule_2.rows[0];
    row.description = "Special deduction".to_string();
    row.legal_basis = "RA 9999".to_string();
    row.amount = WholePeso(50_000);
    let nolco = &mut draft.schedule_3.rows[1];
    nolco.year_incurred = "2023".to_string();
    nolco.amount = WholePeso(300_000);
    nolco.applied_previous_years = WholePeso(100_000);
    nolco.applied_current_year = WholePeso(150_000);
    let mcit = &mut draft.schedule_4.rows[0];
    mcit.year = "2023".to_string();
    mcit.normal_income_tax = WholePeso(50_000);
    mcit.mcit = WholePeso(70_000);
    mcit.applied_current_year = WholePeso(20_000);
    let s5 = &mut draft.schedule_5;
    s5.item_1_net_income_or_loss_per_books = WholePeso(1_800_000);
    s5.additions[0] = named("Non-deductible expenses", 179_500);
    s5.non_taxable_income[0] = named("Interest income subject to final tax", 50_000);

    let p2 = &mut draft.part_ii;
    p2.item_17_surcharge = WholePeso(23_343);
    p2.item_18_interest = WholePeso(4_000);
    p2.item_19_compromise = WholePeso(1_000);
    draft.recompute();
    let cash = &mut draft.payment_details[0];
    cash.drawee_bank_or_agency = "Sample Bank".to_string();
    cash.number = "12345".to_string();
    cash.date = Form1702RTDate::new(2026, 4, 15).ok();
    cash.amount = draft.part_ii.item_21_total_amount_payable_or_overpayment;
    draft.recompute();
    draft
}

/// Checks the 1702RT sample against the official upload.
#[test]
fn form_1702rt_sample_payload_is_current() {
    let payload = sample_1702rt()
        .to_official_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1702RT must validate: {errors:?}"));
    let stem = "1702RT-122025";
    let dir = samples_dir();
    let encrypted = compress_and_encrypt(payload.as_bytes(), BIR_IAF_PASSPHRASE).unwrap();
    if std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some() {
        std::fs::write(dir.join(format!("{stem}.plain.xml")), &payload).unwrap();
        std::fs::write(dir.join(format!("{stem}.iaf.xml")), &encrypted).unwrap();
    }
    let official = std::fs::read_to_string(dir.join(format!("{stem}.official.xml"))).unwrap();
    assert_eq!(
        payload, official,
        "{stem}: differs from the official saveXMLsubmit() output"
    );
    let digest: String = Sha256::digest(&encrypted)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest, "2946796a183bea2a2830c8ab020694ce0a699c70f097d6bd03c78222fccaf1b8",
        "{stem}: not the official Encrypt.exe output"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join(format!("{stem}.plain.xml"))).unwrap(),
        payload
    );
    assert_eq!(
        std::fs::read(dir.join(format!("{stem}.iaf.xml"))).unwrap(),
        encrypted
    );
}
