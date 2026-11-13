//! The form page's template stops: the boxes the section pill's ‹ and ›
//! step to when the branch's template for the form marks fields
//! ([`TemplateField::Marked`], the guided-mode stops of D13/D14).
//!
//! A template names draft fields (`item_13_election`, `schedule_1[0].atc`,
//! [`crate::tax_profile::field_kinds`]); the page binds official keys. This
//! maps one to the other through the form's bindings, so the page never
//! guesses which box a template field is.

use std::collections::BTreeSet;

use serde_json::Value;

use super::{AgentFormKind, Binding, error_pointer};
use crate::tax_profile::{FieldKind, FormTemplate, TemplateField, field_kinds_for};

/// A template's stops as page keys, and the keys the filer may make a stop.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplateStops {
    /// Keys of the boxes that write a marked field.
    pub stops: Vec<String>,
    /// Keys of the boxes that write a templatable field the template does
    /// not keep a value for ("Stop here" can mark them).
    pub markable: Vec<String>,
}

/// `/schedule_1/0/atc` -> `schedule_1[0].atc` (a template's field id).
fn field_of_pointer(path: &str) -> String {
    let mut field = String::new();
    for segment in path.trim_start_matches('/').split('/') {
        if !segment.is_empty() && segment.chars().all(|c| c.is_ascii_digit()) {
            field.push('[');
            field.push_str(segment);
            field.push(']');
        } else {
            if !field.is_empty() {
                field.push('.');
            }
            field.push_str(segment);
        }
    }
    field
}

/// The draft fields a box writes: its path, or each field its choices set
/// to different values (a field every option sets alike is not decided by
/// the box).
fn written_paths(binding: &Binding) -> Vec<&str> {
    match binding {
        Binding::Text { path, .. } | Binding::Amount { path, .. } | Binding::Whole { path, .. } => {
            vec![path.as_str()]
        }
        Binding::Choice { options } => {
            let paths: BTreeSet<&String> = options
                .iter()
                .flat_map(|option| option.set.keys())
                .collect();
            paths
                .into_iter()
                .filter(|path| {
                    let values: Vec<Option<&Value>> =
                        options.iter().map(|option| option.set.get(*path)).collect();
                    values.iter().any(|value| *value != values[0])
                })
                .map(String::as_str)
                .collect()
        }
    }
}

/// The templatable field the page box `key` writes, if any.
pub fn field_of_key(kind: AgentFormKind, key: &str) -> Option<String> {
    let kinds = field_kinds_for(kind.form_code())?;
    let bound = kind.definition().bindings.get(key)?;
    written_paths(&bound.binding)
        .into_iter()
        .map(field_of_pointer)
        .find(|field| matches!(kinds.kind_of(field), Some(FieldKind::Templatable(_))))
}

/// The page boxes that write `field`.
pub fn keys_of_field(kind: AgentFormKind, field: &str) -> Vec<String> {
    let pointer = error_pointer(field);
    kind.definition()
        .bindings
        .iter()
        .filter(|(_, bound)| written_paths(&bound.binding).contains(&pointer.as_str()))
        .map(|(key, _)| key.clone())
        .collect()
}

/// `template`'s stops and stoppable boxes on the page of `kind`.
pub fn page_stops(kind: AgentFormKind, template: &FormTemplate) -> TemplateStops {
    let mut stops: Vec<String> = Vec::new();
    for (field, value) in &template.fields {
        if matches!(value, TemplateField::Marked) {
            for key in keys_of_field(kind, field) {
                if !stops.contains(&key) {
                    stops.push(key);
                }
            }
        }
    }
    let markable = kind
        .definition()
        .bindings
        .keys()
        .filter(|key| {
            field_of_key(kind, key).is_some_and(|field| {
                !matches!(
                    template.fields.get(&field),
                    Some(TemplateField::Kept { .. })
                )
            })
        })
        .cloned()
        .collect();
    TemplateStops { stops, markable }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::tax_profile::TemplateSource;

    fn template(fields: BTreeMap<String, TemplateField>) -> FormTemplate {
        FormTemplate {
            id: 1,
            tin: "00012345300000".into(),
            form_code: "2551Q".into(),
            fields,
            source: TemplateSource::Blank,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn a_pointer_reads_back_as_the_template_field_id() {
        assert_eq!(field_of_pointer("/item_13_election"), "item_13_election");
        assert_eq!(field_of_pointer("/schedule_1/0/atc"), "schedule_1[0].atc");
        assert_eq!(
            error_pointer(&field_of_pointer("/schedule_1/2/tax_due")),
            "/schedule_1/2/tax_due"
        );
    }

    #[test]
    fn item_13_radios_write_the_templatable_election() {
        let kind = AgentFormKind::Form2551Q;
        assert_eq!(
            field_of_key(kind, "frm2551Qv2018:taxRate1").as_deref(),
            Some("item_13_election")
        );
        let keys = keys_of_field(kind, "item_13_election");
        assert!(
            keys.contains(&"frm2551Qv2018:taxRate1".to_string()),
            "{keys:?}"
        );
        assert!(
            keys.contains(&"frm2551Qv2018:taxRate2".to_string()),
            "{keys:?}"
        );
        // The RDO comes from the profile (Auto): never a stop.
        assert_eq!(field_of_key(kind, "frm2551Qv2018:txtRDOCode"), None);
        // Neither is a box whose field the form does not register.
        assert_eq!(field_of_key(kind, "frm2551Qv2018:txt20"), None);
        assert_eq!(
            field_of_key(kind, "txtATCAmt2").as_deref(),
            Some("schedule_1[1].taxable_amount")
        );
    }

    #[test]
    fn marked_fields_are_stops_and_kept_fields_are_not_markable() {
        let kind = AgentFormKind::Form2551Q;
        let marked = template(BTreeMap::from([(
            "item_13_election".to_string(),
            TemplateField::Marked,
        )]));
        let stops = page_stops(kind, &marked);
        assert!(stops.stops.contains(&"frm2551Qv2018:taxRate1".to_string()));
        assert!(
            stops
                .markable
                .contains(&"frm2551Qv2018:taxRate1".to_string())
        );

        let kept = template(BTreeMap::from([(
            "item_13_election".to_string(),
            TemplateField::Kept {
                value: Value::from("graduated"),
            },
        )]));
        let stops = page_stops(kind, &kept);
        assert!(stops.stops.is_empty());
        assert!(
            !stops
                .markable
                .contains(&"frm2551Qv2018:taxRate1".to_string())
        );
    }
}
