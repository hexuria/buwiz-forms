//! Print coverage gate: every value a form's print map carries prints in
//! its box on the frozen sheet, or `writer-cells.json` lists the key in
//! `not_joined` with the reason it has no box.
//!
//! Each form is checked on two drafts built the way its page builds them:
//! the draft a new return opens with (profile, period and computed
//! defaults), and the same draft with every box the assistant can write
//! filled in, so typed text, dates and schedule rows are covered too.

use std::collections::{BTreeMap, BTreeSet};

use bir_core::db::Database;
use bir_core::forms::agent::{self, AgentFormKind, Binding};
use bir_core::forms::registry::FilingFrequency;
use bir_core::profile::TaxpayerProfile;
use serde_json::Value;

use super::queueable_forms::{print_sheet, spec_for_kind};

const YEAR: u16 = 2025;

/// The sample payloads' dummy taxpayer (`bir-core` tests): TIN 123-456-788
/// passes the official check digit; every value is fictitious.
fn dummy_profile(code: &str) -> TaxpayerProfile {
    serde_json::from_value(serde_json::json!({
        "id": null,
        "full_name": "Sample Dummy Taxpayer",
        "tin": {"segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000"},
        "rdo_code": "039",
        "line_of_business": "Sample Consulting Services",
        "registered_address": "123 Sample Street, Barangay Example, Quezon City",
        "zip_code": "1100",
        "phone": "09170000000",
        "email": "sample.taxpayer@example.com",
        "default_form_type": code,
        "taxpayer_type": "Corporation",
        "business_start_date": "2020-01-15"
    }))
    .expect("dummy profile")
}

/// The period a page opens on: June, the second quarter, the annual return
/// or the first open-ended return.
fn period(code: &str) -> u8 {
    match bir_core::forms::find_form(code).map(|form| form.frequency.clone()) {
        Some(FilingFrequency::Monthly) => 6,
        Some(FilingFrequency::Quarterly) => 2,
        Some(FilingFrequency::Annual) => 0,
        _ => 1,
    }
}

/// The draft a new return of `kind` opens with, as JSON.
fn opened_draft(kind: AgentFormKind) -> Value {
    let code = kind.form_code();
    let profile = dummy_profile(code);
    let period = period(code);
    let mut draft = match kind {
        AgentFormKind::Form1601C => serde_json::to_value(
            bir_core::forms::form_1601c::Form1601CDraft::new_unanswered_from_profile(
                &profile, YEAR, period,
            ),
        ),
        AgentFormKind::Form2551Q => serde_json::to_value(
            bir_core::forms::form_2551q::Form2551QDraft::new_from_profile(&profile, YEAR, period),
        ),
        AgentFormKind::Queueable(bir_core::forms::queueable::QueueableKind::Form2550Q) => {
            serde_json::to_value(
                bir_core::forms::form_2550q::Form2550QDraft::new_from_profile(
                    &profile, YEAR, period,
                ),
            )
        }
        AgentFormKind::Queueable(queueable) => {
            let db = Database::open_ephemeral().expect("ephemeral db");
            let spec = spec_for_kind(queueable).expect("form page");
            Ok((spec.load)(&db, &profile, YEAR, period))
        }
    }
    .expect("draft serializes");
    agent::compute(kind, &mut draft).expect("compute");
    draft
}

/// `draft` with every box the assistant may write filled: text `SAMPLE`
/// (dates `06/15/2025`), amounts 1,234.50 and counts 1. Choices keep the
/// page's answer; their boxes print from the opened draft already.
fn filled_draft(kind: AgentFormKind, mut draft: Value) -> Value {
    let def = kind.definition();
    for entry in def.fillable() {
        let Some(bound) = def.bindings.get(&entry.key) else {
            continue;
        };
        let value = match &bound.binding {
            Binding::Text { date: Some(_), .. } => "06/15/2025",
            Binding::Text { .. } => "SAMPLE",
            Binding::Amount { .. } => "1234.50",
            Binding::Whole { .. } => "1",
            _ => continue,
        };
        // Some boxes refuse a value in this state; the rest still fill.
        let _ = agent::fill(kind, &mut draft, &entry.key, &Value::from(value));
    }
    agent::compute(kind, &mut draft).expect("compute");
    draft
}

