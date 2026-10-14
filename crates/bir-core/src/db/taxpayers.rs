//! Taxpayers, branches, yearly forms, form templates and prefill resolution
//! (tax-profile redesign Phase 1). Domain types and rules live in
//! [`crate::tax_profile`].
//!
//! A branch is a `profiles` row. Its own facts stay in `profiles.data_json`
//! (so legacy readers keep working); inherited fields hold a cached copy of
//! the head office's value that is refreshed whenever the head office or the
//! taxpayer changes. Readers of this module always resolve inheritance from
//! the head office's current value instead of trusting the cache.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use super::{Database, DbError};
use crate::forms::FilingStatus;
use crate::forms::forms_set::{FormSetEntry, FormSetSource, PerYearFormsSet};
use crate::forms::registry::canonical_form_code;
use crate::profile::{TaxpayerProfile, TaxpayerType};
use crate::tax_profile::templates::{form_order_key, value_at_path};
use crate::tax_profile::{
    AutoSource, BranchFacts, BranchField, BranchName, BranchSettings, BranchUpdate, FieldKind,
    FieldSetting, FormTemplate, GroupingReview, IdentitySource, NewBranch, NewTaxpayer,
    PrefillPlan, ResolvedBranch, ResolvedPeriod, StarterPack, TaxProfileError, Taxpayer,
    TaxpayerData, TaxpayerUpdate, TaxpayerWithBranches, TemplatableKind, TemplateChoice,
    TemplateField, TemplateFieldError, TemplateSource, default_branch_name, field_kinds_for,
    is_head_office_code, normalize_branch_code, parse_tin_root, resolve_facts,
    validate_template_fields,
};

type Result<T> = std::result::Result<T, TaxProfileError>;

/// Reason recorded on a yearly-form entry the user removed from the list.
const REMOVED_FROM_YEARLY_LIST: &str = "Removed from the yearly form list";

/// D16 conflict policy: the target branch already has a template for the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateConflictPolicy {
    Replace,
    Skip,
}

/// D16 missing-form policy: the target branch does not file the form that year.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingFormPolicy {
    AddForm,
    Skip,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BulkCopyReport {
    pub copied: Vec<String>,
    pub replaced: Vec<String>,
    pub skipped_existing: Vec<String>,
    pub skipped_missing_form: Vec<String>,
    /// Forms added to the target's yearly list under `AddForm`.
    pub forms_added: Vec<String>,
}

// =========================================================================
// Grouping (v20 migration and every-startup repair)
// =========================================================================

struct GroupedProfile {
    id: i64,
    tin: String,
    data: Value,
}

fn str_field<'a>(data: &'a Value, key: &str) -> &'a str {
    data.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn comparable_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Insert one `taxpayers` row per 9-digit root that has profiles but no row,
/// and drop rows whose root has no profile left. Existing rows never change,
/// so the D11 review is recorded exactly once, when the root is first grouped.
pub(crate) fn sync_taxpayer_rows(conn: &Connection) -> std::result::Result<(), DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, tin, data_json FROM profiles
         WHERE length(tin) >= 9
           AND substr(tin, 1, 9) NOT GLOB '*[^0-9]*'
           AND substr(tin, 1, 9) NOT IN (SELECT tin_root FROM taxpayers)
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut by_root: BTreeMap<String, Vec<GroupedProfile>> = BTreeMap::new();
    for row in rows {
        let (id, tin, json) = row?;
        // Lenient on purpose: a row written by a newer build must still be
        // grouped, and only top-level strings are read.
        let data = serde_json::from_str(&json).unwrap_or(Value::Null);
        by_root
            .entry(tin[..9].to_string())
            .or_default()
            .push(GroupedProfile { id, tin, data });
    }
    drop(stmt);

    for (root, profiles) in by_root {
        let zero_branch: Vec<&GroupedProfile> = profiles
            .iter()
            .filter(|profile| is_head_office_code(&profile.tin[9..]))
            .collect();
        let source = zero_branch.first().copied().unwrap_or(&profiles[0]);

        let distinct_names: BTreeSet<String> = profiles
            .iter()
            .map(|profile| comparable_name(str_field(&profile.data, "full_name")))
            .collect();
        let review = GroupingReview {
            differing_names: if distinct_names.len() > 1 {
                profiles
                    .iter()
                    .map(|profile| BranchName {
                        tin: profile.tin.clone(),
                        name: str_field(&profile.data, "full_name").to_string(),
                    })
                    .collect()
            } else {
                Vec::new()
            },
            missing_head_office: zero_branch.is_empty(),
            head_office_candidates: if zero_branch.len() > 1 {
                zero_branch
                    .iter()
                    .map(|profile| profile.tin.clone())
                    .collect()
            } else {
                Vec::new()
            },
            resolved_at: None,
        };
        let data = TaxpayerData {
            birth_date: source
                .data
                .get("birth_date")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .flatten(),
            grouping_review: review.needs_review().then_some(review),
        };
        let taxpayer_type = match str_field(&source.data, "taxpayer_type") {
            "" => "Individual",
            stored => stored,
        };
        conn.execute(
            "INSERT INTO taxpayers (tin_root, name, taxpayer_type, data_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                root,
                str_field(&source.data, "full_name"),
                taxpayer_type,
                serde_json::to_string(&data)?
            ],
        )?;
        tracing::info!(
            tin_root = %root,
            branches = profiles.len(),
            source_profile_id = source.id,
            "Grouped existing profiles under one taxpayer"
        );
    }

    conn.execute(
        "DELETE FROM taxpayers
         WHERE tin_root NOT IN (SELECT substr(tin, 1, 9) FROM profiles)",
        [],
    )?;
    Ok(())
}

// =========================================================================
// Row helpers
// =========================================================================

struct BranchRow {
    id: i64,
    tin: String,
    data: Value,
    settings: BranchSettings,
}

impl BranchRow {
    fn code(&self) -> String {
        normalize_branch_code(&self.tin[9..]).unwrap_or_else(|_| self.tin[9..].to_string())
    }

    fn own_facts(&self) -> BranchFacts {
        let mut facts = BranchFacts::default();
        for field in BranchField::ALL {
            facts.set(field, str_field(&self.data, profile_key(field)).to_string());
        }
        facts
    }
}

/// The `TaxpayerProfile` JSON key each branch fact is stored under.
fn profile_key(field: BranchField) -> &'static str {
    match field {
        BranchField::RdoCode => "rdo_code",
        BranchField::RegisteredAddress => "registered_address",
        BranchField::ZipCode => "zip_code",
        BranchField::Phone => "phone",
        BranchField::Email => "email",
    }
}

fn parse_branch_row(
    id: i64,
    tin: String,
    data_json: String,
    branch_json: Option<String>,
) -> Result<BranchRow> {
    let data = serde_json::from_str(&data_json).unwrap_or(Value::Null);
    let settings = branch_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?
        .unwrap_or_default();
    Ok(BranchRow {
        id,
        tin,
        data,
        settings,
    })
}

fn load_branch_rows(conn: &Connection, root: &str) -> Result<Vec<BranchRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, tin, data_json, branch_json FROM profiles
         WHERE length(tin) >= 9 AND substr(tin, 1, 9) = ?1
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([root], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?;
    let mut branches = Vec::new();
    for row in rows {
        let (id, tin, data_json, branch_json) = row?;
        branches.push(parse_branch_row(id, tin, data_json, branch_json)?);
    }
    Ok(branches)
}

/// Head office = the lowest-id zero-branch profile (D11 records duplicates).
fn head_office(rows: &[BranchRow]) -> Option<&BranchRow> {
    rows.iter().find(|row| is_head_office_code(&row.tin[9..]))
}

