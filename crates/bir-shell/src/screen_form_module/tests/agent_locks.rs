//! PR 15 (decision P4-19): the effective-lock inputs `bir-core`'s shared
//! computation reads — which keys this page's host writes through its own
//! editors (`catalogs::host_writes`, bound or not) and which on-screen
//! keys repeat an official control (`#occurrence-`) without a binding of
//! their own — are pinned for `agent::gate`/`agent::locks` in
//! `crates/bir-core/data/agent-locks.json`, the way `data/agent-forms`
//! pins the bindings. Regenerate after a classifier, schema or binding
//! change with:
//!
//! `UPDATE_AGENT_LOCKS=1 cargo test --manifest-path crates/bir-shell/Cargo.toml agent_locks_table`

use super::*;
use std::path::PathBuf;

fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bir-core/data/agent-locks.json")
}

/// The pin's content: for every package, the host-written keys and the
/// occurrence aliases, over the pinned definition's box and binding keys —
/// exactly the universe `gate` reads them against core-side.
fn table() -> String {
    let mut packages = Map::new();
    for kind in AgentFormKind::all() {
        let def = kind.definition();
        let universe: BTreeSet<&str> = def
            .boxes
            .iter()
            .map(|entry| entry.key.as_str())
            .chain(def.bindings.keys().map(String::as_str))
            .collect();
        let host_written: Vec<String> = universe
            .iter()
            .filter(|key| catalogs::host_writes(kind, key))
            .map(|key| key.to_string())
            .collect();
        let occurrence_aliases: Map<String, Value> = occurrence_keys(kind)
            .into_iter()
            .filter(|(key, _)| !def.bindings.contains_key(*key))
            .map(|(key, official)| (key.clone(), Value::from(official)))
            .collect();
        if host_written.is_empty() && occurrence_aliases.is_empty() {
            continue;
        }
        packages.insert(
            kind.package().to_string(),
            json!({
                "host_written": host_written,
                "occurrence_aliases": occurrence_aliases,
            }),
        );
    }
    let mut table = Value::Object(packages);
    // Workspace dependencies can enable serde_json's preserve_order feature.
    table.sort_all_objects();
    let mut text = serde_json::to_string_pretty(&table).unwrap();
    text.push('\n');
    text
}

/// The pinned table is exactly what the page's own classifiers report:
/// core can never drift from the page, and a classifier change that forgets
/// the pin fails here until it is regenerated.
#[test]
fn agent_locks_table_matches_the_page_classifiers() {
    let text = table();
    let path = data_path();
    if std::env::var("UPDATE_AGENT_LOCKS").is_ok() {
        std::fs::write(&path, &text).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    }
    let pinned = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    assert_eq!(pinned, text, "data/agent-locks.json is stale");
}

/// The shared lock map for a sample draft, on a form that carries every
/// family at once (2551Q: identity boxes, profile boxes, an election, an
/// occurrence alias, computed boxes): `agent::locks` is what the page
/// shows, byte for byte.
#[test]
fn the_shared_lock_map_is_identical_for_a_sample_draft() {
    setup_taxpayer_filing(&["2551Q"]);
    let kind = kind_of("2551Q").unwrap();
    let mut from_profile_draft = with_database(|db| {
        let profile = db
            .get_profile_by_tin(TIN)
            .map_err(|error| HostError::new(error.to_string()))?
            .ok_or_else(|| HostError::new("no profile".to_string()))?;
        load_draft(db, kind, &profile, 2026, 1, true, None).map_err(HostError::new)
    })
    .unwrap();
    agent::compute(kind, &mut from_profile_draft).unwrap();
    let from_profile = agent::FromProfile {
        values: values_json(kind, &from_profile_draft).unwrap(),
        draft: from_profile_draft,
    };
    let expected: Map<String, Value> = serde_json::from_value(json!({
        // 2551Q's email serializes under the unprefixed `txtEmail`, which
        // has no binding of its own (Item 11 is `txtEmail#occurrence-1`).
        "txtEmail#occurrence-2":
            "repeats the official field `txtEmail`, which this page cannot write",
    }))
    .unwrap();
    let locks = agent::locks(kind, &from_profile);
    // The families the page shows: profile-filled boxes, identity boxes,
    // computed boxes, the alias and the unmapped, with their page reasons —
    // never elections, hidden controls, or the host's own keys.
    for (key, reason) in &locks {
        let text = reason.as_str().unwrap_or("");
        assert!(
            text.contains("repeats")
                || text.contains("comes from the taxpayer profile")
                || text.contains("picks which return this is")
                || text.contains("computed by BIR's own formulas")
                || text.contains("cannot be filled here"),
            "{key}: {text}"
        );
    }
    for (key, reason) in &expected {
        assert_eq!(&locks[key], reason, "{key}");
    }
    // 2551Q's election (Item 13) is open on the page and refused to fill:
    // it is never in the lock map.
    assert!(
        !locks.contains_key("frm2551Qv2018:taxRate"),
        "elections stay out of the page's map"
    );
    // What the page computed for the same draft, relocks aside: every
    // shared-map entry appears in the open's lock map with the same reason
    // — a penalty group's own lock may overwrite a computed reason (the
    // page applies its draft-dependent locks on top, by design).
    let penalty_keys: BTreeSet<String> = penalties::computed_groups(kind)
        .into_iter()
        .flat_map(|group| group.keys)
        .collect();
    let opened = open("2551Q", 1);
    for (key, reason) in &locks {
        if penalty_keys.contains(key) {
            continue;
        }
        assert_eq!(&opened["locks"][key], reason, "{key} on the open page");
    }
}
