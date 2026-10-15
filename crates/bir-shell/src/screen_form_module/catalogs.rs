//! Dropdown values come from the typed form's own catalogs. A generic ATC
//! list would offer codes a particular return cannot serialize or validate.

use bir_core::forms::agent::{self, AgentFormKind, Binding};
use bir_core::forms::queueable::QueueableKind;
use bir_core::forms::{excise_places, form_1601fq, form_1702mx_official, form_1702q, form_2000};
use bir_core::reference;
use serde_json::{Map, Value, json};

use super::schema_entry;

fn option(value: &str, label: &str) -> Value {
    json!({"value": value, "label": label})
}

fn code_option(code: &str, description: &str) -> Value {
    option(code, &format!("{code} — {description}"))
}

/// Only the three location structures used by the excise forms. Matching
/// typed paths avoids guessing from opaque or inconsistent official IDs.
fn location(kind: AgentFormKind, key: &str) -> Option<(&str, usize)> {
    if !matches!(
        kind.form_code(),
        "2200A" | "2200AN" | "2200C" | "2200M" | "2200P" | "2200S"
    ) {
        return None;
    }
    let Binding::Text { path, .. } = &kind.definition().bindings.get(key)?.binding else {
        return None;
    };
    let (base, member) = path.rsplit_once('/')?;
    if !matches!(
        base,
        "/place" | "/place_of_production" | "/place_of_removal"
    ) {
        return None;
    }
    let level = ["region", "province", "city"]
        .iter()
        .position(|s| *s == member)?;
    Some((base, level))
}

pub(super) fn edit_order(kind: AgentFormKind, key: &str) -> usize {
    location(kind, key).map_or(0, |(_, level)| level + 1)
}

/// Apply an ordinary user edit, then clear the location's descendants when
/// a parent actually changes. The caller commits this copy only on success.
pub(super) fn fill(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    if let Some(row) = atc_2000_row(kind, key)
        && let Some(choice) = value
            .as_str()
            .filter(|s| s.is_empty() || s.starts_with("catalog:"))
    {
        let selected = if choice.is_empty() {
            None
        } else {
            Some(
                choice
                    .strip_prefix("catalog:")
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|i| form_2000::FORM_2000_ATC_OPTIONS.get(i))
                    .ok_or_else(|| "Choose an ATC from this return's list.".to_string())?,
            )
        };
        // Honor the agent's locks, then use the typed setter: DS112 has
        // two descriptions with different calculations for the same code.
        agent::fill_as_user(
            kind,
            draft,
            key,
            &Value::from(selected.map_or("", |a| a.code)),
        )?;
        let mut typed: form_2000::Form2000Draft =
            serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?;
        if let Some(selected) = selected {
            typed.set_atc(row, selected)?;
        } else {
            typed.clear_atc(row);
        }
        *draft = serde_json::to_value(typed).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let loc = location(kind, key);
    let before = loc.and_then(|(base, _)| draft.pointer(base).cloned());
    agent::fill_as_user(kind, draft, key, value)?;
    // 1702RT Item 5: the official select is enabled only once "Other" is
    // chosen, so picking a code chooses "Other".
    if key == "frm1702RT:drpPg1I5AtcOther"
        && kind == AgentFormKind::Queueable(QueueableKind::Form1702RT)
        && value.as_str().is_some_and(|code| !code.trim().is_empty())
    {
        agent::fill_as_user(
            kind,
            draft,
            "frm1702RT:rdoPg1I5AtcOther",
            &Value::from("true"),
        )?;
    }
    if let Some((base, level)) = loc {
        let member = ["region", "province", "city"][level];
        if before.as_ref().and_then(|v| v.get(member)) != draft.pointer(&format!("{base}/{member}"))
        {
            for child_key in kind.definition().bindings.keys() {
                if location(kind, child_key).is_some_and(|(b, l)| b == base && l > level) {
                    agent::fill_as_user(kind, draft, child_key, &Value::from(""))?;
                }
            }
        }
    }
    Ok(())
}

fn atc_2000_row(kind: AgentFormKind, key: &str) -> Option<usize> {
    if kind.form_code() != "2000" {
        return None;
    }
    let Binding::Text { path, .. } = &kind.definition().bindings.get(key)?.binding else {
        return None;
    };
    path.strip_prefix("/schedule1/")?
        .strip_suffix("/atc_code")?
        .parse()
        .ok()
}

