//! Settings (port of `nsh/screens/settings/`): a section list beside the section's cards, a search
//! over rows. Sections: General, Account, Import / Export, Jobs, Computer.

use std::cell::RefCell;

use cui::Ui;
use serde_json::{Value, json};

use crate::lib::settings_model::{self, SettingsViewSection};
use crate::state::{self, AppState, ViewName};
use crate::theme::ThemePreference;
use crate::types::BackgroundJob;
use crate::ui::kit::UiExt;

thread_local! {
    static QUERY: RefCell<String> = const { RefCell::new(String::new()) };
    static SECTION: RefCell<String> = const { RefCell::new(String::new()) };
    static JOBS_TAB: RefCell<String> = RefCell::new(String::from("jobs"));
    static JOBS_FILTER: RefCell<String> = RefCell::new(String::from("All Jobs"));
    static LOG_FILTER: RefCell<String> = RefCell::new(String::from("All Logs"));
    static JOBS_FAILURE: RefCell<Option<String>> = const { RefCell::new(None) };
    static LOG_TAIL: RefCell<Option<Value>> = const { RefCell::new(None) };
    static LOG_TAIL_ERR: RefCell<Option<String>> = const { RefCell::new(None) };
    /// Narrow layout: the section list or the open section (Settings.tsx `open`).
    static NARROW_OPEN: RefCell<bool> = const { RefCell::new(false) };
    /// The `section` route param already applied this route (it is read once, like the TSX mount).
    static LAST_PARAM: RefCell<Option<String>> = const { RefCell::new(None) };
    // Security card (securityState.ts): the typed PIN, authenticator code, enrolment secret and
    // the one-time recovery key.
    static SEC_PIN: RefCell<String> = const { RefCell::new(String::new()) };
    static SEC_TOTP_CODE: RefCell<String> = const { RefCell::new(String::new()) };
    static SEC_TOTP_SECRET: RefCell<Option<String>> = const { RefCell::new(None) };
    static SEC_RECOVERY: RefCell<Option<String>> = const { RefCell::new(None) };
    // Computer setup change (setup.ts): the step, the picked target, the typed codes.
    static SETUP_STEP: RefCell<String> = const { RefCell::new(String::new()) };
    static SETUP_TARGET: RefCell<Option<Value>> = const { RefCell::new(None) };
    static SETUP_USE_CODE: RefCell<bool> = const { RefCell::new(false) };
    static SETUP_NEW_PIN: RefCell<String> = const { RefCell::new(String::new()) };
    static SETUP_CONFIRM_PIN: RefCell<String> = const { RefCell::new(String::new()) };
    static SETUP_CONFIRM_CODE: RefCell<String> = const { RefCell::new(String::new()) };
    static SETUP_RECOVERY: RefCell<Option<String>> = const { RefCell::new(None) };
    // Buwiz card (cloud.ts): the last status, the busy flag, the last failure, the in-flight call.
    static CLOUD_STATUS: RefCell<Option<Value>> = const { RefCell::new(None) };
    static CLOUD_BUSY: RefCell<bool> = const { RefCell::new(false) };
    static CLOUD_FAILURE: RefCell<Option<String>> = const { RefCell::new(None) };
    static CLOUD_PENDING: RefCell<Option<(CloudJob, crate::bir::AsyncReply)>> =
        const { RefCell::new(None) };
    /// The once-per-run server check of a saved sign-in (openAccount's freshStarted).
    static CLOUD_FRESH: RefCell<bool> = const { RefCell::new(false) };
    // The global-hotkey keybinding editor: open while the capture dialog is up; `draft` is the
    // chord recorded so far ("" until a full modifier+key lands); `captured` arms Save.
    static HOTKEY_EDIT: RefCell<HotkeyEdit> = const {
        RefCell::new(HotkeyEdit {
            open: false,
            draft: String::new(),
            captured: false,
            target: HotkeyTarget::GlobalShow,
        })
    };
}

/// Which binding the capture dialog records.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum HotkeyTarget {
    /// Settings › Computer › "Global show hotkey" (writes `set_global_hotkey`).
    #[default]
    GlobalShow,
    /// Settings › Computer › "Quit shortcut" (writes the `quit_shortcut` setting).
    Quit,
}

/// The keybinding editor's state (Computer › Shortcuts).
#[derive(Default)]
struct HotkeyEdit {
    open: bool,
    /// The stored-format combo being recorded, e.g. "control+alt+E".
    draft: String,
    /// A complete chord (modifier + key) has been pressed since the dialog opened.
    captured: bool,
    target: HotkeyTarget,
}

/// True while the hotkey capture dialog is up: the frame's global shortcuts must not fire.
pub fn hotkey_editing() -> bool {
    HOTKEY_EDIT.with(|e| e.borrow().open)
}

/// The cloud calls the Buwiz card runs one after another (signIn's begin -> finish).
#[derive(Clone, Copy)]
enum CloudJob {
    Begin,
    Finish,
    Refresh,
    SignOut,
}

/// The next session starts from the defaults: no search, the first section.
pub fn reset_for_session() {
    QUERY.with(|q| q.borrow_mut().clear());
    SECTION.with(|s| *s.borrow_mut() = "general".to_owned());
    JOBS_TAB.with(|t| *t.borrow_mut() = "jobs".to_owned());
    JOBS_FILTER.with(|f| *f.borrow_mut() = "All Jobs".to_owned());
    LOG_FILTER.with(|f| *f.borrow_mut() = "All Logs".to_owned());
    JOBS_FAILURE.with(|f| *f.borrow_mut() = None);
    LOG_TAIL.with(|t| *t.borrow_mut() = None);
    LOG_TAIL_ERR.with(|e| *e.borrow_mut() = None);
    NARROW_OPEN.with(|o| *o.borrow_mut() = false);
    LAST_PARAM.with(|p| *p.borrow_mut() = None);
    SEC_PIN.with(|v| v.borrow_mut().clear());
    SEC_TOTP_CODE.with(|v| v.borrow_mut().clear());
    SEC_TOTP_SECRET.with(|v| *v.borrow_mut() = None);
    SEC_RECOVERY.with(|v| *v.borrow_mut() = None);
    reset_setup_change();
    CLOUD_STATUS.with(|v| *v.borrow_mut() = None);
    CLOUD_BUSY.with(|v| *v.borrow_mut() = false);
    CLOUD_FAILURE.with(|v| *v.borrow_mut() = None);
    CLOUD_PENDING.with(|v| *v.borrow_mut() = None);
    CLOUD_FRESH.with(|v| *v.borrow_mut() = false);
}

/// Leaving Settings (Settings.tsx's beforeLeave + cleanup): half-typed codes and the recovery
/// key on screen go; a finished setup change signs the computer out (leaveSetup/leaveSecurity).
pub fn leave(app: &mut AppState) {
    let changed = setup_step() == "key";
    SEC_PIN.with(|v| v.borrow_mut().clear());
    SEC_TOTP_CODE.with(|v| v.borrow_mut().clear());
    SEC_TOTP_SECRET.with(|v| *v.borrow_mut() = None);
    SEC_RECOVERY.with(|v| *v.borrow_mut() = None);
    reset_setup_change();
    if changed {
        app.sign_out_after_setup_change();
    }
}

/// Forgets the setup change's step and everything typed (resetSetup).
fn reset_setup_change() {
    SETUP_STEP.with(|v| v.borrow_mut().clear());
    SETUP_TARGET.with(|v| *v.borrow_mut() = None);
    SETUP_USE_CODE.with(|v| *v.borrow_mut() = false);
    SETUP_NEW_PIN.with(|v| v.borrow_mut().clear());
    SETUP_CONFIRM_PIN.with(|v| v.borrow_mut().clear());
    SETUP_CONFIRM_CODE.with(|v| v.borrow_mut().clear());
    SETUP_RECOVERY.with(|v| *v.borrow_mut() = None);
}

fn query() -> String {
    QUERY.with(|q| q.borrow().clone())
}

fn section() -> String {
    SECTION.with(|s| s.borrow().clone())
}

fn set_section(id: &str) {
    SECTION.with(|s| *s.borrow_mut() = id.to_owned());
}

/// The sections and rows the search keeps for this person.
fn view(app: &AppState) -> Vec<SettingsViewSection> {
    settings_model::settings_view(&query(), app.is_admin())
}

fn row_shown(section: &SettingsViewSection, row: &str) -> bool {
    section.rows.iter().any(|r| *r == row)
}

// ---------------------------------------------------------------- rows

fn job_sub(job: &BackgroundJob) -> String {
    settings_model::job_sub(job)
}

fn draw_general(ui: &mut Ui, app: &mut AppState, sec: &SettingsViewSection) {
    ui.card(&Default::default(), |ui| {
        ui.section_title("Appearance");
        if row_shown(sec, "theme") {
            ui.row(&Default::default(), |ui| {
                ui.text("Theme");
                ui.muted("System follows this computer");
                ui.spacer();
                let mut index = match app.theme_pref {
                    ThemePreference::System => 0,
                    ThemePreference::Light => 1,
                    ThemePreference::Dark => 2,
                };
                if crate::ui::kit::select_with_icons(
                    ui,
                    "settings-theme",
                    &["System", "Light", "Dark"],
                    &["monitor", "sun", "moon"],
                    &mut index,
                ) {
                    let pref = match index {
                        1 => ThemePreference::Light,
                        2 => ThemePreference::Dark,
                        _ => ThemePreference::System,
                    };
                    if pref != app.theme_pref {
                        app.set_theme_preference(pref);
                    }
                }
            });
        }
        if row_shown(sec, "recent") {
            let mut show = app.prefs.show_recent;
            if ui.switch("Show recent returns in the sidebar", &mut show) {
                app.set_pref(Some(show), None);
            }
        }
        if row_shown(sec, "compact") {
            let mut compact = app.prefs.compact_form_nav;
            if ui.switch("Compact form section navigation", &mut compact) {
                app.set_pref(None, Some(compact));
            }
        }
        if row_shown(sec, "updates") {
            // autoDownload(): the host's last status answer.
            let auto = app
                .update_status
                .as_ref()
                .map(|u| u.auto_download)
                .unwrap_or_else(crate::bir::auto_download);
            let mut on = auto;
            if ui.switch("Download updates automatically", &mut on) && on != auto {
                match crate::store::write("set_auto_update_download", vec![json!(on)]) {
                    Ok(_) => app.toast(
                        if on {
                            "Newer releases will download by themselves."
                        } else {
                            "Newer releases will wait for you to download them."
                        },
                        None,
                        None,
                    ),
                    Err(e) => app.toast(
                        crate::lib::errors::error_text(&e),
                        Some("triangle-alert"),
                        None,
                    ),
                };
            }
            ui.muted("A newer release downloads in the background. You choose when to install.");
            if let Some(u) = &app.update_status {
                ui.muted(&u.message.clone().unwrap_or_else(|| u.phase.clone()));
            }
        }
    });
}

// ---------------------------------------------------------------- Account blocks

