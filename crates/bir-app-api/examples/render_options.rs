use bir_app_api::print_sheet::prepare_html_for;
use bir_app_api::static_html::staticize;
use bir_core::forms::form_2551q::{Form2551QDraft, Schedule1Row};
use bir_core::profile::TaxpayerProfile;
use bir_print::frozen_html::filled_document;
use std::path::PathBuf;

fn make_profile(name: &str, addr: &str) -> TaxpayerProfile {
    let mut profile: TaxpayerProfile = serde_json::from_str(
        r#"{"tin":{"segment1":"000","segment2":"123","segment3":"453","branch":"00000"},"rdo_code":"043","full_name":"","registered_address":"","zip_code":"1605","phone":"09171234567","email":"juana.santos@example.com","line_of_business":"MANAGEMENT CONSULTANCY","default_form_type":"2551Q"}"#,
    )
    .unwrap();
    profile.full_name = name.into();
    profile.registered_address = addr.into();
    profile
}

fn render_2551q(draft: &Form2551QDraft, out: &PathBuf) {
    let fields = draft.to_print_field_map();
    let filled = filled_document("2551q-2018", &fields).unwrap();
    let html = staticize(&prepare_html_for("2551q-2018", &filled, true));
    std::fs::write(out, html).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let batch = args.get(1).map(|s| s.as_str()).unwrap_or("batch1");
    let out_dir = PathBuf::from(args.get(2).map(|s| s.as_str()).unwrap_or("/tmp/options-html"));
    std::fs::create_dir_all(&out_dir).unwrap();

    let suffix = args.get(3).map(|s| s.as_str()).unwrap_or("band");

    let normal_addr = "123 RIZAL AVE, SAN ANTONIO, PASIG CITY";

    if batch == "t1" {
        let cases = [
            ("fit", "SANTOS, JUANA MARIA DELA CRUZ-VILLANUEVA"), // 40 chars exact
            ("over1", "SANTOS, JUANA MARIA DELA CRUZ-VILLANUEVAS"), // 41 chars (1 too long)
            ("long", "SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA DE LOS REYES CONCEPCION ENTERPRISES AND TRADING"),
        ];
        for (tag, name) in cases {
            let profile = make_profile(name, normal_addr);
            let mut d = Form2551QDraft::new_from_profile(&profile, 2025, 2);
            d.schedule_1 = vec![Schedule1Row {
                atc: "PT010".into(),
                atc_description: "Persons exempt from VAT".into(),
                taxable_amount: 100_000.0,
                tax_rate: 3.0,
                tax_due: 3_000.0,
            }];
            d.recompute(None);
            render_2551q(&d, &out_dir.join(format!("t1-{tag}-{suffix}.html")));
        }
    } else if batch == "a1" {
        let profile = make_profile("SANTOS, JUANA MARIA DELA CRUZ-VILLANUEVA", normal_addr);
        let cases: [(&str, f64, f64); 4] = [
            ("fit", 1_750.50, 0.0),
            ("maxfit", 999_999_999_999.99, 0.0),
            ("over1", 1_234_567_890_123.45, 0.0),
            ("neg", 0.0, 1_234_567_890_123.45),
        ];
        for (tag, tax_due, credit) in cases {
            let mut d = Form2551QDraft::new_from_profile(&profile, 2025, 2);
            d.total_tax_due = tax_due;
            d.creditable_tax_withheld = credit;
            d.total_tax_credits = credit;
            d.tax_payable = tax_due - credit;
            d.total_amount_payable = tax_due - credit;
            // Override print map directly for exact A1 values on item 19 (txt19)
            let mut fields = d.to_print_field_map();
            fields.insert("frm2551Qv2018:txt14".into(), format!("{tax_due:.2}"));
            fields.insert("frm2551Qv2018:txt15".into(), format!("{credit:.2}"));
            fields.insert("frm2551Qv2018:txt18".into(), format!("{credit:.2}"));
            fields.insert("frm2551Qv2018:txt19".into(), format!("{:.2}", tax_due - credit));
            let filled = filled_document("2551q-2018", &fields).unwrap();
            let html = staticize(&prepare_html_for("2551q-2018", &filled, true));
            std::fs::write(out_dir.join(format!("a1-{tag}-{suffix}.html")), html).unwrap();
        }
    }
}
