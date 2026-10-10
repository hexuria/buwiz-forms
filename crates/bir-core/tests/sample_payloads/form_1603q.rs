use crate::dummy_profile;
use crate::sample_payloads::form_1601eq::check_runtime_sample;
use bir_core::forms::form_1603q::{Form1603QCategory, Form1603QDraft, Form1603QTaxRelief};

fn sample_1603q() -> Form1603QDraft {
    let mut draft =
        Form1603QDraft::new_from_profile(&dummy_profile("1603Q", "Corporation"), 2025, 3);
    draft.category = Some(Form1603QCategory::Government);
    draft.tax_relief = Some(Form1603QTaxRelief::Both);
    draft.set_any_tax_withheld(true);
    draft.monetary_value = [100_000.005, 54_321.1];
    draft.other_remittances_specify = "Bank remittance".into();
    draft.other_remittances = 1_000.505;
    draft.surcharge = 25.5;
    draft.interest = 10.0;
    draft.recompute();
    draft
}

#[test]
fn form_1603q_sample_payload_is_current() {
    let draft = sample_1603q();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1603Q must validate: {errors:?}"));
    check_runtime_sample(
        "1603Q-2025Q3",
        &draft.official_layout().unwrap(),
        &payload,
        "c058fcce0701eac3fe863031a41ed31fb5e5e594c8763fa391127aa076a4fb45",
    );
}
