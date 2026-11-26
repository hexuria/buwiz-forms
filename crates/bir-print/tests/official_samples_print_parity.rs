//! Permanent parity check between official eBIRForms sample XMLs and printed
//! frozen HTML cells.
//!
//! For each official 1702MX sample (`1702MX-1225`, `1702MX-0625-fiscal`,
//! `1702MX-1225-sched6`), every key joined in
//! `html-frozen/1702mx-2018c/writer-cells.json` must print the official XML
//! value. An amount compares as the number it is, sign included: the sheet
//! groups thousands and shows a negative the way the official printout does
//! (`(200,000)` on 1702MX), so `5,000` matches `5000` and `(200000)` matches
//! `-200000`, while a lost sign or digit fails. A TIN compares by its digits
//! (the sheet may dash it). Every other value compares exactly, case included.

use bir_core::forms::form_1702mx::{
    Form1702MXDeductionMethod, Form1702MXDraft, Form1702MXFilingBasis,
    Form1702MXOverpaymentDisposition, Form1702MXSpecialDeductionRow, PercentInput, WholePeso,
    WholePesoInput,
};
use bir_core::forms::queueable::QueueableForm as _;
use bir_core::profile::TaxpayerProfile;
use bir_print::frozen_html::{filled_document, printed_cells};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn dummy_profile() -> TaxpayerProfile {
    serde_json::from_value(serde_json::json!({
        "id": null,
        "full_name": "Sample Dummy Taxpayer",
        "tin": { "segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000" },
        "rdo_code": "039",
        "line_of_business": "Sample Consulting Services",
        "registered_address": "123 Sample Street, Barangay Example, Quezon City",
        "zip_code": "1100",
        "phone": "09170000000",
        "email": "sample.taxpayer@example.com",
        "default_form_type": "1702MXv2018C",
        "taxpayer_type": "Corporation",
        "business_start_date": "2020-01-15",
        "tax_elections": [{
            "taxable_year": 2025,
            "election": "GraduatedUnspecified",
            "elected_at": "2025-01-15T00:00:00",
            "source_form": "sample"
        }]
    }))
    .expect("dummy profile")
}

fn peso(value: i64) -> WholePesoInput {
    WholePesoInput::from_amount(WholePeso(value))
}

fn sample_1702mx() -> Form1702MXDraft {
    let mut d = Form1702MXDraft::new_from_profile(&dummy_profile(), 2025);
    d.incorporation_date = "01/15/2015".into();
    d.deduction_method = Form1702MXDeductionMethod::Itemized;
    d.relief_details.investment_promotion_agency = ["Peza".into(), "Boi".into(), String::new()];
    d.relief_details.legal_basis = ["Ra 7916".into(), "Eo 226".into(), String::new()];
    d.relief_details.registered_activity = ["Export mfg".into(), "Tourism".into(), String::new()];
    d.relief_details.effectivity_from = ["01/01/2020".into(), "01/01/2021".into(), String::new()];
    d.relief_details.effectivity_until = ["12/31/2026".into(), "12/31/2027".into(), String::new()];
    d.relief_basis.special_tax_rate = PercentInput::from_hundredths(500);

    let s2 = &mut d.schedule_2.items;
    s2[0].exempt = peso(1_000_000);
    s2[0].special = peso(2_000_000);
    s2[0].regular = peso(5_000_000);
    s2[1].exempt = peso(100_000);
    s2[1].special = peso(500_000);
    s2[1].regular = peso(2_000_000);
    s2[3].regular = peso(25_000);
    s2[5].regular = peso(150_000);
    d.schedule_2.item_14_special_rate = PercentInput::from_hundredths(500);
    d.schedule_2.item_14_regular_rate = PercentInput::from_hundredths(2_500);
    s2[15].special = peso(1_000);
    s2[17].regular = peso(63_000);

    let s5 = &mut d.schedule_5.amounts;
    s5[0].exempt = peso(50_000);
    s5[0].special = peso(100_000);
    s5[0].regular = peso(800_000);
    s5[2].regular = peso(120_500);
    s5[19].regular = peso(10_000);
    d.schedule_5.other_descriptions_17d_to_17i[0] = "Training costs".into();

    d.schedule_6.rows[0].description = "Pension fund".into();
    d.schedule_6.rows[0].legal_basis = "Ra 4917".into();
    d.schedule_6.rows[0].amounts.exempt = peso(5_000);

    let row = &mut d.schedule_7_1.rows[0];
    row.year_incurred = "2022".into();
    row.amount = peso(300_000);
    row.applied_previous_years = peso(100_000);
    row.applied_current_year = peso(50_000);

    let mcit = &mut d.schedule_9.rows[0];
    mcit.year = "2023".into();
    mcit.normal_income_tax = peso(10_000);
    mcit.mcit = peso(30_000);
    mcit.applied_current_year = peso(5_000);

    let s3 = &mut d.schedule_3.items_20_to_33;
    s3[0].regular = peso(50_000);
    s3[2].special = peso(3_000);
    s3[4].regular = peso(20_000);

    d.schedule_4.items[0].exempt = peso(90_000);
    d.schedule_4.items[0].special = peso(30_000);
    d.schedule_4.items[1].exempt = peso(1_000);

    d.part_ii.item_17_surcharge = peso(1_000);
    d.keep_entered_penalties();
    d.recompute();

    for column in 0..3 {
        let value = match column {
            0 => d.schedule_2.items[12].exempt.clone(),
            1 => d.schedule_2.items[12].special.clone(),
            _ => d.schedule_2.items[12].regular.clone(),
        };
        let target = &mut d.schedule_10.items[0];
        match column {
            0 => target.exempt = value,
            1 => target.special = value,
            _ => target.regular = value,
        }
    }
    d.recompute();
    d
}

