use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1600pt::{
    Form1600PtAgentCategory, Form1600PtDraft, Form1600PtScheduleRow, form_1600pt_atc_index,
};

fn sample_1600pt() -> Form1600PtDraft {
    let mut draft =
        Form1600PtDraft::new_from_profile(&dummy_profile("1600PT", "Corporation"), 2025, 6);
    // Long enough for the official 80-character split into two lines.
    draft.registered_address = "Unit 1201 Sample Tower, 123 Sample Street corner Example Avenue, Barangay Example, Quezon City".into();
    draft.is_amended = true;
    draft.tax_relief = true;
    draft.tax_relief_specify = "Special law exemption sample".into();
    draft.set_agent_category(Form1600PtAgentCategory::Government);
    draft.set_taxes_withheld(true);
    for (code, rate) in [("WB030", "3.0"), ("WB080", "1.0"), ("WB200", "0.6")] {
        draft
            .toggle_atc(form_1600pt_atc_index(code, rate).unwrap())
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
        Form1600PtScheduleRow {
            tin: "987654321000".into(),
            payee_name: "Sample Supplier Inc".into(),
            atc: "wb030".into(),
            income_payment: 100_000.0,
            tax_rate: 5.0,
            tax_withheld: 0.0,
        },
        Form1600PtScheduleRow {
            tin: "111222339000".into(),
            payee_name: "Dela Cruz, Sample Payee".into(),
            atc: "WB080".into(),
            income_payment: 20_100.1,
            tax_rate: 5.0,
            tax_withheld: 0.0,
        },
    ];
    draft.recompute();
    draft
}

#[test]
fn form_1600pt_sample_payload_is_current() {
    let draft = sample_1600pt();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1600PT must validate: {errors:?}"));
    check_sample(
        "1600PT-062025",
        "1600pt-v2018-government",
        &payload,
        "ed181b39116b9cc2bda3bc54185e7a4d982f6ee8fe9a9fe3df80090c96c05eeb",
    );
}

/// A private withholding agent: its ATC popup has 6 rows, not 31.
fn sample_1600pt_private() -> Form1600PtDraft {
    let mut draft =
        Form1600PtDraft::new_from_profile(&dummy_profile("1600PT", "Corporation"), 2025, 7);
    draft.set_agent_category(Form1600PtAgentCategory::Private);
    draft.set_taxes_withheld(true);
    for (code, rate) in [("WB082", "3.0"), ("WB084", "1.0")] {
        draft
            .toggle_atc(form_1600pt_atc_index(code, rate).unwrap())
            .unwrap();
    }
    draft.atc_rows[0].tax_base = 41_666.665;
    draft.atc_rows[1].tax_base = 1_000_000.0;
    draft.compromise = 1_000.0;
    draft.recompute();
    draft
}

#[test]
fn form_1600pt_private_sample_payload_is_current() {
    let draft = sample_1600pt_private();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1600PT must validate: {errors:?}"));
    check_sample(
        "1600PT-072025",
        "1600pt-v2018-private",
        &payload,
        "05a5d8404320cf7bcbdd1021e8dcb16173486a2a19cd1353130024cf2fac43d2",
    );
}
