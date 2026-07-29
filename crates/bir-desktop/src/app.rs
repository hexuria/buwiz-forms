use crate::views::cron_tasks::CronTasksView;
use crate::views::dashboard::{DashboardEvent, DashboardView};
use crate::views::form_0605_view::{Form0605Event, Form0605View};
use crate::views::form_0619e_view::{Form0619EEvent, Form0619EView};
use crate::views::form_0619f_view::{Form0619FEvent, Form0619FView};
use crate::views::form_1601c_view::{Form1601CEvent, Form1601CView};
use crate::views::form_1701_view::{Form1701Event, Form1701View};
use crate::views::form_1701q_view::{Form1701QEvent, Form1701QView};
use crate::views::form_1702mx_view::{Form1702MXEvent, Form1702MXView};
use crate::views::form_1702rt_view::{Form1702RTEvent, Form1702RTView};
use crate::views::form_2550q_view::{Form2550QV2Event, Form2550QV2View};
use crate::views::form_2551q_view::{Form2551QEvent, Form2551QView};
use crate::views::global_dashboard::{GlobalDashboardEvent, GlobalDashboardView};
use crate::views::import_export::{ImportExportEvent, ImportExportView};
use crate::views::lock_screen::{LockScreenEvent, LockScreenView};
use crate::views::profile_manager::ProfileManagerView;
use crate::views::settings::{SettingsEvent, SettingsView};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{InputEvent, InputState, OtpState};
use gpui_component::*;
use gpui_rsx::rsx;

use crate::components::rate_limiter::RateLimiter;
use bir_core::db::Database;
use bir_core::forms::FilingStatus;
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
    Form1701Q,
    Form1601C,
    Form0619E,
    Form0619F,
    Form0605,
    Form2550Q,
    Form1701,
    Form1702RT,
    Form1702MX,
    ProfileManager,
    CronTasks,
    ImportExport,
    Settings,
    AdminCalendarDashboard,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProfileTargetAction {
    ViewDashboard,
    EditProfile,
    #[allow(dead_code)]
    UnlockOnly,
}

/// A change to whether a taxpayer profile exists and is listed. These are the
/// administrator-only actions: they are requested from the profile editor and
/// always pass through `AppState::request_profile_lifecycle`, never applied
/// directly by the view that asked for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileLifecycleAction {
    Archive,
    Restore,
    Delete,
}

impl ProfileLifecycleAction {
    /// Past-tense verb used in the notifications these actions push.
    pub(crate) fn past_tense(self) -> &'static str {
        match self {
            Self::Archive => "archived",
            Self::Restore => "restored",
            Self::Delete => "deleted",
        }
    }
}

pub struct AppState {
    pub(crate) active_view: ActiveView,
    pub(crate) profile_manager: Entity<ProfileManagerView>,
    pub(crate) dashboard_view: Entity<DashboardView>,
    pub(crate) global_dashboard_view: Entity<GlobalDashboardView>,
    pub(crate) cron_tasks_view: Entity<CronTasksView>,
    pub(crate) import_export_view: Entity<ImportExportView>,
    pub(crate) settings_view: Entity<SettingsView>,
    pub(crate) admin_calendar_dashboard_view:
        Entity<crate::views::admin_calendar_dashboard::AdminCalendarDashboard>,
    pub(crate) form_2551q_view: Option<Entity<Form2551QView>>,
    pub(crate) pending_form_draft: Option<Form2551QDraft>,
    pub(crate) form_1701q_view: Option<Entity<Form1701QView>>,
    pub(crate) pending_form_1701q_draft: Option<bir_core::forms::form_1701q::Form1701QDraft>,
    pub(crate) form_1601c_view: Option<Entity<Form1601CView>>,
    pub(crate) pending_form_1601c_draft: Option<bir_core::forms::form_1601c::Form1601CDraft>,
    pub(crate) form_0619e_view: Option<Entity<Form0619EView>>,
    pub(crate) pending_form_0619e_draft: Option<bir_core::forms::form_0619e::Form0619EDraft>,
    pub(crate) form_0619f_view: Option<Entity<Form0619FView>>,
    pub(crate) pending_form_0619f_draft: Option<bir_core::forms::form_0619f::Form0619FDraft>,
    pub(crate) form_0605_view: Option<Entity<Form0605View>>,
    pub(crate) pending_form_0605_draft: Option<bir_core::forms::form_0605::Form0605Draft>,
    pub(crate) form_2550q_view: Option<Entity<Form2550QV2View>>,
    pub(crate) pending_form_2550q_draft: Option<bir_core::forms::form_2550q::Form2550QDraft>,
    pub(crate) form_1701_view: Option<Entity<Form1701View>>,
    pub(crate) pending_form_1701_draft: Option<bir_core::forms::form_1701::Form1701Draft>,
    pub(crate) form_1702rt_view: Option<Entity<Form1702RTView>>,
    pub(crate) pending_form_1702rt_draft: Option<bir_core::forms::form_1702rt::Form1702RTDraft>,
    pub(crate) form_1702mx_view: Option<Entity<Form1702MXView>>,
    pub(crate) pending_form_1702mx_draft: Option<bir_core::forms::form_1702mx::Form1702MXDraft>,
    pub(crate) db: Arc<Mutex<Database>>,
    pub(crate) profiles: Vec<TaxpayerProfile>,
    pub(crate) active_profile_tin: Option<String>,
    pub(crate) profile_filter: Entity<InputState>,
    pub(crate) sidebar_scroll: ScrollHandle,
    pub(crate) show_archived: bool,
    pub(crate) _subscriptions: Vec<Subscription>,
    /// Whether the window-aware subscription for global dashboard notifications has been set up.
    pub(crate) global_dashboard_notif_subscribed: bool,
    pub(crate) is_mini_sidebar: bool,
    pub(crate) is_sidebar_hidden: bool,
    pub(crate) theme_preference: AppThemeMode,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) is_command_palette_open: bool,
    pub(crate) command_palette_view:
        Option<Entity<crate::components::command_palette::CommandPalette>>,
    pub(crate) is_locked: bool,
    pub(crate) lock_screen_view: Option<Entity<LockScreenView>>,
    pub(crate) pending_profile: Option<(TaxpayerProfile, ProfileTargetAction)>,
    pub(crate) profile_otp_state: Entity<OtpState>,
    pub(crate) profile_totp_state: Entity<OtpState>,
    pub(crate) profile_rate_limiter: RateLimiter,
    pub(crate) profile_auth_error: Option<String>,
    pub(crate) os_auth_triggered: bool,
    pub(crate) unlocked_profile: Option<(TaxpayerProfile, ProfileTargetAction)>,
    pub(crate) hide_tax_profiles: bool,
    pub(crate) enable_profile_pins: bool,
    pub(crate) pending_admin_view: Option<ActiveView>,
    /// A profile lifecycle change waiting behind the administrator prompt.
    /// Archiving, restoring, and deleting a taxpayer profile are admin-only,
    /// and the prompt is the same one that guards Settings and Background
    /// Tasks - so the request is parked here and replayed once auth succeeds.
    pub(crate) pending_admin_lifecycle: Option<(String, ProfileLifecycleAction)>,
    /// A lifecycle change whose administrator prompt has just succeeded. The
    /// prompt resolves in three places and the OS-authentication one has no
    /// `Window` to hand, so the authorised action is parked here and applied
    /// at the top of the next `render`, which does have one.
    pub(crate) admin_lifecycle_authorized: Option<(String, ProfileLifecycleAction)>,
    pub(crate) admin_otp_state: Entity<OtpState>,
    pub(crate) admin_totp_state: Entity<OtpState>,
    pub(crate) admin_rate_limiter: RateLimiter,
    pub(crate) has_admin_totp: bool,
    pub(crate) admin_auth_error: Option<String>,
    pub(crate) admin_os_auth_triggered: bool,
    /// The TIN of the currently active/unlocked profile session (only meaningful when hide_tax_profiles is enabled)
    pub(crate) active_session_tin: Option<String>,
}

