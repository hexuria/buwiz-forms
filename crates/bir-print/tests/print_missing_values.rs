use bir_core::forms::form_1604c::Form1604cDraft;
use bir_core::forms::form_1604e::Form1604eDraft;
use bir_core::forms::form_1701::{
    Form1701CivilStatus, Form1701Draft, Form1701EmployerRow, Form1701Party,
};
use bir_core::forms::form_1702mx::{Form1702MXDraft, WholePeso};
use bir_core::forms::form_1707a::Form1707ADraft;
use bir_core::forms::form_1800::{
    Form1800Donee, Form1800Draft, Form1800PersonalProperty, Form1800RealProperty,
};
use bir_core::forms::form_1801::{
    Form1801Draft, Form1801Particular, Form1801RealProperty, Form1801Split,
};
use bir_core::forms::form_2200a::Form2200ADraft;
use bir_core::forms::form_2200s::{
    Form2200SBeverageRow, Form2200SDraft, Form2200SManner, Form2200SOtherRow,
};
use bir_core::forms::queueable::QueueableForm;
use bir_core::TaxpayerProfile;
use bir_print::frozen_html::{filled_document, printed_cell};
use serde_json::json;

/// The signed number an amount cell prints: commas dropped, `(123)` and
/// `-123` both negative. Amounts compare as numbers so the sheet's own way of
/// showing a negative (parentheses on 1702MX) and its thousands separators
/// pass, while a lost sign or digit fails.
fn amount(printed: &str) -> Option<i64> {
    let t = printed.trim().replace(',', "");
    let (negative, digits) = match t.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        Some(inner) => (true, inner.to_string()),
        None => match t.strip_prefix('-') {
            Some(rest) => (true, rest.to_string()),
            None => (false, t.clone()),
        },
    };
    let magnitude: i64 = digits.parse().ok()?;
    Some(if negative { -magnitude } else { magnitude })
}

fn demo_profile() -> TaxpayerProfile {
    serde_json::from_value(json!({
        "id": null,
        "tin": { "segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000" },
        "rdo_code": "044",
        "full_name": "SANTOS, JUANA",
        "registered_address": "123 RIZAL AVE, MAKATI CITY",
        "zip_code": "1200",
        "phone": "09171234567",
        "email": "juana.santos@example.test",
        "line_of_business": "SERVICES",
        "default_form_type": "2551Q"
    }))
    .unwrap()
}

#[test]
fn form_1801_prints_item5_tin_items_16_and_24_and_specify_boxes() {
    let profile = demo_profile();
    let mut d = Form1801Draft::new_from_profile(&profile, 2025, 1);
    d.death_month = 2;
    d.death_day = 14;
    d.death_year = 2025;
    d.administrator_name = "SANTOS, JUANITO".into();
    d.administrator_tin = "000456789".into();
    d.administrator_branch_code = "00001".into();
    d.real_properties.push(Form1801RealProperty {
        title_number: "TCT-1001".into(),
        tax_declaration_number: "TD-2001".into(),
        location: "MAKATI CITY".into(),
        lot_or_improvement: "LOT".into(),
        area: "500 SQM".into(),
        classification: "RESIDENTIAL".into(),
        fmv_per_tax_declaration: "150000000.25".into(),
        fmv_per_zonal_value: "210345678.55".into(),
        value: Form1801Split {
            exclusive: 210_345_678.55,
            conjugal: 140_234_567.35,
        },
    });
    d.other_personal.push(Form1801Particular {
        particulars: "TIME DEPOSIT ACCOUNT".into(),
        value: Form1801Split {
            exclusive: 12_345_678.40,
            conjugal: 24_680_135.20,
        },
    });
    d.ordinary_deductions.claims_against_estate = Form1801Split {
        exclusive: 5_100_000.15,
        conjugal: 3_200_000.25,
    };
    d.ordinary_deductions.others_description = "COURT-APPROVED FUNERAL CLAIMS".into();
    d.ordinary_deductions.others = Form1801Split {
        exclusive: 1_250_000.50,
        conjugal: 750_000.25,
    };
    d.other_special_deduction_description = "SPECIAL TREATY ALLOWANCE".into();
    d.other_special_deduction = 4_500_000.75;
    // Keep this print fixture independent of the wall clock: the return's
    // death date would otherwise accrue penalties after its one-year due date.
    d.penalty.penalty_overrides.surcharge = Some(0.0);
    d.penalty.penalty_overrides.interest = Some(0.0);
    d.penalty.penalty_overrides.compromise = Some(0.0);
    d.compute();

    let map = d.print_field_map();
    let doc = filled_document("1801-2018", &map).expect("1801-2018 render");

    assert_eq!(printed_cell(&doc, "p1c13").0, "000");
    assert_eq!(printed_cell(&doc, "p1c14").0, "123");
    assert_eq!(printed_cell(&doc, "p1c15").0, "453");
    assert_eq!(printed_cell(&doc, "p1c56").0, "292323706");
    assert_eq!(printed_cell(&doc, "p1c57").0, "57");
    assert_eq!(printed_cell(&doc, "p1c112").0, "17539422");
    assert_eq!(printed_cell(&doc, "p1c114").0, "39");
    assert_eq!(printed_cell(&doc, "p2c259").0, "SPECIAL TREATY ALLOWANCE");
    assert_eq!(
        printed_cell(&doc, "p2c261").0,
        "COURT-APPROVED FUNERAL CLAIMS"
    );
}

