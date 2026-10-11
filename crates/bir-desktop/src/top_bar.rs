//! The app's top header strip and the notifications drawer.
//!
//! The strip is its own ~36 px row above every page: the dev pills (debug
//! builds) on the left, the bell and theme buttons on the right. The bell
//! opens the notifications list as a floating drawer over the right edge of
//! the content area instead of a full page.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::*;

use crate::app::{ActiveView, AppState};

/// Height of the header strip.
pub(crate) const TOP_BAR_HEIGHT: f32 = 36.;
/// Width of the notifications drawer.
pub(crate) const DRAWER_WIDTH: f32 = 380.;

/// Bell badge text: hidden at 0, the number for 1-9, "9+" above.
pub(crate) fn badge_label(count: usize) -> Option<String> {
    match count {
        0 => None,
        1..=9 => Some(count.to_string()),
        _ => Some("9+".to_string()),
    }
}

/// Something that can open or close the drawer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DrawerInput {
    /// The bell button.
    Toggle,
    /// Any former "show the Notifications page" entry point.
    Open,
    /// The drawer's own × button.
    Close,
    Escape,
    /// A mouse press outside the drawer. `on_bell` presses are left to the
    /// bell's own toggle, so one click on the bell does not close and reopen.
    OutsideClick {
        on_bell: bool,
    },
}

/// Next open state of the drawer.
pub(crate) fn drawer_next(open: bool, input: DrawerInput) -> bool {
    match input {
        DrawerInput::Toggle => !open,
        DrawerInput::Open => true,
        DrawerInput::Close | DrawerInput::Escape => false,
        DrawerInput::OutsideClick { on_bell: true } => open,
        DrawerInput::OutsideClick { on_bell: false } => false,
    }
}

/// Where a request to show a view lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavTarget {
    Page(ActiveView),
    /// Notifications are a drawer over the current page, not a page.
    NotificationsDrawer,
}

pub(crate) fn nav_target(view: ActiveView) -> NavTarget {
    match view {
        ActiveView::Notifications => NavTarget::NotificationsDrawer,
        other => NavTarget::Page(other),
    }
}

impl AppState {
    pub(crate) fn drawer_input(&mut self, input: DrawerInput, cx: &mut Context<Self>) {
        let next = drawer_next(self.notifications_open, input);
        if next && !self.notifications_open {
            // Keep the view's profile scope current before showing it, so
            // switching profile then opening cannot show the previous one's.
            let tin = self.active_session_tin.clone();
            self.notifications_view.update(cx, |view, cx| {
                view.set_active_session_tin(tin, cx);
            });
        }
        if next != self.notifications_open {
            self.notifications_open = next;
            cx.notify();
        }
    }

    /// Every way into notifications (alert toasts, deep links, the agent's
    /// "notifications" slug) comes through here.
    pub(crate) fn open_notifications(&mut self, cx: &mut Context<Self>) {
        self.drawer_input(DrawerInput::Open, cx);
    }

