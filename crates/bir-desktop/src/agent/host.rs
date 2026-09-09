//! Send `AgentHost` for headless tests and as the semantic snapshot source.
//!
//! Desktop GPUI is not `Send`, so the mailbox drain copies into this type,
//! dispatches, then applies the result back on the UI thread. Headless tests
//! use this host directly with an ephemeral SQLite database.

use std::sync::{Arc, Mutex};

use bir_core::calendar_rules::DeadlineResolver;
use bir_core::db::Database;
use bir_core::forms::form_1601c::Form1601CDraft;
use bir_core::forms::{
    FilingStatus, FormSetSource, FormValidator, PerYearFormsSet, can_queue_for_submission,
};
use bir_core::naming::Tin;
use bir_core::profile::TaxpayerProfile;
use bir_core::validation::validate_profile;
use chrono::Datelike;
use gpui_agent::dispatch::DispatchResult;
use gpui_agent::host::AgentHost;
use gpui_agent::protocol::{DeliveryMode, HelloInfo, Op, PROTOCOL_VERSION, PlatformKind};
use gpui_agent::tree::{UiNode, UiTree};
use gpui_agent::virtual_unavailable;

use crate::agent::ProfileEditor;
use crate::agent::ids;

use crate::app::ActiveView;

const FIXTURE_TIN: &str = "12345678900000";
const FIXTURE_NAME: &str = "Agent Fixture Taxpayer";

#[derive(Debug, Clone)]
struct ListedProfile {
    tin: String,
    name: String,
    selected: bool,
}

#[derive(Debug, Clone)]
struct DueItem {
    form_code: String,
    year: u16,
    period: u8,
    name: String,
    deadline: String,
}

#[derive(Debug, Clone)]
struct Form1601CState {
    draft: Form1601CDraft,
    validation_errors: Vec<(String, String)>,
    validated: bool,
    saved: bool,
}

pub struct BirAgentHost {
    platform: PlatformKind,
    shutdown: bool,
    is_locked: bool,
    admin_lock_enabled: bool,
    enable_profile_pins: bool,
    unsaved_compliance: bool,
    active_view: ActiveView,
    pending_admin: Option<ActiveView>,
    pending_profile_auth: bool,
    selected_tin: Option<String>,
    profiles: Vec<ListedProfile>,
    editor: ProfileEditor,
    dues: Vec<DueItem>,
    form_1601c: Option<Form1601CState>,
    submit_confirmation_visible: bool,
    form_loaded: bool,
    db: Option<Arc<Mutex<Database>>>,
}

impl BirAgentHost {
    pub fn new(platform: PlatformKind) -> Self {
        Self {
            platform,
            shutdown: false,
            is_locked: false,
            admin_lock_enabled: false,
            enable_profile_pins: false,
            unsaved_compliance: false,
            active_view: ActiveView::GlobalDashboard,
            pending_admin: None,
            pending_profile_auth: false,
            selected_tin: None,
            profiles: Vec::new(),
            editor: ProfileEditor::default(),
            dues: Vec::new(),
            form_1601c: None,
            submit_confirmation_visible: false,
            form_loaded: false,
            db: None,
        }
    }

    pub fn with_database(mut self, db: Arc<Mutex<Database>>) -> Self {
        self.reload_from_db(&db);
        self.db = Some(db);
        self
    }

    pub fn wants_shutdown(&self) -> bool {
        self.shutdown
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.is_locked = locked;
    }

    pub fn set_admin_lock_enabled(&mut self, enabled: bool) {
        self.admin_lock_enabled = enabled;
    }

    pub fn set_unsaved_compliance(&mut self, dirty: bool) {
        self.unsaved_compliance = dirty;
    }

    pub fn active_view(&self) -> ActiveView {
        self.active_view
    }

    pub fn selected_tin(&self) -> Option<&str> {
        self.selected_tin.as_deref()
    }

    pub fn submit_confirmation_visible(&self) -> bool {
        self.submit_confirmation_visible
    }

    pub fn set_profile_pins(&mut self, enabled: bool) {
        self.enable_profile_pins = enabled;
    }

    pub fn restore_view(&mut self, view: ActiveView, selected_tin: Option<String>) {
        self.active_view = view;
        self.selected_tin = selected_tin;
    }

    pub fn replace_profiles(
        &mut self,
        profiles: Vec<(String, String)>,
        selected_tin: Option<String>,
    ) {
        self.selected_tin = selected_tin.clone();
        self.profiles = profiles
            .into_iter()
            .map(|(tin, name)| ListedProfile {
                selected: selected_tin.as_deref() == Some(tin.as_str()),
                tin,
                name,
            })
            .collect();
    }

    pub fn replace_editor(&mut self, editor: ProfileEditor) {
        self.editor = editor;
    }

    pub fn editor_snapshot(&self) -> ProfileEditor {
        self.editor.clone()
    }

    pub fn replace_dues(&mut self, dues: Vec<(String, u16, u8, String, String)>) {
        self.dues = dues
            .into_iter()
            .map(|(form_code, year, period, name, deadline)| DueItem {
                form_code,
                year,
                period,
                name,
                deadline,
            })
            .collect();
    }

    pub fn replace_form_1601c(&mut self, draft: Form1601CDraft) {
        self.replace_form_1601c_state(draft, false, false, Vec::new());
    }

    pub fn replace_form_1601c_state(
        &mut self,
        draft: Form1601CDraft,
        validated: bool,
        saved: bool,
        validation_errors: Vec<(String, String)>,
    ) {
        self.form_loaded = true;
        self.form_1601c = Some(Form1601CState {
            draft,
            validation_errors,
            validated,
            saved,
        });
    }

    pub fn set_submit_confirmation_visible(&mut self, visible: bool) {
        self.submit_confirmation_visible = visible;
    }

    pub fn mark_pending_admin(&mut self, target: Option<ActiveView>) {
        self.pending_admin = target;
    }

    pub fn mark_pending_profile_auth(&mut self, pending: bool) {
        self.pending_profile_auth = pending;
    }

    pub fn pending_admin_view(&self) -> Option<ActiveView> {
        self.pending_admin
    }

    pub fn pending_profile_auth(&self) -> bool {
        self.pending_profile_auth
    }

    pub fn form_1601c_tax_14(&self) -> Option<f64> {
        self.form_1601c
            .as_ref()
            .map(|form| form.draft.tax_14_total_compensation)
    }

    pub fn form_1601c_tax_25(&self) -> Option<f64> {
        self.form_1601c
            .as_ref()
            .map(|form| form.draft.tax_25_total_taxes_withheld)
    }