#[test]
fn form_1800_prints_item_14_total_net_gifts() {
    let profile = demo_profile();
    let mut d = Form1800Draft::new_from_profile(&profile, 2025, 1);
    d.donation_month = 3;
    d.donation_day = 15;
    d.donation_year = 2025;
    d.donees.push(Form1800Donee {
        name: "SANTOS, PEDRO".into(),
        tin: "00098765400000".into(),
    });
    d.personal_properties.push(Form1800PersonalProperty {
        particulars: "CASH GIFT SAVINGS".into(),
        fair_market_value: 123_456_789.45,
    });
    d.real_properties.push(Form1800RealProperty {
        title_number: "TCT-9001".into(),
        tax_declaration_number: "TD-8001".into(),
        location: "MAKATI CITY".into(),
        lot_or_improvement: "LOT".into(),
        classification: "RESIDENTIAL".into(),
        area: "300 SQM".into(),
        fmv_per_tax_declaration: "85000000.00".into(),
        fmv_per_zonal_value: "110222333.50".into(),
        fair_market_value: 110_222_333.50,
    });
    d.compute();

    let map = d.print_field_map();
    let doc = filled_document("1800-2018", &map).expect("1800-2018 render");

    assert_eq!(printed_cell(&doc, "p1c62").0, "233429122");
    assert_eq!(printed_cell(&doc, "p1c64").0, "95");
    assert_eq!(printed_cell(&doc, "p1c70").0, "14005747");
    assert_eq!(printed_cell(&doc, "p1c72").0, "38");
}

