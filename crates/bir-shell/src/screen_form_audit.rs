//! Runtime audit of the on-screen form page against the print preview.
//!
//! `audit_all_screen_forms` (ignored; run it on demand) opens every one of
//! the 43 forms for a fictional taxpayer whose profile has every field
//! filled, the way `screen_form_open` does, builds the print field map the
//! way `form_print_html` does (no browser), and compares the two per schema
//! control `k`. It writes `tmp/screen-form-audit/runtime.json` and
//! `runtime.md` at the repository root.
//!
//! ```sh
//! cargo test --locked --manifest-path crates/bir-shell/Cargo.toml \
//!     audit_all_screen_forms -- --ignored --nocapture
//! ```
//!
//! Finding classes:
//! - (i) identity/profile fields empty on the form page while print shows
//!   the profile value, or empty in both although the profile has it;
//! - (ii) form value != print value for the same control, after open and
//!   after a fill -> save -> reopen round trip;
//! - (iii) schema keys the agent definition does not list at all;
//! - (iv) validation errors no schema control can show;
//! - (v) controls the user can never fill (no binding, not computed, not a
//!   profile field) that the print sheet nevertheless prints.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use gpui_shell::{HostArguments, HostError, HostValue};
use serde_json::{Value, json};

use super::*;
use crate::bir_module::{module, print_fields, print_slug_for, use_ephemeral_database};

const TIN: &str = "12345678800000";
const YEAR: i64 = 2026;

/// Every profile fact the form constructors read, filled.
fn profile_json() -> Value {
    json!({
        "id": null,
        "full_name": "DELA CRUZ, JUAN SANTOS",
        "tin": {"segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000"},
        "rdo_code": "039",
        "line_of_business": "Retail of school supplies",
        "registered_address": "12 Example Street, Barangay Sample, Quezon City",
        "zip_code": "1100",
        "phone": "0281234567",
        "email": "juan.example@example.com",
        "default_form_type": "2551Q",
        "taxpayer_type": "Individual",
        "is_vat_registered": false,
        "business_start_date": "2015-01-02",
        "birth_date": "1980-05-17",
        "tax_classification": "SelfEmployed",
        "eopt_tier": "Micro",
        "is_expanded_withholding_agent": true,
        "atc_codes": ["PT010"],
        "has_employees": true,
        "withholds_compensation": true,
        "withholds_expanded": true,
        "withholds_final": true,
    })
}

/// What the profile holds for each identity category (normalized).
fn profile_expectations() -> BTreeMap<&'static str, Vec<&'static str>> {
    BTreeMap::from([
        (
            "tin",
            vec!["123", "456", "788", "00000", "000", "123456788"],
        ),
        ("rdo", vec!["039"]),
        ("name", vec!["DELA CRUZ", "JUAN"]),
        (
            "address",
            vec!["EXAMPLE STREET", "QUEZON CITY", "BARANGAY SAMPLE"],
        ),
        ("zip", vec!["1100"]),
        ("phone", vec!["0281234567", "281234567"]),
        ("email", vec!["JUAN.EXAMPLE@EXAMPLE.COM"]),
        ("line_of_business", vec!["RETAIL OF SCHOOL SUPPLIES"]),
        ("birth_date", vec!["1980", "05/17", "0517"]),
    ])
}

// ------------------------------------------------------------ host calls

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}

fn call(name: &str, values: Vec<HostValue>) -> Result<Value, HostError> {
    module()
        .call(name, &HostArguments::new(values))
        .map(|value| to_json(&value))
}

fn call_async(name: &str, values: Vec<HostValue>) -> Result<Value, HostError> {
    let future = module().begin(name, &HostArguments::new(values))?;
    block_on(future).map(|value| to_json(&value))
}

fn open(code: &str, period: i64) -> Result<Value, String> {
    call_async(
        "screen_form_open",
        vec![TIN.into(), code.into(), YEAR.into(), period.into()],
    )
    .map_err(|error| error.message().to_string())
}

fn string_map(value: &Value) -> BTreeMap<String, String> {
    value
        .as_object()
        .into_iter()
        .flatten()
        .map(|(key, value)| (key.clone(), value.as_str().unwrap_or_default().to_string()))
        .collect()
}

/// The print field map of a draft as it stands: the function
/// `form_print_html` calls on the stored draft.
fn print_map_of(kind: AgentFormKind, draft: &Value) -> Result<BTreeMap<String, String>, String> {
    use bir_core::forms::queueable::QueueableForm as _;
    Ok(match kind {
        AgentFormKind::Form1601C => {
            serde_json::from_value::<bir_core::forms::Form1601CDraft>(draft.clone())
                .map_err(|error| error.to_string())?
                .to_print_field_map()
        }
        AgentFormKind::Form2551Q => {
            serde_json::from_value::<bir_core::forms::form_2551q::Form2551QDraft>(draft.clone())
                .map_err(|error| error.to_string())?
                .to_print_field_map()
        }
        AgentFormKind::Queueable(queueable) => bir_core::with_queueable_kind!(queueable, F => {
            serde_json::from_value::<F>(draft.clone())
                .map_err(|error| error.to_string())?
                .print_field_map()
        }),
    })
}

// ---------------------------------------------------------------- schema

struct SchemaField {
    key: String,
    item: String,
    label: String,
    page: i64,
    kind: String,
    auto: bool,
    computed: bool,
}

fn schema_fields(code: &str) -> Vec<SchemaField> {
    let Some(form) = schema_entry(code) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for part in form["parts"].as_array().into_iter().flatten() {
        for item in part["items"].as_array().into_iter().flatten() {
            let item_label = format!(
                "{} {}",
                item["no"].as_str().unwrap_or_default(),
                item["l"].as_str().unwrap_or_default()
            )
            .trim()
            .to_string();
            let page = item["p"].as_i64().unwrap_or(0);
            for field in item["f"].as_array().into_iter().flatten() {
                let Some(key) = field["k"].as_str() else {
                    continue;
                };
                out.push(SchemaField {
                    key: key.to_string(),
                    item: item_label.clone(),
                    label: field["l"].as_str().unwrap_or_default().to_string(),
                    page,
                    kind: field["t"].as_str().unwrap_or_default().to_string(),
                    auto: field["a"].as_i64() == Some(1),
                    computed: field["c"].as_i64() == Some(1),
                });
            }
            for (r, row) in item["keys"].as_array().into_iter().flatten().enumerate() {
                for (c, key) in row.as_array().into_iter().flatten().enumerate() {
                    let Some(key) = key.as_str() else {
                        continue;
                    };
                    let column = item["cols"]
                        .as_array()
                        .and_then(|cols| cols.get(c))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    out.push(SchemaField {
                        key: key.to_string(),
                        item: item_label.clone(),
                        label: format!("row {} {column}", r + 1).trim().to_string(),
                        page,
                        kind: "cell".to_string(),
                        auto: false,
                        computed: false,
                    });
                }
            }
        }
    }
    out
}