fn unprinted(kind: AgentFormKind, draft: &Value) -> (&'static str, Vec<String>) {
    let (slug, fields) = print_sheet(kind, draft).expect("print map");
    let missing = bir_print::frozen_html::unprinted_keys(slug, &fields)
        .unwrap_or_else(|error| panic!("{slug}: {error}"));
    (slug, missing)
}

fn assert_prints_every_value(code: &str) {
    let kind = AgentFormKind::from_code(code).expect("form code");
    let opened = opened_draft(kind);
    let (slug, mut missing) = unprinted(kind, &opened);
    missing.extend(unprinted(kind, &filled_draft(kind, opened)).1);
    let missing: BTreeSet<String> = missing.into_iter().collect();
    assert!(
        missing.is_empty(),
        "{code} ({slug}): values with no box and no not_joined reason in writer-cells.json: {missing:#?}"
    );
}

/// The print map of the opened draft and of the filled one, for the
/// repeated-header checks. 1706's page 2 header is the buyer, whom a new
/// return has not entered yet: the filled draft names one.
fn print_maps(code: &str) -> Vec<(&'static str, BTreeMap<String, String>)> {
    let kind = AgentFormKind::from_code(code).expect("form code");
    let opened = opened_draft(kind);
    let mut filled = filled_draft(kind, opened.clone());
    if code == "1706" {
        filled["buyer_tin"] = Value::from("987654321");
        filled["buyer_branch_code"] = Value::from("00000");
        filled["buyer_name"] = Value::from("Buyer Fixture");
    }
    [opened, filled]
        .iter()
        .map(|draft| print_sheet(kind, draft).expect("print map"))
        .collect()
}

macro_rules! print_coverage_tests {
    ($($test:ident => $code:literal),* $(,)?) => {
        $(
            #[test]
            fn $test() {
                assert_prints_every_value($code);
            }
        )*

        #[test]
        fn every_form_has_a_print_coverage_test() {
            let listed: BTreeSet<&str> = [$($code),*].into_iter().collect();
            for kind in AgentFormKind::all() {
                assert!(listed.contains(kind.form_code()), "{} has no coverage test", kind.form_code());
            }
        }
    };
}

print_coverage_tests! {
    prints_every_value_0605 => "0605",
    prints_every_value_0619e => "0619E",
    prints_every_value_0619f => "0619F",
    prints_every_value_1600pt => "1600PT",
    prints_every_value_1600vt => "1600VT",
    prints_every_value_1600wp => "1600WP",
    prints_every_value_1601c => "1601C",
    prints_every_value_1601eq => "1601EQ",
    prints_every_value_1601fq => "1601FQ",
    prints_every_value_1602q => "1602Q",
    prints_every_value_1603q => "1603Q",
    prints_every_value_1604c => "1604C",
    prints_every_value_1604e => "1604E",
    prints_every_value_1604f => "1604F",
    prints_every_value_1606 => "1606",
    prints_every_value_1700 => "1700",
    prints_every_value_1701 => "1701",
    prints_every_value_1701a => "1701A",
    prints_every_value_1701ms => "1701MS",
    prints_every_value_1701q => "1701Q",
    prints_every_value_1702ex => "1702EX",
    prints_every_value_1702mx => "1702MX",
    prints_every_value_1702q => "1702Q",
    prints_every_value_1702rt => "1702RT",
    prints_every_value_1706 => "1706",
    prints_every_value_1707 => "1707",
    prints_every_value_1707a => "1707A",
    prints_every_value_1800 => "1800",
    prints_every_value_1801 => "1801",
    prints_every_value_2000 => "2000",
    prints_every_value_2000ot => "2000OT",
    prints_every_value_2200a => "2200A",
    prints_every_value_2200an => "2200AN",
    prints_every_value_2200c => "2200C",
    prints_every_value_2200m => "2200M",
    prints_every_value_2200p => "2200P",
    prints_every_value_2200s => "2200S",
    prints_every_value_2200t => "2200T",
    prints_every_value_2550m => "2550M",
    prints_every_value_2550q => "2550Q",
    prints_every_value_2551q => "2551Q",
    prints_every_value_2552 => "2552",
    prints_every_value_2553 => "2553",
}

