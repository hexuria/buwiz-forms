//! Assisted filling for the hand-built 2551Q and 1601C forms.
//!
//! Which boxes the agent may fill (by the draft's serde field names, schedule
//! rows as `schedule_1.<i>.<field>`), where each box's value came from, and
//! which required boxes are still empty ("needs you"). Always compiled: the
//! GPUI views keep the same per-box state the agent host reads and writes.
//!
//! Profile-owned boxes (TIN, name, address, RDO, ZIP, contact, email, line of
//! business), the period identity (year, month/quarter) and every computed box
//! are not fillable: `form.open` picks the period and the profile owns the
//! rest. Item 13 of 2551Q is never fillable either — the agent must not infer
//! an income-tax election; the user answers it or the stored one prefills it.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use bir_core::forms::ATC_TABLE_2551Q;
use bir_core::forms::form_1601c::{Form1601CDraft, Form1601CSchedule1Row, MAX_SCHEDULE_1_ROWS};
use bir_core::forms::form_2551q::{Form2551QDraft, Item13Election, Schedule1Row};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::ids;

/// Where a box's value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoxSource {
    Profile,
    PastReturn,
    Document,
    User,
    Ai,
}

impl BoxSource {
    /// Source an agent fill may claim. Defaults to `ai`; `user` is reserved
    /// for what the user typed in the form.
    pub fn parse_fill(raw: Option<&str>) -> Result<Self, String> {
        match raw.map(str::trim).filter(|s| !s.is_empty()) {
            None | Some("ai") => Ok(Self::Ai),
            Some("profile") => Ok(Self::Profile),
            Some("past_return") => Ok(Self::PastReturn),
            Some("document") => Ok(Self::Document),
            Some("user") => Err(
                "source `user` is reserved for boxes the user typed; use profile, past_return, document or ai"
                    .into(),
            ),
            Some(other) => Err(format!(
                "unknown source `{other}`; use profile, past_return, document or ai"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::PastReturn => "past_return",
            Self::Document => "document",
            Self::User => "user",
            Self::Ai => "ai",
        }
    }
}

/// Per-form assist state, kept with the draft (host form state and GPUI view).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AssistState {
    /// Box key -> where its current value came from. Only boxes someone
    /// filled or typed are listed; profile prefill at open is not.
    pub sources: BTreeMap<String, BoxSource>,
    /// The user typed something that is not saved yet. The agent may not
    /// discard it (`form.dismiss` is refused).
    pub unsaved_user_edits: bool,
    /// Anything unsaved, the agent's fills included. The Dismiss button asks
    /// before discarding when this is set.
    pub unsaved_changes: bool,
    /// Saved with nothing changed since: the one-form lock is released.
    pub lock_released: bool,
}

impl AssistState {
    pub fn note_saved(&mut self) {
        self.unsaved_user_edits = false;
        self.unsaved_changes = false;
        self.lock_released = true;
    }

    pub fn note_user_edits<I: IntoIterator<Item = String>>(&mut self, keys: I) {
        let mut any = false;
        for key in keys {
            self.sources.insert(key, BoxSource::User);
            any = true;
        }
        if any {
            self.unsaved_user_edits = true;
            self.unsaved_changes = true;
            self.lock_released = false;
        }
    }

    pub fn note_agent_fill(&mut self, key: &str, source: BoxSource) {
        self.sources.insert(key.to_string(), source);
        self.unsaved_changes = true;
        self.lock_released = false;
    }

    pub fn is_user_box(&self, key: &str) -> bool {
        self.sources.get(key) == Some(&BoxSource::User)
    }

    /// Boxes the agent filled, with their source.
    pub fn agent_filled(&self) -> Vec<(String, BoxSource)> {
        self.sources
            .iter()
            .filter(|(_, source)| **source != BoxSource::User)
            .map(|(key, source)| (key.clone(), *source))
            .collect()
    }
}

/// One empty box the user has to answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NeedsYouBox {
    pub field: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Money,
    Text,
    Bool,
    Int,
    Enum(&'static [&'static str]),
    Withheld,
    Category,
}

