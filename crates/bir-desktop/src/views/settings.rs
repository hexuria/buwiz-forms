use crate::components::combobox::{Combobox, ComboboxState};
use crate::components::hotkey_recorder::{HotkeyRecorder, HotkeyRecorderEvent};
use crate::components::otp_paste::paste_otp_value;
use crate::components::setting_row::SettingRow;
use crate::views::form_controls::{new_select_state, on_select, sync_select};
use bir_core::db::Database;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{OtpEvent, OtpInput, OtpState};
use gpui_component::switch::Switch;
use gpui_component::*;
use gpui_rsx::rsx;
use std::sync::{Arc, Mutex};

pub enum SettingsEvent {
    ReloadApp, // Fired when we replace the db file completely and need a cold reload
}

impl EventEmitter<SettingsEvent> for SettingsView {}

pub struct SettingsView {
    db: Arc<Mutex<Database>>,
    is_app_lock_enabled: bool,
    show_pin_setup: bool,
    setup_otp: Entity<OtpState>,
    is_app_totp_enabled: bool,
    show_totp_setup: bool,
    setup_totp_state: Entity<OtpState>,
    totp_secret_temp: Option<String>,
    totp_qr_path: Option<std::path::PathBuf>,
    show_totp_secret_text: bool,
    hide_tax_profiles: bool,
    enable_profile_pins: bool,
    /// D2A "Separate Validate button" (default On).
    separate_validate: bool,
    hotkey_recorder: Entity<HotkeyRecorder>,
    /// "30 days" etc.; the stored value is `log_retention_days`.
    log_retention: Entity<ComboboxState>,
    log_retention_days: u32,
    google_calendar_configured: bool,
    google_calendar_email: Option<String>,
    google_calendar_message: Option<(bool, String)>,
    show_google_calendar_setup_guide: bool,
}