fn sample_1702mx_fiscal() -> Form1702MXDraft {
    let mut d = Form1702MXDraft::new_from_profile(&dummy_profile(), 2025);
    d.filing_basis = Form1702MXFilingBasis::Fiscal;
    d.month = 6;
    d.is_amended = true;
    d.incorporation_date = "03/10/2023".into();
    d.atc.other_selected = true;
    d.atc.other_code = "IC030".into();
    d.deduction_method = Form1702MXDeductionMethod::OptionalStandard;
    d.relief_details.investment_promotion_agency[1] = "Boi".into();
    d.relief_details.legal_basis[1] = "Eo 226".into();
    d.relief_details.registered_activity[1] = "Tourism".into();
    d.relief_details.effectivity_from[1] = "01/01/2021".into();
    d.relief_details.effectivity_until[1] = "12/31/2027".into();
    d.relief_basis.special_tax_rate = PercentInput::from_hundredths(750);
    let s2 = &mut d.schedule_2.items;
    s2[0].special = peso(1_000_000);
    s2[1].special = peso(300_000);
    s2[0].regular = peso(2_000_000);
    s2[1].regular = peso(1_500_000);
    d.schedule_2.item_14_special_rate = PercentInput::from_hundredths(750);
    d.schedule_2.item_14_regular_rate = PercentInput::from_hundredths(2_500);
    d.schedule_5.amounts[0].special = peso(900_000);
    let s3 = &mut d.schedule_3.items_20_to_33;
    s3[0].special = peso(100_000);
    s3[0].regular = peso(100_000);
    s3[7].regular = peso(10_000);
    d.recompute();
    d.schedule_10.items[0].special = d.schedule_2.items[12].special.clone();
    d.schedule_10.items[0].regular = d.schedule_2.items[12].regular.clone();
    d.part_ii.overpayment_disposition = Some(Form1702MXOverpaymentDisposition::CarryOver);
    d.recompute();
    d
}

fn special_row(
    description: &str,
    legal_basis: &str,
    column: usize,
    value: i64,
) -> Form1702MXSpecialDeductionRow {
    let mut row = Form1702MXSpecialDeductionRow {
        description: description.into(),
        legal_basis: legal_basis.into(),
        ..Default::default()
    };
    match column {
        0 => row.amounts.exempt = peso(value),
        1 => row.amounts.special = peso(value),
        _ => row.amounts.regular = peso(value),
    }
    row
}

fn sample_1702mx_schedule_6_rows() -> Form1702MXDraft {
    let mut d = sample_1702mx();
    d.schedule_6.rows[1] = special_row("Fringe", "Ra 8424", 2, 20_000);
    d.schedule_6.rows[2] = special_row("Rd cost", "Ra 9999", 2, 15_000);
    d.schedule_6.rows[3] = special_row("Seminar", "Rr 2-98", 2, 4_000);
    d.schedule_6.more_rows = vec![d.schedule_6.rows[3].clone()];
    for n in 2..=7i64 {
        d.schedule_6.more_rows.push(special_row(
            &format!("Item {n}"),
            &format!("Rr {n}"),
            ((n % 2) * 2) as usize,
            1_000 * n + 250,
        ));
    }
    d.schedule_4.items[1].regular = peso(2_000);
    d.recompute();
    for column in 0..3 {
        let item_13 = &d.schedule_2.items[12];
        let value = [&item_13.exempt, &item_13.special, &item_13.regular][column].clone();
        let target = &mut d.schedule_10.items[0];
        *[&mut target.exempt, &mut target.special, &mut target.regular][column] = value;
    }
    d.recompute();
    d
}

