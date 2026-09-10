//! Frozen HTML fill/print: set `input[name]` from the writer map.
//!
//! Layout lives in `html-frozen/<slug>/`. `name=` is a fail-closed catalog
//! join; `id=` stays the cell id. Catalog `official_field_key` stamps are
//! TIN/branch only. Unstamped leftover keys keep cell-id `name=`;
//! `html-frozen/<slug>/writer-cells.json` may copy a writer value onto that
//! cell when the freeze sheet has a 1:1 printed-caption join (`joins`),
//! split a leftover `{:.2}` money key onto a catalog peso comb plus 2-slot
//! cents comb (`money_joins`), or mark a catalog xbox when a leftover
//! boolean/radio writer is `"true"` (`xbox_joins`, glyph `X`). Text `joins`
//! ASCII-uppercase letters into comb slots and `data-writer-value` so BIR
//! CAPITAL LETTERS print matches the form instruction; digits, punctuation,
//! money, and xbox are unchanged. Profile/DB values stay as stored. Those are
//! fill-paths, not `official_field_key` harvests. Do not stamp `name=` on
//! peso/cent/xbox boxes, do not left-align a dotted money string into the
//! peso comb, and do not write `"true"` into an xbox.

use bir_core::forms::form_2551q::Form2551QDraft;
use std::collections::{BTreeMap, BTreeSet};

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
    fill_by_name_with_cells(html, fields, &BTreeMap::new())
}

fn fill_by_name_with_cells(
    html: &str,
    fields: &BTreeMap<String, String>,
    cells: &BTreeMap<String, String>,
) -> String {
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
        let (input_name, via_cell) = if grouped.contains_key(name.as_str()) {
            (name.as_str(), false)
        } else if let Some(cell) = cells.get(name) {
            (cell.as_str(), true)
        } else {
            continue;
        };
        let Some(indices) = grouped.get(input_name) else {
            continue;
        };
        // Writer-cell text joins print as BIR CAPITAL LETTERS. Stamped
        // TIN/branch (`via_cell` false) keep the writer map as-is.
        let print_value = via_cell.then(|| value.to_ascii_uppercase());
        let print = print_value.as_deref().unwrap_or(value.as_str());
        let comb = indices.iter().any(|&index| tags[index].slot.is_some());
        if comb {
            let mut ordered = indices.clone();
            ordered.sort_by_key(|&index| tags[index].slot.unwrap_or(usize::MAX));
            let chars: Vec<char> = print.chars().collect();
            for (offset, index) in ordered.into_iter().enumerate() {
                let ch = chars
                    .get(offset)
                    .copied()
                    .map(|c| c.to_string())
                    .unwrap_or_default();
                let writer = (via_cell && offset == 0).then_some(print);
                replacements.push((
                    tags[index].start,
                    tags[index].end,
                    set_value(tags[index].tag, &ch, writer),
                ));
            }
        } else {
            for &index in indices {
                let writer = via_cell.then_some(print);
                replacements.push((
                    tags[index].start,
                    tags[index].end,
                    set_value(tags[index].tag, print, writer),
                ));
            }
        }
    }

    apply_replacements(html, replacements)
}

fn apply_replacements(html: &str, mut replacements: Vec<(usize, usize, String)>) -> String {
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

fn writer_cells_json(slug: &str) -> Option<&'static str> {
    match slug {
        "1601c-2018" => Some(include_str!(
            "../../../html-frozen/1601c-2018/writer-cells.json"
        )),
        "2551q-2018" => Some(include_str!(
            "../../../html-frozen/2551q-2018/writer-cells.json"
        )),
        _ => None,
    }
}

const XBOX_CHECKED_GLYPH: &str = "X";

struct MoneyJoin {
    writer_key: String,
    peso_html_id: String,
    cent_html_id: String,
}

struct XboxJoin {
    writer_key: String,
    html_id: String,
}

struct WriterCells {
    joins: BTreeMap<String, String>,
    money_joins: Vec<MoneyJoin>,
    xbox_joins: Vec<XboxJoin>,
}