    pub fn form_1601c_sheets(&self) -> Option<u32> {
        self.form_1601c
            .as_ref()
            .map(|form| form.draft.number_of_sheets)
    }

    pub fn form_1601c_saved(&self) -> bool {
        self.form_1601c.as_ref().is_some_and(|form| form.saved)
    }

    pub fn form_1601c_validated(&self) -> bool {
        self.form_1601c.as_ref().is_some_and(|form| form.validated)
    }

    pub fn form_period(&self) -> Option<u8> {
        self.form_1601c.as_ref().map(|form| form.draft.month)
    }

    pub fn form_1601c_status(&self) -> Option<FilingStatus> {
        self.form_1601c
            .as_ref()
            .map(|form| form.draft.status.clone())
    }

    fn reload_from_db(&mut self, db: &Arc<Mutex<Database>>) {
        let Ok(guard) = db.lock() else {
            return;
        };
        let listed = guard.list_profiles().unwrap_or_default();
        self.profiles = listed
            .iter()
            .map(|profile| ListedProfile {
                tin: profile.tin.full(),
                name: profile.full_name.clone(),
                selected: self.selected_tin.as_deref() == Some(profile.tin.full().as_str()),
            })
            .collect();
        if let Some(tin) = &self.selected_tin
            && let Some(profile) = listed.iter().find(|profile| profile.tin.full() == *tin)
        {
            self.dues = dues_for_profile(profile);
        }
    }

    fn listed_profile(&self, tin: &str) -> Option<&ListedProfile> {
        self.profiles.iter().find(|profile| profile.tin == tin)
    }

    fn load_profile(&self, tin: &str) -> Result<TaxpayerProfile, String> {
        let db = self.db.as_ref().ok_or("agent host has no database")?;
        let guard = db.lock().map_err(|err| err.to_string())?;
        guard
            .get_profile(tin)
            .map_err(|err| err.to_string())?
            .ok_or_else(|| format!("profile `{tin}` not found"))
    }

    fn gate_locked(&self) -> Result<(), String> {
        if self.is_locked {
            Err("app is locked; unlock through the existing lock screen".into())
        } else {
            Ok(())
        }
    }

    fn gate_dirty(&self, target: ActiveView) -> Result<(), String> {
        if self.unsaved_compliance && target != ActiveView::ProfileManager {
            Err("unsaved profile compliance changes block navigation".into())
        } else {
            Ok(())
        }
    }

    fn navigate(&mut self, target: ActiveView) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        self.gate_dirty(target)?;
        self.submit_confirmation_visible = false;

        let admin_gated = matches!(
            target,
            ActiveView::Settings | ActiveView::CronTasks | ActiveView::AdminCalendarDashboard
        );
        if admin_gated && self.admin_lock_enabled {
            self.pending_admin = Some(target);
            return Ok(DispatchResult::json(serde_json::json!({
                "pending_admin": true,
                "target": ids::view_slug(target),
            })));
        }

        if matches!(
            target,
            ActiveView::Dashboard
                | ActiveView::Form2551Q
                | ActiveView::Form1701Q
                | ActiveView::Form1601C
                | ActiveView::Form0619E
                | ActiveView::Form0619F
                | ActiveView::Form0605
                | ActiveView::Form2550Q
                | ActiveView::Form1701
                | ActiveView::Form1702RT
                | ActiveView::Form1702MX
        ) && self.selected_tin.is_none()
            && target == ActiveView::Dashboard
        {
            // Dashboard without a selected profile is still a reachable empty state.
        }

        if let Some(chrome) = ids::form_chrome(target) {
            if self.selected_tin.is_some() {
                self.open_form_view(chrome.code)?;
            } else {
                self.form_loaded = false;
                self.form_1601c = None;
            }
        }

