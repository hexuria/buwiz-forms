//! Frozen HTML fill/print: set `input[name='frm…']` from the writer map.
//!
//! Layout lives in `html-frozen/<slug>/`. `name=` is a fail-closed catalog join;
//! `id=` stays the cell id. Unstamped writer keys have no matching input.

use bir_core::forms::form_2551q::Form2551QDraft;
use std::collections::BTreeMap;

include!(concat!(env!("OUT_DIR"), "/frozen_bundles.rs"));

const BASE_CSS: &str = include_str!("../../../html-frozen/base.css");
const FONT_ARIMO_NORMAL: &[u8] =
    include_bytes!("../../../html-frozen/fonts/arimo-latin-wght-normal.woff2");
const FONT_ARIMO_ITALIC: &[u8] =
    include_bytes!("../../../html-frozen/fonts/arimo-latin-wght-italic.woff2");

const STAMPED_TIN_NAMES: [&str; 4] = [
    "frm2551Qv2018:txtTIN1",
    "frm2551Qv2018:txtTIN2",
    "frm2551Qv2018:txtTIN3",
    "frm2551Qv2018:txtBranchCode",
];

struct InputTag<'a> {
    start: usize,
    end: usize,
    tag: &'a str,
    name: &'a str,
    slot: Option<usize>,
}

pub fn html_2551q() -> &'static str {
    bundle("2551q-2018").expect("2551q-2018 freeze bundle").html
}

/// Rewrite matching `<input name>` tags. Comb slots (`data-slot-index`) receive
/// one character each. Unknown keys are ignored. `id=` is never changed.
pub fn fill_by_name(html: &str, fields: &BTreeMap<String, String>) -> String {
    let tags = input_tags(html);
    if tags.is_empty() {
        return html.to_string();
    }

    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let mut grouped: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, tag) in tags.iter().enumerate() {
        grouped.entry(tag.name).or_default().push(index);
    }

    for (name, value) in fields {
        let Some(indices) = grouped.get(name.as_str()) else {
            continue;
        };
        let comb = indices.iter().any(|&index| tags[index].slot.is_some());
        if comb {
            let mut ordered = indices.clone();
            ordered.sort_by_key(|&index| tags[index].slot.unwrap_or(usize::MAX));
            let chars: Vec<char> = value.chars().collect();
            for (offset, index) in ordered.into_iter().enumerate() {
                let ch = chars
                    .get(offset)
                    .copied()
                    .map(|c| c.to_string())
                    .unwrap_or_default();
                replacements.push((
                    tags[index].start,
                    tags[index].end,
                    set_value(tags[index].tag, &ch),
                ));
            }
        } else {
            for &index in indices {
                replacements.push((
                    tags[index].start,
                    tags[index].end,
                    set_value(tags[index].tag, value),
                ));
            }
        }
    }

    replacements.sort_by_key(|(start, _, _)| *start);
    let mut out = String::with_capacity(html.len() + replacements.len() * 8);
    let mut last = 0;
    for (start, end, tag) in replacements {
        out.push_str(&html[last..start]);
        out.push_str(&tag);
        last = end;
    }
    out.push_str(&html[last..]);
    out
}

/// Frozen 2551Q HTML with writer values on stamped `name=` inputs.
pub fn fill_2551q(draft: &Form2551QDraft) -> String {
    fill_by_name(html_2551q(), &draft.to_bir_field_map())
}

/// Self-contained document for a WebView `with_html` host (inline CSS, fonts, PNGs).
pub fn filled_document(slug: &str, fields: &BTreeMap<String, String>) -> Result<String, String> {
    let Some(loaded) = bundle(slug) else {
        return Err(format!("no frozen HTML bundle for {slug}"));
    };
    Ok(inline_local_assets(
        &fill_by_name(loaded.html, fields),
        &loaded,
    ))
}

pub fn filled_2551q_document(draft: &Form2551QDraft) -> String {
    filled_document("2551q-2018", &draft.to_bir_field_map()).expect("2551q-2018 freeze bundle")
}

pub fn stamped_tin_names() -> &'static [&'static str] {
    &STAMPED_TIN_NAMES
}

fn input_tags(html: &str) -> Vec<InputTag<'_>> {
    let mut tags = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = html[search_from..].find("<input") {
        let start = search_from + rel;
        let Some(gt) = html[start..].find('>') else {
            break;
        };
        let end = start + gt + 1;
        let tag = &html[start..end];
        if let Some(name) = attr(tag, "name") {
            tags.push(InputTag {
                start,
                end,
                tag,
                name,
                slot: attr(tag, "data-slot-index").and_then(|value| value.parse().ok()),
            });
        }
        search_from = end;
    }
    tags
}

