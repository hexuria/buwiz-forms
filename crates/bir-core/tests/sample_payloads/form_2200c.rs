use crate::sample_payloads::penalty_items::{self, Deadline, day};
use crate::{check_sample, dummy_profile};
use bir_core::forms::excise_places::ExcisePlace;
use bir_core::forms::form_2200c::{Form2200CDraft, Form2200CManner, Form2200CRow};
use bir_core::forms::queueable::QueueableForm as _;

fn sample_2200c() -> Form2200CDraft {
    let mut draft =
        Form2200CDraft::new_from_profile(&dummy_profile("2200Cv2018", "Corporation"), 2025);
    draft.month = 6;
    draft.day = 30;
    draft.is_amended = true;
    draft.previous_payment = 400.0;
    draft.place = ExcisePlace {
        region: "130000000".into(),
        province: "137400000".into(),
        city: "137404000".into(),
    };
    draft.tax_relief = Some(true);
    draft.tax_relief_specify = "special law sample".into();
    draft.manner = Form2200CManner::ActualRemoval;
    let row = |a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, g: f64| Form2200CRow {
        exempt_procedures: a,
        excisable_procedures: b,
        non_excisable_procedures: c,
        net_of_vat: d,
        excisable_net: e,
        excisable_vat_exempt: f,
        non_excisable: g,
        ..Default::default()
    };
    draft.schedule = vec![
        row(1.0, 3.0, 2.0, 10_000.0, 50_010.0, 0.0, 5_000.0),
        row(2.0, 5.0, 1.0, 0.0, 12_345.0, 6_789.0, 0.0),
        row(1.0, 1.0, 1.0, 2_500.0, 999.0, 1.0, 0.0),
    ];
    draft.balance_carried_over = 100.0;
    draft.creditable_excise_tax = 8.0;
    draft.surcharge = 25.0;
    draft.compromise = 10.0;
    draft.keep_entered_penalties();
    draft.tax_payment = 3_000.0;
    draft.recompute();
    draft
}

#[test]
fn form_2200c_sample_payload_is_current() {
    let draft = sample_2200c();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 2200-C must validate: {errors:?}"));
    check_sample(
        "2200C-06302025",
        "2200c-v2018",
        &payload,
        "71c985b171d1b0c4a0e1e34a213deb0428882ab572fbec4b554c29ac1c425826",
    );
}

/// Item 1 is a date the calendar's monthly 2200-C rule is not keyed to.
#[test]
fn form_2200c_penalty_items_follow_the_shared_model() {
    penalty_items::check(
        sample_2200c,
        Deadline::None {
            assumed: day(2025, 12, 31),
        },
    );
}

/// Every sample draft above, for the agent-form probe (`agent_forms`).
pub(crate) fn agent_seeds() -> Vec<serde_json::Value> {
    vec![serde_json::to_value(sample_2200c()).expect("sample serializes")]
}
