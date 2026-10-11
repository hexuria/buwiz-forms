//! Shared form controls for the return editors: a single-select combobox, a
//! multi-select combobox with removable chips, and an inline field error.
//!
//! Views only. A page keeps one `Entity<ComboboxState>` per control, created
//! in `new()` with [`new_select_state`] and wired with [`on_select`] (or
//! [`on_multi_select`]). Before rendering, call [`sync_select`] (or
//! [`sync_multi_select`]) so the combobox shows the draft's current value, then
//! build the row with [`select_field`] (or [`multi_select_field`]). Controls
//! are always label-above and full width, so they fit phone, tablet and
//! desktop columns alike.

use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::*;

use crate::components::combobox::{Combobox, ComboboxEvent, ComboboxState};

/// Rows the dropdown shows before it scrolls.
const MAX_VISIBLE_ITEMS: usize = 7;

/// Change callback of a multi-select: receives the full new list of values.
pub type MultiChange<V> = Rc<dyn Fn(&mut V, Vec<String>, &mut Window, &mut Context<V>)>;

fn labels(options: &[(&str, &str)]) -> Vec<String> {
    options.iter().map(|(_, label)| label.to_string()).collect()
}

/// The display label of `value`, if it is one of `options`.
pub fn label_for<'a>(options: &[(&str, &'a str)], value: &str) -> Option<&'a str> {
    options
        .iter()
        .find(|(v, _)| *v == value)
        .map(|(_, label)| *label)
}

/// The stored value behind a displayed `label`.
#[allow(dead_code)] // The event handlers resolve labels themselves; kept for pages.
pub fn value_for<'a>(options: &[(&'a str, &str)], label: &str) -> Option<&'a str> {
    options
        .iter()
        .find(|(_, l)| *l == label)
        .map(|(value, _)| *value)
}

/// Options not yet picked, as dropdown labels.
fn unselected_labels(options: &[(&str, &str)], selected: &[String]) -> Vec<String> {
    options
        .iter()
        .filter(|(value, _)| !selected.iter().any(|s| s.as_str() == *value))
        .map(|(_, label)| label.to_string())
        .collect()
}

// ── State, events, sync ──

/// One combobox state over `(value, label)` options. Create it in `new()`.
pub fn new_select_state(
    options: &[(&str, &str)],
    window: &mut Window,
    cx: &mut App,
) -> Entity<ComboboxState> {
    let labels = labels(options);
    cx.new(|cx| ComboboxState::new(labels, MAX_VISIBLE_ITEMS, window, cx))
}

/// Subscribe a page to a single-select: `handler` gets the chosen value.
/// `options` may be a superset of what the dropdown currently lists.
pub fn on_select<V: 'static>(
    state: &Entity<ComboboxState>,
    options: &[(&str, &str)],
    window: &mut Window,
    cx: &mut Context<V>,
    handler: impl Fn(&mut V, String, &mut Window, &mut Context<V>) + 'static,
) -> Subscription {
    let options: Vec<(String, String)> = options
        .iter()
        .map(|(v, l)| (v.to_string(), l.to_string()))
        .collect();
    cx.subscribe_in(
        state,
        window,
        move |this, _, event: &ComboboxEvent, window, cx| {
            let Some(label) = event.selected.as_deref() else {
                return;
            };
            if let Some((value, _)) = options.iter().find(|(_, l)| l == label) {
                handler(this, value.clone(), window, cx);
            }
        },
    )
}

/// Make a combobox show `value` (and list exactly `options`). Call from the
/// page's `render` before building the row; it does nothing while the user is
/// typing in the box or when nothing changed.
pub fn sync_select(
    state: &Entity<ComboboxState>,
    options: &[(&str, &str)],
    value: Option<&str>,
    window: &mut Window,
    cx: &mut App,
) {
    let wanted = labels(options);
    let current = value.and_then(|v| label_for(options, v)).unwrap_or("");
    apply_sync(state, wanted, current, window, cx);
}