fn attr<'a>(tag: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key}=\"");
    let start = tag.find(&needle)? + needle.len();
    let rest = tag.get(start..)?;
    let end = rest.find('"')?;
    rest.get(..end)
}

fn set_value(tag: &str, value: &str) -> String {
    let escaped = html_escape(value);
    let needle = "value=\"";
    if let Some(value_at) = tag.find(needle) {
        let content_at = value_at + needle.len();
        if let Some(close) = tag[content_at..].find('"') {
            let close_at = content_at + close;
            let mut out = String::with_capacity(tag.len() + escaped.len());
            out.push_str(&tag[..content_at]);
            out.push_str(&escaped);
            out.push_str(&tag[close_at..]);
            return out;
        }
    }
    let insert_at = tag.rfind('>').unwrap_or(tag.len());
    format!(
        "{} value=\"{}\"{}",
        &tag[..insert_at],
        escaped,
        &tag[insert_at..]
    )
}

fn html_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn inline_local_assets(html: &str, loaded: &FrozenBundle) -> String {
    let mut document = html.to_string();
    let base = BASE_CSS.replace(
        "url(\"fonts/arimo-latin-wght-normal.woff2\")",
        &format!("url(\"{}\")", data_uri("font/woff2", FONT_ARIMO_NORMAL)),
    );
    let form = loaded.css.replace(
        "url(\"../fonts/arimo-latin-wght-italic.woff2\")",
        &format!("url(\"{}\")", data_uri("font/woff2", FONT_ARIMO_ITALIC)),
    );
    document = document.replace(
        "<link rel=\"stylesheet\" href=\"../base.css\">",
        &format!("<style>{base}</style>"),
    );
    document = document.replace(
        "<link rel=\"stylesheet\" href=\"form.css\">",
        &format!("<style>{form}</style>"),
    );
    for (name, bytes) in loaded.assets {
        let uri = data_uri("image/png", bytes);
        document = document.replace(&format!("../assets/{name}"), &uri);
    }
    document
}