fn html_unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn attr_of<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')?;
    Some(&tag[start..start + end])
}

/// Extract EVERY printed cell value from a rendered frozen HTML document:
/// - comb cells (from `printed_cells`, including overflow plain lines)
/// - plain `<input type="text">` cells (`value=...`)
/// - checkbox `<input type="checkbox">` cells (`"true"` if `checked`, else `"false"`)
fn all_printed_cells(doc: &str) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = printed_cells(doc)
        .into_iter()
        .map(|(k, (v, _))| (k, v))
        .collect();

    let mut plain_by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut by_id: BTreeMap<String, String> = BTreeMap::new();
    let mut from = 0;
    while let Some(rel) = doc[from..].find("<input") {
        let start = from + rel;
        let Some(end_rel) = doc[start..].find('>') else {
            break;
        };
        let tag = &doc[start..=start + end_rel];
        from = start + end_rel + 1;
        let input_type = attr_of(tag, "type").unwrap_or("text");
        let val = if input_type == "checkbox" {
            let checked = tag.contains(" checked") || tag.contains("checked=\"");
            if checked { "true" } else { "false" }.to_string()
        } else {
            attr_of(tag, "value").map(html_unescape).unwrap_or_default()
        };
        if let Some(id) = attr_of(tag, "id") {
            by_id.insert(id.to_string(), val.clone());
        }
        if let Some(name) = attr_of(tag, "name") {
            if !out.contains_key(name) {
                plain_by_name.entry(name.to_string()).or_default().push(val);
            }
        }
    }
    for (name, vals) in plain_by_name {
        out.insert(name, vals.concat());
    }
    for (id, val) in by_id {
        out.entry(id).or_insert(val);
    }
    out
}

/// The signed number an amount text holds: commas and spaces dropped,
/// `(123)` and `-123` both negative. `None` for anything that is not a number.
fn amount_value(s: &str) -> Option<f64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (negative, inner) = match trimmed.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        Some(inner) => (true, inner.trim()),
        None => match trimmed.strip_prefix('-') {
            Some(rest) => (true, rest.trim()),
            None => (false, trimmed),
        },
    };
    let cleaned: String = inner
        .chars()
        .filter(|&c| c != ',' && !c.is_whitespace())
        .collect();
    if cleaned.is_empty() || !cleaned.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let magnitude: f64 = cleaned.parse().ok()?;
    Some(if negative { -magnitude } else { magnitude })
}

/// What a joined key holds, which decides how its print is compared.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// Compared as a signed number.
    Amount,
    /// Compared by its digits.
    Tin,
    /// Compared exactly.
    Text,
}

/// The 1702MX amount keys, from the official layout
/// (`crates/bir-core/data/official-xml/1702mx-v2018c.json`): its `numbertext`
/// entries (`"number"`) and the controls the official page starts at `0`
/// (`"default": "0"`, the schedule tables' amount columns).
fn official_amount_keys() -> BTreeSet<String> {
    let path = repo_root().join("crates/bir-core/data/official-xml/1702mx-v2018c.json");
    let layout: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("1702mx layout"))
            .expect("parse 1702mx layout");
    layout["entries"]
        .as_array()
        .expect("layout entries")
        .iter()
        .filter(|entry| {
            entry.get("number").is_some() || matches!(entry["default"].as_str(), Some("0" | "0.00"))
        })
        .filter_map(|entry| entry["key"].as_str().map(str::to_string))
        .collect()
}

/// An official value written the way `formatCurrency()` writes an amount
/// (`300,000.00`, `-5.00`): grouped digits with two decimals.
fn is_currency_text(s: &str) -> bool {
    let body = s.trim();
    let body = body.strip_prefix('-').unwrap_or(body);
    let Some((whole, cents)) = body.split_once('.') else {
        return false;
    };
    cents.len() == 2
        && cents.bytes().all(|b| b.is_ascii_digit())
        && !whole.is_empty()
        && whole.bytes().all(|b| b.is_ascii_digit() || b == b',')
}

