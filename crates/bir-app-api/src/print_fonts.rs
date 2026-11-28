//! Diagnostic measurement of filled values through the real print path.
//!
//! The binary wrapper renders each sheet; the regression test uses the same
//! observations. Sizes are effective A4 print sizes, after the paper fit scale.
//! On the eight sheets of #576, class-sized text is raised to the owner's
//! 7.5 pt printed floor; long ordinary text still wraps or truncates at that
//! floor. Whole amounts and identifiers that cannot fit remain the existing
//! whole-figure overflow exception: they are not wrapped or cut into a
//! different figure.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const MIN_PRINTED_PT: f64 = 7.5;

pub const FLOOR_SLUGS: &[&str] = &[
    "1601c-2018",
    "1602q-2019",
    "1603q-2018",
    "1604f-2018",
    "1801-2018",
    "2200a-2020",
    "2550m-2007",
    "2550q-2024",
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct PrintedFont {
    pub slug: String,
    pub id: String,
    pub key: String,
    pub size_class: String,
    pub size_pt: f64,
    pub effective_pt: f64,
    pub text: String,
    pub overflow_of: String,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let at = tag.find(&needle)? + needle.len();
    let end = tag[at..].find('"')?;
    Some(&tag[at..at + end])
}

fn insert_cell(cells: &mut BTreeMap<String, String>, id: &str, key: &str) {
    cells.insert(id.to_string(), key.to_string());
}

fn writer_keys_and_cells(path: &Path) -> (Vec<String>, BTreeSet<String>, BTreeMap<String, String>) {
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("writer-cells.json must read"))
            .expect("writer-cells.json must parse");
    let mut keys = Vec::new();
    let mut xboxes = BTreeSet::new();
    let mut cells = BTreeMap::new();
    for group in ["joins", "xbox_joins"] {
        for join in json[group].as_array().into_iter().flatten() {
            let Some(key) = join["writer_key"].as_str() else {
                continue;
            };
            keys.push(key.to_string());
            if group == "xbox_joins" {
                xboxes.insert(key.to_string());
            }
            if let Some(object) = join.as_object() {
                for (name, value) in object {
                    if !name.contains("html_id") {
                        continue;
                    }
                    if let Some(id) = value.as_str() {
                        insert_cell(&mut cells, id, key);
                    } else if let Some(ids) = value.as_array() {
                        for id in ids.iter().filter_map(|id| id.as_str()) {
                            insert_cell(&mut cells, id, key);
                        }
                    }
                }
            }
        }
    }
    keys.sort();
    keys.dedup();
    (keys, xboxes, cells)
}

fn value_for(key: &str, xbox: bool) -> &'static str {
    if xbox {
        return "true";
    }
    let lower = key.to_ascii_lowercase();
    if lower.contains("email") {
        "a@b.co"
    } else if lower.contains("derived:tin_digits") {
        "000123456"
    } else if lower.contains("tina") || lower.contains("tin1") {
        "000"
    } else if lower.contains("tinb") || lower.contains("tin2") {
        "123"
    } else if lower.contains("tinc") || lower.contains("tin3") {
        "456"
    } else if lower.contains("branch") {
        "00000"
    } else if lower.ends_with(":rtnyear") {
        "01"
    } else if lower.contains("monthyear") || lower.contains("month_year") {
        "01/2025"
    } else if lower.contains("date") {
        "01/02/2025"
    } else if lower.ends_with("year") || lower.contains("txtyear") {
        "2025"
    } else if lower.ends_with("month") || lower.contains("txtmonth") {
        "01"
    } else if lower.ends_with("day") || lower.contains("txtday") {
        "02"
    } else if lower.contains("zip") {
        "1234"
    } else if lower.contains("phone") || lower.contains("tel") {
        "0917"
    } else if lower.contains("tax")
        || lower.contains("amount")
        || lower.contains("income")
        || lower.contains("due")
        || lower.contains("gross")
        || lower.contains("credit")
        || lower.contains("payment")
        || lower.contains("withheld")
        || lower.contains("fmv")
        || lower.contains("rate")
        || lower.contains("area")
        || lower.contains("qty")
    {
        "1.00"
    } else {
        "X"
    }
}

fn class_sizes(html: &str) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for part in html.split('}') {
        let Some((selector, body)) = part.split_once('{') else {
            continue;
        };
        for class in selector
            .split(',')
            .map(str::trim)
            .filter_map(|s| s.strip_prefix('.'))
        {
            if !class.starts_with("fh") {
                continue;
            }
            let Some(at) = body.find("font-size:") else {
                continue;
            };
            let tail = &body[at + "font-size:".len()..];
            if let Some(size) = tail
                .split(';')
                .next()
                .and_then(|s| s.trim().strip_suffix("pt"))
                .and_then(|s| s.parse().ok())
            {
                out.insert(class.to_string(), size);
            }
        }
    }
    out
}