impl SettingsView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let (
            is_app_lock_enabled,
            is_app_totp_enabled,
            hide_tax_profiles,
            enable_profile_pins,
            global_hotkey,
        ) = if let Ok(guard) = db.lock() {
            let lock = guard
                .get_setting("app_lock_enabled")
                .ok()
                .flatten()
                .as_deref()
                == Some("true");
            let totp = guard
                .get_setting("app_totp_secret")
                .ok()
                .flatten()
                .is_some();
            let hide = guard
                .get_setting("hide_tax_profiles")
                .ok()
                .flatten()
                .as_deref()
                == Some("true");
            let pins = guard
                .get_setting("enable_profile_pins")
                .ok()
                .flatten()
                .as_deref()
                == Some("true");
            let hotkey: Option<String> = guard
                .get_setting("global_hotkey_key")
                .ok()
                .flatten()
                .map(|k| k.to_uppercase())
                .filter(|k| !k.is_empty());
            (lock, totp, hide, pins, hotkey)
        } else {
            (false, false, false, false, None)
        };

        let setup_otp = cx.new(|cx| {
            let mut state = OtpState::new(4, window, cx);
            state = state.masked(true);
            state
        });

        let setup_totp_state = cx.new(|cx| {
            let mut state = OtpState::new(6, window, cx);
            state = state.masked(false);
            state
        });

        let separate_validate = crate::views::form_header::separate_validate_button(&db);
        let hotkey_recorder = cx.new(|cx| HotkeyRecorder::new(global_hotkey, window, cx));
        let retention_days = db
            .lock()
            .ok()
            .map(|guard| bir_core::log_files::retention_days(&guard))
            .unwrap_or(bir_core::log_files::DEFAULT_RETENTION_DAYS);
        let retention_choices = RetentionChoices::new(retention_days);
        let log_retention = new_select_state(&retention_choices.as_options(), window, cx);
        let calendar_connection = db
            .lock()
            .ok()
            .map(|db| bir_core::google_calendar::google_calendar_connection_from_db(&db))
            .unwrap_or_else(bir_core::google_calendar::google_calendar_configuration);

        let view = Self {
            db: db.clone(),
            is_app_lock_enabled,
            show_pin_setup: false,
            setup_otp: setup_otp.clone(),
            is_app_totp_enabled,
            show_totp_setup: false,
            setup_totp_state: setup_totp_state.clone(),
            totp_secret_temp: None,
            totp_qr_path: None,
            show_totp_secret_text: false,
            hide_tax_profiles,
            enable_profile_pins,
            separate_validate,
            hotkey_recorder: hotkey_recorder.clone(),
            log_retention: log_retention.clone(),
            log_retention_days: retention_days,
            google_calendar_configured: calendar_connection.configured,
            google_calendar_email: calendar_connection.connected_email,
            google_calendar_message: None,
            show_google_calendar_setup_guide: false,
        };

        on_select(
            &log_retention,
            &retention_choices.as_options(),
            window,
            cx,
            |this: &mut Self, value, _window, cx| {
                let Ok(days) = value.parse::<u32>() else {
                    return;
                };
                this.log_retention_days = days;
                if let Ok(db) = this.db.lock() {
                    let _ =
                        db.set_setting(bir_core::log_files::RETENTION_SETTING, &days.to_string());
                    // Apply now rather than at the next six-hour tick.
                    let _ = bir_core::log_files::maintain(&bir_core::log_files::log_dir(), &db);
                }
                cx.notify();
            },
        )
        .detach();

        cx.subscribe_in(
            &setup_otp,
            window,
            |this: &mut Self, _entity, event: &OtpEvent, _window, cx| {
                if let OtpEvent::Change = event {
                    let pin = this.setup_otp.read(cx).value().to_string();
                    if pin.len() == 4 {
                        let hashed = bir_core::crypto::hash_pin(&pin);

                        if let Ok(db_guard) = this.db.lock() {
                            let _ = db_guard.set_setting("app_lock_pin_hash", &hashed);
                            let _ = db_guard.set_setting("app_lock_enabled", "true");
                        }
                        this.is_app_lock_enabled = true;
                        this.show_pin_setup = false;
                        cx.notify();
                    }
                }
            },
        )
        .detach();

        cx.subscribe_in(
            &setup_totp_state,
            window,
            |this: &mut Self, _entity, event: &OtpEvent, window, cx| {
                if let OtpEvent::Change = event {
                    let token = this.setup_totp_state.read(cx).value().to_string();
                    if token.len() == 6
                        && let Some(ref secret) = this.totp_secret_temp
                    {
                        if bir_core::crypto::validate_totp(secret, &token) {
                            if let Ok(db_guard) = this.db.lock() {
                                let _ = db_guard.set_setting("app_totp_secret", secret);
                            }
                            this.is_app_totp_enabled = true;
                            this.show_totp_setup = false;
                            this.show_totp_secret_text = false;
                            this.totp_secret_temp = None;
                            this.totp_qr_path = None;
                            cx.emit(SettingsEvent::ReloadApp);
                            cx.notify();
                        } else {
                            this.setup_totp_state.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                        }
                    }
                }
            },
        )
        .detach();

        cx.subscribe_in(
            &hotkey_recorder,
            window,
            |this: &mut Self, _entity, event: &HotkeyRecorderEvent, _window, cx| {
                match event {
                    HotkeyRecorderEvent::Changed(key) => {
                        if let Ok(db_guard) = this.db.lock() {
                            let _ = db_guard.set_setting("global_hotkey_key", key);
                        }
                    }
                    HotkeyRecorderEvent::Cleared => {
                        if let Ok(db_guard) = this.db.lock() {
                            let _ = db_guard.set_setting("global_hotkey_key", "");
                        }
                    }
                }
                cx.emit(SettingsEvent::ReloadApp);
                cx.notify();
            },
        )
        .detach();

        view
    }

    /// Turn Profile PINs on or off. Turning them off also un-hides profiles,
    /// since hiding needs PINs.
    fn set_profile_pins(&mut self, on: bool, cx: &mut Context<'_, Self>) {
        self.enable_profile_pins = on;
        if let Ok(db) = self.db.lock() {
            let _ = db.set_setting("enable_profile_pins", if on { "true" } else { "false" });
            if !on && self.hide_tax_profiles {
                self.hide_tax_profiles = false;
                let _ = db.set_setting("hide_tax_profiles", "false");
            }
        }
        cx.emit(SettingsEvent::ReloadApp);
        cx.notify();
    }

    fn render_google_calendar_settings(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        let connected = self.google_calendar_email.is_some();
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_xl()
            .p_6()
            .gap_4()
            .child(
                SettingRow::new(
                    "Google Calendar",
                    "Connect one Google account, then create a separate filing calendar for each tax profile.",
                )
                // Bounded so a long address wraps inside the card.
                .control(rsx! {
                    <div max_w={px(280.)} text_sm text_color={if connected { cx.theme().success } else { cx.theme().muted_foreground }}>
                        {self.google_calendar_email.clone().unwrap_or_else(|| "Not connected".to_string())}
                    </div>
                }),
            )
            .child(rsx! {
                <div text_sm text_color={cx.theme().muted_foreground}>
                    {"eBIRForms requests access only to calendars it creates. Deadline events use email reminders 7 days and 1 day before filing."}
                </div>
            })
            .when(!self.google_calendar_configured, |this| {
                this.child(rsx! {
                    <div text_sm text_color={cx.theme().danger}>
                        {"Google OAuth is not configured. Set GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET (e.g. copy .env.example to .env and fill it in), then restart the app."}
                    </div>
                })
            })
            .when_some(self.google_calendar_message.clone(), |this, message| {
                this.child(rsx! {
                    <div text_sm text_color={if message.0 { cx.theme().success } else { cx.theme().danger }}>
                        {message.1}
                    </div>
                })
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        gpui_component::button::Button::new("connect_google_calendar")
                            .label(if connected {
                                "Reconnect Google"
                            } else {
                                "Connect Google"
                            })
                            .disabled(!self.google_calendar_configured)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.google_calendar_message =
                                    Some((true, "Waiting for Google authorization...".into()));
                                cx.notify();
                                let db = this.db.clone();
                                cx.spawn(async move |this, cx| {
                                    let (tx, rx) = tokio::sync::oneshot::channel();
                                    std::thread::spawn(move || {
                                        let result =
                                            bir_core::google_calendar::connect_google_calendar_account(db);
                                        let _ = tx.send(result);
                                    });
                                    let result = rx.await.unwrap_or_else(|_| {
                                        Err(anyhow::anyhow!("Google OAuth worker stopped"))
                                    });
                                    let _ = this.update(cx, |this, cx| {
                                        match result {
                                            Ok(email) => {
                                                this.google_calendar_email = Some(email.clone());
                                                this.google_calendar_message = Some((
                                                    true,
                                                    format!("Connected Google Calendar as {email}."),
                                                ));
                                            }
                                            Err(error) => {
                                                this.google_calendar_message = Some((
                                                    false,
                                                    format!("Connection failed: {error}"),
                                                ));
                                            }
                                        }
                                        cx.notify();
                                    });
                                })
                                .detach();
                            })),
                    )
                    .when(connected, |this| {
                        this.child(
                            gpui_component::button::Button::new("disconnect_google_calendar")
                                .label("Disconnect")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let db = this.db.clone();
                                    cx.spawn(async move |this, cx| {
                                        let result = std::thread::spawn(move || {
                                            bir_core::google_calendar::disconnect_google_calendar_account(db)
                                        })
                                        .join()
                                        .unwrap_or_else(|_| {
                                            Err(anyhow::anyhow!("Disconnect worker stopped"))
                                        });
                                        let _ = this.update(cx, |this, cx| {
                                            match result {
                                                Ok(()) => {
                                                    this.google_calendar_email = None;
                                                    this.google_calendar_message = Some((
                                                        true,
                                                        "Disconnected. Existing Google calendars were preserved."
                                                            .into(),
                                                    ));
                                                }
                                                Err(error) => {
                                                    this.google_calendar_message = Some((
                                                        false,
                                                        format!("Disconnect failed: {error}"),
                                                    ));
                                                }
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .detach();
                                })),
                        )
                    })
                    .child(
                        gpui_component::button::Button::new("google_calendar_setup_docs")
                            .label(if self.show_google_calendar_setup_guide {
                                "Hide Setup Guide"
                            } else {
                                "Setup Guide"
                            })
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_google_calendar_setup_guide =
                                    !this.show_google_calendar_setup_guide;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.show_google_calendar_setup_guide, |this| {
                this.child(rsx! {
                    <div flex flex_col gap_2 p_4 rounded_lg bg={cx.theme().secondary} border_1 border_color={cx.theme().border}>
                        <div font_weight={FontWeight::SEMIBOLD}>{"Google Calendar setup"}</div>
                        <div text_sm text_color={cx.theme().foreground}>
                            {"1. The app build must include a Google Desktop OAuth client and have the Google Calendar API enabled."}
                        </div>
                        <div text_sm text_color={cx.theme().foreground}>
                            {"2. Connect the Calendar account here in Settings. Email receipt tracking uses a separate Google connection."}
                        </div>
                        <div text_sm text_color={cx.theme().foreground}>
                            {"3. After connection, edit a saved tax profile. Its Calendar tab will appear so you can create a filing calendar."}
                        </div>
                        <div text_sm text_color={cx.theme().muted_foreground}>
                            {"Set GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET in a .env file (see .env.example) and restart, or bake them in at build time for release builds. Tokens are stored in the operating-system credential store, not in the project environment."}
                        </div>
                        {gpui_component::button::Button::new(
                            "google_calendar_official_oauth_docs",
                        )
                        .label("Open official Google OAuth guide")
                        .ghost()
                        .on_click(|_, _, _| {
                            let _ = open::that_detached(
                                "https://developers.google.com/identity/protocols/oauth2/native-app",
                            );
                        })}
                    </div>
                })
            })
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let retention_choices = RetentionChoices::new(self.log_retention_days);
        let retention_value = self.log_retention_days.to_string();
        sync_select(
            &self.log_retention,
            &retention_choices.as_options(),
            Some(&retention_value),
            window,
            cx,
        );
        let bg = cx.theme().background;
        let border = cx.theme().border;

        div()
            .size_full()
            .relative()
            // Intercept Cmd+V / Ctrl+V for OTP paste support
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let mods = &event.keystroke.modifiers;
                let is_paste = event.keystroke.key.as_str() == "v"
                    && (mods.platform || mods.control);
                if !is_paste { return; }
                // Paste into whichever OTP input is currently in a setup flow
                if this.show_totp_setup {
                    paste_otp_value(&this.setup_totp_state, 6, window, cx);
                } else if this.show_pin_setup {
                    paste_otp_value(&this.setup_otp, 4, window, cx);
                }
            }))
            .child(
                div()
                    .id("settings_scroll")
                    .size_full()
                    .flex()
                    .flex_col()
                    .p_8()
                    .gap_6()
                    .overflow_y_scroll()
                    .child(rsx! {
                        <div flex flex_col gap_1>
                            <div text_2xl font_weight={FontWeight::BOLD}>{"Settings"}</div>
                            <div text_color={cx.theme().muted_foreground}>
                                {"Configure security, privacy, and global application preferences."}
                            </div>
                        </div>
                    })
                    .child(self.render_google_calendar_settings(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_xl()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .p_6()
                            .gap_4()
                            .border_b_1()
                            .border_color(border)
                            .child(
                                SettingRow::new(
                                    "Master PIN",
                                    "Require a 4-digit PIN to unlock the app. If forgotten, your OS login password will be required.",
                                )
                                .note(
                                    self.is_app_totp_enabled
                                        .then_some("Disable Authenticator App first to use 4-digit PIN."),
                                )
                                .control(
                                    Switch::new("app_lock_switch")
                                        .checked(self.is_app_lock_enabled)
                                        .disabled(self.is_app_totp_enabled)
                                        .on_click(cx.listener(|this, checked, _window, cx| {
                                            if this.is_app_totp_enabled { return; }
                                            if !checked {
                                                if let Ok(db) = this.db.lock() {
                                                    let _ = db.set_setting("app_lock_enabled", "false");
                                                }
                                                this.is_app_lock_enabled = false;
                                                this.show_pin_setup = false;
                                            } else {
                                                this.show_pin_setup = true;
                                            }
                                            cx.notify();
                                        })),
                                ),
                            )
                            .when(self.show_pin_setup, |this| {
                                this.child(rsx! {
                                    <div flex flex_col gap_4 mt_4>
                                        <div flex flex_col gap_1>
                                            <div text_sm font_weight={FontWeight::MEDIUM}>{"Set your 4-digit PIN"}</div>
                                            {OtpInput::new(&self.setup_otp).groups(1)}
                                        </div>
                                        {gpui_component::button::Button::new("cancel_pin_btn")
                                            .label("Cancel")
                                            .small()
                                            .on_click(cx.listener(|this, _ev, window, cx| {
                                                this.show_pin_setup = false;
                                                this.setup_otp.update(cx, |s, cx| s.set_value("", window, cx));
                                                this.is_app_lock_enabled = false;
                                                cx.notify();
                                            }))}
                                    </div>
                                })
                            })
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .p_6()
                            .gap_4()
                            .border_b_1()
                            .border_color(border)
                            .child(
                                SettingRow::new(
                                    "Master Authenticator App",
                                    "Use a 6-digit TOTP code from an authenticator app (e.g. Google Authenticator) for enhanced security.",
                                )
                                .note(
                                    self.is_app_lock_enabled
                                        .then_some("Disable 4-digit PIN first to use Authenticator App."),
                                )
                                .control(
                                    Switch::new("app_totp_switch")
                                        .checked(self.is_app_totp_enabled)
                                        .disabled(self.is_app_lock_enabled)
                                        .on_click(cx.listener(|this, checked, window, cx| {
                                            if this.is_app_lock_enabled { return; }
                                            if !checked {
                                                if let Ok(db) = this.db.lock() {
                                                    let _ = db.delete_setting("app_totp_secret");
                                                }
                                                this.is_app_totp_enabled = false;
                                                this.show_totp_setup = false;
                                                this.show_totp_secret_text = false;
                                                this.totp_secret_temp = None;
                                                this.totp_qr_path = None;
                                                cx.emit(SettingsEvent::ReloadApp);
                                            } else {
                                                let (secret, qr_path) = bir_core::crypto::generate_totp_secret("e-BIRForms (Master)");
                                                this.totp_secret_temp = Some(secret);
                                                this.totp_qr_path = Some(qr_path);
                                                this.show_totp_setup = true;
                                                this.setup_totp_state.update(cx, |s, cx| s.focus(window, cx));
                                            }
                                            cx.notify();
                                        })),
                                ),
                            )

                    )

                    .child(section(border).child(
                        SettingRow::new(
                            "Keep application logs for",
                            "One log file per day under Background Tasks → Logs. Older days are deleted automatically.",
                        )
                        .control(div().w(px(180.)).child(Combobox::new(&self.log_retention))),
                    ))
                    .child(section(border).child(
                        SettingRow::new(
                            "Enable Profile PINs",
                            "Allow securing individual tax profiles with a separate 4-digit PIN.",
                        )
                        .control(
                            Switch::new("enable_profile_pins_switch")
                                .checked(self.enable_profile_pins)
                                .on_click(cx.listener(|this, checked, _window, cx| {
                                    this.set_profile_pins(*checked, cx);
                                })),
                        ),
                    ))
                    .child(section(border).child({
                        let hide_control = div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .when(!self.enable_profile_pins, |row| {
                                row.child(
                                    gpui_component::button::Button::new("turn_on_profile_pins_btn")
                                        .label("Turn on Profile PINs")
                                        .small()
                                        .on_click(cx.listener(|this, _ev, _window, cx| {
                                            this.set_profile_pins(true, cx);
                                        })),
                                )
                            })
                            .child(
                                Switch::new("hide_tax_profiles_switch")
                                    .checked(self.hide_tax_profiles)
                                    .disabled(!self.enable_profile_pins)
                                    .on_click(cx.listener(|this, checked, _window, cx| {
                                        let Some(hide) = hide_profiles_after_toggle(this.enable_profile_pins, *checked) else {
                                            return;
                                        };
                                        this.hide_tax_profiles = hide;
                                        if let Ok(db) = this.db.lock() {
                                            let _ = db.set_setting("hide_tax_profiles", if hide { "true" } else { "false" });
                                        }
                                        cx.emit(SettingsEvent::ReloadApp);
                                        cx.notify();
                                    })),
                            );
                        SettingRow::new(
                            "Hide Tax Profiles from Sidebar",
                            "Prevent profiles from being listed in the sidebar. Useful in public spaces. Requires Profile PINs to be enabled.",
                        )
                        .note(hide_profiles_note(self.enable_profile_pins))
                        .control(hide_control)
                    }))
                    .child(section(border).child(
                        SettingRow::new(
                            "Separate Validate button",
                            "On: return pages show Validate next to Submit, and Submit waits for a passing Validate. Off: Submit validates first and only queues a return with no errors.",
                        )
                        .control(
                            Switch::new("separate_validate_switch")
                                .checked(self.separate_validate)
                                .on_click(cx.listener(|this, checked, _window, cx| {
                                    this.separate_validate = *checked;
                                    crate::views::form_header::set_separate_validate_button(&this.db, *checked);
                                    cx.notify();
                                })),
                        ),
                    ))
                    // Global Toggle Hotkey
                    .child(section(border).child(
                        SettingRow::new(
                            "Global Toggle Hotkey",
                            "Toggle eBIRForms from anywhere. Click the shortcut, then press your desired modifiers (Ctrl / Option / Shift / Cmd) together with a letter, number, or F-key.",
                        )
                        .control(self.hotkey_recorder.clone()),
                    ))
                    .child(rsx! {
                        <div flex flex_col items_start p_6 gap_4 border_b_1 border_color={border}>
                            {SettingRow::new(
                                "Export Data",
                                "Create a complete backup of your entire application database or all profiles.",
                            )}
                            <div flex flex_wrap gap_4>
                                    {
                                        gpui_component::button::Button::new("export_database_btn")
                                            .label("Export Full Database")
                                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                                cx.spawn(async move |this, cx| {
                                                    let result = crate::export_target::folder_with_fallback(
                                                        "Select Folder to Save Database Backup",
                                                        |target_dir| {
                                                            let db_path = bir_core::db::default_database_path();
                                                            if !db_path.exists() { return Err("Database file not found".to_string()); }
                                                            let backup_path = target_dir.join(format!("BIR_Database_Backup_{}.db.zip", unix_timestamp()));
                                                            this.update(cx, |this, _cx| {
                                                                let db = this.db.lock().map_err(|_| "Failed to acquire database lock".to_string())?;
                                                                let _ = db.checkpoint();
                                                                bir_core::export::export_database_zip(&db, &backup_path).map_err(|e| e.to_string())?;
                                                                Ok(backup_path)
                                                            })
                                                            .map_err(|e| e.to_string())?
                                                        },
                                                    )
                                                    .await;
                                                    report_export("Database Exported", result).await;
                                                }).detach();
                                            }))
                                    }
                                    {
                                        gpui_component::button::Button::new("export_all_profiles_btn")
                                            .label("Export All Profiles (JSON)")
                                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                                cx.spawn(async move |this, cx| {
                                                    let result = crate::export_target::folder_with_fallback(
                                                        "Select Folder to Save Profiles Backup",
                                                        |target_dir| {
                                                            let backup_path = target_dir.join(format!("BIR_Profiles_Backup_{}.zip", unix_timestamp()));
                                                            this.update(cx, |this, _cx| {
                                                                let db = this.db.lock().map_err(|_| "Failed to acquire database lock".to_string())?;
                                                                bir_core::export::export_all_profiles_data(&db, &backup_path).map_err(|e| e.to_string())?;
                                                                Ok(backup_path)
                                                            })
                                                            .map_err(|e| e.to_string())?
                                                        },
                                                    )
                                                    .await;
                                                    report_export("Profiles Exported", result).await;
                                                }).detach();
                                            }))
                                    }
                            </div>
                        </div>
                    })
                    .child(rsx! {
                        <div flex flex_col items_start p_6 gap_4 w_full bg={cx.theme().danger.opacity(0.1)}>
                            {SettingRow::new(
                                "Danger Zone: Factory Reset",
                                "Permanently delete all profiles, submissions, drafts, and settings. This cannot be undone.",
                            )
                            .title_color(cx.theme().danger)}
                            {
                                gpui_component::button::Button::new("factory_reset_btn")
                                    .label("Zero Out Everything")
                                    .on_click(cx.listener(|_this, _ev, _window, cx| {
                                        cx.spawn(async move |this, cx| {
                                            let confirm = rfd::AsyncMessageDialog::new()
                                                .set_title("Factory Reset")
                                                .set_description("You are about to permanently delete all profiles, submissions, drafts, and settings. Do you want to create a portable backup of your data first before proceeding?")
                                                .set_buttons(rfd::MessageButtons::YesNoCancel)
                                                .show()
                                                .await;

                                            let proceed = match confirm {
                                                rfd::MessageDialogResult::Cancel => return,
                                                rfd::MessageDialogResult::Yes => {
                                                    let res = crate::export_target::folder_with_fallback(
                                                        "Select Folder to Save Portable Backup",
                                                        |target_dir| {
                                                            let backup_path = target_dir.join(format!("BIR_Portable_Backup_{}.db.zip", unix_timestamp()));
                                                            this.update(cx, |this, _cx| {
                                                                let db = this.db.lock().map_err(|_| "Failed to acquire database lock".to_string())?;
                                                                let _ = db.checkpoint();
                                                                bir_core::export::export_database_zip(&db, &backup_path).map_err(|e| e.to_string())?;
                                                                Ok(backup_path)
                                                            })
                                                            .map_err(|e| e.to_string())?
                                                        },
                                                    )
                                                    .await;
                                                    // Cancel in the folder picker cancels the reset.
                                                    let res = match res {
                                                        Ok(None) => return,
                                                        Ok(Some(saved)) => Ok(saved.path),
                                                        Err(e) => Err(e),
                                                    };
                                                    match res {
                                                        Ok(path) => {
                                                            let continue_reset = rfd::AsyncMessageDialog::new()
                                                                .set_title("Backup Successful")
                                                                .set_description(format!("Backup saved to {}. Proceed with Factory Reset?", path.display()))
                                                                .set_buttons(rfd::MessageButtons::OkCancel)
                                                                .show()
                                                                .await;
                                                            matches!(continue_reset, rfd::MessageDialogResult::Ok)
                                                        }
                                                        Err(e) => {
                                                            rfd::AsyncMessageDialog::new()
                                                                .set_title("Backup Failed")
                                                                .set_description(format!("Backup failed: {}. Factory Reset cancelled.", e))
                                                                .show()
                                                                .await;
                                                            false
                                                        }
                                                    }
                                                }
                                                rfd::MessageDialogResult::No => {
                                                    let final_confirm = rfd::AsyncMessageDialog::new()
                                                        .set_title("Warning: No Backup")
                                                        .set_description("Are you completely sure you want to delete everything WITHOUT a backup?")
                                                        .set_buttons(rfd::MessageButtons::YesNo)
                                                        .show()
                                                        .await;
                                                    matches!(final_confirm, rfd::MessageDialogResult::Yes)
                                                }
                                                _ => false
                                            };

                                            if proceed {
                                                let res = this.update(cx, |this, cx| {
                                                    if let Ok(db) = this.db.lock() {
                                                        if let Err(e) = db.factory_reset() {
                                                            Err(e.to_string())
                                                        } else {
                                                            cx.emit(SettingsEvent::ReloadApp);
                                                            Ok(())
                                                        }
                                                    } else {
                                                        Err("Failed to acquire database lock".to_string())
                                                    }
                                                });

                                                if let Ok(Ok(())) = res {
                                                    rfd::AsyncMessageDialog::new()
                                                        .set_title("Factory Reset Complete")
                                                        .set_description("All data has been cleared and the application has been reset.")
                                                        .show()
                                                        .await;
                                                }
                                            }
                                        }).detach();
                                    }))
                            }
                        </div>
                    })
            )
            )
            .when(self.show_totp_setup, |this| {
                this.child(rsx! {
                    <div absolute inset_0 bg={gpui::rgba(0x000000b2)} flex items_center justify_center>
                        <div w={px(400.)} bg={bg} border_1 border_color={border} rounded_xl p_6 flex flex_col gap_4 shadow_lg>
                            <div text_lg font_weight={FontWeight::BOLD}>{"Connect your authenticator app"}</div>
                            {
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(rsx! {
                                            <div text_sm font_weight={FontWeight::MEDIUM}>
                                                {if self.show_totp_secret_text {
                                                    "Step 1: Enter the secret code below in your authenticator app:"
                                                } else {
                                                    "Step 1: Scan the QR code using your authenticator app:"
                                                }}
                                            </div>
                                        })
                                        .when(!self.show_totp_secret_text, |this| {
                                            this.when_some(self.totp_qr_path.clone(), |this, path| {
                                                this.child(rsx! {
                                                    <div w_full flex justify_center>
                                                        {gpui::img(path)
                                                            .w(px(200.))
                                                            .h(px(200.))
                                                            .object_fit(gpui::ObjectFit::Contain)}
                                                    </div>
                                                })
                                            })
                                            .child(rsx! {
                                                <div w_full flex justify_center mt_2>
                                                    <div id={"trouble_scanning_btn"} text_sm text_color={cx.theme().primary} cursor_pointer
                                                        on_click={cx.listener(|this, _ev, _window, cx| {
                                                            this.show_totp_secret_text = true;
                                                            cx.notify();
                                                        })}>
                                                        {"Trouble scanning?"}
                                                    </div>
                                                </div>
                                            })
                                        })
                                        .when(self.show_totp_secret_text, |this| {
                                            this.when_some(self.totp_secret_temp.clone(), |this, secret| {
                                                this.child(rsx! {
                                                    <div flex flex_col items_center gap_4 mt_2>
                                                        <div flex items_center gap_2 p_3 rounded_md bg={cx.theme().secondary} border_1 border_color={border}>
                                                            <div text_sm font_family={".SF NS Mono"}>{secret.clone()}</div>
                                                            {gpui_component::clipboard::Clipboard::new("totp-secret-clipboard")
                                                                .value(secret)}
                                                        </div>
                                                        <div id={"show_qr_btn"} text_sm text_color={cx.theme().primary} cursor_pointer
                                                            on_click={cx.listener(|this, _ev, _window, cx| {
                                                                this.show_totp_secret_text = false;
                                                                cx.notify();
                                                            })}>
                                                            {"Show QR code instead"}
                                                        </div>
                                                    </div>
                                                })
                                            })
                                        })
                            }
                            <div flex flex_col gap_1 mt_2>
                                <div text_sm font_weight={FontWeight::MEDIUM}>{"Step 2: Enter the 6-digit code to verify"}</div>
                                {OtpInput::new(&self.setup_totp_state).groups(1).large()}
                            </div>
                            <div flex justify_end mt_4>
                                {gpui_component::button::Button::new("cancel_totp_btn")
                                    .label("Cancel")
                                    .small()
                                    .on_click(cx.listener(|this, _ev, window, cx| {
                                        this.show_totp_setup = false;
                                        this.show_totp_secret_text = false;
                                        this.totp_secret_temp = None;
                                        this.totp_qr_path = None;
                                        this.setup_totp_state.update(cx, |s, cx| s.set_value("", window, cx));
                                        this.is_app_totp_enabled = false;
                                        cx.notify();
                                    }))}
                            </div>
                        </div>
                    </div>
                })
            })
            .into_any_element()
    }
}