        self.pending_admin = None;
        self.active_view = target;
        if target == ActiveView::GlobalDashboard {
            self.selected_tin = None;
            for profile in &mut self.profiles {
                profile.selected = false;
            }
            self.dues.clear();
        }
        if target == ActiveView::ProfileManager
            && self.editor.tin.is_empty()
            && self.selected_tin.is_none()
        {
            self.editor = ProfileEditor::default();
        }
        Ok(DispatchResult::json(serde_json::json!({
            "view": ids::view_slug(target),
            "page": ids::page_root(target),
        })))
    }

    fn new_profile_editor(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        self.gate_dirty(ActiveView::ProfileManager)?;
        self.selected_tin = None;
        for profile in &mut self.profiles {
            profile.selected = false;
        }
        self.editor = ProfileEditor::default();
        self.active_view = ActiveView::ProfileManager;
        self.pending_admin = None;
        Ok(DispatchResult::json(
            serde_json::json!({ "view": "profile-manager" }),
        ))
    }

    fn select_profile(&mut self, tin: &str) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        self.gate_dirty(ActiveView::Dashboard)?;
        let profile = self
            .listed_profile(tin)
            .ok_or_else(|| format!("profile `{tin}` not found"))?
            .clone();
        if self.enable_profile_pins {
            if let Ok(stored) = self.load_profile(tin)
                && (stored.profile_pin_hash.is_some() || stored.totp_secret.is_some())
            {
                self.pending_profile_auth = true;
                return Ok(DispatchResult::json(serde_json::json!({
                    "pending_profile_auth": true,
                    "tin_last4": last4(tin),
                })));
            }
        }
        self.pending_profile_auth = false;
        self.selected_tin = Some(profile.tin.clone());
        for item in &mut self.profiles {
            item.selected = item.tin == profile.tin;
        }
        if let Ok(full) = self.load_profile(&profile.tin) {
            self.dues = dues_for_profile(&full);
            self.editor = editor_from_profile(&full);
        }
        self.active_view = ActiveView::Dashboard;
        Ok(DispatchResult::json(serde_json::json!({
            "selected": last4(&profile.tin),
            "view": "dashboard",
        })))
    }

    fn set_field(&mut self, target: &str, value: &str) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        match target {
            ids::PROFILE_TIN => self.editor.tin = digits_only(value),
            ids::PROFILE_NAME => self.editor.full_name = value.to_string(),
            ids::PROFILE_RDO => {
                self.editor.rdo_code = value.split(" - ").next().unwrap_or(value).to_string()
            }
            ids::PROFILE_LOB => self.editor.line_of_business = value.to_string(),
            ids::PROFILE_ADDRESS => self.editor.registered_address = value.to_string(),
            ids::PROFILE_ZIP => self.editor.zip_code = value.to_string(),
            ids::PROFILE_PHONE => self.editor.phone = value.to_string(),
            ids::PROFILE_EMAIL => self.editor.email = value.to_string(),
            ids::FORM_1601C_TAX_14 => {
                let form = self.form_1601c.as_mut().ok_or("form 1601C is not open")?;
                form.draft.tax_14_total_compensation = parse_money(value)?;
                form.draft.compute();
                form.validated = false;
            }
            ids::FORM_1601C_TAX_25 => {
                let form = self.form_1601c.as_mut().ok_or("form 1601C is not open")?;
                form.draft.tax_25_total_taxes_withheld = parse_money(value)?;
                form.draft.compute();
                form.validated = false;
            }
            ids::FORM_1601C_SHEETS => {
                let form = self.form_1601c.as_mut().ok_or("form 1601C is not open")?;
                form.draft.number_of_sheets = value.trim().parse().unwrap_or(0);
                form.validated = false;
            }
            _ => return Err(format!("`{target}` is not editable")),
        }
        Ok(DispatchResult::json(serde_json::json!({ "value": value })))
    }

    fn type_into(&mut self, target: &str, text: &str) -> Result<DispatchResult, String> {
        let current = match target {
            ids::PROFILE_TIN => self.editor.tin.clone(),
            ids::PROFILE_NAME => self.editor.full_name.clone(),
            ids::PROFILE_RDO => self.editor.rdo_code.clone(),
            ids::PROFILE_LOB => self.editor.line_of_business.clone(),
            ids::PROFILE_ADDRESS => self.editor.registered_address.clone(),
            ids::PROFILE_ZIP => self.editor.zip_code.clone(),
            ids::PROFILE_PHONE => self.editor.phone.clone(),
            ids::PROFILE_EMAIL => self.editor.email.clone(),
            ids::FORM_1601C_TAX_14 => self
                .form_1601c
                .as_ref()
                .map(|form| format!("{:.2}", form.draft.tax_14_total_compensation))
                .unwrap_or_default(),
            ids::FORM_1601C_TAX_25 => self
                .form_1601c
                .as_ref()
                .map(|form| format!("{:.2}", form.draft.tax_25_total_taxes_withheld))
                .unwrap_or_default(),
            ids::FORM_1601C_SHEETS => self
                .form_1601c
                .as_ref()
                .map(|form| form.draft.number_of_sheets.to_string())
                .unwrap_or_default(),
            _ => return Err(format!("`{target}` is not editable")),
        };
        self.set_field(target, &format!("{current}{text}"))
    }

    fn save_profile(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        let db = self.db.clone().ok_or("agent host has no database")?;
        let profile = profile_from_editor(&self.editor)?;
        let errors = validate_profile(&profile);
        if !errors.is_empty() {
            self.editor.errors = errors.iter().map(|err| err.message.clone()).collect();
            self.editor.save_message = None;
            return Err(format!(
                "profile validation failed: {}",
                self.editor.errors.join("; ")
            ));
        }
        let saved = {
            let guard = db.lock().map_err(|err| err.to_string())?;
            guard.save_profile(profile).map_err(|err| err.to_string())?
        };
        let tin = saved.tin.full();
        self.editor.save_message = Some("Profile saved.".into());
        self.editor.errors.clear();
        self.selected_tin = Some(tin.clone());
        self.reload_from_db(&db);
        for item in &mut self.profiles {
            item.selected = item.tin == tin;
        }
        self.active_view = ActiveView::ProfileManager;
        Ok(DispatchResult::json(serde_json::json!({
            "tin_last4": last4(&tin),
            "name": saved.full_name,
        })))
    }

    fn open_form_view(&mut self, code: &str) -> Result<(), String> {
        self.form_loaded = true;
        self.submit_confirmation_visible = false;
        if code != "1601C" {
            self.form_1601c = None;
            return Ok(());
        }
        let tin = self
            .selected_tin
            .clone()
            .ok_or("select a taxpayer profile before opening a form")?;
        let profile = self.load_profile(&tin)?;
        let now = chrono::Local::now().date_naive();
        let mut draft =
            Form1601CDraft::new_from_profile(&profile, now.year() as u16, now.month() as u8);
        if let Some(db) = &self.db
            && let Ok(guard) = db.lock()
            && let Ok(Some(existing)) = guard.get_1601c_draft(&tin, draft.taxable_year, draft.month)
        {
            draft = existing;
        }
        self.form_1601c = Some(Form1601CState {
            draft,
            validation_errors: Vec::new(),
            validated: false,
            saved: false,
        });
        Ok(())
    }

    fn open_form(&mut self, code: &str, year: u16, period: u8) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        let Some(chrome) = ids::FORM_CHROME
            .iter()
            .find(|item| item.code.eq_ignore_ascii_case(code))
        else {
            return Err(format!("unknown form `{code}`"));
        };
        self.gate_dirty(chrome.view)?;
        if chrome.code == "1601C" {
            let tin = self
                .selected_tin
                .clone()
                .ok_or("select a taxpayer profile before opening a form")?;
            let profile = self.load_profile(&tin)?;
            let month = period.clamp(1, 12);
            let mut draft = Form1601CDraft::new_from_profile(&profile, year, month);
            if let Some(db) = &self.db
                && let Ok(guard) = db.lock()
                && let Ok(Some(existing)) = guard.get_1601c_draft(&tin, year, month)
            {
                draft = existing;
            }
            self.form_1601c = Some(Form1601CState {
                draft,
                validation_errors: Vec::new(),
                validated: false,
                saved: false,
            });
        } else {
            self.form_1601c = None;
        }
        self.form_loaded = true;
        self.submit_confirmation_visible = false;
        self.active_view = chrome.view;
        Ok(DispatchResult::json(serde_json::json!({
            "form": chrome.code,
            "page": ids::page_root(chrome.view),
            "year": year,
            "period": period,
        })))
    }

    fn validate_form(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        let form = self.form_1601c.as_mut().ok_or("form 1601C is not open")?;
        form.draft.compute();
        form.validation_errors = form.draft.validate();
        form.validated = true;
        Ok(DispatchResult::json(serde_json::json!({
            "ok": form.validation_errors.is_empty(),
            "errors": form.validation_errors.len(),
        })))
    }

    fn save_form_draft(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        if self.active_view != ActiveView::Form1601C {
            return Err(format!(
                "draft save for {} is not mapped in this agent slice; use 1601C",
                ids::view_slug(self.active_view)
            ));
        }
        let form = self.form_1601c.as_mut().ok_or("form 1601C is not open")?;
        if !form.draft.is_editable() {
            return Err("this return is no longer a draft".into());
        }
        form.draft.compute();
        let db = self.db.as_ref().ok_or("agent host has no database")?;
        {
            let guard = db.lock().map_err(|err| err.to_string())?;
            guard
                .save_1601c_draft(&form.draft)
                .map_err(|err| err.to_string())?;
        }
        form.saved = true;
        Ok(DispatchResult::json(serde_json::json!({
            "status": format!("{:?}", form.draft.status),
            "saved": true,
        })))
    }

    fn request_submit_confirm(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        if self.active_view == ActiveView::Form1601C {
            let form = self.form_1601c.as_mut().ok_or("form 1601C is not open")?;
            form.draft.compute();
            form.validation_errors = form.draft.validate();
            form.validated = true;
            if !form.validation_errors.is_empty() {
                return Err("fix validation errors before submitting".into());
            }
        }
        self.submit_confirmation_visible = true;
        Ok(DispatchResult::json(serde_json::json!({
            "confirmation": true,
            "queued": false,
            "filed": false,
            "queue_supported": can_queue_for_submission("1601C"),
        })))
    }

    fn refuse_submit_confirm(&self) -> Result<DispatchResult, String> {
        Err(
            "agent host refuses to queue or file a return; complete submission in the BIR UI confirmation path"
                .into(),
        )
    }

    fn click_due(&mut self, target: &str) -> Result<DispatchResult, String> {
        let rest = target
            .strip_prefix("due-")
            .ok_or_else(|| format!("cannot click `{target}`"))?;
        let item = self
            .dues
            .iter()
            .find(|due| ids::due_row(&due.form_code, due.year, due.period) == target)
            .cloned()
            .ok_or_else(|| format!("due `{rest}` not found"))?;
        self.open_form(&item.form_code, item.year, item.period)
    }

    fn click(&mut self, target: &str) -> Result<DispatchResult, String> {
        if self.is_locked {
            return Err("app is locked; unlock through the existing lock screen".into());
        }
        match target {
            ids::NAV_GLOBAL_DASHBOARD => self.navigate(ActiveView::GlobalDashboard),
            ids::NAV_NEW_PROFILE => self.new_profile_editor(),
            ids::NAV_IMPORT_EXPORT => self.navigate(ActiveView::ImportExport),
            ids::NAV_NOTIFICATIONS => self.navigate(ActiveView::Notifications),
            ids::NAV_CRON_TASKS => self.navigate(ActiveView::CronTasks),
            ids::NAV_ADMIN_CALENDAR => self.navigate(ActiveView::AdminCalendarDashboard),
            ids::NAV_SETTINGS => self.navigate(ActiveView::Settings),
            ids::NAV_LOGOUT => {
                self.selected_tin = None;
                self.navigate(ActiveView::GlobalDashboard)
            }
            ids::PROFILE_SAVE => self.save_profile(),
            ids::FORM_1601C_SAVE => self.save_form_draft(),
            ids::FORM_1601C_VALIDATE => self.validate_form(),
            ids::FORM_1601C_SUBMIT => self.request_submit_confirm(),
            ids::FORM_1601C_SUBMIT_CONFIRM => self.refuse_submit_confirm(),
            ids::FORM_1601C_BACK => self.navigate(ActiveView::Dashboard),
            other if other.starts_with("profile-") => {
                let tin = other.trim_start_matches("profile-");
                self.select_profile(tin)
            }
            other if other.starts_with("due-") => self.click_due(other),
            other => {
                if let Some(chrome) = ids::form_chrome(self.active_view) {
                    if other == chrome.back {
                        return self.navigate(ActiveView::Dashboard);
                    }
                    if other == chrome.save {
                        if chrome.code == "1601C" {
                            return self.save_form_draft();
                        }
                        return Err(format!(
                            "draft save for {} is not mapped in this agent slice; use 1601C",
                            chrome.code
                        ));
                    }
                    if other == chrome.submit {
                        return self.request_submit_confirm();
                    }
                }
                Err(format!("cannot click `{other}`"))
            }
        }
    }

    fn key(&mut self, target: &str, key: &str) -> Result<DispatchResult, String> {
        match (target, key) {
            (ids::PROFILE_SAVE, "Enter") => self.save_profile(),
            (ids::FORM_1601C_SAVE, "Enter") => self.save_form_draft(),
            (ids::PROFILE_NAME | ids::PROFILE_TIN | ids::FORM_1601C_TAX_14, "Backspace") => {
                let current = match target {
                    ids::PROFILE_NAME => &mut self.editor.full_name,
                    ids::PROFILE_TIN => &mut self.editor.tin,
                    _ => {
                        if let Some(form) = &mut self.form_1601c {
                            let mut text = format!("{:.2}", form.draft.tax_14_total_compensation);
                            text.pop();
                            form.draft.tax_14_total_compensation = text.parse().unwrap_or(0.0);
                            return Ok(DispatchResult::empty());
                        }
                        return Err("form 1601C is not open".into());
                    }
                };
                current.pop();
                Ok(DispatchResult::empty())
            }
            _ => Err(format!("unhandled key `{key}` on `{target}`")),
        }
    }

    fn refresh_dues(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        if let Some(tin) = self.selected_tin.clone()
            && let Ok(profile) = self.load_profile(&tin)
        {
            self.dues = dues_for_profile(&profile);
        }
        Ok(DispatchResult::json(serde_json::json!({
            "count": self.dues.len(),
        })))
    }

    fn invoke(&mut self, name: &str, args: &serde_json::Value) -> Result<DispatchResult, String> {
        match name {
            "nav.go" => {
                let page = args
                    .get("page")
                    .and_then(|value| value.as_str())
                    .ok_or("nav.go requires args.page")?;
                let view =
                    ids::view_from_slug(page).ok_or_else(|| format!("unknown page `{page}`"))?;
                self.navigate(view)
            }
            "profile.create" | "profile.new" => self.new_profile_editor(),
            "profile.save" => self.save_profile(),
            "tax-dues.refresh" => self.refresh_dues(),
            "filing.start" | "form.open" => {
                let code = args
                    .get("code")
                    .and_then(|value| value.as_str())
                    .ok_or("filing.start requires args.code")?;
                let year = args
                    .get("year")
                    .and_then(|value| value.as_u64())
                    .unwrap_or_else(|| chrono::Local::now().year() as u64)
                    as u16;
                let period = args
                    .get("period")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(1) as u8;
                self.open_form(code, year, period)
            }
            "filing.validate" | "form.validate" => self.validate_form(),
            "form.save_draft" => self.save_form_draft(),
            "filing.submit" => self.request_submit_confirm(),
            "form.submit"
            | "form.queue"
            | "form.file"
            | "form.submit_external"
            | "filing.queue"
            | "filing.file" => Err(format!(
                "invoke `{name}` is not allow-listed; agent hosts cannot queue or file returns"
            )),
            other => Err(format!("unknown invoke `{other}`")),
        }
    }

    pub fn tree(&self) -> UiTree {
        let mut window = UiNode::new(ids::WINDOW, "window", "eBIRForms");

        if self.is_locked {
            window = window.with_child(UiNode::new(ids::PAGE_LOCK, "page", "Lock screen"));
            return UiTree {
                app: "bir-desktop".into(),
                platform: self.platform,
                ready: true,
                nodes: vec![window],
            };
        }

        let mut sidebar = UiNode::new(ids::SIDEBAR, "navigation", "Sidebar").with_children(vec![
            nav_button(ids::NAV_GLOBAL_DASHBOARD, "Global Dashboard"),
            nav_button(ids::NAV_NEW_PROFILE, "Create Profile"),
            nav_button(ids::NAV_IMPORT_EXPORT, "Import Data"),
            nav_button(ids::NAV_NOTIFICATIONS, "Notifications"),
            nav_button(ids::NAV_CRON_TASKS, "Background Tasks"),
            nav_button(ids::NAV_ADMIN_CALENDAR, "Tax Calendars"),
            nav_button(ids::NAV_SETTINGS, "Settings"),
            nav_button(ids::NAV_LOGOUT, "Logout"),
        ]);
        let mut list = UiNode::new(ids::SIDEBAR_PROFILE_LIST, "list", "Taxpayer profiles");
        for profile in &self.profiles {
            list = list.with_child(
                UiNode::new(
                    ids::profile_row(&profile.tin),
                    "listitem",
                    profile.name.clone(),
                )
                .with_checked(profile.selected)
                .with_value(last4(&profile.tin)),
            );
        }
        sidebar = sidebar.with_child(list);
        window = window.with_child(sidebar);

        let mut page = UiNode::new(
            ids::page_root(self.active_view),
            "page",
            view_title(self.active_view),
        )
        .with_value(ids::view_slug(self.active_view));

        if self.active_view == ActiveView::ProfileManager {
            page = page.with_children(vec![
                textbox(ids::PROFILE_TIN, "TIN", &self.editor.tin),
                textbox(ids::PROFILE_NAME, "Taxpayer name", &self.editor.full_name),
                textbox(ids::PROFILE_RDO, "RDO", &self.editor.rdo_code),
                textbox(
                    ids::PROFILE_LOB,
                    "Line of business",
                    &self.editor.line_of_business,
                ),
                textbox(
                    ids::PROFILE_ADDRESS,
                    "Registered address",
                    &self.editor.registered_address,
                ),
                textbox(ids::PROFILE_ZIP, "ZIP code", &self.editor.zip_code),
                textbox(ids::PROFILE_PHONE, "Phone", &self.editor.phone),
                textbox(ids::PROFILE_EMAIL, "Email", &self.editor.email),
                UiNode::new(ids::PROFILE_SAVE, "button", "Save Profile"),
                UiNode::new(
                    ids::PROFILE_SAVE_MESSAGE,
                    "status",
                    self.editor.save_message.clone().unwrap_or_default(),
                ),
                UiNode::new(
                    ids::PROFILE_VALIDATION,
                    "status",
                    if self.editor.errors.is_empty() {
                        "valid".into()
                    } else {
                        self.editor.errors.join("; ")
                    },
                ),
            ]);
        }

        if matches!(
            self.active_view,
            ActiveView::Dashboard | ActiveView::GlobalDashboard
        ) {
            let mut dues = UiNode::new(ids::DUES_LIST, "list", "Tax dues");
            for due in &self.dues {
                dues = dues.with_child(
                    UiNode::new(
                        ids::due_row(&due.form_code, due.year, due.period),
                        "listitem",
                        due.name.clone(),
                    )
                    .with_value(due.deadline.clone())
                    .with_states(vec![due.form_code.clone()]),
                );
            }
            page = page.with_child(dues);
        }

        if let Some(chrome) = ids::form_chrome(self.active_view) {
            page = page.with_child(UiNode::new(chrome.back, "button", "Back"));
            page = page.with_child(
                UiNode::new(chrome.save, "button", "Save Draft").with_enabled(self.form_loaded),
            );
            let submit_enabled = self.form_loaded
                && !self.submit_confirmation_visible
                && self
                    .form_1601c
                    .as_ref()
                    .map(|form| form.validation_errors.is_empty())
                    .unwrap_or(true);
            page = page.with_child(
                UiNode::new(chrome.submit, "button", submit_label(chrome.code))
                    .with_enabled(submit_enabled),
            );
        }

        if let Some(form) = &self.form_1601c
            && self.active_view == ActiveView::Form1601C
        {
            page = page.with_child(UiNode::new(ids::FORM_1601C_VALIDATE, "button", "Validate"));
            page = page.with_child(
                UiNode::new(
                    ids::FORM_1601C_STATUS,
                    "status",
                    format!("{:?}", form.draft.status),
                )
                .with_value(format!("{:?}", form.draft.status)),
            );
            page = page.with_child(UiNode::new(
                ids::FORM_1601C_VALIDATION,
                "status",
                if !form.validated {
                    "not-validated".into()
                } else if form.validation_errors.is_empty() {
                    "valid".into()
                } else {
                    form.validation_errors
                        .iter()
                        .map(|(field, message)| format!("{field}: {message}"))
                        .collect::<Vec<_>>()
                        .join("; ")
                },
            ));
            page = page.with_child(textbox(
                ids::FORM_1601C_TAX_14,
                "14 Total Amount of Compensation",
                &format!("{:.2}", form.draft.tax_14_total_compensation),
            ));
            page = page.with_child(textbox(
                ids::FORM_1601C_TAX_25,
                "25 Total Taxes Withheld",
                &format!("{:.2}", form.draft.tax_25_total_taxes_withheld),
            ));
            page = page.with_child(textbox(
                ids::FORM_1601C_SHEETS,
                "Number of sheets",
                &form.draft.number_of_sheets.to_string(),
            ));
            if self.submit_confirmation_visible {
                page = page.with_child(
                    UiNode::new(
                        ids::FORM_1601C_SUBMIT_CONFIRM,
                        "button",
                        "Confirm filing in the BIR window (agent will not queue)",
                    )
                    .with_enabled(false),
                );
            }
        }

        window = window.with_child(page);

        if self.pending_admin.is_some() {
            window = window.with_child(UiNode::new(
                ids::OVERLAY_ADMIN_AUTH,
                "dialog",
                "Administrator authentication required",
            ));
        }
        if self.pending_profile_auth {
            window = window.with_child(UiNode::new(
                ids::OVERLAY_PROFILE_AUTH,
                "dialog",
                "Profile authentication required",
            ));
        }

        UiTree {
            app: "bir-desktop".into(),
            platform: self.platform,
            ready: true,
            nodes: vec![window],
        }
    }
}