/// A small fg3 label over what it names (parts.tsx Labeled).
fn labeled(ui: &mut Ui, label: &str, f: impl FnOnce(&mut Ui)) {
    let p = crate::ui::kit::frame_palette();
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            gap: 8.0,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                label,
                &cui::Text {
                    size: crate::theme::r#type::META_STRONG.0,
                    color: p.subtle_foreground,
                    ..Default::default()
                },
            );
            f(ui);
        },
    );
}

/// A settings row: title (control), hint (meta fg3), the control at the right (SettingRow).
fn setting_row(ui: &mut Ui, title: &str, hint: &str, f: impl FnOnce(&mut Ui)) {
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            gap: 12.0,
            ..Default::default()
        },
        |ui| {
            ui.column(
                &cui::Column {
                    width: cui::Size::FILL,
                    ..Default::default()
                },
                |ui| {
                    ui.text(title);
                    if !hint.is_empty() {
                        ui.muted(hint);
                    }
                },
            );
            f(ui);
        },
    );
}

/// A row whose controls sit under its words (StackRow): title + hint, then a wrapping control row.
fn stack_row(ui: &mut Ui, title: &str, hint: &str, f: impl FnOnce(&mut Ui)) {
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            gap: 12.0,
            ..Default::default()
        },
        |ui| {
            ui.text(title);
            if !hint.is_empty() {
                ui.muted(hint);
            }
            ui.row(
                &cui::Row {
                    width: cui::Size::FILL,
                    gap: 8.0,
                    wrap: true,
                    align: cui::Align::Center,
                    ..Default::default()
                },
                f,
            );
        },
    );
}

// ---- Buwiz (cloud.ts): the cloud account's status, refresh, sign in and sign out.

fn cloud_status_now() -> Value {
    CLOUD_STATUS.with(|s| {
        s.borrow().clone().unwrap_or_else(|| {
            match crate::store::call("cloud_account_status", vec![]) {
                Ok(v) => v,
                Err(e) => {
                    CLOUD_FAILURE
                        .with(|f| *f.borrow_mut() = Some(crate::lib::errors::error_text(&e)));
                    json!({
                        "server_url": "", "server_url_source": "", "signed_in": false,
                        "email": null, "user_id": null, "offline": false,
                        "detail": crate::lib::errors::error_text(&e),
                    })
                }
            }
        })
    })
}

fn cloud_field(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn cloud_start(job: CloudJob, name: &str) {
    if CLOUD_BUSY.with(|b| *b.borrow()) {
        return;
    }
    CLOUD_BUSY.with(|b| *b.borrow_mut() = true);
    let rx = crate::bir::call_async_spawn(name, vec![]);
    CLOUD_PENDING.with(|p| *p.borrow_mut() = Some((job, rx)));
}

/// The finished cloud call's effect (cloud.ts refresh/signIn/signOut).
fn cloud_drain(app: &mut AppState) {
    let mut done: Option<(CloudJob, Result<Value, String>)> = None;
    CLOUD_PENDING.with(|p| {
        let mut b = p.borrow_mut();
        if let Some((job, rx)) = b.as_mut()
            && let Ok(r) = rx.try_recv()
        {
            done = Some((*job, r));
            *b = None;
        }
    });
    // openAccount: once per run, check a saved sign-in with the server.
    if done.is_none() {
        let signed_in = cloud_status_now()
            .get("signed_in")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if signed_in
            && !CLOUD_FRESH.with(|f| *f.borrow())
            && !CLOUD_BUSY.with(|b| *b.borrow())
            && CLOUD_PENDING.with(|p| p.borrow().is_none())
        {
            CLOUD_FRESH.with(|f| *f.borrow_mut() = true);
            cloud_start(CloudJob::Refresh, "cloud_account_refresh");
        }
        return;
    }
    let (job, result) = done.unwrap();
    match (job, result) {
        (CloudJob::Begin, Ok(started)) => {
            let code = cloud_field(&started, "user_code");
            let browser = started
                .get("browser_opened")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let uri = cloud_field(&started, "verification_uri_complete");
            let detail = if browser {
                format!(
                    "Finish signing in in the browser, then return here.{}",
                    if code.is_empty() {
                        String::new()
                    } else {
                        format!(" Code {code}.")
                    }
                )
            } else {
                format!(
                    "Open {uri} and approve this computer.{}",
                    if code.is_empty() {
                        String::new()
                    } else {
                        format!(" Code {code}.")
                    }
                )
            };
            CLOUD_STATUS.with(|s| {
                if let Some(st) = s.borrow_mut().as_mut() {
                    if let Some(obj) = st.as_object_mut() {
                        obj.insert("detail".to_owned(), json!(detail));
                    }
                }
            });
            cloud_start(CloudJob::Finish, "cloud_account_finish_sign_in");
        }
        (CloudJob::Begin, Err(e)) => {
            CLOUD_FAILURE.with(|f| *f.borrow_mut() = Some(crate::lib::errors::error_text(&e)));
            CLOUD_STATUS.with(|s| {
                *s.borrow_mut() = crate::store::call("cloud_account_status", vec![]).ok()
            });
            CLOUD_BUSY.with(|b| *b.borrow_mut() = false);
        }
        (CloudJob::Finish, Ok(v)) => {
            CLOUD_STATUS.with(|s| *s.borrow_mut() = Some(v));
            CLOUD_FAILURE.with(|f| *f.borrow_mut() = None);
            CLOUD_BUSY.with(|b| *b.borrow_mut() = false);
            app.finish_unlock();
        }
        (CloudJob::Finish, Err(e)) => {
            CLOUD_FAILURE.with(|f| *f.borrow_mut() = Some(crate::lib::errors::error_text(&e)));
            CLOUD_STATUS.with(|s| {
                *s.borrow_mut() = crate::store::call("cloud_account_status", vec![]).ok()
            });
            CLOUD_BUSY.with(|b| *b.borrow_mut() = false);
        }
        (CloudJob::Refresh, Ok(v)) => {
            CLOUD_STATUS.with(|s| *s.borrow_mut() = Some(v));
            CLOUD_FAILURE.with(|f| *f.borrow_mut() = None);
        }
        (CloudJob::Refresh, Err(e)) => {
            CLOUD_FAILURE.with(|f| *f.borrow_mut() = Some(crate::lib::errors::error_text(&e)));
        }
        (CloudJob::SignOut, Ok(v)) => {
            CLOUD_STATUS.with(|s| *s.borrow_mut() = Some(v));
            CLOUD_FAILURE.with(|f| *f.borrow_mut() = None);
            CLOUD_BUSY.with(|b| *b.borrow_mut() = false);
            app.lock_cloud();
        }
        (CloudJob::SignOut, Err(e)) => {
            CLOUD_FAILURE.with(|f| *f.borrow_mut() = Some(crate::lib::errors::error_text(&e)));
            CLOUD_BUSY.with(|b| *b.borrow_mut() = false);
        }
    }
}

/// Settings > Account > Buwiz (BuwizCard): the server, who is signed in, Sign in / Sign out.
fn draw_buwiz(ui: &mut Ui, app: &mut AppState) {
    cloud_drain(app);
    let status = cloud_status_now();
    let signed_in = status
        .get("signed_in")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let busy = CLOUD_BUSY.with(|b| *b.borrow());
    let failure = CLOUD_FAILURE.with(|f| f.borrow().clone());
    labeled(ui, "Buwiz", |ui| {
        ui.card(&Default::default(), |ui| {
            let url = cloud_field(&status, "server_url");
            setting_row(
                ui,
                "Server",
                settings_model::server_hint(
                    status.get("server_url_source").and_then(Value::as_str),
                ),
                |ui| {
                    ui.muted(if url.is_empty() {
                        "Not configured"
                    } else {
                        &url
                    });
                },
            );
            crate::ui::kit::row_sep(ui);
            let who = if signed_in {
                [
                    cloud_field(&status, "email"),
                    cloud_field(&status, "user_id"),
                ]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_else(|| "Signed in".to_owned())
            } else {
                "Not signed in".to_owned()
            };
            let hint = if busy {
                let d = cloud_field(&status, "detail");
                if d.is_empty() {
                    "Waiting…".to_owned()
                } else {
                    d
                }
            } else {
                failure
                    .clone()
                    .filter(|f| !f.is_empty())
                    .unwrap_or_else(|| cloud_field(&status, "detail"))
            };
            setting_row(ui, &who, &hint, |ui| {
                if signed_in {
                    if ui.ghost_button(if busy { "Signing out…" } else { "Sign out" }, !busy) {
                        cloud_start(CloudJob::SignOut, "cloud_account_sign_out");
                    }
                } else if ui.primary_button(if busy { "Waiting…" } else { "Sign in" }, !busy) {
                    cloud_start(CloudJob::Begin, "cloud_account_begin_sign_in");
                }
            });
        });
    });
    let _ = app;
}

/// Settings > Account > who is signed in (IdentityBlock): the account card and the profile card.
fn draw_identity(ui: &mut Ui, app: &mut AppState) {
    let (kind, root) = match &app.session {
        state::Session::Taxpayer { root } => (Some("taxpayer"), Some(root.as_str())),
        state::Session::Admin => (Some("admin"), None),
        state::Session::Open => (Some("open"), None),
    };
    let auth = app.auth_state();
    let summary = settings_model::account_summary(
        &app.taxpayers(),
        app.branch().as_ref(),
        app.is_admin(),
        app.is_helpdesk(),
        auth.admin_root.as_deref(),
        kind,
        root,
        app.can_add_taxpayer(),
    );
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            gap: 16.0,
            ..Default::default()
        },
        |ui| {
            ui.column(
                &cui::Column {
                    width: cui::Size::FILL,
                    gap: 16.0,
                    ..Default::default()
                },
                |ui| {
                    // The account card.
                    ui.card(&Default::default(), |ui| {
                        ui.row(
                            &cui::Row {
                                width: cui::Size::FILL,
                                gap: 12.0,
                                ..Default::default()
                            },
                            |ui| {
                                ui.avatar_with(
                                    &summary.name,
                                    &cui::Avatar {
                                        size: 40.0,
                                        ..Default::default()
                                    },
                                );
                                ui.column(
                                    &cui::Column {
                                        width: cui::Size::FILL,
                                        ..Default::default()
                                    },
                                    |ui| {
                                        ui.text(&summary.name);
                                        if summary.email.is_empty() {
                                            ui.muted(&summary.detail);
                                        } else {
                                            ui.muted(&summary.email);
                                            ui.muted(&summary.detail);
                                        }
                                    },
                                );
                            },
                        );
                        ui.row(
                            &cui::Row {
                                width: cui::Size::FILL,
                                gap: 8.0,
                                wrap: true,
                                ..Default::default()
                            },
                            |ui| {
                                if summary.show_add && ui.secondary_button("Add account", true) {
                                    app.go(ViewName::NewTaxpayer, Default::default());
                                }
                                if ui.ghost_button("Sign out", true) {
                                    app.log_out();
                                }
                            },
                        );
                    });
                    // The profile card: the registered name and email, each with a pencil to the editor.
                    if app.taxpayer().is_some() && app.branch().is_some() {
                        let tp_name = app.taxpayer().map(|t| t.name.clone()).unwrap_or_default();
                        let tin_display = app
                            .taxpayer()
                            .map(|t| t.tin_display.clone())
                            .unwrap_or_default();
                        let email = app
                            .branch()
                            .and_then(|b| b.facts.email.clone())
                            .unwrap_or_default();
                        ui.card(&Default::default(), |ui| {
                            setting_row(
                                ui,
                                "Registered name",
                                &if tin_display.is_empty() {
                                    summary.detail.clone()
                                } else {
                                    format!("TIN {tin_display}")
                                },
                                |ui| {
                                    ui.muted(if tp_name.is_empty() {
                                        "Not set"
                                    } else {
                                        &tp_name
                                    });
                                    if crate::ui::kit::icon_button(ui, "pencil", Some("Edit")) {
                                        app.open_profile(Some("taxpayer"));
                                    }
                                },
                            );
                            crate::ui::kit::row_sep(ui);
                            setting_row(ui, "Email", "Printed on this profile's returns", |ui| {
                                ui.muted(if email.is_empty() { "Not set" } else { &email });
                                if crate::ui::kit::icon_button(ui, "pencil", Some("Edit")) {
                                    app.open_profile(Some("address"));
                                }
                            });
                        });
                    }
                },
            );
        },
    );
}

