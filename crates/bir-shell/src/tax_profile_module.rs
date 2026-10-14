//! Tax-profile host functions: taxpayers, branches, tax years, yearly forms,
//! starter packs and form templates. Every rule (TIN shape, tax-year bound,
//! one template per form, D8 setup-only snapshots, the one-transaction TIN
//! move) lives in bir-core; this file only converts JSON in and out.

use std::collections::{BTreeMap, BTreeSet};

use bir_core::db::Database;
use bir_core::forms::registry::canonical_form_code;
use bir_core::profile::TaxpayerType;
use bir_core::tax_profile::{
    BranchFacts, BranchField, BranchUpdate, FieldSetting, FormTemplate, NewBranch, NewTaxpayer,
    ResolvedBranch, StarterPack, TaxProfileError, TaxpayerUpdate, TaxpayerWithBranches,
    TemplateField, format_tin_root, tax_year_bounds,
};
use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};
use serde_json::Value;

use crate::bir_module::{to_json, with_database};

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
export function create_taxpayer(input: NewTaxpayerInput): Taxpayer;
export function update_taxpayer(tin_root: string, update: TaxpayerUpdateInput): Taxpayer;
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
}
