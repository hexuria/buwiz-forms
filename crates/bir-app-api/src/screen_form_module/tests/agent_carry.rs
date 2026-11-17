//! The page-side inputs of the carry rule (`bir_core::forms::agent::carry`):
//! which boxes this page's schema marks computed (`c`), which it draws as
//! money or as a date once [`page_type`] has typed them by their binding,
//! the boxes a Form library sample sets the period with
//! ([`sample_period_parts`]), and the unbound boxes the page's own writer
//! fills (`catalogs::fill`: 2551Q's `drpATC1-6`) that
//! `catalogs::host_writes` does not list. Core cannot read the page schema, so the verdicts are pinned in
//! `crates/bir-core/data/agent-carry.json`, the way `agent-locks.json` pins
//! the lock inputs. Regenerate after a schema, override or binding change
//! with:
//!
//! `UPDATE_AGENT_CARRY=1 cargo test --manifest-path crates/bir-shell/Cargo.toml agent_carry_table`

use super::*;
use std::path::PathBuf;

fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bir-core/data/agent-carry.json")
}

/// Each key the page draws for `code` with the type the page gives it: a
/// field's own `t`, a table cell its column's `t`. The first occurrence of
/// a key wins.
fn page_types(code: &str) -> BTreeMap<String, String> {
    let mut types = BTreeMap::new();
    let Some(form) = schema_entry(code) else {
        return types;
    };
    let items = form["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array().into_iter().flatten());
    for item in items {
        for field in item["f"].as_array().into_iter().flatten() {
            if let (Some(key), Some(t)) = (field["k"].as_str(), field["t"].as_str()) {
                types
                    .entry(key.to_string())
                    .or_insert_with(|| t.to_string());
            }
        }
        for row in item["keys"].as_array().into_iter().flatten() {
            for (c, key) in row.as_array().into_iter().flatten().enumerate() {
                if let (Some(key), Some(t)) = (key.as_str(), item["cols"][c]["t"].as_str()) {
                    types
                        .entry(key.to_string())
                        .or_insert_with(|| t.to_string());
                }
            }
        }
    }
    types
}

/// The pin's content: for every package, the boxes the page schema marks
/// computed that the package does not, the `whole`-bound and unbound boxes
/// the page draws as money (an `amount` binding is an amount in core
/// already), and the boxes the page draws as a date without a `date`
/// binding, the sample's period parts and the unbound boxes the page's
/// writer fills. Only what core cannot see itself.
fn table() -> String {
    setup();
    let profile = with_database(|db| {
        db.get_profile_by_tin(TIN)
            .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap()
    .expect("the test profile");
    let mut packages = Map::new();
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let def = kind.definition();
        let period = page_period(code, 1).unwrap();
        let fresh = with_database(|db| {
            load_draft(db, kind, &profile, 2026, period, true, None).map_err(HostError::new)
        })
        .unwrap();
        // An unbound box the page writes: its writer takes a blank (the
        // agent's own refusal of an unbound box says "cannot be filled").
        let mut page_writes = Vec::new();
        for entry in def.boxes.iter().filter(|entry| entry.is_question()) {
            let key = entry.key.as_str();
            if def.bindings.contains_key(key)
                || catalogs::host_writes(kind, key)
                || key.contains("#occurrence-")
            {
                continue;
            }
            let mut draft = fresh.clone();
            let written =
                catalogs::fill(kind, &mut draft, key, &json!(""), catalogs::Lift::Nothing);
            if !written
                .is_err_and(|why| why.contains("cannot be filled") || why.contains("repeats"))
            {
                page_writes.push(key);
            }
        }
        let types = page_types(code);
        let binding = |key: &str| def.bindings.get(key).map(|bound| &bound.binding);
        let typed = |key: &str, want: &str| types.get(key).is_some_and(|t| t == want);
        let mut computed = Vec::new();
        let mut money = Vec::new();
        let mut dates = Vec::new();
        let parts: BTreeSet<String> = sample_period_parts(kind)
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        let mut period = Vec::new();
        for entry in &def.boxes {
            let key = entry.key.as_str();
            if !entry.computed && entry.required != "computed" && schema_computed(code, key) {
                computed.push(key);
            }
            if typed(key, "money") && matches!(binding(key), None | Some(Binding::Whole { .. })) {
                money.push(key);
            }
            let date_bound = matches!(binding(key), Some(Binding::Text { date: Some(_), .. }));
            if typed(key, "date") && !date_bound {
                dates.push(key);
            }
            if parts.contains(key) {
                period.push(key);
            }
        }
        if [&computed, &money, &dates, &period, &page_writes]
            .iter()
            .all(|list| list.is_empty())
        {
            continue;
        }
        packages.insert(
            kind.package().to_string(),
            json!({
                "computed": computed,
                "money": money,
                "dates": dates,
                "period": period,
                "page_writes": page_writes,
            }),
        );
    }
    let mut table = Value::Object(packages);
    table.sort_all_objects();
    let mut text = serde_json::to_string_pretty(&table).unwrap();
    text.push('\n');
    text
}

/// The pinned table is exactly what the page's schema and types say.
#[test]
fn agent_carry_table_matches_the_page_schema() {
    let text = table();
    let path = data_path();
    if std::env::var("UPDATE_AGENT_CARRY").is_ok() {
        std::fs::write(&path, &text).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    }
    let pinned = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    assert_eq!(pinned, text, "data/agent-carry.json is stale");
}
