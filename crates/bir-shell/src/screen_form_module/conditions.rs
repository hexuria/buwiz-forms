//! Boxes another answer turns off (rule 6): the page fades them and says
//! what turns them on; they come alive when the condition holds. The checks
//! themselves are not touched.
//!
//! Which boxes another answer can turn off is the official corpus's
//! `enabled_when` (`rules/forms/<package>/fields.json`, read only), copied
//! for the boxes the page shows into `app/forms/conditions.json` by the
//! ignored test `write_conditions`; `conditions_match_the_rules_corpus`
//! keeps the copy in step. The corpus text is for developers (it records
//! official defects too), so the page never shows it: each condition has a
//! short reviewed tag in `app/forms/condition-labels.json` ("Needs item 13 =
//! Yes"), which says what the form's own computation needs where that is
//! more than the corpus says (0619F's items also need taxes withheld).
//!
//! Whether a box is off right now is the form's own computation: the host
//! types a value into it on a copy of the draft, recomputes, and the box is
//! off when the computation clears it again (1601C clears Item 28 unless the
//! return is amended). A box that holds a value, or one that takes none of
//! the probe values, is never reported off.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use bir_core::forms::agent::{self, AgentFormKind, Binding, DatePart};
use serde_json::{Map, Value, json};

/// `app/forms/conditions.json`: form code -> key -> the corpus condition.
fn conditions() -> &'static Value {
    static CONDITIONS: OnceLock<Value> = OnceLock::new();
    CONDITIONS.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../app/forms/conditions.json"))
            .expect("app/forms/conditions.json is a JSON object")
    })
}

/// `app/forms/condition-labels.json`: form code -> corpus condition -> the
/// tag the page shows.
fn labels() -> &'static Value {
    static LABELS: OnceLock<Value> = OnceLock::new();
    LABELS.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../app/forms/condition-labels.json"))
            .expect("app/forms/condition-labels.json is a JSON object")
    })
}

/// A value shown as nothing: blank, all zeros (`0.00`, `00`) or unticked.
fn is_blank(shown: &str) -> bool {
    let shown = shown.trim();
    shown == "false" || shown.chars().all(|c| matches!(c, '0' | '.' | ',' | '-'))
}

/// The binding a box writes through: its own, or its radio group's.
fn binding_of(kind: AgentFormKind, key: &str) -> Option<&'static Binding> {
    let def = kind.definition();
    let entry = def.box_def(key)?;
    def.bindings
        .get(key)
        .or_else(|| {
            entry
                .group
                .as_deref()
                .and_then(|group| def.bindings.get(group))
        })
        .map(|bound| &bound.binding)
}

/// The values tried on a box, in order, to see whether the form keeps one:
/// the first one the box takes decides. The tests use the same list.
pub(super) fn probes(kind: AgentFormKind, key: &str, shown: &str) -> Vec<Value> {
    let grouped = kind
        .definition()
        .box_def(key)
        .is_some_and(|entry| entry.group.is_some());
    match binding_of(kind, key) {
        None => Vec::new(),
        Some(Binding::Amount { .. } | Binding::Whole { .. }) => vec![json!("1"), json!("1000")],
        Some(Binding::Text {
            part: Some(part), ..
        }) => vec![json!(match part {
            DatePart::Month => "01",
            DatePart::Day => "15",
            DatePart::Year => "2020",
        })],
        Some(Binding::Text { date: Some(_), .. }) => vec![json!("01/15/2020")],
        Some(Binding::Text { .. }) => vec![json!("A"), json!("01"), json!("2020")],
        // A radio of a group is ticked by its own key.
        Some(Binding::Choice { .. }) if grouped => vec![json!("true")],
        Some(Binding::Choice { options }) => options
            .iter()
            .map(|option| option.value.as_str())
            .filter(|value| *value != shown && !is_blank(value))
            .take(2)
            .map(|value| json!(value))
            .collect(),
    }
}

/// The tag for a box another answer turns off.
fn label(code: &str, condition: &Value) -> Value {
    let text = condition.as_str().unwrap_or_default();
    labels()[code][text].as_str().map_or_else(
        || json!("Needs another answer on this form"),
        |label| json!(label),
    )
}

