#![allow(dead_code)]
use gpui::*;

pub struct RdoSelector {
    pub selected_rdo: String,
}

impl RdoSelector {
    pub fn new() -> Self {
        Self {
            selected_rdo: "043A - Pasig City".into(),
        }
    }
}

impl Render for RdoSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        use gpui_component::ActiveTheme as _;
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child("Revenue District Office (RDO)"),
            )
            .child(
                div()
                    .w_full()
                    .p_2()
                    .bg(cx.theme().secondary)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_color(cx.theme().foreground)
                            .child(self.selected_rdo.clone()),
                    )
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("▼"),
                    ),
            )
    }
}
