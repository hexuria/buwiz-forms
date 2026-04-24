use gpui::*;
use gpui_component::StyledExt;
use gpui_component::*;
use crate::views::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActiveView {
    Dashboard,
    NewForm,
    SavedForms,
    SubmissionHistory,
    ProfileManager,
    Settings,
}

pub struct AppState {
    active_view: ActiveView,
    dashboard: Entity<DashboardView>,
}

impl AppState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            active_view: ActiveView::Dashboard,
            dashboard: cx.new(|_| DashboardView::new()),
        }
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().w_64().h_full().bg(cx.theme().secondary).p_4().child("Sidebar")
    }

    fn render_active_view(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        match self.active_view {
            ActiveView::Dashboard => self.dashboard.clone().into_any_element(),
            _ => div().child("Not implemented").into_any_element(),
        }
    }
}

impl Render for AppState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .size_full()
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .p_4()
                    .child(self.render_active_view(cx))
            )
    }
}