// ---- Google (Google.tsx): Gmail connect, receipt tracking, Disconnect.

thread_local! {
    static GOOGLE_PENDING: RefCell<Option<crate::bir::AsyncReply>> =
        const { RefCell::new(None) };
}

/// Settings > Account > Google: connect or reconnect the profile's Gmail, the tracking switch,
/// and Disconnect once connected.
fn draw_google(ui: &mut Ui, app: &mut AppState) {
    let Some(branch) = app.branch() else {
        labeled(ui, "Google", |ui| {
            ui.card(&Default::default(), |ui| {
                ui.text("No taxpayer yet");
                ui.muted("Add a taxpayer before connecting a Gmail account.");
            });
        });
        return;
    };
    let tin = branch.tin.clone();
    let st = crate::store::inbox_state(Some(&tin));
    let st = serde_json::from_value::<crate::types::InboxState>(st).ok();
    let Some(st) = st else {
        labeled(ui, "Google", |ui| {
            ui.card(&Default::default(), |ui| {
                ui.text("Google account");
                ui.muted("This profile's Gmail state could not be read.");
            });
        });
        return;
    };
    let connected = st.oauth_connected;
    let who = if !st.oauth_inbox_email.as_deref().unwrap_or("").is_empty() {
        st.oauth_inbox_email.clone().unwrap_or_default()
    } else {
        st.inbox_email.clone()
    };
    let busy = GOOGLE_PENDING.with(|p| p.borrow().is_some());
    // A finished connect/reconnect: say so and drop the cached reads.
    let mut done = false;
    GOOGLE_PENDING.with(|p| {
        let mut b = p.borrow_mut();
        if let Some(rx) = b.as_mut()
            && let Ok(r) = rx.try_recv()
        {
            done = true;
            *b = None;
            match r {
                Ok(_) => app.toast("Google account connected.", Some("circle-check"), None),
                Err(e) => app.toast(
                    crate::lib::errors::error_text(&e),
                    Some("triangle-alert"),
                    None,
                ),
            };
        }
    });
    if done {
        crate::store::bump(Some("connect_inbox_oauth"));
    }
    labeled(ui, "Google", |ui| {
        ui.card(&Default::default(), |ui| {
            setting_row(
                ui,
                "Gmail",
                &if connected {
                    format!("Connected as {who}")
                } else {
                    format!(
                        "Not connected. Reads BIR confirmation mail for {}.",
                        if branch.name.is_empty() {
                            "this profile"
                        } else {
                            &branch.name
                        }
                    )
                },
                |ui| {
                    let label = if connected { "Reconnect" } else { "Connect" };
                    if ui.primary_button(label, !busy) {
                        let rx = crate::bir::call_async_spawn(
                            "connect_inbox_oauth",
                            vec![json!(tin.clone())],
                        );
                        GOOGLE_PENDING.with(|p| *p.borrow_mut() = Some(rx));
                    }
                },
            );
            crate::ui::kit::row_sep(ui);
            let mut tracking = st.tracking_enabled;
            if ui.switch("Receipt tracking", &mut tracking) && tracking != st.tracking_enabled {
                match crate::store::write(
                    "set_inbox_tracking",
                    vec![json!(tin.clone()), json!(tracking)],
                ) {
                    Ok(_) => app.toast(
                        if tracking {
                            "Receipt tracking is on."
                        } else {
                            "Receipt tracking is off."
                        },
                        None,
                        None,
                    ),
                    Err(e) => app.toast(
                        crate::lib::errors::error_text(&e),
                        Some("triangle-alert"),
                        None,
                    ),
                };
            }
            ui.muted("Watch this inbox for BIR confirmation mail");
        });
        if connected {
            ui.card(&Default::default(), |ui| {
                setting_row(
                    ui,
                    "Disconnect Google",
                    "Clears the grant for every profile that shares this inbox",
                    |ui| {
                        if ui.danger_button("Disconnect", true) {
                            match crate::store::write(
                                "disconnect_inbox_oauth",
                                vec![json!(tin.clone())],
                            ) {
                                Ok(_) => app.toast(
                                    "Google account disconnected for every profile sharing this inbox.",
                                    None,
                                    None,
                                ),
                                Err(e) => app.toast(
                                    crate::lib::errors::error_text(&e),
                                    Some("triangle-alert"),
                                    None,
                                ),
                            };
                        }
                    },
                );
            });
        }
    });
}

// ---- Security (Security.tsx + securityState.ts): PIN, authenticator, recovery key.

/// Whose codes the card edits (securityTarget).
enum SecTarget {
    Admin {
        has_pin: bool,
        has_totp: bool,
    },
    Taxpayer {
        root: String,
        has_pin: bool,
        has_totp: bool,
    },
}

fn security_target(app: &AppState) -> SecTarget {
    if app.is_admin() {
        let a = app.auth_state();
        SecTarget::Admin {
            has_pin: a.has_admin_pin,
            has_totp: a.has_admin_totp,
        }
    } else {
        let root = match &app.session {
            state::Session::Taxpayer { root } => root.clone(),
            _ => String::new(),
        };
        let c = state::taxpayer_creds(&root);
        SecTarget::Taxpayer {
            root,
            has_pin: c.has_pin,
            has_totp: c.has_totp,
        }
    }
}

/// The first admin code signs the person in and makes a recovery key when there is none
/// (afterAdminCode).
fn after_admin_code(app: &mut AppState) {
    if matches!(app.session, state::Session::Open) {
        app.start_session(state::Session::Admin);
        let mut params = crate::state::ViewParams::new();
        params.insert("section".to_owned(), json!("account"));
        app.go(ViewName::Settings, params);
    }
    if !app.auth_state().has_recovery_key {
        if let Ok(v) = crate::store::write("create_recovery_key", vec![]) {
            let key = v
                .get("recovery_key")
                .and_then(Value::as_str)
                .map(str::to_owned);
            SEC_RECOVERY.with(|r| *r.borrow_mut() = key);
        }
    }
}

fn make_recovery_key(app: &mut AppState) {
    match crate::store::write("create_recovery_key", vec![]) {
        Ok(v) => {
            let key = v
                .get("recovery_key")
                .and_then(Value::as_str)
                .map(str::to_owned);
            SEC_RECOVERY.with(|r| *r.borrow_mut() = key);
        }
        Err(e) => {
            app.toast(
                crate::lib::errors::error_text(&e),
                Some("triangle-alert"),
                None,
            );
        }
    }
}

