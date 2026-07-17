use chrono::Datelike;
use gpui::prelude::*;
use gpui::*;
use gpui_component::WindowExt;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState, OtpInput, OtpState};
use gpui_component::notification::{Notification, NotificationType};
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
    TaxClassification, TaxProfileVersionConfirmationPlan, TaxpayerProfile, TaxpayerType,
};
use bir_core::reference::get_all_rdos;
use bir_core::validation::{ValidationError, validate_profile};

// ─── Tab sub-modules ──────────────────────────────────────────────────────────
// Each file holds one `impl ProfileManagerView` block rendering that tab's UI.
// Imports from this module are re-exported via `use super::*` in each sub-module.
mod tab_calendar;
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
    birth_date_input: Entity<DateInputState>,
    is_vat_registered: bool,
    editing_id: Option<i64>,
    tin_duplicate_error: Option<String>,
    errors: Vec<ValidationError>,
    save_message: Option<String>,
    pending_notification: Option<(gpui_component::notification::NotificationType, String)>,
    has_unsaved_profile_changes: bool,
    profile_change_revision: u64,
    persisted_profile_tin: Option<String>,
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
    stored_atc_codes: Vec<String>,
    stored_tax_elections: Vec<bir_core::profile::TaxElectionHistory>,
    stored_profile_versions: Vec<bir_core::profile::TaxProfileVersion>,
    pending_cor_evidence_cleanup: Vec<(u64, bir_core::profile::CorDocumentRef)>,
    compliance_source_mode: ComplianceSourceMode,
    cor_sub_tab: usize,
    ocr_selected_version_id: Option<String>,
    cor_editing_version_id: Option<String>,
    cor_preview_year_input: Entity<InputState>,
    cor_version_label_input: Entity<InputState>,
    cor_effective_from_input: Entity<InputState>,
    cor_effective_until_input: Entity<InputState>,
    cor_tin_input: Entity<TinInput>,
    cor_registration_date_input: Entity<InputState>,
    cor_registered_name_input: Entity<InputState>,
    cor_trade_name_input: Entity<InputState>,
    cor_rdo_code_input: Entity<InputState>,
    cor_rdo_select: Entity<ComboboxState>,
    cor_registered_address_input: Entity<InputState>,
    cor_lob_code_input: Entity<InputState>,
    cor_lob_description_input: Entity<InputState>,
    cor_taxpayer_type_select: Entity<ComboboxState>,
    cor_tax_classification_select: Entity<ComboboxState>,
    cor_eopt_tier_select: Entity<ComboboxState>,
    cor_registration_status_select: Entity<ComboboxState>,
    cor_extracted_forms: Vec<String>,
    cor_extracted_forms_select: Entity<MultiSelectState>,
    cor_deadline_title_input: Entity<InputState>,
    cor_deadline_source_input: Entity<InputState>,
    cor_deadline_forms_input: Entity<InputState>,
    cor_deadline_original_input: Entity<InputState>,
    cor_deadline_adjusted_input: Entity<InputState>,
    cor_deadline_reason_input: Entity<InputState>,
    gemini_ocr_enabled: bool,
    gemini_ocr_cloud_consent: bool,
    gemini_ocr_api_key_input: Entity<InputState>,
    gemini_ocr_model_select: Entity<ComboboxState>,
    gemini_ocr_custom_model_input: Entity<InputState>,
    gemini_ocr_status: Option<String>,

    enable_profile_pin: bool,
    profile_pin_input: Entity<OtpState>,

    is_totp_enabled: bool,
    show_totp_setup: bool,
    setup_totp_state: Entity<OtpState>,
    totp_secret_temp: Option<String>,
    totp_qr_path: Option<std::path::PathBuf>,
    show_totp_secret_text: bool,
    stored_totp_secret: Option<String>,
    interactive_document_viewer:
        Option<Entity<crate::components::document_viewer::InteractiveDocumentViewer>>,
    focused_ocr_field: Option<String>,
    pending_cor_editor_load: Option<String>,
    pending_profile_version_confirmation: Option<TaxProfileVersionConfirmationPlan>,
    is_uploading_cor: bool,

    pub stored_per_year_forms: std::collections::BTreeMap<u16, bir_core::forms::PerYearFormsSet>,
    pub forms_editor_year: u16,
    pub forms_editor_year_select: Entity<ComboboxState>,
    pub forms_editor_new_code_input: Entity<InputState>,
    pub forms_editor_new_reason_input: Entity<InputState>,
    pub forms_editor_new_frequency_select: Entity<ComboboxState>,
    pub forms_editor_selected_code: Option<String>,
    pub forms_editor_active_note_input: Entity<InputState>,
    calendar_name_input: Entity<InputState>,
    calendar_action_message: Option<(bool, String)>,

    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ProfileEvent> for ProfileManagerView {}

