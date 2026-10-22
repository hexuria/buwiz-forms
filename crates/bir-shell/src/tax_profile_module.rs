//! Tax-profile host functions: taxpayers, branches, tax years, yearly forms,
//! starter packs and form templates. Every rule (TIN shape, tax-year bound,
//! one template per form, D8 setup-only snapshots, the one-transaction TIN
//! move) lives in bir-core; this file only converts JSON in and out.

use std::collections::{BTreeMap, BTreeSet};

use bir_core::db::Database;
use bir_core::forms::registry::canonical_form_code;
use bir_core::profile::{EmailAuthMethod, TaxpayerProfile, TaxpayerType};
use bir_core::tax_profile::{
    BranchFacts, BranchField, BranchUpdate, FieldSetting, FormTemplate, NewBranch, NewTaxpayer,
    ResolvedBranch, StarterPack, TaxProfileError, TaxpayerUpdate, TaxpayerWithBranches,
    TemplateField, format_tin_root, tax_year_bounds,
};
use chrono::Datelike as _;
use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};
use serde_json::Value;

use crate::bir_module::{database_handle, to_json, with_database};

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
export interface StarterPackInfo { id: string; label: string; forms: string[]; }
export interface FormTemplateSummary {
    form_code: string;
    updated_at: string;
    answers_kept: number;
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
export function starter_packs(tin: string): StarterPackInfo[];
export function list_form_templates(tin: string): FormTemplateSummary[];
export function create_template_from_latest(tin: string, form_code: string): FormTemplateSummary;
export function create_template_from_return(draft_id: number): FormTemplateSummary;
export function delete_form_template(tin: string, form_code: string): boolean;
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
        .function("starter_packs", starter_packs)
        .function("list_form_templates", list_form_templates)
        .function("create_template_from_latest", create_template_from_latest)
        .function("create_template_from_return", create_template_from_return)
        .function("delete_form_template", delete_form_template)
        .function("list_rdos", |_| list_rdos())
        .function("list_zip_codes", |_| list_zip_codes())
        .function("inbox_state", inbox_state)
        .function("set_inbox_app_password", set_inbox_app_password)
        .function("set_inbox_tracking", set_inbox_tracking)
        .function("disconnect_inbox_oauth", disconnect_inbox_oauth)
        .async_function("connect_inbox_oauth", connect_inbox_oauth)
        .async_function("test_inbox_connection", test_inbox_connection)
        .async_function("check_inbox_now", check_inbox_now)
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

fn template_record(template: &FormTemplate) -> HostValue {
    let kept = template
        .fields
        .values()
        .filter(|field| matches!(field, TemplateField::Kept { .. }))
        .count();
    let marked = template.fields.len() - kept;
    HostObject::new()
        .field("form_code", template.form_code.as_str())
        .field("updated_at", template.updated_at.as_str())
        .field("answers_kept", kept as f64)
        .field("marked", marked as f64)
        .field("source", template.source.to_db())
        .into()
}

fn load_taxpayer(db: &Database, tin_root: &str) -> Result<TaxpayerWithBranches, HostError> {
    db.get_taxpayer(tin_root)
        .map_err(host_error)?
        .ok_or_else(|| HostError::new(format!("No taxpayer with TIN {tin_root}")))
}

// ---------- taxpayers ----------

fn list_taxpayers() -> HostResult {
    with_database(|db| {
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
    let new = NewTaxpayer {
        tin_root: optional_string(&input, "tin_root")?.unwrap_or_default(),
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
    let update = TaxpayerUpdate {
        name: optional_string(&input, "name")?,
        taxpayer_type: taxpayer_type(&input, "taxpayer_type")?,
        birth_date: None,
    };
    with_database(|db| {
        let updated = db.update_taxpayer(tin_root, update).map_err(host_error)?;
        Ok(taxpayer_record(&load_taxpayer(db, &updated.tin_root)?))
    })
}

fn move_taxpayer_tin(arguments: &HostArguments) -> HostResult {
    let old_root = arguments.string(0)?;
    let new_root = arguments.string(1)?;
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

fn starter_packs(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| {
        let branch = db
            .get_branch(tin)
            .map_err(host_error)?
            .ok_or_else(|| HostError::new(format!("No branch with TIN {tin}")))?;
        let taxpayer = load_taxpayer(db, &branch.tin_root)?.taxpayer;
        Ok(HostValue::Array(
            StarterPack::ALL
                .into_iter()
                .map(|pack| {
                    HostObject::new()
                        .field("id", field_string(serde_json::to_value(pack).ok()))
                        .field("label", pack.label())
                        .field(
                            "forms",
                            strings(
                                pack.form_codes(&taxpayer.taxpayer_type)
                                    .iter()
                                    .map(|code| code.to_string()),
                            ),
                        )
                        .into()
                })
                .collect(),
        ))
    })
}

// ---------- templates ----------

fn list_form_templates(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| {
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

/// Persist a grant returned by `start_oauth_flow` — split out so tests drive
/// it with canned tokens (the browser leg cannot run headlessly).
fn persist_oauth_grant(
    db: &std::sync::Arc<std::sync::Mutex<Database>>,
    tin: &str,
    email: &str,
    access_token: &str,
    refresh_token: &str,
) -> HostResult {
    let guard = lock_db(db)?;
    guard
        .persist_google_oauth_for_inbox(email, access_token, refresh_token, Some(tin))
        .map_err(|error| HostError::new(error.to_string()))?;
    inbox_state_value(&guard, tin)
}

fn connect_inbox_oauth_run(
    tin: &str,
    db: &std::sync::Arc<std::sync::Mutex<Database>>,
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
    persist_oauth_grant(db, tin, &email, &access_token, &refresh_token)
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
) -> HostResult {
    let guard = lock_db(db)?;
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
) -> HostResult {
    let profile = {
        let guard = lock_db(db)?;
        load_inbox_profile(&guard, tin)?
    };
    // The IMAP probe runs with the lock released — it can take many seconds.
    let result = run_connection_test(&profile)?;
    apply_connection_result(db, tin, result)
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

fn connect_inbox_oauth(
    arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let tin = arguments.string(0)?.to_owned();
    let db = database_handle()?;
    let (sender, receiver) = futures::channel::oneshot::channel::<HostResult>();
    std::thread::Builder::new()
        .name("inbox-oauth".into())
        .spawn(move || {
            let _ = sender.send(connect_inbox_oauth_run(&tin, &db));
        })
        .map_err(|error| HostError::new(error.to_string()))?;
    Ok(async move {
        receiver
            .await
            .unwrap_or_else(|_| Err(HostError::new("OAuth thread failed")))
    })
}

fn test_inbox_connection(
    arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let tin = arguments.string(0)?.to_owned();
    let db = database_handle()?;
    let (sender, receiver) = futures::channel::oneshot::channel::<HostResult>();
    std::thread::Builder::new()
        .name("inbox-verify".into())
        .spawn(move || {
            let _ = sender.send(test_inbox_connection_run(&tin, &db));
        })
        .map_err(|error| HostError::new(error.to_string()))?;
    Ok(async move {
        receiver
            .await
            .unwrap_or_else(|_| Err(HostError::new("verify thread failed")))
    })
}

fn check_inbox_now(
    arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let tin = arguments.string(0)?.to_owned();
    let db = database_handle()?;
    let (sender, receiver) = futures::channel::oneshot::channel::<HostResult>();
    std::thread::Builder::new()
        .name("inbox-check".into())
        .spawn(move || {
            let _ = sender.send(check_inbox_now_run(&tin, &db));
        })
        .map_err(|error| HostError::new(error.to_string()))?;
    Ok(async move {
        receiver
            .await
            .unwrap_or_else(|_| Err(HostError::new("inbox check thread failed")))
    })
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
        let moved = ok(move_taxpayer_tin, vec![json!(ROOT), json!("987-654-320")]);
        assert_eq!(moved["tin_root"], "987654320");
        assert_eq!(moved["branches"][1]["tin"], "98765432000001");
        assert_eq!(ok(get_taxpayer, vec![json!(ROOT)]), Value::Null);

        ok(
            create_taxpayer,
            vec![
                json!({"tin_root": ROOT, "name": "Sample Holdings Inc", "taxpayer_type": "Corporation",
                "head_office": {"rdo_code": "1", "registered_address": "a", "zip_code": "1", "phone": "1", "email": "a@example.com"}}),
            ],
        );
        let refused = call(move_taxpayer_tin, vec![json!("987654320"), json!(ROOT)]);
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
        ok(move_taxpayer_tin, vec![json!(ROOT), json!("987654320")]);
        let admin_root = with_database(|db| Ok(db.get_setting("admin_tin_root").unwrap())).unwrap();
        assert_eq!(admin_root.as_deref(), Some("987654320"));
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
    fn starter_packs_follow_the_taxpayer_type() {
        setup();
        let packs = ok(starter_packs, vec![json!(HEAD)]);
        let packs = packs.as_array().unwrap();
        assert_eq!(packs.len(), 4);
        assert_eq!(packs[1]["id"], "VatBusiness");
        assert_eq!(packs[1]["label"], "VAT business");
        assert_eq!(packs[1]["forms"], json!(["2550Q", "1702Q", "1702RT"]));
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
            "Open and save a 2551Q return first. The template keeps its setup answers."
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
        let success = to_json(&apply_connection_result(&db, HEAD, Ok(None)).unwrap());
        assert_eq!(success["ok"], true);
        assert_eq!(success["state"]["tracking_enabled"], true);

        let failure = to_json(
            &apply_connection_result(&db, HEAD, Err(anyhow::anyhow!("IMAP login refused")))
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
            test_inbox_connection_run(HEAD, &db).unwrap_err().message(),
            refused
        );
        assert_eq!(ok(inbox_state, vec![json!(HEAD)])["tracking_enabled"], true);

        TEST_INBOX_CONNECTION.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(|profile| {
                assert_eq!(profile.inbox_email(), "office@example.com");
                Err(anyhow::anyhow!("IMAP login refused"))
            }));
        });
        let verdict = to_json(&test_inbox_connection_run(HEAD, &db).unwrap());
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
            connect_inbox_oauth_run(HEAD, &db).unwrap_err().message(),
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
        let state = to_json(&connect_inbox_oauth_run(HEAD, &db).unwrap());
        assert_eq!(state["oauth_connected"], true);
        assert_eq!(state["oauth_inbox_email"], "receipts@example.com");
    }
}
