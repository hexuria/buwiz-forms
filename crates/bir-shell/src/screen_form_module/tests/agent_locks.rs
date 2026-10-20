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
//!
//! PR 15 round 2: the page and the desktop assistant build the profile
//! context `agent::profile_blank` reads (`agent::FromProfile`) with two
//! different `values` maps — the page `values_json`, the desktop
//! `agent::field_map` alone. The `..._lock_alike_...` tests pin that both
//! give one lock map and one `fill` verdict on every form.

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

/// The profile context the page's open builds for `draft` (`opened_json`:
/// `values_json` of the profile's own recomputed draft) and the one the
/// desktop assistant builds for the same draft
/// (crates/bir-desktop/src/agent/host_generic.rs `profile_context`:
/// `agent::field_map`, each value a JSON string).
fn page_and_desktop_contexts(
    kind: AgentFormKind,
    draft: &Value,
) -> (agent::FromProfile, agent::FromProfile) {
    let code = kind.form_code();
    let page = agent::FromProfile {
        values: values_json(kind, draft).unwrap_or_else(|error| panic!("{code}: {error}")),
        draft: draft.clone(),
    };
    let field_map = agent::field_map(kind, draft).unwrap_or_else(|error| panic!("{code}: {error}"));
    let desktop = agent::FromProfile {
        values: serde_json::to_value(field_map).unwrap(),
        draft: draft.clone(),
    };
    (page, desktop)
}

/// The `values` keys `agent::profile_blank` can read for `key`: the key
/// itself, the official box an occurrence alias repeats, and every radio
/// of the item either belongs to.
fn blank_reads(kind: AgentFormKind, key: &str) -> Vec<String> {
    let def = kind.definition();
    let mut keys = vec![key.to_string()];
    keys.extend(
        agent::occurrence_aliases(kind)
            .into_iter()
            .find(|(alias, _)| alias.as_str() == key)
            .map(|(_, official)| official.to_string()),
    );
    for read in keys.clone() {
        let group = match def.box_def(&read) {
            Some(entry) if entry.radio_group => Some(entry),
            Some(entry) => entry.group.as_deref().and_then(|group| def.box_def(group)),
            None => None,
        };
        keys.extend(
            group
                .into_iter()
                .flat_map(|group| group.members.iter().cloned()),
        );
    }
    keys.sort();
    keys.dedup();
    keys
}

