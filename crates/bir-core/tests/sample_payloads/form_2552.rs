use crate::{check_sample, dummy_profile};
use bir_core::forms::form_2552::{
    Form2552Draft, Form2552ExemptRow, Form2552NonTaxableRow, Form2552ShareRow, Form2552Transaction,
};

fn share(date: &str, seller: &str, buyer: &str, shares: f64, base: f64) -> Form2552ShareRow {
    Form2552ShareRow {
        date: date.to_string(),
        seller: seller.to_string(),
        buyer: buyer.to_string(),
        issuing_corporation: "Sample Listed Corp".to_string(),
        number_of_shares: shares,
        tax_base: base,
        ..Default::default()
    }
}

fn sample_2552() -> Form2552Draft {
    let mut draft =
        Form2552Draft::new_from_profile(&dummy_profile("2552", "Corporation"), 2025, 3, 14);
    draft.is_amended = true;
    draft.number_of_attached_sheets = 1;
    draft.tax_relief = true;
    draft.tax_relief_specification = "Special law sample".to_string();
    draft.set_transaction(Form2552Transaction::LocalStockExchange);
    draft.schedule = vec![
        share(
            "03/14/2025",
            "Sample Seller One",
            "Sample Buyer One",
            1_000.0,
            1_234_567.891,
        ),
        share(
            "03/14/2025",
            "Sample Seller Two",
            "sample buyer two",
            500.5,
            999_999.995,
        ),
        share(
            "03/13/2025",
            "Sample Seller Three",
            "Sample Buyer Three",
            75.0,
            2_500.83,
        ),
    ];
    draft.exempt_transactions = vec![
        Form2552ExemptRow {
            classification: "Exempt sample one".to_string(),
            amount: 1_000.005,
        },
        Form2552ExemptRow {
            classification: "Exempt sample two".to_string(),
            amount: 250.0,
        },
    ];
    draft.non_taxable_transactions = vec![Form2552NonTaxableRow {
        date: "03/12/2025".to_string(),
        seller: "Sample Seller Six".to_string(),
        buyer: "Sample Buyer Six".to_string(),
        issuing_corporation: "Other Sample Corp".to_string(),
        number_of_shares: 42.0,
    }];
    draft.tax_paid_previous = 100.5;
    draft.creditable_tax_withheld = 1_000.25;
    draft.surcharge = 25.5;
    draft.interest = 10.0;
    draft.recompute();
    draft
}

#[test]
fn form_2552_sample_payload_is_current() {
    let draft = sample_2552();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2552 must validate: {errors:?}"));
    check_sample(
        "2552-03142025",
        "2552-v2018",
        &payload,
        "c2a5de6e9b0c9b3d3a82ee0045770ecb4a84194f90cefc798dda28cea654e1f3",
    );
}
