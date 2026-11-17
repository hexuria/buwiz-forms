//! The host's JSON shapes as serde structs (mirrors `bir/types.d.ts`). Fields keep the host's snake_case
//! names; `#[serde(default)]` everywhere so a newer host field never breaks an older read.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HotkeyPreset {
    pub label: String,
    pub combo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub app_version: String,
    pub database_path: String,
    pub data_dir: String,
    pub logs_dir: String,
    /// Null when no global show chord is stored.
    pub global_hotkey: Option<String>,
    pub hotkey_presets: Vec<HotkeyPreset>,
    pub log_retention_days: i64,
    pub log_retention_choices: Vec<i64>,
    pub runtime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProfileSummary {
    pub id: i64,
    pub tin: String,
    pub full_name: String,
    pub rdo_code: String,
    pub email: String,
    pub taxpayer_type: String,
    pub is_vat_registered: bool,
    pub is_archived: bool,
    pub default_form_type: String,
    pub applicable_forms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FormCatalogEntry {
    pub code: String,
    pub title: String,
    pub category: String,
    pub frequency: String,
    pub requires_employees: bool,
    pub is_deprecated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AuthState {
    /// null on a fresh install that has not chosen yet.
    pub mode: Option<String>,
    pub admin_root: Option<String>,
    pub has_admin_pin: bool,
    pub has_admin_totp: bool,
    pub has_recovery_key: bool,
    /// Touch ID / account password (macOS) or Windows Hello can confirm the admin.
    pub mac_login_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProfileAuth {
    pub has_pin: bool,
    pub has_totp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BranchFacts {
    pub rdo_code: Option<String>,
    pub registered_address: Option<String>,
    pub zip_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Branch {
    pub tin: String,
    pub tin_display: String,
    pub tin_root: String,
    pub code: String,
    pub name: String,
    pub is_head_office: bool,
    pub facts: BranchFacts,
    pub inherited: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Taxpayer {
    pub tin_root: String,
    pub tin_display: String,
    pub name: String,
    pub taxpayer_type: String,
    pub birth_date: Option<String>,
    pub needs_grouping_review: bool,
    pub archived: bool,
    pub branches: Vec<Branch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TaxYearBounds {
    pub min: i64,
    pub max: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AddedTaxYear {
    pub year: i64,
    pub copied_from: Option<i64>,
    pub forms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct YearlyFormsChange {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub forms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Setup {
    pub id: String,
    pub name: String,
    pub description: String,
    pub taxpayer_type: String,
    pub built_in: bool,
    pub forms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SetupApplied {
    pub forms: Vec<String>,
    /// Facts written.
    pub set: Vec<String>,
    /// Facts already set to something else and left as they are.
    pub kept: Vec<String>,
    pub election_locked: bool,
    /// Templates copied (copy_setup_from only).
    pub templates_copied: Vec<String>,
    /// Templates the target already had and kept.
    pub templates_kept: Vec<String>,
    /// Forms that were on the list, are not in the setup, and stay on it.
    pub forms_kept: Vec<String>,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FormTemplateSummary {
    pub form_code: String,
    pub updated_at: String,
    pub answers_kept: i64,
    pub marked: i64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormIndexEntry {
    pub c: String,
    pub t: String,
    pub rev: String,
    pub freq: String,
    pub pages: i64,
    pub n: i64,
    pub q: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormError {
    pub field: String,
    pub message: String,
    pub keys: Vec<String>,
    pub dismiss: Option<String>,
    pub dismiss_label: Option<String>,
}

/// A box the form offers as a choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormOption {
    pub value: String,
    pub label: String,
    /// Selected by the typed draft when multiple choices share a code.
    pub selected: Option<bool>,
}

/// One-time notes this opening offers (`key`, `text`); show the ones `screen_form_notes_seen` records.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormNote {
    pub key: String,
    pub text: String,
}

/// A sample's period parts: page inputs (never draft writes) the filer sets the period with.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormPeriodPart {
    pub key: String,
    pub part: String,
    /// For a quarter's tick box: which quarter.
    pub quarter: Option<i64>,
}

/// Each party's Surcharge, Interest and Compromise boxes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormPenalties {
    pub keys: Vec<String>,
    /// The filer entered the amounts: they file instead of the computed ones.
    pub edited: bool,
    /// The amounts are kept as entered before penalties were computed.
    pub kept: Option<bool>,
    /// The party is on the return as it stands.
    pub applies: bool,
}

/// A Draft's tax facts that are not set and change what it files.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormFactError {
    pub field: String,
    pub message: String,
}

/// A Draft's cards: one per fact its form reads that the tax profile has not set for its year.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormAsk {
    pub fact: String,
    /// The box the card sits at; null: the top of the first part.
    pub key: Option<String>,
    pub label: String,
    pub control: String,
    /// A choice card's answers: `value` is what `set_year_facts` takes.
    pub choices: Option<Vec<AskChoice>>,
    /// The card's sentence.
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AskChoice {
    pub value: serde_json::Value,
    pub label: String,
}

/// A filled profile box Edit may open that "Save to profile too" can write back.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProfileSaveField {
    pub field: String,
    pub label: String,
    pub note: String,
}

/// An amended return's filed version, kept in its history.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FiledVersion {
    pub id: i64,
    pub status: String,
    pub filed_on: Option<String>,
}

/// A quiet date message the page holds until it is edited or Check / Save is pressed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormQuiet {
    pub field: String,
    pub keys: Vec<String>,
}

/// What `screen_form_open` answers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormOpened {
    pub form_code: String,
    pub year: i64,
    pub period: i64,
    pub status: String,
    pub editable: bool,
    /// The period column the draft is stored under: pass it to queue and print.
    pub stored_period: Option<i64>,
    pub draft: serde_json::Value,
    pub values: serde_json::Map<String, serde_json::Value>,
    pub options: serde_json::Map<String, serde_json::Value>,
    pub locks: serde_json::Map<String, serde_json::Value>,
    /// Boxes the taxpayer profile filled.
    pub profile: Vec<String>,
    /// Filled profile boxes the filer may open with Edit.
    pub profile_edit: Vec<String>,
    /// Opened from the Form library: a blank sample.
    pub sample: bool,
    /// A sample's boxes shown blank while its period is not set.
    pub blank: Vec<String>,
    /// A sample's held box that cannot be typed: what it waits for.
    pub needs: serde_json::Map<String, serde_json::Value>,
    /// A printed box the filer cannot fill here, while it shows blank: why.
    pub whys: serde_json::Map<String, serde_json::Value>,
    /// A blank box's reason, shown in the box.
    pub waits: serde_json::Map<String, serde_json::Value>,
    /// An untouched date whose message the page holds until it is edited or Check / Save is pressed.
    pub quiet: Option<ScreenFormQuiet>,
    /// Why each locked box is shut, in the filer's words.
    pub lock_notes: serde_json::Map<String, serde_json::Value>,
    /// A sample's period parts.
    pub period_parts: Vec<ScreenFormPeriodPart>,
    /// A sample rebuilt for a period its filer completed.
    pub period_set: bool,
    /// An amended return's filed version, kept in its history.
    pub filed_original: Option<FiledVersion>,
    /// Set on the read-only view of a filed original.
    pub filed_original_view: Option<FiledVersion>,
    /// When a return was sent to BIR and confirmed.
    pub submitted_at: Option<String>,
    pub confirmed_at: Option<String>,
    /// A new amended return: the status of the filed return it amends.
    pub amends: Option<String>,
    /// Boxes another answer turns off right now, each with its tag.
    pub off: serde_json::Map<String, serde_json::Value>,
    /// Each party's Surcharge, Interest and Compromise boxes.
    pub penalties: Vec<ScreenFormPenalties>,
    /// Each pooled address (by its first line) as the draft holds it.
    pub pools: serde_json::Map<String, serde_json::Value>,
    /// A stored Draft whose profile boxes the filer did not edit followed a later tax-profile change.
    pub profile_updates: Vec<String>,
    /// A Draft saved before it recorded its profile baseline.
    pub profile_differs: Vec<String>,
    /// One-time notes this opening offers.
    pub notes: Vec<ScreenFormNote>,
    /// Boxes filled from the previous return or from the template when this return was created.
    pub kept: serde_json::Map<String, serde_json::Value>,
    /// Automatic stops as box keys.
    pub auto_stops: Vec<String>,
    /// The user's own "Stop here" switches for this branch and form.
    pub stop_edits: serde_json::Map<String, serde_json::Value>,
    /// Boxes that may carry a "Stop here" switch.
    pub stoppable: Vec<String>,
    pub errors: Vec<ScreenFormError>,
    /// A Draft's tax facts that are not set and change what it files.
    pub fact_errors: Vec<ScreenFormError>,
    /// A Draft's cards: one per fact its form reads that the tax profile has not set.
    pub asks: Vec<ScreenFormAsk>,
    /// The filled profile boxes Edit may open that "Save to profile too" can write back.
    pub profile_save: serde_json::Map<String, serde_json::Value>,
    /// A return past Queued: what the tax profile changed after it was filed.
    pub profile_changed: Vec<String>,
    /// Lock notes that always show, not only while the box is blank.
    pub notes_always: Vec<String>,
}

/// What `screen_form_fill` answers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormFilled {
    pub draft: serde_json::Value,
    pub values: serde_json::Map<String, serde_json::Value>,
    pub options: serde_json::Map<String, serde_json::Value>,
    pub rejected: serde_json::Map<String, serde_json::Value>,
    pub penalties: Vec<ScreenFormPenalties>,
    /// Each pooled address (by its first line) as the draft holds it.
    pub pools: serde_json::Map<String, serde_json::Value>,
    /// A sample's boxes still blank (null for a return).
    pub blank: Option<Vec<String>>,
    /// Rows the filer typed that this edit removed.
    pub removes: Option<Removal>,
    /// Locks this fill opened or shut.
    pub relocks: serde_json::Map<String, serde_json::Value>,
    pub relock_notes: serde_json::Map<String, serde_json::Value>,
    /// Boxes an emptied number box was cleared in (the session's own rule).
    #[serde(skip)]
    pub cleared: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Removal {
    pub message: String,
    pub done: String,
    pub rows: i64,
}

/// What `screen_form_validate` answers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormValidated {
    pub draft: serde_json::Value,
    pub values: serde_json::Map<String, serde_json::Value>,
    pub options: serde_json::Map<String, serde_json::Value>,
    pub errors: Vec<ScreenFormError>,
    /// Boxes another answer turns off right now, each with its tag.
    pub off: serde_json::Map<String, serde_json::Value>,
    pub penalties: Vec<ScreenFormPenalties>,
    pub pools: serde_json::Map<String, serde_json::Value>,
    pub blank: Option<Vec<String>>,
    pub fact_errors: Vec<ScreenFormError>,
    pub notes_always: Vec<String>,
}

/// What `screen_form_save` answers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormSaved {
    /// A Form library sample: false when its period already has a stored return.
    pub saved: Option<bool>,
    pub existing: Option<ScreenFormExisting>,
    /// A saved sample: open the return it became with these.
    pub opens: Option<OpenTarget>,
    pub id: Option<i64>,
    pub status: Option<String>,
    pub stored_period: Option<i64>,
    pub draft: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OpenTarget {
    pub year: i64,
    pub period: i64,
    pub key: Option<i64>,
    pub new_key: Option<i64>,
    pub new_key_from: Option<String>,
}

/// A stored return of the form already has this period.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormExisting {
    pub form_code: String,
    pub year: i64,
    pub period: i64,
    pub label: String,
    pub status: String,
    pub updated_at: Option<String>,
    pub total_payable: Option<f64>,
    /// What the page may offer: `load`, `view`, `amend`, `keep`.
    pub actions: Vec<String>,
    pub submitted_at: Option<String>,
    pub confirmed_at: Option<String>,
}

/// What `screen_form_sample_period` answers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenFormSamplePeriod {
    pub complete: bool,
    /// A stored return of the form already has this period.
    pub existing: Option<ScreenFormExisting>,
    /// No stored return: a fresh sample built for exactly this period.
    pub sample: Option<ScreenFormOpened>,
    /// What `screen_form_open` takes for it.
    pub opens: Option<OpenTarget>,
}

/// Whether record_income_tax_election would accept the year, and if not what the tax profile lacks.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct IncomeTaxElectionReadiness {
    pub taxable_year: i64,
    pub gap: String,
    pub taxpayer_type: Option<String>,
    pub classification: Option<String>,
    pub since: Option<String>,
    pub new_registration_after: Option<String>,
    pub issues: Vec<String>,
}

/// screen_form_queue_check's answer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct QueueCheck {
    pub blocks: Vec<ScreenFormError>,
    pub warns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DeadlineRow {
    pub tin: String,
    pub tin_root: String,
    pub branch_code: String,
    pub branch_name: String,
    pub taxpayer_name: String,
    pub branch_count: i64,
    pub form_code: String,
    pub form_name: String,
    pub frequency: String,
    pub taxable_year: i64,
    pub period: i64,
    pub period_label: String,
    pub due_date: String,
    pub status: String,
    pub draft_id: Option<i64>,
    pub return_key: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AsNeededReturn {
    pub draft_id: i64,
    pub period: i64,
    pub return_key: Option<i64>,
    pub status: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AsNeededForm {
    pub tin: String,
    pub tin_root: String,
    pub branch_code: String,
    pub branch_name: String,
    pub taxpayer_name: String,
    pub branch_count: i64,
    pub form_code: String,
    pub form_name: String,
    pub taxable_year: i64,
    pub trigger: Option<String>,
    pub due_rule: String,
    pub undated: Vec<AsNeededReturn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ReturnRow {
    pub id: i64,
    pub tin: String,
    pub tin_root: String,
    pub branch_code: String,
    pub branch_name: String,
    pub taxpayer_name: String,
    pub branch_count: i64,
    pub form_code: String,
    pub taxable_year: i64,
    pub period: i64,
    pub period_label: String,
    pub status: String,
    pub updated_at: String,
    pub return_key: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DraftSummary {
    pub id: i64,
    pub tin: String,
    pub tin_root: String,
    pub branch_code: String,
    pub branch_name: String,
    pub taxpayer_name: String,
    pub form_code: String,
    pub taxable_year: i64,
    pub period: i64,
    pub period_label: String,
    pub status: String,
    pub updated_at: String,
    pub return_key: Option<i64>,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TaxDeadline {
    pub tin: String,
    pub tin_root: String,
    pub branch_code: String,
    pub form_code: String,
    pub taxable_year: i64,
    pub period: i64,
    pub due_date: String,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BackgroundJob {
    pub kind: String,
    pub id: Option<i64>,
    pub tin: Option<String>,
    pub form_code: Option<String>,
    pub taxable_year: Option<i64>,
    pub name: String,
    pub job_type: String,
    pub cron_expr: Option<String>,
    pub status: String,
    pub retries: i64,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub created_at: String,
    pub output_log: Option<String>,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PrintPreview {
    pub id: String,
    pub title: String,
    pub pages: i64,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

/// A standing send OK for one form (`SendGrant`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SendGrant {
    pub tin: String,
    pub form: String,
    pub granted_at: String,
}

/// A tax-type chip (`TaxTypeChip`): the tax types the year's forms list gives.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TaxTypeChip {
    #[serde(rename = "type")]
    pub kind: String,
    pub label: String,
    pub forms: Vec<String>,
}

/// The confirmation-inbox state (`InboxState`); only the fields the screens read are named.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct InboxState {
    pub tin: String,
    pub inbox_email: String,
    pub auth_method: String,
    pub tracking_enabled: bool,
    pub imap_login: String,
    pub imap_host: String,
    pub has_app_password: bool,
    pub oauth_connected: bool,
    pub oauth_inbox_email: Option<String>,
    pub still_pending: bool,
    pub last_check_at: Option<String>,
    pub last_check_status: Option<String>,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}