/// A multi-page sheet's identity boxes: where page 1 prints the taxpayer's
/// name and TIN, and each later page's header TIN and name cells. A header
/// with one cell for both (`tin == name`) is a 40-slot comb: TIN digits in
/// slots 0-13, the name from slot 14.
struct Header {
    code: &'static str,
    page1_name: &'static [&'static str],
    page1_tin: &'static [&'static str],
    later: &'static [(&'static str, &'static str)],
}

const HEADERS: &[Header] = &[
    Header {
        code: "1600PT",
        page1_name: &["p1c26"],
        page1_tin: &["p1c16", "p1c18", "p1c20", "p1c22"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1600VT",
        page1_name: &["p1c26"],
        page1_tin: &["p1c16", "p1c18", "p1c20", "p1c22"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1601C",
        page1_name: &["p1c36"],
        page1_tin: &[
            "frm1601c:txtTIN1",
            "frm1601c:txtTIN2",
            "frm1601c:txtTIN3",
            "frm1601c:txtBranchCode",
        ],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1601EQ",
        page1_name: &["p1c30"],
        page1_tin: &["p1c20", "p1c22", "p1c24", "p1c26"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1601FQ",
        page1_name: &["p1c37"],
        page1_tin: &["p1c27", "p1c29", "p1c31", "p1c33"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1602Q",
        page1_name: &["p1c37"],
        page1_tin: &["p1c27", "p1c29", "p1c31", "p1c33"],
        later: &[("p2c5", "p2c6"), ("p3c5", "p3c6")],
    },
    Header {
        code: "1603Q",
        page1_name: &["p1c37"],
        page1_tin: &["p1c27", "p1c29", "p1c31", "p1c33"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1604E",
        page1_name: &["p1c22"],
        page1_tin: &["p1c12", "p1c14", "p1c16", "p1c18"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1604F",
        page1_name: &["p1c22"],
        page1_tin: &["p1c12", "p1c14", "p1c16", "p1c18"],
        later: &[("p2c5", "p2c6")],
    },
    // The page 2 header names the buyer (items 6A/7A on page 1).
    Header {
        code: "1606",
        page1_name: &["p1c25"],
        page1_tin: &["p1c27"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1706",
        page1_name: &["p1c47"],
        page1_tin: &["p1c49"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1700",
        page1_name: &["p1c23"],
        page1_tin: &["p1c14", "p1c16", "p1c18"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1701",
        page1_name: &["p1c37"],
        page1_tin: &[
            "frm1701:txtPg1I4TIN1",
            "frm1701:txtPg1I4TIN2",
            "frm1701:txtPg1I4TIN3",
        ],
        later: &[("p2c5", "p2c6"), ("p3c5", "p3c6"), ("p4c5", "p4c6")],
    },
    Header {
        code: "1701A",
        page1_name: &["p1c32"],
        page1_tin: &["p1c16", "p1c18", "p1c20"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1701MS",
        page1_name: &["p1c29"],
        page1_tin: &["p1c23"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1701Q",
        page1_name: &["p1c39"],
        page1_tin: &["frm1701q:txtTIN1", "frm1701q:txtTIN2", "frm1701q:txtTIN3"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1702EX",
        page1_name: &["p1c36", "p1c37", "p1c38"],
        page1_tin: &["p1c25", "p1c27", "p1c29"],
        later: &[("p2c5", "p2c6"), ("p3c5", "p3c6")],
    },
    Header {
        code: "1702MX",
        page1_name: &["p1c36", "p1c37", "p1c38"],
        page1_tin: &[
            "frm1702MX:txtPg1Pt1I6TINC1",
            "frm1702MX:txtPg1Pt1I6TINC2",
            "frm1702MX:txtPg1Pt1I6TINC3",
        ],
        later: &[("p2c5", "p2c6"), ("p3c5", "p3c6"), ("p4c5", "p4c6")],
    },
    Header {
        code: "1702Q",
        page1_name: &["p1c37", "p1c38"],
        page1_tin: &["p1c26", "p1c28", "p1c30"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1702RT",
        page1_name: &["p1c36", "p1c37", "p1c38"],
        page1_tin: &[
            "frm1702RT:txtPg1Pt1I6TIN1",
            "frm1702RT:txtPg1Pt1I6TIN2",
            "frm1702RT:txtPg1Pt1I6TIN3",
        ],
        later: &[("p2c5", "p2c6"), ("p3c5", "p3c6"), ("p4c5", "p4c6")],
    },
    Header {
        code: "1707A",
        page1_name: &["p1c32"],
        page1_tin: &["p1c21", "p1c23", "p1c25"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "1800",
        page1_name: &["p1c22"],
        page1_tin: &["p1c13", "p1c15", "p1c16"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "2000",
        page1_name: &["p1c23"],
        page1_tin: &["p1c13", "p1c15", "p1c17", "p1c19"],
        later: &[("p2c5", "p2c5")],
    },
    Header {
        code: "2000OT",
        page1_name: &["p1c22"],
        page1_tin: &["p1c18"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "2200A",
        page1_name: &["p1c26"],
        page1_tin: &["p1c16", "p1c18", "p1c20", "p1c22"],
        later: &[("p2c5", "p2c5"), ("p3c5", "p3c5")],
    },
    Header {
        code: "2200AN",
        page1_name: &["p1c26"],
        page1_tin: &["p1c16", "p1c18", "p1c20", "p1c22"],
        later: &[("p2c4", "p2c5")],
    },
    Header {
        code: "2200C",
        page1_name: &["p1c28"],
        page1_tin: &["p1c18", "p1c20", "p1c22", "p1c24"],
        later: &[("p2c5", "p2c5")],
    },
    Header {
        code: "2200M",
        page1_name: &["p1c26"],
        page1_tin: &["p1c16", "p1c18", "p1c20", "p1c22"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "2200P",
        page1_name: &["p1c26"],
        page1_tin: &["p1c16", "p1c18", "p1c20", "p1c22"],
        later: &[("p2c5", "p2c5")],
    },
    Header {
        code: "2200S",
        page1_name: &["p1c25"],
        page1_tin: &["p1c15", "p1c17", "p1c19", "p1c21"],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "2200T",
        page1_name: &["p1c26"],
        page1_tin: &["p1c16", "p1c18", "p1c20", "p1c22"],
        later: &[("p2c4", "p2c5"), ("p3c4", "p3c5")],
    },
    Header {
        code: "2550Q",
        page1_name: &["p1c39"],
        page1_tin: &[
            "frm2550qv2024:txtTIN1",
            "frm2550qv2024:txtTIN2",
            "frm2550qv2024:txtTIN3",
        ],
        later: &[("p2c5", "p2c6")],
    },
    Header {
        code: "2551Q",
        page1_name: &["p1c30"],
        page1_tin: &[
            "frm2551Qv2018:txtTIN1",
            "frm2551Qv2018:txtTIN2",
            "frm2551Qv2018:txtTIN3",
            "frm2551Qv2018:txtBranchCode",
        ],
        later: &[("p2c5", "p2c6")],
    },
];

/// Multi-page sheets left out of the header check, and why.
const NO_HEADER_CHECK: &[(&str, &str)] = &[
    ("0605", "page 2 is the guidelines text: no header boxes"),
    (
        "2550M",
        "the 2007 page 2 has no TIN/name header, and page 1's TIN row has no inputs",
    ),
    ("2552", "page 2 has no TIN/name header"),
    ("2553", "page 2 has no fillable cells"),
    (
        "1707",
        "page 1 has no filer name or TIN box (items 6 and 7 are the parties); the page 2 header is checked against the draft",
    ),
    (
        "1801",
        "page 1's TIN row is mixed cells with captions inside and stays blank; the page 2 header is checked against the draft",
    ),
];

fn digits(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).collect()
}

fn header_parts(document: &str, tin: &str, name: &str) -> (String, String) {
    let printed = |cell: &str| bir_print::frozen_html::printed_text(document, cell);
    if tin == name {
        let line: Vec<char> = printed(tin).chars().collect();
        let split = line.len().min(14);
        (
            line[..split].iter().collect(),
            line[split..].iter().collect(),
        )
    } else {
        (printed(tin), printed(name))
    }
}

/// The repeated-header rule: every later page's header prints the TIN and
/// name page 1 prints (a header comb may hold fewer TIN digits, the branch
/// being preprinted, and a shorter name, cut at the comb width or at the
/// last name's comma).
#[test]
fn later_page_headers_repeat_the_page_one_name_and_tin() {
    let mut checked = BTreeSet::new();
    for header in HEADERS {
        // A party the draft has not entered yet (1706's buyer) prints blank
        // on both pages; some map must compare a printed name and TIN.
        let (mut names, mut tins) = (0, 0);
        for (slug, fields) in print_maps(header.code) {
            let document = bir_print::frozen_html::filled_document(slug, &fields)
                .unwrap_or_else(|error| panic!("{slug}: {error}"));
            let printed = |cell: &str| bir_print::frozen_html::printed_text(&document, cell);
            let name: String = header.page1_name.iter().map(|cell| printed(cell)).collect();
            let name = name.trim().to_string();
            let tin: String = header
                .page1_tin
                .iter()
                .map(|cell| digits(&printed(cell)))
                .collect();
            for (tin_cell, name_cell) in header.later {
                let (later_tin, later_name) = header_parts(&document, tin_cell, name_cell);
                let later_tin = digits(&later_tin);
                let later_name = later_name.trim().to_string();
                // Either side may stop after the nine TIN digits (a branch
                // the sheet preprints, or one with no input on that page).
                assert!(
                    (tin.starts_with(&later_tin) || later_tin.starts_with(&tin))
                        && later_tin.is_empty() == tin.is_empty(),
                    "{slug} {tin_cell}: header TIN {later_tin:?}, page 1 TIN {tin:?}"
                );
                assert!(
                    name.starts_with(&later_name) && later_name.is_empty() == name.is_empty(),
                    "{slug} {name_cell}: header name {later_name:?}, page 1 name {name:?}"
                );
                tins += usize::from(later_tin.len() >= 9);
                names += usize::from(later_name.len() >= 5);
            }
        }
        assert!(
            names > 0 && tins > 0,
            "{}: no draft printed a header name and TIN to compare",
            header.code
        );
        checked.insert(header.code);
    }
    // Every multi-page sheet is checked or listed with its reason.
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let opened = opened_draft(kind);
        let (slug, _) = print_sheet(kind, &opened).expect("print map");
        let document =
            bir_print::frozen_html::filled_document(slug, &BTreeMap::new()).expect("sheet");
        if document.contains("id=\"page-2\"") {
            assert!(
                checked.contains(code) || NO_HEADER_CHECK.iter().any(|(c, _)| *c == code),
                "{code} ({slug}) is multi-page: add it to HEADERS or NO_HEADER_CHECK"
            );
        }
    }
}

/// The two sheets whose page 1 has no filer TIN to compare: their page 2
/// header carries the draft's TIN and name.
#[test]
fn headers_without_a_page_one_tin_carry_the_draft_identity() {
    for (code, tin_cell, name_cell) in [("1707", "p2c5", "p2c6"), ("1801", "p2c5", "p2c6")] {
        for (slug, fields) in print_maps(code) {
            let document = bir_print::frozen_html::filled_document(slug, &fields).unwrap();
            let (tin, name) = header_parts(&document, tin_cell, name_cell);
            assert!(digits(&tin).starts_with("123456788"), "{slug}: {tin:?}");
            assert!(
                "SAMPLE DUMMY TAXPAYER".starts_with(name.trim()) && name.trim().len() >= 5,
                "{slug}: {name:?}"
            );
        }
    }
}