const FIELDS_2551Q: &[(&str, Kind)] = &[
    ("tax_period_basis", Kind::Enum(&["calendar", "fiscal"])),
    ("year_end_month", Kind::Int),
    ("is_amended", Kind::Bool),
    ("original_return_filed_and_paid_on_time", Kind::Bool),
    ("number_of_attached_sheets", Kind::Int),
    ("tax_relief", Kind::Bool),
    ("tax_relief_specification", Kind::Text),
    ("creditable_tax_withheld", Kind::Money),
    ("tax_paid_previous", Kind::Money),
    ("other_tax_credit", Kind::Money),
    ("other_tax_credit_description", Kind::Text),
    (
        "overpayment_disposition",
        Kind::Enum(&["none", "refund", "tax_credit_certificate"]),
    ),
];

const ROW_FIELDS_2551Q: &[&str] = &["atc", "taxable_amount"];

const FIELDS_1601C: &[(&str, Kind)] = &[
    ("is_amended", Kind::Bool),
    ("any_taxes_withheld", Kind::Withheld),
    ("number_of_sheets", Kind::Int),
    ("atc", Kind::Text),
    ("category_of_agent", Kind::Category),
    ("tax_relief", Kind::Bool),
    ("tax_relief_specification", Kind::Text),
    ("tax_14_total_compensation", Kind::Money),
    ("tax_15_statutory_minimum_wage", Kind::Money),
    ("tax_16_holiday_pay", Kind::Money),
    ("tax_17_13th_month_pay", Kind::Money),
    ("tax_18_de_minimis", Kind::Money),
    ("tax_19_sss_gsis", Kind::Money),
    ("tax_20_other_name", Kind::Text),
    ("tax_20_other_amount", Kind::Money),
    ("tax_23_not_subject", Kind::Money),
    ("tax_25_total_taxes_withheld", Kind::Money),
    ("tax_28_tax_remitted_previously", Kind::Money),
    ("tax_29_other_remittances_name", Kind::Text),
    ("tax_29_other_remittances_amount", Kind::Money),
    ("tax_32_surcharge", Kind::Money),
    ("tax_33_interest", Kind::Money),
    ("tax_34_compromise", Kind::Money),
];

const ROW_FIELDS_1601C: &[(&str, Kind)] = &[
    ("previous_month", Kind::Text),
    ("date_paid", Kind::Text),
    ("drawee_bank_code_or_agency", Kind::Text),
    ("payment_number", Kind::Text),
    ("tax_paid", Kind::Money),
    ("should_be_tax_due", Kind::Money),
];

/// `schedule_1.<i>.<field>` -> `(i, field)`.
fn parse_row_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("schedule_1.")?;
    let (index, field) = rest.split_once('.')?;
    Some((index.parse().ok()?, field))
}

fn row_key(index: usize, field: &str) -> String {
    format!("schedule_1.{index}.{field}")
}

/// Canonical 2551Q box key for `key` (legacy aliases and widget ids accepted).
pub fn canonical_2551q_key(key: &str) -> Option<String> {
    let key = key.trim();
    let alias = match key {
        "taxable_amount" | ids::FORM_2551Q_TAXABLE_0 => Some("schedule_1.0.taxable_amount"),
        ids::FORM_2551Q_CREDITABLE => Some("creditable_tax_withheld"),
        ids::FORM_2551Q_OTHER_CREDIT => Some("other_tax_credit"),
        _ => None,
    };
    if let Some(alias) = alias {
        return Some(alias.to_string());
    }
    if FIELDS_2551Q.iter().any(|(name, _)| *name == key) {
        return Some(key.to_string());
    }
    let (index, field) = parse_row_key(key)?;
    ROW_FIELDS_2551Q
        .contains(&field)
        .then(|| row_key(index, field))
}

/// Canonical 1601C box key for `key` (legacy aliases and widget ids accepted).
pub fn canonical_1601c_key(key: &str) -> Option<String> {
    let key = key.trim();
    let alias = match key {
        "tax_14" | ids::FORM_1601C_TAX_14 => Some("tax_14_total_compensation"),
        "tax_25" | ids::FORM_1601C_TAX_25 => Some("tax_25_total_taxes_withheld"),
        "sheets" | ids::FORM_1601C_SHEETS => Some("number_of_sheets"),
        ids::FORM_1601C_WITHHELD | "form-1601c-withheld" => Some("any_taxes_withheld"),
        ids::FORM_1601C_CATEGORY | "form-1601c-category" => Some("category_of_agent"),
        _ => None,
    };
    if let Some(alias) = alias {
        return Some(alias.to_string());
    }
    if FIELDS_1601C.iter().any(|(name, _)| *name == key) {
        return Some(key.to_string());
    }
    let (index, field) = parse_row_key(key)?;
    ROW_FIELDS_1601C
        .iter()
        .any(|(name, _)| *name == field)
        .then(|| row_key(index, field))
}