/// Settings > Account > Security: My PIN, My authenticator and the admin's Recovery key.
fn draw_security(ui: &mut Ui, app: &mut AppState) {
    let t = security_target(app);
    let (admin, has_pin, has_totp) = match &t {
        SecTarget::Admin { has_pin, has_totp } => (true, *has_pin, *has_totp),
        SecTarget::Taxpayer {
            has_pin, has_totp, ..
        } => (false, *has_pin, *has_totp),
    };
    let helpdesk_admin = admin && app.is_helpdesk();
    let pin_label = if helpdesk_admin {
        "Admin PIN"
    } else {
        "My PIN"
    };
    let totp_label = if helpdesk_admin {
        "Admin authenticator"
    } else {
        "My authenticator"
    };
    let tp_name = app
        .taxpayer()
        .map(|t| t.tin_display.clone())
        .unwrap_or_default();
    let pin_hint = if !has_pin {
        "Not set. Anyone at this computer can open this account".to_owned()
    } else if helpdesk_admin {
        "Signs in TIN 000-000-000; opens locked-out taxpayers".to_owned()
    } else if admin {
        "Signs you in; opens locked-out taxpayers".to_owned()
    } else {
        "Asked when you sign in with your TIN".to_owned()
    };
    labeled(ui, "Security", |ui| {
        ui.card(&Default::default(), |ui| {
            // My PIN: four digits, Set/Change, Remove.
            stack_row(ui, pin_label, &pin_hint, |ui| {
                let mut pin = SEC_PIN.with(|p| p.borrow().clone());
                if ui.text_input_limited("settings-pin", &mut pin, pin_label, true, 4, true) {
                    SEC_PIN.with(|p| *p.borrow_mut() = pin);
                }
                let typed = SEC_PIN.with(|p| p.borrow().clone());
                if ui.secondary_button(if has_pin { "Change" } else { "Set PIN" }, true) {
                    if typed.len() != 4 {
                        app.toast(
                            "Type all 4 digits of the new PIN first.",
                            Some("lock"),
                            None,
                        );
                    } else {
                        let result = match &t {
                            SecTarget::Admin { .. } => {
                                crate::store::write("set_admin_pin", vec![json!(typed)])
                            }
                            SecTarget::Taxpayer { root, .. } => crate::store::write(
                                "set_taxpayer_pin",
                                vec![json!(root), json!(typed)],
                            ),
                        };
                        match result {
                            Ok(_) => {
                                SEC_PIN.with(|p| p.borrow_mut().clear());
                                if admin {
                                    after_admin_code(app);
                                }
                                app.toast(
                                    if has_pin {
                                        "PIN changed."
                                    } else {
                                        "PIN saved. Sign-in asks for it from now on."
                                    },
                                    Some("lock"),
                                    None,
                                );
                            }
                            Err(e) => {
                                app.toast(
                                    crate::lib::errors::error_text(&e),
                                    Some("triangle-alert"),
                                    None,
                                );
                            }
                        };
                    }
                }
                if has_pin && ui.ghost_button("Remove", true) {
                    let result = match &t {
                        SecTarget::Admin { .. } => {
                            crate::store::write("set_admin_pin", vec![Value::Null])
                        }
                        SecTarget::Taxpayer { root, .. } => {
                            crate::store::write("set_taxpayer_pin", vec![json!(root), Value::Null])
                        }
                    };
                    match result {
                        Ok(_) => app.toast("PIN removed.", Some("lock"), None),
                        Err(e) => app.toast(
                            crate::lib::errors::error_text(&e),
                            Some("triangle-alert"),
                            None,
                        ),
                    };
                }
            });
            crate::ui::kit::row_sep(ui);
            // My authenticator: Add, the secret + six cells with Confirm/Cancel, Remove.
            let secret = SEC_TOTP_SECRET.with(|s| s.borrow().clone());
            let totp_hint = if let Some(sec) = &secret {
                format!(
                    "Add this key to your authenticator app, then type the code it shows: {sec}"
                )
            } else if has_totp {
                "A code from your authenticator app also signs you in".to_owned()
            } else {
                "Use a 6-digit code from an authenticator app instead of the PIN".to_owned()
            };
            stack_row(ui, totp_label, &totp_hint, |ui| {
                if let Some(_sec) = &secret {
                    let mut code = SEC_TOTP_CODE.with(|c| c.borrow().clone());
                    if ui.text_input_limited(
                        "settings-totp-code",
                        &mut code,
                        totp_label,
                        false,
                        6,
                        true,
                    ) {
                        SEC_TOTP_CODE.with(|c| *c.borrow_mut() = code);
                    }
                    if ui.primary_button("Confirm", true) {
                        let s = SEC_TOTP_SECRET.with(|x| x.borrow().clone());
                        let code = SEC_TOTP_CODE.with(|c| c.borrow().clone());
                        if let Some(s) = s {
                            let result = match &t {
                                SecTarget::Admin { .. } => crate::store::write(
                                    "set_admin_totp",
                                    vec![json!(s), json!(code)],
                                ),
                                SecTarget::Taxpayer { root, .. } => crate::store::write(
                                    "set_taxpayer_totp",
                                    vec![json!(root), json!(s), json!(code)],
                                ),
                            };
                            match result {
                                Ok(_) => {
                                    SEC_TOTP_SECRET.with(|x| *x.borrow_mut() = None);
                                    SEC_TOTP_CODE.with(|c| c.borrow_mut().clear());
                                    if admin {
                                        after_admin_code(app);
                                    }
                                    app.toast("Authenticator added.", Some("lock"), None);
                                }
                                Err(e) => {
                                    app.toast(
                                        crate::lib::errors::error_text(&e),
                                        Some("triangle-alert"),
                                        None,
                                    );
                                }
                            };
                        }
                    }
                    if ui.ghost_button("Cancel", true) {
                        SEC_TOTP_SECRET.with(|x| *x.borrow_mut() = None);
                        SEC_TOTP_CODE.with(|c| c.borrow_mut().clear());
                    }
                } else if has_totp {
                    if ui.ghost_button("Remove", true) {
                        let result = match &t {
                            SecTarget::Admin { .. } => {
                                crate::store::write("set_admin_totp", vec![Value::Null])
                            }
                            SecTarget::Taxpayer { root, .. } => crate::store::write(
                                "set_taxpayer_totp",
                                vec![json!(root), Value::Null],
                            ),
                        };
                        match result {
                            Ok(_) => app.toast("Authenticator removed.", Some("lock"), None),
                            Err(e) => app.toast(
                                crate::lib::errors::error_text(&e),
                                Some("triangle-alert"),
                                None,
                            ),
                        };
                    }
                } else if ui.secondary_button("Add", true) {
                    let who = if admin {
                        if app.is_helpdesk() {
                            "admin".to_owned()
                        } else if tp_name.is_empty() {
                            "admin".to_owned()
                        } else {
                            tp_name.clone()
                        }
                    } else if tp_name.is_empty() {
                        match &t {
                            SecTarget::Taxpayer { root, .. } => root.clone(),
                            _ => String::new(),
                        }
                    } else {
                        tp_name.clone()
                    };
                    match crate::store::call(
                        "generate_totp_secret",
                        vec![json!(format!("eBIRForms {who}"))],
                    ) {
                        Ok(v) => {
                            let sec = v.get("secret").and_then(Value::as_str).map(str::to_owned);
                            SEC_TOTP_SECRET.with(|x| *x.borrow_mut() = sec);
                            SEC_TOTP_CODE.with(|c| c.borrow_mut().clear());
                        }
                        Err(e) => {
                            app.toast(
                                crate::lib::errors::error_text(&e),
                                Some("triangle-alert"),
                                None,
                            );
                        }
                    };
                }
            });
            // The admin's recovery key.
            let has_code = has_pin || has_totp;
            let recovery = SEC_RECOVERY.with(|r| r.borrow().clone());
            if admin && (has_code || recovery.is_some() || app.auth_state().has_recovery_key) {
                crate::ui::kit::row_sep(ui);
                let hint = if recovery.is_some() {
                    "Write it down now. It is shown once and works once.".to_owned()
                } else if !has_code {
                    "Made when you set a PIN or authenticator".to_owned()
                } else if app.auth_state().has_recovery_key {
                    "Signs you in if you forget the PIN. Making a new one cancels the old one"
                        .to_owned()
                } else {
                    "None yet. Without one, only your Mac login can reset a forgotten PIN"
                        .to_owned()
                };
                stack_row(ui, "Recovery key", &hint, |ui| {
                    if let Some(key) = &recovery {
                        ui.code(key);
                    } else {
                        let label = if app.auth_state().has_recovery_key {
                            "Make a new key"
                        } else {
                            "Make a key"
                        };
                        if ui.secondary_button(label, true) {
                            make_recovery_key(app);
                        }
                    }
                });
            }
        });
    });
}

fn draw_account(ui: &mut Ui, app: &mut AppState, sec: &SettingsViewSection) {
    if row_shown(sec, "buwiz") {
        draw_buwiz(ui, app);
    }
    if row_shown(sec, "identity") {
        draw_identity(ui, app);
    }
    if row_shown(sec, "google") {
        draw_google(ui, app);
    }
    if row_shown(sec, "security") {
        draw_security(ui, app);
    }
}

/// One import/export job in flight (importState.ts `busy`, actions.ts `exportBusy`).
enum TransferJob {
    Import,
    Export,
}

struct TransferPending {
    job: TransferJob,
    rx: crate::bir::AsyncReply,
}

thread_local! {
    static TRANSFER_PENDING: RefCell<Option<TransferPending>> = const { RefCell::new(None) };
    /// The export card's PIN cells; cleared after every attempt and never kept.
    static EXPORT_PIN: RefCell<String> = const { RefCell::new(String::new()) };
}

fn transfer_start(job: TransferJob, name: &str, args: Vec<Value>) {
    if TRANSFER_PENDING.with(|p| p.borrow().is_some()) {
        return;
    }
    let rx = crate::bir::call_async_spawn(name, args);
    TRANSFER_PENDING.with(|p| {
        *p.borrow_mut() = Some(TransferPending { job, rx });
    });
}

/// The finished import/export's toast (importState.ts `runImport`, actions.ts `exportProfile`).
fn transfer_drain(app: &mut AppState) {
    let mut done: Option<(TransferJob, Result<Value, String>)> = None;
    TRANSFER_PENDING.with(|p| {
        let mut b = p.borrow_mut();
        if let Some(pend) = b.as_mut()
            && let Ok(r) = pend.rx.try_recv()
        {
            let job = match pend.job {
                TransferJob::Import => TransferJob::Import,
                TransferJob::Export => TransferJob::Export,
            };
            done = Some((job, r));
            *b = None;
        }
    });
    let Some((job, result)) = done else {
        return;
    };
    match (job, result) {
        (TransferJob::Import, Ok(v)) => {
            let kind = v.get("kind").and_then(Value::as_str).unwrap_or("");
            match kind {
                "database" => {
                    app.toast("Database imported.", Some("upload"), None);
                }
                "profile" => {
                    app.toast("Taxpayer data restored.", Some("upload"), None);
                }
                "invalid" => {
                    app.toast(
                        "That zip has no taxpayer or database export in it.",
                        Some("triangle-alert"),
                        None,
                    );
                }
                _ => {}
            }
            // every cached read is dropped after a restore
            crate::store::bump(None);
        }
        (TransferJob::Import, Err(e)) => {
            app.toast(
                format!("Import failed: {}", crate::lib::errors::error_text(&e)),
                Some("triangle-alert"),
                None,
            );
        }
        (TransferJob::Export, Ok(v)) => {
            if v.get("kind").and_then(Value::as_str) == Some("exported") {
                app.toast("Profile exported.", Some("circle-check"), None);
            }
            // "cancelled" (the save dialog's Cancel) is quiet
        }
        (TransferJob::Export, Err(e)) => {
            app.toast(
                crate::lib::errors::error_text(&e),
                Some("triangle-alert"),
                None,
            );
        }
    }
}

fn draw_transfer(ui: &mut Ui, app: &mut AppState, sec: &SettingsViewSection) {
    transfer_drain(app);
    let has_taxpayer = app.taxpayer().is_some();
    let show_export = row_shown(sec, "export") && has_taxpayer;
    let show_import = row_shown(sec, "import");
    if !show_export && !show_import {
        ui.muted("Add a taxpayer before exporting a profile.");
        return;
    }
    let busy = TRANSFER_PENDING.with(|p| p.borrow().is_some());
    if show_import {
        ui.card(&Default::default(), |ui| {
            ui.section_title("Import");
            ui.row(&Default::default(), |ui| {
                ui.text("Import data");
                ui.muted("Restore a taxpayer or database export (.zip)");
                ui.spacer();
                let label = if busy { "Importing…" } else { "Import" };
                if crate::ui::kit::icon_ghost_button(ui, "upload", label, !busy)
                    && TRANSFER_PENDING.with(|p| p.borrow().is_none())
                {
                    transfer_start(TransferJob::Import, "import_archive", vec![]);
                }
            });
        });
    }
    if show_export {
        // ProfileExportCard: a credential-free archive through the host's save dialog; a
        // taxpayer with a PIN types it in the cells, which clear after every attempt.
        let tp = app.taxpayer();
        let has_pin = tp
            .as_ref()
            .map(|t| state::taxpayer_creds(&t.tin_root).has_pin)
            .unwrap_or(false);
        ui.card(&Default::default(), |ui| {
            ui.section_title("Export");
            ui.row(&Default::default(), |ui| {
                ui.text("Export this profile");
                ui.muted(if has_pin {
                    "The archive leaves out credentials; the profile PIN gates saving it"
                } else {
                    "Saves a credential-free archive of every branch and return"
                });
                ui.spacer();
                if has_pin {
                    let mut pin = EXPORT_PIN.with(|p| p.borrow().clone());
                    if ui.text_input("tp-export-pin", &mut pin, "Profile PIN", true) {
                        EXPORT_PIN.with(|p| *p.borrow_mut() = digits_only(&pin, 4));
                    }
                }
                if crate::ui::kit::icon_ghost_button(ui, "save", "Export", !busy) {
                    let pin = EXPORT_PIN.with(|p| {
                        let v = p.borrow().clone();
                        *p.borrow_mut() = String::new();
                        v
                    });
                    if has_pin && pin.len() != 4 {
                        app.toast(
                            "Type all 4 digits of the profile PIN first.",
                            Some("lock"),
                            None,
                        );
                    } else {
                        let tin = app
                            .branch()
                            .map(|b| b.tin.clone())
                            .or_else(|| tp.as_ref().map(|t| t.tin_root.clone()))
                            .unwrap_or_default();
                        transfer_start(
                            TransferJob::Export,
                            "export_profile",
                            vec![json!(tin), if has_pin { json!(pin) } else { Value::Null }],
                        );
                    }
                }
            });
        });
    }
}