impl AppState {
    pub fn new(
        db: Arc<Mutex<Database>>,
        profiles: Vec<TaxpayerProfile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (theme_preference, hide_tax_profiles, enable_profile_pins, is_locked, has_admin_totp) = {
            let db_guard = db.lock().unwrap();
            let tp = if let Ok(Some(val)) = db_guard.get_setting("theme_preference") {
                serde_json::from_str(&val).unwrap_or(AppThemeMode::System)
            } else {
                AppThemeMode::System
            };
            let htp = db_guard
                .get_setting("hide_tax_profiles")
                .ok()
                .flatten()
                .as_deref()
                == Some("true");
            let epp = db_guard
                .get_setting("enable_profile_pins")
                .ok()
                .flatten()
                .as_deref()
                == Some("true");
            let pin_enabled = db_guard
                .get_setting("app_lock_enabled")
                .ok()
                .flatten()
                .as_deref()
                == Some("true");
            let totp_enabled = db_guard
                .get_setting("app_totp_secret")
                .ok()
                .flatten()
                .is_some();
            (tp, htp, epp, pin_enabled || totp_enabled, totp_enabled)
        };

        let target_mode = crate::theme::resolve_theme_mode(theme_preference, window);
        Theme::change(target_mode, Some(window), cx);

        let active_view = if profiles.is_empty() {
            ActiveView::ProfileManager
        } else {
            ActiveView::GlobalDashboard
        };

        let bus = cx.new(|_| crate::events::EventBus {});
        cx.set_global(crate::events::GlobalEventBus(bus));

        // On macOS: instant event-driven notifications from the daemon via NSDistributedNotificationCenter.
        #[cfg(target_os = "macos")]
        crate::events::start_macos_notification_listener(cx);

        // On all platforms: PRAGMA data_version polling as primary (Linux/Windows) or fallback (macOS).
        crate::events::start_db_watcher(Arc::clone(&db), cx);

        let db_clone = Arc::clone(&db);
        let profile_manager = cx.new(|cx| ProfileManagerView::new(db_clone, window, cx));

        let db_clone_global = Arc::clone(&db);
        let global_dashboard_view =
            cx.new(|cx| GlobalDashboardView::new(db_clone_global, window, cx));

        let db_clone_lock = Arc::clone(&db);
        let lock_screen_view = if is_locked {
            let view = cx.new(|cx| LockScreenView::new(db_clone_lock, window, cx));
            // Auto-trigger OS biometrics when the app starts locked, so the user
            // doesn't have to manually click the override button.
            view.update(cx, |this, cx| this.trigger_auth(cx));
            Some(view)
        } else {
            None
        };

        let profile_otp_state = cx.new(|cx| OtpState::new(4, window, cx).masked(true));
        let profile_totp_state = cx.new(|cx| OtpState::new(6, window, cx).masked(false));
        let admin_otp_state = cx.new(|cx| OtpState::new(4, window, cx).masked(true));
        let admin_totp_state = cx.new(|cx| OtpState::new(6, window, cx).masked(false));

        cx.subscribe_in(
            &profile_otp_state,
            window,
            |this: &mut Self, _entity, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    if this.profile_rate_limiter.is_locked() {
                        this.profile_otp_state
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        return;
                    }
                    let entered_pin = this.profile_otp_state.read(cx).value().to_string();
                    if entered_pin.len() == 4 {
                        let hashed = bir_core::crypto::hash_pin(&entered_pin);
                        if let Some((p, a)) = this.pending_profile.clone() {
                            if Some(hashed) == p.profile_pin_hash {
                                this.unlocked_profile = Some((p, a));
                                this.pending_profile = None;
                                this.profile_auth_error = None;
                                this.profile_rate_limiter.reset();
                                this.profile_otp_state
                                    .update(cx, |input, cx| input.set_value("", window, cx));
                                this.focus_handle.focus(window, cx);
                            } else {
                                this.profile_rate_limiter.record_failure();
                                this.profile_auth_error =
                                    Some("Incorrect PIN. Please try again.".to_string());
                                this.profile_otp_state.update(cx, |input, cx| {
                                    input.set_value("", window, cx);
                                    input.focus(window, cx);
                                });
                            }
                        }
                    }
                    cx.notify();
                }
            },
        )
        .detach();

        cx.subscribe_in(
            &admin_otp_state,
            window,
            |this: &mut Self, _entity, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    if this.admin_rate_limiter.is_locked() {
                        this.admin_otp_state
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        return;
                    }
                    let entered_pin = this.admin_otp_state.read(cx).value().to_string();
                    if entered_pin.len() == 4 {
                        let hashed = bir_core::crypto::hash_pin(&entered_pin);
                        let valid = if let Ok(db) = this.db.lock() {
                            let hash = db.get_setting("app_lock_pin_hash").ok().flatten();
                            hash.as_deref() == Some(&hashed)
                        } else {
                            false
                        };

                        if valid {
                            this.admin_rate_limiter.reset();
                            if let Some(target) = this.pending_admin_view.take() {
                                this.active_view = target;
                                if target == ActiveView::CronTasks {
                                    this.cron_tasks_view
                                        .update(cx, |view, cx| view.load_settings(cx));
                                }
                            }
                            if let Some(pending) = this.pending_admin_lifecycle.take() {
                                this.admin_lifecycle_authorized = Some(pending);
                            }
                            this.admin_auth_error = None;
                            this.admin_otp_state
                                .update(cx, |input, cx| input.set_value("", window, cx));
                            this.focus_handle.focus(window, cx);
                        } else {
                            this.admin_rate_limiter.record_failure();
                            this.admin_auth_error = Some("Incorrect Admin PIN.".to_string());
                            this.admin_otp_state.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                        }
                    }
                    cx.notify();
                }
            },
        )
        .detach();

        cx.subscribe_in(
            &profile_totp_state,
            window,
            |this: &mut Self, _entity, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    let entered_token = this.profile_totp_state.read(cx).value().to_string();
                    if entered_token.len() == 6
                        && let Some((p, a)) = this.pending_profile.clone()
                        && let Some(ref secret) = p.totp_secret
                    {
                        if bir_core::crypto::validate_totp(secret, &entered_token) {
                            this.unlocked_profile = Some((p, a));
                            this.pending_profile = None;
                            this.profile_auth_error = None;
                            this.profile_totp_state
                                .update(cx, |input, cx| input.set_value("", window, cx));
                            this.focus_handle.focus(window, cx);
                        } else {
                            this.profile_auth_error =
                                Some("Incorrect Authenticator code.".to_string());
                            this.profile_totp_state.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                        }
                    }
                    cx.notify();
                }
            },
        )
        .detach();

        cx.subscribe_in(
            &admin_totp_state,
            window,
            |this: &mut Self, _entity, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    if this.admin_rate_limiter.is_locked() {
                        this.admin_totp_state
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        return;
                    }
                    let entered_token = this.admin_totp_state.read(cx).value().to_string();
                    if entered_token.len() == 6 {
                        let mut is_valid = false;
                        if let Ok(db_guard) = this.db.lock()
                            && let Ok(Some(secret)) = db_guard.get_setting("app_totp_secret")
                        {
                            is_valid = bir_core::crypto::validate_totp(&secret, &entered_token);
                        }

                        if is_valid {
                            this.admin_rate_limiter.reset();
                            this.admin_auth_error = None;
                            if let Some(target) = this.pending_admin_view.take() {
                                this.active_view = target;
                                if target == ActiveView::CronTasks {
                                    this.cron_tasks_view
                                        .update(cx, |view, cx| view.load_settings(cx));
                                }
                            }
                            if let Some(pending) = this.pending_admin_lifecycle.take() {
                                this.admin_lifecycle_authorized = Some(pending);
                            }
                            this.admin_totp_state
                                .update(cx, |input, cx| input.set_value("", window, cx));
                            this.focus_handle.focus(window, cx);
                        } else {
                            this.admin_rate_limiter.record_failure();
                            this.admin_auth_error =
                                Some("Incorrect Authenticator code.".to_string());
                            this.admin_totp_state.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                        }
                        cx.notify();
                    }
                }
            },
        )
        .detach();

        if let Some(view) = &lock_screen_view {
            cx.subscribe_in(
                view,
                window,
                |this: &mut Self, _entity, event: &LockScreenEvent, window, cx| match event {
                    LockScreenEvent::Unlocked => {
                        this.is_locked = false;
                        this.focus_handle.focus(window, cx);
                        cx.notify();
                    }
                },
            )
            .detach();
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
                        view.hide_tax_profiles = this.hide_tax_profiles;
                        view.active_session_tin = this.active_session_tin.clone();
                        view.set_profiles(profiles.clone(), cx);
                    });
                    this.cron_tasks_view.update(cx, |view, cx| {
                        view.load_settings(cx);
                    });

                    this.active_profile_tin = None;
                    this.active_view = if this.profiles.is_empty() {
                        ActiveView::ProfileManager
                    } else {
                        ActiveView::GlobalDashboard
                    };

                    // Re-evaluate settings (like lock and theme)
                    if let Ok(db) = this.db.lock() {
                        if let Ok(Some(val)) = db.get_setting("theme_preference") {
                            this.theme_preference =
                                serde_json::from_str(&val).unwrap_or(AppThemeMode::System);
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

        let db_clone_admin_cal = Arc::clone(&db);
        let admin_calendar_dashboard_view = cx.new(|cx| {
            crate::views::admin_calendar_dashboard::AdminCalendarDashboard::new(
                db_clone_admin_cal,
                window,
                cx,
            )
        });

        cx.subscribe(
            &settings_view,
            |this: &mut Self, _entity, event: &SettingsEvent, cx| match event {
                SettingsEvent::ReloadApp => {
                    if let Ok(db) = this.db.lock() {
                        this.hide_tax_profiles = db
                            .get_setting("hide_tax_profiles")
                            .ok()
                            .flatten()
                            .as_deref()
                            == Some("true");
                        this.enable_profile_pins = db
                            .get_setting("enable_profile_pins")
                            .ok()
                            .flatten()
                            .as_deref()
                            == Some("true");
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
                    let active_session = this.active_session_tin.clone();
                    this.global_dashboard_view.update(cx, |view, _cx| {
                        view.hide_tax_profiles = htp;
                        view.active_session_tin = active_session;
                    });
                    cx.notify();
                }
            },
        )
        .detach();

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
                        if let Some(profile) = this
                            .profiles
                            .iter()
                            .find(|p| p.tin.full() == query || p.tin.formatted() == query)
                            .cloned()
                        {
                            this.profile_filter.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                            });
                            this.select_profile(
                                profile,
                                ProfileTargetAction::ViewDashboard,
                                window,
                                cx,
                            );
                        } else {
                            let is_tin_like = !query.is_empty()
                                && query.chars().all(|c| c.is_ascii_digit() || c == '-');
                            let query_digits: String =
                                query.chars().filter(|c| c.is_ascii_digit()).collect();

                            if is_tin_like && query_digits.len() >= 9 {
                                if this.block_unsaved_compliance_navigation(window, cx) {
                                    return;
                                }
                                this.active_session_tin = None;
                                this.active_view = ActiveView::ProfileManager;
                                this.active_profile_tin = None;
                                this.profile_manager.update(cx, |view, cx| {
                                    view.reset_for_new(window, cx);
                                    view.prefill_tin(&query, window, cx);
                                });
                                this.profile_filter.update(cx, |input, cx| {
                                    input.set_value("", window, cx);
                                });
                                cx.notify();
                            }
                        }
                    }
                    _ => {}
                }
            },
        );

        let profile_sub = cx.subscribe_in(
            &profile_manager,
            window,
            |this: &mut Self,
             _entity,
             event: &crate::views::profile_manager::ProfileEvent,
             window,
             cx| {
                let saved_tin = match event {
                    crate::views::profile_manager::ProfileEvent::Saved(tin) => Some(tin.clone()),
                    // Not a save: the editor is asking for an admin-gated
                    // change and has written nothing. Route it through the
                    // gate and stop - the write refreshes the list itself.
                    crate::views::profile_manager::ProfileEvent::LifecycleRequested {
                        tin,
                        action,
                    } => {
                        this.request_profile_lifecycle(tin.clone(), *action, window, cx);
                        return;
                    }
                };

                if let Some(tin) = &saved_tin
                    && this.hide_tax_profiles
                {
                    this.active_session_tin = Some(tin.clone());
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
                            view.hide_tax_profiles = this.hide_tax_profiles;
                            view.active_session_tin = this.active_session_tin.clone();
                            view.set_profiles(profiles.clone(), cx);
                        });

                        if let Some(tin) = &active_tin
                            && let Some(profile) =
                                this.profiles.iter().find(|p| p.tin.full() == *tin)
                        {
                            let p = profile.clone();
                            this.dashboard_view.update(cx, |view, cx| {
                                view.set_profile(p, cx);
                            });
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
                    let profile_clone =
                        this.profiles.iter().find(|p| p.tin.full() == *tin).cloned();
                    if let Some(profile) = profile_clone {
                        this.select_profile(
                            profile,
                            ProfileTargetAction::ViewDashboard,
                            window,
                            cx,
                        );
                    } else {
                        this.active_profile_tin = Some(tin.clone());
                    }

                    let event = DashboardEvent::FileForm {
                        form_code: form_code.clone(),
                        year: *year,
                        quarter: *quarter,
                    };
                    this.handle_file_form(&event, window, cx);
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

        cx.subscribe_in(
            &dashboard_view,
            window,
            |this: &mut Self, _entity, event: &DashboardEvent, window, cx| match event {
                DashboardEvent::FileForm { .. } => this.handle_file_form(event, window, cx),
                DashboardEvent::Reload => {
                    if let Some(tin) = &this.active_profile_tin
                        && let Some(profile) = this.profiles.iter().find(|p| p.tin.full() == *tin)
                    {
                        let p = profile.clone();
                        this.dashboard_view.update(cx, |view, cx| {
                            view.set_profile(p, cx);
                        });
                    }
                    cx.notify();
                }
                DashboardEvent::LogoutProfile(_tin) => {
                    this.logout(window, cx);
                }
                DashboardEvent::OpenProfileManager => {
                    this.active_view = ActiveView::ProfileManager;
                    cx.notify();
                }
                DashboardEvent::EditProfile(tin) => {
                    // The header's Profile Settings control. Reuses the normal
                    // selection path so a PIN-protected profile still
                    // authenticates before its editor opens.
                    if let Some(profile) = this
                        .profiles
                        .iter()
                        .find(|candidate| candidate.tin.full() == *tin)
                        .cloned()
                    {
                        this.select_profile(profile, ProfileTargetAction::EditProfile, window, cx);
                    }
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
            form_1701q_view: None,
            pending_form_1701q_draft: None,
            form_1601c_view: None,
            pending_form_1601c_draft: None,
            form_0619e_view: None,
            pending_form_0619e_draft: None,
            form_0619f_view: None,
            pending_form_0619f_draft: None,
            form_0605_view: None,
            pending_form_0605_draft: None,
            form_2550q_view: None,
            pending_form_2550q_draft: None,
            form_1701_view: None,
            pending_form_1701_draft: None,
            form_1702rt_view: None,
            pending_form_1702rt_draft: None,
            form_1702mx_view: None,
            pending_form_1702mx_draft: None,
            db,
            profiles,
            active_profile_tin: None,
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
            profile_totp_state,
            profile_rate_limiter: RateLimiter::new(),
            profile_auth_error: None,
            os_auth_triggered: false,
            unlocked_profile: None,
            settings_view,
            admin_calendar_dashboard_view,
            hide_tax_profiles,
            enable_profile_pins,
            pending_admin_view: None,
            pending_admin_lifecycle: None,
            admin_lifecycle_authorized: None,
            admin_otp_state,
            admin_totp_state,
            admin_rate_limiter: RateLimiter::new(),
            has_admin_totp,
            admin_auth_error: None,
            admin_os_auth_triggered: false,
            active_session_tin: None,
        }
    }

    pub(crate) fn select_profile(
        &mut self,
        profile: TaxpayerProfile,
        action: ProfileTargetAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.block_unsaved_compliance_navigation(window, cx) {
            return;
        }

        let tin = profile.tin.full();

        // If this profile is already the active session, skip PIN entirely
        if self.active_session_tin.as_ref() == Some(&tin) {
            self.apply_profile_action(profile, action, window, cx);
            cx.notify();
            return;
        }

        // Profile needs auth if it has EITHER a PIN or TOTP configured (mutually exclusive)
        let needs_auth = self.enable_profile_pins
            && (profile.profile_pin_hash.is_some() || profile.totp_secret.is_some());

        if needs_auth {
            let use_totp = profile.totp_secret.is_some();
            self.pending_profile = Some((profile, action));
            self.profile_auth_error = None;
            self.os_auth_triggered = false;
            self.profile_rate_limiter.reset();
            if use_totp {
                self.profile_totp_state.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                });
            } else {
                self.profile_otp_state.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                });
            }
        } else {
            self.apply_profile_action(profile, action, window, cx);
        }
        cx.notify();
    }

    fn apply_profile_action(
        &mut self,
        profile: TaxpayerProfile,
        action: ProfileTargetAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tin = profile.tin.full();
        self.active_profile_tin = Some(tin.clone());
        // Set session whenever profile PINs are enabled OR privacy mode is on,
        // so subsequent access to the same profile skips PIN re-entry.
        if self.enable_profile_pins || self.hide_tax_profiles {
            self.active_session_tin = Some(tin);
        }

        // Keep dashboard in sync with hide_tax_profiles state
        let htp = self.hide_tax_profiles;
        self.dashboard_view.update(cx, |view, _cx| {
            view.hide_tax_profiles = htp;
        });

        match action {
            ProfileTargetAction::UnlockOnly => {
                // Just unlock the session, no navigation needed
            }
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

    /// Whether an administrator credential is configured at all. With neither
    /// an app-lock PIN nor an app TOTP secret there is nobody to authenticate
    /// against, so the admin-only actions run unprompted - the same rule the
    /// Settings and Background Tasks gates already follow.
    fn admin_credential_configured(&self) -> (bool, bool) {
        if let Ok(db) = self.db.lock() {
            let pin = db.get_setting("app_lock_enabled").ok().flatten().as_deref() == Some("true");
            let totp = db.get_setting("app_totp_secret").ok().flatten().is_some();
            (pin, totp)
        } else {
            (false, false)
        }
    }

    /// Entry point for every archive, restore, and delete. Prompts for the
    /// administrator credential first when one is configured, parking the
    /// request in `pending_admin_lifecycle` so it replays on success.
    pub(crate) fn request_profile_lifecycle(
        &mut self,
        tin: String,
        action: ProfileLifecycleAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.block_unsaved_compliance_navigation(window, cx) {
            return;
        }

        let (is_app_lock_enabled, has_app_totp) = self.admin_credential_configured();

        if is_app_lock_enabled || has_app_totp {
            self.pending_admin_lifecycle = Some((tin, action));
            self.admin_auth_error = None;
            self.admin_os_auth_triggered = false;
            self.admin_rate_limiter.reset();
            if has_app_totp {
                self.admin_totp_state.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                });
            } else {
                self.admin_otp_state.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                });
            }
            cx.notify();
        } else {
            self.perform_profile_lifecycle(tin, action, window, cx);
        }
    }

    /// Applies an already-authorised lifecycle change. Only ever reached from
    /// `request_profile_lifecycle` or from the admin prompt succeeding.
    pub(crate) fn perform_profile_lifecycle(
        &mut self,
        tin: String,
        action: ProfileLifecycleAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self
            .profiles
            .iter()
            .find(|candidate| candidate.tin.full() == tin)
            .cloned()
        else {
            push_notification(
                "error",
                "Profile unchanged",
                &format!(
                    "{tin} was not {}: it is no longer loaded.",
                    action.past_tense()
                ),
                window,
                cx,
            );
            return;
        };

        match action {
            ProfileLifecycleAction::Archive => {
                self.persist_profile_archived(profile, true, window, cx)
            }
            ProfileLifecycleAction::Restore => {
                self.persist_profile_archived(profile, false, window, cx)
            }
            ProfileLifecycleAction::Delete => self.export_and_delete_profile(tin, cx),
        }
    }

    /// Deleting a profile is irreversible, so it always writes a backup first:
    /// the save dialog doubles as the confirmation, and cancelling it cancels
    /// the deletion. The export must succeed before anything is removed.
    fn export_and_delete_profile(&mut self, tin: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            // Safe: SystemTime arithmetic is infallible after UNIX_EPOCH.
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let Some(export_handle) = rfd::AsyncFileDialog::new()
                .set_title("Save Profile Archive")
                .set_file_name(format!("BIR_Archive_{tin}_{timestamp}.zip"))
                .add_filter("Zip Archive", &["zip"])
                .save_file()
                .await
            else {
                return;
            };
            let export_dir = export_handle.path().to_path_buf();

            let deletion_result = this.update(cx, |this, cx| {
                let db = this
                    .db
                    .lock()
                    .map_err(|_| "Database lock poisoned".to_string())?;
                bir_core::export_profile_data(&db, &tin, &export_dir)
                    .map_err(|error| format!("Profile export failed: {error}"))?;
                db.delete_profile(&tin)
                    .map_err(|error| format!("Profile deletion failed: {error}"))?;
                drop(db);

                this.profiles.retain(|p| p.tin.full() != tin);
                if !this.profiles.iter().any(|p| p.is_archived) {
                    this.show_archived = false;
                }
                if this.active_profile_tin.as_ref() == Some(&tin) {
                    this.active_profile_tin = None;
                    this.active_view = ActiveView::ProfileManager;
                }
                cx.notify();
                Ok::<(), String>(())
            });

            match deletion_result {
                Ok(Ok(())) => {
                    rfd::AsyncMessageDialog::new()
                        .set_title("Profile Exported & Deleted")
                        .set_description(format!("Saved to {}", export_dir.display()))
                        .show()
                        .await;
                }
                Ok(Err(error)) => {
                    rfd::AsyncMessageDialog::new()
                        .set_title("Profile Was Not Deleted")
                        .set_description(error)
                        .show()
                        .await;
                }
                Err(error) => {
                    rfd::AsyncMessageDialog::new()
                        .set_title("Profile Was Not Deleted")
                        .set_description(error.to_string())
                        .show()
                        .await;
                }
            }
        })
        .detach();
    }

    pub(crate) fn request_admin_access(
        &mut self,
        target: ActiveView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.block_unsaved_compliance_navigation(window, cx) {
            return;
        }

        let (is_app_lock_enabled, has_app_totp) = if let Ok(db) = self.db.lock() {
            let pin = db.get_setting("app_lock_enabled").ok().flatten().as_deref() == Some("true");
            let totp = db.get_setting("app_totp_secret").ok().flatten().is_some();
            (pin, totp)
        } else {
            (false, false)
        };

        if is_app_lock_enabled || has_app_totp {
            self.pending_admin_view = Some(target);
            self.admin_auth_error = None;
            self.admin_os_auth_triggered = false;
            self.admin_rate_limiter.reset();
            if has_app_totp {
                self.admin_totp_state.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                });
            } else {
                self.admin_otp_state.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                });
            }
        } else {
            self.active_view = target;
            if target == ActiveView::CronTasks {
                self.cron_tasks_view
                    .update(cx, |view, cx| view.load_settings(cx));
            }
        }
        cx.notify();
    }

    pub(crate) fn logout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.block_unsaved_compliance_navigation(window, cx) {
            return;
        }

        self.active_session_tin = None;
        self.active_profile_tin = None;
        self.active_view = ActiveView::GlobalDashboard;

        self.global_dashboard_view.update(cx, |view, _cx| {
            view.active_session_tin = None;
        });

        cx.notify();
    }

    // NOTE: render_sidebar() is implemented in sidebar.rs

    fn render_active_view(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        match self.active_view {
            ActiveView::GlobalDashboard => self.global_dashboard_view.clone().into_any_element(),
            ActiveView::ProfileManager => self.profile_manager.clone().into_any_element(),
            ActiveView::CronTasks => self.cron_tasks_view.clone().into_any_element(),
            ActiveView::ImportExport => self.import_export_view.clone().into_any_element(),
            ActiveView::Settings => self.settings_view.clone().into_any_element(),
            ActiveView::AdminCalendarDashboard => self
                .admin_calendar_dashboard_view
                .clone()
                .into_any_element(),
            ActiveView::Dashboard => self.dashboard_view.clone().into_any_element(),
            ActiveView::Form2551Q => {
                if let Some(view) = &self.form_2551q_view {
                    view.clone().into_any_element()
                } else {
                    let root = rsx! { <div>{"No form loaded"}</div> };
                    root.into_any_element()
                }
            }
            ActiveView::Form1701Q => {
                if let Some(view) = &self.form_1701q_view {
                    view.clone().into_any_element()
                } else {
                    let root = rsx! { <div>{"No form loaded"}</div> };
                    root.into_any_element()
                }
            }
            ActiveView::Form1601C => {
                if let Some(view) = &self.form_1601c_view {
                    view.clone().into_any_element()
                } else {
                    let root = rsx! { <div>{"No form loaded"}</div> };
                    root.into_any_element()
                }
            }
            ActiveView::Form0619E => {
                if let Some(view) = &self.form_0619e_view {
                    view.clone().into_any_element()
                } else {
                    let root = rsx! { <div>{"No form loaded"}</div> };
                    root.into_any_element()
                }
            }
            ActiveView::Form0619F => {
                if let Some(view) = &self.form_0619f_view {
                    view.clone().into_any_element()
                } else {
                    let root = rsx! { <div>{"No form loaded"}</div> };
                    root.into_any_element()
                }
            }
            ActiveView::Form0605 => {
                if let Some(view) = &self.form_0605_view {
                    view.clone().into_any_element()
                } else {
                    let root = rsx! { <div>{"No form loaded"}</div> };
                    root.into_any_element()
                }
            }
            ActiveView::Form2550Q => {
                if let Some(view) = &self.form_2550q_view {
                    view.clone().into_any_element()
                } else {
                    let root = rsx! { <div>{"No form loaded"}</div> };
                    root.into_any_element()
                }
            }
            ActiveView::Form1701 => {
                if let Some(view) = &self.form_1701_view {
                    view.clone().into_any_element()
                } else {
                    let root = rsx! { <div>{"No form loaded"}</div> };
                    root.into_any_element()
                }
            }
            ActiveView::Form1702RT => {
                if let Some(view) = &self.form_1702rt_view {
                    view.clone().into_any_element()
                } else {
                    let root = rsx! { <div>{"No form loaded"}</div> };
                    root.into_any_element()
                }
            }
            ActiveView::Form1702MX => {
                if let Some(view) = &self.form_1702mx_view {
                    view.clone().into_any_element()
                } else {
                    let root = rsx! { <div>{"No form loaded"}</div> };
                    root.into_any_element()
                }
            }
        }
    }

    pub(crate) fn block_unsaved_compliance_navigation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .profile_manager
            .read(cx)
            .has_unsaved_compliance_changes()
        {
            return false;
        }

        self.active_view = ActiveView::ProfileManager;
        self.profile_manager.update(cx, |view, cx| {
            view.notify_unsaved_compliance_blocked(window, cx);
        });
        cx.notify();
        true
    }

    pub(crate) fn request_application_quit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        before_quit: impl FnOnce(),
    ) -> bool {
        let decision = crate::quit_guard::application_quit_decision(
            self.profile_manager
                .read(cx)
                .has_unsaved_compliance_changes(),
        );
        match decision {
            crate::quit_guard::ApplicationQuitDecision::Quit => {
                before_quit();
                cx.quit();
                true
            }
            crate::quit_guard::ApplicationQuitDecision::StayOpenForUnsavedCompliance => {
                self.active_view = ActiveView::ProfileManager;
                self.profile_manager.update(cx, |view, cx| {
                    view.notify_unsaved_compliance_blocked(window, cx);
                });
                crate::platform::show_in_dock();
                cx.activate(true);
                cx.notify();
                false
            }
        }
    }

    fn handle_file_form(
        &mut self,
        event: &DashboardEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.block_unsaved_compliance_navigation(window, cx) {
            return;
        }

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

        // Production builds only open certified forms. Debug/dev-tools builds
        // may additionally open a semantically complete HTML draft while its
        // platform/package release evidence is still being collected.
        let support = bir_core::forms::form_support_level(form_code);
        let is_certification_build = cfg!(any(debug_assertions, feature = "dev-tools"));
        let can_open = support.is_fileable_in_app()
            || (is_certification_build && bir_core::forms::can_open_certification_draft(form_code));
        if !can_open {
            tracing::warn!(
                form_code,
                "Attempted to open an uncertified or unsupported form — manual filing required"
            );
            return;
        }

        if form_code == "2551Q"
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            let draft = if let Ok(db) = self.db.lock() {
                let existing = db.get_2551q_draft(tin, year, quarter).ok().flatten();
                if let Some(d) = existing {
                    if matches!(d.status, FilingStatus::Draft) {
                        // Draft mode: sync profile fields so edits to the profile
                        // are reflected in the form before submission.
                        let mut d = d;
                        if let Err(error) = d.reconcile_with_effective_profile(profile) {
                            tracing::warn!(%error, "2551Q effective profile could not be resolved");
                        }
                        if let Err(error) = db.save_2551q_draft(&d) {
                            tracing::warn!(%error, "Failed to persist refreshed 2551Q draft");
                        }
                        tracing::info!(
                            tin = %d.tin,
                            status = ?d.status,
                            name = %d.taxpayer_name,
                            "Loading existing draft from DB (profile synced)"
                        );
                        d
                    } else {
                        // Non-draft: compare against the current profile without
                        // replacing any reviewed values. The draft owns a stale
                        // marker that drives the explicit revert/amend warning.
                        let d = match db.reconcile_immutable_2551q_profile_snapshot(
                            &d.tin,
                            d.taxable_year,
                            d.quarter,
                            profile,
                        ) {
                            Ok(refreshed) => refreshed,
                            Err(error) => {
                                tracing::warn!(%error, "Failed to reconcile immutable 2551Q profile-snapshot marker");
                                d
                            }
                        };
                        tracing::info!(
                            tin = %d.tin,
                            status = ?d.status,
                            name = %d.taxpayer_name,
                            profile_snapshot_stale = d.profile_snapshot_stale,
                            "Loading existing draft from DB (immutable profile snapshot)"
                        );
                        d
                    }
                } else {
                    tracing::info!(tin = %tin, year, quarter, "Creating new draft from profile");
                    let prev = if quarter > 1 {
                        db.get_2551q_draft(tin, year, quarter - 1).ok().flatten()
                    } else {
                        None
                    };
                    let new_draft =
                        Form2551QDraft::new_from_effective_profile(profile, year, quarter);
                    let new_draft = if let Some(prev_draft) = prev {
                        new_draft.with_carried_forward(&prev_draft)
                    } else {
                        new_draft
                    };
                    if let Err(error) = db.save_2551q_draft(&new_draft) {
                        tracing::warn!(%error, "Failed to persist newly opened 2551Q draft");
                    }
                    new_draft
                }
            } else {
                Form2551QDraft::new_from_effective_profile(profile, year, quarter)
            };
            // Store the draft; the view will be created on next render with Window access
            self.pending_form_draft = Some(draft);
            self.active_view = ActiveView::Form2551Q;
            cx.notify();
        } else if form_code == "1701Q"
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            let load_result = self
                .db
                .lock()
                .map_err(|_| "1701Q draft database lock is unavailable".to_string())
                .and_then(|db| {
                    db.get_form_draft::<bir_core::forms::form_1701q::Form1701QDraft>(
                        tin,
                        "1701Q",
                        year,
                        Some(quarter),
                    )
                    .map_err(|error| error.to_string())
                });
            let draft = match load_result {
                Ok(Some(draft)) => draft,
                Ok(None) => bir_core::forms::form_1701q::Form1701QDraft::new_from_profile(
                    profile, year, quarter,
                ),
                Err(error) => {
                    tracing::error!(
                        %error,
                        tin,
                        year,
                        quarter,
                        "Refusing to replace an unreadable 1701Q draft"
                    );
                    return;
                }
            };
            self.pending_form_1701q_draft = Some(draft);
            self.active_view = ActiveView::Form1701Q;
            cx.notify();
        } else if form_code == "1601C"
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            let draft = bir_core::forms::form_1601c::Form1601CDraft::new_from_profile(
                profile, year, quarter,
            );
            self.pending_form_1601c_draft = Some(draft);
            self.active_view = ActiveView::Form1601C;
            cx.notify();
        } else if form_code == "0619E"
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            let draft = if let Ok(db) = self.db.lock() {
                db.get_form_draft::<bir_core::forms::form_0619e::Form0619EDraft>(
                    tin,
                    "0619E",
                    year,
                    Some(quarter),
                )
                .ok()
                .flatten()
                .unwrap_or_else(|| {
                    bir_core::forms::form_0619e::Form0619EDraft::new_from_profile(
                        profile, year, quarter,
                    )
                })
            } else {
                bir_core::forms::form_0619e::Form0619EDraft::new_from_profile(
                    profile, year, quarter,
                )
            };
            self.pending_form_0619e_draft = Some(draft);
            self.active_view = ActiveView::Form0619E;
            cx.notify();
        } else if form_code == "0619F"
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            let draft = if let Ok(db) = self.db.lock() {
                db.get_form_draft::<bir_core::forms::form_0619f::Form0619FDraft>(
                    tin,
                    "0619F",
                    year,
                    Some(quarter),
                )
                .ok()
                .flatten()
                .unwrap_or_else(|| {
                    bir_core::forms::form_0619f::Form0619FDraft::new_from_profile(
                        profile, year, quarter,
                    )
                })
            } else {
                bir_core::forms::form_0619f::Form0619FDraft::new_from_profile(
                    profile, year, quarter,
                )
            };
            self.pending_form_0619f_draft = Some(draft);
            self.active_view = ActiveView::Form0619F;
            cx.notify();
        } else if form_code == "0605"
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            let draft = if let Ok(db) = self.db.lock() {
                db.get_form_draft::<bir_core::forms::form_0605::Form0605Draft>(
                    tin,
                    "0605",
                    year,
                    Some(quarter),
                )
                .ok()
                .flatten()
                .unwrap_or_else(|| {
                    bir_core::forms::form_0605::Form0605Draft::new_from_profile(
                        profile, year, quarter,
                    )
                })
            } else {
                bir_core::forms::form_0605::Form0605Draft::new_from_profile(profile, year, quarter)
            };
            self.pending_form_0605_draft = Some(draft);
            self.active_view = ActiveView::Form0605;
            cx.notify();
        } else if form_code == "2550Q"
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            let load_result = self
                .db
                .lock()
                .map_err(|_| "2550Q draft database lock is unavailable".to_string())
                .and_then(|db| {
                    db.get_form_draft::<bir_core::forms::form_2550q::Form2550QDraft>(
                        tin,
                        "2550Q",
                        year,
                        Some(quarter),
                    )
                    .map_err(|error| error.to_string())
                });
            let mut draft = match load_result {
                Ok(Some(draft)) => draft,
                Ok(None) => bir_core::forms::form_2550q::Form2550QDraft::new_from_profile(
                    profile, year, quarter,
                ),
                Err(error) => {
                    tracing::error!(
                        %error,
                        tin,
                        year,
                        quarter,
                        "Refusing to replace an unreadable 2550Q draft"
                    );
                    self.active_view = ActiveView::Dashboard;
                    push_notification(
                        "error",
                        "2550Q could not be opened",
                        &format!(
                            "The draft could not be read safely, so it was not opened. \
                             Your stored data was not replaced. Details: {error}"
                        ),
                        window,
                        cx,
                    );
                    cx.notify();
                    return;
                }
            };
            if let Err(error) =
                bir_core::form_rules::prepare_form_2550q_live_row_identities(&mut draft)
            {
                tracing::error!(
                    %error,
                    tin,
                    year,
                    quarter,
                    "Refusing to open a 2550Q draft with unsafe row identities"
                );
                self.active_view = ActiveView::Dashboard;
                push_notification(
                    "error",
                    "2550Q could not be opened",
                    &format!(
                        "The draft's row identities could not be prepared safely, so it was not \
                         opened. No changes were saved. Details: {error}"
                    ),
                    window,
                    cx,
                );
                cx.notify();
                return;
            }
            self.pending_form_2550q_draft = Some(draft);
            self.active_view = ActiveView::Form2550Q;
            cx.notify();
        } else if form_code == "1701"
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            let draft = if let Ok(db) = self.db.lock() {
                db.get_form_draft::<bir_core::forms::form_1701::Form1701Draft>(
                    tin, "1701", year, None,
                )
                .ok()
                .flatten()
                .unwrap_or_else(|| {
                    bir_core::forms::form_1701::Form1701Draft::new_from_profile(profile, year, 1)
                })
            } else {
                bir_core::forms::form_1701::Form1701Draft::new_from_profile(profile, year, 1)
            };
            self.pending_form_1701_draft = Some(draft);
            self.active_view = ActiveView::Form1701;
            cx.notify();
        } else if form_code == "1702RT"
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            let draft = if let Ok(db) = self.db.lock() {
                db.get_form_draft::<bir_core::forms::form_1702rt::Form1702RTDraft>(
                    tin, "1702RT", year, None,
                )
                .ok()
                .flatten()
                .unwrap_or_else(|| {
                    bir_core::forms::form_1702rt::Form1702RTDraft::new_from_profile(
                        profile, year, 1,
                    )
                })
            } else {
                bir_core::forms::form_1702rt::Form1702RTDraft::new_from_profile(profile, year, 1)
            };
            self.pending_form_1702rt_draft = Some(draft);
            self.active_view = ActiveView::Form1702RT;
            cx.notify();
        } else if form_code == "1702MX"
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            let draft = if let Ok(db) = self.db.lock() {
                db.get_form_draft::<bir_core::forms::form_1702mx::Form1702MXDraft>(
                    tin, "1702MX", year, None,
                )
                .ok()
                .flatten()
                .unwrap_or_else(|| {
                    bir_core::forms::form_1702mx::Form1702MXDraft::new_from_profile(profile, year)
                })
            } else {
                bir_core::forms::form_1702mx::Form1702MXDraft::new_from_profile(profile, year)
            };
            self.pending_form_1702mx_draft = Some(draft);
            self.active_view = ActiveView::Form1702MX;
            cx.notify();
        }
    }
}