fn compliance_affected_years(profile: &TaxpayerProfile) -> Vec<u16> {
    let current_year = chrono::Local::now().year().clamp(0, i32::from(u16::MAX)) as u16;
    let mut years = std::collections::BTreeSet::from([current_year]);
    years.extend(profile.per_year_forms.keys().copied());
    years.extend(
        profile
            .tax_elections
            .iter()
            .map(|election| election.taxable_year),
    );

    for version in &profile.profile_versions {
        let Some(start) = version.effective_from else {
            continue;
        };
        let start_year = start.year().clamp(0, i32::from(u16::MAX)) as u16;
        let raw_end_year = version
            .effective_until
            .map(|date| date.year())
            .unwrap_or_else(|| i32::from(current_year).max(start.year()));
        let end_year = raw_end_year.clamp(i32::from(start_year), i32::from(u16::MAX)) as u16;
        years.extend(start_year..=end_year);
    }

    years.into_iter().collect()
}

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
        let calendar_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Google calendar name"));
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
        let birth_date_input = cx.new(|cx| DateInputState::new(window, cx));
        let cor_preview_year_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Preview year");
            input.set_value(current_year.to_string(), window, cx);
            input
        });
        let cor_version_label_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Version label"));
        let cor_effective_from_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Effective from YYYY-MM-DD"));
        let cor_effective_until_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Effective until YYYY-MM-DD"));
        let cor_tin_input = cx.new(|cx| TinInput::new(window, cx));
        let cor_rdo_select = cx.new(|cx| ComboboxState::new(rdo_options.clone(), 5, window, cx));
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
        let cor_extracted_forms_select = cx.new(|cx| {
            let mut options: Vec<_> = bir_core::forms::registry::FORM_REGISTRY
                .iter()
                .map(|f| MultiSelectOption::new(f.code, format!("{} - {}", f.code, f.title)))
                .collect();
            options.sort_by(|a, b| a.id.cmp(&b.id));
            MultiSelectState::new(options, window, cx)
                .placeholder("Add form code...")
                .hide_trigger_chips(true)
        });
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
        let (gemini_ocr_enabled, gemini_ocr_model) = if let Ok(db_guard) = db.lock() {
            let enabled = db_guard
                .get_setting(crate::cor_ocr::COR_OCR_GEMINI_ENABLED_SETTING)
                .ok()
                .flatten()
                .as_deref()
                == Some("true");
            let model = db_guard
                .get_setting(crate::cor_ocr::COR_OCR_GEMINI_MODEL_SETTING)
                .ok()
                .flatten()
                .unwrap_or_else(|| crate::cor_ocr::DEFAULT_GEMINI_MODEL.to_string());
            (enabled, model)
        } else {
            (false, crate::cor_ocr::DEFAULT_GEMINI_MODEL.to_string())
        };
        let gemini_key_placeholder = "Paste Gemini API key (stored in OS keychain)";
        let gemini_ocr_api_key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder(gemini_key_placeholder)
        });
        let gemini_model_is_supported =
            crate::cor_ocr::SUPPORTED_GEMINI_MODELS.contains(&gemini_ocr_model.as_str());
        let gemini_ocr_model_select = cx.new(|cx| {
            let mut state = ComboboxState::new(
                crate::cor_ocr::SUPPORTED_GEMINI_MODELS
                    .iter()
                    .map(|model| (*model).to_string())
                    .collect(),
                6,
                window,
                cx,
            );
            state.set_selected_value(
                if gemini_model_is_supported {
                    &gemini_ocr_model
                } else {
                    crate::cor_ocr::DEFAULT_GEMINI_MODEL
                },
                window,
                cx,
            );
            state
        });
        let gemini_ocr_custom_model_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Custom Gemini model id");
            if !gemini_model_is_supported {
                input.set_value(gemini_ocr_model.clone(), window, cx);
            }
            input
        });

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

        let forms_editor_year = current_year as u16;
        let forms_editor_year_select = cx.new(|cx| {
            let years = (2018..=2026).map(|y| y.to_string()).collect::<Vec<_>>();
            let mut state = ComboboxState::new(years, 5, window, cx);
            state.set_selected_value(&current_year.to_string(), window, cx);
            state
        });
        let forms_editor_new_code_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Form code"));
        let forms_editor_new_reason_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Reason / note"));
        let forms_editor_new_frequency_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Monthly".to_string(),
                    "Quarterly".to_string(),
                    "Annual".to_string(),
                    "Open Ended / Event".to_string(),
                ],
                4,
                window,
                cx,
            )
        });
        let forms_editor_active_note_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Edit note / reason"));

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
            cx.subscribe(&birth_date_input, Self::on_date_event),
            cx.subscribe(
                &registration_activity_status_select,
                Self::on_combobox_event,
            ),
            cx.subscribe_in(&cor_preview_year_input, window, Self::on_input_event),
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

        cx.subscribe(
            &cor_extracted_forms_select,
            |this: &mut Self, _, event: &MultiSelectEvent, cx| {
                this.cor_extracted_forms = event.selected.clone();
                // We do not reset the input, MultiSelect handles its own state
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(
            &forms_editor_year_select,
            |this: &mut Self, _, event: &ComboboxEvent, cx| {
                if let Some(val) = event.selected.as_ref() {
                    if let Ok(year) = val.parse::<u16>() {
                        this.forms_editor_year = year;
                        this.forms_editor_selected_code = None; // Reset detail view
                        cx.notify();
                    }
                }
            },
        )
        .detach();

        cx.subscribe_in(
            &forms_editor_active_note_input,
            window,
            |this: &mut Self, _entity, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    let val = this
                        .forms_editor_active_note_input
                        .read(cx)
                        .value()
                        .to_string();
                    if let Some(code) = &this.forms_editor_selected_code {
                        let year = this.forms_editor_year;
                        if let Some(set) = this.stored_per_year_forms.get_mut(&year) {
                            if let Some(entry) =
                                set.entries.iter_mut().find(|e| e.form_code == *code)
                            {
                                let next_reason = if val.trim().is_empty() {
                                    None
                                } else {
                                    Some(val)
                                };
                                if entry.reason != next_reason {
                                    entry.reason = next_reason;
                                    this.mark_profile_changed();
                                }
                            }
                        }
                    }
                }
            },
        )
        .detach();

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
            birth_date_input,
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
            stored_atc_codes: vec![],
            stored_tax_elections: vec![],
            stored_profile_versions: vec![],
            pending_cor_evidence_cleanup: vec![],
            compliance_source_mode: ComplianceSourceMode::TemporalSuggestion,
            cor_sub_tab: 0,
            ocr_selected_version_id: None,
            cor_editing_version_id: None,
            cor_preview_year_input,
            cor_version_label_input,
            cor_effective_from_input,
            cor_effective_until_input,
            cor_tin_input,
            cor_registration_date_input,
            cor_registered_name_input,
            cor_trade_name_input,
            cor_rdo_code_input,
            cor_rdo_select,
            cor_registered_address_input,
            cor_lob_code_input,
            cor_lob_description_input,
            cor_taxpayer_type_select,
            cor_tax_classification_select,
            cor_eopt_tier_select,
            cor_registration_status_select,
            cor_extracted_forms: Vec::new(),
            cor_extracted_forms_select,
            cor_deadline_title_input,
            cor_deadline_source_input,
            cor_deadline_forms_input,
            cor_deadline_original_input,
            cor_deadline_adjusted_input,
            cor_deadline_reason_input,
            gemini_ocr_enabled,
            gemini_ocr_cloud_consent: false,
            gemini_ocr_api_key_input,
            gemini_ocr_model_select,
            gemini_ocr_custom_model_input,
            gemini_ocr_status: None,
            enable_profile_pin: false,
            profile_pin_input,
            is_totp_enabled: false,
            show_totp_setup: false,
            setup_totp_state,
            totp_secret_temp: None,
            totp_qr_path: None,
            show_totp_secret_text: false,
            stored_totp_secret: None,
            interactive_document_viewer: None,
            focused_ocr_field: None,
            pending_cor_editor_load: None,
            pending_profile_version_confirmation: None,
            is_uploading_cor: false,
            pending_notification: None,
            has_unsaved_profile_changes: false,
            profile_change_revision: 0,
            persisted_profile_tin: None,
            stored_per_year_forms: std::collections::BTreeMap::new(),
            forms_editor_year,
            forms_editor_year_select,
            forms_editor_new_code_input,
            forms_editor_new_reason_input,
            forms_editor_new_frequency_select,
            forms_editor_selected_code: None,
            forms_editor_active_note_input,
            calendar_name_input,
            calendar_action_message: None,
            _subscriptions: subscriptions,
        }
    }

    /// Whether the in-memory profile, compliance ledger, or yearly Forms Set
    /// contains changes that have not completed a database save.
    pub fn has_unsaved_compliance_changes(&self) -> bool {
        self.has_unsaved_profile_changes
    }

    /// Shows the reason cross-view navigation was refused. App-level callers
    /// can use this together with [`Self::has_unsaved_compliance_changes`]
    /// before opening a filing form or replacing the edited profile.
    pub fn notify_unsaved_compliance_blocked(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.push_notification(
            Notification::error("Navigation blocked")
                .message(
                    "Save or discard the pending profile and Forms Set changes before opening another profile or filing form.",
                )
                .title("Unsaved profile changes"),
            cx,
        );
        cx.notify();
    }

    fn mark_profile_changed(&mut self) {
        self.has_unsaved_profile_changes = true;
        self.profile_change_revision = self.profile_change_revision.wrapping_add(1);
    }

    fn clear_profile_changed(&mut self) {
        self.has_unsaved_profile_changes = false;
    }

    fn discard_profile_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let persisted_profile = self.persisted_profile_tin.as_deref().and_then(|tin| {
            self.db
                .lock()
                .ok()
                .and_then(|db| db.get_profile(tin).ok().flatten())
        });

        if let Some(profile) = persisted_profile {
            self.edit_profile(profile, window, cx);
            self.pending_notification = Some((
                NotificationType::Success,
                "Unsaved profile and Forms Set changes were discarded.".to_string(),
            ));
        } else if self.editing_id.is_none() {
            self.reset_for_new(window, cx);
            self.pending_notification = Some((
                NotificationType::Success,
                "Unsaved new profile was cleared.".to_string(),
            ));
        } else {
            self.pending_notification = Some((
                NotificationType::Error,
                "The saved profile could not be reloaded; no in-memory changes were discarded."
                    .to_string(),
            ));
        }
        cx.notify();
    }

    pub fn sync_document_viewer(&mut self, cx: &mut Context<Self>) {
        if let Some(version_id) = &self.ocr_selected_version_id {
            tracing::info!("[COR Viewer] Syncing viewer for version_id={version_id}");
            if let Some(version) = self
                .stored_profile_versions
                .iter()
                .find(|v| v.id == *version_id)
            {
                if let Some(evidence) = version.evidence.first() {
                    let path = evidence.stored_path.clone();
                    tracing::info!("[COR Viewer] Creating viewer for path={path}");
                    let bboxes = evidence.field_bboxes.clone();
                    self.interactive_document_viewer = Some(cx.new(|cx| {
                        crate::components::document_viewer::InteractiveDocumentViewer::new(
                            path, bboxes, cx,
                        )
                    }));
                    return;
                } else {
                    tracing::info!("[COR Viewer] Version has no evidence documents");
                }
            } else {
                tracing::info!(
                    "[COR Viewer] Version {version_id} not found in stored_profile_versions (count={})",
                    self.stored_profile_versions.len()
                );
            }
        } else {
            tracing::info!("[COR Viewer] No ocr_selected_version_id set");
        }
        self.interactive_document_viewer = None;
    }

    pub fn focus_ocr_field(&mut self, field_id: &str, cx: &mut Context<Self>) {
        if let Some(viewer) = &self.interactive_document_viewer {
            viewer.update(cx, |viewer, cx| {
                viewer.set_active_field(Some(field_id.to_string()), cx);
            });
        }
    }

    pub fn reset_for_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_id = None;
        self.persisted_profile_tin = None;
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
        self.stored_atc_codes.clear();
        self.stored_tax_elections = vec![];
        self.stored_profile_versions = vec![];
        self.pending_cor_evidence_cleanup.clear();
        self.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;
        self.ocr_selected_version_id = None;
        self.cor_editing_version_id = None;
        self.gemini_ocr_cloud_consent = false;
        self.gemini_ocr_status = None;
        let current_year = chrono::Local::now().date_naive().year();
        self.cor_preview_year_input.update(cx, |input, cx| {
            input.set_value(current_year.to_string(), window, cx)
        });
        self.clear_cor_version_editor(window, cx);
        self.clear_cor_override_inputs(window, cx);
        self.is_totp_enabled = false;
        self.show_totp_setup = false;
        self.show_totp_secret_text = false;
        self.totp_secret_temp = None;
        self.totp_qr_path = None;
        self.stored_totp_secret = None;
        self.interactive_document_viewer = None;
        self.focused_ocr_field = None;
        self.stored_per_year_forms.clear();
        self.forms_editor_selected_code = None;
        let current_year = chrono::Local::now().date_naive().year();
        self.forms_editor_year = current_year as u16;
        self.forms_editor_year_select.update(cx, |select, cx| {
            select.set_selected_value(&current_year.to_string(), window, cx);
        });
        self.forms_editor_new_code_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.forms_editor_new_reason_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.forms_editor_new_frequency_select
            .update(cx, |select, cx| {
                select.set_selected_value("", window, cx);
            });
        self.forms_editor_active_note_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.calendar_name_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.calendar_action_message = None;
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
        self.profile_change_revision = 0;
        self.clear_profile_changed();
        cx.notify();
    }

    pub fn prefill_name(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.name_input.update(cx, |input, cx| {
            input.set_value(name.to_string(), window, cx);
        });
        self.mark_profile_changed();
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
        self.mark_profile_changed();
        cx.notify();
    }

    pub fn edit_profile(
        &mut self,
        profile: TaxpayerProfile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.persisted_profile_tin = Some(profile.tin.full());
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
        self.stored_atc_codes = profile.atc_codes.clone();
        self.stored_tax_elections = profile.tax_elections.clone();
        self.stored_profile_versions = profile.profile_versions.clone();
        self.pending_cor_evidence_cleanup.clear();
        self.compliance_source_mode =
            Self::derive_compliance_source_mode(&self.stored_profile_versions);
        tracing::info!(
            "[Profile Load] Loaded {} COR versions for profile",
            self.stored_profile_versions.len()
        );
        self.gemini_ocr_cloud_consent = false;
        self.gemini_ocr_status = None;
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
        self.focused_ocr_field = None;
        // Reset OCR selection — user must click Review from timeline.
        // Don't auto-select a version_id here because the viewer needs
        // to be created AFTER all other state is set.
        self.ocr_selected_version_id = None;
        self.interactive_document_viewer = None;
        tracing::info!(
            "[Profile Load] OCR viewer reset. User must click Review to enter detail view."
        );

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
        self.birth_date_input.update(cx, |input, cx| {
            input.set_date(profile.birth_date, window, cx)
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

        self.stored_per_year_forms = profile.per_year_forms.clone();
        let calendar_name = if let Ok(db) = self.db.lock() {
            db.get_profile_calendar_link(&profile.tin.full())
                .ok()
                .flatten()
                .map(|link| link.calendar_name)
                .unwrap_or_else(|| {
                    bir_core::google_calendar::default_profile_calendar_name(&profile)
                })
        } else {
            bir_core::google_calendar::default_profile_calendar_name(&profile)
        };
        self.calendar_name_input
            .update(cx, |input, cx| input.set_value(calendar_name, window, cx));
        self.calendar_action_message = None;
        let select_val = self.forms_editor_year_select.read(cx).selected_value(cx);
        if let Ok(y) = select_val.parse::<u16>() {
            self.forms_editor_year = y;
        }
        self.forms_editor_selected_code = None;

        self.profile_change_revision = 0;
        self.clear_profile_changed();
        cx.notify();
    }

    /// Syncs the projected TaxpayerProfile fields back to the UI inputs without resetting
    /// secondary state like OCR viewer, passwords, or tab selections. Used after "Commit to Profile".
    pub fn sync_projection_to_ui(
        &mut self,
        profile: &TaxpayerProfile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_vat_registered = profile.is_vat_registered;
        self.withholds_compensation = profile.withholds_compensation;
        self.withholds_expanded = profile.withholds_expanded;
        self.withholds_final = profile.withholds_final;
        self.is_top_withholding_agent = profile.is_top_withholding_agent;
        self.is_government_withholding_entity = profile.is_government_withholding_entity;
        self.has_single_employer = profile.has_single_employer;
        self.is_gpp_partner = profile.is_gpp_partner;

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
            .find(|o| o.starts_with(&profile.zip_code))
            .cloned()
            .unwrap_or(profile.zip_code.clone());
        self.zip_select.update(cx, |select, cx| {
            select.set_selected_value(&zip_val, window, cx)
        });

        self.line_of_business.update(cx, |input, cx| {
            input.set_value(profile.line_of_business.clone(), window, cx)
        });
        self.business_start_input.update(cx, |input, cx| {
            input.set_date(profile.business_start_date, window, cx)
        });
        self.birth_date_input.update(cx, |input, cx| {
            input.set_date(profile.birth_date, window, cx)
        });

        let rdo_value = self
            .rdo_options
            .iter()
            .find(|o| o.starts_with(&profile.rdo_code))
            .cloned()
            .unwrap_or(profile.rdo_code.clone());
        self.rdo_select.update(cx, |select, cx| {
            select.set_selected_value(&rdo_value, window, cx)
        });

        let type_value = taxpayer_type_label(&profile.taxpayer_type).to_string();
        self.type_select.update(cx, |select, cx| {
            select.set_selected_value(&type_value, window, cx)
        });

        let tax_class_value = match profile.tax_classification {
            Some(bir_core::profile::TaxClassification::PurelyCompensation) => "Purely Compensation",
            Some(bir_core::profile::TaxClassification::SelfEmployed) => {
                "Self-Employed / Professional"
            }
            Some(bir_core::profile::TaxClassification::MixedIncome) => "Mixed Income",
            _ => "",
        };
        self.tax_classification_select.update(cx, |select, cx| {
            select.set_selected_value(tax_class_value, window, cx)
        });

        let tier_value = match profile.eopt_tier {
            Some(bir_core::profile::EoptTier::Micro) => "Micro",
            Some(bir_core::profile::EoptTier::Small) => "Small",
            Some(bir_core::profile::EoptTier::Medium) => "Medium",
            Some(bir_core::profile::EoptTier::Large) => "Large",
            None => "",
        };
        self.eopt_tier_select.update(cx, |select, cx| {
            select.set_selected_value(tier_value, window, cx)
        });

        let coop_value = match profile.tax_classification {
            Some(bir_core::profile::TaxClassification::CooperativeExempt) => "Exempt",
            Some(bir_core::profile::TaxClassification::CooperativeTaxable) => "Taxable",
            Some(bir_core::profile::TaxClassification::CooperativeMixed) => "Mixed",
            _ => "",
        };
        self.cooperative_treatment_select.update(cx, |select, cx| {
            select.set_selected_value(coop_value, window, cx)
        });

        cx.notify();
    }

    fn on_tin_event(
        &mut self,
        _state: Entity<TinInput>,
        event: &gpui_component::input::InputEvent,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            self.mark_profile_changed();
        }
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
            } else if state == &self.cor_preview_year_input {
                cx.notify();
                return;
            }

            if let Some(field) = field_to_validate {
                self.validate_field(field, &value);
                self.mark_profile_changed();
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
            self.mark_profile_changed();
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
            let changes_profile = state == self.rdo_select
                || state == self.zip_select
                || state == self.type_select
                || state == self.tax_classification_select
                || state == self.eopt_tier_select
                || state == self.cooperative_treatment_select
                || state == self.registration_activity_status_select;

            if state == self.rdo_select {
                field_to_validate = Some("rdo_code");
            } else if state == self.zip_select {
                field_to_validate = Some("zip_code");
                value = val.split(" - ").next().unwrap_or("").trim().to_string();
            } else if state == self.forms_editor_year_select {
                if let Ok(year) = val.parse::<u16>() {
                    self.forms_editor_year = year;
                    self.forms_editor_selected_code = None; // Reset detail view
                    cx.notify();
                }
            }

            if let Some(field) = field_to_validate {
                self.validate_field(field, &value);
            }
            if changes_profile {
                self.mark_profile_changed();
                cx.notify();
            }
        }
    }

    fn on_date_event(
        &mut self,
        _state: Entity<DateInputState>,
        _event: &DateInputEvent,
        cx: &mut Context<Self>,
    ) {
        self.mark_profile_changed();
        cx.notify();
    }

    fn on_multi_select_event(
        &mut self,
        _state: Entity<MultiSelectState>,
        _event: &MultiSelectEvent,
        cx: &mut Context<Self>,
    ) {
        self.mark_profile_changed();
        cx.notify();
    }

    fn derive_compliance_source_mode(
        versions: &[bir_core::profile::TaxProfileVersion],
    ) -> ComplianceSourceMode {
        if versions
            .iter()
            .any(|version| version.status == bir_core::profile::TaxProfileVersionStatus::Confirmed)
        {
            ComplianceSourceMode::CorVersioned
        } else {
            ComplianceSourceMode::TemporalSuggestion
        }
    }

    fn current_profile(&self, cx: &Context<Self>) -> TaxpayerProfile {
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
        let birth_date = if taxpayer_type == TaxpayerType::Individual {
            self.birth_date_input.read(cx).date
        } else {
            None
        };

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
            birth_date,
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
            atc_codes: self.stored_atc_codes.clone(),
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
            compliance_source_mode: Self::derive_compliance_source_mode(
                &self.stored_profile_versions,
            ),
            per_year_forms: self.stored_per_year_forms.clone(),
        }
    }

    fn selected_gemini_ocr_model(&self, cx: &mut Context<Self>) -> String {
        let custom_model = self
            .gemini_ocr_custom_model_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let selected_model = self
            .gemini_ocr_model_select
            .read(cx)
            .selected_value(cx)
            .trim()
            .to_string();
        crate::cor_ocr::resolve_gemini_model_id(&selected_model, &custom_model)
    }

    fn cor_ocr_options(&self, cx: &mut Context<Self>) -> crate::cor_ocr::CorOcrOptions {
        let model = self.selected_gemini_ocr_model(cx);
        // If Gemini OCR is enabled, consent is implied — the user already
        // opted in by enabling Gemini OCR and saving their API key.
        let cloud_consent = self.gemini_ocr_enabled || self.gemini_ocr_cloud_consent;
        tracing::info!(
            "[COR OCR] Building options: provider={}, model={model}, cloud_consent={cloud_consent} (enabled={}, checkbox={})",
            if self.gemini_ocr_enabled {
                "GeminiByok"
            } else {
                "SidecarText"
            },
            self.gemini_ocr_enabled,
            self.gemini_ocr_cloud_consent
        );
        crate::cor_ocr::CorOcrOptions {
            provider: if self.gemini_ocr_enabled {
                crate::cor_ocr::CorOcrProviderKind::GeminiByok
            } else {
                crate::cor_ocr::CorOcrProviderKind::SidecarText
            },
            gemini_model: model,
            allow_cloud_upload: cloud_consent,
        }
    }

    fn save_cor_ocr_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed_key = self
            .gemini_ocr_api_key_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        tracing::info!(
            "[OCR Settings] Saving settings. enabled={}, key_len={}, consent={}",
            self.gemini_ocr_enabled,
            typed_key.len(),
            self.gemini_ocr_cloud_consent
        );
        if let Ok(db_guard) = self.db.lock() {
            let _ = db_guard.set_setting(
                crate::cor_ocr::COR_OCR_GEMINI_ENABLED_SETTING,
                if self.gemini_ocr_enabled {
                    "true"
                } else {
                    "false"
                },
            );
            let model = self.selected_gemini_ocr_model(cx);
            let _ = db_guard.set_setting(crate::cor_ocr::COR_OCR_GEMINI_MODEL_SETTING, &model);
            tracing::info!("[OCR Settings] DB settings saved. model={model}");
        }

        if !typed_key.is_empty() {
            tracing::info!(
                "[OCR Settings] Storing API key to keychain ({} chars)…",
                typed_key.len()
            );
            self.gemini_ocr_api_key_input
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.gemini_ocr_status = Some(
                "Gemini OCR key accepted for model ".to_string()
                    + &self.selected_gemini_ocr_model(cx)
                    + ".",
            );
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { crate::cor_ocr::save_gemini_api_key(&typed_key) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    match &result {
                        Ok(()) => {
                            tracing::info!("[OCR Settings] API key stored in keychain successfully")
                        }
                        Err(e) => tracing::info!("[OCR Settings] Keychain store FAILED: {e}"),
                    }
                    this.gemini_ocr_status = Some(match result {
                        Ok(()) => "Gemini OCR key stored in OS keychain.".to_string(),
                        Err(error) => error,
                    });
                    cx.notify();
                });
            })
            .detach();
            window.push_notification(
                Notification::success("Gemini API key saved to OS keychain.").title("OCR Settings"),
                cx,
            );
        } else {
            tracing::info!("[OCR Settings] No key typed, saving settings only");
            self.gemini_ocr_status = Some(
                "Gemini OCR settings saved. API keys are stored outside profile JSON.".to_string(),
            );
            window.push_notification(
                Notification::success("Gemini OCR settings saved.").title("OCR Settings"),
                cx,
            );
        }
        cx.notify();
    }

    fn remove_gemini_ocr_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        tracing::info!("[OCR Settings] Removing Gemini API key from keychain…");
        self.gemini_ocr_cloud_consent = false;
        self.gemini_ocr_status =
            Some("Removing Gemini API key from OS secure storage...".to_string());
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::cor_ocr::delete_gemini_api_key() })
                .await;
            let _ = this.update(cx, |this, cx| {
                match &result {
                    Ok(()) => tracing::info!("[OCR Settings] API key removed from keychain"),
                    Err(e) => tracing::info!("[OCR Settings] Keychain removal FAILED: {e}"),
                }
                this.gemini_ocr_status = Some(match result {
                    Ok(()) => "Gemini API key removed from OS keychain.".to_string(),
                    Err(error) => error,
                });
                cx.notify();
            });
        })
        .detach();
        window.push_notification(
            Notification::info("Gemini API key removed from OS keychain.").title("OCR Settings"),
            cx,
        );
        cx.notify();
    }

    fn test_cor_ocr_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed_key = self
            .gemini_ocr_api_key_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let typed_key = if typed_key.is_empty() {
            None
        } else {
            Some(typed_key)
        };
        let model = self.selected_gemini_ocr_model(cx);
        self.gemini_ocr_status =
            Some("Testing Gemini OCR key. This sends a tiny test request to Google.".to_string());
        window.push_notification(
            Notification::info("Testing Gemini API key...").title("OCR Settings"),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let result =
                cx.background_executor()
                    .spawn(async move {
                        crate::cor_ocr::test_gemini_api_key(&model, typed_key.as_deref())
                    })
                    .await;
            let _ = this.update(cx, |this, cx| {
                match &result {
                    Ok(msg) => tracing::info!("[OCR Settings] API key verified: {msg}"),
                    Err(e) => tracing::info!("[OCR Settings] API key verification FAILED: {e}"),
                }
                this.gemini_ocr_status = Some(match result {
                    Ok(message) => {
                        this.pending_notification = Some((
                            NotificationType::Success,
                            message.clone().replace('\n', " "),
                        ));
                        message
                    }
                    Err(error) => {
                        this.pending_notification =
                            Some((NotificationType::Error, error.clone().replace('\n', " ")));
                        error
                    }
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn sync_current_profile_to_cor_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut profile = self.current_profile(cx);
        profile.profile_versions.clear();
        profile.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;

        let mut version = bir_core::profile::TaxProfileVersion::from_profile_backfill(&profile);
        version.id = format!("manual-cor-{}", chrono::Local::now().timestamp_millis());
        version.label = format!("Manual COR {}", chrono::Local::now().format("%Y-%m-%d"));
        version.status = bir_core::profile::TaxProfileVersionStatus::Draft;
        version.source = bir_core::profile::TaxProfileVersionSource::ManualCor;
        version.needs_effective_date_review = version.effective_from.is_none();

        self.stored_profile_versions.retain(|existing| {
            existing.status != bir_core::profile::TaxProfileVersionStatus::Draft
        });
        self.ocr_selected_version_id = Some(version.id.clone());
        self.stored_profile_versions.push(version.clone());
        self.compliance_source_mode =
            Self::derive_compliance_source_mode(&self.stored_profile_versions);
        self.active_tab = 1;
        self.sync_document_viewer(cx);
        if let Err(e) = self.load_cor_version_editor(&version.id, window, cx) {
            self.save_message = Some(e);
        }
    }

    fn upload_cor_document(
        &mut self,
        target_version_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(message) = self.cor_upload_target_error(target_version_id.as_deref()) {
            let message = message.to_string();
            self.save_message = Some(message.clone());
            window.push_notification(Notification::error(message).title("COR is read-only"), cx);
            return;
        }

        let mut profile = self.current_profile(cx);
        profile.profile_versions.clear();
        profile.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;
        let tin = profile.tin.full().replace(['-', ' '], "");
        let ocr_options = self.cor_ocr_options(cx);
        let provider_label =
            if ocr_options.provider == crate::cor_ocr::CorOcrProviderKind::GeminiByok {
                "Gemini BYOK"
            } else {
                "Local/manual"
            }
            .to_string();
        let model_label = if ocr_options.provider == crate::cor_ocr::CorOcrProviderKind::GeminiByok
        {
            Some(ocr_options.gemini_model.clone())
        } else {
            None
        };

        tracing::info!(
            "[COR Upload] Starting upload flow. Provider={provider_label}, consent={}, target_version_id={:?}",
            ocr_options.allow_cloud_upload,
            target_version_id
        );
        self.save_message = Some("Processing document… please wait.".to_string());
        self.is_uploading_cor = true;
        cx.notify();

        let target_ver_id = target_version_id.clone();
        cx.spawn(async move |this, cx| {
            tracing::info!("[COR Upload] Waiting for file picker…");
            let Some(file_handle) = rfd::AsyncFileDialog::new()
                .add_filter("COR document", &["png", "jpg", "jpeg", "pdf"])
                .pick_file()
                .await
            else {
                tracing::info!("[COR Upload] File picker cancelled.");
                let _ = this.update(cx, |this, cx| {
                    this.save_message = None;
                    this.is_uploading_cor = false;
                    cx.notify();
                });
                return;
            };
            let source_path = file_handle.path().to_path_buf();
            tracing::info!("[COR Upload] File selected: {}", source_path.display());

            // Update status
            let _ = this.update(cx, |this, cx| {
                this.save_message = Some("Extracting data from document…".to_string());
                cx.notify();
            });

            // Run OCR extraction on background thread (it uses blocking reqwest)
            let ocr_options_clone = ocr_options.clone();
            let source_clone = source_path.clone();
            tracing::info!("[COR Upload] Starting OCR extraction on background executor…");
            let ocr = cx
                .background_executor()
                .spawn(async move {
                    crate::cor_ocr::extract_cor_document_with_options(
                        &source_clone,
                        &ocr_options_clone,
                    )
                })
                .await;
            tracing::info!(
                "[COR Upload] OCR extraction done. Status: {}",
                ocr.status_message
            );
            if let Some(ref text) = ocr.text {
                tracing::info!("[COR Upload] OCR text length: {} chars", text.len());
            } else {
                tracing::info!("[COR Upload] OCR returned no text.");
            }
            tracing::info!(
                "[COR Upload] OCR fields — TIN: {:?}, Name: {:?}, Type: {:?}, Forms: {:?}",
                ocr.fields.tin,
                ocr.fields.registered_name,
                ocr.fields.taxpayer_type,
                ocr.fields.extracted_form_codes
            );

            // The target may have been confirmed or archived while OCR was running.
            // Re-check before copying evidence into managed storage, then check once
            // more before mutating the in-memory profile below.
            let target_still_editable = this.update(cx, |this, cx| {
                if let Some(message) =
                    this.cor_upload_target_error(target_ver_id.as_deref())
                {
                    let message = message.to_string();
                    this.save_message = Some(message.clone());
                    this.pending_notification = Some((NotificationType::Error, message));
                    this.is_uploading_cor = false;
                    cx.notify();
                    false
                } else {
                    true
                }
            });
            if !matches!(target_still_editable, Ok(true)) {
                tracing::info!(
                    "[COR Upload] Target became read-only or disappeared before evidence storage."
                );
                return;
            }

            // Store evidence file
            tracing::info!("[COR Upload] Storing evidence file for TIN={tin}…");
            match crate::cor_evidence::store_cor_document(&source_path, &tin) {
                Ok(mut evidence) => {
                    tracing::info!("[COR Upload] Evidence stored at: {}", evidence.stored_path);
                    evidence.provider = Some(provider_label);
                    evidence.model = model_label;
                    let _ = this.update(cx, move |this, cx| {
                        if let Some(message) =
                            this.cor_upload_target_error(target_ver_id.as_deref())
                        {
                            crate::cor_evidence::remove_stored_cor_document(&evidence);
                            let message = message.to_string();
                            this.save_message = Some(message.clone());
                            this.pending_notification = Some((NotificationType::Error, message));
                            this.gemini_ocr_cloud_consent = false;
                            this.is_uploading_cor = false;
                            cx.notify();
                            tracing::info!(
                                "[COR Upload] Refused replacement because target became read-only or disappeared."
                            );
                            return;
                        }

                        let version = crate::cor_ocr::create_draft_cor_version_from_ocr(
                            &profile,
                            evidence,
                            ocr.clone(),
                            chrono::Local::now().naive_local(),
                        );

                        let final_ver_id = if let Some(ref ver_id) = target_ver_id {
                            let mut new_ver = version;
                            new_ver.id = ver_id.clone();
                            if let Some(existing) = this
                                .stored_profile_versions
                                .iter_mut()
                                .find(|v| &v.id == ver_id)
                            {
                                new_ver.label = existing.label.clone();
                                *existing = new_ver;
                            } else {
                                this.stored_profile_versions.push(new_ver);
                            }
                            ver_id.clone()
                        } else {
                            let version_id = version.id.clone();
                            this.stored_profile_versions.push(version);
                            version_id
                        };

                        this.cor_editing_version_id = None;
                        this.ocr_selected_version_id = Some(final_ver_id.clone());
                        this.sync_document_viewer(cx);
                        this.compliance_source_mode =
                            Self::derive_compliance_source_mode(&this.stored_profile_versions);
                        this.save_profile(cx);
                        this.save_message = Some(ocr.status_message.clone());
                        this.pending_notification = Some((
                            NotificationType::Success,
                            ocr.status_message.replace('\n', " "),
                        ));
                        // Defer editor field load to next render frame (needs Window)
                        this.pending_cor_editor_load = Some(final_ver_id);
                        // Reset consent AFTER we've used the options
                        this.gemini_ocr_cloud_consent = false;
                        this.is_uploading_cor = false;
                        cx.notify();
                        tracing::info!("[COR Upload] UI updated. pending_cor_editor_load set.");
                    });
                }
                Err(error) => {
                    tracing::error!("[COR Upload] Evidence storage failed: {error}");
                    let _ = this.update(cx, |this, cx| {
                        this.save_message = Some(error.clone());
                        this.pending_notification =
                            Some((NotificationType::Error, error.replace('\n', " ")));
                        this.is_uploading_cor = false;
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

    fn remove_cor_document(
        &mut self,
        version_id: &str,
        document_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(version) = self
            .stored_profile_versions
            .iter_mut()
            .find(|version| version.id == version_id)
        else {
            self.save_message = Some("COR version was not found.".to_string());
            window.push_notification(
                Notification::error("COR version was not found.").title("OCR"),
                cx,
            );
            return;
        };

        if !Self::profile_version_facts_are_editable(&version.status) {
            let message = Self::immutable_cor_version_message().to_string();
            self.save_message = Some(message.clone());
            window.push_notification(Notification::error(message).title("COR is read-only"), cx);
            return;
        }

        let removed_document = version
            .evidence
            .iter()
            .find(|document| document.id == document_id)
            .cloned();
        let before = version.evidence.len();
        version
            .evidence
            .retain(|document| document.id != document_id);

        if version.evidence.len() == before {
            self.save_message = Some("COR evidence file was not found.".to_string());
            window.push_notification(
                Notification::error("COR evidence file was not found.").title("OCR"),
                cx,
            );
            return;
        }

        self.mark_profile_changed();
        if let Some(document) = removed_document {
            self.pending_cor_evidence_cleanup
                .push((self.profile_change_revision, document));
        }
        self.save_profile(cx);
    }

    fn delete_cor_version(
        &mut self,
        version_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(version_index) = self
            .stored_profile_versions
            .iter()
            .position(|version| version.id == version_id)
        else {
            self.save_message = Some("COR version was not found.".to_string());
            return;
        };
        if !Self::profile_version_facts_are_editable(
            &self.stored_profile_versions[version_index].status,
        ) {
            let message = Self::immutable_cor_version_message().to_string();
            self.save_message = Some(message.clone());
            window.push_notification(Notification::error(message).title("COR is read-only"), cx);
            return;
        }

        let version = self.stored_profile_versions.remove(version_index);
        if self.ocr_selected_version_id.as_deref() == Some(version_id) {
            self.ocr_selected_version_id = None;
            self.interactive_document_viewer = None;
        }
        if self.cor_editing_version_id.as_deref() == Some(version_id) {
            self.cor_editing_version_id = None;
        }
        self.mark_profile_changed();
        let cleanup_revision = self.profile_change_revision;
        self.pending_cor_evidence_cleanup.extend(
            version
                .evidence
                .into_iter()
                .map(|evidence| (cleanup_revision, evidence)),
        );
        self.save_profile(cx);
    }

    fn cor_version_confirmation_plan(
        &self,
        version_id: &str,
        cx: &Context<Self>,
    ) -> Result<TaxProfileVersionConfirmationPlan, String> {
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

        let mut profile = self.current_profile(cx);
        profile.profile_versions = self.stored_profile_versions.clone();
        profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;
        profile
            .profile_version_confirmation_plan(version_id, effective_from)
            .ok_or_else(|| "COR version confirmation could not be prepared.".to_string())
    }

    fn request_cor_version_confirmation(
        &mut self,
        version_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.cor_version_confirmation_plan(version_id, cx) {
            Ok(plan) if plan.auto_close_consequences.is_empty() => {
                self.apply_cor_version_confirmation(plan, window, cx);
            }
            Ok(plan) => {
                self.pending_profile_version_confirmation = Some(plan);
                cx.notify();
            }
            Err(message) => {
                self.save_message = Some(message);
                cx.notify();
            }
        }
    }

    fn apply_cor_version_confirmation(
        &mut self,
        plan: TaxProfileVersionConfirmationPlan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pending_profile_version_confirmation = None;
        match self.confirm_cor_version(&plan, window, cx) {
            Ok(()) => {
                self.compliance_source_mode =
                    Self::derive_compliance_source_mode(&self.stored_profile_versions);
            }
            Err(message) => self.save_message = Some(message),
        }
        cx.notify();
    }

    fn confirm_cor_version(
        &mut self,
        plan: &TaxProfileVersionConfirmationPlan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(version_clone) = self
            .stored_profile_versions
            .iter()
            .find(|version| version.id == plan.version_id)
            .cloned()
        else {
            return Err("COR version was not found.".to_string());
        };

        let mut profile = self.current_profile(cx);
        profile.profile_versions = self.stored_profile_versions.clone();
        profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;

        if profile.apply_profile_version_confirmation_plan(plan) {
            self.compliance_source_mode = ComplianceSourceMode::CorVersioned;
            self.stored_profile_versions = profile.profile_versions.clone();

            // Sync the projected profile to the UI inputs so that when the user clicks "Save Profile",
            // the OCR-derived classifications and fields are persisted.
            let projected = profile.projection_for_version(&version_clone);
            self.sync_projection_to_ui(&projected, window, cx);

            use chrono::Datelike as _;
            let year = plan.effective_from.year() as u16;
            let suggestions =
                bir_core::integration::form_suggestions_for_profile_year(&profile, year);
            let reconciliation = bir_core::forms::reconcile_forms_set_for_year(
                year,
                self.stored_per_year_forms.get(&year),
                &suggestions,
            );
            self.stored_per_year_forms
                .insert(year, reconciliation.forms_set);
            profile.per_year_forms = self.stored_per_year_forms.clone();

            self.mark_profile_changed();
            self.save_profile_with_reviewed_confirmation(plan.clone(), cx);

            Ok(())
        } else {
            Err(
                "The profile timeline changed while confirmation was open. Review the dates and confirm again; no profile or Forms Set data was changed."
                    .to_string(),
            )
        }
    }

    fn clear_cor_override_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for input in [
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

    fn profile_version_facts_are_editable(
        status: &bir_core::profile::TaxProfileVersionStatus,
    ) -> bool {
        matches!(
            status,
            bir_core::profile::TaxProfileVersionStatus::Draft
                | bir_core::profile::TaxProfileVersionStatus::NeedsReview
        )
    }

    fn immutable_cor_version_message() -> &'static str {
        "Confirmed and archived COR facts are read-only. Create a replacement version to correct dates, tax facts, evidence, or overrides, then confirm it through the reviewed workflow."
    }

    fn cor_upload_target_error(&self, target_version_id: Option<&str>) -> Option<&'static str> {
        let target_version_id = target_version_id?;
        match self
            .stored_profile_versions
            .iter()
            .find(|version| version.id == target_version_id)
        {
            Some(version) if Self::profile_version_facts_are_editable(&version.status) => None,
            Some(_) => Some(Self::immutable_cor_version_message()),
            None => Some(
                "COR version was not found. Create a replacement draft and upload the document there.",
            ),
        }
    }

    fn cor_override_target_version_id(&self) -> Option<String> {
        if let Some(editing_id) = self.cor_editing_version_id.as_ref() {
            return self
                .stored_profile_versions
                .iter()
                .find(|version| {
                    version.id == *editing_id
                        && Self::profile_version_facts_are_editable(&version.status)
                })
                .map(|version| version.id.clone());
        }

        self.stored_profile_versions
            .iter()
            .rev()
            .find(|version| Self::profile_version_facts_are_editable(&version.status))
            .map(|version| version.id.clone())
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
        self.cor_tin_input
            .update(cx, |input, cx| input.clear(window, cx));
        self.cor_rdo_select.update(cx, |select, cx| {
            select.set_selected_value("", window, cx);
        });
        self.cor_extracted_forms.clear();
        self.cor_extracted_forms_select.update(cx, |select, cx| {
            select.set_selected_ids(vec![], cx);
        });
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
        self.cor_tin_input.update(cx, |input, cx| {
            input.set_text_value(&version.cor.tin.clone().unwrap_or_default(), window, cx);
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
            input.set_value(version.cor.rdo_code.clone(), window, cx)
        });
        let rdo_value = self
            .rdo_options
            .iter()
            .find(|option| option.starts_with(&version.cor.rdo_code))
            .cloned()
            .unwrap_or(version.cor.rdo_code.clone());
        self.cor_rdo_select.update(cx, |select, cx| {
            select.set_selected_value(&rdo_value, window, cx);
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

        self.cor_extracted_forms = version
            .evidence
            .first()
            .map(|e| e.extracted_form_codes.clone())
            .unwrap_or_default();
        let forms = self.cor_extracted_forms.clone();
        self.cor_extracted_forms_select.update(cx, |select, cx| {
            select.set_selected_ids(forms, cx);
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
        let label = self
            .cor_version_label_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        if label.is_empty() {
            return Err("COR version label is required.".to_string());
        }
        let Some(version_index) = self
            .stored_profile_versions
            .iter()
            .position(|version| version.id == version_id)
        else {
            return Err("COR version was not found.".to_string());
        };
        if !Self::profile_version_facts_are_editable(
            &self.stored_profile_versions[version_index].status,
        ) {
            self.stored_profile_versions[version_index].label = label;
            self.save_profile(cx);
            self.save_message = Some(
                "COR version label updated. Confirmed and archived facts remain read-only; create a replacement version for corrections."
                    .to_string(),
            );
            self.load_cor_version_editor(&version_id, window, cx)?;
            return Ok(());
        }

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

        let version = &mut self.stored_profile_versions[version_index];

        version.label = label;
        version.effective_from = effective_from;
        version.effective_until = effective_until;
        version.needs_effective_date_review = version.effective_from.is_none();
        version.cor.tin = {
            let value = self.cor_tin_input.read(cx).value(cx).trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        };
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
        version.cor.rdo_code = {
            let val = self.cor_rdo_select.read(cx).selected_value(cx);
            val.split(" - ").next().unwrap_or("").trim().to_string()
        };
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

        let extracted_forms = self.cor_extracted_forms.clone();
        if version.evidence.is_empty() {
            version.evidence.push(bir_core::profile::CorDocumentRef {
                id: format!(
                    "manual-evidence-{}",
                    chrono::Local::now().timestamp_millis()
                ),
                file_name: "manual_entry".to_string(),
                stored_path: "manual_entry".to_string(),
                uploaded_at: Some(chrono::Local::now().naive_local()),
                provider: Some("Manual".to_string()),
                model: None,
                document_type: Some("COR Form 2303".to_string()),
                extracted_form_codes: extracted_forms,
                ocr_text: None,
                ocr_confidence: None,
                field_bboxes: std::collections::HashMap::new(),
            });
        } else if let Some(evidence) = version.evidence.first_mut() {
            evidence.extracted_form_codes = extracted_forms;
        }

        let version_clone = version.clone();
        let mut profile = self.current_profile(cx);
        profile.profile_versions = self.stored_profile_versions.clone();

        if version_clone.status == bir_core::profile::TaxProfileVersionStatus::Confirmed {
            let projected = profile.projection_for_version(&version_clone);
            self.sync_projection_to_ui(&projected, window, cx);
        }

        self.save_profile(cx);

        self.save_message = Some("COR version details updated and saved.".to_string());
        self.load_cor_version_editor(&version_id, window, cx)?;
        Ok(())
    }

    fn toggle_cor_registered_tax_type(
        &mut self,
        version_id: &str,
        tax_type: RegisteredTaxType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(version) = self
            .stored_profile_versions
            .iter_mut()
            .find(|version| version.id == version_id)
        else {
            self.save_message = Some("COR version was not found.".to_string());
            return;
        };

        if !Self::profile_version_facts_are_editable(&version.status) {
            let message = Self::immutable_cor_version_message().to_string();
            self.save_message = Some(message.clone());
            window.push_notification(Notification::error(message).title("COR is read-only"), cx);
            return;
        }

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

        let version_clone = version.clone();
        if version_clone.status == bir_core::profile::TaxProfileVersionStatus::Confirmed {
            let mut profile = self.current_profile(cx);
            profile.profile_versions = self.stored_profile_versions.clone();
            let projected = profile.projection_for_version(&version_clone);
            self.sync_projection_to_ui(&projected, window, cx);
        }

        self.save_profile(cx);
        self.save_message = Some("Registered tax type updated and saved.".to_string());
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

        let Some(target_version_id) = self.cor_override_target_version_id() else {
            return Err("Create a COR/manual version before adding profile overrides.".to_string());
        };
        let Some(version) = self
            .stored_profile_versions
            .iter_mut()
            .find(|version| version.id == target_version_id)
        else {
            return Err("COR version was not found.".to_string());
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
        self.save_profile(cx);
        self.save_message = Some(format!(
            "Profile deadline override '{title}' added and saved."
        ));
        Ok(())
    }

    fn remove_cor_deadline_override(
        &mut self,
        version_id: &str,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(version) = self
            .stored_profile_versions
            .iter_mut()
            .find(|version| version.id == version_id)
        else {
            self.save_message = Some("COR version was not found.".to_string());
            return;
        };
        if !Self::profile_version_facts_are_editable(&version.status) {
            self.save_message = Some(Self::immutable_cor_version_message().to_string());
            return;
        }
        if index >= version.deadline_overrides.len() {
            self.save_message = Some("Profile deadline override was not found.".to_string());
            return;
        }
        version.deadline_overrides.remove(index);
        self.save_profile(cx);
        self.save_message = Some("Profile deadline override removed.".to_string());
    }

    fn save_profile(&mut self, cx: &mut Context<Self>) {
        self.save_profile_inner(None, cx);
    }

    fn save_profile_with_reviewed_confirmation(
        &mut self,
        reviewed_plan: TaxProfileVersionConfirmationPlan,
        cx: &mut Context<Self>,
    ) {
        self.save_profile_inner(Some(reviewed_plan), cx);
    }

    fn save_profile_inner(
        &mut self,
        reviewed_plan: Option<TaxProfileVersionConfirmationPlan>,
        cx: &mut Context<Self>,
    ) {
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
                self.active_tab = 3;
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

        let save_revision = self.profile_change_revision;
        let db_arc_clone = db_arc.clone();
        cx.spawn(async move |this, cx| {
            let is_email_tracking_active = profile.is_email_tracking_active();
            let save_result = cx
                .background_executor()
                .spawn(async move {
                    if let Ok(db) = db_arc.lock() {
                        if let Some(reviewed_plan) = reviewed_plan.as_ref() {
                            db.save_profile_with_confirmation_plan_and_post_commit_status(
                                profile,
                                reviewed_plan,
                            )
                            .map(bir_core::db::PostCommitWrite::into_parts)
                            .map_err(|e| e.to_string())
                        } else {
                            db.save_profile_with_post_commit_status(profile)
                                .map(bir_core::db::PostCommitWrite::into_parts)
                                .map_err(|e| e.to_string())
                        }
                    } else {
                        Err("Database lock is poisoned".to_string())
                    }
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                match save_result {
                    Ok((saved, refresh_status)) => {
                        let Some(saved_id) = saved.id else {
                            this.save_message = None;
                            this.pending_notification = Some((
                                NotificationType::Error,
                                "Save failed: database did not return a profile id".to_string(),
                            ));
                            cx.notify();
                            return;
                        };
                        let tin_val = saved.tin.full();
                        let affected_years = compliance_affected_years(&saved);
                        this.cleanup_saved_cor_evidence(&saved, save_revision);
                        this.editing_id = Some(saved_id);
                        this.persisted_profile_tin = Some(tin_val.clone());
                        this.stored_atc_codes.clone_from(&saved.atc_codes);
                        this.stored_tax_elections.clone_from(&saved.tax_elections);
                        this.stored_profile_versions
                            .clone_from(&saved.profile_versions);
                        this.stored_per_year_forms.clone_from(&saved.per_year_forms);
                        if this.profile_change_revision == save_revision {
                            this.clear_profile_changed();
                        }
                        this.save_message = None;
                        this.pending_notification = Some(match refresh_status.warning() {
                            Some(warning) => (
                                gpui_component::notification::NotificationType::Warning,
                                format!("Profile saved. {warning}"),
                            ),
                            None => (
                                gpui_component::notification::NotificationType::Success,
                                "Profile saved".to_string(),
                            ),
                        });

                        cx.emit(ProfileEvent::Saved(tin_val.clone()));
                        let bus = cx.global::<crate::events::GlobalEventBus>().0.clone();
                        bus.update(cx, |_, cx| {
                            cx.emit(crate::events::AppEvent::ProfileComplianceChanged {
                                tin: tin_val.clone(),
                                affected_years: affected_years.clone(),
                            });
                        });

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

    fn cleanup_saved_cor_evidence(&mut self, saved: &TaxpayerProfile, save_revision: u64) {
        let referenced_paths = saved
            .profile_versions
            .iter()
            .flat_map(|version| version.evidence.iter())
            .map(|evidence| evidence.stored_path.clone())
            .collect::<std::collections::HashSet<_>>();
        let mut ready = Vec::new();
        self.pending_cor_evidence_cleanup
            .retain(|(revision, evidence)| {
                let should_remove =
                    *revision <= save_revision && !referenced_paths.contains(&evidence.stored_path);
                if should_remove {
                    ready.push(evidence.clone());
                }
                !should_remove
            });
        for evidence in ready {
            crate::cor_evidence::remove_stored_cor_document(&evidence);
        }
    }

    fn field_label(text: &str, cx: &Context<Self>) -> Div {
        div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .mb_1()
            .child(text.to_string())
    }

    fn render_unsaved_profile_banner(&self, cx: &Context<Self>) -> gpui::AnyElement {
        if !self.has_unsaved_profile_changes {
            return div().into_any_element();
        }

        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .px_4()
            .py_3()
            .rounded_lg()
            .border_1()
            .border_color(gpui::rgba(0xd97706a0))
            .bg(gpui::rgba(0xfef3c730))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::rgb(0x92400e))
                            .child("Unsaved profile and Forms Set changes"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0x92400e))
                            .child(
                                "Save or discard these changes before switching profiles or opening a filing form.",
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        gpui_component::button::Button::new("discard_profile_changes")
                            .label("Discard")
                            .ghost()
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.discard_profile_changes(window, cx);
                            })),
                    )
                    .child(
                        gpui_component::button::Button::new("save_profile_changes")
                            .label("Save Changes")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.save_profile(cx);
                            })),
                    ),
            )
            .into_any_element()
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

        // Deferred editor field load after upload (needs Window access)
        if let Some(version_id) = self.pending_cor_editor_load.take() {
            if self.active_tab == 1 {
                tracing::info!("[COR Editor] Deferred load firing for version_id={version_id}");
                match self.load_cor_version_editor(&version_id, _window, cx) {
                    Ok(()) => tracing::info!("[COR Editor] Fields populated successfully"),
                    Err(e) => tracing::info!("[COR Editor] Failed to load: {e}"),
                }
            } else {
                tracing::info!(
                    "[COR Editor] Deferred load skipped — not on OCR tab (tab={}), re-queuing",
                    self.active_tab
                );
                self.pending_cor_editor_load = Some(version_id);
            }
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

        let is_purely_compensation = is_individual && tax_class_val == "Purely Compensation";
        if is_purely_compensation && self.active_tab == 1 {
            self.active_tab = 0;
        }

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
                        this.save_profile(cx);
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
                    // Bug 4+7: Reduce padding and remove max-width on OCR detail view
                    .when(self.active_tab == 1 && self.ocr_selected_version_id.is_some(), |this| {
                        this.px_4().py_6()
                    })
                    .when(!(self.active_tab == 1 && self.ocr_selected_version_id.is_some()), |this| {
                        this.p_12()
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .when(!(self.active_tab == 1 && self.ocr_selected_version_id.is_some()), |this| {
                                this.max_w(px(960.))
                            })
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
                            .child(self.render_unsaved_profile_banner(cx))
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
                                            .when(!is_purely_compensation, |this| {
                                                this.child(
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
                                                            // Always show timeline first when switching to OCR tab
                                                            this.ocr_selected_version_id = None;
                                                            this.interactive_document_viewer = None;
                                                            cx.notify();
                                                        }))
                                                        .child(div().text_sm().child("COR")),
                                                )
                                            })
                                            .child(
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
                                                    .child(div().text_sm().child("Email Settings")),
                                            )
                                            .when(global_pins_enabled, |this| {
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
                                                        .child(div().text_sm().child("Security")),
                                                )
                                            })
                                            .when(self.editing_id.is_some(), |this| {
                                                this.child(
                                                    div()
                                                        .id("tab_4")
                                                        .px_4()
                                                        .py_1p5()
                                                        .rounded_md()
                                                        .cursor_pointer()
                                                        .when(self.active_tab == 4, |s| {
                                                            s.bg(cx.theme().background)
                                                                .shadow_sm()
                                                                .text_color(cx.theme().foreground)
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                        })
                                                        .when(self.active_tab != 4, |s| {
                                                            s.hover(|s| s.bg(cx.theme().muted))
                                                                .text_color(cx.theme().muted_foreground)
                                                                .font_weight(FontWeight::MEDIUM)
                                                        })
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.active_tab = 4;
                                                            cx.notify();
                                                        }))
                                                        .child(div().text_sm().child("Export")),
                                                )
                                                .child(
                                                    div()
                                                        .id("tab_5")
                                                        .px_4()
                                                        .py_1p5()
                                                        .rounded_md()
                                                        .cursor_pointer()
                                                        .when(self.active_tab == 5, |s| {
                                                            s.bg(cx.theme().background)
                                                                .shadow_sm()
                                                                .text_color(cx.theme().foreground)
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                        })
                                                        .when(self.active_tab != 5, |s| {
                                                            s.hover(|s| s.bg(cx.theme().muted))
                                                                .text_color(cx.theme().muted_foreground)
                                                                .font_weight(FontWeight::MEDIUM)
                                                        })
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.active_tab = 5;
                                                            cx.notify();
                                                        }))
                                                        .child(div().text_sm().child("Forms Set")),
                                                )
                                                .child(
                                                    div()
                                                        .id("tab_6")
                                                        .px_4()
                                                        .py_1p5()
                                                        .rounded_md()
                                                        .cursor_pointer()
                                                        .when(self.active_tab == 6, |s| {
                                                            s.bg(cx.theme().background)
                                                                .shadow_sm()
                                                                .text_color(cx.theme().foreground)
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                        })
                                                        .when(self.active_tab != 6, |s| {
                                                            s.hover(|s| s.bg(cx.theme().muted))
                                                                .text_color(cx.theme().muted_foreground)
                                                                .font_weight(FontWeight::MEDIUM)
                                                        })
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.active_tab = 6;
                                                            cx.notify();
                                                        }))
                                                        .child(div().text_sm().child("Calendar")),
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
                                    .when(!is_purely_compensation, |this| {
                                        this.child(self.render_ocr_tab(cx))
                                    })
                                    .child(self.render_email_settings_tab(cx))
                                    .child(self.render_security_tab(global_pins_enabled, cx))
                                    .child(self.render_export_tab(cx))
                                    .child(self.render_active_forms_tab(cx))
                                    .child(self.render_calendar_tab(cx))
                            )
                            .when(self.active_tab != 1 && self.active_tab != 6, |this| {
                                this.child(
                                    div()
                                        .mt_4()
                                        .pb(px(80.))
                                        .flex()
                                        .items_center()
                                        .gap_4()
                                        .child(
                                            gpui_component::button::Button::new("save_profile")
                                                .label("Save Profile")
                                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                                    this.save_profile(cx);
                                                })),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(self.save_message.clone().unwrap_or_default())
                                        )
                                )
                            })
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
            .when_some(
                self.pending_profile_version_confirmation.clone(),
                |this, plan| {
                    let plan_for_confirm = plan.clone();
                    let closes_multiple_versions = plan.auto_close_consequences.len() > 1;
                    let consequences = plan.auto_close_consequences.iter().enumerate().fold(
                        div().flex().flex_col().gap_2(),
                        |list, (index, consequence)| {
                            let prior_effective_from = consequence
                                .effective_from
                                .map(|date| date.format("%Y-%m-%d").to_string())
                                .unwrap_or_else(|| "Needs review (no effective date)".to_string());
                            list.child(
                                div()
                                    .id(format!("profile-version-auto-close-{index}"))
                                    .p_3()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(cx.theme().danger.opacity(0.5))
                                    .bg(cx.theme().danger.opacity(0.08))
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(format!(
                                                "Prior version: {}",
                                                consequence.version_label
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "Version ID: {}",
                                                consequence.version_id
                                            )),
                                    )
                                    .child(
                                        div().text_sm().child(format!(
                                            "Effective From: {prior_effective_from}"
                                        )),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().danger)
                                            .child(format!(
                                                "Effective Until: Open (no end date) → {}",
                                                consequence.effective_until.format("%Y-%m-%d")
                                            )),
                                    ),
                            )
                        },
                    );

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
                                    .w_full()
                                    .max_w(px(560.))
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_xl()
                                    .p_6()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .shadow_lg()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::BOLD)
                                            .child("Confirm profile timeline change"),
                                    )
                                    .child(
                                        div().text_sm().child(format!(
                                            "Confirm “{}” with Effective From {}?",
                                            plan.version_label,
                                            plan.effective_from.format("%Y-%m-%d")
                                        )),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("Version ID: {}", plan.version_id)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(if closes_multiple_versions {
                                                "This will close the currently open confirmed profile versions shown below. Profile data and the yearly Forms Set will not change unless you confirm."
                                            } else {
                                                "This will close the currently open confirmed profile version shown below. Profile data and the yearly Forms Set will not change unless you confirm."
                                            }),
                                    )
                                    .child(consequences)
                                    .child(
                                        div()
                                            .flex()
                                            .justify_end()
                                            .gap_2()
                                            .child(
                                                gpui_component::button::Button::new(
                                                    "cancel-profile-version-confirmation",
                                                )
                                                .label("Cancel")
                                                .ghost()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.pending_profile_version_confirmation =
                                                        None;
                                                    cx.notify();
                                                })),
                                            )
                                            .child(
                                                gpui_component::button::Button::new(
                                                    "confirm-profile-version-timeline-change",
                                                )
                                                .label(if closes_multiple_versions {
                                                    "Confirm and Close Prior Versions"
                                                } else {
                                                    "Confirm and Close Prior Version"
                                                })
                                                .danger()
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.apply_cor_version_confirmation(
                                                            plan_for_confirm.clone(),
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                )),
                                            ),
                                    ),
                            ),
                    )
                },
            )
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
