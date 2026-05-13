use crate::components::otp_paste::paste_otp_value;
use bir_core::db::Database;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{Input, InputEvent, OtpInput, OtpState};
use gpui_component::switch::Switch;
use gpui_component::*;
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
    global_hotkey_input: Entity<gpui_component::input::TextInput>,
}

impl SettingsView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let (is_app_lock_enabled, is_app_totp_enabled, hide_tax_profiles, enable_profile_pins, global_hotkey) =
            if let Ok(guard) = db.lock() {
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
                let hotkey = guard
                    .get_setting("global_hotkey_key")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "E".to_string())
                    .to_uppercase();
                (lock, totp, hide, pins, hotkey)
            } else {
                (false, false, false, false, "E".to_string())
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

        let global_hotkey_input = cx.new(|cx| {
            let mut input = gpui_component::input::TextInput::new(window, cx);
            input.set_text(global_hotkey, window, cx);
            input
        });

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
            global_hotkey_input: global_hotkey_input.clone(),
        };

        cx.subscribe_in(
            &setup_otp,
            window,
            |this: &mut Self, _entity, event: &InputEvent, _window, cx| {
                if let InputEvent::Change = event {
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
            |this: &mut Self, _entity, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
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
            &global_hotkey_input,
            window,
            |this: &mut Self, _entity, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    let text = this.global_hotkey_input.read(cx).text().to_uppercase();
                    let first_char = text.chars().next().unwrap_or('E').to_string();
                    if text != first_char {
                        this.global_hotkey_input.update(cx, |input, cx| {
                            input.set_text(first_char.clone(), window, cx);
                        });
                    }
                    if let Ok(db_guard) = this.db.lock() {
                        let _ = db_guard.set_setting("global_hotkey_key", &first_char);
                    }
                    cx.emit(SettingsEvent::ReloadApp);
                    cx.notify();
                }
            },
        )
        .detach();

        view
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
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
                    .child(
                        div()
                            .flex()
                            .flex_col()
                    .gap_1()
                    .child(div().text_2xl().font_weight(FontWeight::BOLD).child("Settings"))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("Configure security, privacy, and global application preferences."),
                    ),
            )
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
                                div()
                                    .flex()
                                    .w_full()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Master PIN"))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Require a 4-digit PIN to unlock the app. If forgotten, your OS login password will be required."),
                                            ),
                                    )
                                    .child(
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
                                            }))
                                    )
                            )
                            .when(self.is_app_totp_enabled, |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Disable Authenticator App first to use 4-digit PIN.")
                                )
                            })
                            .when(self.show_pin_setup, |this| {
                                this.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_4()
                                        .mt_4()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Set your 4-digit PIN"))
                                                .child(OtpInput::new(&self.setup_otp).groups(1))
                                        )
                                        .child(
                                            gpui_component::button::Button::new("cancel_pin_btn")
                                                .label("Cancel")
                                                .small()
                                                .on_click(cx.listener(|this, _ev, window, cx| {
                                                    this.show_pin_setup = false;
                                                    this.setup_otp.update(cx, |s, cx| s.set_value("", window, cx));
                                                    this.is_app_lock_enabled = false;
                                                    cx.notify();
                                                }))
                                        )
                                )
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
                                div()
                                    .flex()
                                    .w_full()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Master Authenticator App"))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Use a 6-digit TOTP code from an authenticator app (e.g. Google Authenticator) for enhanced security."),
                                            ),
                                    )
                                    .child(
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
                                            }))
                                    )
                            )
                            .when(self.is_app_lock_enabled, |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Disable 4-digit PIN first to use Authenticator App.")
                                )
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
                                div()
                                    .flex()
                                    .w_full()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Enable Profile PINs"))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Allow securing individual tax profiles with a separate 4-digit PIN."),
                                            ),
                                    )
                                    .child(
                                        Switch::new("enable_profile_pins_switch")
                                            .checked(self.enable_profile_pins)
                                            .on_click(cx.listener(|this, checked, _window, cx| {
                                                this.enable_profile_pins = *checked;
                                                if let Ok(db) = this.db.lock() {
                                                    let _ = db.set_setting("enable_profile_pins", if *checked { "true" } else { "false" });
                                                    if !*checked && this.hide_tax_profiles {
                                                        this.hide_tax_profiles = false;
                                                        let _ = db.set_setting("hide_tax_profiles", "false");
                                                    }
                                                }
                                                cx.emit(SettingsEvent::ReloadApp);
                                                cx.notify();
                                            }))
                                    )
                            )
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
                            .when(!self.enable_profile_pins, |this| this.opacity(0.5))
                            .child(
                                div()
                                    .flex()
                                    .w_full()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Hide Tax Profiles from Sidebar"))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Prevent profiles from being listed in the sidebar. Useful in public spaces. Requires Profile PINs to be enabled."),
                                            ),
                                    )
                                    .child(
                                        div().child(
                                            Switch::new("hide_tax_profiles_switch")
                                                .checked(self.hide_tax_profiles)
                                                .on_click(cx.listener(|this, checked, _window, cx| {
                                                    if !this.enable_profile_pins { return; }
                                                    this.hide_tax_profiles = *checked;
                                                    if let Ok(db) = this.db.lock() {
                                                        let _ = db.set_setting("hide_tax_profiles", if *checked { "true" } else { "false" });
                                                    }
                                                    cx.emit(SettingsEvent::ReloadApp);
                                                    cx.notify();
                                                }))
                                        )
                                    )
                            )
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
                                div()
                                    .flex()
                                    .w_full()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Global Toggle Hotkey (Windows)"))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Choose a letter to combine with Win + Shift to toggle the application from anywhere."),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(div().text_sm().child("Win + Shift +"))
                                            .child(
                                                div()
                                                    .w(px(50.))
                                                    .child(Input::new(&self.global_hotkey_input))
                                            )
                                    )
                            )
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
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Export Data"))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Create a complete backup of your entire application database or all profiles."),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_4()
                                    .child(
                                        gpui_component::button::Button::new("export_database_btn")
                                            .label("Export Full Database")
                                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                                cx.spawn(async move |this, cx| {
                                                    let Some(target_dir_handle) = rfd::AsyncFileDialog::new()
                                                        .set_title("Select Folder to Save Database Backup")
                                                        .pick_folder()
                                                        .await
                                                    else { return; };

                                                    let target_dir = target_dir_handle.path().to_path_buf();

                                                    let res = this.update(cx, |this, _cx| {
                                                        let db_path = bir_core::db::default_database_path();
                                                        if !db_path.exists() { return Err("Database file not found".to_string()); }

                                                        let timestamp = std::time::SystemTime::now()
                                                            .duration_since(std::time::UNIX_EPOCH)
                                                            .unwrap()
                                                            .as_secs();
                                                        let backup_path = target_dir.join(format!("BIR_Database_Backup_{}.db.zip", timestamp));

                                                        if let Ok(db) = this.db.lock() {
                                                            let _ = db.checkpoint();
                                                            match bir_core::export::export_database_zip(&db, &backup_path) {
                                                                Ok(_) => Ok(backup_path),
                                                                Err(e) => Err(e.to_string()),
                                                            }
                                                        } else {
                                                            Err("Failed to acquire database lock".to_string())
                                                        }
                                                    });

                                                    match res {
                                                        Ok(Ok(path)) => {
                                                            rfd::AsyncMessageDialog::new()
                                                                .set_title("Database Exported")
                                                                .set_description(format!("Saved to {}", path.display()))
                                                                .show()
                                                                .await;
                                                        }
                                                        Ok(Err(e)) => {
                                                            rfd::AsyncMessageDialog::new()
                                                                .set_title("Export Failed")
                                                                .set_description(&e)
                                                                .show()
                                                                .await;
                                                        }
                                                        _ => {}
                                                    }
                                                }).detach();
                                            })),
                                    )
                                    .child(
                                        gpui_component::button::Button::new("export_all_profiles_btn")
                                            .label("Export All Profiles (JSON)")
                                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                                cx.spawn(async move |this, cx| {
                                                    let Some(target_dir_handle) = rfd::AsyncFileDialog::new()
                                                        .set_title("Select Folder to Save Profiles Backup")
                                                        .pick_folder()
                                                        .await
                                                    else { return; };

                                                    let target_dir = target_dir_handle.path().to_path_buf();

                                                    let res = this.update(cx, |this, _cx| {
                                                        let timestamp = std::time::SystemTime::now()
                                                            .duration_since(std::time::UNIX_EPOCH)
                                                            .unwrap()
                                                            .as_secs();
                                                        let backup_path = target_dir.join(format!("BIR_Profiles_Backup_{}.zip", timestamp));

                                                        if let Ok(db) = this.db.lock() {
                                                            match bir_core::export::export_all_profiles_data(&db, &backup_path) {
                                                                Ok(_) => Ok(backup_path),
                                                                Err(e) => Err(e.to_string()),
                                                            }
                                                        } else {
                                                            Err("Failed to acquire database lock".to_string())
                                                        }
                                                    });

                                                    match res {
                                                        Ok(Ok(path)) => {
                                                            rfd::AsyncMessageDialog::new()
                                                                .set_title("Profiles Exported")
                                                                .set_description(format!("Saved to {}", path.display()))
                                                                .show()
                                                                .await;
                                                        }
                                                        Ok(Err(e)) => {
                                                            rfd::AsyncMessageDialog::new()
                                                                .set_title("Export Failed")
                                                                .set_description(&e)
                                                                .show()
                                                                .await;
                                                        }
                                                        _ => {}
                                                    }
                                                }).detach();
                                            })),
                                    )
                            )
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .p_6()
                            .gap_4()
                            .w_full()
                            .bg(cx.theme().danger.opacity(0.1))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(cx.theme().danger).child("Danger Zone: Factory Reset"))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Permanently delete all profiles, submissions, drafts, and settings. This cannot be undone."),
                                    ),
                            )
                            .child(
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
                                                    let Some(target_dir_handle) = rfd::AsyncFileDialog::new()
                                                        .set_title("Select Folder to Save Portable Backup")
                                                        .pick_folder()
                                                        .await
                                                    else { return; };

                                                    let target_dir = target_dir_handle.path().to_path_buf();
                                                    let res = this.update(cx, |this, _cx| {
                                                        let timestamp = std::time::SystemTime::now()
                                                            .duration_since(std::time::UNIX_EPOCH)
                                                            .unwrap()
                                                            .as_secs();
                                                        let backup_path = target_dir.join(format!("BIR_Portable_Backup_{}.db.zip", timestamp));
                                                        if let Ok(db) = this.db.lock() {
                                                            let _ = db.checkpoint();
                                                            match bir_core::export::export_database_zip(&db, &backup_path) {
                                                                Ok(_) => Ok(backup_path),
                                                                Err(e) => Err(e.to_string()),
                                                            }
                                                        } else {
                                                            Err("Failed to acquire database lock".to_string())
                                                        }
                                                    });

                                                    match res {
                                                        Ok(Ok(path)) => {
                                                            let continue_reset = rfd::AsyncMessageDialog::new()
                                                                .set_title("Backup Successful")
                                                                .set_description(format!("Backup saved to {}. Proceed with Factory Reset?", path.display()))
                                                                .set_buttons(rfd::MessageButtons::OkCancel)
                                                                .show()
                                                                .await;
                                                            matches!(continue_reset, rfd::MessageDialogResult::Ok)
                                                        }
                                                        Ok(Err(e)) => {
                                                            rfd::AsyncMessageDialog::new()
                                                                .set_title("Backup Failed")
                                                                .set_description(format!("Backup failed: {}. Factory Reset cancelled.", e))
                                                                .show()
                                                                .await;
                                                            false
                                                        }
                                                        _ => false
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
                                    })),
                            )
                    )
            )
            )
            .when(self.show_totp_setup, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(gpui::rgba(0x000000b2))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(px(400.))
                                .bg(bg)
                                .border_1()
                                .border_color(border)
                                .rounded_xl()
                                .p_6()
                                .flex()
                                .flex_col()
                                .gap_4()
                                .shadow_lg()
                                .child(div().text_lg().font_weight(FontWeight::BOLD).child("Connect your authenticator app"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(
                                            if self.show_totp_secret_text {
                                                "Step 1: Enter the secret code below in your authenticator app:"
                                            } else {
                                                "Step 1: Scan the QR code using your authenticator app:"
                                            }
                                        ))
                                        .when(!self.show_totp_secret_text, |this| {
                                            this.when_some(self.totp_qr_path.clone(), |this, path| {
                                                this.child(
                                                    div()
                                                        .w_full()
                                                        .flex()
                                                        .justify_center()
                                                        .child(
                                                            gpui::img(path)
                                                                .w(px(200.))
                                                                .h(px(200.))
                                                                .object_fit(gpui::ObjectFit::Contain)
                                                        )
                                                )
                                            })
                                            .child(
                                                div()
                                                    .w_full()
                                                    .flex()
                                                    .justify_center()
                                                    .mt_2()
                                                    .child(
                                                        div()
                                                            .id("trouble_scanning_btn")
                                                            .text_sm()
                                                            .text_color(cx.theme().primary)
                                                            .cursor_pointer()
                                                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                                                this.show_totp_secret_text = true;
                                                                cx.notify();
                                                            }))
                                                            .child("Trouble scanning?")
                                                    )
                                            )
                                        })
                                        .when(self.show_totp_secret_text, |this| {
                                            this.when_some(self.totp_secret_temp.clone(), |this, secret| {
                                                this.child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .items_center()
                                                        .gap_4()
                                                        .mt_2()
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap_2()
                                                                .p_3()
                                                                .rounded_md()
                                                                .bg(cx.theme().secondary)
                                                                .border_1()
                                                                .border_color(border)
                                                                .child(div().text_sm().font_family(".SF NS Mono").child(secret.clone()))
                                                                .child(
                                                                    gpui_component::clipboard::Clipboard::new("totp-secret-clipboard")
                                                                        .value(secret)
                                                                )
                                                        )
                                                        .child(
                                                            div()
                                                                .id("show_qr_btn")
                                                                .text_sm()
                                                                .text_color(cx.theme().primary)
                                                                .cursor_pointer()
                                                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                                                    this.show_totp_secret_text = false;
                                                                    cx.notify();
                                                                }))
                                                                .child("Show QR code instead")
                                                        )
                                                )
                                            })
                                        })
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .mt_2()
                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Step 2: Enter the 6-digit code to verify"))
                                        .child(OtpInput::new(&self.setup_totp_state).groups(1).large())
                                )
                                .child(
                                    div()
                                        .flex()
                                        .justify_end()
                                        .mt_4()
                                        .child(
                                            gpui_component::button::Button::new("cancel_totp_btn")
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
                                                }))
                                        )
                                )
                        )
                )
            })
            .into_any_element()
    }
}
