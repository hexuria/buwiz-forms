//! Tax-relief specification dropdown (1601C Item 13A, 2551Q Item 12A).
//!
//! The official forms use a `<select>` here, not free text: `1` Special Rate,
//! `2` International Tax Treaty and, on 1601C only, `3` Both. The draft stores
//! the option code, which is what the XML serializes.

use bir_core::validation::official_tax_relief_code;
use gpui::{AppContext, Context, Entity, SharedString, Window};
use gpui_component::select::{SelectItem, SelectState};

#[derive(Clone)]
pub struct TaxReliefOption {
    code: String,
    label: &'static str,
}

impl SelectItem for TaxReliefOption {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.into()
    }

    fn value(&self) -> &Self::Value {
        &self.code
    }

    fn matches(&self, query: &str) -> bool {
        self.label
            .to_ascii_lowercase()
            .contains(&query.trim().to_ascii_lowercase())
    }
}

pub type TaxReliefSelectState = SelectState<Vec<TaxReliefOption>>;

/// The official options; `allow_both` is true for 1601C only.
pub fn tax_relief_options(allow_both: bool) -> Vec<TaxReliefOption> {
    let mut options = vec![
        TaxReliefOption {
            code: "1".to_string(),
            label: "Special Rate",
        },
        TaxReliefOption {
            code: "2".to_string(),
            label: "International Tax Treaty",
        },
    ];
    if allow_both {
        options.push(TaxReliefOption {
            code: "3".to_string(),
            label: "Both",
        });
    }
    options
}

/// Build the dropdown, preselecting the draft's value. Legacy drafts saved
/// from the old free-text field that hold a known label are mapped to its
/// code; anything else starts unselected so validation asks for a choice.
pub fn new_tax_relief_select<V: 'static>(
    current: &str,
    allow_both: bool,
    window: &mut Window,
    cx: &mut Context<V>,
) -> Entity<TaxReliefSelectState> {
    let selected = official_tax_relief_code(current, allow_both);
    cx.new(|cx| {
        let mut state = SelectState::new(tax_relief_options(allow_both), None, window, cx);
        if let Some(code) = selected {
            state.set_selected_value(&code, window, cx);
        }
        state
    })
}

/// The selected option code, or an empty string when nothing is selected.
pub fn selected_tax_relief_code(state: &Entity<TaxReliefSelectState>, cx: &gpui::App) -> String {
    state.read(cx).selected_value().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_match_the_official_selects() {
        let codes = |allow_both| {
            tax_relief_options(allow_both)
                .iter()
                .map(|option| (option.code.clone(), option.label))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            codes(true),
            [
                ("1".to_string(), "Special Rate"),
                ("2".to_string(), "International Tax Treaty"),
                ("3".to_string(), "Both"),
            ]
        );
        assert_eq!(codes(false).len(), 2, "2551Q has no Both option");
        for (code, label) in codes(true) {
            assert_eq!(
                official_tax_relief_code(&code, true).as_deref(),
                Some(code.as_str())
            );
            assert_eq!(
                official_tax_relief_code(label, true).as_deref(),
                Some(code.as_str())
            );
        }
    }
}