/// `txtEmail#occurrence-2` -> `txtEmail`: the official serialized key.
fn serialized(key: &str) -> &str {
    key.split_once("#occurrence-").map_or(key, |(base, _)| base)
}

/// The identity/profile category of a control, from its key (schema item
/// labels are too often a neighbour's to trust) with the item label as a
/// fallback for fields the schema marks auto-filled; `None` for anything
/// else, including other parties (spouse, payee, schedule rows, stock
/// issuers), UI masks and row-family patterns.
fn identity_category(field: &SchemaField) -> Option<&'static str> {
    let key = field.key.to_lowercase();
    const NOT_THE_FILER: &[&str] = &[
        "{",
        "mask",
        "ebironline",
        "other",
        "sched",
        "spouse",
        "admin",
        "stock",
        "printedname",
        "miller",
        "withholdingagent",
        "buyer",
        "seller",
        "payee",
        "heir",
        "donee",
        "donor",
        "treaty",
        "relief",
        "trans",
        "currentpage",
        "maxpage",
        "particular",
        "signatory",
        "pg2sc",
        "tins",
        "tinb",
        "rdocodes",
    ];
    if NOT_THE_FILER.iter().any(|word| key.contains(word)) {
        return None;
    }
    let text = format!("{} {}", field.item, field.label).to_lowercase();
    let ends_tin = [
        "tin", "tin1", "tin2", "tin3", "tin4", "tina", "tinb", "tinc",
    ]
    .iter()
    .any(|end| key.ends_with(end));
    if key.contains("email") {
        Some("email")
    } else if key.contains("zip") {
        Some("zip")
    } else if field.key.contains("RDO") || field.key.contains("Rdo") || key.contains("rdocode") {
        Some("rdo")
    } else if key.contains("lob") || key.contains("lineofbusiness") {
        Some("line_of_business")
    } else if key.contains("birth") || key.contains("dob") {
        Some("birth_date")
    } else if key.contains("tel") || key.contains("phone") || key.contains("contactn") {
        Some("phone")
    } else if ends_tin || key.contains("txttin") || key.contains("branchcode") {
        Some("tin")
    } else if key.contains("address") || key.contains("addr") {
        Some("address")
    } else if key.contains("name")
        && ["taxpayer", "registered", "agent", "profile", "filer", "pg2"]
            .iter()
            .any(|word| key.contains(word))
    {
        Some("name")
    } else if field.auto && text.contains("taxpayer identification number") {
        Some("tin")
    } else if field.auto && text.contains("name") && !text.contains("spouse") {
        Some("name")
    } else {
        None
    }
}

/// `...Name2`, `...Address3`: a continuation line, blank when line 1 holds
/// the whole value.
fn continuation_line(field: &SchemaField) -> bool {
    field.key.ends_with('2') || field.key.ends_with('3') || field.label.contains("line 2")
}

fn norm_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

/// `same`, `format` (equal once case, spacing and number format are
/// ignored) or `value`.
fn compare(form: &str, print: &str) -> &'static str {
    if form == print {
        return "same";
    }
    let numbers = (agent::parse_amount(form), agent::parse_amount(print));
    if let (Some(a), Some(b)) = numbers
        && ((a - b).abs() < 0.005
            || ((a - b).abs() < 1.0 && (a.fract() == 0.0 || b.fract() == 0.0)))
    {
        // Equal, or one side rounded to whole pesos.
        return "format";
    }
    // An empty or zero amount on one side and the other blank.
    let blankish = |s: &str| s.trim().is_empty() || agent::parse_amount(s) == Some(0.0);
    if blankish(form) && blankish(print) {
        return "format";
    }
    if norm_text(form) == norm_text(print) {
        return "format";
    }
    "value"
}

// ----------------------------------------------------------- print sheet

/// `writer-cells.json` `not_joined`: writer key -> reviewed reason the sheet
/// prints it nowhere.
fn not_joined(slug: &str) -> BTreeMap<String, String> {
    let path = format!(
        "{}/../../html-frozen/{slug}/writer-cells.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return BTreeMap::new();
    };
    let payload: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    let mut out = BTreeMap::new();
    for row in payload["not_joined"].as_array().into_iter().flatten() {
        let reason = row["reason"].as_str().unwrap_or_default().to_string();
        let keys: Vec<String> = match row["writer_key"].as_str() {
            Some(key) => vec![key.to_string()],
            None => row["writer_keys"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|key| key.as_str().map(str::to_string))
                .collect(),
        };
        for key in keys {
            out.insert(key, reason.clone());
        }
    }
    out
}

/// Which writer keys a form's print sheet prints (a stamped `name=` or a
/// writer-cells join), as opposed to a reviewed `not_joined` or nothing.
struct Sheet {
    skipped: BTreeMap<String, String>,
    printed: BTreeSet<String>,
}

impl Sheet {
    fn new(slug: &'static str, candidates: impl IntoIterator<Item = String>) -> Self {
        let skipped = not_joined(slug);
        let probe: BTreeMap<String, String> = candidates
            .into_iter()
            .map(|key| (key, "X".to_string()))
            .collect();
        let left: BTreeSet<String> = bir_print::frozen_html::unprinted_keys(slug, &probe)
            .unwrap_or_else(|_| probe.keys().cloned().collect())
            .into_iter()
            .collect();
        let printed = probe
            .into_keys()
            .filter(|key| !left.contains(key) && !skipped.contains_key(key))
            .collect();
        Self { skipped, printed }
    }

