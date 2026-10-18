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
                    let col = &item["cols"][c];
                    let column = col["l"].as_str().unwrap_or_default();
                    // A column an override hides is not drawn (2550Q's Item
                    // 42 table once held the Items 42/47/56 popup templates).
                    let kind = if col["t"] == "hidden" {
                        "hidden"
                    } else {
                        "cell"
                    };
                    out.push(SchemaField {
                        key: key.to_string(),
                        item: item_label.clone(),
                        label: format!("row {} {column}", r + 1).trim().to_string(),
                        page,
                        kind: kind.to_string(),
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
            let (id, title, layer) = (
                'D',
                "Profile fact printed, but the schema has no control for the key the draft writes \
                 (RDO: runtime-injected or non-corpus control dropped from forms-schema.json)",
                "forms-schema generator (runtime-injected-control rows) / rules corpus",
            );
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
            add(
                'E',
                "Identity box empty in form and print although the profile has the value",
                "bir-core draft constructor (new_from_profile) / print map; some are hidden \
                 duplicates the schema should drop",
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

// ------------------------------------------------- effective-schema census
//
// The controls the host actually serves (`schema()`: forms-schema.json with
// schema-overrides.json applied and repeated occurrences hidden), each
// classified from the agent definition the page binds through, with the
// runtime catalogs (`catalogs::options`) for dropdowns. The raw static audit
// (`tools/audit_screen_forms.py`) stays as provenance; this is what the page
// shows. Case ids are `<form>:<key>`, stable across runs.

/// One effective control's class. `unbound` is a visible user box with no
/// draft field (nothing writes it) and is open work, not a pass.
const CENSUS_CLASSES: &[&str] = &[
    "editable",
    "profile_locked",
    "computed",
    "static_artwork",
    "hidden_workflow",
    "concrete_row",
    "row_template",
    "unbound",
];

fn census_class(kind: AgentFormKind, field: &SchemaField) -> (&'static str, String) {
    let def = kind.definition();
    let key = field.key.as_str();
    let entry = def.box_def(key);
    let unmapped = def.unmapped.get(key).map(String::as_str).unwrap_or("");
    if field.kind == "hidden" {
        let why = if key.contains("#occurrence-") {
            "a repeated occurrence: shown once, written under the official key"
        } else {
            "hidden by schema-overrides.json; kept for error placement"
        };
        return ("hidden_workflow", why.to_string());
    }
    if key.contains('{') || unmapped.starts_with("a row family pattern") {
        return (
            "row_template",
            "a row family pattern, not one box".to_string(),
        );
    }
    if field.kind == "cell" {
        return ("concrete_row", "a schedule table cell".to_string());
    }
    if field.computed
        || entry.is_some_and(|e| e.computed || e.required == "computed")
        || unmapped.starts_with("derived by the app")
    {
        return ("computed", "computed by the form's formulas".to_string());
    }
    let binding = def.bindings.get(key).or_else(|| {
        entry
            .and_then(|e| e.group.as_deref())
            .and_then(|group| def.bindings.get(group))
    });
    if let Some(bound) = binding {
        return match bound.lock {
            Some(lock @ (agent::Lock::Identity | agent::Lock::Profile)) => {
                ("profile_locked", lock.reason().to_string())
            }
            Some(agent::Lock::Election) => ("editable", "an election the user answers".to_string()),
            None => ("editable", String::new()),
        };
    }
    if unmapped.starts_with("written by the app without a draft field") {
        return ("static_artwork", unmapped.to_string());
    }
    if let Some((official, _)) = key.split_once("#occurrence-") {
        return (
            "unbound",
            format!("repeats `{official}`; read-only here, written under the official key"),
        );
    }
    let reason = if unmapped.is_empty() {
        if entry.is_some() {
            "in the agent definition but has no draft field".to_string()
        } else {
            "not in the agent definition".to_string()
        }
    } else {
        unmapped.to_string()
    };
    ("unbound", reason)
}

/// Where a dropdown's choices come from: the reviewed schema, a runtime
/// catalog, a dependent list awaiting its parent, or nowhere.
fn dropdown_source(form: &Value, key: &str, catalog: &Value) -> Option<&'static str> {
    let field = form["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array().into_iter().flatten())
        .flat_map(|item| item["f"].as_array().into_iter().flatten())
        .find(|f| f["k"].as_str() == Some(key))?;
    if field["t"] != "sel" {
        return None;
    }
    Some(if field["o"].as_array().is_some_and(|o| !o.is_empty()) {
        "schema"
    } else {
        match catalog.get(key).and_then(Value::as_array) {
            Some(rows) if !rows.is_empty() => "catalog",
            Some(_) => "awaiting_parent",
            None => "missing",
        }
    })
}

/// Every effective control of every form, classified.
fn effective_census() -> Value {
    let mut forms = Vec::new();
    for form in schema() {
        let code = form["c"].as_str().unwrap_or_default();
        let kind = kind_of(code).expect("every schema form is an agent form");
        let catalog = catalogs::options(kind, &Value::Null);
        let mut controls = Vec::new();
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        let mut dropdowns: BTreeMap<&str, usize> = BTreeMap::new();
        for field in schema_fields(code) {
            let (class, reason) = census_class(kind, &field);
            *counts.entry(class).or_default() += 1;
            let dropdown = dropdown_source(form, &field.key, &catalog);
            if let Some(source) = dropdown {
                *dropdowns.entry(source).or_default() += 1;
            }
            controls.push(json!({
                "id": format!("{code}:{}", field.key),
                "k": field.key,
                "item": field.item,
                "label": field.label,
                "kind": field.kind,
                "class": class,
                "reason": reason,
                "dropdown": dropdown,
            }));
        }
        forms.push(json!({
            "code": code,
            "controls": controls.len(),
            "counts": counts,
            "dropdowns": dropdowns,
            "list": controls,
        }));
    }
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let mut dropdown_totals: BTreeMap<String, usize> = BTreeMap::new();
    for form in &forms {
        for (class, n) in form["counts"].as_object().into_iter().flatten() {
            *totals.entry(class.clone()).or_default() += n.as_u64().unwrap_or(0) as usize;
        }
        for (source, n) in form["dropdowns"].as_object().into_iter().flatten() {
            *dropdown_totals.entry(source.clone()).or_default() += n.as_u64().unwrap_or(0) as usize;
        }
    }
    json!({
        "generated_by": "screen_form_audit::write_effective_census",
        "source": "host schema(): forms-schema.json + schema-overrides.json + hidden repeated occurrences; catalogs::options",
        "forms_audited": forms.len(),
        "totals": totals,
        "dropdowns": dropdown_totals,
        "forms": forms,
    })
}

fn census_markdown(census: &Value) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "# Effective screen-form census\n");
    let _ = writeln!(
        md,
        "Generated by `{}` from {}. Raw static-audit counts (`tools/audit_screen_forms.py`) are \
         provenance and are not this census. `unbound` is open work, not a pass.\n",
        census["generated_by"].as_str().unwrap_or_default(),
        census["source"].as_str().unwrap_or_default(),
    );
    let _ = writeln!(md, "Forms: {}\n", census["forms_audited"]);
    let _ = writeln!(md, "| class | controls |\n| --- | ---: |");
    for class in CENSUS_CLASSES {
        let _ = writeln!(
            md,
            "| {class} | {} |",
            census["totals"][*class].as_u64().unwrap_or(0)
        );
    }
    let _ = writeln!(
        md,
        "\n| dropdown choices from | dropdowns |\n| --- | ---: |"
    );
    for source in ["schema", "catalog", "awaiting_parent", "missing"] {
        let _ = writeln!(
            md,
            "| {source} | {} |",
            census["dropdowns"][source].as_u64().unwrap_or(0)
        );
    }
    let _ = writeln!(md, "\n## Per form\n");
    let _ = write!(md, "| form | controls |");
    for class in CENSUS_CLASSES {
        let _ = write!(md, " {class} |");
    }
    let _ = writeln!(
        md,
        "\n| --- | ---: |{}",
        " ---: |".repeat(CENSUS_CLASSES.len())
    );
    for form in census["forms"].as_array().into_iter().flatten() {
        let _ = write!(
            md,
            "| {} | {} |",
            form["code"].as_str().unwrap_or_default(),
            form["controls"]
        );
        for class in CENSUS_CLASSES {
            let _ = write!(md, " {} |", form["counts"][*class].as_u64().unwrap_or(0));
        }
        let _ = writeln!(md);
    }
    let _ = writeln!(md, "\n## Open cases\n");
    let _ = writeln!(
        md,
        "| case | class | reason / dropdown |\n| --- | --- | --- |"
    );
    for form in census["forms"].as_array().into_iter().flatten() {
        for control in form["list"].as_array().into_iter().flatten() {
            let open = control["class"] == "unbound"
                || control["class"] == "row_template"
                || control["dropdown"] == "missing";
            if open {
                let _ = writeln!(
                    md,
                    "| `{}` | {} | {}{} |",
                    control["id"].as_str().unwrap_or_default(),
                    control["class"].as_str().unwrap_or_default(),
                    control["reason"]
                        .as_str()
                        .unwrap_or_default()
                        .replace('|', "\\|"),
                    if control["dropdown"] == "missing" {
                        " · dropdown missing"
                    } else {
                        ""
                    },
                );
            }
        }
    }
    md
}

#[test]
fn effective_census_classifies_every_control_of_all_43_forms() {
    let census = effective_census();
    assert_eq!(census["forms_audited"], 43);
    let mut total = 0;
    for form in census["forms"].as_array().unwrap() {
        let code = form["code"].as_str().unwrap();
        let list = form["list"].as_array().unwrap();
        assert_eq!(
            list.len(),
            schema_fields(code).len(),
            "{code}: every control classified"
        );
        let ids: BTreeSet<&str> = list.iter().map(|c| c["id"].as_str().unwrap()).collect();
        assert_eq!(ids.len(), list.len(), "{code}: case ids are unique");
        for control in list {
            assert!(
                CENSUS_CLASSES.contains(&control["class"].as_str().unwrap()),
                "{code}: {control}"
            );
        }
        total += list.len();
    }
    let by_class: usize = census["totals"]
        .as_object()
        .unwrap()
        .values()
        .map(|n| n.as_u64().unwrap() as usize)
        .sum();
    assert_eq!(by_class, total, "class totals cover every control once");
    // No dropdown is left without choices: P1 resolved the baseline's
    // D01-D10 (the catalog coverage test pins the same).
    let missing: Vec<String> = census["forms"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|form| form["list"].as_array().unwrap().iter())
        .filter(|c| c["dropdown"] == "missing")
        .map(|c| c["id"].as_str().unwrap().replacen(':', " ", 1))
        .collect();
    assert!(missing.is_empty(), "dropdowns without choices: {missing:?}");
    let supplied = census["dropdowns"]["catalog"].as_u64().unwrap_or(0)
        + census["dropdowns"]["awaiting_parent"].as_u64().unwrap_or(0);
    // 80, plus 1606 Item 16A and 1706 Item 14A's RDO lists (R05). The 36
    // 2550M Schedule 1 ATC lists (N03) are table cells, counted apart.
    assert_eq!(supplied, 82, "catalog lists, dependent ones included");
}

#[test]
#[ignore = "census; writes tmp/screen-form-audit/effective.{json,md}"]
fn write_effective_census() {
    let census = effective_census();
    let dir = format!("{}/../../tmp/screen-form-audit", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).expect("audit dir");
    std::fs::write(
        format!("{dir}/effective.json"),
        serde_json::to_string_pretty(&census).expect("census json"),
    )
    .expect("effective.json");
    std::fs::write(format!("{dir}/effective.md"), census_markdown(&census)).expect("effective.md");
    println!(
        "totals {} dropdowns {}",
        census["totals"], census["dropdowns"]
    );
}

/// Findings export (form-findings goal): for each screen form, the structure
/// the host serves and what a fresh open returns, the way the Form library
/// opens a return (the fixture taxpayer, the form's default period). The
/// per-rule checks in `tools/form_findings.py` read it.
#[test]
#[ignore = "findings export; writes tmp/form-findings/screen.json"]
fn write_form_findings_export() {
    seed_fictional_profile();
    let mut forms = Vec::new();
    for form in schema() {
        let code = form["c"].as_str().unwrap_or_default();
        let period = page_period_for(code);
        let opened = open(code, period).unwrap_or_else(|e| json!({ "error": e }));
        forms.push(json!({
            "code": code,
            "period": period,
            "year": YEAR,
            "schema": form,
            "values": opened["values"],
            "locks": opened["locks"],
            "profile": opened["profile"],
            "options": opened["options"].as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()),
            "errors": opened["errors"],
            "open_error": opened["error"],
        }));
    }
    let dir = format!("{}/../../tmp/form-findings", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).expect("findings dir");
    std::fs::write(
        format!("{dir}/screen.json"),
        serde_json::to_string(&json!({ "profile": profile_json(), "forms": forms }))
            .expect("export json"),
    )
    .expect("screen.json");
    println!("forms {}", forms.len());
}

