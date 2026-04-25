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
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xcdd6f4))
                    .child("Revenue District Office (RDO)"),
            )
            .child(
                div()
                    .w_full()
                    .p_2()
                    .bg(rgb(0x11111b))
                    .border_1()
                    .border_color(rgb(0x45475a))
                    .rounded_md()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_color(rgb(0xcdd6f4))
                            .child(self.selected_rdo.clone()),
                    )
                    .child(div().text_color(rgb(0x6c7086)).child("▼")),
            )
    }
}