#[test]
fn form_1701_prints_schedule1_employer_names_and_tins() {
    let profile = demo_profile();
    let mut d = Form1701Draft::new_from_profile(&profile, 2025, 12);
    d.civil_status = Some(Form1701CivilStatus::Married);
    d.spouse_has_income = Some(true);
    d.spouse.name = "SANTOS, JUANITO".into();
    d.spouse.tin = "00045678900000".into();
    d.spouse_rdo_code = "044".into();
    d.employers[0] = Form1701EmployerRow {
        owner: Some(Form1701Party::Taxpayer),
        employer_name: "ACME PACIFIC HOLDINGS CORPORATION PHILIPPINES".into(),
        employer_tin: "00077788800001".into(),
        compensation_income: Some(125_456_789.45),
        tax_withheld: Some(31_234_567.89),
    };
    d.employers[1] = Form1701EmployerRow {
        owner: Some(Form1701Party::Spouse),
        employer_name: "MANILA GLOBAL ENTERPRISES INCORPORATED".into(),
        employer_tin: "00099911100002".into(),
        compensation_income: Some(110_222_333.50),
        tax_withheld: Some(25_111_222.25),
    };
    d.compute();

    let map = d.print_field_map();
    let doc = filled_document("1701-2018", &map).expect("1701-2018 render");

    assert_eq!(printed_cell(&doc, "p2c54").0, "X");
    assert_eq!(
        printed_cell(&doc, "p2c55").0,
        "ACME PACIFIC HOLDINGS CORPORATION"
    );
    assert_eq!(printed_cell(&doc, "p2c56").0, "PHILIPPINES");
    assert_eq!(printed_cell(&doc, "p2c58").0, "00077788800001");

    assert_eq!(printed_cell(&doc, "p2c59").0, "X");
    assert_eq!(printed_cell(&doc, "p2c60").0, "MANILA GLOBAL ENTERPRISES");
    assert_eq!(printed_cell(&doc, "p2c61").0, "INCORPORATED");
    assert_eq!(printed_cell(&doc, "p2c63").0, "00099911100002");

    // Space right at char 35 must not leave a leading space on line 2
    d.employers[0].employer_name = "PHILIPPINE NATIONAL INFRASTRUCTURE CORP".into();
    let doc2 = filled_document("1701-2018", &d.print_field_map()).expect("1701-2018 render");
    assert_eq!(
        printed_cell(&doc2, "p2c55").0,
        "PHILIPPINE NATIONAL INFRASTRUCTURE"
    );
    assert_eq!(printed_cell(&doc2, "p2c56").0, "CORP");
}

#[test]
fn form_1604c_prints_two_digit_year_after_preprinted_20() {
    let profile = demo_profile();
    let d = Form1604cDraft::new_from_profile(&profile, 2025);
    assert_eq!(
        d.to_bir_field_map()
            .get("frm1604c:txtYear")
            .map(String::as_str),
        Some("2025")
    );
    let map = d.to_print_field_map();
    assert_eq!(map.get("frm1604c:txtYear").map(String::as_str), Some("25"));
    let doc = filled_document("1604c-2018", &map).expect("1604c-2018 render");
    assert_eq!(printed_cell(&doc, "p1c4").0, "25");

    // Years outside 2000..=2099 print blank rather than a wrong century after preprinted "20"
    let d1999 = Form1604cDraft::new_from_profile(&profile, 1999);
    let doc1999 = filled_document("1604c-2018", &d1999.to_print_field_map()).unwrap();
    assert_eq!(printed_cell(&doc1999, "p1c4").0, "");
    assert_eq!(d1999.to_bir_field_map()["frm1604c:txtYear"], "1999");
}

#[test]
fn form_1604e_prints_two_digit_year_after_preprinted_20() {
    let profile = demo_profile();
    let d = Form1604eDraft::new_from_profile(&profile, 2025);
    assert_eq!(
        d.to_bir_field_map()
            .get("frm1604e:txtYear")
            .map(String::as_str),
        Some("2025")
    );
    let map = d.to_print_field_map();
    assert_eq!(map.get("frm1604e:txtYear").map(String::as_str), Some("25"));
    let doc = filled_document("1604e-2018", &map).expect("1604e-2018 render");
    assert_eq!(printed_cell(&doc, "p1c4").0, "25");

    // Years outside 2000..=2099 print blank rather than a wrong century after preprinted "20"
    let d1999 = Form1604eDraft::new_from_profile(&profile, 1999);
    let doc1999 = filled_document("1604e-2018", &d1999.to_print_field_map()).unwrap();
    assert_eq!(printed_cell(&doc1999, "p1c4").0, "");
    assert_eq!(d1999.to_bir_field_map()["frm1604e:txtYear"], "1999");
}

#[test]
fn form_1707a_prints_item_12a_tax_relief_specify() {
    let profile = demo_profile();
    let mut d = Form1707ADraft::new_from_profile(&profile, 2025, 12);
    d.tax_relief = Some("PH-US TAX TREATY".to_string());
    d.recompute();

    let map = d.to_print_field_map();
    let doc = filled_document("1707a-2021", &map).expect("1707a-2021 render");
    assert_eq!(printed_cell(&doc, "p1c42").0, "X");
    assert_eq!(printed_cell(&doc, "p1c207").0, "PH-US TAX TREATY");
}