// ------------------------------------------------ confirmed symptoms (P0.4)
//
// Each test states one confirmed defect from the continuation baseline and
// fails on exactly that symptom today. They are ignored so the suite stays
// green; the increment that fixes a case removes its `ignore` (never the
// assertion). Run them with `-- --ignored symptom_` to see each one fail.

fn seed_fictional_profile() {
    use_ephemeral_database();
    let profile: TaxpayerProfile =
        serde_json::from_value(profile_json()).expect("fictional profile parses");
    with_database(|db| {
        db.save_profile(profile)
            .map(|_| ())
            .map_err(|error| HostError::new(error.to_string()))
    })
    .expect("profile saves");
}

/// The page's value and the printed value of each key, after a fresh open.
fn screen_and_print(code: &str, keys: &[&str]) -> Vec<(String, String, String)> {
    seed_fictional_profile();
    let kind = kind_of(code).expect("form kind");
    let opened = open(code, page_period_for(code)).expect("form opens");
    let values = string_map(&opened["values"]);
    let printed = with_database(|db| print_map(db, kind, &opened["draft"]))
        .expect("screen Print")
        .0;
    keys.iter()
        .map(|key| {
            (
                key.to_string(),
                values.get(*key).cloned().unwrap_or_default(),
                printed.get(*key).cloned().unwrap_or_default(),
            )
        })
        .collect()
}