fn to_object<T: Serialize>(draft: &T) -> serde_json::Map<String, Value> {
    match serde_json::to_value(draft) {
        Ok(Value::Object(object)) => object,
        _ => serde_json::Map::new(),
    }
}

fn collect_values(
    object: &serde_json::Map<String, Value>,
    top: &[&str],
    rows: &[&str],
) -> BTreeMap<String, Value> {
    let mut values = BTreeMap::new();
    for key in top {
        values.insert(
            (*key).to_string(),
            object.get(*key).cloned().unwrap_or(Value::Null),
        );
    }
    if let Some(Value::Array(schedule)) = object.get("schedule_1") {
        for (index, row) in schedule.iter().enumerate() {
            for field in rows {
                values.insert(
                    row_key(index, field),
                    row.get(*field).cloned().unwrap_or(Value::Null),
                );
            }
        }
    }
    values
}

/// Every fillable 2551Q box as it stands -> its current JSON value.
pub fn fillable_values_2551q(draft: &Form2551QDraft) -> BTreeMap<String, Value> {
    let top: Vec<&str> = FIELDS_2551Q.iter().map(|(name, _)| *name).collect();
    collect_values(&to_object(draft), &top, ROW_FIELDS_2551Q)
}

/// Every fillable 1601C box as it stands -> its current JSON value.
pub fn fillable_values_1601c(draft: &Form1601CDraft) -> BTreeMap<String, Value> {
    let top: Vec<&str> = FIELDS_1601C.iter().map(|(name, _)| *name).collect();
    let rows: Vec<&str> = ROW_FIELDS_1601C.iter().map(|(name, _)| *name).collect();
    collect_values(&to_object(draft), &top, &rows)
}

/// Keys whose value differs between two snapshots (added/removed rows included).
pub fn changed_keys(
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
) -> Vec<String> {
    let mut changed: Vec<String> = after
        .iter()
        .filter(|(key, value)| before.get(*key) != Some(*value))
        .map(|(key, _)| key.clone())
        .collect();
    changed.extend(
        before
            .keys()
            .filter(|key| !after.contains_key(*key))
            .cloned(),
    );
    changed
}

pub fn parse_money(value: &Value) -> Result<f64, String> {
    let amount = match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => {
            let cleaned: String = text
                .trim()
                .chars()
                .filter(|c| *c != ',' && *c != '₱')
                .collect();
            cleaned.trim().parse::<f64>().ok()
        }
        _ => None,
    };
    match amount {
        Some(amount) if amount.is_finite() => Ok(amount),
        _ => Err(format!("invalid amount `{}`", value_text(value))),
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn parse_bool(value: &Value, key: &str) -> Result<bool, String> {
    let parsed = match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => match number.as_i64() {
            Some(1) => Some(true),
            Some(0) => Some(false),
            _ => None,
        },
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => Some(true),
            "false" | "no" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    };
    parsed.ok_or_else(|| format!("{key} must be true/false or Yes/No"))
}

pub fn parse_withheld_flag(value: &Value) -> Result<bool, String> {
    match value {
        Value::Bool(flag) => Ok(*flag),
        Value::Number(number) => match number.as_i64() {
            Some(1) => Ok(true),
            Some(0) => Ok(false),
            _ => Err(
                "any_taxes_withheld must be boolean true/false or Yes/No, not a non-0/1 number"
                    .into(),
            ),
        },
        Value::String(text) => parse_withheld_text(text),
        _ => Err("any_taxes_withheld must be boolean true/false or Yes/No".into()),
    }
}

pub fn parse_withheld_text(text: &str) -> Result<bool, String> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        other => Err(format!(
            "any_taxes_withheld must be boolean true/false or Yes/No, not `{other}`"
        )),
    }
}

pub fn parse_category_of_agent(value: &Value) -> Result<String, String> {
    match value {
        Value::Bool(true) => Ok("P".into()),
        Value::Bool(false) => Ok("G".into()),
        Value::Number(number) => match number.as_i64() {
            Some(1) => Ok("P".into()),
            Some(0) => Ok("G".into()),
            _ => Err(
                "category_of_agent must be P or G (private/government); boolean true/false maps to P/G, not a non-0/1 number"
                    .into(),
            ),
        },
        Value::String(text) => parse_category_of_agent_text(text),
        _ => Err(
            "category_of_agent must be P or G (private/government), or boolean true/false for P/G"
                .into(),
        ),
    }
}