/// Fill `edits` on a copy of `draft` the way the page does, recompute, and
/// give each key's shown value, or `None` for a key the copy refused.
fn trial(
    kind: AgentFormKind,
    draft: &Value,
    edits: &[(String, Value)],
) -> BTreeMap<String, Option<String>> {
    let mut copy = draft.clone();
    let mut taken = BTreeMap::new();
    for (key, value) in edits {
        let mut next = copy.clone();
        let ok = super::catalogs::fill(kind, &mut next, key, value, super::catalogs::Lift::Nothing)
            .is_ok();
        if ok {
            copy = next;
        }
        taken.insert(key.clone(), ok);
    }
    let shown = agent::compute(kind, &mut copy)
        .ok()
        .and_then(|()| agent::field_map(kind, &copy).ok())
        .unwrap_or_default();
    taken
        .into_iter()
        .map(|(key, ok)| {
            let value = ok.then(|| shown.get(&key).cloned().unwrap_or_default());
            (key, value)
        })
        .collect()
}

/// Whether one box is off: the first probe it takes is cleared by the form.
fn off_alone(kind: AgentFormKind, draft: &Value, key: &str, shown: &str) -> bool {
    for probe in probes(kind, key, shown) {
        if let Some(Some(after)) = trial(kind, draft, &[(key.to_string(), probe)]).remove(key) {
            return is_blank(&after);
        }
    }
    false
}