impl AgentHost for BirAgentHost {
    fn hello(&self) -> HelloInfo {
        HelloInfo {
            protocol: PROTOCOL_VERSION,
            app: "bir-desktop".into(),
            platform: self.platform,
            ready: true,
            deliveries: vec![DeliveryMode::Semantic],
            // Do not set `auth` by hand. `handle_request` / the TCP server fill
            // it via `HelloAuth::from_token_configured(expected_token)`.
            auth: Default::default(),
        }
    }

    fn snapshot(&self) -> UiTree {
        self.tree()
    }

    fn screenshot(&self, path: Option<&str>) -> Result<DispatchResult, String> {
        let _ = path;
        let detail = match self.platform {
            PlatformKind::Headless => "headless host has no pixel surface",
            PlatformKind::Desktop => {
                "desktop host has no GPUI surface export yet (gpui-agent PR #20 is not depended on)"
            }
            _ => "this host has no pixel surface",
        };
        Err(gpui_agent::screenshot_unavailable(detail))
    }

    fn dispatch(&mut self, op: &Op) -> Result<DispatchResult, String> {
        if op.is_virtual_input() {
            return Err(virtual_unavailable(
                "bir-desktop ships semantic delivery first; \
                 virtual in-window events are not wired on this host. \
                 Protocol is unchanged; this host does not synthesize OS HID",
            ));
        }
        match op {
            Op::Click { target, .. } => self.click(target),
            Op::Type { target, text, .. } => self.type_into(target, text),
            Op::SetValue { target, value } => self.set_field(target, value),
            Op::Key { target, key, .. } => self.key(target, key),
            Op::Invoke { name, args } => self.invoke(name, args),
            Op::Screenshot { path } => AgentHost::screenshot(self, path.as_deref()),
            Op::Shutdown => {
                self.shutdown = true;
                Ok(DispatchResult::empty())
            }
            Op::Hello | Op::Snapshot | Op::Assert { .. } | Op::Wait { .. } => {
                Ok(DispatchResult::empty())
            }
        }
    }
}