fn parse_writer_cells(json: &str) -> Result<WriterCells, String> {
    let payload: serde_json::Value =
        serde_json::from_str(json).map_err(|error| format!("writer-cells.json: {error}"))?;
    let joins = payload
        .get("joins")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "writer-cells.json missing joins[]".to_string())?;
    let mut cells = BTreeMap::new();
    for join in joins {
        let key = join
            .get("writer_key")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "writer-cells.json join missing writer_key".to_string())?;
        let html_id = join
            .get("html_id")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("writer-cells.json {key} missing html_id"))?;
        if html_id.starts_with("frm") {
            return Err(format!(
                "writer-cells.json {key} must target a cell id, not {html_id}"
            ));
        }
        if cells.insert(key.to_string(), html_id.to_string()).is_some() {
            return Err(format!("writer-cells.json duplicate writer_key {key}"));
        }
    }

    let mut money_joins = Vec::new();
    if let Some(rows) = payload
        .get("money_joins")
        .and_then(|value| value.as_array())
    {
        for join in rows {
            let key = join
                .get("writer_key")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "writer-cells.json money_join missing writer_key".to_string())?;
            let peso_html_id = join
                .get("peso_html_id")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("writer-cells.json {key} missing peso_html_id"))?;
            let cent_html_id = join
                .get("cent_html_id")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("writer-cells.json {key} missing cent_html_id"))?;
            if peso_html_id.starts_with("frm") || cent_html_id.starts_with("frm") {
                return Err(format!(
                    "writer-cells.json {key} must target cell ids, not stamped names"
                ));
            }
            if peso_html_id == cent_html_id {
                return Err(format!(
                    "writer-cells.json {key} peso and cent cells must differ"
                ));
            }
            if cells.contains_key(key) {
                return Err(format!(
                    "writer-cells.json duplicate writer_key {key} across joins and money_joins"
                ));
            }
            if money_joins
                .iter()
                .any(|row: &MoneyJoin| row.writer_key == key)
            {
                return Err(format!(
                    "writer-cells.json duplicate money writer_key {key}"
                ));
            }
            money_joins.push(MoneyJoin {
                writer_key: key.to_string(),
                peso_html_id: peso_html_id.to_string(),
                cent_html_id: cent_html_id.to_string(),
            });
        }
    }

    let mut xbox_joins = Vec::new();
    if let Some(rows) = payload.get("xbox_joins").and_then(|value| value.as_array()) {
        for join in rows {
            let key = join
                .get("writer_key")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "writer-cells.json xbox_join missing writer_key".to_string())?;
            let html_id = join
                .get("html_id")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("writer-cells.json {key} missing html_id"))?;
            if html_id.starts_with("frm") {
                return Err(format!(
                    "writer-cells.json {key} must target a cell id, not {html_id}"
                ));
            }
            if cells.contains_key(key)
                || money_joins
                    .iter()
                    .any(|row: &MoneyJoin| row.writer_key == key)
                || xbox_joins
                    .iter()
                    .any(|row: &XboxJoin| row.writer_key == key)
            {
                return Err(format!(
                    "writer-cells.json duplicate writer_key {key} across joins"
                ));
            }
            xbox_joins.push(XboxJoin {
                writer_key: key.to_string(),
                html_id: html_id.to_string(),
            });
        }
    }

    {
        let mut used_cells: BTreeSet<&str> = cells.values().map(|id| id.as_str()).collect();
        for join in &money_joins {
            if !used_cells.insert(&join.peso_html_id) {
                return Err(format!(
                    "writer-cells.json {} peso cell {} is already a fill target",
                    join.writer_key, join.peso_html_id
                ));
            }
            if !used_cells.insert(&join.cent_html_id) {
                return Err(format!(
                    "writer-cells.json {} cent cell {} is already a fill target",
                    join.writer_key, join.cent_html_id
                ));
            }
        }
        for join in &xbox_joins {
            if !used_cells.insert(&join.html_id) {
                return Err(format!(
                    "writer-cells.json {} xbox cell {} is already a fill target",
                    join.writer_key, join.html_id
                ));
            }
        }
    }

    Ok(WriterCells {
        joins: cells,
        money_joins,
        xbox_joins,
    })
}

fn writer_cells(slug: &str) -> Result<WriterCells, String> {
    match writer_cells_json(slug) {
        Some(json) => parse_writer_cells(json),
        None => Ok(WriterCells {
            joins: BTreeMap::new(),
            money_joins: Vec::new(),
            xbox_joins: Vec::new(),
        }),
    }
}

fn grouped_input_indices<'a>(tags: &[InputTag<'a>]) -> BTreeMap<&'a str, Vec<usize>> {
    let mut grouped: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, tag) in tags.iter().enumerate() {
        grouped.entry(tag.name).or_default().push(index);
    }
    grouped
}