fn data_uri(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64_encode(bytes))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_draft() -> Form2551QDraft {
        let mut draft: Form2551QDraft = serde_json::from_value(json!({
            "id": null,
            "tin": "261708015000",
            "taxpayer_type": "Individual",
            "business_start_date": "2010-01-01",
            "taxable_year": 2026,
            "quarter": 1,
            "tax_period_basis": "calendar",
            "year_end_month": 12,
            "eopt_tier": null,
            "is_amended": false,
            "original_return_filed_and_paid_on_time": true,
            "number_of_attached_sheets": 0,
            "tax_relief": false,
            "tax_relief_specification": "",
            "item_13_election": "graduated",
            "rdo_code": "018",
            "taxpayer_name": "Frozen Html Fixture",
            "registered_address": "New Cabalan",
            "zip_code": "2200",
            "contact_number": "09156837000",
            "email": "tax@example.com",
            "schedule_1": [],
            "total_tax_due": 0.0,
            "creditable_tax_withheld": 0.0,
            "tax_paid_previous": 0.0,
            "other_tax_credit": 0.0,
            "other_tax_credit_description": "",
            "total_tax_credits": 0.0,
            "tax_payable": 0.0,
            "auto_compute_penalties": false,
            "surcharge": 0.0,
            "interest": 0.0,
            "compromise": 0.0,
            "total_penalties": 0.0,
            "total_amount_payable": 0.0,
            "overpayment_disposition": "none",
            "status": "Draft",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "submitted_at": null,
            "confirmed_at": null,
            "submission_filename": null,
            "receipt_id": null,
            "submission_attempts": 0,
            "next_retry_at": null,
            "last_error": null,
            "carried_forward_from": null,
            "payment_receipt_path": null
        }))
        .expect("fixture draft");
        draft.recompute(None);
        draft
    }

    fn named_values(html: &str, name: &str) -> Vec<String> {
        input_tags(html)
            .into_iter()
            .filter(|tag| tag.name == name)
            .map(|tag| attr(tag.tag, "value").unwrap_or("").to_string())
            .collect()
    }

    #[test]
    fn frozen_2551q_stamps_only_catalog_tin_names_from_fields_json() {
        let fields: serde_json::Value =
            serde_json::from_str(include_str!("../../../rules/forms/2551q-v2018/fields.json"))
                .unwrap();
        let allowed: std::collections::BTreeSet<String> = fields["fields"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|row| row["serialized_key"].as_str().map(str::to_string))
            .collect();

        let present: std::collections::BTreeSet<String> = input_tags(html_2551q())
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm2551Qv2018:"))
            .collect();

        assert_eq!(
            present,
            STAMPED_TIN_NAMES
                .iter()
                .map(|name| (*name).to_string())
                .collect()
        );
        for name in &present {
            assert!(allowed.contains(name), "{name} is not in fields.json");
        }
        assert!(html_2551q().contains("id=\"p1c20-s0\""));
        assert!(
            input_tags(html_2551q())
                .into_iter()
                .all(|tag| tag.name != "p1c20")
        );
    }

    #[test]
    fn frozen_0619e_stamps_only_catalog_tin_names() {
        let html = bundle("0619e-2018").expect("0619e bundle").html;
        let present: std::collections::BTreeSet<String> = input_tags(html)
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm0619E:"))
            .collect();
        assert_eq!(
            present,
            [
                "frm0619E:txtTIN1",
                "frm0619E:txtTIN2",
                "frm0619E:txtTIN3",
                "frm0619E:txtBranchCode",
            ]
            .into_iter()
            .map(str::to_string)
            .collect()
        );
        let document = filled_document("0619e-2018", &BTreeMap::new()).unwrap();
        assert!(document.contains("<style>"));
        assert!(!document.contains("href=\"../base.css\""));
    }

    #[test]
    fn fill_by_name_distributes_tin_digits_across_comb_slots() {
        let html = concat!(
            "<div data-field-name=\"p1c20\">",
            "<input id=\"p1c20-s0\" name=\"frm2551Qv2018:txtTIN1\" data-slot-index=\"0\" maxlength=\"1\">",
            "<input id=\"p1c20-s1\" name=\"frm2551Qv2018:txtTIN1\" data-slot-index=\"1\" maxlength=\"1\">",
            "<input id=\"p1c20-s2\" name=\"frm2551Qv2018:txtTIN1\" data-slot-index=\"2\" maxlength=\"1\">",
            "</div>"
        );
        let mut fields = BTreeMap::new();
        fields.insert("frm2551Qv2018:txtTIN1".to_string(), "261".to_string());
        let filled = fill_by_name(html, &fields);
        assert_eq!(
            named_values(&filled, "frm2551Qv2018:txtTIN1"),
            ["2", "6", "1"]
        );
        assert!(filled.contains("id=\"p1c20-s0\""));
        assert!(filled.contains("data-field-name=\"p1c20\""));
    }

    #[test]
    fn fill_2551q_sets_stamped_tin_names_from_the_writer_map() {
        let filled = fill_2551q(&sample_draft());
        assert_eq!(
            named_values(&filled, "frm2551Qv2018:txtTIN1"),
            ["2", "6", "1"]
        );
        assert_eq!(
            named_values(&filled, "frm2551Qv2018:txtTIN2"),
            ["7", "0", "8"]
        );
        assert_eq!(
            named_values(&filled, "frm2551Qv2018:txtTIN3"),
            ["0", "1", "5"]
        );
        assert_eq!(
            named_values(&filled, "frm2551Qv2018:txtBranchCode"),
            ["0", "0", "0", "0", "0"]
        );
        let document = filled_2551q_document(&sample_draft());
        assert!(document.contains("data:image/png;base64,"));
        assert!(document.contains("<style>"));
        assert!(!document.contains("href=\"../base.css\""));
    }

    #[test]
    fn frozen_preview_bundles_stamp_only_catalog_frm_names() {
        let cases = [
            ("0619f-2018", "frm0619F:", 4usize),
            ("0605-1999", "frm0605:", 4),
            ("1601c-2018", "frm1601c:", 4),
            ("1701q-2018", "frm1701q:", 3),
            ("2550q-2024", "frm2550qv2024:", 3),
            ("1701-2018", "frm1701:", 6),
            ("1702rt-2018c", "frm1702RT:", 3),
            ("1702mx-2018c", "frm1702MX:", 3),
        ];
        for (slug, prefix, count) in cases {
            let html = bundle(slug).unwrap_or_else(|| panic!("{slug}")).html;
            let present: std::collections::BTreeSet<String> = input_tags(html)
                .into_iter()
                .map(|tag| tag.name.to_string())
                .filter(|name| name.starts_with(prefix))
                .collect();
            assert_eq!(present.len(), count, "{slug} stamped names");
            let document = filled_document(slug, &BTreeMap::new()).unwrap();
            assert!(document.contains("<style>"), "{slug}");
        }
    }
}