#[test]
fn form_2200a_prints_part_iv_payment_details() {
    let profile = demo_profile();
    let mut d = Form2200ADraft::new_from_profile(&profile, 2025, 3);
    d.day = 15;
    d.payment.cash_bank = "BDO".into();
    d.payment.cash_number = "100201".into();
    d.payment.cash_date = "03/15/2025".into();
    d.payment.cash_amount = "123,456,789.12".into();
    d.payment.check_bank = "BPI".into();
    d.payment.check_number = "200302".into();
    d.payment.check_date = "03/16/2025".into();
    d.payment.check_amount = "234,567,890.34".into();
    d.payment.tax_number = "300403".into();
    d.payment.tax_date = "03/17/2025".into();
    d.payment.tax_amount = "345,678,901.56".into();
    d.payment.others_details = "EPAY".into();
    d.payment.others_bank = "MBTC".into();
    d.payment.others_number = "400504".into();
    d.payment.others_date = "03/18/2025".into();
    d.payment.others_amount = "456,789,012.78".into();
    d.recompute();

    let map = d.to_print_field_map();
    let doc = filled_document("2200a-2020", &map).expect("2200a-2020 render");
    assert_eq!(
        printed_cell(&doc, "p1c108").0,
        "BDO  10020103152025  12345678912"
    );
    assert_eq!(
        printed_cell(&doc, "p1c110").0,
        "BPI  20030203162025  23456789034"
    );
    assert_eq!(
        printed_cell(&doc, "p1c111").0,
        "30040303172025  34567890156"
    );
    assert_eq!(printed_cell(&doc, "p1c113").0, "EPAY");
    assert_eq!(
        printed_cell(&doc, "p1c114").0,
        "MBTC 40050403182025  45678901278"
    );
}

#[test]
fn form_2200s_prints_schedule1_rows_and_part_iii_items() {
    let profile = demo_profile();
    let mut d = Form2200SDraft::new_from_profile(&profile, 2025, 3);
    d.manner = Form2200SManner::ActualRemoval;
    d.beverages.resize(10, Form2200SBeverageRow::default());
    d.balance_carried_over = 1_238.24;
    d.creditable_excise_tax = 1_255.25;
    d.tax_payment = 1_340.30;
    d.beverages[0].sales_value = 123_456_789.12;
    d.beverages[0].volume = 10_000.50;
    d.beverages[8].sales_value = 234_567_890.34;
    d.beverages[8].volume = 5_000.25;
    d.others = vec![Form2200SOtherRow {
        atc: "XB010".into(),
        description: "CUSTOM SYRUP MIX".into(),
        tax_bracket: "PER LITER".into(),
        rate: 6.0,
        sales_value: 9_876_543.21,
        volume: 1_500.00,
        basic_tax_due: 0.0,
    }];
    d.recompute();

    let map = d.to_print_field_map();
    let doc = filled_document("2200s-2018", &map).expect("2200s-2018 render");

    // Page 1 Items 19A, 19B, 25A (already joined via decimal_html_id)
    assert_eq!(printed_cell(&doc, "p1c62").0, "123824");
    assert_eq!(printed_cell(&doc, "p1c64").0, "125525");
    assert_eq!(printed_cell(&doc, "p1c84").0, "134030");

    // Page 2 Schedule 1 fixed rows (XB010 = row 0, XB090 = row 8)
    assert_eq!(printed_cell(&doc, "p2c27").0, "123,456,789.12");
    assert_eq!(printed_cell(&doc, "p2c28").0, "10,000.50");
    assert_eq!(printed_cell(&doc, "p2c29").0, "60,003.00");
    assert_eq!(printed_cell(&doc, "p2c83").0, "234,567,890.34");
    assert_eq!(printed_cell(&doc, "p2c84").0, "5,000.25");
    assert_eq!(printed_cell(&doc, "p2c85").0, "60,003.00");

    // form-review/RULES.md section 7: untouched fixed-row amount boxes stay
    // blank even though the upload carries zero.
    assert_eq!(printed_cell(&doc, "p2c34").0, "");
    assert_eq!(printed_cell(&doc, "p2c35").0, "");
    assert_eq!(printed_cell(&doc, "p2c36").0, "");

    // Page 2 Schedule 1 Others row 1
    assert_eq!(printed_cell(&doc, "p2c100").0, "XB010");
    assert_eq!(printed_cell(&doc, "p2c102").0, "PER LITER");
    assert_eq!(printed_cell(&doc, "p2c103").0, "6.00");
    assert_eq!(printed_cell(&doc, "p2c104").0, "9,876,543.21");
    assert_eq!(printed_cell(&doc, "p2c105").0, "1,500.00");
    assert_eq!(printed_cell(&doc, "p2c106").0, "9,000.00");
    // Grand total still prints
    assert_eq!(printed_cell(&doc, "p2c115").0, "129,006.00");
}