pub fn parse_category_of_agent_text(text: &str) -> Result<String, String> {
    match text.trim().to_ascii_lowercase().as_str() {
        "p" | "private" | "true" | "yes" | "1" => Ok("P".into()),
        "g" | "government" | "false" | "no" | "0" => Ok("G".into()),
        other => Err(format!(
            "category_of_agent must be P or G (private/government), not `{other}`"
        )),
    }
}

fn coerce(kind: Kind, key: &str, value: &Value) -> Result<Value, String> {
    Ok(match kind {
        Kind::Money => Value::from(parse_money(value)?),
        Kind::Text => match value {
            Value::String(text) => Value::String(text.trim().to_string()),
            Value::Number(number) => Value::String(number.to_string()),
            _ => return Err(format!("{key} must be text")),
        },
        Kind::Bool => Value::Bool(parse_bool(value, key)?),
        Kind::Int => {
            let number = match value {
                Value::Number(number) => number.as_u64(),
                Value::String(text) => text.trim().parse::<u64>().ok(),
                _ => None,
            };
            Value::from(number.ok_or_else(|| format!("{key} must be a whole number"))?)
        }
        Kind::Enum(options) => {
            let text = match value {
                Value::String(text) => text.trim().to_ascii_lowercase().replace([' ', '-'], "_"),
                _ => String::new(),
            };
            if !options.contains(&text.as_str()) {
                return Err(format!("{key} must be one of {}", options.join(", ")));
            }
            Value::String(text)
        }
        Kind::Withheld => Value::Bool(parse_withheld_flag(value)?),
        Kind::Category => Value::String(parse_category_of_agent(value)?),
    })
}

/// Item 12A (2551Q) / 13A (1601C) is a select: store the official option
/// code (labels such as "Special Rate" are accepted), or clear it.
fn tax_relief_code(value: &Value, allow_both: bool) -> Result<Value, String> {
    let text = value_text(value);
    if text.trim().is_empty() {
        return Ok(Value::String(String::new()));
    }
    bir_core::validation::official_tax_relief_code(&text, allow_both)
        .map(Value::String)
        .ok_or_else(|| {
            if allow_both {
                "tax_relief_specification must be 1 (Special Rate), 2 (International Tax Treaty) or 3 (Both)".into()
            } else {
                "tax_relief_specification must be 1 (Special Rate) or 2 (International Tax Treaty)".into()
            }
        })
}

fn set_top_level<T: Serialize + DeserializeOwned>(
    draft: &mut T,
    key: &str,
    value: Value,
) -> Result<(), String> {
    let mut object = to_object(draft);
    object.insert(key.to_string(), value);
    *draft = serde_json::from_value(Value::Object(object))
        .map_err(|err| format!("invalid value for {key}: {err}"))?;
    Ok(())
}

/// Write one 2551Q box. `key` must be canonical. Does not recompute.
pub fn apply_2551q(draft: &mut Form2551QDraft, key: &str, value: &Value) -> Result<(), String> {
    if let Some((index, field)) = parse_row_key(key) {
        return apply_2551q_row(draft, index, field, value);
    }
    let kind = FIELDS_2551Q
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, kind)| *kind)
        .ok_or_else(|| format!("unknown or read-only 2551Q field `{key}`"))?;
    let value = if key == "tax_relief_specification" {
        tax_relief_code(value, false)?
    } else {
        coerce(kind, key, value)?
    };
    set_top_level(draft, key, value)?;
    if key == "tax_period_basis"
        && matches!(
            draft.tax_period_basis,
            bir_core::forms::form_2551q::TaxPeriodBasis::Calendar
        )
    {
        // Calendar filers end the year in December (the view enforces it too).
        draft.year_end_month = 12;
    }
    Ok(())
}

