//! Dropdown values come from the typed form's own catalogs. A generic ATC
//! list would offer codes a particular return cannot serialize or validate.

use bir_core::forms::agent::{self, AgentFormKind, Binding};
use bir_core::forms::queueable::QueueableKind;
use bir_core::forms::{
    excise_places, form_1601fq, form_1702mx_official, form_1702q, form_2000, form_2550m,
};
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
    // A 2550M Schedule 1 amount needs its row, which choosing the ATC makes.
    if sales_2550m_row(kind, key).is_some() {
        return 1;
    }
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
    if let Some(row) = atc_2550m_row(kind, key) {
        return fill_2550m_atc(draft, row, value);
    }
    if let Some(row) = sales_2550m_row(kind, key) {
        return fill_2550m_amount(draft, row, value);
    }
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

/// 2550M Schedule 1 row of `frm2550m:txtAtcCde<n>` (row `n - 1`, the way
/// `schedule_row_controls` writes it). The official page fills these from
/// its ATC popup, one row per ticked ATC in popup order; the probed
/// bindings cannot say that (they tie rows to the samples' codes), so the
/// host edits them through the typed draft.
fn atc_2550m_row(kind: AgentFormKind, key: &str) -> Option<usize> {
    if kind.form_code() != "2550M" {
        return None;
    }
    let n: usize = key.strip_prefix("frm2550m:txtAtcCde")?.parse().ok()?;
    (1..=form_2550m::FORM_2550M_ATCS.len())
        .contains(&n)
        .then(|| n - 1)
}

/// Whether the host writes `key` itself, so the agent's "cannot be filled
/// here" verdict for it does not apply on the page.
pub(super) fn host_writes(kind: AgentFormKind, key: &str) -> bool {
    atc_2550m_row(kind, key).is_some() || sales_2550m_row(kind, key).is_some()
}

/// 2550M Schedule 1 row of `frm2550m:txtAmountSales<n>`. Rows re-sort by
/// ATC, so the probe saw these move with another row's code and left four
/// of them unbound; the host writes the row's amount directly.
fn sales_2550m_row(kind: AgentFormKind, key: &str) -> Option<usize> {
    if kind.form_code() != "2550M" {
        return None;
    }
    let n: usize = key.strip_prefix("frm2550m:txtAmountSales")?.parse().ok()?;
    (1..=form_2550m::FORM_2550M_ATCS.len())
        .contains(&n)
        .then(|| n - 1)
}

fn fill_2550m_amount(draft: &mut Value, row: usize, value: &Value) -> Result<(), String> {
    let text = match value {
        Value::Number(n) => n.to_string(),
        other => other.as_str().unwrap_or_default().to_string(),
    };
    let amount = if text.trim().is_empty() {
        0.0
    } else {
        agent::parse_amount(&text).ok_or_else(|| format!("`{text}` is not an amount"))?
    };
    let mut typed: form_2550m::Form2550MDraft =
        serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?;
    let entry = typed
        .sales_schedule
        .get_mut(row)
        .ok_or_else(|| "Choose this row's ATC first.".to_string())?;
    entry.amount = amount;
    typed.recompute();
    *draft = serde_json::to_value(typed).map_err(|e| e.to_string())?;
    Ok(())
}

/// Choose an ATC for a Schedule 1 row: a new row ticks it in the popup, an
/// existing row changes its code and keeps its amount, blank unticks it.
/// Rows re-sort into popup order and an ATC appears once, as on the page.
fn fill_2550m_atc(draft: &mut Value, row: usize, value: &Value) -> Result<(), String> {
    let code = value.as_str().unwrap_or_default().trim();
    let mut typed: form_2550m::Form2550MDraft =
        serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?;
    let current = typed.sales_schedule.get(row).map(|r| r.atc_code.clone());
    if code.is_empty() {
        if let Some(current) = current {
            typed.remove_sales_atc(&current);
        }
    } else if current.as_deref() != Some(code) {
        if form_2550m::form_2550m_atc_index(code).is_none() {
            return Err(format!("{code} is not a 2550M ATC."));
        }
        if typed.sales_schedule.iter().any(|r| r.atc_code == code) {
            return Err(format!("{code} is already in Schedule 1."));
        }
        match typed.sales_schedule.get_mut(row) {
            Some(existing) => {
                existing.atc_code = code.to_string();
                typed.recompute();
            }
            None => typed.add_sales_atc(code)?,
        }
    }
    *draft = serde_json::to_value(typed).map_err(|e| e.to_string())?;
    Ok(())
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
        "2550M" if atc_2550m_row(kind, key).is_some() => Some(
            form_2550m::FORM_2550M_ATCS
                .iter()
                .map(|(c, n)| code_option(c, n))
                .collect(),
        ),
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
