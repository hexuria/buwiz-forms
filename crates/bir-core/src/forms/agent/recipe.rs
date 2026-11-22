//! A gpui-agent recipe generated for one taxpayer and one form.
//!
//! The form's box definitions, the taxpayer's template and their previous
//! return decide what the recipe holds. An answer the template pins is a
//! constant. A box the user answers each period (an amount, a date, an
//! election, a stop) is a typed parameter. A box that carries but is not
//! pinned is a parameter whose default is the last answer given. Profile,
//! identity and computed boxes are not written at all, and a box the
//! earlier returns all left blank is left out. An election is required
//! once an earlier return answered it, and is never given a default.
//!
//! The recipe is plain JSON in gpui-agent recipe format v1 with typed
//! params; `input_schema` is the JSON Schema of those params.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::carry::{self, Carry};
use super::{AgentFormDef, AgentFormKind, Binding, BoxDef, DateFormat, parse_amount};
use crate::tax_profile::templates::FormTemplate;

/// The stamp of "no template".
pub const NO_TEMPLATE: &str = "none";

/// When the recipe's dry run (`form.check`) stops the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckMode {
    /// Stop before any write when a value is refused or the return would
    /// not validate.
    #[default]
    Strict,
    /// Run the check and report, but go on.
    Report,
    /// No check step.
    Off,
}