fn ordered_comb_indices(tags: &[InputTag<'_>], indices: &[usize]) -> Vec<usize> {
    let mut ordered = indices.to_vec();
    ordered.sort_by_key(|&index| tags[index].slot.unwrap_or(usize::MAX));
    ordered
}

fn split_writer_money(value: &str) -> Option<(String, String)> {
    let cleaned: String = value
        .chars()
        .filter(|ch| *ch != ',' && !ch.is_whitespace())
        .collect();
    if cleaned.is_empty() || cleaned.starts_with('-') {
        return None;
    }
    let (peso, cents) = match cleaned.split_once('.') {
        Some((peso, cents)) => (peso, cents),
        None => (cleaned.as_str(), "00"),
    };
    if peso.chars().any(|ch| !ch.is_ascii_digit()) {
        return None;
    }
    if cents.is_empty() || cents.len() > 2 || cents.chars().any(|ch| !ch.is_ascii_digit()) {
        return None;
    }
    Some((peso.to_string(), format!("{cents:0<2}")))
}

fn right_aligned_slot_values(slots: usize, digits: &str) -> Option<Vec<String>> {
    if digits.len() > slots {
        return None;
    }
    let pad = slots - digits.len();
    let mut values = vec![String::new(); slots];
    for (offset, ch) in digits.chars().enumerate() {
        values[pad + offset] = ch.to_string();
    }
    Some(values)
}

fn fill_money_joins(
    html: &str,
    fields: &BTreeMap<String, String>,
    money_joins: &[MoneyJoin],
) -> String {
    if money_joins.is_empty() {
        return html.to_string();
    }
    let tags = input_tags(html);
    let grouped = grouped_input_indices(&tags);
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for join in money_joins {
        let Some(value) = fields.get(&join.writer_key) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        let Some((peso, cents)) = split_writer_money(value) else {
            continue;
        };
        let Some(peso_indices) = grouped.get(join.peso_html_id.as_str()) else {
            continue;
        };
        let Some(cent_indices) = grouped.get(join.cent_html_id.as_str()) else {
            continue;
        };
        let peso_ordered = ordered_comb_indices(&tags, peso_indices);
        let cent_ordered = ordered_comb_indices(&tags, cent_indices);
        if cent_ordered.len() != 2 {
            continue;
        }
        let Some(peso_values) = right_aligned_slot_values(peso_ordered.len(), &peso) else {
            continue;
        };
        for (offset, index) in peso_ordered.into_iter().enumerate() {
            let writer = (offset == 0).then_some(value.as_str());
            replacements.push((
                tags[index].start,
                tags[index].end,
                set_value(tags[index].tag, &peso_values[offset], writer),
            ));
        }
        for (offset, index) in cent_ordered.into_iter().enumerate() {
            let ch = cents
                .chars()
                .nth(offset)
                .map(|ch| ch.to_string())
                .unwrap_or_default();
            replacements.push((
                tags[index].start,
                tags[index].end,
                set_value(tags[index].tag, &ch, None),
            ));
        }
    }
    apply_replacements(html, replacements)
}

fn writer_is_checked(value: &str) -> bool {
    matches!(value.trim(), "true" | "TRUE" | "1" | "Y" | "y" | "X" | "x")
}

fn fill_xbox_joins(
    html: &str,
    fields: &BTreeMap<String, String>,
    xbox_joins: &[XboxJoin],
) -> String {
    if xbox_joins.is_empty() {
        return html.to_string();
    }
    let tags = input_tags(html);
    let grouped = grouped_input_indices(&tags);
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for join in xbox_joins {
        let Some(value) = fields.get(&join.writer_key) else {
            continue;
        };
        if !writer_is_checked(value) {
            continue;
        }
        let Some(indices) = grouped.get(join.html_id.as_str()) else {
            continue;
        };
        if indices.len() != 1 {
            continue;
        }
        let index = indices[0];
        if tags[index].slot.is_some() {
            continue;
        }
        replacements.push((
            tags[index].start,
            tags[index].end,
            set_value(tags[index].tag, XBOX_CHECKED_GLYPH, Some(value.as_str())),
        ));
    }
    apply_replacements(html, replacements)
}

fn validate_writer_cells(html: &str, slug: &str, cells: &WriterCells) -> Result<(), String> {
    if cells.joins.is_empty() && cells.money_joins.is_empty() && cells.xbox_joins.is_empty() {
        return Ok(());
    }
    let tags = input_tags(html);
    let present: BTreeSet<&str> = tags.iter().map(|tag| tag.name).collect();
    let grouped = grouped_input_indices(&tags);
    for (key, html_id) in &cells.joins {
        if !present.contains(html_id.as_str()) {
            return Err(format!(
                "{slug}: writer-cells.html_id {html_id} for {key} is not an input name="
            ));
        }
        if present.contains(key.as_str()) {
            return Err(format!(
                "{slug}: writer-cells {key} is already a stamped name=; remove the fill join"
            ));
        }
    }
    for join in &cells.money_joins {
        if !present.contains(join.peso_html_id.as_str()) {
            return Err(format!(
                "{slug}: writer-cells peso_html_id {} for {} is not an input name=",
                join.peso_html_id, join.writer_key
            ));
        }
        if !present.contains(join.cent_html_id.as_str()) {
            return Err(format!(
                "{slug}: writer-cells cent_html_id {} for {} is not an input name=",
                join.cent_html_id, join.writer_key
            ));
        }
        if present.contains(join.writer_key.as_str()) {
            return Err(format!(
                "{slug}: writer-cells {} is already a stamped name=; remove the money join",
                join.writer_key
            ));
        }
        let peso_slots = grouped
            .get(join.peso_html_id.as_str())
            .map(|indices| indices.len())
            .unwrap_or(0);
        let cent_slots = grouped
            .get(join.cent_html_id.as_str())
            .map(|indices| indices.len())
            .unwrap_or(0);
        if peso_slots < 3 {
            return Err(format!(
                "{slug}: writer-cells {} peso comb {} must have at least 3 slots",
                join.writer_key, join.peso_html_id
            ));
        }
        if cent_slots != 2 {
            return Err(format!(
                "{slug}: writer-cells {} cent comb {} must be a 2-slot comb",
                join.writer_key, join.cent_html_id
            ));
        }
    }
    for join in &cells.xbox_joins {
        if !present.contains(join.html_id.as_str()) {
            return Err(format!(
                "{slug}: writer-cells xbox html_id {} for {} is not an input name=",
                join.html_id, join.writer_key
            ));
        }
        if present.contains(join.writer_key.as_str()) {
            return Err(format!(
                "{slug}: writer-cells {} is already a stamped name=; remove the xbox join",
                join.writer_key
            ));
        }
        let indices = grouped
            .get(join.html_id.as_str())
            .cloned()
            .unwrap_or_default();
        if indices.len() != 1 {
            return Err(format!(
                "{slug}: writer-cells {} xbox {} must be a single unslotted input",
                join.writer_key, join.html_id
            ));
        }
        if tags[indices[0]].slot.is_some() {
            return Err(format!(
                "{slug}: writer-cells {} xbox {} must not be a comb",
                join.writer_key, join.html_id
            ));
        }
    }
    Ok(())
}

fn fill_bundle(
    html: &str,
    fields: &BTreeMap<String, String>,
    slug: &str,
) -> Result<String, String> {
    let cells = writer_cells(slug)?;
    validate_writer_cells(html, slug, &cells)?;
    let filled = fill_by_name_with_cells(html, fields, &cells.joins);
    let filled = fill_money_joins(&filled, fields, &cells.money_joins);
    Ok(fill_xbox_joins(&filled, fields, &cells.xbox_joins))
}

/// Frozen 2551Q HTML with writer values on stamped `name=` inputs,
/// writer-cell identity, money, and xbox joins.
pub fn fill_2551q(draft: &Form2551QDraft) -> String {
    fill_bundle(html_2551q(), &draft.to_bir_field_map(), "2551q-2018")
        .expect("2551q-2018 writer-cells")
}

/// Self-contained document for a WebView `with_html` host (inline CSS, fonts, PNGs).
pub fn filled_document(slug: &str, fields: &BTreeMap<String, String>) -> Result<String, String> {
    let Some(loaded) = bundle(slug) else {
        return Err(format!("no frozen HTML bundle for {slug}"));
    };
    Ok(inline_local_assets(
        &fill_bundle(loaded.html, fields, slug)?,
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

fn set_value(tag: &str, value: &str, writer_value: Option<&str>) -> String {
    let escaped = html_escape(value);
    let needle = "value=\"";
    let mut out = if let Some(value_at) = tag.find(needle) {
        let content_at = value_at + needle.len();
        if let Some(close) = tag[content_at..].find('"') {
            let close_at = content_at + close;
            let mut rewritten = String::with_capacity(tag.len() + escaped.len());
            rewritten.push_str(&tag[..content_at]);
            rewritten.push_str(&escaped);
            rewritten.push_str(&tag[close_at..]);
            rewritten
        } else {
            tag.to_string()
        }
    } else {
        let insert_at = tag.rfind('>').unwrap_or(tag.len());
        format!(
            "{} value=\"{}\"{}",
            &tag[..insert_at],
            escaped,
            &tag[insert_at..]
        )
    };
    if let Some(writer) = writer_value.filter(|value| !value.is_empty()) {
        let attr = format!(" data-writer-value=\"{}\"", html_escape(writer));
        if !out.contains("data-writer-value=\"") {
            let insert_at = out.rfind('>').unwrap_or(out.len());
            out.insert_str(insert_at, &attr);
        }
    }
    out
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
        assert!(input_tags(html_2551q())
            .into_iter()
            .all(|tag| tag.name != "p1c20"));
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
    fn fill_by_name_with_cells_uppercases_letter_combs_not_stamped_digits() {
        let html = concat!(
            r#"<input name="p1c36" data-slot-index="0" maxlength="1">"#,
            r#"<input name="p1c36" data-slot-index="1" maxlength="1">"#,
            r#"<input name="p1c36" data-slot-index="2" maxlength="1">"#,
            r#"<input name="frm1601c:txtTIN1" data-slot-index="0" maxlength="1">"#,
        );
        let mut fields = BTreeMap::new();
        fields.insert("frm1601c:txtTaxpayerName".to_string(), "Ab".to_string());
        fields.insert("frm1601c:txtTIN1".to_string(), "9".to_string());
        let mut cells = BTreeMap::new();
        cells.insert("frm1601c:txtTaxpayerName".to_string(), "p1c36".to_string());
        let filled = fill_by_name_with_cells(html, &fields, &cells);
        assert_eq!(comb_text(&filled, "p1c36"), "AB");
        assert!(filled.contains("data-writer-value=\"AB\""));
        assert!(!filled.contains("data-writer-value=\"Ab\""));
        assert_eq!(named_values(&filled, "frm1601c:txtTIN1"), ["9"]);
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
            ("1701q-2018", "frm1701q:", 6),
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

    fn comb_text(html: &str, name: &str) -> String {
        named_values(html, name).concat()
    }

    fn identity_map(
        name_key: &str,
        name: &str,
        address_key: &str,
        address: &str,
    ) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        fields.insert(name_key.to_string(), name.to_string());
        fields.insert(address_key.to_string(), address.to_string());
        fields
    }

    #[test]
    fn fill_by_name_without_writer_cells_leaves_unstamped_identity_blank() {
        let html = bundle("1601c-2018").expect("1601c").html;
        let mut fields = identity_map(
            "frm1601c:txtTaxpayerName",
            "NEXUS PAYROLL CORP",
            "frm1601c:txtAddress",
            "42 Banahaw Street",
        );
        fields.insert("txtEmail".to_string(), "juan@example.com".to_string());
        fields.insert("frm1601c:txtTax14".to_string(), "8888.88".to_string());
        let filled = fill_by_name(html, &fields);
        assert_eq!(comb_text(&filled, "p1c36"), "");
        assert_eq!(comb_text(&filled, "p1c38"), "");
        assert_eq!(comb_text(&filled, "p1c48"), "");
        assert_eq!(comb_text(&filled, "p1c56"), "");
        assert!(!filled.contains("NEXUS PAYROLL CORP"));
        assert!(!filled.contains("42 Banahaw Street"));
        assert!(!filled.contains("juan@example.com"));
        assert!(!filled.contains("8888.88"));
    }

    #[test]
    fn filled_document_1601c_fills_taxpayer_name_and_address() {
        let fields = identity_map(
            "frm1601c:txtTaxpayerName",
            "Andrea Mae Alicando Galang",
            "frm1601c:txtAddress",
            "42 Banahaw Street",
        );
        let html = filled_document("1601c-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c36"), "ANDREA MAE ALICANDO GALANG");
        assert_eq!(comb_text(&html, "p1c38"), "42 BANAHAW STREET");
        assert!(html.contains("ANDREA MAE ALICANDO GALANG"));
        assert!(html.contains("data-writer-value=\"ANDREA MAE ALICANDO GALANG\""));
        assert!(html.contains("42 BANAHAW STREET"));
        assert!(!html.contains("Andrea Mae Alicando Galang"));
        assert!(!html.contains("42 Banahaw Street"));
        let stamped: std::collections::BTreeSet<String> = input_tags(&html)
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm1601c:"))
            .collect();
        assert_eq!(stamped.len(), 4);
    }

    #[test]
    fn filled_document_1601c_fills_email_and_splits_tax_money() {
        let mut fields = BTreeMap::new();
        fields.insert("txtEmail".to_string(), "juan@example.com".to_string());
        fields.insert("frm1601c:txtTax14".to_string(), "8888.88".to_string());
        fields.insert("frm1601c:txtTax25".to_string(), "7777.77".to_string());
        let html = filled_document("1601c-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c48"), "JUAN@EXAMPLE.COM");
        assert!(html.contains("JUAN@EXAMPLE.COM"));
        assert!(!html.contains("juan@example.com"));
        assert_eq!(comb_text(&html, "p1c56"), "8888");
        assert_eq!(comb_text(&html, "p1c58"), "88");
        assert_eq!(comb_text(&html, "p1c101"), "7777");
        assert_eq!(comb_text(&html, "p1c103"), "77");
        assert!(!comb_text(&html, "p1c56").contains('.'));
        assert!(!comb_text(&html, "p1c101").contains('.'));
        assert!(html.contains("8888.88"));
        assert!(html.contains("7777.77"));
        let stamped: std::collections::BTreeSet<String> = input_tags(&html)
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm1601c:"))
            .collect();
        assert_eq!(stamped.len(), 4);
        assert!(!html.contains("name=\"frm1601c:txtTax14\""));
        assert!(!html.contains("name=\"txtEmail\""));
    }

    #[test]
    fn filled_document_2551q_fills_taxpayer_name_and_address() {
        let draft = sample_draft();
        assert_eq!(draft.taxpayer_name, "Frozen Html Fixture");
        assert_eq!(draft.registered_address, "New Cabalan");
        assert_eq!(draft.email, "tax@example.com");
        let html = filled_2551q_document(&draft);
        assert_eq!(draft.taxpayer_name, "Frozen Html Fixture");
        assert_eq!(comb_text(&html, "p1c30"), "FROZEN HTML FIXTURE");
        assert_eq!(comb_text(&html, "p1c32"), "NEW CABALAN");
        assert_eq!(comb_text(&html, "p1c39"), "TAX@EXAMPLE.COM");
        assert_eq!(comb_text(&html, "p1c9"), "12");
        assert_eq!(comb_text(&html, "p1c10"), "2026");
        assert_eq!(named_values(&html, "p1c7"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c11"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c15"), vec!["".to_string()]);
        assert!(html.contains("FROZEN HTML FIXTURE"));
        assert!(html.contains("data-writer-value=\"FROZEN HTML FIXTURE\""));
        assert!(html.contains("NEW CABALAN"));
        assert!(html.contains("TAX@EXAMPLE.COM"));
        assert!(!html.contains("Frozen Html Fixture"));
        assert!(!html.contains("New Cabalan"));
        assert!(!html.contains("tax@example.com"));
        assert!(html.contains("2026"));
        let stamped: std::collections::BTreeSet<String> = input_tags(&html)
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm2551Qv2018:"))
            .collect();
        assert_eq!(
            stamped,
            STAMPED_TIN_NAMES
                .iter()
                .map(|name| (*name).to_string())
                .collect()
        );
    }

    #[test]
    fn filled_document_2551q_fills_email_and_splits_tax_money() {
        let mut fields = BTreeMap::new();
        fields.insert("txtEmail".to_string(), "andrea@example.com".to_string());
        fields.insert("frm2551Qv2018:txt14".to_string(), "1643.10".to_string());
        fields.insert("txtATCAmt1".to_string(), "54770.00".to_string());
        let html = filled_document("2551q-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c39"), "ANDREA@EXAMPLE.COM");
        assert!(html.contains("ANDREA@EXAMPLE.COM"));
        assert!(!html.contains("andrea@example.com"));
        assert_eq!(comb_text(&html, "p1c50"), "1643");
        assert_eq!(comb_text(&html, "p1c52"), "10");
        assert_eq!(comb_text(&html, "p2c15"), "54770");
        assert_eq!(comb_text(&html, "p2c17"), "00");
        assert!(!comb_text(&html, "p1c50").contains('.'));
        assert!(!comb_text(&html, "p2c15").contains('.'));
        assert!(html.contains("1643.10"));
        assert!(html.contains("54770.00"));
        assert!(!html.contains("name=\"frm2551Qv2018:txt14\""));
        assert!(!html.contains("name=\"txtATCAmt1\""));
        assert!(!html.contains("name=\"txtEmail\""));
    }

    #[test]
    fn filled_document_2551q_fills_year_ended_and_quarter() {
        let mut fields = BTreeMap::new();
        fields.insert("frm2551Qv2018:txtYear".to_string(), "2026".to_string());
        fields.insert("frm2551Qv2018:rtnMonth".to_string(), "12".to_string());
        fields.insert("frm2551Qv2018:qtr_1".to_string(), "false".to_string());
        fields.insert("frm2551Qv2018:qtr_2".to_string(), "true".to_string());
        fields.insert("frm2551Qv2018:qtr_3".to_string(), "false".to_string());
        fields.insert("frm2551Qv2018:qtr_4".to_string(), "false".to_string());
        fields.insert("frm2551Qv2018:forThe_1".to_string(), "true".to_string());
        fields.insert("frm2551Qv2018:forThe_2".to_string(), "false".to_string());
        let html = filled_document("2551q-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c10"), "2026");
        assert_eq!(comb_text(&html, "p1c9"), "12");
        assert!(html.contains("2026"));
        assert_eq!(named_values(&html, "p1c15"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c11"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c12"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c13"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c7"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c8"), vec!["".to_string()]);
        assert!(!comb_text(&html, "p1c10").contains("true"));
        assert_ne!(named_values(&html, "p1c15"), vec!["true".to_string()]);
        assert!(!html.contains("name=\"frm2551Qv2018:txtYear\""));
        assert!(!html.contains("name=\"frm2551Qv2018:qtr_2\""));
        let stamped: std::collections::BTreeSet<String> = input_tags(&html)
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm2551Qv2018:"))
            .collect();
        assert_eq!(stamped.len(), 4);
    }

    #[test]
    fn writer_cells_target_catalog_cell_ids_not_stamps() {
        for slug in ["1601c-2018", "2551q-2018"] {
            let html = bundle(slug).unwrap().html;
            let cells = writer_cells(slug).unwrap();
            assert!(!cells.joins.is_empty(), "{slug}");
            assert!(!cells.money_joins.is_empty(), "{slug} money_joins");
            if slug == "2551q-2018" {
                assert!(!cells.xbox_joins.is_empty(), "{slug} xbox_joins");
            }
            let present: std::collections::BTreeSet<&str> =
                input_tags(html).into_iter().map(|tag| tag.name).collect();
            for (key, html_id) in &cells.joins {
                assert!(
                    present.contains(html_id.as_str()),
                    "{slug} {key} -> {html_id}"
                );
                assert!(
                    !key.starts_with("p"),
                    "{slug} writer_key looks like a cell id: {key}"
                );
                assert!(
                    html_id.starts_with("p"),
                    "{slug} html_id must be a cell id: {html_id}"
                );
                assert!(!present.contains(key.as_str()), "{slug} {key} is stamped");
            }
            for join in &cells.money_joins {
                assert!(
                    present.contains(join.peso_html_id.as_str()),
                    "{slug} {} peso {}",
                    join.writer_key,
                    join.peso_html_id
                );
                assert!(
                    present.contains(join.cent_html_id.as_str()),
                    "{slug} {} cent {}",
                    join.writer_key,
                    join.cent_html_id
                );
                assert!(join.peso_html_id.starts_with("p"));
                assert!(join.cent_html_id.starts_with("p"));
                assert_ne!(join.peso_html_id, join.cent_html_id);
                assert!(!present.contains(join.writer_key.as_str()));
            }
            for join in &cells.xbox_joins {
                assert!(
                    present.contains(join.html_id.as_str()),
                    "{slug} {} xbox {}",
                    join.writer_key,
                    join.html_id
                );
                assert!(join.html_id.starts_with("p"));
                assert!(!present.contains(join.writer_key.as_str()));
            }
        }
    }
}
