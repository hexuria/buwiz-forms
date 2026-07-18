//! Shared searchable multi-select over the official BIR form registry.
//!
//! The COR evidence editor and the calendar override editor both need a
//! type-to-filter checkbox picker of every official form code. Building the
//! option list here keeps the two selectors identical as the registry grows.

use gpui::{Context, Window};

use super::multi_select::{MultiSelectOption, MultiSelectState};

/// Every official registry form as `CODE - Title` options, sorted by code.
pub fn registry_form_options() -> Vec<MultiSelectOption> {
    let mut options: Vec<MultiSelectOption> = bir_core::forms::registry::FORM_REGISTRY
        .iter()
        .map(|form| MultiSelectOption::new(form.code, format!("{} - {}", form.code, form.title)))
        .collect();
    options.sort_by(|a, b| a.id.cmp(&b.id));
    options
}

/// A registry-backed multi-select whose trigger shows only the placeholder;
/// callers render their own chips for the selected codes.
pub fn registry_form_multi_select(
    placeholder: &str,
    window: &mut Window,
    cx: &mut Context<MultiSelectState>,
) -> MultiSelectState {
    MultiSelectState::new(registry_form_options(), window, cx)
        .placeholder(placeholder)
        .hide_trigger_chips(true)
}
