//! Character boxes on the form page (rule 5): which boxes the printed sheet
//! gives a fixed number of character slots, and which stacked lines of one
//! printed address the page shows as one pool.
//!
//! The slot counts come from the print sheets (`html-frozen/<slug>/`: a
//! writer-cells join puts a key in a comb cell, whose `data-comb-slots` is
//! its capacity), copied into `app/forms/comb-slots.json` by the ignored
//! test `write_comb_slots`; `comb_slots_match_the_print_sheets` keeps the
//! copy in step. The page reads them from the schema:
//!
//! - `slots: N` on a text, phone or number box: boxes until the text no
//!   longer fits, then a plain field. Email, amounts and dates have none.
//! - `pool: {keys, slots, write}` on the first line of a printed address
//!   that runs over stacked lines (2552's 40 + 31 slots): the page shows one
//!   box for the whole address, laid over the lines' slots while it fits.
//!   `write` are the lines with a draft field of their own; the page splits
//!   the text over them (the last one takes the rest), or gives it whole to
//!   the first line when that is the only one (its form's print splits it).
//!   The later lines are hidden: they are part of the pool.

use std::sync::OnceLock;

use bir_core::forms::agent::{AgentFormKind, Binding};
use serde_json::{Map, Value, json};

/// `app/forms/comb-slots.json`: form code -> key -> `[slots, printed caption]`.
fn comb_slots() -> &'static Value {
    static SLOTS: OnceLock<Value> = OnceLock::new();
    SLOTS.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../app/forms/comb-slots.json"))
            .expect("app/forms/comb-slots.json is a JSON object")
    })
}

/// A printed caption without its line marker: "9 Registered Address (second
/// line)" and "9 Registered Address, line 2" both read "9 registered address".
fn caption_stem(caption: &str) -> String {
    let lower = caption.trim().to_ascii_lowercase();
    const MARKERS: [&str; 12] = [
        " (first line)",
        " (second line)",
        " (third line)",
        ", first line",
        ", second line",
        ", third line",
        " (line 1)",
        " (line 2)",
        " (line 3)",
        ", line 1",
        ", line 2",
        ", line 3",
    ];
    MARKERS
        .iter()
        .find_map(|marker| lower.strip_suffix(marker))
        .unwrap_or(&lower)
        .trim()
        .to_string()
}

/// Boxes that keep character slots: text, phone and number boxes. An email
/// is always a plain field, and amounts and dates have their own look.
fn takes_slots(field: &Value) -> bool {
    matches!(field["t"].as_str(), Some("text" | "phone" | "int"))
}