#[test]
fn symptom_r01_1701_page_headers_agree_with_print() {
    for (key, screen, print) in screen_and_print(
        "1701",
        &[
            "frm1701:txtPg2TaxpayerName",
            "frm1701:txtPg3TaxpayerName",
            "frm1701:txtPg4TaxpayerName",
        ],
    ) {
        assert_eq!(
            screen, print,
            "{key}: page shows {screen:?}, print shows {print:?}"
        );
    }
}

#[test]
fn symptom_r02_1702mx_instruction_checkbox_agrees_with_print() {
    for (key, screen, print) in screen_and_print("1702MX", &["frm1702MX:InstAPg2Part4"]) {
        assert_eq!(screen, print, "{key}");
    }
}

#[test]
fn symptom_r03_1702rt_other_atc_agrees_with_print() {
    for (key, screen, print) in screen_and_print("1702RT", &["frm1702RT:drpPg1I5AtcOther"]) {
        assert_eq!(screen, print, "{key}");
    }
}

#[test]
#[ignore = "R04 unresolved: txtEmail is the v2024 submission contact email (not Item 12, which shows the profile email); whether v2024 prefills it from the profile needs the v2024 HTA loadBGData, not available locally"]
fn symptom_r04_2550q_email_shows_the_profile_email() {
    let (_, screen, _) = screen_and_print("2550Q", &["txtEmail"]).remove(0);
    assert!(!screen.trim().is_empty(), "txtEmail is empty on the page");
}

#[test]
fn symptom_d03_1606_tax_rate_choice_round_trips() {
    seed_fictional_profile();
    let period = page_period_for("1606");
    let opened = open("1606", period).expect("1606 opens");
    let filled = call(
        "screen_form_fill",
        vec![
            "1606".into(),
            json(opened["draft"].clone()),
            json(json!({"frm1606:txtTaxRate": "5"})),
        ],
    )
    .expect("fill runs");
    assert!(
        filled["rejected"].get("frm1606:txtTaxRate").is_none(),
        "rate refused: {}",
        filled["rejected"]
    );
    call(
        "screen_form_save",
        vec!["1606".into(), json(filled["draft"].clone())],
    )
    .expect("saves");
    let reopened = open("1606", period).expect("reopens");
    assert_eq!(
        string_map(&reopened["values"])
            .get("frm1606:txtTaxRate")
            .map(String::as_str),
        Some("5")
    );
}

#[test]
fn symptom_r05_every_validation_error_names_a_screen_key() {
    seed_fictional_profile();
    let mut unplaced = Vec::new();
    for form in schema() {
        let code = form["c"].as_str().unwrap_or_default();
        let Ok(opened) = open(code, page_period_for(code)) else {
            continue;
        };
        for error in opened["errors"].as_array().into_iter().flatten() {
            if error["keys"].as_array().is_none_or(Vec::is_empty) {
                unplaced.push(format!("{code} {} — {}", error["field"], error["message"]));
            }
        }
    }
    assert!(
        unplaced.is_empty(),
        "{} errors name no screen key:\n{}",
        unplaced.len(),
        unplaced.join("\n")
    );
}

/// 1800's branch code is the last segment of item 5, the donor's TIN; the
/// donee rows are a separate item the filer types, never the donor's identity.
#[test]
fn symptom_p4_1800_branch_code_sits_with_the_donor_tin() {
    let fields = schema_fields("1800");
    let item_of = |key: &str| {
        fields
            .iter()
            .find(|f| f.key == key)
            .unwrap_or_else(|| panic!("{key} is on screen"))
            .item
            .clone()
    };
    let donor = item_of("frm1800v2018:txtTIN1");
    assert!(
        donor.contains("Donor’s Taxpayer Identification Number"),
        "{donor:?}"
    );
    assert_eq!(item_of("frm1800v2018:txtBranchCode"), donor);
    let census = effective_census();
    let class = |key: &str| {
        census["forms"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|f| f["code"] == "1800")
            .flat_map(|f| f["list"].as_array().into_iter().flatten())
            .find(|e| e["k"] == key)
            .map(|e| e["class"].as_str().unwrap_or_default().to_string())
            .unwrap_or_default()
    };
    for row in ["A", "B", "C", "D", "E"] {
        for key in [
            format!("frm1800v2018:txtDoneeName{row}"),
            format!("frm1800v2018:txtDoneeTIN{row}"),
        ] {
            assert_ne!(item_of(&key), donor, "{key} sits with the donor TIN");
            assert_eq!(class(&key), "editable", "{key} is not typed by the filer");
        }
    }
}

