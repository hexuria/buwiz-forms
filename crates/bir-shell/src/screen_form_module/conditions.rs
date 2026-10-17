//! Boxes another answer turns off (rule 6): the page fades them and says
//! what turns them on; they come alive when the condition holds. The checks
//! themselves are not touched.
//!
//! What turns a box on is the official corpus's `enabled_when`
//! (`rules/forms/<package>/fields.json`, read only), copied for the boxes
//! the page shows into `app/forms/conditions.json` by the ignored test
//! `write_conditions`; `conditions_match_the_rules_corpus` keeps the copy
//! in step. Whether a box is off right now is the form's own computation:
//! the host types a value into it on a copy of the draft, recomputes, and
//! the box is off when the computation clears it again (1601C clears Item 28
//! unless the return is amended). A box that holds a value, or one the
//! filer cannot type for another reason, is never reported off.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use bir_core::forms::agent::{self, AgentFormKind, Binding, DatePart};
use serde_json::{Map, Value, json};

/// `app/forms/conditions.json`: form code -> key -> what turns the box on.
fn conditions() -> &'static Value {
    static CONDITIONS: OnceLock<Value> = OnceLock::new();
    CONDITIONS.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../app/forms/conditions.json"))
            .expect("app/forms/conditions.json is a JSON object")
    })
}

/// A value shown as nothing: blank, all zeros (`0.00`, `00`) or unticked.
fn is_blank(shown: &str) -> bool {
    let shown = shown.trim();
    shown == "false" || shown.chars().all(|c| matches!(c, '0' | '.' | ',' | '-'))
}

/// A value the box accepts if it is on, to see whether the form keeps it.
fn probe_value(kind: AgentFormKind, key: &str, shown: &str) -> Option<Value> {
    let def = kind.definition();
    let entry = def.box_def(key)?;
    let binding = def.bindings.get(key).or_else(|| {
        entry
            .group
            .as_deref()
            .and_then(|group| def.bindings.get(group))
    })?;
    Some(match &binding.binding {
        Binding::Amount { .. } | Binding::Whole { .. } => json!("1"),
        Binding::Text {
            part: Some(part), ..
        } => json!(match part {
            DatePart::Month => "01",
            DatePart::Day => "15",
            DatePart::Year => "2020",
        }),
        Binding::Text { date: Some(_), .. } => json!("01/15/2020"),
        Binding::Text { .. } => json!("A"),
        // A radio of a group is ticked by its own key.
        Binding::Choice { .. } if entry.group.is_some() => json!("true"),
        Binding::Choice { options } => {
            let option = options
                .iter()
                .map(|option| option.value.as_str())
                .find(|value| *value != shown && !is_blank(value))?;
            json!(option)
        }
    })
}

/// The boxes of `draft` that another answer turns off right now, each with
/// what turns it on. `values` is what the page shows for the draft.
pub(super) fn off_boxes(kind: AgentFormKind, draft: &Value, values: &Value) -> Map<String, Value> {
    let mut off = Map::new();
    let Some(list) = conditions()[kind.form_code()].as_object() else {
        return off;
    };
    for (key, condition) in list {
        let shown = values[key].as_str().unwrap_or_default();
        if !is_blank(shown) {
            continue;
        }
        let Some(probe) = probe_value(kind, key, shown) else {
            continue;
        };
        let mut copy = draft.clone();
        let edits = Map::from_iter([(key.clone(), probe)]);
        let rejected = super::apply_screen_edits(
            kind,
            &mut copy,
            &edits,
            true,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        if !matches!(rejected, Ok(ref rejected) if rejected.is_empty()) {
            continue;
        }
        if agent::compute(kind, &mut copy).is_err() {
            continue;
        }
        let after = super::values_json(kind, &copy).unwrap_or(Value::Null);
        if is_blank(after[key].as_str().unwrap_or_default()) {
            off.insert(key.clone(), condition.clone());
        }
    }
    off
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// Corpus conditions that describe the official runtime's own states (a
    /// static-disabled box, an ATC modal's slots, the official Edit path), not
    /// an answer the filer gives: the page leaves those boxes as they are.
    const NOT_AN_ANSWER: [&str; 10] = [
        "disabled in the static form",
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

    /// What `app/forms/conditions.json` should hold, read from the corpus:
    /// every box the page shows (a schedule cell too) whose `enabled_when`
    /// names an answer, computed boxes left out.
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
            let shown = super::super::schema_keys(code);
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
}