fn apply_2551q_row(
    draft: &mut Form2551QDraft,
    index: usize,
    field: &str,
    value: &Value,
) -> Result<(), String> {
    match field {
        "atc" => {
            let code = value_text(value).trim().to_ascii_uppercase();
            let entry = ATC_TABLE_2551Q
                .iter()
                .find(|entry| entry.code == code)
                .ok_or_else(|| format!("`{code}` is not a 2551Q ATC"))?;
            if draft
                .schedule_1
                .iter()
                .enumerate()
                .any(|(i, row)| i != index && row.atc.trim().eq_ignore_ascii_case(entry.code))
            {
                return Err(format!("ATC {code} is already on Schedule 1"));
            }
            let mut row = Schedule1Row::new(entry.code)
                .ok_or_else(|| format!("`{code}` is not a 2551Q ATC"))?;
            let rows = draft.schedule_1.len();
            if let Some(existing) = draft.schedule_1.get_mut(index) {
                row.taxable_amount = existing.taxable_amount;
                *existing = row;
            } else if index == rows && rows < ATC_TABLE_2551Q.len() {
                draft.schedule_1.push(row);
            } else {
                return Err(format!(
                    "schedule_1.{index} cannot be added; the next new row is schedule_1.{rows}"
                ));
            }
        }
        "taxable_amount" => {
            let amount = parse_money(value)?;
            let row = draft.schedule_1.get_mut(index).ok_or_else(|| {
                format!(
                    "2551Q schedule_1.{index} does not exist; fill schedule_1.{index}.atc first"
                )
            })?;
            row.taxable_amount = amount;
            row.recompute();
        }
        other => {
            return Err(format!(
                "unknown or read-only 2551Q field `schedule_1.{index}.{other}`"
            ));
        }
    }
    Ok(())
}

/// Write one 1601C box. `key` must be canonical. Does not recompute.
pub fn apply_1601c(draft: &mut Form1601CDraft, key: &str, value: &Value) -> Result<(), String> {
    if let Some((index, field)) = parse_row_key(key) {
        let kind = ROW_FIELDS_1601C
            .iter()
            .find(|(name, _)| *name == field)
            .map(|(_, kind)| *kind)
            .ok_or_else(|| format!("unknown or read-only 1601C field `{key}`"))?;
        if index == draft.schedule_1.len() && draft.schedule_1.len() < MAX_SCHEDULE_1_ROWS {
            draft.schedule_1.push(Form1601CSchedule1Row::default());
        }
        let rows = draft.schedule_1.len();
        let row = draft.schedule_1.get_mut(index).ok_or_else(|| {
            format!(
                "1601C schedule_1.{index} cannot be added (at most {MAX_SCHEDULE_1_ROWS} rows; the next new row is schedule_1.{rows})"
            )
        })?;
        return set_top_level(row, field, coerce(kind, key, value)?);
    }
    let kind = FIELDS_1601C
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, kind)| *kind)
        .ok_or_else(|| format!("unknown or read-only 1601C field `{key}`"))?;
    let value = if key == "tax_relief_specification" {
        tax_relief_code(value, true)?
    } else {
        coerce(kind, key, value)?
    };
    set_top_level(draft, key, value)
}

// ---------------------------------------------------------------- needs you

#[derive(Debug, Deserialize)]
struct FieldsDoc {
    fields: Vec<FieldEntry>,
}

#[derive(Debug, Deserialize)]
struct FieldEntry {
    field_key: String,
    label: String,
    required: String,
}

fn requirement_index(raw: &str) -> BTreeMap<String, (String, String)> {
    let doc: FieldsDoc =
        serde_json::from_str(raw).expect("rules/forms fields.json must stay parseable");
    doc.fields
        .into_iter()
        .map(|entry| (entry.field_key, (entry.required, entry.label)))
        .collect()
}

fn rules_2551q() -> &'static BTreeMap<String, (String, String)> {
    static INDEX: OnceLock<BTreeMap<String, (String, String)>> = OnceLock::new();
    INDEX.get_or_init(|| {
        requirement_index(include_str!(
            "../../../../rules/forms/2551q-v2018/fields.json"
        ))
    })
}

fn rules_1601c() -> &'static BTreeMap<String, (String, String)> {
    static INDEX: OnceLock<BTreeMap<String, (String, String)>> = OnceLock::new();
    INDEX.get_or_init(|| {
        requirement_index(include_str!(
            "../../../../rules/forms/1601c-v2018/fields.json"
        ))
    })
}

/// When a `conditional` box is required for this draft.
#[derive(Clone, Copy)]
enum When {
    Always,
    TaxRelief,
    TaxesWithheld,
}