/// Isolated host with a fixture taxpayer, 1601C dues, and no live-user DB.
pub fn fixture_host() -> BirAgentHost {
    let db = Database::open_ephemeral().expect("ephemeral db");
    let mut profile = fixture_profile();
    let year = chrono::Local::now().year() as u16;
    profile.per_year_forms.insert(
        year,
        PerYearFormsSet::from_codes(year, ["1601C"], FormSetSource::Manual),
    );
    profile.withholds_compensation = true;
    profile.has_employees = true;
    db.save_profile(profile).expect("fixture profile");
    let mut host =
        BirAgentHost::new(PlatformKind::Headless).with_database(Arc::new(Mutex::new(db)));
    host.select_profile(FIXTURE_TIN).expect("select fixture");
    host
}

pub fn empty_host() -> BirAgentHost {
    let db = Database::open_ephemeral().expect("ephemeral db");
    BirAgentHost::new(PlatformKind::Headless).with_database(Arc::new(Mutex::new(db)))
}

pub fn fixture_profile() -> TaxpayerProfile {
    serde_json::from_value(serde_json::json!({
        "id": null,
        "full_name": FIXTURE_NAME,
        "tin": {
            "segment1": "123",
            "segment2": "456",
            "segment3": "789",
            "branch": "00000"
        },
        "rdo_code": "018",
        "line_of_business": "Software Development",
        "registered_address": "Olongapo",
        "zip_code": "2200",
        "phone": "09123456789",
        "email": "agent-fixture@example.com",
        "default_form_type": "1601Cv2018",
        "taxpayer_type": "Corporation",
        "withholds_compensation": true,
        "has_employees": true
    }))
    .expect("fixture profile")
}