fn field_options(kind: AgentFormKind, key: &str, draft: &Value) -> Option<Vec<Value>> {
    let code = kind.form_code();
    if let Some((base, level)) = location(kind, key) {
        let place = &draft[base.trim_start_matches('/')];
        let region = place["region"].as_str().unwrap_or_default();
        let province = place["province"].as_str().unwrap_or_default();
        let rows = match level {
            0 => excise_places::regions(code),
            1 => excise_places::provinces(code, region),
            _ => excise_places::cities(code, region, province),
        };
        return Some(rows.into_iter().map(|(c, n)| code_option(c, n)).collect());
    }
    if matches!(kind.definition().bindings.get(key).map(|b| &b.binding), Some(Binding::Text { path, .. }) if path.rsplit('/').next().is_some_and(|leaf| leaf == "rdo_code" || leaf.ends_with("_rdo_code")))
    {
        return Some(
            reference::get_all_rdos()
                .iter()
                .map(|r| code_option(&r.code, &r.description))
                .collect(),
        );
    }
    match code {
        "1601FQ" if key.starts_with("drpTreatyCode") => Some(
            std::iter::once(option("NA", "Not applicable"))
                .chain(
                    form_1601fq::FORM_1601FQ_TREATIES
                        .iter()
                        .map(|(c, n)| code_option(c, n)),
                )
                .collect(),
        ),
        "1601FQ" if key.starts_with("drpATCCode") => {
            // Schedule 1 accepts the union of both categories (the main
            // ATC popup is category-specific). Keep the first label per code.
            let mut seen = std::collections::BTreeSet::new();
            Some(
                std::iter::once(option("NA", "Not applicable"))
                    .chain(
                        form_1601fq::FORM_1601FQ_PRIVATE_ATCS
                            .iter()
                            .chain(form_1601fq::FORM_1601FQ_GOVERNMENT_ATCS)
                            .filter(|a| seen.insert(a.code))
                            .map(|a| code_option(a.code, a.description)),
                    )
                    .collect(),
            )
        }
        "1702Q" if key == "frm1702q:cbATC_2" => Some(
            form_1702q::FORM_1702Q_ATC_OPTIONS
                .iter()
                .map(|a| option(a.value, a.label))
                .collect(),
        ),
        "1702MX" if key == "frm1702MX:drpPg1I5ATCR2" => Some(
            form_1702mx_official::FORM_1702MX_ATC_OPTIONS
                .iter()
                .map(|(c, n)| option(c, n))
                .collect(),
        ),
        "1702RT" if key == "frm1702RT:drpPg1I5AtcOther" => {
            // Same eight actual codes as the painted page's OTHER_ATCS
            // (form_1702rt_view.rs). Its "none" is a UI sentinel, not a code.
            Some(
                [
                    ("IC010", "Corporation in general"),
                    ("IC020", "Taxable partnership"),
                    ("IC040", "Government entity"),
                    ("IC041", "National government and LGUs"),
                    ("IC070", "Resident foreign corporation"),
                    ("IC101_25%", "Regional operating headquarters 25%"),
                    ("IC190_25%", "Offshore banking units 25%"),
                    ("IC191_25%", "Foreign currency deposit units 25%"),
                ]
                .into_iter()
                .map(|(c, n)| code_option(c, n))
                .collect(),
            )
        }
        "2000" if atc_2000_row(kind, key).is_some() => Some(
            form_2000::FORM_2000_ATC_OPTIONS
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let row = &draft["schedule1"][atc_2000_row(kind, key).unwrap_or_default()];
                    json!({
                        "value": format!("catalog:{i}"),
                        "label": format!("{} — {}", a.code, a.description),
                        "selected": row["atc_code"] == a.code && row["description"] == a.description,
                    })
                })
                .collect(),
        ),
        // These six boxes are currently locked by the agent seam; supplying
        // labels does not unlock them or implement the missing row editor.
        "2551Q" if key.starts_with("drpATC") => {
            // The field map holds a 1-based option value, not PT010 etc.
            // In particular PT010 has separate 3% and 1% entries.
            static ATCS: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
            let atcs = ATCS.get_or_init(|| {
                serde_json::from_str(include_str!(
                    "../../../bir-core/data/official-xml/2551q-v2018-atc-options.json"
                ))
                .expect("pinned 2551Q ATC options")
            });
            Some(
                atcs["options"]
                    .as_array()
                    .expect("ATC options array")
                    .iter()
                    .map(|a| {
                        option(
                            a["value"].as_str().expect("option value"),
                            &format!(
                                "{} — {} ({}%)",
                                a["code"].as_str().expect("ATC code"),
                                a["description"].as_str().expect("ATC description"),
                                a["rate"].as_str().expect("ATC rate")
                            ),
                        )
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// Only supply missing lists. Reviewed schema options remain authoritative.
/// Empty province/city lists are intentional until their parent is chosen.
pub(super) fn options(kind: AgentFormKind, draft: &Value) -> Value {
    let mut options = Map::new();
    let Some(form) = schema_entry(kind.form_code()) else {
        return Value::Object(options);
    };
    let mut add = |field: &Value, key: &str| {
        if field["t"] == "sel"
            && field["o"].as_array().is_none_or(Vec::is_empty)
            && let Some(rows) = field_options(kind, key, draft)
        {
            options.insert(key.to_string(), Value::Array(rows));
        }
    };
    for part in form["parts"].as_array().into_iter().flatten() {
        for item in part["items"].as_array().into_iter().flatten() {
            for field in item["f"].as_array().into_iter().flatten() {
                if let Some(key) = field["k"].as_str() {
                    add(field, key);
                }
            }
            for row in item["keys"].as_array().into_iter().flatten() {
                for (col, key) in row.as_array().into_iter().flatten().enumerate() {
                    if let Some(key) = key.as_str() {
                        add(&item["cols"][col], key);
                    }
                }
            }
        }
    }
    Value::Object(options)
}