/// Push a typed notification to the GPUI window overlay.
fn push_notification(level: &str, title: &str, message: &str, window: &mut Window, cx: &mut App) {
    use gpui_component::WindowExt;
    use gpui_component::notification::Notification;
    let notification = match level {
        "success" => Notification::success(title.to_string()).message(message.to_string()),
        "error" => Notification::error(title.to_string()).message(message.to_string()),
        _ => Notification::info(title.to_string()).message(message.to_string()),
    };
    window.push_notification(notification, cx);
}

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Apply a lifecycle change the administrator prompt authorised. It
        // lands here because this is the first point after the prompt
        // resolves that is guaranteed to hold a `Window`.
        if let Some((tin, action)) = self.admin_lifecycle_authorized.take() {
            self.perform_profile_lifecycle(tin, action, window, cx);
        }

        // Set up window-aware subscription for global dashboard notifications (once)
        if !self.global_dashboard_notif_subscribed {
            self.global_dashboard_notif_subscribed = true;
            cx.subscribe_in(
                &self.global_dashboard_view,
                window,
                |_this: &mut Self, _entity, event: &GlobalDashboardEvent, window, cx| {
                    if let GlobalDashboardEvent::PushNotification(level, title, message) = event {
                        push_notification(level, title, message, window, cx);
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
                        push_notification(level, title, message, window, cx);
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

        if let Some(draft) = self.pending_form_1701q_draft.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| Form1701QView::new(draft, db_for_view, window, cx));
            cx.subscribe_in(
                &form_view,
                window,
                |this: &mut Self, _entity, event: &Form1701QEvent, window, cx| match event {
                    Form1701QEvent::BackToDashboard => {
                        this.active_view = ActiveView::Dashboard;
                        cx.notify();
                    }
                    Form1701QEvent::PushNotification(level, title, message) => {
                        push_notification(level, title, message, window, cx);
                    }
                    Form1701QEvent::Saved => cx.notify(),
                },
            )
            .detach();
            self.form_1701q_view = Some(form_view);
        }

        if let Some(draft) = self.pending_form_1601c_draft.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| Form1601CView::new(draft, db_for_view, window, cx));

            cx.subscribe_in(
                &form_view,
                window,
                |this: &mut Self, _entity, event: &Form1601CEvent, window, cx| match event {
                    Form1601CEvent::BackToDashboard => {
                        this.active_view = ActiveView::Dashboard;
                        cx.notify();
                    }
                    Form1601CEvent::PushNotification(level, title, message) => {
                        push_notification(level, title, message, window, cx);
                    }
                    Form1601CEvent::Saved
                    | Form1601CEvent::Submitted
                    | Form1601CEvent::Confirmed => {
                        cx.notify();
                    }
                },
            )
            .detach();

            self.form_1601c_view = Some(form_view);
        }

        if let Some(draft) = self.pending_form_0619e_draft.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| Form0619EView::new(draft, db_for_view, window, cx));

            cx.subscribe_in(
                &form_view,
                window,
                |this: &mut Self, _entity, event: &Form0619EEvent, window, cx| match event {
                    Form0619EEvent::BackToDashboard => {
                        this.active_view = ActiveView::Dashboard;
                        cx.notify();
                    }
                    Form0619EEvent::PushNotification(level, title, message) => {
                        push_notification(level, title, message, window, cx);
                    }
                    Form0619EEvent::Saved
                    | Form0619EEvent::Submitted
                    | Form0619EEvent::Confirmed => {
                        cx.notify();
                    }
                },
            )
            .detach();

            self.form_0619e_view = Some(form_view);
        }

        if let Some(draft) = self.pending_form_0619f_draft.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| Form0619FView::new(draft, db_for_view, window, cx));

            cx.subscribe_in(
                &form_view,
                window,
                |this: &mut Self, _entity, event: &Form0619FEvent, window, cx| match event {
                    Form0619FEvent::BackToDashboard => {
                        this.active_view = ActiveView::Dashboard;
                        cx.notify();
                    }
                    Form0619FEvent::PushNotification(level, title, message) => {
                        push_notification(level, title, message, window, cx);
                    }
                    Form0619FEvent::Saved
                    | Form0619FEvent::Submitted
                    | Form0619FEvent::Confirmed => {
                        cx.notify();
                    }
                },
            )
            .detach();

            self.form_0619f_view = Some(form_view);
        }

        if let Some(draft) = self.pending_form_0605_draft.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| Form0605View::new(draft, db_for_view, window, cx));

            cx.subscribe_in(
                &form_view,
                window,
                |this: &mut Self, _entity, event: &Form0605Event, window, cx| match event {
                    Form0605Event::BackToDashboard => {
                        this.active_view = ActiveView::Dashboard;
                        cx.notify();
                    }
                    Form0605Event::PushNotification(level, title, message) => {
                        push_notification(level, title, message, window, cx);
                    }
                    Form0605Event::Saved | Form0605Event::Submitted | Form0605Event::Confirmed => {
                        cx.notify();
                    }
                },
            )
            .detach();

            self.form_0605_view = Some(form_view);
        }

        if let Some(draft) = self.pending_form_2550q_draft.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| Form2550QV2View::new(draft, db_for_view, window, cx));
            cx.subscribe_in(
                &form_view,
                window,
                |this: &mut Self, _entity, event: &Form2550QV2Event, window, cx| match event {
                    Form2550QV2Event::BackToDashboard => {
                        this.active_view = ActiveView::Dashboard;
                        cx.notify();
                    }
                    Form2550QV2Event::PushNotification(level, title, message) => {
                        push_notification(level, title, message, window, cx);
                    }
                    Form2550QV2Event::Saved
                    | Form2550QV2Event::Submitted
                    | Form2550QV2Event::Confirmed => {
                        cx.notify();
                    }
                },
            )
            .detach();
            self.form_2550q_view = Some(form_view);
        }

        if let Some(draft) = self.pending_form_1701_draft.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| Form1701View::new(draft, db_for_view, window, cx));
            cx.subscribe_in(
                &form_view,
                window,
                |this: &mut Self, _entity, event: &Form1701Event, window, cx| match event {
                    Form1701Event::BackToDashboard => {
                        this.active_view = ActiveView::Dashboard;
                        cx.notify();
                    }
                    Form1701Event::PushNotification(level, title, message) => {
                        push_notification(level, title, message, window, cx);
                    }
                    Form1701Event::Saved | Form1701Event::Submitted | Form1701Event::Confirmed => {
                        cx.notify();
                    }
                },
            )
            .detach();
            self.form_1701_view = Some(form_view);
        }

        if let Some(draft) = self.pending_form_1702rt_draft.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| Form1702RTView::new(draft, db_for_view, window, cx));
            cx.subscribe_in(
                &form_view,
                window,
                |this: &mut Self, _entity, event: &Form1702RTEvent, window, cx| match event {
                    Form1702RTEvent::BackToDashboard => {
                        this.active_view = ActiveView::Dashboard;
                        cx.notify();
                    }
                    Form1702RTEvent::PushNotification(level, title, message) => {
                        push_notification(level, title, message, window, cx);
                    }
                    Form1702RTEvent::Saved
                    | Form1702RTEvent::Submitted
                    | Form1702RTEvent::Confirmed => {
                        cx.notify();
                    }
                },
            )
            .detach();
            self.form_1702rt_view = Some(form_view);
        }

        if let Some(draft) = self.pending_form_1702mx_draft.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| Form1702MXView::new(draft, db_for_view, window, cx));
            cx.subscribe_in(
                &form_view,
                window,
                |this: &mut Self, _entity, event: &Form1702MXEvent, window, cx| match event {
                    Form1702MXEvent::BackToDashboard => {
                        this.active_view = ActiveView::Dashboard;
                        cx.notify();
                    }
                    Form1702MXEvent::PushNotification(level, title, message) => {
                        push_notification(level, title, message, window, cx);
                    }
                    Form1702MXEvent::Saved
                    | Form1702MXEvent::Submitted
                    | Form1702MXEvent::Confirmed => {
                        cx.notify();
                    }
                },
            )
            .detach();
            self.form_1702mx_view = Some(form_view);
        }

        if let Some((profile, action)) = self.unlocked_profile.take() {
            self.apply_profile_action(profile, action, window, cx);
            self.focus_handle.focus(window, cx);
        }

        let notification_layer = Root::render_notification_layer(window, cx);

        if self.is_locked
            && let Some(lock_screen) = &self.lock_screen_view
        {
            let root = rsx! { <div size_full>{lock_screen.clone()}</div> };
            return root.into_any_element();
        }

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .on_action(cx.listener(Self::handle_toggle_sidebar))
            .on_action(cx.listener(Self::handle_toggle_sidebar_mini))
            .on_action(cx.listener(Self::handle_focus_search))
            .on_action(cx.listener(Self::handle_create_profile))
            .on_action(cx.listener(Self::handle_toggle_theme))
            .on_action(cx.listener(Self::handle_open_cron_tasks))
            .on_action(cx.listener(Self::handle_open_settings))
            .on_action(cx.listener(Self::handle_open_global_dashboard))
            .on_action(cx.listener(Self::handle_open_command_palette))
            .on_action(cx.listener(Self::handle_quit_application))
            .on_action(cx.listener(Self::handle_hide_application))
            .on_action(cx.listener(Self::handle_hide_others))
            .on_action(cx.listener(Self::handle_close_window))
            .on_action(cx.listener(Self::handle_minimize_window))
            .on_action(cx.listener(Self::handle_zoom_window))
            .on_action(cx.listener(Self::handle_toggle_fullscreen))
            .child(rsx! {
                <div
                    flex
                    flex_row
                    flex_1
                    min_h_0
                    when={(!self.is_sidebar_hidden, |this| {
                        this.child(self.render_sidebar(window, cx))
                    })}
                >
                    <div flex_1 flex flex_col h_full overflow_hidden>
                        {self.render_active_view(cx)}
                    </div>
                </div>
            })
            .child(crate::components::footer::render_footer(cx))
            .children(notification_layer)
            .children(self.render_profile_auth_overlay(window, cx))
            .children(self.render_admin_auth_overlay(window, cx))
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
