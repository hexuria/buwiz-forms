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
        #[cfg(feature = "agent")]
        self.note_keybinding_fired(crate::agent::ids::KEY_TOGGLE_SIDEBAR, "focused");
        cx.notify();
    }

    pub(crate) fn handle_toggle_app_visibility(
        &mut self,
        _action: &ToggleAppVisibility,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.perform_toggle_app_visibility(cx);
    }

    pub(crate) fn perform_toggle_app_visibility(&mut self, cx: &mut Context<Self>) {
        #[cfg(feature = "agent")]
        if self.toggling_visibility {
            return;
        }
        #[cfg(feature = "agent")]
        {
            self.toggling_visibility = true;
        }
        crate::platform::toggle_app_visibility();
        #[cfg(feature = "agent")]
        self.note_keybinding_fired(crate::agent::ids::KEY_TOGGLE_VISIBILITY, "global");
        #[cfg(feature = "agent")]
        {
            self.toggling_visibility = false;
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

    /// `QuitApplication`: Cmd+Q / the app menu's "Quit eBIRForms" / agent
    /// `app.quit` on macOS, Ctrl+Q and Alt+F4 on Linux, Alt+F4 on Windows.
    ///
    /// macOS quits by default through the guarded `request_application_quit`,
    /// exactly like tray Quit (drops the tray icon, releases the agent bind
    /// and the live-DB owner lock; unsaved Profile compliance changes keep the
    /// window open). With the "Cmd+Q hides to menu bar" setting on it instead
    /// hides to the menu bar and the background cron keeps polling; the real
    /// quit is then tray Quit or the agent's `shutdown`. Linux always quits;
    /// Windows hides to the tray like its close button.
    /// See `quit_shortcut::quit_shortcut_route`.
    pub(crate) fn handle_quit_application(
        &mut self,
        _action: &QuitApplication,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(feature = "agent")]
        self.note_keybinding_fired(crate::agent::ids::KEY_QUIT, "global");
        let hides = self
            .db
            .lock()
            .map(|db| crate::quit_shortcut::quit_shortcut_hides(&db))
            .unwrap_or(false);
        match crate::quit_shortcut::quit_shortcut_route(
            crate::quit_shortcut::QuitShortcutPlatform::current(),
            hides,
        ) {
            crate::quit_shortcut::QuitShortcutRoute::Quit => {
                let hook = self.quit_hook.clone();
                self.request_application_quit(window, cx, move || {
                    if let Some(hook) = hook {
                        hook();
                    }
                });
            }
            crate::quit_shortcut::QuitShortcutRoute::HideToTray => {
                crate::platform::hide_from_dock()
            }
        }
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

    /// Cmd+W / Ctrl+W. On Linux (`close_button_quits`) it quits like the X
    /// button, through the guarded `request_application_quit`; elsewhere it
    /// hides the window to the tray.
    pub(crate) fn handle_close_window(
        &mut self,
        _action: &CloseWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match crate::platform::close_window_route(crate::platform::close_button_quits()) {
            crate::platform::CloseWindowRoute::Quit => {
                let hook = self.quit_hook.clone();
                self.request_application_quit(window, cx, move || {
                    if let Some(hook) = hook {
                        hook();
                    }
                });
            }
            crate::platform::CloseWindowRoute::Hide => crate::platform::hide_from_dock(),
        }
    }

    pub(crate) fn handle_minimize_window(
        &mut self,
        _action: &MinimizeWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.minimize_window();
        #[cfg(feature = "agent")]
        self.note_keybinding_fired(crate::agent::ids::KEY_MINIMIZE, "focused");
        cx.notify();
    }

    pub(crate) fn handle_zoom_window(
        &mut self,
        _action: &ZoomWindow,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        window.zoom_window();
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