    fn prints(&self, key: &str) -> bool {
        self.printed.contains(key)
    }

    /// The writer key the sheet shows a control under: the control key
    /// itself, else its serialized key.
    fn key_for<'a>(&self, key: &'a str, print: &BTreeMap<String, String>) -> &'a str {
        if print.contains_key(key) || self.prints(key) || self.skipped.contains_key(key) {
            key
        } else {
            serialized(key)
        }
    }
}

// ------------------------------------------------------------------ run

fn page_period_for(code: &str) -> i64 {
    match bir_core::forms::find_form(code).map(|form| &form.frequency) {
        Some(FilingFrequency::Monthly) => 3,
        Some(FilingFrequency::Annual) => 0,
        _ => 1,
    }
}

/// Up to four boxes a user would fill by key: one of each scalar binding,
/// unlocked and on screen.
fn round_trip_edits(kind: AgentFormKind) -> Map<String, Value> {
    let def = kind.definition();
    let on_screen = schema_keys(kind.form_code());
    let mut edits = Map::new();
    let mut taken: BTreeSet<&str> = BTreeSet::new();
    for entry in def.fillable() {
        if !on_screen.contains(&entry.key) || edits.len() >= 4 {
            continue;
        }
        let (tag, value) = match def.bindings.get(&entry.key).map(|bound| &bound.binding) {
            Some(agent::Binding::Amount { .. }) => ("amount", "1234.56"),
            Some(agent::Binding::Whole { .. }) => ("whole", "2"),
            Some(agent::Binding::Text { date: None, .. }) => ("text", "AUDIT SAMPLE"),
            _ => continue,
        };
        let limit = if tag == "amount" { 2 } else { 1 };
        if taken.iter().filter(|t| **t == tag).count() >= limit {
            continue;
        }
        taken.insert(tag);
        edits.insert(entry.key.clone(), json!(value));
    }
    edits
}

/// Validation errors the page cannot place: no schema key names them.
fn unplaceable_errors(
    kind: AgentFormKind,
    stage: &str,
    errors: &Value,
    on_screen: &BTreeSet<String>,
) -> Vec<Value> {
    let mut out = Vec::new();
    for error in errors.as_array().into_iter().flatten() {
        if error["keys"].as_array().is_some_and(|keys| keys.is_empty()) {
            let field = error["field"].as_str().unwrap_or_default();
            let named: Vec<String> = agent::error_keys(kind, field);
            let off_screen: Vec<&String> = named
                .iter()
                .filter(|key| !on_screen.contains(*key))
                .collect();
            out.push(json!({
                "stage": stage, "field": field, "message": error["message"],
                "agent_keys": named, "agent_keys_not_on_screen": off_screen,
            }));
        }
    }
    out
}

#[derive(Default)]
struct Totals {
    per_class: BTreeMap<&'static str, usize>,
}

/// The five forms whose drafts the legacy period-keyed lookup cannot find
/// (stored under a transaction date or another period): the screen's Print
/// path (`print_map` on the reopened draft) must print them, while the legacy
/// `print_fields` diagnostic keeps failing, so the audit reports the two
/// paths apart instead of calling the legacy miss a Print failure.
#[test]
fn screen_print_works_where_the_legacy_period_lookup_misses() {
    use_ephemeral_database();
    let profile: TaxpayerProfile =
        serde_json::from_value(profile_json()).expect("fictional profile parses");
    with_database(|db| {
        db.save_profile(profile)
            .map(|_| ())
            .map_err(|error| HostError::new(error.to_string()))
    })
    .expect("profile saves");
    for code in ["2000OT", "2200AN", "2200C", "2200S", "2552"] {
        let kind = kind_of(code).expect("form kind");
        let period = page_period_for(code);
        let opened = open(code, period).unwrap_or_else(|error| panic!("{code} opens: {error}"));
        call(
            "screen_form_save",
            vec![code.into(), json(opened["draft"].clone())],
        )
        .unwrap_or_else(|error| panic!("{code} saves: {}", error.message()));
        let reopened = open(code, period).unwrap_or_else(|error| panic!("{code} reopens: {error}"));
        let quarter = (reopened["period"].as_i64().unwrap_or(0) != 0)
            .then(|| reopened["period"].as_i64().unwrap_or(0) as u8);
        let legacy = with_database(|db| {
            print_fields(db, TIN, kind.form_code(), YEAR as u16, quarter).map(|_| ())
        });
        assert!(
            legacy.is_err(),
            "{code}: the legacy lookup now finds the draft; drop it from this list"
        );
        let printed = with_database(|db| print_map(db, kind, &reopened["draft"]))
            .unwrap_or_else(|error| panic!("{code}: screen Print failed: {}", error.message()));
        assert!(
            !printed.0.is_empty(),
            "{code}: screen Print produced no fields"
        );
    }
}

