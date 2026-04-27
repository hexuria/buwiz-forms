use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::scroll::ScrollableElement;

pub struct DebugLogViewerView {
    job_name: String,
    log_text: String,
}

impl DebugLogViewerView {
    pub fn new(job_name: String, log_text: String) -> Self {
        Self { job_name, log_text }
    }
}

impl Render for DebugLogViewerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(gpui::rgb(0x0b0b0b))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .px_5()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .child(
                        div().flex().items_center().gap_3().child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().foreground)
                                .child(format!("Debug Log: {}", self.job_name)),
                        ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p_4()
                    .bg(cx.theme().background)
                    .overflow_y_scrollbar()
                    .child(
                        div()
                            .text_sm()
                            // using monospaced if possible or just standard text
                            .font_family(crate::platform::MONOSPACE_FONT)
                            .text_color(cx.theme().foreground)
                            .child(self.log_text.clone()),
                    ),
            )
    }
}
