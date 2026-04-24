use gpui::*;
use gpui_component::StyledExt;

pub struct DashboardView;

impl DashboardView {
    pub fn new() -> Self { Self }
}

impl Render for DashboardView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .child("📊 Dashboard — Coming Soon")
    }
}
