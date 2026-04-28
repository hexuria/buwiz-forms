use crate::views::cron_tasks::CronTasksView;
use crate::views::dashboard::{DashboardEvent, DashboardView};
use crate::views::form_2551q_view::{Form2551QEvent, Form2551QView};
use crate::views::global_dashboard::{GlobalDashboardEvent, GlobalDashboardView};
use crate::views::import_export::{ImportExportEvent, ImportExportView};
use crate::views::lock_screen::{LockScreenEvent, LockScreenView};
use crate::views::profile_manager::ProfileManagerView;
use crate::views::settings::{SettingsEvent, SettingsView};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState, OtpInput, OtpState};
use gpui_component::*;

use bir_core::db::Database;
use bir_core::forms::form_2551q::Form2551QDraft;
use bir_core::profile::TaxpayerProfile;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum AppThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

impl AppThemeMode {
    pub fn next(&self) -> Self {
        match self {
            Self::System => Self::Dark,
            Self::Dark => Self::Light,
            Self::Light => Self::System,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActiveView {
    GlobalDashboard,
    Dashboard,
    Form2551Q,
    ProfileManager,
    CronTasks,
    ImportExport,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProfileTargetAction {
    ViewDashboard,
    EditProfile,
}

pub struct AppState {
    active_view: ActiveView,
    profile_manager: Entity<ProfileManagerView>,
    dashboard_view: Entity<DashboardView>,
    global_dashboard_view: Entity<GlobalDashboardView>,
    cron_tasks_view: Entity<CronTasksView>,
    import_export_view: Entity<ImportExportView>,
    settings_view: Entity<SettingsView>,
    form_2551q_view: Option<Entity<Form2551QView>>,
    pending_form_draft: Option<Form2551QDraft>,
    db: Arc<Mutex<Database>>,
    profiles: Vec<TaxpayerProfile>,
    active_profile_tin: Option<String>,
    expanded_profile_tin: Option<String>,
    profile_filter: Entity<InputState>,
    sidebar_scroll: ScrollHandle,
    show_archived: bool,
    _subscriptions: Vec<Subscription>,
    /// Whether the window-aware subscription for global dashboard notifications has been set up.
    global_dashboard_notif_subscribed: bool,
    is_mini_sidebar: bool,
    is_sidebar_hidden: bool,
    theme_preference: AppThemeMode,
    focus_handle: FocusHandle,
    is_command_palette_open: bool,
    command_palette_view: Option<Entity<crate::components::command_palette::CommandPalette>>,
    is_locked: bool,
    lock_screen_view: Option<Entity<LockScreenView>>,
    pending_profile: Option<(TaxpayerProfile, ProfileTargetAction)>,
    profile_otp_state: Entity<OtpState>,
    profile_auth_error: Option<String>,
    os_auth_triggered: bool,
    unlocked_profile: Option<(TaxpayerProfile, ProfileTargetAction)>,
    hide_tax_profiles: bool,
    enable_profile_pins: bool,
    pending_admin_view: Option<ActiveView>,
    admin_otp_state: Entity<OtpState>,
    admin_auth_error: Option<String>,
    admin_os_auth_triggered: bool,
    /// The TIN of the currently active/unlocked profile session (only meaningful when hide_tax_profiles is enabled)
    active_session_tin: Option<String>,
}

impl AppState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let db_path = bir_core::db::default_database_path();
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let legacy_db_path = std::env::current_dir()
            .unwrap_or_default()
            .join("bir_data.db");
        if !db_path.exists()
            && legacy_db_path.exists()
            && legacy_db_path.metadata().map(|m| m.len()).unwrap_or(0) > 0
            && legacy_db_path != db_path
        {
            let _ = std::fs::copy(&legacy_db_path, &db_path);
        }
        let (db, recovered_backup) =
            Database::open_or_recreate(&db_path).expect("Failed to open database");
        if let Some(backup_path) = recovered_backup {
            eprintln!(
                "Recovered unreadable database at {} by moving it to {}",
                db_path.display(),
                backup_path.display()
            );
        }
        
        // Phase 2: Automated Cleanup of Regenerated PDFs
        // Safely clear out the temporary directory on application startup so it doesn't grow indefinitely.
        cx.background_executor().spawn(async move {
            let temp_pdf_dir = bir_core::platform::temp_dir().join("taxman-ebir-pdf");
            if temp_pdf_dir.exists() {
                let _ = std::fs::remove_dir_all(&temp_pdf_dir);
            }
        }).detach();

        let theme_preference = if let Ok(Some(val)) = db.get_setting("theme_preference") {
            serde_json::from_str(&val).unwrap_or(AppThemeMode::System)
        } else {
            AppThemeMode::System
        };
        let hide_tax_profiles = db.get_setting("hide_tax_profiles").ok().flatten().as_deref() == Some("true");
        let enable_profile_pins = db.get_setting("enable_profile_pins").ok().flatten().as_deref() == Some("true");

        let target_mode = match theme_preference {
            AppThemeMode::Light => ThemeMode::Light,
            AppThemeMode::Dark => ThemeMode::Dark,
            AppThemeMode::System => match window.appearance() {
                gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight => {
                    ThemeMode::Light
                }
                gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark => {
                    ThemeMode::Dark
                }
            },
        };
        Theme::change(target_mode, Some(window), cx);

        let profiles = db.list_profiles().unwrap_or_default();
        let db = Arc::new(Mutex::new(db));

        let active_view = if profiles.is_empty() {
            ActiveView::ProfileManager
        } else {
            ActiveView::GlobalDashboard
        };

        let bus = cx.new(|_| crate::events::EventBus {});
        cx.set_global(crate::events::GlobalEventBus(bus));
        crate::events::start_db_watcher(Arc::clone(&db), cx);

        let db_clone = Arc::clone(&db);
        let profile_manager = cx.new(|cx| ProfileManagerView::new(db_clone, window, cx));

        let db_clone_global = Arc::clone(&db);
        let global_dashboard_view =
            cx.new(|cx| GlobalDashboardView::new(db_clone_global, window, cx));

        let is_locked = db.lock().unwrap().get_setting("app_lock_enabled").ok().flatten().as_deref() == Some("true");
        let db_clone_lock = Arc::clone(&db);
        let lock_screen_view = Some(cx.new(|cx| LockScreenView::new(db_clone_lock, window, cx)));
        
        let profile_otp_state = cx.new(|cx| OtpState::new(4, window, cx).masked(true));
        let admin_otp_state = cx.new(|cx| OtpState::new(4, window, cx).masked(true));
        
        cx.subscribe_in(&profile_otp_state, window, |this: &mut Self, _entity, event: &InputEvent, window, cx| match event {
            InputEvent::Change => {
                let entered_pin = this.profile_otp_state.read(cx).value().to_string();
                if entered_pin.len() == 4 {
                    let hashed = bir_core::crypto::hash_pin(&entered_pin);
                    if let Some((p, a)) = this.pending_profile.clone() {
                        if Some(hashed) == p.profile_pin_hash {
                            this.unlocked_profile = Some((p, a));
                            this.pending_profile = None;
                            this.profile_auth_error = None;
                            this.profile_otp_state.update(cx, |input, cx| input.set_value("", window, cx));
                            this.focus_handle.focus(window, cx);
                        } else {
                            this.profile_auth_error = Some("Incorrect PIN. Please try again.".to_string());
                            this.profile_otp_state.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                        }
                    }
                }
                cx.notify();
            }
            _ => {}
        }).detach();

        cx.subscribe_in(&admin_otp_state, window, |this: &mut Self, _entity, event: &InputEvent, window, cx| match event {
            InputEvent::Change => {
                let entered_pin = this.admin_otp_state.read(cx).value().to_string();
                if entered_pin.len() == 4 {
                    let hashed = bir_core::crypto::hash_pin(&entered_pin);
                    let valid = if let Ok(db) = this.db.lock() {
                        let hash = db.get_setting("app_lock_pin_hash").ok().flatten();
                        hash.as_deref() == Some(&hashed)
                    } else { false };
                    
                    if valid {
                        if let Some(target) = this.pending_admin_view.take() {
                            this.active_view = target;
                            if target == ActiveView::CronTasks {
                                this.cron_tasks_view.update(cx, |view, cx| view.load_settings(cx));
                            }
                        }
                        this.admin_auth_error = None;
                        this.admin_otp_state.update(cx, |input, cx| input.set_value("", window, cx));
                        this.focus_handle.focus(window, cx);
                    } else {
                        this.admin_auth_error = Some("Incorrect Admin PIN.".to_string());
                        this.admin_otp_state.update(cx, |input, cx| {
                            input.set_value("", window, cx);
                            input.focus(window, cx);
                        });
                    }
                }
                cx.notify();
            }
            _ => {}
        }).detach();
        
        if let Some(view) = &lock_screen_view {
            cx.subscribe_in(view, window, |this: &mut Self, _entity, event: &LockScreenEvent, window, cx| match event {
                LockScreenEvent::Unlocked => {
                    this.is_locked = false;
                    this.focus_handle.focus(window, cx);
                    cx.notify();
                }
            }).detach();
        }

        let db_clone_cron = Arc::clone(&db);
        let cron_tasks_view = cx.new(|cx| CronTasksView::new(db_clone_cron, window, cx));

        let db_clone_import = Arc::clone(&db);
        let import_export_view = cx.new(|cx| ImportExportView::new(db_clone_import, window, cx));

        cx.subscribe(
            &import_export_view,
            |this: &mut Self, _entity, event: &ImportExportEvent, cx| match event {
                ImportExportEvent::ReloadApp => {
                    // Re-open DB
                    let db_path = bir_core::db::default_database_path();
                    let (new_db, _) = Database::open_or_recreate(&db_path)
                        .expect("Failed to open database on reload");
                    let profiles = new_db.list_profiles().unwrap_or_default();

                    if let Ok(mut locked_db) = this.db.lock() {
                        *locked_db = new_db;
                    }

                    this.profiles = profiles.clone();
                    this.global_dashboard_view.update(cx, |view, cx| {
                        view.set_profiles(profiles.clone(), cx);
                    });
                    this.cron_tasks_view.update(cx, |view, cx| {
                        view.load_settings(cx);
                    });

                    this.active_profile_tin = None;
                    this.expanded_profile_tin = None;
                    this.active_view = if this.profiles.is_empty() {
                        ActiveView::ProfileManager
                    } else {
                        ActiveView::GlobalDashboard
                    };

                    // Re-evaluate settings (like lock and theme)
                    if let Ok(db) = this.db.lock() {
                        if let Ok(Some(val)) = db.get_setting("theme_preference") {
                            this.theme_preference = serde_json::from_str(&val).unwrap_or(AppThemeMode::System);
                        } else {
                            this.theme_preference = AppThemeMode::System;
                        }
                    }

                    cx.notify();
                }
            },
        )
        .detach();
        
        let db_clone_settings = Arc::clone(&db);
        let settings_view = cx.new(|cx| SettingsView::new(db_clone_settings, window, cx));
        cx.subscribe(
            &settings_view,
            |this: &mut Self, _entity, event: &SettingsEvent, cx| match event {
                SettingsEvent::ReloadApp => {
                    if let Ok(db) = this.db.lock() {
                        this.hide_tax_profiles = db.get_setting("hide_tax_profiles").ok().flatten().as_deref() == Some("true");
                        this.enable_profile_pins = db.get_setting("enable_profile_pins").ok().flatten().as_deref() == Some("true");
                    }
                    // Sync dashboard
                    let htp = this.hide_tax_profiles;
                    this.dashboard_view.update(cx, |view, _cx| {
                        view.hide_tax_profiles = htp;
                    });
                    // Clear session if privacy mode was disabled
                    if !this.hide_tax_profiles {
                        this.active_session_tin = None;
                    }
                    cx.notify();
                }
            }
        ).detach();

        let hide_profiles_placeholder = if hide_tax_profiles {
            "Enter full TIN to access profile..."
        } else {
            "Search TIN or name"
        };
        let profile_filter =
            cx.new(|cx| InputState::new(window, cx).placeholder(hide_profiles_placeholder));
        let filter_sub = cx.subscribe_in(
            &profile_filter,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| {
                match event {
                    InputEvent::Change => {
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => {
                        let query = this.profile_filter.read(cx).value().trim().to_string();
                        // Match both raw digits (e.g. 261708015000) and formatted (e.g. 261-708-015-000)
                        if let Some(profile) = this.profiles.iter().find(|p| {
                            p.tin.full() == query || p.tin.formatted() == query
                        }).cloned() {
                            if this.hide_tax_profiles {
                                // End current session and start new one
                                this.active_session_tin = Some(profile.tin.full());
                            }
                            this.profile_filter.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                            });
                            this.select_profile(profile, ProfileTargetAction::ViewDashboard, window, cx);
                        }
                    }
                    _ => {}
                }
            },
        );

        let profile_sub = cx.subscribe(
            &profile_manager,
            |this: &mut Self, _entity, event: &crate::views::profile_manager::ProfileEvent, cx| {
                let saved_tin = match event {
                    crate::views::profile_manager::ProfileEvent::Saved(tin) => Some(tin.clone()),
                };
                
                if let Some(tin) = &saved_tin {
                    if this.hide_tax_profiles {
                        this.active_session_tin = Some(tin.clone());
                    }
                }

                let db_clone = this.db.clone();
                let active_tin = this.active_profile_tin.clone();

                cx.spawn(async move |this, cx| {
                    let profiles = cx
                        .background_executor()
                        .spawn(async move {
                            if let Ok(db) = db_clone.lock() {
                                db.list_profiles().unwrap_or_default()
                            } else {
                                Vec::new()
                            }
                        })
                        .await;

                    let _ = this.update(cx, |this, cx| {
                        this.profiles = profiles.clone();
                        this.global_dashboard_view.update(cx, |view, cx| {
                            view.set_profiles(profiles.clone(), cx);
                        });

                        if let Some(tin) = &active_tin {
                            if let Some(profile) =
                                this.profiles.iter().find(|p| p.tin.full() == *tin)
                            {
                                let p = profile.clone();
                                this.dashboard_view.update(cx, |view, cx| {
                                    view.set_profile(p, cx);
                                });
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            },
        );

        let db_clone2 = Arc::clone(&db);
        let dashboard_view = cx.new(|cx| DashboardView::new(db_clone2, window, cx));

        cx.subscribe_in(
            &global_dashboard_view,
            window,
            |this: &mut Self, _entity, event: &GlobalDashboardEvent, window, cx| match event {
                GlobalDashboardEvent::OpenForm {
                    tin,
                    form_code,
                    year,
                    quarter,
                } => {
                    let profile_clone = this.profiles.iter().find(|p| p.tin.full() == *tin).cloned();
                    if let Some(profile) = profile_clone {
                        this.select_profile(profile, ProfileTargetAction::ViewDashboard, window, cx);
                    } else {
                        this.active_profile_tin = Some(tin.clone());
                    }
                    
                    let event = DashboardEvent::FileForm {
                        form_code: form_code.clone(),
                        year: *year,
                        quarter: *quarter,
                    };
                    this.handle_file_form(&event, cx);
                }
                GlobalDashboardEvent::CheckStatus { .. } => {
                    // Now handled internally by GlobalDashboardView::check_status_for_tin
                }
                GlobalDashboardEvent::PushNotification(_level, _title, _message) => {
                    // Notifications need window — they'll be handled in the subscribe_in below
                }
                GlobalDashboardEvent::StatusChanged => {
                    cx.notify();
                }
            },
        )
        .detach();

        cx.subscribe(
            &dashboard_view,
            |this: &mut Self, _entity, event: &DashboardEvent, cx| match event {
                DashboardEvent::FileForm { .. } => this.handle_file_form(event, cx),
                DashboardEvent::Reload => {
                    if let Some(tin) = &this.active_profile_tin {
                        if let Some(profile) = this.profiles.iter().find(|p| p.tin.full() == *tin) {
                            let p = profile.clone();
                            this.dashboard_view.update(cx, |view, cx| {
                                view.set_profile(p, cx);
                            });
                        }
                    }
                    cx.notify();
                }
                DashboardEvent::LogoutProfile(_tin) => {
                    this.active_session_tin = None;
                    this.active_profile_tin = None;
                    this.active_view = ActiveView::GlobalDashboard;
                    cx.notify();
                }
            },
        )
        .detach();

        Self {
            active_view,
            profile_manager,
            dashboard_view,
            global_dashboard_view,
            cron_tasks_view,
            import_export_view,
            form_2551q_view: None,
            pending_form_draft: None,
            db,
            profiles,
            active_profile_tin: None,
            expanded_profile_tin: None,
            profile_filter,
            sidebar_scroll: ScrollHandle::new(),
            show_archived: false,
            _subscriptions: vec![profile_sub, filter_sub],
            global_dashboard_notif_subscribed: false,
            is_mini_sidebar: false,
            is_sidebar_hidden: false,
            theme_preference,
            focus_handle: cx.focus_handle(),
            is_command_palette_open: false,
            command_palette_view: None,
            is_locked,
            lock_screen_view,
            pending_profile: None,
            profile_otp_state,
            profile_auth_error: None,
            os_auth_triggered: false,
            unlocked_profile: None,
            settings_view,
            hide_tax_profiles,
            enable_profile_pins,
            pending_admin_view: None,
            admin_otp_state,
            admin_auth_error: None,
            admin_os_auth_triggered: false,
            active_session_tin: None,
        }
    }
    
    fn select_profile(&mut self, profile: TaxpayerProfile, action: ProfileTargetAction, window: &mut Window, cx: &mut Context<Self>) {
        let tin = profile.tin.full();

        // If this profile is already the active session, skip PIN entirely
        if self.hide_tax_profiles && self.active_session_tin.as_ref() == Some(&tin) {
            self.apply_profile_action(profile, action, window, cx);
            cx.notify();
            return;
        }

        // Switching profiles: set new session (old one is automatically replaced)
        if self.hide_tax_profiles {
            self.active_session_tin = Some(tin);
        }

        if self.enable_profile_pins && profile.profile_pin_hash.is_some() {
            self.pending_profile = Some((profile, action));
            self.profile_auth_error = None;
            self.os_auth_triggered = false;
            self.profile_otp_state.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.focus(window, cx);
            });
        } else {
            self.apply_profile_action(profile, action, window, cx);
        }
        cx.notify();
    }
    
    fn apply_profile_action(&mut self, profile: TaxpayerProfile, action: ProfileTargetAction, window: &mut Window, cx: &mut Context<Self>) {
        self.active_profile_tin = Some(profile.tin.full());

        // Keep dashboard in sync with hide_tax_profiles state
        let htp = self.hide_tax_profiles;
        self.dashboard_view.update(cx, |view, _cx| {
            view.hide_tax_profiles = htp;
        });

        match action {
            ProfileTargetAction::ViewDashboard => {
                self.active_view = ActiveView::Dashboard;
                let p = profile.clone();
                self.dashboard_view.update(cx, |view, cx| {
                    view.set_profile(p, cx);
                });
            }
            ProfileTargetAction::EditProfile => {
                self.active_view = ActiveView::ProfileManager;
                self.profile_manager.update(cx, |view, cx| {
                    view.edit_profile(profile.clone(), window, cx);
                });
            }
        }
    }
    
    fn request_admin_access(&mut self, target: ActiveView, window: &mut Window, cx: &mut Context<Self>) {
        let is_app_lock_enabled = if let Ok(db) = self.db.lock() {
            db.get_setting("app_lock_enabled").ok().flatten().as_deref() == Some("true")
        } else { false };
        
        if is_app_lock_enabled {
            self.pending_admin_view = Some(target);
            self.admin_auth_error = None;
            self.admin_os_auth_triggered = false;
            self.admin_otp_state.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.focus(window, cx);
            });
        } else {
            self.active_view = target;
            if target == ActiveView::CronTasks {
                self.cron_tasks_view.update(cx, |view, cx| view.load_settings(cx));
            }
        }
        cx.notify();
    }

    fn render_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_mini = self.is_mini_sidebar || window.viewport_size().width < px(768.);
        let filter = self.profile_filter.read(cx).value().to_lowercase();
        let mut archived_count = 0;

        for p in &self.profiles {
            if p.is_archived {
                archived_count += 1;
            }
        }

        let filtered_profiles: Vec<TaxpayerProfile> = self
            .profiles
            .iter()
            .filter(|profile| {
                if self.show_archived != profile.is_archived {
                    return false;
                }
                if self.hide_tax_profiles {
                    let is_active_session = self.active_session_tin.as_ref() == Some(&profile.tin.full());
                    // When filter is empty, show only the active session profile
                    if filter.trim().is_empty() {
                        return is_active_session;
                    }
                    // When typing, only exact full TIN match (digits or formatted) reveals a profile
                    let is_exact_tin_match = filter.trim() == profile.tin.full().to_lowercase()
                        || filter.trim() == profile.tin.formatted().to_lowercase();
                    is_active_session || is_exact_tin_match
                } else {
                    filter.trim().is_empty()
                        || profile
                            .full_name
                            .to_lowercase()
                            .contains(&filter.trim().to_lowercase())
                        || profile.tin.full().contains(filter.trim())
                        || profile.tin.formatted().contains(filter.trim())
                }
            })
            .cloned()
            .collect();
        div()
            .w(if is_mini { px(72.) } else { px(280.) })
            .h_full()
            .bg(cx.theme().background)
            .border_r_1()
            .border_color(cx.theme().border)
            .when(!is_mini, |this| this.py_6())
            .when(is_mini, |this| this.py_3())
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .when(!is_mini, |this| this.px_6())
                            .when(is_mini, |this| this.px_3())
                            .child(
                                div()
                                    .flex()
                                    .w_full()
                                    .when(!is_mini, |this| this.justify_end())
                                    .when(is_mini, |this| this.justify_center())
                                    .mb(px(10.))
                                    .child(
                                        div()
                                            .id("sidebar_toggle_btn")
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size(px(40.))
                                            .flex_shrink_0()
                                            .cursor_pointer()
                                            .hover(|s| s.bg(cx.theme().muted).rounded_md())
                                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                                this.is_mini_sidebar = !this.is_mini_sidebar;
                                                this.dashboard_view.update(cx, |view, cx| {
                                                    view.set_mini_sidebar(this.is_mini_sidebar, cx);
                                                });
                                                cx.notify();
                                            }))
                                            .child(
                                                Icon::new(if is_mini { IconName::ChevronRight } else { IconName::ChevronLeft })
                                                    .size(px(28.))
                                                    .text_color(cx.theme().foreground)
                                            )
                                    )
                            )
                            .child(
                                div()
                                    .id("global_dashboard_btn")
                                    .w_full()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.active_view = ActiveView::GlobalDashboard;
                                        this.active_profile_tin = None;
                                        cx.notify();
                                    }))
                                    .child(
                                        if is_mini {
                                            gpui::img("svg/e_logo.svg")
                                                .w_full()
                                                .h_8()
                                                .object_fit(gpui::ObjectFit::Contain)
                                        } else {
                                            gpui::img("svg/ebirforms.png")
                                                .w_full()
                                                .h_10()
                                                .object_fit(gpui::ObjectFit::Contain)
                                        }
                                    )
                            ),
                    )
                    .child(
                            div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .when(!is_mini, |this| this.px_6())
                            .when(is_mini, |this| this.px_3())
                            .when(!is_mini, |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(cx.theme().muted_foreground)
                                        .child("TAXPAYER PROFILES"),
                                )
                            })
                            .when(is_mini, |this| this.justify_center())
                            .child(
                                div()
                                    .id("add_profile_mini_btn")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(px(40.))
                                    .flex_shrink_0()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(cx.theme().muted).rounded_md())
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        // End any active session when creating a new profile
                                        this.active_session_tin = None;
                                        this.active_view = ActiveView::ProfileManager;
                                        this.active_profile_tin = None;
                                        this.profile_manager.update(cx, |view, cx| {
                                            view.reset_for_new(_window, cx);
                                        });
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .text_color(cx.theme().foreground)
                                            .font_weight(FontWeight::BOLD)
                                            .text_xl()
                                            .child("+"),
                                    )
                            ),
                    )
                    .child(
                        if is_mini {
                            div().px_3().flex().justify_center().w_full().child(
                                div()
                                    .id("sidebar_search_mini_btn")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(px(48.))
                                    .rounded_full()
                                    .bg(cx.theme().secondary)
                                    .flex_shrink_0()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(cx.theme().muted))
                                    .on_click(cx.listener(|this, _ev, window, cx| {
                                        this.is_mini_sidebar = false;
                                        this.dashboard_view.update(cx, |view, cx| {
                                            view.set_mini_sidebar(this.is_mini_sidebar, cx);
                                        });
                                        cx.focus_view(&this.profile_filter, window);
                                        cx.notify();
                                    }))
                                    .child(
                                        Icon::new(IconName::Search)
                                            .size(px(20.))
                                            .text_color(cx.theme().foreground)
                                    )
                            ).into_any_element()
                        } else {
                            div().px_6().w_full().child(Input::new(&self.profile_filter)).into_any_element()
                        }
                    )
                    .child(div().h_2())
                    .child(
                        div()
                            .w_full()
                            .h_px()
                            .bg(cx.theme().border)
                    )
                    .child(
                        v_flex()
                            .id("sidebar-profile-list")
                            .max_h(px(320.))
                            .overflow_y_scroll()
                            .track_scroll(&self.sidebar_scroll)
                            .pb_2()
                            .pt_2()
                            .gap_2()
                            .children(filtered_profiles.iter().map(|profile| {
                                let is_active =
                                    self.active_profile_tin.as_ref() == Some(&profile.tin.full());
                                let is_expanded =
                                    self.expanded_profile_tin.as_ref() == Some(&profile.tin.full());
                                let bg_color = if is_active {
                                    cx.theme().accent
                                } else {
                                    gpui::rgba(0x00000000).into()
                                };
                                let border_color = if is_active {
                                    cx.theme().border
                                } else {
                                    gpui::rgba(0x00000000).into()
                                };

                                let rdo_desc = bir_core::reference::get_rdo(&profile.rdo_code)
                                    .map(|r| r.description.clone())
                                    .unwrap_or_else(|| "Unknown".to_string());

                                div()
                                    .id(profile.tin.full())
                                    .w_full()
                                    .when(!is_mini, |this| this.py_2().px_6())
                                    .when(is_mini, |this| this.py_2().px_3())
                                    .bg(bg_color)
                                    .border_1()
                                    .border_color(border_color)
                                    .flex()
                                    .flex_col()
                                    .cursor_pointer()
                                    .hover(|s| {
                                        if !is_active {
                                            s.bg(cx.theme().muted)
                                        } else {
                                            s
                                        }
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .when(is_expanded && !is_mini, |d| d.mb_4())
                                            .when(is_mini, |this| this.justify_center())
                                            .when(is_mini, |this| {
                                                let initials = profile.full_name
                                                    .split_whitespace()
                                                    .filter_map(|w| w.chars().next())
                                                    .take(2)
                                                    .collect::<String>()
                                                    .to_uppercase();
                                                
                                                this.child(
                                                    div()
                                                        .size(px(48.))
                                                        .flex_shrink_0()
                                                        .rounded_full()
                                                        .bg(cx.theme().secondary)
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .text_color(cx.theme().primary)
                                                                .font_weight(FontWeight::BOLD)
                                                                .child(initials)
                                                        )
                                                )
                                            })
                                            .when(!is_mini, |this| {
                                                this.child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .child(
                                                            div()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(if profile.is_archived { cx.theme().muted_foreground } else { cx.theme().foreground })
                                                                .child(if profile.is_archived { format!("{} (Archived)", profile.full_name) } else { profile.full_name.clone() }),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .text_color(cx.theme().muted_foreground)
                                                                .child(format!("TIN: {}", profile.tin.full())),
                                                        ),
                                                )
                                            }),
                                    )
                                    .when(is_expanded && !is_mini, |this| {
                                        this.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .justify_between()
                                                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child("RDO:"))
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(cx.theme().foreground)
                                                                .child(format!("{} - {}", profile.rdo_code, rdo_desc)),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .justify_between()
                                                        .gap_2()
                                                        .mt_2()
                                                        .when(!profile.is_archived, |this| {
                                                            this.child(
                                                                gpui_component::button::Button::new(format!("view_{}", profile.tin.full()))
                                                                    .small()
                                                                    .label("View")
                                                                    .on_click(cx.listener({
                                                                        let profile_clone = profile.clone();
                                                                        move |this, _ev, window, cx| {
                                                                            cx.stop_propagation();
                                                                            this.select_profile(profile_clone.clone(), ProfileTargetAction::ViewDashboard, window, cx);
                                                                        }
                                                                    })),
                                                            )
                                                            .child(
                                                                gpui_component::button::Button::new(format!("edit_{}", profile.tin.full()))
                                                                    .small()
                                                                    .label("Edit")
                                                                    .on_click(cx.listener({
                                                                        let profile_clone = profile.clone();
                                                                        move |this, _ev, window, cx| {
                                                                            cx.stop_propagation();
                                                                            this.select_profile(profile_clone.clone(), ProfileTargetAction::EditProfile, window, cx);
                                                                        }
                                                                    })),
                                                            )
                                                            .child(
                                                                gpui_component::button::Button::new(format!("archive_{}", profile.tin.full()))
                                                                    .small()
                                                                    .label("Archive")
                                                                    .on_click(cx.listener({
                                                                        let profile_clone = profile.clone();
                                                                        let tin = profile_clone.tin.full();
                                                                        move |this, _ev, _window, cx| {
                                                                            cx.stop_propagation();
                                                                            let mut profile_mut = profile_clone.clone();
                                                                            profile_mut.is_archived = true;
                                                                            if let Ok(db) = this.db.lock() {
                                                                                let _ = db.save_profile(profile_mut);
                                                                            }
                                                                            if let Some(p) = this.profiles.iter_mut().find(|p| p.tin.full() == tin) {
                                                                                p.is_archived = true;
                                                                            }
                                                                            cx.notify();
                                                                        }
                                                                    })),
                                                            )
                                                        })
                                                        .when(profile.is_archived, |this| {
                                                            this.child(
                                                                gpui_component::button::Button::new(format!("restore_{}", profile.tin.full()))
                                                                    .small()
                                                                    .label("Restore")
                                                                    .on_click(cx.listener({
                                                                        let profile_clone = profile.clone();
                                                                        let tin = profile_clone.tin.full();
                                                                        move |this, _ev, _window, cx| {
                                                                            cx.stop_propagation();
                                                                            let mut profile_mut = profile_clone.clone();
                                                                            profile_mut.is_archived = false;
                                                                            if let Ok(db) = this.db.lock() {
                                                                                let _ = db.save_profile(profile_mut);
                                                                            }
                                                                            if let Some(p) = this.profiles.iter_mut().find(|p| p.tin.full() == tin) {
                                                                                p.is_archived = false;
                                                                            }
                                                                            cx.notify();
                                                                        }
                                                                    })),
                                                            )
                                                            .child(
                                                                gpui_component::button::Button::new(format!("delete_{}", profile.tin.full()))
                                                                    .small()
                                                                    .label("Delete")
                                                                    .on_click(cx.listener({
                                                                        let profile_clone = profile.clone();
                                                                        let tin = profile_clone.tin.full();
                                                                        move |_this, _ev, _window, cx| {
                                                                            cx.stop_propagation();
                                                                            cx.spawn({
                                                                                let tin = tin.clone();
                                                                                async move |this, cx| {
                                                                                    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                                                                                    let Some(export_handle) = rfd::AsyncFileDialog::new()
                                                                                        .set_title("Save Profile Archive")
                                                                                        .set_file_name(&format!("BIR_Archive_{}_{}.zip", tin, timestamp))
                                                                                        .add_filter("Zip Archive", &["zip"])
                                                                                        .save_file()
                                                                                        .await
                                                                                    else {
                                                                                        return;
                                                                                    };
                                                                                    let export_dir = export_handle.path().to_path_buf();
                                                                                    
                                                                                    let success = this.update(cx, |this, cx| {
                                                                                        if let Ok(db) = this.db.lock() {
                                                                                            if let Err(e) = bir_core::export_profile_data(&db, &tin, &export_dir) {
                                                                                                println!("Export failed: {}", e);
                                                                                            } else {
                                                                                                let _ = db.delete_profile(&tin);
                                                                                                this.profiles.retain(|p| p.tin.full() != tin);
                                                                                                
                                                                                                if !this.profiles.iter().any(|p| p.is_archived) {
                                                                                                    this.show_archived = false;
                                                                                                }
                                                                                                
                                                                                                if this.active_profile_tin.as_ref() == Some(&tin) {
                                                                                                    this.active_profile_tin = None;
                                                                                                    this.active_view = ActiveView::ProfileManager;
                                                                                                }
                                                                                                if this.expanded_profile_tin.as_ref() == Some(&tin) {
                                                                                                    this.expanded_profile_tin = None;
                                                                                                }
                                                                                                cx.notify();
                                                                                                return true;
                                                                                            }
                                                                                        }
                                                                                        false
                                                                                    });
                                                                                    
                                                                                    if let Ok(true) = success {
                                                                                        rfd::AsyncMessageDialog::new()
                                                                                            .set_title("Profile Exported & Deleted")
                                                                                            .set_description(&format!("Saved to {}", export_dir.display()))
                                                                                            .show()
                                                                                            .await;
                                                                                    }
                                                                                }
                                                                            }).detach();
                                                                        }
                                                                    })),
                                                            )
                                                        })
                                                )
                                        )
                                        .on_mouse_down_out(cx.listener(|this, _ev, _window, cx| {
                                            this.expanded_profile_tin = None;
                                            cx.notify();
                                        }))
                                    })
                                    .on_click(cx.listener({
                                        let profile_clone = profile.clone();
                                        let tin = profile_clone.tin.full();
                                        move |this, _ev, window, cx| {
                                            if this.is_mini_sidebar || window.viewport_size().width < px(768.) {
                                                this.select_profile(profile_clone.clone(), ProfileTargetAction::ViewDashboard, window, cx);
                                            } else {
                                                if this.expanded_profile_tin.as_ref() == Some(&tin) {
                                                    this.expanded_profile_tin = None;
                                                } else {
                                                    this.expanded_profile_tin = Some(tin.clone());
                                                }
                                            }
                                            cx.notify();
                                        }
                                    }))
                            }))
                            .when(archived_count > 0 || self.show_archived, |this| {
                                this.child(
                                    div()
                                        .w_full()
                                        .py_2()
                                        .when(!is_mini, |this| this.px_6())
                                        .when(is_mini, |this| this.px_3())
                                        .flex()
                                        .justify_center()
                                        .child(
                                            if is_mini {
                                                div()
                                                    .id("toggle_archived_mini_btn")
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .size(px(48.))
                                                    .flex_shrink_0()
                                                    .rounded_full()
                                                    .bg(cx.theme().secondary)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(cx.theme().muted))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.show_archived = !this.show_archived;
                                                        cx.notify();
                                                    }))
                                                    .child(Icon::new(if self.show_archived { IconName::EyeOff } else { IconName::Eye }).size(px(20.)).text_color(cx.theme().foreground))
                                                    .into_any_element()
                                            } else {
                                                gpui_component::button::Button::new("toggle_archived")
                                                    .small()
                                                    .label(if self.show_archived { "Hide Archived Profiles".to_string() } else { format!("Show {} Archived", archived_count) })
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.show_archived = !this.show_archived;
                                                        cx.notify();
                                                    }))
                                                    .into_any_element()
                                            }
                                        )
                                )
                            })
                    )
                    .when(self.hide_tax_profiles && self.active_session_tin.is_some(), |this| {
                        this.child(
                            div()
                                .w_full()
                                .py_2()
                                .when(!is_mini, |this| this.px_6())
                                .when(is_mini, |this| this.px_3())
                                .flex()
                                .justify_center()
                                .child(
                                    if is_mini {
                                        div()
                                            .id("exit_session_mini_btn")
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size(px(48.))
                                            .flex_shrink_0()
                                            .rounded_full()
                                            .bg(gpui::rgba(0xef444420))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(gpui::rgba(0xef444440)))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.active_session_tin = None;
                                                this.active_profile_tin = None;
                                                this.active_view = ActiveView::GlobalDashboard;
                                                cx.notify();
                                            }))
                                            .child(Icon::new(IconName::CircleX).size(px(20.)).text_color(Hsla::from(gpui::rgba(0xef4444ff))))
                                            .into_any_element()
                                    } else {
                                        gpui_component::button::Button::new("exit_session")
                                            .small()
                                            .label("Exit Session")
                                            .icon(IconName::CircleX)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.active_session_tin = None;
                                                this.active_profile_tin = None;
                                                this.active_view = ActiveView::GlobalDashboard;
                                                cx.notify();
                                            }))
                                            .into_any_element()
                                    }
                                )
                        )
                    })
                    .child(
                        div()
                            .w_full()
                            .h_px()
                            .bg(cx.theme().border)
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .w_full()
                    .when(!is_mini, |this| this.px_6())
                    .when(is_mini, |this| this.px_3())
                    .child(
                        div()
                            .id("import_export_sidebar_btn")
                            .flex()
                            .items_center()
                            .w_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .when(is_mini, |this| {
                                this.justify_center().size(px(48.)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h_10().px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.active_view = ActiveView::ImportExport;
                                cx.notify();
                            }))
                            .child(Icon::new(IconName::HardDrive).size(px(20.)).text_color(cx.theme().foreground))
                            .when(!is_mini, |this| {
                                this.child(div().text_sm().text_color(cx.theme().foreground).child("Import Data"))
                            })
                    )
                    .child(
                        div()
                            .id("theme_toggle_sidebar_btn")
                            .flex()
                            .items_center()
                            .w_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .when(is_mini, |this| {
                                this.justify_center().size(px(48.)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h_10().px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.theme_preference = this.theme_preference.next();
                                if let Ok(db) = this.db.lock() {
                                    if let Ok(val) = serde_json::to_string(&this.theme_preference) {
                                        let _ = db.set_setting("theme_preference", &val);
                                    }
                                }
                                let target_mode = match this.theme_preference {
                                    AppThemeMode::Light => ThemeMode::Light,
                                    AppThemeMode::Dark => ThemeMode::Dark,
                                    AppThemeMode::System => match window.appearance() {
                                        gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight => ThemeMode::Light,
                                        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark => ThemeMode::Dark,
                                    },
                                };
                                Theme::change(target_mode, Some(window), cx);
                                cx.notify();
                            }))
                            .child(match self.theme_preference {
                                AppThemeMode::Dark => Icon::new(IconName::Moon).size(px(20.)).text_color(cx.theme().foreground),
                                AppThemeMode::Light => Icon::new(IconName::Sun).size(px(20.)).text_color(cx.theme().foreground),
                                AppThemeMode::System => Icon::new(IconName::Settings).size(px(20.)).text_color(cx.theme().foreground),
                            })
                            .when(!is_mini, |this| {
                                this.child(div().text_sm().text_color(cx.theme().foreground).child(match self.theme_preference {
                                    AppThemeMode::Dark => "Dark Mode",
                                    AppThemeMode::Light => "Light Mode",
                                    AppThemeMode::System => "System Theme",
                                }))
                            })
                    )
                    .child(
                        div()
                            .id("cron_tasks_sidebar_btn")
                            .flex()
                            .items_center()
                            .w_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .when(is_mini, |this| {
                                this.justify_center().size(px(48.)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h_10().px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.request_admin_access(ActiveView::CronTasks, window, cx);
                            }))
                            .child(Icon::new(IconName::SquareTerminal).size(px(20.)).text_color(cx.theme().foreground))
                            .when(!is_mini, |this| {
                                this.child(div().text_sm().text_color(cx.theme().foreground).child("Background Tasks"))
                            })
                    )
                    .child(
                        div()
                            .id("settings_sidebar_btn")
                            .flex()
                            .items_center()
                            .w_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .when(is_mini, |this| {
                                this.justify_center().size(px(48.)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h_10().px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.request_admin_access(ActiveView::Settings, window, cx);
                            }))
                            .child(Icon::new(IconName::Settings).size(px(20.)).text_color(cx.theme().foreground))
                            .when(!is_mini, |this| {
                                this.child(div().text_sm().text_color(cx.theme().foreground).child("Settings"))
                            })
                    )
            )
    }

    fn render_active_view(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        match self.active_view {
            ActiveView::GlobalDashboard => self.global_dashboard_view.clone().into_any_element(),
            ActiveView::ProfileManager => self.profile_manager.clone().into_any_element(),
            ActiveView::CronTasks => self.cron_tasks_view.clone().into_any_element(),
            ActiveView::ImportExport => self.import_export_view.clone().into_any_element(),
            ActiveView::Settings => self.settings_view.clone().into_any_element(),
            ActiveView::Dashboard => self.dashboard_view.clone().into_any_element(),
            ActiveView::Form2551Q => {
                if let Some(view) = &self.form_2551q_view {
                    view.clone().into_any_element()
                } else {
                    div().child("No form loaded").into_any_element()
                }
            }
        }
    }

    fn handle_file_form(&mut self, event: &DashboardEvent, cx: &mut Context<Self>) {
        let (form_code, year, quarter) = match event {
            DashboardEvent::FileForm {
                form_code,
                year,
                quarter,
            } => (form_code, year, quarter),
            _ => return,
        };
        let year = *year;
        let quarter = *quarter;

        if form_code == "2551Q"
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            let draft = if let Ok(db) = self.db.lock() {
                let existing = db.get_2551q_draft(tin, year, quarter).ok().flatten();
                if let Some(d) = existing {
                    d
                } else {
                    let prev = if quarter > 1 {
                        db.get_2551q_draft(tin, year, quarter - 1).ok().flatten()
                    } else {
                        None
                    };
                    let new_draft = Form2551QDraft::new_from_profile(profile, year, quarter);
                    if let Some(prev_draft) = prev {
                        new_draft.with_carried_forward(&prev_draft)
                    } else {
                        new_draft
                    }
                }
            } else {
                Form2551QDraft::new_from_profile(profile, year, quarter)
            };
            // Store the draft; the view will be created on next render with Window access
            self.pending_form_draft = Some(draft);
            self.active_view = ActiveView::Form2551Q;
            cx.notify();
        }
    }
}

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Set up window-aware subscription for global dashboard notifications (once)
        if !self.global_dashboard_notif_subscribed {
            self.global_dashboard_notif_subscribed = true;
            cx.subscribe_in(
                &self.global_dashboard_view,
                window,
                |_this: &mut Self, _entity, event: &GlobalDashboardEvent, window, cx| {
                    if let GlobalDashboardEvent::PushNotification(level, title, message) = event {
                        use gpui_component::WindowExt;
                        use gpui_component::notification::Notification;
                        let notification = match level.as_str() {
                            "success" => {
                                Notification::success(title.clone()).message(message.clone())
                            }
                            "error" => Notification::error(title.clone()).message(message.clone()),
                            _ => Notification::info(title.clone()).message(message.clone()),
                        };
                        window.push_notification(notification, cx);
                    }
                },
            )
            .detach();
        }

        // Materialize any pending form view now that we have `window`
        if let Some(draft) = self.pending_form_draft.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| Form2551QView::new(draft, db_for_view, window, cx));

            cx.subscribe_in(
                &form_view,
                window,
                |this: &mut Self, _entity, event: &Form2551QEvent, window, cx| match event {
                    Form2551QEvent::BackToDashboard => {
                        this.active_view = ActiveView::Dashboard;
                        if let Some(tin) = &this.active_profile_tin
                            && let Some(profile) =
                                this.profiles.iter().find(|p| p.tin.full() == *tin)
                        {
                            let p = profile.clone();
                            this.dashboard_view.update(cx, |view, cx| {
                                view.set_profile(p, cx);
                            });
                        }
                        cx.notify();
                    }
                    Form2551QEvent::PushNotification(level, title, message) => {
                        use gpui_component::WindowExt;
                        use gpui_component::notification::Notification;
                        let notification = match level.as_str() {
                            "success" => {
                                Notification::success(title.clone()).message(message.clone())
                            }
                            "error" => Notification::error(title.clone()).message(message.clone()),
                            _ => Notification::info(title.clone()).message(message.clone()),
                        };
                        window.push_notification(notification, cx);
                    }
                    Form2551QEvent::Saved
                    | Form2551QEvent::Submitted
                    | Form2551QEvent::Confirmed => {
                        cx.notify();
                    }
                },
            )
            .detach();

            self.form_2551q_view = Some(form_view);
        }

        if let Some((profile, action)) = self.unlocked_profile.take() {
            self.apply_profile_action(profile, action, window, cx);
            self.focus_handle.focus(window, cx);
        }

        let notification_layer = Root::render_notification_layer(window, cx);

        if self.is_locked {
            if let Some(lock_screen) = &self.lock_screen_view {
                return div().size_full().child(lock_screen.clone()).into_any_element();
            }
        }

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .on_action(cx.listener(
                |this: &mut Self, _action: &crate::global_actions::ToggleSidebar, window, cx| {
                    this.is_sidebar_hidden = !this.is_sidebar_hidden;
                    if this.is_sidebar_hidden {
                        this.focus_handle.focus(window, cx);
                    }
                    cx.notify();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self,
                 _action: &crate::global_actions::ToggleSidebarMini,
                 _window,
                 cx| {
                    this.is_mini_sidebar = !this.is_mini_sidebar;
                    this.dashboard_view.update(cx, |view, cx| {
                        view.set_mini_sidebar(this.is_mini_sidebar, cx);
                    });
                    cx.notify();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _action: &crate::global_actions::FocusSearch, window, cx| {
                    this.is_sidebar_hidden = false;
                    this.is_mini_sidebar = false;
                    this.dashboard_view.update(cx, |view, cx| {
                        view.set_mini_sidebar(this.is_mini_sidebar, cx);
                    });
                    cx.focus_view(&this.profile_filter, window);
                    cx.notify();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _action: &crate::global_actions::CreateProfile, window, cx| {
                    this.active_view = ActiveView::ProfileManager;
                    this.active_profile_tin = None;
                    this.profile_manager.update(cx, |view, cx| {
                        view.reset_for_new(window, cx);
                    });
                    cx.notify();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _action: &crate::global_actions::ToggleTheme, window, cx| {
                    this.theme_preference = this.theme_preference.next();
                    if let Ok(db) = this.db.lock() {
                        if let Ok(val) = serde_json::to_string(&this.theme_preference) {
                            let _ = db.set_setting("theme_preference", &val);
                        }
                    }
                    let target_mode = match this.theme_preference {
                        AppThemeMode::Light => ThemeMode::Light,
                        AppThemeMode::Dark => ThemeMode::Dark,
                        AppThemeMode::System => match window.appearance() {
                            gpui::WindowAppearance::Light
                            | gpui::WindowAppearance::VibrantLight => ThemeMode::Light,
                            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark => {
                                ThemeMode::Dark
                            }
                        },
                    };
                    Theme::change(target_mode, Some(window), cx);
                    cx.notify();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _action: &crate::global_actions::OpenCronTasks, _window, cx| {
                    this.active_view = ActiveView::CronTasks;
                    this.cron_tasks_view.update(cx, |view, cx| {
                        view.load_settings(cx);
                    });
                    cx.notify();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _action: &crate::global_actions::OpenSettings, window, cx| {
                    this.request_admin_access(ActiveView::Settings, window, cx);
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _action: &crate::global_actions::OpenGlobalDashboard, _window, cx| {
                    this.active_view = ActiveView::GlobalDashboard;
                    this.active_profile_tin = None;
                    cx.notify();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _action: &crate::global_actions::OpenCommandPalette, window, cx| {
                    this.is_command_palette_open = true;
                    let profiles = this.profiles.clone();
                    let hide_tax_profiles = this.hide_tax_profiles;
                    let active_session_tin = this.active_session_tin.clone();
                    let palette = cx.new(|cx| crate::components::command_palette::CommandPalette::new(profiles, hide_tax_profiles, active_session_tin, window, cx));
                    
                    cx.subscribe_in(&palette, window, |this: &mut Self, _, event: &crate::components::command_palette::CommandPaletteEvent, window, cx| {
                        match event {
                            crate::components::command_palette::CommandPaletteEvent::SelectProfile(tin) => {
                                if let Some(profile) = this.profiles.iter().find(|p| p.tin.full() == *tin).cloned() {
                                    if this.hide_tax_profiles {
                                        this.active_session_tin = Some(tin.clone());
                                    }
                                    this.select_profile(profile, ProfileTargetAction::ViewDashboard, window, cx);
                                }
                                this.is_command_palette_open = false;
                                if this.pending_profile.is_none() {
                                    this.focus_handle.focus(window, cx);
                                }
                                cx.notify();
                            }
                            crate::components::command_palette::CommandPaletteEvent::CreateProfile(query) => {
                                this.is_command_palette_open = false;
                                this.active_view = ActiveView::ProfileManager;
                                this.profile_manager.update(cx, |view, cx| {
                                    view.reset_for_new(window, cx);
                                    view.prefill_name(&query, window, cx);
                                });
                                // Focus is already set to the name input by prefill_name
                                cx.notify();
                            }
                            crate::components::command_palette::CommandPaletteEvent::EditProfile(tin) => {
                                if let Some(profile) = this.profiles.iter().find(|p| p.tin.full() == *tin).cloned() {
                                    if this.hide_tax_profiles {
                                        this.active_session_tin = Some(tin.clone());
                                    }
                                    this.select_profile(profile, ProfileTargetAction::EditProfile, window, cx);
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
                    }).detach();
                    
                    this.command_palette_view = Some(palette.clone());
                    palette.update(cx, |view, cx| {
                        view.focus_input(window, cx);
                    });
                    cx.notify();
                },
            ))
            .on_action(cx.listener(
                |_this: &mut Self, _action: &crate::global_actions::QuitApplication, _window, cx| {
                    cx.quit();
                },
            ))
            .on_action(cx.listener(
                |_this: &mut Self, _action: &crate::global_actions::HideApplication, _window, cx| {
                    cx.hide();
                },
            ))
            .on_action(cx.listener(
                |_this: &mut Self, _action: &crate::global_actions::HideOthers, _window, cx| {
                    // Hide other applications (macOS mainly)
                    cx.hide_other_apps();
                },
            ))
            .on_action(cx.listener(
                |_this: &mut Self, _action: &crate::global_actions::CloseWindow, window, _cx| {
                    window.remove_window();
                },
            ))
            .on_action(cx.listener(
                |_this: &mut Self, _action: &crate::global_actions::MinimizeWindow, window, _cx| {
                    window.minimize_window();
                },
            ))
            .on_action(cx.listener(
                |_this: &mut Self, _action: &crate::global_actions::ToggleFullScreen, window, _cx| {
                    window.toggle_fullscreen();
                },
            ))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .when(!self.is_sidebar_hidden, |this| {
                        this.child(self.render_sidebar(window, cx))
                    })
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .h_full()
                            .overflow_hidden()
                            .child(self.render_active_view(cx)),
                    ),
            )
            .child(crate::components::footer::render_footer(cx))
            .children(notification_layer)
            .when_some(self.pending_profile.clone(), |this, pending| {
                let (profile, _) = pending;
                let error_msg = self.profile_auth_error.clone();
                let os_triggered = self.os_auth_triggered;
                
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(cx.theme().background)
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_6()
                                .child(
                                    gpui::img(std::path::PathBuf::from("assets/svg/ebirforms.png"))
                                        .w(px(200.))
                                        .h(px(60.))
                                        .object_fit(gpui::ObjectFit::Contain)
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xl()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().foreground)
                                                .child("Enter PIN to unlock Tax Profile")
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("Profile: {}", profile.tin.full()))
                                        )
                                )
                                .child(
                                    OtpInput::new(&self.profile_otp_state)
                                        .groups(1)
                                        .large()
                                        .disabled(os_triggered)
                                )
                                .when_some(error_msg, |this, msg| {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().danger)
                                            .child(msg)
                                    )
                                })
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_4()
                                        .child(
                                            gpui_component::button::Button::new("admin_override")
                                                .label(if os_triggered { "Waiting for OS..." } else { "Admin Override" })
                                                .ghost()
                                                .disabled(os_triggered)
                                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                                    if this.os_auth_triggered { return; }
                                                    this.os_auth_triggered = true;
                                                    this.profile_auth_error = None;
                                                    
                                                    cx.spawn(async move |this, cx| {
                                                        use robius_authentication::{BiometricStrength, Context, PolicyBuilder, Text, AndroidText, WindowsText};
                                                        let policy = match PolicyBuilder::new()
                                                            .biometrics(Some(BiometricStrength::Strong))
                                                            .password(true)
                                                            .watch(true)
                                                            .build() {
                                                                Some(p) => p,
                                                                None => return,
                                                        };

                                                        let text = Text {
                                                            android: AndroidText {
                                                                title: "Admin Override",
                                                                subtitle: None,
                                                                description: None,
                                                            },
                                                            apple: "Admin Override to Unlock Profile",
                                                            windows: WindowsText::new_truncated("Admin Override", "Authenticate to override profile PIN."),
                                                        };

                                                        let success = Context::new(())
                                                            .authenticate(text, &policy)
                                                            .await
                                                            .is_ok();
                                                        
                                                        let _ = this.update(cx, |this, cx| {
                                                            this.os_auth_triggered = false;
                                                            if success {
                                                                if let Some((p, a)) = this.pending_profile.take() {
                                                                    this.unlocked_profile = Some((p, a));
                                                                }
                                                            } else {
                                                                this.profile_auth_error = Some("Admin Authentication failed or canceled.".to_string());
                                                            }
                                                            cx.notify();
                                                        });
                                                    }).detach();
                                                }))
                                        )
                                        .child(
                                            gpui_component::button::Button::new("cancel_profile_pin")
                                                .label("Cancel")
                                                .ghost()
                                                .small()
                                                .disabled(os_triggered)
                                                .on_click(cx.listener(|this, _ev, window, cx| {
                                                    this.pending_profile = None;
                                                    this.profile_otp_state.update(cx, |input, cx| input.set_value("", window, cx));
                                                    this.focus_handle.focus(window, cx);
                                                    cx.notify();
                                                }))
                                        )
                                )
                        )
                )
            })
            .when_some(self.pending_admin_view, |this, target| {
                let error_msg = self.admin_auth_error.clone();
                let os_triggered = self.admin_os_auth_triggered;
                
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(cx.theme().background)
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_6()
                                .child(
                                    gpui::img(std::path::PathBuf::from("assets/svg/ebirforms.png"))
                                        .w(px(200.))
                                        .h(px(60.))
                                        .object_fit(gpui::ObjectFit::Contain)
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xl()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().foreground)
                                                .child("Admin Access Required")
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(match target {
                                                    ActiveView::Settings => "Enter App Lock PIN to access Settings",
                                                    ActiveView::CronTasks => "Enter App Lock PIN to access Background Tasks",
                                                    _ => "Enter App Lock PIN to continue",
                                                })
                                        )
                                )
                                .child(
                                    OtpInput::new(&self.admin_otp_state)
                                        .groups(1)
                                        .large()
                                        .disabled(os_triggered)
                                )
                                .when_some(error_msg, |this, msg| {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().danger)
                                            .child(msg)
                                    )
                                })
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_4()
                                        .child(
                                            gpui_component::button::Button::new("admin_os_override")
                                                .label(if os_triggered { "Waiting for OS..." } else { "OS Auth Override" })
                                                .ghost()
                                                .disabled(os_triggered)
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    if this.admin_os_auth_triggered { return; }
                                                    this.admin_os_auth_triggered = true;
                                                    this.admin_auth_error = None;
                                                    
                                                    cx.spawn(async move |this, cx| {
                                                        use robius_authentication::{BiometricStrength, Context, PolicyBuilder, Text, AndroidText, WindowsText};
                                                        let policy = match PolicyBuilder::new()
                                                            .biometrics(Some(BiometricStrength::Strong))
                                                            .password(true)
                                                            .watch(true)
                                                            .build() {
                                                                Some(p) => p,
                                                                None => return,
                                                        };

                                                        let text = Text {
                                                            android: AndroidText {
                                                                title: "Admin Override",
                                                                subtitle: None,
                                                                description: None,
                                                            },
                                                            apple: "OS Authentication to Override App Lock",
                                                            windows: WindowsText::new_truncated("Admin Override", "Authenticate to override App Lock PIN."),
                                                        };

                                                        let success = Context::new(())
                                                            .authenticate(text, &policy)
                                                            .await
                                                            .is_ok();
                                                        
                                                        let _ = this.update(cx, |this, cx| {
                                                            this.admin_os_auth_triggered = false;
                                                            if success {
                                                                if let Some(target) = this.pending_admin_view.take() {
                                                                    this.active_view = target;
                                                                    if target == ActiveView::CronTasks {
                                                                        this.cron_tasks_view.update(cx, |view, cx| view.load_settings(cx));
                                                                    }
                                                                }
                                                            } else {
                                                                this.admin_auth_error = Some("OS Authentication failed or canceled.".to_string());
                                                            }
                                                            cx.notify();
                                                        });
                                                    }).detach();
                                                }))
                                        )
                                        .child(
                                            gpui_component::button::Button::new("cancel_admin_pin")
                                                .label("Cancel")
                                                .ghost()
                                                .small()
                                                .disabled(os_triggered)
                                                .on_click(cx.listener(|this, _ev, window, cx| {
                                                    this.pending_admin_view = None;
                                                    this.admin_otp_state.update(cx, |input, cx| input.set_value("", window, cx));
                                                    this.focus_handle.focus(window, cx);
                                                    cx.notify();
                                                }))
                                        )
                                )
                        )
                )
            })
            .when(self.is_command_palette_open, |this| {
                if let Some(palette) = &self.command_palette_view {
                    this.child(palette.clone())
                } else {
                    this
                }
            })
            .into_any_element()
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        // Flush any pending WAL data to the main database file before shutdown
        if let Ok(db) = self.db.lock()
            && let Err(e) = db.checkpoint()
        {
            eprintln!("Warning: WAL checkpoint on shutdown failed: {e}");
        }
    }
}
