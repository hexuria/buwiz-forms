use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::scroll::ScrollableElement;
use gpui_rsx::rsx;

pub struct DebugLogViewerView {
    job_name: String,
    log_text: String,
    focus_handle: FocusHandle,
}

impl DebugLogViewerView {
    pub fn new(
        job_name: String,
        log_text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        super::secondary_window::focus_on_open(&focus_handle, window, cx);
        Self {
            job_name,
            log_text,
            focus_handle,
        }
    }
}

impl Render for DebugLogViewerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let body = rsx! {
            <div size_full flex flex_col bg={gpui::rgb(0x0b0b0b)}>
                <div
                    flex
                    flex_wrap
                    items_center
                    justify_between
                    gap_3
                    px_5
                    py_3
                    border_b_1
                    border_color={cx.theme().border}
                    bg={cx.theme().background}
                >
                    <div flex flex_col gap_1 flex_1 min_w_0>
                        <div
                            text_lg
                            font_weight={FontWeight::BOLD}
                            text_color={cx.theme().foreground}
                        >
                            {"Debug Log"}
                        </div>
                        // The job title can be long; let it wrap inside the
                        // body rather than fight the window title bar.
                        <div text_sm text_color={cx.theme().muted_foreground}>
                            {self.job_name.clone()}
                        </div>
                    </div>
                </div>
                <div
                    flex_1
                    min_h_0
                    p_4
                    bg={cx.theme().background}
                    overflow_y_scrollbar
                >
                    <div
                        text_sm
                        // using monospaced if possible or just standard text
                        font_family={crate::platform::MONOSPACE_FONT}
                        text_color={cx.theme().foreground}
                    >
                        {self.log_text.clone()}
                    </div>
                </div>
            </div>
        };
        super::secondary_window::with_window_actions(body, &self.focus_handle, "DebugLogViewer", cx)
    }
}