/// One bordered section of the settings card.
fn section(border: Hsla) -> Div {
    div()
        .flex()
        .flex_col()
        .items_start()
        .w_full()
        .p_6()
        .gap_4()
        .border_b_1()
        .border_color(border)
}

/// Note under "Hide Tax Profiles": hiding needs Profile PINs.
fn hide_profiles_note(profile_pins: bool) -> Option<&'static str> {
    (!profile_pins).then_some("Turn on Profile PINs first")
}

/// The new hide setting after the switch is clicked, or `None` when the
/// click must be ignored because Profile PINs are off.
fn hide_profiles_after_toggle(profile_pins: bool, checked: bool) -> Option<bool> {
    profile_pins.then_some(checked)
}

/// Log retention offered in Settings, in days.
const RETENTION_OPTIONS: [u32; 4] = [7, 14, 30, 90];

/// The dropdown's `(value, label)` options: the standard four, plus the saved
/// value when it is something else, so the current setting always shows.
struct RetentionChoices(Vec<(String, String)>);

impl RetentionChoices {
    fn new(saved_days: u32) -> Self {
        let mut days = RETENTION_OPTIONS.to_vec();
        if saved_days > 0 && !days.contains(&saved_days) {
            days.push(saved_days);
            days.sort_unstable();
        }
        Self(
            days.into_iter()
                .map(|d| (d.to_string(), retention_label(d)))
                .collect(),
        )
    }

