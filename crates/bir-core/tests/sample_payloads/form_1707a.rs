use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1707a::{Form1707AAtc, Form1707ADraft, Form1707ARow};

fn row(date: &str, name: &str, selling: f64, cost: f64, tax_paid: f64) -> Form1707ARow {
    Form1707ARow {
        date: date.into(),
        corporation: name.into(),
        selling_price: selling,
        cost,
        gain_or_loss: 0.0,
        tax_paid,
    }
}

fn sample_1707a() -> Form1707ADraft {
    let mut draft =
        Form1707ADraft::new_from_profile(&dummy_profile("1707A", "Individual"), 2025, 1);
    draft.set_atc(Form1707AAtc::Individual);
    draft.year_end_year = 2025;
    draft.number_of_attached_sheets = 1;
    draft.tax_relief = Some("Treaty relief".into());
    draft.gains = vec![
        row(
            "02/15/2025",
            "Sample Corp",
            500_000.009,
            120_000.0,
            15_000.5,
        ),
        row(
            "06/30/2025",
            "Example Mining",
            1_234_567.891,
            1_000_000.105,
            0.0,
        ),
    ];
    draft.losses = vec![row("09/01/2025", "Loss Co", 50_000.0, 80_000.55, 0.0)];
    draft.surcharge = 25.5;
    draft.interest = 10.009;
    draft.recompute();
    draft
}

#[test]
fn form_1707a_sample_payload_is_current() {
    let draft = sample_1707a();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1707A must validate: {errors:?}"));
    check_sample(
        "1707A-12312025",
        "1707a-v2021",
        &payload,
        "2bf77639d5416e8bdabb63900b0eafc19afef5e2a6ff421971cf1ec4a2fbdc30",
    );
}