impl CheckMode {
    pub fn parse(raw: Option<&str>) -> Result<Self, String> {
        match raw.map(str::trim) {
            None | Some("") | Some("strict") => Ok(Self::Strict),
            Some("report") => Ok(Self::Report),
            Some("off") | Some("none") => Ok(Self::Off),
            Some(other) => Err(format!(
                "unknown check `{other}`; use strict, report or off"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub check: CheckMode,
    /// Keep boxes every earlier return left blank as optional params.
    pub include_unused: bool,
    /// Leave the return open in the app after the run instead of saving
    /// and closing it.
    pub keep_open: bool,
}

/// What a generated recipe was built from.
pub struct Inputs<'a> {
    pub kind: AgentFormKind,
    pub tin: &'a str,
    pub template: Option<&'a FormTemplate>,
    /// Stored drafts of the earlier returns, newest first.
    pub previous: &'a [Value],
    pub options: Options,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Generated {
    pub recipe: Value,
    pub input_schema: Value,
    pub template_stamp: String,
    /// Box key -> pinned value written as a constant.
    pub constants: BTreeMap<String, String>,
    /// Param name -> box key.
    pub params: BTreeMap<String, String>,
    /// Box key -> why the recipe does not write it.
    pub left_out: BTreeMap<String, &'static str>,
}

/// A value's type as a recipe param and JSON Schema property.
#[derive(Debug, Clone, PartialEq)]
enum Shape {
    Text,
    Amount,
    Whole,
    Flag,
    Choice(Vec<String>),
    Date(DateFormat),
}

const AMOUNT_PATTERN: &str = r"^\(?-?[0-9][0-9,]*(\.[0-9]+)?\)?$";

impl Shape {
    fn of(def: &AgentFormDef, entry: &BoxDef) -> Self {
        let bound = def.bindings.get(&entry.key).map(|bound| &bound.binding);
        // What the writer matches is the binding's own values; the official
        // option values can differ from them (1701MS civil status).
        let mut values: Vec<String> = match bound {
            Some(Binding::Choice { options }) => {
                options.iter().map(|option| option.value.clone()).collect()
            }
            _ => entry
                .options
                .iter()
                .map(|option| option.value.clone())
                .collect(),
        };
        values.retain(|value| !value.is_empty());
        if !values.is_empty() {
            if values
                .iter()
                .all(|value| value == "true" || value == "false")
            {
                return Self::Flag;
            }
            return Self::Choice(values);
        }
        match bound {
            Some(Binding::Amount { .. }) => Self::Amount,
            Some(Binding::Whole { .. }) => Self::Whole,
            Some(Binding::Choice { .. }) => Self::Flag,
            Some(Binding::Text {
                date: Some(format),
                part: None,
                ..
            }) => Self::Date(*format),
            _ => Self::Text,
        }
    }

    fn pattern(&self) -> Option<&'static str> {
        match self {
            Self::Amount => Some(AMOUNT_PATTERN),
            Self::Date(DateFormat::Iso) => Some(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$"),
            Self::Date(DateFormat::Mdy) => Some(r"^[0-9]{2}/[0-9]{2}/[0-9]{4}$"),
            Self::Date(DateFormat::MonthYear) => Some(r"^[0-9]{2}/[0-9]{4}$"),
            _ => None,
        }
    }

    fn param_type(&self) -> &'static str {
        match self {
            Self::Whole => "integer",
            Self::Flag => "bool",
            _ => "string",
        }
    }

    /// Whether `value` passes the param this shape declares, so a default
    /// taken from history never makes the recipe invalid.
    fn accepts(&self, value: &str) -> bool {
        let digits = |text: &str, len: usize| {
            text.len() == len && text.bytes().all(|byte| byte.is_ascii_digit())
        };
        match self {
            Self::Text => true,
            Self::Amount => {
                parse_amount(value).is_some()
                    && value.bytes().all(|byte| {
                        byte.is_ascii_digit() || matches!(byte, b',' | b'.' | b'-' | b'(' | b')')
                    })
                    && value
                        .trim_start_matches(['(', '-'])
                        .starts_with(|c: char| c.is_ascii_digit())
            }
            Self::Whole => value.parse::<i64>().is_ok(),
            Self::Flag => value == "true" || value == "false",
            Self::Choice(values) => values.iter().any(|allowed| allowed == value),
            Self::Date(format) => {
                let parts: Vec<&str> = match format {
                    DateFormat::Iso => value.split('-').collect(),
                    _ => value.split('/').collect(),
                };
                match (format, parts.as_slice()) {
                    (DateFormat::Iso, [y, m, d]) => digits(y, 4) && digits(m, 2) && digits(d, 2),
                    (DateFormat::Mdy, [m, d, y]) => digits(y, 4) && digits(m, 2) && digits(d, 2),
                    (DateFormat::MonthYear, [m, y]) => digits(y, 4) && digits(m, 2),
                    _ => false,
                }
            }
        }
    }

    fn param(&self, name: &str, description: String) -> Map<String, Value> {
        let mut param = Map::new();
        param.insert("name".into(), json!(name));
        if self.param_type() != "string" {
            param.insert("type".into(), json!(self.param_type()));
        }
        if let Self::Choice(values) = self {
            param.insert("enum".into(), json!(values));
        }
        if let Some(pattern) = self.pattern() {
            param.insert("pattern".into(), json!(pattern));
        }
        param.insert("description".into(), json!(description));
        param
    }

    fn property(&self, description: &str) -> Map<String, Value> {
        let mut property = Map::new();
        property.insert(
            "type".into(),
            json!(match self {
                Self::Whole => "integer",
                Self::Flag => "boolean",
                _ => "string",
            }),
        );
        if let Self::Choice(values) = self {
            property.insert("enum".into(), json!(values));
        }
        if let Some(pattern) = self.pattern() {
            property.insert("pattern".into(), json!(pattern));
        }
        property.insert("description".into(), json!(description));
        property
    }

    fn typed(&self, value: &str) -> Value {
        match self {
            Self::Whole => value
                .parse::<i64>()
                .map_or_else(|_| json!(value), Value::from),
            Self::Flag => json!(value == "true"),
            _ => json!(value),
        }
    }
}

fn carry_name(carry: Carry) -> &'static str {
    match carry {
        Carry::Carries => "carries",
        Carry::NotAnAnswer => "not_an_answer",
        Carry::Computed => "computed",
        Carry::Locked => "locked",
        Carry::Election => "election",
        Carry::Fact => "fact",
        Carry::OneOff => "one_off",
        Carry::Penalty => "penalty",
        Carry::Amount => "amount",
        Carry::Date => "date",
        Carry::Period => "period",
        Carry::Amended => "amended",
        Carry::LeftOpen => "left_open",
        Carry::RowEntry => "row_entry",
    }
}

fn describe(entry: &BoxDef) -> String {
    let label = entry.label.trim();
    match (entry.item.trim(), label) {
        ("", "") => entry.key.clone(),
        ("", label) => label.to_string(),
        (item, "") => format!("Item {item}"),
        (item, label) => format!("Item {item}: {label}"),
    }
}

/// A recipe param name for a box key: its control id in lowercase, made
/// an identifier, unique within the recipe.
fn param_name(key: &str, taken: &BTreeSet<String>) -> String {
    let id = key.rsplit(':').next().unwrap_or(key);
    let mut name: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if !name.starts_with(|c: char| c.is_ascii_lowercase()) {
        name.insert_str(0, "box_");
    }
    if !taken.contains(&name) {
        return name;
    }
    (2..)
        .map(|n| format!("{name}_{n}"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or(name)
}

/// A literal in a recipe: `$` is the placeholder sigil.
fn literal(value: &str) -> String {
    value.replace('$', "$$")
}

/// FNV-1a 64 of a template's id and boxes: changes whenever a pin or a
/// stop switch does, so a recipe generated from an older template stops.
pub fn template_stamp(template: Option<&FormTemplate>) -> String {
    let Some(template) = template else {
        return NO_TEMPLATE.into();
    };
    let fields = serde_json::to_string(&template.fields).unwrap_or_default();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in template.form_code.bytes().chain([0]).chain(fields.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{}-{hash:016x}", template.id)
}

/// Every question box of a form: what it is, whether the assistant may
/// write it, and the shape of a valid value.
pub fn form_schema(kind: AgentFormKind) -> Value {
    let def = kind.definition();
    let boxes: Vec<Value> = def
        .boxes
        .iter()
        .filter(|entry| entry.is_question())
        .map(|entry| {
            let shape = Shape::of(def, entry);
            let refusal = super::fill_refusal(kind, &entry.key);
            let mut out = Map::new();
            out.insert("key".into(), json!(entry.key));
            out.insert("item".into(), json!(entry.item));
            out.insert("label".into(), json!(entry.label));
            out.insert("kind".into(), json!(entry.kind));
            out.insert("required".into(), json!(entry.required));
            out.insert(
                "carry".into(),
                json!(carry_name(carry::carry_of(kind, &entry.key))),
            );
            out.insert("fillable".into(), json!(refusal.is_none()));
            if let Some(why) = refusal {
                out.insert("refusal".into(), json!(why));
            }
            if !entry.options.is_empty() {
                out.insert("options".into(), json!(entry.options));
            }
            out.insert(
                "value".into(),
                Value::Object(shape.property(&describe(entry))),
            );
            Value::Object(out)
        })
        .collect();
    json!({
        "form": kind.form_code(),
        "package": def.package,
        "boxes": boxes,
        "computed": def.boxes.iter().filter(|entry| entry.computed).map(|entry| &entry.key).collect::<Vec<_>>(),
    })
}

/// Boxes the form's own validation asks for only under another answer,
/// whatever their metadata or the history says: 1701Q Items 16A and 25A
/// (the filer's and the spouse's method of deduction) apply under graduated
/// rates only, and the form clears them under the 8% rate.
fn depends_on_another_answer(kind: AgentFormKind, key: &str) -> bool {
    matches!(
        (kind.form_code(), key),
        (
            "1701Q",
            "frm1701q:optMethodOfDeduction" | "frm1701q:optSpouseMethod"
        )
    )
}

/// The recipe for `inputs`. See the module docs for the rules.
pub fn generate(inputs: &Inputs<'_>) -> Result<Generated, String> {
    let kind = inputs.kind;
    let def = kind.definition();
    let code = kind.form_code();
    let pinned = inputs
        .template
        .map(FormTemplate::pinned)
        .unwrap_or_default();
    let stops = inputs
        .template
        .map(FormTemplate::stop_edits)
        .unwrap_or_default();
    let history = inputs
        .previous
        .iter()
        .map(|draft| super::field_map(kind, draft))
        .collect::<Result<Vec<_>, _>>()?;
    let stamp = template_stamp(inputs.template);

    let mut taken: BTreeSet<String> = ["year", "period"].map(String::from).into();
    let mut params = vec![
        json!({"name": "year", "type": "integer", "description": "Taxable year of the return"}),
        json!({"name": "period", "type": "integer", "description": "Month, quarter or entry number; 0 for an annual return"}),
    ];
    let mut properties = Map::new();
    properties.insert(
        "year".into(),
        json!({"type": "integer", "description": "Taxable year of the return"}),
    );
    properties.insert(
        "period".into(),
        json!({"type": "integer", "description": "Month, quarter or entry number; 0 for an annual return"}),
    );
    let mut required = vec![json!("year"), json!("period")];
    let mut fields = Map::new();
    let mut constants = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut left_out = BTreeMap::new();
    // Stopped boxes the form asks for only under another answer: their
    // stored answer is dropped, so the form asks again when they apply.
    let mut ask = Vec::new();

    for entry in def.boxes.iter().filter(|entry| entry.is_question()) {
        let key = &entry.key;
        let carry = carry::carry_of(kind, key);
        if super::fill_refusal(kind, key).is_some() {
            left_out.insert(
                key.clone(),
                match carry {
                    Carry::Computed => "computed",
                    Carry::Election => "election the user answers in the app",
                    _ => "from the profile or not writable",
                },
            );
            continue;
        }
        let shape = Shape::of(def, entry);
        let stop = stops.get(key).copied();
        if stop == Some(true) && depends_on_another_answer(kind, key) {
            ask.push(key.clone());
        }
        if stop != Some(true)
            && let Some(value) = pinned.get(key)
            && !carry::is_blank_value(value)
        {
            fields.insert(key.clone(), json!(literal(value)));
            constants.insert(key.clone(), value.clone());
            continue;
        }
        // The newest earlier return that answered the box.
        let last = history
            .iter()
            .map(|map| def.box_value(key, map))
            .find(|value| !carry::is_blank_value(value));
        // A box the form asks for only under another answer is never
        // required of the caller: validation asks for it when it applies.
        let must = !depends_on_another_answer(kind, key)
            && (entry.required == "required"
                || stop == Some(true)
                || (carry == Carry::Election && last.is_some()));
        if !must
            && !inputs.options.include_unused
            && carry != Carry::Election
            && !history.is_empty()
            && last.is_none()
            && inputs
                .template
                .is_none_or(|template| !template.fields.contains_key(key))
        {
            left_out.insert(key.clone(), "blank in every earlier return");
            continue;
        }
        let default = (carry == Carry::Carries && stop != Some(true))
            .then_some(last)
            .flatten()
            .filter(|value| shape.accepts(value));
        let name = param_name(key, &taken);
        taken.insert(name.clone());
        let description = describe(entry);
        let mut param = shape.param(&name, description.clone());
        let mut property = shape.property(&description);
        property.insert("x-box".into(), json!(key));
        property.insert("x-carry".into(), json!(carry_name(carry)));
        match &default {
            Some(value) => {
                param.insert("default".into(), shape.typed(value));
                property.insert("default".into(), shape.typed(value));
            }
            None if must => required.push(json!(name)),
            None => {
                param.insert("required".into(), json!(false));
            }
        }
        params.push(Value::Object(param));
        properties.insert(name.clone(), Value::Object(property));
        fields.insert(key.clone(), json!(format!("${name}")));
        names.insert(name, key.clone());
    }

    let fields = Value::Object(fields);
    let mut steps = vec![
        json!({"id": "select", "op": "invoke", "name": "profile.set", "args": {"tin": inputs.tin}}),
        json!({
            "id": "template", "op": "invoke", "name": "template.get",
            "args": {"code": code, "tin": inputs.tin, "expect_stamp": stamp},
            "needs": ["select"],
        }),
    ];
    let mut before_open = "template";
    if inputs.options.check != CheckMode::Off {
        steps.push(json!({
            "id": "check", "op": "invoke", "name": "form.check",
            "args": {
                "code": code, "year": "$year", "period": "$period", "tin": inputs.tin,
                "fields": fields,
                // The return `form.open` will open: a draft already stored
                // for the period, not a fresh one.
                "stored": true,
                "strict": inputs.options.check == CheckMode::Strict,
                "ask": ask,
            },
            "needs": ["template"],
        }));
        before_open = "check";
    }
    steps.push(json!({
        "id": "open", "op": "invoke", "name": "form.open",
        "args": {"code": code, "year": "$year", "period": "$period", "tin": inputs.tin},
        "needs": [before_open],
    }));
    steps.push(json!({
        "id": "fill", "op": "invoke", "name": "form.fill",
        "args": {"fields": fields, "source": "document", "ask": ask},
        "needs": ["open"],
    }));
    steps.push(json!({"id": "readback", "op": "invoke", "name": "form.fields", "needs": ["fill"]}));
    steps.push(
        json!({"id": "validate", "op": "invoke", "name": "form.validate", "needs": ["readback"]}),
    );
    if !inputs.options.keep_open {
        steps.push(
            json!({"id": "save", "op": "invoke", "name": "form.save_draft", "needs": ["validate"]}),
        );
        steps.push(
            json!({"id": "close", "op": "invoke", "name": "form.dismiss", "needs": ["save"]}),
        );
    }

    let recipe = json!({
        "v": 1,
        "name": format!("bir-{}-{}", code.to_ascii_lowercase(), inputs.tin),
        "app": "bir-shell",
        "params": params,
        "steps": steps,
    });
    let input_schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": format!("Form {code} for {}", inputs.tin),
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
        "required": required,
    });
    Ok(Generated {
        recipe,
        input_schema,
        template_stamp: stamp,
        constants,
        params: names,
        left_out,
    })
}