fn dues_for_profile(profile: &TaxpayerProfile) -> Vec<DueItem> {
    let year = chrono::Local::now().year();
    let codes = profile
        .per_year_forms
        .get(&(year as u16))
        .map(|set| set.active_form_codes())
        .unwrap_or_default();
    DeadlineResolver::deadlines_for_forms(&codes, year, &[])
        .into_iter()
        .filter_map(|deadline| {
            let (tax_year, period) = deadline.route_year_quarter()?;
            Some(DueItem {
                form_code: deadline.form_code.clone(),
                year: tax_year,
                period,
                name: format!(
                    "{} {}",
                    deadline.form_code,
                    deadline.final_deadline_string()
                ),
                deadline: deadline.final_deadline_string(),
            })
        })
        .collect()
}

fn editor_from_profile(profile: &TaxpayerProfile) -> ProfileEditor {
    ProfileEditor {
        tin: profile.tin.full(),
        full_name: profile.full_name.clone(),
        rdo_code: profile.rdo_code.clone(),
        line_of_business: profile.line_of_business.clone(),
        registered_address: profile.registered_address.clone(),
        zip_code: profile.zip_code.clone(),
        phone: profile.phone.clone(),
        email: profile.email.clone(),
        save_message: None,
        errors: Vec::new(),
    }
}

fn profile_from_editor(editor: &ProfileEditor) -> Result<TaxpayerProfile, String> {
    let tin = parse_tin(&editor.tin)?;
    serde_json::from_value(serde_json::json!({
        "id": null,
        "full_name": editor.full_name,
        "tin": {
            "segment1": tin.segment1,
            "segment2": tin.segment2,
            "segment3": tin.segment3,
            "branch": tin.branch
        },
        "rdo_code": editor.rdo_code,
        "line_of_business": editor.line_of_business,
        "registered_address": editor.registered_address,
        "zip_code": editor.zip_code,
        "phone": editor.phone,
        "email": editor.email,
        "default_form_type": "1601Cv2018",
        "taxpayer_type": "Corporation",
        "compliance_source_mode": "TemporalSuggestion"
    }))
    .map_err(|err| err.to_string())
}

fn parse_tin(raw: &str) -> Result<Tin, String> {
    let digits = digits_only(raw);
    if digits.len() < 12 || digits.len() > 14 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err("TIN must have 12 to 14 digits including branch code".into());
    }
    Ok(Tin {
        segment1: digits[0..3].to_string(),
        segment2: digits[3..6].to_string(),
        segment3: digits[6..9].to_string(),
        branch: digits[9..].to_string(),
    })
}