fn digits_only(s: &str, n: usize) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).take(n).collect()
}

fn job_period(job: &BackgroundJob) -> Option<i64> {
    job.rest
        .get("quarter")
        .and_then(Value::as_i64)
        .or_else(|| job.rest.get("month").and_then(Value::as_i64))
        .filter(|v| *v != 0)
}

fn job_write(f: impl FnOnce() -> Result<Value, String>) {
    match f() {
        Ok(_) => JOBS_FAILURE.with(|f| *f.borrow_mut() = None),
        Err(e) => JOBS_FAILURE.with(|f| *f.borrow_mut() = Some(e)),
    }
}

fn read_log_tail() {
    LOG_TAIL.with(|t| *t.borrow_mut() = None);
    LOG_TAIL_ERR.with(|e| *e.borrow_mut() = None);
    match crate::bir::call0("job_log_tail") {
        Ok(v) => LOG_TAIL.with(|t| *t.borrow_mut() = Some(v)),
        Err(e) => LOG_TAIL_ERR.with(|err| *err.borrow_mut() = Some(e)),
    }
}

fn draw_jobs(ui: &mut Ui, app: &mut AppState, sec: &SettingsViewSection) {
    let tab = JOBS_TAB.with(|t| t.borrow().clone());
    // Jobs / Logs switch (Segmented)
    let tab_opts: [(&str, Option<&str>); 2] = [("Jobs", None), ("Logs", None)];
    let tab_cur = if tab == "logs" { 1 } else { 0 };
    if let Some(i) = crate::ui::kit::segmented(ui, "jobs-tabs", &tab_opts, tab_cur) {
        let next = if i == 1 { "logs" } else { "jobs" };
        if next != tab {
            JOBS_TAB.with(|t| *t.borrow_mut() = next.to_owned());
            if next == "logs" {
                // forgotten first: the tab reads the file as it opens, once
                read_log_tail();
            }
        }
    }
    if JOBS_TAB.with(|t| t.borrow().clone()) == "logs" {
        draw_logs(ui);
        return;
    }

    // status filter (Segmented) + purge
    let filter = JOBS_FILTER.with(|f| f.borrow().clone());
    let filter_opts: Vec<(&str, Option<&str>)> = settings_model::STATUS_FILTERS
        .iter()
        .map(|f| (*f, None))
        .collect();
    let filter_cur = settings_model::STATUS_FILTERS
        .iter()
        .position(|f| *f == filter)
        .unwrap_or(0);
    let mut purge = false;
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            gap: 8.0,
            wrap: true,
            align: cui::Align::Center,
            ..Default::default()
        },
        |ui| {
            if let Some(i) = crate::ui::kit::segmented(ui, "jobs-filter", &filter_opts, filter_cur)
                && let Some(f) = settings_model::STATUS_FILTERS.get(i)
            {
                JOBS_FILTER.with(|x| *x.borrow_mut() = (*f).to_owned());
            }
            ui.spacer();
            purge = ui.ghost_button("Purge archived", true);
        },
    );
    if purge {
        job_write(|| crate::store::write("purge_archived_jobs", vec![]));
        JOBS_FILTER.with(|f| *f.borrow_mut() = "All Jobs".to_owned());
    }
    if let Some(err) = JOBS_FAILURE.with(|f| f.borrow().clone()) {
        ui.danger_text(&err);
    }

    let jobs = crate::state::background_jobs();
    let filter_now = JOBS_FILTER.with(|f| f.borrow().clone());
    let visible = settings_model::filter_jobs(&jobs, &filter_now);
    if !row_shown(sec, "jobs") {
        return;
    }
    crate::ui::kit::list_group(ui, |ui| {
        if visible.is_empty() {
            ui.muted("No background jobs in this view.");
        }
        for (i, job) in visible.iter().enumerate() {
            if i > 0 {
                crate::ui::kit::row_sep(ui);
            }
            let key = settings_model::job_key(job);
            let _g = ui.push_id(format!("job-{key}"));
            ui.row(&Default::default(), |ui| {
                ui.column(&Default::default(), |ui| {
                    ui.text(&job.name);
                    ui.muted(&job_sub(job));
                });
                ui.spacer();
                ui.tone_badge(&job.status, settings_model::job_tone(&job.status));
            });
            let (cancel, archive, remove) = settings_model::job_actions(job);
            ui.row(&Default::default(), |ui| {
                ui.spacer();
                if cancel
                    && ui.ghost_button("Cancel queue", true)
                    && let (Some(tin), Some(code), Some(year)) =
                        (job.tin.clone(), job.form_code.clone(), job.taxable_year)
                {
                    let period = job_period(job);
                    let _ = state::cancel_queue(app, &tin, &code, year, period);
                }
                if archive
                    && let Some(id) = job.id
                    && ui.ghost_button("Archive", true)
                {
                    job_write(|| crate::store::write("archive_background_job", vec![json!(id)]));
                }
                if remove
                    && let Some(id) = job.id
                    && ui.ghost_button("Delete", true)
                {
                    job_write(|| crate::store::write("delete_background_job", vec![json!(id)]));
                }
                let _ = key;
            });
        }
    });
}

fn draw_logs(ui: &mut Ui) {
    let (file, lines, content, error) = match LOG_TAIL.with(|t| t.borrow().clone()) {
        Some(v) => (
            v.get("file_name")
                .and_then(Value::as_str)
                .map(str::to_owned),
            v.get("lines").and_then(Value::as_i64).unwrap_or(0),
            v.get("content").and_then(Value::as_str).map(str::to_owned),
            None,
        ),
        None => (None, 0, None, LOG_TAIL_ERR.with(|e| e.borrow().clone())),
    };
    let filter_now = LOG_FILTER.with(|f| f.borrow().clone());
    let view = settings_model::log_view(content.as_deref(), &filter_now);
    let headline = settings_model::log_headline(file.as_deref(), lines, view.matching);
    // Level filter (Segmented) + headline + Refresh on one row.
    let level_opts: Vec<(&str, Option<&str>)> = settings_model::LOG_FILTERS
        .iter()
        .map(|(_v, l)| (*l, None))
        .collect();
    let level_cur = settings_model::LOG_FILTERS
        .iter()
        .position(|(v, _)| *v == filter_now)
        .unwrap_or(0);
    let mut refresh = false;
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            gap: 12.0,
            wrap: true,
            align: cui::Align::Center,
            ..Default::default()
        },
        |ui| {
            if let Some(i) = crate::ui::kit::segmented(ui, "logs-filter", &level_opts, level_cur)
                && let Some((v, _)) = settings_model::LOG_FILTERS.get(i)
            {
                LOG_FILTER.with(|f| *f.borrow_mut() = (*v).to_owned());
            }
            ui.muted(&headline);
            ui.spacer();
            refresh = ui.secondary_button("Refresh", true);
        },
    );
    if refresh {
        read_log_tail();
    }
    if let Some(err) = error {
        ui.danger_text(&err);
    }
    // The log box: a bordered card-bg panel of cap mono lines.
    let p = crate::ui::kit::frame_palette();
    ui.box_(
        &cui::BoxOpts {
            width: cui::Size::FILL,
            pad: 12.0,
            gap: 2.0,
            bg: p.background,
            border: 1.0,
            border_color: p.border,
            radius: crate::theme::radius::CARD,
            ..Default::default()
        },
        |ui| {
            if view.shown.is_empty() {
                ui.muted(if filter_now == "All Logs" {
                    "No log entries yet today."
                } else {
                    "No lines at this level."
                });
            }
            for (_i, line, level) in &view.shown {
                let color = match settings_model::level_tone(*level) {
                    "danger" => p.danger,
                    "warning" => p.warning,
                    _ => p.muted_foreground,
                };
                ui.text_with(
                    if line.is_empty() {
                        " ".to_owned()
                    } else {
                        line.clone()
                    },
                    &cui::Text {
                        font: cui::font::MONO,
                        size: crate::theme::r#type::CAP.0,
                        color,
                        ..Default::default()
                    },
                );
            }
        },
    );
}

/// A row that tells something: a title and a sentence under it (NoteRow).
fn note_row(ui: &mut Ui, title: &str, note: &str) {
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            gap: 4.0,
            ..Default::default()
        },
        |ui| {
            ui.text(title);
            ui.muted(note);
        },
    );
}

/// A labelled select over `options` (value,label); writes the picked value through `on_pick`.
fn preset_select(
    ui: &mut Ui,
    id: &str,
    options: &[settings_model::SettingsPreset],
    current: &str,
) -> Option<String> {
    let labels: Vec<String> = options.iter().map(|o| o.label.clone()).collect();
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let mut index = options.iter().position(|o| o.value == current).unwrap_or(0) as i32;
    if crate::ui::kit::select(ui, id, &refs, &mut index) {
        return options.get(index as usize).map(|o| o.value.clone());
    }
    None
}

// ---- the global-hotkey keybinding editor (key caps + capture dialog)

/// The words a stored combo's modifiers may use, in the order `hotkey_shortcut` writes them.
const HOTKEY_MODS: &[(&str, &str)] = &[
    ("control", "Ctrl"),
    ("alt", "Alt"),
    ("shift", "Shift"),
    ("platform", "Super"),
];

/// `cui::Key` -> the stored word (`hotkey_combo::KEYS` spells them upper case).
const HOTKEY_KEYS: &[(cui::Key, &str)] = &[
    (cui::Key::A, "A"),
    (cui::Key::B, "B"),
    (cui::Key::C, "C"),
    (cui::Key::D, "D"),
    (cui::Key::E, "E"),
    (cui::Key::F, "F"),
    (cui::Key::G, "G"),
    (cui::Key::H, "H"),
    (cui::Key::I, "I"),
    (cui::Key::J, "J"),
    (cui::Key::K, "K"),
    (cui::Key::L, "L"),
    (cui::Key::M, "M"),
    (cui::Key::N, "N"),
    (cui::Key::O, "O"),
    (cui::Key::P, "P"),
    (cui::Key::Q, "Q"),
    (cui::Key::R, "R"),
    (cui::Key::S, "S"),
    (cui::Key::T, "T"),
    (cui::Key::U, "U"),
    (cui::Key::V, "V"),
    (cui::Key::W, "W"),
    (cui::Key::X, "X"),
    (cui::Key::Y, "Y"),
    (cui::Key::Z, "Z"),
    (cui::Key::Num0, "0"),
    (cui::Key::Num1, "1"),
    (cui::Key::Num2, "2"),
    (cui::Key::Num3, "3"),
    (cui::Key::Num4, "4"),
    (cui::Key::Num5, "5"),
    (cui::Key::Num6, "6"),
    (cui::Key::Num7, "7"),
    (cui::Key::Num8, "8"),
    (cui::Key::Num9, "9"),
    (cui::Key::F1, "F1"),
    (cui::Key::F2, "F2"),
    (cui::Key::F3, "F3"),
    (cui::Key::F4, "F4"),
    (cui::Key::F5, "F5"),
    (cui::Key::F6, "F6"),
    (cui::Key::F7, "F7"),
    (cui::Key::F8, "F8"),
    (cui::Key::F9, "F9"),
    (cui::Key::F10, "F10"),
    (cui::Key::F11, "F11"),
    (cui::Key::F12, "F12"),
];

