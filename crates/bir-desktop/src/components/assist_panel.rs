//! Compact "filled by AI / needs you" strip shown under a form's toolbar
//! (2551Q and 1601C) while it is an editable draft.

use gpui::*;
use gpui_component::ActiveTheme as _;

use crate::agent::assist::{BoxSource, NeedsYouBox};

/// `None` when there is nothing to report.
pub fn assist_panel(
    id: &'static str,
    filled: &[(String, BoxSource)],
    needs_you: &[NeedsYouBox],
    cx: &App,
) -> Option<AnyElement> {
    if filled.is_empty() && needs_you.is_empty() {
        return None;
    }
    let theme = cx.theme();
    let row = |title: &'static str, body: String, color: Hsla| {
        div()
            .flex()
            .gap_2()
            .text_xs()
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color)
                    .child(title),
            )
            .child(div().text_color(theme.muted_foreground).child(body))
    };
    let mut panel = div()
        .id(id)
        .flex()
        .flex_col()
        .gap_1()
        .px_8()
        .py_2()
        .bg(theme.background)
        .border_b_1()
        .border_color(theme.border);
    if !filled.is_empty() {
        let body = filled
            .iter()
            .map(|(key, source)| format!("{key} ({})", source.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        panel = panel.child(row("Filled by AI", body, theme.foreground));
    }
    if !needs_you.is_empty() {
        let body = needs_you
            .iter()
            .map(|item| item.label.clone())
            .collect::<Vec<_>>()
            .join(", ");
        panel = panel.child(row("Needs you", body, theme.danger));
    }
    Some(panel.into_any_element())
}
