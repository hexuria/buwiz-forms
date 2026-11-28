//! Tax-profile host functions: taxpayers, branches, tax years, yearly forms,
//! starter packs and form templates. Every rule (TIN shape, tax-year bound,
//! one template per form, the carry rule a snapshot pins by, the one-transaction TIN
//! move) lives in bir-core; this file only converts JSON in and out. The
//! account's TINs (`account_tins`, `activate_tin`) are read and added
//! through bir-submit's client.

use std::collections::{BTreeMap, BTreeSet};

use bir_core::bir_submit::tins::{AccountTin, AccountTins, Next, TinKind};
use bir_core::bir_submit::{Submitter, TinKey};
use bir_core::db::Database;
use bir_core::forms::agent::AgentFormKind;
use bir_core::forms::agent::carry;
use bir_core::forms::registry::canonical_form_code;
use bir_core::profile::{
    EmailAuthMethod, IncomeTaxElection, TaxClassification, TaxpayerProfile, TaxpayerType,
};
use bir_core::tax_profile::{
    BranchFacts, BranchField, BranchUpdate, FieldSetting, FormTemplate, NewBranch, NewTaxpayer,
    ResolvedBranch, TaxProfileError, TaxpayerUpdate, TaxpayerWithBranches, format_tin_root,
    tax_year_bounds,
};
use chrono::Datelike as _;
use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};
use serde_json::Value;

use crate::bir_module::{database_handle, to_json, with_database, with_reader};

/// TypeScript appended to the `bir` module's declarations.
pub const DECLARATIONS: &str = r#"
export type BranchFieldName = "rdo_code" | "registered_address" | "zip_code" | "phone" | "email";
export interface BranchFacts {
    rdo_code: string;
    registered_address: string;
    zip_code: string;
    phone: string;
    email: string;
}
export interface Branch {
    tin: string;
    tin_display: string;
    tin_root: string;
    code: string;
    name: string;
    is_head_office: boolean;
    facts: BranchFacts;
    inherited: BranchFieldName[];
}
export interface Taxpayer {
    tin_root: string;
    tin_display: string;
    name: string;
    taxpayer_type: "Individual" | "Corporation" | "Partnership" | "Cooperative" | "Estate" | "Trust";
    birth_date: string | null;
    needs_grouping_review: boolean;
    /** Every branch is archived (set_taxpayer_archived). Archived taxpayers stay listed. */
    archived: boolean;
    branches: Branch[];
}
export interface NewTaxpayerInput {
    tin_root: string;
    name: string;
    taxpayer_type: Taxpayer["taxpayer_type"];
    birth_date?: string | null;
    head_office: Partial<BranchFacts>;
}
export interface TaxpayerUpdateInput {
    name?: string;
    taxpayer_type?: Taxpayer["taxpayer_type"];
    /** "YYYY-MM-DD", or null to clear. Omitted: unchanged. */
    birth_date?: string | null;
}
export interface NewBranchInput {
    branch_code: string;
    name: string;
    fields?: Partial<Record<BranchFieldName, string | null>>;
}
export interface BranchUpdateInput {
    name?: string;
    fields?: Partial<Record<BranchFieldName, string | null>>;
}
export interface TaxYearBounds { min: number; max: number; }
export interface AddedTaxYear { year: number; copied_from: number | null; forms: string[]; }
export interface YearlyFormsChange { added: string[]; removed: string[]; forms: string[]; }
/** A setup (decision 14): the forms of a year and the facts they read. Never a date, a line of business or anything personal. */
export interface Setup {
    /** A built-in id such as "non_vat_professional", or "user-<n>" for one the user saved. */
    id: string;
    name: string;
    taxpayer_types: Taxpayer["taxpayer_type"][];
    forms: string[];
    facts: {
        tax_classification: YearFactsView["facts"]["tax_classification"];
        income_tax_election: YearFactsView["facts"]["income_tax_election"];
        eopt_tier: YearFactsView["facts"]["eopt_tier"];
        business_start_date: null;
        line_of_business: null;
        is_government_withholding_entity: boolean | null;
        atc_codes: string[];
    };
    built_in: boolean;
    /** The forms are added to the year's list instead of replacing it (Employer). */
    adds_forms: boolean;
}
export interface SetupApplied {
    /** The year's forms list afterwards. */
    forms: string[];
    /** Facts written. */
    set: Fact[];
    /** Facts already set to something else and left as they are (without `replace`, or a locked 8% or graduated choice). */
    kept: Fact[];
    election_locked: boolean;
    /** Templates copied (copy_setup_from only). */
    templates_copied: string[];
    /** Templates the target already had and kept (copy_setup_from only). */
    templates_kept: string[];
    /** Forms that were on the list, are not in the setup, and stay on it: without `replace` a setup only adds forms. Empty after a replace. */
    forms_kept: string[];
}
export interface FormTemplateSummary {
    form_code: string;
    updated_at: string;
    /** How many answers the template fixes (box keys it pins). */
    answers_kept: number;
    /** How many "Stop here" switches are on. */
    marked: number;
    source: string;
}
export function list_taxpayers(): Taxpayer[];
export function get_taxpayer(tin_root: string): Taxpayer | null;
/** Refuses TIN root 000000000 (reserved for the help desk admin). */
export function create_taxpayer(input: NewTaxpayerInput): Taxpayer;
export function update_taxpayer(tin_root: string, update: TaxpayerUpdateInput): Taxpayer;
/** Refuses TIN root 000000000; moving the solo owner keeps it the admin. */
export function move_taxpayer_tin(old_root: string, new_root: string): Taxpayer;
export function create_branch(tin_root: string, input: NewBranchInput): Branch;
export function update_branch(tin: string, update: BranchUpdateInput): Branch;
export function move_branch_code(tin: string, new_code: string): Branch;
export function suggest_next_branch_code(tin_root: string): string;
export function tax_year_bounds(): TaxYearBounds;
export function list_tax_years(tin: string): number[];
export function add_tax_year(tin: string, year: number, copy_forms: boolean): AddedTaxYear;
export function yearly_forms(tin: string, year: number): string[];
export function set_yearly_forms(tin: string, year: number, form_codes: string[]): YearlyFormsChange;
/** The setups for a taxpayer type: the built-in ones, then the user's. */
export function list_setups(taxpayer_type: Taxpayer["taxpayer_type"]): Setup[];
/** Set the year's forms (replaced, or added to for an `adds_forms` setup) and the setup's facts. A fact already set to something else is kept unless `replace`; a locked 8% or graduated choice always is. Never touches name, TIN, address or contact. */
export function apply_setup(tin: string, year: number, setup_id: string, replace?: boolean): SetupApplied;
/** Save the branch's year as a setup: its forms and facts, no dates, no line of business, nothing personal. */
export function save_setup(tin: string, year: number, name: string): Setup;
/** Delete one of the user's setups; a built-in one is refused. */
export function delete_setup(id: string): boolean;
/** Start `to_tin`'s year from `from_tin`'s: templates of its forms, its forms list and the facts a setup would hold. What the target already has is kept unless `replace`. */
export function copy_setup_from(from_tin: string, to_tin: string, year: number, replace?: boolean): SetupApplied;
export function list_form_templates(tin: string): FormTemplateSummary[];
export function create_template_from_latest(tin: string, form_code: string): FormTemplateSummary;
export function create_template_from_return(draft_id: number): FormTemplateSummary;
export function delete_form_template(tin: string, form_code: string): boolean;
/** Older form page only (`screen_form_open` now carries `stop_edits` and `stoppable`): `stops` are the boxes switched on as a stop, `markable` the boxes that may be one, radios by each radio's key. */
export interface TemplateStops { stops: string[]; markable: string[]; }
/** null when the branch has no template for the form. */
export function template_stops(tin: string, form_code: string): TemplateStops | null;
/** "Stop here": on true = always a stop, false = never a stop, null = back to automatic. Creates the template row when the branch has none for the form. Works for every form; a radio is switched as its item's group box. Returns the new switches, a radio item's for each of its radios (and, for the older page, `stops` and `markable`). */
export function set_template_stop(tin: string, form_code: string, box_key: string, on: boolean | null): { stop_edits: Record<string, boolean> } & TemplateStops;
/** The annual income-tax election for one taxable year, on the head office's year facts; refused for a year whose income type is not self-employed or mixed income, and for a change once a return carrying it was filed. */
export function record_income_tax_election(tin: string, year: number, election: "graduated" | "eight_percent"): boolean;
export type IncomeTaxElectionGap = "ready" | "not_individual" | "not_business_income";
export type IndividualIncomeType = "PurelyCompensation" | "SelfEmployed" | "MixedIncome";
/** Whether record_income_tax_election would accept the year, and if not what the tax profile lacks. A read. */
export interface IncomeTaxElectionReadiness {
    taxable_year: number;
    gap: IncomeTaxElectionGap;
    taxpayer_type: Taxpayer["taxpayer_type"] | null;
    classification: string | null;
    /** Always null: the year facts hold no start date for the income type. */
    since: string | null;
    /** Always null: any date may be confirmed. */
    new_registration_after: string | null;
    /** Always empty. */
    issues: string[];
}
export function income_tax_election_readiness(tin: string, year: number): IncomeTaxElectionReadiness;
/** An individual's income type from `effective_from` (YYYY-MM-DD): the year facts' income type of that date's year, and of every later year that has a record without one. */
export function confirm_income_registration(tin: string, income_type: IndividualIncomeType, effective_from: string): boolean;
export type Fact = "birth_date" | "tax_classification" | "income_tax_election" | "eopt_tier"
    | "business_start_date" | "line_of_business" | "is_government_withholding_entity" | "atc_codes";
export interface YearFactsView {
    tin: string;
    year: number;
    facts: {
        tax_classification: "PurelyCompensation" | "SelfEmployed" | "MixedIncome" | "Corporation" | "CooperativeExempt" | "CooperativeTaxable" | "CooperativeMixed" | "EstateOrTrust" | null;
        income_tax_election: "EightPercent" | "GraduatedUnspecified" | "GraduatedOsd" | "GraduatedItemized" | null;
        eopt_tier: "Micro" | "Small" | "Medium" | "Large" | null;
        /** YYYY-MM-DD */
        business_start_date: string | null;
        line_of_business: string | null;
        is_government_withholding_entity: boolean | null;
        atc_codes: string[];
    };
    /** Which row holds each fact: the head office or this branch. */
    owner: Record<Exclude<Fact, "birth_date">, "head_office" | "branch">;
    /** Form codes on this branch's list for the year that read each fact. Empty list: not in use. */
    used_by: Record<Fact, string[]>;
    /** Facts in use that are not set. */
    not_set: Fact[];
    /** The 8% or graduated choice cannot change: a return carrying it was filed. */
    election_locked: boolean;
    /** Where the year's election was made, e.g. "2551Q Q1" or "Tax profile"; null when none is set. */
    election_source: string | null;
}
export function year_facts(tin: string, year: number): YearFactsView;
/** Sets only the keys present in `patch` (null clears). Head-office facts are written to the head office's record whichever branch `tin` names. Refuses a change to a locked election. */
export function set_year_facts(tin: string, year: number, patch: Partial<YearFactsView["facts"]>): YearFactsView;
export interface TaxTypeChip { type: string; label: string; forms: string[]; }
/** The chips: the tax types the year's forms list gives. */
export function tax_types_for_year(tin: string, year: number): TaxTypeChip[];
/** Groups of two or more annual income tax returns among `codes` that stand for one another (1700, 1701, 1701A, 1701MS; 1702, 1702RT, 1702EX, 1702MX): a taxpayer files one of a group for a year. The forms list keeps whatever the user picks; the page only warns. */
export function annual_return_conflicts(codes: string[]): string[][];
export interface RdoOption { code: string; description: string; }
export function list_rdos(): RdoOption[];
export function list_zip_codes(): string[];

export type InboxAuthMethod = "AppPassword" | "GoogleOAuth";
export interface InboxState {
    tin: string;
    /** Mailbox the poller keys on: imap_email, else the branch email. */
    inbox_email: string;
    auth_method: InboxAuthMethod;
    tracking_enabled: boolean;
    /** The stored IMAP login, "" when the mailbox falls back to the branch email. */
    imap_login: string;
    imap_host: string;
    /** An app password is stored — the secret itself never crosses the bridge. */
    has_app_password: boolean;
    /** A usable Google refresh token exists on the profile or the inbox row. */
    oauth_connected: boolean;
    /** This build can start Google's sign-in for mail (a client id and secret exist); false: offer the app password only. */
    google_available: boolean;
    oauth_inbox_email: string | null;
    oauth_updated_at: string | null;
    /** A sibling on this inbox still has a Submitted return awaiting its BIR receipt. */
    still_pending: boolean;
    matching_tins: string[];
    /** Last run of this inbox's `bir_poll_email` job, when one exists. */
    last_check_at: string | null;
    last_check_status: string | null;
    last_check_log: string | null;
}
export interface InboxCredentialsInput {
    imap_email: string;
    /** Blank means the default imap.gmail.com. */
    imap_host?: string | null;
    /** Omitted keeps the stored password; a string replaces it (spaces stripped). */
    app_password?: string | null;
}
export interface InboxVerifyResult { ok: boolean; message: string; state: InboxState; }
export interface InboxCheckResult {
    ok: boolean;
    still_pending: boolean;
    error: string | null;
    state: InboxState;
}
export function inbox_state(tin: string): InboxState;
/** Stores App Password credentials on the profile (encrypted DB); tracking stays off until Verify succeeds. */
export function set_inbox_app_password(tin: string, input: InboxCredentialsInput): InboxState;
export function set_inbox_tracking(tin: string, enabled: boolean): InboxState;
/** Clears the Google grant for this inbox across every profile sharing it. */
export function disconnect_inbox_oauth(tin: string): InboxState;
/** Runs the browser OAuth flow, then persists the grant to the shared inbox row. */
export function connect_inbox_oauth(tin: string): Promise<InboxState>;
/** Real IMAP login probe: success turns tracking on, failure turns it off. */
export function test_inbox_connection(tin: string): Promise<InboxVerifyResult>;
/** Runs the cron-side fetch→confirm poll for this inbox once, updating its job row. */
export function check_inbox_now(tin: string): Promise<InboxCheckResult>;
/** Standing OK for an assistant to file one form for this branch. The Queue button does not use it. */
export interface SendGrant { tin: string; form: string; granted_at: string; }
export function list_send_grants(tin: string): SendGrant[];
export function grant_send(tin: string, form: string): SendGrant;
export function revoke_send(tin: string, form: string): void;
/** Where a TIN stands for sending on the Buwiz account: "activated" sends, "pending" waits for Buwiz (in review or paused), "fix" has a step on the account site (`action_url`), "not_activated" is a tax profile on this device whose TIN and branch the account does not have. */
export type TinState = "activated" | "pending" | "fix" | "not_activated";
export interface TinStatusRow {
    /** 9 digits. */
    tin_root: string;
    /** 5 digits. */
    branch: string;
    state: TinState;
    /** The server's line for the row; null for "not_activated". */
    message: string | null;
    /** The row's Verify or Fix page, only on the account site and only for "fix"; open it as it is. */
    action_url: string | null;
    /** A test TIN: its sends are dry runs, never sent to the BIR. */
    test_only: boolean;
    /** Answered from an earlier read (offline, or the server asked to wait). */
    stale: boolean;
    /** When the list was read, in seconds since the epoch. */
    fetched_at: number;
}
/** The account's TINs, then a "not_activated" row for each tax profile branch on this device the account lacks: those only when the list is complete. */
export interface TinStatusList {
    rows: TinStatusRow[];
    /** The server listed every state: a TIN missing from it is not on the account. */
    complete: boolean;
    stale: boolean;
    fetched_at: number;
}
/** The account's TINs, from memory when the last read is younger than `max_age` seconds (screens 60, Queue 0). Never on a timer. Throws the reason when there is no list (signed out, offline with no list, the server's refusal). */
export function account_tins(max_age: number): Promise<TinStatusList>;
/** "Activate this TIN to send": adds the tax profile's TIN and branch (14 digits) to the account as not verified, with the profile's kind and registered name. Only from that button, never when a profile is created. The added row (null when the answer could not be read), or throws the refusal's sentence; after one saying the account already holds the TIN, the next account_tins asks the server. */
export function activate_tin(tin_full: string): Promise<TinStatusRow | null>;
"#;

pub fn register(module: HostModule) -> HostModule {
    module
        .function("list_taxpayers", |_| list_taxpayers())
        .function("get_taxpayer", get_taxpayer)
        .function("create_taxpayer", create_taxpayer)
        .function("update_taxpayer", update_taxpayer)
        .function("move_taxpayer_tin", move_taxpayer_tin)
        .function("create_branch", create_branch)
        .function("update_branch", update_branch)
        .function("move_branch_code", move_branch_code)
        .function("suggest_next_branch_code", suggest_next_branch_code)
        .function("tax_year_bounds", |_| tax_year_bounds_fn())
        .function("list_tax_years", list_tax_years)
        .function("add_tax_year", add_tax_year)
        .function("yearly_forms", yearly_forms)
        .function("set_yearly_forms", set_yearly_forms)
        .function("list_setups", list_setups)
        .function("apply_setup", apply_setup)
        .function("save_setup", save_setup)
        .function("delete_setup", delete_setup)
        .function("copy_setup_from", copy_setup_from)
        .function("list_form_templates", list_form_templates)
        .function("create_template_from_latest", create_template_from_latest)
        .function("create_template_from_return", create_template_from_return)
        .function("delete_form_template", delete_form_template)
        .function("template_stops", template_stops)
        .function("set_template_stop", set_template_stop)
        .function("record_income_tax_election", record_income_tax_election)
        .function(
            "income_tax_election_readiness",
            income_tax_election_readiness,
        )
        .function("confirm_income_registration", confirm_income_registration)
        .function("year_facts", year_facts)
        .function("set_year_facts", set_year_facts)
        .function("tax_types_for_year", tax_types_for_year)
        .function("annual_return_conflicts", annual_return_conflicts)
        .function("list_rdos", |_| list_rdos())
        .function("list_zip_codes", |_| list_zip_codes())
        .function("inbox_state", inbox_state)
        .function("set_inbox_app_password", set_inbox_app_password)
        .function("set_inbox_tracking", set_inbox_tracking)
        .function("disconnect_inbox_oauth", disconnect_inbox_oauth)
        .async_function("connect_inbox_oauth", connect_inbox_oauth)
        .async_function("test_inbox_connection", test_inbox_connection)
        .async_function("check_inbox_now", check_inbox_now)
        .function("list_send_grants", list_send_grants)
        .function("grant_send", grant_send)
        .function("revoke_send", revoke_send)
        .async_function("account_tins", account_tins)
        .async_function("activate_tin", activate_tin)
}