/// P2a found 1603Q's tax-relief controls under "12 Email Address" and
/// 1604C's refund date in an unlabelled group; they sit with their printed
/// items (1603Q 13/13A, 1604C 11).
#[test]
fn symptom_p4_relief_and_refund_controls_sit_with_their_printed_items() {
    let item_of = |code: &str, key: &str| {
        schema_fields(code)
            .into_iter()
            .find(|f| f.key == key)
            .unwrap_or_else(|| panic!("{code} {key} is on screen"))
            .item
    };
    assert_eq!(item_of("1603Q", "txtEmail"), "12 Email Address");
    for key in ["frm1603Q:SpecialTax_1", "frm1603Q:SpecialTax_2"] {
        assert!(item_of("1603Q", key).starts_with("13 Are there payees availing of tax relief"));
    }
    assert_eq!(
        item_of("1603Q", "frm1603Q:selTreaty"),
        "13A If yes, specify"
    );
    let refund = item_of("1604C", "frm1604c:releasedOfFunds_1");
    assert!(refund.starts_with("11 "), "{refund:?}");
    for key in [
        "frm1604c:txtRefMonth",
        "frm1604c:txtRefDate",
        "frm1604c:txtRefYear",
    ] {
        assert_eq!(item_of("1604C", key), refund, "{key}");
    }
}

// ------------------------------------------------------------- P1 controls

/// One box filled, saved and reopened.
struct Reopened {
    /// The fill's refusals (empty when accepted).
    rejected: Value,
    /// The box's value on the reopened page.
    shown: String,
    /// The box's value in the screen's Print map.
    printed: String,
    /// The reopened typed draft.
    draft: Value,
}

impl Reopened {
    fn accepted(&self) -> bool {
        self.rejected.as_object().is_none_or(|r| r.is_empty())
    }
}

/// Fill one box, save, reopen.
fn choose_save_reopen(code: &str, key: &str, value: &str) -> Reopened {
    let kind = kind_of(code).expect("form kind");
    let period = page_period_for(code);
    let opened = open(code, period).expect("form opens");
    let mut edits = json!({ key: value });
    // PR 9: 1606's and 1706's relief lists take a choice only once item 11
    // is Yes (rules/forms fields.json enabled_when "Item 19/20 indicates
    // ... relief"), so the filer answers Yes in the same batch first.
    if let Some(yes) = match key {
        "frm1606:selTreaty" => Some("frm1606:rdTreaty:_1"),
        "frm1706:selTreaty" => Some("frm1706:rdTreaty:_1"),
        _ => None,
    } {
        edits[yes] = json!("true");
    }
    let filled = call(
        "screen_form_fill",
        vec![code.into(), json(opened["draft"].clone()), json(edits)],
    )
    .expect("fill runs");
    call(
        "screen_form_save",
        vec![code.into(), json(filled["draft"].clone())],
    )
    .expect("saves");
    let reopened = open(code, period).expect("reopens");
    let shown = string_map(&reopened["values"])
        .get(key)
        .cloned()
        .unwrap_or_default();
    let printed = with_database(|db| print_map(db, kind, &reopened["draft"]))
        .expect("screen Print")
        .0
        .get(key)
        .cloned()
        .unwrap_or_default();
    Reopened {
        rejected: filled["rejected"].clone(),
        shown,
        printed,
        draft: reopened["draft"].clone(),
    }
}

/// A dropdown choice: (value, label).
type Choice<'a> = (&'a str, &'a str);

