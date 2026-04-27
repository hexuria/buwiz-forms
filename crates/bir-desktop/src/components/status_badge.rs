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
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let (bg_col, text_col, label) = match self.status {
            FormStatus::Draft => (rgba(0x45475a66), rgb(0xa6adc8), "Draft"),
            FormStatus::Valid => (rgba(0xa6e3a133), rgb(0xa6e3a1), "Valid"),
            FormStatus::Error => (rgba(0xf38ba833), rgb(0xf38ba8), "Error"),
            FormStatus::Submitted => (rgba(0xfacc1533), rgb(0xfacc15), "Submitted"),
            FormStatus::Confirmed => (rgba(0x22c55e33), rgb(0x22c55e), "Confirmed"),
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
