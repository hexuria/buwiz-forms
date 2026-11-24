use bir_app_api::print_sheet::prepare_html_for;
use bir_app_api::static_html::staticize;
use bir_core::forms::form_1701::Form1701Draft;
use bir_core::forms::form_1701ms::Form1701MsDraft;
use bir_core::forms::form_2000ot::{Form2000OTDraft, Form2000OTShareRow};
use bir_core::forms::form_2551q::{Form2551QDraft, Schedule1Row};
use bir_core::profile::TaxpayerProfile;
use bir_print::frozen_html::filled_document;
use std::collections::BTreeMap;
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

fn write_sheet(slug: &str, fields: &BTreeMap<String, String>, out: &PathBuf) {
    let filled = filled_document(slug, fields).unwrap();
    let html = staticize(&prepare_html_for(slug, &filled, true));
    std::fs::write(out, html).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let batch = args.get(1).map(|s| s.as_str()).unwrap_or("t1");
    let out_dir = PathBuf::from(args.get(2).map(|s| s.as_str()).unwrap_or("/tmp/options-html"));
    let suffix = args.get(3).map(|s| s.as_str()).unwrap_or("band");
    std::fs::create_dir_all(&out_dir).unwrap();

    let normal_name = "SANTOS, JUANA MARIA DELA CRUZ-VILLANUEVA";
    let normal_addr = "123 RIZAL AVE, SAN ANTONIO, PASIG CITY";

    match batch {
        "t1" => {
            let cases = [
                ("fit", "SANTOS, JUANA MARIA DELA CRUZ-VILLANUEVA"),
                ("over1", "SANTOS, JUANA MARIA DELA CRUZ-VILLANUEVAS"),
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
                write_sheet("2551q-2018", &d.to_print_field_map(), &out_dir.join(format!("t1-{tag}-{suffix}.html")));
            }
        }
        "a1" => {
            let profile = make_profile(normal_name, normal_addr);
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
                let mut fields = d.to_print_field_map();
                fields.insert("frm2551Qv2018:txt14".into(), format!("{tax_due:.2}"));
                fields.insert("frm2551Qv2018:txt15".into(), format!("{credit:.2}"));
                fields.insert("frm2551Qv2018:txt18".into(), format!("{credit:.2}"));
                fields.insert("frm2551Qv2018:txt19".into(), format!("{:.2}", tax_due - credit));
                write_sheet("2551q-2018", &fields, &out_dir.join(format!("a1-{tag}-{suffix}.html")));
            }
        }
        "t2" => {
            // 2551Q Item 9 Registered Address: p1c32 (40 boxes) + p1c33 (31 boxes) = 71 boxes total.
            let cases = [
                ("fit", "UNIT 1402 ROBINSOFT TOWER, ORTIGAS CENTER, SAN ANTONIO, PASIG CITY"),
                ("over1", "UNIT 1402 ONE CORPORATE CENTRE, ORTIGAS CENTER, SAN ANTONIO, PASIG CITY1"), // 72 chars = 1 over 71
                ("long", "UNIT 1402 ONE CORPORATE CENTRE, JULIA VARGAS AVENUE CORNER MERALCO AVENUE, ORTIGAS CENTER, BARANGAY SAN ANTONIO, PASIG CITY, METRO MANILA 1605"),
            ];
            for (tag, addr) in cases {
                let profile = make_profile(normal_name, addr);
                let mut d = Form2551QDraft::new_from_profile(&profile, 2025, 2);
                d.recompute(None);
                write_sheet("2551q-2018", &d.to_print_field_map(), &out_dir.join(format!("t2-{tag}-{suffix}.html")));
            }
        }
        "a2" => {
            // 1701-2018 page 1 Item 22 Tax Due (p1c87): 9-slot whole-peso comb.
            let profile = make_profile(normal_name, normal_addr);
            let cases: [(&str, f64); 2] = [
                ("fit", 12_345_678.0),
                ("over1", 1_234_567_890.0), // 10 digits > 9 slots
            ];
            for (tag, amt) in cases {
                let d = Form1701Draft::new_from_profile(&profile, 2025, 12);
                let mut fields = d.to_print_field_map();
                fields.insert("frm1701:txtPg1I22ATaxDue".into(), format!("{amt:.2}"));
                fields.insert("frm1701:txtPg1I24ATaxPayable".into(), format!("{amt:.2}"));
                write_sheet("1701-2018", &fields, &out_dir.join(format!("a2-{tag}-{suffix}.html")));
            }
        }
        "n1" => {
            // 2551Q page 1 Item 6 TIN (000-123-453-00000) & Item 7 RDO (043): always fits.
            let profile = make_profile(normal_name, normal_addr);
            let mut d = Form2551QDraft::new_from_profile(&profile, 2025, 2);
            d.recompute(None);
            write_sheet("2551q-2018", &d.to_print_field_map(), &out_dir.join(format!("n1-fit-{suffix}.html")));
        }
        "w1" => {
            // 2000-OT page 2 Schedule 1.B row 1 stock name (p2c71, >=18pt tall plain box).
            let profile = make_profile(normal_name, normal_addr);
            let mut d = Form2000OTDraft::new_from_profile(&profile, 2025);
            d.shares = vec![Form2000OTShareRow {
                corporation: "PHILIPPINE METROPOLITAN COMMERCIAL HOLDINGS AND INFRASTRUCTURE DEVELOPMENT CORPORATION (CLASS A COMMON SHARES)".into(),
                shares_sold: "10,000".into(),
                certificate_number: "CERT-2025-00142".into(),
                par_value: 1_000_000.0,
                dst_paid_on_issue: 0.0,
            }];
            write_sheet("2000-ot-2018", &d.to_print_field_map(), &out_dir.join(format!("w1-long-{suffix}.html")));
        }
        "w2" => {
            // 1701MS page 2 Part V Item 7 Foreign Tax Credits (specify) (p2c177, <18pt short plain box).
            let profile = make_profile(normal_name, normal_addr);
            let d = Form1701MsDraft::new_from_profile(&profile, 2025);
            let mut fields = d.to_print_field_map();
            fields.insert(
                "derived:foreign_tax_credits_description".into(),
                "WITHHOLDING TAX PAID IN SINGAPORE AND JAPAN ON CONSULTING FEES EARNED UNDER DOUBLE TAXATION TREATY ARTICLE 14".into(),
            );
            write_sheet("1701ms-2024", &fields, &out_dir.join(format!("w2-long-{suffix}.html")));
        }
        "p1" => {
            // 2551Q page 1 full page with realistic long values (name, address, overflowing amount).
            let profile = make_profile(
                "SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA DE LOS REYES CONCEPCION ENTERPRISES AND TRADING",
                "UNIT 1402 ONE CORPORATE CENTRE, JULIA VARGAS AVENUE CORNER MERALCO AVENUE, ORTIGAS CENTER, BARANGAY SAN ANTONIO, PASIG CITY, METRO MANILA 1605",
            );
            let mut d = Form2551QDraft::new_from_profile(&profile, 2025, 2);
            d.schedule_1 = vec![Schedule1Row {
                atc: "PT010".into(),
                atc_description: "Persons exempt from VAT".into(),
                taxable_amount: 41_152_263_004_115.0,
                tax_rate: 3.0,
                tax_due: 1_234_567_890_123.45,
            }];
            d.recompute(None);
            let mut fields = d.to_print_field_map();
            fields.insert("frm2551Qv2018:txt14".into(), "1234567890123.45".into());
            fields.insert("frm2551Qv2018:txt19".into(), "1234567890123.45".into());
            fields.insert("frm2551Qv2018:txt25".into(), "1234567890123.45".into());
            write_sheet("2551q-2018", &fields, &out_dir.join(format!("p1-page-{suffix}.html")));
        }
        other => panic!("unknown batch {other}"),
    }
}