// ---------- reference lists ----------

fn list_rdos() -> HostResult {
    Ok(HostValue::Array(
        bir_core::reference::get_all_rdos()
            .into_iter()
            .map(|rdo| {
                HostObject::new()
                    .field("code", rdo.code)
                    .field("description", rdo.description)
                    .into()
            })
            .collect(),
    ))
}

/// Each entry reads "1000 - Manila CPO - Ermita": the code, then the place.
fn list_zip_codes() -> HostResult {
    Ok(strings(bir_core::reference::get_all_zipcodes()))
}

// ---------- conversion helpers ----------

fn host_error(error: TaxProfileError) -> HostError {
    HostError::new(error.to_string())
}

fn strings(values: impl IntoIterator<Item = String>) -> HostValue {
    HostValue::Array(values.into_iter().map(HostValue::Str).collect())
}

fn field_name(field: BranchField) -> String {
    match serde_json::to_value(field) {
        Ok(Value::String(name)) => name,
        _ => format!("{field:?}"),
    }
}

fn parse_field(name: &str) -> Result<BranchField, HostError> {
    serde_json::from_value(Value::String(name.to_string()))
        .map_err(|_| HostError::new(format!("`{name}` is not a branch field")))
}

fn year_argument(arguments: &HostArguments, index: usize) -> Result<u16, HostError> {
    let year = arguments.integer(index)?;
    u16::try_from(year).map_err(|_| HostError::new(format!("{year} is not a tax year")))
}

fn object_argument(arguments: &HostArguments, index: usize) -> Result<Value, HostError> {
    match to_json(arguments.value(index)?) {
        value @ Value::Object(_) => Ok(value),
        _ => Err(HostError::new(format!(
            "argument {} must be an object",
            index + 1
        ))),
    }
}

fn optional_string(value: &Value, key: &str) -> Result<Option<String>, HostError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(HostError::new(format!("`{key}` must be a string"))),
    }
}

fn taxpayer_type(value: &Value, key: &str) -> Result<Option<TaxpayerType>, HostError> {
    optional_string(value, key)?
        .map(|raw| {
            serde_json::from_value(Value::String(raw.clone()))
                .map_err(|_| HostError::new(format!("`{raw}` is not a taxpayer type")))
        })
        .transpose()
}

/// `{field: "value" | null}`: a string overrides, null inherits.
fn field_settings(value: &Value) -> Result<BTreeMap<BranchField, FieldSetting>, HostError> {
    let Some(fields) = value.get("fields").filter(|fields| !fields.is_null()) else {
        return Ok(BTreeMap::new());
    };
    let fields = fields
        .as_object()
        .ok_or_else(|| HostError::new("`fields` must be an object"))?;
    fields
        .iter()
        .map(|(name, setting)| {
            let setting = match setting {
                Value::Null => FieldSetting::Inherit,
                Value::String(text) => FieldSetting::Override(text.clone()),
                _ => {
                    return Err(HostError::new(format!(
                        "`{name}` must be a string, or null to use the head office's value"
                    )));
                }
            };
            Ok((parse_field(name)?, setting))
        })
        .collect()
}

fn branch_record(branch: &ResolvedBranch) -> HostValue {
    let mut facts = HostObject::new();
    let mut inherited = Vec::new();
    for field in BranchField::ALL {
        facts = facts.field(field_name(field), branch.value(field));
        if branch
            .facts
            .get(&field)
            .is_some_and(|resolved| resolved.inherited)
        {
            inherited.push(field_name(field));
        }
    }
    HostObject::new()
        .field("tin", branch.tin.as_str())
        .field(
            "tin_display",
            format!(
                "{}-{}",
                format_tin_root(&branch.tin_root),
                branch.branch_code
            ),
        )
        .field("tin_root", branch.tin_root.as_str())
        .field("code", branch.branch_code.as_str())
        .field("name", branch.name.as_str())
        .field("is_head_office", branch.is_head_office)
        .field("facts", facts)
        .field("inherited", strings(inherited))
        .into()
}

fn taxpayer_record(entry: &TaxpayerWithBranches) -> HostValue {
    let taxpayer = &entry.taxpayer;
    HostObject::new()
        .field("tin_root", taxpayer.tin_root.as_str())
        .field("tin_display", format_tin_root(&taxpayer.tin_root))
        .field("name", taxpayer.name.as_str())
        .field(
            "taxpayer_type",
            field_string(serde_json::to_value(&taxpayer.taxpayer_type).ok()),
        )
        .field(
            "birth_date",
            taxpayer
                .birth_date
                .map_or(HostValue::Null, |date| HostValue::Str(date.to_string())),
        )
        .field(
            "needs_grouping_review",
            taxpayer
                .grouping_review
                .as_ref()
                .is_some_and(|review| review.needs_review()),
        )
        .field("archived", taxpayer.archived)
        .field(
            "branches",
            HostValue::Array(entry.branches.iter().map(branch_record).collect()),
        )
        .into()
}

fn field_string(value: Option<Value>) -> String {
    match value {
        Some(Value::String(text)) => text,
        _ => String::new(),
    }
}

/// One template on the Templates tab: how many answers it pins
/// (`answers_kept`) and how many "Stop here" switches are on (`marked`).
fn template_record(template: &FormTemplate) -> HostValue {
    HostObject::new()
        .field("form_code", template.form_code.as_str())
        .field("updated_at", template.updated_at.as_str())
        .field("answers_kept", template.pinned().len() as f64)
        .field("marked", template.stops_on() as f64)
        .field("source", template.source.to_db())
        .into()
}

fn load_taxpayer(db: &Database, tin_root: &str) -> Result<TaxpayerWithBranches, HostError> {
    db.get_taxpayer(tin_root)
        .map_err(host_error)?
        .ok_or_else(|| HostError::new(format!("No taxpayer with TIN {tin_root}")))
}

// ---------- taxpayers ----------

/// Polled with every page's reads (Home's taxpayer rows), so it does not
/// wait on the submission worker: every query under it is a SELECT.
fn list_taxpayers() -> HostResult {
    with_reader(|db| {
        let taxpayers = db.list_taxpayers().map_err(host_error)?;
        Ok(HostValue::Array(
            taxpayers.iter().map(taxpayer_record).collect(),
        ))
    })
}

fn get_taxpayer(arguments: &HostArguments) -> HostResult {
    let tin_root = arguments.string(0)?;
    with_database(|db| {
        Ok(db
            .get_taxpayer(tin_root)
            .map_err(host_error)?
            .as_ref()
            .map_or(HostValue::Null, taxpayer_record))
    })
}

fn list_send_grants(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    with_database(|db| {
        let grants = db
            .list_send_grants()
            .map_err(|error| HostError::new(error.to_string()))?;
        let digits: String = tin.chars().filter(|c| c.is_ascii_digit()).collect();
        let matched: Vec<serde_json::Value> = grants
            .into_iter()
            .filter(|grant| grant.tin == digits)
            .map(|grant| {
                serde_json::json!({
                    "tin": grant.tin,
                    "form": grant.form,
                    "granted_at": grant.granted_at,
                })
            })
            .collect();
        crate::bir_module::serialize(&matched)
    })
}

fn grant_send(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form = arguments.string(1)?.to_owned();
    with_database(|db| {
        let grant = db
            .grant_send(&tin, &form)
            .map_err(|error| HostError::new(error.to_string()))?;
        crate::bir_module::serialize(&serde_json::json!({
            "tin": grant.tin,
            "form": grant.form,
            "granted_at": grant.granted_at,
        }))
    })
}

fn revoke_send(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form = arguments.string(1)?.to_owned();
    with_database(|db| {
        db.revoke_send(&tin, &form)
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Null)
    })
}

/// The official TIN check digit at the two places a person types a TIN
/// root in: a taxpayer's creation and the correction of its TIN. The same
/// check the form pages run (`official_tin_check_code`), with the dev-mode
/// escape they use. It catches misread and transposed digits, not every
/// mistake — a permutation that keeps the digit valid passes it.
///
/// The sentence is deliberately not the form pages' "You have entered an
/// incorrect TIN" (the official program's own alert, kept there because the
/// form pages mirror the official program). Here the filer is not inside a
/// form: what they need is what to do next, so the sentence says it.
fn reject_wrong_check_digit(tin_root: &str) -> Result<(), HostError> {
    // Normalized by the store's own rule (`parse_tin_root` drops dashes and
    // spaces): a wrong digit is refused however it was spaced. What is not
    // nine digits even then is the store's own error — "A TIN root is
    // exactly 9 digits" — which says it more precisely; there is no digit
    // to check. The reserved admin root fails the digit too, but its own
    // refusal says why more precisely; leave it to the store.
    let root = match bir_core::tax_profile::parse_tin_root(tin_root) {
        Ok(root) => root,
        Err(_) => return Ok(()),
    };
    if root == "000000000"
        || bir_core::validation::relaxed_dev_mode()
        || bir_core::validation::official_tin_check_code(&root) == 0
    {
        return Ok(());
    }
    Err(HostError::new(format!(
        "TIN {} fails the official check digit. Compare each digit with the registration and type it again.",
        bir_core::tax_profile::format_tin_root(&root)
    )))
}

fn create_taxpayer(arguments: &HostArguments) -> HostResult {
    let input = object_argument(arguments, 0)?;
    let head = input.get("head_office").cloned().unwrap_or(Value::Null);
    let fact = |key: &str| -> Result<String, HostError> {
        Ok(optional_string(&head, key)?.unwrap_or_default())
    };
    let birth_date = optional_string(&input, "birth_date")?
        .filter(|raw| !raw.is_empty())
        .map(|raw| {
            raw.parse()
                .map_err(|_| HostError::new(format!("`{raw}` is not a date (YYYY-MM-DD)")))
        })
        .transpose()?;
    let tin_root = optional_string(&input, "tin_root")?.unwrap_or_default();
    reject_wrong_check_digit(&tin_root)?;
    let new = NewTaxpayer {
        tin_root,
        name: optional_string(&input, "name")?.unwrap_or_default(),
        taxpayer_type: taxpayer_type(&input, "taxpayer_type")?.unwrap_or_default(),
        birth_date,
        head_office: BranchFacts {
            rdo_code: fact("rdo_code")?,
            registered_address: fact("registered_address")?,
            zip_code: fact("zip_code")?,
            phone: fact("phone")?,
            email: fact("email")?,
        },
    };
    with_database(|db| {
        let created = db.create_taxpayer(new).map_err(host_error)?;
        Ok(taxpayer_record(&created))
    })
}

fn update_taxpayer(arguments: &HostArguments) -> HostResult {
    let tin_root = arguments.string(0)?;
    let input = object_argument(arguments, 1)?;
    let birth_date = match input.get("birth_date") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(raw)) if raw.trim().is_empty() => Some(None),
        Some(Value::String(raw)) => {
            Some(Some(raw.trim().parse().map_err(|_| {
                HostError::new(format!("`{raw}` is not a date (YYYY-MM-DD)"))
            })?))
        }
        Some(_) => return Err(HostError::new("`birth_date` must be a string or null")),
    };
    let update = TaxpayerUpdate {
        name: optional_string(&input, "name")?,
        taxpayer_type: taxpayer_type(&input, "taxpayer_type")?,
        birth_date,
    };
    with_database(|db| {
        let updated = db.update_taxpayer(tin_root, update).map_err(host_error)?;
        Ok(taxpayer_record(&load_taxpayer(db, &updated.tin_root)?))
    })
}

fn move_taxpayer_tin(arguments: &HostArguments) -> HostResult {
    let old_root = arguments.string(0)?;
    let new_root = arguments.string(1)?;
    reject_wrong_check_digit(new_root)?;
    with_database(|db| {
        let moved = db
            .move_taxpayer_tin(old_root, new_root)
            .map_err(host_error)?;
        // The solo owner is the admin: its new root stays the admin root.
        let old = bir_core::tax_profile::parse_tin_root(old_root).map_err(host_error)?;
        if db.get_setting("access_mode").ok().flatten().as_deref() == Some("solo")
            && db.get_setting("admin_tin_root").ok().flatten().as_deref() == Some(old.as_str())
        {
            db.set_setting("admin_tin_root", &moved.taxpayer.tin_root)
                .map_err(|error| HostError::new(error.to_string()))?;
        }
        Ok(taxpayer_record(&moved))
    })
}

// ---------- branches ----------

fn create_branch(arguments: &HostArguments) -> HostResult {
    let tin_root = arguments.string(0)?;
    let input = object_argument(arguments, 1)?;
    let new = NewBranch {
        branch_code: optional_string(&input, "branch_code")?.unwrap_or_default(),
        name: optional_string(&input, "name")?.unwrap_or_default(),
        fields: field_settings(&input)?,
    };
    with_database(|db| {
        let branch = db.create_branch(tin_root, new).map_err(host_error)?;
        Ok(branch_record(&branch))
    })
}

fn update_branch(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let input = object_argument(arguments, 1)?;
    let update = BranchUpdate {
        name: optional_string(&input, "name")?,
        fields: field_settings(&input)?,
    };
    with_database(|db| {
        let branch = db.update_branch(tin, update).map_err(host_error)?;
        Ok(branch_record(&branch))
    })
}

fn move_branch_code(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let new_code = arguments.string(1)?;
    with_database(|db| {
        let branch = db.move_branch_code(tin, new_code).map_err(host_error)?;
        Ok(branch_record(&branch))
    })
}

fn suggest_next_branch_code(arguments: &HostArguments) -> HostResult {
    let tin_root = arguments.string(0)?;
    with_database(|db| {
        db.suggest_next_branch_code(tin_root)
            .map(HostValue::Str)
            .map_err(host_error)
    })
}

// ---------- tax years and yearly forms ----------

fn tax_year_bounds_fn() -> HostResult {
    let (min, max) = tax_year_bounds();
    Ok(HostObject::new()
        .field("min", f64::from(min))
        .field("max", f64::from(max))
        .into())
}

fn list_tax_years(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| {
        let years = db.list_tax_years(tin).map_err(host_error)?;
        Ok(HostValue::Array(
            years
                .into_iter()
                .map(|year| HostValue::Number(f64::from(year)))
                .collect(),
        ))
    })
}

fn add_tax_year(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let year = year_argument(arguments, 1)?;
    let copy_forms = arguments.boolean(2)?;
    with_database(|db| {
        let added = db.add_tax_year(tin, year, copy_forms).map_err(host_error)?;
        Ok(HostObject::new()
            .field("year", f64::from(added.year))
            .field(
                "copied_from",
                added
                    .copied_from
                    .map_or(HostValue::Null, |year| HostValue::Number(f64::from(year))),
            )
            .field("forms", strings(added.forms))
            .into())
    })
}

fn yearly_forms(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let year = year_argument(arguments, 1)?;
    with_database(|db| {
        db.active_form_codes_for_year(tin, year)
            .map(strings)
            .map_err(|error| HostError::new(error.to_string()))
    })
}