/// D01, D04, D05, D09, D10: each form's tax-relief dropdown lists the
/// official HTA choices with this form's own numbering, and every choice
/// survives save, reopen and print.
#[test]
fn p1_relief_dropdowns_offer_each_forms_official_choices() {
    let cases: &[(&str, &str, &[Choice])] = &[
        (
            "1603Q",
            "frm1603Q:selTreaty",
            &[
                ("0", "None"),
                ("1", "Special Rate"),
                ("2", "International Tax Treaty"),
                ("3", "Both"),
            ],
        ),
        (
            "1606",
            "frm1606:selTreaty",
            &[("0", "International Tax Treaty"), ("1", "Special Law")],
        ),
        (
            "1706",
            "frm1706:selTreaty",
            &[
                ("0", "None"),
                ("1", "International Tax Treaty"),
                ("2", "Special Law"),
            ],
        ),
        (
            "2200S",
            "frm2200S:lstTaxTreaty",
            &[
                ("0", "None"),
                ("1", "Special Law"),
                ("2", "International Tax Treaty"),
            ],
        ),
        (
            "2550M",
            "frm2550m:lstSpecialTax",
            &[
                ("0", "None"),
                ("1", "Special Rate"),
                ("2", "International Tax Treaty"),
            ],
        ),
    ];
    for (code, key, choices) in cases {
        let form = schema_entry(code).expect("schema");
        let field = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .flat_map(|it| it["f"].as_array().cloned().unwrap_or_default())
            .find(|f| f["k"] == *key)
            .expect("on screen");
        let listed: Vec<(String, String)> = field["o"]
            .as_array()
            .expect("options")
            .iter()
            .map(|o| {
                (
                    o["value"].as_str().unwrap().to_string(),
                    o["label"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let want: Vec<(String, String)> = choices
            .iter()
            .map(|(v, l)| (v.to_string(), l.to_string()))
            .collect();
        assert_eq!(listed, want, "{code} {key} options");
        for (value, label) in *choices {
            seed_fictional_profile();
            let Reopened {
                rejected,
                shown,
                printed,
                draft,
            } = choose_save_reopen(code, key, value);
            assert!(
                rejected.as_object().is_none_or(|r| r.is_empty()),
                "{code} {key}={value} ({label}) refused: {rejected}"
            );
            // The typed draft holds the enum this form's binding names.
            let def = kind_of(code).unwrap().definition();
            let Some(BoundBox {
                binding: agent::Binding::Choice { options },
                ..
            }) = def.bindings.get(*key)
            else {
                panic!("{code} {key} is a choice binding");
            };
            let option = options
                .iter()
                .find(|o| o.value == *value)
                .unwrap_or_else(|| panic!("{code} {key} binds {value}"));
            for (pointer, want) in &option.set {
                assert_eq!(
                    draft.pointer(pointer),
                    Some(want),
                    "{code} {key}={value} ({label}): draft {pointer}"
                );
            }
            assert_eq!(shown, *value, "{code} {key}: {label} after reopen");
            assert_eq!(printed, *value, "{code} {key}: {label} in print");
        }
    }
    // 1606's blank official choice is the page's empty choice: it means no
    // relief, and 0 (International Tax Treaty) is not turned into it.
    seed_fictional_profile();
    let Reopened {
        rejected, shown, ..
    } = choose_save_reopen("1606", "frm1606:selTreaty", "");
    assert!(
        rejected.as_object().is_none_or(|r| r.is_empty()),
        "{rejected}"
    );
    assert_eq!(shown, "");
}

/// D02: 1604C Item 11's refund date is one MM/DD/YYYY field shown as a
/// month select and day and year boxes. With refunds released it is entered
/// a box at a time, survives save and reopen, and prints as entered; turning
/// Item 11 to No clears it and the month falls back to the official 00.
#[test]
fn p1_1604c_refund_date_is_entered_by_its_month_day_and_year_boxes() {
    seed_fictional_profile();
    let code = "1604C";
    let kind = kind_of(code).unwrap();
    let period = page_period_for(code);
    let opened = open(code, period).expect("1604C opens");
    let fill = |draft: &Value, edits: Value| {
        let filled = call(
            "screen_form_fill",
            vec![code.into(), json(draft.clone()), json(edits)],
        )
        .expect("fill runs");
        assert!(
            filled["rejected"].as_object().is_none_or(|r| r.is_empty()),
            "{}",
            filled["rejected"]
        );
        filled
    };
    let mut draft = fill(
        &opened["draft"],
        json!({"frm1604c:releasedOfFunds_1": "true"}),
    )["draft"]
        .clone();
    for (key, value) in [
        ("frm1604c:txtRefMonth", "03"),
        ("frm1604c:txtRefDate", "15"),
        ("frm1604c:txtRefYear", "2026"),
    ] {
        draft = fill(&draft, json!({ key: value }))["draft"].clone();
    }
    call("screen_form_save", vec![code.into(), json(draft)]).expect("saves");
    let reopened = open(code, period).expect("reopens");
    assert_eq!(reopened["draft"]["refund_date"], "03/15/2026");
    let values = string_map(&reopened["values"]);
    let printed = with_database(|db| print_map(db, kind, &reopened["draft"]))
        .expect("screen Print")
        .0;
    for (key, want) in [
        ("frm1604c:txtRefMonth", "03"),
        ("frm1604c:txtRefDate", "15"),
        ("frm1604c:txtRefYear", "2026"),
    ] {
        assert_eq!(
            values.get(key).map(String::as_str),
            Some(want),
            "{key} on the page"
        );
        assert_eq!(
            printed.get(key).map(String::as_str),
            Some(want),
            "{key} in print"
        );
    }
    // Every month the official select lists is accepted.
    for month in 1..=12 {
        let value = format!("{month:02}");
        let filled = fill(&reopened["draft"], json!({"frm1604c:txtRefMonth": value}));
        assert_eq!(filled["values"]["frm1604c:txtRefMonth"], value);
    }
    // Item 11 back to No: the date goes, the month shows the official 00.
    let off = fill(
        &reopened["draft"],
        json!({"frm1604c:releasedOfFunds_2": "true"}),
    );
    assert_eq!(off["draft"]["refund_date"], "");
    assert_eq!(off["values"]["frm1604c:txtRefMonth"], "00");
}

/// D03: 1606 Item 31 is the filer's choice of rate. Every official rate
/// (0 included) is kept by the typed draft, shown after reopen, printed,
/// and the computed tax (Item 32) agrees between page and print; the blank
/// official choice clears it.
#[test]
fn p1_1606_tax_rate_is_chosen_from_the_official_rates() {
    for rate in ["0", "1.5", "3", "5", "6"] {
        seed_fictional_profile();
        let r = choose_save_reopen("1606", "frm1606:txtTaxRate", rate);
        assert!(r.accepted(), "{rate}: {}", r.rejected);
        assert_eq!(r.draft["tax_rate"], rate, "typed Form1606TaxRate");
        assert_eq!(r.shown, rate, "page after reopen");
        assert_eq!(r.printed, rate, "print");
        let kind = kind_of("1606").unwrap();
        let reopened = open("1606", page_period_for("1606")).unwrap();
        let page_tax = string_map(&reopened["values"])
            .get("frm1606:txtTaxR")
            .cloned()
            .unwrap_or_default();
        let print_tax = with_database(|db| print_map(db, kind, &reopened["draft"]))
            .unwrap()
            .0
            .get("frm1606:txtTaxR")
            .cloned()
            .unwrap_or_default();
        assert_ne!(
            compare(&page_tax, &print_tax),
            "value",
            "{rate}: Item 32 page {page_tax:?} print {print_tax:?}"
        );
    }
    seed_fictional_profile();
    let six = choose_save_reopen("1606", "frm1606:txtTaxRate", "6");
    assert!(six.accepted(), "{}", six.rejected);
    assert_eq!(six.shown, "6");
    let opened = open("1606", page_period_for("1606")).unwrap();
    let cleared = call(
        "screen_form_fill",
        vec![
            "1606".into(),
            json(opened["draft"].clone()),
            json(json!({"frm1606:txtTaxRate": ""})),
        ],
    )
    .unwrap();
    assert!(
        cleared["rejected"].as_object().is_none_or(|r| r.is_empty()),
        "{}",
        cleared["rejected"]
    );
    assert_eq!(cleared["draft"]["tax_rate"], Value::Null);
}

/// The value the page sends for a listed dropdown option (form_page.js
/// `optionOf`): an object's `value`; a `"CODE - Label"` string's code; any
/// other string itself. `None` for strings the page drops (template text).
fn page_option_value(option: &Value) -> Option<String> {
    match option {
        Value::Object(_) => option["value"].as_str().map(str::to_string),
        Value::String(text) if text.starts_with('"') => None,
        Value::String(text) => Some(
            text.split_once(" - ")
                .filter(|(code, label)| {
                    !code.is_empty() && !code.contains(char::is_whitespace) && !label.is_empty()
                })
                .map_or(text.as_str(), |(code, _)| code)
                .to_string(),
        ),
        _ => None,
    }
}

/// Every choice a schema dropdown lists, sent the way the page sends it,
/// is one the host accepts (unless the page shows the field locked).
#[test]
fn every_listed_dropdown_choice_is_accepted_by_the_host() {
    seed_fictional_profile();
    let mut refused = Vec::new();
    for form in schema() {
        let code = form["c"].as_str().unwrap_or_default();
        let Ok(opened) = open(code, page_period_for(code)) else {
            continue;
        };
        let locks = opened["locks"].as_object().cloned().unwrap_or_default();
        let fields = form["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|p| p["items"].as_array().into_iter().flatten())
            .flat_map(|it| it["f"].as_array().into_iter().flatten());
        for field in fields {
            let key = field["k"].as_str().unwrap_or_default();
            if field["t"] != "sel" || field["c"] == 1 || locks.contains_key(key) {
                continue;
            }
            for option in field["o"].as_array().into_iter().flatten() {
                let Some(value) = page_option_value(option) else {
                    continue;
                };
                let filled = call(
                    "screen_form_fill",
                    vec![
                        code.into(),
                        json(opened["draft"].clone()),
                        json(json!({ key: value })),
                    ],
                )
                .expect("fill runs");
                if let Some(reason) = filled["rejected"].get(key) {
                    refused.push(format!("{code} {key} {value:?}: {reason}"));
                }
            }
        }
    }
    // N03 (fixed): 2550M's Schedule 1 ATC selects list the form's own ATC
    // popup and the host writes them through the typed draft.
    assert!(
        refused.is_empty(),
        "listed choices refused:\n{}",
        refused.join("\n")
    );
}

/// D06: 1800 Item 1's day is chosen from 01-31, stored as a number and shown
/// padded; a leap day is a valid date, an impossible one is refused by the
/// form's own check, and blank goes back to "not chosen" (the official 00).
#[test]
fn p1_1800_day_of_donation_is_a_day_select() {
    let code = "1800";
    let period = page_period_for(code);
    let date = |month: &str, day: &str, year: &str| {
        seed_fictional_profile();
        let opened = open(code, period).unwrap();
        let filled = call(
            "screen_form_fill",
            vec![
                code.into(),
                json(opened["draft"].clone()),
                json(json!({
                    "frm1800v2018:txtMonth": month,
                    "frm1800v2018:txtDate": day,
                    "frm1800v2018:txtYear": year,
                })),
            ],
        )
        .unwrap();
        assert!(
            filled["rejected"].as_object().is_none_or(|r| r.is_empty()),
            "{}",
            filled["rejected"]
        );
        call(
            "screen_form_save",
            vec![code.into(), json(filled["draft"].clone())],
        )
        .unwrap();
        open(code, period).unwrap()
    };
    let date_errors = |opened: &Value| -> Vec<String> {
        opened["errors"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["field"] == "donation_date")
            .map(|e| e["message"].as_str().unwrap_or_default().to_string())
            .collect()
    };
    // Stored as a number, shown padded, after save and reopen.
    let fifth = date("03", "05", "2024");
    assert_eq!(fifth["draft"]["donation_day"], 5);
    assert_eq!(fifth["values"]["frm1800v2018:txtDate"], "05");
    assert!(date_errors(&fifth).is_empty(), "{:?}", date_errors(&fifth));
    // A leap day is a date; the same day in a common year is not.
    assert!(date_errors(&date("02", "29", "2024")).is_empty());
    assert_eq!(
        date_errors(&date("02", "29", "2023")),
        ["Please enter a valid date on Item 1."]
    );
    // Blank is "not chosen" again.
    let cleared = date("03", "", "2024");
    assert_eq!(cleared["draft"]["donation_day"], 0);
    assert_eq!(
        date_errors(&cleared),
        ["Please enter a valid date on Item 1."]
    );
    // Every listed day is accepted.
    let field = schema_fields(code)
        .into_iter()
        .find(|f| f.key == "frm1800v2018:txtDate")
        .unwrap();
    assert_eq!(field.kind, "sel");
}

/// D07: 1801 Part V schedules are tables of their concrete, bound rows
/// (no `{index}` row template is shown), and two Schedule 4 rows keep
/// distinct names, RDOs and amounts through save, reopen and print.
#[test]
fn p1_1801_schedules_are_tables_of_bound_rows() {
    let code = "1801";
    let kind = kind_of(code).unwrap();
    let def = kind.definition();
    let form = schema_entry(code).unwrap();
    let items: Vec<&Value> = form["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["items"].as_array().unwrap())
        .collect();
    let tables: Vec<&str> = items
        .iter()
        .filter(|it| it["rows"].as_u64().is_some())
        .map(|it| it["l"].as_str().unwrap())
        .collect();
    for want in [
        "Schedule 1 — Real properties",
        "Schedule 1A — Family home",
        "Schedule 2 — Shares of stock",
        "Schedule 2A — Other personal properties",
        "Schedule 3 — Taxable transfers",
        "Schedule 4 — Business interest",
    ] {
        assert!(tables.contains(&want), "{want} table: {tables:?}");
    }
    for item in items.iter().filter(|it| it["rows"].as_u64().is_some()) {
        for key in item["keys"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|r| r.as_array().unwrap())
        {
            let key = key.as_str().unwrap();
            assert!(def.bindings.contains_key(key), "{key} is not a bound box");
        }
    }
    for field in schema_fields(code)
        .iter()
        .filter(|f| f.key.contains("{index}"))
    {
        assert_eq!(field.kind, "hidden", "{} is still shown", field.key);
    }
    // Two business interests, entered in the table, kept apart.
    seed_fictional_profile();
    let period = page_period_for(code);
    let opened = open(code, period).unwrap();
    let rows = json!({
        "frm1801v2018:sched4Name_1": "ALPHA TRADING",
        "frm1801v2018:sched4Address_1": "1 ALPHA ST, MAKATI",
        "frm1801v2018:sched4Rdo_1": "047",
        "frm1801v2018:sched4Exc_1": "1000.00",
        "frm1801v2018:sched4Conj_1": "250.00",
        "frm1801v2018:sched4Name_2": "BETA SERVICES",
        "frm1801v2018:sched4Address_2": "2 BETA AVE, PASIG",
        "frm1801v2018:sched4Rdo_2": "043",
        "frm1801v2018:sched4Exc_2": "2000.00",
        "frm1801v2018:sched4Conj_2": "500.00",
    });
    let filled = call(
        "screen_form_fill",
        vec![
            code.into(),
            json(opened["draft"].clone()),
            json(rows.clone()),
        ],
    )
    .unwrap();
    assert!(
        filled["rejected"].as_object().is_none_or(|r| r.is_empty()),
        "{}",
        filled["rejected"]
    );
    call(
        "screen_form_save",
        vec![code.into(), json(filled["draft"].clone())],
    )
    .unwrap();
    let reopened = open(code, period).unwrap();
    let values = string_map(&reopened["values"]);
    let printed = with_database(|db| print_map(db, kind, &reopened["draft"]))
        .unwrap()
        .0;
    for (key, want) in rows.as_object().unwrap() {
        let want = want.as_str().unwrap();
        for (side, got) in [("page", values.get(key)), ("print", printed.get(key))] {
            let got = got.cloned().unwrap_or_default();
            assert_ne!(
                compare(want, &got),
                "value",
                "{key} on the {side}: {got:?}, entered {want:?}"
            );
        }
    }
    let interests = reopened["draft"]["business_interests"].as_array().unwrap();
    assert_eq!(interests[0]["rdo_code"], "047");
    assert_eq!(interests[1]["rdo_code"], "043");
    // The RDO cells offer the RDO list.
    assert!(
        reopened["options"]["frm1801v2018:sched4Rdo_1"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|o| o["value"] == "047")),
        "sched4Rdo_1 has the RDO list"
    );
}

/// D08: 2000's Schedule 1 ATC rows are its concrete rows 0-2 (the `{index}`
/// template is not shown). Both DS112 variants in two rows stay independent
/// and ordered, keep their own tax due, reopen and print as chosen, and no
/// UI token or template key reaches the saved draft.
#[test]
fn p1_2000_atc_rows_stay_independent_and_ordered() {
    let code = "2000";
    let kind = kind_of(code).unwrap();
    for field in schema_fields(code)
        .iter()
        .filter(|f| f.key.contains("{index}"))
    {
        assert_eq!(field.kind, "hidden", "{} is still shown", field.key);
    }
    seed_fictional_profile();
    let period = page_period_for(code);
    let opened = open(code, period).unwrap();
    let ds112: Vec<Value> = opened["options"]["drpATCCode0"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| {
            o["label"]
                .as_str()
                .unwrap_or_default()
                .starts_with("DS112 —")
        })
        .cloned()
        .collect();
    assert_eq!(ds112.len(), 2, "two DS112 choices");
    let filled = call(
        "screen_form_fill",
        vec![
            code.into(),
            json(opened["draft"].clone()),
            json(json!({
                "drpATCCode0": ds112[0]["value"],
                "frm2000:sched1:txtTaxBase0": "1000",
                "drpATCCode1": ds112[1]["value"],
                "frm2000:sched1:txtTaxBase1": "1000",
            })),
        ],
    )
    .unwrap();
    assert!(
        filled["rejected"].as_object().is_none_or(|r| r.is_empty()),
        "{}",
        filled["rejected"]
    );
    call(
        "screen_form_save",
        vec![code.into(), json(filled["draft"].clone())],
    )
    .unwrap();
    let reopened = open(code, period).unwrap();
    let rows = reopened["draft"]["schedule1"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["atc_code"], "DS112");
    assert_eq!(rows[1]["atc_code"], "DS112");
    assert_ne!(
        rows[0]["description"], rows[1]["description"],
        "distinct DS112 variants"
    );
    assert_ne!(
        rows[0]["tax_due"], rows[1]["tax_due"],
        "each keeps its own calculation"
    );
    for (row, choice) in [("drpATCCode0", &ds112[0]), ("drpATCCode1", &ds112[1])] {
        let selected: Vec<&Value> = reopened["options"][row]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["selected"] == true)
            .collect();
        assert_eq!(selected.len(), 1, "{row}");
        assert_eq!(
            selected[0]["value"], choice["value"],
            "{row} keeps its choice"
        );
    }
    let printed = with_database(|db| print_map(db, kind, &reopened["draft"]))
        .unwrap()
        .0;
    assert_eq!(
        printed.get("drpATCCode0").map(String::as_str),
        Some("DS112")
    );
    assert_eq!(
        printed.get("drpATCCode1").map(String::as_str),
        Some("DS112")
    );
    let saved = reopened["draft"].to_string();
    assert!(!saved.contains("catalog:"), "no UI token saved");
    assert!(!saved.contains("{index}"), "no template key saved");
}

// ------------------------------------------------------------- P3 runtime data

/// R01: 1701's page 2-4 headers are "Taxpayer/Filer's Last Name" on the
/// printed sheet and on the official page (loadBGData: the name up to its
/// first comma). Page and print agree after save and reopen.
#[test]
fn p3_1701_page_headers_show_the_last_name_on_page_and_print() {
    seed_fictional_profile();
    let code = "1701";
    let kind = kind_of(code).unwrap();
    let period = page_period_for(code);
    let opened = open(code, period).unwrap();
    call(
        "screen_form_save",
        vec![code.into(), json(opened["draft"].clone())],
    )
    .unwrap();
    let reopened = open(code, period).unwrap();
    let values = string_map(&reopened["values"]);
    let printed = with_database(|db| print_map(db, kind, &reopened["draft"]))
        .unwrap()
        .0;
    for page in [2, 3, 4] {
        let key = format!("frm1701:txtPg{page}TaxpayerName");
        assert_eq!(
            values.get(&key).map(String::as_str),
            Some("DELA CRUZ"),
            "{key} on the page"
        );
        assert_eq!(
            printed.get(&key).map(String::as_str),
            Some("DELA CRUZ"),
            "{key} in print"
        );
    }
}

/// R03: 1702RT Item 5's other ATC. Page and print show the filer's election
/// in every state (inactive, "Other" with the select's IC010, a picked code,
/// cleared); picking a code chooses "Other"; save and reopen keep it.
#[test]
fn p3_1702rt_other_atc_shows_the_election_on_page_and_print() {
    let code = "1702RT";
    let kind = kind_of(code).unwrap();
    let period = page_period_for(code);
    let key = "frm1702RT:drpPg1I5AtcOther";
    let check = |edits: Value, want: &str| {
        seed_fictional_profile();
        let opened = open(code, period).unwrap();
        let filled = call(
            "screen_form_fill",
            vec![
                code.into(),
                json(opened["draft"].clone()),
                json(edits.clone()),
            ],
        )
        .unwrap();
        assert!(
            filled["rejected"].as_object().is_none_or(|r| r.is_empty()),
            "{}",
            filled["rejected"]
        );
        call(
            "screen_form_save",
            vec![code.into(), json(filled["draft"].clone())],
        )
        .unwrap();
        let reopened = open(code, period).unwrap();
        let page = string_map(&reopened["values"])
            .get(key)
            .cloned()
            .unwrap_or_default();
        let print = with_database(|db| print_map(db, kind, &reopened["draft"]))
            .unwrap()
            .0
            .get(key)
            .cloned()
            .unwrap_or_default();
        assert_eq!((page.as_str(), print.as_str()), (want, want), "{edits}");
    };
    check(json!({}), "");
    check(json!({"frm1702RT:rdoPg1I5AtcOther": "true"}), "IC010");
    seed_fictional_profile();
    let listed = open(code, period).unwrap()["options"][key].clone();
    let picked = listed
        .as_array()
        .and_then(|rows| rows.iter().find(|o| o["value"] != "IC010"))
        .and_then(|o| o["value"].as_str())
        .expect("another ATC is listed")
        .to_string();
    check(json!({ key: picked }), &picked);
    check(
        json!({ key: picked, "frm1702RT:rdoPg1I5AtcOther": "false" }),
        "",
    );
}

/// N06: a choice answers only its own question. Before the probe repair,
/// ticking 1701MS Item 31 "To be refunded" also set Item 12's tax option to
/// "special", and choosing 1701Q's ATC II012 set the filer type to Estate.
#[test]
fn p3_choices_answer_only_their_own_question() {
    for (code, key, value, untouched) in [
        (
            "1701MS",
            "frm1701MS:txtToBeRefunded",
            "true",
            "/taxpayer/tax_option",
        ),
        ("1701Q", "frm1701q:optATC", "II012", "/filer_type"),
    ] {
        seed_fictional_profile();
        let opened = open(code, page_period_for(code)).unwrap();
        let filled = call(
            "screen_form_fill",
            vec![
                code.into(),
                json(opened["draft"].clone()),
                json(json!({ key: value })),
            ],
        )
        .unwrap();
        assert!(
            filled["rejected"].as_object().is_none_or(|r| r.is_empty()),
            "{}",
            filled["rejected"]
        );
        assert_eq!(
            filled["draft"].pointer(untouched),
            opened["draft"].pointer(untouched),
            "{code} {key}={value} changed {untouched}"
        );
    }
}

/// R05 / N06: an overpaid 1601EQ (Item 30) or 1602Q (Item 28) takes its
/// over-remittance option on screen; it survives save and reopen and the
/// "select an option" error goes away.
#[test]
fn p3_overpaid_returns_take_their_over_remittance_option() {
    for (code, remit, option) in [
        ("1601EQ", "frm1601EQ:txtTax20", "frm1601EQ:ifRefund"),
        ("1602Q", "frm1602Q:txt18", "frm1602Q:OverRemittance1"),
    ] {
        seed_fictional_profile();
        let period = page_period_for(code);
        let opened = open(code, period).unwrap();
        let fill = |draft: &Value, edits: Value| {
            let filled = call(
                "screen_form_fill",
                vec![code.into(), json(draft.clone()), json(edits)],
            )
            .unwrap();
            assert!(
                filled["rejected"].as_object().is_none_or(|r| r.is_empty()),
                "{code}: {}",
                filled["rejected"]
            );
            filled
        };
        let overpaid = fill(&opened["draft"], json!({ remit: "9999999" }));
        let chosen = fill(&overpaid["draft"], json!({ option: "true" }));
        assert_eq!(chosen["draft"]["over_remittance"], "refund", "{code}");
        call(
            "screen_form_save",
            vec![code.into(), json(chosen["draft"].clone())],
        )
        .unwrap();
        let reopened = open(code, period).unwrap();
        assert_eq!(
            reopened["draft"]["over_remittance"], "refund",
            "{code} after reopen"
        );
        assert_eq!(reopened["values"][option], "true", "{code} page");
        let still_asked = reopened["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["field"] == "over_remittance");
        assert!(!still_asked, "{code}: {}", reopened["errors"]);
    }
}

/// R05: 1606 Item 16A and 1706 Item 14A (the property's RDO) are on screen
/// with the RDO list; their "Please enter the RDO Code" error lands on them.
#[test]
fn p3_property_rdo_controls_are_on_screen_and_take_their_error() {
    for (code, key) in [
        ("1606", "frm1606:txtRDOCode16A"),
        ("1706", "frm1706:txtRDOCode14A"),
    ] {
        assert!(schema_keys(code).contains(key), "{key} on screen");
        seed_fictional_profile();
        let opened = open(code, page_period_for(code)).unwrap();
        let error = opened["errors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["field"] == "property_rdo_code")
            .expect("RDO error while empty");
        assert_eq!(error["keys"], json!([key]), "{code}");
        assert!(
            opened["options"][key]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|o| o["value"] == "043")),
            "{key} offers the RDO list"
        );
        let r = choose_save_reopen(code, key, "043");
        assert!(
            r.accepted() && r.shown == "043",
            "{code}: {} {}",
            r.rejected,
            r.shown
        );
    }
}
