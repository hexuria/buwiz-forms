//! Profile fields are filled the way `loadBGData` does; the other steps are
//! raw filer input in form order.

use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1702ex::{
    Form1702ExAtc, Form1702ExDeduction, Form1702ExDraft, Form1702ExOverpayment, Form1702ExRow,
    Form1702ExSpecialRow,
};

fn sample_1702ex() -> Form1702ExDraft {
    let mut draft =
        Form1702ExDraft::new_from_profile(&dummy_profile("1702EX", "Corporation"), 2025);
    draft.registered_name = "Sample Dummy Foundation Inc".to_string();
    draft.date_of_incorporation = "03/01/2010".to_string();
    draft.atc = Form1702ExAtc::IC011;
    draft.deduction = Form1702ExDeduction::Itemized;
    draft.legal_basis = "Sec 30 NIRC".to_string();
    draft.promotion_agency = "Sample Agency".to_string();
    draft.registered_activity = "REG-0001".to_string();
    draft.effectivity_from = "01/01/2010".to_string();
    draft.effectivity_to = "12/31/2030".to_string();
    draft.sales = 5_000_000.5;
    draft.sales_returns = 100_000.0;
    draft.cost_of_sales = 2_000_000.0;
    draft.other_income = 50_000.0;
    draft.cwt_q4 = 1_000.0;
    draft.regular_income_tax = 600_000.0;
    draft.special_allowable_relief = 15_000.0;
    draft.ordinary_items[1] = 10_000.0;
    draft.ordinary_items[12] = 500_000.0;
    draft.ordinary_items[16] = 20_000.0;
    draft.other_deductions = vec![Form1702ExRow {
        description: "Sample expense".to_string(),
        amount: 5_000.0,
    }];
    draft.special_rows = vec![Form1702ExSpecialRow {
        description: "Sample deduction".to_string(),
        legal_basis: "RA 0000".to_string(),
        amount: 15_000.0,
    }];
    draft.net_income_per_books = 2_300_000.0;
    draft.non_deductible = vec![Form1702ExRow {
        description: "Nondeductible expense".to_string(),
        amount: 120_001.0,
    }];
    draft.non_taxable_income = vec![Form1702ExRow {
        description: "Interest income".to_string(),
        amount: 20_000.0,
    }];
    draft.overpayment = Form1702ExOverpayment::Refund;
    draft.recompute();
    draft
}

#[test]
fn form_1702ex_sample_payload_is_current() {
    let draft = sample_1702ex();
    assert_eq!(draft.total_amount_payable, -1_000.0);
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1702EX must validate: {errors:?}"));
    check_sample(
        "1702EX-122025",
        "1702ex-v2018c",
        &payload,
        "f101fdf1977954a564dc3e586b8a71d192772ddcc40c900e3cf6941fb2c7aed6",
    );
}