fn kind_of(key: &str, official: &str, amounts: &BTreeSet<String>) -> Kind {
    if amounts.contains(key) || is_currency_text(official) {
        Kind::Amount
    } else if key.to_ascii_uppercase().contains("TIN") {
        Kind::Tin
    } else {
        Kind::Text
    }
}

fn values_match(official: &str, printed: &str, kind: Kind) -> bool {
    let off = official.trim();
    let prt = printed.trim();
    if off == prt {
        return true;
    }
    match kind {
        Kind::Amount => {
            // form-review/RULES.md section 7: official "0" / "0.00" for an
            // untouched amount box prints blank; typed or computed zero may
            // print as zero.
            if amount_value(off) == Some(0.0) && prt.is_empty() {
                return true;
            }
            match (amount_value(off), amount_value(prt)) {
                (Some(a), Some(b)) => (a - b).abs() < 1e-6,
                _ => false,
            }
        }
        Kind::Tin => {
            let digits = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>();
            off.chars().all(|c| c.is_ascii_digit())
                && prt
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '-' || c == ' ')
                && digits(off) == digits(prt)
        }
        Kind::Text => false,
    }
}

/// Official keys in `1702MX` that are NOT joined in `html-frozen/1702mx-2018c/writer-cells.json`
/// and therefore may stay blank on the 4-page sheet. No joined key is exempted.
const MX_ALLOWED_UNJOINED_BLANK: &[&str] = &[
    "frm1702MX:drpPg3Sc1I11CB",
    "frm1702MX:txtPg2Pt4I34SpecialTaxRate",
    "frm1702MX:txtPg2Sc2It14B",
    "frm1702MX:txtPg2Sc2It14C",
    "frm1702MX:txtPg2Pt4I35CA",
    "frm1702MX:txtPg2Pt4I35CB",
    "frm1702MX:txtPg2Pt4I35CC",
    "frm1702MX:txtPg2Pt4I36CA",
    "frm1702MX:txtPg2Pt4I36CB",
    "frm1702MX:txtPg2Pt4I36CC",
];

