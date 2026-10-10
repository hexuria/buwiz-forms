use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1606::{
    Form1606AgentCategory, Form1606Draft, Form1606PropertyClass, Form1606SellerType,
    Form1606TaxRate, Form1606TaxRelief, Form1606TaxableBaseBox, Form1606Transaction,
};

fn sample_1606() -> Form1606Draft {
    let mut draft = Form1606Draft::new_from_profile(&dummy_profile("1606", "Corporation"), 2025, 6);
    draft.transaction_day = 15;
    draft.is_amended = true;
    draft.taxes_withheld = Some(true);
    // Fictitious seller; 987-654-321 passes the official check digit.
    draft.seller_tin = "98765432100000".into();
    draft.seller_rdo_code = "040".into();
    draft.seller_name = "Dela Cruz, Sample Seller".into();
    draft.seller_address = "456 Example Avenue, Makati City".into();
    draft.seller_type = Some(Form1606SellerType::Individual);
    draft.agent_category = Some(Form1606AgentCategory::Private);
    draft.property_class = Some(Form1606PropertyClass::Others);
    draft.property_class_other = "Mixed use lot".into();
    draft.property_location = "Lot 1 Block 2, Sample Subdivision, Quezon City".into();
    draft.property_rdo_code = "039".into();
    draft.tct_number = "T-123456".into();
    draft.area_sold = "250".into();
    draft.tax_declaration_number = "0123456789".into();
    draft.other_description = "Corner lot".into();
    draft.covers_more_than_one_property = Some(false);
    draft.tax_relief = Form1606TaxRelief::SpecialLaw;
    draft.set_transaction(Form1606Transaction::InstallmentSale);
    draft.gross_selling_price = 5_000_000.005;
    draft.cost_and_expenses = 3_200_000.0;
    draft.mortgage_assumed = 500_000.0;
    draft.initial_year_payments = 1_250_000.5;
    draft.installment_this_month = 104_166.675;
    draft.number_of_installments = 48.0;
    draft.fmv_land_tax_declaration = Some(3_000_000.0);
    draft.fmv_improvements_tax_declaration = Some(1_500_000.0);
    draft.fmv_land_zonal = Some(4_200_000.0);
    draft.fmv_improvements_bir = Some(0.0);
    draft.taxable_base_box = Some(Form1606TaxableBaseBox::InstallmentCollected);
    draft.installment_collected = 104_166.675;
    draft.seller_habitual = Some(true);
    draft.tax_rate = Some(Form1606TaxRate::OnePointFive);
    draft.tax_remitted_previous = 1_000.0;
    draft.surcharge = 140.625;
    draft.interest = 33.335;
    draft.recompute();
    draft
}

#[test]
fn form_1606_sample_payload_is_current() {
    let draft = sample_1606();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1606 must validate: {errors:?}"));
    check_sample(
        "1606-06152025_T123456",
        "1606-v2018",
        &payload,
        "f2181d801152335549db45e27599c2b3aed39990cd8ecc07d6a760b9cba33a72",
    );
}
