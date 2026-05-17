use chrono::Datelike;
use gpui::prelude::*;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState, OtpInput, OtpState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

use crate::components::combobox::{Combobox, ComboboxEvent, ComboboxState};
use crate::components::date_input::{DateInput, DateInputEvent, DateInputState};
use crate::components::multi_select::{
    MultiSelect, MultiSelectEvent, MultiSelectOption, MultiSelectState,
};
use crate::components::otp_paste::paste_otp_value;

use crate::components::tin_input::TinInput;
use bir_core::db::Database;
use bir_core::naming::Tin;
use bir_core::profile::{
    ComplianceSourceMode, EoptTier, RegisteredTaxType, RegistrationActivityStatus,
    TaxClassification, TaxpayerProfile, TaxpayerType,
};
use bir_core::reference::get_all_rdos;
use bir_core::validation::{ValidationError, validate_profile};

// ─── Tab sub-modules ──────────────────────────────────────────────────────────
// Each file holds one `impl ProfileManagerView` block rendering that tab's UI.
// Imports from this module are re-exported via `use super::*` in each sub-module.
mod tab_email_settings;
mod tab_export;
mod tab_security;
mod tab_tax_profile;
// ─────────────────────────────────────────────────────────────────────────────

pub enum ProfileEvent {
    Saved(String),
}

use bir_core::profile::EmailAuthMethod;

pub struct ProfileManagerView {
    db: Arc<Mutex<Database>>,
    tin_input: Entity<TinInput>,
    rdo_select: Entity<ComboboxState>,
    type_select: Entity<ComboboxState>,
    tax_classification_select: Entity<ComboboxState>,
    eopt_tier_select: Entity<ComboboxState>,
    cooperative_treatment_select: Entity<ComboboxState>,
    is_gpp_partner: bool,
    // ── Granular Withholding (replaces has_employees + is_expanded_withholding_agent) ──
    withholds_compensation: bool,
    withholds_expanded: bool,
    withholds_final: bool,
    is_top_withholding_agent: bool,
    is_government_withholding_entity: bool,
    has_single_employer: bool,
    is_dormant: bool,
    registration_activity_status_select: Entity<ComboboxState>,
    // ── Tax Election Ledger (replaces is_8_percent_flat_rate) ──
    tax_election_year_input: Entity<InputState>,
    tax_election_select: Entity<ComboboxState>,
    excise_select: Entity<MultiSelectState>,
    line_of_business: Entity<InputState>,
    name_input: Entity<InputState>,
    address_input: Entity<InputState>,
    zip_select: Entity<ComboboxState>,
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
    zip_options: Vec<String>,

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
    stored_tax_elections: Vec<bir_core::profile::TaxElectionHistory>,
    stored_profile_versions: Vec<bir_core::profile::TaxProfileVersion>,
    compliance_source_mode: ComplianceSourceMode,
    cor_editing_version_id: Option<String>,
    cor_version_label_input: Entity<InputState>,
    cor_effective_from_input: Entity<InputState>,
    cor_effective_until_input: Entity<InputState>,
    cor_registration_date_input: Entity<InputState>,
    cor_registered_name_input: Entity<InputState>,
    cor_trade_name_input: Entity<InputState>,
    cor_rdo_code_input: Entity<InputState>,
    cor_registered_address_input: Entity<InputState>,
    cor_lob_code_input: Entity<InputState>,
    cor_lob_description_input: Entity<InputState>,
    cor_taxpayer_type_select: Entity<ComboboxState>,
    cor_tax_classification_select: Entity<ComboboxState>,
    cor_eopt_tier_select: Entity<ComboboxState>,
    cor_registration_status_select: Entity<ComboboxState>,
    cor_obligation_form_input: Entity<InputState>,
    cor_obligation_reason_input: Entity<InputState>,
    cor_obligation_source_input: Entity<InputState>,
    cor_deadline_title_input: Entity<InputState>,
    cor_deadline_source_input: Entity<InputState>,
    cor_deadline_forms_input: Entity<InputState>,
    cor_deadline_original_input: Entity<InputState>,
    cor_deadline_adjusted_input: Entity<InputState>,
    cor_deadline_reason_input: Entity<InputState>,

    enable_profile_pin: bool,
    profile_pin_input: Entity<OtpState>,

    is_totp_enabled: bool,
    show_totp_setup: bool,
    setup_totp_state: Entity<OtpState>,
    totp_secret_temp: Option<String>,
    totp_qr_path: Option<std::path::PathBuf>,
    show_totp_secret_text: bool,
    stored_totp_secret: Option<String>,

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