fn apply_sync(
    state: &Entity<ComboboxState>,
    wanted: Vec<String>,
    shown: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let (open, stale_options, stale_value) = {
        let s = state.read(cx);
        (s.open, s.options != wanted, s.selected_value(cx) != shown)
    };
    if open || !(stale_options || stale_value) {
        return;
    }
    state.update(cx, |s, cx| {
        if stale_options {
            s.options = wanted;
        }
        // Also resets the filtered list to the full option list.
        s.set_selected_value(shown, window, cx);
    });
}

// ── Rendering ──

/// A small danger-coloured line to put directly under a field. The colour is
/// read from the theme when it renders, so callers need no `cx`.
#[derive(IntoElement)]
pub struct FieldError {
    message: SharedString,
}

impl RenderOnce for FieldError {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .w_full()
            .min_w_0()
            .text_xs()
            .text_color(cx.theme().danger)
            .child(self.message)
    }
}

pub fn field_error(message: impl Into<SharedString>) -> FieldError {
    FieldError {
        message: message.into(),
    }
}

/// The validation messages recorded for any of `keys` (the pages' `(field,
/// message)` lists), joined into one line; `None` when there are none.
pub fn messages_for<'a>(
    errors: impl IntoIterator<Item = &'a (String, String)>,
    keys: &[&str],
) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    for (field, message) in errors {
        if keys.contains(&field.as_str()) {
            let message = message.replace('\n', " ");
            if !out.contains(&message) {
                out.push(message);
            }
        }
    }
    (!out.is_empty()).then(|| out.join(" "))
}

/// Put `error` (if any) directly under an already-built control.
pub fn with_field_error(content: AnyElement, error: Option<String>) -> AnyElement {
    match error {
        None => content,
        Some(message) => div()
            .flex()
            .flex_col()
            .gap_1()
            .w_full()
            .min_w_0()
            .child(content)
            .child(field_error(message))
            .into_any_element(),
    }
}

fn field_label(label: &str, has_error: bool, cx: &App) -> Div {
    let text = div()
        .min_w_0()
        .max_w_full()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .child(label.to_string());
    if has_error {
        text.text_color(cx.theme().danger)
    } else {
        text
    }
}

/// Label above a full-width single-select combobox. `value` is the stored
/// value (not the label). A disabled select shows the current label read-only.
/// `error` renders as [`field_error`] under the control.
pub fn select_field(
    label: &str,
    state: &Entity<ComboboxState>,
    options: &[(&str, &str)],
    value: Option<&str>,
    disabled: bool,
    error: Option<&str>,
    cx: &App,
) -> AnyElement {
    let body: AnyElement = if disabled {
        let shown = value
            .and_then(|v| label_for(options, v))
            .unwrap_or("Not selected");
        div()
            .w_full()
            .min_w_0()
            .px_3()
            .py_2()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .bg(cx.theme().muted.opacity(0.5))
            .text_color(cx.theme().muted_foreground)
            .child(shown.to_string())
            .into_any_element()
    } else {
        div()
            .w_full()
            .min_w_0()
            .child(Combobox::new(state))
            .into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .w_full()
        .min_w_0()
        .child(field_label(label, error.is_some(), cx))
        .child(body)
        .children(error.map(|message| field_error(message.to_string())))
        .into_any_element()
}

/// Subscribe a page to a multi-select. Adding an option calls `on_change`
/// with `selected(page)` plus the new value, unless `max` is reached.
#[allow(dead_code)] // No page needs several values yet; 0605 takes one ATC.
pub fn on_multi_select<V: 'static>(
    state: &Entity<ComboboxState>,
    options: &[(&str, &str)],
    max: usize,
    window: &mut Window,
    cx: &mut Context<V>,
    selected: impl Fn(&V) -> Vec<String> + 'static,
    on_change: MultiChange<V>,
) -> Subscription {
    on_select(
        state,
        options,
        window,
        cx,
        move |this, value, window, cx| {
            let mut next = selected(this);
            if next.len() >= max || next.contains(&value) {
                return;
            }
            next.push(value);
            on_change(this, next, window, cx);
        },
    )
}