fn set_yearly_forms(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let year = year_argument(arguments, 1)?;
    let codes = arguments
        .value(2)?
        .as_array()
        .ok_or_else(|| HostError::new("argument 3 must be a list of form codes"))?
        .iter()
        .map(|code| {
            code.as_str()
                .map(str::to_owned)
                .ok_or_else(|| HostError::new("every form code must be a string"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    with_database(|db| {
        let canonical = |codes: Vec<String>| -> BTreeSet<String> {
            codes.iter().map(|code| canonical_form_code(code)).collect()
        };
        let before = canonical(
            db.active_form_codes_for_year(tin, year)
                .map_err(|error| HostError::new(error.to_string()))?,
        );
        let set = db.set_yearly_forms(tin, year, &codes).map_err(host_error)?;
        let after = canonical(set.active_form_codes());
        Ok(HostObject::new()
            .field("added", strings(after.difference(&before).cloned()))
            .field("removed", strings(before.difference(&after).cloned()))
            .field("forms", strings(after))
            .into())
    })
}

// ---------- setups ----------

fn optional_flag(arguments: &HostArguments, index: usize) -> bool {
    arguments
        .get(index)
        .and_then(HostValue::as_bool)
        .unwrap_or(false)
}

fn list_setups(arguments: &HostArguments) -> HostResult {
    let raw = arguments.string(0)?;
    let taxpayer_type: TaxpayerType = serde_json::from_value(Value::String(raw.to_string()))
        .map_err(|_| HostError::new(format!("`{raw}` is not a taxpayer type")))?;
    with_reader(|db| {
        let setups = db.list_setups(&taxpayer_type).map_err(host_error)?;
        crate::bir_module::serialize(&setups)
    })
}

fn apply_setup(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let year = year_argument(arguments, 1)?;
    let id = arguments.string(2)?;
    let replace = optional_flag(arguments, 3);
    with_database(|db| {
        let applied = db.apply_setup(tin, year, id, replace).map_err(host_error)?;
        crate::bir_module::serialize(&applied)
    })
}

fn save_setup(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let year = year_argument(arguments, 1)?;
    let name = arguments.string(2)?;
    with_database(|db| {
        let setup = db.save_setup(tin, year, name).map_err(host_error)?;
        crate::bir_module::serialize(&setup)
    })
}

fn delete_setup(arguments: &HostArguments) -> HostResult {
    let id = arguments.string(0)?;
    with_database(|db| db.delete_setup(id).map(HostValue::Bool).map_err(host_error))
}

fn copy_setup_from(arguments: &HostArguments) -> HostResult {
    let from_tin = arguments.string(0)?;
    let to_tin = arguments.string(1)?;
    let year = year_argument(arguments, 2)?;
    let replace = optional_flag(arguments, 3);
    with_database(|db| {
        let applied = db
            .copy_setup_from(from_tin, to_tin, year, replace)
            .map_err(host_error)?;
        crate::bir_module::serialize(&applied)
    })
}

// ---------- templates ----------

fn list_form_templates(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_reader(|db| {
        let templates = db.list_form_templates(tin).map_err(host_error)?;
        Ok(HostValue::Array(
            templates.iter().map(template_record).collect(),
        ))
    })
}

fn create_template_from_latest(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let form_code = arguments.string(1)?;
    with_database(|db| {
        let template = db
            .snapshot_template_from_latest_return(tin, form_code)
            .map_err(host_error)?;
        Ok(template_record(&template))
    })
}

fn create_template_from_return(arguments: &HostArguments) -> HostResult {
    let draft_id = arguments.integer(0)?;
    with_database(|db| {
        let template = db
            .snapshot_template_from_return(draft_id)
            .map_err(host_error)?;
        Ok(template_record(&template))
    })
}

fn delete_form_template(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let form_code = arguments.string(1)?;
    with_database(|db| {
        db.delete_form_template(tin, form_code)
            .map(HostValue::Bool)
            .map_err(host_error)
    })
}

/// A box key and, for a radio item's group box, each of its radios: the
/// older page draws a radio by its own key.
fn with_radios(kind: AgentFormKind, key: &str) -> Vec<String> {
    let mut keys = vec![key.to_string()];
    if let Some(entry) = kind.definition().box_def(key) {
        keys.extend(entry.members.iter().cloned());
    }
    keys
}

/// The older page's stops record: the switches on, and the boxes that may
/// be stops, radios by their own keys too.
fn stops_record(kind: AgentFormKind, stop_edits: &BTreeMap<String, bool>) -> HostObject {
    let stops = stop_edits
        .iter()
        .filter(|(_, on)| **on)
        .flat_map(|(key, _)| with_radios(kind, key));
    let markable = carry::stoppable(kind)
        .into_iter()
        .flat_map(|key| with_radios(kind, key));
    HostObject::new()
        .field("stops", strings(stops))
        .field("markable", strings(markable))
}

fn page_kind(form_code: &str) -> Result<AgentFormKind, HostError> {
    AgentFormKind::from_code(&canonical_form_code(form_code))
        .ok_or_else(|| HostError::new(format!("No on-screen form {form_code}")))
}

fn template_stops(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let form_code = arguments.string(1)?;
    let kind = page_kind(form_code)?;
    with_reader(|db| {
        let template = db.get_form_template(tin, form_code).map_err(host_error)?;
        Ok(template.map_or(HostValue::Null, |template| {
            stops_record(kind, &template.stop_edits()).into()
        }))
    })
}

fn set_template_stop(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let form_code = arguments.string(1)?;
    let key = arguments.string(2)?;
    let on = match arguments.get(3).map(to_json) {
        None | Some(Value::Null) => None,
        Some(Value::Bool(on)) => Some(on),
        Some(other) => {
            return Err(HostError::new(format!(
                "`on` must be true, false or null, not {other}"
            )));
        }
    };
    let kind = page_kind(form_code)?;
    with_database(|db| {
        let stop_edits = db
            .set_template_stop(tin, form_code, key, on)
            .map_err(host_error)?;
        // By the keys the page draws: a radio item's switch for each radio.
        let edits = stop_edits
            .iter()
            .flat_map(|(key, on)| {
                carry::page_keys(kind, key)
                    .into_iter()
                    .map(move |page_key| (page_key, *on))
            })
            .fold(HostObject::new(), |object, (key, on)| {
                object.field(key.as_str(), on)
            });
        Ok(stops_record(kind, &stop_edits)
            .field("stop_edits", HostValue::from(edits))
            .into())
    })
}

// ---------- income-tax election ----------

/// The 2551Q page's Item 13 question: the election for the return's own
/// taxable year, written to the tax profile (bir-core checks the year is
/// one the confirmed profile may elect in). The page then re-syncs its open
/// draft with `screen_form_use_profile`.
fn record_income_tax_election(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let year = year_argument(arguments, 1)?;
    let election = match arguments.string(2)? {
        "graduated" => IncomeTaxElection::GraduatedUnspecified,
        "eight_percent" => IncomeTaxElection::EightPercent,
        other => {
            return Err(HostError::new(format!(
                "`{other}` is not an income-tax election; use graduated or eight_percent"
            )));
        }
    };
    with_database(|db| {
        db.record_income_tax_election(tin, year, election)
            .map_err(plain_db_error)?;
        Ok(HostValue::Bool(true))
    })
}

/// A refusal in bir-core's own words: `DbError::Other` prints as
/// "Other error: …", which is no part of what the filer needs to read.
fn plain_db_error(error: bir_core::db::DbError) -> HostError {
    match error {
        bir_core::db::DbError::Other(text) => HostError::new(text),
        error => HostError::new(error.to_string()),
    }
}

/// Whether `record_income_tax_election` would accept the year, and if not
/// what the tax profile lacks (bir-core `income_tax_election_readiness`).
/// A read: the form page asks it before it offers the election.
fn income_tax_election_readiness(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let year = year_argument(arguments, 1)?;
    with_reader(|db| {
        let readiness = db
            .income_tax_election_readiness(tin, year)
            .map_err(plain_db_error)?;
        crate::bir_module::serialize(&readiness)
    })
}

/// An individual's income type from a date, as a new confirmed profile
/// version saved with its reviewed confirmation plan (bir-core
/// `confirm_income_registration`). The taxpayer page's Registration section.
fn confirm_income_registration(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let classification = match arguments.string(1)? {
        "PurelyCompensation" => TaxClassification::PurelyCompensation,
        "SelfEmployed" => TaxClassification::SelfEmployed,
        "MixedIncome" => TaxClassification::MixedIncome,
        other => {
            return Err(HostError::new(format!(
                "`{other}` is not an individual's income type; use PurelyCompensation, SelfEmployed or MixedIncome"
            )));
        }
    };
    let raw = arguments.string(2)?;
    let effective_from = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|_| HostError::new(format!("`{raw}` is not a date (YYYY-MM-DD)")))?;
    with_database(|db| {
        db.confirm_income_registration(tin, classification, effective_from)
            .map_err(plain_db_error)?;
        Ok(HostValue::Bool(true))
    })
}

// ---------- year facts ----------

fn year_facts(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let year = year_argument(arguments, 1)?;
    with_reader(|db| {
        let view = db.year_facts_view(tin, year).map_err(host_error)?;
        crate::bir_module::serialize(&view)
    })
}

/// Writes the year facts directly: never through the profile save.
fn set_year_facts(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let year = year_argument(arguments, 1)?;
    let patch = bir_core::tax_profile::YearFactsPatch::from_json(&object_argument(arguments, 2)?)
        .map_err(host_error)?;
    with_database(|db| {
        db.set_year_facts(tin, year, &patch).map_err(host_error)?;
        let view = db.year_facts_view(tin, year).map_err(host_error)?;
        crate::bir_module::serialize(&view)
    })
}

fn tax_types_for_year(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let year = year_argument(arguments, 1)?;
    with_reader(|db| {
        let chips = db.tax_types_for_year(tin, year).map_err(host_error)?;
        crate::bir_module::serialize(&chips)
    })
}

/// The annual income tax returns among the given codes that stand for one
/// another. Nothing is removed from a forms list (decision 1): the Forms
/// tab shows a warning and the user decides.
fn annual_return_conflicts(arguments: &HostArguments) -> HostResult {
    let codes = arguments
        .value(0)?
        .as_array()
        .ok_or_else(|| HostError::new("argument 1 must be a list of form codes"))?
        .iter()
        .map(|code| {
            code.as_str()
                .map(canonical_form_code)
                .ok_or_else(|| HostError::new("every form code must be a string"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    crate::bir_module::serialize(
        &bir_core::integration::validation::conflicting_annual_itr_code_groups(&codes),
    )
}

// ---------- confirmation inbox ----------

fn lock_db(
    db: &std::sync::Arc<std::sync::Mutex<Database>>,
) -> Result<std::sync::MutexGuard<'_, Database>, HostError> {
    db.lock()
        .map_err(|_| HostError::new("database lock poisoned"))
}

fn load_inbox_profile(db: &Database, tin: &str) -> Result<TaxpayerProfile, HostError> {
    db.get_profile(tin)
        .map_err(|error| HostError::new(error.to_string()))?
        .ok_or_else(|| HostError::new(format!("No taxpayer profile with TIN {tin}")))
}

/// Everything the UI needs to draw the inbox card — no secrets cross here:
/// passwords and tokens are reported as booleans only.
fn inbox_state_value(db: &Database, tin: &str) -> Result<HostValue, HostError> {
    let profile = load_inbox_profile(db, tin)?;
    let inbox = profile.inbox_email().to_string();
    let tokens = if inbox.is_empty() {
        None
    } else {
        db.inbox_oauth_tokens(&inbox)
            .map_err(|error| HostError::new(error.to_string()))?
    };
    let oauth_connected = profile.has_usable_oauth_refresh()
        || tokens.as_ref().is_some_and(|t| t.has_usable_refresh());
    let (still_pending, matching_tins) = if inbox.is_empty() {
        (false, Vec::new())
    } else {
        let plan = bir_core::email::plan_email_poll_for_address(
            db,
            &inbox,
            chrono::Local::now().year() as u16,
        );
        (plan.still_pending, plan.matching_tins)
    };
    // "Last check" lives on the cron-side `bir_poll_email <address>` job row.
    let poll_command = format!("bir_poll_email {inbox}");
    let poll_job = db
        .list_jobs()
        .unwrap_or_default()
        .into_iter()
        .filter(|job| job.command.as_deref() == Some(poll_command.as_str()))
        .max_by_key(|job| job.last_run_at.clone().unwrap_or_default());
    Ok(HostObject::new()
        .field("tin", tin)
        .field("inbox_email", inbox.clone())
        .field(
            "auth_method",
            match profile.email_auth_method {
                EmailAuthMethod::AppPassword => "AppPassword",
                EmailAuthMethod::GoogleOAuth => "GoogleOAuth",
            },
        )
        .field("tracking_enabled", profile.email_tracking_enabled)
        .field("imap_login", profile.imap_email.clone().unwrap_or_default())
        .field(
            "imap_host",
            profile
                .imap_host
                .clone()
                .unwrap_or_else(|| "imap.gmail.com".to_string()),
        )
        .field(
            "has_app_password",
            profile
                .imap_app_password
                .as_deref()
                .is_some_and(|pw| !pw.trim().is_empty()),
        )
        .field("oauth_connected", oauth_connected)
        .field(
            "google_available",
            bir_core::email::google_sign_in_available(),
        )
        .field(
            "oauth_inbox_email",
            tokens
                .as_ref()
                .map(|t| t.imap_user.clone())
                .or(if oauth_connected {
                    Some(inbox.clone())
                } else {
                    None
                })
                .map_or(HostValue::Null, HostValue::Str),
        )
        .field(
            "oauth_updated_at",
            tokens
                .map(|t| t.updated_at.clone())
                .map_or(HostValue::Null, HostValue::Str),
        )
        .field("still_pending", still_pending)
        .field("matching_tins", strings(matching_tins))
        .field(
            "last_check_at",
            poll_job
                .as_ref()
                .and_then(|job| job.last_run_at.clone())
                .map_or(HostValue::Null, HostValue::Str),
        )
        .field(
            "last_check_status",
            poll_job
                .as_ref()
                .map(|job| job.status.clone())
                .map_or(HostValue::Null, HostValue::Str),
        )
        .field(
            "last_check_log",
            poll_job
                .and_then(|job| job.output_log)
                .map_or(HostValue::Null, HostValue::Str),
        )
        .into())
}

fn inbox_state(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| inbox_state_value(db, tin))
}

fn set_inbox_app_password(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let input = object_argument(arguments, 1)?;
    let imap_email = optional_string(&input, "imap_email")?
        .map(|email| email.trim().to_string())
        .filter(|email| !email.is_empty())
        .ok_or_else(|| HostError::new("Type the inbox email, like name@example.com."))?;
    let imap_host = optional_string(&input, "imap_host")?
        .map(|host| host.trim().to_string())
        .filter(|host| !host.is_empty());
    // App passwords arrive as "abcd efgh ijkl mnop"; Google strips the spaces.
    let app_password = optional_string(&input, "app_password")?
        .map(|pw| pw.replace(' ', ""))
        .map(|pw| {
            if pw.is_empty() {
                Err(HostError::new(
                    "The app password is empty. Paste the 16-letter password from your provider.",
                ))
            } else {
                Ok(pw)
            }
        })
        .transpose()?;
    with_database(|db| {
        let mut profile = load_inbox_profile(db, tin)?;
        profile.email_auth_method = EmailAuthMethod::AppPassword;
        profile.imap_email = Some(imap_email);
        profile.imap_host = imap_host;
        if let Some(password) = app_password {
            profile.imap_app_password = Some(password);
        }
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))?;
        inbox_state_value(db, tin)
    })
}

fn set_inbox_tracking(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let enabled = arguments.boolean(1)?;
    with_database(|db| {
        let mut profile = load_inbox_profile(db, tin)?;
        profile.email_tracking_enabled = enabled;
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))?;
        inbox_state_value(db, tin)
    })
}

fn disconnect_inbox_oauth(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| {
        let profile = load_inbox_profile(db, tin)?;
        let inbox = profile.inbox_email().to_string();
        if inbox.trim().is_empty() {
            return Err(HostError::new(
                "No inbox email is set, so there is nothing to disconnect.",
            ));
        }
        db.disconnect_google_oauth_for_inbox(&inbox)
            .map_err(|error| HostError::new(error.to_string()))?;
        // The grant fan-out re-saved every sibling; reload before toggling.
        let mut profile = load_inbox_profile(db, tin)?;
        profile.email_tracking_enabled = false;
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))?;
        inbox_state_value(db, tin)
    })
}

/// F4: a room thread's write runs only while the open room is still the one it started in.
/// The captured Arc outlives the room it came from, so without this check B's consent would
/// write into A's room after a switch.
fn refuse_if_room_changed(generation: Option<u64>) -> Result<(), HostError> {
    if let Some(captured) = generation
        && crate::session::snapshot()
            .room
            .map(|(_, generation, _, _)| generation)
            != Some(captured)
    {
        return Err(HostError::new(
            "the room changed while this ran; its result was dropped",
        ));
    }
    Ok(())
}

/// Persist a grant returned by `start_oauth_flow` — split out so tests drive
/// it with canned tokens (the browser leg cannot run headlessly).
#[cfg(test)]
fn persist_oauth_grant(
    db: &std::sync::Arc<std::sync::Mutex<Database>>,
    tin: &str,
    email: &str,
    access_token: &str,
    refresh_token: &str,
) -> HostResult {
    persist_oauth_grant_for(db, tin, email, access_token, refresh_token, None)
}

fn persist_oauth_grant_for(
    db: &std::sync::Arc<std::sync::Mutex<Database>>,
    tin: &str,
    email: &str,
    access_token: &str,
    refresh_token: &str,
    generation: Option<u64>,
) -> HostResult {
    refuse_if_room_changed(generation)?;
    let guard = lock_db(db)?;
    refuse_if_room_changed(generation)?;
    guard
        .persist_google_oauth_for_inbox(email, access_token, refresh_token, Some(tin))
        .map_err(|error| HostError::new(error.to_string()))?;
    inbox_state_value(&guard, tin)
}

fn connect_inbox_oauth_run(
    tin: &str,
    db: &std::sync::Arc<std::sync::Mutex<Database>>,
    generation: Option<u64>,
) -> HostResult {
    // Blocking: browser consent + a loopback listener. Same error surface the
    // legacy Profile Manager shows ("OAuth failed: ...").
    let (email, access_token, refresh_token) =
        run_oauth_flow()?.map_err(|error| HostError::new(format!("OAuth failed: {error}")))?;
    if refresh_token.trim().is_empty() {
        return Err(HostError::new(
            "Google did not return a refresh token. Re-authorize with consent so a new refresh token is issued.",
        ));
    }
    persist_oauth_grant_for(db, tin, &email, &access_token, &refresh_token, generation)
}

/// The browser sign-in leg: bir-core's `start_oauth_flow`, answering
/// `(email, access_token, refresh_token)`. Tests run only a fake installed
/// on this thread through [`TEST_INBOX_OAUTH`]; without one the leg refuses,
/// so no test opens a browser.
fn run_oauth_flow() -> Result<Result<(String, String, String), anyhow::Error>, HostError> {
    #[cfg(test)]
    {
        TEST_INBOX_OAUTH
            .with(|slot| slot.borrow_mut().take())
            .map(|fake| fake())
            .ok_or_else(|| HostError::new("The Google sign-in is not available in tests."))
    }
    #[cfg(not(test))]
    {
        Ok(bir_core::email::start_oauth_flow())
    }
}

/// Apply a `test_connection` verdict the way the legacy Email Settings tab
/// does: success enables tracking (and fans a rotated access token out to the
/// inbox), failure disables it.
fn apply_connection_result(
    db: &std::sync::Arc<std::sync::Mutex<Database>>,
    tin: &str,
    result: Result<Option<String>, anyhow::Error>,
    generation: Option<u64>,
) -> HostResult {
    refuse_if_room_changed(generation)?;
    let guard = lock_db(db)?;
    refuse_if_room_changed(generation)?;
    match result {
        Ok(new_access_token) => {
            if let Some(token) = new_access_token {
                let inbox = load_inbox_profile(&guard, tin)?.inbox_email().to_string();
                let _ = guard.update_inbox_oauth_access_token(&inbox, &token);
            }
            let mut profile = load_inbox_profile(&guard, tin)?;
            profile.email_tracking_enabled = true;
            guard
                .save_profile(profile)
                .map_err(|error| HostError::new(error.to_string()))?;
            Ok(HostObject::new()
                .field("ok", true)
                .field(
                    "message",
                    "Connection successful! Email tracking is now active.",
                )
                .field("state", inbox_state_value(&guard, tin)?)
                .into())
        }
        Err(error) => {
            tracing::warn!(%error, "IMAP login failed");
            let mut profile = load_inbox_profile(&guard, tin)?;
            profile.email_tracking_enabled = false;
            guard
                .save_profile(profile)
                .map_err(|error| HostError::new(error.to_string()))?;
            Ok(HostObject::new()
                .field("ok", false)
                .field("message", format!("Connection failed: {error}"))
                .field("state", inbox_state_value(&guard, tin)?)
                .into())
        }
    }
}

