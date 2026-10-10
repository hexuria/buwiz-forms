use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1600wp::{
    Form1600WpAgentCategory, Form1600WpDraft, Form1600WpScheduleRow,
};

fn draft(month: u8, day: u8) -> Form1600WpDraft {
    let mut draft =
        Form1600WpDraft::new_from_profile(&dummy_profile("1600WP", "Corporation"), 2025, month);
    draft.day = day;
    draft
}

/// Government agent, both ATCs, an amended return and Schedule II rows.
fn sample_two_atcs() -> Form1600WpDraft {
    let mut draft = draft(6, 15);
    draft.is_amended = true;
    draft.set_taxes_withheld(true);
    draft.set_agent_category(Form1600WpAgentCategory::Government);
    draft.toggle_atc("WB191").unwrap();
    draft.toggle_atc("WB192").unwrap();
    draft.atc_rows[0].tax_base = 123_456.789;
    draft.atc_rows[1].tax_base = 50_000.015;
    draft.tax_remitted_previous = 1_000.0;
    draft.surcharge = 125.125;
    draft.interest = 10.0;
    draft.schedule = vec![
        Form1600WpScheduleRow {
            tin: "987654321000".into(),
            payee_name: "Dela Cruz, Juan Sample".into(),
            atc_code: "WB191".into(),
            amount: 100_000.0,
            tax_withheld: 0.0,
        },
        Form1600WpScheduleRow {
            tin: "111222339000".into(),
            payee_name: "Sample Stables Inc".into(),
            atc_code: "WB192".into(),
            amount: 20_100.105,
            tax_withheld: 0.0,
        },
    ];
    draft.recompute();
    draft
}

/// Private agent, one ATC.
fn sample_one_atc() -> Form1600WpDraft {
    let mut draft = draft(7, 1);
    draft.set_taxes_withheld(true);
    draft.set_agent_category(Form1600WpAgentCategory::Private);
    draft.toggle_atc("WB194").unwrap();
    draft.atc_rows[0].tax_base = 7_777.775;
    draft.compromise = 1_000.0;
    draft.recompute();
    draft
}

/// No tax withheld.
fn sample_no_atc() -> Form1600WpDraft {
    let mut draft = draft(8, 31);
    draft.set_taxes_withheld(false);
    draft.set_agent_category(Form1600WpAgentCategory::Private);
    draft.recompute();
    draft
}

fn check(draft: Form1600WpDraft, stem: &str, layout: &str, sha: &str) {
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1600WP must validate: {errors:?}"));
    assert_eq!(draft.layout_id(), layout);
    check_sample(stem, layout, &payload, sha);
}

#[test]
fn form_1600wp_two_atc_sample_payload_is_current() {
    check(
        sample_two_atcs(),
        "1600WP-06152025",
        "1600wp-v2010-atc2",
        "e0af6176cf74f1e034f1f8ecd1089d436ea933fd3f07c0c9257103b5264beaaf",
    );
}

#[test]
fn form_1600wp_one_atc_sample_payload_is_current() {
    check(
        sample_one_atc(),
        "1600WP-07012025",
        "1600wp-v2010-atc1",
        "2ed0afc32305592567d6b2eaec4cacbb1e7ed9673ec7c419d7c70348f15b3850",
    );
}

#[test]
fn form_1600wp_no_atc_sample_payload_is_current() {
    check(
        sample_no_atc(),
        "1600WP-08312025",
        "1600wp-v2010-atc0",
        "e2033c2abd9d04a53c2362fce89ed60dd5393d8d3a6815bb5b9b0fc64351718a",
    );
}