#[test]
#[ignore = "runtime audit; writes tmp/screen-form-audit/runtime.{json,md}"]
fn audit_all_screen_forms() {
    use_ephemeral_database();
    let profile: TaxpayerProfile =
        serde_json::from_value(profile_json()).expect("fictional profile parses");
    with_database(|db| {
        db.save_profile(profile)
            .map(|_| ())
            .map_err(|error| HostError::new(error.to_string()))
    })
    .expect("profile saves");
    let expectations = profile_expectations();

    let index = call("screen_forms_index", vec![]).expect("index");
    let codes: Vec<String> = index
        .as_array()
        .expect("array")
        .iter()
        .map(|row| row["c"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(codes.len(), 43);

    let mut forms_out = Vec::new();
    let mut totals = Totals::default();

    for code in &codes {
        let kind = AgentFormKind::from_code(code).expect("agent form");
        let def = kind.definition();
        let period = page_period_for(code);
        let slug = print_slug_for(kind.form_code()).unwrap_or_default();
        let fields = schema_fields(code);

        let mut findings: BTreeMap<&'static str, Vec<Value>> = BTreeMap::new();
        let mut info: BTreeMap<&'static str, Vec<Value>> = BTreeMap::new();

        let opened = match open(code, period) {
            Ok(opened) => opened,
            Err(error) => {
                findings
                    .entry("open_failed")
                    .or_default()
                    .push(json!({"error": error}));
                forms_out.push(json!({"code": code, "findings": findings}));
                continue;
            }
        };
        let values = string_map(&opened["values"]);
        let locks = string_map(&opened["locks"]);
        let raw_map = agent::field_map(kind, &opened["draft"]).unwrap_or_default();
        let print_open = print_map_of(kind, &opened["draft"]).unwrap_or_else(|error| {
            findings
                .entry("print_map_failed")
                .or_default()
                .push(json!({"stage": "open", "error": error}));
            BTreeMap::new()
        });

        let sheet = Sheet::new(
            slug,
            fields
                .iter()
                .flat_map(|field| [field.key.clone(), serialized(&field.key).to_string()])
                .chain(print_open.keys().cloned())
                .chain(raw_map.keys().cloned()),
        );

        // ---- per control
        let filled_categories: BTreeSet<&str> = fields
            .iter()
            .filter(|field| {
                values
                    .get(&field.key)
                    .is_some_and(|value| !value.trim().is_empty())
            })
            .filter_map(identity_category)
            .collect();
        let _ = &filled_categories;
        let mut rows = Vec::new();
        for field in &fields {
            let key = field.key.as_str();
            let entry = def.box_def(key);
            let binding = def.bindings.get(key);
            let unmapped = def.unmapped.get(key);
            let computed = field.computed
                || entry.is_some_and(|entry| entry.computed || entry.required == "computed");
            let hidden = entry.is_some_and(|entry| !entry.is_user_box() && !computed);
            let form_value = values.get(key).cloned();
            let raw_value = raw_map.get(key).cloned();
            let pkey = sheet.key_for(key, &print_open);
            let print_value = print_open.get(pkey).cloned();
            let printed = sheet.prints(pkey);
            // A box whose draft source is another party's (a schedule row,
            // the spouse, seller, buyer or a signatory) is not the filer's.
            let source = match binding.map(|bound| &bound.binding) {
                Some(
                    agent::Binding::Text { path, .. }
                    | agent::Binding::Amount { path, .. }
                    | agent::Binding::Whole { path, .. },
                ) => path.clone(),
                _ => unmapped.cloned().unwrap_or_default(),
            };
            let other_party = source.split(['/', ',', ' ']).any(|segment| {
                segment.parse::<usize>().is_ok()
                    || [
                        "spouse",
                        "civil_status",
                        "seller",
                        "buyer",
                        "president",
                        "treasurer",
                        "payee",
                    ]
                    .iter()
                    .any(|word| segment.contains(word))
            });
            let category = identity_category(field).filter(|_| !other_party);
            let lock = locks.get(key).cloned();
            let binding_text = binding.map(|bound| match &bound.binding {
                agent::Binding::Text { path, .. } => format!("text {path}"),
                agent::Binding::Amount { path, .. } => format!("amount {path}"),
                agent::Binding::Whole { path, .. } => format!("whole {path}"),
                agent::Binding::Choice { .. } => "choice".to_string(),
            });
            let row = json!({
                "k": key,
                "item": field.item,
                "label": field.label,
                "page": field.page,
                "type": field.kind,
                "auto": field.auto,
                "in_definition": entry.is_some(),
                "binding": binding_text,
                "binding_lock": binding.and_then(|bound| bound.lock.map(|lock| lock.reason())),
                "unmapped": unmapped,
                "computed": computed,
                "hidden": hidden,
                "lock": lock,
                "identity": category,
                "form_value": form_value,
                "field_map_value": raw_value,
                "print_key": pkey,
                "print_value": print_value,
                "sheet_prints": printed,
            });

            // The shell's occurrence alias filled a box the field map left out.
            if raw_value.is_none() && form_value.is_some() {
                info.entry("occurrence_alias_applied")
                    .or_default()
                    .push(json!({
                        "k": key, "official_key": serialized(key), "form_value": form_value,
                        "print_value": print_value,
                    }));
            }

            // (iii) not in the agent definition at all.
            if entry.is_none() {
                findings
                    .entry("iii_missing_from_definition")
                    .or_default()
                    .push(json!({
                        "k": key, "item": field.item, "label": field.label,
                        "form_value": form_value, "print_key": pkey, "print_value": print_value,
                        "sheet_prints": printed,
                    }));
            }

            let form_text = form_value.clone().unwrap_or_default();
            let print_text = print_value.clone().unwrap_or_default();

            // (i) identity/profile fields.
            if let Some(category) = category
                && !hidden
            {
                let expected = &expectations[category];
                let shows = |text: &str| {
                    let upper = norm_text(text);
                    !upper.is_empty() && expected.iter().any(|want| upper.contains(want))
                };
                let form_empty = form_text.trim().is_empty();
                let print_has = shows(&print_text);
                let spill_line = matches!(category, "name" | "address")
                    && continuation_line(field)
                    && filled_categories.contains(category);
                if form_empty && print_has && printed {
                    findings
                        .entry("i_identity_empty_on_form_filled_in_print")
                        .or_default()
                        .push(json!({
                            "k": key, "category": category, "item": field.item,
                            "print_key": pkey, "print_value": print_text,
                            "field_map_value": raw_value, "unmapped": unmapped, "lock": lock,
                        }));
                } else if form_empty && print_has {
                    info.entry("identity_in_print_map_but_sheet_does_not_print_it").or_default().push(json!({
                        "k": key, "category": category, "print_key": pkey, "print_value": print_text,
                    }));
                } else if form_empty
                    && print_text.trim().is_empty()
                    && !spill_line
                    && (field.auto || printed)
                {
                    // Is the profile fact printed under some other key?
                    let elsewhere: Vec<String> = print_open
                        .iter()
                        .filter(|(_, value)| shows(value))
                        .map(|(key, _)| key.clone())
                        .take(4)
                        .collect();
                    findings
                        .entry("i_identity_empty_in_both")
                        .or_default()
                        .push(json!({
                            "k": key, "category": category, "item": field.item,
                            "auto": field.auto, "print_key": pkey, "sheet_prints": printed,
                            "print_shows_profile_value_under": elsewhere,
                            "in_definition": entry.is_some(),
                            "binding": binding_text, "unmapped": unmapped, "lock": lock,
                        }));
                } else if !form_empty && !shows(&form_text) && category != "tin" {
                    info.entry("identity_form_value_not_profile")
                        .or_default()
                        .push(json!({
                            "k": key, "category": category, "form_value": form_text,
                        }));
                }
            }

            // (ii) form vs print after open.
            if form_value.is_some() && print_value.is_some() {
                match compare(&form_text, &print_text) {
                    "value" => findings.entry("ii_open_mismatch").or_default().push(json!({
                        "k": key, "item": field.item, "form_value": form_text,
                        "print_key": pkey, "print_value": print_text,
                    })),
                    "format" => {
                        info.entry("open_format_only_difference")
                            .or_default()
                            .push(json!({
                                "k": key, "form_value": form_text, "print_value": print_text,
                            }))
                    }
                    _ => {}
                }
            } else if !form_text.trim().is_empty() && print_value.is_none() {
                info.entry("form_value_without_print_key").or_default().push(json!({
                    "k": key, "form_value": form_text, "print_key": pkey, "sheet_prints": printed,
                }));
            } else if form_value.is_none() && !print_text.trim().is_empty() && category.is_none() {
                info.entry("print_value_without_form_value")
                    .or_default()
                    .push(json!({
                        "k": key, "print_key": pkey, "print_value": print_text,
                        "unmapped": unmapped,
                    }));
            }

            // (v) never fillable by the user, yet printed.
            let fillable_binding = binding.is_some_and(|bound| bound.lock.is_none())
                || entry
                    .and_then(|entry| entry.group.as_deref())
                    .and_then(|group| def.bindings.get(group))
                    .is_some_and(|bound| bound.lock.is_none());
            let profile_bound = binding.is_some_and(|bound| bound.lock.is_some());
            if !computed
                && !hidden
                && !fillable_binding
                && !profile_bound
                && category.is_none()
                && printed
            {
                let reason = unmapped.map(String::as_str).unwrap_or("");
                let class = if reason.starts_with("derived by the app") {
                    None
                } else if reason.starts_with("written by the app without a draft field") {
                    Some("v_app_constant_printed")
                } else if reason.starts_with("a row family pattern") {
                    None
                } else {
                    Some("v_never_fillable_but_printed")
                };
                let record = json!({
                    "k": key, "item": field.item, "label": field.label,
                    "in_definition": entry.is_some(),
                    "unmapped": unmapped, "lock": lock, "print_key": pkey,
                    "print_value_now": print_text,
                });
                match class {
                    Some(class) => findings.entry(class).or_default().push(record),
                    None => info
                        .entry("derived_printed_not_directly_fillable")
                        .or_default()
                        .push(record),
                }
            }
            rows.push(row);
        }

        // (i) by content: a profile fact the print sheet prints somewhere
        // that no control on the form page shows.
        let mut coverage = Map::new();
        for (category, expected) in &expectations {
            let holds = |text: &str| {
                let upper = norm_text(text);
                expected.iter().any(|want| upper.contains(want))
            };
            let on_form: Vec<&str> = fields
                .iter()
                .filter(|field| values.get(&field.key).is_some_and(|value| holds(value)))
                .map(|field| field.key.as_str())
                .collect();
            let in_print: Vec<&str> = print_open
                .iter()
                .filter(|(key, value)| sheet.prints(key) && holds(value))
                .map(|(key, _)| key.as_str())
                .collect();
            coverage.insert(
                category.to_string(),
                json!({"form": on_form.len(), "print": in_print.len()}),
            );
            if on_form.is_empty() && !in_print.is_empty() {
                findings
                    .entry("i_profile_fact_printed_but_not_on_form_page")
                    .or_default()
                    .push(json!({
                        "category": category, "printed_under": in_print,
                    }));
            }
        }

        // (vi) values the draft's field map holds under a key no schema
        // control shows (directly or as a `#occurrence-N` duplicate): the
        // page can never display them.
        let shown: BTreeSet<&str> = fields
            .iter()
            .flat_map(|field| [field.key.as_str(), serialized(&field.key)])
            .collect();
        let meaningful =
            |value: &str| !matches!(value.trim(), "" | "0" | "0.00" | "false" | "-" | "00");
        let off_screen: Vec<Value> = raw_map
            .iter()
            .filter(|(key, value)| !shown.contains(key.as_str()) && meaningful(value))
            .map(|(key, value)| json!({"key": key, "value": value, "sheet_prints": sheet.prints(key)}))
            .collect();
        if !off_screen.is_empty() {
            findings
                .entry("vi_field_map_values_not_on_screen")
                .or_default()
                .extend(off_screen);
        }

        // (iv) errors no control can show.
        let on_screen = schema_keys(code);
        let unplaceable = unplaceable_errors(kind, "open", &opened["errors"], on_screen);
        if !unplaceable.is_empty() {
            findings
                .entry("iv_unplaceable_errors")
                .or_default()
                .extend(unplaceable);
        }

        // ---- round trip: fill by key, save, reopen, print from the store.
        let edits = round_trip_edits(kind);
        let mut round = json!({"edits": edits});
        let filled = call(
            "screen_form_fill",
            vec![
                code.as_str().into(),
                json(opened["draft"].clone()),
                json(Value::Object(edits.clone())),
            ],
        );
        match filled {
            Err(error) => round["fill_error"] = json!(error.message()),
            Ok(filled) => {
                round["rejected"] = filled["rejected"].clone();
                match call(
                    "screen_form_save",
                    vec![code.as_str().into(), json(filled["draft"].clone())],
                ) {
                    Err(error) => {
                        round["save_error"] = json!(error.message());
                        findings
                            .entry("ii_roundtrip_failed")
                            .or_default()
                            .push(json!({
                                "stage": "save", "error": error.message(),
                            }));
                    }
                    Ok(_) => {
                        match open(code, period) {
                            Err(error) => {
                                findings
                                    .entry("ii_roundtrip_failed")
                                    .or_default()
                                    .push(json!({
                                        "stage": "reopen", "error": error,
                                    }));
                            }
                            Ok(reopened) => {
                                let values = string_map(&reopened["values"]);
                                let after_fill = string_map(&filled["values"]);
                                for (key, value) in &edits {
                                    let typed = value.as_str().unwrap_or_default();
                                    let shown = after_fill.get(key).cloned().unwrap_or_default();
                                    let got = values.get(key).cloned().unwrap_or_default();
                                    if filled["rejected"]
                                        .as_object()
                                        .is_some_and(|r| r.contains_key(key))
                                    {
                                        continue;
                                    }
                                    if compare(typed, &shown) == "value" {
                                        // The form's own recompute replaced the typed value.
                                        info.entry("fill_overridden_by_recompute")
                                            .or_default()
                                            .push(json!({
                                                "k": key, "typed": typed, "after_fill": shown,
                                            }));
                                    } else if compare(&shown, &got) == "value" {
                                        findings.entry("ii_roundtrip_failed").or_default().push(json!({
                                        "stage": "value lost between fill and reopen", "k": key,
                                        "after_fill": shown, "reopened": got,
                                    }));
                                    }
                                }
                                let unplaceable = unplaceable_errors(
                                    kind,
                                    "after round trip",
                                    &reopened["errors"],
                                    on_screen,
                                );
                                if !unplaceable.is_empty() {
                                    findings
                                        .entry("iv_unplaceable_errors")
                                        .or_default()
                                        .extend(unplaceable);
                                }
                                // Legacy diagnostic: the period-keyed store
                                // lookup with the page's period as a `u8`
                                // quarter. It fails when a form stores its
                                // draft under another key (a transaction
                                // date, a month); it does not decide whether
                                // the screen's Print works.
                                let quarter = (reopened["period"].as_i64().unwrap_or(0) != 0)
                                    .then(|| reopened["period"].as_i64().unwrap_or(0) as u8);
                                if let Err(error) = with_database(|db| {
                                    print_fields(db, TIN, kind.form_code(), YEAR as u16, quarter)
                                }) {
                                    findings
                                        .entry("legacy_print_fields_failed")
                                        .or_default()
                                        .push(json!({
                                            "quarter_argument": quarter,
                                            "stored_period": reopened["stored_period"],
                                            "error": error.message(),
                                        }));
                                }
                                // The screen's Print action: `print_map` on
                                // the draft the page holds, exactly as
                                // `screen_form_print` calls it.
                                let printed = with_database(|db| {
                                    print_map(db, kind, &reopened["draft"])
                                        .map(|(fields, _)| fields)
                                });
                                match printed {
                                    Err(error) => findings
                                        .entry("ii_roundtrip_failed")
                                        .or_default()
                                        .push(json!({
                                            "stage": "screen Print (print_map) on the reopened draft",
                                            "error": error.message(),
                                        })),
                                    Ok(print_saved) => {
                                        if let Err(error) = bir_print::frozen_html::filled_document(
                                            slug,
                                            &print_saved,
                                        ) {
                                            findings
                                                .entry("print_sheet_fill_failed")
                                                .or_default()
                                                .push(json!({"error": error}));
                                        }
                                        for field in &fields {
                                            let key = field.key.as_str();
                                            let pkey = sheet.key_for(key, &print_saved);
                                            let (Some(form), Some(print)) =
                                                (values.get(key), print_saved.get(pkey))
                                            else {
                                                continue;
                                            };
                                            if compare(form, print) == "value" {
                                                findings.entry("ii_roundtrip_mismatch").or_default().push(json!({
                                                "k": key, "item": field.item, "form_value": form,
                                                "print_key": pkey, "print_value": print,
                                                "edited": edits.contains_key(key),
                                            }));
                                            }
                                        }
                                        for key in edits.keys() {
                                            let pkey = sheet.key_for(key, &print_saved);
                                            let Some(value) = values.get(key) else {
                                                continue;
                                            };
                                            if value.trim().is_empty() || !sheet.prints(pkey) {
                                                continue;
                                            }
                                            if print_saved.get(pkey).is_none_or(|print| {
                                                compare(value, print) == "value"
                                            }) {
                                                findings.entry("ii_roundtrip_edit_not_in_print").or_default().push(json!({
                                                "k": key, "reopened_value": value, "print_key": pkey,
                                                "print_value": print_saved.get(pkey),
                                            }));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let counts: BTreeMap<&str, usize> = findings
            .iter()
            .map(|(class, list)| (*class, list.len()))
            .collect();
        for (class, count) in &counts {
            *totals.per_class.entry(class).or_default() += count;
        }
        let info_counts: BTreeMap<&str, usize> = info
            .iter()
            .map(|(class, list)| (*class, list.len()))
            .collect();
        forms_out.push(json!({
            "code": code,
            "package": def.package,
            "slug": slug,
            "period": period,
            "schema_controls": fields.len(),
            "bound": fields.iter().filter(|f| def.bindings.contains_key(&f.key)).count(),
            "unmapped": fields.iter().filter(|f| def.unmapped.contains_key(&f.key)).count(),
            "locked": fields.iter().filter(|f| locks.contains_key(&f.key)).count(),
            "with_form_value": fields.iter().filter(|f| values.get(&f.key).is_some_and(|v| !v.trim().is_empty())).count(),
            "counts": counts,
            "info_counts": info_counts,
            "identity_coverage": coverage,
            "findings": findings,
            "info": info,
            "round_trip": round,
            "controls": rows,
        }));
    }

    let report = json!({
        "generated_by": "crates/bir-shell/src/screen_form_audit.rs audit_all_screen_forms",
        "taxpayer": profile_json(),
        "year": YEAR,
        "totals": totals.per_class,
        "forms": forms_out,
    });
    let dir = format!("{}/../../tmp/screen-form-audit", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).expect("audit dir");
    std::fs::write(
        format!("{dir}/runtime.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .expect("runtime.json");
    std::fs::write(format!("{dir}/runtime.md"), markdown(&report)).expect("runtime.md");
    println!(
        "{}",
        serde_json::to_string_pretty(&report["totals"]).unwrap()
    );
}

// --------------------------------------------------------------- report

const CLASSES: &[(&str, &str, &str)] = &[
    ("open_failed", "form failed to open", "open-fail"),
    ("print_map_failed", "print map failed", "print-fail"),
    (
        "i_profile_fact_printed_but_not_on_form_page",
        "(i) profile fact printed but shown by no form control",
        "i-fact",
    ),
    (
        "i_identity_empty_on_form_filled_in_print",
        "(i) identity box empty on form, filled in print",
        "i-form",
    ),
    (
        "i_identity_empty_in_both",
        "(i) identity box empty on form and in print",
        "i-both",
    ),
    (
        "ii_open_mismatch",
        "(ii) form != print after open",
        "ii-open",
    ),
    (
        "ii_roundtrip_mismatch",
        "(ii) form != print after fill/save",
        "ii-saved",
    ),
    (
        "ii_roundtrip_edit_not_in_print",
        "(ii) edited value missing from print",
        "ii-edit",
    ),
    (
        "ii_roundtrip_failed",
        "(ii) round trip / print load failed",
        "ii-fail",
    ),
    (
        "legacy_print_fields_failed",
        "legacy period-keyed print load failed (diagnostic, not the Print action)",
        "legacy-print",
    ),
    (
        "print_sheet_fill_failed",
        "print sheet fill failed",
        "sheet-fail",
    ),
    (
        "iii_missing_from_definition",
        "(iii) schema key not in agent definition",
        "iii",
    ),
    (
        "iv_unplaceable_errors",
        "(iv) validation error with no on-screen key",
        "iv",
    ),
    (
        "vi_field_map_values_not_on_screen",
        "(vi) draft value under a key no schema control shows",
        "vi",
    ),
    (
        "v_never_fillable_but_printed",
        "(v) never fillable (no draft field) but printed",
        "v-none",
    ),
    (
        "v_app_constant_printed",
        "(v) app constant/period, printed, user cannot change",
        "v-const",
    ),
];

fn short(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}

fn markdown(report: &Value) -> String {
    let mut md = String::new();
    let forms = report["forms"].as_array().unwrap();
    let _ = writeln!(md, "# Screen form runtime audit\n");
    let _ = writeln!(
        md,
        "Generated by `{}` on an ephemeral database: one fictional Individual taxpayer \
         (TIN 123-456-788-00000) with every profile field filled, tax year {}. Each form opens \
         the way `screen_form_open` does; print values come from the same print field map \
         `form_print_html` fills (after open: from the open draft; after the round trip: from \
         `print_map` on the reopened draft, the path the screen's Print action takes). The \
         legacy period-keyed `bir_module::print_fields` load is reported separately as a \
         diagnostic.\n",
        report["generated_by"].as_str().unwrap(),
        report["year"]
    );
    let _ = writeln!(md, "## Totals\n");
    let _ = writeln!(md, "| class | findings | forms |\n| --- | ---: | ---: |");
    for (class, title, _) in CLASSES {
        let forms_hit = forms
            .iter()
            .filter(|form| form["counts"][*class].as_u64().unwrap_or(0) > 0)
            .count();
        let _ = writeln!(
            md,
            "| {title} | {} | {forms_hit} |",
            report["totals"][*class].as_u64().unwrap_or(0)
        );
    }

    let aliased: Vec<String> = forms
        .iter()
        .filter_map(|form| {
            let count = form["info"]["occurrence_alias_applied"].as_array()?.len();
            Some(format!("{} ({count})", short(&form["code"])))
        })
        .collect();
    let _ = writeln!(
        md,
        "\nThe shell's `#occurrence-N` alias (class A below) filled controls the field map \
         leaves out on: {}. Before that change they were empty on the form page (2551Q's \
         Email Address among them) and counted under (i).",
        if aliased.is_empty() {
            "none".to_string()
        } else {
            aliased.join(", ")
        }
    );
    let _ = writeln!(md, "\n## Per form\n");
    let header: Vec<(&str, &str)> = CLASSES
        .iter()
        .filter(|(class, _, _)| class.starts_with('i') || class.starts_with('v'))
        .map(|(class, _, short)| (*class, *short))
        .collect();
    let _ = writeln!(
        md,
        "| form | period | controls | bound | unmapped | locked | with value | {} |",
        header
            .iter()
            .map(|(_, short)| *short)
            .collect::<Vec<_>>()
            .join(" | ")
    );
    let _ = writeln!(md, "|{}", " --- |".repeat(7 + header.len()));
    for form in forms {
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            short(&form["code"]),
            short(&form["period"]),
            short(&form["schema_controls"]),
            short(&form["bound"]),
            short(&form["unmapped"]),
            short(&form["locked"]),
            short(&form["with_form_value"]),
            header
                .iter()
                .map(|(class, _)| form["counts"][*class].as_u64().unwrap_or(0).to_string())
                .collect::<Vec<_>>()
                .join(" | ")
        );
    }

    let _ = writeln!(md, "\n## Findings by form\n");
    for form in forms {
        let findings = &form["findings"];
        if findings.as_object().is_none_or(|map| map.is_empty()) {
            continue;
        }
        let _ = writeln!(md, "### {}\n", short(&form["code"]));
        for (class, title, _) in CLASSES {
            let Some(list) = findings[*class].as_array() else {
                continue;
            };
            let _ = writeln!(md, "- **{title}** ({})", list.len());
            for entry in list.iter().take(40) {
                let line = entry
                    .as_object()
                    .unwrap()
                    .iter()
                    .filter(|(_, value)| !value.is_null())
                    .map(|(key, value)| format!("{key}=`{}`", short(value).replace('`', "'")))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(md, "  - {line}");
            }
            if list.len() > 40 {
                let _ = writeln!(md, "  - ... {} more in runtime.json", list.len() - 40);
            }
        }
        let _ = writeln!(md);
    }
    md.push_str(&fix_classes(forms));
    md
}

/// Group the findings into fix classes, each with the layer that owns it.
fn fix_classes(forms: &[Value]) -> String {
    struct Class {
        title: &'static str,
        layer: &'static str,
        status: &'static str,
        items: Vec<String>,
    }
    let mut classes: BTreeMap<char, Class> = BTreeMap::new();
    let mut add = |id: char, title, layer, status, item: String| {
        classes
            .entry(id)
            .or_insert(Class {
                title,
                layer,
                status,
                items: Vec::new(),
            })
            .items
            .push(item);
    };
    for form in forms {
        let code = short(&form["code"]);
        let list = |class: &str| {
            form["findings"][class]
                .as_array()
                .cloned()
                .unwrap_or_default()
        };
        let info = |class: &str| form["info"][class].as_array().cloned().unwrap_or_default();
        for entry in info("occurrence_alias_applied") {
            add(
                'A',
                "Duplicate `#occurrence-N` controls read their official serialized key",
                "shell host (`screen_form_module::values_json` / `locks`); root cause in the \
                 bir-core agent probe, which lists these keys as unmapped",
                "FIXED in this change (read-only alias; writes still refused)",
                format!(
                    "{code} {} = `{}`",
                    short(&entry["k"]),
                    short(&entry["form_value"])
                ),
            );
        }
        for entry in list("i_identity_empty_on_form_filled_in_print") {
            add(
                'B',
                "Identity box empty on the form page while the same control prints the profile value",
                "bir-core agent definition (unmapped) / field map",
                "open",
                format!(
                    "{code} {} (print `{}`)",
                    short(&entry["k"]),
                    short(&entry["print_key"])
                ),
            );
        }
        for entry in list("i_profile_fact_printed_but_not_on_form_page") {
            let (id, title, layer) = if code == "1700" {
                (
                    'C',
                    "Form page built from a different revision than the draft (1700: schema from the \
                     1700-v2013 rule package, draft/print use 2018 keys)",
                    "rules corpus (no 1700 v2018 package) + forms-schema generator",
                )
            } else {
                (
                    'D',
                    "Profile fact printed, but the schema has no control for the key the draft writes \
                     (RDO: runtime-injected or non-corpus control dropped from forms-schema.json)",
                    "forms-schema generator (runtime-injected-control rows) / rules corpus",
                )
            };
            add(
                id,
                title,
                layer,
                "open",
                format!(
                    "{code} {} printed under {}",
                    short(&entry["category"]),
                    short(&entry["printed_under"])
                ),
            );
        }
        for entry in list("i_identity_empty_in_both") {
            let id = if code == "1700" { 'C' } else { 'E' };
            add(
                id,
                if id == 'C' {
                    "Form page built from a different revision than the draft (1700: schema from the \
                     1700-v2013 rule package, draft/print use 2018 keys)"
                } else {
                    "Identity box empty in form and print although the profile has the value"
                },
                if id == 'C' {
                    "rules corpus (no 1700 v2018 package) + forms-schema generator"
                } else {
                    "bir-core draft constructor (new_from_profile) / print map; some are hidden \
                     duplicates the schema should drop"
                },
                "open",
                format!(
                    "{code} {} [{}] binding={} unmapped={}",
                    short(&entry["k"]),
                    short(&entry["category"]),
                    short(&entry["binding"]),
                    short(&entry["unmapped"])
                ),
            );
        }
        for class in ["ii_open_mismatch", "ii_roundtrip_mismatch"] {
            for entry in list(class) {
                add(
                    'F',
                    "Form value differs from print value for the same control",
                    "bir-core: field map vs print map (to_print_field_map / print_field_map overrides)",
                    "open",
                    format!(
                        "{code} [{class}] {} form=`{}` print=`{}`",
                        short(&entry["k"]),
                        short(&entry["form_value"]),
                        short(&entry["print_value"])
                    ),
                );
            }
        }
        for class in ["ii_roundtrip_failed", "ii_roundtrip_edit_not_in_print"] {
            for entry in list(class) {
                add(
                    'G',
                    "Print preview cannot load the draft the form page saved (draft keyed by an \
                     MMDD period; the page passes its month/slot period)",
                    "shell host (`form_print_html` period argument vs `load_draft`'s lookup)",
                    "open",
                    format!("{code} {}", entry),
                );
            }
        }
        for entry in list("iv_unplaceable_errors") {
            add(
                'H',
                "Validation error names no on-screen control",
                "bir-core `agent::error_keys` / validator field names (or the schema lacks the box)",
                "open",
                format!(
                    "{code} [{}] {} -> keys {}",
                    short(&entry["field"]),
                    short(&entry["message"]),
                    short(&entry["agent_keys"])
                ),
            );
        }
        for entry in list("v_never_fillable_but_printed") {
            add(
                'I',
                "Printed box the user can never fill (no draft field)",
                "bir-core draft type lacks the field; agent definition lists it unmapped",
                "open",
                format!("{code} {} ({})", short(&entry["k"]), short(&entry["item"])),
            );
        }
        for entry in list("v_app_constant_printed") {
            add(
                'J',
                "Printed box the app writes as a constant/period; the user cannot change it",
                "bir-core draft type (constant in the field map) / agent definition",
                "open",
                format!(
                    "{code} {} = `{}`",
                    short(&entry["k"]),
                    short(&entry["print_value_now"])
                ),
            );
        }
        for entry in list("vi_field_map_values_not_on_screen") {
            add(
                'K',
                "Draft value under a key no schema control shows",
                "forms-schema generator (hidden/computed/runtime-injected rows) or rules corpus",
                "open",
                format!(
                    "{code} {} = `{}`",
                    short(&entry["key"]),
                    short(&entry["value"])
                ),
            );
        }
    }
    let mut md = String::from("## Cross-form fix classes\n\n");
    md.push_str("| class | layer | status | instances |\n| --- | --- | --- | ---: |\n");
    for (id, class) in &classes {
        let _ = writeln!(
            md,
            "| {id}. {} | {} | {} | {} |",
            class.title,
            class.layer,
            class.status,
            class.items.len()
        );
    }
    md.push('\n');
    for (id, class) in classes {
        let _ = writeln!(
            md,
            "### {id}. {}\n\nLayer: {}. Status: {}. {} instances.\n",
            class.title,
            class.layer,
            class.status,
            class.items.len()
        );
        for item in class.items.iter().take(60) {
            let _ = writeln!(md, "- {item}");
        }
        if class.items.len() > 60 {
            let _ = writeln!(md, "- ... {} more in runtime.json", class.items.len() - 60);
        }
        let _ = writeln!(md);
    }
    md
}