/// Give each box its printed slots and gather each stacked address into one
/// pool (see the module docs).
pub(super) fn apply_combs(form: &mut Value, code: &str) {
    let Some(slots) = comb_slots()[code].as_object() else {
        return;
    };
    let def = AgentFormKind::from_code(code).map(|kind| kind.definition());
    let bound = |key: &str| {
        def.and_then(|def| def.bindings.get(key))
            .is_some_and(|bound| matches!(bound.binding, Binding::Text { .. }))
    };
    let items = form["parts"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array_mut().into_iter().flatten());
    for item in items {
        let Some(fields) = item["f"].as_array_mut() else {
            continue;
        };
        let slot = |field: &Value| -> Option<(u64, String)> {
            let entry = slots.get(field["k"].as_str()?)?;
            Some((entry[0].as_u64()?, entry[1].as_str()?.to_string()))
        };
        for field in fields.iter_mut() {
            if let (true, Some((n, _))) = (takes_slots(field), slot(field)) {
                field["slots"] = json!(n);
            }
        }
        // Runs of text lines printed under one address caption.
        let mut start = 0;
        while start < fields.len() {
            let head = &fields[start];
            let stem = slot(head)
                .filter(|_| head["t"] == "text")
                .map(|(_, caption)| caption_stem(&caption))
                .filter(|stem| stem.contains("address"));
            let Some(stem) = stem else {
                start += 1;
                continue;
            };
            let mut end = start + 1;
            while end < fields.len()
                && fields[end]["t"] == "text"
                && slot(&fields[end]).is_some_and(|(_, caption)| caption_stem(&caption) == stem)
            {
                end += 1;
            }
            let head_key = head["k"].as_str().unwrap_or_default().to_string();
            if end - start >= 2 && bound(&head_key) {
                let lines: Vec<(String, u64)> = fields[start..end]
                    .iter()
                    .filter_map(|field| Some((field["k"].as_str()?.to_string(), slot(field)?.0)))
                    .collect();
                let write: Vec<&String> = lines
                    .iter()
                    .map(|(key, _)| key)
                    .filter(|key| bound(key))
                    .collect();
                let mut pool = Map::new();
                pool.insert(
                    "keys".into(),
                    json!(lines.iter().map(|l| &l.0).collect::<Vec<_>>()),
                );
                pool.insert(
                    "slots".into(),
                    json!(lines.iter().map(|l| l.1).collect::<Vec<_>>()),
                );
                pool.insert("write".into(), json!(write));
                fields[start]["pool"] = Value::Object(pool);
                for field in &mut fields[start + 1..end] {
                    field["t"] = Value::from("hidden");
                    field["pooled"] = Value::from(head_key.as_str());
                }
            }
            start = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// The comb cells of a print sheet: html id -> slots.
    fn sheet_combs(html: &str) -> BTreeMap<String, u64> {
        let attr = |tag: &str, name: &str| -> Option<String> {
            let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
            let len = tag[start..].find('"')?;
            Some(tag[start..start + len].to_string())
        };
        html.split("<div ")
            .skip(1)
            .filter_map(|rest| {
                let tag = format!(" {}", &rest[..rest.find('>')?]);
                if attr(&tag, "data-field-kind").as_deref() != Some("comb") {
                    return None;
                }
                Some((
                    attr(&tag, "id")?,
                    attr(&tag, "data-comb-slots")?.parse().ok()?,
                ))
            })
            .collect()
    }

    /// What `app/forms/comb-slots.json` should hold, read from the sheets.
    fn derived() -> Value {
        let root = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
        let mut out = Map::new();
        for form in super::super::schema() {
            let code = form["c"].as_str().unwrap_or_default();
            let slug = form["s"].as_str().unwrap_or_default();
            let Ok(html) = std::fs::read_to_string(format!("{root}/html-frozen/{slug}/index.html"))
            else {
                continue;
            };
            let cells: Value =
                std::fs::read_to_string(format!("{root}/html-frozen/{slug}/writer-cells.json"))
                    .ok()
                    .and_then(|raw| serde_json::from_str(&raw).ok())
                    .unwrap_or(Value::Null);
            let combs = sheet_combs(&html);
            let mut keys = Map::new();
            for join in cells["joins"].as_array().into_iter().flatten() {
                let (Some(key), Some(id)) = (join["writer_key"].as_str(), join["html_id"].as_str())
                else {
                    continue;
                };
                if let Some(slots) = combs.get(id) {
                    keys.entry(key.to_string()).or_insert_with(|| {
                        json!([slots, join["printed"].as_str().unwrap_or_default()])
                    });
                }
            }
            if !keys.is_empty() {
                out.insert(code.to_string(), Value::Object(keys));
            }
        }
        Value::Object(out)
    }

    #[test]
    #[ignore = "writes app/forms/comb-slots.json from the print sheets"]
    fn write_comb_slots() {
        let path = format!(
            "{}/../../app/forms/comb-slots.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = serde_json::to_string(&derived()).expect("json") + "\n";
        std::fs::write(path, text).expect("comb-slots.json");
    }

    #[test]
    fn comb_slots_match_the_print_sheets() {
        assert_eq!(
            comb_slots(),
            &derived(),
            "app/forms/comb-slots.json is stale: run `cargo test -p bir-shell write_comb_slots -- --ignored`"
        );
    }

    #[test]
    fn a_caption_stem_drops_only_the_line_marker() {
        for caption in [
            "9 Registered Address",
            "9 Registered Address (second line)",
            "9 Registered Address, line 2",
            "9 Registered Address (line 2)",
        ] {
            assert_eq!(caption_stem(caption), "9 registered address", "{caption}");
        }
        assert_eq!(caption_stem("17 d description"), "17 d description");
    }
}
