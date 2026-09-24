use crate::views::cron_tasks::CronTasksView;
use crate::views::dashboard::{DashboardEvent, DashboardView};
use crate::views::form_inventory_view::{FormInventoryEvent, FormInventoryView};
use crate::views::global_dashboard::{GlobalDashboardEvent, GlobalDashboardView};
use crate::views::import_export::{ImportExportEvent, ImportExportView};
use crate::views::lock_screen::{LockScreenEvent, LockScreenView};
use crate::views::notifications::{NotificationsEvent, NotificationsView};
use crate::views::profile_manager::ProfileManagerView;
use crate::views::settings::{SettingsEvent, SettingsView};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{InputEvent, InputState, OtpEvent, OtpState};
use gpui_component::*;
use gpui_rsx::rsx;

use crate::components::rate_limiter::RateLimiter;
use bir_core::db::Database;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    FormInventory,
    ProfileManager,
    CronTasks,
    Notifications,
    ImportExport,
    Settings,
    AdminCalendarDashboard,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProfileTargetAction {
    ViewDashboard,
    EditProfile,
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

/// Why a lifecycle request was refused before touching the database.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleRefusal {
    /// Deleting is irreversible, so it is reachable only from the archived
    /// state. An active profile must be archived first.
    DeleteNeedsArchivedProfile,
    /// The profile is already in the state the action would move it to.
    AlreadyInTargetState,
}

impl LifecycleRefusal {
    pub(crate) fn message(self, action: ProfileLifecycleAction) -> String {
        match self {
            Self::DeleteNeedsArchivedProfile => {
                "Archive this profile before deleting it. Deleting cannot be undone.".to_string()
            }
            Self::AlreadyInTargetState => {
                format!("This profile is already {}.", action.past_tense())
            }
        }
    }
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

    /// Whether this action is legal for a profile that is currently archived
    /// (or not), independent of any UI that offered it.
    ///
    /// The editor only renders Delete for an archived profile, but a markup
    /// condition is not an invariant: any other caller reaching
    /// `perform_profile_lifecycle` would otherwise export-and-delete a live
    /// profile in one step. The transition is checked here so the rule holds
    /// wherever the request comes from.
    pub(crate) fn refusal_for(self, is_archived: bool) -> Option<LifecycleRefusal> {
        match (self, is_archived) {
            (Self::Delete, false) => Some(LifecycleRefusal::DeleteNeedsArchivedProfile),
            (Self::Archive, true) | (Self::Restore, false) => {
                Some(LifecycleRefusal::AlreadyInTargetState)
            }
            _ => None,
        }
    }
}