fn test_inbox_connection_run(
    tin: &str,
    db: &std::sync::Arc<std::sync::Mutex<Database>>,
    generation: Option<u64>,
) -> HostResult {
    let profile = {
        let guard = lock_db(db)?;
        load_inbox_profile(&guard, tin)?
    };
    // The IMAP probe runs with the lock released — it can take many seconds.
    let result = run_connection_test(&profile)?;
    apply_connection_result(db, tin, result, generation)
}

/// The IMAP login probe leg: bir-core's `test_connection`. Tests run only a
/// fake installed on this thread through [`TEST_INBOX_CONNECTION`]; without
/// one the leg refuses, so no test reaches a mail server.
fn run_connection_test(
    profile: &TaxpayerProfile,
) -> Result<Result<Option<String>, anyhow::Error>, HostError> {
    #[cfg(test)]
    {
        TEST_INBOX_CONNECTION
            .with(|slot| slot.borrow_mut().take())
            .map(|fake| fake(profile))
            .ok_or_else(|| HostError::new("The inbox connection test is not available in tests."))
    }
    #[cfg(not(test))]
    {
        Ok(bir_core::email::test_connection(profile))
    }
}

/// The poll leg: bir-core's fetch→confirm path. Tests run only a fake
/// transport installed on this thread through [`TEST_INBOX_POLL`]; without
/// one the leg refuses, so no test fetches mail.
fn run_inbox_poll(
    email: &str,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
) -> Result<(bool, bool, Option<String>), HostError> {
    #[cfg(test)]
    {
        TEST_INBOX_POLL
            .with(|slot| slot.borrow_mut().take())
            .map(|fake| fake(email, db))
            .ok_or_else(|| HostError::new("The inbox poll is not available in tests."))
    }
    #[cfg(not(test))]
    {
        Ok(bir_core::email::fetch_and_process_emails_for_address(
            email, db,
        ))
    }
}

/// One user-triggered "check now" — the same inline run the legacy Cron Tasks
/// "Run now" performs, including stamping the job row with the outcome.
fn check_inbox_now_run(tin: &str, db: &std::sync::Arc<std::sync::Mutex<Database>>) -> HostResult {
    let email = {
        let guard = lock_db(db)?;
        let email = load_inbox_profile(&guard, tin)?.inbox_email().to_string();
        if email.trim().is_empty() {
            return Err(HostError::new(
                "No inbox email is set on this profile. Save App Password credentials or connect Google first.",
            ));
        }
        email
    };
    let (poll_success, still_pending, err_msg) = run_inbox_poll(&email, db.clone())?;

    let guard = lock_db(db)?;
    let command = format!("bir_poll_email {email}");
    let now = chrono::Utc::now().to_rfc3339();
    for mut job in guard.list_jobs().unwrap_or_default() {
        if job.command.as_deref() != Some(command.as_str())
            || matches!(job.status.as_str(), "Archived" | "Done")
        {
            continue;
        }
        job.last_run_at = Some(now.clone());
        if poll_success {
            job.output_log = Some("Email polling completed successfully.".to_string());
            job.retries = 0;
            if !still_pending {
                job.status = "Archived".to_string();
            }
        } else {
            job.output_log = Some(
                err_msg
                    .clone()
                    .unwrap_or_else(|| "Email polling failed (unknown).".to_string()),
            );
            job.retries += 1;
            job.status = "Queued".to_string();
        }
        let _ = guard.save_job(job);
    }
    Ok(HostObject::new()
        .field("ok", poll_success)
        .field("still_pending", still_pending)
        .field("error", err_msg.map_or(HostValue::Null, HostValue::Str))
        .field("state", inbox_state_value(&guard, tin)?)
        .into())
}

/// Fake poll transport installed by tests — returns the same
/// `(poll_success, still_pending, error)` triple as the real fetch.
#[cfg(test)]
type TestInboxPoll = Box<
    dyn FnOnce(&str, std::sync::Arc<std::sync::Mutex<Database>>) -> (bool, bool, Option<String>),
>;

/// Fake IMAP login probe installed by tests — answers like `test_connection`.
#[cfg(test)]
type TestInboxConnection =
    Box<dyn FnOnce(&TaxpayerProfile) -> Result<Option<String>, anyhow::Error>>;

/// Fake browser sign-in installed by tests — answers like `start_oauth_flow`.
#[cfg(test)]
type TestInboxOAuth = Box<dyn FnOnce() -> Result<(String, String, String), anyhow::Error>>;

#[cfg(test)]
thread_local! {
    static TEST_INBOX_POLL: std::cell::RefCell<Option<TestInboxPoll>> =
        const { std::cell::RefCell::new(None) };
    static TEST_INBOX_CONNECTION: std::cell::RefCell<Option<TestInboxConnection>> =
        const { std::cell::RefCell::new(None) };
    static TEST_INBOX_OAUTH: std::cell::RefCell<Option<TestInboxOAuth>> =
        const { std::cell::RefCell::new(None) };
}

// ---------- the account's TINs (Track D) ----------

/// The client the account's TINs are read and added through: the relay's,
/// one per process ([`bir_core::relay::submitter`]); in tests, one a test
/// installed on its thread ([`TEST_ACCOUNT_SUBMITTER`]), else none, so no
/// test reaches a server it did not start.
fn account_submitter() -> Result<std::sync::Arc<Submitter>, HostError> {
    #[cfg(test)]
    {
        TEST_ACCOUNT_SUBMITTER
            .with(|slot| slot.borrow().clone())
            .ok_or_else(|| HostError::new("The Buwiz account is not available in tests."))
    }
    #[cfg(not(test))]
    {
        bir_core::relay::submitter().map_err(|error| HostError::new(error.to_string()))
    }
}

#[cfg(test)]
thread_local! {
    static TEST_ACCOUNT_SUBMITTER: std::cell::RefCell<Option<std::sync::Arc<Submitter>>> =
        const { std::cell::RefCell::new(None) };
}

/// Run the client's `work` on a thread and a runtime of its own, as the
/// account verbs do, and answer it to the page.
fn on_account_thread<T: Send + 'static>(
    name: &str,
    work: impl std::future::Future<Output = T> + Send + 'static,
    answer: impl FnOnce(T) -> HostResult + Send + 'static,
) -> Result<std::pin::Pin<Box<dyn std::future::Future<Output = HostResult> + Send>>, HostError> {
    // AL2: a non-send account verb is still work for one owner and epoch. Capture both from the
    // session snapshot at the verb's start and scope the whole client call with them, so the real
    // AccountTokens never falls back to whoever is stored when the network answer comes back.
    let snap = crate::session::snapshot();
    let context = snap.owner.map(|owner| {
        bir_core::account_session::SendContext::new(
            bir_core::account_session::OwnerKey {
                origin: owner.origin,
                user_id: owner.user_id,
            },
            snap.epoch,
        )
    });
    crate::cloud_account::spawn_account(name, move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| HostError::new(error.to_string()))?;
        if let Some(context) = context {
            answer(runtime.block_on(bir_core::account_session::SEND_CONTEXT.scope(context, work)))
        } else {
            answer(runtime.block_on(work))
        }
    })
}

/// Every branch of every tax profile on this device: `(tin_root, branch_code)`.
fn local_branches(db: &Database) -> Result<Vec<(String, String)>, HostError> {
    Ok(db
        .list_taxpayers()
        .map_err(host_error)?
        .into_iter()
        .flat_map(|taxpayer| taxpayer.branches)
        .map(|branch| (branch.tin_root, branch.branch_code))
        .collect())
}

fn tin_state(next: Next) -> &'static str {
    match next {
        Next::Send => "activated",
        Next::Wait => "pending",
        Next::Fix => "fix",
    }
}

/// A row reduced to what the screens show. `action_url` is the row's, which
/// the client kept only when it is a page of the account site
/// (`pinned_account_url`), and only for a row to fix.
fn tin_status_row(row: &AccountTin, stale: bool, fetched_at: i64) -> Value {
    serde_json::json!({
        "tin_root": row.key.root(),
        "branch": row.key.branch(),
        "state": tin_state(row.next),
        "message": row.message,
        "action_url": row.action_url.as_deref().filter(|_| row.next == Next::Fix),
        "test_only": row.test_only,
        "stale": stale,
        "fetched_at": fetched_at,
    })
}

/// The account's rows, one per TIN and branch, then, when the list is
/// complete, a `not_activated` row for each of `local` the list lacks (a
/// TIN missing from an incomplete list may be on the account).
fn tin_status_list(list: &AccountTins, local: &[(String, String)]) -> Value {
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for row in &list.rows {
        if seen.insert(row.key.clone())
            && let Some(row) = list.find(&row.key)
        {
            rows.push(tin_status_row(row, list.stale, list.fetched_at));
        }
    }
    if list.complete {
        for (root, branch) in local {
            let Some(key) = TinKey::parse(&format!("{root}{branch}")) else {
                continue;
            };
            if seen.insert(key.clone()) {
                rows.push(serde_json::json!({
                    "tin_root": key.root(),
                    "branch": key.branch(),
                    "state": "not_activated",
                    "message": null,
                    "action_url": null,
                    "test_only": false,
                    "stale": list.stale,
                    "fetched_at": list.fetched_at,
                }));
            }
        }
    }
    serde_json::json!({
        "rows": rows,
        "complete": list.complete,
        "stale": list.stale,
        "fetched_at": list.fetched_at,
    })
}

/// `account_tins(max_age)`. Needs the filing data open, as every verb that
/// reads tax profiles does ([`with_reader`]: signed in to Buwiz); the room
/// lock adds its owner check here.
fn account_tins(
    arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let max_age = arguments.integer(0)?;
    let local = with_reader(local_branches)?;
    let submitter = account_submitter()?;
    on_account_thread(
        "buwiz-account-tins",
        async move { submitter.account_tins(max_age).await },
        move |read| {
            let list = read.map_err(|error| HostError::new(error.to_string()))?;
            Ok(crate::bir_module::json(tin_status_list(&list, &local)))
        },
    )
}

/// `activate_tin(tin_full)`: the explicit "Activate this TIN to send". The
/// kind is the tax profile's type (an individual is personal, any other
/// type a business) and the registered name its name; the room keeps no
/// trade name, so none is sent. Needs the filing data open as
/// [`account_tins`] does; the room lock adds its owner check here.
fn activate_tin(
    arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let tin = arguments.string(0)?.to_owned();
    let key = TinKey::parse(&tin)
        .filter(|_| tin.chars().filter(char::is_ascii_digit).count() == 14)
        .ok_or_else(|| HostError::new("Give the tax profile's TIN with its branch: 14 digits."))?;
    let (kind, registered_name) = with_reader(|db| {
        let taxpayer = db
            .get_taxpayer(key.root())
            .map_err(host_error)?
            .filter(|taxpayer| {
                taxpayer
                    .branches
                    .iter()
                    .any(|branch| branch.branch_code == key.branch())
            })
            .ok_or_else(|| HostError::new("This tax profile is not on this device."))?
            .taxpayer;
        let kind = if taxpayer.taxpayer_type == TaxpayerType::Individual {
            TinKind::Individual
        } else {
            TinKind::NonIndividual
        };
        Ok((kind, taxpayer.name))
    })?;
    let submitter = account_submitter()?;
    on_account_thread(
        "buwiz-account-activate-tin",
        async move {
            submitter
                .add_account_tin(kind, &key, &registered_name, None)
                .await
        },
        |added| match added {
            Ok(Some(row)) => Ok(crate::bir_module::json(tin_status_row(
                &row,
                false,
                bir_core::bir_submit::device::now(),
            ))),
            Ok(None) => Ok(HostValue::Null),
            Err(error) => Err(HostError::new(error.to_string())),
        },
    )
}

/// §A.4: a thread started for a room — the inbox OAuth, verify and check threads — returns its result
/// only while the open room's generation is still the one it captured at spawn; after the room changed
/// the result is dropped and logged, never written into the new room. `None` at spawn is single-room
/// mode (no generations), and the guard does not apply. The account and email threads (`spawn_account`)
/// carry no room.
async fn finish_room_thread(
    generation: Option<u64>,
    receiver: futures::channel::oneshot::Receiver<HostResult>,
    failed: &'static str,
) -> HostResult {
    let result = receiver
        .await
        .unwrap_or_else(|_| Err(HostError::new(failed)));
    if let Some(captured) = generation
        && crate::session::snapshot()
            .room
            .map(|(_, generation, _, _)| generation)
            != Some(captured)
    {
        tracing::warn!(
            captured,
            current = ?crate::session::snapshot()
                .room
                .map(|(_, generation, _, _)| generation),
            "room thread result dropped: the room changed while it ran"
        );
        return Err(HostError::new(
            "the room changed while this ran; its result was dropped",
        ));
    }
    result
}

fn connect_inbox_oauth(
    arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let tin = arguments.string(0)?.to_owned();
    // X4/F4: the handle and the generation are one read of the slot; a switch between two reads
    // would write this thread's result into the wrong room as "not stale".
    let snap = crate::session::snapshot();
    let slot_generation = snap.room.as_ref().map(|(_, generation, _, _)| *generation);
    let (db, generation) = match crate::bir_module::thread_slot_handle()? {
        Some(handle) => (handle, None),
        None => match snap.room {
            Some((handle, generation, _, _)) => (handle, Some(generation)),
            None => (database_handle()?, slot_generation),
        },
    };
    let (sender, receiver) = futures::channel::oneshot::channel::<HostResult>();
    std::thread::Builder::new()
        .name("inbox-oauth".into())
        .spawn(move || {
            let _ = sender.send(connect_inbox_oauth_run(&tin, &db, generation));
        })
        .map_err(|error| HostError::new(error.to_string()))?;
    Ok(async move { finish_room_thread(generation, receiver, "OAuth thread failed").await })
}

fn test_inbox_connection(
    arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let tin = arguments.string(0)?.to_owned();
    // X4/F4: the handle and the generation are one read of the slot; a switch between two reads
    // would write this thread's result into the wrong room as "not stale".
    let snap = crate::session::snapshot();
    let slot_generation = snap.room.as_ref().map(|(_, generation, _, _)| *generation);
    let (db, generation) = match crate::bir_module::thread_slot_handle()? {
        Some(handle) => (handle, None),
        None => match snap.room {
            Some((handle, generation, _, _)) => (handle, Some(generation)),
            None => (database_handle()?, slot_generation),
        },
    };
    let (sender, receiver) = futures::channel::oneshot::channel::<HostResult>();
    std::thread::Builder::new()
        .name("inbox-verify".into())
        .spawn(move || {
            let _ = sender.send(test_inbox_connection_run(&tin, &db, generation));
        })
        .map_err(|error| HostError::new(error.to_string()))?;
    Ok(async move { finish_room_thread(generation, receiver, "verify thread failed").await })
}