        let rdo_select = cx.new(|cx| ComboboxState::new(rdo_options.clone(), 5, window, cx));
        let type_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Individual".to_string(),
                    "Corporation".to_string(),
                    "Partnership".to_string(),
                    "Cooperative".to_string(),
                    "Estate".to_string(),
                    "Trust".to_string(),
                ],
                6,
                window,
                cx,
            )
        });

        let tax_classification_select = cx.new(|cx| {
            // For Individual taxpayers only; non-Individual types auto-derive
            // the classification from TaxpayerType.
            ComboboxState::new(
                vec![
                    "Purely Compensation".to_string(),
                    "Self-Employed / Professional".to_string(),
                    "Mixed Income".to_string(),
                ],
                3,
                window,
                cx,
            )
        });

        let eopt_tier_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Micro".to_string(),
                    "Small".to_string(),
                    "Medium".to_string(),
                    "Large".to_string(),
                ],
                4,
                window,
                cx,
            )
        });

        // Cooperative-only: tax treatment dropdown
        let cooperative_treatment_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Exempt".to_string(),
                    "Taxable".to_string(),
                    "Mixed".to_string(),
                ],
                3,
                window,
                cx,
            )
        });

        // Registration activity status
        let registration_activity_status_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Active".to_string(),
                    "Dormant Operational".to_string(),
                    "Temporarily Inactive".to_string(),
                    "Officially Closed".to_string(),
                ],
                4,
                window,
                cx,
            )
        });

        // Tax election ledger inputs
        let current_year = chrono::Local::now().date_naive().year();
        let tax_election_year_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Year");
            input.set_value(current_year.to_string(), window, cx);
            input
        });
        let tax_election_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "8% Flat Rate".to_string(),
                    "Graduated + OSD".to_string(),
                    "Graduated + Itemized".to_string(),
                ],
                3,
                window,
                cx,
            )
        });

        // Excise tax liabilities multi-select
        let excise_select = cx.new(|cx| {
            MultiSelectState::new(
                vec![
                    MultiSelectOption::new("alcohol", "Alcohol"),
                    MultiSelectOption::new("auto", "Automobiles & Non-Essential"),
                    MultiSelectOption::new("mineral", "Mineral Products"),
                    MultiSelectOption::new("petroleum", "Petroleum Products"),
                    MultiSelectOption::new("tobacco", "Tobacco Products"),
                    MultiSelectOption::new("sweetened", "Sweetened Beverages"),
                    MultiSelectOption::new("coal", "Coal and Coke"),
                ],
                window,
                cx,
            )
            .placeholder("Select Excise Tax Liabilities")
            .max_visible_chips(3)
        });

        let line_of_business =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. SOFTWARE DEVELOPMENT"));
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Taxpayer's Name (Last Name, First Name, Middle Name)")
        });
        let address_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Registered Address"));
        let zip_options = bir_core::reference::get_all_zipcodes();
        let zip_select = cx.new(|cx| ComboboxState::new(zip_options.clone(), 5, window, cx));
        let tel_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Mobile or Telephone No."));
        let email_input = cx.new(|cx| InputState::new(window, cx).placeholder("Email Address"));
        let business_start_input = cx.new(|cx| DateInputState::new(window, cx));
        let cor_version_label_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Version label"));
        let cor_effective_from_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Effective from YYYY-MM-DD"));
        let cor_effective_until_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Effective until YYYY-MM-DD"));
        let cor_registration_date_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Registration date YYYY-MM-DD"));
        let cor_registered_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Registered name"));
        let cor_trade_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Trade name"));
        let cor_rdo_code_input = cx.new(|cx| InputState::new(window, cx).placeholder("RDO code"));
        let cor_registered_address_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Registered address"));
        let cor_lob_code_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Line of business code"));
        let cor_lob_description_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Line of business description"));
        let cor_taxpayer_type_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Individual".to_string(),
                    "Corporation".to_string(),
                    "Partnership".to_string(),
                    "Cooperative".to_string(),
                    "Estate".to_string(),
                    "Trust".to_string(),
                ],
                6,
                window,
                cx,
            )
        });
        let cor_tax_classification_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Purely Compensation".to_string(),
                    "Self-Employed / Professional".to_string(),
                    "Mixed Income".to_string(),
                    "Corporation".to_string(),
                    "Cooperative Exempt".to_string(),
                    "Cooperative Taxable".to_string(),
                    "Cooperative Mixed".to_string(),
                    "Estate or Trust".to_string(),
                    "None".to_string(),
                ],
                4,
                window,
                cx,
            )
        });
        let cor_eopt_tier_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Micro".to_string(),
                    "Small".to_string(),
                    "Medium".to_string(),
                    "Large".to_string(),
                    "None".to_string(),
                ],
                5,
                window,
                cx,
            )
        });
        let cor_registration_status_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Active".to_string(),
                    "Dormant Operational".to_string(),
                    "Temporarily Inactive".to_string(),
                    "Officially Closed".to_string(),
                ],
                4,
                window,
                cx,
            )
        });
        let cor_obligation_form_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Form code (e.g. 2551Q)"));
        let cor_obligation_reason_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Reason required"));
        let cor_obligation_source_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Source required"));
        let cor_deadline_title_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Title"));
        let cor_deadline_source_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Source required"));
        let cor_deadline_forms_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Form codes (e.g. 1702Q,2551Q)"));
        let cor_deadline_original_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Original deadline YYYY-MM-DD"));
        let cor_deadline_adjusted_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Adjusted deadline YYYY-MM-DD"));
        let cor_deadline_reason_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Reason / note"));

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

        let setup_totp_state = cx.new(|cx| {
            let mut state = OtpState::new(6, window, cx);
            state = state.masked(false);
            state
        });

        let subscriptions = vec![
            cx.subscribe(&tin_input, Self::on_tin_event),
            cx.subscribe_in(&name_input, window, Self::on_input_event),
            cx.subscribe_in(&line_of_business, window, Self::on_input_event),
            cx.subscribe_in(&address_input, window, Self::on_input_event),
            cx.subscribe_in(&email_input, window, Self::on_input_event),
            cx.subscribe(&rdo_select, Self::on_combobox_event),
            cx.subscribe(&zip_select, Self::on_combobox_event),
            cx.subscribe_in(&tel_input, window, Self::on_tel_event),
            cx.subscribe(&type_select, Self::on_combobox_event),
            cx.subscribe(&tax_classification_select, Self::on_combobox_event),
            cx.subscribe(&eopt_tier_select, Self::on_combobox_event),
            cx.subscribe(&cooperative_treatment_select, Self::on_combobox_event),
            cx.subscribe(&excise_select, Self::on_multi_select_event),
            cx.subscribe(&business_start_input, Self::on_date_event),
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
                                this.stored_totp_secret = Some(secret.clone());
                                this.is_totp_enabled = true;
                                this.show_totp_setup = false;
                                this.show_totp_secret_text = false;
                                this.totp_secret_temp = None;
                                this.totp_qr_path = None;
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
            ),
        ];

        Self {
            db,
            tin_input,
            rdo_select,
            type_select,
            tax_classification_select,
            eopt_tier_select,
            cooperative_treatment_select,
            is_gpp_partner: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            has_single_employer: false,
            is_dormant: false,
            registration_activity_status_select,
            tax_election_year_input,
            tax_election_select,
            excise_select,
            line_of_business,
            name_input,
            address_input,
            zip_select,
            tel_input,
            email_input,
            business_start_input,
            is_vat_registered: false,
            editing_id: None,
            tin_duplicate_error: None,
            errors: Vec::new(),
            save_message: None,
            rdo_options,
            zip_options,
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
            stored_tax_elections: vec![],
            stored_profile_versions: vec![],
            compliance_source_mode: ComplianceSourceMode::TemporalSuggestion,
            cor_editing_version_id: None,
            cor_version_label_input,
            cor_effective_from_input,
            cor_effective_until_input,
            cor_registration_date_input,
            cor_registered_name_input,
            cor_trade_name_input,
            cor_rdo_code_input,
            cor_registered_address_input,
            cor_lob_code_input,
            cor_lob_description_input,
            cor_taxpayer_type_select,
            cor_tax_classification_select,
            cor_eopt_tier_select,
            cor_registration_status_select,
            cor_obligation_form_input,
            cor_obligation_reason_input,
            cor_obligation_source_input,
            cor_deadline_title_input,
            cor_deadline_source_input,
            cor_deadline_forms_input,
            cor_deadline_original_input,
            cor_deadline_adjusted_input,
            cor_deadline_reason_input,
            enable_profile_pin: false,
            profile_pin_input,
            is_totp_enabled: false,
            show_totp_setup: false,
            setup_totp_state,
            totp_secret_temp: None,
            totp_qr_path: None,
            show_totp_secret_text: false,
            stored_totp_secret: None,
            pending_notification: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn reset_for_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_id = None;
        self.tin_duplicate_error = None;
        self.is_vat_registered = false;
        self.withholds_compensation = false;
        self.withholds_expanded = false;
        self.withholds_final = false;
        self.is_top_withholding_agent = false;
        self.is_government_withholding_entity = false;
        self.has_single_employer = false;
        self.is_dormant = false;
        self.is_gpp_partner = false;
        self.excise_select.update(cx, |state, cx| {
            state.set_selected_ids(vec![], cx);
        });
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
        self.stored_tax_elections = vec![];
        self.stored_profile_versions = vec![];
        self.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;
        self.cor_editing_version_id = None;
        self.clear_cor_version_editor(window, cx);
        self.clear_cor_override_inputs(window, cx);
        self.is_totp_enabled = false;
        self.show_totp_setup = false;
        self.show_totp_secret_text = false;
        self.totp_secret_temp = None;
        self.totp_qr_path = None;
        self.stored_totp_secret = None;
        self.setup_totp_state
            .update(cx, |input, cx| input.set_value("", window, cx));

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
            &self.tel_input,
            &self.email_input,
        ] {
            input.update(cx, |input, cx| input.set_value(String::new(), window, cx));
        }
        self.zip_select.update(cx, |select, cx| {
            select.set_selected_value("", window, cx);
        });
        self.business_start_input
            .update(cx, |input, cx| input.set_date(None, window, cx));
        self.rdo_select.update(cx, |select, cx| {
            select.set_selected_value("", window, cx);
        });
        self.type_select.update(cx, |select, cx| {
            select.set_selected_value("Individual", window, cx);
        });
        self.tax_classification_select.update(cx, |select, cx| {
            select.set_selected_value("", window, cx);
        });
        self.eopt_tier_select.update(cx, |select, cx| {
            select.set_selected_value("", window, cx);
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
        self.withholds_compensation = profile.withholds_compensation;
        self.withholds_expanded = profile.withholds_expanded;
        self.withholds_final = profile.withholds_final;
        self.is_top_withholding_agent = profile.is_top_withholding_agent;
        self.is_government_withholding_entity = profile.is_government_withholding_entity;
        self.has_single_employer = profile.has_single_employer;
        self.is_dormant = profile.is_dormant;
        self.is_gpp_partner = profile.is_gpp_partner;
        self.stored_tax_elections = profile.tax_elections.clone();
        self.stored_profile_versions = profile.profile_versions.clone();
        self.compliance_source_mode = profile.compliance_source_mode.clone();
        // Populate excise tax multi-select from profile categories
        let mut excise_ids = Vec::new();
        for cat in &profile.excise_tax_categories {
            match cat {
                bir_core::profile::ExciseTaxCategory::Alcohol => {
                    excise_ids.push("alcohol".to_string())
                }
                bir_core::profile::ExciseTaxCategory::AutomobilesAndNonEssential => {
                    excise_ids.push("auto".to_string())
                }
                bir_core::profile::ExciseTaxCategory::Mineral => {
                    excise_ids.push("mineral".to_string())
                }
                bir_core::profile::ExciseTaxCategory::Petroleum => {
                    excise_ids.push("petroleum".to_string())
                }
                bir_core::profile::ExciseTaxCategory::Tobacco => {
                    excise_ids.push("tobacco".to_string())
                }
                bir_core::profile::ExciseTaxCategory::SweetenedBeverages => {
                    excise_ids.push("sweetened".to_string())
                }
                bir_core::profile::ExciseTaxCategory::CoalAndCoke => {
                    excise_ids.push("coal".to_string())
                }
            }
        }
        self.excise_select.update(cx, |state, cx| {
            state.set_selected_ids(excise_ids, cx);
        });
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

        self.stored_totp_secret = profile.totp_secret.clone();
        self.is_totp_enabled = profile.totp_secret.is_some();
        self.show_totp_setup = false;
        self.totp_secret_temp = None;
        self.totp_qr_path = None;
        self.setup_totp_state
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

        let zip_val = self
            .zip_options
            .iter()
            .find(|option| option.starts_with(&profile.zip_code))
            .cloned()
            .unwrap_or(profile.zip_code.clone());

        self.zip_select.update(cx, |select, cx| {
            select.set_selected_value(&zip_val, window, cx);
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
        let tax_class_value = match profile.tax_classification {
            Some(bir_core::profile::TaxClassification::PurelyCompensation) => "Purely Compensation",
            Some(bir_core::profile::TaxClassification::SelfEmployed) => {
                "Self-Employed / Professional"
            }
            Some(bir_core::profile::TaxClassification::MixedIncome) => "Mixed Income",
            // Non-Individual classifications: dropdown is hidden, but keep safe defaults
            Some(bir_core::profile::TaxClassification::Corporation) => "",
            Some(bir_core::profile::TaxClassification::CooperativeExempt) => "",
            Some(bir_core::profile::TaxClassification::CooperativeTaxable) => "",
            Some(bir_core::profile::TaxClassification::CooperativeMixed) => "",
            Some(bir_core::profile::TaxClassification::EstateOrTrust) => "",
            None => "",
        };
        self.tax_classification_select.update(cx, |select, cx| {
            select.set_selected_value(tax_class_value, window, cx);
        });
        let tier_value = match profile.eopt_tier {
            Some(bir_core::profile::EoptTier::Micro) => "Micro",
            Some(bir_core::profile::EoptTier::Small) => "Small",
            Some(bir_core::profile::EoptTier::Medium) => "Medium",
            Some(bir_core::profile::EoptTier::Large) => "Large",
            None => "",
        };
        self.eopt_tier_select.update(cx, |select, cx| {
            select.set_selected_value(tier_value, window, cx);
        });

        // Cooperative tax treatment
        let coop_value = match profile.tax_classification {
            Some(bir_core::profile::TaxClassification::CooperativeExempt) => "Exempt",
            Some(bir_core::profile::TaxClassification::CooperativeTaxable) => "Taxable",
            Some(bir_core::profile::TaxClassification::CooperativeMixed) => "Mixed",
            _ => "",
        };
        self.cooperative_treatment_select.update(cx, |select, cx| {
            select.set_selected_value(coop_value, window, cx);
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
        state: &Entity<InputState>,
        event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            let mut field_to_validate = None;
            let mut value = String::new();

            if state == &self.line_of_business {
                field_to_validate = Some("line_of_business");
                value = self.line_of_business.read(cx).value().to_string();
            } else if state == &self.name_input {
                field_to_validate = Some("full_name");
                value = self.name_input.read(cx).value().to_string();
            } else if state == &self.address_input {
                field_to_validate = Some("registered_address");
                value = self.address_input.read(cx).value().to_string();
            } else if state == &self.email_input {
                field_to_validate = Some("email");
                value = self.email_input.read(cx).value().to_string();
            }

            if let Some(field) = field_to_validate {
                self.validate_field(field, &value);
                cx.notify();
            }
        }
    }

    fn on_tel_event(
        &mut self,
        _state: &Entity<InputState>,
        event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            let phone = self.tel_input.read(cx).value();
            self.validate_field("phone", &phone);
            cx.notify();
        }
    }

    fn on_combobox_event(
        &mut self,
        state: Entity<ComboboxState>,
        event: &ComboboxEvent,
        cx: &mut Context<Self>,
    ) {
        if let Some(val) = event.selected.as_ref() {
            let mut field_to_validate = None;
            let mut value = val.clone();

            if state == self.rdo_select {
                field_to_validate = Some("rdo_code");
            } else if state == self.zip_select {
                field_to_validate = Some("zip_code");
                value = val.split(" - ").next().unwrap_or("").trim().to_string();
            }

            if let Some(field) = field_to_validate {
                self.validate_field(field, &value);
                cx.notify();
            }
        }
    }

    fn on_date_event(
        &mut self,
        _state: Entity<DateInputState>,
        _event: &DateInputEvent,
        _cx: &mut Context<Self>,
    ) {
        // Handle date event if needed
    }

    fn on_multi_select_event(
        &mut self,
        _state: Entity<MultiSelectState>,
        _event: &MultiSelectEvent,
        cx: &mut Context<Self>,
    ) {
        // Just notify to refresh the UI when multi-select changes
        cx.notify();
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
            "Cooperative" => TaxpayerType::Cooperative,
            "Estate" => TaxpayerType::Estate,
            "Trust" => TaxpayerType::Trust,
            _ => TaxpayerType::Individual,
        };

        // For Individual: read user selection from dropdown
        // For Cooperative: read cooperative_treatment_select
        // For other non-Individual types: auto-derive via effective_classification()
        let tax_class_val = self.tax_classification_select.read(cx).selected_value(cx);
        let tax_classification = match taxpayer_type {
            TaxpayerType::Individual => match tax_class_val.as_str() {
                "Purely Compensation" => {
                    Some(bir_core::profile::TaxClassification::PurelyCompensation)
                }
                "Self-Employed / Professional" => {
                    Some(bir_core::profile::TaxClassification::SelfEmployed)
                }
                "Mixed Income" => Some(bir_core::profile::TaxClassification::MixedIncome),
                _ => None,
            },
            TaxpayerType::Cooperative => {
                let coop_val = self
                    .cooperative_treatment_select
                    .read(cx)
                    .selected_value(cx);
                match coop_val.as_str() {
                    "Exempt" => Some(bir_core::profile::TaxClassification::CooperativeExempt),
                    "Taxable" => Some(bir_core::profile::TaxClassification::CooperativeTaxable),
                    "Mixed" => Some(bir_core::profile::TaxClassification::CooperativeMixed),
                    _ => Some(bir_core::profile::TaxClassification::CooperativeTaxable),
                }
            }
            _ => None, // Auto-derived via effective_classification()
        };
        let has_business_activity =
            !matches!(
                tax_classification,
                Some(bir_core::profile::TaxClassification::PurelyCompensation)
            ) && !matches!(taxpayer_type, TaxpayerType::Estate | TaxpayerType::Trust);
        let is_individual_with_business = matches!(taxpayer_type, TaxpayerType::Individual)
            && matches!(
                tax_classification,
                Some(bir_core::profile::TaxClassification::SelfEmployed)
                    | Some(bir_core::profile::TaxClassification::MixedIncome)
            );
        let is_vat_registered = if has_business_activity {
            self.is_vat_registered
        } else {
            false
        };
        let is_gpp_partner = if is_individual_with_business {
            self.is_gpp_partner
        } else {
            false
        };

        let tier_val = self.eopt_tier_select.read(cx).selected_value(cx);
        let eopt_tier = match tier_val.as_str() {
            "Micro" => Some(bir_core::profile::EoptTier::Micro),
            "Small" => Some(bir_core::profile::EoptTier::Small),
            "Medium" => Some(bir_core::profile::EoptTier::Medium),
            "Large" => Some(bir_core::profile::EoptTier::Large),
            _ => None,
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

        let mut profile = TaxpayerProfile {
            id: self.editing_id,
            full_name: self.name_input.read(cx).value().trim().to_string(),
            tin,
            rdo_code,
            line_of_business: self.line_of_business.read(cx).value().trim().to_string(),
            registered_address: self.address_input.read(cx).value().trim().to_string(),
            zip_code: self
                .zip_select
                .read(cx)
                .selected_value(cx)
                .split(" - ")
                .next()
                .unwrap_or("")
                .trim()
                .to_string(),
            phone: self.tel_input.read(cx).value().trim().to_string(),
            email: self.email_input.read(cx).value().trim().to_string(),
            default_form_type: "2551Qv2018".into(),
            taxpayer_type,
            is_vat_registered,
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
            totp_secret: self.stored_totp_secret.clone(),
            tax_classification,
            eopt_tier,
            is_bmbe: false,
            is_gpp_partner,
            is_create_msme: false,
            is_expanded_withholding_agent: self.withholds_expanded
                || self.is_top_withholding_agent
                || self.is_government_withholding_entity,
            atc_codes: vec![],
            excise_tax_categories: {
                let selected = self.excise_select.read(cx).selected_ids();
                let mut cats = vec![];
                for id in selected {
                    match id.as_str() {
                        "alcohol" => cats.push(bir_core::profile::ExciseTaxCategory::Alcohol),
                        "auto" => cats
                            .push(bir_core::profile::ExciseTaxCategory::AutomobilesAndNonEssential),
                        "mineral" => cats.push(bir_core::profile::ExciseTaxCategory::Mineral),
                        "petroleum" => cats.push(bir_core::profile::ExciseTaxCategory::Petroleum),
                        "tobacco" => cats.push(bir_core::profile::ExciseTaxCategory::Tobacco),
                        "sweetened" => {
                            cats.push(bir_core::profile::ExciseTaxCategory::SweetenedBeverages)
                        }
                        "coal" => cats.push(bir_core::profile::ExciseTaxCategory::CoalAndCoke),
                        _ => {}
                    }
                }
                cats
            },
            // Tax elections are now managed via the ledger UI — pass through directly
            tax_elections: self.stored_tax_elections.clone(),
            has_employees: self.withholds_compensation, // compat mirror
            is_dormant: self.is_dormant,
            has_single_employer: self.has_single_employer,
            withholds_compensation: self.withholds_compensation,
            withholds_expanded: self.withholds_expanded,
            withholds_final: self.withholds_final,
            is_top_withholding_agent: self.is_top_withholding_agent,
            is_government_withholding_entity: self.is_government_withholding_entity,
            registration_activity_status: {
                let val = self
                    .registration_activity_status_select
                    .read(cx)
                    .selected_value(cx);
                match val.as_str() {
                    "Dormant Operational" => {
                        bir_core::profile::RegistrationActivityStatus::DormantOperational
                    }
                    "Temporarily Inactive" => {
                        bir_core::profile::RegistrationActivityStatus::TemporarilyInactive
                    }
                    "Officially Closed" => {
                        bir_core::profile::RegistrationActivityStatus::OfficiallyClosed
                    }
                    _ => bir_core::profile::RegistrationActivityStatus::Active,
                }
            },
            profile_versions: self.stored_profile_versions.clone(),
            compliance_source_mode: self.compliance_source_mode.clone(),
        };

        profile
    }

    fn sync_current_profile_to_cor_draft(&mut self, cx: &mut Context<Self>) {
        let mut profile = self.current_profile(cx);
        profile.profile_versions.clear();
        profile.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;

        let mut version = bir_core::profile::TaxProfileVersion::from_profile_backfill(&profile);
        version.id = format!("manual-cor-{}", chrono::Local::now().timestamp_millis());
        version.label = format!("Manual COR {}", chrono::Local::now().format("%Y-%m-%d"));
        version.status = bir_core::profile::TaxProfileVersionStatus::Draft;
        version.source = bir_core::profile::TaxProfileVersionSource::ManualCor;
        version.needs_effective_date_review = version.effective_from.is_none();

        self.compliance_source_mode = ComplianceSourceMode::CorVersioned;
        self.stored_profile_versions.retain(|existing| {
            existing.status != bir_core::profile::TaxProfileVersionStatus::Draft
        });
        self.stored_profile_versions.push(version);
    }

    fn upload_cor_document(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let mut profile = self.current_profile(cx);
        profile.profile_versions.clear();
        profile.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;
        let tin = profile.tin.full().replace(['-', ' '], "");

        cx.spawn(async move |this, cx| {
            let Some(file_handle) = rfd::AsyncFileDialog::new()
                .add_filter("COR document", &["png", "jpg", "jpeg", "pdf"])
                .pick_file()
                .await
            else {
                return;
            };
            let source_path = file_handle.path().to_path_buf();
            match crate::cor_evidence::store_cor_document(&source_path, &tin) {
                Ok(evidence) => {
                    let _ = this.update(cx, move |this, cx| {
                        let mut version =
                            bir_core::profile::TaxProfileVersion::from_profile_backfill(&profile);
                        version.id = format!(
                            "ocr-cor-{}",
                            chrono::Local::now().timestamp_millis()
                        );
                        version.label =
                            format!("Uploaded COR {}", chrono::Local::now().format("%Y-%m-%d"));
                        version.status = bir_core::profile::TaxProfileVersionStatus::Draft;
                        version.source = bir_core::profile::TaxProfileVersionSource::OcrCor;
                        version.needs_effective_date_review = version.effective_from.is_none();
                        version.evidence.push(evidence);

                        this.compliance_source_mode = ComplianceSourceMode::CorVersioned;
                        this.cor_editing_version_id = None;
                        this.stored_profile_versions.push(version);
                        this.save_message = Some(
                            "COR uploaded and attached to a draft. OCR is not configured yet, so review the fields manually before confirming.".into(),
                        );
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.save_message = Some(error);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn open_cor_document(&self, version_id: &str, document_id: &str) {
        let Some(path) = self
            .stored_profile_versions
            .iter()
            .find(|version| version.id == version_id)
            .and_then(|version| {
                version
                    .evidence
                    .iter()
                    .find(|doc| doc.id == document_id)
                    .map(|doc| doc.stored_path.clone())
            })
        else {
            return;
        };
        crate::platform::open_in_system(std::path::Path::new(&path));
    }

    fn remove_cor_document(&mut self, version_id: &str, document_id: &str) {
        let Some(version) = self
            .stored_profile_versions
            .iter_mut()
            .find(|version| version.id == version_id)
        else {
            self.save_message = Some("COR version was not found.".to_string());
            return;
        };

        let removed_path = version
            .evidence
            .iter()
            .find(|document| document.id == document_id)
            .map(|document| document.stored_path.clone());
        let before = version.evidence.len();
        version
            .evidence
            .retain(|document| document.id != document_id);

        if version.evidence.len() == before {
            self.save_message = Some("COR evidence file was not found.".to_string());
            return;
        }

        if let Some(path) = removed_path {
            let _ = std::fs::remove_file(path);
        }
        self.save_message =
            Some("COR evidence removed. Save the profile to persist the change.".to_string());
    }

    fn confirm_cor_version(&mut self, version_id: &str) -> Result<(), String> {
        let Some(version) = self
            .stored_profile_versions
            .iter()
            .find(|version| version.id == version_id)
        else {
            return Err("COR version was not found.".to_string());
        };
        let Some(effective_from) = version.effective_from.or(version.cor.registration_date) else {
            return Err(
                "Set the Business Start Date before confirming this COR version.".to_string(),
            );
        };

        let mut profile = TaxpayerProfile {
            id: self.editing_id,
            full_name: String::new(),
            tin: Tin {
                segment1: String::new(),
                segment2: String::new(),
                segment3: String::new(),
                branch: String::new(),
            },
            rdo_code: String::new(),
            line_of_business: String::new(),
            registered_address: String::new(),
            zip_code: String::new(),
            phone: String::new(),
            email: String::new(),
            default_form_type: String::new(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: self.stored_profile_versions.clone(),
            compliance_source_mode: ComplianceSourceMode::CorVersioned,
        };

        if profile.set_profile_version_confirmed(version_id, effective_from) {
            self.compliance_source_mode = ComplianceSourceMode::CorVersioned;
            self.stored_profile_versions = profile.profile_versions.clone();
            Ok(())
        } else {
            Err("COR version was not found.".to_string())
        }
    }

    fn clear_cor_override_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for input in [
            &self.cor_obligation_form_input,
            &self.cor_obligation_reason_input,
            &self.cor_obligation_source_input,
            &self.cor_deadline_title_input,
            &self.cor_deadline_source_input,
            &self.cor_deadline_forms_input,
            &self.cor_deadline_original_input,
            &self.cor_deadline_adjusted_input,
            &self.cor_deadline_reason_input,
        ] {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
    }

    fn clear_cor_version_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for input in [
            &self.cor_version_label_input,
            &self.cor_effective_from_input,
            &self.cor_effective_until_input,
            &self.cor_registration_date_input,
            &self.cor_registered_name_input,
            &self.cor_trade_name_input,
            &self.cor_rdo_code_input,
            &self.cor_registered_address_input,
            &self.cor_lob_code_input,
            &self.cor_lob_description_input,
        ] {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.cor_taxpayer_type_select.update(cx, |state, cx| {
            state.set_selected_value("Individual", window, cx)
        });
        self.cor_tax_classification_select.update(cx, |state, cx| {
            state.set_selected_value("Self-Employed / Professional", window, cx)
        });
        self.cor_eopt_tier_select
            .update(cx, |state, cx| state.set_selected_value("None", window, cx));
        self.cor_registration_status_select.update(cx, |state, cx| {
            state.set_selected_value("Active", window, cx)
        });
    }

    fn load_cor_version_editor(
        &mut self,
        version_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(version) = self
            .stored_profile_versions
            .iter()
            .find(|version| version.id == version_id)
            .cloned()
        else {
            return Err("COR version was not found.".to_string());
        };

        self.cor_editing_version_id = Some(version.id.clone());
        self.cor_version_label_input
            .update(cx, |input, cx| input.set_value(version.label, window, cx));
        self.cor_effective_from_input.update(cx, |input, cx| {
            input.set_value(
                version
                    .effective_from
                    .map(|date| date.to_string())
                    .unwrap_or_default(),
                window,
                cx,
            )
        });
        self.cor_effective_until_input.update(cx, |input, cx| {
            input.set_value(
                version
                    .effective_until
                    .map(|date| date.to_string())
                    .unwrap_or_default(),
                window,
                cx,
            )
        });
        self.cor_registration_date_input.update(cx, |input, cx| {
            input.set_value(
                version
                    .cor
                    .registration_date
                    .map(|date| date.to_string())
                    .unwrap_or_default(),
                window,
                cx,
            )
        });
        self.cor_registered_name_input.update(cx, |input, cx| {
            input.set_value(version.cor.registered_name, window, cx)
        });
        self.cor_trade_name_input.update(cx, |input, cx| {
            input.set_value(version.cor.trade_name.unwrap_or_default(), window, cx)
        });
        self.cor_rdo_code_input.update(cx, |input, cx| {
            input.set_value(version.cor.rdo_code, window, cx)
        });
        self.cor_registered_address_input.update(cx, |input, cx| {
            input.set_value(version.cor.registered_address, window, cx)
        });
        self.cor_lob_code_input.update(cx, |input, cx| {
            input.set_value(
                version.cor.line_of_business_code.unwrap_or_default(),
                window,
                cx,
            )
        });
        self.cor_lob_description_input.update(cx, |input, cx| {
            input.set_value(version.cor.line_of_business_description, window, cx)
        });
        self.cor_taxpayer_type_select.update(cx, |state, cx| {
            state.set_selected_value(
                Self::taxpayer_type_label(&version.taxpayer_type),
                window,
                cx,
            )
        });
        self.cor_tax_classification_select.update(cx, |state, cx| {
            state.set_selected_value(
                version
                    .tax_classification
                    .as_ref()
                    .map(Self::tax_classification_label)
                    .unwrap_or("None"),
                window,
                cx,
            )
        });
        self.cor_eopt_tier_select.update(cx, |state, cx| {
            state.set_selected_value(
                version
                    .eopt_tier
                    .as_ref()
                    .map(Self::eopt_tier_label)
                    .unwrap_or("None"),
                window,
                cx,
            )
        });
        self.cor_registration_status_select.update(cx, |state, cx| {
            state.set_selected_value(
                Self::registration_status_label(&version.registration_activity_status),
                window,
                cx,
            )
        });
        Ok(())
    }

    fn apply_cor_version_editor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(version_id) = self.cor_editing_version_id.clone() else {
            return Err("Select a COR version to edit first.".to_string());
        };
        let effective_from = Self::parse_optional_cor_date(
            self.cor_effective_from_input.read(cx).value().trim(),
            "effective from",
        )?;
        let effective_until = Self::parse_optional_cor_date(
            self.cor_effective_until_input.read(cx).value().trim(),
            "effective until",
        )?;
        if let (Some(from), Some(until)) = (effective_from, effective_until)
            && until < from
        {
            return Err("Effective until cannot be before effective from.".to_string());
        }
        let registration_date = Self::parse_optional_cor_date(
            self.cor_registration_date_input.read(cx).value().trim(),
            "registration date",
        )?;

        let Some(version) = self
            .stored_profile_versions
            .iter_mut()
            .find(|version| version.id == version_id)
        else {
            return Err("COR version was not found.".to_string());
        };

        let label = self
            .cor_version_label_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        if label.is_empty() {
            return Err("COR version label is required.".to_string());
        }

        version.label = label;
        version.effective_from = effective_from;
        version.effective_until = effective_until;
        version.needs_effective_date_review = version.effective_from.is_none();
        version.cor.registration_date = registration_date;
        version.cor.registered_name = self
            .cor_registered_name_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        version.cor.trade_name = {
            let value = self
                .cor_trade_name_input
                .read(cx)
                .value()
                .trim()
                .to_string();
            if value.is_empty() { None } else { Some(value) }
        };
        version.cor.rdo_code = self.cor_rdo_code_input.read(cx).value().trim().to_string();
        version.cor.registered_address = self
            .cor_registered_address_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        version.cor.line_of_business_code = {
            let value = self.cor_lob_code_input.read(cx).value().trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        };
        version.cor.line_of_business_description = self
            .cor_lob_description_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        version.taxpayer_type = Self::taxpayer_type_from_label(
            &self.cor_taxpayer_type_select.read(cx).selected_value(cx),
        );
        version.tax_classification = Self::tax_classification_from_label(
            &self
                .cor_tax_classification_select
                .read(cx)
                .selected_value(cx),
        );
        version.eopt_tier =
            Self::eopt_tier_from_label(&self.cor_eopt_tier_select.read(cx).selected_value(cx));
        version.registration_activity_status = Self::registration_status_from_label(
            &self
                .cor_registration_status_select
                .read(cx)
                .selected_value(cx),
        );

        self.save_message =
            Some("COR version details updated. Save the profile to persist them.".to_string());
        self.load_cor_version_editor(&version_id, window, cx)?;
        Ok(())
    }

    fn toggle_cor_registered_tax_type(&mut self, version_id: &str, tax_type: RegisteredTaxType) {
        let Some(version) = self
            .stored_profile_versions
            .iter_mut()
            .find(|version| version.id == version_id)
        else {
            self.save_message = Some("COR version was not found.".to_string());
            return;
        };

        if let Some(index) = version
            .registered_tax_types
            .iter()
            .position(|existing| existing == &tax_type)
        {
            version.registered_tax_types.remove(index);
        } else {
            version.registered_tax_types.push(tax_type);
            version.registered_tax_types.sort();
        }
        Self::sync_version_flags_from_registered_tax_types(version);
        self.save_message =
            Some("Registered tax type updated. Save the profile to persist it.".to_string());
    }

    fn sync_version_flags_from_registered_tax_types(
        version: &mut bir_core::profile::TaxProfileVersion,
    ) {
        version.is_vat_registered = version
            .registered_tax_types
            .contains(&RegisteredTaxType::ValueAddedTax);
        version.withholds_compensation = version
            .registered_tax_types
            .contains(&RegisteredTaxType::WithholdingCompensation);
        version.withholds_expanded = version
            .registered_tax_types
            .contains(&RegisteredTaxType::WithholdingExpanded);
        version.withholds_final = version
            .registered_tax_types
            .contains(&RegisteredTaxType::WithholdingFinal);
    }

    fn parse_optional_cor_date(
        value: &str,
        label: &str,
    ) -> Result<Option<chrono::NaiveDate>, String> {
        if value.is_empty() {
            return Ok(None);
        }
        chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| format!("COR {label} must use YYYY-MM-DD."))
    }

    fn taxpayer_type_label(taxpayer_type: &TaxpayerType) -> &'static str {
        match taxpayer_type {
            TaxpayerType::Individual => "Individual",
            TaxpayerType::Corporation => "Corporation",
            TaxpayerType::Partnership => "Partnership",
            TaxpayerType::Cooperative => "Cooperative",
            TaxpayerType::Estate => "Estate",
            TaxpayerType::Trust => "Trust",
        }
    }

    fn taxpayer_type_from_label(label: &str) -> TaxpayerType {
        match label {
            "Corporation" => TaxpayerType::Corporation,
            "Partnership" => TaxpayerType::Partnership,
            "Cooperative" => TaxpayerType::Cooperative,
            "Estate" => TaxpayerType::Estate,
            "Trust" => TaxpayerType::Trust,
            _ => TaxpayerType::Individual,
        }
    }

    fn tax_classification_label(classification: &TaxClassification) -> &'static str {
        match classification {
            TaxClassification::PurelyCompensation => "Purely Compensation",
            TaxClassification::SelfEmployed => "Self-Employed / Professional",
            TaxClassification::Corporation => "Corporation",
            TaxClassification::CooperativeExempt => "Cooperative Exempt",
            TaxClassification::CooperativeTaxable => "Cooperative Taxable",
            TaxClassification::CooperativeMixed => "Cooperative Mixed",
            TaxClassification::EstateOrTrust => "Estate or Trust",
            TaxClassification::MixedIncome => "Mixed Income",
        }
    }

    fn tax_classification_from_label(label: &str) -> Option<TaxClassification> {
        match label {
            "Purely Compensation" => Some(TaxClassification::PurelyCompensation),
            "Self-Employed / Professional" => Some(TaxClassification::SelfEmployed),
            "Corporation" => Some(TaxClassification::Corporation),
            "Cooperative Exempt" => Some(TaxClassification::CooperativeExempt),
            "Cooperative Taxable" => Some(TaxClassification::CooperativeTaxable),
            "Cooperative Mixed" => Some(TaxClassification::CooperativeMixed),
            "Estate or Trust" => Some(TaxClassification::EstateOrTrust),
            "Mixed Income" => Some(TaxClassification::MixedIncome),
            _ => None,
        }
    }

    fn eopt_tier_label(tier: &EoptTier) -> &'static str {
        match tier {
            EoptTier::Micro => "Micro",
            EoptTier::Small => "Small",
            EoptTier::Medium => "Medium",
            EoptTier::Large => "Large",
        }
    }

    fn eopt_tier_from_label(label: &str) -> Option<EoptTier> {
        match label {
            "Micro" => Some(EoptTier::Micro),
            "Small" => Some(EoptTier::Small),
            "Medium" => Some(EoptTier::Medium),
            "Large" => Some(EoptTier::Large),
            _ => None,
        }
    }

    fn registration_status_label(status: &RegistrationActivityStatus) -> &'static str {
        match status {
            RegistrationActivityStatus::Active => "Active",
            RegistrationActivityStatus::DormantOperational => "Dormant Operational",
            RegistrationActivityStatus::TemporarilyInactive => "Temporarily Inactive",
            RegistrationActivityStatus::OfficiallyClosed => "Officially Closed",
        }
    }

    fn registration_status_from_label(label: &str) -> RegistrationActivityStatus {
        match label {
            "Dormant Operational" => RegistrationActivityStatus::DormantOperational,
            "Temporarily Inactive" => RegistrationActivityStatus::TemporarilyInactive,
            "Officially Closed" => RegistrationActivityStatus::OfficiallyClosed,
            _ => RegistrationActivityStatus::Active,
        }
    }

    fn add_cor_obligation_override(
        &mut self,
        action: bir_core::profile::ManualObligationOverrideAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let form_code = self
            .cor_obligation_form_input
            .read(cx)
            .value()
            .trim()
            .to_ascii_uppercase()
            .replace(' ', "");
        let reason = self
            .cor_obligation_reason_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let source_reference = self
            .cor_obligation_source_input
            .read(cx)
            .value()
            .trim()
            .to_string();

        if form_code.is_empty() {
            return Err("Set a form code for the obligation override.".to_string());
        }
        if reason.is_empty() || source_reference.is_empty() {
            return Err("Obligation overrides require both a reason and source.".to_string());
        }

        let Some(version) = self
            .stored_profile_versions
            .iter_mut()
            .rev()
            .find(|version| version.status != bir_core::profile::TaxProfileVersionStatus::Archived)
        else {
            return Err("Create a COR/manual version before adding profile overrides.".to_string());
        };

        version
            .obligation_overrides
            .retain(|override_rule| override_rule.form_code.to_ascii_uppercase() != form_code);
        version
            .obligation_overrides
            .push(bir_core::profile::ManualObligationOverride {
                form_code: form_code.clone(),
                action,
                reason,
                source_reference: Some(source_reference),
            });
        self.clear_cor_override_inputs(window, cx);
        self.save_message = Some(format!(
            "Profile obligation override for {form_code} added. Save the profile to persist it."
        ));
        Ok(())
    }

    fn remove_cor_obligation_override(&mut self, version_id: &str, index: usize) {
        let Some(version) = self
            .stored_profile_versions
            .iter_mut()
            .find(|version| version.id == version_id)
        else {
            self.save_message = Some("COR version was not found.".to_string());
            return;
        };
        if index >= version.obligation_overrides.len() {
            self.save_message = Some("Profile obligation override was not found.".to_string());
            return;
        }
        version.obligation_overrides.remove(index);
        self.save_message = Some(
            "Profile obligation override removed. Save the profile to persist it.".to_string(),
        );
    }

    fn add_cor_deadline_override(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let title = self
            .cor_deadline_title_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let source_reference = self
            .cor_deadline_source_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let form_codes = self
            .cor_deadline_forms_input
            .read(cx)
            .value()
            .split(',')
            .map(|code| code.trim().to_ascii_uppercase().replace(' ', ""))
            .filter(|code| !code.is_empty())
            .collect::<Vec<_>>();
        let original_deadline = chrono::NaiveDate::parse_from_str(
            self.cor_deadline_original_input.read(cx).value().trim(),
            "%Y-%m-%d",
        )
        .map_err(|_| "Original deadline must use YYYY-MM-DD.".to_string())?;
        let adjusted_deadline = chrono::NaiveDate::parse_from_str(
            self.cor_deadline_adjusted_input.read(cx).value().trim(),
            "%Y-%m-%d",
        )
        .map_err(|_| "Adjusted deadline must use YYYY-MM-DD.".to_string())?;
        let reason = self
            .cor_deadline_reason_input
            .read(cx)
            .value()
            .trim()
            .to_string();

        if title.is_empty() || source_reference.is_empty() || form_codes.is_empty() {
            return Err(
                "Deadline overrides require a title, source, and at least one form code."
                    .to_string(),
            );
        }

        let Some(version) = self
            .stored_profile_versions
            .iter_mut()
            .rev()
            .find(|version| version.status != bir_core::profile::TaxProfileVersionStatus::Archived)
        else {
            return Err("Create a COR/manual version before adding profile overrides.".to_string());
        };

        version
            .deadline_overrides
            .push(bir_core::profile::ProfileDeadlineOverride {
                id: format!(
                    "profile-deadline-{}",
                    chrono::Local::now().timestamp_millis()
                ),
                title: title.clone(),
                source_reference,
                affected_form_codes: form_codes,
                original_deadline,
                adjusted_deadline,
                reason: if reason.is_empty() {
                    None
                } else {
                    Some(reason)
                },
            });
        self.clear_cor_override_inputs(window, cx);
        self.save_message = Some(format!(
            "Profile deadline override '{title}' added. Save the profile to persist it."
        ));
        Ok(())
    }

    fn remove_cor_deadline_override(&mut self, version_id: &str, index: usize) {
        let Some(version) = self
            .stored_profile_versions
            .iter_mut()
            .find(|version| version.id == version_id)
        else {
            self.save_message = Some("COR version was not found.".to_string());
            return;
        };
        if index >= version.deadline_overrides.len() {
            self.save_message = Some("Profile deadline override was not found.".to_string());
            return;
        }
        version.deadline_overrides.remove(index);
        self.save_message =
            Some("Profile deadline override removed. Save the profile to persist it.".to_string());
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

        // We no longer force users to authenticate an email.
        // If they opt out, they won't get automated tracking updates but can still submit.

        let db_arc = self.db.clone();

        // Immediately update stored password so we don't lose it
        if !typed_pw.is_empty() {
            self.stored_imap_app_password = Some(typed_pw);
            self.is_editing_password = false;
        }

        self.save_message = Some("Saving...".to_string());
        cx.notify();

        let db_arc_clone = db_arc.clone();
        cx.spawn(async move |this, cx| {
            let is_email_tracking_active = profile.is_email_tracking_active();
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
                        cx.emit(ProfileEvent::Saved(tin_val.clone()));

                        // Retroactively schedule email polling for any pending submissions
                        if is_email_tracking_active {
                            if let Ok(db) = db_arc_clone.lock() {
                                if let Ok(summaries) = db.list_all_queued_submissions() {
                                    for sum in summaries {
                                        if sum.tin == tin_val
                                            && sum.status
                                                == bir_core::forms::FilingStatus::Submitted
                                        {
                                            if let Ok(Some(saved_profile)) =
                                                db.get_profile(&tin_val)
                                            {
                                                bir_core::background_cron::schedule_email_poll(
                                                    &saved_profile,
                                                    &sum.form_code,
                                                    &db,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
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

    fn field_label(text: &str, cx: &Context<Self>) -> Div {
        div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .mb_1()
            .child(text.to_string())
    }

    fn field_error(&self, field: &'static str, _cx: &Context<Self>) -> gpui::Div {
        let text = self
            .errors
            .iter()
            .find(|err| err.field == field)
            .map(|err| err.message.clone())
            .unwrap_or_default();
        div()
            .min_h_5()
            .text_xs()
            .text_color(gpui::rgba(0xff6b6bff))
            .child(text)
    }

    fn validate_field(&mut self, field: &'static str, value: &str) {
        self.errors.retain(|e| e.field != field);
        if value.trim().is_empty() {
            let label = match field {
                "line_of_business" => "Line of business",
                "full_name" => "Taxpayer name",
                "registered_address" => "Registered address",
                "email" => "Email",
                "zip_code" => "ZIP code",
                "rdo_code" => "RDO",
                "phone" => "Phone number",
                _ => "This field",
            };
            self.errors
                .push(ValidationError::new(field, format!("{label} is required")));
        } else {
            if field == "email" && !bir_core::validation::validate_email(value) {
                self.errors
                    .push(ValidationError::new(field, "Email address is invalid"));
            } else if field == "zip_code" && !bir_core::validation::validate_zip(value.trim()) {
                self.errors
                    .push(ValidationError::new(field, "ZIP code must be 4 digits"));
            } else if field == "phone" && !bir_core::validation::validate_ph_phone(value) {
                self.errors.push(ValidationError::new(
                    field,
                    "Phone must be a valid Philippine mobile or landline number",
                ));
            }
        }
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
        let is_cooperative = type_val == "Cooperative";

        let tax_class_val = self.tax_classification_select.read(cx).selected_value(cx);
        let is_eligible_for_election = is_individual
            && !self.is_vat_registered
            && matches!(
                tax_class_val.as_str(),
                "Self-Employed / Professional" | "Mixed Income"
            );

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
            .size_full()
            .relative()
            // Intercept Cmd+V / Ctrl+V for OTP paste support
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let mods = &event.keystroke.modifiers;
                let is_paste = event.keystroke.key.as_str() == "v"
                    && (mods.platform || mods.control);
                if !is_paste { return; }
                if this.show_totp_setup {
                    paste_otp_value(&this.setup_totp_state, 6, window, cx);
                } else if this.enable_profile_pin {
                    paste_otp_value(&this.profile_pin_input, 4, window, cx);
                }
            }))
            .child(
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
                                    .child(self.render_tax_profile_tab(
                                        is_individual,
                                        is_cooperative,
                                        is_eligible_for_election,
                                        date_label,
                                        cx,
                                    ))
                                    .child(self.render_email_settings_tab(cx))
                            )
                            .child(self.render_security_tab(global_pins_enabled, cx))
                            .child(self.render_export_tab(cx))

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
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
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
                                                                .border_color(cx.theme().border)
                                                                .child(div().text_sm().font_family(".SF NS Mono").child(secret.clone()))
                                                                .child(
                                                                    gpui_component::clipboard::Clipboard::new("totp-secret-clipboard-profile")
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
                                            gpui_component::button::Button::new("cancel_totp_profile_btn")
                                                .label("Cancel")
                                                .small()
                                                .on_click(cx.listener(|this, _ev, window, cx| {
                                                    this.show_totp_setup = false;
                                                    this.show_totp_secret_text = false;
                                                    this.totp_secret_temp = None;
                                                    this.totp_qr_path = None;
                                                    this.setup_totp_state.update(cx, |s, cx| s.set_value("", window, cx));
                                                    this.is_totp_enabled = false;
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

fn taxpayer_type_label(taxpayer_type: &TaxpayerType) -> &'static str {
    match taxpayer_type {
        TaxpayerType::Individual => "Individual",
        TaxpayerType::Corporation => "Corporation",
        TaxpayerType::Partnership => "Partnership",
        TaxpayerType::Cooperative => "Cooperative",
        TaxpayerType::Estate => "Estate",
        TaxpayerType::Trust => "Trust",
    }
}
