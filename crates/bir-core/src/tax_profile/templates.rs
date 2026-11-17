//! Form templates: exactly one per form per branch (D7), for every form. A
//! template names the form's OFFICIAL BOX KEYS. It pins answers a new return
//! starts from instead of the previous return's (owner's decision 9) and
//! holds the user's "Stop here" switches; it never holds amounts, dates or
//! tax facts (decisions 10 and 11). Validation lives here so every write
//! path (snapshot, stop switch, bulk copy) enforces the same rules.
//!
//! `fields_json` keeps the two `kind`s an older build parses:
//!
//! - `{"kind":"kept","value":"PT010"}` pins an answer;
//! - `{"kind":"marked"}` is a "Stop here" the user switched on;
//! - an optional `"stop": true|false` on either records the user's switch:
//!   `{"kind":"kept","value":"…","stop":true}` is pinned and a stop,
//!   `{"kind":"marked","stop":false}` switched an automatic stop off.
//!
//! Templates stored before box keys name draft fields of 2551Q and 1601C
//! (`item_13_election`, `schedule_1[0].atc`); [`migrate_fields`] reads them
//! as box keys.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::TaxProfileError;
use crate::forms::agent::carry::{self, Carry};
use crate::forms::agent::{AgentFormKind, Binding};

/// One box of a template. Absent boxes are "off".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TemplateField {
    /// A pinned answer (the box's value as the page shows it), and the
    /// user's stop switch for the box if they set one.
    Kept {
        value: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stop: Option<bool>,
    },
    /// No answer: a stop switch. Without `stop` (as older builds wrote it)
    /// the stop is switched on.
    Marked {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stop: Option<bool>,
    },
}

impl TemplateField {
    /// A pinned answer with no stop switch.
    pub fn pinned(value: impl Into<String>) -> Self {
        Self::Kept {
            value: Value::String(value.into()),
            stop: None,
        }
    }

    /// The user's stop switch: `Some(true)` always a stop, `Some(false)`
    /// never, `None` automatic.
    pub fn stop_switch(&self) -> Option<bool> {
        match self {
            Self::Kept { stop, .. } => *stop,
            Self::Marked { stop } => Some(stop.unwrap_or(true)),
        }
    }

    /// The pinned value, as text.
    pub fn pinned_value(&self) -> Option<String> {
        match self {
            Self::Kept { value, .. } => Some(value_text(value)),
            Self::Marked { .. } => None,
        }
    }

    /// The field for a pinned value and a stop switch; `None` when the box
    /// holds neither.
    pub fn from_parts(value: Option<Value>, stop: Option<bool>) -> Option<Self> {
        match (value, stop) {
            (Some(value), stop) => Some(Self::Kept { value, stop }),
            (None, Some(true)) => Some(Self::Marked { stop: None }),
            (None, Some(false)) => Some(Self::Marked { stop: Some(false) }),
            (None, None) => None,
        }
    }
}

/// A pinned value as the text the page shows (older templates stored typed
/// draft values).
fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// How the template came to exist. Persisted as `filed:<draft id>`,
/// `return:<draft id>`, `blank` or `copy:<from full TIN>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateSource {
    Filed {
        draft_id: i64,
    },
    /// Snapshot of a return that was not filed (Draft or Queued) at the time.
    Return {
        draft_id: i64,
    },
    /// Made by a "Stop here" switch, with no return behind it.
    Blank,
    Copy {
        from_tin: String,
    },
}

impl TemplateSource {
    pub fn to_db(&self) -> String {
        match self {
            Self::Filed { draft_id } => format!("filed:{draft_id}"),
            Self::Return { draft_id } => format!("return:{draft_id}"),
            Self::Blank => "blank".to_string(),
            Self::Copy { from_tin } => format!("copy:{from_tin}"),
        }
    }

