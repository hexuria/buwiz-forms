//! Writes the staticize print-proof renders for docs/proof (ignored test,
//! run by hand). ONE command generates both halves of every proof pair with
//! the same inputs; the output name prefix comes from PROOF_TAG:
//!
//! ```text
//! # on main (the pre-change behaviour):
//! PROOF_TAG=before cargo test -p bir-app-api --test proof_renders -- --ignored
//! # on the branch:
//! PROOF_TAG=after cargo test -p bir-app-api --test proof_renders -- --ignored
//! ```
//!
//! Every render goes through the app's print path:
//! `staticize(prepare_html_for(slug, &filled_document_with_receipt(..), true))`.

#[test]
#[ignore = "writes proof render files"]
fn write_uniform_section_renders() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/proof/print-uniform-amount-sections");
    std::fs::create_dir_all(&root).unwrap();
    let tag = std::env::var("PROOF_TAG").unwrap_or_else(|_| "after".to_string());
    let render = |slug: &str, fields: &[(&str, &str)]| -> String {
        let map: std::collections::BTreeMap<String, String> = fields
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let html =
            bir_print::frozen_html::filled_document_with_receipt(slug, &map, None).expect("render");
        bir_app_api::static_html::staticize(&bir_app_api::print_sheet::prepare_html_for(
            slug, &html, true,
        ))
    };
    // 1702Q Schedule 4, item 2 overflows.
    let q: Vec<(&str, &str)> = [
        ("derived:Sched4:txtTax1", "1000"),
        ("derived:Sched4:txtTax2", "12345678901234"),
        ("derived:Sched4:txtTax3", "1000"),
        ("derived:Sched4:txtTax4", "1000"),
        ("derived:Sched4:txtTax5", "1000"),
        ("derived:Sched4:txtTax6", "1000"),
        ("derived:Sched4:txtOthrTxCrdtAmnt0", "1000"),
        ("derived:Sched4:txtOthrTxCrdtAmnt1", "1000"),
        ("derived:Sched4:txtTax7", "1000"),
    ]
    .into();
    std::fs::write(
        root.join(format!("{tag}-1702q-sched4-overflow.html")),
        render("1702q-2018", &q),
    )
    .unwrap();
    // 1701 Part VII column A, item 2 overflows — filled through the draft's
    // own print field map, the same keys the upload writes.
    let profile: bir_core::TaxpayerProfile = serde_json::from_value(serde_json::json!({
        "id": null,
        "tin": { "segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000" },
        "rdo_code": "044",
        "full_name": "SANTOS, JUANA",
        "registered_address": "123 RIZAL AVE, MAKATI CITY",
        "zip_code": "1200",
        "phone": "09171234567",
        "email": "juana.santos@example.test",
        "line_of_business": "SERVICES",
        "default_form_type": "2551Q"
    }))
    .unwrap();
    use bir_core::forms::form_1701::{Form1701AmountSection, Form1701Draft, Form1701Party};
    use bir_core::forms::queueable::QueueableForm;
    let mut draft = Form1701Draft::new_from_profile(&profile, 2025, 12);
    for item in 1u8..=10 {
        let amount = if item == 2 {
            1_234_567_890.0
        } else {
            1_000_000.0
        };
        draft.set_amount(
            Form1701AmountSection::PartVii,
            item,
            Form1701Party::Taxpayer,
            Some(amount),
        );
    }
    let map = draft.print_field_map();
    let html = bir_print::frozen_html::filled_document_with_receipt("1701-2018", &map, None)
        .expect("1701 render");
    std::fs::write(
        root.join(format!("{tag}-1701-part4-overflow.html")),
        bir_app_api::static_html::staticize(&bir_app_api::print_sheet::prepare_html_for(
            "1701-2018",
            &html,
            true,
        )),
    )
    .unwrap();
    // 1601C Part II: item 14 overflows, empty rows blanked (centavo proof).
    let c: Vec<(&str, &str)> = [("frm1601c:txtTax14", "12345678901234.00")].into();
    std::fs::write(
        root.join(format!("{tag}-1601c-centavo-blank.html")),
        render("1601c-2018", &c),
    )
    .unwrap();
    eprintln!("proof renders written to {}", root.display());
}