/// The display label of one combo token ("control" -> "Ctrl", "E" -> "E").
fn hotkey_token_label(token: &str) -> String {
    let lower = token.to_ascii_lowercase();
    for (word, label) in HOTKEY_MODS {
        if lower == *word {
            // Command reads "Cmd" on macOS; everywhere else the Super/Windows key is "Super".
            if *word == "platform" && cfg!(target_os = "macos") {
                return "Cmd".to_owned();
            }
            return (*label).to_owned();
        }
    }
    token.to_ascii_uppercase()
}

/// The combo's tokens for display: modifiers in canonical order, the key last.
fn hotkey_tokens(combo: &str) -> Vec<String> {
    combo
        .split('+')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Draw a stored combo as key caps (⌃Ctrl ⌥Alt ⇧Shift ⌘Cmd E). Empty draws "Off".
fn key_caps(ui: &mut Ui, combo: &str) {
    let tokens = hotkey_tokens(combo);
    let p = crate::ui::kit::frame_palette();
    ui.row(
        &cui::Row {
            gap: 4.0,
            align: cui::Align::Center,
            ..Default::default()
        },
        |ui| {
            if tokens.is_empty() {
                ui.muted("Off");
                return;
            }
            for token in &tokens {
                let label = hotkey_token_label(token);
                ui.box_(
                    &cui::BoxOpts {
                        pad_x: 7.0,
                        pad_y: 3.0,
                        bg: p.hover,
                        border: 1.0,
                        border_color: p.border,
                        radius: crate::theme::radius::ROW,
                        ..Default::default()
                    },
                    |ui| {
                        ui.text_with(
                            &label,
                            &cui::Text {
                                size: crate::theme::r#type::CAP.0,
                                font: cui::font::MEDIUM,
                                color: p.foreground,
                                ..Default::default()
                            },
                        );
                    },
                );
            }
        },
    );
}

/// The modifier words held right now, in stored-combo spelling.
fn held_mod_words(ui: &mut Ui) -> Vec<&'static str> {
    let mods = ui.key_mods();
    let mut out = Vec::new();
    if mods.contains(cui::Mods::CTRL) {
        out.push("control");
    }
    if mods.contains(cui::Mods::ALT) {
        out.push("alt");
    }
    if mods.contains(cui::Mods::SHIFT) {
        out.push("shift");
    }
    if mods.contains(cui::Mods::SUPER) {
        out.push("platform");
    }
    out
}

/// The bindable key pressed this frame, in stored-combo spelling.
fn pressed_bind_key(ui: &mut Ui) -> Option<&'static str> {
    HOTKEY_KEYS
        .iter()
        .find(|(key, _)| ui.key_pressed(*key))
        .map(|(_, word)| *word)
}

/// The capture dialog: records the chord, offers Remove binding / Cancel / Save.
fn draw_hotkey_dialog(ui: &mut Ui, app: &mut AppState) {
    let mut open = true;
    /// What the footer asked for this frame: save a combo, remove the binding, or just close.
    enum HotkeyPick {
        Save(String),
        Remove,
        Cancel,
    }
    let mut pick: Option<HotkeyPick> = None;
    {
        // Record this frame's chord before drawing: a full modifier+key becomes the draft.
        HOTKEY_EDIT.with(|e| {
            let mut edit = e.borrow_mut();
            if let Some(word) = pressed_bind_key(ui) {
                let mods = held_mod_words(ui);
                if !mods.is_empty() {
                    let mut parts: Vec<&str> = mods;
                    parts.push(word);
                    edit.draft = parts.join("+");
                    edit.captured = true;
                }
            } else if ui.key_pressed(cui::Key::Backspace) || ui.key_pressed(cui::Key::Delete) {
                // Delete backs the recording out (an incorrect entry).
                edit.draft.clear();
                edit.captured = false;
            }
        });
    }
    let target = HOTKEY_EDIT.with(|e| e.borrow().target);
    let (title, description) = match target {
        HotkeyTarget::GlobalShow => (
            "Global show hotkey",
            "Press a new shortcut. It works from any app.",
        ),
        HotkeyTarget::Quit => (
            "Quit shortcut",
            "Press the keys that quit eBIRForms in this window.",
        ),
    };
    ui.dialog_with(
        title,
        &mut open,
        &cui::Dialog {
            width: 380.0,
            description: Some(description),
            ..Default::default()
        },
        |ui| {
            // The capture surface: the draft as caps, or the held modifiers while recording.
            let (draft, captured) = HOTKEY_EDIT.with(|e| {
                let e = e.borrow();
                (e.draft.clone(), e.captured)
            });
            ui.column(
                &cui::Column {
                    width: cui::Size::FILL,
                    gap: 10.0,
                    pad_y: 8.0,
                    ..Default::default()
                },
                |ui| {
                    ui.row(
                        &cui::Row {
                            width: cui::Size::FILL,
                            height: cui::Size::px(56.0),
                            align: cui::Align::Center,
                            justify: cui::Justify::Center,
                            bg: crate::ui::kit::frame_palette().hover,
                            border: 1.0,
                            border_color: crate::ui::kit::frame_palette().border,
                            radius: crate::theme::radius::CARD,
                            ..Default::default()
                        },
                        |ui| {
                            if captured {
                                key_caps(ui, &draft);
                            } else {
                                // Show the modifiers held right now as a live preview.
                                let mods = held_mod_words(ui);
                                if mods.is_empty() {
                                    ui.muted("Press a shortcut…");
                                } else {
                                    let preview = mods.join("+");
                                    key_caps(ui, &preview);
                                    ui.muted(" + a key");
                                }
                            }
                        },
                    );
                    ui.muted(if captured {
                        "Recorded. Press again to change it, or save."
                    } else {
                        "Hold Ctrl, Alt, Shift or Super with a letter, number or F-key."
                    });
                    ui.separator();
                    ui.row(
                        &cui::Row {
                            width: cui::Size::FILL,
                            gap: 8.0,
                            align: cui::Align::Center,
                            ..Default::default()
                        },
                        |ui| {
                            if crate::ui::kit::small_button(
                                ui,
                                "Remove binding",
                                cui::ButtonVariant::Ghost,
                                None,
                            ) {
                                pick = Some(HotkeyPick::Remove);
                            }
                            ui.spacer();
                            if crate::ui::kit::small_button(
                                ui,
                                "Cancel",
                                cui::ButtonVariant::Secondary,
                                None,
                            ) {
                                pick = Some(HotkeyPick::Cancel);
                            }
                            if crate::ui::kit::primary_button(ui, "Save", captured) && captured {
                                pick = Some(HotkeyPick::Save(draft.clone()));
                            }
                        },
                    );
                },
            );
        },
    );
    let closing = !open || matches!(pick, Some(HotkeyPick::Cancel));
    if closing {
        HOTKEY_EDIT.with(|e| {
            let mut e = e.borrow_mut();
            e.open = false;
            e.draft.clear();
            e.captured = false;
        });
    }
    let combo = match pick {
        Some(HotkeyPick::Save(combo)) => Some(combo),
        Some(HotkeyPick::Remove) => {
            HOTKEY_EDIT.with(|e| {
                let mut e = e.borrow_mut();
                e.open = false;
                e.draft.clear();
                e.captured = false;
            });
            Some(String::new())
        }
        _ => None,
    };
    if let Some(combo) = combo {
        let value = if combo == settings_model::OFF {
            String::new()
        } else {
            combo
        };
        // Quit has no dedicated host action: it is a plain app setting.
        let written = match target {
            HotkeyTarget::GlobalShow => {
                crate::store::write("set_global_hotkey", vec![json!(value)])
            }
            HotkeyTarget::Quit => {
                crate::store::write("set_setting", vec![json!("quit_shortcut"), json!(value)])
            }
        };
        match written {
            Ok(_) => crate::store::bump(Some("set_global_hotkey")),
            Err(e) => {
                app.toast(
                    crate::lib::errors::error_text(&e),
                    Some("triangle-alert"),
                    None,
                );
            }
        }
    }
}

// ---- Computer setup change (setup.ts + SetupCard.tsx)

fn setup_step() -> String {
    SETUP_STEP.with(|s| s.borrow().clone())
}

/// The current owner's name and TIN, or None on a help desk (currentOwner).
fn current_owner(app: &AppState) -> Option<(String, String, String)> {
    if app.is_helpdesk() {
        return None;
    }
    let root = app.auth_state().admin_root.unwrap_or_default();
    if let Some(t) = app.taxpayers().into_iter().find(|t| t.tin_root == root) {
        return Some((t.tin_root, t.name, t.tin_display));
    }
    Some((root.clone(), String::new(), root))
}

/// What "Set up for" says now (setupSummary).
fn setup_summary(app: &AppState) -> String {
    match current_owner(app) {
        None => "A help desk. The admin signs in with TIN 000-000-000".to_owned(),
        Some((_, name, tin_display)) => format!(
            "Just me or my business. Owner: {}TIN {}",
            if name.is_empty() {
                String::new()
            } else {
                format!("{name}, ")
            },
            tin_display
        ),
    }
}

/// Taxpayers that could become the owner, with whether they can sign in yet (candidates).
fn setup_candidates(app: &AppState) -> Vec<(crate::types::Taxpayer, bool)> {
    let current = if app.is_helpdesk() {
        None
    } else {
        app.auth_state().admin_root.clone()
    };
    app.taxpayers()
        .into_iter()
        .filter(|t| Some(&t.tin_root) != current.as_ref() && !t.archived)
        .map(|t| {
            let c = state::taxpayer_creds(&t.tin_root);
            (t, c.has_pin || c.has_totp)
        })
        .collect()
}