pub struct AppState {
    pub(crate) active_view: ActiveView,
    pub(crate) profile_manager: Entity<ProfileManagerView>,
    pub(crate) dashboard_view: Entity<DashboardView>,
    pub(crate) global_dashboard_view: Entity<GlobalDashboardView>,
    pub(crate) cron_tasks_view: Entity<CronTasksView>,
    pub(crate) notifications_view: Entity<NotificationsView>,
    pub(crate) import_export_view: Entity<ImportExportView>,
    pub(crate) settings_view: Entity<SettingsView>,
    pub(crate) admin_calendar_dashboard_view:
        Entity<crate::views::admin_calendar_dashboard::AdminCalendarDashboard>,
    pub(crate) form_inventory_view: Option<Entity<FormInventoryView>>,
    pub(crate) pending_inventory: Option<(String, TaxpayerProfile, u16, u8)>,
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
    /// Painted bounds of the sidebar and the content column, for the agent tree.
    pub(crate) layout_probe: crate::layout_probe::LayoutProbe,
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
    /// Set when a profile is deleted while its editor is open. The editor must
    /// be cleared, but the delete completes inside an async task with no
    /// `Window`, and `reset_for_new` needs one - so the reset is applied at the
    /// top of the next `render`, the same way `admin_lifecycle_authorized` is.
    pub(crate) profile_editor_needs_reset: bool,
    /// Result of a background Google Calendar sync, waiting for a `render` that
    /// holds a `Window` to turn it into a notification.
    pub(crate) pending_calendar_notice: Option<(bool, String)>,
    pub(crate) admin_otp_state: Entity<OtpState>,
    pub(crate) admin_totp_state: Entity<OtpState>,
    pub(crate) admin_rate_limiter: RateLimiter,
    pub(crate) has_admin_totp: bool,
    pub(crate) admin_auth_error: Option<String>,
    pub(crate) admin_os_auth_triggered: bool,
    /// The TIN of the currently active/unlocked profile session (only meaningful when hide_tax_profiles is enabled)
    pub(crate) active_session_tin: Option<String>,
    #[cfg(feature = "agent")]
    pub(crate) agent_mailbox: Option<gpui_agent::mailbox::AgentMailbox>,
    /// Configured `GPUI_AGENT_TOKEN` from `from_env`. Never log this value.
    #[cfg(feature = "agent")]
    pub(crate) agent_token: Option<String>,
    #[cfg(feature = "agent")]
    pub(crate) agent_shutdown: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    #[cfg(feature = "agent")]
    pub(crate) agent_refresh: Option<Task<()>>,
    /// Semantic submit reached the existing confirmation gate without queuing.
    #[cfg(feature = "agent")]
    pub(crate) agent_submit_confirmation_visible: bool,
    #[cfg(feature = "agent")]
    pub(crate) agent_request_backlog:
        std::collections::VecDeque<gpui_agent::mailbox::MailboxRequest>,
    #[cfg(feature = "agent")]
    pub(crate) pending_keybinding: Option<crate::agent::drain::PendingKeybindingFire>,
    #[cfg(feature = "agent")]
    pub(crate) last_keybinding_result: Option<(String, Result<gpui_agent::DispatchResult, String>)>,
    #[cfg(feature = "agent")]
    pub(crate) scrolled_job: Option<crate::agent::scrolled_shot::ScrolledShotJob>,
    #[cfg(feature = "agent")]
    pub(crate) toggling_visibility: bool,
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
            |this: &mut Self, _entity, event: &OtpEvent, window, cx| {
                if let OtpEvent::Change = event {
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
            |this: &mut Self, _entity, event: &OtpEvent, window, cx| {
                if let OtpEvent::Change = event {
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
            |this: &mut Self, _entity, event: &OtpEvent, window, cx| {
                if let OtpEvent::Change = event {
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
            |this: &mut Self, _entity, event: &OtpEvent, window, cx| {
                if let OtpEvent::Change = event {
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

        let db_clone_notifications = Arc::clone(&db);
        let notifications_view =
            cx.new(|cx| NotificationsView::new(db_clone_notifications, window, cx));

        // The view emits rather than navigating itself, so routing stays here
        // with the rest of the shell's knowledge of how views connect.
        cx.subscribe_in(
            &notifications_view,
            window,
            |this, _view, event: &NotificationsEvent, _window, cx| {
                match event {
                    // Both land in the profile manager; the Google reconnect
                    // control lives on its Email Settings tab.
                    NotificationsEvent::ReconnectGoogleAccount
                    | NotificationsEvent::OpenProfileManager => {
                        this.active_view = ActiveView::ProfileManager;
                    }
                    NotificationsEvent::Open => {
                        this.active_view = ActiveView::Notifications;
                    }
                }
                cx.notify();
            },
        )
        .detach();

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
                DashboardEvent::AddToNativeCalendar(tin) => {
                    this.add_profile_to_native_calendar(tin.clone(), window, cx);
                }
                DashboardEvent::SyncGoogleCalendar(tin) => {
                    this.sync_profile_google_calendar(tin.clone(), window, cx);
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
            notifications_view,
            import_export_view,
            form_inventory_view: None,
            pending_inventory: None,
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
            layout_probe: crate::layout_probe::LayoutProbe::default(),
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
            profile_editor_needs_reset: false,
            pending_calendar_notice: None,
            admin_otp_state,
            admin_totp_state,
            admin_rate_limiter: RateLimiter::new(),
            has_admin_totp,
            admin_auth_error: None,
            admin_os_auth_triggered: false,
            active_session_tin: None,
            #[cfg(feature = "agent")]
            agent_mailbox: None,
            #[cfg(feature = "agent")]
            agent_token: None,
            #[cfg(feature = "agent")]
            agent_shutdown: None,
            #[cfg(feature = "agent")]
            agent_refresh: None,
            #[cfg(feature = "agent")]
            agent_submit_confirmation_visible: false,
            #[cfg(feature = "agent")]
            agent_request_backlog: std::collections::VecDeque::new(),
            #[cfg(feature = "agent")]
            pending_keybinding: None,
            #[cfg(feature = "agent")]
            last_keybinding_result: None,
            #[cfg(feature = "agent")]
            scrolled_job: None,
            #[cfg(feature = "agent")]
            toggling_visibility: false,
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

    /// Writes this profile's deadlines as an `.ics` and hands it to whatever the
    /// platform registered for calendar files - Calendar on macOS, the default
    /// handler on Windows, `xdg-open` on Linux.
    ///
    /// This is the always-available calendar path: it needs no account and no
    /// network. It renders the same event set the Google sync would push, so the
    /// two cannot disagree about which obligations are deadlines.
    fn add_profile_to_native_calendar(
        &mut self,
        tin: String,
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
                "Calendar not created",
                &format!("{tin} is no longer loaded."),
                window,
                cx,
            );
            return;
        };

        let built = match self.db.lock() {
            Ok(db) => bir_core::google_calendar::build_desired_events(&db, &profile)
                .map_err(|error| error.to_string()),
            Err(_) => Err("The profile database is temporarily unavailable".to_string()),
        };
        let (events, excluded_undated) = match built {
            Ok(pair) => pair,
            Err(error) => {
                push_notification("error", "Calendar not created", &error, window, cx);
                return;
            }
        };

        if events.is_empty() {
            push_notification(
                "info",
                "Nothing to add",
                "This profile has no dated deadlines for its registered Forms Set yet.",
                window,
                cx,
            );
            return;
        }

        let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let directory = bir_core::platform::data_dir().join("calendars");
        let written = bir_core::calendar_ics::write_profile_calendar_ics(
            &directory, &profile, &events, &stamp,
        );

        let path = match written {
            Ok(path) => path,
            Err(error) => {
                push_notification(
                    "error",
                    "Calendar not created",
                    &format!("Could not write the calendar file: {error}"),
                    window,
                    cx,
                );
                return;
            }
        };

        if let Err(error) = open::that(&path) {
            // The file is still on disk and importable by hand, so say where.
            push_notification(
                "error",
                "Calendar file created",
                &format!(
                    "Saved to {} but no calendar application could be opened: {error}",
                    path.display()
                ),
                window,
                cx,
            );
            return;
        }

        let detail = if excluded_undated > 0 {
            format!(
                "{} deadlines handed to your calendar app. {excluded_undated} without a resolved date were skipped.",
                events.len()
            )
        } else {
            format!("{} deadlines handed to your calendar app.", events.len())
        };
        push_notification("success", "Added to Calendar", &detail, window, cx);
    }

    /// Pushes this profile's deadlines to its linked Google Calendar.
    ///
    /// `sync_profile_calendar` is blocking - an OAuth refresh plus one HTTP round
    /// trip per event - so running it inline froze the whole window for its
    /// duration. It goes to a worker thread instead, the same shape the editor's
    /// own Calendar tab uses in `run_calendar_action`, and the result is parked
    /// for `render` to surface because this completes without a `Window`.
    fn sync_profile_google_calendar(
        &mut self,
        tin: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        push_notification(
            "info",
            "Syncing Google Calendar",
            "Working in the background...",
            window,
            cx,
        );

        let db = self.db.clone();
        cx.spawn(async move |this, cx| {
            let (tx, rx) = tokio::sync::oneshot::channel();
            std::thread::spawn(move || {
                let _ = tx.send(bir_core::google_calendar::sync_profile_calendar(db, &tin));
            });
            let result = rx
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Calendar worker stopped")));

            let _ = this.update(cx, |this, cx| {
                this.pending_calendar_notice = Some(match result {
                    Ok(report) => {
                        let mut parts = Vec::new();
                        if report.inserted > 0 {
                            parts.push(format!("{} added", report.inserted));
                        }
                        if report.updated > 0 {
                            parts.push(format!("{} updated", report.updated));
                        }
                        if report.deleted > 0 {
                            parts.push(format!("{} removed", report.deleted));
                        }
                        if report.excluded_undated > 0 {
                            parts.push(format!("{} undated skipped", report.excluded_undated));
                        }
                        // Everything already matching is the steady state, so say
                        // that rather than reporting a row of zeroes.
                        let detail = if parts.is_empty() {
                            format!("Already up to date - {} deadlines match.", report.unchanged)
                        } else {
                            parts.join(", ")
                        };
                        (true, detail)
                    }
                    Err(error) => (false, error.to_string()),
                });
                cx.notify();
            });
        })
        .detach();
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

        // The transition is validated against the profile's real stored state,
        // not against whichever control happened to be on screen.
        if let Some(refusal) = action.refusal_for(profile.is_archived) {
            push_notification(
                "error",
                "Profile unchanged",
                &refusal.message(action),
                window,
                cx,
            );
            return;
        }

        match action {
            ProfileLifecycleAction::Archive => {
                self.persist_profile_archived(profile, true, window, cx);
                self.adopt_archived_state_in_editor(&tin, true, cx);
            }
            ProfileLifecycleAction::Restore => {
                self.persist_profile_archived(profile, false, window, cx);
                self.adopt_archived_state_in_editor(&tin, false, cx);
            }
            ProfileLifecycleAction::Delete => self.export_and_delete_profile(tin, cx),
        }
    }

    /// Tells the open editor about an archived flag this method just wrote.
    ///
    /// The editor caches `stored_is_archived` at load time and writes it back on
    /// every save. Archive and Restore are now reachable from inside that editor,
    /// directly above its Save button, so without this the next save would revert
    /// the change the user just made.
    fn adopt_archived_state_in_editor(
        &mut self,
        tin: &str,
        archived: bool,
        cx: &mut Context<Self>,
    ) {
        let editing_this_profile = self
            .profile_manager
            .read_with(cx, |view, cx| view.is_editing_tin(tin, cx));
        if editing_this_profile {
            self.profile_manager
                .update(cx, |view, cx| view.adopt_archived_state(archived, cx));
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
                // Delete is only reachable from this profile's own editor, so
                // that editor is still open on a row that no longer exists. Its
                // `editing_id` would make the next save fall through to the
                // INSERT branch and resurrect the profile just deleted.
                if this
                    .profile_manager
                    .read_with(cx, |view, cx| view.is_editing_tin(&tin, cx))
                {
                    this.profile_editor_needs_reset = true;
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

    pub(crate) fn open_named_form(
        &mut self,
        form_code: &str,
        year: u16,
        quarter: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.handle_file_form(
            &DashboardEvent::FileForm {
                form_code: form_code.to_string(),
                year,
                quarter,
            },
            window,
            cx,
        );
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

    fn render_form_page(&self, fallback: Option<AnyElement>) -> AnyElement {
        if let Some(view) = &self.form_inventory_view {
            view.clone().into_any_element()
        } else {
            fallback.unwrap_or_else(|| rsx! { <div>{"No form loaded"}</div> }.into_any_element())
        }
    }

    fn render_active_view(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let page = match self.active_view {
            ActiveView::GlobalDashboard => self.global_dashboard_view.clone().into_any_element(),
            ActiveView::ProfileManager => self.profile_manager.clone().into_any_element(),
            ActiveView::CronTasks => self.cron_tasks_view.clone().into_any_element(),
            ActiveView::Notifications => self.notifications_view.clone().into_any_element(),
            ActiveView::ImportExport => self.import_export_view.clone().into_any_element(),
            ActiveView::Settings => self.settings_view.clone().into_any_element(),
            ActiveView::AdminCalendarDashboard => self
                .admin_calendar_dashboard_view
                .clone()
                .into_any_element(),
            ActiveView::Dashboard => self.dashboard_view.clone().into_any_element(),
            ActiveView::Form2551Q
            | ActiveView::Form1701Q
            | ActiveView::Form1601C
            | ActiveView::Form0619E
            | ActiveView::Form0619F
            | ActiveView::Form0605
            | ActiveView::Form2550Q
            | ActiveView::Form1701
            | ActiveView::Form1702RT
            | ActiveView::Form1702MX
            | ActiveView::FormInventory => self.render_form_page(None),
        };
        div()
            .id(crate::agent::ids::page_root(self.active_view))
            .size_full()
            .child(page)
            .into_any_element()
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
                #[cfg(feature = "agent")]
                self.release_agent_listener();
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
            || (is_certification_build && bir_core::forms::can_open_certification_draft(form_code))
            || bir_core::forms::can_open_inventory_editor(form_code);
        if !can_open {
            tracing::warn!(
                form_code,
                "Attempted to open an uncertified or unsupported form — manual filing required"
            );
            return;
        }

        if bir_core::forms::has_inventory(form_code)
            && let Some(tin) = &self.active_profile_tin
            && let Some(profile) = self.profiles.iter().find(|p| p.tin.full() == *tin)
        {
            self.pending_inventory = Some((form_code.clone(), profile.clone(), year, quarter));
            self.active_view = crate::agent::ids::active_view_for_form_code(form_code)
                .unwrap_or(ActiveView::FormInventory);
            cx.notify();
            return;
        }

        tracing::warn!(form_code, "Form open requested without an inventory editor");
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
        #[cfg(feature = "agent")]
        crate::agent::apply_agent(self, window, cx);

        // Apply a lifecycle change the administrator prompt authorised. It
        // lands here because this is the first point after the prompt
        // resolves that is guaranteed to hold a `Window`.
        if let Some((tin, action)) = self.admin_lifecycle_authorized.take() {
            self.perform_profile_lifecycle(tin, action, window, cx);
        }

        // Clear an editor left bound to a profile that has since been deleted.
        if std::mem::take(&mut self.profile_editor_needs_reset) {
            self.profile_manager
                .update(cx, |view, cx| view.reset_for_new(window, cx));
        }

        if let Some((ok, detail)) = self.pending_calendar_notice.take() {
            push_notification(
                if ok { "success" } else { "error" },
                if ok {
                    "Google Calendar synced"
                } else {
                    "Google Calendar not synced"
                },
                &detail,
                window,
                cx,
            );
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
        if let Some((code, profile, year, slot)) = self.pending_inventory.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| {
                FormInventoryView::new(&code, &profile, year, slot, db_for_view, window, cx)
            });
            cx.subscribe_in(
                &form_view,
                window,
                |this: &mut Self, _entity, event: &FormInventoryEvent, window, cx| match event {
                    FormInventoryEvent::BackToDashboard => {
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
                    FormInventoryEvent::PushNotification(level, title, message) => {
                        push_notification(level, title, message, window, cx);
                    }
                    FormInventoryEvent::Saved | FormInventoryEvent::Submitted => {
                        cx.notify();
                    }
                },
            )
            .detach();
            self.form_inventory_view = Some(form_view);
        }

        if let Some((profile, action)) = self.unlocked_profile.take() {
            self.apply_profile_action(profile, action, window, cx);
            self.focus_handle.focus(window, cx);
        }

        let notification_layer = Root::render_notification_layer(window, cx);

        if self.is_locked
            && let Some(lock_screen) = &self.lock_screen_view
        {
            let root = rsx! { <div id={crate::agent::ids::PAGE_LOCK} size_full>{lock_screen.clone()}</div> };
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
            .on_action(cx.listener(Self::handle_toggle_app_visibility))
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
                        this.child(
                            div()
                                .relative()
                                .flex_none()
                                .h_full()
                                .child(crate::layout_probe::bounds_probe(
                                    self.layout_probe.sidebar.clone(),
                                ))
                                .child(self.render_sidebar(window, cx)),
                        )
                    })}
                >
                    <div flex_1 flex flex_col h_full overflow_hidden relative>
                        {crate::layout_probe::bounds_probe(self.layout_probe.page.clone())}
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
        #[cfg(feature = "agent")]
        self.release_agent_listener();
        // Flush any pending WAL data to the main database file before shutdown
        if let Ok(db) = self.db.lock()
            && let Err(e) = db.checkpoint()
        {
            eprintln!("Warning: WAL checkpoint on shutdown failed: {e}");
        }
    }
}

#[cfg(test)]
mod profile_lifecycle_transition_tests {
    use super::{LifecycleRefusal, ProfileLifecycleAction};

    const ARCHIVED: bool = true;
    const ACTIVE: bool = false;

    /// The editor only renders Delete for an archived profile. That is a
    /// markup condition, not an invariant - this is the invariant.
    #[test]
    fn deleting_an_active_profile_is_refused() {
        assert_eq!(
            ProfileLifecycleAction::Delete.refusal_for(ACTIVE),
            Some(LifecycleRefusal::DeleteNeedsArchivedProfile),
        );
    }

    #[test]
    fn deleting_an_archived_profile_is_allowed() {
        assert_eq!(ProfileLifecycleAction::Delete.refusal_for(ARCHIVED), None);
    }

    #[test]
    fn archive_and_restore_are_allowed_only_from_the_opposite_state() {
        assert_eq!(ProfileLifecycleAction::Archive.refusal_for(ACTIVE), None);
        assert_eq!(ProfileLifecycleAction::Restore.refusal_for(ARCHIVED), None);
    }

    /// A double-submit must not re-write the same state.
    #[test]
    fn repeating_an_action_that_already_happened_is_refused() {
        assert_eq!(
            ProfileLifecycleAction::Archive.refusal_for(ARCHIVED),
            Some(LifecycleRefusal::AlreadyInTargetState),
        );
        assert_eq!(
            ProfileLifecycleAction::Restore.refusal_for(ACTIVE),
            Some(LifecycleRefusal::AlreadyInTargetState),
        );
    }

    /// The archive-first rule is the one the user has to act on, so it must
    /// say what to do rather than only that something was refused.
    #[test]
    fn the_delete_refusal_names_the_required_step() {
        let msg =
            LifecycleRefusal::DeleteNeedsArchivedProfile.message(ProfileLifecycleAction::Delete);
        assert!(msg.contains("Archive"), "{msg}");
        assert!(msg.contains("cannot be undone"), "{msg}");
    }

    #[test]
    fn the_already_in_state_refusal_names_the_state() {
        assert!(
            LifecycleRefusal::AlreadyInTargetState
                .message(ProfileLifecycleAction::Archive)
                .contains("already archived"),
        );
    }
}