/// Keep a multi-select dropdown listing only options not yet picked, with an
/// empty search box.
#[allow(dead_code)] // See `on_multi_select`.
pub fn sync_multi_select(
    state: &Entity<ComboboxState>,
    options: &[(&str, &str)],
    selected: &[String],
    window: &mut Window,
    cx: &mut App,
) {
    apply_sync(state, unselected_labels(options, selected), "", window, cx);
}

/// Label, the selected values as removable chips, a "Selected N of max" hint
/// and (below `max`) a searchable combobox to add more. Removing a chip calls
/// `on_change` with the remaining values. `id` must be unique on the page.
#[allow(dead_code, clippy::too_many_arguments)] // See `on_multi_select`.
pub fn multi_select_field<V: 'static>(
    id: &str,
    label: &str,
    state: &Entity<ComboboxState>,
    options: &[(&str, &str)],
    selected: &[String],
    max: usize,
    disabled: bool,
    error: Option<&str>,
    on_change: MultiChange<V>,
    cx: &mut Context<V>,
) -> AnyElement {
    let chips: Vec<AnyElement> = selected
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let text = label_for(options, value).unwrap_or(value.as_str());
            let chip_id = SharedString::from(format!("{id}_chip_{index}"));
            let remaining: Vec<String> = selected.iter().filter(|s| *s != value).cloned().collect();
            let on_change = on_change.clone();
            div()
                .max_w_full()
                .min_w_0()
                .overflow_hidden()
                .child(
                    Button::new(chip_id)
                        .label(format!("{text} ×"))
                        .outline()
                        .small()
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            on_change(this, remaining.clone(), window, cx)
                        })),
                )
                .into_any_element()
        })
        .collect();
    let can_add = !disabled && selected.len() < max;
    div()
        .flex()
        .flex_col()
        .gap_1()
        .w_full()
        .min_w_0()
        .child(field_label(label, error.is_some(), cx))
        .when(!chips.is_empty(), |col| {
            col.child(div().flex().flex_wrap().gap_2().w_full().children(chips))
        })
        .when(can_add, |col| {
            col.child(div().w_full().min_w_0().child(Combobox::new(state)))
        })
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("Selected {} of {max}", selected.len())),
        )
        .children(error.map(|message| field_error(message.to_string())))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{label_for, messages_for, unselected_labels, value_for};

    #[test]
    fn messages_join_for_matching_keys_only() {
        let errors = vec![
            ("tin".to_string(), "Bad TIN.".to_string()),
            ("zip".to_string(), "Bad\nzip.".to_string()),
            ("zip".to_string(), "Bad zip.".to_string()),
        ];
        assert_eq!(messages_for(&errors, &["zip"]), Some("Bad zip.".into()));
        assert_eq!(messages_for(&errors, &["tin", "zip"]).unwrap().len(), 17);
        assert_eq!(messages_for(&errors, &["email"]), None);
    }

    const OPTIONS: &[(&str, &str)] = &[("1", "January"), ("2", "February"), ("3", "March")];

    #[test]
    fn values_and_labels_map_both_ways() {
        assert_eq!(label_for(OPTIONS, "2"), Some("February"));
        assert_eq!(value_for(OPTIONS, "March"), Some("3"));
        assert_eq!(label_for(OPTIONS, "9"), None);
        assert_eq!(value_for(OPTIONS, "Smarch"), None);
    }

    #[test]
    fn picked_options_leave_the_dropdown() {
        let picked = vec!["2".to_string()];
        assert_eq!(unselected_labels(OPTIONS, &picked), ["January", "March"]);
        assert_eq!(unselected_labels(OPTIONS, &[]).len(), 3);
    }
}
