//! Report-only audit harness for third-decimal money input behavior.
//!
//! This does not pin a desired rounding rule. It writes `1000.005`,
//! `1.005` and `2.675` through the same `screen_form_fill` path the page
//! uses and records the refusal or the exact draft/upload representation
//! for one representative ordinary item, penalty item and schedule-row
//! amount box on every form. The report generator compares those records
//! with the official behavior cataloged under `rules/forms/`.

use super::*;

const PROBES: [&str; 3] = ["1000.005", "1.005", "2.675"];

fn key_roles(code: &str) -> BTreeMap<String, &'static str> {
    let mut roles = BTreeMap::new();
    let Some(form) = schema_entry(code) else {
        return roles;
    };
    for item in form["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array().into_iter().flatten())
    {
        for field in item["f"].as_array().into_iter().flatten() {
            if let Some(key) = field["k"].as_str() {
                roles.entry(key.to_string()).or_insert("ordinary-item");
            }
        }
        for row in item["keys"].as_array().into_iter().flatten() {
            for key in row
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                roles.insert(key.to_string(), "schedule-row");
            }
        }
    }
    roles
}

fn amount_candidates(kind: AgentFormKind) -> Vec<(String, String, String)> {
    let roles = key_roles(kind.form_code());
    let def = kind.definition();
    let mut out: Vec<_> = def
        .bindings
        .iter()
        .filter_map(|(key, bound)| {
            let Binding::Amount { path, .. } = &bound.binding else {
                return None;
            };
            let entry = def.box_def(key)?;
            if entry.computed || entry.required == "computed" || !entry.is_user_box() {
                return None;
            }
            // Rates, volumes, counts and share quantities are decimal inputs,
            // but they are not money boxes. The audit is about peso amounts.
            let name = format!("{key} {path}").to_ascii_lowercase();
            if ["rate", "percent", "volume", "share", "quantity", "qty"]
                .iter()
                .any(|word| name.contains(word))
            {
                return None;
            }
            let numeric_path = path
                .split('/')
                .any(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));
            let category = if roles.get(key) == Some(&"schedule-row") || numeric_path {
                "schedule-row"
            } else if path.contains("penalty") {
                "penalty-item"
            } else {
                "ordinary-item"
            };
            Some((category.to_string(), key.clone(), path.clone()))
        })
        .collect();
    let first_index = |path: &str| {
        path.split('/')
            .find_map(|part| part.parse::<usize>().ok())
            .unwrap_or(usize::MAX)
    };
    out.sort_by(|(_, key_a, path_a), (_, key_b, path_b)| {
        (first_index(path_a), key_a).cmp(&(first_index(path_b), key_b))
    });
    out
}

fn amountish_refusal(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    [
        "amount",
        "decimal",
        "character",
        "digit",
        "non-negative",
        "smaller",
    ]
    .iter()
    .any(|word| lower.contains(word))
}

fn collect_subcent(value: &Value, path: String, out: &mut BTreeMap<String, f64>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                collect_subcent(child, format!("{path}/{key}"), out);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_subcent(child, format!("{path}/{index}"), out);
            }
        }
        _ => {
            if let Some(number) = value.as_f64() {
                let nearest_cent = (number * 100.0).round() / 100.0;
                if number.is_finite() && (number - nearest_cent).abs() > 0.000000001 {
                    out.insert(path, number);
                }
            }
        }
    }
}

fn changed_subcent_values(before: &Value, after: &Value, input_path: &str) -> Vec<Value> {
    let mut before_values = BTreeMap::new();
    let mut after_values = BTreeMap::new();
    collect_subcent(before, String::new(), &mut before_values);
    collect_subcent(after, String::new(), &mut after_values);
    after_values
        .into_iter()
        .filter(|(path, number)| {
            path != input_path && before_values.get(path).copied() != Some(*number)
        })
        .map(|(path, number)| json!({"path": path, "value": number}))
        .collect()
}

