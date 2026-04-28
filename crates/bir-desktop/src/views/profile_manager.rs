use gpui::prelude::*;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState, OtpInput, OtpState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

use crate::components::combobox::{Combobox, ComboboxEvent, ComboboxState};
use crate::components::date_input::{DateInput, DateInputEvent, DateInputState};

use crate::components::tin_input::TinInput;
use bir_core::db::Database;
use bir_core::naming::Tin;
use bir_core::profile::{TaxpayerProfile, TaxpayerType};
use bir_core::reference::get_all_rdos;
use bir_core::validation::{ValidationError, validate_profile};

pub enum ProfileEvent {
    Saved(String),
}

use bir_core::profile::EmailAuthMethod;

pub struct ProfileManagerView {
    db: Arc<Mutex<Database>>,
    tin_input: Entity<TinInput>,
    rdo_select: Entity<ComboboxState>,
    type_select: Entity<ComboboxState>,
    line_of_business: Entity<InputState>,
    name_input: Entity<InputState>,
    address_input: Entity<InputState>,
    zip_input: Entity<InputState>,
    tel_input: Entity<InputState>,
    email_input: Entity<InputState>,
    business_start_input: Entity<DateInputState>,
    is_vat_registered: bool,
    editing_id: Option<i64>,
    tin_duplicate_error: Option<String>,
    errors: Vec<ValidationError>,
    save_message: Option<String>,
    pending_notification: Option<(gpui_component::notification::NotificationType, String)>,
    rdo_options: Vec<String>,

    // Email Tracking Settings
    email_tracking_enabled: bool,

    email_auth_method: EmailAuthMethod,
    imap_email_input: Entity<InputState>,
    imap_password_input: Entity<InputState>,
    is_editing_password: bool,
    imap_host_input: Entity<InputState>,
    connection_test_message: Option<(bool, String)>,
    oauth_connected: bool,
    active_tab: usize,

    // Stored credentials from DB to prevent overwriting when saving
    stored_imap_app_password: Option<String>,
    stored_oauth_access_token: Option<String>,
    stored_oauth_refresh_token: Option<String>,
    stored_test_notification_enabled: bool,
    stored_is_archived: bool,
    stored_profile_pin_hash: Option<String>,

    enable_profile_pin: bool,
    profile_pin_input: Entity<OtpState>,

    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ProfileEvent> for ProfileManagerView {}

impl ProfileManagerView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let tin_input = cx.new(|cx| TinInput::new(window, cx));
        let rdo_options = get_all_rdos()
            .into_iter()
            .map(|r| format!("{} - {}", r.code, r.description))
            .collect::<Vec<_>>();

