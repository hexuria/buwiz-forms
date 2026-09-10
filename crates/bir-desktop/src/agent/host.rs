//! Send `AgentHost` for headless tests and as the semantic snapshot source.
//!
//! Desktop GPUI is not `Send`, so the mailbox drain copies into this type,
//! dispatches, then applies the result back on the UI thread. Headless tests
//! use this host directly with an ephemeral SQLite database.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bir_core::calendar_rules::{DeadlineResolver, ResolvedTaxDeadline};
use bir_core::db::Database;
use bir_core::forms::form_1601c::Form1601CDraft;
use bir_core::forms::form_2551q::Form2551QDraft;
use bir_core::forms::{
    FilingStatus, FormSetSource, FormValidator, PerYearFormsSet, can_queue_for_submission,
};
use bir_core::naming::Tin;
use bir_core::profile::TaxpayerProfile;
use bir_core::validation::validate_profile;
use chrono::{Datelike, NaiveDate};
use gpui_agent::dispatch::DispatchResult;
use gpui_agent::host::AgentHost;
use gpui_agent::protocol::{DeliveryMode, HelloInfo, Op, PROTOCOL_VERSION, PlatformKind};
use gpui_agent::tree::{UiNode, UiTree};
use gpui_agent::virtual_unavailable;
use serde::Serialize;
use serde_json::{Value, json};

use crate::agent::ProfileEditor;
use crate::agent::ids;
use crate::agent::search::{self, ProfileHit};

use crate::app::ActiveView;

const FIXTURE_TIN: &str = "12345678900000";
const FIXTURE_NAME: &str = "Agent Fixture Taxpayer";

#[derive(Debug, Clone, Serialize)]
struct ListedProfile {
    tin: String,
    name: String,
    last4: String,
    selected: bool,
    archived: bool,
}

impl ListedProfile {
    fn new(tin: String, name: String, selected: bool, archived: bool) -> Self {
        Self {
            last4: last4(&tin),
            tin,
            name,
            selected,
            archived,
        }
    }

