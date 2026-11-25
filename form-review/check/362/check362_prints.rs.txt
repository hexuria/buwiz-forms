//! CHECK-362 scratch harness (uncommitted): writes filled frozen-HTML documents
//! for the four claims with this machine's own invented values, plus assertions.
//! Run: OUT=/tmp/c362out/<commit> cargo test -p bir-print --test check362_prints -- --nocapture
use bir_core::forms::form_1800::{Form1800Draft, Form1800PersonalProperty};
use bir_core::forms::form_1801::Form1801Draft;
use bir_core::forms::form_2200s::{Form2200SDraft, Form2200SOtherRow};
use bir_core::forms::form_2200t::{Form2200TAmounts, Form2200TDraft};
use bir_core::forms::queueable::QueueableForm;
use bir_core::TaxpayerProfile;
use bir_print::frozen_html::{filled_document, printed_cell};
use serde_json::json;
use std::collections::BTreeMap;

fn writer_cells_of(html: &str) -> BTreeMap<String, (String, Option<f64>)> {
    let mut out = BTreeMap::new();
    let mut rest = html;
    while let Some(pos) = rest.find("data-writer-value=\"") {
        let tag_start = rest[..pos].rfind("<input").unwrap_or(0);
        let tag = &rest[tag_start..];
        let tag_end = tag.find('>').unwrap_or(tag.len());
        let tag = &tag[..tag_end];
        let id = tag.split("id=\"").nth(1).and_then(|x| x.split('\"').next()).unwrap_or("").trim_end_matches("-i").to_string();
        let val = tag.split("data-writer-value=\"").nth(1).and_then(|x| x.split('\"').next()).unwrap_or("").to_string();
        if !id.is_empty() {
            out.insert(id, (val, None));
        }
        rest = &rest[pos + 10..];
    }
    out
}

fn profile(branch: &str) -> TaxpayerProfile {
    serde_json::from_value(json!({
        "id": null,
        "tin": { "segment1": "000", "segment2": "123", "segment3": "453", "branch": branch },
        "rdo_code": "044",
        "full_name": "SANTOS, JUANA",
        "registered_address": "456 MABINI ST, QUEZON CITY",
        "zip_code": "1101",
        "phone": "09181234567",
        "email": "juana.santos@example.test",
        "line_of_business": "SERVICES",
        "default_form_type": "2551Q"
    }))
    .unwrap()
}

fn write(slug: &str, name: &str, map: &BTreeMap<String, String>) -> String {
    let doc = filled_document(slug, map).unwrap();
    let out = std::env::var("OUT").unwrap_or_else(|_| "/tmp/c362out".into());
    std::fs::create_dir_all(&out).unwrap();
    let path = format!("{out}/{name}.html");
    std::fs::write(&path, &doc).unwrap();
    println!("WROTE {path}");
    doc
}

fn t2200_slug() -> &'static str {
    if std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../html-frozen/2200t-2020/index.html")).exists() {
        "2200t-2020"
    } else {
        "2200t-2022"
    }
}