fn check_inbox_now(
    arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let tin = arguments.string(0)?.to_owned();
    // X4/F4: the handle and the generation are one read of the slot; a switch between two reads
    // would write this thread's result into the wrong room as "not stale".
    let snap = crate::session::snapshot();
    let slot_generation = snap.room.as_ref().map(|(_, generation, _, _)| *generation);
    let (db, generation) = match crate::bir_module::thread_slot_handle()? {
        Some(handle) => (handle, None),
        None => match snap.room {
            Some((handle, generation, _, _)) => (handle, Some(generation)),
            None => (database_handle()?, slot_generation),
        },
    };
    let (sender, receiver) = futures::channel::oneshot::channel::<HostResult>();
    std::thread::Builder::new()
        .name("inbox-check".into())
        .spawn(move || {
            let _ = sender.send(check_inbox_now_run(&tin, &db));
        })
        .map_err(|error| HostError::new(error.to_string()))?;
    Ok(async move { finish_room_thread(generation, receiver, "inbox check thread failed").await })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bir_module::use_ephemeral_database;
    use serde_json::json;

    const ROOT: &str = "123456788";
    const HEAD: &str = "12345678800000";

    fn args(values: Vec<Value>) -> HostArguments {
        HostArguments::new(values.into_iter().map(crate::bir_module::json))
    }

    fn call(
        function: fn(&HostArguments) -> HostResult,
        values: Vec<Value>,
    ) -> Result<Value, String> {
        function(&args(values))
            .map(|value| to_json(&value))
            .map_err(|error| error.message().to_string())
    }

    fn ok(function: fn(&HostArguments) -> HostResult, values: Vec<Value>) -> Value {
        call(function, values).unwrap()
    }

    /// Fresh database with "Example Trading Corp" and its head office.
    fn setup() -> Value {
        use_ephemeral_database();
        ok(
            create_taxpayer,
            vec![json!({
                "tin_root": "123-456-788",
                "name": "Example Trading Corp",
                "taxpayer_type": "Corporation",
                "head_office": {
                    "rdo_code": "040",
                    "registered_address": "1 Example Street, Pasig",
                    "zip_code": "1600",
                    "phone": "0280000000",
                    "email": "office@example.com"
                }
            })],
        )
    }

    fn add_branch(code: &str) -> Value {
        ok(
            create_branch,
            vec![
                json!(ROOT),
                json!({"branch_code": code, "name": "Example Store", "fields": {"zip_code": "1700"}}),
            ],
        )
    }

    /// A saved Draft 2551Q return for the head office in `quarter`.
    fn save_return(quarter: u8) -> i64 {
        with_database(|db| {
            let profile = db.get_profile(HEAD).unwrap().unwrap();
            let mut draft =
                bir_core::forms::Form2551QDraft::new_from_profile(&profile, 2026, quarter);
            draft.item_13_election = bir_core::forms::form_2551q::Item13Election::Graduated;
            Ok(db.save_2551q_draft(&draft).unwrap())
        })
        .unwrap()
    }

    fn create_with(tin_root: &str) -> Result<Value, String> {
        use_ephemeral_database();
        call(
            create_taxpayer,
            vec![
                json!({"tin_root": tin_root, "name": "Juana Santos", "taxpayer_type": "Individual", "head_office": {}}),
            ],
        )
    }

    #[test]
    fn a_tin_that_passes_the_official_check_digit_is_created() {
        assert!(
            create_with("000-123-453").is_ok(),
            "{:?}",
            create_with("000-123-453")
        );
    }

    #[test]
    fn a_tin_one_digit_wrong_is_refused_with_what_to_do() {
        let err = create_with("000-123-454").unwrap_err();
        assert!(
            err.contains("fails the official check digit") && err.contains("type it again"),
            "{err}"
        );
    }

    #[test]
    fn a_tin_two_digits_swapped_is_refused() {
        // 000-123-453 with two neighboring digits transposed.
        assert!(create_with("000-123-543").is_err());
    }

    #[test]
    fn a_moved_digit_tin_the_burst_produced_passes_the_check_digit() {
        // 001-234-530 is what fast typing stored for 000-123-453 (one digit
        // moved to the end): a permutation, and a permutation can keep the
        // check digit valid. Nobody should believe this check catches the
        // burst; it catches misread and transposed digits only.
        assert!(create_with("001-234-530").is_ok());
    }

    #[test]
    fn a_wrong_digit_spaced_like_the_store_allows_is_still_refused() {
        // The store's own normalization drops spaces as well as dashes, so
        // the check sees the same nine digits however they were spaced.
        for typed in ["000 123 454", " 000123454 ", "000-123 454"] {
            let err = create_with(typed).unwrap_err();
            assert!(
                err.contains("fails the official check digit"),
                "{typed}: {err}"
            );
        }
    }

    #[test]
    fn what_is_not_nine_digits_is_the_stores_own_refusal() {
        // A letter or a wrong length never reaches the digit check: the
        // store's parse already says what is wrong, more precisely.
        for typed in ["00012345X", "00012345", "0001234530"] {
            let err = create_with(typed).unwrap_err();
            assert!(err.contains("exactly 9 digits"), "{typed}: {err}");
            assert!(!err.contains("check digit"), "{typed}: {err}");
        }
    }

    #[test]
    fn a_tin_correction_checks_the_check_digit_too() {
        setup();
        let wrong = call(move_taxpayer_tin, vec![json!(ROOT), json!("000123454")]);
        assert!(
            wrong
                .unwrap_err()
                .contains("fails the official check digit")
        );
        let right = ok(move_taxpayer_tin, vec![json!(ROOT), json!("000123453")]);
        assert_eq!(right["tin_root"], "000123453");
    }

    #[test]
    fn create_taxpayer_returns_the_taxpayer_with_its_head_office() {
        let created = setup();
        assert_eq!(created["tin_root"], ROOT);
        assert_eq!(created["tin_display"], "123-456-788");
        assert_eq!(created["name"], "Example Trading Corp");
        assert_eq!(created["taxpayer_type"], "Corporation");
        let head = &created["branches"][0];
        assert_eq!(head["tin"], HEAD);
        assert_eq!(head["code"], "00000");
        assert_eq!(head["is_head_office"], true);
        assert_eq!(head["facts"]["rdo_code"], "040");
        assert_eq!(head["inherited"], json!([]));
        let again = call(
            create_taxpayer,
            vec![
                json!({"tin_root": ROOT, "name": "Other", "taxpayer_type": "Individual", "head_office": {}}),
            ],
        );
        assert!(again.unwrap_err().contains("already exists"));
    }

    #[test]
    fn list_taxpayers_lists_head_office_first() {
        setup();
        add_branch("00002");
        let listed = to_json(&list_taxpayers().unwrap());
        assert_eq!(listed.as_array().unwrap().len(), 1);
        let codes: Vec<&str> = listed[0]["branches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|branch| branch["code"].as_str().unwrap())
            .collect();
        assert_eq!(codes, vec!["00000", "00002"]);
    }

    #[test]
    fn get_taxpayer_returns_null_when_missing() {
        setup();
        assert_eq!(
            ok(get_taxpayer, vec![json!(ROOT)])["name"],
            "Example Trading Corp"
        );
        assert_eq!(ok(get_taxpayer, vec![json!("987654321")]), Value::Null);
    }

    #[test]
    fn update_taxpayer_changes_name_and_type() {
        setup();
        let updated = ok(
            update_taxpayer,
            vec![
                json!(ROOT),
                json!({"name": "Example Trading Co.", "taxpayer_type": "Partnership"}),
            ],
        );
        assert_eq!(updated["name"], "Example Trading Co.");
        assert_eq!(updated["taxpayer_type"], "Partnership");
        assert_eq!(updated["branches"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn move_taxpayer_tin_moves_and_refuses_a_used_tin() {
        setup();
        add_branch("00001");
        let moved = ok(move_taxpayer_tin, vec![json!(ROOT), json!("987-654-321")]);
        assert_eq!(moved["tin_root"], "987654321");
        assert_eq!(moved["branches"][1]["tin"], "98765432100001");
        assert_eq!(ok(get_taxpayer, vec![json!(ROOT)]), Value::Null);

        ok(
            create_taxpayer,
            vec![
                json!({"tin_root": ROOT, "name": "Sample Holdings Inc", "taxpayer_type": "Corporation",
                "head_office": {"rdo_code": "1", "registered_address": "a", "zip_code": "1", "phone": "1", "email": "a@example.com"}}),
            ],
        );
        let refused = call(move_taxpayer_tin, vec![json!("987654321"), json!(ROOT)]);
        assert_eq!(
            refused.unwrap_err(),
            "TIN 123-456-788 is already used by Sample Holdings Inc"
        );
    }

    #[test]
    fn the_help_desk_root_is_refused_and_a_moved_owner_stays_the_admin() {
        let created = setup();
        assert_eq!(created["archived"], false);
        let reserved = "TIN 000-000-000 is reserved for the help desk admin.";
        let refused = call(
            create_taxpayer,
            vec![
                json!({"tin_root": "000-000-000", "name": "Help desk", "taxpayer_type": "Individual",
                "head_office": {}}),
            ],
        );
        assert_eq!(refused.unwrap_err(), reserved);
        let refused = call(move_taxpayer_tin, vec![json!(ROOT), json!("000000000")]);
        assert_eq!(refused.unwrap_err(), reserved);

        with_database(|db| {
            db.set_setting("access_mode", "solo").unwrap();
            db.set_setting("admin_tin_root", ROOT).unwrap();
            Ok(())
        })
        .unwrap();
        ok(move_taxpayer_tin, vec![json!(ROOT), json!("987654321")]);
        let admin_root = with_database(|db| Ok(db.get_setting("admin_tin_root").unwrap())).unwrap();
        assert_eq!(admin_root.as_deref(), Some("987654321"));
    }

    #[test]
    fn create_branch_inherits_unlisted_fields() {
        setup();
        let branch = add_branch("00001");
        assert_eq!(branch["tin"], "12345678800001");
        assert_eq!(branch["tin_display"], "123-456-788-00001");
        assert_eq!(branch["facts"]["zip_code"], "1700");
        assert_eq!(branch["facts"]["rdo_code"], "040");
        assert_eq!(
            branch["inherited"],
            json!(["rdo_code", "registered_address", "phone", "email"])
        );
        let bad = call(
            create_branch,
            vec![
                json!(ROOT),
                json!({"branch_code": "00002", "name": "X", "fields": {"nickname": "y"}}),
            ],
        );
        assert!(bad.unwrap_err().contains("not a branch field"));
    }

    #[test]
    fn update_branch_overrides_and_inherits() {
        setup();
        add_branch("00001");
        let updated = ok(
            update_branch,
            vec![
                json!("12345678800001"),
                json!({"name": "Example Mall Store", "fields": {"rdo_code": "050", "zip_code": null}}),
            ],
        );
        assert_eq!(updated["name"], "Example Mall Store");
        assert_eq!(updated["facts"]["rdo_code"], "050");
        assert_eq!(updated["facts"]["zip_code"], "1600");
        let inherited = updated["inherited"].as_array().unwrap();
        assert!(inherited.contains(&json!("zip_code")));
        assert!(!inherited.contains(&json!("rdo_code")));
    }

    #[test]
    fn move_branch_code_rekeys_the_branch() {
        setup();
        add_branch("00001");
        add_branch("00003");
        let moved = ok(
            move_branch_code,
            vec![json!("12345678800001"), json!("00005")],
        );
        assert_eq!(moved["tin"], "12345678800005");
        assert_eq!(moved["code"], "00005");
        assert_eq!(
            call(
                move_branch_code,
                vec![json!("12345678800005"), json!("00003")]
            )
            .unwrap_err(),
            "Branch 00003 already exists."
        );
        assert_eq!(
            call(move_branch_code, vec![json!("12345678800005"), json!("12")]).unwrap_err(),
            "Branch code is 5 digits, like 00001."
        );
    }

    #[test]
    fn suggest_next_branch_code_follows_the_highest() {
        setup();
        assert_eq!(ok(suggest_next_branch_code, vec![json!(ROOT)]), "00001");
        add_branch("00004");
        assert_eq!(ok(suggest_next_branch_code, vec![json!(ROOT)]), "00005");
    }

    #[test]
    fn tax_year_bounds_are_2010_to_next_year() {
        use chrono::Datelike;
        let bounds = to_json(&tax_year_bounds_fn().unwrap());
        assert_eq!(bounds["min"], 2010);
        assert_eq!(bounds["max"], chrono::Local::now().year() + 1);
    }

    #[test]
    fn list_tax_years_is_oldest_first() {
        setup();
        assert_eq!(ok(list_tax_years, vec![json!(HEAD)]), json!([]));
        ok(add_tax_year, vec![json!(HEAD), json!(2025), json!(false)]);
        ok(add_tax_year, vec![json!(HEAD), json!(2023), json!(false)]);
        assert_eq!(ok(list_tax_years, vec![json!(HEAD)]), json!([2023, 2025]));
    }

    #[test]
    fn add_tax_year_copies_the_latest_year_and_enforces_the_bound() {
        setup();
        let first = ok(add_tax_year, vec![json!(HEAD), json!(2024), json!(true)]);
        assert_eq!(
            first,
            json!({"year": 2024, "copied_from": null, "forms": []})
        );
        ok(
            set_yearly_forms,
            vec![json!(HEAD), json!(2024), json!(["2551Q", "0605"])],
        );
        let second = ok(add_tax_year, vec![json!(HEAD), json!(2025), json!(true)]);
        assert_eq!(
            second,
            json!({"year": 2025, "copied_from": 2024, "forms": ["0605", "2551Q"]})
        );
        let max = tax_year_bounds().1;
        assert_eq!(
            call(add_tax_year, vec![json!(HEAD), json!(max + 1), json!(true)]).unwrap_err(),
            format!("Choose a tax year from 2010 to {max}.")
        );
        assert!(
            call(add_tax_year, vec![json!(HEAD), json!(2025), json!(false)])
                .unwrap_err()
                .contains("already has tax year 2025")
        );
    }

    #[test]
    fn yearly_forms_reads_the_active_list() {
        setup();
        assert_eq!(ok(yearly_forms, vec![json!(HEAD), json!(2026)]), json!([]));
        ok(
            set_yearly_forms,
            vec![json!(HEAD), json!(2026), json!(["1601C"])],
        );
        assert_eq!(
            ok(yearly_forms, vec![json!(HEAD), json!(2026)]),
            json!(["1601C"])
        );
    }

    #[test]
    fn set_yearly_forms_reports_added_and_removed() {
        setup();
        ok(
            set_yearly_forms,
            vec![json!(HEAD), json!(2026), json!(["2551Q", "1601C"])],
        );
        let change = ok(
            set_yearly_forms,
            vec![json!(HEAD), json!(2026), json!(["2551Q", "0619E", "1701Q"])],
        );
        assert_eq!(
            change,
            json!({"added": ["0619E", "1701Q"], "removed": ["1601C"], "forms": ["0619E", "1701Q", "2551Q"]})
        );
        assert!(
            call(set_yearly_forms, vec![json!(HEAD), json!(2009), json!([])])
                .unwrap_err()
                .starts_with("Choose a tax year from 2010 to ")
        );
    }

    #[test]
    fn setups_follow_the_taxpayer_type_and_round_trip_through_the_host() {
        setup();
        let listed = ok(list_setups, vec![json!("Corporation")]);
        let ids: Vec<&str> = listed
            .as_array()
            .unwrap()
            .iter()
            .map(|setup| setup["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            [
                "vat_business_corporation",
                "corporation_non_vat",
                "employer"
            ]
        );
        assert_eq!(listed[0]["name"], "VAT business");
        assert_eq!(listed[0]["forms"], json!(["2550Q", "1702Q", "1702RT"]));
        assert_eq!(listed[0]["built_in"], true);
        assert_eq!(listed[2]["adds_forms"], true);
        assert!(
            call(list_setups, vec![json!("Robot")])
                .unwrap_err()
                .contains("not a taxpayer type")
        );

        let applied = ok(
            apply_setup,
            vec![json!(HEAD), json!(2026), json!("corporation_non_vat")],
        );
        assert_eq!(applied["forms"], json!(["1702Q", "1702RT", "2551Q"]));
        assert_eq!(applied["set"], json!(["tax_classification"]));
        assert_eq!(applied["kept"], json!([]));
        let added = ok(
            apply_setup,
            vec![json!(HEAD), json!(2026), json!("employer"), json!(false)],
        );
        assert_eq!(
            added["forms"],
            json!(["1601C", "1604C", "1702Q", "1702RT", "2551Q"])
        );

        ok(
            set_year_facts,
            vec![json!(HEAD), json!(2026), json!({"eopt_tier": "Medium"})],
        );
        let saved = ok(
            save_setup,
            vec![json!(HEAD), json!(2026), json!("Our shop")],
        );
        assert_eq!(saved["name"], "Our shop");
        assert_eq!(saved["built_in"], false);
        assert_eq!(saved["facts"]["eopt_tier"], "Medium");
        assert_eq!(saved["facts"]["business_start_date"], Value::Null);
        let id = saved["id"].as_str().unwrap().to_string();
        assert!(id.starts_with("user-"));

        ok(
            set_year_facts,
            vec![json!(HEAD), json!(2026), json!({"eopt_tier": "Large"})],
        );
        let kept = ok(apply_setup, vec![json!(HEAD), json!(2026), json!(&id)]);
        assert_eq!(kept["kept"], json!(["eopt_tier"]), "asks before replacing");
        let replaced = ok(
            apply_setup,
            vec![json!(HEAD), json!(2026), json!(&id), json!(true)],
        );
        assert_eq!(replaced["set"], json!(["eopt_tier"]));

        add_branch("00001");
        let copied = ok(
            copy_setup_from,
            vec![json!(HEAD), json!("12345678800001"), json!(2026)],
        );
        assert_eq!(
            copied["forms"],
            json!(["1601C", "1604C", "1702Q", "1702RT", "2551Q"])
        );
        assert_eq!(copied["templates_copied"], json!([]));

        assert_eq!(ok(delete_setup, vec![json!(&id)]), json!(true));
        assert!(
            call(delete_setup, vec![json!("employer")])
                .unwrap_err()
                .contains("built-in")
        );
    }

    #[test]
    fn list_form_templates_counts_kept_answers() {
        setup();
        assert_eq!(ok(list_form_templates, vec![json!(HEAD)]), json!([]));
        save_return(1);
        ok(
            create_template_from_latest,
            vec![json!(HEAD), json!("2551Q")],
        );
        let listed = ok(list_form_templates, vec![json!(HEAD)]);
        assert_eq!(listed[0]["form_code"], "2551Q");
        assert!(listed[0]["answers_kept"].as_f64().unwrap() >= 1.0);
        assert_eq!(listed[0]["marked"], 0);
    }

    #[test]
    fn create_template_from_latest_needs_a_return() {
        setup();
        assert_eq!(
            call(
                create_template_from_latest,
                vec![json!(HEAD), json!("2551Q")]
            )
            .unwrap_err(),
            "Open and save a 2551Q return first. The template starts from its answers."
        );
        let id = save_return(1);
        let template = ok(
            create_template_from_latest,
            vec![json!(HEAD), json!("2551Q")],
        );
        assert_eq!(template["source"], format!("return:{id}"));
        // Update replaces, it never adds a second template.
        ok(
            create_template_from_latest,
            vec![json!(HEAD), json!("2551Q")],
        );
        assert_eq!(
            ok(list_form_templates, vec![json!(HEAD)])
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn create_template_from_return_uses_the_chosen_return() {
        setup();
        let chosen = save_return(1);
        save_return(2);
        let template = ok(create_template_from_return, vec![json!(chosen)]);
        assert_eq!(template["form_code"], "2551Q");
        assert_eq!(template["source"], format!("return:{chosen}"));
        assert!(
            call(create_template_from_return, vec![json!(999_999)])
                .unwrap_err()
                .contains("No return with id 999999")
        );
    }

    #[test]
    fn set_template_stop_takes_true_false_and_null_on_any_form() {
        setup();
        let stop = |key: &str, on: Value| {
            ok(
                set_template_stop,
                vec![json!(HEAD), json!("0619E"), json!(key), on],
            )
        };
        assert_eq!(
            ok(template_stops, vec![json!(HEAD), json!("0619E")]),
            Value::Null
        );
        let on = stop("frm0619E:txtTax14", json!(true));
        assert_eq!(on["stop_edits"], json!({"frm0619E:txtTax14": true}));
        assert_eq!(on["stops"], json!(["frm0619E:txtTax14"]));
        assert!(
            on["markable"]
                .as_array()
                .unwrap()
                .contains(&json!("frm0619E:txtTax14"))
        );
        let off = stop("frm0619E:txtTax14", json!(false));
        assert_eq!(off["stop_edits"], json!({"frm0619E:txtTax14": false}));
        assert_eq!(off["stops"], json!([]));
        let auto = stop("frm0619E:txtTax14", Value::Null);
        assert_eq!(auto["stop_edits"], json!({}));
        // The older page's reader still answers for the template made.
        let stops = ok(template_stops, vec![json!(HEAD), json!("0619E")]);
        assert_eq!(stops["stops"], json!([]));
        assert!(
            call(
                set_template_stop,
                vec![
                    json!(HEAD),
                    json!("0619E"),
                    json!("frm0619E:txtTax14"),
                    json!("yes")
                ]
            )
            .is_err()
        );
        let listed = ok(list_form_templates, vec![json!(HEAD)]);
        assert_eq!(listed[0]["form_code"], "0619E");
        assert_eq!(listed[0]["marked"], 0);
    }

    #[test]
    fn delete_form_template_reports_whether_one_existed() {
        setup();
        save_return(1);
        ok(
            create_template_from_latest,
            vec![json!(HEAD), json!("2551Q")],
        );
        assert_eq!(
            ok(delete_form_template, vec![json!(HEAD), json!("2551Q")]),
            true
        );
        assert_eq!(
            ok(delete_form_template, vec![json!(HEAD), json!("2551Q")]),
            false
        );
    }

    #[test]
    fn reference_lists_carry_codes_and_places() {
        let rdos = to_json(&list_rdos().unwrap());
        let rdos = rdos.as_array().unwrap();
        assert!(rdos.iter().any(|r| r["code"] == "040"));
        assert!(rdos.iter().all(|r| r["description"].is_string()));
        let zips = to_json(&list_zip_codes().unwrap());
        assert!(
            zips.as_array()
                .unwrap()
                .iter()
                .any(|z| z.as_str().unwrap().starts_with("1000 - "))
        );
    }

    // ---------- confirmation inbox ----------

    /// A `bir_poll_email <address>` job row like the submission flow creates.
    fn poll_job(email: &str) -> bir_core::db::Job {
        bir_core::db::Job {
            id: None,
            name: format!("Waiting for confirmation for {email}"),
            job_type: "System".to_string(),
            cron_expr: Some("0 * * * * *".to_string()),
            command: Some(format!("bir_poll_email {email}")),
            status: "Queued".to_string(),
            retries: 0,
            last_run_at: None,
            next_run_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            output_log: None,
        }
    }

    #[test]
    fn inbox_state_defaults_for_a_new_taxpayer() {
        setup();
        let state = ok(inbox_state, vec![json!(HEAD)]);
        // The branch email is the default tracking mailbox.
        assert_eq!(state["tin"], HEAD);
        assert_eq!(state["inbox_email"], "office@example.com");
        assert_eq!(state["auth_method"], "AppPassword");
        assert_eq!(state["tracking_enabled"], false);
        assert_eq!(state["has_app_password"], false);
        assert_eq!(state["oauth_connected"], false);
        // Whether Google is offered is the build's fact (a client id and secret), not the profile's.
        assert_eq!(
            state["google_available"],
            bir_core::email::google_sign_in_available()
        );
        assert_eq!(state["still_pending"], false);
        assert_eq!(state["matching_tins"], json!([HEAD]));
        assert_eq!(state["last_check_at"], Value::Null);
    }

    #[test]
    fn set_inbox_app_password_persists_and_never_echoes_the_secret() {
        setup();
        let state = ok(
            set_inbox_app_password,
            vec![
                json!(HEAD),
                json!({
                    "imap_email": " receipts@example.com ",
                    "imap_host": "",
                    "app_password": "abcd efgh ijkl mnop"
                }),
            ],
        );
        assert_eq!(state["auth_method"], "AppPassword");
        assert_eq!(state["inbox_email"], "receipts@example.com");
        assert_eq!(state["imap_host"], "imap.gmail.com");
        assert_eq!(state["has_app_password"], true);
        let raw = serde_json::to_string(&state).unwrap();
        assert!(
            !raw.contains("abcdefghijklmnop"),
            "the stored secret must never leave the bridge: {raw}"
        );
        with_database(|db| {
            let profile = db.get_profile(HEAD).unwrap().unwrap();
            // Spaces stripped exactly like the legacy Email Settings tab.
            assert_eq!(
                profile.imap_app_password.as_deref(),
                Some("abcdefghijklmnop")
            );
            assert_eq!(profile.imap_host, None);
            assert_eq!(
                profile.email_auth_method,
                bir_core::profile::EmailAuthMethod::AppPassword
            );
            Ok(())
        })
        .unwrap();

        // An omitted password keeps the stored one; an empty one is refused.
        let again = ok(
            set_inbox_app_password,
            vec![json!(HEAD), json!({"imap_email": "receipts@example.com"})],
        );
        assert_eq!(again["has_app_password"], true);
        let refused = call(
            set_inbox_app_password,
            vec![
                json!(HEAD),
                json!({"imap_email": "x@y.z", "app_password": "   "}),
            ],
        );
        assert!(refused.unwrap_err().contains("app password is empty"));
        let refused = call(
            set_inbox_app_password,
            vec![json!(HEAD), json!({"imap_email": "   "})],
        );
        assert!(refused.unwrap_err().contains("inbox email"));
    }

    #[test]
    fn set_inbox_tracking_toggles_the_profile_flag() {
        setup();
        let state = ok(set_inbox_tracking, vec![json!(HEAD), json!(true)]);
        assert_eq!(state["tracking_enabled"], true);
        with_database(|db| {
            assert!(
                db.get_profile(HEAD)
                    .unwrap()
                    .unwrap()
                    .email_tracking_enabled
            );
            Ok(())
        })
        .unwrap();
        let state = ok(set_inbox_tracking, vec![json!(HEAD), json!(false)]);
        assert_eq!(state["tracking_enabled"], false);
    }

    #[test]
    fn disconnect_inbox_oauth_clears_the_shared_grant_and_tracking() {
        setup();
        with_database(|db| {
            db.persist_google_oauth_for_inbox(
                "receipts@example.com",
                "access-1",
                "refresh-1",
                Some(HEAD),
            )
            .unwrap();
            Ok(())
        })
        .unwrap();
        let state = ok(inbox_state, vec![json!(HEAD)]);
        assert_eq!(state["oauth_connected"], true);
        assert_eq!(state["tracking_enabled"], true);

        let state = ok(disconnect_inbox_oauth, vec![json!(HEAD)]);
        assert_eq!(state["oauth_connected"], false);
        assert_eq!(state["tracking_enabled"], false);
        with_database(|db| {
            assert!(
                db.inbox_oauth_tokens("receipts@example.com")
                    .unwrap()
                    .is_none()
            );
            let profile = db.get_profile(HEAD).unwrap().unwrap();
            assert!(!profile.has_usable_oauth_refresh());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn disconnect_inbox_oauth_refuses_when_no_inbox_is_set() {
        setup();
        ok(
            create_taxpayer,
            vec![json!({
                "tin_root": "987-654-321", "name": "No Mail Corp", "taxpayer_type": "Corporation",
                "head_office": {"rdo_code": "040", "registered_address": "a", "zip_code": "1600",
                                "phone": "1", "email": ""}
            })],
        );
        let refused = call(disconnect_inbox_oauth, vec![json!("98765432100000")]);
        assert_eq!(
            refused.unwrap_err(),
            "No inbox email is set, so there is nothing to disconnect."
        );
    }

    #[test]
    fn persist_oauth_grant_marks_connected_and_stays_secret_free() {
        setup();
        let db = database_handle().unwrap();
        let state = to_json(
            &persist_oauth_grant(&db, HEAD, "receipts@example.com", "access-x", "refresh-x")
                .unwrap(),
        );
        assert_eq!(state["oauth_connected"], true);
        assert_eq!(state["auth_method"], "GoogleOAuth");
        assert_eq!(state["tracking_enabled"], true);
        assert_eq!(state["oauth_inbox_email"], "receipts@example.com");
        let raw = serde_json::to_string(&state).unwrap();
        assert!(!raw.contains("access-x") && !raw.contains("refresh-x"));

        // An empty refresh fails closed — the inbox must not look connected.
        let refused = persist_oauth_grant(&db, HEAD, "receipts@example.com", "access-y", "  ");
        assert!(refused.unwrap_err().message().contains("refresh token"));
    }

    #[test]
    fn apply_connection_result_enables_tracking_on_success_and_clears_on_failure() {
        setup();
        let db = database_handle().unwrap();
        let success = to_json(&apply_connection_result(&db, HEAD, Ok(None), None).unwrap());
        assert_eq!(success["ok"], true);
        assert_eq!(success["state"]["tracking_enabled"], true);

        let failure = to_json(
            &apply_connection_result(&db, HEAD, Err(anyhow::anyhow!("IMAP login refused")), None)
                .unwrap(),
        );
        assert_eq!(failure["ok"], false);
        assert!(
            failure["message"]
                .as_str()
                .unwrap()
                .contains("IMAP login refused")
        );
        assert_eq!(failure["state"]["tracking_enabled"], false);
    }

    #[test]
    fn check_inbox_now_runs_the_poll_and_stamps_the_job() {
        setup();
        ok(
            set_inbox_app_password,
            vec![
                json!(HEAD),
                json!({"imap_email": "receipts@example.com", "app_password": "abcd efgh ijkl mnop"}),
            ],
        );
        with_database(|db| {
            db.save_job(poll_job("receipts@example.com")).unwrap();
            Ok(())
        })
        .unwrap();

        // Fake transport: the real one is bir_core's IMAP fetch, covered by
        // bir-core tests; here the poll answers "worked, nothing pending".
        TEST_INBOX_POLL.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(|email, _db| {
                assert_eq!(email, "receipts@example.com");
                (true, false, None)
            }));
        });
        let db = database_handle().unwrap();
        let result = to_json(&check_inbox_now_run(HEAD, &db).unwrap());
        assert_eq!(result["ok"], true);
        assert_eq!(result["still_pending"], false);
        assert_eq!(result["error"], Value::Null);
        assert_eq!(result["state"]["inbox_email"], "receipts@example.com");
        with_database(|db| {
            let job = db
                .list_jobs()
                .unwrap()
                .into_iter()
                .find(|job| job.command.as_deref() == Some("bir_poll_email receipts@example.com"))
                .unwrap();
            // A clean poll with nothing left pending archives the job.
            assert_eq!(job.status, "Archived");
            assert!(job.last_run_at.is_some());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn check_inbox_now_reports_poll_failure_and_requeues_the_job() {
        setup();
        ok(
            set_inbox_app_password,
            vec![
                json!(HEAD),
                json!({"imap_email": "receipts@example.com", "app_password": "abcdefghijklmnop"}),
            ],
        );
        with_database(|db| {
            db.save_job(poll_job("receipts@example.com")).unwrap();
            Ok(())
        })
        .unwrap();
        TEST_INBOX_POLL.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(|_email, _db| {
                (false, true, Some("IMAP authentication failed".to_string()))
            }));
        });
        let db = database_handle().unwrap();
        let result = to_json(&check_inbox_now_run(HEAD, &db).unwrap());
        assert_eq!(result["ok"], false);
        assert_eq!(result["still_pending"], true);
        assert_eq!(result["error"], "IMAP authentication failed");
        with_database(|db| {
            let job = db
                .list_jobs()
                .unwrap()
                .into_iter()
                .find(|job| job.command.as_deref() == Some("bir_poll_email receipts@example.com"))
                .unwrap();
            assert_eq!(job.status, "Queued");
            assert_eq!(job.retries, 1);
            assert_eq!(
                job.output_log.as_deref(),
                Some("IMAP authentication failed")
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn check_inbox_now_refuses_without_an_inbox_address() {
        setup();
        ok(
            create_taxpayer,
            vec![json!({
                "tin_root": "987-654-321", "name": "No Mail Corp", "taxpayer_type": "Corporation",
                "head_office": {"rdo_code": "040", "registered_address": "a", "zip_code": "1600",
                                "phone": "1", "email": ""}
            })],
        );
        let db = database_handle().unwrap();
        let refused = check_inbox_now_run("98765432100000", &db);
        assert!(
            refused
                .unwrap_err()
                .message()
                .contains("No inbox email is set")
        );
    }

    // ---------- no test reaches the network through an inbox verb ----------

    /// Calls the registered async verb `name` for `tin` the way the page
    /// does: its network leg runs on the verb's own thread, which a fake
    /// installed on the test thread never reaches.
    fn call_async_verb(name: &str, tin: &str) -> Result<Value, String> {
        let future = crate::bir_module::module()
            .begin(name, &args(vec![json!(tin)]))
            .map_err(|error| error.message().to_string())?;
        futures::executor::block_on(future)
            .map(|value| to_json(&value))
            .map_err(|error| error.message().to_string())
    }

    /// The poll leg refuses without a fake on its thread, through the
    /// registered verb (its `inbox-check` thread) and on this one, and the
    /// job row stays as saved: no poll ran.
    #[test]
    fn check_inbox_now_refuses_to_poll_without_a_fake() {
        setup();
        with_database(|db| {
            db.save_job(poll_job("office@example.com")).unwrap();
            Ok(())
        })
        .unwrap();
        let refused = "The inbox poll is not available in tests.";
        assert_eq!(
            call_async_verb("check_inbox_now", HEAD).unwrap_err(),
            refused
        );
        let db = database_handle().unwrap();
        assert_eq!(
            check_inbox_now_run(HEAD, &db).unwrap_err().message(),
            refused
        );
        with_database(|db| {
            let job = db
                .list_jobs()
                .unwrap()
                .into_iter()
                .find(|job| job.command.as_deref() == Some("bir_poll_email office@example.com"))
                .unwrap();
            assert_eq!(job.status, "Queued");
            assert_eq!(job.retries, 0);
            assert_eq!(job.last_run_at, None);
            Ok(())
        })
        .unwrap();
    }

    /// The login probe refuses without a fake on its thread, through the
    /// registered verb and on this one, before any verdict is recorded (a
    /// failed probe would turn tracking off). A fake installed on this thread
    /// answers in its place.
    #[test]
    fn test_inbox_connection_refuses_to_log_in_without_a_fake() {
        setup();
        ok(set_inbox_tracking, vec![json!(HEAD), json!(true)]);
        let refused = "The inbox connection test is not available in tests.";
        assert_eq!(
            call_async_verb("test_inbox_connection", HEAD).unwrap_err(),
            refused
        );
        let db = database_handle().unwrap();
        assert_eq!(
            test_inbox_connection_run(HEAD, &db, None)
                .unwrap_err()
                .message(),
            refused
        );
        assert_eq!(ok(inbox_state, vec![json!(HEAD)])["tracking_enabled"], true);

        TEST_INBOX_CONNECTION.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(|profile| {
                assert_eq!(profile.inbox_email(), "office@example.com");
                Err(anyhow::anyhow!("IMAP login refused"))
            }));
        });
        let verdict = to_json(&test_inbox_connection_run(HEAD, &db, None).unwrap());
        assert_eq!(verdict["ok"], false);
        assert_eq!(verdict["state"]["tracking_enabled"], false);
    }

    /// The browser sign-in refuses without a fake on its thread, through the
    /// registered verb and on this one, and nothing is connected. A fake
    /// installed on this thread answers in its place.
    #[test]
    fn connect_inbox_oauth_refuses_to_open_a_browser_without_a_fake() {
        setup();
        let refused = "The Google sign-in is not available in tests.";
        assert_eq!(
            call_async_verb("connect_inbox_oauth", HEAD).unwrap_err(),
            refused
        );
        let db = database_handle().unwrap();
        assert_eq!(
            connect_inbox_oauth_run(HEAD, &db, None)
                .unwrap_err()
                .message(),
            refused
        );
        assert_eq!(ok(inbox_state, vec![json!(HEAD)])["oauth_connected"], false);

        TEST_INBOX_OAUTH.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(|| {
                Ok((
                    "receipts@example.com".to_string(),
                    "access-z".to_string(),
                    "refresh-z".to_string(),
                ))
            }));
        });
        let state = to_json(&connect_inbox_oauth_run(HEAD, &db, None).unwrap());
        assert_eq!(state["oauth_connected"], true);
        assert_eq!(state["oauth_inbox_email"], "receipts@example.com");
    }

    // ---------- year facts (interface I-1) ----------

    const JUANA_ROOT: &str = "000123453";
    const JUANA_HEAD: &str = "00012345300000";

    /// Fresh database with the demo individual and her head office.
    fn setup_individual() {
        use_ephemeral_database();
        ok(
            create_taxpayer,
            vec![json!({
                "tin_root": "000-123-453",
                "name": "Juana Santos",
                "taxpayer_type": "Individual",
                "head_office": {
                    "rdo_code": "039",
                    "registered_address": "Quezon City",
                    "zip_code": "1100",
                    "phone": "09170000000",
                    "email": "juana@example.com"
                }
            })],
        );
    }

    fn bir_call(name: &str, values: Vec<Value>) -> Result<Value, String> {
        crate::bir_module::module()
            .call(name, &args(values))
            .map(|value| to_json(&value))
            .map_err(|error| error.message().to_string())
    }

    /// A saved Draft 2551Q for Q1 of `year`, built from the year's profile,
    /// with Item 13 answered.
    fn save_year_return(
        tin: &str,
        year: u16,
        eight_percent: bool,
    ) -> bir_core::forms::Form2551QDraft {
        use bir_core::forms::form_2551q::Item13Election;
        with_database(|db| {
            let profile = db.get_profile_for_year(tin, year).unwrap().unwrap();
            let mut draft =
                bir_core::forms::Form2551QDraft::new_from_effective_profile(&profile, year, 1);
            draft.item_13_election = if eight_percent {
                Item13Election::EightPercent
            } else {
                Item13Election::Graduated
            };
            db.save_2551q_draft(&draft).unwrap();
            Ok(draft)
        })
        .unwrap()
    }

    #[test]
    fn year_facts_round_trip_through_the_host() {
        setup_individual();
        ok(
            create_branch,
            vec![
                json!(JUANA_ROOT),
                json!({"branch_code": "00001", "name": "Stall"}),
            ],
        );
        let branch = "00012345300001";
        ok(
            set_yearly_forms,
            vec![json!(branch), json!(2025), json!(["2551Q", "1701"])],
        );
        let view = ok(
            set_year_facts,
            vec![
                json!(branch),
                json!(2025),
                json!({"tax_classification": "SelfEmployed", "line_of_business": "Stall"}),
            ],
        );
        assert_eq!(view["tin"], branch);
        assert_eq!(view["year"], 2025);
        assert_eq!(view["facts"]["tax_classification"], "SelfEmployed");
        assert_eq!(view["facts"]["line_of_business"], "Stall");
        assert_eq!(view["facts"]["income_tax_election"], Value::Null);
        assert_eq!(view["facts"]["atc_codes"], json!([]));
        assert_eq!(view["owner"]["tax_classification"], "head_office");
        assert_eq!(view["owner"]["line_of_business"], "branch");
        assert_eq!(view["used_by"]["birth_date"], json!(["1701"]));
        assert_eq!(
            view["used_by"]["income_tax_election"],
            json!(["1701", "2551Q"])
        );
        assert!(
            view["not_set"]
                .as_array()
                .unwrap()
                .contains(&json!("birth_date"))
        );
        assert_eq!(view["election_locked"], false);
        assert_eq!(view["election_source"], Value::Null);
        // The head office holds the income type, not the line of business.
        let head = ok(year_facts, vec![json!(JUANA_HEAD), json!(2025)]);
        assert_eq!(head["facts"]["tax_classification"], "SelfEmployed");
        assert_eq!(head["facts"]["line_of_business"], Value::Null);
        assert_eq!(head["used_by"]["tax_classification"], json!([]));
        // Null clears; an unknown key or value is refused.
        let cleared = ok(
            set_year_facts,
            vec![
                json!(branch),
                json!(2025),
                json!({"line_of_business": null}),
            ],
        );
        assert_eq!(cleared["facts"]["line_of_business"], Value::Null);
        assert!(
            call(
                set_year_facts,
                vec![
                    json!(branch),
                    json!(2025),
                    json!({"is_vat_registered": true})
                ]
            )
            .is_err()
        );
        assert!(
            call(
                set_year_facts,
                vec![json!(branch), json!(2025), json!({"eopt_tier": "Huge"})]
            )
            .is_err()
        );

        let chips = ok(tax_types_for_year, vec![json!(branch), json!(2025)]);
        assert_eq!(
            chips,
            json!([
                {"type": "percentage_tax", "label": "Percentage tax", "forms": ["2551Q"]},
                {"type": "income_tax", "label": "Income tax", "forms": ["1701"]},
            ])
        );
        assert_eq!(
            ok(tax_types_for_year, vec![json!(branch), json!(2024)]),
            json!([])
        );

        // Two annual income tax returns of one kind are named, for a warning.
        assert_eq!(
            ok(
                annual_return_conflicts,
                vec![json!(["1701", "1701A", "2551Q", "1702RT"])]
            ),
            json!([["1701", "1701A"]])
        );
        assert_eq!(
            ok(annual_return_conflicts, vec![json!(["1701", "1701Q"])]),
            json!([])
        );
    }

    #[test]
    fn update_taxpayer_accepts_a_birth_date() {
        setup_individual();
        let set = ok(
            update_taxpayer,
            vec![json!(JUANA_ROOT), json!({"birth_date": "1990-01-02"})],
        );
        assert_eq!(set["birth_date"], "1990-01-02");
        let kept = ok(
            update_taxpayer,
            vec![json!(JUANA_ROOT), json!({"name": "Juana D. Santos"})],
        );
        assert_eq!(kept["birth_date"], "1990-01-02", "omitted: unchanged");
        let cleared = ok(
            update_taxpayer,
            vec![json!(JUANA_ROOT), json!({"birth_date": null})],
        );
        assert_eq!(cleared["birth_date"], Value::Null);
        assert!(
            call(
                update_taxpayer,
                vec![json!(JUANA_ROOT), json!({"birth_date": "Jan 2"})]
            )
            .is_err()
        );
    }

    /// The current screens' calls keep working on year facts: readiness,
    /// the registration control and the 2551Q Item 13 question.
    #[test]
    fn the_election_host_functions_map_onto_year_facts() {
        setup_individual();
        let ready = ok(
            income_tax_election_readiness,
            vec![json!(JUANA_HEAD), json!(2025)],
        );
        assert_eq!(ready["gap"], "not_business_income");
        assert_eq!(ready["new_registration_after"], Value::Null);
        assert_eq!(ready["issues"], json!([]));
        assert!(
            call(
                record_income_tax_election,
                vec![json!(JUANA_HEAD), json!(2025), json!("eight_percent")]
            )
            .is_err()
        );
        assert_eq!(
            ok(
                confirm_income_registration,
                vec![
                    json!(JUANA_HEAD),
                    json!("SelfEmployed"),
                    json!("2025-01-01")
                ]
            ),
            json!(true)
        );
        let ready = ok(
            income_tax_election_readiness,
            vec![json!(JUANA_HEAD), json!(2025)],
        );
        assert_eq!(ready["gap"], "ready");
        assert_eq!(ready["classification"], "SelfEmployed");
        assert_eq!(
            ok(
                record_income_tax_election,
                vec![json!(JUANA_HEAD), json!(2025), json!("eight_percent")]
            ),
            json!(true)
        );
        let view = ok(year_facts, vec![json!(JUANA_HEAD), json!(2025)]);
        assert_eq!(view["facts"]["income_tax_election"], "EightPercent");
        assert_eq!(view["election_source"], "Tax profile");
        // No confirmed profile version was written for any of it.
        with_database(|db| {
            assert!(
                db.get_profile(JUANA_HEAD)
                    .unwrap()
                    .unwrap()
                    .profile_versions
                    .is_empty()
            );
            Ok(())
        })
        .unwrap();
    }

    /// The 2551Q call sites in the host — the queue (`queue_form_draft`),
    /// the page's "Use profile values" (`screen_form_use_profile`) and its
    /// open (`screen_form_open`) — for a profile with no confirmed version
    /// and for one whose versions are only drafts and reviews.
    #[test]
    fn the_2551q_host_paths_need_no_confirmed_profile_version() {
        use bir_core::profile::{TaxProfileVersion, TaxProfileVersionStatus};
        for messy in [false, true] {
            setup_individual();
            ok(
                set_year_facts,
                vec![
                    json!(JUANA_HEAD),
                    json!(2025),
                    json!({"tax_classification": "SelfEmployed"}),
                ],
            );
            if messy {
                with_database(|db| {
                    let mut profile = db.get_profile(JUANA_HEAD).unwrap().unwrap();
                    let mut draft = TaxProfileVersion::from_profile_backfill(&profile);
                    draft.id = "draft".into();
                    draft.status = TaxProfileVersionStatus::Draft;
                    draft.cor.registered_name = "Someone Else".into();
                    let mut review = draft.clone();
                    review.id = "review".into();
                    review.status = TaxProfileVersionStatus::NeedsReview;
                    review.effective_from = None;
                    profile.profile_versions = vec![draft, review];
                    db.save_profile(profile).unwrap();
                    Ok(())
                })
                .unwrap();
            }

            // The page opens a 2551Q of the year from the year's facts.
            let opened = block_on_open(JUANA_HEAD, 2025, 2);
            assert_eq!(
                opened["draft"]["taxpayer_name"], "Juana Santos",
                "messy={messy}"
            );
            assert!(
                opened["draft"]["profile_resolution_error"].is_null(),
                "messy={messy}"
            );

            // "Use profile values" on a Q2 draft takes the recorded election.
            let draft = save_year_return(JUANA_HEAD, 2025, true);
            with_database(|db| {
                db.record_income_tax_election(
                    JUANA_HEAD,
                    2025,
                    bir_core::profile::IncomeTaxElection::EightPercent,
                )
                .map_err(|error| HostError::new(error.to_string()))?;
                Ok(())
            })
            .unwrap();
            let mut q2 = serde_json::to_value(&draft).unwrap();
            q2["quarter"] = json!(2);
            q2["id"] = Value::Null;
            q2["annual_income_tax_election"] = json!("unrecorded");
            let used = bir_call(
                "screen_form_use_profile",
                vec![json!("2551Q"), json!(2025), json!(2), q2],
            )
            .unwrap_or_else(|error| panic!("messy={messy}: {error}"));
            assert_eq!(used["draft"]["annual_income_tax_election"], "eight_percent");
            assert!(used["draft"]["profile_resolution_error"].is_null());

            // The queue.
            let id = bir_call(
                "queue_form_draft",
                vec![json!(JUANA_HEAD), json!("2551Q"), json!(2025), json!(1)],
            )
            .unwrap_or_else(|error| panic!("messy={messy}: {error}"));
            assert!(id.as_f64().unwrap() > 0.0);
            let view = ok(year_facts, vec![json!(JUANA_HEAD), json!(2025)]);
            assert_eq!(view["facts"]["income_tax_election"], "EightPercent");
        }
    }

    fn block_on_open(tin: &str, year: u16, quarter: u8) -> Value {
        let future = crate::bir_module::module()
            .begin(
                "screen_form_open",
                &args(vec![
                    json!(tin),
                    json!("2551Q"),
                    json!(year),
                    json!(quarter),
                ]),
            )
            .unwrap();
        let mut future = std::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        loop {
            if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return to_json(&output.unwrap());
            }
        }
    }

    // ---------- the account's TINs (Track D) ----------

    /// A fixed access token, as the host's sign-in would give it.
    struct FixedToken(String);
    impl bir_core::bir_submit::TokenSource for FixedToken {
        fn access_token(
            &self,
            _force_refresh: bool,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<String, bir_core::bir_submit::ClientError>>
                    + Send
                    + '_,
            >,
        > {
            let token = self.0.clone();
            Box::pin(async move { Ok(token) })
        }
        fn login(&self) -> Option<String> {
            Some("filer@example.test".into())
        }
        fn session_ended(&self) {}
    }

    /// The in-repo mock account on a runtime of its own, its rows linking to
    /// the app's account site, and a client for it installed on this thread
    /// for the account verbs.
    fn against_the_account(body: impl FnOnce(&bir_core::mock_account::MockAccount)) {
        use bir_core::mock_account::{MockAccount, MockOptions};
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mock = runtime
            .block_on(MockAccount::start_with(
                "127.0.0.1:0".parse().unwrap(),
                MockOptions {
                    account_origin: Some(bir_core::bir_submit::tins::account_site()),
                    ..MockOptions::default()
                },
            ))
            .unwrap();
        let token = mock.app_token("filer@example.test");
        let submitter = Submitter::new(
            bir_core::bir_submit::SubmitConfig::new(&mock.url()).unwrap(),
            Box::new(bir_core::bir_submit::secrets::MemoryStore::default()),
            std::sync::Arc::new(FixedToken(token)),
        );
        TEST_ACCOUNT_SUBMITTER
            .with(|slot| *slot.borrow_mut() = Some(std::sync::Arc::new(submitter)));
        struct Uninstall;
        impl Drop for Uninstall {
            fn drop(&mut self) {
                TEST_ACCOUNT_SUBMITTER.with(|slot| slot.borrow_mut().take());
            }
        }
        let _uninstall = Uninstall;
        body(&mock);
    }

    /// Calls the registered async verb `name` with `values`, as the page does.
    fn call_verb(name: &str, values: Vec<Value>) -> Result<Value, String> {
        let future = crate::bir_module::module()
            .begin(name, &args(values))
            .map_err(|error| error.message().to_string())?;
        futures::executor::block_on(future)
            .map(|value| to_json(&value))
            .map_err(|error| error.message().to_string())
    }

    /// "Juana Santos", an individual, beside the corporation of [`setup`].
    fn add_juana() {
        ok(
            create_taxpayer,
            vec![
                json!({"tin_root": "000-123-453", "name": "Juana Santos", "taxpayer_type": "Individual", "head_office": {}}),
            ],
        );
    }

    /// AL2's real-entry regression test: the verb's own context reaches the process-wide
    /// AccountTokens. The client is the test client's only injection; its token source is the
    /// real one, not FixedToken.
    #[test]
    fn account_tins_through_real_account_tokens_uses_the_verb_context() {
        use bir_core::mock_account::{MockAccount, MockOptions, TinState};
        let _turn = crate::session::test_turn();
        setup();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mock = runtime
            .block_on(MockAccount::start_with(
                "127.0.0.1:0".parse().unwrap(),
                MockOptions::default(),
            ))
            .unwrap();
        let email = "filer@example.test";
        mock.add_tin(email, "123-456-788-00000", TinState::Approved);
        let access = mock.app_token(email);
        let stored = bir_core::cloud_account::StoredAccount {
            server_url: mock.url(),
            access_token: access,
            refresh_token: "refresh-not-needed-while-access-is-valid".into(),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: "user-filer".into(),
            email: Some(email.into()),
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        };
        bir_core::account_session::save_stored(&stored).unwrap();
        let owner = crate::session::Owner::of_stored(&stored).unwrap();
        crate::session::CLAIM_SPAWN_SUPPRESSED.store(true, std::sync::atomic::Ordering::SeqCst);
        crate::session::begin_open(owner, 1, crate::session::OpenWhy::SignIn);
        let submitter = Submitter::new(
            bir_core::bir_submit::SubmitConfig::new(&mock.url()).unwrap(),
            Box::new(bir_core::bir_submit::secrets::MemoryStore::default()),
            std::sync::Arc::new(bir_core::account_session::AccountTokens),
        );
        TEST_ACCOUNT_SUBMITTER
            .with(|slot| *slot.borrow_mut() = Some(std::sync::Arc::new(submitter)));
        let result = call_verb("account_tins", vec![json!(0)]);
        TEST_ACCOUNT_SUBMITTER.with(|slot| slot.borrow_mut().take());
        crate::session::reset_for_tests();
        bir_core::account_session::clear_stored();
        let list = result
            .unwrap_or_else(|error| panic!("the real AccountTokens verb call failed: {error}"));
        assert_eq!(list["complete"], true);
        assert!(
            list["rows"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| { row["tin_root"] == "123456788" && row["state"] == "activated" }),
            "the account row did not list: {list}"
        );
    }

    /// One row per TIN and branch on the account, reduced to what screens
    /// show, then a `not_activated` row for each branch on this device the
    /// account lacks.
    #[test]
    fn account_tins_lists_the_account_and_marks_local_profiles_not_activated() {
        use bir_core::mock_account::TinState;
        setup();
        add_branch("00001");
        add_juana();
        against_the_account(|mock| {
            let email = "filer@example.test";
            mock.add_tin(email, "123-456-788-00000", TinState::Approved);
            mock.add_tin(
                email,
                "000-123-453-00000",
                TinState::NeedsFix {
                    reasons: vec![("blurry".into(), "A photo is too blurry to read.".into())],
                    note: None,
                },
            );
            mock.add_tin(email, "000-123-460-00000", TinState::Pending);
            mock.add_tin(email, "111-111-114-00000", TinState::Approved);
            let list = call_verb("account_tins", vec![json!(0)]).unwrap();
            assert_eq!(list["complete"], true);
            assert_eq!(list["stale"], false);
            let fetched_at = list["fetched_at"].as_f64().unwrap();
            let rows = list["rows"].as_array().unwrap();
            let states: Vec<(&str, &str, &str)> = rows
                .iter()
                .map(|row| {
                    (
                        row["tin_root"].as_str().unwrap(),
                        row["branch"].as_str().unwrap(),
                        row["state"].as_str().unwrap(),
                    )
                })
                .collect();
            assert_eq!(
                states,
                [
                    ("123456788", "00000", "activated"),
                    ("111111114", "00000", "activated"),
                    ("000123453", "00000", "fix"),
                    ("000123460", "00000", "pending"),
                    ("123456788", "00001", "not_activated"),
                ]
            );
            for row in rows {
                assert_eq!(row["stale"], false);
                assert_eq!(row["fetched_at"].as_f64().unwrap(), fetched_at);
                assert_eq!(row["action_url"].is_null(), row["state"] != "fix", "{row}");
                assert_eq!(row["test_only"], row["tin_root"] == "111111114", "{row}");
            }
            assert!(
                rows[2]["action_url"]
                    .as_str()
                    .unwrap()
                    .starts_with("https://account.buwiz.com/tins/claims/")
            );
            assert_eq!(
                rows[2]["message"],
                "Sent back for a fix: A photo is too blurry to read."
            );
            assert!(rows[4]["message"].is_null());
            assert_eq!(mock.tax_profile_posts(), 0, "a read never adds");
        });
    }

    /// A list that is not complete says nothing about a TIN missing from it:
    /// no `not_activated` row. A link off the account site was never kept.
    #[test]
    fn an_incomplete_list_marks_nothing_not_activated() {
        let body = json!({"profiles":[
            {"tin":"123-456-788-00000","registered_name":"X","status":"needs_fix","can_send":false,"next":"fix",
             "action":{"label":"Fix","url":"https://evil.example/fix"}}
        ]});
        let list = AccountTins::parse(&body, 7);
        let local = [("123456788".to_owned(), "00001".to_owned())];
        let shown = tin_status_list(&list, &local);
        assert_eq!(shown["complete"], false);
        let rows = shown["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["state"], "fix");
        assert!(rows[0]["action_url"].is_null());
        let complete = AccountTins::parse(
            &json!({"include":"all","profiles":body["profiles"].clone()}),
            7,
        );
        let rows = tin_status_list(&complete, &local)["rows"].clone();
        assert_eq!(rows[1]["state"], "not_activated");
        assert_eq!(rows[1]["fetched_at"], 7);
    }

    /// "Activate this TIN to send" posts the profile's kind and registered
    /// name for its 14-digit TIN, and the row comes back to fix; pressed
    /// again, the account's answer is the app's sentence and the list then
    /// shows the row.
    #[test]
    fn activate_tin_posts_the_profiles_kind_and_name() {
        setup();
        add_branch("00001");
        add_juana();
        against_the_account(|mock| {
            let row = call_verb("activate_tin", vec![json!("12345678800001")]).unwrap();
            assert_eq!(row["state"], "fix");
            assert_eq!(
                (row["tin_root"].as_str(), row["branch"].as_str()),
                (Some(ROOT), Some("00001"))
            );
            assert!(row["action_url"].as_str().unwrap().ends_with("/verify"));
            call_verb("activate_tin", vec![json!("00012345300000")]).unwrap();
            let held: Vec<(String, String, String)> = mock
                .tins_of("filer@example.test")
                .into_iter()
                .map(|(kind, tin, name, _)| (kind, tin, name))
                .collect();
            assert_eq!(
                held,
                [
                    (
                        "business".into(),
                        "123-456-788-00001".into(),
                        "Example Trading Corp".into()
                    ),
                    (
                        "personal".into(),
                        "000-123-453-00000".into(),
                        "Juana Santos".into()
                    ),
                ]
            );
            assert_eq!(
                call_verb("activate_tin", vec![json!("12345678800001")]),
                Err("This TIN is already on your account, waiting to be verified.".into())
            );
            let list = call_verb("account_tins", vec![json!(60)]).unwrap();
            let states: Vec<&str> = list["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["state"].as_str().unwrap())
                .collect();
            assert_eq!(states, ["fix", "fix", "not_activated"]);
            // Only a tax profile on this device, by its 14 digits.
            assert_eq!(
                call_verb("activate_tin", vec![json!("12345678800002")]),
                Err("This tax profile is not on this device.".into())
            );
            assert!(call_verb("activate_tin", vec![json!("123456788")]).is_err());
            assert_eq!(mock.tax_profile_posts(), 3);
        });
    }

    /// The one road to `POST /api/tax-profiles`, outside tests, in the
    /// app's Rust (every `src` under `crates/` and `nsh/src`):
    /// - the name `add_account_tin` appears only where it is defined
    ///   (`Submitter::add_account_tin` in bir-submit's tins.rs) and once in
    ///   `activate_tin` (the "Activate this TIN to send" verb), as a call or
    ///   as a path taken as a value alike;
    /// - a `bearer_send` with `POST` to `PLAIN_PATH` or `/api/tax-profiles`
    ///   appears only inside `add_account_tin`.
    ///
    /// So no other path (a profile made or changed, a sign-in, a sync) can
    /// add a TIN to the account. Comments, strings and every `#[cfg(test)]`
    /// item are left out, wherever in the file they are.
    #[test]
    fn only_activate_tin_calls_add_account_tin() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut dirs = vec![root.join("nsh/src")];
        for entry in std::fs::read_dir(root.join("crates")).expect("crates/") {
            dirs.push(entry.expect("a crate").path().join("src"));
        }
        let mut found = Vec::new();
        while let Some(dir) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                if path.extension().is_none_or(|extension| extension != "rs") {
                    continue;
                }
                let file = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .display()
                    .to_string()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&path).expect("a source file");
                found.extend(account_add_roads(&file, &text));
            }
        }
        found.sort();
        assert_eq!(
            found,
            [
                (
                    "crates/bir-app-api/src/tax_profile_module.rs".to_owned(),
                    "activate_tin".to_owned(),
                    "add_account_tin"
                ),
                (
                    "crates/bir-submit/src/tins.rs".to_owned(),
                    "add_account_tin".to_owned(),
                    "POST"
                ),
            ],
            "every road to POST /api/tax-profiles outside tests"
        );
    }

    /// The scanner behind [`only_activate_tin_calls_add_account_tin`] finds
    /// what a first-match cut or a `name(` search would miss.
    #[test]
    fn the_road_scanner_sees_past_test_modules_comments_and_paths() {
        let source = "fn a() {}\r\n\
            #[cfg(test)]\r\n\
            mod early { fn t() { s.add_account_tin(1); } }\r\n\
            // s.add_account_tin(2) in a comment\r\n\
            /* and add_account_tin in a /* nested */ block */\r\n\
            fn b() { let s = \"add_account_tin(\"; let c = '{'; }\r\n\
            pub async fn late<'a>(x: &'a X) {\r\n\
                let f = Submitter::add_account_tin;\r\n\
                x.bearer_send(Method::POST, PLAIN_PATH, None);\r\n\
            }\r\n\
            impl X { pub(crate) fn m(&self) { self.add_account_tin(3) } }\r\n\
            fn c() { let r = br#\"add_account_tin(\"#; let n = ('\\n', '\\n'\"x\", \"add_account_tin\"); let e = '\\''; }\r\n\
            #[cfg(test)]\r\n\
            mod t;\r\n\
            fn d() { self.add_account_tin(5); }\r\n\
            #[cfg(test)]\r\n\
            #[allow(dead_code)]\r\n\
            fn helper() { add_account_tin(4); }\r\n";
        let mut found = account_add_roads("f.rs", source);
        found.sort();
        assert_eq!(
            found,
            [
                ("f.rs".to_owned(), "d".to_owned(), "add_account_tin"),
                ("f.rs".to_owned(), "late".to_owned(), "POST"),
                ("f.rs".to_owned(), "late".to_owned(), "add_account_tin"),
                ("f.rs".to_owned(), "m".to_owned(), "add_account_tin"),
            ]
        );
    }

    /// Every use of the name `add_account_tin` (its definition aside) and
    /// every `bearer_send(.. POST .. /api/tax-profiles ..)` in `text`
    /// outside `#[cfg(test)]` items, comments and strings, each with the
    /// innermost `fn` around it.
    fn account_add_roads(file: &str, text: &str) -> Vec<(String, String, &'static str)> {
        let text = text.replace("\r\n", "\n");
        let blank = blank_out(&text);
        let blank = drop_test_items(&blank);
        let functions = functions_in(&blank);
        let around = |at: usize| {
            functions
                .iter()
                .filter(|(_, open, close)| *open < at && at < *close)
                .max_by_key(|(_, open, _)| *open)
                .map_or_else(|| "(none)".to_owned(), |(name, _, _)| name.clone())
        };
        let mut found = Vec::new();
        for at in words(&blank, "add_account_tin") {
            if blank[..at].trim_end().ends_with("fn") {
                continue;
            }
            found.push((file.to_owned(), around(at), "add_account_tin"));
        }
        for at in words(&blank, "bearer_send") {
            let Some(open) = blank[at..].find('(').map(|i| at + i) else {
                continue;
            };
            let close = matching(&blank, open).unwrap_or(blank.len());
            let arguments = &text[open..close];
            if arguments.contains("POST")
                && (arguments.contains("PLAIN_PATH") || arguments.contains("/api/tax-profiles"))
            {
                found.push((file.to_owned(), around(at), "POST"));
            }
        }
        found
    }

    /// `text` with comments and the insides of string and char literals as
    /// spaces (same byte offsets), so braces and names there count for
    /// nothing.
    fn blank_out(text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = bytes.to_vec();
        let mut i = 0;
        let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
            for b in &mut out[from..to] {
                if *b != b'\n' {
                    *b = b' ';
                }
            }
        };
        while i < bytes.len() {
            match bytes[i] {
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    let end = text[i..].find('\n').map_or(bytes.len(), |n| i + n);
                    blank(&mut out, i, end);
                    i = end;
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    let (mut depth, mut j) = (1, i + 2);
                    while j < bytes.len() && depth > 0 {
                        if bytes[j..].starts_with(b"/*") {
                            depth += 1;
                            j += 2;
                        } else if bytes[j..].starts_with(b"*/") {
                            depth -= 1;
                            j += 2;
                        } else {
                            j += 1;
                        }
                    }
                    blank(&mut out, i, j);
                    i = j;
                }
                // A raw string: `r"…"`, `r#"…"#`, and its byte and C forms
                // (`br…`, `cr…`), the `r` starting a token.
                b'r' if (bytes.get(i + 1) == Some(&b'"') || bytes.get(i + 1) == Some(&b'#'))
                    && {
                        let ident =
                            |at: usize| bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_';
                        i == 0
                            || !ident(i - 1)
                            || (matches!(bytes[i - 1], b'b' | b'c') && (i == 1 || !ident(i - 2)))
                    } =>
                {
                    let hashes = bytes[i + 1..].iter().take_while(|b| **b == b'#').count();
                    if bytes.get(i + 1 + hashes) != Some(&b'"') {
                        i += 1;
                        continue;
                    }
                    let closing = format!("\"{}", "#".repeat(hashes));
                    let body = i + 2 + hashes;
                    let end = text[body..]
                        .find(&closing)
                        .map_or(bytes.len(), |n| body + n + closing.len());
                    blank(&mut out, body, end.saturating_sub(closing.len()).max(body));
                    i = end;
                }
                b'"' => {
                    let mut j = i + 1;
                    while j < bytes.len() && bytes[j] != b'"' {
                        j += if bytes[j] == b'\\' { 2 } else { 1 };
                    }
                    blank(&mut out, i + 1, j.min(bytes.len()));
                    i = j + 1;
                }
                b'\'' => {
                    // A char literal ('x', '\n', '\u{..}') or a lifetime ('a).
                    let rest = &text[i + 1..];
                    // Its body: one escape (`\\n`, `\\'`, `\\x41`, `\\u{1F600}`) or
                    // one char, and then the closing quote.
                    let body = match rest.strip_prefix('\\') {
                        Some(escape) if escape.starts_with("u{") => {
                            escape.find('}').map(|close| 1 + close + 1)
                        }
                        Some(escape) if escape.starts_with('x') => Some(1 + 3),
                        Some(escape) => escape.chars().next().map(|c| 1 + c.len_utf8()),
                        None => rest.chars().next().map(char::len_utf8),
                    };
                    let literal = body.filter(|len| {
                        rest.get(*len..)
                            .is_some_and(|after| after.starts_with('\''))
                    });
                    match literal {
                        Some(len) => {
                            blank(&mut out, i + 1, i + 1 + len);
                            i += len + 2;
                        }
                        None => i += 1,
                    }
                }
                _ => i += 1,
            }
        }
        String::from_utf8(out).expect("only ASCII bytes were replaced")
    }

    /// `blank` with every `#[cfg(test)]` item (a module, a fn, a `mod x;`)
    /// turned to spaces, wherever it stands in the file.
    fn drop_test_items(blank: &str) -> String {
        let mut out = blank.to_owned();
        let mut from = 0;
        while let Some(found) = out[from..].find("#[cfg(test)]") {
            let start = from + found;
            let mut at = start + "#[cfg(test)]".len();
            // Further attributes on the same item.
            loop {
                let rest = out[at..].trim_start();
                at = out.len() - rest.len();
                if rest.starts_with("#[") {
                    at = matching(&out, at + 1).map_or(out.len(), |close| close + 1);
                } else {
                    break;
                }
            }
            let semicolon = out[at..].find(';').map(|n| at + n);
            let brace = out[at..].find('{').map(|n| at + n);
            let end = match (brace, semicolon) {
                (Some(open), Some(semi)) if semi < open => semi + 1,
                (Some(open), _) => matching(&out, open).map_or(out.len(), |close| close + 1),
                (None, Some(semi)) => semi + 1,
                (None, None) => out.len(),
            };
            let spaces: String = out[start..end]
                .chars()
                .map(|c| if c == '\n' { '\n' } else { ' ' })
                .collect();
            out.replace_range(start..end, &spaces);
            from = end;
        }
        out
    }

    /// Every `fn` with a body in `blank`: its name and its braces' offsets.
    fn functions_in(blank: &str) -> Vec<(String, usize, usize)> {
        let mut functions = Vec::new();
        for at in words(blank, "fn") {
            let name: String = blank[at + 2..]
                .trim_start()
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            let after = at + 2;
            let open = blank[after..].find('{').map(|n| after + n);
            let semi = blank[after..].find(';').map(|n| after + n);
            if let Some(open) = open.filter(|open| semi.is_none_or(|semi| *open < semi))
                && let Some(close) = matching(blank, open)
            {
                functions.push((name, open, close));
            }
        }
        functions
    }

    /// The offsets of `word` in `blank` as a whole identifier.
    fn words<'a>(blank: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
        let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        blank
            .match_indices(word)
            .map(|(at, _)| at)
            .filter(move |at| {
                !ident(blank[..*at].chars().next_back())
                    && !ident(blank[at + word.len()..].chars().next())
            })
    }

    /// The offset of the bracket closing the one at `open` (`{`, `(` or
    /// `[`), counting only its own kind.
    fn matching(blank: &str, open: usize) -> Option<usize> {
        let bytes = blank.as_bytes();
        let (opening, closing) = match bytes[open] {
            b'{' => (b'{', b'}'),
            b'(' => (b'(', b')'),
            b'[' => (b'[', b']'),
            _ => return None,
        };
        let mut depth = 0;
        for (i, b) in bytes.iter().enumerate().skip(open) {
            if *b == opening {
                depth += 1;
            } else if *b == closing {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Making, changing and listing tax profiles never adds a TIN to the
    /// account: only "Activate this TIN to send" posts.
    #[test]
    fn profile_creation_never_posts() {
        against_the_account(|mock| {
            setup();
            add_branch("00001");
            add_juana();
            ok(
                update_taxpayer,
                vec![json!(ROOT), json!({"name": "Example Trading Corporation"})],
            );
            ok(
                update_branch,
                vec![
                    json!("12345678800001"),
                    json!({"name": "Example Store Two"}),
                ],
            );
            ok(list_taxpayers_fn, vec![]);
            call_verb("account_tins", vec![json!(0)]).unwrap();
            assert_eq!(mock.tax_profile_posts(), 0, "no profile change posts");
            assert!(mock.tins_of("filer@example.test").is_empty());
            call_verb("activate_tin", vec![json!(HEAD)]).unwrap();
            assert_eq!(mock.tax_profile_posts(), 1);
        });
    }

    fn list_taxpayers_fn(_: &HostArguments) -> HostResult {
        list_taxpayers()
    }

    /// Both verbs need the filing data open (signed in), and then ask
    /// nothing of the account.
    #[test]
    fn the_account_verbs_need_the_data_open() {
        setup();
        against_the_account(|mock| {
            crate::account_gate::force_locked(|| {
                for (verb, value) in [("account_tins", json!(0)), ("activate_tin", json!(HEAD))] {
                    assert_eq!(
                        call_verb(verb, vec![value]),
                        Err(crate::account_gate::LOCKED_MESSAGE.into()),
                        "{verb}"
                    );
                }
            });
            assert_eq!(mock.tax_profile_posts(), 0);
        });
    }

    /// Without a client installed, a test reaches no account server.
    #[test]
    fn the_account_verbs_reach_no_server_in_tests() {
        setup();
        assert_eq!(
            call_verb("account_tins", vec![json!(0)]),
            Err("The Buwiz account is not available in tests.".into())
        );
    }

    /// F4: the write itself is refused after the room changed — B's completed consent never lands
    /// in A's room, even though the thread still holds A's Arc.
    #[test]
    fn oauth_grant_is_not_written_after_the_room_changed() {
        let _turn = crate::open_room::room_test_turn();
        let folder = tempfile::tempdir().unwrap();
        bir_core::platform::set_device_dir_override(Some(folder.path().to_path_buf()));
        let layout = bir_core::rooms::Layout::at(folder.path().to_path_buf());
        std::fs::create_dir_all(layout.device_dir()).unwrap();
        let (handle, first) = crate::session::open_legacy_in_place_for_test(
            &layout,
            "user-a".into(),
            "https://api.buwiz.test".into(),
        )
        .unwrap();
        // The room changes (a sign-out and a sign-in) while the consent is still open in the browser.
        crate::session::close(crate::session::Reason::SignOut);
        let (_other, _second) = crate::session::open_legacy_in_place_for_test(
            &layout,
            "user-b".into(),
            "https://api.buwiz.test".into(),
        )
        .unwrap();
        let written = persist_oauth_grant_for(
            &handle,
            HEAD,
            "receipts@example.com",
            "access-x",
            "refresh-x",
            Some(first),
        );
        assert!(
            matches!(&written, Err(error) if error.message().contains("the room changed")),
            "the grant write was not refused after the room changed"
        );
        {
            let guard = bir_core::lock_timing::lock(&handle).unwrap();
            assert!(
                guard
                    .inbox_oauth_tokens("receipts@example.com")
                    .unwrap()
                    .is_none(),
                "B's grant was written into A's room after the switch"
            );
        }
        crate::session::close(crate::session::Reason::SignOut);
        bir_core::platform::set_device_dir_override(None);
    }

    /// §A.8: an OAuth (or verify, or check) thread's result is dropped after the room changed — never
    /// written into the new room — and passes through while the room is still the thread's own.
    #[test]
    fn oauth_result_dropped_after_the_room_changed() {
        // The room slot and its generation are process-wide: without the turn any other test's
        // open or close moves the generation under this one, and the pass-through arm drops a
        // result that was never stale — the flake the suites saw.
        let _turn = crate::open_room::room_test_turn();
        let folder = tempfile::tempdir().unwrap();
        bir_core::platform::set_device_dir_override(Some(folder.path().to_path_buf()));
        let layout = bir_core::rooms::Layout::at(folder.path().to_path_buf());
        std::fs::create_dir_all(layout.device_dir()).unwrap();
        // The legacy-in-place open populates the slot without the claim guard (legacy mode); every
        // open is a new generation, as a sign-out and sign-in makes one.
        let (_handle, first) = crate::session::open_legacy_in_place_for_test(
            &layout,
            "user-a".into(),
            "https://api.buwiz.test".into(),
        )
        .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            // The room is still the thread's own: the result passes through.
            let (sender, receiver) = futures::channel::oneshot::channel::<HostResult>();
            sender.send(Ok(HostValue::Null)).unwrap();
            let passed = finish_room_thread(Some(first), receiver, "x").await;
            assert!(
                passed.is_ok(),
                "the result of the room's own thread dropped"
            );
            // The room changed (a close and an open: a new generation): the result is dropped.
            crate::session::close(crate::session::Reason::SignOut);
            let (_handle2, second) = crate::session::open_legacy_in_place_for_test(
                &layout,
                "user-b".into(),
                "https://api.buwiz.test".into(),
            )
            .unwrap();
            assert_ne!(first, second);
            let (sender, receiver) = futures::channel::oneshot::channel::<HostResult>();
            sender.send(Ok(HostValue::Null)).unwrap();
            let dropped = finish_room_thread(Some(first), receiver, "x").await;
            assert!(
                dropped.is_err(),
                "the result was written after the room changed"
            );
        });
        crate::session::close(crate::session::Reason::SignOut);
        bir_core::platform::set_device_dir_override(None);
    }
}
