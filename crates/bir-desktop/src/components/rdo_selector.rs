#![allow(dead_code)]
use gpui::*;
use gpui_rsx::rsx;

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
        rsx! {
            <div flex flex_col w_full gap_2>
                <div
                    text_sm
                    font_weight={FontWeight::BOLD}
                    text_color={cx.theme().foreground}
                >
                    {"Revenue District Office (RDO)"}
                </div>
                <div
                    w_full
                    p_2
                    bg={cx.theme().secondary}
                    border_1
                    border_color={cx.theme().border}
                    rounded_md
                    flex
                    justify_between
                    items_center
                >
                    <div text_color={cx.theme().foreground}>
                        {self.selected_rdo.clone()}
                    </div>
                    <div text_color={cx.theme().muted_foreground}>{"▼"}</div>
                </div>
            </div>
        }
    }
}