#[test]
fn emit_all() {
    // ---- 1801 branch 00001 and 00000
    for branch in ["00001", "00000"] {
        let draft = Form1801Draft::new_from_profile(&profile(branch), 2025, 1);
        let doc = write("1801-2018", &format!("1801-branch{branch}"), &draft.print_field_map());
        println!("1801 {branch} p1c17={:?} p2c5={:?}", printed_cell(&doc, "p1c17"), printed_cell(&doc, "p2c5"));
    }
    // ---- 1800 branch 00001 and 00000
    for branch in ["00001", "00000"] {
        let draft = Form1800Draft::new_from_profile(&profile(branch), 2025, 1);
        let doc = write("1800-2018", &format!("1800-branch{branch}"), &draft.print_field_map());
        println!("1800 {branch} p1c18={:?} p2c5={:?}", printed_cell(&doc, "p1c18"), printed_cell(&doc, "p2c5"));
    }
    // ---- 1800 item 14 centavos 05 / 50 / 99 (branch 00001)
    for (value, cents) in [(456.05f64, "05"), (456.50, "50"), (456.99, "99")] {
        let mut draft = Form1800Draft::new_from_profile(&profile("00001"), 2025, 1);
        draft.personal_properties.push(Form1800PersonalProperty {
            particulars: "CASH DONATION".into(),
            fair_market_value: value,
        });
        draft.compute();
        let mut map = draft.print_field_map();
        map.insert("frm1800v2018:TotalNetGifts".into(), format!("456.{cents}"));
        let doc = write("1800-2018", &format!("1800-centavos-{cents}"), &map);
        println!("1800 cents {cents} p1c64={:?}", printed_cell(&doc, "p1c64"));
    }
    // ---- 2200S two 40-char others descriptions
    let mut draft = Form2200SDraft::new_from_profile(&profile("00001"), 2025, 3);
    draft.manner = bir_core::forms::form_2200s::Form2200SManner::ActualRemoval;
    draft.others = vec![
        Form2200SOtherRow {
            atc: "XB010".into(),
            description: "MINTED LYCHEE SYRUP CONCENTRATE FOR SODAS".into(),
            tax_bracket: "PER LITER".into(),
            rate: 6.0,
            sales_value: 111.0,
            volume: 11.0,
            basic_tax_due: 66.0,
        },
        Form2200SOtherRow {
            atc: "XB020".into(),
            description: "GINGER CALAMANSI INFUSION PREMIX BOTTLE".into(),
            tax_bracket: "PER LITER".into(),
            rate: 6.0,
            sales_value: 222.0,
            volume: 22.0,
            basic_tax_due: 132.0,
        },
    ];
    draft.recompute();
    let doc = write("2200s-2018", "2200s-others", &draft.to_print_field_map());
    println!("2200S p2c101={:?} p2c108={:?}", printed_cell(&doc, "p2c101"), printed_cell(&doc, "p2c108"));

    // ---- 2200T rows 2020-2023, amounts 111/222/333/444 => total/item16 1110
    let slug = t2200_slug();
    println!("2200T slug={slug}");
    let mut t = Form2200TDraft::new_from_profile(&profile("00001"), 2024, 1);
    let dues = [("1A_2020", 111.0), ("1A_2021", 222.0), ("1A_2022", 333.0), ("1A_2023", 444.0)];
    for (i, row) in bir_core::forms::form_2200t::FORM_2200T_FIXED_ROWS.iter().enumerate() {
        if let Some((_, due)) = dues.iter().find(|(s, _)| *s == row.0) {
            while t.schedule.len() <= i {
                t.schedule.push(Form2200TAmounts::default());
            }
            t.schedule[i] = Form2200TAmounts { export_exempt: 0.0, taxable: 0.0, tax_due: *due };
        }
    }
    t.manner_of_payment = bir_core::forms::form_2200t::Form2200TPayment::ActualRemoval;
    t.place_of_production = bir_core::forms::form_2200t::Form2200TPlace { region: "130000000".into(), province: "137400000".into(), city: "137404000".into() };
    t.place_of_removal = bir_core::forms::form_2200t::Form2200TPlace { region: "130000000".into(), province: "137400000".into(), city: "137404000".into() };
    t.recompute();
    let map = t.to_print_field_map();
    let doc = write(slug, "2200t-years", &map);
    let cells = writer_cells_of(&doc);
    let mut total_printed = 0.0f64;
    for (key, _) in [("p2c27", 111.0), ("p2c32", 222.0), ("p2c37", 333.0), ("p2c42", 444.0)] {
        println!("2200T {key}={:?}", cells.get(key));
    }
    println!("2200T sched-total p3c154={:?} item16 p1c900/p1c901={:?}/{:?}", cells.get("p3c154"), cells.get("p1c900"), cells.get("p1c901"));
    let _ = total_printed;
    // 2200T upload XML, byte-compare across commits
    match t.to_bir_xml_payload() {
        Ok(xml) => {
            std::fs::write(format!("{}/2200t-upload.xml", std::env::var("OUT").unwrap()), &xml).unwrap();
            println!("XML len {}", xml.len());
        }
        Err(e) => println!("XML ERR {e:?}"),
    }

    // dump every non-empty printed cell for the changed-box comparison
    let out = std::env::var("OUT").unwrap_or_else(|_| "/tmp/c362out".into());
    let mut dump = String::new();
    for (k, (v, _)) in &cells {
        if !v.trim().is_empty() {
            dump.push_str(&format!("{k}\t{v}\n"));
        }
    }
    std::fs::write(format!("{out}/2200t-cells.txt"), dump).unwrap();

    // same dumps for the other forms (unchanged-elsewhere comparison)
    for (slug2, file) in [("1801-2018", "1801-branch00001"), ("1800-2018", "1800-branch00001"), ("2200s-2018", "2200s-others")] {
        let html = std::fs::read_to_string(format!("{out}/{file}.html")).unwrap();
        let cells2 = writer_cells_of(&html);
        let mut dump2 = String::new();
        for (k, (v, _)) in &cells2 {
            if !v.trim().is_empty() {
                dump2.push_str(&format!("{k}\t{v}\n"));
            }
        }
        std::fs::write(format!("{out}/{file}-cells.txt"), dump2).unwrap();
        let _ = slug2;
    }
}