fn digits_only(value: &str) -> String {
    value.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn last4(tin: &str) -> String {
    let digits = digits_only(tin);
    digits
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn parse_money(value: &str) -> Result<f64, String> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("invalid amount `{value}`"))
}

fn nav_button(id: &str, name: &str) -> UiNode {
    UiNode::new(id, "button", name)
}

fn textbox(id: &str, name: &str, value: &str) -> UiNode {
    UiNode::new(id, "textbox", name).with_value(value.to_string())
}

fn view_title(view: ActiveView) -> &'static str {
    match view {
        ActiveView::GlobalDashboard => "Global Dashboard",
        ActiveView::Dashboard => "Dashboard",
        ActiveView::ProfileManager => "Profile Manager",
        ActiveView::CronTasks => "Background Tasks",
        ActiveView::Notifications => "Notifications",
        ActiveView::ImportExport => "Import Data",
        ActiveView::Settings => "Settings",
        ActiveView::AdminCalendarDashboard => "Tax Calendars",
        ActiveView::Form2551Q => "Form 2551Q",
        ActiveView::Form1701Q => "Form 1701Q",
        ActiveView::Form1601C => "Form 1601C",
        ActiveView::Form0619E => "Form 0619E",
        ActiveView::Form0619F => "Form 0619F",
        ActiveView::Form0605 => "Form 0605",
        ActiveView::Form2550Q => "Form 2550Q",
        ActiveView::Form1701 => "Form 1701",
        ActiveView::Form1702RT => "Form 1702RT",
        ActiveView::Form1702MX => "Form 1702MX",
    }
}

fn submit_label(code: &str) -> String {
    if can_queue_for_submission(code) {
        "Generate XML & Submit".into()
    } else {
        "Manual / external filing".into()
    }
}

trait WithEnabled {
    fn with_enabled(self, enabled: bool) -> Self;
    fn with_states(self, states: Vec<String>) -> Self;
}

