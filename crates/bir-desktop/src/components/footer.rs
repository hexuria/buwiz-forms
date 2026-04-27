use gpui::prelude::*;
use gpui::*;
use gpui_component::ActiveTheme;
use std::path::PathBuf;

pub fn render_footer<T>(cx: &mut Context<T>) -> impl IntoElement {
    div()
        .w_full()
        .h(px(72.))
        .px_4()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .bg(cx.theme().background)
        .border_t_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .flex_row()
                .items_center()
                .gap_4()
                // Bagong Pilipinas Logo
                .child(
                    img(PathBuf::from("assets/svg/bagong-pilipinas.svg"))
                        .w_12()
                        .h_12()
                        .object_fit(ObjectFit::Contain),
                )
                // BIR Logo
                .child(
                    img(PathBuf::from("assets/images/bir-new-logo.png"))
                        .w_12()
                        .h_12()
                        .object_fit(ObjectFit::Contain),
                ),
        )
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("© 2026 Goldcoders Corp. goldcoders.dev"),
                )
                // Goldcoders Logo
                .child(
                    div()
                        .id("goldcoders_logo")
                        .cursor_pointer()
                        .on_click(|_event, _window, cx| {
                            cx.open_url("https://goldcoders.dev");
                        })
                        .child(
                            img(PathBuf::from("assets/images/goldcoders_logo.png"))
                                .w_12()
                                .h_12()
                                .object_fit(ObjectFit::Contain),
                        ),
                ),
        )
}