/// The confirm step's host write (confirmChange): the host re-checks the admin's PIN or code,
/// makes the change and answers a new recovery key shown once.
fn setup_confirm(app: &mut AppState) {
    let target = SETUP_TARGET.with(|t| t.borrow().clone());
    let Some(target) = target else {
        return;
    };
    let use_code = SETUP_USE_CODE.with(|u| *u.borrow());
    let confirm_pin = SETUP_CONFIRM_PIN.with(|p| p.borrow().clone());
    let confirm_code = SETUP_CONFIRM_CODE.with(|c| c.borrow().clone());
    let new_pin = SETUP_NEW_PIN.with(|p| p.borrow().clone());
    let (pin, code) = if use_code {
        (Value::Null, json!(confirm_code))
    } else {
        (json!(confirm_pin), Value::Null)
    };
    if !use_code && confirm_pin.len() != 4 {
        app.toast(
            "Type all 4 digits of your current admin PIN.",
            Some("lock"),
            None,
        );
        return;
    }
    if use_code && confirm_code.len() != 6 {
        app.toast("Type all 6 digits of the code.", Some("lock"), None);
        return;
    }
    let result = match target.get("kind").and_then(Value::as_str) {
        Some("owner") => crate::store::write(
            "change_owner",
            vec![
                json!(target.get("root").and_then(Value::as_str).unwrap_or("")),
                pin,
                code,
            ],
        ),
        _ => crate::store::write("change_to_helpdesk", vec![pin, code, json!(new_pin)]),
    };
    match result {
        Ok(v) => {
            let key = v
                .get("recovery_key")
                .and_then(Value::as_str)
                .map(str::to_owned);
            SETUP_RECOVERY.with(|r| *r.borrow_mut() = key);
            SETUP_STEP.with(|s| *s.borrow_mut() = "key".to_owned());
            SETUP_CONFIRM_PIN.with(|p| p.borrow_mut().clear());
            SETUP_CONFIRM_CODE.with(|c| c.borrow_mut().clear());
        }
        Err(e) => {
            SETUP_CONFIRM_PIN.with(|p| p.borrow_mut().clear());
            SETUP_CONFIRM_CODE.with(|c| c.borrow_mut().clear());
            app.toast(
                crate::lib::errors::error_text(&e),
                Some("triangle-alert"),
                None,
            );
        }
    }
}

/// The Computer setup card: the summary and Change, then whichever step the change is at.
fn draw_setup_card(ui: &mut Ui, app: &mut AppState) {
    ui.card(&Default::default(), |ui| {
        let step = setup_step();
        if step.is_empty() {
            setting_row(ui, "Set up for", &setup_summary(app), |ui| {
                if app.is_admin() && ui.secondary_button("Change", true) {
                    SETUP_STEP.with(|s| *s.borrow_mut() = "choose".to_owned());
                }
            });
            return;
        }
        let cancel = |ui: &mut Ui| {
            if ui.ghost_button("Cancel", true) {
                reset_setup_change();
            }
        };
        match step.as_str() {
            "choose" => {
                let helpdesk = app.is_helpdesk();
                stack_row(
                    ui,
                    "Change to",
                    if helpdesk {
                        "Make one of the taxpayers the owner. They become the admin and sign in with their own PIN."
                    } else {
                        "Hand the computer to another taxpayer, or use it to help other taxpayers."
                    },
                    |ui| {
                        if ui.secondary_button(if helpdesk { "An owner" } else { "Another owner" }, true) {
                            SETUP_STEP.with(|s| *s.borrow_mut() = "owner".to_owned());
                        }
                        if !helpdesk && ui.secondary_button("A help desk", true) {
                            SETUP_STEP.with(|s| *s.borrow_mut() = "helpdesk".to_owned());
                        }
                        cancel(ui);
                    },
                );
            }
            "owner" => {
                setting_row(
                    ui,
                    "New owner",
                    "The owner is the admin of this computer. Only a taxpayer with a PIN or authenticator can be chosen.",
                    cancel,
                );
                let candidates = setup_candidates(app);
                if candidates.is_empty() {
                    setting_row(
                        ui,
                        "No other taxpayers",
                        "Add the taxpayer first, then come back here.",
                        |_| {},
                    );
                }
                for (tp, ready) in candidates {
                    let _g = ui.push_id(format!("setup-cand-{}", tp.tin_root));
                    setting_row(
                        ui,
                        &if tp.name.is_empty() { tp.tin_display.clone() } else { tp.name.clone() },
                        &if ready {
                            format!("TIN {}", tp.tin_display)
                        } else {
                            format!("TIN {}. No PIN yet: they set one in Settings after signing in", tp.tin_display)
                        },
                        |ui| {
                            if ready && ui.secondary_button("Choose", true) {
                                SETUP_TARGET.with(|t| {
                                    *t.borrow_mut() = Some(json!({
                                        "kind": "owner",
                                        "root": tp.tin_root,
                                        "name": tp.name,
                                        "tin_display": tp.tin_display,
                                    }));
                                });
                                SETUP_STEP.with(|s| *s.borrow_mut() = "confirm".to_owned());
                            }
                        },
                    );
                }
            }
            "helpdesk" => {
                setting_row(
                    ui,
                    "Switch to a help desk",
                    "The admin signs in with TIN 000-000-000. The current owner stays a taxpayer with their own PIN.",
                    cancel,
                );
                stack_row(ui, "New admin PIN", "4 digits, for TIN 000-000-000", |ui| {
                    let mut pin = SETUP_NEW_PIN.with(|p| p.borrow().clone());
                    if ui.text_input_limited(
                        "setup-new-pin",
                        &mut pin,
                        "New admin PIN",
                        true,
                        4,
                        true,
                    ) {
                        SETUP_NEW_PIN.with(|p| *p.borrow_mut() = pin);
                    }
                    if ui.primary_button("Continue", true) {
                        let pin = SETUP_NEW_PIN.with(|p| p.borrow().clone());
                        if pin.len() != 4 {
                            app.toast(
                                "Type all 4 digits of the new admin PIN.",
                                Some("lock"),
                                None,
                            );
                        } else {
                            SETUP_TARGET.with(|t| {
                                *t.borrow_mut() = Some(json!({ "kind": "helpdesk" }));
                            });
                            SETUP_STEP.with(|s| *s.borrow_mut() = "confirm".to_owned());
                        }
                    }
                });
            }
            "confirm" => {
                let what = match SETUP_TARGET.with(|t| t.borrow().clone()) {
                    Some(t) if t.get("kind").and_then(Value::as_str) == Some("owner") => {
                        let name = t.get("name").and_then(Value::as_str).unwrap_or("");
                        let tin = t.get("tin_display").and_then(Value::as_str).unwrap_or("");
                        format!(
                            "Make {} (TIN {tin}) the owner and admin",
                            if name.is_empty() { "this taxpayer" } else { name }
                        )
                    }
                    _ => "Switch to a help desk with the new admin PIN".to_owned(),
                };
                setting_row(ui, "Confirm the change", &what, cancel);
                let use_code = SETUP_USE_CODE.with(|u| *u.borrow());
                stack_row(
                    ui,
                    if use_code {
                        "Your admin authenticator code"
                    } else {
                        "Your current admin PIN"
                    },
                    "Then a new recovery key, and you are signed out",
                    |ui| {
                        if use_code {
                            let mut code = SETUP_CONFIRM_CODE.with(|c| c.borrow().clone());
                            if ui.text_input_limited(
                                "setup-confirm-code",
                                &mut code,
                                "Admin authenticator code",
                                false,
                                6,
                                true,
                            ) {
                                SETUP_CONFIRM_CODE.with(|c| *c.borrow_mut() = code);
                            }
                        } else {
                            let mut pin = SETUP_CONFIRM_PIN.with(|p| p.borrow().clone());
                            if ui.text_input_limited(
                                "setup-confirm-pin",
                                &mut pin,
                                "Current admin PIN",
                                true,
                                4,
                                true,
                            ) {
                                SETUP_CONFIRM_PIN.with(|p| *p.borrow_mut() = pin);
                            }
                        }
                        if ui.primary_button("Confirm change", true) {
                            setup_confirm(app);
                        }
                        if app.auth_state().has_admin_totp
                            && ui.ghost_button(
                                if use_code { "Use PIN" } else { "Use code" },
                                true,
                            )
                        {
                            SETUP_USE_CODE.with(|u| *u.borrow_mut() = !use_code);
                            SETUP_CONFIRM_PIN.with(|p| p.borrow_mut().clear());
                            SETUP_CONFIRM_CODE.with(|c| c.borrow_mut().clear());
                        }
                    },
                );
            }
            "key" => {
                stack_row(
                    ui,
                    "New recovery key",
                    "Write it down now. It is shown once and works once; the old key no longer works.",
                    |ui| {
                        if let Some(key) = SETUP_RECOVERY.with(|r| r.borrow().clone()) {
                            ui.code(&key);
                        }
                    },
                );
                setting_row(
                    ui,
                    "Setup changed",
                    "You are signed out next, so the admin signs in with their own PIN",
                    |ui| {
                        if ui.primary_button("I wrote it down", true) {
                            reset_setup_change();
                            app.sign_out_after_setup_change();
                        }
                    },
                );
            }
            _ => {}
        }
    });
}

/// Settings > Computer (ComputerSection): local files, shortcuts, window, setup, version.
fn draw_computer(ui: &mut Ui, app: &mut AppState, sec: &SettingsViewSection) {
    let info = crate::store::system_info();
    let files = row_shown(sec, "data") || row_shown(sec, "logs");
    if files {
        labeled(ui, "Local files", |ui| {
            ui.card(&Default::default(), |ui| {
                if row_shown(sec, "data") {
                    note_row(ui, "Database", &info.database_path);
                    crate::ui::kit::row_sep(ui);
                    note_row(ui, "Data folder", &info.data_dir);
                }
                if row_shown(sec, "logs") {
                    if row_shown(sec, "data") {
                        crate::ui::kit::row_sep(ui);
                    }
                    note_row(ui, "Logs folder", &info.logs_dir);
                    crate::ui::kit::row_sep(ui);
                    let current = info.log_retention_days.to_string();
                    let options = settings_model::retention_choices(Some(&info));
                    setting_row(
                        ui,
                        "Keep logs for",
                        "Older daily log files are deleted",
                        |ui| {
                            if let Some(days) =
                                preset_select(ui, "settings-retention", &options, &current)
                                && let Ok(d) = days.parse::<i64>()
                            {
                                match crate::store::write("set_log_retention", vec![json!(d)]) {
                                    Ok(_) => crate::store::bump(Some("set_log_retention")),
                                    Err(e) => {
                                        app.toast(
                                            crate::lib::errors::error_text(&e),
                                            Some("triangle-alert"),
                                            None,
                                        );
                                    }
                                };
                            }
                        },
                    );
                }
            });
        });
    }
    if row_shown(sec, "hotkey") {
        labeled(ui, "Shortcuts", |ui| {
            ui.card(&Default::default(), |ui| {
                let current = info
                    .global_hotkey
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_default();
                setting_row(
                    ui,
                    "Global show hotkey",
                    "Shows or hides eBIRForms from any app",
                    |ui| {
                        // The binding as key caps, with a pencil that opens the capture dialog.
                        ui.row(
                            &cui::Row {
                                gap: 8.0,
                                align: cui::Align::Center,
                                ..Default::default()
                            },
                            |ui| {
                                key_caps(ui, &current);
                                if crate::ui::kit::icon_button(
                                    ui,
                                    "pencil",
                                    Some("Change shortcut"),
                                ) {
                                    HOTKEY_EDIT.with(|e| {
                                        let mut e = e.borrow_mut();
                                        e.open = true;
                                        e.draft.clear();
                                        e.captured = false;
                                        e.target = HotkeyTarget::GlobalShow;
                                    });
                                }
                            },
                        );
                    },
                );
                // The quit accelerator is a plain setting (menu_model's quit item reads it).
                let quit = crate::state::setting("quit_shortcut")
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "mod+q".to_owned());
                setting_row(
                    ui,
                    "Quit shortcut",
                    "Quits eBIRForms from this window",
                    |ui| {
                        ui.row(
                            &cui::Row {
                                gap: 8.0,
                                align: cui::Align::Center,
                                ..Default::default()
                            },
                            |ui| {
                                key_caps(ui, &quit);
                                if crate::ui::kit::icon_button(
                                    ui,
                                    "pencil",
                                    Some("Change shortcut"),
                                ) {
                                    HOTKEY_EDIT.with(|e| {
                                        let mut e = e.borrow_mut();
                                        e.open = true;
                                        e.draft.clear();
                                        e.captured = false;
                                        e.target = HotkeyTarget::Quit;
                                    });
                                }
                            },
                        );
                    },
                );
            });
        });
    }
    if row_shown(sec, "tray") {
        labeled(ui, "Window", |ui| {
            ui.card(&Default::default(), |ui| {
                note_row(
                    ui,
                    "Close window",
                    "Closing hides eBIRForms to the menu bar when a tray icon exists. With no tray, closing quits. Quit is always in the application menu.",
                );
            });
        });
    }
    if row_shown(sec, "setup") && app.is_admin() {
        labeled(ui, "Computer setup", |ui| draw_setup_card(ui, app));
    }
    if row_shown(sec, "version") {
        ui.muted(&format!("eBIRForms {}", info.app_version).trim().to_owned());
    }
}