/// The boxes of `draft` that another answer turns off right now, each with
/// its tag. `values` is what the page shows for the draft.
///
/// Typed boxes (amounts, numbers, text) are tried together in one trial
/// first: a box the form keeps there is on. Only the ones it clears, and
/// the choices (a choice may be the answer another box waits for), are
/// tried one at a time, which decides.
pub(super) fn off_boxes(kind: AgentFormKind, draft: &Value, values: &Value) -> Map<String, Value> {
    let mut off = Map::new();
    let code = kind.form_code();
    let Some(list) = conditions()[code].as_object() else {
        return off;
    };
    let blank: Vec<(&String, &Value, &str)> = list
        .iter()
        .map(|(key, condition)| (key, condition, values[key].as_str().unwrap_or_default()))
        .filter(|(_, _, shown)| is_blank(shown))
        .collect();
    let typed: Vec<(String, Value)> = blank
        .iter()
        .filter(|(key, _, _)| !matches!(binding_of(kind, key), Some(Binding::Choice { .. })))
        .filter_map(|(key, _, shown)| {
            probes(kind, key, shown)
                .into_iter()
                .next()
                .map(|probe| (key.to_string(), probe))
        })
        .collect();
    let together = trial(kind, draft, &typed);
    for (key, condition, shown) in blank {
        let kept_together = together
            .get(key)
            .is_some_and(|after| after.as_deref().is_some_and(|after| !is_blank(after)));
        if !kept_together && off_alone(kind, draft, key, shown) {
            off.insert(key.clone(), label(code, condition));
        }
    }
    off
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// Corpus conditions that describe the official runtime's own states (a
    /// static-disabled box, an ATC modal's slots, the official Edit path,
    /// internal page metadata), not an answer the filer gives: the page
    /// leaves those boxes as they are.
    const NOT_AN_ANSWER: [&str; 13] = [
        "disabled in the static form",
        "statically disabled",
        "never enabled",
        "internal",
        "rendered in the",
        "runtime",
        "modal",
        "ui state",
        "while editing",
        "after successful validate",
        "identity-owned",
        "official edit path",
        "official dependency/state transition",
    ];

    /// Whether a corpus condition is one the filer controls.
    fn is_an_answer(condition: &str) -> bool {
        let lower = condition.to_ascii_lowercase();
        !NOT_AN_ANSWER.iter().any(|phrase| lower.contains(phrase))
    }

    /// The boxes the page draws: fields and table cells that are not
    /// hidden, never a row family's template key (`txtAmt{N>=0}`).
    fn drawn(code: &str) -> std::collections::BTreeSet<String> {
        let form = super::super::schema_entry(code).unwrap();
        let mut keys = std::collections::BTreeSet::new();
        for item in form["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|part| part["items"].as_array().into_iter().flatten())
        {
            for field in item["f"].as_array().into_iter().flatten() {
                if field["t"] != "hidden" {
                    keys.extend(field["k"].as_str().map(str::to_string));
                }
            }
            for row in item["keys"].as_array().into_iter().flatten() {
                for (c, key) in row.as_array().into_iter().flatten().enumerate() {
                    if item["cols"][c]["t"] != "hidden" {
                        keys.extend(key.as_str().map(str::to_string));
                    }
                }
            }
        }
        keys.retain(|key| !key.contains('{'));
        keys
    }

    /// What `app/forms/conditions.json` should hold, read from the corpus:
    /// every box the page draws whose `enabled_when` names an answer,
    /// computed boxes left out.
    pub(crate) fn derived() -> Value {
        let root = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
        let mut out = Map::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            let path = format!("{root}/rules/forms/{}/fields.json", kind.package());
            let corpus: Value = std::fs::read_to_string(&path)
                .ok()
                .and_then(|raw| serde_json::from_str(&raw).ok())
                .unwrap_or_else(|| panic!("{path} reads"));
            let fields = corpus.get("fields").unwrap_or(&corpus);
            let shown = drawn(code);
            let mut keys = Map::new();
            for field in fields.as_array().into_iter().flatten() {
                let (Some(key), Some(condition)) =
                    (field["field_key"].as_str(), field["enabled_when"].as_str())
                else {
                    continue;
                };
                let computed = def.box_def(key).is_some_and(|entry| entry.computed);
                if shown.contains(key) && !computed && is_an_answer(condition) {
                    keys.insert(key.to_string(), json!(condition));
                }
            }
            if !keys.is_empty() {
                out.insert(code.to_string(), Value::Object(keys));
            }
        }
        Value::Object(out)
    }

    #[test]
    #[ignore = "writes app/forms/conditions.json from the rules corpus"]
    fn write_conditions() {
        let path = format!(
            "{}/../../app/forms/conditions.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = serde_json::to_string(&derived()).expect("json") + "\n";
        std::fs::write(path, text).expect("conditions.json");
    }

    #[test]
    fn conditions_match_the_rules_corpus() {
        assert_eq!(
            conditions(),
            &derived(),
            "app/forms/conditions.json is stale: run `cargo test -p bir-shell write_conditions -- --ignored`"
        );
    }

    /// Every condition has a reviewed tag that names an answer, and no tag
    /// carries the corpus's developer notes.
    #[test]
    fn every_condition_has_a_short_tag_for_filers() {
        let mut tags = 0;
        for (code, list) in conditions().as_object().unwrap() {
            for condition in list.as_object().unwrap().values() {
                let text = condition.as_str().unwrap();
                let tag = labels()[code.as_str()][text]
                    .as_str()
                    .unwrap_or_else(|| panic!("{code}: no tag for {text:?}"));
                tags += 1;
                assert!(tag.starts_with("Needs "), "{code}: {tag:?}");
                assert!(tag.len() <= 90, "{code}: {tag:?} is long");
                for word in [
                    "incorrect",
                    "mistaken",
                    "contrary",
                    "official",
                    "edit ",
                    "runtime",
                    "enabled",
                    "disabled",
                    "frm",
                ] {
                    assert!(
                        !tag.to_ascii_lowercase().contains(word),
                        "{code}: {tag:?} says {word:?}"
                    );
                }
            }
        }
        assert!(tags > 100, "{tags}");
        // No tag for a condition the corpus no longer has.
        for (code, list) in labels().as_object().unwrap() {
            let current: Vec<&Value> = conditions()[code.as_str()]
                .as_object()
                .map(|o| o.values().collect())
                .unwrap_or_default();
            for text in list.as_object().unwrap().keys() {
                assert!(
                    current.iter().any(|c| c.as_str() == Some(text)),
                    "{code}: stale tag for {text:?}"
                );
            }
        }
    }
}
