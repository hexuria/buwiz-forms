//! The decimal-drop fault: a count-labelled box bound to a decimal amount
//! (2200M's Volume Removed, in metric tons) was typed `int` on the page,
//! whose digits-only mask dropped the decimal point of a pasted
//! "9,876,543.21" and stored 987654321 — a hundred times what was typed,
//! without a word. These tests pin the fix's layers: the schema typing (a
//! decimal amount is never the digits-only box), the write-path parsers (an
//! amount keeps its decimals; a whole number refuses them), and 2200M's own
//! field through the screen's write path.

use super::*;
use bir_core::forms::form_2200m::{Form2200MDraft, Form2200MRow};

fn profile() -> TaxpayerProfile {
    serde_json::from_value(json!({
        "id": null,
        "full_name": "Example Trading Corp",
        "tin": {"segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000"},
        "rdo_code": "039",
        "line_of_business": "Wholesale trading",
        "registered_address": "1 Example Street, Quezon City",
        "zip_code": "1100",
        "phone": "0281234567",
        "email": "books@example.com",
        "default_form_type": "0605",
        "taxpayer_type": "Corporation",
        "birth_date": "1980-02-03",
        "business_start_date": "2015-01-02",
    }))
    .expect("the test profile parses")
}

fn page_types(code: &str) -> BTreeMap<String, String> {
    let mut types = BTreeMap::new();
    let Some(form) = schema_entry(code) else {
        return types;
    };
    for item in form["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array().into_iter().flatten())
    {
        for field in item["f"].as_array().into_iter().flatten() {
            if let (Some(key), Some(t)) = (field["k"].as_str(), field["t"].as_str()) {
                types
                    .entry(key.to_string())
                    .or_insert_with(|| t.to_string());
            }
        }
        for row in item["keys"].as_array().into_iter().flatten() {
            for (c, key) in row.as_array().into_iter().flatten().enumerate() {
                if let (Some(key), Some(t)) = (key.as_str(), item["cols"][c]["t"].as_str()) {
                    types
                        .entry(key.to_string())
                        .or_insert_with(|| t.to_string());
                }
            }
        }
    }
    types
}

/// 2200M's Volume Removed is a decimal input: the auditor's "9,876,543.21"
/// keeps its point there. A true count — attached sheets — stays the
/// digits-only box.
#[test]
fn form_2200m_volume_types_as_num_and_counts_stay_int() {
    setup();
    let types = page_types("2200M");
    assert_eq!(
        types
            .get("frm2200M:txtSched1_VOMRITaxableA1")
            .map(String::as_str),
        Some("num"),
        "the taxable volume is a decimal input"
    );
    assert_eq!(
        types
            .get("frm2200M:txtSched1_VOMRIExemptB1")
            .map(String::as_str),
        Some("num")
    );
}

/// The class, pinned over every form: no box bound to a decimal amount is
/// ever typed `int` (the digits-only mask), whatever its path or label
/// says. Whole bindings may be.
#[test]
fn no_amount_bound_box_types_as_int_on_any_form() {
    setup();
    let mut offenders = Vec::new();
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let types = page_types(code);
        let def = kind.definition();
        for (key, t) in &types {
            if t != "int" {
                continue;
            }
            if let Some(bound) = def.bindings.get(key) {
                if matches!(bound.binding, Binding::Amount { .. }) {
                    offenders.push(format!("{code}:{key}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "amount bindings typed int: {offenders:?}"
    );
}

/// The amount parser: commas and spaces are separators, one decimal point
/// is the fraction, a trailing point is no fraction, a leading minus
/// negates. Nothing is ever concatenated into another number.
#[test]
fn amount_parser_keeps_the_decimal_point() {
    assert_eq!(agent::parse_amount("9,876,543.21"), Some(9876543.21));
    assert_eq!(agent::parse_amount("1,234.50"), Some(1234.50));
    assert_eq!(agent::parse_amount("1234."), Some(1234.0));
    assert_eq!(agent::parse_amount(" 1,234.50 "), Some(1234.50));
    assert_eq!(agent::parse_amount("-1234.50"), Some(-1234.50));
}

/// A whole-number box refuses text with a decimal part instead of dropping
/// the point: the same refusal the auditor saw on 1702MX, pinned here on
/// 0605's sheets count through the screen's write path.
#[test]
fn whole_boxes_refuse_a_decimal_part() {
    setup();
    let kind = kind_of("0605").unwrap();
    let blank = bir_core::forms::form_0605::Form0605Draft::new_from_profile(&profile(), 2026, 1);
    let mut draft = serde_json::to_value(blank).expect("a blank 0605 draft");
    let key = "frm0605:txtNoOfSheets";
    for text in ["2345678.91", "1,234.50", "1234."] {
        let result = agent::fill_as_user(kind, &mut draft, key, &json!(text));
        assert!(
            result.is_err(),
            "{text} must be refused as a whole number, never stored as concatenated digits"
        );
    }
    assert!(agent::fill_as_user(kind, &mut draft, key, &json!("3")).is_ok());
}

/// 2200M's volume field through the screen's write path: with the mask no
/// longer eating the point, "9,876,543.21" reaches the writer as a decimal
/// and the draft keeps it.
#[test]
fn form_2200m_volume_keeps_decimals_through_the_write_path() {
    setup();
    let kind = kind_of("2200M").unwrap();
    let mut typed = Form2200MDraft::new_from_profile(&profile(), 2026, 1);
    typed.schedule.push(Form2200MRow::default());
    let mut draft = serde_json::to_value(&typed).expect("a one-row 2200M draft");
    agent::fill_as_user(
        kind,
        &mut draft,
        "frm2200M:txtSched1_VOMRITaxableA1",
        &json!("9,876,543.21"),
    )
    .unwrap_or_else(|why| panic!("the volume takes decimals: {why}"));
    assert_eq!(draft["schedule"][0]["volume_taxable"], json!(9876543.21));
}