fn verify_1702mx_sample(stem: &str, draft: &Form1702MXDraft) -> Vec<String> {
    let root = repo_root();
    let map = draft.to_print_field_map();
    let doc = filled_document("1702mx-2018c", &map).expect("render 1702mx-2018c");
    let cells = all_printed_cells(&doc);

    let layout = draft.official_layout().expect("1702mx layout");
    let official_path = root
        .join("crates/bir-core/tests/sample-payloads")
        .join(format!("{stem}.official.xml"));
    let official_text = std::fs::read_to_string(&official_path).expect("official xml");
    let official = bir_core::official_xml::read(&layout, &official_text).expect("official parse");

    let wc_text = std::fs::read_to_string(root.join("html-frozen/1702mx-2018c/writer-cells.json"))
        .expect("writer-cells.json");
    let wc: Value = serde_json::from_str(&wc_text).expect("parse writer-cells");

    let allowed_unjoined: BTreeSet<&str> = MX_ALLOWED_UNJOINED_BLANK.iter().copied().collect();
    let amounts = official_amount_keys();
    let mut failures = Vec::new();

    // 1. Plain & comb text joins
    for join in wc["joins"].as_array().unwrap() {
        let key = join["writer_key"].as_str().unwrap();
        let cell = join["html_id"].as_str().unwrap();
        assert!(
            !allowed_unjoined.contains(key),
            "key {key} is in MX_ALLOWED_UNJOINED_BLANK but is actually joined to {cell}"
        );
        if key.starts_with("derived:") {
            let expected = map.get(key).map(String::as_str).unwrap_or("");
            let prt = cells.get(cell).map(String::as_str).unwrap_or("");
            if !values_match(expected, prt, kind_of(key, expected, &amounts)) {
                failures.push(format!(
                    "{stem} derived {key} -> {cell}: map={expected:?} printed={prt:?}"
                ));
            }
            continue;
        }
        // Multi-line stacked address comb (p1c40 + p1c41 + p1c42) wraps line 1
        // onto line 2/3 when longer than line 1's 38 slots.
        if key == "frm1702MX:txtPg1Pt1I10RegisteredAddress" {
            let off = official.get(key).map(String::as_str).unwrap_or("").trim();
            let combined = ["p1c40", "p1c41", "p1c42"]
                .iter()
                .filter_map(|id| cells.get(*id).map(|s| s.trim()))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if off != combined {
                failures.push(format!(
                    "{stem} stacked address {key}: official={off:?} printed={combined:?}"
                ));
            }
            continue;
        }
        if key == "frm1702MX:txtPg1Pt1I10RegisteredAddress2"
            || key == "frm1702MX:txtPg1Pt1I10RegisteredAddress3"
        {
            continue;
        }
        let off = official.get(key).map(String::as_str).unwrap_or("");
        let prt = cells.get(cell).map(String::as_str).unwrap_or("");
        // Attachment count is zero-padded to the comb's 3 digit boxes ("00" -> "000").
        let kind = if key == "frm1702MX:txtPg1P2I23NumOfAttachments" {
            Kind::Amount
        } else {
            kind_of(key, off, &amounts)
        };
        if !values_match(off, prt, kind) {
            failures.push(format!(
                "{stem} join {key} -> {cell}: official={off:?} printed={prt:?}"
            ));
        }
    }

    // 2. Peso joins & money joins
    for list in ["peso_joins", "money_joins"] {
        for join in wc[list].as_array().unwrap() {
            let key = join["writer_key"].as_str().unwrap();
            let off = official.get(key).map(String::as_str).unwrap_or("");
            let prt = if let Some(cid) = join["html_id"].as_str() {
                cells.get(cid).cloned().unwrap_or_default()
            } else if let Some(dec) = join["decimal_html_id"].as_str() {
                cells.get(dec).cloned().unwrap_or_default()
            } else {
                let p = join["peso_html_id"]
                    .as_str()
                    .and_then(|id| cells.get(id))
                    .map(String::as_str)
                    .unwrap_or("");
                let c = join["centavo_html_id"]
                    .as_str()
                    .and_then(|id| cells.get(id))
                    .map(String::as_str)
                    .unwrap_or("");
                if p.trim().is_empty() && c.trim().is_empty() {
                    String::new()
                } else {
                    format!("{}.{}", p.trim(), c.trim())
                }
            };
            if !values_match(off, &prt, Kind::Amount) {
                failures.push(format!(
                    "{stem} {list} {key}: official={off:?} printed={prt:?}"
                ));
            }
        }
    }

    // 3. Checkbox (`xbox_joins`) — frozen sheet prints "X" when selected
    for join in wc["xbox_joins"].as_array().unwrap() {
        let key = join["writer_key"].as_str().unwrap();
        let cell = join["html_id"].as_str().unwrap();
        let off = official.get(key).map(String::as_str).unwrap_or("false");
        let off_bool = off.eq_ignore_ascii_case("true") || off == "1";
        let prt = cells.get(cell).map(String::as_str).unwrap_or("");
        let prt_bool = prt.eq_ignore_ascii_case("true") || prt.eq_ignore_ascii_case("X");
        if off_bool != prt_bool {
            failures.push(format!(
                "{stem} xbox {key} -> {cell}: official={off:?} printed={prt:?}"
            ));
        }
    }

    // 4. Date joins (`MM/DD/YYYY`)
    for join in wc["date_joins"].as_array().unwrap() {
        let key = join["writer_key"].as_str().unwrap();
        let mm = cells
            .get(join["mm_html_id"].as_str().unwrap())
            .map(String::as_str)
            .unwrap_or("")
            .trim();
        let dd = cells
            .get(join["dd_html_id"].as_str().unwrap())
            .map(String::as_str)
            .unwrap_or("")
            .trim();
        let yyyy = cells
            .get(join["yyyy_html_id"].as_str().unwrap())
            .map(String::as_str)
            .unwrap_or("")
            .trim();
        let off = official.get(key).map(String::as_str).unwrap_or("").trim();
        let prt = if mm.is_empty() && dd.is_empty() && yyyy.is_empty() {
            String::new()
        } else {
            format!("{mm}/{dd}/{yyyy}")
        };
        if off != prt {
            failures.push(format!(
                "{stem} date {key}: official={off:?} printed={prt:?}"
            ));
        }
    }

    failures
}

#[test]
fn form_1702mx_official_samples_print_every_joined_key_exact() {
    let mut all_failures = Vec::new();
    all_failures.extend(verify_1702mx_sample("1702MX-1225", &sample_1702mx()));
    all_failures.extend(verify_1702mx_sample(
        "1702MX-0625-fiscal",
        &sample_1702mx_fiscal(),
    ));
    all_failures.extend(verify_1702mx_sample(
        "1702MX-1225-sched6",
        &sample_1702mx_schedule_6_rows(),
    ));
    assert!(
        all_failures.is_empty(),
        "1702MX official sample print mismatches ({}):\n{}",
        all_failures.len(),
        all_failures.join("\n")
    );
}