#[test]
fn form_1702mx_prints_part_ii_and_schedule_1_details() {
    let profile = demo_profile();
    let mut d = Form1702MXDraft::new_from_profile(&profile, 2025);
    d.relief_details.investment_promotion_agency = ["PEZA".into(), "BOI".into(), "SBMA".into()];
    d.relief_details.legal_basis = ["RA 7916".into(), "EO 226".into(), "RA 7227".into()];
    d.relief_details.registered_activity =
        ["IT BPO".into(), "EXPORT MFG".into(), "LOGISTICS".into()];
    d.schedule_2.items[0].exempt.set(WholePeso(123_456_789));
    d.schedule_2.items[0].special.set(WholePeso(234_567_890));
    d.schedule_2.items[0].regular.set(WholePeso(345_678_901));
    d.schedule_3.items_20_to_33[0]
        .regular
        .set(WholePeso(10_000_000));
    d.penalty.penalty_overrides.surcharge = Some(1_234.0);
    d.penalty.penalty_overrides.interest = Some(2_345.0);
    d.penalty.penalty_overrides.compromise = Some(3_456.0);
    d.recompute();

    let map = d.to_print_field_map();
    let doc = filled_document("1702mx-2018c", &map).expect("1702mx-2018c render");

    // Part II Items 14-21 (whole-peso combs), compared as signed numbers
    assert_eq!(
        amount(&printed_cell(&doc, "p1c59").0),
        Some(d.part_ii.item_14_total_tax_due_or_overpayment.0)
    );
    assert_eq!(
        amount(&printed_cell(&doc, "p1c61").0),
        Some(d.part_ii.item_15_total_tax_credits.0)
    );
    assert_eq!(
        amount(&printed_cell(&doc, "p1c63").0),
        Some(d.part_ii.item_16_net_tax_payable_or_overpayment.0)
    );
    assert_eq!(amount(&printed_cell(&doc, "p1c66").0), Some(1234));
    assert_eq!(amount(&printed_cell(&doc, "p1c68").0), Some(2345));
    assert_eq!(amount(&printed_cell(&doc, "p1c70").0), Some(3456));
    assert_eq!(amount(&printed_cell(&doc, "p1c72").0), Some(7035));

    // Schedule 1 Items 1-3 (Investment Promotion Agency, Legal Basis, Registered Activity)
    assert_eq!(printed_cell(&doc, "p2c17").0, "PEZA");
    assert_eq!(printed_cell(&doc, "p2c18").0, "BOI");
    assert_eq!(printed_cell(&doc, "p2c19").0, "SBMA");
    assert_eq!(printed_cell(&doc, "p2c21").0, "RA 7916");
    assert_eq!(printed_cell(&doc, "p2c22").0, "EO 226");
    assert_eq!(printed_cell(&doc, "p2c23").0, "RA 7227");
    assert_eq!(printed_cell(&doc, "p2c25").0, "IT BPO");
    assert_eq!(printed_cell(&doc, "p2c26").0, "EXPORT MFG");
    assert_eq!(printed_cell(&doc, "p2c27").0, "LOGISTICS");

    // Schedule 2 sales row
    assert_eq!(amount(&printed_cell(&doc, "p2c70").0), Some(123_456_789));
    assert_eq!(amount(&printed_cell(&doc, "p2c71").0), Some(234_567_890));
    assert_eq!(amount(&printed_cell(&doc, "p2c72").0), Some(345_678_901));
}