fn resolve_branch(row: &BranchRow, head: Option<&BranchRow>) -> ResolvedBranch {
    let is_head_office = head.is_some_and(|head| head.id == row.id);
    let head_facts = head.filter(|_| !is_head_office).map(BranchRow::own_facts);
    let code = row.code();
    ResolvedBranch {
        tin: row.tin.clone(),
        tin_root: row.tin[..9].to_string(),
        name: row
            .settings
            .name
            .clone()
            .unwrap_or_else(|| default_branch_name(&code)),
        branch_code: code,
        is_head_office,
        facts: resolve_facts(
            &row.own_facts(),
            &row.settings.inherited,
            head_facts.as_ref(),
        ),
    }
}

fn resolve_all(rows: &[BranchRow]) -> Vec<ResolvedBranch> {
    let head = head_office(rows);
    let mut branches: Vec<ResolvedBranch> =
        rows.iter().map(|row| resolve_branch(row, head)).collect();
    branches.sort_by(|a, b| {
        b.is_head_office
            .cmp(&a.is_head_office)
            .then_with(|| a.branch_code.cmp(&b.branch_code))
            .then_with(|| a.tin.cmp(&b.tin))
    });
    branches
}

fn parse_taxpayer_type(raw: &str) -> TaxpayerType {
    serde_json::from_value(Value::String(raw.to_string())).unwrap_or_else(|_| {
        tracing::warn!(
            taxpayer_type = raw,
            "Unknown taxpayer type; using the default"
        );
        TaxpayerType::default()
    })
}

fn taxpayer_type_str(taxpayer_type: &TaxpayerType) -> Result<String> {
    match serde_json::to_value(taxpayer_type)? {
        Value::String(value) => Ok(value),
        other => Err(TaxProfileError::Invalid(format!(
            "Unexpected taxpayer type encoding {other}"
        ))),
    }
}

fn load_taxpayer(conn: &Connection, root: &str) -> Result<Option<Taxpayer>> {
    conn.query_row(
        "SELECT tin_root, name, taxpayer_type, data_json, created_at, updated_at
         FROM taxpayers WHERE tin_root = ?1",
        [root],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        },
    )
    .optional()?
    .map(
        |(tin_root, name, taxpayer_type, data_json, created_at, updated_at)| {
            let data: TaxpayerData = serde_json::from_str(&data_json)?;
            Ok(Taxpayer {
                tin_root,
                name,
                taxpayer_type: parse_taxpayer_type(&taxpayer_type),
                birth_date: data.birth_date,
                grouping_review: data.grouping_review,
                created_at,
                updated_at,
            })
        },
    )
    .transpose()
}

fn load_taxpayer_data(conn: &Connection, root: &str) -> Result<TaxpayerData> {
    let json: String = conn.query_row(
        "SELECT data_json FROM taxpayers WHERE tin_root = ?1",
        [root],
        |row| row.get(0),
    )?;
    Ok(serde_json::from_str(&json)?)
}

/// Set top-level keys of a stored profile without touching any other key, so
/// profile versions and settings this module does not own are preserved.
fn write_profile_keys(conn: &Connection, tin: &str, updates: &[(&str, Value)]) -> Result<()> {
    let json: String = conn.query_row(
        "SELECT data_json FROM profiles WHERE tin = ?1",
        [tin],
        |row| row.get(0),
    )?;
    let mut data: Value = serde_json::from_str(&json)?;
    let Some(object) = data.as_object_mut() else {
        return Err(TaxProfileError::Invalid(format!(
            "Profile {tin} is not a JSON object"
        )));
    };
    let mut changed = false;
    for (key, value) in updates {
        if object.get(*key) != Some(value) {
            object.insert((*key).to_string(), value.clone());
            changed = true;
        }
    }
    if changed {
        conn.execute(
            "UPDATE profiles SET data_json = ?1 WHERE tin = ?2",
            params![serde_json::to_string(&data)?, tin],
        )?;
    }
    Ok(())
}

/// Refresh the cached value of every inherited field under `root`.
fn refresh_inherited_caches(conn: &Connection, root: &str) -> Result<()> {
    let rows = load_branch_rows(conn, root)?;
    let Some(head) = head_office(&rows) else {
        return Ok(());
    };
    let head_facts = head.own_facts();
    for row in rows.iter().filter(|row| row.id != head.id) {
        let updates: Vec<(&str, Value)> = row
            .settings
            .inherited
            .iter()
            .map(|field| (profile_key(*field), Value::from(head_facts.get(*field))))
            .collect();
        if !updates.is_empty() {
            write_profile_keys(conn, &row.tin, &updates)?;
        }
    }
    Ok(())
}

fn new_profile_json(
    taxpayer: &Taxpayer,
    root: &str,
    branch_code: &str,
    facts: &BranchFacts,
) -> Result<String> {
    let profile: TaxpayerProfile = serde_json::from_value(serde_json::json!({
        "id": null,
        "full_name": taxpayer.name,
        "tin": {
            "segment1": &root[0..3],
            "segment2": &root[3..6],
            "segment3": &root[6..9],
            "branch": branch_code,
        },
        "rdo_code": facts.rdo_code,
        "line_of_business": "",
        "registered_address": facts.registered_address,
        "zip_code": facts.zip_code,
        "phone": facts.phone,
        "email": facts.email,
        "default_form_type": "",
        "taxpayer_type": taxpayer.taxpayer_type,
        "birth_date": taxpayer.birth_date,
    }))?;
    Ok(serde_json::to_string(&profile)?)
}

fn branch_exists(conn: &Connection, tin: &str) -> Result<()> {
    let exists = conn
        .query_row("SELECT 1 FROM profiles WHERE tin = ?1", [tin], |_| Ok(()))
        .optional()?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(TaxProfileError::NotFound(format!(
            "No branch with TIN {tin}"
        )))
    }
}

fn root_of(tin: &str) -> Result<String> {
    tin.get(..9)
        .ok_or_else(|| TaxProfileError::Invalid(format!("`{tin}` is not a full TIN")))
        .and_then(parse_tin_root)
}

fn non_empty(value: &str, what: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(TaxProfileError::Invalid(format!("{what} cannot be empty")))
    } else {
        Ok(trimmed.to_string())
    }
}

struct TemplateRow {
    id: i64,
    tin: String,
    form_code: String,
    fields_json: String,
    source: String,
    created_at: String,
    updated_at: String,
}

