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
    assert_eq!(printed_cell(&doc, "p1c17").0, "00001");
    assert_eq!(printed_cell(&doc, "p2c5").0, "00012345300001");
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