#[test]
fn form_2200a_part_iv_never_truncates_or_drops_negative_sign() {
    let profile = demo_profile();

    // 1. Fitting case unchanged byte for byte
    let mut fit = Form2200ADraft::new_from_profile(&profile, 2026, 1);
    fit.payment.cash_bank = "BDO".into();
    fit.payment.cash_number = "123456".into();
    fit.payment.cash_date = "01/15/2026".into();
    fit.payment.cash_amount = "500.00".into();
    // 2. 0.05 (leading zero peso + centavos)
    fit.payment.check_bank = "BPI".into();
    fit.payment.check_number = "654321".into();
    fit.payment.check_date = "01/16/2026".into();
    fit.payment.check_amount = "0.05".into();
    // 3. 12-digit amount with centavos (1 over 11-slot peso comb)
    fit.payment.tax_number = "TDM001".into();
    fit.payment.tax_date = "01/17/2026".into();
    fit.payment.tax_amount = "123,456,789,012.34".into();
    // 4. 13-digit amount (2 over 11-slot peso comb)
    fit.payment.others_bank = "LBP".into();
    fit.payment.others_number = "OTH001".into();
    fit.payment.others_date = "01/18/2026".into();
    fit.payment.others_amount = "1,234,567,890,123.45".into();
    let map = fit.to_print_field_map();
    let doc = filled_document("2200a-2020", &map).expect("2200a-2020 render");

    assert_eq!(
        printed_cell(&doc, "p1c108").0,
        "BDO  12345601152026        50000",
        "fitting cash row must remain byte-for-byte unchanged"
    );
    assert_eq!(
        printed_cell(&doc, "p1c110").0,
        "BPI  65432101162026          005",
        "0.05 must print 0 in peso units and 05 in centavos"
    );
    assert_eq!(
        printed_cell(&doc, "p1c111").0,
        "TDM00101172026             ",
        "12-digit amount must not be truncated to 11 digits"
    );
    assert_eq!(
        printed_cell(&doc, "p1c114").0,
        "LBP  OTH00101182026             ",
        "13-digit amount must not be truncated to 11 digits"
    );

    // 5. Negative amount, 6. Bank 1 char over 5, 7. Number 1 char over 6
    let mut edges = Form2200ADraft::new_from_profile(&profile, 2026, 1);
    edges.payment.cash_bank = "BDO".into();
    edges.payment.cash_number = "123456".into();
    edges.payment.cash_date = "01/15/2026".into();
    edges.payment.cash_amount = "-500.00".into();
    edges.payment.check_bank = "BDO-QC".into(); // 6 chars (1 over 5)
    edges.payment.check_number = "654321".into();
    edges.payment.check_date = "01/16/2026".into();
    edges.payment.check_amount = "250.00".into();
    edges.payment.others_bank = "LBP".into();
    edges.payment.others_number = "1234567".into(); // 7 chars (1 over 6)
    edges.payment.others_date = "01/18/2026".into();
    edges.payment.others_amount = "750.00".into();
    let map2 = edges.to_print_field_map();
    let doc2 = filled_document("2200a-2020", &map2).expect("2200a-2020 edge render");

    assert_eq!(
        printed_cell(&doc2, "p1c108").0,
        "BDO  12345601152026             ",
        "negative amount must never print as positive"
    );
    assert_eq!(
        printed_cell(&doc2, "p1c110").0,
        "     65432101162026        25000",
        "6-char bank must not be cut to 5 chars"
    );
    assert_eq!(
        printed_cell(&doc2, "p1c114").0,
        "LBP        01182026        75000",
        "7-char number must not be cut to 6 chars"
    );
}