fn normalized_cell(id: &str) -> &str {
    if let Some(base) = id.strip_suffix("-i") {
        return base;
    }
    if let Some(at) = id.rfind("-s")
        && id[at + 2..].bytes().all(|b| b.is_ascii_digit())
        && at + 2 < id.len()
    {
        return &id[..at];
    }
    id
}

fn inline_font(style: &str) -> Option<f64> {
    style.split(';').find_map(|decl| {
        decl.trim()
            .strip_prefix("font-size:")
            .and_then(|v| v.strip_suffix("pt"))
            .and_then(|v| v.trim().parse().ok())
    })
}

fn sheet_size(html: &str) -> (f64, f64) {
    let fallback = (612.0, 936.0);
    let Some(at) = html.find("@page") else {
        return fallback;
    };
    let Some(size_at) = html[at..].find("size:") else {
        return fallback;
    };
    let rest = &html[at + size_at + 5..];
    let Some(end) = rest.find([';', '}']) else {
        return fallback;
    };
    let mut parts = rest[..end].split_whitespace();
    let width = parts
        .next()
        .and_then(|v| v.strip_suffix("pt"))
        .and_then(|v| v.parse().ok());
    let height = parts
        .next()
        .and_then(|v| v.strip_suffix("pt"))
        .and_then(|v| v.parse().ok());
    match (width, height) {
        (Some(width), Some(height)) => (width, height),
        _ => fallback,
    }
}

fn a4_scale(width: f64, height: f64) -> f64 {
    ((crate::print_sheet::PAPER_WIDTH_PT as f64 - 20.0) / width)
        .min((crate::print_sheet::PAPER_HEIGHT_PT as f64 - 20.0) / height)
        .min(1.0)
}

/// Fill every writer key with a short invented value, render through
/// `filled_document_with_receipt -> prepare_html_for -> staticize`, and return
/// every non-empty printed value span with its effective A4 font size.
pub fn probe_sheet(slug: &str, checkout: &Path) -> Result<Vec<PrintedFont>, String> {
    let writer_path = checkout
        .join("html-frozen")
        .join(slug)
        .join("writer-cells.json");
    let (keys, xboxes, cells) = writer_keys_and_cells(&writer_path);
    let fields: BTreeMap<String, String> = keys
        .iter()
        .map(|key| {
            (
                key.clone(),
                value_for(key, xboxes.contains(key)).to_string(),
            )
        })
        .collect();
    let html = bir_print::frozen_html::filled_document_with_receipt(slug, &fields, None)?;
    let scale = sheet_size(&html);
    let scale = a4_scale(scale.0, scale.1);
    let html = crate::print_sheet::prepare_html_for(slug, &html, true);
    let html = crate::static_html::staticize(&html);
    let sizes = class_sizes(&html);
    let mut seen = Vec::new();
    let mut rest = html.as_str();
    while let Some(start) = rest.find("<span ") {
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        let tag = &rest[start..start + end + 1];
        let after = &rest[start + end + 1..];
        let Some(close) = after.find("</span>") else {
            break;
        };
        let text = &after[..close];
        rest = &after[close + 7..];
        let class = attr(tag, "class").unwrap_or("");
        if !class.split_whitespace().any(|c| c == "fs") || text.trim().is_empty() {
            continue;
        }
        let style = attr(tag, "style").unwrap_or("");
        let mut size = inline_font(style);
        let mut size_class = "inline".to_string();
        if size.is_none() {
            for c in class.split_whitespace() {
                if let Some(found) = sizes.get(c) {
                    size = Some(*found);
                    size_class = c.to_string();
                    break;
                }
            }
        }
        let Some(size) = size else {
            continue;
        };
        let id = attr(tag, "id").unwrap_or("").to_string();
        let key = cells
            .get(normalized_cell(&id))
            .cloned()
            .unwrap_or_else(|| format!("generated:{}", normalized_cell(&id)));
        seen.push(PrintedFont {
            slug: slug.to_string(),
            id,
            key,
            size_class,
            size_pt: size,
            effective_pt: size * scale,
            text: text.chars().take(100).collect(),
            overflow_of: attr(tag, "data-overflow-of").unwrap_or("").to_string(),
        });
    }
    seen.sort_by(|a, b| {
        a.effective_pt
            .partial_cmp(&b.effective_pt)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    seen.dedup_by(|a, b| a.id == b.id && a.size_pt == b.size_pt);
    Ok(seen)
}

pub fn probe_all(checkout: &Path) -> Result<Vec<PrintedFont>, String> {
    let mut all = Vec::new();
    for slug in FLOOR_SLUGS {
        all.extend(probe_sheet(slug, checkout)?);
    }
    Ok(all)
}