/// `(form, key, page value, desktop value)` for every key `key`'s blank
/// check reads where the two contexts' `values` differ; the key itself when
/// none does.
fn divergence(
    kind: AgentFormKind,
    key: &str,
    page: &agent::FromProfile,
    desktop: &agent::FromProfile,
) -> String {
    let shown = |context: &agent::FromProfile, read: &str| {
        context
            .values
            .get(read)
            .map_or_else(|| "<absent>".to_string(), Value::to_string)
    };
    let mut reads: Vec<String> = blank_reads(kind, key)
        .into_iter()
        .filter(|read| page.values.get(read) != desktop.values.get(read))
        .collect();
    if reads.is_empty() {
        reads.push(key.to_string());
    }
    reads
        .iter()
        .map(|read| {
            format!(
                "({}, {read}, {}, {})",
                kind.form_code(),
                shown(page, read),
                shown(desktop, read)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What one profile's pass compared, so neither test passes on nothing.
#[derive(Debug, Default)]
struct Compared {
    forms: usize,
    /// Profile boxes and repeated occurrences whose `fill` verdicts were
    /// compared.
    keys: usize,
    /// Profile boxes the profile left blank (open) and filled (locked), as
    /// the page's context reads them.
    blank: usize,
    filled: usize,
    /// Keys a compared box's blank check can read (`blank_reads`) whose
    /// `values` entry the two contexts hold differently: the comparison
    /// runs where the two maps really disagree (a duplicate occurrence, a
    /// row module's display text), not only where they match.
    differing_reads: usize,
}

/// Every lock-map entry and `fill` verdict the page's and the desktop's
/// profile contexts disagree on, on every form, for `profile`: both built
/// from one profile draft the way the page's open builds it (the
/// profile's own fresh draft for the period, recomputed).
fn profile_context_divergences(profile: &TaxpayerProfile) -> (Vec<String>, Compared) {
    let mut failures = Vec::new();
    let mut compared = Compared::default();
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let def = kind.definition();
        let mut draft = with_database(|db| {
            let period = page_period(code, page_period_of(code)).map_err(HostError::new)?;
            load_draft(db, kind, profile, YEAR as u16, period, true, None).map_err(HostError::new)
        })
        .unwrap_or_else(|error| panic!("{code}: {error:?}"));
        agent::compute(kind, &mut draft).unwrap_or_else(|error| panic!("{code}: {error}"));
        let (page, desktop) = page_and_desktop_contexts(kind, &draft);
        compared.forms += 1;
        // 1. One lock map.
        let page_locks = agent::locks(kind, &page);
        let desktop_locks = agent::locks(kind, &desktop);
        for key in page_locks
            .keys()
            .chain(desktop_locks.keys())
            .collect::<BTreeSet<_>>()
        {
            if page_locks.get(key) != desktop_locks.get(key) {
                failures.push(format!(
                    "locks {}: page {:?}, desktop {:?}",
                    divergence(kind, key, &page, &desktop),
                    page_locks.get(key),
                    desktop_locks.get(key)
                ));
            }
        }
        // 2. One `fill` verdict on every profile box and every repeated
        // occurrence: the whole result — its `is_ok()` and its reason — so
        // a lock refusal cannot hide behind a write the value "X" fails
        // anyway (an amount, a date).
        let profile_boxes: BTreeSet<String> = def
            .boxes
            .iter()
            .filter(|entry| agent::is_profile_box(def, entry))
            .map(|entry| entry.key.clone())
            .collect();
        for key in &profile_boxes {
            if agent::profile_blank(def, key, &page) {
                compared.blank += 1;
            } else {
                compared.filled += 1;
            }
        }
        let mut keys = profile_boxes;
        keys.extend(
            agent::occurrence_aliases(kind)
                .into_iter()
                .map(|(key, _)| key.clone()),
        );
        keys.extend(agent::profile_keys(kind, &page));
        keys.extend(agent::profile_keys(kind, &desktop));
        compared.differing_reads += keys
            .iter()
            .flat_map(|key| blank_reads(kind, key))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|read| page.values.get(read) != desktop.values.get(read))
            .count();
        for key in &keys {
            compared.keys += 1;
            let by_page = agent::fill(kind, &mut draft.clone(), key, &json!("X"), Some(&page));
            let by_desktop =
                agent::fill(kind, &mut draft.clone(), key, &json!("X"), Some(&desktop));
            if by_page != by_desktop {
                failures.push(format!(
                    "fill {}: page {by_page:?}, desktop {by_desktop:?}",
                    divergence(kind, key, &page, &desktop)
                ));
            }
        }
    }
    (failures, compared)
}

/// The test profile as the page's open reads it.
fn stored_profile() -> TaxpayerProfile {
    with_database(|db| {
        db.get_profile_by_tin(TIN)
            .map_err(|error| HostError::new(error.to_string()))?
            .ok_or_else(|| HostError::new("no profile".to_string()))
    })
    .unwrap()
}

/// PR 15 round 2 (review MEDIUM 1): `agent::profile_blank` reads the
/// profile context's `values`, which the page builds with `values_json`
/// (the field map, each duplicate occurrence, the row modules' display
/// additions) and the desktop assistant with `agent::field_map` alone. For
/// the full test profile, on every form, both contexts built from one
/// profile draft give the same lock map and the same `fill` verdict on
/// every profile box: a box the page shows locked is never open to the
/// assistant, nor the other way round.
#[test]
fn the_page_and_the_desktop_lock_alike_for_a_full_profile() {
    setup();
    let (failures, compared) = profile_context_divergences(&stored_profile());
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(compared.forms, AgentFormKind::all().len());
    // 328 of the 330 profile boxes are filled, and the repeated
    // occurrences are compared beside them; 20 of the keys read differ
    // between the two maps: duplicate occurrences on 2551Q, 1702Q, 1707A
    // and 2200A, display-only text on 1601EQ, 1700, 1702EX, 1800 and 2200S.
    assert!(
        compared.filled > 300
            && compared.keys > compared.blank + compared.filled
            && compared.differing_reads > 0,
        "{compared:?}"
    );
}

/// The same for the test profile with the facts a filer may leave out left
/// blank — telephone, email, ZIP code, the registered address (every
/// address line), RDO and line of business (`install_sparse_profile`'s
/// set): the boxes it leaves blank open on both sides alike.
#[test]
fn the_page_and_the_desktop_lock_alike_for_a_sparse_profile() {
    setup();
    let mut sparse = stored_profile();
    sparse.phone.clear();
    sparse.email.clear();
    sparse.zip_code.clear();
    sparse.registered_address.clear();
    sparse.rdo_code.clear();
    sparse.line_of_business.clear();
    with_database(|db| {
        db.save_profile(sparse)
            .map(|_| ())
            .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap();
    let sparse = stored_profile();
    assert!(sparse.phone.is_empty() && sparse.registered_address.is_empty());
    let (failures, compared) = profile_context_divergences(&sparse);
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(compared.forms, AgentFormKind::all().len());
    // 248 profile boxes are left blank and 82 (the names) still filled.
    assert!(
        compared.blank > 200
            && compared.filled > 50
            && compared.keys > compared.blank + compared.filled
            && compared.differing_reads > 0,
        "{compared:?}"
    );
}