fn draw_section(ui: &mut Ui, app: &mut AppState, sec: &SettingsViewSection) {
    match sec.id {
        "general" => draw_general(ui, app, sec),
        "account" => draw_account(ui, app, sec),
        "transfer" => draw_transfer(ui, app, sec),
        "jobs" => draw_jobs(ui, app, sec),
        "computer" => draw_computer(ui, app, sec),
        _ => {}
    }
}

// ---------------------------------------------------------------- chrome (Settings.tsx)

/// The section rail's width and the cards' column cap (RAIL = 220, COLUMN = 680).
const RAIL: f32 = 220.0;
const COLUMN: f32 = 680.0;

/// The search field both layouts share (TextField icon="search").
fn search_field(ui: &mut Ui) {
    let mut q = query();
    if q.capacity() < 256 {
        q.reserve(256);
    }
    if ui
        .input_with(
            "settings-search",
            &mut q,
            &cui::Input {
                placeholder: Some("Search settings"),
                icon: Some(crate::ui::icons::glyph("search")),
                ..Default::default()
            },
        )
        .changed()
    {
        QUERY.with(|c| *c.borrow_mut() = q);
    }
}

/// A group heading over the section rows (GroupLabel: 12/500 fg3, 16 above, 4 below, 8 in).
fn group_label(ui: &mut Ui, text: &str) {
    let p = crate::ui::kit::frame_palette();
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            pad_left: 8.0,
            pad_top: 16.0,
            pad_bottom: 4.0,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                text,
                &cui::Text {
                    size: crate::theme::r#type::META_STRONG.0,
                    color: p.subtle_foreground,
                    ..Default::default()
                },
            );
        },
    );
}

/// A section row (NavRow, default size): 32 tall, radius 8, px-2, gap-2.5, icon + 13/500 label,
/// the current one on the selected fill with fg ink; narrow adds a chevron-right trailing.
fn nav_row(ui: &mut Ui, id: &str, sec: &SettingsViewSection, current: bool, narrow: bool) -> bool {
    let p = crate::ui::kit::frame_palette();
    let ink = if current {
        p.foreground
    } else {
        p.muted_foreground
    };
    crate::ui::kit::press_cell(
        ui,
        id,
        crate::theme::size::CONTROL,
        current,
        crate::theme::radius::ROW,
        8.0,
        7.0,
        0.0,
        |ui| {
            ui.row(
                &cui::Row {
                    width: cui::Size::FILL,
                    align: cui::Align::Center,
                    gap: 10.0,
                    ..Default::default()
                },
                |ui| {
                    ui.icon(
                        crate::ui::icons::glyph(sec.icon),
                        crate::theme::size::ICON,
                        ink,
                    );
                    ui.text_with(
                        sec.label,
                        &cui::Text {
                            size: crate::theme::r#type::CONTROL.0,
                            color: ink,
                            ..Default::default()
                        },
                    );
                    ui.spacer();
                    if narrow {
                        ui.icon(
                            crate::ui::icons::glyph("chevron-right"),
                            crate::theme::size::ICON,
                            p.subtle_foreground,
                        );
                    }
                },
            );
        },
    )
}

/// The grouped section rows; returns the section picked this frame, if any.
fn section_list(
    ui: &mut Ui,
    view: &[SettingsViewSection],
    shown: Option<&str>,
    narrow: bool,
) -> Option<String> {
    let mut picked = None;
    let mut last_group = "";
    for sec in view {
        if sec.group != last_group {
            last_group = sec.group;
            group_label(ui, sec.group);
        }
        // Narrow rows are never "current" (NavRow current={!narrow && ...}).
        let current = !narrow && shown == Some(sec.id);
        if nav_row(ui, &format!("setnav-{}", sec.id), sec, current, narrow) {
            picked = Some(sec.id.to_owned());
        }
    }
    if view.is_empty() {
        ui.muted("No matching settings.");
    }
    picked
}

/// The cards' column of the open section (Pane): a scroll holding the Back row (narrow), the
/// PageTitle and the section's cards, centred at COLUMN on wide and full width on narrow.
fn draw_pane(
    ui: &mut Ui,
    app: &mut AppState,
    view: &[SettingsViewSection],
    shown: Option<&str>,
    narrow: bool,
) {
    let pane_w = if narrow {
        f32::INFINITY
    } else {
        // pane width = main area minus the rail and its divider, less the px-8 padding.
        (app.main_width() - RAIL - 1.0 - 64.0)
            .min(COLUMN)
            .max(200.0)
    };
    ui.scroll(
        &cui::Scroll {
            id: Some("settings-scroll"),
            ..Default::default()
        },
        |ui| {
            ui.column(
                &cui::Column {
                    width: cui::Size::FILL,
                    align: cui::Align::Center,
                    pad_x: if narrow { 16.0 } else { 32.0 },
                    pad_top: if narrow { 12.0 } else { 28.0 },
                    pad_bottom: 56.0,
                    gap: crate::theme::space(5),
                    ..Default::default()
                },
                |ui| {
                    ui.column(
                        &cui::Column {
                            width: if narrow {
                                cui::Size::FILL
                            } else {
                                cui::Size::px(pane_w)
                            },
                            gap: crate::theme::space(5),
                            ..Default::default()
                        },
                        |ui| {
                            if narrow
                                && crate::ui::kit::icon_ghost_button(
                                    ui,
                                    "chevron-left",
                                    "Settings",
                                    true,
                                )
                            {
                                NARROW_OPEN.with(|o| *o.borrow_mut() = false);
                            }
                            let sec = shown.and_then(|id| view.iter().find(|s| s.id == id));
                            ui.heading(sec.map_or("Settings", |s| s.label), 1);
                            match sec {
                                Some(sec) => draw_section(ui, app, sec),
                                None => ui.muted("No settings match that search."),
                            }
                        },
                    );
                },
            );
        },
    );
}

/// From 600 points: the rail (title, search, grouped sections) beside the cards' pane.
fn draw_wide(ui: &mut Ui, app: &mut AppState, view: &[SettingsViewSection], shown: Option<&str>) {
    let p = crate::ui::kit::frame_palette();
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            height: cui::Size::FILL,
            align: cui::Align::Stretch,
            ..Default::default()
        },
        |ui| {
            ui.column(
                &cui::Column {
                    width: cui::Size::px(RAIL),
                    height: cui::Size::FILL,
                    pad_top: 16.0,
                    pad_right: 12.0,
                    pad_bottom: 8.0,
                    pad_left: 12.0,
                    ..Default::default()
                },
                |ui| {
                    ui.row(
                        &cui::Row {
                            width: cui::Size::FILL,
                            pad_x: 8.0,
                            pad_bottom: 12.0,
                            ..Default::default()
                        },
                        |ui| {
                            ui.text_with(
                                "Settings",
                                &cui::Text {
                                    size: crate::theme::r#type::EMPH.0,
                                    color: p.foreground,
                                    ..Default::default()
                                },
                            );
                        },
                    );
                    search_field(ui);
                    ui.scroll(
                        &cui::Scroll {
                            id: Some("settings-rail-scroll"),
                            ..Default::default()
                        },
                        |ui| {
                            if let Some(id) = section_list(ui, view, shown, false) {
                                set_section(&id);
                            }
                        },
                    );
                },
            );
            // border-r
            ui.box_(
                &cui::BoxOpts {
                    width: cui::Size::px(1.0),
                    height: cui::Size::FILL,
                    bg: p.border,
                    ..Default::default()
                },
                |_| {},
            );
            ui.column(
                &cui::Column {
                    width: cui::Size::FILL,
                    height: cui::Size::FILL,
                    ..Default::default()
                },
                |ui| draw_pane(ui, app, view, shown, false),
            );
        },
    );
}

/// Under 600 points: the section list first, then the section as its own page with Back.
fn draw_narrow(ui: &mut Ui, app: &mut AppState, view: &[SettingsViewSection], shown: Option<&str>) {
    if NARROW_OPEN.with(|o| *o.borrow()) {
        draw_pane(ui, app, view, shown, true);
        return;
    }
    ui.scroll(
        &cui::Scroll {
            id: Some("settings-list-scroll"),
            ..Default::default()
        },
        |ui| {
            ui.column(
                &cui::Column {
                    width: cui::Size::FILL,
                    pad_x: 16.0,
                    pad_top: 16.0,
                    pad_bottom: 56.0,
                    gap: 8.0,
                    ..Default::default()
                },
                |ui| {
                    search_field(ui);
                    if let Some(id) = section_list(ui, view, shown, true) {
                        set_section(&id);
                        NARROW_OPEN.with(|o| *o.borrow_mut() = true);
                    }
                },
            );
        },
    );
}

pub fn draw(ui: &mut Ui, app: &mut AppState) {
    // route param `section` opens a section directly; applied once per route like the TSX mount.
    let param = app
        .params()
        .get("section")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    if param != LAST_PARAM.with(|p| p.borrow().clone()) {
        LAST_PARAM.with(|p| *p.borrow_mut() = param.clone());
        if let Some(s) = &param {
            set_section(s);
            NARROW_OPEN.with(|o| *o.borrow_mut() = true);
        }
    }
    let view = view(app);
    let shown = settings_model::shown_section(Some(&section()), &view).map(str::to_owned);
    if app.narrow() {
        draw_narrow(ui, app, &view, shown.as_deref());
    } else {
        draw_wide(ui, app, &view, shown.as_deref());
    }
    // The hotkey capture dialog stands over whichever layout drew the row.
    if hotkey_editing() {
        draw_hotkey_dialog(ui, app);
    }
}
