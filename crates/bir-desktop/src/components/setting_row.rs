//! One settings row: title, description, optional note, and a control.
//!
//! The text column is `flex_1` + `min_w_0`, so long text wraps instead of
//! pushing the control off the edge, and the control never shrinks. When the
//! window is narrow the control drops under the text. The row draws no
//! padding or border; the caller's container owns those.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::*;

/// Below this window width the control sits under the text.
pub const STACK_BELOW: Pixels = px(720.);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowLayout {
    /// Text on the left, control on the right.
    Inline,
    /// Control under the text.
    Stacked,
}

/// The layout for a window `width` wide.
pub fn layout_for(width: Pixels) -> RowLayout {
    if width < STACK_BELOW {
        RowLayout::Stacked
    } else {
        RowLayout::Inline
    }
}

#[derive(IntoElement)]
pub struct SettingRow {
    title: SharedString,
    title_color: Option<Hsla>,
    description: SharedString,
    note: Option<SharedString>,
    note_color: Option<Hsla>,
    control: Option<AnyElement>,
}

impl SettingRow {
    pub fn new(title: impl Into<SharedString>, description: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            title_color: None,
            description: description.into(),
            note: None,
            note_color: None,
            control: None,
        }
    }

    pub fn title_color(mut self, color: Hsla) -> Self {
        self.title_color = Some(color);
        self
    }

    /// A small line under the description; `None` shows nothing.
    pub fn note(mut self, note: Option<impl Into<SharedString>>) -> Self {
        self.note = note.map(Into::into);
        self
    }

    /// Note colour; muted by default.
    pub fn note_color(mut self, color: Hsla) -> Self {
        self.note_color = Some(color);
        self
    }

    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }
}

impl RenderOnce for SettingRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let layout = layout_for(window.viewport_size().width);
        let muted = cx.theme().muted_foreground;
        let text = div()
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .when_some(self.title_color, |d, c| d.text_color(c))
                    .child(self.title),
            )
            .child(div().text_sm().text_color(muted).child(self.description))
            .when_some(self.note, |col, note| {
                col.child(
                    div()
                        .text_xs()
                        .text_color(self.note_color.unwrap_or(muted))
                        .child(note),
                )
            });
        let row = div().flex().w_full().gap_4().child(text);
        let row = match layout {
            RowLayout::Inline => row.flex_row().items_center().justify_between(),
            RowLayout::Stacked => row.flex_col().items_start().gap_3(),
        };
        row.when_some(self.control, |row, control| {
            row.child(div().flex_shrink_0().child(control))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{RowLayout, STACK_BELOW, layout_for};
    use gpui::px;

    #[test]
    fn narrow_windows_stack_and_wide_ones_stay_inline() {
        assert_eq!(layout_for(px(480.)), RowLayout::Stacked);
        assert_eq!(layout_for(STACK_BELOW - px(1.)), RowLayout::Stacked);
        assert_eq!(layout_for(STACK_BELOW), RowLayout::Inline);
        assert_eq!(layout_for(px(1400.)), RowLayout::Inline);
    }
}
