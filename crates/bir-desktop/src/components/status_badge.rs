#![allow(dead_code)]
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FormStatus {
    Draft,
    Valid,
    Error,
    Submitted,
    Confirmed,
}

pub struct StatusBadge {
    status: FormStatus,
}

impl StatusBadge {
    pub fn new(status: FormStatus) -> Self {
        Self { status }
    }
}

impl Render for StatusBadge {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        use gpui_component::ActiveTheme as _;
        let theme = cx.theme();
        let (bg_col, text_col, label) = match self.status {
            FormStatus::Draft => (theme.secondary, theme.muted_foreground, "Draft"),
            FormStatus::Valid => (
                theme.success.opacity(0.15),
                crate::theme::success_on_tint(theme),
                "Valid",
            ),
            FormStatus::Error => (
                theme.danger.opacity(0.15),
                crate::theme::danger_on_tint(theme),
                "Error",
            ),
            FormStatus::Submitted => (
                theme.warning.opacity(0.15),
                crate::theme::warning_on_tint(theme),
                "Submitted",
            ),
            FormStatus::Confirmed => (
                theme.success.opacity(0.15),
                crate::theme::success_on_tint(theme),
                "Confirmed",
            ),
        };

        div()
            .bg(bg_col)
            .text_color(text_col)
            .px_3()
            .py_1()
            .rounded_full()
            .text_sm()
            .font_weight(FontWeight::BOLD)
            .child(label)
    }
}
