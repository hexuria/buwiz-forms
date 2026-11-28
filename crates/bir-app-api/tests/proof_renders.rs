//! Writes the staticize print-proof renders for docs/proof (ignored test,
//! run by hand): `cargo test -p bir-app-api --test proof_renders -- --ignored`.

#[test]
#[ignore = "writes proof render files"]
fn write_uniform_section_renders() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/proof/print-uniform-amount-sections");
    std::fs::create_dir_all(&root).unwrap();
    let render = |slug: &str, fields: &[(&str, &str)]| -> String {
        let map: std::collections::BTreeMap<String, String> = fields
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let html = bir_print::frozen_html::filled_document(slug, &map).expect("render");
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
        root.join("after-1702q-sched4-overflow.html"),
        render("1702q-2018", &q),
    )
    .unwrap();
    // 1701 Part VII column A, item 2 overflows.
    let mut p: Vec<(&str, &str)> = Vec::new();
    for item in 1u8..=10 {
        let amount = if item == 2 { "1234567890" } else { "1000000" };
        // leak-free small strings: use Box::leak for static keys
        let key: &'static str = Box::leak(format!("frm1701:txtPg4IPart7_{item}A").into_boxed_str());
        p.push((key, amount));
    }
    std::fs::write(
        root.join("after-1701-part4-overflow.html"),
        render("1701-2018", &p),
    )
    .unwrap();
    // 1601C Part II: item 14 overflows, empty rows blanked (centavo proof).
    let c: Vec<(&str, &str)> = [("frm1601c:txtTax14", "12345678901234.00")].into();
    std::fs::write(
        root.join("after-1601c-centavo-blank.html"),
        render("1601c-2018", &c),
    )
    .unwrap();
    eprintln!("proof renders written to {}", root.display());
}
