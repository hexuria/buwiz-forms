use bir_core::forms::form_1800::{Form1800Draft, Form1800PersonalProperty};
use bir_core::forms::form_1801::Form1801Draft;
use bir_core::forms::form_2200s::{Form2200SDraft, Form2200SOtherRow};
use bir_core::forms::queueable::QueueableForm;
use bir_core::TaxpayerProfile;
use bir_print::frozen_html::{filled_document, printed_cell};
use serde_json::json;

fn profile(branch: &str) -> TaxpayerProfile {
    serde_json::from_value(json!({
        "id": null,
        "tin": { "segment1": "000", "segment2": "123", "segment3": "453", "branch": branch },
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
fn form_1801_prints_branch_code_on_both_pages() {
    let draft = Form1801Draft::new_from_profile(&profile("00001"), 2025, 1);
    let doc = filled_document("1801-2018", &draft.print_field_map()).unwrap();
    assert_eq!(printed_cell(&doc, "p1c13").0, "000");
    assert_eq!(printed_cell(&doc, "p1c14").0, "123");
    assert_eq!(printed_cell(&doc, "p1c15").0, "453");
    assert_eq!(printed_cell(&doc, "p1c17").0, "00001");
    assert_eq!(printed_cell(&doc, "p2c5").0, "00012345300001");
}

#[test]
fn branch_zero_prints_once_from_the_taxpayer_value() {
    let estate = Form1801Draft::new_from_profile(&profile("00000"), 2025, 1);
    let estate_doc = filled_document("1801-2018", &estate.print_field_map()).unwrap();
    assert_eq!(printed_cell(&estate_doc, "p1c17").0, "00000");
    assert_eq!(printed_cell(&estate_doc, "p2c5").0, "00012345300000");

    let donor = Form1800Draft::new_from_profile(&profile("00000"), 2025, 1);
    let donor_doc = filled_document("1800-2018", &donor.print_field_map()).unwrap();
    assert_eq!(printed_cell(&donor_doc, "p1c18").0, "00000");
    assert_eq!(printed_cell(&donor_doc, "p2c5").0, "00012345300000");
}

#[test]
fn form_1800_prints_branch_code_on_both_pages() {
    let draft = Form1800Draft::new_from_profile(&profile("00001"), 2025, 1);
    let doc = filled_document("1800-2018", &draft.print_field_map()).unwrap();
    assert_eq!(printed_cell(&doc, "p1c18").0, "00001");
    assert_eq!(printed_cell(&doc, "p2c5").0, "00012345300001");
}

#[test]
fn form_1800_prints_item14_centavos_as_a_comb() {
    for (value, cents) in [(123.50, "50"), (123.05, "05")] {
        let mut draft = Form1800Draft::new_from_profile(&profile("00001"), 2025, 1);
        draft.personal_properties.push(Form1800PersonalProperty {
            particulars: "CASH GIFT".into(),
            fair_market_value: value,
        });
        draft.compute();
        let mut map = draft.print_field_map();
        map.insert("frm1800v2018:TotalNetGifts".into(), format!("123.{cents}"));
        let doc = filled_document("1800-2018", &map).unwrap();
        assert_eq!(printed_cell(&doc, "p1c64").0, cents);
        // The peso comb holds the pesos alone: no centavo digit and no
        // decimal point left in it now that the centavos have their cell.
        assert_eq!(printed_cell(&doc, "p1c62").0, "123");
    }
}

/// Page 1's branch cell, page 2's 14-slot header and the upload carry the
/// same branch for a 1-, 3- and 5-digit branch code: the upload's split
/// left-pads it (`1` -> `00001`), and page 2 now reads the upload's own
/// page 2 keys instead of right-padding the stored TIN (`00100` before).
#[test]
fn pages_one_and_two_and_the_upload_agree_on_the_branch() {
    for (branch, padded) in [("1", "00001"), ("123", "00123"), ("12345", "12345")] {
        let donor = Form1800Draft::new_from_profile(&profile(branch), 2025, 1);
        let upload = donor.to_bir_field_map();
        assert_eq!(
            upload["frm1800v2018:txtBranchCode"], padded,
            "1800 {branch}"
        );
        assert_eq!(
            upload["frm1800v2018:txtPg2BranchCode"], padded,
            "1800 {branch}"
        );
        let doc = filled_document("1800-2018", &donor.print_field_map()).unwrap();
        assert_eq!(
            printed_cell(&doc, "p1c18").0,
            padded,
            "1800 {branch} page 1"
        );
        assert_eq!(
            printed_cell(&doc, "p2c5").0,
            format!("000123453{padded}"),
            "1800 {branch} page 2"
        );

        let estate = Form1801Draft::new_from_profile(&profile(branch), 2025, 1);
        let upload = estate.to_bir_field_map();
        assert_eq!(upload["frm1801v2018:branchCode_1"], padded, "1801 {branch}");
        assert_eq!(upload["frm1801v2018:branchCode_2"], padded, "1801 {branch}");
        let doc = filled_document("1801-2018", &estate.print_field_map()).unwrap();
        assert_eq!(
            printed_cell(&doc, "p1c17").0,
            padded,
            "1801 {branch} page 1"
        );
        assert_eq!(
            printed_cell(&doc, "p2c5").0,
            format!("000123453{padded}"),
            "1801 {branch} page 2"
        );
    }
}

#[test]
fn form_2200s_prints_both_other_descriptions() {
    let mut draft = Form2200SDraft::new_from_profile(&profile("00001"), 2025, 3);
    draft.manner = bir_core::forms::form_2200s::Form2200SManner::ActualRemoval;
    draft.others = vec![
        Form2200SOtherRow {
            atc: "XB010".into(),
            description: "CUSTOM SYRUP MIX".into(),
            tax_bracket: "PER LITER".into(),
            rate: 6.0,
            sales_value: 100.0,
            volume: 10.0,
            basic_tax_due: 60.0,
        },
        Form2200SOtherRow {
            atc: "XB020".into(),
            description: "HERBAL INFUSION".into(),
            tax_bracket: "PER LITER".into(),
            rate: 6.0,
            sales_value: 200.0,
            volume: 20.0,
            basic_tax_due: 120.0,
        },
    ];
    draft.recompute();
    let doc = filled_document("2200s-2018", &draft.to_print_field_map()).unwrap();
    assert_eq!(printed_cell(&doc, "p2c101").0, "CUSTOM SYRUP MIX");
    assert_eq!(printed_cell(&doc, "p2c108").0, "HERBAL INFUSION");
}

#[test]
fn form_2200t_prints_2020_2021_2023_rows_and_item16_sum() {
    let mut map = std::collections::BTreeMap::new();
    for (suffix, due) in [("2020", "100.00"), ("2021", "200.00"), ("2023", "300.00")] {
        for part in ["Export", "Taxable", "Due"] {
            map.insert(
                format!("frm2200Tv2020:txtSched1_1A_{suffix}_{part}"),
                if part == "Due" {
                    due.into()
                } else {
                    "0.00".into()
                },
            );
        }
    }
    map.insert("frm2200Tv2020:txtSched1_TotalDue".into(), "600.00".into());
    map.insert("frm2200Tv2020:txtExciseDue".into(), "600.00".into());

    let doc = filled_document("2200t-2020", &map).unwrap();
    assert_eq!(printed_cell(&doc, "p2c27").0, "100.00");
    assert_eq!(printed_cell(&doc, "p2c32").0, "200.00");
    assert_eq!(printed_cell(&doc, "p2c42").0, "300.00");
    assert_eq!(printed_cell(&doc, "p3c154").0, "600.00");
    assert_eq!(printed_cell(&doc, "p1c900").0, "600");
    assert_eq!(printed_cell(&doc, "p1c901").0, "00");
}

#[test]
fn form_2200t_long_registered_name_prints_whole_over_the_comb() {
    let text = "A".repeat(70);
    let mut map = std::collections::BTreeMap::new();
    map.insert("frm2200Tv2020:registeredName".to_string(), text.clone());
    let doc = filled_document("2200t-2020", &map).unwrap();
    assert_eq!(printed_cell(&doc, "p1c26").0, text);
}
