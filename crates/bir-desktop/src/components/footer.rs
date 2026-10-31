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
            min_h={px(72.)}
            px_4
            py_2
            flex
            flex_row
            flex_wrap
            gap_2
            items_center
            justify_end
            bg={cx.theme().background}
            border_t_1
            border_color={cx.theme().border}
        >
            // No government seals: this app is not the BIR's. Text may shrink and wrap.
            <div flex flex_row items_center gap_2 min_w_0 max_w_full>
                <div flex_1 min_w_0 text_xs text_color={cx.theme().muted_foreground}>
                    {"© 2026 Goldcoders Corp. goldcoders.dev"}
                </div>
                // Goldcoders Logo
                <div
                    id="goldcoders_logo"
                    flex_shrink_0
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