impl WithEnabled for UiNode {
    fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn with_states(mut self, states: Vec<String>) -> Self {
        self.states = states;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_agent::dispatch::handle_request;
    use gpui_agent::protocol::{AssertSpec, Request};

    fn req(op: Op) -> Request {
        Request::new("t", op)
    }

    #[test]
    fn navigation_asserts_every_active_view_page_root() {
        let mut host = fixture_host();
        for view in ids::ALL_VIEWS {
            let slug = ids::view_slug(*view);
            handle_request(
                &mut host,
                req(Op::Invoke {
                    name: "nav.go".into(),
                    args: serde_json::json!({ "page": slug }),
                }),
                None,
            );
            let resp = handle_request(
                &mut host,
                req(Op::Assert {
                    spec: AssertSpec {
                        target: ids::page_root(*view).into(),
                        exists: Some(true),
                        ..Default::default()
                    },
                }),
                None,
            );
            assert!(resp.ok, "{}: {:?}", slug, resp.error);
        }
    }

    #[test]
    fn admin_gate_is_not_bypassed() {
        let mut host = empty_host();
        host.set_admin_lock_enabled(true);
        let resp = handle_request(&mut host, req(Op::click(ids::NAV_SETTINGS)), None);
        assert!(resp.ok, "{:?}", resp.error);
        assert_ne!(host.active_view(), ActiveView::Settings);
        let tree = host.tree();
        assert!(tree.find(ids::OVERLAY_ADMIN_AUTH).is_some());
        assert!(tree.find(ids::PAGE_SETTINGS).is_none());
    }

    #[test]
    fn lock_screen_blocks_navigation() {
        let mut host = empty_host();
        host.set_locked(true);
        let resp = handle_request(&mut host, req(Op::click(ids::NAV_SETTINGS)), None);
        assert!(!resp.ok);
        let tree = host.tree();
        assert!(tree.find(ids::PAGE_LOCK).is_some());
        assert!(tree.find(ids::PAGE_SETTINGS).is_none());
    }

    #[test]
    fn creates_profile_and_asserts_selected() {
        let mut host = empty_host();
        handle_request(&mut host, req(Op::click(ids::NAV_NEW_PROFILE)), None);
        for (id, value) in [
            (ids::PROFILE_TIN, "98765432100000"),
            (ids::PROFILE_NAME, "Created By Agent"),
            (ids::PROFILE_RDO, "018"),
            (ids::PROFILE_LOB, "Retail"),
            (ids::PROFILE_ADDRESS, "Manila"),
            (ids::PROFILE_ZIP, "1000"),
            (ids::PROFILE_PHONE, "09170000000"),
            (ids::PROFILE_EMAIL, "created@example.com"),
        ] {
            let resp = handle_request(
                &mut host,
                req(Op::SetValue {
                    target: id.into(),
                    value: value.into(),
                }),
                None,
            );
            assert!(resp.ok, "{id}: {:?}", resp.error);
        }
        let saved = handle_request(&mut host, req(Op::click(ids::PROFILE_SAVE)), None);
        assert!(saved.ok, "{:?}", saved.error);
        let row = ids::profile_row("98765432100000");
        let resp = handle_request(
            &mut host,
            req(Op::Assert {
                spec: AssertSpec {
                    target: row,
                    name: Some("Created By Agent".into()),
                    checked: Some(true),
                    ..Default::default()
                },
            }),
            None,
        );
        assert!(resp.ok, "{:?}", resp.error);
    }

    #[test]
    fn dues_snapshot_includes_fixture_1601c() {
        let host = fixture_host();
        let tree = host.tree();
        let due = tree
            .flatten()
            .into_iter()
            .find(|node| node.id.starts_with("due-1601C-"))
            .expect("fixture 1601C due");
        assert!(due.name.contains("1601C"));
        assert!(!due.id.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn form_1601c_draft_validate_and_confirmation_do_not_file() {
        let mut host = fixture_host();
        let year = chrono::Local::now().year() as u16;
        let opened = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.open".into(),
                args: serde_json::json!({ "code": "1601C", "year": year, "period": 1 }),
            }),
            None,
        );
        assert!(opened.ok, "{:?}", opened.error);
        handle_request(
            &mut host,
            req(Op::SetValue {
                target: ids::FORM_1601C_TAX_14.into(),
                value: "1000.00".into(),
            }),
            None,
        );
        handle_request(
            &mut host,
            req(Op::SetValue {
                target: ids::FORM_1601C_TAX_25.into(),
                value: "100.00".into(),
            }),
            None,
        );
        let validated = handle_request(&mut host, req(Op::click(ids::FORM_1601C_VALIDATE)), None);
        assert!(validated.ok, "{:?}", validated.error);
        let saved = handle_request(&mut host, req(Op::click(ids::FORM_1601C_SAVE)), None);
        assert!(saved.ok, "{:?}", saved.error);
        assert_eq!(host.form_1601c_status(), Some(FilingStatus::Draft));
        let submit = handle_request(&mut host, req(Op::click(ids::FORM_1601C_SUBMIT)), None);
        assert!(submit.ok, "{:?}", submit.error);
        assert!(host.submit_confirmation_visible());
        let confirm = handle_request(
            &mut host,
            req(Op::click(ids::FORM_1601C_SUBMIT_CONFIRM)),
            None,
        );
        assert!(!confirm.ok);
        assert_eq!(host.form_1601c_status(), Some(FilingStatus::Draft));
        let forbidden = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.submit".into(),
                args: serde_json::json!({}),
            }),
            None,
        );
        assert!(!forbidden.ok);
        assert_eq!(host.form_1601c_status(), Some(FilingStatus::Draft));
    }

    #[test]
    fn preferred_invoke_names_do_not_file() {
        let mut host = fixture_host();
        let created = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.create".into(),
                args: serde_json::json!({}),
            }),
            None,
        );
        assert!(created.ok, "{:?}", created.error);
        assert_eq!(host.active_view(), ActiveView::ProfileManager);

        host = fixture_host();
        let refreshed = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "tax-dues.refresh".into(),
                args: serde_json::json!({}),
            }),
            None,
        );
        assert!(refreshed.ok, "{:?}", refreshed.error);
        let year = chrono::Local::now().year() as u16;
        let started = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "filing.start".into(),
                args: serde_json::json!({ "code": "1601C", "year": year, "period": 1 }),
            }),
            None,
        );
        assert!(started.ok, "{:?}", started.error);
        handle_request(
            &mut host,
            req(Op::SetValue {
                target: ids::FORM_1601C_TAX_14.into(),
                value: "1000.00".into(),
            }),
            None,
        );
        handle_request(
            &mut host,
            req(Op::SetValue {
                target: ids::FORM_1601C_TAX_25.into(),
                value: "100.00".into(),
            }),
            None,
        );
        let validated = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "filing.validate".into(),
                args: serde_json::json!({}),
            }),
            None,
        );
        assert!(validated.ok, "{:?}", validated.error);
        let submit = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "filing.submit".into(),
                args: serde_json::json!({}),
            }),
            None,
        );
        assert!(submit.ok, "{:?}", submit.error);
        assert!(host.submit_confirmation_visible());
        assert_eq!(host.form_1601c_status(), Some(FilingStatus::Draft));
        let queued = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "filing.queue".into(),
                args: serde_json::json!({}),
            }),
            None,
        );
        assert!(!queued.ok);
        assert_eq!(host.form_1601c_status(), Some(FilingStatus::Draft));
    }

    #[test]
    fn virtual_delivery_is_unavailable_on_headless_and_desktop_hosts() {
        for mut host in [empty_host(), BirAgentHost::new(PlatformKind::Desktop)] {
            let resp = handle_request(&mut host, req(Op::click_virtual(ids::NAV_SETTINGS)), None);
            assert!(!resp.ok);
            assert!(
                resp.error
                    .as_deref()
                    .unwrap_or("")
                    .starts_with(gpui_agent::VIRTUAL_UNAVAILABLE)
            );
        }
        let desktop = BirAgentHost::new(PlatformKind::Desktop);
        assert_eq!(desktop.hello().deliveries, vec![DeliveryMode::Semantic]);
        // Host hello() leaves auth at Default; handle_request fills it.
        assert_eq!(desktop.hello().auth, gpui_agent::HelloAuth::default());
    }

    #[test]
    fn hello_auth_follows_handle_request_token() {
        let mut host = empty_host();
        let none = handle_request(&mut host, req(Op::Hello), None);
        assert!(none.ok, "{:?}", none.error);
        assert_eq!(
            none.hello.as_ref().map(|hello| hello.auth),
            Some(gpui_agent::HelloAuth::None)
        );

        let required = handle_request(
            &mut host,
            req(Op::Hello).with_token("dev-secret"),
            Some("dev-secret"),
        );
        assert!(required.ok, "{:?}", required.error);
        assert_eq!(
            required.hello.as_ref().map(|hello| hello.auth),
            Some(gpui_agent::HelloAuth::Required)
        );
        assert_eq!(host.hello().auth, gpui_agent::HelloAuth::default());
    }

    #[test]
    fn remaining_form_pages_expose_chrome_ids() {
        let mut host = fixture_host();
        for chrome in ids::FORM_CHROME {
            handle_request(
                &mut host,
                req(Op::Invoke {
                    name: "nav.go".into(),
                    args: serde_json::json!({ "page": ids::view_slug(chrome.view) }),
                }),
                None,
            );
            let tree = host.tree();
            assert!(
                tree.find(ids::page_root(chrome.view)).is_some(),
                "{}",
                chrome.code
            );
            assert!(tree.find(chrome.back).is_some(), "{} back", chrome.code);
            assert!(tree.find(chrome.save).is_some(), "{} save", chrome.code);
            assert!(tree.find(chrome.submit).is_some(), "{} submit", chrome.code);
        }
    }

    #[test]
    fn headless_tcp_host_serves_hello_and_nav() {
        use std::sync::{Arc, Mutex};
        use std::time::Duration;

        let store = Arc::new(Mutex::new(fixture_host()));
        let (addr, shutdown) =
            gpui_agent::server::spawn_host("127.0.0.1:0".parse().unwrap(), None, store.clone())
                .expect("bind");
        let mut client =
            gpui_agent::client::AgentClient::connect(addr).with_timeout(Duration::from_secs(5));
        client.wait_ready().expect("hello");
        client
            .invoke("nav.go", serde_json::json!({ "page": "profile-manager" }))
            .expect("nav");
        client
            .assert(AssertSpec {
                target: ids::PAGE_PROFILE_MANAGER.into(),
                exists: Some(true),
                ..Default::default()
            })
            .expect("page");
        let virt = client
            .rpc(Op::click_virtual(ids::NAV_SETTINGS))
            .expect("rpc");
        assert!(!virt.ok);
        client.expect_ok(Op::Shutdown).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        assert!(
            shutdown.load(std::sync::atomic::Ordering::SeqCst)
                || store.lock().unwrap().wants_shutdown()
        );
    }
}
