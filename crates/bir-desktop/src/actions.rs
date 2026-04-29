//! Named action handlers for global application events.

use crate::app::{ActiveView, AppState, ProfileTargetAction};
use crate::global_actions::*;
use gpui::*;

impl AppState {
    pub(crate) fn handle_toggle_sidebar(
        &mut self,
        _action: &ToggleSidebar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_sidebar_hidden = !self.is_sidebar_hidden;
        if self.is_sidebar_hidden {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn handle_toggle_sidebar_mini(
        &mut self,
        _action: &ToggleSidebarMini,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_mini_sidebar = !self.is_mini_sidebar;
        self.dashboard_view.update(cx, |view, cx| {
            view.set_mini_sidebar(self.is_mini_sidebar, cx);
        });
        cx.notify();
    }

    pub(crate) fn handle_focus_search(
        &mut self,
        _action: &FocusSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_sidebar_hidden = false;
        self.is_mini_sidebar = false;
        self.dashboard_view.update(cx, |view, cx| {
            view.set_mini_sidebar(self.is_mini_sidebar, cx);
        });
        cx.focus_view(&self.profile_filter, window);
        cx.notify();
    }

    pub(crate) fn handle_create_profile(
        &mut self,
        _action: &CreateProfile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_view = ActiveView::ProfileManager;
        self.active_profile_tin = None;
        self.profile_manager.update(cx, |view, cx| {
            view.reset_for_new(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn handle_toggle_theme(
        &mut self,
        _action: &ToggleTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::theme::cycle_theme(&mut self.theme_preference, &self.db, window, cx);
        cx.notify();
    }

    pub(crate) fn handle_open_cron_tasks(
        &mut self,
        _action: &OpenCronTasks,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_admin_access(ActiveView::CronTasks, window, cx);
    }

    pub(crate) fn handle_open_settings(
        &mut self,
        _action: &OpenSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_admin_access(ActiveView::Settings, window, cx);
    }

    pub(crate) fn handle_open_global_dashboard(
        &mut self,
        _action: &OpenGlobalDashboard,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_view = ActiveView::GlobalDashboard;
        self.active_profile_tin = None;
        cx.notify();
    }

    pub(crate) fn handle_open_command_palette(
        &mut self,
        _action: &OpenCommandPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_command_palette_open = true;
        let profiles = self.profiles.clone();
        let hide_tax_profiles = self.hide_tax_profiles;
        let active_session_tin = self.active_session_tin.clone();
        let palette = cx.new(|cx| {
            crate::components::command_palette::CommandPalette::new(
                profiles,
                hide_tax_profiles,
                active_session_tin,
                window,
                cx,
            )
        });

        cx.subscribe_in(
            &palette,
            window,
            |this: &mut Self,
             _,
             event: &crate::components::command_palette::CommandPaletteEvent,
             window,
             cx| {
                match event {
                    crate::components::command_palette::CommandPaletteEvent::SelectProfile(tin) => {
                        if let Some(profile) =
                            this.profiles.iter().find(|p| p.tin.full() == *tin).cloned()
                        {
                            this.select_profile(
                                profile,
                                ProfileTargetAction::ViewDashboard,
                                window,
                                cx,
                            );
                        }
                        this.is_command_palette_open = false;
                        if this.pending_profile.is_none() {
                            this.focus_handle.focus(window, cx);
                        }
                        cx.notify();
                    }
                    crate::components::command_palette::CommandPaletteEvent::CreateProfile(
                        query,
                    ) => {
                        this.is_command_palette_open = false;
                        this.active_view = ActiveView::ProfileManager;
                        this.profile_manager.update(cx, |view, cx| {
                            view.reset_for_new(window, cx);

                            let is_tin_like = !query.is_empty()
                                && query.chars().all(|c| c.is_ascii_digit() || c == '-');
                            let query_digits: String =
                                query.chars().filter(|c| c.is_ascii_digit()).collect();

                            if is_tin_like && query_digits.len() >= 9 {
                                view.prefill_tin(query, window, cx);
                            } else {
                                view.prefill_name(query, window, cx);
                            }
                        });
                        cx.notify();
                    }
                    crate::components::command_palette::CommandPaletteEvent::EditProfile(tin) => {
                        if let Some(profile) =
                            this.profiles.iter().find(|p| p.tin.full() == *tin).cloned()
                        {
                            this.select_profile(
                                profile,
                                ProfileTargetAction::EditProfile,
                                window,
                                cx,
                            );
                        }
                        this.is_command_palette_open = false;
                        if this.pending_profile.is_none() {
                            this.focus_handle.focus(window, cx);
                        }
                        cx.notify();
                    }
                    crate::components::command_palette::CommandPaletteEvent::Dismiss => {
                        this.is_command_palette_open = false;
                        this.focus_handle.focus(window, cx);
                        cx.notify();
                    }
                }
            },
        )
        .detach();

        self.command_palette_view = Some(palette.clone());
        palette.update(cx, |view, cx| {
            view.focus_input(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn handle_quit_application(
        &mut self,
        _action: &QuitApplication,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.quit();
    }

    pub(crate) fn handle_hide_application(
        &mut self,
        _action: &HideApplication,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.hide();
    }

    pub(crate) fn handle_hide_others(
        &mut self,
        _action: &HideOthers,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.hide_other_apps();
    }

    pub(crate) fn handle_close_window(
        &mut self,
        _action: &CloseWindow,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        window.remove_window();
    }

    pub(crate) fn handle_minimize_window(
        &mut self,
        _action: &MinimizeWindow,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        window.minimize_window();
    }

    pub(crate) fn handle_toggle_fullscreen(
        &mut self,
        _action: &ToggleFullScreen,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        window.toggle_fullscreen();
    }
}