    fn hit(&self) -> ProfileHit {
        ProfileHit {
            tin: self.tin.clone(),
            name: self.name.clone(),
            last4: self.last4.clone(),
            selected: self.selected,
            archived: self.archived,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct DueItem {
    form_code: String,
    year: u16,
    period: u8,
    name: String,
    deadline: String,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
struct JobItem {
    id: i64,
    name: String,
    status: String,
    job_type: String,
}

#[derive(Debug, Clone, Serialize)]
struct SubmissionItem {
    id: i64,
    tin: String,
    form_code: String,
    status: String,
    period: String,
}

#[derive(Debug, Clone)]
struct Form1601CState {
    draft: Form1601CDraft,
    validation_errors: Vec<(String, String)>,
    validated: bool,
    saved: bool,
}

#[derive(Debug, Clone)]
struct Form2551QState {
    draft: Form2551QDraft,
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
    form_2551q: Option<Form2551QState>,
    submit_confirmation_visible: bool,
    form_loaded: bool,
    db: Option<Arc<Mutex<Database>>>,
    profile_tab: ids::ProfileManagerTab,
    hide_tax_profiles: bool,
    dues_filter: String,
    dues_scope: String,
    dashboard_forms: Option<Vec<String>>,
    dashboard_query: String,
    jobs: Vec<JobItem>,
    submissions: Vec<SubmissionItem>,
    print_requested: bool,
    palette_open: bool,
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
            form_2551q: None,
            submit_confirmation_visible: false,
            form_loaded: false,
            db: None,
            profile_tab: ids::ProfileManagerTab::Tax,
            hide_tax_profiles: false,
            dues_filter: "upcoming".into(),
            dues_scope: "profile".into(),
            dashboard_forms: None,
            dashboard_query: String::new(),
            jobs: Vec::new(),
            submissions: Vec::new(),
            print_requested: false,
            palette_open: false,
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
        profiles: Vec<(String, String, bool)>,
        selected_tin: Option<String>,
    ) {
        self.selected_tin = selected_tin.clone();
        self.profiles = profiles
            .into_iter()
            .map(|(tin, name, archived)| {
                ListedProfile::new(
                    tin.clone(),
                    name,
                    selected_tin.as_deref() == Some(tin.as_str()),
                    archived,
                )
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
        let today = dues_as_of();
        self.dues = dues
            .into_iter()
            .map(|(form_code, year, period, name, deadline)| {
                let status = classify_due(&deadline, today).to_string();
                DueItem {
                    form_code,
                    year,
                    period,
                    name,
                    deadline,
                    status,
                }
            })
            .collect();
    }

    pub fn set_hide_tax_profiles(&mut self, hide: bool) {
        self.hide_tax_profiles = hide;
    }

    pub fn set_profile_tab(&mut self, tab: ids::ProfileManagerTab) {
        self.profile_tab = tab;
    }

    pub fn profile_tab(&self) -> ids::ProfileManagerTab {
        self.profile_tab
    }

    pub fn take_print_request(&mut self) -> bool {
        let requested = self.print_requested;
        self.print_requested = false;
        requested
    }

    pub fn set_palette_open(&mut self, open: bool) {
        self.palette_open = open;
    }

    pub fn wants_palette(&self) -> bool {
        self.palette_open
    }

    pub fn dashboard_forms(&self) -> Option<&[String]> {
        self.dashboard_forms.as_deref()
    }

    pub fn dashboard_query(&self) -> &str {
        &self.dashboard_query
    }

    pub fn form_2551q_creditable(&self) -> Option<f64> {
        self.form_2551q
            .as_ref()
            .map(|form| form.draft.creditable_tax_withheld)
    }

    pub fn form_2551q_other_credit(&self) -> Option<f64> {
        self.form_2551q
            .as_ref()
            .map(|form| form.draft.other_tax_credit)
    }

    pub fn form_2551q_taxable_0(&self) -> Option<f64> {
        self.form_2551q
            .as_ref()
            .and_then(|form| form.draft.schedule_1.first())
            .map(|row| row.taxable_amount)
    }

    pub fn form_2551q_saved(&self) -> bool {
        self.form_2551q.as_ref().is_some_and(|form| form.saved)
    }

    pub fn form_2551q_validated(&self) -> bool {
        self.form_2551q.as_ref().is_some_and(|form| form.validated)
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

    pub fn replace_form_2551q_state(
        &mut self,
        draft: Form2551QDraft,
        validated: bool,
        saved: bool,
        validation_errors: Vec<(String, String)>,
    ) {
        self.form_loaded = true;
        self.form_2551q = Some(Form2551QState {
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
        self.form_1601c
            .as_ref()
            .map(|form| form.draft.month)
            .or_else(|| self.form_2551q.as_ref().map(|form| form.draft.quarter))
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
            .map(|profile| {
                ListedProfile::new(
                    profile.tin.full(),
                    profile.full_name.clone(),
                    self.selected_tin.as_deref() == Some(profile.tin.full().as_str()),
                    profile.is_archived,
                )
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
        self.select_profile_view(tin, ActiveView::Dashboard)
    }

    fn select_profile_view(
        &mut self,
        tin: &str,
        view: ActiveView,
    ) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        self.gate_dirty(view)?;
        let profile = self
            .listed_profile(tin)
            .cloned()
            .or_else(|| {
                self.load_profile(tin).ok().map(|stored| {
                    ListedProfile::new(
                        stored.tin.full(),
                        stored.full_name.clone(),
                        true,
                        stored.is_archived,
                    )
                })
            })
            .ok_or_else(|| format!("profile `{tin}` not found"))?;
        if self.enable_profile_pins
            && let Ok(stored) = self.load_profile(tin)
            && (stored.profile_pin_hash.is_some() || stored.totp_secret.is_some())
        {
            self.pending_profile_auth = true;
            return Ok(DispatchResult::json(json!({
                "pending_profile_auth": true,
                "tin_last4": last4(tin),
            })));
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
        self.form_1601c = None;
        self.form_2551q = None;
        self.active_view = view;
        Ok(DispatchResult::json(json!({
            "selected": last4(&profile.tin),
            "tin": profile.tin,
            "name": profile.name,
            "view": ids::view_slug(view),
        })))
    }

    fn listed_hits(&self) -> Vec<ProfileHit> {
        self.profiles.iter().map(ListedProfile::hit).collect()
    }

    fn search_hits(&self, query: &str) -> Vec<ProfileHit> {
        let q = query.trim().to_lowercase();
        self.profiles
            .iter()
            .filter(|profile| {
                q.is_empty()
                    || profile.tin.to_lowercase().contains(&q)
                    || profile.name.to_lowercase().contains(&q)
            })
            .map(ListedProfile::hit)
            .collect()
    }

    fn parse_select_view(raw: Option<&str>) -> Result<ActiveView, String> {
        match raw.map(str::trim).filter(|s| !s.is_empty()) {
            None | Some("dashboard") => Ok(ActiveView::Dashboard),
            Some("profile-manager") => Ok(ActiveView::ProfileManager),
            Some(other) => Err(format!(
                "unknown view `{other}` (expected dashboard or profile-manager)"
            )),
        }
    }

    fn profile_list(&self) -> Result<DispatchResult, String> {
        Ok(DispatchResult::json(json!(self.listed_hits())))
    }

    fn profile_search(&self, args: &Value) -> Result<DispatchResult, String> {
        let query = args.get("q").and_then(Value::as_str).unwrap_or("");
        Ok(DispatchResult::json(json!(self.search_hits(query))))
    }

    fn profile_set(&mut self, args: &Value) -> Result<DispatchResult, String> {
        let tin_arg = args
            .get("tin")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let query = args
            .get("q")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let view = Self::parse_select_view(args.get("view").and_then(Value::as_str))?;
        let matches = if let Some(tin) = tin_arg.as_deref() {
            self.search_hits(tin)
                .into_iter()
                .filter(|hit| hit.tin == tin)
                .collect::<Vec<_>>()
        } else if let Some(query) = query.as_deref() {
            self.search_hits(query)
        } else {
            return Err("profile.set requires args.tin or args.q".into());
        };
        match matches.as_slice() {
            [] => Ok(DispatchResult::json(json!({
                "status": "not_found",
                "query": tin_arg.or(query).unwrap_or_default(),
                "candidates": []
            }))),
            [only] => {
                let selected = self.select_profile_view(&only.tin, view)?;
                Ok(DispatchResult::json(json!({
                    "status": "ok",
                    "selected": selected.value,
                    "candidates": matches
                })))
            }
            _ => Ok(DispatchResult::json(json!({
                "status": "ambiguous",
                "query": tin_arg.or(query),
                "candidates": matches
            }))),
        }
    }

    fn profile_edit(&mut self, args: &Value) -> Result<DispatchResult, String> {
        let tin = args
            .get("tin")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| self.selected_tin.clone())
            .ok_or_else(|| "profile.edit requires args.tin or a selected profile".to_string())?;
        let selected = self.select_profile_view(&tin, ActiveView::ProfileManager)?;
        Ok(DispatchResult::json(json!({
            "status": "ok",
            "selected": selected.value,
            "tab": self.profile_tab.slug()
        })))
    }

    fn profile_tab_invoke(&mut self, args: &Value) -> Result<DispatchResult, String> {
        if self.active_view != ActiveView::ProfileManager {
            return Err("open Profile Manager with profile.edit first".into());
        }
        let slug = args
            .get("tab")
            .and_then(Value::as_str)
            .ok_or_else(|| "profile.tab requires args.tab".to_string())?;
        let tab = ids::ProfileManagerTab::from_slug(slug).ok_or_else(|| {
            format!(
                "unknown profile tab `{slug}` (expected tax, cor, email, export, calendar, or security)"
            )
        })?;
        if tab == ids::ProfileManagerTab::Calendar && !self.calendar_available() {
            return Err(
                "calendar tab is unavailable until a Google Calendar account is linked".into(),
            );
        }
        self.profile_tab = tab;
        Ok(DispatchResult::json(json!({
            "tab": tab.slug(),
            "id": tab.id(),
            "section": tab.section_id()
        })))
    }

    fn calendar_available(&self) -> bool {
        let Some(db) = &self.db else {
            return false;
        };
        let Ok(guard) = db.lock() else {
            return false;
        };
        bir_core::google_calendar::google_calendar_connection_from_db(&guard)
            .profile_calendar_available()
    }

    fn dues_list(&mut self, args: &Value) -> Result<DispatchResult, String> {
        self.dues_filter = match args
            .get("filter")
            .and_then(Value::as_str)
            .unwrap_or("upcoming")
        {
            "upcoming" | "overdue" | "all" => args
                .get("filter")
                .and_then(Value::as_str)
                .unwrap_or("upcoming")
                .to_string(),
            other => {
                return Err(format!(
                    "unknown dues filter `{other}` (expected upcoming, overdue, or all)"
                ));
            }
        };
        let selected = self.selected_tin.clone();
        self.dues_scope = match args.get("scope").and_then(Value::as_str) {
            None => {
                if selected.is_some() {
                    "profile".into()
                } else {
                    "global".into()
                }
            }
            Some("profile") | Some("global") => {
                args.get("scope").and_then(Value::as_str).unwrap().into()
            }
            Some(other) => {
                return Err(format!(
                    "unknown dues scope `{other}` (expected profile or global)"
                ));
            }
        };
        if self.dues_scope == "profile" && self.selected_tin.is_none() {
            return Err("dues.list scope=profile requires a selected profile".into());
        }
        self.reload_dues_filtered()?;
        Ok(DispatchResult::json(json!({
            "filter": self.dues_filter,
            "scope": self.dues_scope,
            "as_of": dues_as_of().to_string(),
            "date_basis": "local-calendar-date (chrono::Local::now().date_naive())",
            "dues": self.dues
        })))
    }

    fn reload_dues_filtered(&mut self) -> Result<(), String> {
        let today = dues_as_of();
        let mut dues = if self.dues_scope == "global" {
            global_month_dues(today)
        } else {
            let tin = self
                .selected_tin
                .clone()
                .ok_or("dues.list scope=profile requires a selected profile")?;
            dues_for_profile(&self.load_profile(&tin)?)
        };
        let filter = self.dues_filter.as_str();
        dues.retain(|due| match filter {
            "upcoming" => due.status == "upcoming",
            "overdue" => due.status == "overdue",
            _ => true,
        });
        if let Some(forms) = &self.dashboard_forms {
            dues.retain(|due| {
                forms
                    .iter()
                    .any(|code| due.form_code.eq_ignore_ascii_case(code))
            });
        }
        if !self.dashboard_query.is_empty() {
            let q = self.dashboard_query.to_lowercase();
            dues.retain(|due| {
                due.form_code.to_lowercase().contains(&q) || due.name.to_lowercase().contains(&q)
            });
        }
        self.dues = dues;
        Ok(())
    }

    fn jobs_list(&mut self, args: &Value) -> Result<DispatchResult, String> {
        self.reload_jobs_and_submissions()?;
        let status = args
            .get("status")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let jobs: Vec<&JobItem> = self
            .jobs
            .iter()
            .filter(|job| {
                status
                    .map(|want| job.status.eq_ignore_ascii_case(want))
                    .unwrap_or(true)
            })
            .collect();
        Ok(DispatchResult::json(json!({ "jobs": jobs })))
    }

    fn submissions_list(&mut self, args: &Value) -> Result<DispatchResult, String> {
        self.reload_jobs_and_submissions()?;
        let tin = args
            .get("tin")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let status = args
            .get("status")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let submissions: Vec<&SubmissionItem> = self
            .submissions
            .iter()
            .filter(|item| tin.map(|want| item.tin == want).unwrap_or(true))
            .filter(|item| {
                status
                    .map(|want| item.status.eq_ignore_ascii_case(want))
                    .unwrap_or(true)
            })
            .collect();
        Ok(DispatchResult::json(json!({ "submissions": submissions })))
    }

    pub fn reload_jobs_and_submissions(&mut self) -> Result<(), String> {
        let db = self.db.as_ref().ok_or("agent host has no database")?;
        let guard = db.lock().map_err(|err| err.to_string())?;
        self.jobs = guard
            .list_jobs()
            .map_err(|err| err.to_string())?
            .into_iter()
            .filter_map(|job| {
                Some(JobItem {
                    id: job.id?,
                    name: job.name,
                    status: job.status,
                    job_type: job.job_type,
                })
            })
            .collect();
        let mut submissions = Vec::new();
        for summary in guard
            .list_all_queued_submissions()
            .map_err(|err| err.to_string())?
        {
            submissions.push(SubmissionItem {
                id: summary.id,
                tin: summary.tin,
                form_code: summary.form_code,
                status: format!("{:?}", summary.status),
                period: submission_period(summary.taxable_year, summary.month, summary.quarter),
            });
        }
        let tins: Vec<String> = if let Some(tin) = &self.selected_tin {
            vec![tin.clone()]
        } else {
            self.profiles
                .iter()
                .map(|profile| profile.tin.clone())
                .collect()
        };
        for tin in tins {
            for sub in guard
                .list_submissions_for_tin(&tin)
                .map_err(|err| err.to_string())?
            {
                let Some(id) = sub.id else {
                    continue;
                };
                if submissions
                    .iter()
                    .any(|item| item.id == id && item.tin == sub.tin)
                {
                    continue;
                }
                submissions.push(SubmissionItem {
                    id,
                    tin: sub.tin,
                    form_code: sub.form_type,
                    status: sub.status,
                    period: sub.period,
                });
            }
        }
        self.submissions = submissions;
        Ok(())
    }

    fn open_command_palette(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        self.palette_open = true;
        Ok(DispatchResult::json(json!({
            "open": true,
            "id": ids::OVERLAY_COMMAND_PALETTE,
        })))
    }

    fn palette_search(&self, args: &Value) -> Result<DispatchResult, String> {
        let query = args.get("q").and_then(Value::as_str).unwrap_or("");
        let all = self.palette_profiles();
        let ranked = search::search_profiles_for_palette(&all, query, self.hide_tax_profiles);
        let matches: Vec<ProfileHit> = ranked
            .matches
            .iter()
            .map(|profile| ProfileHit::from_profile(profile, self.selected_tin.as_deref()))
            .collect();
        Ok(DispatchResult::json(json!({
            "matches": matches,
            "can_create": ranked.can_create,
            "create_query": ranked.create_query
        })))
    }

    fn palette_profiles(&self) -> Vec<TaxpayerProfile> {
        let Some(db) = &self.db else {
            return Vec::new();
        };
        let Ok(guard) = db.lock() else {
            return Vec::new();
        };
        guard.list_profiles().unwrap_or_default()
    }

    fn dashboard_set_forms(&mut self, args: &Value) -> Result<DispatchResult, String> {
        self.dashboard_forms = parse_dashboard_forms(args.get("forms"))?;
        if self.dues_scope == "global" || self.active_view == ActiveView::Dashboard {
            let _ = self.reload_dues_filtered();
        }
        Ok(DispatchResult::json(json!({
            "forms": match &self.dashboard_forms {
                None => Value::String("all".into()),
                Some(forms) => json!(forms),
            }
        })))
    }

    fn dashboard_filter(&mut self, args: &Value) -> Result<DispatchResult, String> {
        self.dashboard_query = args
            .get("q")
            .or_else(|| args.get("query"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if self.dues_scope == "global" || self.active_view == ActiveView::Dashboard {
            let _ = self.reload_dues_filtered();
        }
        Ok(DispatchResult::json(json!({ "q": self.dashboard_query })))
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
            ids::FORM_2551Q_CREDITABLE => {
                let form = self.form_2551q.as_mut().ok_or("form 2551Q is not open")?;
                form.draft.creditable_tax_withheld = parse_money(value)?;
                form.draft.recompute(None);
                form.validated = false;
            }
            ids::FORM_2551Q_OTHER_CREDIT => {
                let form = self.form_2551q.as_mut().ok_or("form 2551Q is not open")?;
                form.draft.other_tax_credit = parse_money(value)?;
                form.draft.recompute(None);
                form.validated = false;
            }
            ids::FORM_2551Q_TAXABLE_0 => {
                let form = self.form_2551q.as_mut().ok_or("form 2551Q is not open")?;
                let amount = parse_money(value)?;
                let row = form
                    .draft
                    .schedule_1
                    .first_mut()
                    .ok_or("2551Q schedule 1 has no rows")?;
                row.taxable_amount = amount;
                row.recompute();
                form.draft.recompute(None);
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
            ids::FORM_2551Q_CREDITABLE => self
                .form_2551q
                .as_ref()
                .map(|form| format!("{:.2}", form.draft.creditable_tax_withheld))
                .unwrap_or_default(),
            ids::FORM_2551Q_OTHER_CREDIT => self
                .form_2551q
                .as_ref()
                .map(|form| format!("{:.2}", form.draft.other_tax_credit))
                .unwrap_or_default(),
            ids::FORM_2551Q_TAXABLE_0 => self
                .form_2551q
                .as_ref()
                .and_then(|form| form.draft.schedule_1.first())
                .map(|row| format!("{:.2}", row.taxable_amount))
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
        match code {
            "1601C" => {
                let tin = self
                    .selected_tin
                    .clone()
                    .ok_or("select a taxpayer profile before opening a form")?;
                let profile = self.load_profile(&tin)?;
                let now = chrono::Local::now().date_naive();
                let mut draft = Form1601CDraft::new_from_profile(
                    &profile,
                    now.year() as u16,
                    now.month() as u8,
                );
                if let Some(db) = &self.db
                    && let Ok(guard) = db.lock()
                    && let Ok(Some(existing)) =
                        guard.get_1601c_draft(&tin, draft.taxable_year, draft.month)
                {
                    draft = existing;
                }
                self.form_1601c = Some(Form1601CState {
                    draft,
                    validation_errors: Vec::new(),
                    validated: false,
                    saved: false,
                });
                self.form_2551q = None;
            }
            "2551Q" => {
                self.open_2551q_draft(None, None)?;
            }
            _ => {
                self.form_1601c = None;
                self.form_2551q = None;
            }
        }
        Ok(())
    }

    fn open_2551q_draft(&mut self, year: Option<u16>, quarter: Option<u8>) -> Result<(), String> {
        let tin = self
            .selected_tin
            .clone()
            .ok_or("select a taxpayer profile before opening a form")?;
        let profile = self.load_profile(&tin)?;
        let now = chrono::Local::now().date_naive();
        let year = year.unwrap_or(now.year() as u16);
        let quarter = quarter.unwrap_or(((now.month() as u8).saturating_sub(1) / 3) + 1);
        let mut draft = Form2551QDraft::new_from_profile(&profile, year, quarter.clamp(1, 4));
        if let Some(db) = &self.db
            && let Ok(guard) = db.lock()
            && let Ok(Some(existing)) = guard.get_2551q_draft(&tin, year, draft.quarter)
        {
            draft = existing;
        }
        self.form_2551q = Some(Form2551QState {
            draft,
            validation_errors: Vec::new(),
            validated: false,
            saved: false,
        });
        self.form_1601c = None;
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
            self.form_2551q = None;
        } else if chrome.code == "2551Q" {
            self.open_2551q_draft(Some(year), Some(period.clamp(1, 4)))?;
        } else {
            self.form_1601c = None;
            self.form_2551q = None;
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
        if let Some(form) = self.form_1601c.as_mut()
            && self.active_view == ActiveView::Form1601C
        {
            form.draft.compute();
            form.validation_errors = form.draft.validate();
            form.validated = true;
            return Ok(DispatchResult::json(json!({
                "ok": form.validation_errors.is_empty(),
                "errors": form.validation_errors.len(),
                "form": "1601C",
            })));
        }
        if let Some(form) = self.form_2551q.as_mut()
            && self.active_view == ActiveView::Form2551Q
        {
            form.draft.recompute(None);
            form.validation_errors = form.draft.validate();
            form.validated = true;
            return Ok(DispatchResult::json(json!({
                "ok": form.validation_errors.is_empty(),
                "errors": form.validation_errors.len(),
                "form": "2551Q",
            })));
        }
        Err("open form 1601C or 2551Q first".into())
    }

    fn save_form_draft(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        match self.active_view {
            ActiveView::Form1601C => {
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
                Ok(DispatchResult::json(json!({
                    "status": format!("{:?}", form.draft.status),
                    "saved": true,
                    "form": "1601C",
                })))
            }
            ActiveView::Form2551Q => {
                let form = self.form_2551q.as_mut().ok_or("form 2551Q is not open")?;
                if !form.draft.is_editable() {
                    return Err("this return is no longer a draft".into());
                }
                form.draft.recompute(None);
                let db = self.db.as_ref().ok_or("agent host has no database")?;
                {
                    let guard = db.lock().map_err(|err| err.to_string())?;
                    guard
                        .save_2551q_draft(&form.draft)
                        .map_err(|err| err.to_string())?;
                }
                form.saved = true;
                Ok(DispatchResult::json(json!({
                    "status": format!("{:?}", form.draft.status),
                    "saved": true,
                    "form": "2551Q",
                })))
            }
            other => Err(format!(
                "draft save for {} is not mapped in this agent slice; use 1601C or 2551Q",
                ids::view_slug(other)
            )),
        }
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

    fn form_fields(&self) -> Result<DispatchResult, String> {
        if let Some(form) = &self.form_1601c
            && self.active_view == ActiveView::Form1601C
        {
            return Ok(DispatchResult::json(json!({
                "form": "1601C",
                "status": format!("{:?}", form.draft.status),
                "fields": form_1601c_fields(&form.draft),
            })));
        }
        if let Some(form) = &self.form_2551q
            && self.active_view == ActiveView::Form2551Q
        {
            return Ok(DispatchResult::json(json!({
                "form": "2551Q",
                "status": format!("{:?}", form.draft.status),
                "fields": form_2551q_fields(&form.draft),
            })));
        }
        Err("open form 1601C or 2551Q first".into())
    }

    fn form_fill(&mut self, args: &Value) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        let fields = args
            .get("fields")
            .and_then(Value::as_object)
            .ok_or("form.fill requires args.fields as an object")?;
        if self.active_view == ActiveView::Form1601C {
            let form = self.form_1601c.as_mut().ok_or("form 1601C is not open")?;
            if !form.draft.is_editable() {
                return Err("this return is no longer a draft".into());
            }
            for key in fields.keys() {
                if !is_1601c_fillable(key) {
                    return Err(format!("unknown or read-only 1601C field `{key}`"));
                }
            }
            for (key, value) in fields {
                apply_1601c_fill(&mut form.draft, key, value)?;
            }
            form.draft.compute();
            form.validated = false;
            return Ok(DispatchResult::json(json!({
                "form": "1601C",
                "applied": fields.keys().cloned().collect::<Vec<_>>(),
            })));
        }
        if self.active_view == ActiveView::Form2551Q {
            let form = self.form_2551q.as_mut().ok_or("form 2551Q is not open")?;
            if !form.draft.is_editable() {
                return Err("this return is no longer a draft".into());
            }
            for key in fields.keys() {
                if !is_2551q_fillable(key) {
                    return Err(format!("unknown or read-only 2551Q field `{key}`"));
                }
            }
            for (key, value) in fields {
                apply_2551q_fill(&mut form.draft, key, value)?;
            }
            form.draft.recompute(None);
            form.validated = false;
            return Ok(DispatchResult::json(json!({
                "form": "2551Q",
                "applied": fields.keys().cloned().collect::<Vec<_>>(),
            })));
        }
        Err("open form 1601C or 2551Q first".into())
    }

    fn form_pdf(&self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        let (slug, fields, form) = if let Some(form) = &self.form_1601c
            && self.active_view == ActiveView::Form1601C
        {
            ("1601c-2018", form.draft.to_bir_field_map(), "1601C")
        } else if let Some(form) = &self.form_2551q
            && self.active_view == ActiveView::Form2551Q
        {
            ("2551q-2018", form.draft.to_bir_field_map(), "2551Q")
        } else {
            return Err("open form 1601C or 2551Q first".into());
        };
        let path = write_agent_frozen_html(slug, &fields)?;
        let path = path.canonicalize().unwrap_or(path);
        if !path.is_absolute() {
            return Err("form.pdf must return an absolute path, not file bytes".into());
        }
        Ok(DispatchResult::json(json!({
            "path": path.to_string_lossy(),
            "kind": "frozen-html",
            "form": form,
            "note": "the app print pipeline is frozen HTML (bir_print::frozen_html::filled_document); invoke result is an absolute path, never file bytes"
        })))
    }

    fn form_print(&mut self, args: &Value) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        let copies = args.get("copies").and_then(Value::as_u64).unwrap_or(1);
        if !matches!(
            self.active_view,
            ActiveView::Form1601C | ActiveView::Form2551Q
        ) {
            return Err("open form 1601C or 2551Q first".into());
        }
        if self.platform == PlatformKind::Headless {
            return Err(
                "form.print needs the desktop window's frozen HTML preview; headless cannot print"
                    .into(),
            );
        }
        self.print_requested = true;
        Ok(DispatchResult::json(json!({
            "queued": false,
            "filed": false,
            "copies": copies,
            "copies_supported": false,
            "preview": "frozen-html"
        })))
    }

    fn form_revert_draft(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        if self.active_view == ActiveView::Form1601C {
            let form = self.form_1601c.as_mut().ok_or("form 1601C is not open")?;
            let db = self.db.as_ref().ok_or("agent host has no database")?;
            let canceled = {
                let guard = db.lock().map_err(|err| err.to_string())?;
                guard
                    .cancel_queued_1601c_submission(&form.draft)
                    .map_err(|err| err.to_string())?
            };
            form.draft = canceled;
            form.validated = false;
            form.saved = true;
            return Ok(DispatchResult::json(json!({
                "form": "1601C",
                "status": format!("{:?}", form.draft.status),
            })));
        }
        if self.active_view == ActiveView::Form2551Q {
            let form = self.form_2551q.as_mut().ok_or("form 2551Q is not open")?;
            let db = self.db.as_ref().ok_or("agent host has no database")?;
            let canceled = {
                let guard = db.lock().map_err(|err| err.to_string())?;
                guard
                    .cancel_queued_2551q_submission(&form.draft)
                    .map_err(|err| err.to_string())?
            };
            form.draft = canceled;
            form.validated = false;
            form.saved = true;
            return Ok(DispatchResult::json(json!({
                "form": "2551Q",
                "status": format!("{:?}", form.draft.status),
            })));
        }
        Err("open form 1601C or 2551Q first".into())
    }

    fn form_mark_paid(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        if self.active_view == ActiveView::Form1601C {
            return Ok(DispatchResult::json(json!({
                "status": "unsupported",
                "form": "1601C",
                "reason": "1601-C payment status requires a separately verified confirmation workflow; the agent will not fake Paid"
            })));
        }
        if self.active_view == ActiveView::Form2551Q {
            let form = self.form_2551q.as_mut().ok_or("form 2551Q is not open")?;
            if !matches!(form.draft.status, FilingStatus::Confirmed) {
                return Err("only a Confirmed 2551Q return can be marked paid".into());
            }
            form.draft.transition_to_paid();
            let db = self.db.as_ref().ok_or("agent host has no database")?;
            {
                let guard = db.lock().map_err(|err| err.to_string())?;
                guard
                    .save_paid_2551q_draft(&form.draft)
                    .map_err(|err| err.to_string())?;
            }
            form.saved = true;
            return Ok(DispatchResult::json(json!({
                "form": "2551Q",
                "status": format!("{:?}", form.draft.status),
            })));
        }
        Err("open form 1601C or 2551Q first".into())
    }

    fn form_upload_receipt(&self) -> Result<DispatchResult, String> {
        Ok(DispatchResult::json(json!({
            "status": "needs_file",
            "path": Value::Null,
            "reason": "receipt upload requires the desktop file picker; a later success returns an absolute path and never file bytes on the invoke result"
        })))
    }

    fn calendar_add(&self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        let tin = self
            .selected_tin
            .as_deref()
            .ok_or("select a taxpayer profile before calendar.add")?;
        let profile = self.load_profile(tin)?;
        let db = self.db.as_ref().ok_or("agent host has no database")?;
        let (events, excluded_undated) = {
            let guard = db.lock().map_err(|err| err.to_string())?;
            bir_core::google_calendar::build_desired_events(&guard, &profile)
                .map_err(|err| err.to_string())?
        };
        if events.is_empty() {
            return Ok(DispatchResult::json(json!({
                "status": "empty",
                "excluded_undated": excluded_undated,
                "reason": "this profile has no dated deadlines for its Forms Set"
            })));
        }
        let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let dir = std::env::temp_dir().join(format!("bir-agent-calendar-{}", uuid::Uuid::new_v4()));
        let path =
            bir_core::calendar_ics::write_profile_calendar_ics(&dir, &profile, &events, &stamp)
                .map_err(|err| err.to_string())?;
        Ok(DispatchResult::json(json!({
            "status": "ok",
            "path": path.to_string_lossy(),
            "kind": "ics",
            "opened": false,
            "excluded_undated": excluded_undated
        })))
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
            ids::FORM_2551Q_VALIDATE => self.validate_form(),
            other if ids::ProfileManagerTab::from_id(other).is_some() => {
                let tab = ids::ProfileManagerTab::from_id(other).expect("tab id");
                if tab == ids::ProfileManagerTab::Calendar && !self.calendar_available() {
                    return Err(
                        "calendar tab is unavailable until a Google Calendar account is linked"
                            .into(),
                    );
                }
                self.profile_tab = tab;
                Ok(DispatchResult::json(json!({
                    "tab": tab.slug(),
                    "id": tab.id(),
                })))
            }
            other if ids::profile_row_tin(other).is_some() => {
                let tin = ids::profile_row_tin(other).expect("profile tin");
                self.select_profile(tin)
            }
            other if other.starts_with("due-") => self.click_due(other),
            other => {
                if let Some(chrome) = ids::form_chrome(self.active_view) {
                    if other == chrome.back {
                        return self.navigate(ActiveView::Dashboard);
                    }
                    if other == chrome.save {
                        if chrome.code == "1601C" || chrome.code == "2551Q" {
                            return self.save_form_draft();
                        }
                        return Err(format!(
                            "draft save for {} is not mapped in this agent slice; use 1601C or 2551Q",
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
            "profile.list" => self.profile_list(),
            "profile.search" => self.profile_search(args),
            "profile.set" => self.profile_set(args),
            "profile.edit" => self.profile_edit(args),
            "profile.tab" => self.profile_tab_invoke(args),
            "dues.list" => self.dues_list(args),
            "tax-dues.refresh" => self.refresh_dues(),
            "jobs.list" => self.jobs_list(args),
            "submissions.list" => self.submissions_list(args),
            "search.open" | "palette.open" => self.open_command_palette(),
            "palette.search" => self.palette_search(args),
            "dashboard.set_forms" => self.dashboard_set_forms(args),
            "dashboard.filter" => self.dashboard_filter(args),
            "form.fields" => self.form_fields(),
            "form.fill" => self.form_fill(args),
            "form.pdf" | "form.preview_pdf" => self.form_pdf(),
            "form.print" => self.form_print(args),
            "form.revert_draft" | "draft.revert" => self.form_revert_draft(),
            "form.mark_paid" | "payment.mark_paid" => self.form_mark_paid(),
            "form.upload_receipt" | "receipt.upload" => self.form_upload_receipt(),
            "calendar.add" => self.calendar_add(),
            "profile.calendar_sync" => Err(
                "profile.calendar_sync needs a linked Google Calendar account and the Profile Manager calendar tab; the agent will not push events"
                    .into(),
            ),
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
            "profile.ensure" => Err(
                "profile.ensure is not implemented; the host will not auto-create a taxpayer. \
                 Use profile.create to open the editor; profile.save stays confirm-gated"
                    .into(),
            ),
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
        window = window.with_child(
            UiNode::new(ids::CONTEXT_SELECTED_TIN, "status", "Selected taxpayer")
                .with_value(self.selected_tin.clone().unwrap_or_default()),
        );

        let mut page = UiNode::new(
            ids::page_root(self.active_view),
            "page",
            view_title(self.active_view),
        )
        .with_value(ids::view_slug(self.active_view));

        if self.active_view == ActiveView::ProfileManager {
            let mut tabs = UiNode::new("profile-tabs", "tablist", "Profile tabs");
            for tab in [
                ids::ProfileManagerTab::Tax,
                ids::ProfileManagerTab::Cor,
                ids::ProfileManagerTab::Email,
                ids::ProfileManagerTab::Security,
                ids::ProfileManagerTab::Export,
            ] {
                tabs = tabs.with_child(
                    UiNode::new(tab.id(), "tab", tab.slug())
                        .with_checked(self.profile_tab == tab)
                        .with_value(tab.slug().to_string()),
                );
            }
            if self.calendar_available() {
                tabs = tabs.with_child(
                    UiNode::new(ids::ProfileManagerTab::Calendar.id(), "tab", "calendar")
                        .with_checked(self.profile_tab == ids::ProfileManagerTab::Calendar)
                        .with_value("calendar".to_string()),
                );
            }
            page = page.with_child(tabs);
            page = page.with_child(
                UiNode::new(
                    self.profile_tab.section_id(),
                    "region",
                    self.profile_tab.slug(),
                )
                .with_value(self.profile_tab.slug().to_string()),
            );
            page = page.with_child(textbox(ids::PROFILE_TIN, "TIN", &self.editor.tin));
            page = page.with_child(textbox(
                ids::PROFILE_NAME,
                "Taxpayer name",
                &self.editor.full_name,
            ));
            page = page.with_child(textbox(ids::PROFILE_RDO, "RDO", &self.editor.rdo_code));
            page = page.with_child(textbox(
                ids::PROFILE_LOB,
                "Line of business",
                &self.editor.line_of_business,
            ));
            page = page.with_child(textbox(
                ids::PROFILE_ADDRESS,
                "Registered address",
                &self.editor.registered_address,
            ));
            page = page.with_child(textbox(ids::PROFILE_ZIP, "ZIP code", &self.editor.zip_code));
            page = page.with_child(textbox(ids::PROFILE_PHONE, "Phone", &self.editor.phone));
            page = page.with_child(textbox(ids::PROFILE_EMAIL, "Email", &self.editor.email));
            page = page.with_child(UiNode::new(ids::PROFILE_SAVE, "button", "Save Profile"));
            page = page.with_child(UiNode::new(
                ids::PROFILE_SAVE_MESSAGE,
                "status",
                self.editor.save_message.clone().unwrap_or_default(),
            ));
            page = page.with_child(UiNode::new(
                ids::PROFILE_VALIDATION,
                "status",
                if self.editor.errors.is_empty() {
                    "valid".into()
                } else {
                    self.editor.errors.join("; ")
                },
            ));
        }

        if matches!(
            self.active_view,
            ActiveView::Dashboard | ActiveView::GlobalDashboard
        ) {
            page = page.with_child(
                UiNode::new(ids::DASHBOARD_FORM_FILTER, "combobox", "Form filter").with_value(
                    self.dashboard_forms
                        .as_ref()
                        .map(|forms| forms.join(","))
                        .unwrap_or_else(|| "all".into()),
                ),
            );
            page = page.with_child(
                UiNode::new(ids::DASHBOARD_FILTER_QUERY, "textbox", "Dashboard search")
                    .with_value(self.dashboard_query.clone()),
            );
            if let Some(forms) = &self.dashboard_forms {
                for code in forms {
                    page = page.with_child(
                        UiNode::new(ids::dashboard_form_chip(code), "checkbox", code.clone())
                            .with_checked(true)
                            .with_value(code.clone()),
                    );
                }
            }
            let mut dues = UiNode::new(ids::DUES_LIST, "list", "Tax dues");
            for due in &self.dues {
                dues = dues.with_child(
                    UiNode::new(
                        ids::due_row(&due.form_code, due.year, due.period),
                        "listitem",
                        format!("{} ({})", due.name, due.status),
                    )
                    .with_value(due.deadline.clone())
                    .with_states(vec![due.form_code.clone(), due.status.clone()]),
                );
            }
            page = page.with_child(dues);
        }

        if self.active_view == ActiveView::CronTasks {
            let mut jobs = UiNode::new(ids::JOBS_LIST, "list", "Background jobs");
            for job in &self.jobs {
                jobs = jobs.with_child(
                    UiNode::new(ids::job_row(job.id), "listitem", job.name.clone())
                        .with_value(job.status.clone())
                        .with_states(vec![job.status.clone(), job.job_type.clone()]),
                );
            }
            page = page.with_child(jobs);
            let mut submissions = UiNode::new(ids::SUBMISSIONS_LIST, "list", "Submissions");
            for item in &self.submissions {
                submissions = submissions.with_child(
                    UiNode::new(
                        ids::submission_row(item.id),
                        "listitem",
                        format!("{} {}", item.form_code, item.period),
                    )
                    .with_value(item.status.clone())
                    .with_states(vec![item.tin.clone(), item.status.clone()]),
                );
            }
            page = page.with_child(submissions);
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

        if let Some(form) = &self.form_2551q
            && self.active_view == ActiveView::Form2551Q
        {
            page = page.with_child(UiNode::new(ids::FORM_2551Q_VALIDATE, "button", "Validate"));
            page = page.with_child(
                UiNode::new(
                    ids::FORM_2551Q_STATUS,
                    "status",
                    format!("{:?}", form.draft.status),
                )
                .with_value(format!("{:?}", form.draft.status)),
            );
            page = page.with_child(UiNode::new(
                ids::FORM_2551Q_VALIDATION,
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
                ids::FORM_2551Q_CREDITABLE,
                "Creditable tax withheld",
                &format!("{:.2}", form.draft.creditable_tax_withheld),
            ));
            page = page.with_child(textbox(
                ids::FORM_2551Q_OTHER_CREDIT,
                "Other tax credit",
                &format!("{:.2}", form.draft.other_tax_credit),
            ));
            let taxable = form
                .draft
                .schedule_1
                .first()
                .map(|row| format!("{:.2}", row.taxable_amount))
                .unwrap_or_else(|| "0.00".into());
            page = page.with_child(textbox(
                ids::FORM_2551Q_TAXABLE_0,
                "Schedule 1 row 1 taxable amount",
                &taxable,
            ));
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
        if self.palette_open {
            window = window.with_child(UiNode::new(
                ids::OVERLAY_COMMAND_PALETTE,
                "dialog",
                "Command palette",
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
    let today = dues_as_of();
    let codes = profile
        .per_year_forms
        .get(&(year as u16))
        .map(|set| set.active_form_codes())
        .unwrap_or_default();
    DeadlineResolver::deadlines_for_forms(&codes, year, &[])
        .into_iter()
        .filter_map(|deadline| due_item_from_deadline(&deadline, today, false))
        .collect()
}

fn global_month_dues(today: NaiveDate) -> Vec<DueItem> {
    DeadlineResolver::resolve_deadline_calendar_year(today.year())
        .into_iter()
        .filter_map(|deadline| due_item_from_deadline(&deadline, today, true))
        .collect()
}

fn due_item_from_deadline(
    deadline: &ResolvedTaxDeadline,
    today: NaiveDate,
    current_month_only: bool,
) -> Option<DueItem> {
    let (tax_year, period) = deadline.route_year_quarter()?;
    let date = deadline.final_deadline_date()?;
    if current_month_only && (date.year() != today.year() || date.month() != today.month()) {
        return None;
    }
    let status = if date < today { "overdue" } else { "upcoming" };
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
        status: status.into(),
    })
}

fn dues_as_of() -> NaiveDate {
    chrono::Local::now().date_naive()
}

fn classify_due(deadline: &str, today: NaiveDate) -> &'static str {
    if deadline.eq_ignore_ascii_case("Event Based") {
        return "event-based";
    }
    match NaiveDate::parse_from_str(deadline, "%Y-%m-%d") {
        Ok(date) if date < today => "overdue",
        Ok(_) => "upcoming",
        Err(_) => "unknown",
    }
}

fn submission_period(year: u16, month: Option<u8>, quarter: Option<u8>) -> String {
    if let Some(month) = month {
        format!("{year}-{month:02}")
    } else if let Some(quarter) = quarter {
        format!("{year}-Q{quarter}")
    } else {
        year.to_string()
    }
}

fn parse_dashboard_forms(raw: Option<&Value>) -> Result<Option<Vec<String>>, String> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    match raw {
        Value::String(s) if s.eq_ignore_ascii_case("all") => Ok(None),
        Value::String(s) => {
            let codes: Vec<String> = s
                .split([',', ' '])
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect();
            if codes.is_empty() {
                Ok(None)
            } else {
                Ok(Some(codes))
            }
        }
        Value::Array(items) => {
            let mut codes = Vec::new();
            for item in items {
                let Some(code) = item.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
                    return Err("dashboard.set_forms forms[] entries must be strings".into());
                };
                codes.push(code.to_string());
            }
            Ok(if codes.is_empty() { None } else { Some(codes) })
        }
        _ => Err(
            "dashboard.set_forms forms must be \"all\", a comma list, or an array of codes".into(),
        ),
    }
}

fn write_agent_frozen_html(
    slug: &str,
    fields: &BTreeMap<String, String>,
) -> Result<PathBuf, String> {
    let html = bir_print::frozen_html::filled_document(slug, fields)?;
    let dir = std::env::temp_dir().join(format!("bir-agent-export-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("could not create agent export dir: {err}"))?;
    let path = dir.join("index.html");
    std::fs::write(&path, html).map_err(|err| format!("could not write frozen HTML: {err}"))?;
    Ok(path)
}

fn form_1601c_fields(draft: &Form1601CDraft) -> Vec<Value> {
    vec![
        field_desc("tin", true, &draft.tin, true, false),
        field_desc("taxpayer_name", true, &draft.taxpayer_name, true, false),
        field_desc("rdo_code", true, &draft.rdo_code, true, false),
        field_desc(
            "registered_address",
            true,
            &draft.registered_address,
            true,
            false,
        ),
        field_desc("zip_code", false, &draft.zip_code, true, false),
        field_desc("contact_number", false, &draft.contact_number, true, false),
        field_desc("email_address", false, &draft.email_address, true, false),
        field_desc(
            "tax_14",
            true,
            &format!("{:.2}", draft.tax_14_total_compensation),
            false,
            true,
        ),
        field_desc(
            "tax_25",
            true,
            &format!("{:.2}", draft.tax_25_total_taxes_withheld),
            false,
            true,
        ),
        field_desc(
            "sheets",
            false,
            &draft.number_of_sheets.to_string(),
            false,
            true,
        ),
    ]
}

fn form_2551q_fields(draft: &Form2551QDraft) -> Vec<Value> {
    let taxable = draft
        .schedule_1
        .first()
        .map(|row| format!("{:.2}", row.taxable_amount))
        .unwrap_or_else(|| "0.00".into());
    vec![
        field_desc("tin", true, &draft.tin, true, false),
        field_desc("taxpayer_name", true, &draft.taxpayer_name, true, false),
        field_desc("rdo_code", true, &draft.rdo_code, true, false),
        field_desc(
            "registered_address",
            true,
            &draft.registered_address,
            true,
            false,
        ),
        field_desc("zip_code", false, &draft.zip_code, true, false),
        field_desc("contact_number", false, &draft.contact_number, true, false),
        field_desc("email", false, &draft.email, true, false),
        field_desc(
            "creditable_tax_withheld",
            false,
            &format!("{:.2}", draft.creditable_tax_withheld),
            false,
            true,
        ),
        field_desc(
            "other_tax_credit",
            false,
            &format!("{:.2}", draft.other_tax_credit),
            false,
            true,
        ),
        field_desc("taxable_amount", false, &taxable, false, true),
    ]
}

fn field_desc(
    key: &str,
    required: bool,
    value: &str,
    profile_defaulted: bool,
    fillable: bool,
) -> Value {
    json!({
        "key": key,
        "required": required,
        "value": value,
        "profile_defaulted": profile_defaulted,
        "fillable": fillable,
    })
}

fn is_1601c_fillable(key: &str) -> bool {
    matches!(
        key,
        "tax_14"
            | "tax_25"
            | "sheets"
            | ids::FORM_1601C_TAX_14
            | ids::FORM_1601C_TAX_25
            | ids::FORM_1601C_SHEETS
    )
}

fn apply_1601c_fill(draft: &mut Form1601CDraft, key: &str, value: &Value) -> Result<(), String> {
    let text = value_as_text(value)?;
    match key {
        "tax_14" | ids::FORM_1601C_TAX_14 => {
            draft.tax_14_total_compensation = parse_money(&text)?;
        }
        "tax_25" | ids::FORM_1601C_TAX_25 => {
            draft.tax_25_total_taxes_withheld = parse_money(&text)?;
        }
        "sheets" | ids::FORM_1601C_SHEETS => {
            draft.number_of_sheets = text
                .trim()
                .parse()
                .map_err(|_| format!("invalid sheets `{text}`"))?;
        }
        _ => return Err(format!("unknown 1601C field `{key}`")),
    }
    Ok(())
}

fn is_2551q_fillable(key: &str) -> bool {
    matches!(
        key,
        "creditable_tax_withheld"
            | "other_tax_credit"
            | "taxable_amount"
            | "schedule_1.0.taxable_amount"
            | ids::FORM_2551Q_CREDITABLE
            | ids::FORM_2551Q_OTHER_CREDIT
            | ids::FORM_2551Q_TAXABLE_0
    )
}

fn apply_2551q_fill(draft: &mut Form2551QDraft, key: &str, value: &Value) -> Result<(), String> {
    let text = value_as_text(value)?;
    match key {
        "creditable_tax_withheld" | ids::FORM_2551Q_CREDITABLE => {
            draft.creditable_tax_withheld = parse_money(&text)?;
        }
        "other_tax_credit" | ids::FORM_2551Q_OTHER_CREDIT => {
            draft.other_tax_credit = parse_money(&text)?;
        }
        "taxable_amount" | "schedule_1.0.taxable_amount" | ids::FORM_2551Q_TAXABLE_0 => {
            let amount = parse_money(&text)?;
            if let Some(row) = draft.schedule_1.first_mut() {
                row.taxable_amount = amount;
                row.recompute();
            } else {
                return Err("2551Q schedule 1 has no rows".into());
            }
        }
        _ => return Err(format!("unknown 2551Q field `{key}`")),
    }
    Ok(())
}

fn value_as_text(value: &Value) -> Result<String, String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Bool(flag) => Ok(flag.to_string()),
        _ => Err("field values must be strings or numbers".into()),
    }
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
        let palette = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "search.open".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(!palette.ok);
        let tree = host.tree();
        assert!(tree.find(ids::PAGE_LOCK).is_some());
        assert!(tree.find(ids::PAGE_SETTINGS).is_none());
        assert!(tree.find(ids::OVERLAY_COMMAND_PALETTE).is_none());
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
        assert_eq!(
            host.tree()
                .find(ids::CONTEXT_SELECTED_TIN)
                .and_then(|node| node.value.as_deref()),
            Some("98765432100000")
        );
    }

    #[test]
    fn selected_profile_is_checked_listitem_and_context_tin() {
        let host = fixture_host();
        let tree = host.tree();
        let row = tree
            .find(&ids::profile_row(FIXTURE_TIN))
            .expect("selected profile listitem");
        assert_eq!(row.role, "listitem");
        assert_eq!(row.checked, Some(true));
        assert_eq!(
            tree.find(ids::CONTEXT_SELECTED_TIN)
                .and_then(|node| node.value.as_deref()),
            Some(FIXTURE_TIN)
        );
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
                name: "filing.start".into(),
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

    #[test]
    fn profile_search_set_ambiguous_and_not_found() {
        let mut host = fixture_host();
        let extra = {
            let mut profile = fixture_profile();
            profile.full_name = "Agent Other Shop".into();
            profile.tin = bir_core::naming::Tin {
                segment1: "987".into(),
                segment2: "654".into(),
                segment3: "321".into(),
                branch: "00000".into(),
            };
            profile
        };
        {
            let db = host.db.as_ref().expect("db").clone();
            db.lock()
                .unwrap()
                .save_profile(extra)
                .expect("second profile");
            host.reload_from_db(&db);
        }

        let listed = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.list".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(listed.ok, "{:?}", listed.error);
        let profiles = listed.result.as_ref().expect("profiles");
        assert!(profiles.as_array().map(|rows| rows.len()).unwrap_or(0) >= 2);

        let none = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.set".into(),
                args: json!({ "q": "zzz-no-such-taxpayer" }),
            }),
            None,
        );
        assert!(none.ok, "{:?}", none.error);
        assert_eq!(none.result.as_ref().unwrap()["status"], "not_found");

        let ambiguous = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.set".into(),
                args: json!({ "q": "agent" }),
            }),
            None,
        );
        assert!(ambiguous.ok, "{:?}", ambiguous.error);
        assert_eq!(ambiguous.result.as_ref().unwrap()["status"], "ambiguous");
        assert!(
            ambiguous.result.as_ref().unwrap()["candidates"]
                .as_array()
                .map(|rows| rows.len())
                .unwrap_or(0)
                >= 2
        );
        assert_eq!(host.selected_tin(), Some(FIXTURE_TIN));

        let set = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.set".into(),
                args: json!({ "q": "Other Shop", "view": "profile-manager" }),
            }),
            None,
        );
        assert!(set.ok, "{:?}", set.error);
        assert_eq!(set.result.as_ref().unwrap()["status"], "ok");
        assert_eq!(host.active_view(), ActiveView::ProfileManager);
        assert_eq!(host.selected_tin(), Some("98765432100000"));
        assert_eq!(host.editor_snapshot().full_name, "Agent Other Shop");
        assert!(host.tree().find(ids::PROFILE_TAB_TAX).is_some());
        assert!(host.tree().find(ids::CONTEXT_SELECTED_TIN).is_some());
        assert_eq!(
            host.tree()
                .find(ids::CONTEXT_SELECTED_TIN)
                .and_then(|node| node.value.as_deref()),
            Some("98765432100000")
        );
    }

    #[test]
    fn profile_edit_stays_on_profile_manager() {
        let mut host = fixture_host();
        assert_eq!(host.active_view(), ActiveView::Dashboard);
        let edited = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.edit".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(edited.ok, "{:?}", edited.error);
        assert_eq!(host.active_view(), ActiveView::ProfileManager);
        assert_eq!(host.editor_snapshot().tin, FIXTURE_TIN);
        let tab = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.tab".into(),
                args: json!({ "tab": "security" }),
            }),
            None,
        );
        assert!(tab.ok, "{:?}", tab.error);
        assert_eq!(host.profile_tab(), ids::ProfileManagerTab::Security);
        assert!(host.tree().find(ids::PROFILE_SECTION_SECURITY).is_some());
    }

    #[test]
    fn dues_list_filters_upcoming_and_overdue() {
        let mut host = fixture_host();
        let upcoming = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "dues.list".into(),
                args: json!({ "filter": "upcoming", "scope": "profile" }),
            }),
            None,
        );
        assert!(upcoming.ok, "{:?}", upcoming.error);
        let overdue = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "dues.list".into(),
                args: json!({ "filter": "overdue", "scope": "profile" }),
            }),
            None,
        );
        assert!(overdue.ok, "{:?}", overdue.error);
        let all = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "dues.list".into(),
                args: json!({ "filter": "all", "scope": "profile" }),
            }),
            None,
        );
        assert!(all.ok, "{:?}", all.error);
        let all_count = all.result.as_ref().unwrap()["dues"]
            .as_array()
            .map(|rows| rows.len())
            .unwrap_or(0);
        let up_count = upcoming.result.as_ref().unwrap()["dues"]
            .as_array()
            .map(|rows| rows.len())
            .unwrap_or(0);
        let over_count = overdue.result.as_ref().unwrap()["dues"]
            .as_array()
            .map(|rows| rows.len())
            .unwrap_or(0);
        assert_eq!(all_count, up_count + over_count);
        let global = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "dues.list".into(),
                args: json!({ "filter": "all", "scope": "global" }),
            }),
            None,
        );
        assert!(global.ok, "{:?}", global.error);
        assert!(
            global.result.as_ref().unwrap()["date_basis"]
                .as_str()
                .unwrap()
                .contains("local-calendar-date")
        );
    }

    #[test]
    fn jobs_list_is_read_only() {
        let mut host = fixture_host();
        {
            let db = host.db.as_ref().expect("db").clone();
            db.lock()
                .unwrap()
                .save_job(bir_core::db::Job {
                    id: None,
                    name: "Nightly sync".into(),
                    job_type: "Custom".into(),
                    cron_expr: None,
                    command: None,
                    status: "Idle".into(),
                    retries: 0,
                    last_run_at: None,
                    next_run_at: None,
                    created_at: String::new(),
                    output_log: None,
                })
                .expect("job");
        }
        let listed = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "jobs.list".into(),
                args: json!({ "status": "Idle" }),
            }),
            None,
        );
        assert!(listed.ok, "{:?}", listed.error);
        let jobs = listed.result.as_ref().unwrap()["jobs"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            jobs.iter()
                .any(|job| job["name"] == "Nightly sync" && job["status"] == "Idle")
        );
        handle_request(
            &mut host,
            req(Op::Invoke {
                name: "nav.go".into(),
                args: json!({ "page": "cron-tasks" }),
            }),
            None,
        );
        let _ = host.reload_jobs_and_submissions();
        assert!(host.tree().find("job-1").is_some() || host.tree().find(ids::JOBS_LIST).is_some());
    }

    #[test]
    fn palette_search_does_not_create() {
        let ranked = handle_request(
            &mut empty_host(),
            req(Op::Invoke {
                name: "palette.search".into(),
                args: json!({ "q": "brand new taxpayer" }),
            }),
            None,
        );
        assert!(ranked.ok, "{:?}", ranked.error);
        assert_eq!(ranked.result.as_ref().unwrap()["can_create"], true);
        assert_eq!(
            ranked.result.as_ref().unwrap()["create_query"],
            "brand new taxpayer"
        );
        assert!(
            ranked.result.as_ref().unwrap()["matches"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn search_open_and_palette_open_show_overlay_without_creating() {
        let mut host = empty_host();
        assert!(host.tree().find(ids::OVERLAY_COMMAND_PALETTE).is_none());
        let opened = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "search.open".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(opened.ok, "{:?}", opened.error);
        assert_eq!(opened.result.as_ref().unwrap()["open"], true);
        assert_eq!(
            opened.result.as_ref().unwrap()["id"],
            ids::OVERLAY_COMMAND_PALETTE
        );
        assert!(host.tree().find(ids::OVERLAY_COMMAND_PALETTE).is_some());
        let ranked = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "palette.search".into(),
                args: json!({ "q": "brand new taxpayer" }),
            }),
            None,
        );
        assert!(ranked.ok, "{:?}", ranked.error);
        assert_eq!(ranked.result.as_ref().unwrap()["can_create"], true);
        assert!(
            ranked.result.as_ref().unwrap()["matches"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(host.tree().find(ids::OVERLAY_COMMAND_PALETTE).is_some());
        let listed = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.list".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(listed.ok, "{:?}", listed.error);
        assert!(
            listed
                .result
                .as_ref()
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );

        let mut alias_host = empty_host();
        let palette = handle_request(
            &mut alias_host,
            req(Op::Invoke {
                name: "palette.open".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(palette.ok, "{:?}", palette.error);
        assert_eq!(palette.result.as_ref().unwrap()["open"], true);
        assert!(
            alias_host
                .tree()
                .find(ids::OVERLAY_COMMAND_PALETTE)
                .is_some()
        );
    }

    #[test]
    fn palette_search_does_not_require_the_overlay() {
        let mut host = empty_host();
        assert!(host.tree().find(ids::OVERLAY_COMMAND_PALETTE).is_none());
        let ranked = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "palette.search".into(),
                args: json!({ "q": "fixture" }),
            }),
            None,
        );
        assert!(ranked.ok, "{:?}", ranked.error);
        assert!(host.tree().find(ids::OVERLAY_COMMAND_PALETTE).is_none());
    }

    #[test]
    fn invoke_aliases_share_handlers() {
        let mut host = fixture_host();
        let year = chrono::Local::now().year() as u16;
        let opened = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.open".into(),
                args: json!({ "code": "1601C", "year": year, "period": 1 }),
            }),
            None,
        );
        assert!(opened.ok, "{:?}", opened.error);

        let pdf = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.pdf".into(),
                args: json!({}),
            }),
            None,
        );
        let preview = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.preview_pdf".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(pdf.ok, "{:?}", pdf.error);
        assert!(preview.ok, "{:?}", preview.error);
        assert_eq!(pdf.result.as_ref().unwrap()["kind"], "frozen-html");
        assert_eq!(preview.result.as_ref().unwrap()["kind"], "frozen-html");
        assert_eq!(pdf.result.as_ref().unwrap()["form"], "1601C");
        assert_eq!(preview.result.as_ref().unwrap()["form"], "1601C");
        let pdf_path = pdf.result.as_ref().unwrap()["path"].as_str().expect("path");
        let preview_path = preview.result.as_ref().unwrap()["path"]
            .as_str()
            .expect("path");
        assert!(std::path::Path::new(pdf_path).is_absolute());
        assert!(std::path::Path::new(preview_path).is_absolute());

        let receipt = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.upload_receipt".into(),
                args: json!({}),
            }),
            None,
        );
        let receipt_alias = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "receipt.upload".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(receipt.ok, "{:?}", receipt.error);
        assert!(receipt_alias.ok, "{:?}", receipt_alias.error);
        assert_eq!(receipt.result.as_ref().unwrap()["status"], "needs_file");
        assert_eq!(
            receipt_alias.result.as_ref().unwrap()["status"],
            "needs_file"
        );
        assert!(receipt.result.as_ref().unwrap()["path"].is_null());
        assert!(receipt_alias.result.as_ref().unwrap()["path"].is_null());

        let paid = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.mark_paid".into(),
                args: json!({}),
            }),
            None,
        );
        let paid_alias = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "payment.mark_paid".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(paid.ok, "{:?}", paid.error);
        assert!(paid_alias.ok, "{:?}", paid_alias.error);
        assert_eq!(paid.result.as_ref().unwrap()["status"], "unsupported");
        assert_eq!(paid_alias.result.as_ref().unwrap()["status"], "unsupported");

        let revert = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.revert_draft".into(),
                args: json!({}),
            }),
            None,
        );
        let revert_alias = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "draft.revert".into(),
                args: json!({}),
            }),
            None,
        );
        assert_eq!(revert.ok, revert_alias.ok);
        assert_eq!(revert.error, revert_alias.error);
    }

    #[test]
    fn profile_create_stays_confirm_gated_and_ensure_is_not_auto_write() {
        let mut host = empty_host();
        let created = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.create".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(created.ok, "{:?}", created.error);
        assert_eq!(host.active_view(), ActiveView::ProfileManager);
        assert!(host.editor_snapshot().tin.is_empty());
        assert!(host.editor_snapshot().full_name.is_empty());
        let listed = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.list".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(listed.ok, "{:?}", listed.error);
        assert!(
            listed
                .result
                .as_ref()
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );

        let ensure = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.ensure".into(),
                args: json!({ "q": "should not be written" }),
            }),
            None,
        );
        assert!(!ensure.ok);
        let err = ensure.error.as_deref().unwrap_or_default();
        assert!(err.contains("profile.ensure is not implemented"), "{err:?}");
        let listed_after = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.list".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(
            listed_after
                .result
                .as_ref()
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn form_fill_fields_pdf_and_2551q_draft() {
        let mut host = fixture_host();
        let year = chrono::Local::now().year() as u16;
        let opened = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "filing.start".into(),
                args: json!({ "code": "1601C", "year": year, "period": 1 }),
            }),
            None,
        );
        assert!(opened.ok, "{:?}", opened.error);
        let filled = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.fill".into(),
                args: json!({ "fields": { "tax_14": "1000.00", "tax_25": "100.00" } }),
            }),
            None,
        );
        assert!(filled.ok, "{:?}", filled.error);
        let unknown = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.fill".into(),
                args: json!({ "fields": { "not_a_field": "1" } }),
            }),
            None,
        );
        assert!(!unknown.ok);
        let fields = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.fields".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(fields.ok, "{:?}", fields.error);
        let pdf = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.pdf".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(pdf.ok, "{:?}", pdf.error);
        let path = pdf.result.as_ref().unwrap()["path"].as_str().expect("path");
        assert!(std::path::Path::new(path).is_absolute());
        assert!(std::fs::read_to_string(path).unwrap().contains("<html"));
        assert_eq!(pdf.result.as_ref().unwrap()["kind"], "frozen-html");
        let print = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.print".into(),
                args: json!({ "copies": 2 }),
            }),
            None,
        );
        assert!(!print.ok);

        let opened_q = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "filing.start".into(),
                args: json!({ "code": "2551Q", "year": year, "period": 1 }),
            }),
            None,
        );
        assert!(opened_q.ok, "{:?}", opened_q.error);
        let filled_q = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.fill".into(),
                args: json!({ "fields": { "taxable_amount": "500.00" } }),
            }),
            None,
        );
        assert!(filled_q.ok, "{:?}", filled_q.error);
        let saved = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.save_draft".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(saved.ok, "{:?}", saved.error);
        let receipt = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.upload_receipt".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(receipt.ok, "{:?}", receipt.error);
        assert_eq!(receipt.result.as_ref().unwrap()["status"], "needs_file");
        assert!(receipt.result.as_ref().unwrap()["path"].is_null());
        let paid = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "form.mark_paid".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(!paid.ok);
        let sync = handle_request(
            &mut host,
            req(Op::Invoke {
                name: "profile.calendar_sync".into(),
                args: json!({}),
            }),
            None,
        );
        assert!(!sync.ok);
    }

    #[test]
    fn hello_auth_still_required_when_token_configured() {
        let mut host = empty_host();
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
        let missing = handle_request(&mut host, req(Op::Hello), Some("dev-secret"));
        assert!(!missing.ok);
    }
}