        let rdo_select = cx.new(|cx| ComboboxState::new(rdo_options.clone(), window, cx));
        let type_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Individual".to_string(),
                    "Corporation".to_string(),
                    "Partnership".to_string(),
                ],
                window,
                cx,
            )
        });

        let line_of_business =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. SOFTWARE DEVELOPMENT"));
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Taxpayer's Name (Last Name, First Name, Middle Name)")
        });
        let address_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Registered Address"));
        let zip_input = cx.new(|cx| InputState::new(window, cx).placeholder("Zip Code"));
        let tel_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Mobile or Telephone No."));
        let email_input = cx.new(|cx| InputState::new(window, cx).placeholder("Email Address"));
        let business_start_input = cx.new(|cx| DateInputState::new(window, cx));

        let imap_email_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Email Address"));
        let imap_password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("App Password")
        });
        let imap_host_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("imap.gmail.com");
            input.set_value("imap.gmail.com".to_string(), window, cx);
            input
        });

        let profile_pin_input = cx.new(|cx| {
            let mut input = OtpState::new(4, window, cx);
            input = input.masked(true);
            input
        });

        let subscriptions = vec![
            cx.subscribe(&tin_input, Self::on_tin_event),
            cx.subscribe_in(&name_input, window, Self::on_input_event),
            cx.subscribe(&rdo_select, Self::on_combobox_event),
            cx.subscribe(&type_select, Self::on_combobox_event),
            cx.subscribe(&business_start_input, Self::on_date_event),
        ];

        Self {
            db,
            tin_input,
            rdo_select,
            type_select,
            line_of_business,
            name_input,
            address_input,
            zip_input,
            tel_input,
            email_input,
            business_start_input,
            is_vat_registered: false,
            editing_id: None,
            tin_duplicate_error: None,
            errors: Vec::new(),
            save_message: None,
            rdo_options,
            email_tracking_enabled: false,

            email_auth_method: EmailAuthMethod::GoogleOAuth,
            imap_email_input,
            imap_password_input,
            is_editing_password: true,
            imap_host_input,
            connection_test_message: None,
            oauth_connected: false,
            active_tab: 0,
            stored_imap_app_password: None,
            stored_oauth_access_token: None,
            stored_oauth_refresh_token: None,
            stored_test_notification_enabled: false,
            stored_is_archived: false,
            stored_profile_pin_hash: None,
            enable_profile_pin: false,
            profile_pin_input,
            pending_notification: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn reset_for_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_id = None;
        self.tin_duplicate_error = None;
        self.is_vat_registered = false;
        self.errors.clear();
        self.save_message = None;

        self.email_auth_method = EmailAuthMethod::GoogleOAuth;
        self.stored_test_notification_enabled = false;
        self.stored_is_archived = false;
        self.connection_test_message = None;
        self.oauth_connected = false;
        self.active_tab = 0;
        self.stored_imap_app_password = None;
        self.stored_oauth_access_token = None;
        self.stored_oauth_refresh_token = None;
        self.pending_notification = None;
        self.is_editing_password = true;
        self.stored_profile_pin_hash = None;

        // Auto-check PIN if global "Enable Profile PINs" is on
        let global_pins_enabled = if let Ok(db) = self.db.lock() {
            db.get_setting("enable_profile_pins")
                .ok()
                .flatten()
                .as_deref()
                == Some("true")
        } else {
            false
        };
        self.enable_profile_pin = global_pins_enabled;
        self.profile_pin_input
            .update(cx, |input, cx| input.set_value("", window, cx));

        self.imap_email_input
            .update(cx, |input, cx| input.set_value(String::new(), window, cx));
        self.imap_password_input
            .update(cx, |input, cx| input.set_value(String::new(), window, cx));
        self.imap_host_input.update(cx, |input, cx| {
            input.set_value("imap.gmail.com".to_string(), window, cx)
        });
        self.tin_input.update(cx, |tin, cx| tin.clear(window, cx));
        for input in [
            &self.line_of_business,
            &self.name_input,
            &self.address_input,
            &self.zip_input,
            &self.tel_input,
            &self.email_input,
        ] {
            input.update(cx, |input, cx| input.set_value(String::new(), window, cx));
        }
        self.business_start_input
            .update(cx, |input, cx| input.set_date(None, window, cx));
        self.rdo_select.update(cx, |select, cx| {
            select.set_selected_value("", window, cx);
        });
        self.type_select.update(cx, |select, cx| {
            select.set_selected_value("Individual", window, cx);
        });
        cx.notify();
    }

    pub fn prefill_name(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.name_input.update(cx, |input, cx| {
            input.set_value(name.to_string(), window, cx);
        });
        cx.notify();
    }

    pub fn prefill_tin(&mut self, tin_str: &str, window: &mut Window, cx: &mut Context<Self>) {
        let tin_clean = tin_str.replace("-", "");
        let tin = bir_core::naming::Tin {
            segment1: tin_clean.get(0..3).unwrap_or("").to_string(),
            segment2: tin_clean.get(3..6).unwrap_or("").to_string(),
            segment3: tin_clean.get(6..9).unwrap_or("").to_string(),
            branch: tin_clean.get(9..).unwrap_or("").to_string(),
        };
        self.tin_input.update(cx, |input, cx| {
            input.set_from_tin(&tin, window, cx);
        });
        cx.notify();
    }

    pub fn edit_profile(
        &mut self,
        profile: TaxpayerProfile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editing_id = profile.id;
        self.is_vat_registered = profile.is_vat_registered;
        self.email_tracking_enabled = profile.email_tracking_enabled;

        self.email_auth_method = profile.email_auth_method.clone();

        self.imap_email_input.update(cx, |input, cx| {
            input.set_value(profile.imap_email.clone().unwrap_or_default(), window, cx)
        });
        self.imap_host_input.update(cx, |input, cx| {
            input.set_value(
                profile
                    .imap_host
                    .clone()
                    .unwrap_or_else(|| "imap.gmail.com".to_string()),
                window,
                cx,
            )
        });

        self.stored_imap_app_password = profile.imap_app_password.clone();
        self.stored_oauth_access_token = profile.oauth_access_token.clone();
        self.stored_oauth_refresh_token = profile.oauth_refresh_token.clone();
        self.stored_test_notification_enabled = profile.test_notification_enabled;
        self.stored_is_archived = profile.is_archived;
        self.stored_profile_pin_hash = profile.profile_pin_hash.clone();
        self.enable_profile_pin = profile.profile_pin_hash.is_some();
        self.profile_pin_input
            .update(cx, |input, cx| input.set_value("", window, cx));

        if profile.email_auth_method == EmailAuthMethod::GoogleOAuth
            && profile.oauth_refresh_token.is_some()
        {
            self.is_editing_password = false;
        } else {
            self.is_editing_password = profile.imap_app_password.is_none();
        }

        self.imap_password_input.update(cx, |input, cx| {
            input.set_value(
                profile.imap_app_password.clone().unwrap_or_default(),
                window,
                cx,
            )
        });

        self.oauth_connected = profile.oauth_refresh_token.is_some()
            && !profile.oauth_refresh_token.as_ref().unwrap().is_empty();

        self.errors.clear();
        self.save_message = None;
        self.pending_notification = None;
        self.tin_input
            .update(cx, |tin, cx| tin.set_from_tin(&profile.tin, window, cx));

        self.name_input.update(cx, |input, cx| {
            input.set_value(profile.full_name.clone(), window, cx)
        });
        self.address_input.update(cx, |input, cx| {
            input.set_value(profile.registered_address.clone(), window, cx)
        });
        self.zip_input.update(cx, |input, cx| {
            input.set_value(profile.zip_code.clone(), window, cx)
        });
        self.tel_input.update(cx, |input, cx| {
            input.set_value(profile.phone.clone(), window, cx)
        });
        self.email_input.update(cx, |input, cx| {
            input.set_value(profile.email.clone(), window, cx)
        });
        self.line_of_business.update(cx, |input, cx| {
            input.set_value(profile.line_of_business.clone(), window, cx)
        });
        self.business_start_input.update(cx, |input, cx| {
            input.set_date(profile.business_start_date, window, cx)
        });

        let rdo_value = self
            .rdo_options
            .iter()
            .find(|option| option.starts_with(&profile.rdo_code))
            .cloned()
            .unwrap_or(profile.rdo_code.clone());
        self.rdo_select.update(cx, |select, cx| {
            select.set_selected_value(&rdo_value, window, cx);
        });
        let type_value = taxpayer_type_label(&profile.taxpayer_type).to_string();
        self.type_select.update(cx, |select, cx| {
            select.set_selected_value(&type_value, window, cx);
        });

        cx.notify();
    }

    fn on_tin_event(
        &mut self,
        _state: Entity<TinInput>,
        _event: &gpui_component::input::InputEvent,
        cx: &mut Context<Self>,
    ) {
        let tin_val = self.tin_input.read(cx).value(cx);
        let is_valid_format = tin_val.len() == 12 || tin_val.len() == 13;

        if !is_valid_format {
            self.tin_duplicate_error = None;
            cx.notify();
            return;
        }

        // Only check for duplicates when creating a new profile (not editing)
        if self.editing_id.is_some() {
            self.tin_duplicate_error = None;
            cx.notify();
            return;
        }

        // Check DB for existing profile with same TIN
        if let Ok(db) = self.db.lock() {
            if let Ok(Some(_existing)) = db.get_profile(&tin_val) {
                self.tin_duplicate_error = Some(format!(
                    "A profile with TIN {} already exists. Each TIN must be unique.",
                    self.tin_input.read(cx).formatted_value(cx)
                ));
            } else {
                self.tin_duplicate_error = None;
            }
        }
        cx.notify();
    }

    fn on_input_event(
        &mut self,
        _state: &Entity<InputState>,
        _event: &InputEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // No-op since sync_card is removed
    }

    fn on_combobox_event(
        &mut self,
        _state: Entity<ComboboxState>,
        _event: &ComboboxEvent,
        _cx: &mut Context<Self>,
    ) {
        // No-op since sync_card is removed
    }

    fn on_date_event(
        &mut self,
        _state: Entity<DateInputState>,
        _event: &DateInputEvent,
        _cx: &mut Context<Self>,
    ) {
        // Handle date event if needed
    }

    fn current_profile(&self, cx: &mut Context<Self>) -> TaxpayerProfile {
        let tin_val = self.tin_input.read(cx).formatted_value(cx);
        let tin_clean = tin_val.replace("-", "");
        let tin = Tin {
            segment1: tin_clean.get(0..3).unwrap_or("").to_string(),
            segment2: tin_clean.get(3..6).unwrap_or("").to_string(),
            segment3: tin_clean.get(6..9).unwrap_or("").to_string(),
            branch: tin_clean.get(9..).unwrap_or("").to_string(),
        };

        let type_val = self.type_select.read(cx).selected_value(cx);
        let taxpayer_type = match type_val.as_str() {
            "Corporation" => TaxpayerType::Corporation,
            "Partnership" => TaxpayerType::Partnership,
            _ => TaxpayerType::Individual,
        };

        let rdo_code = self
            .rdo_select
            .read(cx)
            .selected_value(cx)
            .split(" - ")
            .next()
            .unwrap_or("")
            .to_string();

        let business_start_date = self.business_start_input.read(cx).date;

        let profile_pin_hash = if self.enable_profile_pin {
            let pin = self.profile_pin_input.read(cx).value().to_string();
            if pin.len() == 4 {
                Some(bir_core::crypto::hash_pin(&pin))
            } else {
                self.stored_profile_pin_hash.clone()
            }
        } else {
            None
        };

        TaxpayerProfile {
            id: self.editing_id,
            full_name: self.name_input.read(cx).value().trim().to_string(),
            tin,
            rdo_code,
            line_of_business: self.line_of_business.read(cx).value().trim().to_string(),
            registered_address: self.address_input.read(cx).value().trim().to_string(),
            zip_code: self.zip_input.read(cx).value().trim().to_string(),
            phone: self.tel_input.read(cx).value().trim().to_string(),
            email: self.email_input.read(cx).value().trim().to_string(),
            default_form_type: "2551Qv2018".into(),
            taxpayer_type,
            is_vat_registered: self.is_vat_registered,
            business_start_date,
            email_tracking_enabled: self.email_tracking_enabled,

            email_auth_method: self.email_auth_method.clone(),
            imap_email: {
                let val = self.imap_email_input.read(cx).value().trim().to_string();
                if val.is_empty() { None } else { Some(val) }
            },
            imap_host: {
                let val = self.imap_host_input.read(cx).value().trim().to_string();
                if val.is_empty() { None } else { Some(val) }
            },
            _imap_enabled_compat: None,

            // Password logic: use the input if typed, otherwise keep stored
            imap_app_password: {
                let typed_pw = self
                    .imap_password_input
                    .read(cx)
                    .value()
                    .to_string()
                    .replace(' ', "");
                if typed_pw.is_empty() {
                    self.stored_imap_app_password.clone()
                } else {
                    Some(typed_pw)
                }
            },

            // Tokens logic
            oauth_access_token: self.stored_oauth_access_token.clone(),
            oauth_refresh_token: self.stored_oauth_refresh_token.clone(),
            test_notification_enabled: self.stored_test_notification_enabled,
            is_archived: self.stored_is_archived,
            profile_pin_hash,
            tax_classification: None,
            opted_for_8_percent_flat_rate: false,
        }
    }

    fn save_profile(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let profile = self.current_profile(cx);
        self.errors = validate_profile(&profile);

        // Gate: Duplicate TIN check (defense in depth — also checked reactively)
        if self.editing_id.is_none() {
            let tin_str = profile.tin.full();
            if let Ok(db) = self.db.lock()
                && let Ok(Some(_)) = db.get_profile(&tin_str)
            {
                self.tin_duplicate_error = Some(format!(
                    "A profile with TIN {} already exists.",
                    profile.tin.formatted()
                ));
                self.errors.push(ValidationError::new(
                    "tin",
                    "This TIN is already registered to another profile.",
                ));
            }
        }

        if !self
            .business_start_input
            .read(cx)
            .value(cx)
            .trim()
            .is_empty()
            && profile.business_start_date.is_none()
        {
            self.errors.push(ValidationError::new(
                "business_start_date",
                "Date must use YYYY-MM-DD",
            ));
        }

        let typed_pw = self
            .imap_password_input
            .read(cx)
            .value()
            .to_string()
            .replace(' ', "");
        if !typed_pw.is_empty() && typed_pw.len() != 16 {
            self.errors.push(ValidationError::new(
                "imap_app_password",
                "App password must be exactly 16 characters (ignoring spaces).",
            ));
        }

        if !self.errors.is_empty() {
            self.save_message = None;
            cx.notify();
            return;
        }

        // Gate: PIN required when global "Enable Profile PINs" is on
        let global_pins_enabled = if let Ok(db) = self.db.lock() {
            db.get_setting("enable_profile_pins")
                .ok()
                .flatten()
                .as_deref()
                == Some("true")
        } else {
            false
        };
        if global_pins_enabled && self.enable_profile_pin && self.stored_profile_pin_hash.is_none()
        {
            let pin = self.profile_pin_input.read(cx).value().to_string();
            if pin.len() != 4 {
                self.pending_notification = Some((
                    gpui_component::notification::NotificationType::Error,
                    "A 4-digit PIN is required for this profile.".to_string(),
                ));
                self.active_tab = 2;
                cx.notify();
                return;
            }
        }

        // Gate: Email auth required when background cron (Automated Form Submission) is globally active
        let global_cron_active = if let Ok(db) = self.db.lock() {
            db.get_setting("background_cron_enabled")
                .unwrap_or(Some("true".to_string()))
                .map(|s| s == "true")
                .unwrap_or(true)
        } else {
            false
        };
        if global_cron_active && !self.email_tracking_enabled {
            self.pending_notification = Some((
                gpui_component::notification::NotificationType::Error,
                "Email authentication is required. Authenticate your email first.".to_string(),
            ));
            self.active_tab = 1;
            cx.notify();
            return;
        }

        let db_arc = self.db.clone();

        // Immediately update stored password so we don't lose it
        if !typed_pw.is_empty() {
            self.stored_imap_app_password = Some(typed_pw);
            self.is_editing_password = false;
        }

        self.save_message = Some("Saving...".to_string());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let save_result = cx
                .background_executor()
                .spawn(async move {
                    if let Ok(db) = db_arc.lock() {
                        db.save_profile(profile).map_err(|e| e.to_string())
                    } else {
                        Err("Database lock is poisoned".to_string())
                    }
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                match save_result {
                    Ok(saved) => {
                        let saved_id = saved.id.unwrap();
                        this.editing_id = Some(saved_id);
                        this.save_message = None;
                        this.pending_notification = Some((
                            gpui_component::notification::NotificationType::Success,
                            "Profile saved".to_string(),
                        ));

                        let tin_val = this.tin_input.read(cx).value(cx).to_string();
                        cx.emit(ProfileEvent::Saved(tin_val));
                    }
                    Err(err) => {
                        this.save_message = None;
                        this.pending_notification = Some((
                            gpui_component::notification::NotificationType::Error,
                            format!("Save failed: {err}"),
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn field_label(label: &str, cx: &Context<Self>) -> gpui::Div {
        div()
            .text_sm()
            .font_weight(FontWeight::BOLD)
            .text_color(cx.theme().foreground)
            .mb_2()
            .child(label.to_string())
    }

    fn field_error(&self, field: &'static str, _cx: &Context<Self>) -> gpui::Div {
        let text = self
            .errors
            .iter()
            .find(|err| err.field == field)
            .map(|err| err.message.clone())
            .unwrap_or_default();
        div()
            .h_5()
            .text_xs()
            .text_color(gpui::rgba(0xff6b6bff))
            .child(text)
    }
}

impl Render for ProfileManagerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if let Some((notif_type, msg)) = self.pending_notification.take() {
            let notification = gpui_component::notification::Notification::new()
                .message(msg)
                .with_type(notif_type)
                .autohide(true);
            use gpui_component::WindowExt;
            _window.push_notification(notification, cx);
        }

        let title = if self.editing_id.is_some() {
            "Edit Taxpayer Profile"
        } else {
            "Taxpayer Profile Setup"
        };

        let type_val = self.type_select.read(cx).selected_value(cx);
        let is_individual = type_val == "Individual";
        let date_label = if is_individual {
            "Birth Date"
        } else {
            "Business Start Date"
        };

        let global_pins_enabled = if let Ok(db) = self.db.lock() {
            db.get_setting("enable_profile_pins")
                .ok()
                .flatten()
                .as_deref()
                == Some("true")
        } else {
            false
        };

        div()
            .id("profile-scroll")
            .size_full()
            .overflow_y_scroll()
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                    let is_enter = event.keystroke.key == "enter";
                    let is_modifier =
                        event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
                    if is_enter && is_modifier {
                        this.save_profile(_window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .w_full()
                    .p_12()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .max_w(px(600.))
                            .gap_6()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_3xl()
                                            .font_weight(FontWeight::BLACK)
                                            .text_color(cx.theme().foreground)
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .text_base()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(
                                                "Required information is used to pre-fill 2551Q.",
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_start()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .p_1()
                                            .rounded_lg()
                                            .bg(cx.theme().secondary)
                                            .child(
                                                div()
                                                    .id("tab_0")
                                                    .px_4()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .cursor_pointer()
                                                    .when(self.active_tab == 0, |s| {
                                                        s.bg(cx.theme().background)
                                                            .shadow_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                    })
                                                    .when(self.active_tab != 0, |s| {
                                                        s.hover(|s| s.bg(cx.theme().muted))
                                                            .text_color(cx.theme().muted_foreground)
                                                            .font_weight(FontWeight::MEDIUM)
                                                    })
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.active_tab = 0;
                                                        cx.notify();
                                                    }))
                                                    .child(div().text_sm().child("Tax Profile")),
                                            )
                                            .child(
                                                div()
                                                    .id("tab_1")
                                                    .px_4()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .cursor_pointer()
                                                    .when(self.active_tab == 1, |s| {
                                                        s.bg(cx.theme().background)
                                                            .shadow_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                    })
                                                    .when(self.active_tab != 1, |s| {
                                                        s.hover(|s| s.bg(cx.theme().muted))
                                                            .text_color(cx.theme().muted_foreground)
                                                            .font_weight(FontWeight::MEDIUM)
                                                    })
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.active_tab = 1;
                                                        cx.notify();
                                                    }))
                                                    .child(div().text_sm().child("Email Settings")),
                                            )
                                            .when(global_pins_enabled, |this| {
                                                this.child(
                                                    div()
                                                        .id("tab_2")
                                                        .px_4()
                                                        .py_1p5()
                                                        .rounded_md()
                                                        .cursor_pointer()
                                                        .when(self.active_tab == 2, |s| {
                                                            s.bg(cx.theme().background)
                                                                .shadow_sm()
                                                                .text_color(cx.theme().foreground)
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                        })
                                                        .when(self.active_tab != 2, |s| {
                                                            s.hover(|s| s.bg(cx.theme().muted))
                                                                .text_color(cx.theme().muted_foreground)
                                                                .font_weight(FontWeight::MEDIUM)
                                                        })
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.active_tab = 2;
                                                            cx.notify();
                                                        }))
                                                        .child(div().text_sm().child("Security")),
                                                )
                                            })
                                            .when(self.editing_id.is_some(), |this| {
                                                this.child(
                                                    div()
                                                        .id("tab_3")
                                                        .px_4()
                                                        .py_1p5()
                                                        .rounded_md()
                                                        .cursor_pointer()
                                                        .when(self.active_tab == 3, |s| {
                                                            s.bg(cx.theme().background)
                                                                .shadow_sm()
                                                                .text_color(cx.theme().foreground)
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                        })
                                                        .when(self.active_tab != 3, |s| {
                                                            s.hover(|s| s.bg(cx.theme().muted))
                                                                .text_color(cx.theme().muted_foreground)
                                                                .font_weight(FontWeight::MEDIUM)
                                                        })
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.active_tab = 3;
                                                            cx.notify();
                                                        }))
                                                        .child(div().text_sm().child("Export")),
                                                )
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .w_full()
                                    .child(if self.active_tab == 0 {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_4()
                                            .child(self.tin_input.clone().into_any_element())
                                            .when(self.tin_duplicate_error.is_some(), |this| {
                                                let msg = self.tin_duplicate_error.clone().unwrap_or_default();
                                                this.child(
                                                    div()
                                                        .px_2()
                                                        .py_1()
                                                        .rounded_md()
                                                        .bg(gpui::rgba(0xef444415))
                                                        .border_1()
                                                        .border_color(gpui::Hsla::from(gpui::rgba(0xef444460)))
                                                        .flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(gpui::Hsla::from(gpui::rgba(0xef4444ff)))
                                                                .child("⚠")
                                                        )
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .text_color(gpui::Hsla::from(gpui::rgba(0xef4444ff)))
                                                                .child(msg)
                                                        )
                                                )
                                            })
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_4()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .child(Self::field_label(
                                                                "Revenue District Office (RDO)",
                                                                cx,
                                                            ))
                                                            .child(Combobox::new(&self.rdo_select))
                                                            .child(self.field_error("rdo_code", cx)),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .child(Self::field_label("Taxpayer Type", cx))
                                                            .child(Combobox::new(&self.type_select)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .child(Self::field_label("Line of Business", cx))
                                                    .child(Input::new(&self.line_of_business))
                                                    .child(self.field_error("line_of_business", cx)),
                                            )
                                            .child(
                                                div()
                                                    .child(Self::field_label("Taxpayer's Name", cx))
                                                    .child(Input::new(&self.name_input))
                                                    .child(self.field_error("full_name", cx)),
                                            )
                                            .child(
                                                div()
                                                    .child(Self::field_label("Registered Address", cx))
                                                    .child(Input::new(&self.address_input))
                                                    .child(self.field_error("registered_address", cx)),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_4()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .child(Self::field_label("Zip Code", cx))
                                                            .child(Input::new(&self.zip_input))
                                                            .child(self.field_error("zip_code", cx)),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .child(Self::field_label("Phone / Telephone No.", cx))
                                                            .child(Input::new(&self.tel_input))
                                                            .child(self.field_error("phone", cx)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .child(Self::field_label("Email Address", cx))
                                                    .child(Input::new(&self.email_input))
                                                    .child(self.field_error("email", cx)),
                                            )
                                            .child(
                                                div()
                                                    .child(Self::field_label(date_label, cx))
                                                    .child(DateInput::new(&self.business_start_input))
                                                    .child(self.field_error("business_start_date", cx)),
                                            )
                                            .child(
                                                div()
                                                    .id("vat_toggle")
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.is_vat_registered = !this.is_vat_registered;
                                                        cx.notify();
                                                    }))
                                                    .child(
                                                        div()
                                                            .w_4()
                                                            .h_4()
                                                            .rounded_sm()
                                                            .border_1()
                                                            .border_color(cx.theme().border)
                                                            .bg(if self.is_vat_registered {
                                                                cx.theme().primary
                                                            } else {
                                                                cx.theme().background
                                                            })
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(if self.is_vat_registered {
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(cx.theme().primary_foreground)
                                                                    .child("✓")
                                                            } else {
                                                                div()
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .child("VAT registered taxpayer"),
                                                    ),
                                            )
                                    } else { div() })
                                    .child(if self.active_tab == 1 {
                                        div()
                                            .p_4()
                                            .rounded_lg()
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().background)
                                            .flex()
                                            .flex_col()
                                            .gap_4()
                                            .w_full()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_4()
                                                    .child(Self::field_label("Authentication Method", cx))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .gap_6()
                                                            .child(
                                                                div()
                                                                    .id("app_password_select")
                                                                    .flex()
                                                                    .items_center()
                                                                    .gap_2()
                                                                    .cursor_pointer()
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        this.email_auth_method = EmailAuthMethod::AppPassword;
                                                                        this.connection_test_message = None;
                                                                        cx.notify();
                                                                    }))
                                                                    .child(
                                                                        div()
                                                                            .w_4()
                                                                            .h_4()
                                                                            .rounded_full()
                                                                            .border_1()
                                                                            .border_color(cx.theme().primary)
                                                                            .flex()
                                                                            .items_center()
                                                                            .justify_center()
                                                                            .child(if matches!(self.email_auth_method, EmailAuthMethod::AppPassword) {
                                                                                div().w_2().h_2().rounded_full().bg(cx.theme().primary)
                                                                            } else {
                                                                                div()
                                                                            })
                                                                    )
                                                                    .child(div().text_sm().child("App Password (Gmail/Outlook/Yahoo)")),
                                                            )
                                                            .child(
                                                                div()
                                                                    .id("oauth_select")
                                                                    .flex()
                                                                    .items_center()
                                                                    .gap_2()
                                                                    .cursor_pointer()
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        this.email_auth_method = EmailAuthMethod::GoogleOAuth;
                                                                        this.connection_test_message = None;
                                                                        cx.notify();
                                                                    }))
                                                                    .child(
                                                                        div()
                                                                            .w_4()
                                                                            .h_4()
                                                                            .rounded_full()
                                                                            .border_1()
                                                                            .border_color(cx.theme().primary)
                                                                            .flex()
                                                                            .items_center()
                                                                            .justify_center()
                                                                            .child(if matches!(self.email_auth_method, EmailAuthMethod::GoogleOAuth) {
                                                                                div().w_2().h_2().rounded_full().bg(cx.theme().primary)
                                                                            } else {
                                                                                div()
                                                                            })
                                                                    )
                                                                    .child(div().text_sm().child("Google Account (OAuth2)")),
                                                            ),
                                                    )
                                                    .child(
                                                        if matches!(self.email_auth_method, EmailAuthMethod::AppPassword) {
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .gap_3()
                                                                .w_full()
                                                                .overflow_x_hidden()
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .child(Self::field_label("IMAP Host", cx))
                                                                        .child(Input::new(&self.imap_host_input)),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .child(Self::field_label("IMAP Email", cx))
                                                                        .child(Input::new(&self.imap_email_input)),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .child(Self::field_label("App Password", cx))
                                                                        .child(
                                                                            div()
                                                                                .flex()
                                                                                .gap_2()
                                                                                .items_center()
                                                                                .child(
                                                                                    div()
                                                                                        .flex_1()
                                                                                        .child(
                                                                                            Input::new(&self.imap_password_input)
                                                                                                .mask_toggle()
                                                                                                .disabled(!self.is_editing_password)
                                                                                        )
                                                                                )
                                                                                .child(
                                                                                    gpui_component::button::Button::new("edit_app_pw")
                                                                                        .ghost()
                                                                                        .label(if self.is_editing_password && self.stored_imap_app_password.is_some() {
                                                                                            "Cancel"
                                                                                        } else {
                                                                                            "Edit"
                                                                                        })
                                                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                                                            this.is_editing_password = !this.is_editing_password;
                                                                                            cx.notify();
                                                                                        }))
                                                                                )
                                                                        )
                                                                        .child(self.field_error("imap_app_password", cx))
                                                                        .child(
                                                                            div()
                                                                                .flex()
                                                                                .flex_col()
                                                                                .gap_2()
                                                                                .child(
                                                                                    div()
                                                                                        .flex()
                                                                                        .items_center()
                                                                                        .gap_1()
                                                                                        .text_xs()
                                                                                        .text_color(cx.theme().muted_foreground)
                                                                                        .child("Get App Password:")
                                                                                        .child(
                                                                                            div()
                                                                                                .id("link_google_app_pw")
                                                                                                .text_xs()
                                                                                                .text_color(gpui::Hsla::from(gpui::rgba(0x3b82f6ff)))
                                                                                                .cursor_pointer()
                                                                                                .hover(|s| s.underline())
                                                                                                .child("Google")
                                                                                                .on_click(|_, _, _| {
                                                                                                    let _ = open::that("https://myaccount.google.com/apppasswords");
                                                                                                })
                                                                                        )
                                                                                        .child(
                                                                                            div()
                                                                                                .text_xs()
                                                                                                .text_color(cx.theme().muted_foreground)
                                                                                                .child("·")
                                                                                        )
                                                                                        .child(
                                                                                            div()
                                                                                                .id("link_outlook_app_pw")
                                                                                                .text_xs()
                                                                                                .text_color(gpui::Hsla::from(gpui::rgba(0x3b82f6ff)))
                                                                                                .cursor_pointer()
                                                                                                .hover(|s| s.underline())
                                                                                                .child("Outlook")
                                                                                                .on_click(|_, _, _| {
                                                                                                    let _ = open::that("https://account.live.com/proofs/AppPassword");
                                                                                                })
                                                                                        )
                                                                                        .child(
                                                                                            div()
                                                                                                .text_xs()
                                                                                                .text_color(cx.theme().muted_foreground)
                                                                                                .child("·")
                                                                                        )
                                                                                        .child(
                                                                                            div()
                                                                                                .id("link_yahoo_app_pw")
                                                                                                .text_xs()
                                                                                                .text_color(gpui::Hsla::from(gpui::rgba(0x3b82f6ff)))
                                                                                                .cursor_pointer()
                                                                                                .hover(|s| s.underline())
                                                                                                .child("Yahoo")
                                                                                                .on_click(|_, _, _| {
                                                                                                    let _ = open::that("https://login.yahoo.com/account/security/app-passwords");
                                                                                                })
                                                                                        )
                                                                                )
                                                                                .child(
                                                                                    div()
                                                                                        .text_xs()
                                                                                        .text_color(cx.theme().muted_foreground)
                                                                                        .child("Note: You must enable 2-Step Verification (2FA) in your account settings before you can generate an App Password.")
                                                                                )
                                                                        ),
                                                                )
                                                        } else {
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .gap_3()
                                                                .w_full()
                                                                .overflow_x_hidden()
                                                                .child(
                                                                    if self.oauth_connected {
                                                                        let email = self.imap_email_input.read(cx).value();
                                                                        div()
                                                                            .text_sm()
                                                                            .text_color(cx.theme().muted_foreground)
                                                                            .w_full()
                                                                            .overflow_x_hidden()
                                                                            .child(format!("Connected as {}", email))
                                                                    } else {
                                                                        div()
                                                                    }
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex()
                                                                        .items_center()
                                                                        .gap_2()
                                                                        .child(
                                                                            gpui_component::button::Button::new("connect_google")
                                                                                .label(if self.oauth_connected { "Re-authorize" } else { "Connect Google Account" })
                                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                                    let editing_id = this.editing_id;
                                                                                    cx.spawn(async move |this, cx| {
                                                                                        let (tx, rx) = tokio::sync::oneshot::channel();
                                                                                        std::thread::spawn(move || {
                                                                                            let res = bir_core::email::start_oauth_flow();
                                                                                            let _ = tx.send(res);
                                                                                        });
                                                                                        let result = rx.await.unwrap_or_else(|_| Err(anyhow::anyhow!("OAuth thread failed")));

                                                                                        let _ = this.update(cx, |this, cx| {
                                                                                            match result {
                                                                                                Ok((email, access_token, refresh_token)) => {
                                                                                                    this.oauth_connected = true;
                                                                                                    this.email_tracking_enabled = true;
                                                                                                    this.stored_oauth_access_token = Some(access_token.clone());
                                                                                                    this.stored_oauth_refresh_token = Some(refresh_token.clone());
                                                                                                    this.connection_test_message = Some((true, format!("Google account connected successfully for {}.", email)));

                                                                                                    // If profile is already saved, persist tokens to DB immediately
                                                                                                    if let Some(id) = editing_id
                                                                                                        && let Ok(db) = this.db.lock()
                                                                                                            && let Ok(Some(mut profile)) = db.get_profile(&id.to_string()) {
                                                                                                                profile.email = email;
                                                                                                                profile.oauth_access_token = Some(access_token);
                                                                                                                profile.oauth_refresh_token = Some(refresh_token);
                                                                                                                let _ = db.save_profile(profile);
                                                                                                            }
                                                                                                    // For unsaved profiles, tokens are stored in memory and
                                                                                                    // will be persisted when the profile is saved via current_profile()
                                                                                                }
                                                                                                Err(e) => {
                                                                                                    this.connection_test_message = Some((false, format!("OAuth failed: {}", e)));
                                                                                                }
                                                                                            }
                                                                                            cx.notify();
                                                                                        });
                                                                                    }).detach();
                                                                                })),
                                                                        )
                                                                        .child(if self.oauth_connected {
                                                                                div()
                                                                                    .text_sm()
                                                                                    .text_color(gpui::Hsla::from(gpui::rgba(0x22c55eff)))
                                                                                    .child("● Connected ✓")
                                                                        } else {
                                                                            div()
                                                                                .text_xs()
                                                                                .text_color(gpui::Hsla::from(gpui::rgba(0xef4444ff)))
                                                                                .child("You are required to log in to your Google account.")
                                                                        }),
                                                                )
                                                        }
                                                    )
                                                    .child(
                                                        div()
                                                            .mt_2()
                                                            .flex()
                                                            .flex_col()
                                                            .items_start()
                                                            .gap_4()
                                                            .child(
                                                                gpui_component::button::Button::new("test_connection")
                                                                    .label(if matches!(self.email_auth_method, EmailAuthMethod::AppPassword) {
                                                                        "Verify App Password"
                                                                    } else {
                                                                        "Verify Google OAuth2"
                                                                    })
                                                                    .outline()
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        let profile = this.current_profile(cx);

                                                                        if profile.id.is_none() {
                                                                            this.connection_test_message = Some((false, "Please save the profile first.".to_string()));
                                                                            cx.notify();
                                                                            return;
                                                                        }

                                                                        let typed_password = this.imap_password_input.read(cx).value().to_string().replace(' ', "");

                                                                        // Test with typed password or stored password
                                                                        let mut test_profile = profile.clone();
                                                                        if !typed_password.is_empty() {
                                                                            test_profile.imap_app_password = Some(typed_password.clone());
                                                                        }

                                                                        this.connection_test_message = Some((true, "Testing connection...".to_string()));
                                                                        cx.notify();

                                                                        cx.spawn(async move |this, cx| {
                                                                            let (tx, rx) = tokio::sync::oneshot::channel();
                                                                            std::thread::spawn(move || {
                                                                                let res = bir_core::email::test_connection(&test_profile);
                                                                                let _ = tx.send(res);
                                                                            });
                                                                            let result = rx.await.unwrap_or_else(|_| Err(anyhow::anyhow!("Test connection thread failed")));

                                                                            let _ = this.update(cx, |this, cx| {
                                                                                match result {
                                                                                    Ok(new_access_token) => {
                                                                                        this.connection_test_message = Some((true, "Connection successful! ✓ Email tracking is now active.".to_string()));
                                                                                        // Auto-enable email tracking for the tested method
                                                                                        this.email_tracking_enabled = true;
                                                                                        // Disable other auth method
                                                                                        if matches!(this.email_auth_method, EmailAuthMethod::AppPassword) {
                                                                                            this.oauth_connected = false;
                                                                                        }

                                                                                        // If token refreshed, save it
                                                                                        if let Some(token) = new_access_token {
                                                                                            this.stored_oauth_access_token = Some(token.clone());
                                                                                            if let Ok(db) = this.db.lock()
                                                                                                && let Some(id) = this.editing_id
                                                                                                    && let Ok(Some(mut prof)) = db.get_profile(&id.to_string()) {
                                                                                                        prof.oauth_access_token = Some(token);
                                                                                                        let _ = db.save_profile(prof);
                                                                                                    }
                                                                                        }
                                                                                    }
                                                                                    Err(e) => {
                                                                                        this.connection_test_message = Some((false, format!("Connection failed: {}", e)));
                                                                                        this.email_tracking_enabled = false;
                                                                                    }
                                                                                }
                                                                                cx.notify();
                                                                            });
                                                                        }).detach();
                                                                    })),
                                                            )
                                                            .child(
                                                                if let Some((success, msg)) = self.connection_test_message.clone() {
                                                                    div()
                                                                        .text_sm()
                                                                        .whitespace_normal()
                                                                        .w_full()
                                                                        .overflow_x_hidden()
                                                                        .text_color(if success { gpui::Hsla::from(gpui::rgba(0x22c55eff)) } else { gpui::Hsla::from(gpui::rgba(0xef4444ff)) })
                                                                        .child(msg)
                                                                } else {
                                                                    div()
                                                                }
                                                            )
                                                    )
                                                    .child(
                                                        div()
                                                            .mt_4()
                                                            .pt_4()
                                                            .border_t_1()
                                                            .border_color(cx.theme().border)
                                                            .flex()
                                                            .items_center()
                                                            .gap_2()
                                                            .child(
                                                                if self.email_tracking_enabled {
                                                                    div()
                                                                        .text_sm()
                                                                        .text_color(gpui::Hsla::from(gpui::rgba(0x22c55eff)))
                                                                        .child("● Automated BIR Receipt Tracking is active")
                                                                } else {
                                                                    div()
                                                                        .text_sm()
                                                                        .text_color(cx.theme().muted_foreground)
                                                                        .child("○ Verify your connection to activate email tracking")
                                                                }
                                                            ),
                                                    )
                                            )
                                    } else { div() })
                            )
                            .child(if self.active_tab == 2 && global_pins_enabled {
                                div()
                                    .p_4()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().background)
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .w_full()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .p_4()
                                            .bg(cx.theme().muted)
                                            .rounded_md()
                                            .flex()
                                            .flex_col()
                                            .gap_4()
                                            .child(
                                                div()
                                                    .id("profile_pin_toggle")
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.enable_profile_pin = !this.enable_profile_pin;
                                                        cx.notify();
                                                    }))
                                                    .child(
                                                        div()
                                                            .w_4()
                                                            .h_4()
                                                            .rounded_sm()
                                                            .border_1()
                                                            .border_color(cx.theme().border)
                                                            .bg(if self.enable_profile_pin {
                                                                cx.theme().primary
                                                            } else {
                                                                cx.theme().background
                                                            })
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(if self.enable_profile_pin {
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(cx.theme().primary_foreground)
                                                                    .child("✓")
                                                            } else {
                                                                div()
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(cx.theme().foreground).child("Secure this Profile with a PIN"))
                                                            .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Require a 4-digit PIN when switching to this profile."))
                                                    ),
                                            )
                                            .when(self.enable_profile_pin, |this| {
                                                this.child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_2()
                                                        .child(Self::field_label("4-Digit PIN", cx))
                                                        .child(OtpInput::new(&self.profile_pin_input).large())
                                                )
                                            })
                                    )
                            } else { div() })
                            .child(if self.active_tab == 3 && self.editing_id.is_some() {
                                div()
                                    .p_4()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().background)
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .w_full()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Export Profile"))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Create a complete backup of this profile (JSON)."),
                                            ),
                                    )
                                    .child(
                                        gpui_component::button::Button::new("export_profile_btn")
                                            .label("Export Profile Data")
                                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                                let tin = this.tin_input.read(cx).value(cx).to_string();
                                                cx.spawn(async move |this, cx| {
                                                    let Some(target_dir_handle) = rfd::AsyncFileDialog::new()
                                                        .set_title("Select Folder to Save Profile Backup")
                                                        .pick_folder()
                                                        .await
                                                    else { return; };

                                                    let target_dir = target_dir_handle.path().to_path_buf();

                                                    let res = this.update(cx, |this, _cx| {
                                                        let timestamp = std::time::SystemTime::now()
                                                            .duration_since(std::time::UNIX_EPOCH)
                                                            .unwrap()
                                                            .as_secs();
                                                        let backup_path = target_dir.join(format!("BIR_Profile_{}_{}.zip", tin, timestamp));

                                                        if let Ok(db) = this.db.lock() {
                                                            match bir_core::export::export_profile_data(&db, &tin, &backup_path) {
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
                                                                .set_title("Profile Exported")
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
                                            }))
                                    )
                            } else { div() })
                            .child(
                                div()
                                    .mt_4()
                                    .flex()
                                    .items_center()
                                    .gap_4()
                                    .child(
                                        gpui_component::button::Button::new("save_profile")
                                            .label("Save Profile")
                                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                                this.save_profile(_window, cx);
                                            })),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(self.save_message.clone().unwrap_or_default())
                                    )
                            )
                    )
            )
    }
}

fn taxpayer_type_label(taxpayer_type: &TaxpayerType) -> &'static str {
    match taxpayer_type {
        TaxpayerType::Individual => "Individual",
        TaxpayerType::Corporation => "Corporation",
        TaxpayerType::Partnership => "Partnership",
    }
}