/// How "empty" is judged for the draft box an official field maps to.
#[derive(Clone, Copy)]
enum Empty {
    Text,
    /// Official Validate requires a value greater than zero.
    PositiveMoney,
    NonZero,
}

/// Official field (rules/forms/<form>/fields.json) -> draft box.
struct Rule {
    official: &'static str,
    field: &'static str,
    label: Option<&'static str>,
    when: When,
    empty: Empty,
}

const fn rule(
    official: &'static str,
    field: &'static str,
    label: Option<&'static str>,
    when: When,
    empty: Empty,
) -> Rule {
    Rule {
        official,
        field,
        label,
        when,
        empty,
    }
}

const RULES_2551Q: &[Rule] = &[
    rule(
        "frm2551Qv2018:rtnMonth",
        "year_end_month",
        Some("Item 2 Year-end month"),
        When::Always,
        Empty::NonZero,
    ),
    rule(
        "frm2551Qv2018:txtYear",
        "taxable_year",
        Some("Item 2 Taxable year"),
        When::Always,
        Empty::NonZero,
    ),
    rule(
        "frm2551Qv2018:txtTIN1",
        "tin",
        Some("Item 6 TIN (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm2551Qv2018:txtRDOCode",
        "rdo_code",
        Some("Item 7 RDO code (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm2551Qv2018:registeredName",
        "taxpayer_name",
        Some("Item 8 Taxpayer's name (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm2551Qv2018:registeredAddress",
        "registered_address",
        Some("Item 9 Registered address (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm2551Qv2018:zipCode",
        "zip_code",
        Some("Item 9A ZIP code (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm2551Qv2018:telNo",
        "contact_number",
        Some("Item 10 Contact number (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm2551Qv2018:txtTaxReliefSpecify",
        "tax_relief_specification",
        Some("Item 12A Tax relief specification"),
        When::TaxRelief,
        Empty::Text,
    ),
];

const RULES_1601C: &[Rule] = &[
    rule(
        "frm1601c:txtMonth",
        "month",
        None,
        When::Always,
        Empty::NonZero,
    ),
    rule(
        "frm1601c:txtYear",
        "taxable_year",
        None,
        When::Always,
        Empty::NonZero,
    ),
    rule(
        "frm1601c:txtTIN1",
        "tin",
        Some("Item 6 TIN (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm1601c:txtRDOCode",
        "rdo_code",
        Some("Item 7 RDO code (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm1601c:txtTaxpayerName",
        "taxpayer_name",
        Some("Item 8 Withholding agent name (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm1601c:txtAddress",
        "registered_address",
        Some("Item 9 Registered address (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm1601c:txtZipCode",
        "zip_code",
        Some("Item 9A ZIP code (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm1601c:txtTelNum",
        "contact_number",
        Some("Item 10 Telephone number (taxpayer profile)"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm1601c:CatAgent_P",
        "category_of_agent",
        Some("Item 11 Category of withholding agent"),
        When::Always,
        Empty::Text,
    ),
    rule(
        "frm1601c:selTreaty",
        "tax_relief_specification",
        Some("Item 13A Tax relief specification"),
        When::TaxRelief,
        Empty::Text,
    ),
    rule(
        "frm1601c:txtTax14",
        "tax_14_total_compensation",
        Some("Item 14 Total amount of compensation"),
        When::TaxesWithheld,
        Empty::PositiveMoney,
    ),
    rule(
        "frm1601c:txtTax25",
        "tax_25_total_taxes_withheld",
        Some("Item 25 Total taxes withheld"),
        When::TaxesWithheld,
        Empty::PositiveMoney,
    ),
];

/// Schedule I row boxes, required for every row that exists
/// (`frm1601c:sched1:*{N>=0}`: "the corresponding row exists").
const RULES_1601C_ROWS: &[(&str, &str)] = &[
    ("frm1601c:sched1:txtMonthYear{N>=0}", "previous_month"),
    ("frm1601c:sched1:txtDatePaid{N>=0}", "date_paid"),
    (
        "frm1601c:sched1:txtBankCode{N>=0}",
        "drawee_bank_code_or_agency",
    ),
    ("frm1601c:sched1:txtNumber{N>=0}", "payment_number"),
];

fn is_empty(object: &serde_json::Map<String, Value>, field: &str, empty: Empty) -> bool {
    let value = object.get(field).unwrap_or(&Value::Null);
    match empty {
        Empty::Text => value.as_str().is_none_or(|text| text.trim().is_empty()),
        Empty::PositiveMoney => value.as_f64().is_none_or(|amount| amount <= 0.0),
        Empty::NonZero => value.as_f64().is_none_or(|number| number == 0.0),
    }
}

fn rule_applies(
    index: &BTreeMap<String, (String, String)>,
    rule: &Rule,
    condition_holds: bool,
) -> Option<String> {
    let (required, label) = index.get(rule.official)?;
    let applies = match required.as_str() {
        "required" => true,
        "conditional" => condition_holds,
        _ => false,
    };
    applies.then(|| {
        rule.label
            .map(str::to_string)
            .unwrap_or_else(|| label.clone())
    })
}

fn push_box(boxes: &mut Vec<NeedsYouBox>, field: String, label: String) {
    if !boxes.iter().any(|existing| existing.field == field) {
        boxes.push(NeedsYouBox { field, label });
    }
}

/// Required (or conditionally required) 2551Q boxes that are empty.
///
/// Item 13 is added on top of fields.json (where its radios are optional):
/// this draft refuses to validate while it is unanswered, and only the user
/// or a stored election may answer it.
pub fn needs_you_2551q(draft: &Form2551QDraft) -> Vec<NeedsYouBox> {
    let index = rules_2551q();
    let object = to_object(draft);
    let mut boxes = Vec::new();
    for rule in RULES_2551Q {
        let condition = match rule.when {
            When::Always => true,
            When::TaxRelief => draft.tax_relief,
            When::TaxesWithheld => false,
        };
        if let Some(label) = rule_applies(index, rule, condition)
            && is_empty(&object, rule.field, rule.empty)
        {
            push_box(&mut boxes, rule.field.to_string(), label);
        }
    }
    if draft.item_13_election == Item13Election::Unanswered {
        push_box(
            &mut boxes,
            "item_13_election".into(),
            "Item 13 Income-tax-rate election (never inferred; the user answers it)".into(),
        );
    }
    boxes
}

/// Required (or conditionally required) 1601C boxes that are empty.
pub fn needs_you_1601c(draft: &Form1601CDraft) -> Vec<NeedsYouBox> {
    let index = rules_1601c();
    let object = to_object(draft);
    let mut boxes = Vec::new();
    for rule in RULES_1601C {
        let condition = match rule.when {
            When::Always => true,
            When::TaxRelief => draft.tax_relief,
            When::TaxesWithheld => draft.any_taxes_withheld,
        };
        if let Some(label) = rule_applies(index, rule, condition)
            && is_empty(&object, rule.field, rule.empty)
        {
            push_box(&mut boxes, rule.field.to_string(), label);
        }
    }
    for (row_index, row) in draft.schedule_1.iter().enumerate() {
        let row_object = to_object(row);
        for (official, field) in RULES_1601C_ROWS {
            let Some((required, label)) = index.get(*official) else {
                continue;
            };
            if required == "conditional" && is_empty(&row_object, field, Empty::Text) {
                push_box(
                    &mut boxes,
                    row_key(row_index, field),
                    label.replace("row N", &format!("row {row_index}")),
                );
            }
        }
    }
    boxes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_needs_you_rule_names_a_real_official_field() {
        for rule in RULES_2551Q {
            assert!(
                rules_2551q().contains_key(rule.official),
                "{}",
                rule.official
            );
        }
        for rule in RULES_1601C {
            assert!(
                rules_1601c().contains_key(rule.official),
                "{}",
                rule.official
            );
        }
        for (official, _) in RULES_1601C_ROWS {
            assert!(rules_1601c().contains_key(*official), "{official}");
        }
    }

    #[test]
    fn aliases_resolve_to_serde_field_names() {
        assert_eq!(
            canonical_1601c_key("tax_14").as_deref(),
            Some("tax_14_total_compensation")
        );
        assert_eq!(
            canonical_2551q_key("taxable_amount").as_deref(),
            Some("schedule_1.0.taxable_amount")
        );
        assert_eq!(
            canonical_1601c_key("schedule_1.2.tax_paid").as_deref(),
            Some("schedule_1.2.tax_paid")
        );
        assert!(canonical_2551q_key("total_tax_due").is_none());
        assert!(canonical_2551q_key("item_13_election").is_none());
        assert!(canonical_1601c_key("tax_21_total_non_taxable").is_none());
        assert!(canonical_1601c_key("taxpayer_name").is_none());
    }
}
