//! Paint-side validation gate: pristine pages stay clean; Submit still sees
//! every blocking issue.
//!
//! Evaluator stamping is unchanged. This only decides which already-computed
//! field errors are shown in the editor.

use std::collections::BTreeSet;

/// Two bits of editor state: which fields the user has touched, and whether
/// this view has saved the draft once in the current session.
///
/// `saved_once` is view-session only. Newly opened drafts are often persisted
/// immediately (they already have a row id), so a database id is not a
/// reliable "the user saved" bit. Reloading an existing draft therefore starts
/// pristine again — `?` until a persisted flag exists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationPaintGate {
    touched: BTreeSet<String>,
    saved_once: bool,
}

impl ValidationPaintGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn touch(&mut self, field: impl Into<String>) {
        let field = field.into();
        if !field.is_empty() {
            self.touched.insert(field);
        }
    }

    pub fn mark_saved(&mut self) {
        self.saved_once = true;
    }

    pub const fn saved_once(&self) -> bool {
        self.saved_once
    }

    pub fn is_touched(&self, field: &str) -> bool {
        self.touched.contains(field)
    }

    /// Whether this field's issues should be painted.
    ///
    /// After a successful save in this session, every field paints. Until then,
    /// only fields the user has touched (change/blur) paint.
    pub fn should_paint(&self, field: &str) -> bool {
        self.saved_once || self.touched.contains(field)
    }

    pub fn visible_error<'a>(
        &self,
        errors: &'a [(String, String)],
        field: &str,
    ) -> Option<&'a String> {
        if !self.should_paint(field) {
            return None;
        }
        errors
            .iter()
            .find(|(candidate, _)| candidate == field)
            .map(|(_, message)| message)
    }

    pub fn visible_field_errors(&self, errors: &[(String, String)]) -> Vec<(String, String)> {
        errors
            .iter()
            .filter(|(field, _)| self.should_paint(field))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_errors() -> Vec<(String, String)> {
        vec![
            ("tin".to_string(), "TIN is required".to_string()),
            (
                "taxable_year".to_string(),
                "Taxable year must be a 4-digit year".to_string(),
            ),
            ("txtYear".to_string(), "Year is required".to_string()),
        ]
    }

    #[test]
    fn pristine_snapshot_has_zero_visible_errors() {
        let gate = ValidationPaintGate::new();
        assert!(gate.visible_field_errors(&sample_errors()).is_empty());
        assert!(gate.visible_error(&sample_errors(), "tin").is_none());
        assert!(gate.visible_error(&sample_errors(), "txtYear").is_none());
    }

    #[test]
    fn after_touch_only_that_field_is_visible() {
        let mut gate = ValidationPaintGate::new();
        gate.touch("txtYear");
        let visible = gate.visible_field_errors(&sample_errors());
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].0, "txtYear");
        assert!(gate.visible_error(&sample_errors(), "tin").is_none());
        assert!(gate.visible_error(&sample_errors(), "txtYear").is_some());
    }

    #[test]
    fn after_mark_saved_every_error_is_visible() {
        let mut gate = ValidationPaintGate::new();
        gate.mark_saved();
        let visible = gate.visible_field_errors(&sample_errors());
        assert_eq!(visible.len(), 3);
        assert!(gate.saved_once());
    }

    fn blank_2551q_submit_errors() -> Vec<(String, String)> {
        use bir_core::forms::form_2551q::Form2551QDraft;
        use bir_core::forms::FormValidator;
        use bir_core::profile::TaxpayerProfile;

        let profile: TaxpayerProfile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "",
            "tin": {
                "segment1": "123",
                "segment2": "456",
                "segment3": "789",
                "branch": "000"
            },
            "rdo_code": "",
            "line_of_business": "Services",
            "registered_address": "",
            "zip_code": "",
            "phone": "",
            "email": "",
            "default_form_type": "2551Qv2018"
        }))
        .expect("fixture profile");
        let mut draft = Form2551QDraft::new_from_profile(&profile, 0, 0);
        draft.tin.clear();
        FormValidator::validate(&draft)
    }

    #[test]
    fn blank_2551q_draft_paints_nothing_until_touch_or_save() {
        let all = blank_2551q_submit_errors();
        assert!(!all.is_empty(), "Submit still full-validates a blank 2551Q");
        assert!(
            all.iter()
                .any(|(field, _)| field == "tin" || field == "taxable_year"),
            "expected TIN or year in the submit list, got {all:?}"
        );
        let mut gate = ValidationPaintGate::new();
        assert!(gate.visible_field_errors(&all).is_empty());
        gate.touch("tin");
        let painted = gate.visible_field_errors(&all);
        assert!(
            painted.iter().all(|(field, _)| field == "tin"),
            "after touch only tin should paint, got {painted:?}"
        );
        gate.mark_saved();
        assert_eq!(gate.visible_field_errors(&all).len(), all.len());
    }
}
