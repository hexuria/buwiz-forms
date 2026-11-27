use bir_core::forms::form_1707a::Form1707ADraft;
use bir_core::forms::form_2000::Form2000Draft;
use bir_core::forms::form_2200a::Form2200ADraft;
use bir_core::forms::form_2200an::Form2200ANDraft;
use bir_core::forms::form_2200c::Form2200CDraft;
use bir_core::forms::form_2200p::Form2200PDraft;
use bir_core::forms::form_2200s::Form2200SDraft;
use bir_core::forms::queueable::QueueableForm;
use bir_core::profile::TaxpayerProfile;
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
        "default_form_type": "2000"
    }))
    .unwrap()
}

fn expected(branch: &str) -> String {
    format!("000123453{branch:0>5}")
}

fn first_fourteen(text: &str) -> String {
    text.chars().take(14).collect()
}

fn direct_case(
    slug: &str,
    cell: &str,
    map: std::collections::BTreeMap<String, String>,
    branch: &str,
) {
    let doc = filled_document(slug, &map).unwrap_or_else(|err| panic!("{slug} render: {err}"));
    assert_eq!(
        printed_cell(&doc, cell).0,
        expected(branch),
        "{slug} branch {branch}"
    );
}

fn header_case(
    slug: &str,
    cell: &str,
    map: std::collections::BTreeMap<String, String>,
    branch: &str,
) {
    let doc = filled_document(slug, &map).unwrap_or_else(|err| panic!("{slug} render: {err}"));
    assert_eq!(
        first_fourteen(&printed_cell(&doc, cell).0),
        expected(branch),
        "{slug} branch {branch}"
    );
}

fn branches() -> [&'static str; 3] {
    ["123", "1234", "12345"]
}

#[test]
fn form_2200s_page2_tin_left_pads_branch() {
    for branch in branches() {
        let d = Form2200SDraft::new_from_profile(&profile(branch), 2025, 3);
        direct_case("2200s-2018", "p2c5", d.print_field_map(), branch);
    }
}

#[test]
fn form_2200an_page2_tin_left_pads_branch() {
    for branch in branches() {
        let d = Form2200ANDraft::new_from_profile(&profile(branch), 2025, 3);
        direct_case("2200an-2018", "p2c4", d.print_field_map(), branch);
    }
}

#[test]
fn form_1707a_page2_tin_left_pads_branch() {
    for branch in branches() {
        let d = Form1707ADraft::new_from_profile(&profile(branch), 2025, 1);
        direct_case("1707a-2021", "p2c5", d.print_field_map(), branch);
    }
}

#[test]
fn form_2200a_header_tin_left_pads_branch() {
    for branch in branches() {
        let d = Form2200ADraft::new_from_profile(&profile(branch), 2025, 3);
        header_case("2200a-2020", "p2c5", d.print_field_map(), branch);
    }
}

#[test]
fn form_2200p_header_tin_left_pads_branch() {
    for branch in branches() {
        let d = Form2200PDraft::new_from_profile(&profile(branch), 2025, 3);
        header_case("2200p-2020", "p2c5", d.print_field_map(), branch);
    }
}

#[test]
fn form_2200c_header_tin_left_pads_branch() {
    for branch in branches() {
        let d = Form2200CDraft::new_from_profile(&profile(branch), 2025);
        header_case("2200c-2018", "p2c5", d.print_field_map(), branch);
    }
}

#[test]
fn form_2000_header_tin_left_pads_branch() {
    for branch in branches() {
        let d = Form2000Draft::new_from_profile(&profile(branch), 2025, 3);
        header_case("2000-dst-2018", "p2c5", d.print_field_map(), branch);
    }
}
