use chrono::Datelike;
use gpui::prelude::*;
use gpui::*;
use gpui_component::WindowExt;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState, OtpEvent, OtpInput, OtpState};
use gpui_component::notification::{Notification, NotificationType};
use gpui_component::*;
use gpui_rsx::rsx;
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
    ComplianceSourceMode, EoptTier, RegistrationActivityStatus, TaxClassification, TaxpayerProfile,
    TaxpayerType, unused_profile_years,
};
use bir_core::reference::get_all_rdos;
use bir_core::validation::{ValidationError, validate_profile};

// ─── Tab sub-modules ──────────────────────────────────────────────────────────
// Each file holds one `impl ProfileManagerView` block rendering that tab's UI.
// Imports from this module are re-exported via `use super::*` in each sub-module.
mod dirty_state;
mod tab_calendar;
mod tab_email_settings;
mod tab_export;
mod tab_security;
mod tab_tax_profile;
// ─────────────────────────────────────────────────────────────────────────────

pub enum ProfileEvent {
    Saved(String),
    /// The editor is asking for an administrator-gated lifecycle change.
    /// `AppState` owns the admin prompt and the database write, so the editor
    /// only ever states the intent - it never archives or deletes anything
    /// itself.
    LifecycleRequested {
        tin: String,
        action: crate::app::ProfileLifecycleAction,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProfileSaveRequest {
    profile_session_epoch: u64,
    profile_change_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileSaveDispatchAction {
    Dispatch,
    DuplicateInFlight,
    QueueBehindInFlight,
}

/// Builds the duplicate-TIN error shown when creating a profile whose TIN is
/// already taken.
///
/// The uniqueness check queries the database directly, but the sidebar list is
/// filtered - by the archive toggle, and by the `hide_tax_profiles` privacy
/// setting, which shows only the active session profile while the search box is
/// empty. So the offending profile is frequently *not visible*, and a bare
/// "already exists" leaves the user unable to find or reach it.
///
/// The lookup already returns the profile, so say which case it is.
fn duplicate_tin_message(formatted_tin: &str, existing_is_archived: bool) -> String {
    let hint = if existing_is_archived {
        "It is archived - use the archive toggle in the sidebar to show it."
    } else {
        "If it is not listed in the sidebar, search that TIN there to open it."
    };
    format!("A profile with TIN {formatted_tin} already exists. Each TIN must be unique. {hint}")
}

fn profile_save_dispatch_action(
    active: Option<&ProfileSaveRequest>,
    requested: &ProfileSaveRequest,
) -> ProfileSaveDispatchAction {
    match active {
        None => ProfileSaveDispatchAction::Dispatch,
        Some(active) if active == requested => ProfileSaveDispatchAction::DuplicateInFlight,
        Some(_) => ProfileSaveDispatchAction::QueueBehindInFlight,
    }
}

fn income_tax_election_from_label(
    label: &str,
) -> Result<Option<bir_core::profile::IncomeTaxElection>, String> {
    match label.trim() {
        "" => Ok(None),
        "8% Flat Rate" => Ok(Some(bir_core::profile::IncomeTaxElection::EightPercent)),
        "Graduated + OSD" => Ok(Some(bir_core::profile::IncomeTaxElection::GraduatedOsd)),
        "Graduated + Itemized" => Ok(Some(
            bir_core::profile::IncomeTaxElection::GraduatedItemized,
        )),
        _ => Err("Choose an income-tax election from the list.".to_string()),
    }
}

fn upsert_income_tax_election(
    elections: &mut Vec<bir_core::profile::TaxElectionHistory>,
    election: bir_core::profile::TaxElectionHistory,
) {
    elections.retain(|stored| stored.taxable_year != election.taxable_year);
    elections.push(election);
    elections.sort_by_key(|stored| std::cmp::Reverse(stored.taxable_year));
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
    has_unsaved_forms_set_changes: bool,
    clean_profile_snapshot: Option<serde_json::Value>,
    clean_forms_set_snapshot: std::collections::BTreeMap<u16, bir_core::forms::PerYearFormsSet>,
    profile_change_revision: u64,
    /// Bumped every time the view is re-pointed at a different profile
    /// (`edit_profile`) or a fresh new-profile form (`reset_for_new`).
    /// `profile_change_revision` resets on those transitions, so revision
    /// equality alone cannot tell a save completion that its profile is no
    /// longer the one on screen.
    profile_session_epoch: u64,
    /// Revision at which the most recent save was dispatched. Lets an older
    /// completion know a newer save is still in flight, so it must not clear
    /// the "Saving..." indicator or claim success for edits it never wrote.
    last_save_dispatch_revision: u64,
    /// Number of background profile saves dispatched but not yet completed.
    /// Discard must not delete "unpersisted" evidence files while one is in
    /// flight — the write may be about to persist references to them.
    saves_in_flight: u32,
    /// The single profile save currently allowed to write the database.
    /// Later save requests are coalesced instead of being dispatched in
    /// parallel, because background tasks are not guaranteed to acquire the
    /// database mutex in dispatch order.
    active_profile_save: Option<ProfileSaveRequest>,
    /// Latest distinct request made while a profile save is active. It is
    /// dispatched after the active write completes, but only if the view is
    /// still showing the same profile session.
    queued_profile_save: Option<ProfileSaveRequest>,
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
    /// Google account email for a shared inbox grant. Used so Save Profile
    /// after Connect writes `imap_email` even if the IMAP input was empty.
    stored_oauth_inbox_email: Option<String>,
    stored_test_notification_enabled: bool,
    stored_is_archived: bool,
    stored_profile_pin_hash: Option<String>,
    stored_atc_codes: Vec<String>,
    stored_tax_elections: Vec<bir_core::profile::TaxElectionHistory>,

    enable_profile_pin: bool,
    profile_pin_input: Entity<OtpState>,

    is_totp_enabled: bool,
    show_totp_setup: bool,
    setup_totp_state: Entity<OtpState>,
    totp_secret_temp: Option<String>,
    totp_qr_path: Option<std::path::PathBuf>,
    show_totp_secret_text: bool,
    stored_totp_secret: Option<String>,

    pub stored_per_year_forms: std::collections::BTreeMap<u16, bir_core::forms::PerYearFormsSet>,
    stored_profile_years: std::collections::BTreeMap<u16, bir_core::profile::ProfileYearFacts>,
    pub forms_editor_year: u16,
    pub forms_editor_year_select: Entity<ComboboxState>,
    pub forms_editor_year_add_select: Entity<ComboboxState>,
    pub forms_editor_new_code_input: Entity<InputState>,
    pub forms_editor_registry_form_select: Entity<ComboboxState>,
    pub forms_editor_custom_code_mode: bool,
    pub forms_editor_new_reason_input: Entity<InputState>,
    pub forms_editor_new_frequency_select: Entity<ComboboxState>,
    pub forms_editor_selected_code: Option<String>,
    pub forms_editor_active_note_input: Entity<InputState>,
    calendar_name_input: Entity<InputState>,
    calendar_action_message: Option<(bool, String)>,
    calendar_form_selection: bir_core::google_calendar::CalendarFormSelection,

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
    years.extend(profile.profile_years.keys().copied());

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
            let mut state = ComboboxState::new(vec![current_year.to_string()], 5, window, cx);
            state.set_selected_value(&current_year.to_string(), window, cx);
            state
        });
        let forms_editor_year_add_select = cx.new(|cx| {
            let unused = unused_profile_years(None, current_year, [forms_editor_year])
                .into_iter()
                .map(|year| year.to_string())
                .collect::<Vec<_>>();
            let default = unused.last().cloned().unwrap_or_default();
            let mut state = ComboboxState::new(unused, 5, window, cx);
            if !default.is_empty() {
                state.set_selected_value(&default, window, cx);
            }
            state
        });
        let forms_editor_new_code_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Custom form code"));
        let forms_editor_registry_form_select = cx.new(|cx| {
            let mut options: Vec<String> = bir_core::forms::registry::FORM_REGISTRY
                .iter()
                .map(|form| format!("{} - {}", form.code, form.title))
                .collect();
            options.sort();
            ComboboxState::new(options, 8, window, cx)
        });
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
            cx.subscribe(&tax_election_select, Self::on_tax_election_select_event),
            cx.subscribe_in(
                &tax_election_year_input,
                window,
                Self::on_tax_election_year_event,
            ),
            cx.subscribe(&excise_select, Self::on_multi_select_event),
            cx.subscribe_in(
                &business_start_input,
                window,
                Self::on_business_start_date_event,
            ),
            cx.subscribe(&birth_date_input, Self::on_date_event),
            cx.subscribe(
                &registration_activity_status_select,
                Self::on_combobox_event,
            ),
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

        cx.subscribe_in(
            &forms_editor_year_select,
            window,
            |this: &mut Self, _, event: &ComboboxEvent, window, cx| {
                if let Some(val) = event.selected.as_ref()
                    && let Ok(year) = val.parse::<u16>()
                {
                    this.switch_profile_year(year, window, cx);
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

        let mut view = Self {
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
            stored_oauth_inbox_email: None,
            stored_test_notification_enabled: false,
            stored_is_archived: false,
            stored_profile_pin_hash: None,
            stored_atc_codes: vec![],
            stored_tax_elections: vec![],
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
            has_unsaved_profile_changes: false,
            has_unsaved_forms_set_changes: false,
            clean_profile_snapshot: None,
            clean_forms_set_snapshot: std::collections::BTreeMap::new(),
            profile_change_revision: 0,
            profile_session_epoch: 0,
            last_save_dispatch_revision: 0,
            saves_in_flight: 0,
            active_profile_save: None,
            queued_profile_save: None,
            persisted_profile_tin: None,
            stored_per_year_forms: std::collections::BTreeMap::new(),
            stored_profile_years: std::collections::BTreeMap::new(),
            forms_editor_year,
            forms_editor_year_select,
            forms_editor_year_add_select,
            forms_editor_new_code_input,
            forms_editor_registry_form_select,
            forms_editor_custom_code_mode: false,
            forms_editor_new_reason_input,
            forms_editor_new_frequency_select,
            forms_editor_selected_code: None,
            forms_editor_active_note_input,
            calendar_name_input,
            calendar_action_message: None,
            calendar_form_selection: Default::default(),
            _subscriptions: subscriptions,
        };
        view.capture_clean_baseline(cx);
        view
    }

    /// Whether the in-memory profile, compliance ledger, or yearly Forms Set
    /// contains changes that have not completed a database save.
    pub fn has_unsaved_compliance_changes(&self) -> bool {
        self.has_unsaved_profile_changes || self.has_unsaved_forms_set_changes
    }

    pub(crate) fn agent_read_editor(&self, cx: &App) -> crate::agent::ProfileEditor {
        let rdo = self
            .rdo_select
            .read(cx)
            .selected_value(cx)
            .split(" - ")
            .next()
            .unwrap_or("")
            .to_string();
        crate::agent::ProfileEditor {
            tin: self.tin_input.read(cx).value(cx),
            full_name: self.name_input.read(cx).value().to_string(),
            rdo_code: rdo,
            line_of_business: self.line_of_business.read(cx).value().to_string(),
            registered_address: self.address_input.read(cx).value().to_string(),
            zip_code: self.zip_select.read(cx).selected_value(cx),
            phone: self.tel_input.read(cx).value().to_string(),
            email: self.email_input.read(cx).value().to_string(),
            save_message: self.save_message.clone(),
            errors: self.errors.iter().map(|err| err.message.clone()).collect(),
        }
    }

    pub(crate) fn agent_apply_editor(
        &mut self,
        editor: &crate::agent::ProfileEditor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tin_input.update(cx, |tin, cx| {
            tin.set_text_value(&editor.tin, window, cx);
        });
        self.name_input.update(cx, |input, cx| {
            input.set_value(editor.full_name.clone(), window, cx);
        });
        self.line_of_business.update(cx, |input, cx| {
            input.set_value(editor.line_of_business.clone(), window, cx);
        });
        self.address_input.update(cx, |input, cx| {
            input.set_value(editor.registered_address.clone(), window, cx);
        });
        self.tel_input.update(cx, |input, cx| {
            input.set_value(editor.phone.clone(), window, cx);
        });
        self.email_input.update(cx, |input, cx| {
            input.set_value(editor.email.clone(), window, cx);
        });
        self.rdo_select.update(cx, |select, cx| {
            select.set_selected_value(&editor.rdo_code, window, cx);
        });
        self.zip_select.update(cx, |select, cx| {
            select.set_selected_value(&editor.zip_code, window, cx);
        });
    }

    pub(crate) fn agent_active_tab(&self) -> crate::agent::ids::ProfileManagerTab {
        crate::agent::ids::ProfileManagerTab::from_index(self.active_tab)
            .unwrap_or(crate::agent::ids::ProfileManagerTab::Tax)
    }

    pub(crate) fn agent_set_tab(&mut self, tab: crate::agent::ids::ProfileManagerTab) {
        let tab = if tab == crate::agent::ids::ProfileManagerTab::Cor {
            crate::agent::ids::ProfileManagerTab::Tax
        } else {
            tab
        };
        self.active_tab = tab.index();
    }

    /// Shows the reason cross-view navigation was refused. App-level callers
    /// can use this together with [`Self::has_unsaved_compliance_changes`]
    /// before opening a filing form or replacing the edited profile.
    pub fn notify_unsaved_compliance_blocked(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dirty_state = self.dirty_state();
        window.push_notification(
            Notification::error("Navigation blocked")
                .message(dirty_state.navigation_message())
                .title(dirty_state.title()),
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
        self.has_unsaved_forms_set_changes = false;
    }

    fn dirty_state(&self) -> dirty_state::ComplianceDirtyState {
        dirty_state::ComplianceDirtyState {
            profile: self.has_unsaved_profile_changes,
            forms_set: self.has_unsaved_forms_set_changes,
        }
    }

    fn profile_snapshot(&self, cx: &Context<Self>) -> serde_json::Value {
        let mut snapshot = serde_json::to_value(self.current_profile(cx))
            .expect("TaxpayerProfile must remain serializable for persistence");
        if let Some(fields) = snapshot.as_object_mut() {
            // Forms Sets have their own dirty state and user-facing copy.
            fields.remove("per_year_forms");
            fields.remove("profile_versions");
            // The election row is a pending editor value until Apply or Save
            // Profile consumes it. Tracking it prevents a selected election
            // from looking saved while the persisted ledger is still empty.
            fields.insert(
                "_tax_election_draft".to_string(),
                self.tax_election_draft_snapshot(cx),
            );
        }
        snapshot
    }

    fn tax_election_draft_snapshot(&self, cx: &Context<Self>) -> serde_json::Value {
        let election = self.tax_election_select.read(cx).selected_value(cx);
        if election.trim().is_empty() {
            return serde_json::Value::Null;
        }

        serde_json::json!({
            "taxable_year": self.tax_election_year_input.read(cx).value().to_string(),
            "election": election,
        })
    }

    fn forms_set_snapshot(
        forms: &std::collections::BTreeMap<u16, bir_core::forms::PerYearFormsSet>,
    ) -> std::collections::BTreeMap<u16, bir_core::forms::PerYearFormsSet> {
        let mut snapshot = forms.clone();
        for set in snapshot.values_mut() {
            // Entry order is presentation-only. Reconciliation and the editor can
            // produce the same authoritative set in different display order.
            set.entries
                .sort_by(|left, right| left.form_code.cmp(&right.form_code));
        }
        snapshot
    }

    fn capture_clean_baseline(&mut self, cx: &Context<Self>) {
        let snapshot = self.profile_snapshot(cx);
        self.clean_profile_snapshot = Some(snapshot);
        self.clean_forms_set_snapshot = Self::forms_set_snapshot(&self.stored_per_year_forms);
        self.clear_profile_changed();
    }

    fn refresh_dirty_state(&mut self, cx: &Context<Self>) {
        let Some(clean_profile) = self.clean_profile_snapshot.as_ref() else {
            return;
        };
        let current_profile = self.profile_snapshot(cx);
        let current_forms_set = Self::forms_set_snapshot(&self.stored_per_year_forms);
        let state = dirty_state::ComplianceDirtyState::from_comparison(
            clean_profile,
            &current_profile,
            &self.clean_forms_set_snapshot,
            &current_forms_set,
        );
        self.has_unsaved_profile_changes = state.profile;
        self.has_unsaved_forms_set_changes = state.forms_set;
    }

    fn discard_profile_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_dirty_state(cx);
        let discarded_state = self.dirty_state();
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
                discarded_state.discarded_message().to_string(),
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

    /// Whether this editor is currently open on `tin`.
    ///
    /// Lifecycle writes happen in `AppState`, which owns the database, while the
    /// editor holds its own cached copy of the profile. `AppState` uses this to
    /// find out whether the editor it is about to correct is even showing the
    /// profile that changed.
    pub fn is_editing_tin(&self, tin: &str, cx: &App) -> bool {
        self.editing_id.is_some() && self.tin_input.read(cx).value(cx) == tin
    }

    /// Adopts an archived flag written elsewhere.
    ///
    /// `stored_is_archived` is otherwise only set when a profile is loaded, and
    /// `current_profile` serialises it back into every save. Without this, using
    /// the editor's own Archive button left the cached flag stale, so the Save
    /// button a few pixels below it would write the pre-archive value straight
    /// back and silently un-archive the profile. Narrow on purpose: it touches
    /// only this flag, so unsaved edits elsewhere in the form survive.
    pub fn adopt_archived_state(&mut self, archived: bool, cx: &mut Context<Self>) {
        self.stored_is_archived = archived;
        cx.notify();
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
        self.stored_oauth_inbox_email = None;
        self.pending_notification = None;
        self.is_editing_password = true;
        self.stored_profile_pin_hash = None;
        self.stored_atc_codes.clear();
        self.stored_tax_elections = vec![];
        self.tax_election_select.update(cx, |select, cx| {
            select.set_selected_value("", window, cx);
        });
        self.stored_profile_years.clear();
        self.is_totp_enabled = false;
        self.show_totp_setup = false;
        self.show_totp_secret_text = false;
        self.totp_secret_temp = None;
        self.totp_qr_path = None;
        self.stored_totp_secret = None;
        self.stored_per_year_forms.clear();
        self.forms_editor_selected_code = None;
        let current_year = chrono::Local::now().date_naive().year();
        self.forms_editor_year = current_year as u16;
        self.forms_editor_year_select.update(cx, |select, cx| {
            select.set_selected_value(&current_year.to_string(), window, cx);
        });
        self.forms_editor_new_code_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.forms_editor_registry_form_select
            .update(cx, |select, cx| {
                select.set_selected_value("", window, cx);
            });
        self.forms_editor_custom_code_mode = false;
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
        self.calendar_form_selection = Default::default();
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
        self.birth_date_input
            .update(cx, |input, cx| input.set_date(None, window, cx));
        self.refresh_profile_year_selector(window, cx);
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
        self.profile_session_epoch = self.profile_session_epoch.wrapping_add(1);
        self.queued_profile_save = None;
        self.capture_clean_baseline(cx);
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
        mut profile: TaxpayerProfile,
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
        self.tax_election_select.update(cx, |select, cx| {
            select.set_selected_value("", window, cx);
        });
        self.stored_profile_years = profile.profile_years.clone();
        if let Some(facts) = self
            .stored_profile_years
            .get(&self.forms_editor_year)
            .cloned()
        {
            facts.apply_to(&mut profile);
        }
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

        self.oauth_connected = profile.has_usable_oauth_refresh();
        if !self.oauth_connected
            && let Ok(db) = self.db.lock()
            && let Ok(Some(tokens)) = db.inbox_oauth_tokens(profile.inbox_email())
            && tokens.has_usable_refresh()
        {
            self.oauth_connected = true;
            self.stored_oauth_access_token = Some(tokens.access_token.clone());
            self.stored_oauth_refresh_token = Some(tokens.refresh_token.clone());
        }
        self.stored_oauth_inbox_email = if self.oauth_connected {
            Some(profile.inbox_email().to_string())
        } else {
            None
        };

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
        self.refresh_profile_year_selector(window, cx);

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
        self.calendar_form_selection = self
            .db
            .lock()
            .map(|db| bir_core::google_calendar::calendar_form_selection(&db, &profile.tin.full()))
            .unwrap_or_default();
        let select_val = self.forms_editor_year_select.read(cx).selected_value(cx);
        if let Ok(y) = select_val.parse::<u16>() {
            self.forms_editor_year = y;
        }
        self.forms_editor_selected_code = None;

        self.profile_change_revision = 0;
        self.profile_session_epoch = self.profile_session_epoch.wrapping_add(1);
        self.queued_profile_save = None;
        self.capture_clean_baseline(cx);
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

        let zip_val = if profile.zip_code.trim().is_empty() {
            String::new()
        } else {
            self.zip_options
                .iter()
                .find(|o| o.starts_with(&profile.zip_code))
                .cloned()
                .unwrap_or_else(|| profile.zip_code.clone())
        };
        self.zip_select.update(cx, |select, cx| {
            select.set_selected_value(&zip_val, window, cx)
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

        let rdo_value = if profile.rdo_code.trim().is_empty() {
            String::new()
        } else {
            self.rdo_options
                .iter()
                .find(|o| o.starts_with(&profile.rdo_code))
                .cloned()
                .unwrap_or_else(|| profile.rdo_code.clone())
        };
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
            if let Ok(Some(existing)) = db.get_profile(&tin_val) {
                self.tin_duplicate_error = Some(duplicate_tin_message(
                    &self.tin_input.read(cx).formatted_value(cx),
                    existing.is_archived,
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
            }
            if field_to_validate.is_some() {
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
                // Year switching is handled by subscribe_in so the editor
                // can load that year's clone with a Window.
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

    fn on_business_start_date_event(
        &mut self,
        _state: &Entity<DateInputState>,
        _event: &DateInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refresh_profile_year_selector(window, cx);
        self.mark_profile_changed();
        cx.notify();
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

    fn on_tax_election_select_event(
        &mut self,
        _state: Entity<ComboboxState>,
        event: &ComboboxEvent,
        cx: &mut Context<Self>,
    ) {
        if event.selected.is_some() {
            self.mark_profile_changed();
            cx.notify();
        }
    }

    fn on_tax_election_year_event(
        &mut self,
        _state: &Entity<InputState>,
        event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change)
            && !self
                .tax_election_select
                .read(cx)
                .selected_value(cx)
                .trim()
                .is_empty()
        {
            self.mark_profile_changed();
            cx.notify();
        }
    }

    fn apply_pending_tax_election(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<bool, String> {
        let election_label = self.tax_election_select.read(cx).selected_value(cx);
        let Some(election) = income_tax_election_from_label(&election_label)? else {
            return Ok(false);
        };
        let year_text = self
            .tax_election_year_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let year = year_text.parse::<u16>().map_err(|_| {
            "Enter a valid four-digit taxable year for the income-tax election.".to_string()
        })?;
        if !(1900..=2200).contains(&year) {
            return Err(
                "Enter a taxable year from 1900 through 2200 for the income-tax election."
                    .to_string(),
            );
        }
        if !self
            .current_profile(cx)
            .eligible_for_income_tax_election_in_year(year)
        {
            return Err(format!(
                "No {year} profile-year clone is Individual Self-Employed or Mixed Income, so an income-tax election cannot be recorded for it. Create or edit that year on the Tax Profile tab first."
            ));
        }

        upsert_income_tax_election(
            &mut self.stored_tax_elections,
            bir_core::profile::TaxElectionHistory {
                taxable_year: year,
                election,
                elected_at: chrono::Local::now().naive_local(),
                source_form: "profile_manager".to_string(),
            },
        );
        self.tax_election_select.update(cx, |select, cx| {
            select.set_selected_value("", window, cx);
        });
        self.mark_profile_changed();
        cx.notify();
        Ok(true)
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
            birth_date,
            email_tracking_enabled: self.email_tracking_enabled,

            email_auth_method: self.email_auth_method.clone(),
            imap_email: {
                if matches!(self.email_auth_method, EmailAuthMethod::GoogleOAuth)
                    && let Some(email) = self
                        .stored_oauth_inbox_email
                        .as_deref()
                        .map(str::trim)
                        .filter(|email| !email.is_empty())
                {
                    Some(email.to_string())
                } else {
                    let val = self.imap_email_input.read(cx).value().trim().to_string();
                    if val.is_empty() { None } else { Some(val) }
                }
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
            profile_versions: Vec::new(),
            compliance_source_mode: ComplianceSourceMode::TemporalSuggestion,
            per_year_forms: self.stored_per_year_forms.clone(),
            profile_years: self.stored_profile_years.clone(),
        };
        let _ = profile.capture_current_as_year(self.forms_editor_year);
        profile
    }

    fn switch_profile_year(&mut self, year: u16, window: &mut Window, cx: &mut Context<Self>) {
        if year == self.forms_editor_year {
            return;
        }
        if let Err(message) = self.current_profile(cx).profile_year_allowed(year) {
            self.pending_notification = Some((
                gpui_component::notification::NotificationType::Error,
                message,
            ));
            self.forms_editor_year_select.update(cx, |select, cx| {
                select.set_selected_value(&self.forms_editor_year.to_string(), window, cx);
            });
            cx.notify();
            return;
        }
        let snapshot = self.current_profile(cx);
        self.stored_profile_years = snapshot.profile_years.clone();
        self.forms_editor_year = year;
        self.forms_editor_selected_code = None;
        let mut projected = snapshot;
        if let Some(facts) = self.stored_profile_years.get(&year).cloned() {
            facts.apply_to(&mut projected);
        } else {
            projected = projected.tin_only_projection();
        }
        self.sync_projection_to_ui(&projected, window, cx);
        self.mark_profile_changed();
        cx.notify();
    }

    fn existing_profile_years(&self) -> Vec<u16> {
        let mut years: std::collections::BTreeSet<u16> =
            self.stored_profile_years.keys().copied().collect();
        years.insert(self.forms_editor_year);
        years.into_iter().collect()
    }

    fn unused_add_years(&self, cx: &App) -> Vec<u16> {
        let current_year = chrono::Local::now().date_naive().year();
        let business_start = self.business_start_input.read(cx).date;
        unused_profile_years(business_start, current_year, self.existing_profile_years())
    }

    fn add_profile_year(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self
            .forms_editor_year_add_select
            .read(cx)
            .selected_value(cx);
        let Ok(year) = selected.parse::<u16>() else {
            self.pending_notification = Some((
                gpui_component::notification::NotificationType::Warning,
                "Pick a year to add.".to_string(),
            ));
            cx.notify();
            return;
        };
        let current_year =
            u16::try_from(chrono::Local::now().date_naive().year()).unwrap_or(u16::MAX);
        if year > current_year {
            self.pending_notification = Some((
                gpui_component::notification::NotificationType::Error,
                format!("year {year} is after the current year ({current_year})"),
            ));
            cx.notify();
            return;
        }
        let mut snapshot = self.current_profile(cx);
        if let Err(message) = snapshot.profile_year_allowed(year) {
            self.pending_notification = Some((
                gpui_component::notification::NotificationType::Error,
                message,
            ));
            cx.notify();
            return;
        }
        snapshot.profile_years.entry(year).or_default();
        self.stored_profile_years = snapshot.profile_years.clone();
        if year != self.forms_editor_year {
            self.switch_profile_year(year, window, cx);
        } else {
            self.mark_profile_changed();
        }
        self.refresh_profile_year_selector(window, cx);
        self.pending_notification = Some((
            gpui_component::notification::NotificationType::Success,
            format!("Added {year}."),
        ));
        cx.notify();
    }

    fn refresh_profile_year_selector(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let existing = self.existing_profile_years();
        let unused = self.unused_add_years(cx);
        let selected = if existing.contains(&self.forms_editor_year) {
            self.forms_editor_year
        } else {
            *existing.last().unwrap_or(&self.forms_editor_year)
        };
        let existing_labels: Vec<String> = existing.iter().map(u16::to_string).collect();
        self.forms_editor_year_select.update(cx, |select, cx| {
            select.set_options(existing_labels, cx);
            select.set_selected_value(&selected.to_string(), window, cx);
        });
        let add_default = unused.last().copied();
        let unused_labels: Vec<String> = unused.iter().map(u16::to_string).collect();
        self.forms_editor_year_add_select.update(cx, |select, cx| {
            select.set_options(unused_labels, cx);
            if let Some(year) = add_default {
                select.set_selected_value(&year.to_string(), window, cx);
            } else {
                select.set_selected_value("", window, cx);
            }
        });
        if selected != self.forms_editor_year {
            self.switch_profile_year(selected, window, cx);
        }
    }

    fn save_profile(&mut self, cx: &mut Context<Self>) {
        self.save_profile_inner(cx);
    }

    fn save_all_profile_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(message) = self.apply_pending_tax_election(window, cx) {
            self.save_message = Some(message.clone());
            window.push_notification(
                Notification::error(message).title("Income-tax election needs review"),
                cx,
            );
            cx.notify();
            return;
        }

        self.save_profile(cx);
    }

    fn save_profile_inner(&mut self, cx: &mut Context<Self>) {
        if !self
            .tax_election_select
            .read(cx)
            .selected_value(cx)
            .trim()
            .is_empty()
        {
            self.save_message = Some(
                "Apply the pending income-tax election or use Save Profile so it is included in the saved ledger."
                    .to_string(),
            );
            cx.notify();
            return;
        }
        let profile = self.current_profile(cx);
        self.errors = validate_profile(&profile);

        // Gate: Duplicate TIN check (defense in depth — also checked reactively)
        if self.editing_id.is_none() {
            let tin_str = profile.tin.full();
            if let Ok(db) = self.db.lock()
                && let Ok(Some(existing)) = db.get_profile(&tin_str)
            {
                self.tin_duplicate_error = Some(duplicate_tin_message(
                    &profile.tin.formatted(),
                    existing.is_archived,
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
                "Date must use MM/DD/YYYY",
            ));
        }

        if profile.taxpayer_type == TaxpayerType::Individual
            && self.birth_date_input.read(cx).has_invalid_value(cx)
        {
            self.errors.push(ValidationError::new(
                "birth_date",
                "Date must use MM/DD/YYYY",
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

        let save_request = ProfileSaveRequest {
            profile_session_epoch: self.profile_session_epoch,
            profile_change_revision: self.profile_change_revision,
        };
        match profile_save_dispatch_action(self.active_profile_save.as_ref(), &save_request) {
            ProfileSaveDispatchAction::DuplicateInFlight => {
                self.save_message = Some("Saving...".to_string());
                cx.notify();
                return;
            }
            ProfileSaveDispatchAction::QueueBehindInFlight => {
                self.queued_profile_save = Some(save_request);
                self.save_message = Some("Saving latest changes next...".to_string());
                cx.notify();
                return;
            }
            ProfileSaveDispatchAction::Dispatch => {
                self.active_profile_save = Some(save_request.clone());
                self.queued_profile_save = None;
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

        let save_revision = save_request.profile_change_revision;
        let save_epoch = save_request.profile_session_epoch;
        self.last_save_dispatch_revision = save_revision;
        self.saves_in_flight = self.saves_in_flight.saturating_add(1);
        let db_arc_clone = db_arc.clone();
        cx.spawn(async move |this, cx| {
            let is_email_tracking_active = profile.is_email_tracking_active();
            let save_result = cx
                .background_executor()
                .spawn(async move {
                    if let Ok(db) = db_arc.lock() {
                        db.save_profile_with_post_commit_status(profile)
                            .map(bir_core::db::PostCommitWrite::into_parts)
                            .map_err(|e| e.to_string())
                    } else {
                        Err("Database lock is poisoned".to_string())
                    }
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.saves_in_flight = this.saves_in_flight.saturating_sub(1);
                this.active_profile_save = None;
                let has_queued_save = this.queued_profile_save.as_ref().is_some_and(|request| {
                    request.profile_session_epoch == this.profile_session_epoch
                });
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
                        // A completion may only touch view-local state while
                        // the view still shows the profile it saved. After a
                        // profile switch (or New Profile), the revision
                        // counter restarts, so it cannot detect this on its
                        // own — re-pointing editing_id/persisted_profile_tin
                        // here would make the next save UPDATE the old
                        // profile's row with the new profile's data.
                        if save_completion_matches_profile_session(
                            save_epoch,
                            this.profile_session_epoch,
                        ) {
                            this.editing_id = Some(saved_id);
                            this.persisted_profile_tin = Some(tin_val.clone());
                            // A completion is stale when the profile changed
                            // after this save was dispatched. Adopting the
                            // older database response would overwrite those
                            // newer in-memory edits; the follow-up save
                            // persists and baselines them instead.
                            if save_completion_is_current(
                                save_revision,
                                this.profile_change_revision,
                            ) {
                                this.stored_atc_codes.clone_from(&saved.atc_codes);
                                this.stored_tax_elections.clone_from(&saved.tax_elections);
                                this.stored_per_year_forms.clone_from(&saved.per_year_forms);
                                this.capture_clean_baseline(cx);
                            }
                            // Only the most recently dispatched save may clear
                            // the "Saving..." indicator and announce success;
                            // an older completion must not claim edits that a
                            // still-in-flight save is responsible for.
                            if this.last_save_dispatch_revision == save_revision
                                && !has_queued_save
                            {
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
                            }
                        }

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
                                                    &sum.tin,
                                                    sum.taxable_year,
                                                    sum.month,
                                                    sum.quarter,
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
                        if has_queued_save {
                            tracing::warn!(
                                %err,
                                "An earlier profile save failed; dispatching the queued latest state"
                            );
                            this.save_message = Some("Saving latest changes next...".to_string());
                        } else {
                            this.save_message = None;
                            this.pending_notification = Some((
                                gpui_component::notification::NotificationType::Error,
                                format!("Save failed: {err}"),
                            ));
                        }
                    }
                }
                let queued_save = this.queued_profile_save.take().filter(|request| {
                    request.profile_session_epoch == this.profile_session_epoch
                });
                if let Some(queued_save) = queued_save {
                    this.save_profile_inner(cx);
                } else {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn field_label(text: &str, cx: &Context<Self>) -> Div {
        crate::components::form_parts::field_label(text, cx)
    }

    fn render_unsaved_profile_banner(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let dirty_state = self.dirty_state();
        if !dirty_state.any() {
            return div().into_any_element();
        }

        // The message is one long sentence. The row used to be a single
        // unwrapped line, so the text claimed its whole natural width, the row
        // overflowed, and Discard / Save Changes were pushed outside the
        // banner — the actions the message asks for became unreachable.
        //
        // The text column keeps its natural width as its flex basis, so while
        // message and buttons both fit they share one line; when they do not,
        // the line breaks, the message wraps in the full width of the banner
        // and the buttons take a row of their own underneath. The buttons
        // never shrink, and `ml_auto` keeps them at the right edge on whichever
        // row they land.
        let root = rsx! {
            <div
                flex
                flex_wrap
                w_full
                items_center
                gap_3
                px_4
                py_3
                rounded_lg
                border_1
                border_color={cx.theme().warning.opacity(0.45)}
                bg={cx.theme().warning.opacity(0.12)}
            >
                <div flex flex_col gap_1 min_w_0>
                    <div
                        text_sm
                        font_weight={FontWeight::BOLD}
                        text_color={crate::theme::warning_on_tint(cx.theme())}
                    >
                        {dirty_state.title()}
                    </div>
                    <div
                        text_xs
                        text_color={crate::theme::warning_on_tint(cx.theme())}
                    >
                        {dirty_state.navigation_message()}
                    </div>
                </div>
                <div flex items_center gap_2 flex_none ml_auto>
                    {gpui_component::button::Button::new("discard_profile_changes")
                        .label("Discard")
                        .ghost()
                        .on_click(cx.listener(|this, _event, window, cx| {
                            this.discard_profile_changes(window, cx);
                        }))}
                    {gpui_component::button::Button::new("save_profile_changes")
                        .label("Save Changes")
                        .on_click(cx.listener(|this, _event, window, cx| {
                            this.save_all_profile_changes(window, cx);
                        }))}
                </div>
            </div>
        };
        root.into_any_element()
    }

    fn field_error(&self, field: &'static str, _cx: &Context<Self>) -> gpui::Div {
        let text = self
            .errors
            .iter()
            .find(|err| err.field == field)
            .map(|err| err.message.clone())
            .unwrap_or_default();
        rsx! {
            <div min_h_5 text_xs text_color={crate::theme::danger_on_tint(_cx.theme())}>
                {text}
            </div>
        }
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
        // Programmatic InputState/Combobox population emits the same change
        // events as user edits. Reconcile against the loaded baseline before
        // deciding whether to show or enforce the unsaved-changes guard.
        self.refresh_dirty_state(cx);
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
        let profile_calendar_available = self
            .db
            .lock()
            .ok()
            .map(|db| {
                bir_core::google_calendar::google_calendar_connection_from_db(&db)
                    .profile_calendar_available()
            })
            .unwrap_or(false);
        if self.active_tab == 1 {
            self.active_tab = 0;
        }
        if self.active_tab == 6 && !profile_calendar_available {
            self.active_tab = 0;
        }

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
                cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                    let is_enter = event.keystroke.key == "enter";
                    let is_modifier =
                        event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
                    if is_enter && is_modifier {
                        this.save_all_profile_changes(window, cx);
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
                            .max_w(px(960.))
                            .gap_6()
                            .child(rsx! {
                                <div flex flex_col gap_2>
                                    <div flex items_center justify_between gap_3>
                                        <div
                                            text_3xl
                                            font_weight={FontWeight::BLACK}
                                            text_color={cx.theme().foreground}
                                        >
                                            {title}
                                        </div>
                                        <div flex items_center gap_2 flex_wrap>
                                            <div flex items_center gap_2 id={crate::agent::ids::PROFILE_YEAR_SELECT}>
                                                <div text_xs font_weight={FontWeight::BOLD} text_color={cx.theme().muted_foreground}>{"Year"}</div>
                                                <div w={px(100.)}>{Combobox::new(&self.forms_editor_year_select)}</div>
                                            </div>
                                            {if !self.unused_add_years(cx).is_empty() {
                                                rsx! {
                                                    <div flex items_center gap_2 id={crate::agent::ids::PROFILE_YEAR_ADD_SELECT}>
                                                        <div text_xs font_weight={FontWeight::BOLD} text_color={cx.theme().muted_foreground}>{"Add"}</div>
                                                        <div w={px(100.)}>{Combobox::new(&self.forms_editor_year_add_select)}</div>
                                                        {gpui_component::button::Button::new(crate::agent::ids::PROFILE_YEAR_ADD)
                                                            .label("Add year")
                                                            .small()
                                                            .on_click(cx.listener(|this, _, window, cx| {
                                                                this.add_profile_year(window, cx);
                                                            }))}
                                                    </div>
                                                }
                                                .into_any_element()
                                            } else {
                                                div().into_any_element()
                                            }}
                                        </div>
                                    </div>
                                    <div text_sm text_color={cx.theme().muted_foreground}>
                                        {"Forms use this year's tax profile."}
                                    </div>
                                </div>
                            })
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
                                            .child(rsx! {
                                                <div
                                                    id={crate::agent::ids::PROFILE_TAB_TAX}
                                                    px_4
                                                    py_1p5
                                                    rounded_md
                                                    cursor_pointer
                                                    when={(self.active_tab == 0, |s| {
                                                        s.bg(cx.theme().background)
                                                            .shadow_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                    })}
                                                    when={(self.active_tab != 0, |s| {
                                                        s.hover(|s| s.bg(cx.theme().muted))
                                                            .text_color(cx.theme().muted_foreground)
                                                            .font_weight(FontWeight::MEDIUM)
                                                    })}
                                                    on_click={cx.listener(|this, _, _, cx| {
                                                        this.active_tab = 0;
                                                        cx.notify();
                                                    })}
                                                >
                                                    <div text_sm>{"Tax Profile"}</div>
                                                </div>
                                            })
                                            .child(rsx! {
                                                <div
                                                    id={crate::agent::ids::PROFILE_TAB_EMAIL}
                                                    px_4
                                                    py_1p5
                                                    rounded_md
                                                    cursor_pointer
                                                    when={(self.active_tab == 2, |s| {
                                                        s.bg(cx.theme().background)
                                                            .shadow_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                    })}
                                                    when={(self.active_tab != 2, |s| {
                                                        s.hover(|s| s.bg(cx.theme().muted))
                                                            .text_color(cx.theme().muted_foreground)
                                                            .font_weight(FontWeight::MEDIUM)
                                                    })}
                                                    on_click={cx.listener(|this, _, _, cx| {
                                                        this.active_tab = 2;
                                                        cx.notify();
                                                    })}
                                                >
                                                    <div text_sm>{"Email Settings"}</div>
                                                </div>
                                            })
                                            .when(global_pins_enabled, |this| {
                                                this.child(rsx! {
                                                    <div
                                                        id={crate::agent::ids::PROFILE_TAB_SECURITY}
                                                        px_4
                                                        py_1p5
                                                        rounded_md
                                                        cursor_pointer
                                                        when={(self.active_tab == 3, |s| {
                                                            s.bg(cx.theme().background)
                                                                .shadow_sm()
                                                                .text_color(cx.theme().foreground)
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                        })}
                                                        when={(self.active_tab != 3, |s| {
                                                            s.hover(|s| s.bg(cx.theme().muted))
                                                                .text_color(cx.theme().muted_foreground)
                                                                .font_weight(FontWeight::MEDIUM)
                                                        })}
                                                        on_click={cx.listener(|this, _, _, cx| {
                                                            this.active_tab = 3;
                                                            cx.notify();
                                                        })}
                                                    >
                                                        <div text_sm>{"Security"}</div>
                                                    </div>
                                                })
                                            })
                                            .when(self.editing_id.is_some(), |this| {
                                                this.child(rsx! {
                                                    <div
                                                        id={crate::agent::ids::PROFILE_TAB_EXPORT}
                                                        px_4
                                                        py_1p5
                                                        rounded_md
                                                        cursor_pointer
                                                        when={(self.active_tab == 4, |s| {
                                                            s.bg(cx.theme().background)
                                                                .shadow_sm()
                                                                .text_color(cx.theme().foreground)
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                        })}
                                                        when={(self.active_tab != 4, |s| {
                                                            s.hover(|s| s.bg(cx.theme().muted))
                                                                .text_color(cx.theme().muted_foreground)
                                                                .font_weight(FontWeight::MEDIUM)
                                                        })}
                                                        on_click={cx.listener(|this, _, _, cx| {
                                                            this.active_tab = 4;
                                                            cx.notify();
                                                        })}
                                                    >
                                                        <div text_sm>{"Export"}</div>
                                                    </div>
                                                })
                                                .when(profile_calendar_available, |this| {
                                                    this.child(rsx! {
                                                        <div
                                                            id={crate::agent::ids::PROFILE_TAB_CALENDAR}
                                                            px_4
                                                            py_1p5
                                                            rounded_md
                                                            cursor_pointer
                                                            when={(self.active_tab == 6, |s| {
                                                                s.bg(cx.theme().background)
                                                                    .shadow_sm()
                                                                    .text_color(cx.theme().foreground)
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                            })}
                                                            when={(self.active_tab != 6, |s| {
                                                                s.hover(|s| s.bg(cx.theme().muted))
                                                                    .text_color(cx.theme().muted_foreground)
                                                                    .font_weight(FontWeight::MEDIUM)
                                                            })}
                                                            on_click={cx.listener(|this, _, _, cx| {
                                                                this.active_tab = 6;
                                                                cx.notify();
                                                            })}
                                                        >
                                                            <div text_sm>{"Calendar"}</div>
                                                        </div>
                                                    })
                                                })
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .w_full()
                                    .child(
                                        div()
                                            .id(crate::agent::ids::PROFILE_SECTION_TAX)
                                            .child(self.render_tax_profile_tab(
                                                is_individual,
                                                is_cooperative,
                                                date_label,
                                                cx,
                                            )),
                                    )
                                    .child(
                                        div()
                                            .id(crate::agent::ids::PROFILE_SECTION_EMAIL)
                                            .child(self.render_email_settings_tab(cx)),
                                    )
                                    .child(
                                        div()
                                            .id(crate::agent::ids::PROFILE_SECTION_SECURITY)
                                            .child(self.render_security_tab(global_pins_enabled, cx)),
                                    )
                                    .child(
                                        div()
                                            .id(crate::agent::ids::PROFILE_SECTION_EXPORT)
                                            .child(self.render_export_tab(cx)),
                                    )
                                    .when(profile_calendar_available, |this| {
                                        this.child(
                                            div()
                                                .id(crate::agent::ids::PROFILE_SECTION_CALENDAR)
                                                .child(self.render_calendar_tab(cx)),
                                        )
                                    })
                            )
                            .when(self.active_tab != 6, |this| {
                                this.child(rsx! {
                                    <div mt_4 pb={px(80.)} flex items_center gap_4>
                                        {gpui_component::button::Button::new("save_profile")
                                            .label("Save Profile")
                                            .on_click(cx.listener(|this, _ev, window, cx| {
                                                this.save_all_profile_changes(window, cx);
                                            }))}
                                        <div text_sm text_color={cx.theme().muted_foreground}>
                                            {self.save_message.clone().unwrap_or_default()}
                                        </div>
                                    </div>
                                })
                            })
                    )
            )
            )
            .when(self.show_totp_setup, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .occlude()
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
                                .child(rsx! {
                                    <div text_lg font_weight={FontWeight::BOLD}>
                                        {"Connect your authenticator app"}
                                    </div>
                                })
                                .child(
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
                                                    <div
                                                        id="trouble_scanning_btn"
                                                        text_sm
                                                        text_color={cx.theme().primary}
                                                        cursor_pointer
                                                        on_click={cx.listener(|this, _ev, _window, cx| {
                                                            this.show_totp_secret_text = true;
                                                            cx.notify();
                                                        })}
                                                    >
                                                        {"Trouble scanning?"}
                                                    </div>
                                                </div>
                                            })
                                        })
                                        .when(self.show_totp_secret_text, |this| {
                                            this.when_some(self.totp_secret_temp.clone(), |this, secret| {
                                                this.child(rsx! {
                                                    <div flex flex_col items_center gap_4 mt_2>
                                                        <div
                                                            flex
                                                            items_center
                                                            gap_2
                                                            p_3
                                                            rounded_md
                                                            bg={cx.theme().secondary}
                                                            border_1
                                                            border_color={cx.theme().border}
                                                        >
                                                            <div text_sm font_family={".SF NS Mono"}>
                                                                {secret.clone()}
                                                            </div>
                                                            {gpui_component::clipboard::Clipboard::new("totp-secret-clipboard-profile")
                                                                .value(secret)}
                                                        </div>
                                                        <div
                                                            id="show_qr_btn"
                                                            text_sm
                                                            text_color={cx.theme().primary}
                                                            cursor_pointer
                                                            on_click={cx.listener(|this, _ev, _window, cx| {
                                                                this.show_totp_secret_text = false;
                                                                cx.notify();
                                                            })}
                                                        >
                                                            {"Show QR code instead"}
                                                        </div>
                                                    </div>
                                                })
                                            })
                                        })
                                )
                                .child(rsx! {
                                    <div flex flex_col gap_1 mt_2>
                                        <div text_sm font_weight={FontWeight::MEDIUM}>
                                            {"Step 2: Enter the 6-digit code to verify"}
                                        </div>
                                        {OtpInput::new(&self.setup_totp_state).groups(1).large()}
                                    </div>
                                })
                                .child(rsx! {
                                    <div flex justify_end mt_4>
                                        {gpui_component::button::Button::new("cancel_totp_profile_btn")
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
                                            }))}
                                    </div>
                                })
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

/// Whether an async save completion may adopt the persisted profile state
/// (elections, COR versions, Forms Sets) and re-capture the clean baseline.
///
/// `save_revision` is the profile change revision captured when the save was
/// dispatched; `current_revision` is the live revision when its completion
/// arrives. Any edit in between bumps the revision, so a mismatch means the
/// database response reflects an older profile and must not replace the newer
/// in-memory edits.
fn save_completion_is_current(save_revision: u64, current_revision: u64) -> bool {
    save_revision == current_revision
}

/// Whether an async save completion still belongs to the profile the view is
/// showing. `edit_profile` and `reset_for_new` reset the change revision to
/// zero, so two different profiles can produce colliding revision numbers;
/// the session epoch is bumped on every such transition and never resets,
/// which makes cross-profile completions detectable.
fn save_completion_matches_profile_session(save_epoch: u64, current_epoch: u64) -> bool {
    save_epoch == current_epoch
}

#[cfg(test)]
mod duplicate_tin_message_tests {
    use super::duplicate_tin_message;

    // The uniqueness check sees every profile, but the sidebar list does not:
    // it is filtered by the archive toggle and by `hide_tax_profiles`. So the
    // message has to explain why the offending profile may be invisible,
    // otherwise the user is told a TIN is taken by something they cannot find.
    #[test]
    fn archived_profile_points_at_the_archive_toggle() {
        let msg = duplicate_tin_message("000-000-000-00000", true);
        assert!(msg.contains("000-000-000-00000"), "{msg}");
        assert!(msg.contains("archived"), "{msg}");
        assert!(msg.contains("archive toggle"), "{msg}");
    }

    #[test]
    fn visible_or_hidden_profile_points_at_the_sidebar_search() {
        let msg = duplicate_tin_message("123-456-789-00000", false);
        assert!(msg.contains("123-456-789-00000"), "{msg}");
        assert!(msg.contains("search that TIN"), "{msg}");
        assert!(
            !msg.contains("archived"),
            "must not claim a live profile is archived: {msg}"
        );
    }

    #[test]
    fn both_variants_keep_the_uniqueness_explanation() {
        for archived in [true, false] {
            let msg = duplicate_tin_message("000-000-000-00000", archived);
            assert!(msg.contains("already exists"), "{msg}");
            assert!(msg.contains("Each TIN must be unique"), "{msg}");
        }
    }
}

#[cfg(test)]
mod save_revision_tests {
    use super::{
        ProfileSaveDispatchAction, ProfileSaveRequest, income_tax_election_from_label,
        profile_save_dispatch_action, save_completion_is_current,
        save_completion_matches_profile_session, upsert_income_tax_election,
    };

    fn request(epoch: u64, revision: u64) -> ProfileSaveRequest {
        ProfileSaveRequest {
            profile_session_epoch: epoch,
            profile_change_revision: revision,
        }
    }

    #[test]
    fn distinct_save_requests_are_serialized_behind_the_active_write() {
        let active = request(7, 10);
        let later = request(7, 11);

        assert_eq!(
            profile_save_dispatch_action(None, &active),
            ProfileSaveDispatchAction::Dispatch
        );
        assert_eq!(
            profile_save_dispatch_action(Some(&active), &active),
            ProfileSaveDispatchAction::DuplicateInFlight
        );
        assert_eq!(
            profile_save_dispatch_action(Some(&active), &later),
            ProfileSaveDispatchAction::QueueBehindInFlight
        );
    }

    #[test]
    fn a_new_profile_session_cannot_be_mistaken_for_the_active_save() {
        let profile_a = request(7, 0);
        let profile_b = request(8, 0);

        assert_eq!(
            profile_save_dispatch_action(Some(&profile_a), &profile_b),
            ProfileSaveDispatchAction::QueueBehindInFlight
        );
    }

    #[test]
    fn pending_graduated_osd_label_maps_to_the_persisted_election_variant() {
        let election = income_tax_election_from_label("Graduated + OSD")
            .unwrap()
            .expect("a selected label should produce an election");

        assert_eq!(election, bir_core::profile::IncomeTaxElection::GraduatedOsd);
    }

    #[test]
    fn upsert_income_tax_election_replaces_the_same_year_without_duplicates() {
        let elected_at = chrono::NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let mut elections = vec![bir_core::profile::TaxElectionHistory {
            taxable_year: 2026,
            election: bir_core::profile::IncomeTaxElection::EightPercent,
            elected_at,
            source_form: "existing".to_string(),
        }];

        upsert_income_tax_election(
            &mut elections,
            bir_core::profile::TaxElectionHistory {
                taxable_year: 2026,
                election: bir_core::profile::IncomeTaxElection::GraduatedOsd,
                elected_at,
                source_form: "profile_manager".to_string(),
            },
        );

        assert_eq!(
            (elections.len(), &elections[0].election),
            (1, &bir_core::profile::IncomeTaxElection::GraduatedOsd)
        );
    }

    /// Switching profiles resets the change revision, so a save dispatched on
    /// profile A can collide with profile B's counter (both zero after a
    /// clean load). The session epoch — bumped on every edit_profile /
    /// reset_for_new — is what must invalidate the completion, otherwise A's
    /// completion re-points editing_id/persisted_profile_tin at A while B's
    /// data fills the form, and the next save corrupts A's database row.
    #[test]
    fn cross_profile_completion_is_rejected_by_epoch_despite_colliding_revisions() {
        let mut epoch: u64 = 7;
        let mut revision: u64 = 0;

        // Save dispatched on profile A with zero pending edits.
        let save_epoch = epoch;
        let save_revision = revision;

        // User opens profile B while the save is in flight: revision resets
        // to zero (colliding with the dispatched save), epoch bumps.
        revision = 0;
        epoch = epoch.wrapping_add(1);

        // The revision gate alone would wrongly accept the stale completion…
        assert!(save_completion_is_current(save_revision, revision));
        // …the epoch gate is what rejects it.
        assert!(!save_completion_matches_profile_session(save_epoch, epoch));

        // A save dispatched on profile B itself is accepted.
        assert!(save_completion_matches_profile_session(epoch, epoch));
    }

    /// Two saves dispatched at different revisions can complete in any order.
    /// Only the completion whose captured revision still matches the live
    /// revision may adopt persisted state; the stale one must be ignored no
    /// matter when it arrives.
    #[test]
    fn out_of_order_save_completions_never_adopt_stale_state() {
        let mut revision: u64 = 0;

        // First save dispatched, then the user keeps editing (for example
        // archiving a COR version), then a second save is dispatched.
        let first_save = revision;
        revision = revision.wrapping_add(1); // mark_profile_changed()
        let second_save = revision;

        // The newer save completes first: it is current and may adopt state.
        assert!(save_completion_is_current(second_save, revision));
        // The older save completes afterwards: it must be ignored, otherwise
        // it would restore the pre-edit state over the newer edits.
        assert!(!save_completion_is_current(first_save, revision));
    }

    #[test]
    fn completion_goes_stale_once_the_profile_is_edited_again() {
        let mut revision: u64 = 41;
        let save = revision;
        assert!(save_completion_is_current(save, revision));

        revision = revision.wrapping_add(1);
        assert!(!save_completion_is_current(save, revision));
    }

    #[test]
    fn revision_wraparound_keeps_staleness_detection_intact() {
        let mut revision: u64 = u64::MAX;
        let stale_save = revision;
        revision = revision.wrapping_add(1); // wraps to zero

        assert!(!save_completion_is_current(stale_save, revision));
        assert!(save_completion_is_current(revision, revision));
    }
}