    fn as_options(&self) -> Vec<(&str, &str)> {
        self.0
            .iter()
            .map(|(value, label)| (value.as_str(), label.as_str()))
            .collect()
    }
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Report a folder export: "Saved to <path>" with "Open folder", the error,
/// or nothing at all when the user cancelled.
async fn report_export(
    title: &str,
    result: Result<Option<crate::export_target::SavedExport>, String>,
) {
    match result {
        Ok(None) => {}
        Ok(Some(saved)) => {
            let choice = rfd::AsyncMessageDialog::new()
                .set_title(title)
                .set_description(saved.message())
                .set_buttons(rfd::MessageButtons::OkCancelCustom(
                    "Open folder".into(),
                    "OK".into(),
                ))
                .show()
                .await;
            if choice == rfd::MessageDialogResult::Custom("Open folder".into())
                && let Err(error) = crate::export_target::open_folder(&saved)
            {
                tracing::warn!("{error}");
            }
        }
        Err(e) => {
            rfd::AsyncMessageDialog::new()
                .set_title("Export Failed")
                .set_description(&e)
                .show()
                .await;
        }
    }
}

fn retention_label(days: u32) -> String {
    if days == 1 {
        "1 day".to_string()
    } else {
        format!("{days} days")
    }
}

#[cfg(test)]
mod tests {
    // Not `use super::*`: that would import gpui's `#[test]` attribute macro.
    use super::{
        RetentionChoices, hide_profiles_after_toggle, hide_profiles_note, retention_label,
    };

    #[test]
    fn retention_values_map_to_labels_and_back() {
        let choices = RetentionChoices::new(30);
        assert_eq!(
            choices.as_options(),
            vec![
                ("7", "7 days"),
                ("14", "14 days"),
                ("30", "30 days"),
                ("90", "90 days")
            ]
        );
        assert_eq!(retention_label(1), "1 day");
    }

    #[test]
    fn an_off_list_saved_retention_still_shows() {
        let choices = RetentionChoices::new(3);
        assert_eq!(choices.as_options()[0], ("3", "3 days"));
        assert_eq!(choices.as_options().len(), 5);
    }

    #[test]
    fn hiding_profiles_needs_profile_pins() {
        assert_eq!(
            hide_profiles_note(false),
            Some("Turn on Profile PINs first")
        );
        assert_eq!(hide_profiles_note(true), None);
        assert_eq!(hide_profiles_after_toggle(false, true), None);
        assert_eq!(hide_profiles_after_toggle(true, true), Some(true));
        assert_eq!(hide_profiles_after_toggle(true, false), Some(false));
    }
}
