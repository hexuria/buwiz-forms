use gpui::prelude::*;
use gpui::*;
use gpui_component::ActiveTheme;
use gpui_rsx::rsx;

pub fn render_footer<T>(cx: &mut Context<T>) -> impl IntoElement {
    // Bare flags (`flex_row`, `w_12`) expand to the identically-named GPUI
    // builder methods, so this is a 1:1 rewrite of the previous chain.
    // Deliberately *not* using rsx's Tailwind `class=`: there `gap-4` means
    // `gap(px(4.))`, whereas GPUI's `.gap_4()` is 16px.
    rsx! {
        <div
            w_full
            h={px(72.)}
            px_4
            flex
            flex_row
            items_center
            justify_between
            bg={cx.theme().background}
            border_t_1
            border_color={cx.theme().border}
        >
            <div flex_shrink_0 flex flex_row items_center gap_4>
                // Bagong Pilipinas Logo
                <img src="svg/bagong-pilipinas.svg" w_12 h_12 object_fit={ObjectFit::Contain} />
                // BIR Logo
                <img src="images/bir-new-logo.png" w_12 h_12 object_fit={ObjectFit::Contain} />
            </div>
            <div flex_shrink_0 flex flex_row items_center gap_2>
                <div text_xs text_color={cx.theme().muted_foreground}>
                    {"© 2026 Goldcoders Corp. goldcoders.dev"}
                </div>
                // Goldcoders Logo
                <div
                    id="goldcoders_logo"
                    cursor_pointer
                    on_click={|_event, _window, cx: &mut App| {
                        cx.open_url("https://www.facebook.com/goldcoders.corp");
                    }}
                >
                    <img src="images/goldcoders_logo.png" w_12 h_12 object_fit={ObjectFit::Contain} />
                </div>
            </div>
        </div>
    }
}