fn probe_once(
    kind: AgentFormKind,
    opened: &Value,
    key: &str,
    path: &str,
    category: &str,
    probe: &str,
) -> Value {
    let code = kind.form_code();
    let filled = fill_unlocked(code, &opened["draft"], json!({key: probe}), &[key]);
    let rejected = filled["rejected"][key].as_str().map(str::to_string);
    let stored = filled["draft"]
        .pointer(path)
        .cloned()
        .unwrap_or(Value::Null);
    let (upload, upload_error) = match agent::field_map(kind, &filled["draft"]) {
        Ok(map) => (map.get(key).cloned(), Value::Null),
        Err(error) => (None, json!(error)),
    };
    json!({
        "form": code,
        "package": kind.package(),
        "category": category,
        "box": key,
        "path": path,
        "probe": probe,
        "outcome": if rejected.is_some() { "refused" } else { "accepted" },
        "refusal": rejected,
        "stored": stored,
        "changed_subcent_elsewhere": changed_subcent_values(&opened["draft"], &filled["draft"], path),
        "displayed": filled["values"][key].clone(),
        "upload": upload,
        "upload_error": upload_error,
    })
}

fn audit_category(
    kind: AgentFormKind,
    opened: &Value,
    category: &str,
    candidates: &[(String, String, String)],
    records: &mut Vec<Value>,
) {
    let mut first_refusal: Option<(String, String, Value)> = None;
    let mut chosen: Option<(String, String)> = None;
    let mut attempted = 0;
    for (_, key, path) in candidates.iter().filter(|(c, _, _)| c == category).take(30) {
        attempted += 1;
        let first = probe_once(kind, opened, key, path, category, PROBES[0]);
        let refusal = first["refusal"].as_str().map(str::to_string);
        if refusal.is_none() || refusal.as_deref().is_some_and(amountish_refusal) {
            records.push(first);
            chosen = Some((key.clone(), path.clone()));
            break;
        }
        if first_refusal.is_none() {
            first_refusal = Some((key.clone(), path.clone(), first));
        }
    }
    let Some((key, path)) = chosen else {
        if let Some((key, path, mut first)) = first_refusal {
            first["selection_note"] =
                json!("no candidate accepted the first probe; the first refusal is recorded");
            records.push(first);
            for probe in &PROBES[1..] {
                records.push(probe_once(kind, opened, &key, &path, category, probe));
            }
        }
        return;
    };
    for probe in &PROBES[1..] {
        records.push(probe_once(kind, opened, &key, &path, category, probe));
    }
    if attempted > 1 {
        if let Some(last) = records.last_mut() {
            last["selection_note"] = json!(format!("selected after {attempted} candidates"));
        }
    }
}

/// Run the report-only probe matrix and write it as JSON. The assertions
/// are only harness-level sanity checks: every money-bearing form must
/// produce records, and every record must be one of the three requested
/// probes. The comparison itself is in the generated report, not here.
#[test]
fn money_third_decimal_audit_matrix() {
    setup();
    let mut records = Vec::new();
    let mut forms_with_amounts = 0;
    for kind in AgentFormKind::all() {
        let candidates = amount_candidates(kind);
        if candidates.is_empty() {
            continue;
        }
        forms_with_amounts += 1;
        let opened = open(kind.form_code(), page_period_of(kind.form_code()));
        for category in ["ordinary-item", "penalty-item", "schedule-row"] {
            audit_category(kind, &opened, category, &candidates, &mut records);
        }
    }
    assert!(forms_with_amounts >= 35, "{forms_with_amounts}");
    assert!(!records.is_empty());
    assert!(
        records
            .iter()
            .all(|record| PROBES.contains(&record["probe"].as_str().unwrap_or_default()))
    );
    let out = std::env::var("MONEY_AUDIT_OUT")
        .unwrap_or_else(|_| "target/money-third-decimal-audit.json".to_string());
    if let Some(parent) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(parent).expect("audit output directory");
    }
    let payload = json!({
        "main_commit": option_env!("GIT_COMMIT_HASH").unwrap_or("unknown"),
        "probes": PROBES,
        "forms_with_amount_bindings": forms_with_amounts,
        "record_count": records.len(),
        "records": records,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&payload).unwrap())
        .unwrap_or_else(|error| panic!("write {out}: {error}"));
    eprintln!("money audit wrote {out}: {} records", records.len());
}
