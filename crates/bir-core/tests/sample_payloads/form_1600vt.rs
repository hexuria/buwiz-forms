use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1600vt::{
    Form1600VtAgentCategory, Form1600VtDraft, Form1600VtScheduleRow, form_1600vt_atc_index,
};

fn sample_1600vt() -> Form1600VtDraft {
    let mut draft =
        Form1600VtDraft::new_from_profile(&dummy_profile("1600VT", "Corporation"), 2025, 6);
    // Long enough for the official 80-character split into two lines.
    draft.registered_address = "Unit 1201 Sample Tower, 123 Sample Street corner Example Avenue, Barangay Example, Quezon City".into();
    draft.is_amended = true;
    draft.tax_relief = true;
    draft.tax_relief_specify = "Special law exemption sample".into();
    draft.set_agent_category(Form1600VtAgentCategory::Government);
    draft.set_taxes_withheld(true);
    for (code, rate) in [("WV010", "5.0"), ("WV020", "5.0"), ("WV110", "12.0")] {
        draft
            .toggle_atc(form_1600vt_atc_index(code, rate).unwrap())
            .unwrap();
    }
    draft.atc_rows[0].tax_base = 123_456.789;
    draft.atc_rows[1].tax_base = 50_000.005;
    draft.atc_rows[2].tax_base = 8_333.33;
    draft.tax_remitted_previous = 1_000.0;
    draft.other_payments = 250.5;
    draft.surcharge = 125.125;
    draft.interest = 10.0;
    draft.schedule = vec![
        Form1600VtScheduleRow {
            tin: "987654321000".into(),
            payee_name: "Sample Supplier Inc".into(),
            atc: "wv010".into(),
            income_payment: 100_000.0,
            tax_rate: 5.0,
            tax_withheld: 0.0,
        },
        Form1600VtScheduleRow {
            tin: "111222339000".into(),
            payee_name: "Dela Cruz, Sample Payee".into(),
            atc: "WV020".into(),
            income_payment: 20_100.1,
            tax_rate: 5.0,
            tax_withheld: 0.0,
        },
    ];
    draft.recompute();
    draft
}

#[test]
fn form_1600vt_sample_payload_is_current() {
    let draft = sample_1600vt();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1600VT must validate: {errors:?}"));
    check_sample(
        "1600VT-062025",
        "1600vt-v2018-government",
        &payload,
        "d596ad6dec862ff67d13315e03999782d3711becdfa4222b25d8a6fbfcc22d68",
    );
}

/// A private withholding agent: its ATC popup has 10 rows, not 12.
fn sample_1600vt_private() -> Form1600VtDraft {
    let mut draft =
        Form1600VtDraft::new_from_profile(&dummy_profile("1600VT", "Corporation"), 2025, 7);
    draft.set_agent_category(Form1600VtAgentCategory::Private);
    draft.set_taxes_withheld(true);
    for (code, rate) in [("WV050", "12.0"), ("WV080", "12.0")] {
        draft
            .toggle_atc(form_1600vt_atc_index(code, rate).unwrap())
            .unwrap();
    }
    draft.atc_rows[0].tax_base = 41_666.665;
    draft.atc_rows[1].tax_base = 1_000_000.0;
    draft.compromise = 1_000.0;
    draft.recompute();
    draft
}

#[test]
fn form_1600vt_private_sample_payload_is_current() {
    let draft = sample_1600vt_private();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1600VT must validate: {errors:?}"));
    check_sample(
        "1600VT-072025",
        "1600vt-v2018-private",
        &payload,
        "597795b4e6382a69401893c9385ad8b582d98e05e0ae775f5b6e13f9fdce5a37",
    );
}