    fn icon_button(id: &'static str, icon: IconName, cx: &App) -> Stateful<Div> {
        div()
            .id(id)
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.))
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(cx.theme().muted))
            .child(
                Icon::new(icon)
                    .size(px(18.))
                    .text_color(cx.theme().foreground),
            )
    }

    pub(crate) fn render_top_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let count = self.notifications_view.read(cx).active_count();
        let dark = cx.theme().mode.is_dark();
        let bell_bounds = self.bell_bounds.clone();

        let bell = Self::icon_button(crate::agent::ids::NAV_NOTIFICATIONS, IconName::Bell, cx)
            .when(self.notifications_open, |this| this.bg(cx.theme().accent))
            .child(crate::layout_probe::bounds_probe(bell_bounds))
            .on_click(cx.listener(|this, _ev, _window, cx| {
                this.drawer_input(DrawerInput::Toggle, cx);
            }))
            .when_some(badge_label(count), |this, label| {
                this.child(
                    div()
                        .absolute()
                        .top(px(-2.))
                        .right(px(-4.))
                        .min_w(px(16.))
                        .h(px(16.))
                        .px(px(3.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(cx.theme().danger)
                        .text_color(gpui::white())
                        .text_size(px(10.))
                        .font_weight(FontWeight::BOLD)
                        .child(label),
                )
            });

        let theme = Self::icon_button(
            "theme_toggle_btn",
            if dark { IconName::Moon } else { IconName::Sun },
            cx,
        )
        .on_click(|_ev, window, cx| {
            window.dispatch_action(Box::new(crate::global_actions::ToggleTheme), cx);
        });

        div()
            .id("top_bar")
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .w_full()
            .h(px(TOP_BAR_HEIGHT))
            .pr_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .children(crate::dev_mode::render_pills(cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .items_center()
                    .gap_1()
                    .child(bell)
                    .child(theme),
            )
            .into_any_element()
    }

    /// The drawer, positioned over the right edge of the content area below
    /// the strip. `None` while closed.
    pub(crate) fn render_notifications_drawer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.notifications_open {
            return None;
        }
        let bell_bounds = self.bell_bounds.clone();
        Some(
            div()
                .id(crate::agent::ids::PAGE_NOTIFICATIONS)
                .absolute()
                .top(px(TOP_BAR_HEIGHT))
                .right_0()
                .bottom_0()
                .w(px(DRAWER_WIDTH))
                .max_w_full()
                .flex()
                .flex_col()
                .bg(cx.theme().background)
                .border_l_1()
                .border_color(cx.theme().border)
                .shadow_lg()
                .occlude()
                .on_mouse_down_out(cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                    let on_bell = bell_bounds
                        .get()
                        .is_some_and(|bounds| bounds.contains(&ev.position));
                    this.drawer_input(DrawerInput::OutsideClick { on_bell }, cx);
                }))
                .child(self.notifications_view.clone())
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{ActiveView, DrawerInput, NavTarget, badge_label, drawer_next, nav_target};

    #[test]
    fn badge_hides_at_zero_and_caps_at_nine() {
        assert_eq!(badge_label(0), None);
        for n in 1..=9 {
            assert_eq!(badge_label(n), Some(n.to_string()));
        }
        assert_eq!(badge_label(10), Some("9+".into()));
        assert_eq!(badge_label(250), Some("9+".into()));
    }

    #[test]
    fn drawer_transitions() {
        assert!(drawer_next(false, DrawerInput::Toggle));
        assert!(!drawer_next(true, DrawerInput::Toggle));
        assert!(drawer_next(false, DrawerInput::Open));
        assert!(drawer_next(true, DrawerInput::Open));
        for open in [false, true] {
            assert!(!drawer_next(open, DrawerInput::Escape));
            assert!(!drawer_next(open, DrawerInput::Close));
            assert!(!drawer_next(
                open,
                DrawerInput::OutsideClick { on_bell: false }
            ));
            // A press on the bell is left to its toggle.
            assert_eq!(
                drawer_next(open, DrawerInput::OutsideClick { on_bell: true }),
                open
            );
        }
        // Clicking the bell while open: outside-press keeps it, toggle closes.
        let after_press = drawer_next(true, DrawerInput::OutsideClick { on_bell: true });
        assert!(!drawer_next(after_press, DrawerInput::Toggle));
    }

    #[test]
    fn notifications_entry_points_open_the_drawer() {
        // The view itself and its agent/deep-link slug.
        assert_eq!(
            nav_target(ActiveView::Notifications),
            NavTarget::NotificationsDrawer
        );
        let view = crate::agent::ids::view_from_slug("notifications").expect("slug");
        assert_eq!(nav_target(view), NavTarget::NotificationsDrawer);
        // Toast clicks, deep links and the agent all send `Open`.
        for open in [false, true] {
            assert!(drawer_next(open, DrawerInput::Open));
        }
        // No other view is diverted.
        for view in [
            ActiveView::GlobalDashboard,
            ActiveView::Dashboard,
            ActiveView::Settings,
            ActiveView::CronTasks,
            ActiveView::ImportExport,
        ] {
            assert_eq!(nav_target(view), NavTarget::Page(view));
        }
    }
}
