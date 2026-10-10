use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1604e::{Form1604eAgentCategory, Form1604eDraft, Form1604eRemittance};

fn row(date: &str, bank: &str, reference: &str, tax: f64, penalty: f64) -> Form1604eRemittance {
    Form1604eRemittance {
        date: date.to_string(),
        bank: bank.to_string(),
        reference: reference.to_string(),
        taxes_withheld: tax,
        penalties: penalty,
        total_remitted: 0.0,
    }
}

fn sample_1604e() -> Form1604eDraft {
    let mut draft = Form1604eDraft::new_from_profile(&dummy_profile("1604E", "Corporation"), 2025);
    draft.agent_category = Form1604eAgentCategory::Private;
    draft.top_withholding_agent = Some(false);
    draft.schedule1[0] = row("04/30/2025", "SampleBank01", "traQ1", 12_345.675, 100.005);
    draft.schedule1[1] = row("07/31/2025", "Agency2", "eROR2", 999.995, 0.0);
    draft.schedule1[3] = row("01/30/2026", "bank4", "Ear4", 250_000.0, 0.0);
    draft.schedule2[0] = row("02/10/2025", "BankJan", "TRA0101", 1_000.5, 0.5);
    draft.schedule2[5] = row("07/10/2025", "bankjun", "tra06", 2_500.125, 0.0);
    draft.schedule2[11] = row("01/15/2026", "BANKDEC", "TRA12", 333.333, 12.0);
    draft.recompute();
    draft
}

#[test]
fn form_1604e_sample_payload_is_current() {
    let draft = sample_1604e();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1604E must validate: {errors:?}"));
    check_sample(
        "1604E-2025",
        "1604e-v2018",
        &payload,
        "6750ccb229caf518675c04d024244ac3b747f0a491167e9432645d8764be6b17",
    );
}