    pub fn from_db(value: &str) -> Result<Self, TaxProfileError> {
        if value == "blank" {
            return Ok(Self::Blank);
        }
        if let Some(id) = value.strip_prefix("filed:")
            && let Ok(draft_id) = id.parse()
        {
            return Ok(Self::Filed { draft_id });
        }
        if let Some(id) = value.strip_prefix("return:")
            && let Ok(draft_id) = id.parse()
        {
            return Ok(Self::Return { draft_id });
        }
        if let Some(from_tin) = value.strip_prefix("copy:")
            && !from_tin.is_empty()
        {
            return Ok(Self::Copy {
                from_tin: from_tin.to_string(),
            });
        }
        Err(TaxProfileError::Invalid(format!(
            "Unknown template source `{value}`"
        )))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormTemplate {
    pub id: i64,
    /// Full TIN of the branch.
    pub tin: String,
    pub form_code: String,
    /// Box key -> what the template holds for it.
    pub fields: BTreeMap<String, TemplateField>,
    pub source: TemplateSource,
    pub created_at: String,
    pub updated_at: String,
}

impl FormTemplate {
    /// The pinned answers: box key -> value as the page shows it.
    pub fn pinned(&self) -> BTreeMap<String, String> {
        self.fields
            .iter()
            .filter_map(|(key, field)| Some((key.clone(), field.pinned_value()?)))
            .collect()
    }

    /// The user's stop switches: box key -> `true` (always a stop) or
    /// `false` (never).
    pub fn stop_edits(&self) -> BTreeMap<String, bool> {
        self.fields
            .iter()
            .filter_map(|(key, field)| Some((key.clone(), field.stop_switch()?)))
            .collect()
    }

    /// How many "Stop here" switches are on.
    pub fn stops_on(&self) -> usize {
        self.stop_edits().values().filter(|on| **on).count()
    }
}

/// Why a box cannot be stored in a template.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TemplateFieldError {
    #[error("Form {0} has no on-screen form; it cannot have a template")]
    UnknownForm(String),
    #[error("`{key}` is not a box of form {form_code}")]
    UnknownBox { form_code: String, key: String },
    #[error("`{key}` cannot be fixed by a template: {why}")]
    NotPinnable { key: String, why: &'static str },
    #[error("`{0}` cannot be a stop: it is computed or filled from the profile")]
    NotStoppable(String),
    #[error("A blank template can only hold stops; `{0}` has a fixed answer")]
    KeptOnBlankTemplate(String),
}

/// Why a box that does not carry cannot be pinned, in the user's words.
fn why_not(carry: Carry) -> &'static str {
    match carry {
        Carry::Carries => "",
        Carry::NotAnAnswer => "it is not an answer of its own",
        Carry::Computed => "the form computes it",
        Carry::Locked => "it comes from the profile or picks the return",
        Carry::Election => "the 8% or graduated choice is made on each year's return",
        Carry::Fact => "it comes from the tax profile",
        Carry::RowEntry => "it is an entry of that period's schedule",
        Carry::OneOff => "each return of this form is a new transaction",
        Carry::Penalty | Carry::Amount => "it is an amount",
        Carry::Date => "it is a date",
        Carry::Period => "it names the return's period",
        Carry::Amended => "it says whether the return is amended",
        Carry::LeftOpen => "the form asks it on every return",
    }
}

/// The on-screen form of `form_code`.
pub fn template_form(form_code: &str) -> Result<AgentFormKind, TemplateFieldError> {
    AgentFormKind::from_code(form_code)
        .ok_or_else(|| TemplateFieldError::UnknownForm(form_code.to_string()))
}

/// Enforce the rules for one form: only boxes of the form; a pinned answer
/// only on a box that carries ([`carry::carries`]); a stop switch only on a
/// box that may be a stop ([`carry::is_stoppable`]); a blank-source
/// template pins nothing.
pub fn validate_template_fields(
    form_code: &str,
    fields: &BTreeMap<String, TemplateField>,
    source: &TemplateSource,
) -> Result<AgentFormKind, TemplateFieldError> {
    let kind = template_form(form_code)?;
    let def = kind.definition();
    for (key, field) in fields {
        if def.box_def(key).is_none() {
            return Err(TemplateFieldError::UnknownBox {
                form_code: kind.form_code().to_string(),
                key: key.clone(),
            });
        }
        if let TemplateField::Kept { .. } = field {
            let carry = carry::carry_of(kind, key);
            if carry != Carry::Carries {
                return Err(TemplateFieldError::NotPinnable {
                    key: key.clone(),
                    why: why_not(carry),
                });
            }
            if *source == TemplateSource::Blank {
                return Err(TemplateFieldError::KeptOnBlankTemplate(key.clone()));
            }
        }
        if field.stop_switch().is_some() && !carry::is_stoppable(kind, key) {
            return Err(TemplateFieldError::NotStoppable(key.clone()));
        }
    }
    Ok(kind)
}

/// The answers of a stored return a template pins: every box that carries
/// and holds a value (box key -> value as the page shows it).
pub fn snapshot_answers(kind: AgentFormKind, draft: &Value) -> BTreeMap<String, String> {
    let def = kind.definition();
    let Ok(map) = crate::forms::agent::field_map(kind, draft) else {
        return BTreeMap::new();
    };
    carry::carried_boxes(kind)
        .into_iter()
        .filter_map(|key| {
            let value = def.box_value(key, &map);
            (!carry::is_blank_value(&value)).then(|| (key.to_string(), value))
        })
        .collect()
}

/// A template's boxes after an Update from a return: every stop switch it
/// held stays; its pinned answers become `answers`.
pub fn updated_fields(
    existing: &BTreeMap<String, TemplateField>,
    answers: &BTreeMap<String, String>,
) -> BTreeMap<String, TemplateField> {
    let mut fields = BTreeMap::new();
    for (key, field) in existing {
        if let Some(field) = TemplateField::from_parts(None, field.stop_switch()) {
            fields.insert(key.clone(), field);
        }
    }
    for (key, value) in answers {
        let stop = existing.get(key).and_then(TemplateField::stop_switch);
        if let Some(field) = TemplateField::from_parts(Some(Value::String(value.clone())), stop) {
            fields.insert(key.clone(), field);
        }
    }
    fields
}

// ------------------------------------------------------------ old ids

/// `schedule_1[0].atc` -> `/schedule_1/0/atc`.
fn pointer_of(field: &str) -> String {
    let mut pointer = String::new();
    for segment in field.split('.') {
        let (name, index) = match segment.split_once('[') {
            Some((name, rest)) => (name, rest.strip_suffix(']')),
            None => (segment, None),
        };
        pointer.push('/');
        pointer.push_str(name);
        if let Some(index) = index {
            pointer.push('/');
            pointer.push_str(index);
        }
    }
    pointer
}

/// The box an old template field id (a draft field of 2551Q or 1601C)
/// names, and the value it pins as that box's value (`None` for a stop).
/// `None` when no box writes the field (a tax fact such as `eopt_tier`, an
/// internal flag), or the value is no option of the box.
fn legacy_box(
    kind: AgentFormKind,
    field: &str,
    value: Option<&Value>,
) -> Option<(String, Option<String>)> {
    let def = kind.definition();
    // 2551Q's ATC rows are its host-written selects `drpATC1-6`.
    if kind == AgentFormKind::Form2551Q
        && let Some(index) = field
            .strip_prefix("schedule_1[")
            .and_then(|rest| rest.strip_suffix("].atc"))
            .and_then(|index| index.parse::<usize>().ok())
    {
        // The select's value is the official option number, not the code;
        // the rate follows the period when the return computes.
        let option = |code: String| {
            crate::forms::form_2551q_xml::official_atc_options()
                .iter()
                .find(|option| option.code == code.trim())
                .map(|option| option.value.clone())
        };
        return match value {
            None => Some((format!("drpATC{}", index + 1), None)),
            Some(value) => Some((
                format!("drpATC{}", index + 1),
                Some(option(value_text(value))?),
            )),
        };
    }
    let pointer = pointer_of(field);
    let entry = def.boxes.iter().find(|entry| {
        entry.is_question()
            && std::iter::once(&entry.key)
                .chain(&entry.members)
                .filter_map(|key| def.bindings.get(key))
                .any(|bound| {
                    crate::forms::agent::binding_paths(&bound.binding).contains(&pointer.as_str())
                })
    })?;
    let Some(value) = value else {
        return Some((entry.key.clone(), None));
    };
    let shown = match def.bindings.get(&entry.key).map(|bound| &bound.binding) {
        Some(Binding::Choice { options }) => options
            .iter()
            .find(|option| option.set.get(&pointer) == Some(value))
            .map(|option| option.value.clone())?,
        Some(_) => value_text(value),
        None => return None,
    };
    Some((entry.key.clone(), Some(shown)))
}

/// A stored template's boxes, read as box keys: a box key stays (a radio
/// is read as its item's group box); an old draft field id of 2551Q or
/// 1601C becomes the box that writes it, its typed value that box's value,
/// with the same meaning (a pinned answer, or a stop). An old pinned value
/// the carry rule no longer copies (the "Amended Return?" answer, 2551Q's
/// Item 13, a profile box) is not pinned any more; a field no box writes
/// (`eopt_tier`, `business_start_date`: tax facts, decision 11) is dropped.
pub fn migrate_fields(
    form_code: &str,
    fields: BTreeMap<String, TemplateField>,
) -> BTreeMap<String, TemplateField> {
    let Ok(kind) = template_form(form_code) else {
        return fields;
    };
    let def = kind.definition();
    let mut out: BTreeMap<String, TemplateField> = BTreeMap::new();
    for (key, field) in fields {
        let (key, value, stop) = if def.box_def(&key).is_some() {
            (
                carry::answer_key(kind, &key),
                field.pinned_value(),
                field.stop_switch(),
            )
        } else {
            let pinned = match &field {
                TemplateField::Kept { value, .. } => Some(value),
                TemplateField::Marked { .. } => None,
            };
            let Some((key, value)) = legacy_box(kind, &key, pinned) else {
                tracing::warn!(form_code, field = %key, "dropping an old template field no box writes");
                continue;
            };
            (key, value, field.stop_switch())
        };
        let value = value
            .filter(|value| !carry::is_blank_value(value) && carry::carries(kind, &key))
            .map(Value::String);
        let stop = stop.filter(|_| carry::is_stoppable(kind, &key));
        if out.contains_key(&key) {
            continue;
        }
        if let Some(field) = TemplateField::from_parts(value, stop) {
            out.insert(key, field);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn kept(value: &str) -> TemplateField {
        TemplateField::pinned(value)
    }

    #[test]
    fn fields_json_shape_is_the_documented_contract() {
        let fields = BTreeMap::from([
            ("a".to_string(), kept("x")),
            ("b".to_string(), TemplateField::Marked { stop: None }),
            (
                "c".to_string(),
                TemplateField::Kept {
                    value: json!("y"),
                    stop: Some(true),
                },
            ),
            ("d".to_string(), TemplateField::Marked { stop: Some(false) }),
        ]);
        let stored = json!({
            "a": {"kind": "kept", "value": "x"},
            "b": {"kind": "marked"},
            "c": {"kind": "kept", "value": "y", "stop": true},
            "d": {"kind": "marked", "stop": false},
        });
        assert_eq!(serde_json::to_value(&fields).unwrap(), stored);
        let read: BTreeMap<String, TemplateField> = serde_json::from_value(stored).unwrap();
        assert_eq!(read, fields);
        let template = FormTemplate {
            id: 1,
            tin: String::new(),
            form_code: "2551Q".into(),
            fields,
            source: TemplateSource::Blank,
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert_eq!(
            template.stop_edits(),
            BTreeMap::from([
                ("b".to_string(), true),
                ("c".to_string(), true),
                ("d".to_string(), false),
            ])
        );
        assert_eq!(template.stops_on(), 2);
        assert_eq!(
            template.pinned(),
            BTreeMap::from([
                ("a".to_string(), "x".to_string()),
                ("c".to_string(), "y".to_string())
            ])
        );
    }

    #[test]
    fn validation_follows_the_carry_rule_for_every_form() {
        let filed = TemplateSource::Filed { draft_id: 1 };
        let one = |key: &str, field: TemplateField| BTreeMap::from([(key.to_string(), field)]);
        let stop = || TemplateField::Marked { stop: None };
        // 2551Q: an ATC is pinned; an amount is a stop, never pinned.
        assert!(validate_template_fields("2551Q", &one("drpATC1", kept("1")), &filed).is_ok());
        assert!(validate_template_fields("2551Q", &one("txtATCAmt1", stop()), &filed).is_ok());
        assert_eq!(
            validate_template_fields("2551Q", &one("txtATCAmt1", kept("100")), &filed).err(),
            Some(TemplateFieldError::NotPinnable {
                key: "txtATCAmt1".into(),
                why: "it is an amount"
            })
        );
        // A form the old registry never knew: 0619E's ATC.
        let kind = AgentFormKind::from_code("0619E").unwrap();
        let atc = carry::carried_boxes(kind);
        assert!(!atc.is_empty());
        assert!(validate_template_fields("0619E", &one(atc[0], kept("WI010")), &filed).is_ok());
        // Profile boxes are neither pinned nor stops.
        assert_eq!(
            validate_template_fields("2551Q", &one("frm2551Qv2018:txtRDOCode", stop()), &filed)
                .err(),
            Some(TemplateFieldError::NotStoppable(
                "frm2551Qv2018:txtRDOCode".into()
            ))
        );
        assert!(matches!(
            validate_template_fields("2551Q", &one("status", kept("x")), &filed),
            Err(TemplateFieldError::UnknownBox { .. })
        ));
        assert_eq!(
            validate_template_fields(
                "2551Q",
                &one("drpATC1", kept("PT010")),
                &TemplateSource::Blank
            )
            .err(),
            Some(TemplateFieldError::KeptOnBlankTemplate("drpATC1".into()))
        );
        assert_eq!(
            validate_template_fields("9999", &BTreeMap::new(), &filed).err(),
            Some(TemplateFieldError::UnknownForm("9999".into()))
        );
    }

    #[test]
    fn update_keeps_every_stop_switch_and_replaces_the_pins() {
        let existing = BTreeMap::from([
            (
                "drpATC1".to_string(),
                TemplateField::Kept {
                    value: json!("PT010"),
                    stop: Some(true),
                },
            ),
            ("drpATC2".to_string(), kept("PT040")),
            (
                "txtATCAmt1".to_string(),
                TemplateField::Marked { stop: None },
            ),
            (
                "txtATCAmt2".to_string(),
                TemplateField::Marked { stop: Some(false) },
            ),
        ]);
        let answers = BTreeMap::from([("drpATC1".to_string(), "PT012".to_string())]);
        assert_eq!(
            updated_fields(&existing, &answers),
            BTreeMap::from([
                (
                    "drpATC1".to_string(),
                    TemplateField::Kept {
                        value: json!("PT012"),
                        stop: Some(true)
                    }
                ),
                (
                    "txtATCAmt1".to_string(),
                    TemplateField::Marked { stop: None }
                ),
                (
                    "txtATCAmt2".to_string(),
                    TemplateField::Marked { stop: Some(false) }
                ),
            ])
        );
    }

    #[test]
    fn old_2551q_and_1601c_ids_read_as_box_keys_with_the_same_meaning() {
        let stored: BTreeMap<String, TemplateField> = serde_json::from_value(json!({
            "schedule_1[0].atc": {"kind": "kept", "value": "PT010"},
            "schedule_1[1].atc": {"kind": "kept", "value": "PT040"},
            "schedule_1[0].taxable_amount": {"kind": "marked"},
            "tax_relief": {"kind": "kept", "value": false},
            "item_13_election": {"kind": "kept", "value": "graduated"},
            "eopt_tier": {"kind": "kept", "value": "micro"},
            "creditable_tax_withheld": {"kind": "marked"},
        }))
        .unwrap();
        let read = migrate_fields("2551Q", stored);
        let option = |code: &str| {
            crate::forms::form_2551q_xml::official_atc_options()
                .iter()
                .find(|option| option.code == code)
                .map(|option| option.value.clone())
                .unwrap()
        };
        // The select's value: the official option number of the code.
        assert_eq!(read["drpATC1"], kept(&option("PT010")));
        assert_eq!(read["drpATC2"], kept(&option("PT040")));
        assert_eq!(read["txtATCAmt1"], TemplateField::Marked { stop: None });
        assert_eq!(
            read["frm2551Qv2018:txt15"],
            TemplateField::Marked { stop: None }
        );
        // Tax relief "No" is the group box's option for `false`.
        assert!(
            read["frm2551Qv2018:taxTreaty"]
                .pinned_value()
                .is_some_and(|value| !value.is_empty()),
            "{read:?}"
        );
        // Item 13 is asked on each return and the EOPT tier is a tax fact.
        assert!(!read.contains_key("frm2551Qv2018:taxRate"), "{read:?}");
        assert_eq!(read.len(), 5, "{read:?}");
        assert!(
            validate_template_fields("2551Q", &read, &TemplateSource::Return { draft_id: 1 })
                .is_ok()
        );

        let stored: BTreeMap<String, TemplateField> = serde_json::from_value(json!({
            "atc": {"kind": "kept", "value": "WW010"},
            "tax_relief": {"kind": "kept", "value": true},
            "tax_relief_specification": {"kind": "kept", "value": "Treaty X"},
            "category_of_agent": {"kind": "marked"},
            "tax_14_total_compensation": {"kind": "marked"},
        }))
        .unwrap();
        let read = migrate_fields("1601C", stored);
        assert_eq!(read["frm1601c:txtATC"], kept("WW010"));
        assert_eq!(read["frm1601c:SpecialTax"], kept("1"));
        assert_eq!(read["frm1601c:selTreaty"], kept("Treaty X"));
        // Item 11 stays a stop: the user asked to stop there.
        assert_eq!(
            read["frm1601c:CatAgent"],
            TemplateField::Marked { stop: None }
        );
        assert_eq!(
            read["frm1601c:txtTax14"],
            TemplateField::Marked { stop: None }
        );
        assert!(
            validate_template_fields("1601C", &read, &TemplateSource::Return { draft_id: 1 })
                .is_ok()
        );
        // Box keys read back unchanged.
        assert_eq!(migrate_fields("1601C", read.clone()), read);
    }

    #[test]
    fn source_round_trips_and_rejects_garbage() {
        for source in [
            TemplateSource::Blank,
            TemplateSource::Filed { draft_id: 42 },
            TemplateSource::Return { draft_id: 7 },
            TemplateSource::Copy {
                from_tin: "12345678900001".into(),
            },
        ] {
            assert_eq!(TemplateSource::from_db(&source.to_db()).unwrap(), source);
        }
        for bad in ["", "filed:", "filed:x", "return:", "copy:", "kept"] {
            assert!(TemplateSource::from_db(bad).is_err(), "{bad}");
        }
    }
}