impl TemplateRow {
    fn into_template(self) -> Result<FormTemplate> {
        Ok(FormTemplate {
            id: self.id,
            tin: self.tin,
            form_code: self.form_code,
            fields: serde_json::from_str(&self.fields_json)?,
            source: TemplateSource::from_db(&self.source)?,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

fn query_template(conn: &Connection, tin: &str, form_code: &str) -> Result<Option<FormTemplate>> {
    conn.query_row(
        "SELECT id, tin, form_code, fields_json, source, created_at, updated_at
         FROM form_templates WHERE tin = ?1 AND form_code = ?2",
        params![tin, form_code],
        |row| {
            Ok(TemplateRow {
                id: row.get(0)?,
                tin: row.get(1)?,
                form_code: row.get(2)?,
                fields_json: row.get(3)?,
                source: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )
    .optional()?
    .map(TemplateRow::into_template)
    .transpose()
}

fn insert_template(
    conn: &Connection,
    tin: &str,
    form_code: &str,
    fields: &BTreeMap<String, TemplateField>,
    source: &TemplateSource,
) -> Result<()> {
    let inserted = conn.execute(
        "INSERT INTO form_templates (tin, form_code, fields_json, source)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(tin, form_code) DO NOTHING",
        params![
            tin,
            form_code,
            serde_json::to_string(fields)?,
            source.to_db()
        ],
    )?;
    if inserted == 0 {
        return Err(TaxProfileError::Conflict(format!(
            "Branch {tin} already has a {form_code} template; a branch has one template per form"
        )));
    }
    Ok(())
}

/// Make `codes` the active yearly forms of `(tin, year)` inside `conn`'s
/// transaction. Chosen forms become manual includes; every other entry becomes
/// a manual exclusion, so COR reconciliation cannot silently re-add it.
fn replace_yearly_forms(
    conn: &Connection,
    tin: &str,
    year: u16,
    codes: &BTreeSet<String>,
) -> Result<PerYearFormsSet> {
    let mut set = super::forms_set::query_per_year_forms(conn, tin, year)?;
    for entry in &mut set.entries {
        let chosen = codes.contains(&canonical_form_code(&entry.form_code));
        let manual = entry.source == FormSetSource::Manual;
        if chosen && !(entry.is_filing_active() && manual) {
            entry.apply_manual_decision(true, None);
        } else if !chosen && (entry.is_filing_active() || !manual || entry.needs_review()) {
            entry.apply_manual_decision(false, Some(REMOVED_FROM_YEARLY_LIST.to_string()));
        }
    }
    for code in codes {
        if !set
            .entries
            .iter()
            .any(|entry| canonical_form_code(&entry.form_code) == *code)
        {
            set.entries
                .push(FormSetEntry::from_code(code.clone(), FormSetSource::Manual));
        }
    }
    set.entries.sort_by(|a, b| a.form_code.cmp(&b.form_code));
    super::forms_set::execute_replace_per_year_forms(conn, tin, year, &set)?;
    Ok(set)
}

fn canonical_codes<I, S>(codes: I) -> Result<BTreeSet<String>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    codes
        .into_iter()
        .map(|code| {
            let code = canonical_form_code(code.as_ref());
            if code.is_empty() {
                Err(TaxProfileError::Invalid("Empty form code".to_string()))
            } else {
                Ok(code)
            }
        })
        .collect()
}

impl Database {
    // =====================================================================
    // Taxpayers
    // =====================================================================

    /// Every taxpayer with its branches (head office first).
    pub fn list_taxpayers(&self) -> Result<Vec<TaxpayerWithBranches>> {
        let roots: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT tin_root FROM taxpayers ORDER BY name COLLATE NOCASE, tin_root")?;
            stmt.query_map([], |row| row.get(0))?
                .collect::<std::result::Result<_, _>>()?
        };
        roots
            .iter()
            .filter_map(|root| self.get_taxpayer(root).transpose())
            .collect()
    }

    pub fn get_taxpayer(&self, tin_root: &str) -> Result<Option<TaxpayerWithBranches>> {
        let root = parse_tin_root(tin_root)?;
        let Some(taxpayer) = load_taxpayer(&self.conn, &root)? else {
            return Ok(None);
        };
        let branches = resolve_all(&load_branch_rows(&self.conn, &root)?);
        Ok(Some(TaxpayerWithBranches { taxpayer, branches }))
    }

    /// Create a taxpayer and its `00000` head office.
    pub fn create_taxpayer(&self, new: NewTaxpayer) -> Result<TaxpayerWithBranches> {
        let root = parse_tin_root(&new.tin_root)?;
        let name = non_empty(&new.name, "Taxpayer name")?;
        let tx = self.conn.unchecked_transaction()?;
        let taken = tx
            .query_row(
                "SELECT 1 FROM taxpayers WHERE tin_root = ?1
                 UNION SELECT 1 FROM profiles WHERE substr(tin, 1, 9) = ?1",
                [&root],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if taken {
            return Err(TaxProfileError::Conflict(format!(
                "A taxpayer with TIN root {root} already exists"
            )));
        }
        let data = TaxpayerData {
            birth_date: new.birth_date,
            grouping_review: None,
        };
        tx.execute(
            "INSERT INTO taxpayers (tin_root, name, taxpayer_type, data_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                root,
                name,
                taxpayer_type_str(&new.taxpayer_type)?,
                serde_json::to_string(&data)?
            ],
        )?;
        let taxpayer = load_taxpayer(&tx, &root)?.expect("inserted above");
        tx.execute(
            "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
            params![
                format!("{root}00000"),
                new_profile_json(&taxpayer, &root, "00000", &new.head_office)?
            ],
        )?;
        tx.commit()?;
        self.get_taxpayer(&root)?
            .ok_or_else(|| TaxProfileError::NotFound(root))
    }

    /// Update taxpayer-level facts and write them through to every branch's
    /// stored profile, so legacy readers see the same name and type.
    pub fn update_taxpayer(&self, tin_root: &str, update: TaxpayerUpdate) -> Result<Taxpayer> {
        let root = parse_tin_root(tin_root)?;
        let tx = self.conn.unchecked_transaction()?;
        let mut taxpayer = load_taxpayer(&tx, &root)?
            .ok_or_else(|| TaxProfileError::NotFound(format!("No taxpayer {root}")))?;
        if let Some(name) = &update.name {
            taxpayer.name = non_empty(name, "Taxpayer name")?;
        }
        if let Some(taxpayer_type) = update.taxpayer_type {
            taxpayer.taxpayer_type = taxpayer_type;
        }
        if let Some(birth_date) = update.birth_date {
            taxpayer.birth_date = birth_date;
        }
        let mut data = load_taxpayer_data(&tx, &root)?;
        data.birth_date = taxpayer.birth_date;
        tx.execute(
            "UPDATE taxpayers
             SET name = ?2, taxpayer_type = ?3, data_json = ?4, updated_at = datetime('now')
             WHERE tin_root = ?1",
            params![
                root,
                taxpayer.name,
                taxpayer_type_str(&taxpayer.taxpayer_type)?,
                serde_json::to_string(&data)?
            ],
        )?;
        let updates = [
            ("full_name", Value::from(taxpayer.name.clone())),
            (
                "taxpayer_type",
                serde_json::to_value(&taxpayer.taxpayer_type)?,
            ),
            ("birth_date", serde_json::to_value(taxpayer.birth_date)?),
        ];
        for row in load_branch_rows(&tx, &root)? {
            write_profile_keys(&tx, &row.tin, &updates)?;
        }
        tx.commit()?;
        load_taxpayer(&self.conn, &root)?.ok_or_else(|| TaxProfileError::NotFound(root))
    }

    /// Close the one-time D11 grouping review. Names are not changed.
    pub fn resolve_taxpayer_grouping_review(&self, tin_root: &str) -> Result<()> {
        let root = parse_tin_root(tin_root)?;
        let mut data = load_taxpayer_data(&self.conn, &root)?;
        if let Some(review) = data.grouping_review.as_mut()
            && review.resolved_at.is_none()
        {
            review.resolved_at = Some(chrono::Utc::now().to_rfc3339());
            self.conn.execute(
                "UPDATE taxpayers SET data_json = ?2, updated_at = datetime('now')
                 WHERE tin_root = ?1",
                params![root, serde_json::to_string(&data)?],
            )?;
        }
        Ok(())
    }

    /// The profile row that owns the taxpayer-level settings of `tin`'s
    /// taxpayer (D12: Email, Security, Export, Calendar): the head office, or
    /// `tin` itself when the root has no head office.
    pub fn taxpayer_settings_tin(&self, tin: &str) -> Result<String> {
        let rows = load_branch_rows(&self.conn, &root_of(tin)?)?;
        if !rows.iter().any(|row| row.tin == tin) {
            return Err(TaxProfileError::NotFound(format!(
                "No branch with TIN {tin}"
            )));
        }
        Ok(head_office(&rows).map_or_else(|| tin.to_string(), |head| head.tin.clone()))
    }

    // =====================================================================
    // Branches
    // =====================================================================

    pub fn get_branch(&self, tin: &str) -> Result<Option<ResolvedBranch>> {
        let rows = load_branch_rows(&self.conn, &root_of(tin)?)?;
        let head = head_office(&rows);
        Ok(rows
            .iter()
            .find(|row| row.tin == tin)
            .map(|row| resolve_branch(row, head)))
    }

    /// The next unused 5-digit code after the highest one under the root. A
    /// suggestion only: the user picks the code BIR registered.
    pub fn suggest_next_branch_code(&self, tin_root: &str) -> Result<String> {
        let root = parse_tin_root(tin_root)?;
        let highest = load_branch_rows(&self.conn, &root)?
            .iter()
            .filter_map(|row| row.code().parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        Ok(format!("{:05}", (highest + 1).min(99_999)))
    }

    /// Create a branch under an existing taxpayer. Fields not listed inherit
    /// from the head office. Creating `00000` is allowed only for a root that
    /// has no head office yet, and then every field must be given.
    pub fn create_branch(&self, tin_root: &str, new: NewBranch) -> Result<ResolvedBranch> {
        let root = parse_tin_root(tin_root)?;
        if new.branch_code.len() != 5 {
            return Err(TaxProfileError::Invalid(format!(
                "A new branch code is 5 digits; got `{}`",
                new.branch_code
            )));
        }
        let code = normalize_branch_code(&new.branch_code)?;
        let name = non_empty(&new.name, "Branch name")?;
        let tx = self.conn.unchecked_transaction()?;
        let taxpayer = load_taxpayer(&tx, &root)?
            .ok_or_else(|| TaxProfileError::NotFound(format!("No taxpayer {root}")))?;
        let rows = load_branch_rows(&tx, &root)?;
        if rows.iter().any(|row| row.code() == code) {
            return Err(TaxProfileError::Conflict(format!(
                "Taxpayer {root} already has branch {code}"
            )));
        }
        let head_facts = head_office(&rows)
            .filter(|_| !is_head_office_code(&code))
            .map(BranchRow::own_facts);

        let mut facts = BranchFacts::default();
        let mut inherited = BTreeSet::new();
        for field in BranchField::ALL {
            match new.fields.get(&field).unwrap_or(&FieldSetting::Inherit) {
                FieldSetting::Override(value) => facts.set(field, value.clone()),
                FieldSetting::Inherit => {
                    let head = head_facts.as_ref().ok_or_else(|| {
                        TaxProfileError::Invalid(format!(
                            "{field:?} needs a value: there is no head office to inherit from"
                        ))
                    })?;
                    facts.set(field, head.get(field).to_string());
                    inherited.insert(field);
                }
            }
        }
        let tin = format!("{root}{code}");
        let settings = BranchSettings {
            name: Some(name),
            inherited,
        };
        tx.execute(
            "INSERT INTO profiles (tin, data_json, branch_json) VALUES (?1, ?2, ?3)",
            params![
                tin,
                new_profile_json(&taxpayer, &root, &code, &facts)?,
                serde_json::to_string(&settings)?
            ],
        )?;
        tx.commit()?;
        self.get_branch(&tin)?
            .ok_or_else(|| TaxProfileError::NotFound(tin))
    }

    /// Rename a branch and set fields to inherit or override (D2). Head-office
    /// changes flow to every branch that inherits the field.
    pub fn update_branch(&self, tin: &str, update: BranchUpdate) -> Result<ResolvedBranch> {
        let root = root_of(tin)?;
        let tx = self.conn.unchecked_transaction()?;
        let rows = load_branch_rows(&tx, &root)?;
        let row = rows
            .iter()
            .find(|row| row.tin == tin)
            .ok_or_else(|| TaxProfileError::NotFound(format!("No branch with TIN {tin}")))?;
        let head = head_office(&rows);
        let is_head_office = head.is_some_and(|head| head.id == row.id);
        let head_facts = head.filter(|_| !is_head_office).map(BranchRow::own_facts);

        let mut settings = row.settings.clone();
        if let Some(name) = &update.name {
            settings.name = Some(non_empty(name, "Branch name")?);
        }
        let mut writes = Vec::new();
        for (field, setting) in &update.fields {
            match setting {
                FieldSetting::Override(value) => {
                    settings.inherited.remove(field);
                    writes.push((profile_key(*field), Value::from(value.clone())));
                }
                FieldSetting::Inherit => {
                    let head = head_facts.as_ref().ok_or_else(|| {
                        TaxProfileError::Invalid(if is_head_office {
                            "The head office cannot inherit".to_string()
                        } else {
                            "There is no head office to inherit from".to_string()
                        })
                    })?;
                    settings.inherited.insert(*field);
                    writes.push((profile_key(*field), Value::from(head.get(*field))));
                }
            }
        }
        write_profile_keys(&tx, tin, &writes)?;
        if settings != row.settings {
            tx.execute(
                "UPDATE profiles SET branch_json = ?2 WHERE tin = ?1",
                params![tin, serde_json::to_string(&settings)?],
            )?;
        }
        if is_head_office {
            refresh_inherited_caches(&tx, &root)?;
        }
        tx.commit()?;
        self.get_branch(tin)?
            .ok_or_else(|| TaxProfileError::NotFound(tin.to_string()))
    }

    // =====================================================================
    // Yearly forms (D5, D9) — `per_year_forms`, manual source only
    // =====================================================================

    /// Make `form_codes` the forms this branch files in `year`. Read the list
    /// back with [`Database::active_form_codes_for_year`].
    pub fn set_yearly_forms<S: AsRef<str>>(
        &self,
        tin: &str,
        year: u16,
        form_codes: &[S],
    ) -> Result<PerYearFormsSet> {
        branch_exists(&self.conn, tin)?;
        let codes = canonical_codes(form_codes)?;
        let tx = self.conn.unchecked_transaction()?;
        let set = replace_yearly_forms(&tx, tin, year, &codes)?;
        tx.commit()?;
        let _ = self.request_google_calendar_sync();
        Ok(set)
    }

    pub fn clear_yearly_forms(&self, tin: &str, year: u16) -> Result<PerYearFormsSet> {
        self.set_yearly_forms::<&str>(tin, year, &[])
    }

    /// Add a starter pack's forms (for the taxpayer's type) to the list.
    pub fn apply_starter_pack(
        &self,
        tin: &str,
        year: u16,
        pack: StarterPack,
    ) -> Result<PerYearFormsSet> {
        let root = root_of(tin)?;
        let taxpayer = load_taxpayer(&self.conn, &root)?
            .ok_or_else(|| TaxProfileError::NotFound(format!("No taxpayer {root}")))?;
        let mut codes = self.active_form_codes_for_year(tin, year)?;
        codes.extend(
            pack.form_codes(&taxpayer.taxpayer_type)
                .iter()
                .map(|code| code.to_string()),
        );
        self.set_yearly_forms(tin, year, &codes)
    }

    /// D9 "Copy 2026 forms to 2027?": add `from_year`'s forms to `to_year`.
    pub fn copy_yearly_forms(
        &self,
        tin: &str,
        from_year: u16,
        to_year: u16,
    ) -> Result<PerYearFormsSet> {
        let source = self.active_form_codes_for_year(tin, from_year)?;
        if source.is_empty() {
            return Err(TaxProfileError::NotFound(format!(
                "Branch {tin} has no forms in {from_year} to copy"
            )));
        }
        let mut codes = self.active_form_codes_for_year(tin, to_year)?;
        codes.extend(source);
        self.set_yearly_forms(tin, to_year, &codes)
    }

    // =====================================================================
    // Templates (D6, D7, D8, D15, D16, D17)
    // =====================================================================

    pub fn get_form_template(&self, tin: &str, form_code: &str) -> Result<Option<FormTemplate>> {
        query_template(&self.conn, tin, &canonical_form_code(form_code))
    }

    pub fn list_form_templates(&self, tin: &str) -> Result<Vec<FormTemplate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tin, form_code, fields_json, source, created_at, updated_at
             FROM form_templates WHERE tin = ?1 ORDER BY form_code",
        )?;
        let rows = stmt.query_map([tin], |row| {
            Ok(TemplateRow {
                id: row.get(0)?,
                tin: row.get(1)?,
                form_code: row.get(2)?,
                fields_json: row.get(3)?,
                source: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        rows.map(|row| row?.into_template()).collect()
    }

    /// Path 2: a blank form in template mode; fields can only be marked.
    pub fn create_blank_form_template(
        &self,
        tin: &str,
        form_code: &str,
        marked_fields: &[String],
    ) -> Result<FormTemplate> {
        let form_code = canonical_form_code(form_code);
        branch_exists(&self.conn, tin)?;
        let fields = marked_fields
            .iter()
            .map(|field| (field.clone(), TemplateField::Marked))
            .collect();
        validate_template_fields(&form_code, &fields, &TemplateSource::Blank)?;
        insert_template(&self.conn, tin, &form_code, &fields, &TemplateSource::Blank)?;
        self.get_form_template(tin, &form_code)?
            .ok_or_else(|| TaxProfileError::NotFound(form_code))
    }

    /// Path 1 / D17: "Use as template" on a filed return. Kept values are read
    /// from the filed return itself; the caller only chooses keep or mark.
    pub fn create_form_template_from_filed_draft(
        &self,
        draft_id: i64,
        choices: &BTreeMap<String, TemplateChoice>,
    ) -> Result<FormTemplate> {
        let (tin, form_code, status, data_json): (String, String, String, String) = self
            .conn
            .query_row(
                "SELECT tin, form_code, status, data_json FROM form_drafts WHERE id = ?1",
                [draft_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?
            .ok_or_else(|| TaxProfileError::NotFound(format!("No return with id {draft_id}")))?;
        if !matches!(
            super::drafts::filing_status_from_db(&status),
            FilingStatus::Submitted | FilingStatus::Confirmed | FilingStatus::Paid
        ) {
            return Err(TaxProfileError::Invalid(format!(
                "Return {draft_id} has not been filed ({status}); only a filed return can become a template"
            )));
        }
        branch_exists(&self.conn, &tin)?;
        let form_code = canonical_form_code(&form_code);
        let draft: Value = serde_json::from_str(&data_json)?;
        let mut fields = BTreeMap::new();
        for (field, choice) in choices {
            let value = match choice {
                TemplateChoice::Mark => TemplateField::Marked,
                TemplateChoice::Keep => TemplateField::Kept {
                    value: value_at_path(&draft, field).cloned().ok_or_else(|| {
                        TaxProfileError::Invalid(format!(
                            "Return {draft_id} has no value for `{field}`"
                        ))
                    })?,
                },
            };
            fields.insert(field.clone(), value);
        }
        let source = TemplateSource::Filed { draft_id };
        validate_template_fields(&form_code, &fields, &source)?;
        insert_template(&self.conn, &tin, &form_code, &fields, &source)?;
        self.get_form_template(&tin, &form_code)?
            .ok_or_else(|| TaxProfileError::NotFound(form_code))
    }

    /// Replace an existing template's fields (template-mode edits, D10 "Update
    /// template?"). The source is unchanged.
    pub fn update_form_template_fields(
        &self,
        tin: &str,
        form_code: &str,
        fields: BTreeMap<String, TemplateField>,
    ) -> Result<FormTemplate> {
        let form_code = canonical_form_code(form_code);
        let existing = query_template(&self.conn, tin, &form_code)?.ok_or_else(|| {
            TaxProfileError::NotFound(format!("Branch {tin} has no {form_code} template"))
        })?;
        validate_template_fields(&form_code, &fields, &existing.source)?;
        self.conn.execute(
            "UPDATE form_templates SET fields_json = ?3, updated_at = datetime('now')
             WHERE tin = ?1 AND form_code = ?2",
            params![tin, form_code, serde_json::to_string(&fields)?],
        )?;
        self.get_form_template(tin, &form_code)?
            .ok_or_else(|| TaxProfileError::NotFound(form_code))
    }

    /// Returns whether a template was deleted.
    pub fn delete_form_template(&self, tin: &str, form_code: &str) -> Result<bool> {
        Ok(self.conn.execute(
            "DELETE FROM form_templates WHERE tin = ?1 AND form_code = ?2",
            params![tin, canonical_form_code(form_code)],
        )? > 0)
    }

    /// D16 bulk copy of `from_tin`'s templates to `to_tin`, as independent
    /// copies (D6). `year` is the target year whose form list decides whether
    /// a form is "missing at target". All or nothing.
    pub fn copy_form_templates<S: AsRef<str>>(
        &self,
        from_tin: &str,
        form_codes: &[S],
        to_tin: &str,
        year: u16,
        on_conflict: TemplateConflictPolicy,
        on_missing_form: MissingFormPolicy,
    ) -> Result<BulkCopyReport> {
        if from_tin == to_tin {
            return Err(TaxProfileError::Invalid(
                "Copy templates to a different branch".to_string(),
            ));
        }
        branch_exists(&self.conn, to_tin)?;
        let codes = canonical_codes(form_codes)?;
        let tx = self.conn.unchecked_transaction()?;
        let mut report = BulkCopyReport::default();
        let mut target_forms: BTreeSet<String> =
            super::forms_set::query_per_year_forms(&tx, to_tin, year)?
                .active_form_codes()
                .into_iter()
                .map(|code| canonical_form_code(&code))
                .collect();
        let source_tag = TemplateSource::Copy {
            from_tin: from_tin.to_string(),
        };

        for code in &codes {
            let template = query_template(&tx, from_tin, code)?.ok_or_else(|| {
                TaxProfileError::NotFound(format!("Branch {from_tin} has no {code} template"))
            })?;
            validate_template_fields(code, &template.fields, &source_tag)?;

            if !target_forms.contains(code) {
                match on_missing_form {
                    MissingFormPolicy::Skip => {
                        report.skipped_missing_form.push(code.clone());
                        continue;
                    }
                    MissingFormPolicy::AddForm => {
                        target_forms.insert(code.clone());
                        report.forms_added.push(code.clone());
                    }
                }
            }

            if query_template(&tx, to_tin, code)?.is_some() {
                match on_conflict {
                    TemplateConflictPolicy::Skip => {
                        report.skipped_existing.push(code.clone());
                        continue;
                    }
                    TemplateConflictPolicy::Replace => {
                        tx.execute(
                            "UPDATE form_templates
                             SET fields_json = ?3, source = ?4, updated_at = datetime('now')
                             WHERE tin = ?1 AND form_code = ?2",
                            params![
                                to_tin,
                                code,
                                serde_json::to_string(&template.fields)?,
                                source_tag.to_db()
                            ],
                        )?;
                        report.replaced.push(code.clone());
                    }
                }
            } else {
                insert_template(&tx, to_tin, code, &template.fields, &source_tag)?;
                report.copied.push(code.clone());
            }
        }

        if !report.forms_added.is_empty() {
            replace_yearly_forms(&tx, to_tin, year, &target_forms)?;
        }
        tx.commit()?;
        if !report.forms_added.is_empty() {
            let _ = self.request_google_calendar_sync();
        }
        Ok(report)
    }

    // =====================================================================
    // Prefill
    // =====================================================================

    /// What opening `form_code` from the Tax Form Library applies for this
    /// branch: Auto values (period and identity), then kept template values,
    /// then the guided-mode stop list (D13, D14, D18).
    ///
    /// `year` and `slot` are exactly the library's `FileForm { year, quarter }`.
    pub fn resolve_prefill(
        &self,
        tin: &str,
        form_code: &str,
        year: u16,
        slot: u8,
    ) -> Result<PrefillPlan> {
        let form_code = canonical_form_code(form_code);
        let kinds = field_kinds_for(&form_code)
            .ok_or_else(|| TemplateFieldError::FormNotClassified(form_code.clone()))?;
        let period = ResolvedPeriod::from_library_selection(&form_code, year, slot)?;
        let branch = self
            .get_branch(tin)?
            .ok_or_else(|| TaxProfileError::NotFound(format!("No branch with TIN {tin}")))?;
        let taxpayer = load_taxpayer(&self.conn, &branch.tin_root)?
            .ok_or_else(|| TaxProfileError::NotFound(format!("No taxpayer {}", branch.tin_root)))?;

        let mut auto_values = BTreeMap::new();
        let mut locked_fields = Vec::new();
        for (field, source) in kinds.auto_fields() {
            locked_fields.push(field.to_string());
            let value = match source {
                AutoSource::Period(source) => period.value_of(source),
                AutoSource::Identity(source) => Some(match source {
                    IdentitySource::Tin => Value::from(branch.tin.clone()),
                    IdentitySource::TaxpayerName => Value::from(taxpayer.name.clone()),
                    IdentitySource::TaxpayerType => serde_json::to_value(&taxpayer.taxpayer_type)?,
                    IdentitySource::RdoCode => Value::from(branch.value(BranchField::RdoCode)),
                    IdentitySource::RegisteredAddress => {
                        Value::from(branch.value(BranchField::RegisteredAddress))
                    }
                    IdentitySource::ZipCode => Value::from(branch.value(BranchField::ZipCode)),
                    IdentitySource::Phone => Value::from(branch.value(BranchField::Phone)),
                    IdentitySource::Email => Value::from(branch.value(BranchField::Email)),
                }),
                AutoSource::Computed(_) => None,
            };
            if let Some(value) = value {
                auto_values.insert(field.to_string(), value);
            }
        }

        let mut kept_values = BTreeMap::new();
        let mut marked_fields = Vec::new();
        if let Some(template) = query_template(&self.conn, tin, &form_code)? {
            for (field, value) in template.fields {
                // Re-check against the current registry: a field reclassified
                // since the template was saved must never override an Auto
                // value or prefill an amount.
                match (kinds.kind_of(&field), value) {
                    (
                        Some(FieldKind::Templatable(TemplatableKind::Setup)),
                        TemplateField::Kept { value },
                    ) => {
                        kept_values.insert(field, value);
                    }
                    (Some(FieldKind::Templatable(_)), TemplateField::Marked) => {
                        marked_fields.push(field);
                    }
                    (kind, _) => tracing::warn!(
                        tin,
                        form_code,
                        field,
                        ?kind,
                        "Ignoring a template field that is no longer templatable"
                    ),
                }
            }
        }
        marked_fields.sort_by_key(|field| form_order_key(kinds, field));

        Ok(PrefillPlan {
            tin: branch.tin,
            period,
            auto_values,
            locked_fields,
            kept_values,
            guided_default_on: !marked_fields.is_empty(),
            marked_fields,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tax_profile::ResolvedField;
    use serde_json::json;

    const ROOT: &str = "123456789";
    const HEAD: &str = "12345678900000";

    fn facts(rdo: &str, address: &str) -> BranchFacts {
        BranchFacts {
            rdo_code: rdo.into(),
            registered_address: address.into(),
            zip_code: "1100".into(),
            phone: "09170000000".into(),
            email: "head@example.com".into(),
        }
    }

    fn db_with_taxpayer(taxpayer_type: TaxpayerType) -> Database {
        let db = Database::open_in_memory_for_tests().unwrap();
        db.create_taxpayer(NewTaxpayer {
            tin_root: "123-456-789".into(),
            name: "Juan Dela Cruz".into(),
            taxpayer_type,
            birth_date: None,
            head_office: facts("039", "Quezon City"),
        })
        .unwrap();
        db
    }

    fn add_branch(db: &Database, code: &str, fields: &[(BranchField, FieldSetting)]) -> String {
        db.create_branch(
            ROOT,
            NewBranch {
                branch_code: code.into(),
                name: format!("Store {code}"),
                fields: fields.iter().cloned().collect(),
            },
        )
        .unwrap()
        .tin
    }

    fn override_rdo(value: &str) -> BTreeMap<BranchField, FieldSetting> {
        BTreeMap::from([(BranchField::RdoCode, FieldSetting::Override(value.into()))])
    }

    #[test]
    fn branch_inherits_head_office_until_overridden() {
        let db = db_with_taxpayer(TaxpayerType::Individual);
        let branch = add_branch(
            &db,
            "00001",
            &[(
                BranchField::RegisteredAddress,
                FieldSetting::Override("Makati".into()),
            )],
        );

        db.update_branch(
            HEAD,
            BranchUpdate {
                fields: override_rdo("040"),
                ..BranchUpdate::default()
            },
        )
        .unwrap();
        let resolved = db.get_branch(&branch).unwrap().unwrap();
        assert_eq!(resolved.branch_code, "00001");
        assert_eq!(resolved.name, "Store 00001");
        assert_eq!(
            resolved.facts[&BranchField::RdoCode],
            ResolvedField {
                value: "040".into(),
                inherited: true
            }
        );
        assert_eq!(
            resolved.facts[&BranchField::RegisteredAddress],
            ResolvedField {
                value: "Makati".into(),
                inherited: false
            }
        );
        // Legacy readers of the branch profile see the inherited value too.
        assert_eq!(db.get_profile(&branch).unwrap().unwrap().rdo_code, "040");

        // Override, then go back to "Same as head office".
        db.update_branch(
            &branch,
            BranchUpdate {
                fields: override_rdo("050"),
                ..BranchUpdate::default()
            },
        )
        .unwrap();
        db.update_branch(
            HEAD,
            BranchUpdate {
                fields: override_rdo("041"),
                ..BranchUpdate::default()
            },
        )
        .unwrap();
        assert_eq!(
            db.get_branch(&branch)
                .unwrap()
                .unwrap()
                .value(BranchField::RdoCode),
            "050"
        );
        let back = db
            .update_branch(
                &branch,
                BranchUpdate {
                    fields: BTreeMap::from([(BranchField::RdoCode, FieldSetting::Inherit)]),
                    ..BranchUpdate::default()
                },
            )
            .unwrap();
        assert_eq!(back.facts[&BranchField::RdoCode].value, "041");
        assert!(back.facts[&BranchField::RdoCode].inherited);

        let head_inherit = db.update_branch(
            HEAD,
            BranchUpdate {
                fields: BTreeMap::from([(BranchField::Email, FieldSetting::Inherit)]),
                ..BranchUpdate::default()
            },
        );
        assert!(matches!(head_inherit, Err(TaxProfileError::Invalid(_))));
    }

    #[test]
    fn taxpayer_facts_apply_to_every_branch_and_settings_resolve_to_head_office() {
        let db = db_with_taxpayer(TaxpayerType::Individual);
        let branch = add_branch(&db, "00001", &[]);

        let updated = db
            .update_taxpayer(
                ROOT,
                TaxpayerUpdate {
                    name: Some("Juan P. Dela Cruz".into()),
                    taxpayer_type: Some(TaxpayerType::Corporation),
                    ..TaxpayerUpdate::default()
                },
            )
            .unwrap();
        assert_eq!(updated.name, "Juan P. Dela Cruz");
        for tin in [HEAD, branch.as_str()] {
            let profile = db.get_profile(tin).unwrap().unwrap();
            assert_eq!(profile.full_name, "Juan P. Dela Cruz", "{tin}");
            assert_eq!(profile.taxpayer_type, TaxpayerType::Corporation, "{tin}");
        }
        assert_eq!(db.taxpayer_settings_tin(&branch).unwrap(), HEAD);
        assert_eq!(db.taxpayer_settings_tin(HEAD).unwrap(), HEAD);
    }

    #[test]
    fn identity_conflicts_are_rejected() {
        let db = db_with_taxpayer(TaxpayerType::Individual);
        let again = db.create_taxpayer(NewTaxpayer {
            tin_root: ROOT.into(),
            name: "Someone".into(),
            taxpayer_type: TaxpayerType::Individual,
            birth_date: None,
            head_office: BranchFacts::default(),
        });
        assert!(matches!(again, Err(TaxProfileError::Conflict(_))));

        // A legacy 3-digit `001` branch occupies code 00001.
        let mut legacy = db.get_profile(HEAD).unwrap().unwrap();
        legacy.id = None;
        legacy.tin.branch = "001".into();
        db.save_profile(legacy).unwrap();
        assert_eq!(db.suggest_next_branch_code(ROOT).unwrap(), "00002");
        let clash = db.create_branch(
            ROOT,
            NewBranch {
                branch_code: "00001".into(),
                name: "Dup".into(),
                fields: BTreeMap::new(),
            },
        );
        assert!(matches!(clash, Err(TaxProfileError::Conflict(_))));
    }

    #[test]
    fn legacy_root_without_head_office_can_add_one_but_branches_cannot_inherit_before() {
        let db = Database::open_in_memory_for_tests().unwrap();
        let mut profile: TaxpayerProfile = serde_json::from_value(json!({
            "id": null, "full_name": "Branch Only",
            "tin": {"segment1": "123", "segment2": "456", "segment3": "789", "branch": "00003"},
            "rdo_code": "039", "line_of_business": "", "registered_address": "Pasig",
            "zip_code": "1600", "phone": "", "email": "", "default_form_type": ""
        }))
        .unwrap();
        profile.id = None;
        db.save_profile(profile).unwrap();
        let grouped = db.get_taxpayer(ROOT).unwrap().unwrap();
        assert!(
            grouped
                .taxpayer
                .grouping_review
                .is_some_and(|review| review.missing_head_office)
        );
        db.resolve_taxpayer_grouping_review(ROOT).unwrap();
        let review = db
            .get_taxpayer(ROOT)
            .unwrap()
            .unwrap()
            .taxpayer
            .grouping_review;
        assert!(review.is_some_and(|review| !review.needs_review()));
        assert_eq!(
            db.taxpayer_settings_tin("12345678900003").unwrap(),
            "12345678900003"
        );

        let inheriting = db.create_branch(
            ROOT,
            NewBranch {
                branch_code: "00004".into(),
                name: "Four".into(),
                fields: BTreeMap::new(),
            },
        );
        assert!(matches!(inheriting, Err(TaxProfileError::Invalid(_))));

        let head = db
            .create_branch(
                ROOT,
                NewBranch {
                    branch_code: "00000".into(),
                    name: "Main".into(),
                    fields: BranchField::ALL
                        .into_iter()
                        .map(|field| (field, FieldSetting::Override("x".into())))
                        .collect(),
                },
            )
            .unwrap();
        assert!(head.is_head_office);
    }

    #[test]
    fn yearly_forms_are_manual_and_survive_legacy_profile_saves() {
        let db = db_with_taxpayer(TaxpayerType::Individual);
        // A COR-generated entry from the old flow.
        db.save_per_year_forms(
            HEAD,
            2026,
            &PerYearFormsSet::from_codes(2026, ["1701Q"], FormSetSource::ReviewedCor),
        )
        .unwrap();

        db.set_yearly_forms(HEAD, 2026, &["2551Q", "0619E"])
            .unwrap();
        assert_eq!(
            db.active_form_codes_for_year(HEAD, 2026).unwrap(),
            vec!["0619E", "2551Q"]
        );
        let excluded = db.get_per_year_forms(HEAD, 2026).unwrap();
        let excluded = excluded.entry("1701Q").unwrap();
        assert!(!excluded.active);
        assert_eq!(excluded.source, FormSetSource::Manual);

        let profile = db.get_profile(HEAD).unwrap().unwrap();
        db.save_profile(profile).unwrap();
        assert_eq!(
            db.active_form_codes_for_year(HEAD, 2026).unwrap(),
            vec!["0619E", "2551Q"]
        );

        db.clear_yearly_forms(HEAD, 2026).unwrap();
        assert!(
            db.active_form_codes_for_year(HEAD, 2026)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn starter_packs_and_year_copy_add_to_the_list() {
        let db = db_with_taxpayer(TaxpayerType::Corporation);
        db.set_yearly_forms(HEAD, 2026, &["0605"]).unwrap();
        db.apply_starter_pack(HEAD, 2026, StarterPack::VatBusiness)
            .unwrap();
        let mut expected = vec!["0605", "1702Q", "1702RT", "2550Q"];
        assert_eq!(db.active_form_codes_for_year(HEAD, 2026).unwrap(), expected);

        db.set_yearly_forms(HEAD, 2027, &["1601C"]).unwrap();
        db.copy_yearly_forms(HEAD, 2026, 2027).unwrap();
        expected.insert(1, "1601C");
        assert_eq!(db.active_form_codes_for_year(HEAD, 2027).unwrap(), expected);
        assert!(matches!(
            db.copy_yearly_forms(HEAD, 2030, 2031),
            Err(TaxProfileError::NotFound(_))
        ));
    }

    /// Insert a filed 2551Q the way the dedicated persistence paths leave it.
    fn insert_return(db: &Database, tin: &str, status: &str) -> i64 {
        let profile = db.get_profile(tin).unwrap().unwrap();
        let mut draft = crate::forms::Form2551QDraft::new_from_profile(&profile, 2026, 1);
        draft.item_13_election = crate::forms::form_2551q::Item13Election::Graduated;
        draft.schedule_1[0].taxable_amount = 150_000.0;
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json)
                 VALUES (?1, '2551Q', 2026, 1, 'Q1', ?2, ?3)",
                params![tin, status, serde_json::to_string(&draft).unwrap()],
            )
            .unwrap();
        db.conn.last_insert_rowid()
    }

    fn choices(entries: &[(&str, TemplateChoice)]) -> BTreeMap<String, TemplateChoice> {
        entries
            .iter()
            .map(|(field, choice)| (field.to_string(), *choice))
            .collect()
    }

    #[test]
    fn template_from_filed_return_keeps_filed_setup_values_only() {
        let db = db_with_taxpayer(TaxpayerType::Individual);
        let filed = insert_return(&db, HEAD, "Confirmed");
        let template = db
            .create_form_template_from_filed_draft(
                filed,
                &choices(&[
                    ("item_13_election", TemplateChoice::Keep),
                    ("schedule_1[0].atc", TemplateChoice::Keep),
                    ("schedule_1[0].taxable_amount", TemplateChoice::Mark),
                ]),
            )
            .unwrap();
        assert_eq!(template.tin, HEAD);
        assert_eq!(template.source, TemplateSource::Filed { draft_id: filed });
        assert_eq!(
            template.fields["item_13_election"],
            TemplateField::Kept {
                value: json!("graduated")
            }
        );
        assert_eq!(
            template.fields["schedule_1[0].atc"],
            TemplateField::Kept {
                value: json!("PT010")
            }
        );

        // D7: one template per form per branch.
        let duplicate = db.create_blank_form_template(HEAD, "2551Q", &[]);
        assert!(matches!(duplicate, Err(TaxProfileError::Conflict(_))));

        // D8: an amount can be marked, never kept - not even from a filed return.
        db.delete_form_template(HEAD, "2551Q").unwrap();
        let amount = db.create_form_template_from_filed_draft(
            filed,
            &choices(&[("schedule_1[0].taxable_amount", TemplateChoice::Keep)]),
        );
        assert!(matches!(
            amount,
            Err(TaxProfileError::Template(TemplateFieldError::AmountKept(_)))
        ));
        let auto = db.update_form_template_fields(HEAD, "2551Q", BTreeMap::new());
        assert!(matches!(auto, Err(TaxProfileError::NotFound(_))));

        let draft = insert_return(&db, &add_branch(&db, "00001", &[]), "Draft");
        let unfiled = db.create_form_template_from_filed_draft(
            draft,
            &choices(&[("item_13_election", TemplateChoice::Keep)]),
        );
        assert!(matches!(unfiled, Err(TaxProfileError::Invalid(_))));
    }

    #[test]
    fn template_writes_reject_auto_fields_and_blank_kept_values() {
        let db = db_with_taxpayer(TaxpayerType::Individual);
        let auto = db.create_blank_form_template(HEAD, "2551Q", &["rdo_code".to_string()]);
        assert!(matches!(
            auto,
            Err(TaxProfileError::Template(TemplateFieldError::AutoField(_)))
        ));
        db.create_blank_form_template(HEAD, "2551Q", &["item_13_election".to_string()])
            .unwrap();
        let kept_on_blank = db.update_form_template_fields(
            HEAD,
            "2551Q",
            BTreeMap::from([(
                "item_13_election".to_string(),
                TemplateField::Kept {
                    value: json!("graduated"),
                },
            )]),
        );
        assert!(matches!(
            kept_on_blank,
            Err(TaxProfileError::Template(
                TemplateFieldError::KeptOnBlankTemplate(_)
            ))
        ));
        assert!(db.delete_form_template(HEAD, "2551Q").unwrap());
        assert!(db.get_form_template(HEAD, "2551Q").unwrap().is_none());
    }

    #[test]
    fn bulk_copy_follows_conflict_and_missing_form_policies() {
        let db = db_with_taxpayer(TaxpayerType::Individual);
        let target = add_branch(&db, "00001", &[]);
        let filed = insert_return(&db, HEAD, "Submitted");
        db.create_form_template_from_filed_draft(
            filed,
            &choices(&[("item_13_election", TemplateChoice::Keep)]),
        )
        .unwrap();
        db.create_blank_form_template(HEAD, "1601C", &["category_of_agent".to_string()])
            .unwrap();
        db.set_yearly_forms(&target, 2026, &["2551Q"]).unwrap();
        db.create_blank_form_template(&target, "2551Q", &["tax_relief".to_string()])
            .unwrap();
        let both = ["2551Q", "1601C"];

        let skipped = db
            .copy_form_templates(
                HEAD,
                &both,
                &target,
                2026,
                TemplateConflictPolicy::Skip,
                MissingFormPolicy::Skip,
            )
            .unwrap();
        assert_eq!(
            skipped,
            BulkCopyReport {
                skipped_existing: vec!["2551Q".into()],
                skipped_missing_form: vec!["1601C".into()],
                ..BulkCopyReport::default()
            }
        );
        assert!(db.get_form_template(&target, "1601C").unwrap().is_none());

        let replaced = db
            .copy_form_templates(
                HEAD,
                &both,
                &target,
                2026,
                TemplateConflictPolicy::Replace,
                MissingFormPolicy::AddForm,
            )
            .unwrap();
        assert_eq!(
            replaced,
            BulkCopyReport {
                copied: vec!["1601C".into()],
                replaced: vec!["2551Q".into()],
                forms_added: vec!["1601C".into()],
                ..BulkCopyReport::default()
            }
        );
        assert_eq!(
            db.active_form_codes_for_year(&target, 2026).unwrap(),
            vec!["1601C", "2551Q"]
        );
        let copy = db.get_form_template(&target, "2551Q").unwrap().unwrap();
        assert_eq!(
            copy.source,
            TemplateSource::Copy {
                from_tin: HEAD.into()
            }
        );

        // D6: an independent copy.
        db.update_form_template_fields(&target, "2551Q", BTreeMap::new())
            .unwrap();
        assert_eq!(
            db.get_form_template(HEAD, "2551Q")
                .unwrap()
                .unwrap()
                .fields
                .len(),
            1
        );

        // A missing source template fails the whole copy.
        let missing = db.copy_form_templates(
            &target,
            &["0605"],
            HEAD,
            2026,
            TemplateConflictPolicy::Replace,
            MissingFormPolicy::AddForm,
        );
        assert!(matches!(missing, Err(TaxProfileError::NotFound(_))));
    }

    #[test]
    fn prefill_applies_auto_then_kept_values_then_marked_stops() {
        let db = db_with_taxpayer(TaxpayerType::Individual);
        let branch = add_branch(
            &db,
            "00002",
            &[(
                BranchField::RegisteredAddress,
                FieldSetting::Override("Cebu City".into()),
            )],
        );
        let fields = BTreeMap::from([
            (
                "item_13_election".to_string(),
                TemplateField::Kept {
                    value: json!("eight_percent"),
                },
            ),
            (
                "schedule_1[1].taxable_amount".to_string(),
                TemplateField::Marked,
            ),
            (
                "schedule_1[0].taxable_amount".to_string(),
                TemplateField::Marked,
            ),
            ("tax_relief".to_string(), TemplateField::Marked),
        ]);
        db.conn
            .execute(
                "INSERT INTO form_templates (tin, form_code, fields_json, source)
                 VALUES (?1, '2551Q', ?2, 'filed:1')",
                params![branch, serde_json::to_string(&fields).unwrap()],
            )
            .unwrap();
        // A field stored by a build that classified it differently must not
        // override the Auto value.
        db.conn
            .execute(
                "UPDATE form_templates SET fields_json = json_set(fields_json,
                    '$.rdo_code', json('{\"kind\":\"kept\",\"value\":\"999\"}'))",
                [],
            )
            .unwrap();

        let plan = db.resolve_prefill(&branch, "2551Q", 2026, 2).unwrap();
        assert_eq!(
            plan.auto_values,
            BTreeMap::from([
                ("tin".to_string(), json!("12345678900002")),
                ("taxable_year".to_string(), json!(2026)),
                ("quarter".to_string(), json!(2)),
                ("rdo_code".to_string(), json!("039")),
                ("taxpayer_name".to_string(), json!("Juan Dela Cruz")),
                ("registered_address".to_string(), json!("Cebu City")),
                ("zip_code".to_string(), json!("1100")),
                ("contact_number".to_string(), json!("09170000000")),
                ("email".to_string(), json!("head@example.com")),
                ("taxpayer_type".to_string(), json!("Individual")),
            ])
        );
        assert!(plan.locked_fields.contains(&"total_tax_due".to_string()));
        assert_eq!(
            plan.kept_values,
            BTreeMap::from([("item_13_election".to_string(), json!("eight_percent"))])
        );
        assert_eq!(
            plan.marked_fields,
            vec![
                "tax_relief",
                "schedule_1[0].taxable_amount",
                "schedule_1[1].taxable_amount"
            ]
        );
        assert!(plan.guided_default_on);

        let monthly = db.resolve_prefill(HEAD, "1601C", 2026, 2).unwrap();
        assert_eq!(monthly.auto_values["month"], json!(2));
        assert_eq!(
            monthly.auto_values["email_address"],
            json!("head@example.com")
        );
        assert!(monthly.kept_values.is_empty() && !monthly.guided_default_on);

        assert!(matches!(
            db.resolve_prefill(HEAD, "1701", 2025, 0),
            Err(TaxProfileError::Template(
                TemplateFieldError::FormNotClassified(_)
            ))
        ));
    }

    #[test]
    fn deleting_a_profile_removes_its_templates_and_empty_taxpayer() {
        let db = db_with_taxpayer(TaxpayerType::Individual);
        db.create_blank_form_template(HEAD, "2551Q", &[]).unwrap();
        db.delete_profile(HEAD).unwrap();
        assert!(db.get_form_template(HEAD, "2551Q").unwrap().is_none());
        assert!(db.get_taxpayer(ROOT).unwrap().is_none());
    }
}
