//! The redesigned on-screen form page's host functions.
//!
//! The page (JavaScript) renders a form from `app/forms/forms-schema.json`
//! and binds every control by its rules-corpus key `k` (e.g.
//! `frm1601c:txtMonth`). Everything it needs about a draft goes through
//! bir-core's agent seam (`bir_core::forms::agent`): `fill` writes one box,
//! `compute`/`validate` run the form's own formulas and checks, `field_map`
//! reads every box back as official key -> string, and `save_draft` saves
//! the typed draft the way the painted page's "Save draft" does.
//!
//! Drafts open the way each painted page opens them (stored draft for the
//! taxpayer and period, else a new one built from the profile), reproduced
//! here because bir-shell cannot depend on bir-desktop.
//!
//! `screen_form_open` and `screen_form_validate` are asynchronous: the main
//! thread reads the database and parses arguments, and the returned future
//! runs compute, validation and the field map off the main thread without
//! touching the database (it is thread-local and not `Send`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use bir_core::db::Database;
use bir_core::forms::agent::{self, AgentFormDef, AgentFormKind, Binding, BoundBox, BoxDef};
use bir_core::forms::queueable::{QueueableForm, QueueableKind};
use bir_core::forms::registry::FilingFrequency;
use bir_core::forms::{FilingPeriod, FilingStatus};
use bir_core::profile::TaxpayerProfile;
use gpui_shell::{HostArguments, HostError, HostModule, HostResult, HostValue};
use serde_json::{Map, Value, json};

use crate::bir_module::{json, to_json, with_database};

mod catalogs;
mod rows_2550m;
mod rows_2550q;

/// TypeScript appended to the `bir` module's declarations.
pub const DECLARATIONS: &str = r#"
export interface ScreenFormIndexEntry {
    c: string;
    t: string;
    rev: string;
    freq: string;
    pages: number;
    n: number;
    q: number;
}
export interface ScreenFormError {
    field: string;
    message: string;
    keys: string[];
}
export type ScreenFormStatus = "Draft" | "Queued" | "Submitted" | "Confirmed" | "Paid";
export interface ScreenFormOpened {
    form_code: string;
    year: number;
    period: number;
    status: ScreenFormStatus;
    editable: boolean;
    /** The period column the draft is stored under: pass it to queue and print. */
    stored_period: number | null;
    draft: object;
    values: Record<string, string>;
    options: Record<string, ScreenFormOption[]>;
    locks: Record<string, string>;
    /** Boxes the taxpayer profile fills (the page marks their items). */
    profile: string[];
    errors: ScreenFormError[];
}
export interface ScreenFormFilled {
    draft: object;
    values: Record<string, string>;
    options: Record<string, ScreenFormOption[]>;
    rejected: Record<string, string>;
}
export interface ScreenFormValidated {
    draft: object;
    values: Record<string, string>;
    options: Record<string, ScreenFormOption[]>;
    errors: ScreenFormError[];
}
export interface ScreenFormOption {
    value: string;
    label: string;
    /** Selected by the typed draft when multiple choices share a code. */
    selected?: boolean;
}
export interface ScreenFormSaved {
    id: number;
    status: ScreenFormStatus;
    stored_period: number | null;
    draft: object;
}
export function screen_form_schema(form_code: string): object;
export function screen_forms_index(): ScreenFormIndexEntry[];
export function screen_form_open(tin: string, form_code: string, year: number, period: number): Promise<ScreenFormOpened>;
export function screen_form_fill(form_code: string, draft: object, edits: Record<string, string | number | boolean | null>): ScreenFormFilled;
export function screen_form_validate(form_code: string, draft: object): Promise<ScreenFormValidated>;
export function screen_form_save(form_code: string, draft: object): ScreenFormSaved;
/** Print preview of a saved draft (opens the filled official sheet in the browser). */
export function screen_form_print(form_code: string, draft: object): { path: string };
/** Queue a saved draft; 1601C and 2551Q use queue_form_draft instead. */
export function screen_form_queue(form_code: string, draft: object): number;
/** Take a queued draft out of the queue; 1601C and 2551Q use cancel_queued. */
export function screen_form_cancel_queue(form_code: string, draft: object): null;
"#;

pub fn register(module: HostModule) -> HostModule {
    module
        .function("screen_form_schema", screen_form_schema)
        .function("screen_forms_index", |_| screen_forms_index())
        .async_function("screen_form_open", screen_form_open)
        .function("screen_form_fill", screen_form_fill)
        .async_function("screen_form_validate", screen_form_validate)
        .function("screen_form_save", screen_form_save)
        .function("screen_form_print", screen_form_print)
        .function("screen_form_queue", screen_form_queue)
        .function("screen_form_cancel_queue", screen_form_cancel_queue)
}

fn host_error(message: impl Into<String>) -> HostError {
    HostError::new(message)
}

// ---------------------------------------------------------------- schema

/// The on-screen form schema: one entry per form, parsed once, with the
/// reviewed corrections in `app/forms/schema-overrides.json` applied.
fn schema() -> &'static [Value] {
    static SCHEMA: OnceLock<Vec<Value>> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        let mut forms: Vec<Value> =
            serde_json::from_str(include_str!("../../../app/forms/forms-schema.json"))
                .expect("app/forms/forms-schema.json is a JSON array of forms");
        let overrides: Value =
            serde_json::from_str(include_str!("../../../app/forms/schema-overrides.json"))
                .expect("app/forms/schema-overrides.json is a JSON object");
        for form in &mut forms {
            let code = form["c"].as_str().unwrap_or_default().to_string();
            if let Some(fixes) = overrides.get(&code).and_then(Value::as_object) {
                apply_overrides(form, fixes);
                if let Some(layout) = fixes.get("layout") {
                    apply_layout(form, layout);
                }
            }
            hide_repeated_occurrences(form);
        }
        forms
    })
}

/// A box the official form repeats (`X#occurrence-2`, `-3`, …: 2551Q's
/// email, 2200A's TIN, branch and name) is shown once: every later
/// occurrence is hidden. The serializer writes the same value to all of
/// them, so nothing is lost on save or print.
fn hide_repeated_occurrences(form: &mut Value) {
    let Some(parts) = form["parts"].as_array_mut() else {
        return;
    };
    for part in parts.iter_mut() {
        let Some(items) = part["items"].as_array_mut() else {
            continue;
        };
        for item in items.iter_mut() {
            let Some(fields) = item["f"].as_array_mut() else {
                continue;
            };
            for field in fields.iter_mut() {
                let later = field["k"]
                    .as_str()
                    .and_then(|k| k.rsplit_once("#occurrence-"))
                    .and_then(|(_, n)| n.parse::<u32>().ok())
                    .is_some_and(|n| n >= 2);
                if later {
                    field["t"] = Value::from("hidden");
                }
            }
        }
    }
}

/// Apply one form's corrections. Keys are a field key `k` (label, kind,
/// options or computed flag of that field, or of a schedule column when `k`
/// is a table cell)
/// or `item:<part>:<index>` (that item's printed label). `kind: "hidden"`
/// keeps the field for validation placement but the page does not draw it.
/// The `evidence` text is provenance only.
fn apply_overrides(form: &mut Value, fixes: &serde_json::Map<String, Value>) {
    let set = |target: &mut Value, fix: &Value, kind_key: &str| {
        if let Some(label) = fix.get("label") {
            target["l"] = label.clone();
        }
        if let Some(kind) = fix.get("kind") {
            target[kind_key] = kind.clone();
        }
        if let Some(options) = fix.get("options") {
            target["o"] = options.clone();
        }
        // `computed: false` corrects a field the extractor marked computed
        // although the filer chooses it (1606 Item 31).
        if let Some(computed) = fix.get("computed").and_then(Value::as_bool)
            && let Some(object) = target.as_object_mut()
        {
            if computed {
                object.insert("c".into(), Value::from(1));
            } else {
                object.remove("c");
            }
        }
    };
    let Some(parts) = form["parts"].as_array_mut() else {
        return;
    };
    // Tables first, so a cell key's correction (a column's options) finds
    // the table it belongs to.
    for (key, fix) in fixes {
        if let Some(part) = key.strip_prefix("table:") {
            append_table(parts, part, fix);
        }
    }
    // `move:<field key>` with `to: "<part>:<item>"`: a field the extractor
    // grouped under the wrong item moves to the printed item it belongs to.
    // `to: "<part>:new:<no>"` with `item: {l, order}` moves it to printed
    // item `<no>` (one per `<no>` and heading), which the extractor left
    // out, appended at the part's end.
    // The emptied item stays (indexes hold) and the page skips it.
    for (key, fix) in fixes {
        if let Some(field_key) = key.strip_prefix("move:") {
            move_field(parts, field_key, fix);
        }
    }
    // Added items sit after every extracted item, ordered by their `order`
    // (the overrides map's own order is not relied on).
    for part in parts.iter_mut() {
        if let Some(items) = part["items"].as_array_mut() {
            let first = items
                .iter()
                .position(|it| it["added"] == true)
                .unwrap_or(items.len());
            items[first..].sort_by_key(|it| it["order"].as_i64().unwrap_or(i64::MAX));
        }
    }
    for (key, fix) in fixes {
        if key.starts_with("table:") || key.starts_with("move:") {
            continue;
        }
        // `add:<part>:<item>`: a bound control the extracted schema left out,
        // appended to that item (`fields`: `{k, l, t}`).
        if let Some(rest) = key.strip_prefix("add:") {
            let mut at = rest.split(':').filter_map(|n| n.parse::<usize>().ok());
            if let (Some(p), Some(i)) = (at.next(), at.next())
                && let Some(item) = parts.get_mut(p).and_then(|part| part["items"].get_mut(i))
                && let Some(fields) = fix["fields"].as_array()
            {
                match item["f"].as_array_mut() {
                    Some(existing) => existing.extend(fields.iter().cloned()),
                    None => item["f"] = Value::Array(fields.clone()),
                }
            }
            continue;
        }
        if let Some(rest) = key.strip_prefix("item:") {
            let mut indexes = rest.split(':').filter_map(|n| n.parse::<usize>().ok());
            if let (Some(p), Some(i)) = (indexes.next(), indexes.next())
                && let Some(item) = parts.get_mut(p).and_then(|part| part["items"].get_mut(i))
                && let Some(label) = fix.get("label")
            {
                item["l"] = label.clone();
            }
            continue;
        }
        for part in parts.iter_mut() {
            let Some(items) = part["items"].as_array_mut() else {
                continue;
            };
            for item in items.iter_mut() {
                if let Some(fields) = item["f"].as_array_mut() {
                    for field in fields.iter_mut().filter(|f| f["k"] == key.as_str()) {
                        set(field, fix, "t");
                    }
                }
                let column = item["keys"].as_array().and_then(|rows| {
                    rows.iter().find_map(|row| {
                        row.as_array()?
                            .iter()
                            .position(|cell| cell.as_str() == Some(key.as_str()))
                    })
                });
                if let Some(c) = column
                    && let Some(col) = item["cols"].get_mut(c)
                {
                    set(col, fix, "t");
                }
            }
        }
    }
}

/// A `layout` correction: the printed sheet's own structure for a form whose
/// extracted items follow another numbering (the official program's
/// fields.json). `parts: [{n, items: [{no, l, k: [field keys]} |
/// {table: <cell key>}], keep}]` rebuilds the form in that order, after
/// every other correction; a `table` entry takes the whole schedule table
/// holding that cell, and `keep: <part index>` appends that extracted part's
/// items (less what the layout names elsewhere) unchanged, its name too when
/// `n` is absent. Whatever the layout does not name keeps its own label and
/// goes to a last part (`rest`, "Not on the printed sheet") without an item
/// number, so no control is lost.
fn apply_layout(form: &mut Value, layout: &Value) {
    let Some(old_parts) = form["parts"].as_array().cloned() else {
        return;
    };
    let specs: Vec<&Value> = layout["parts"].as_array().into_iter().flatten().collect();
    let named_items = || {
        specs
            .iter()
            .flat_map(|p| p["items"].as_array().into_iter().flatten())
    };
    // What the layout places itself: field keys, and every cell of a named table.
    let mut claimed: BTreeSet<String> = named_items()
        .flat_map(|it| it["k"].as_array().into_iter().flatten())
        .filter_map(|k| k.as_str().map(str::to_string))
        .collect();
    let table_keys = |item: &Value| -> Vec<String> {
        item["keys"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|row| row.as_array().into_iter().flatten())
            .filter_map(|k| k.as_str().map(str::to_string))
            .collect()
    };
    let mut fields: BTreeMap<String, (Value, Value)> = BTreeMap::new();
    let mut tables: Vec<Value> = Vec::new();
    for part in &old_parts {
        for item in part["items"].as_array().into_iter().flatten() {
            if item.get("rows").is_some() {
                tables.push(item.clone());
            }
            for field in item["f"].as_array().into_iter().flatten() {
                if let Some(key) = field["k"].as_str() {
                    fields.insert(key.to_string(), (field.clone(), item["p"].clone()));
                }
            }
        }
    }
    let wanted_tables: Vec<&str> = named_items()
        .filter_map(|it| it["table"].as_str())
        .collect();
    for table in &tables {
        let keys = table_keys(table);
        if keys.iter().any(|k| wanted_tables.contains(&k.as_str())) {
            claimed.extend(keys);
        }
    }
    let mut kept = BTreeSet::new();
    // An extracted item less what the layout claims; None when nothing is left.
    let remainder = |item: &Value, claimed: &BTreeSet<String>, number: bool| -> Option<Value> {
        if item.get("rows").is_some() {
            let keys = table_keys(item);
            return (!keys.iter().any(|k| claimed.contains(k))).then(|| item.clone());
        }
        let left: Vec<Value> = item["f"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|f| f["k"].as_str().is_some_and(|k| !claimed.contains(k)))
            .cloned()
            .collect();
        if left.is_empty() {
            return None;
        }
        let mut item = item.clone();
        item["f"] = Value::Array(left);
        if !number {
            item["no"] = Value::from("");
        }
        Some(item)
    };
    let mut parts = Vec::new();
    for spec in &specs {
        let mut items = Vec::new();
        for entry in spec["items"].as_array().into_iter().flatten() {
            if let Some(cell) = entry["table"].as_str() {
                if let Some(table) = tables
                    .iter()
                    .find(|t| table_keys(t).iter().any(|k| k == cell))
                {
                    items.push(table.clone());
                }
                continue;
            }
            let mut page = Value::Null;
            let chosen: Vec<Value> = entry["k"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(|key| fields.get(key).cloned())
                .map(|(field, p)| {
                    if page.is_null() {
                        page = p;
                    }
                    field
                })
                .collect();
            if !chosen.is_empty() {
                items.push(json!({ "no": entry["no"].clone(), "l": entry["l"].clone(), "p": page, "f": chosen }));
            }
        }
        let mut name = spec["n"].clone();
        if let Some(i) = spec["keep"].as_u64().map(|i| i as usize)
            && let Some(old) = old_parts.get(i)
        {
            kept.insert(i);
            if name.is_null() {
                name = old["n"].clone();
            }
            items.extend(
                old["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|it| remainder(it, &claimed, true)),
            );
        }
        parts.push(json!({ "n": name, "items": items }));
    }
    // The rest, in its extracted order and grouping.
    let rest: Vec<Value> = old_parts
        .iter()
        .enumerate()
        .filter(|(i, _)| !kept.contains(i))
        .flat_map(|(_, part)| part["items"].as_array().cloned().unwrap_or_default())
        .filter_map(|it| remainder(&it, &claimed, false))
        .collect();
    if !rest.is_empty() {
        let name = layout["rest"]
            .as_str()
            .unwrap_or("Not on the printed sheet");
        parts.push(json!({ "n": name, "items": rest }));
    }
    form["parts"] = Value::Array(parts);
}

/// Take the field `key` out of its item and append it to the item `fix.to`
/// names (`<part>:<item>`, or `<part>:new:<no>` for a printed item the
/// extractor left out). Nothing changes when the field or target is missing.
fn move_field(parts: &mut [Value], key: &str, fix: &Value) {
    let to: Vec<&str> = fix["to"].as_str().unwrap_or_default().split(':').collect();
    let Some(p) = to.first().and_then(|n| n.parse::<usize>().ok()) else {
        return;
    };
    let new_no = match to.as_slice() {
        [_, "new", no] => Some(*no),
        _ => None,
    };
    let index = to.get(1).and_then(|n| n.parse::<usize>().ok());
    let Some(items) = parts.get(p).and_then(|part| part["items"].as_array()) else {
        return;
    };
    if new_no.is_none() && index.is_none_or(|i| i >= items.len()) {
        return;
    }
    let mut taken = None;
    for part in parts.iter_mut() {
        for item in part["items"].as_array_mut().into_iter().flatten() {
            let page = item["p"].clone();
            if let Some(fields) = item["f"].as_array_mut()
                && let Some(n) = fields.iter().position(|f| f["k"] == key)
            {
                taken = Some((fields.remove(n), page));
            }
        }
    }
    let Some((field, page)) = taken else {
        return;
    };
    let Some(items) = parts[p]["items"].as_array_mut() else {
        return;
    };
    let i = match new_no {
        Some(no) => match items
            .iter()
            .position(|it| it["added"] == true && it["no"] == no && it["l"] == fix["item"]["l"])
        {
            Some(i) => i,
            None => {
                items.push(json!({
                    "no": no,
                    "l": fix["item"]["l"].clone(),
                    "p": page,
                    "f": [],
                    "added": true,
                    "order": fix["item"]["order"].clone(),
                }));
                items.len() - 1
            }
        },
        None => index.unwrap_or_default(),
    };
    match items[i]["f"].as_array_mut() {
        Some(fields) => fields.push(field),
        None => items[i]["f"] = Value::Array(vec![field]),
    }
}

/// A `table:<part>:<name>` correction: the row-template fields `templates`
/// of part `<part>` (keys with `{index}`, in column order) become a schedule
/// table appended to that part, with rows `rows[0]..=rows[1]` (`{index}`
/// replaced by the row number, in the template key or in its `row_keys`
/// pattern when the official row key differs) and `cols` captions; the
/// template fields are hidden. Appending keeps every existing
/// `item:<part>:<index>` correction pointing at the same item. Each row key
/// must be a box the agent definition binds (a test checks).
fn append_table(parts: &mut [Value], spec: &str, fix: &Value) {
    let Some(at) = spec
        .split(':')
        .next()
        .and_then(|p| p.parse::<usize>().ok())
        .filter(|p| *p < parts.len())
    else {
        return;
    };
    let wanted: Vec<&str> = fix["templates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let (first, last) = (
        fix["rows"][0].as_u64().unwrap_or(1),
        fix["rows"][1].as_u64().unwrap_or(0),
    );
    let captions = fix["cols"].as_array().cloned().unwrap_or_default();
    // Find every template first; hide them only when the table can be built.
    let mut found: BTreeMap<&str, Value> = BTreeMap::new();
    let mut page = Value::Null;
    for item in parts[at]["items"].as_array().into_iter().flatten() {
        for field in item["f"].as_array().into_iter().flatten() {
            let key = field["k"].as_str().unwrap_or_default();
            if let Some(name) = wanted.iter().find(|w| **w == key) {
                found.insert(name, field.clone());
                page = item["p"].clone();
            }
        }
        // A row template can also be a cell of an extracted table (2550Q's
        // Items 42, 47 and 56 popups sit in the Item 42 table's columns);
        // it takes that column's type, and an override hides the column.
        for row in item["keys"].as_array().into_iter().flatten() {
            for (c, cell) in row.as_array().into_iter().flatten().enumerate() {
                let key = cell.as_str().unwrap_or_default();
                if let Some(name) = wanted.iter().find(|w| **w == key) {
                    let col = &item["cols"][c];
                    let mut field = json!({"k": key, "l": col["l"].clone(), "t": col["t"].clone()});
                    if col.get("c").is_some() {
                        field["c"] = col["c"].clone();
                    }
                    found.insert(name, field);
                    page = item["p"].clone();
                }
            }
        }
    }
    let templates: Vec<Value> = wanted.iter().filter_map(|k| found.remove(k)).collect();
    if templates.is_empty() || templates.len() != wanted.len() {
        return;
    }
    let cols: Vec<Value> = templates
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let mut col = json!({
                "l": captions.get(i).cloned().unwrap_or_else(|| field["l"].clone()),
                "t": field["t"].clone(),
            });
            if field.get("c").is_some() {
                col["c"] = field["c"].clone();
            }
            col
        })
        .collect();
    let keys: Vec<Value> = (first..=last)
        .map(|row| {
            Value::Array(
                templates
                    .iter()
                    .map(|field| {
                        let template = field["k"].as_str().unwrap_or_default();
                        let pattern = fix["row_keys"][template].as_str().unwrap_or(template);
                        Value::from(pattern.replace("{index}", &row.to_string()))
                    })
                    .collect(),
            )
        })
        .collect();
    // A concrete row field the extractor listed is a table cell now: drop
    // the loose copy, in whichever part it sits (one control, one place;
    // 2550Q splits its first two schedule rows across two parts). Row
    // templates stay, hidden.
    let cells: BTreeSet<&str> = keys
        .iter()
        .flat_map(|row| row.as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    for item in parts
        .iter_mut()
        .flat_map(|part| part["items"].as_array_mut().into_iter().flatten())
    {
        if let Some(fields) = item["f"].as_array_mut() {
            fields.retain(|field| !cells.contains(field["k"].as_str().unwrap_or_default()));
            for field in fields.iter_mut() {
                if wanted.contains(&field["k"].as_str().unwrap_or_default()) {
                    field["t"] = Value::from("hidden");
                }
            }
        }
    }
    if let Some(items) = parts[at]["items"].as_array_mut() {
        let mut table = json!({
            "no": fix["no"].clone(),
            "l": fix["label"].clone(),
            "p": page,
            "cols": cols,
            "rows": keys.len(),
            "keys": keys,
        });
        // `start`: the number the first row shows, when a continuation
        // table picks up after the printed rows (1707A's row 4 onward).
        if let Some(start) = fix["start"].as_u64() {
            table["start"] = Value::from(start);
        }
        items.push(table);
    }
}

/// `1601-C`, `form 1601c` -> `1601C`.
fn normalize_code(code: &str) -> String {
    let upper = code.trim().to_ascii_uppercase();
    let upper = upper.strip_prefix("FORM").unwrap_or(&upper);
    upper.chars().filter(char::is_ascii_alphanumeric).collect()
}

fn schema_entry(form_code: &str) -> Option<&'static Value> {
    let wanted = normalize_code(form_code);
    schema()
        .iter()
        .find(|form| form["c"].as_str().map(normalize_code).as_deref() == Some(&wanted))
}

/// Every control key `k` the schema shows for a form (fields and table cells).
fn schema_keys(form_code: &str) -> &'static BTreeSet<String> {
    static KEYS: OnceLock<BTreeMap<String, BTreeSet<String>>> = OnceLock::new();
    static EMPTY: BTreeSet<String> = BTreeSet::new();
    let keys = KEYS.get_or_init(|| {
        schema()
            .iter()
            .map(|form| {
                let mut keys = BTreeSet::new();
                let items = form["parts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|part| part["items"].as_array().into_iter().flatten());
                for item in items {
                    for field in item["f"].as_array().into_iter().flatten() {
                        if let Some(key) = field["k"].as_str() {
                            keys.insert(key.to_string());
                        }
                    }
                    for row in item["keys"].as_array().into_iter().flatten() {
                        for key in row.as_array().into_iter().flatten() {
                            if let Some(key) = key.as_str() {
                                keys.insert(key.to_string());
                            }
                        }
                    }
                }
                (normalize_code(form["c"].as_str().unwrap_or_default()), keys)
            })
            .collect()
    });
    keys.get(&normalize_code(form_code)).unwrap_or(&EMPTY)
}

fn screen_form_schema(arguments: &HostArguments) -> HostResult {
    let code = arguments.string(0)?;
    schema_entry(code)
        .cloned()
        .map(json)
        .ok_or_else(|| host_error(format!("no on-screen schema for form {code}")))
}

fn screen_forms_index() -> HostResult {
    Ok(HostValue::Array(
        schema()
            .iter()
            .map(|form| {
                json(json!({
                    "c": form["c"],
                    "t": form["t"],
                    "rev": form["rev"],
                    "freq": form["freq"],
                    "pages": form["pages"],
                    "n": form["n"],
                    "q": form["q"],
                }))
            })
            .collect(),
    ))
}

// ----------------------------------------------------------- open drafts

fn kind_of(form_code: &str) -> Result<AgentFormKind, HostError> {
    AgentFormKind::from_code(form_code).ok_or_else(|| {
        host_error(format!(
            "unknown form `{form_code}`; use one of the 43 BIR form codes, e.g. 1601C"
        ))
    })
}

/// The period a form's page opens for: month 1-12, quarter 1-4, `0` for an
/// annual return, the open-ended key (at least 1) otherwise.
fn page_period(code: &str, period: i64) -> Result<u8, String> {
    let frequency = bir_core::forms::find_form(code)
        .map(|form| form.frequency.clone())
        .unwrap_or(FilingFrequency::Annual);
    let period =
        u8::try_from(period).map_err(|_| format!("{code}: period {period} is out of range"))?;
    match frequency {
        FilingFrequency::Monthly if (1..=12).contains(&period) => Ok(period),
        FilingFrequency::Monthly => Err(format!("{code} is monthly: period must be 1-12")),
        FilingFrequency::Quarterly if (1..=4).contains(&period) => Ok(period),
        FilingFrequency::Quarterly => Err(format!("{code} is quarterly: period must be 1-4")),
        FilingFrequency::Annual => Ok(0),
        FilingFrequency::OpenEnded => Ok(period.max(1)),
    }
}

fn read_error(code: &str, error: impl std::fmt::Display) -> String {
    format!("the stored {code} draft could not be read: {error}")
}

/// The stored generic-queue draft in `period`'s column.
fn stored<F: QueueableForm>(
    db: &Database,
    tin: &str,
    year: u16,
    period: i64,
) -> Result<Option<F>, String> {
    db.get_queueable_draft::<F>(tin, year, period)
        .map_err(|error| read_error(F::FORM_CODE, error))
}

/// The most recently updated stored draft among `periods` that `keep` accepts.
fn latest<F: QueueableForm>(
    db: &Database,
    tin: &str,
    year: u16,
    periods: impl IntoIterator<Item = i64>,
    keep: impl Fn(&F) -> bool,
) -> Result<Option<F>, String> {
    let mut best: Option<F> = None;
    for period in periods {
        if let Some(draft) = stored::<F>(db, tin, year, period)?
            && keep(&draft)
            && best
                .as_ref()
                .is_none_or(|current| draft.lifecycle().updated_at > current.lifecycle().updated_at)
        {
            best = Some(draft);
        }
    }
    Ok(best)
}

/// An annual return stored before the generic queue (NULL period column,
/// period key `A`): the first save adopts the row.
fn stored_annual<F: QueueableForm>(
    db: &Database,
    tin: &str,
    year: u16,
) -> Result<Option<F>, String> {
    match stored::<F>(db, tin, year, 0)? {
        Some(draft) => Ok(Some(draft)),
        None => db
            .get_form_draft_v2::<F>(tin, F::FORM_CODE, year, &FilingPeriod::Annual)
            .map_err(|error| read_error(F::FORM_CODE, error)),
    }
}

/// Penalties measured from the Calendar's deadline with the profile's EOPT
/// class; an open Draft shows today's values (the painted `prepare_penalties`).
fn finish<F: QueueableForm>(
    db: &Database,
    profile: &TaxpayerProfile,
    mut draft: F,
) -> Result<Value, String> {
    if !F::penalty_items().is_empty() {
        draft.apply_penalty_profile(profile, &db.get_deadline_overrides());
        if draft.lifecycle().is_editable() {
            draft.compute();
        }
    }
    serde_json::to_value(draft).map_err(|error| error.to_string())
}

fn month_or(period: u8, fallback: u8) -> u8 {
    if (1..=12).contains(&period) {
        period
    } else {
        fallback
    }
}

/// The draft each painted page opens for `(profile, year, period)`: the
/// stored one, else a new one from the profile.
fn load_draft(
    db: &Database,
    kind: AgentFormKind,
    profile: &TaxpayerProfile,
    year: u16,
    period: u8,
) -> Result<Value, String> {
    use bir_core::forms::*;
    let tin = profile.tin.full();
    let tin = tin.as_str();
    let queueable = match kind {
        AgentFormKind::Form1601C => {
            let month = period.clamp(1, 12);
            let draft = match db
                .get_1601c_draft(tin, year, month)
                .map_err(|error| read_error("1601C", error))?
            {
                Some(draft) => draft,
                None => {
                    form_1601c::Form1601CDraft::new_unanswered_from_profile(profile, year, month)
                }
            };
            return serde_json::to_value(draft).map_err(|error| error.to_string());
        }
        AgentFormKind::Form2551Q => {
            let quarter = period.clamp(1, 4);
            let draft = match db
                .get_2551q_draft(tin, year, quarter)
                .map_err(|error| read_error("2551Q", error))?
            {
                Some(draft) => draft,
                None => form_2551q::Form2551QDraft::new_from_profile(profile, year, quarter),
            };
            return serde_json::to_value(draft).map_err(|error| error.to_string());
        }
        AgentFormKind::Queueable(kind) => kind,
    };
    let column = i64::from(period);
    macro_rules! simple {
        ($ty:ty, $new:expr) => {{
            let draft: $ty = stored::<$ty>(db, tin, year, column)?.unwrap_or_else(|| $new);
            finish(db, profile, draft)
        }};
    }
    match queueable {
        QueueableKind::Form2550Q => {
            let quarter = period.clamp(1, 4);
            let mut draft = db
                .get_form_draft::<form_2550q::Form2550QDraft>(tin, "2550Q", year, Some(quarter))
                .map_err(|error| read_error("2550Q", error))?
                .unwrap_or_else(|| {
                    form_2550q::Form2550QDraft::new_from_profile(profile, year, quarter)
                });
            bir_core::form_rules::prepare_form_2550q_live_row_identities(&mut draft)
                .map_err(|error| format!("the 2550Q draft's row identities are unsafe: {error}"))?;
            serde_json::to_value(draft).map_err(|error| error.to_string())
        }
        QueueableKind::Form0605 => {
            type F = form_0605::Form0605Draft;
            let slot = period.max(1);
            let draft = match stored::<F>(db, tin, year, i64::from(slot))? {
                Some(draft) => Some(draft),
                None => db
                    .get_form_draft_v2::<F>(
                        tin,
                        "0605",
                        year,
                        &FilingPeriod::OpenEnded(u32::from(slot)),
                    )
                    .map_err(|error| read_error("0605", error))?,
            }
            .unwrap_or_else(|| F::new_from_profile(profile, year, slot));
            finish(db, profile, draft)
        }
        QueueableKind::Form0619E => simple!(
            form_0619e::Form0619EDraft,
            form_0619e::Form0619EDraft::new_from_profile(profile, year, period)
        ),
        QueueableKind::Form0619F => simple!(
            form_0619f::Form0619FDraft,
            form_0619f::Form0619FDraft::new_from_profile(profile, year, period)
        ),
        QueueableKind::Form1600PT => simple!(
            form_1600pt::Form1600PtDraft,
            form_1600pt::Form1600PtDraft::new_from_profile(profile, year, period.clamp(1, 12))
        ),
        QueueableKind::Form1600VT => simple!(
            form_1600vt::Form1600VtDraft,
            form_1600vt::Form1600VtDraft::new_from_profile(profile, year, period.clamp(1, 12))
        ),
        QueueableKind::Form1600WP => simple!(
            form_1600wp::Form1600WpDraft,
            form_1600wp::Form1600WpDraft::new_from_profile(profile, year, period.clamp(1, 12))
        ),
        QueueableKind::Form1601EQ => simple!(
            form_1601eq::Form1601EqDraft,
            form_1601eq::Form1601EqDraft::new_from_profile(profile, year, period.clamp(1, 4))
        ),
        QueueableKind::Form1601FQ => simple!(
            form_1601fq::Form1601FqDraft,
            form_1601fq::Form1601FqDraft::new_from_profile(profile, year, period.clamp(1, 4))
        ),
        QueueableKind::Form1602Q => simple!(
            form_1602q::Form1602QDraft,
            form_1602q::Form1602QDraft::new_from_profile(profile, year, period.clamp(1, 4))
        ),
        QueueableKind::Form1603Q => simple!(
            form_1603q::Form1603QDraft,
            form_1603q::Form1603QDraft::new_from_profile(profile, year, period.clamp(1, 4))
        ),
        QueueableKind::Form1604C => simple!(
            form_1604c::Form1604cDraft,
            form_1604c::Form1604cDraft::new_from_profile(profile, year)
        ),
        QueueableKind::Form1604E => simple!(
            form_1604e::Form1604eDraft,
            form_1604e::Form1604eDraft::new_from_profile(profile, year)
        ),
        QueueableKind::Form1604F => simple!(
            form_1604f::Form1604fDraft,
            form_1604f::Form1604fDraft::new_from_profile(profile, year)
        ),
        QueueableKind::Form1606 => simple!(
            form_1606::Form1606Draft,
            form_1606::Form1606Draft::new_from_profile(profile, year, u32::from(period.max(1)))
        ),
        QueueableKind::Form1700 => simple!(
            form_1700::Form1700Draft,
            form_1700::Form1700Draft::new_from_profile(profile, year)
        ),
        QueueableKind::Form1701 => {
            type F = form_1701::Form1701Draft;
            let draft = stored_annual::<F>(db, tin, year)?.unwrap_or_else(|| {
                let mut draft = F::new_from_profile(profile, year, 12);
                draft.recompute();
                draft
            });
            finish(db, profile, draft)
        }
        QueueableKind::Form1701A => simple!(
            form_1701a::Form1701ADraft,
            form_1701a::Form1701ADraft::new_from_profile(profile, year)
        ),
        QueueableKind::Form1701MS => simple!(
            form_1701ms::Form1701MsDraft,
            form_1701ms::Form1701MsDraft::new_from_profile(profile, year)
        ),
        QueueableKind::Form1701Q => simple!(form_1701q::Form1701QDraft, {
            let mut draft =
                form_1701q::Form1701QDraft::new_from_profile(profile, year, period.clamp(1, 3));
            draft.recompute();
            draft
        }),
        QueueableKind::Form1702EX => simple!(
            form_1702ex::Form1702ExDraft,
            form_1702ex::Form1702ExDraft::new_from_profile(profile, year)
        ),
        QueueableKind::Form1702MX => {
            type F = form_1702mx::Form1702MXDraft;
            let draft = stored_annual::<F>(db, tin, year)?.unwrap_or_else(|| {
                let mut draft = F::new_from_profile(profile, year);
                draft.recompute();
                draft
            });
            finish(db, profile, draft)
        }
        QueueableKind::Form1702Q => simple!(form_1702q::Form1702qDraft, {
            let quarter = if (1..=3).contains(&period) { period } else { 1 };
            form_1702q::Form1702qDraft::new_from_profile(profile, year, quarter)
        }),
        QueueableKind::Form1702RT => {
            type F = form_1702rt::Form1702RTDraft;
            let draft = stored_annual::<F>(db, tin, year)?.unwrap_or_else(|| {
                let mut draft = F::new_from_profile(profile, year, 12);
                draft.part_iv.item_40_income_tax_rate_percent = 30;
                draft.deduction_method = form_1702rt::Form1702RTDeductionMethod::Itemized;
                draft.recompute();
                draft
            });
            finish(db, profile, draft)
        }
        QueueableKind::Form1706 => simple!(
            form_1706::Form1706Draft,
            form_1706::Form1706Draft::new_from_profile(profile, year, u32::from(period.max(1)))
        ),
        QueueableKind::Form1707 => simple!(
            form_1707::Form1707Draft,
            form_1707::Form1707Draft::new_from_profile(profile, year, u32::from(period.max(1)))
        ),
        QueueableKind::Form1707A => simple!(
            form_1707a::Form1707ADraft,
            form_1707a::Form1707ADraft::new_from_profile(profile, year, u32::from(period.max(1)))
        ),
        QueueableKind::Form1800 => {
            type F = form_1800::Form1800Draft;
            let mut draft = stored::<F>(db, tin, year, column)?
                .unwrap_or_else(|| F::new_from_profile(profile, year, u32::from(period.max(1))));
            // A TIN fixed in the profile reaches a still-editable draft.
            if draft.lifecycle().is_editable() {
                draft.tin = tin.to_string();
            }
            finish(db, profile, draft)
        }
        QueueableKind::Form1801 => {
            type F = form_1801::Form1801Draft;
            let mut draft = stored::<F>(db, tin, year, column)?
                .unwrap_or_else(|| F::new_from_profile(profile, year, u32::from(period.max(1))));
            if draft.lifecycle().is_editable() {
                draft.tin = tin.to_string();
            }
            finish(db, profile, draft)
        }
        QueueableKind::Form2000 => simple!(
            form_2000::Form2000Draft,
            form_2000::Form2000Draft::new_from_profile(profile, year, period.clamp(1, 12))
        ),
        QueueableKind::Form2000OT => {
            // One return per transaction date (`MMDD`): this year's latest
            // editable draft, else a new transaction.
            type F = form_2000ot::Form2000OTDraft;
            let draft = latest::<F>(db, tin, year, mmdd_keys(0..=12, 0..=31), |draft| {
                draft.lifecycle().is_editable()
            })?
            .unwrap_or_else(|| F::new_from_profile(profile, year));
            finish(db, profile, draft)
        }
        QueueableKind::Form2200A => simple!(
            form_2200a::Form2200ADraft,
            form_2200a::Form2200ADraft::new_from_profile(profile, year, period)
        ),
        QueueableKind::Form2200AN => {
            type F = form_2200an::Form2200ANDraft;
            let month = i64::from(period.clamp(1, 12));
            let draft = latest::<F>(db, tin, year, mmdd_keys(month..=month, 1..=31), |_| true)?
                .unwrap_or_else(|| F::new_from_profile(profile, year, period));
            finish(db, profile, draft)
        }
        QueueableKind::Form2200C => {
            type F = form_2200c::Form2200CDraft;
            let months = if (1..=12).contains(&period) {
                i64::from(period)..=i64::from(period)
            } else {
                1..=12
            };
            let draft = latest::<F>(db, tin, year, mmdd_keys(months, 1..=31), |_| true)?
                .unwrap_or_else(|| F::new_for_month(profile, year, period));
            finish(db, profile, draft)
        }
        QueueableKind::Form2200M => simple!(
            form_2200m::Form2200MDraft,
            form_2200m::Form2200MDraft::new_from_profile(profile, year, period)
        ),
        QueueableKind::Form2200P => simple!(
            form_2200p::Form2200PDraft,
            form_2200p::Form2200PDraft::new_from_profile(profile, year, period)
        ),
        QueueableKind::Form2200S => {
            type F = form_2200s::Form2200SDraft;
            let month = i64::from(period.clamp(1, 12));
            let draft = latest::<F>(db, tin, year, mmdd_keys(month..=month, 1..=31), |_| true)?
                .unwrap_or_else(|| F::new_from_profile(profile, year, period));
            finish(db, profile, draft)
        }
        QueueableKind::Form2200T => simple!(
            form_2200t::Form2200TDraft,
            form_2200t::Form2200TDraft::new_from_profile(profile, year, period)
        ),
        QueueableKind::Form2550M => simple!(
            form_2550m::Form2550MDraft,
            form_2550m::Form2550MDraft::new_from_profile(profile, year, month_or(period, 1))
        ),
        QueueableKind::Form2552 => {
            // Keyed by its Item 1 date: this year's latest return still in
            // progress (month/day 0 is a blank component), else a new one.
            type F = form_2552::Form2552Draft;
            let draft = latest::<F>(db, tin, year, mmdd_keys(0..=12, 0..=31), |draft| {
                !matches!(
                    draft.lifecycle().status,
                    FilingStatus::Confirmed | FilingStatus::Paid
                )
            })?
            .unwrap_or_else(|| F::new_from_profile(profile, year, 0, 0));
            finish(db, profile, draft)
        }
        QueueableKind::Form2553 => {
            // The return names its own quarter (Item 3): this year's latest.
            type F = form_2553::Form2553Draft;
            let quarter = if (1..=4).contains(&period) { period } else { 0 };
            let draft = latest::<F>(db, tin, year, 1..=4, |_| true)?
                .unwrap_or_else(|| F::new_from_profile(profile, year, quarter));
            finish(db, profile, draft)
        }
    }
}

/// `MMDD` period keys for every month and day in the ranges.
fn mmdd_keys(
    months: std::ops::RangeInclusive<i64>,
    days: std::ops::RangeInclusive<i64>,
) -> impl Iterator<Item = i64> {
    months.flat_map(move |month| days.clone().map(move |day| month * 100 + day))
}

// ------------------------------------------------------- draft evaluation

/// Why a box on the form cannot be edited here, or `None` when it can (or
/// when it is a hidden control, which the page never shows).
fn lock_reason(def: &AgentFormDef, entry: &BoxDef) -> Option<String> {
    if entry.computed || entry.required == "computed" {
        return Some("computed by BIR's own formulas".to_string());
    }
    if !entry.is_user_box() {
        return None;
    }
    // An income-tax election is the user's to answer: it is locked against
    // the assistant only, so the page leaves it open.
    let bound_lock = |key: &str| match def.bindings.get(key) {
        Some(BoundBox {
            lock: Some(lock), ..
        }) if *lock != bir_core::forms::agent::Lock::Election => Some(lock.reason().to_string()),
        _ => None,
    };
    if def.bindings.contains_key(&entry.key) {
        // A radio is locked when its item (group box) is.
        return bound_lock(&entry.key).or_else(|| entry.group.as_deref().and_then(bound_lock));
    }
    Some(format!(
        "cannot be filled here: {}",
        def.unmapped
            .get(&entry.key)
            .map(String::as_str)
            .unwrap_or("it has no field in this app's draft")
    ))
}

fn locks(kind: AgentFormKind) -> Map<String, Value> {
    let def = kind.definition();
    let mut locks: Map<String, Value> = def
        .boxes
        .iter()
        .filter_map(|entry| {
            lock_reason(def, entry)
                .filter(|_| !catalogs::host_writes(kind, &entry.key))
                .map(|reason| (entry.key.clone(), Value::String(reason)))
        })
        .collect();
    // A duplicate occurrence shows its official field (see `values_json`)
    // but is never written by key: say where that field is edited instead
    // of "not written by this app's field map".
    for (key, official) in occurrence_aliases(kind) {
        let reason = match def.bindings.get(official) {
            Some(BoundBox {
                lock: Some(lock), ..
            }) => lock.reason().to_string(),
            Some(_) => format!("repeats `{official}`; fill it there"),
            None => {
                format!("repeats the official field `{official}`, which this page cannot write")
            }
        };
        locks.insert(key.clone(), Value::String(reason));
    }
    locks
}

/// Boxes the taxpayer profile fills: bound (themselves or through their
/// item's group box) with a profile lock, plus repeated occurrences of one.
/// The extracted schema's auto flag is a guess; this is what the app does.
fn profile_keys(kind: AgentFormKind) -> Vec<String> {
    let def = kind.definition();
    let profile = |key: &str| {
        matches!(
            def.bindings.get(key),
            Some(BoundBox {
                lock: Some(bir_core::forms::agent::Lock::Profile),
                ..
            })
        )
    };
    let mut keys: Vec<String> = def
        .boxes
        .iter()
        .filter(|entry| {
            !entry.computed
                && entry.is_user_box()
                && def.bindings.contains_key(&entry.key)
                && (profile(&entry.key) || entry.group.as_deref().is_some_and(profile))
        })
        .map(|entry| entry.key.clone())
        .collect();
    for (key, official) in occurrence_aliases(kind) {
        if profile(official) {
            keys.push(key.clone());
        }
    }
    keys
}

/// On-screen keys that repeat an official control id (`txtEmail#occurrence-2`,
/// the rules corpus's name for the same `serialized_key` placed again on the
/// official form) and have no binding of their own, each with that official
/// key. The draft holds one value per official key and the field map, the
/// XML and the print sheet all write it under the official key, so every
/// occurrence shows that one value. Read-only: `fill` still refuses these
/// keys, so every write goes through a bound box of the typed draft.
fn occurrence_aliases(kind: AgentFormKind) -> Vec<(&'static String, &'static str)> {
    let def = kind.definition();
    schema_keys(kind.form_code())
        .iter()
        .filter(|key| !def.bindings.contains_key(*key))
        .filter_map(|key| {
            key.split_once("#occurrence-")
                .map(|(official, _)| (key, official))
        })
        .collect()
}

/// A validation error with the schema keys it names: a radio item's group
/// box becomes its radios, since the page binds each radio by its own key.
fn errors_json(kind: AgentFormKind, errors: &[(String, String)]) -> Value {
    let def = kind.definition();
    let on_screen = schema_keys(kind.form_code());
    Value::Array(
        errors
            .iter()
            .map(|(field, message)| {
                let mut keys: Vec<String> = Vec::new();
                for key in agent::error_keys(kind, field) {
                    match def.box_def(&key) {
                        Some(entry) if entry.radio_group => {
                            keys.extend(entry.members.iter().cloned())
                        }
                        _ => keys.push(key),
                    }
                }
                // A 2550M schedule row has no draft binding of its own.
                keys.extend(rows_2550m::error_keys(kind, field));
                // Keys the page cannot bind (not in the schema) are dropped;
                // the error still carries its field and message.
                let mut seen = BTreeSet::new();
                keys.retain(|key| on_screen.contains(key) && seen.insert(key.clone()));
                json!({"field": field, "message": message, "keys": keys})
            })
            .collect(),
    )
}

/// Every box as official key -> string, plus each duplicate occurrence the
/// page shows (`txtEmail#occurrence-1`) with its official key's value.
fn values_json(kind: AgentFormKind, draft: &Value) -> Result<Value, String> {
    let mut map = agent::field_map(kind, draft)?;
    // A row family's row the upload leaves out still shows what the draft
    // holds: 1702RT writes its "(Add more...)" rows only once two or more
    // are saved, but a single row is already the filer's entry.
    let def = kind.definition();
    for entry in def.boxes.iter().filter(|entry| entry.family.is_some()) {
        if map.contains_key(&entry.key) || catalogs::inactive_schedule(kind, &entry.key, draft) {
            continue;
        }
        let path = match def.bindings.get(&entry.key).map(|bound| &bound.binding) {
            Some(
                Binding::Text { path, .. }
                | Binding::Amount { path, .. }
                | Binding::Whole { path, .. },
            ) => path,
            _ => continue,
        };
        let shown = match draft.pointer(path) {
            Some(Value::String(text)) => text.clone(),
            Some(Value::Number(number)) => number.to_string(),
            _ => continue,
        };
        map.insert(entry.key.clone(), shown);
    }
    // 1702RT Item 5: the official select keeps IC010 even while "Other" is
    // not selected (the upload keeps that raw default); the page shows the
    // election, as the print does.
    if kind == AgentFormKind::Queueable(QueueableKind::Form1702RT)
        && let Ok(atc) = serde_json::from_value::<
            bir_core::forms::form_1702rt::Form1702RTAtcSelection,
        >(draft["atc"].clone())
    {
        map.insert(
            "frm1702RT:drpPg1I5AtcOther".into(),
            atc.elected_other_code(),
        );
    }
    for (key, official) in occurrence_aliases(kind) {
        if !map.contains_key(key)
            && let Some(value) = map.get(official).cloned()
        {
            map.insert(key.clone(), value);
        }
    }
    Ok(Value::Object(
        map.into_iter()
            .map(|(key, value)| (key, Value::String(value)))
            .collect(),
    ))
}

fn status_name(status: &FilingStatus) -> String {
    format!("{status:?}")
}

/// Stack for form evaluation. The async halves run on the executor's worker
/// threads (512 KB on macOS); a form's validators compile regexes on first
/// use, and in debug builds that alone overflowed the worker stack (2551Q's
/// phone check took the app down with SIGILL).
const EVALUATE_STACK: usize = 16 * 1024 * 1024;

/// Run `work` on a thread of its own with [`EVALUATE_STACK`]. The async
/// halves call it for everything that reads the typed draft: deserializing a
/// large form (1702MX) in a debug build overflows a 512 KB worker stack too,
/// and GPUI Fast runs host futures on those workers.
fn on_large_stack<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    std::thread::Builder::new()
        .name("screen-form-evaluate".into())
        .stack_size(EVALUATE_STACK)
        .spawn(work)
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "validation stopped unexpectedly".to_string())
}

/// Recompute and validate an editable draft in place; a filed return is
/// validated on a copy so it keeps the values it was filed with.
fn evaluate_here(
    kind: AgentFormKind,
    draft: &mut Value,
) -> Result<(Value, Value, FilingStatus), String> {
    let status = agent::identity(kind, draft)?.status;
    let errors = if status == FilingStatus::Draft {
        agent::validate(kind, draft)?
    } else {
        agent::validate(kind, &mut draft.clone())?
    };
    Ok((
        values_json(kind, draft)?,
        errors_json(kind, &errors),
        status,
    ))
}

// --------------------------------------------------------- host functions

fn draft_argument(arguments: &HostArguments, index: usize) -> Result<Value, HostError> {
    let draft = to_json(arguments.value(index)?);
    if !draft.is_object() {
        return Err(host_error("draft must be an object"));
    }
    Ok(draft)
}

fn screen_form_open(
    arguments: &HostArguments,
) -> Result<impl Future<Output = HostResult> + Send + use<>, HostError> {
    let tin: String = arguments
        .string(0)?
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let kind = kind_of(arguments.string(1)?)?;
    let year =
        u16::try_from(arguments.integer(2)?).map_err(|_| host_error("year is out of range"))?;
    let code = kind.form_code();
    let period = page_period(code, arguments.integer(3)?).map_err(host_error)?;
    let draft = with_database(|db| {
        let profile = db
            .get_profile_by_tin(&tin)
            .map_err(|error| host_error(error.to_string()))?
            .ok_or_else(|| host_error(format!("no taxpayer profile with TIN {tin}")))?;
        load_draft(db, kind, &profile, year, period).map_err(host_error)
    })?;
    Ok(async move {
        let opened = on_large_stack(move || -> Result<Value, String> {
            let mut draft = draft;
            let (values, errors, status) = evaluate_here(kind, &mut draft)?;
            Ok(json!({
                "form_code": code,
                "year": year,
                "period": period,
                "status": status_name(&status),
                "editable": status == FilingStatus::Draft,
                "stored_period": stored_period(kind, &draft),
                "draft": draft,
                "values": values,
                "options": catalogs::options(kind, &draft),
                "locks": locks(kind),
                "profile": profile_keys(kind),
                "errors": errors,
            }))
        })
        .and_then(|result| result)
        .map_err(host_error)?;
        Ok(json(opened))
    })
}

fn screen_form_fill(arguments: &HostArguments) -> HostResult {
    let kind = kind_of(arguments.string(0)?)?;
    let mut draft = draft_argument(arguments, 1)?;
    let edits = arguments
        .value(2)?
        .as_object()
        .ok_or_else(|| host_error("edits must be an object of key -> value"))?;
    let editable = agent::identity(kind, &draft).map_err(host_error)?.status == FilingStatus::Draft;
    let mut rejected = Map::new();
    // Location parents precede their children, including a pasted batch
    // whose JSON keys happen to sort city/province/region.
    let mut edits: Vec<_> = edits.iter().collect();
    edits.sort_by_key(|(key, _)| catalogs::edit_order(kind, key));
    for (key, value) in edits {
        if !editable {
            rejected.insert(key.clone(), json!("this return is no longer a draft"));
            continue;
        }
        let mut next = draft.clone();
        match catalogs::fill(kind, &mut next, key, &to_json(value)) {
            Ok(()) => draft = next,
            Err(message) => {
                rejected.insert(key.clone(), Value::String(message));
            }
        }
    }
    if editable {
        // An added 2550Q row needs a stable row ID before it is saved; the
        // same step opening a draft runs.
        if kind == AgentFormKind::Queueable(QueueableKind::Form2550Q) {
            let mut typed: bir_core::forms::form_2550q::Form2550QDraft =
                serde_json::from_value(draft.clone()).map_err(|e| host_error(e.to_string()))?;
            bir_core::form_rules::prepare_form_2550q_live_row_identities(&mut typed).map_err(
                |e| host_error(format!("the 2550Q draft's row identities are unsafe: {e}")),
            )?;
            draft = serde_json::to_value(typed).map_err(|e| host_error(e.to_string()))?;
        }
        agent::compute(kind, &mut draft).map_err(host_error)?;
    }
    let values = values_json(kind, &draft).map_err(host_error)?;
    Ok(json(json!({
        "draft": draft,
        "values": values,
        "options": catalogs::options(kind, &draft),
        "rejected": rejected,
    })))
}

fn screen_form_validate(
    arguments: &HostArguments,
) -> Result<impl Future<Output = HostResult> + Send + use<>, HostError> {
    let kind = kind_of(arguments.string(0)?)?;
    let draft = draft_argument(arguments, 1)?;
    Ok(async move {
        let checked = on_large_stack(move || -> Result<Value, String> {
            let mut draft = draft;
            let (values, errors, _) = evaluate_here(kind, &mut draft)?;
            Ok(json!({
                "draft": draft,
                "values": values,
                "options": catalogs::options(kind, &draft),
                "errors": errors,
            }))
        })
        .and_then(|result| result)
        .map_err(host_error)?;
        Ok(json(checked))
    })
}

fn screen_form_save(arguments: &HostArguments) -> HostResult {
    let kind = kind_of(arguments.string(0)?)?;
    let mut draft = draft_argument(arguments, 1)?;
    let id = with_database(|db| agent::save_draft(kind, db, &mut draft).map_err(host_error))?;
    let status = agent::identity(kind, &draft).map_err(host_error)?.status;
    Ok(json(json!({
        "id": id,
        "status": status_name(&status),
        "stored_period": stored_period(kind, &draft),
        "draft": draft,
    })))
}

/// The print sheet's field map and receipt page for a draft as saved.
pub(crate) fn print_map(
    db: &Database,
    kind: AgentFormKind,
    draft: &Value,
) -> Result<
    (
        BTreeMap<String, String>,
        Option<bir_print::frozen_html::ReceiptPage>,
    ),
    HostError,
> {
    fn typed<F: serde::de::DeserializeOwned>(draft: &Value) -> Result<F, HostError> {
        serde_json::from_value(draft.clone()).map_err(|error| host_error(error.to_string()))
    }
    Ok(match kind {
        AgentFormKind::Form1601C => {
            let d: bir_core::forms::Form1601CDraft = typed(draft)?;
            let page = crate::bir_module::receipt_page(db, d.receipt_id, &d.email_address);
            (d.to_print_field_map(), page)
        }
        AgentFormKind::Form2551Q => {
            let d: bir_core::forms::Form2551QDraft = typed(draft)?;
            let page = crate::bir_module::receipt_page(db, d.receipt_id, &d.email);
            (d.to_print_field_map(), page)
        }
        AgentFormKind::Queueable(queueable) => bir_core::with_queueable_kind!(queueable, F => {
            let d: F = typed(draft)?;
            let page = crate::bir_module::receipt_page(
                db,
                d.lifecycle().receipt_id,
                d.submission_email(),
            );
            (d.print_field_map(), page)
        }),
    })
}

/// Print preview of the draft the page holds (save it first). Works from the
/// draft itself, so forms stored under a transaction-date key print too.
fn screen_form_print(arguments: &HostArguments) -> HostResult {
    let code = arguments.string(0)?.to_owned();
    let kind = kind_of(&code)?;
    let draft = draft_argument(arguments, 1)?;
    let slug = crate::bir_module::print_slug_for(kind.form_code())
        .ok_or_else(|| host_error(format!("Form {code} has no print sheet")))?;
    let year = agent::identity(kind, &draft)
        .map_err(host_error)?
        .taxable_year;
    with_database(|db| {
        let (fields, receipt) = print_map(db, kind, &draft)?;
        crate::bir_module::open_print_preview(
            slug,
            kind.form_code(),
            year,
            &fields,
            receipt.as_ref(),
        )
    })
}

/// Queue the saved draft the page holds for background submission. 1601C and
/// 2551Q use their own queue path (`queue_form_draft`); every other form is
/// queued from the draft itself, after it validates.
fn screen_form_queue(arguments: &HostArguments) -> HostResult {
    let code = arguments.string(0)?.to_owned();
    let kind = kind_of(&code)?;
    let draft = draft_argument(arguments, 1)?;
    let AgentFormKind::Queueable(queueable) = kind else {
        return Err(host_error(format!(
            "{code} is queued with queue_form_draft (month or quarter)"
        )));
    };
    with_database(|db| {
        bir_core::with_queueable_kind!(queueable, F => {
            let mut typed: F = serde_json::from_value(draft.clone())
                .map_err(|error| host_error(error.to_string()))?;
            if typed.draft_id().is_none() {
                return Err(host_error("Save the draft before queueing it."));
            }
            typed
                .queue(bir_core::filing_queue::QueueAuthSource::Gui)
                .map_err(|errors| {
                    host_error(
                        errors
                            .iter()
                            .map(|(field, message)| format!("{field}: {message}"))
                            .collect::<Vec<_>>()
                            .join("; "),
                    )
                })?;
            db.save_queued_queueable(&typed)
                .map(|id| HostValue::Number(id as f64))
                .map_err(|error| host_error(error.to_string()))
        })
    })
}

/// Take a queued return out of the queue so it is an editable draft again.
/// 1601C and 2551Q use `cancel_queued`.
fn screen_form_cancel_queue(arguments: &HostArguments) -> HostResult {
    let code = arguments.string(0)?.to_owned();
    let kind = kind_of(&code)?;
    let draft = draft_argument(arguments, 1)?;
    let AgentFormKind::Queueable(queueable) = kind else {
        return Err(host_error(format!(
            "{code} is cancelled with cancel_queued"
        )));
    };
    with_database(|db| {
        bir_core::with_queueable_kind!(queueable, F => {
            let typed: F = serde_json::from_value(draft.clone())
                .map_err(|error| host_error(error.to_string()))?;
            db.cancel_queued_queueable(&typed)
                .map(|_| HostValue::Null)
                .map_err(|error| host_error(error.to_string()))
        })
    })
}

/// The period column the draft is stored under, which is what the queue and
/// print host functions look it up by. It differs from the page's period on
/// the forms keyed by a transaction date (`MMDD`: 2000OT, 2200AN, 2200C,
/// 2200S, 2552) and by a quarter the return names itself.
fn stored_period(kind: AgentFormKind, draft: &Value) -> Option<i64> {
    agent::identity(kind, draft)
        .ok()
        .map(|identity| bir_core::forms::queueable::period_column(&identity.period))
}

#[cfg(test)]
mod tests {
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use super::*;
    use crate::bir_module::{module, use_ephemeral_database};

    const TIN: &str = "12345678800000";
    const YEAR: i64 = 2026;

    /// A fictional corporation that withholds and is VAT-registered.
    fn install_profile() {
        let profile: TaxpayerProfile = serde_json::from_value(json!({
            "id": null,
            "full_name": "Example Trading Corp",
            "tin": {"segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000"},
            "rdo_code": "039",
            "line_of_business": "Wholesale trading",
            "registered_address": "1 Example Street, Quezon City",
            "zip_code": "1100",
            "phone": "0281234567",
            "email": "books@example.com",
            "default_form_type": "1601C",
            "taxpayer_type": "Corporation",
            "is_vat_registered": true,
            "has_employees": true,
            "withholds_compensation": true,
            "withholds_expanded": true,
        }))
        .expect("fictional profile parses");
        with_database(|db| {
            db.save_profile(profile)
                .map(|_| ())
                .map_err(|error| HostError::new(error.to_string()))
        })
        .expect("profile saves");
    }

    fn setup() {
        use_ephemeral_database();
        install_profile();
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return output;
            }
        }
    }

    fn args(values: Vec<HostValue>) -> HostArguments {
        HostArguments::new(values)
    }

    fn call(name: &str, values: Vec<HostValue>) -> Result<Value, HostError> {
        module()
            .call(name, &args(values))
            .map(|value| to_json(&value))
    }

    fn call_async(name: &str, values: Vec<HostValue>) -> Result<Value, HostError> {
        let future = module().begin(name, &args(values))?;
        block_on(future).map(|value| to_json(&value))
    }

    fn open(code: &str, period: i64) -> Value {
        call_async(
            "screen_form_open",
            vec![TIN.into(), code.into(), YEAR.into(), period.into()],
        )
        .unwrap_or_else(|error| panic!("{code} opens: {error}"))
    }

    fn fill(code: &str, draft: &Value, edits: Value) -> Value {
        call(
            "screen_form_fill",
            vec![code.into(), json(draft.clone()), json(edits)],
        )
        .unwrap_or_else(|error| panic!("{code} fills: {error}"))
    }

    fn save(code: &str, draft: &Value) -> Value {
        call("screen_form_save", vec![code.into(), json(draft.clone())])
            .unwrap_or_else(|error| panic!("{code} saves: {error}"))
    }

    fn amount(values: &Value, key: &str) -> f64 {
        agent::parse_amount(values[key].as_str().unwrap_or_default())
            .unwrap_or_else(|| panic!("{key} is an amount: {:?}", values[key]))
    }

    fn assert_error_keys_on_screen(code: &str, errors: &Value) {
        let on_screen = schema_keys(code);
        for error in errors.as_array().expect("errors array") {
            for key in error["keys"].as_array().expect("keys array") {
                let key = key.as_str().expect("key string");
                assert!(
                    on_screen.contains(key),
                    "{code}: error key {key} is not in the schema"
                );
            }
        }
    }

    #[test]
    fn declarations_match_the_registered_functions() {
        use_ephemeral_database();
        module().validate().expect("bir module declarations agree");
        let module = module();
        assert!(module.is_async("screen_form_open"));
        assert!(module.is_async("screen_form_validate"));
        assert!(!module.is_async("screen_form_fill"));
    }

    #[test]
    fn schema_overrides_apply_to_labels_items_and_kinds() {
        let form = schema_entry("2551Q").unwrap();
        let field = |k: &str| {
            form["parts"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|p| p["items"].as_array().unwrap())
                .flat_map(|it| it["f"].as_array().cloned().unwrap_or_default())
                .find(|f| f["k"] == k)
                .unwrap()
        };
        assert_eq!(field("frm2551Qv2018:taxTreaty_1")["l"], "Yes");
        assert_eq!(field("frm2551Qv2018:taxTreaty_2")["l"], "No");
        assert_eq!(
            form["parts"][1]["items"][6]["l"],
            "Tax relief under Special Law or Treaty?"
        );
        let overrides: Value =
            serde_json::from_str(include_str!("../../../app/forms/schema-overrides.json")).unwrap();
        // Every override names a field, a table cell or an item that exists.
        for (code, fixes) in overrides.as_object().unwrap() {
            if code.starts_with('_') {
                continue;
            }
            let keys = schema_keys(code);
            for key in fixes.as_object().unwrap().keys() {
                assert!(
                    key.starts_with("item:")
                        || key.starts_with("table:")
                        || key.starts_with("add:")
                        || key.strip_prefix("move:").is_some_and(|k| keys.contains(k))
                        || key == "layout"
                        || keys.contains(key),
                    "{code}: override {key} names no control"
                );
            }
            // A printed-sheet layout names each control once, and only
            // controls the form has; what it leaves out is kept in the last
            // part, so the form loses nothing.
            if let Some(layout) = fixes.get("layout") {
                let mut seen = BTreeSet::new();
                let named = layout["parts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|p| p["items"].as_array().into_iter().flatten())
                    .flat_map(|it| it["k"].as_array().into_iter().flatten())
                    .filter_map(Value::as_str);
                for key in named {
                    assert!(keys.contains(key), "{code}: layout names {key}, no control");
                    assert!(seen.insert(key), "{code}: layout names {key} twice");
                }
                let form = schema().iter().find(|f| f["c"] == code.as_str()).unwrap();
                let built = form["parts"].as_array().unwrap();
                for (i, part) in layout["parts"].as_array().unwrap().iter().enumerate() {
                    // A `keep` part without its own name keeps the extracted one.
                    if !part["n"].is_null() {
                        assert_eq!(built[i]["n"], part["n"], "{code}: layout part {i} moved");
                    }
                    assert!(
                        built[i]["items"]
                            .as_array()
                            .is_some_and(|items| !items.is_empty()),
                        "{code}: layout part {i} came out empty"
                    );
                }
            }
        }
    }

    #[test]
    fn screen_form_queue_queues_a_saved_draft_and_refuses_the_rest() {
        setup();
        let opened = open("0619E", 7);
        let unsaved = call(
            "screen_form_queue",
            vec![
                "0619E".into(),
                crate::bir_module::json(opened["draft"].clone()),
            ],
        );
        assert!(unsaved.unwrap_err().message().contains("Save the draft"));
        let saved = save("0619E", &opened["draft"]);
        let queued = call(
            "screen_form_queue",
            vec![
                "0619E".into(),
                crate::bir_module::json(saved["draft"].clone()),
            ],
        )
        .unwrap_or_else(|error| panic!("0619E queues: {}", error.message()));
        assert!(queued.as_i64().is_some_and(|id| id > 0));
        let reopened = open("0619E", 7);
        assert_eq!(reopened["status"], "Queued");
        call(
            "screen_form_cancel_queue",
            vec![
                "0619E".into(),
                crate::bir_module::json(reopened["draft"].clone()),
            ],
        )
        .unwrap_or_else(|error| panic!("0619E cancels: {}", error.message()));
        assert_eq!(open("0619E", 7)["status"], "Draft");
        let c = open("1601C", 7);
        let refused = call(
            "screen_form_queue",
            vec!["1601C".into(), crate::bir_module::json(c["draft"].clone())],
        );
        assert!(refused.unwrap_err().message().contains("queue_form_draft"));
    }

    #[test]
    fn the_user_answers_item_13_election_on_2551q() {
        setup();
        let opened = open("2551Q", 3);
        assert!(
            opened["locks"].get("frm2551Qv2018:taxRate1").is_none(),
            "election radios are open"
        );
        let filled = fill(
            "2551Q",
            &opened["draft"],
            json!({"frm2551Qv2018:taxRate": "2"}),
        );
        assert!(
            filled["rejected"].as_object().unwrap().is_empty(),
            "{}",
            filled["rejected"]
        );
        assert_eq!(filled["draft"]["item_13_election"], "eight_percent");
    }

    #[test]
    fn print_finds_date_keyed_drafts_by_their_stored_period() {
        setup();
        for code in ["2000OT", "2200AN", "2200C", "2200S", "2552"] {
            let opened = open(code, 1);
            let saved = save(code, &opened["draft"]);
            assert!(
                saved["stored_period"].as_i64().is_some(),
                "{code} has a stored period"
            );
            assert_eq!(opened["stored_period"], saved["stored_period"], "{code}");
            with_database(|db| {
                let kind = kind_of(code)?;
                print_map(db, kind, &saved["draft"]).map(|_| ())
            })
            .unwrap_or_else(|error| panic!("{code} prints: {}", error.message()));
        }
    }

    #[test]
    fn repeated_occurrences_show_once_and_12a_is_a_dropdown() {
        let fields: Vec<Value> = schema_entry("2551Q").unwrap()["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .flat_map(|it| it["f"].as_array().cloned().unwrap_or_default())
            .collect();
        let kind = |k: &str| fields.iter().find(|f| f["k"] == k).unwrap()["t"].clone();
        assert_eq!(kind("txtEmail#occurrence-1"), "text");
        assert_eq!(kind("txtEmail#occurrence-2"), "hidden");
        let relief = fields
            .iter()
            .find(|f| f["k"] == "frm2551Qv2018:txtTaxReliefSpecify")
            .unwrap();
        assert_eq!(relief["t"], "sel");
        assert_eq!(relief["o"][1]["label"], "International Tax Treaty");
        // The codes are what the draft takes.
        setup();
        let opened = open("2551Q", 3);
        let filled = fill(
            "2551Q",
            &opened["draft"],
            json!({
                "frm2551Qv2018:taxTreaty_1": "true",
                "frm2551Qv2018:taxTreaty_2": "false",
                "frm2551Qv2018:txtTaxReliefSpecify": "2"
            }),
        );
        assert!(
            filled["rejected"].as_object().unwrap().is_empty(),
            "{}",
            filled["rejected"]
        );
        assert_eq!(filled["values"]["frm2551Qv2018:txtTaxReliefSpecify"], "2");
    }

    #[test]
    fn catalog_rdo_labels_preserve_profile_codes_and_locks() {
        setup();
        let opened = open("0619E", 7);
        let key = "frm0619E:txtRDOCode";
        let rows = opened["options"][key].as_array().expect("RDO options");
        let qc = rows.iter().find(|o| o["value"] == "039").unwrap();
        assert!(qc["label"].as_str().unwrap().contains("Quezon City"));
        assert_eq!(opened["values"][key], "039");
        let refused = fill("0619E", &opened["draft"], json!({key: "040"}));
        assert!(refused["rejected"].get(key).is_some());
        assert_eq!(refused["values"][key], "039");
    }

    #[test]
    fn catalog_locations_filter_cascade_and_roundtrip_all_excise_forms() {
        setup();
        for code in ["2200A", "2200AN", "2200C", "2200M", "2200P", "2200S"] {
            let kind = kind_of(code).unwrap();
            let base = if code == "2200C" {
                "/place"
            } else {
                "/place_of_production"
            };
            let key = |member: &str| {
                kind.definition().bindings.iter().find_map(|(k, b)| {
                    matches!(&b.binding, agent::Binding::Text { path, .. } if path == &format!("{base}/{member}"))
                        .then_some(k.clone())
                }).unwrap()
            };
            let (r, p, c) = (key("region"), key("province"), key("city"));
            let opened = open(code, 1);
            assert_eq!(
                opened["options"][&r].as_array().unwrap().len(),
                17,
                "{code}"
            );
            assert_eq!(
                opened["options"][&p],
                json!([]),
                "{code} needs a region first"
            );
            assert_eq!(
                opened["options"][&c],
                json!([]),
                "{code} needs a province first"
            );
            // A batch arrives in lexical key order, which is city first on
            // several forms. The host must apply parents first.
            let region = excise_places_for_test(code);
            let filled = fill(
                code,
                &opened["draft"],
                json!({&r: region.0, &p: region.1, &c: region.2}),
            );
            assert_eq!(filled["rejected"], json!({}), "{code}");
            assert!(
                filled["options"][&c]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|o| o["value"] == region.2),
                "{code}"
            );
            let saved = save(code, &filled["draft"]);
            let reopened = open(code, 1);
            assert_eq!(
                reopened["draft"].pointer(base),
                saved["draft"].pointer(base),
                "{code}"
            );
            assert_eq!(reopened["values"][&r], region.0, "{code}");
            assert_eq!(reopened["values"][&p], region.1, "{code}");
            assert_eq!(reopened["values"][&c], region.2, "{code}");
            // Re-selecting the same parent keeps its descendants.
            let same = fill(code, &reopened["draft"], json!({&r: region.0}));
            assert_eq!(
                same["draft"].pointer(base),
                reopened["draft"].pointer(base),
                "{code}"
            );
            let changed = fill(code, &same["draft"], json!({&p: ""}));
            assert_eq!(
                changed["draft"].pointer(&format!("{base}/city")),
                Some(&json!("")),
                "{code}"
            );
            let changed = fill(code, &same["draft"], json!({&r: ""}));
            assert_eq!(
                changed["draft"].pointer(base),
                Some(&json!({"region": "", "province": "", "city": ""})),
                "{code}"
            );
            assert_eq!(changed["options"][&p], json!([]), "{code}");
            let validated = call_async(
                "screen_form_validate",
                vec![code.into(), json(changed["draft"].clone())],
            )
            .unwrap();
            assert_eq!(validated["options"], changed["options"], "{code}");
        }
    }

    fn excise_places_for_test(code: &str) -> (&'static str, &'static str, &'static str) {
        use bir_core::forms::excise_places;
        let region = excise_places::regions(code)[0].0;
        let province = excise_places::provinces(code, region)[0].0;
        let city = excise_places::cities(code, region, province)[0].0;
        (region, province, city)
    }

    #[test]
    fn catalog_location_parent_changes_leave_the_other_place_intact() {
        setup();
        let opened = open("2200S", 1);
        let (r, p, c) = excise_places_for_test("2200S");
        let filled = fill(
            "2200S",
            &opened["draft"],
            json!({
                "frm2200SoptRegion": r, "frm2200SoptProvince": p, "frm2200SoptCity": c,
                "frm2200SoptRegion1": r, "frm2200SoptProvince1": p, "frm2200SoptCity1": c,
            }),
        );
        assert_eq!(filled["rejected"], json!({}));
        let changed = fill(
            "2200S",
            &filled["draft"],
            json!({"frm2200SoptRegion": "020000000"}),
        );
        assert_eq!(
            changed["draft"]["place_of_production"],
            json!({"region": "020000000", "province": "", "city": ""})
        );
        assert_eq!(
            changed["draft"]["place_of_removal"],
            filled["draft"]["place_of_removal"]
        );
        let cleared = fill("2200S", &changed["draft"], json!({"frm2200SoptRegion": ""}));
        save("2200S", &cleared["draft"]);
        assert_eq!(
            open("2200S", 1)["draft"]["place_of_production"],
            cleared["draft"]["place_of_production"]
        );
    }

    #[test]
    fn catalog_atc_and_treaty_choices_store_codes_instead_of_labels() {
        setup();
        for (code, key, chosen) in [
            ("1601FQ", "drpTreatyCode0", "AU"),
            ("1601FQ", "drpATCCode0", "WI330"),
            ("1702Q", "frm1702q:cbATC_2", "IC010_25%"),
            ("1702MX", "frm1702MX:drpPg1I5ATCR2", "IC011"),
            ("1702RT", "frm1702RT:drpPg1I5AtcOther", "IC010"),
        ] {
            let opened = open(code, 1);
            assert!(
                opened["options"][key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|o| o["value"] == chosen && o["label"] != chosen),
                "{code} {chosen}"
            );
            let mut draft = opened["draft"].clone();
            if code == "1601FQ" {
                // The existing agent definition has no relief-control
                // binding. Exercise catalogs on an already enabled typed
                // schedule, without claiming the fresh-page flow is ported.
                use bir_core::forms::form_1601fq::{Form1601FqDraft, Form1601FqTaxRelief};
                let mut typed: Form1601FqDraft = serde_json::from_value(draft).unwrap();
                typed.set_tax_relief(Some(Form1601FqTaxRelief::InternationalTaxTreaty));
                draft = serde_json::to_value(typed).unwrap();
            }
            let filled = fill(code, &draft, json!({key: chosen}));
            assert_eq!(filled["rejected"], json!({}), "{code}");
            save(code, &filled["draft"]);
            assert_eq!(open(code, 1)["values"][key], chosen, "{code}");
        }
    }

    #[test]
    fn catalog_2551q_uses_official_option_numbers_without_unlocking_rows() {
        setup();
        let opened = open("2551Q", 3);
        let rows = opened["options"]["drpATC1"].as_array().unwrap();
        assert_eq!(rows.len(), 23);
        for value in ["1", "23"] {
            let row = rows.iter().find(|o| o["value"] == value).unwrap();
            assert!(row["label"].as_str().unwrap().contains("PT010"));
        }
        let filled = fill("2551Q", &opened["draft"], json!({"drpATC1": "1"}));
        assert!(filled["rejected"].get("drpATC1").is_some());
        assert_eq!(filled["draft"]["schedule_1"], opened["draft"]["schedule_1"]);
        assert_eq!(filled["values"]["drpATC1"], opened["values"]["drpATC1"]);
    }

    #[test]
    fn catalog_2000_distinguishes_same_code_with_different_tax_calculations() {
        use bir_core::forms::form_2000::Form2000Draft;

        setup();
        let opened = open("2000", 1);
        let choices: Vec<_> = opened["options"]["drpATCCode0"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["label"].as_str().unwrap().starts_with("DS112 —"))
            .collect();
        assert_eq!(choices.len(), 2);
        assert_ne!(choices[0]["value"], choices[1]["value"]);
        for (choice, due) in choices.iter().zip([5.0, 2.0]) {
            let filled = fill(
                "2000",
                &opened["draft"],
                json!({
                    "drpATCCode0": choice["value"], "frm2000:sched1:txtTaxBase0": "1000",
                }),
            );
            assert_eq!(filled["rejected"], json!({}));
            assert_eq!(filled["draft"]["schedule1"][0]["atc_code"], "DS112");
            assert_eq!(filled["draft"]["schedule1"][0]["tax_due"], due);
            save("2000", &filled["draft"]);
            let reopened = open("2000", 1);
            let selected: Vec<_> = reopened["options"]["drpATCCode0"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["selected"] == true)
                .collect();
            assert_eq!(selected.len(), 1);
            assert_eq!(selected[0]["value"], choice["value"]);
            let map =
                with_database(|db| print_map(db, kind_of("2000")?, &reopened["draft"])).unwrap();
            assert_eq!(map.0.get("drpATCCode0").map(String::as_str), Some("DS112"));
            assert!(!reopened["draft"].to_string().contains("catalog:"));
            let cleared = fill("2000", &reopened["draft"], json!({"drpATCCode0": ""}));
            assert_eq!(cleared["rejected"], json!({}));
            assert_eq!(cleared["draft"]["schedule1"], json!([]));
            let mut queued_form: Form2000Draft =
                serde_json::from_value(reopened["draft"].clone()).unwrap();
            queued_form.lifecycle.status = FilingStatus::Queued;
            let queued = serde_json::to_value(&queued_form).unwrap();
            let refused = fill("2000", &queued, json!({"drpATCCode0": choice["value"]}));
            assert!(refused["rejected"].get("drpATCCode0").is_some());
            assert_eq!(
                serde_json::from_value::<Form2000Draft>(refused["draft"].clone()).unwrap(),
                queued_form
            );
        }
        let invalid = fill(
            "2000",
            &opened["draft"],
            json!({"drpATCCode0": "catalog:999"}),
        );
        assert!(invalid["rejected"].get("drpATCCode0").is_some());
        assert_eq!(invalid["draft"]["schedule1"], opened["draft"]["schedule1"]);
    }

    /// N03: 2550M Schedule 1 ATC rows follow the official popup. A choice in
    /// a new row ticks the ATC, rows sort into popup order, an existing row
    /// keeps its amount when its code changes, an ATC appears once, blank
    /// unticks it, and a queued return refuses the edit.
    #[test]
    fn catalog_2550m_schedule_1_atcs_follow_the_popup() {
        use bir_core::forms::form_2550m::{FORM_2550M_ATCS, Form2550MDraft};

        setup();
        let opened = open("2550M", 7);
        let listed = opened["options"]["frm2550m:txtAtcCde5"].as_array().unwrap();
        assert_eq!(listed.len(), FORM_2550M_ATCS.len());
        assert!(!opened["options"].to_string().contains("See fixtures"));
        assert!(opened["locks"].get("frm2550m:txtAtcCde5").is_none());

        // One batch, the way the page sends a route's edits: the amount is
        // applied after the ATCs that make its row.
        let two = fill(
            "2550M",
            &opened["draft"],
            json!({
                "frm2550m:txtAtcCde1": "VT010",
                "frm2550m:txtAtcCde2": "VB010",
                "frm2550m:txtAmountSales1": "500",
            }),
        );
        assert_eq!(two["rejected"], json!({}));
        let codes = |d: &Value| -> Vec<String> {
            d["sales_schedule"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["atc_code"].as_str().unwrap().to_string())
                .collect()
        };
        assert_eq!(codes(&two["draft"]), ["VB010", "VT010"]);
        assert_eq!(two["draft"]["sales_schedule"][0]["amount"], 500.0);
        assert_eq!(two["draft"]["sales_schedule"][0]["output_tax"], 60.0);
        assert_eq!(two["values"]["frm2550m:txtAtcCde1"], "VB010");
        assert_eq!(two["values"]["frm2550m:txtAtcCde2"], "VT010");

        let amount = fill(
            "2550M",
            &two["draft"],
            json!({"frm2550m:txtAmountSales2": "1000"}),
        );
        assert_eq!(amount["rejected"], json!({}));
        let changed = fill(
            "2550M",
            &amount["draft"],
            json!({"frm2550m:txtAtcCde2": "VS010"}),
        );
        assert_eq!(changed["rejected"], json!({}));
        assert_eq!(codes(&changed["draft"]), ["VB010", "VS010"]);
        assert_eq!(changed["draft"]["sales_schedule"][1]["amount"], 1000.0);

        let duplicate = fill(
            "2550M",
            &changed["draft"],
            json!({"frm2550m:txtAtcCde1": "VS010"}),
        );
        assert!(duplicate["rejected"].get("frm2550m:txtAtcCde1").is_some());
        let cleared = fill(
            "2550M",
            &changed["draft"],
            json!({"frm2550m:txtAtcCde1": ""}),
        );
        assert_eq!(codes(&cleared["draft"]), ["VS010"]);

        let mut queued_form: Form2550MDraft =
            serde_json::from_value(changed["draft"].clone()).unwrap();
        queued_form.lifecycle.status = FilingStatus::Queued;
        let queued = serde_json::to_value(&queued_form).unwrap();
        let refused = fill("2550M", &queued, json!({"frm2550m:txtAtcCde3": "VC010"}));
        assert!(refused["rejected"].get("frm2550m:txtAtcCde3").is_some());
        assert_eq!(codes(&refused["draft"]), ["VB010", "VS010"]);
    }

    /// N04: 1601C Schedule I is one table of its three printed rows, and
    /// Previous Month/s (which the probe left unbound) saves.
    #[test]
    fn rows_1601c_schedule_i_table_saves() {
        setup();
        let form = schema_entry("1601C").unwrap();
        let items: Vec<&Value> = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .collect();
        let table = items
            .iter()
            .find(|it| it["keys"][0][0] == "frm1601c:sched1:txtMonthYear0")
            .expect("Schedule I table");
        assert_eq!(table["rows"], 3);
        assert_eq!(table["cols"][0]["l"], "1 Previous Month/s (MM/YYYY)");
        assert_eq!(table["cols"][4]["t"], "money");
        assert!(table["cols"][6].get("c").is_some());
        // Each row box sits in the table only; templates and selectors hide.
        for it in &items {
            for f in it["f"].as_array().into_iter().flatten() {
                let k = f["k"].as_str().unwrap();
                if k.contains("{N>=") || k.starts_with("chkScheduleDelete") {
                    assert_eq!(f["t"], "hidden", "{k}");
                }
                assert!(
                    k.contains("{N>=") || !k.starts_with("frm1601c:sched1:txtMonthYear"),
                    "{k}"
                );
            }
        }

        let opened = open("1601C", 7);
        assert!(
            opened["locks"]
                .get("frm1601c:sched1:txtMonthYear1")
                .is_none()
        );
        let filled = fill(
            "1601C",
            &opened["draft"],
            json!({
                "frm1601c:sched1:txtMonthYear0": "05/2026",
                "frm1601c:sched1:txtDatePaid0": "06/10/2026",
                "frm1601c:sched1:txtTaxPaid0": "1000",
                "frm1601c:sched1:txtShouldTaxDue0": "1500",
            }),
        );
        assert_eq!(filled["rejected"], json!({}));
        assert_eq!(
            filled["draft"]["schedule_1"][0]["previous_month"],
            "05/2026"
        );
        assert_eq!(
            amount(&filled["values"], "frm1601c:sched1:txtAdjustments0"),
            500.0
        );
        let long = fill(
            "1601C",
            &filled["draft"],
            json!({"frm1601c:sched1:txtMonthYear0": "May 2026"}),
        );
        assert!(
            long["rejected"]
                .get("frm1601c:sched1:txtMonthYear0")
                .is_some()
        );
        let blank = fill(
            "1601C",
            &filled["draft"],
            json!({"frm1601c:sched1:txtMonthYear2": ""}),
        );
        assert_eq!(blank["draft"]["schedule_1"].as_array().unwrap().len(), 1);

        save("1601C", &filled["draft"]);
        let reopened = open("1601C", 7);
        assert_eq!(
            reopened["values"]["frm1601c:sched1:txtMonthYear0"],
            "05/2026"
        );
    }

    /// N04: 2200P rows added past the 27 official ones save. The page
    /// appends them (addRowSched1) and the upload carries them; the probe
    /// now binds rows 28-41, which an "Added rows" table shows.
    #[test]
    fn rows_2200p_added_rows_save() {
        setup();
        let form = schema_entry("2200P").unwrap();
        let table = form["parts"][4]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|it| it["keys"][0][0] == "frm2200P:txtSched1ATC28")
            .expect("added rows table")
            .clone();
        assert_eq!(
            (table["rows"].clone(), table["start"].clone()),
            (json!(14), json!(28))
        );

        let opened = open("2200P", 7);
        let filled = fill(
            "2200P",
            &opened["draft"],
            json!({
                "frm2200P:txtSched1ATC28": "999",
                "frm2200P:txtSched1Description28": "Added product",
                "frm2200P:txtSched1ApplicableRate28": "10",
                "frm2200P:txtSched1BasicExciseTaxDue28": "500",
            }),
        );
        assert_eq!(filled["rejected"], json!({}));
        assert_eq!(filled["draft"]["schedule"].as_array().unwrap().len(), 28);
        save("2200P", &filled["draft"]);
        let reopened = open("2200P", 7);
        assert_eq!(reopened["values"]["frm2200P:txtSched1ATC28"], "999");
        let map = with_database(|db| print_map(db, kind_of("2200P")?, &reopened["draft"])).unwrap();
        assert_eq!(
            map.0
                .get("frm2200P:sched1AddedRowCount")
                .map(String::as_str),
            Some("1")
        );
    }

    /// N04: 1702RT "(Add more...)" rows save. One row shows from the draft
    /// (the upload carries the modal rows only once two are saved); two
    /// rows fold the item into OTHERS with their subtotal.
    #[test]
    fn rows_1702rt_add_more_rows_save() {
        setup();
        let form = schema_entry("1702RT").unwrap();
        let tables: Vec<&Value> = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .filter(|it| {
                it["keys"][0][0]
                    .as_str()
                    .is_some_and(|k| k.ends_with(".1C1"))
            })
            .collect();
        assert_eq!(tables.len(), 6);
        assert!(tables.iter().all(|t| t["rows"] == 12));

        let opened = open("1702RT", 0);
        let one = fill(
            "1702RT",
            &opened["draft"],
            json!({
                "frm1702RT:txtPg2Pt4I54.1C1": "Credit A",
                "frm1702RT:txtPg2Pt4I54.1C2": "100",
            }),
        );
        assert_eq!(one["rejected"], json!({}));
        assert_eq!(one["values"]["frm1702RT:txtPg2Pt4I54.1C1"], "Credit A");
        let two = fill(
            "1702RT",
            &one["draft"],
            json!({
                "frm1702RT:txtPg2Pt4I54.2C1": "Credit B",
                "frm1702RT:txtPg2Pt4I54.2C2": "50",
            }),
        );
        assert_eq!(two["rejected"], json!({}));
        assert_eq!(two["values"]["frm1702RT:txtPg2Pt4I54C1"], "OTHERS");
        assert_eq!(amount(&two["values"], "frm1702RT:txtPg2Pt4I54C2"), 150.0);

        save("1702RT", &two["draft"]);
        let reopened = open("1702RT", 0);
        assert_eq!(
            reopened["draft"]["more_rows"]["item_54"],
            two["draft"]["more_rows"]["item_54"]
        );
        assert_eq!(
            amount(&reopened["values"], "frm1702RT:txtPg2Pt4I54.2C2"),
            50.0
        );
    }

    /// N04: 1800 and 2000OT schedules are tables of every row the draft
    /// keeps; a row past the five the page opens with saves.
    #[test]
    fn rows_1800_and_2000ot_schedule_tables_save() {
        setup();
        for (code, first, extra, count) in [
            (
                "1800",
                "frm1800v2018:schedA:txtParticulars0",
                "frm1800v2018:schedA:txtParticulars6",
                15,
            ),
            (
                "1800",
                "frm1800v2018:schedB1:txtOCT0",
                "frm1800v2018:schedB1:txtOCT6",
                15,
            ),
            (
                "2000OT",
                "frm2000OT:sched1A:txtOCTNo0",
                "frm2000OT:sched1A:txtOCTNo6",
                15,
            ),
            (
                "2000OT",
                "frm2000OT:sched1B:txtNameOfCorpStock0",
                "frm2000OT:sched1B:txtNameOfCorpStock6",
                13,
            ),
        ] {
            let form = schema_entry(code).unwrap();
            let items: Vec<&Value> = form["parts"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|p| p["items"].as_array().unwrap())
                .collect();
            let table = items
                .iter()
                .find(|it| it["keys"][0][0] == first)
                .unwrap_or_else(|| panic!("{code} table at {first}"));
            assert_eq!(table["rows"], count, "{code} {first}");
            // A row box sits in the table only.
            assert!(
                items
                    .iter()
                    .flat_map(|it| it["f"].as_array().into_iter().flatten())
                    .all(|f| f["k"] != first),
                "{code} {first} is still a loose box"
            );
            let opened = open(code, 1);
            let filled = fill(code, &opened["draft"], json!({ extra: "Row seven" }));
            assert_eq!(filled["rejected"], json!({}), "{code} {extra}");
            save(code, &filled["draft"]);
            let reopened = open(code, 1);
            assert_eq!(
                reopened["values"][extra].as_str().map(str::to_uppercase),
                Some("ROW SEVEN".to_string()),
                "{code} {extra}"
            );
        }
    }

    /// N04: "(more...)"/"More"/"Add row" rows of 1702EX, 1707 and 2200M are
    /// tables, and a continuation row saves through the probed bindings.
    #[test]
    fn rows_popup_tables_save() {
        setup();
        for (code, period, first, rows, edits, check) in [
            (
                "1702EX",
                0,
                "frm1702EX:txtPg3Pt6S1I17_1Col1",
                12,
                json!({
                    "frm1702EX:txtPg3Pt6S1I17_1Col1": "Other A",
                    "frm1702EX:txtPg3Pt6S1I17_1Col2": "100",
                    "frm1702EX:txtPg3Pt6S1I17_2Col1": "Other B",
                    "frm1702EX:txtPg3Pt6S1I17_2Col2": "50",
                }),
                ("frm1702EX:txtPg3Pt6S1I17_2Col1", "OTHER B"),
            ),
            (
                "1707",
                1,
                "frm1707:txtPg2Pt4S2_1Col1",
                12,
                json!({
                    "frm1707:txtPg2Pt4S2_1Col1": "Corp D",
                    "frm1707:txtPg2Pt4S2_2Col1": "Corp E",
                }),
                ("frm1707:txtPg2Pt4S2_2Col1", "CORP E"),
            ),
            (
                "2200M",
                7,
                "frm2200M:txtSched1ATC11",
                12,
                json!({
                    "frm2200M:txtSched1_description11": "Added mineral",
                    "frm2200M:txtSched1_LocallyExtractedTaxDueF11": "250",
                }),
                ("frm2200M:txtSched1_description11", "ADDED MINERAL"),
            ),
        ] {
            let form = schema_entry(code).unwrap();
            let table = form["parts"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|p| p["items"].as_array().unwrap())
                .find(|it| it["keys"][0][0] == first)
                .unwrap_or_else(|| panic!("{code} table at {first}"))
                .clone();
            assert_eq!(table["rows"], rows, "{code}");
            let opened = open(code, period);
            let filled = fill(code, &opened["draft"], edits);
            assert_eq!(filled["rejected"], json!({}), "{code}");
            save(code, &filled["draft"]);
            let reopened = open(code, period);
            assert_eq!(
                reopened["values"][check.0].as_str().map(str::to_uppercase),
                Some(check.1.to_string()),
                "{code} {}",
                check.0
            );
        }
    }

    /// N04: 2200S "Others (specify)" rows are one table; an added row saves,
    /// its rate through the host (the probe could not bind it) and its tax
    /// due computed as rate x volume.
    #[test]
    fn rows_2200s_others_rows_save() {
        setup();
        let form = schema_entry("2200S").unwrap();
        let table = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .find(|it| it["keys"][0][0] == "frm2200S:txtsched1Atc0")
            .expect("Others table")
            .clone();
        assert_eq!(table["rows"], 12);
        assert!(table["cols"][6].get("c").is_some() && table["cols"][3].get("c").is_none());

        let opened = open("2200S", 7);
        assert!(
            opened["locks"]
                .get("frm2200S:txtsched1AppTaxRate1")
                .is_none()
        );
        let filled = fill(
            "2200S",
            &opened["draft"],
            json!({
                // Schedule 1 belongs to a payment on actual removal.
                "frm2200S:chkPymntManner_1": "true",
                "frm2200S:txtsched1Atc1": "XB999",
                "frm2200S:txtsched1Desc1": "Other drink",
                "frm2200S:txtsched1AppTaxRate1": "6.5",
                "frm2200S:txtsched1VolumeRemovals1": "100",
            }),
        );
        assert_eq!(filled["rejected"], json!({}));
        assert_eq!(filled["draft"]["others"][1]["rate"], 6.5);
        assert_eq!(
            amount(&filled["values"], "frm2200S:txtsched1BasicTaxDue1"),
            650.0
        );
        let bad = fill(
            "2200S",
            &filled["draft"],
            json!({"frm2200S:txtsched1AppTaxRate1": "1000"}),
        );
        assert!(
            bad["rejected"]
                .get("frm2200S:txtsched1AppTaxRate1")
                .is_some()
        );
        save("2200S", &filled["draft"]);
        let reopened = open("2200S", 7);
        assert_eq!(reopened["values"]["frm2200S:txtsched1Atc1"], "XB999");
    }

    /// N04: 2552 Part V popup rows (row 5 on) save for the schedule Item 11
    /// selects; the other schedules' boxes refuse edits, since all three
    /// keep their rows in one draft list.
    #[test]
    fn rows_2552_popup_rows_follow_item_11() {
        use bir_core::forms::form_2552::{Form2552Draft, Form2552Transaction};

        setup();
        let form = schema_entry("2552").unwrap();
        for s in 1..=3 {
            let first = format!("frm2552:txtPg2Pt5Sch{s}_1Col1");
            let table = form["parts"][4]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|it| it["keys"][0][0] == first.as_str())
                .unwrap_or_else(|| panic!("2552 Schedule {s} table"))
                .clone();
            assert_eq!(
                (table["rows"].clone(), table["start"].clone()),
                (json!(9), json!(5))
            );
        }

        let opened = open("2552", 3);
        let mut typed: Form2552Draft = serde_json::from_value(opened["draft"].clone()).unwrap();
        typed.set_transaction(Form2552Transaction::SecondaryOffering);
        let draft = serde_json::to_value(&typed).unwrap();
        let other = fill(
            "2552",
            &draft,
            json!({"frm2552:txtPg2Pt5Sch2_1Col2": "Seller X"}),
        );
        assert!(
            other["rejected"]
                .get("frm2552:txtPg2Pt5Sch2_1Col2")
                .is_some()
        );
        let filled = fill(
            "2552",
            &draft,
            json!({
                "frm2552:txtPg2Pt5Sch3_1Col1": "03/10/2026",
                "frm2552:txtPg2Pt5Sch3_1Col2": "Seller Five",
                "frm2552:txtPg2Pt5Sch3_1Col6": "1000",
                "frm2552:txtPg2Pt5Sch3_1Col7": "4",
            }),
        );
        assert_eq!(filled["rejected"], json!({}));
        assert_eq!(filled["draft"]["schedule"][4]["seller"], "Seller Five");
        // The other schedules' popup boxes show nothing of Schedule 3's rows.
        assert_eq!(filled["values"].get("frm2552:txtPg2Pt5Sch2_1Col2"), None);
        save("2552", &filled["draft"]);
        let reopened = open("2552", 3);
        assert_eq!(
            reopened["values"]["frm2552:txtPg2Pt5Sch3_1Col2"]
                .as_str()
                .map(str::to_uppercase),
            Some("SELLER FIVE".to_string())
        );
    }

    /// N04: 1600WP Part II rows follow the ATC popup: choosing an ATC ticks
    /// it (at most two, the category's), its tax base saves, and rate and
    /// tax withheld follow.
    #[test]
    fn rows_1600wp_part_ii_follows_the_popup() {
        setup();
        let form = schema_entry("1600WP").unwrap();
        let table = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .find(|it| it["keys"][0][0] == "frm1600WP:txtAtcCode1")
            .expect("Part II table")
            .clone();
        assert_eq!(table["rows"], 2);
        assert_eq!(table["cols"][0]["t"], "sel");
        assert!(table["cols"][0].get("c").is_none() && table["cols"][3].get("c").is_some());

        let opened = open("1600WP", 7);
        assert!(opened["locks"].get("frm1600WP:txtAtcCode1").is_none());
        let filled = fill(
            "1600WP",
            &opened["draft"],
            json!({
                "frm1600WP:AnyTaxHeld_1": "true",
                "frm1600WP:CategoryAgent": "G",
                "frm1600WP:txtAtcCode1": "WB191",
                "frm1600WP:txtTaxBase1": "10000",
            }),
        );
        let listed = filled["options"]["frm1600WP:txtAtcCode1"]
            .as_array()
            .unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(filled["rejected"], json!({}));
        assert_eq!(filled["draft"]["atc_rows"][0]["atc_code"], "WB191");
        assert_eq!(
            amount(&filled["values"], "frm1600WP:txtTaxbeWithHeld1"),
            400.0
        );
        save("1600WP", &filled["draft"]);
        let reopened = open("1600WP", 7);
        assert_eq!(reopened["values"]["frm1600WP:txtAtcCode1"], "WB191");
        let cleared = fill(
            "1600WP",
            &reopened["draft"],
            json!({"frm1600WP:txtAtcCode1": ""}),
        );
        assert_eq!(cleared["draft"]["atc_rows"], json!([]));
    }

    /// N04: 1701A Items 42 and 51 "(add more...)" rows save; two rows fold
    /// Item 42 into OTHERS, and the spouse cells save through the host.
    #[test]
    fn rows_1701a_add_more_rows_save() {
        setup();
        let form = schema_entry("1701A").unwrap();
        let tables: Vec<&Value> = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .filter(|it| {
                it["keys"][0][0]
                    .as_str()
                    .is_some_and(|k| k.starts_with("frm1701A:txtmodalPartIV"))
            })
            .collect();
        assert_eq!(tables.len(), 2);

        let opened = open("1701A", 0);
        assert!(
            opened["locks"]
                .get("frm1701A:txtmodalPartIVA_5C3")
                .is_none()
        );
        let filled = fill(
            "1701A",
            &opened["draft"],
            json!({
                // Part IV.A opens with a graduated-rate ATC.
                "frm1701A:optTaxType_1": "true",
                "frm1701A:optATC_1": "true",
                "frm1701A:txtmodalPartIVADesc_1C1": "Rental income",
                "frm1701A:txtmodalPartIVA_1C2": "500",
                "frm1701A:txtmodalPartIVADesc_2C1": "Royalty income",
                "frm1701A:txtmodalPartIVA_2C2": "1000",
                "frm1701A:txtmodalPartIVA_5C3": "250",
            }),
        );
        assert_eq!(filled["rejected"], json!({}));
        let rows = filled["draft"]["other_income_more"].as_array().unwrap();
        // The spouse cell starts row 5; a return without a joint spouse
        // keeps the spouse column at zero, as the page does.
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[4]["spouse"], 0.0);
        assert_eq!(filled["draft"]["other_income_42_description"], "OTHERS");
        assert_eq!(rows[1]["description"], "Royalty income");
        save("1701A", &filled["draft"]);
        let reopened = open("1701A", 0);
        assert_eq!(
            reopened["draft"]["other_income_more"][1]["taxpayer"],
            filled["draft"]["other_income_more"][1]["taxpayer"]
        );
    }

    /// N04: 2550Q Schedules 1, 3 and 4 are tables of every row, kept
    /// gap-free (the upload leaves empty rows out), with dates through the
    /// host; the Items 19, 42, 47 and 56 popup rows are tables of their own.
    #[test]
    fn rows_2550q_schedules_and_popups_save() {
        setup();
        let form = schema_entry("2550Q").unwrap();
        let items: Vec<&Value> = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .collect();
        let table = |first: &str| {
            items
                .iter()
                .find(|it| it["keys"][0][0] == first)
                .unwrap_or_else(|| panic!("2550Q table at {first}"))
        };
        assert_eq!(table("txtDatePurchase10")["rows"], 12);
        assert_eq!(table("txtDateCovered30")["rows"], 12);
        assert_eq!(table("txtDate40")["rows"], 12);
        for item in ["19", "42", "47", "56"] {
            let first = format!("frm2550qv2024:totalTaxPayableNo{item}Description0");
            assert_eq!(table(&first)["rows"], 12);
        }
        assert_eq!(
            table("frm2550qv2024:addSpecifyNo42")["cols"][2]["t"],
            "hidden"
        );
        // A row box sits in its table only, though the extractor split the
        // first two rows across two parts.
        let loose: Vec<&str> = items
            .iter()
            .flat_map(|it| it["f"].as_array().into_iter().flatten())
            .filter_map(|f| f["k"].as_str())
            .collect();
        for key in [
            "txtNameWithHoldingAgent30",
            "txtDateCovered31",
            "txtAmountPurchase10",
        ] {
            assert!(!loose.contains(&key), "{key} is still a loose box");
        }

        let opened = open("2550Q", 2);
        assert!(opened["locks"].get("txtNameWithHoldingAgent30").is_none());
        let gap = fill(
            "2550Q",
            &opened["draft"],
            json!({"txtNameWithHoldingAgent32": "Too far"}),
        );
        assert!(gap["rejected"].get("txtNameWithHoldingAgent32").is_some());
        let filled = fill(
            "2550Q",
            &opened["draft"],
            json!({
                "txtDateCovered30": "04/01/2026",
                "txtDateCovered3To0": "06/30/2026",
                "txtNameWithHoldingAgent30": "Agent One",
                "txtIncomePayment30": "10000",
                "txtTotalTaxWithHeld30": "500",
                "txtNameWithHoldingAgent31": "Agent Two",
                "txtIncomePayment31": "2000",
                "txtNameWithHoldingAgent32": "Agent Three",
                "frm2550qv2024:totalTaxPayableNo42Description0": "Other A",
                "frm2550qv2024:totalTaxPayableNo42Amount0": "100",
            }),
        );
        assert_eq!(filled["rejected"], json!({}));
        let rows = filled["draft"]["schedule_3"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0]["period_from"],
            json!({"year": 2026, "month": 4, "day": 1})
        );
        assert_eq!(filled["values"]["txtNameWithHoldingAgent32"], "Agent Three");
        assert_eq!(
            filled["draft"]["item_42_additional_rows"][0]["description"],
            "Other A"
        );
        let bad = fill(
            "2550Q",
            &filled["draft"],
            json!({"txtDate40": "13/40/2026"}),
        );
        assert!(bad["rejected"].get("txtDate40").is_some());

        // Clearing row 2 removes it; row 3 moves up, as the upload sends it.
        let cleared = fill(
            "2550Q",
            &filled["draft"],
            json!({"txtNameWithHoldingAgent31": "", "txtIncomePayment31": ""}),
        );
        assert_eq!(cleared["rejected"], json!({}));
        assert_eq!(
            cleared["values"]["txtNameWithHoldingAgent31"],
            "Agent Three"
        );

        save("2550Q", &filled["draft"]);
        let reopened = open("2550Q", 2);
        assert_eq!(reopened["values"]["txtDateCovered30"], "4/01/2026");
        assert_eq!(
            reopened["values"]["txtNameWithHoldingAgent32"],
            "Agent Three"
        );
    }

    /// Item 4: 2200AN's per-row Basic Excise Tax Due is the filer's entry on
    /// the official page (rules/forms/2200an-v2018: computed false, no
    /// calculation); what the page computes are the Schedule 1A and 1B
    /// totals and Item 16, which follow the rows here.
    #[test]
    fn excise_2200an_totals_follow_the_typed_row_tax_due() {
        setup();
        let opened = open("2200AN", 7);
        assert!(
            opened["locks"]
                .get("frm2200ANv2018:txtXG021TaxDue")
                .is_none()
        );
        let filled = fill(
            "2200AN",
            &opened["draft"],
            json!({
                "frm2200ANv2018:txtXG021TaxDue": "1000",
                "frm2200ANv2018:txtXG022TaxDue": "250.50",
                "frm2200ANv2018:txtXG100TaxDue": "500",
            }),
        );
        assert_eq!(filled["rejected"], json!({}));
        let v = &filled["values"];
        assert_eq!(amount(v, "frm2200ANv2018:txtBasicExciseTaxDue"), 1_250.5);
        assert_eq!(
            amount(v, "frm2200ANv2018:txtBasicExciseTaxDueNonEssentialGoods"),
            500.0
        );
        assert_eq!(amount(v, "frm2200ANv2018:txt16"), 1_750.5);
    }

    /// Item 3: 1601EQ ATCs are chosen from the popup list on every row (the
    /// probe could not bind them: rows re-sort into popup order); rows 7-30
    /// are one table instead of loose boxes, and rows past 30 hide.
    #[test]
    fn atc_1601eq_rows_follow_the_popup() {
        setup();
        let form = schema_entry("1601EQ").unwrap();
        let items: Vec<&Value> = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .collect();
        let more = items
            .iter()
            .find(|it| it["keys"][0][0] == "frm1601EQ:txtAtcCd7")
            .expect("rows 7-30 table");
        assert_eq!(
            (more["rows"].clone(), more["start"].clone()),
            (json!(24), json!(7))
        );
        assert_eq!(more["cols"][0]["t"], "sel");
        let printed = items
            .iter()
            .find(|it| it["keys"][0][0] == "frm1601EQ:txtAtcCd1")
            .expect("printed rows table");
        assert_eq!(printed["cols"][0]["t"], "sel");

        let opened = open("1601EQ", 3);
        assert!(opened["locks"].get("frm1601EQ:txtAtcCd1").is_none());
        let filled = fill(
            "1601EQ",
            &opened["draft"],
            json!({
                "frm1601EQ:optWithheld:Y": "true",
                "frm1601EQ:optCategory:P": "true",
                "frm1601EQ:txtAtcCd1": "WI020",
                "frm1601EQ:txtAtcCd2": "WI010",
                "frm1601EQ:txtTaxBase1": "1000",
            }),
        );
        assert_eq!(filled["rejected"], json!({}));
        let codes: Vec<&str> = filled["draft"]["schedule"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["atc_code"].as_str().unwrap())
            .collect();
        // Popup order: WI010 before WI020.
        assert_eq!(codes, ["WI010", "WI020"]);
        assert!(
            filled["options"]["frm1601EQ:txtAtcCd8"]
                .as_array()
                .is_some_and(|o| !o.is_empty())
        );
        let dup = fill(
            "1601EQ",
            &filled["draft"],
            json!({"frm1601EQ:txtAtcCd3": "WI010"}),
        );
        assert!(dup["rejected"].get("frm1601EQ:txtAtcCd3").is_some());
        save("1601EQ", &filled["draft"]);
        let reopened = open("1601EQ", 3);
        assert_eq!(reopened["values"]["frm1601EQ:txtAtcCd1"], "WI010");
        let cleared = fill(
            "1601EQ",
            &reopened["draft"],
            json!({"frm1601EQ:txtAtcCd1": ""}),
        );
        assert_eq!(cleared["draft"]["schedule"].as_array().unwrap().len(), 1);
    }

    /// Item 3: boxes the official page leaves open but the models never
    /// stored now save: 1801/2200A/2200T Details of Payment, 2200C's number
    /// of sheets attached and 1701MS's signatory name and representative.
    #[test]
    fn open_boxes_the_models_now_keep_save() {
        setup();
        for (code, period, edits, check) in [
            (
                "2200A",
                7,
                json!({
                    "frm2200Av2020:cashBank": "BDO",
                    "frm2200Av2020:cashAmt": "1,000.00",
                    "frm2200Av2020:taxNum": "TDM-9",
                }),
                ("frm2200Av2020:taxNum", "TDM-9"),
            ),
            (
                "2200T",
                7,
                json!({"frm2200Tv2020:checkNum": "CHK-1"}),
                ("frm2200Tv2020:checkNum", "CHK-1"),
            ),
            (
                "1801",
                1,
                json!({"frm1801v2018taxkNum": "TDM-2"}),
                ("frm1801v2018taxkNum", "TDM-2"),
            ),
            (
                "2200C",
                7,
                json!({"frm2200C:txtPg1I4NoSheets": "2"}),
                ("frm2200C:txtPg1I4NoSheets", "2"),
            ),
            (
                "1701MS",
                0,
                json!({"frm1701MS:printedName": "Juan Dela Cruz"}),
                ("frm1701MS:printedName", "Juan Dela Cruz"),
            ),
        ] {
            let opened = open(code, period);
            assert!(opened["locks"].get(check.0).is_none(), "{code} {}", check.0);
            let filled = fill(code, &opened["draft"], edits);
            assert_eq!(filled["rejected"], json!({}), "{code}");
            save(code, &filled["draft"]);
            let reopened = open(code, period);
            assert_eq!(reopened["values"][check.0], check.1, "{code} {}", check.0);
        }
    }

    /// N04: 1707A rows 4 and after save. Schedules 1 and 2 print rows 1-4;
    /// the official "More" pop-up holds row 4 onward, and its row k is draft
    /// row 3+k. Once a schedule has more than four rows, printed row 4 reads
    /// OTHERS with the continuation totals.
    #[test]
    fn rows_1707a_continuation_tables_save() {
        setup();
        let form = schema_entry("1707A").unwrap();
        let table = |first: &str| -> Value {
            form["parts"][3]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|it| it["keys"][0][0] == first)
                .unwrap_or_else(|| panic!("table starting {first}"))
                .clone()
        };
        let s1 = table("frm1707Av2021:txtS1C1DateOfTransaction_1");
        assert_eq!(
            (s1["rows"].clone(), s1["start"].clone()),
            (json!(11), json!(4))
        );
        assert_eq!(s1["keys"][10][5], "frm1707Av2021:txtS1C6TaxPaid_11");
        assert!(s1["cols"][4].get("c").is_some() && s1["cols"][3].get("c").is_none());
        let s2 = table("frm1707Av2021:txtS2C1DateOfTransaction_1");
        assert_eq!(s2["rows"], 9);
        assert!(s2["cols"][4].get("c").is_some());
        let printed = table("frm1707Av2021:txtS1C1DateOfTransactionI1");
        assert_eq!(printed["cols"][0]["l"], "Sch. 1 (A) Date of Transaction");

        let opened = open("1707A", 1);
        let mut edits = serde_json::Map::new();
        for (n, key) in ["I1", "I2", "I3", "_1", "_2"].iter().enumerate() {
            let put = |edits: &mut serde_json::Map<String, Value>, col: &str, value: Value| {
                edits.insert(format!("frm1707Av2021:txtS1{col}{key}"), value);
            };
            put(&mut edits, "C1DateOfTransaction", json!("03/15/2026"));
            put(
                &mut edits,
                "C2NameOfCorporateStock",
                json!(format!("Corp {n}")),
            );
            put(&mut edits, "C3SellingPrice", json!("1000"));
            put(&mut edits, "C4Cost", json!("400"));
            put(&mut edits, "C6TaxPaid", json!("10"));
        }
        edits.insert(
            "frm1707Av2021:txtS2C1DateOfTransaction_1".into(),
            json!("03/16/2026"),
        );
        let filled = fill("1707A", &opened["draft"], Value::Object(edits));
        assert_eq!(filled["rejected"], json!({}));
        let gains = filled["draft"]["gains"].as_array().unwrap();
        assert_eq!(gains.len(), 5);
        assert_eq!(gains[4]["corporation"], "Corp 4");
        assert_eq!(filled["draft"]["losses"][3]["date"], "03/16/2026");
        let v = &filled["values"];
        assert_eq!(amount(v, "frm1707Av2021:txtS1C5CapitalGains_2"), 600.0);
        // Printed row 4 is OTHERS with rows 4 and 5 summed.
        assert_eq!(v["frm1707Av2021:txtS1C2NameOfCorporateStockI4"], "OTHERS");
        assert_eq!(amount(v, "frm1707Av2021:txtS1C3SellingPriceI4"), 2000.0);
        assert_eq!(amount(v, "frm1707Av2021:txtS1I20TotalCapitalGains"), 3000.0);

        save("1707A", &filled["draft"]);
        let reopened = open("1707A", 1);
        assert_eq!(reopened["draft"]["gains"], filled["draft"]["gains"]);
        assert_eq!(
            reopened["values"]["frm1707Av2021:txtS1C2NameOfCorporateStock_2"],
            filled["values"]["frm1707Av2021:txtS1C2NameOfCorporateStock_2"]
        );
    }

    /// Item 1: 2550M Schedules 2, 3A, 3B, 6, 7 and 8 save. The page builds
    /// their rows in script; the host writes them into the typed draft, an
    /// amount fills its input tax at 12% before a typed input tax replaces
    /// it, a row cannot skip ahead, clearing a row removes it, Schedule 8
    /// waits for Item 13A, and the rows survive save and reopen.
    #[test]
    fn rows_2550m_schedules_2_to_8_save() {
        setup();
        let form = schema_entry("2550M").unwrap();
        let table = |first: &str| -> Value {
            form["parts"][2]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|it| it["keys"][0][0] == first)
                .unwrap_or_else(|| panic!("table starting {first}"))
                .clone()
        };
        let editable = |t: &Value, col: usize| t["cols"][col].get("c").is_none();
        let s2 = table("txtDatePurchased0");
        assert_eq!(s2["rows"], 10);
        assert!((0..4).all(|c| editable(&s2, c)));
        let s3a = table("txtDatePurchased3A0");
        assert!((0..6).all(|c| editable(&s3a, c)));
        assert!(!editable(&s3a, 6) && !editable(&s3a, 7));
        let s3b = table("txtDatePurchased3B0");
        assert_eq!(s3b["keys"][9][3], "txtBalInputTaxPrevious3B9");
        assert!(editable(&s3b, 3) && !editable(&s3b, 7));
        assert!((0..5).all(|c| editable(&table("txtPeriodCovered0"), c)));
        let s7 = table("txtPeriodCoveredSch70");
        assert_eq!(s7["cols"][2]["t"], "text");
        assert!((0..5).all(|c| editable(&table("txtPeriodCoveredSch80"), c)));

        let opened = open("2550M", 8);
        let refused = fill(
            "2550M",
            &opened["draft"],
            json!({"txtNameAgentSch80": "Gov Agency"}),
        );
        assert!(refused["rejected"].get("txtNameAgentSch80").is_some());
        assert_eq!(refused["draft"]["schedule_8"], json!([]));

        let filled = fill(
            "2550M",
            &opened["draft"],
            json!({
                "txtDatePurchased0": "08/05/2026", "txtDescription0": "Office printer",
                "txtAmt0": "85000.55",
                "txtDatePurchased1": "08/20/2026", "txtDescription1": "Delivery bike",
                "txtInputTax1": "14000", "txtAmt1": "120000",
                "txtDatePurchased3A0": "08/10/2026", "txtDescription3A0": "Factory machine",
                "txtAmt3A0": "1500000.25", "txtEstLife3A0": "120", "txtRecogLife3A0": "60",
                "txtDatePurchased3B0": "03/01/2026", "txtDescription3B0": "Warehouse",
                "txtAmt3B0": "2000000", "txtBalInputTaxPrevious3B0": "200000.50",
                "txtEstLife3B0": "120", "txtRecogLife3B0": "50",
                "txtPeriodCovered0": "08/31/2026", "txtNameAgent0": "Sample Agent Corp",
                "txtIncomePayment0": "100000", "txtTotalWithheld0": "5000",
                "txtAppliedCurr0": "4000.50",
                "txtPeriodCovered1": "08/15/2026", "txtNameAgent1": "Agent Two",
                "txtIncomePayment1": "50000", "txtTotalWithheld1": "2500",
                "txtAppliedCurr1": "2500",
                "txtPeriodCoveredSch70": "08/10/2026", "txtNameMillerSch70": "Rice Mill",
                "txtNameTaxPayerSch70": "Sample Dummy", "txtORNumSch70": "OR-12345",
                "txtAmountPaidSch70": "3000", "txtAppliedCurrSch70": "1500.25",
                "frm2550m:txtTax13A": "200000",
                "txtPeriodCoveredSch80": "08/31/2026", "txtNameAgentSch80": "Gov Agency",
                "txtIncomePaymentSch80": "200000", "txtTotalWithheldSch80": "10000",
                "txtAppliedCurrSch80": "10000",
            }),
        );
        assert_eq!(filled["rejected"], json!({}));
        let d = &filled["draft"];
        assert_eq!(d["schedule_2"][0]["input_tax"], 10_200.07);
        // The typed input tax replaces the 12% (14,400) its amount filled.
        assert_eq!(d["schedule_2"][1]["input_tax"], 14_000.0);
        assert_eq!(d["schedule_totals"]["schedule_2_amount"], 205_000.55);
        assert_eq!(d["schedule_3a"][0]["input_tax"], 180_000.03);
        assert_eq!(d["schedule_3a"][0]["allowable_input_tax"], 3_000.0);
        assert_eq!(d["schedule_3b"][0]["input_tax"], 200_000.5);
        assert_eq!(d["schedule_3b"][0]["allowable_input_tax"], 4_000.01);
        assert_eq!(d["schedule_totals"]["schedule_6_applied"], 6_500.5);
        assert_eq!(d["schedule_7"][0]["or_number"], "OR-12345");
        assert_eq!(d["schedule_totals"]["schedule_8_withheld"], 10_000.0);
        assert_eq!(amount(&filled["values"], "txtAllowInputTax3A0"), 3_000.0);
        assert_eq!(amount(&filled["values"], "txtmodalTotal23A"), 7_500.0);

        // A row cannot skip ahead; a life outside the page's range is refused.
        let gap = fill("2550M", d, json!({"txtNameAgent3": "Too far"}));
        assert!(gap["rejected"].get("txtNameAgent3").is_some());
        let life = fill("2550M", d, json!({"txtRecogLife3A0": "0"}));
        assert!(life["rejected"].get("txtRecogLife3A0").is_some());
        assert_eq!(life["draft"]["schedule_3a"], d["schedule_3a"]);

        // Clearing every box of Schedule 6 row 1 removes it; row 2 moves up.
        let cleared = fill(
            "2550M",
            d,
            json!({
                "txtPeriodCovered0": "", "txtNameAgent0": "", "txtIncomePayment0": "",
                "txtTotalWithheld0": "", "txtAppliedCurr0": "",
            }),
        );
        assert_eq!(cleared["rejected"], json!({}));
        assert_eq!(cleared["draft"]["schedule_6"].as_array().unwrap().len(), 1);
        assert_eq!(cleared["values"]["txtNameAgent0"], "AGENT TWO");
        assert_eq!(cleared["values"].get("txtNameAgent1"), None);

        save("2550M", d);
        let reopened = open("2550M", 8);
        assert_eq!(reopened["draft"]["schedule_2"], d["schedule_2"]);
        assert_eq!(reopened["draft"]["schedule_8"], d["schedule_8"]);
        assert_eq!(reopened["values"]["txtDescription3A0"], "FACTORY MACHINE");
        assert_eq!(amount(&reopened["values"], "txtAmountPaidSch70"), 3_000.0);
        let map = with_database(|db| print_map(db, kind_of("2550M")?, &reopened["draft"])).unwrap();
        assert_eq!(
            map.0.get("txtNameMillerSch70").map(String::as_str),
            Some("RICE MILL")
        );

        // A row problem lands on that row's boxes.
        let blank = fill("2550M", d, json!({"txtDescription1": ""}));
        let validated = call_async(
            "screen_form_validate",
            vec!["2550M".into(), json(blank["draft"].clone())],
        )
        .unwrap();
        let row = validated["errors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["field"] == "schedule_2[1]")
            .expect("Schedule 2 row 2 error");
        assert!(
            row["keys"]
                .as_array()
                .unwrap()
                .contains(&json!("txtDescription1"))
        );
        assert_error_keys_on_screen("2550M", &validated["errors"]);
    }

    #[test]
    fn catalog_lists_have_unique_values_across_the_schema() {
        let mut supplied = 0;
        let mut missing = Vec::new();
        for form in schema() {
            let code = form["c"].as_str().unwrap();
            let options = catalogs::options(kind_of(code).unwrap(), &Value::Null);
            for (key, rows) in options.as_object().unwrap() {
                let rows = rows.as_array().unwrap();
                let unique: BTreeSet<_> =
                    rows.iter().map(|o| o["value"].as_str().unwrap()).collect();
                assert_eq!(
                    unique.len(),
                    rows.len(),
                    "{code} {key} repeats a dropdown value"
                );
                assert!(
                    rows.iter()
                        .all(|o| o["label"].as_str().is_some_and(|s| !s.is_empty())),
                    "{code} {key}"
                );
            }
            for part in form["parts"].as_array().unwrap() {
                for item in part["items"].as_array().unwrap() {
                    for f in item["f"].as_array().into_iter().flatten() {
                        if f["t"] == "sel" && f["o"].as_array().is_none_or(Vec::is_empty) {
                            let key = f["k"].as_str().unwrap();
                            if options.get(key).is_some() {
                                supplied += 1;
                            } else {
                                missing.push(format!("{code} {key}"));
                            }
                        }
                    }
                }
            }
        }
        println!(
            "Catalogs supply {supplied} schema dropdowns (including dependent lists awaiting a parent); remaining: {missing:#?}"
        );
        // Explicitly retain the non-catalog / unbound cases as open work.
        // They must not silently turn into an incomplete catalog rollout.
        // D01-D10 left this list with reviewed schema
        // options (schema-overrides.json, from each form's official HTA).
        // 80, plus 1606 Item 16A and 1706 Item 14A's RDO lists (R05). The
        // 36 2550M Schedule 1 ATC lists (N03) are table cells, not fields.
        assert_eq!(supplied, 82);
        assert!(missing.is_empty(), "dropdowns without choices: {missing:?}");
    }

    #[test]
    fn screen_form_schema_returns_one_forms_entry() {
        use_ephemeral_database();
        let entry = call("screen_form_schema", vec!["1601-C".into()]).expect("1601C schema");
        assert_eq!(entry["c"], "1601C");
        assert!(
            entry["parts"]
                .as_array()
                .is_some_and(|parts| !parts.is_empty())
        );
        assert!(schema_keys("1601C").contains("frm1601c:txtMonth"));
        assert!(call("screen_form_schema", vec!["9999".into()]).is_err());
    }

    #[test]
    fn screen_forms_index_lists_all_43_forms() {
        use_ephemeral_database();
        let index = call("screen_forms_index", vec![]).expect("index");
        let rows = index.as_array().expect("array");
        assert_eq!(rows.len(), 43);
        for row in rows {
            for field in ["c", "t", "rev", "freq", "pages", "n", "q"] {
                assert!(
                    row.get(field).is_some_and(|value| !value.is_null()),
                    "{row} lacks {field}"
                );
            }
            let code = row["c"].as_str().unwrap();
            assert!(
                AgentFormKind::from_code(code).is_some(),
                "{code} is an agent form"
            );
        }
        assert!(rows.iter().any(|row| row["c"] == "0619E"));
    }

    #[test]
    fn screen_form_open_names_the_boxes_the_profile_fills() {
        setup();
        let open = |code: &str, period: i64| {
            let future = module()
                .begin(
                    "screen_form_open",
                    &args(vec![TIN.into(), code.into(), YEAR.into(), period.into()]),
                )
                .expect("sync half");
            to_json(&block_on(future).expect("future"))
        };
        let has = |opened: &Value, key: &str| {
            opened["profile"]
                .as_array()
                .is_some_and(|keys| keys.iter().any(|k| k == key))
        };
        let q = open("1603Q", 1);
        assert!(has(&q, "frm1603Q:rdoCode"), "{}", q["profile"]);
        // The treaty choice is the filer's (D01), whatever the schema guessed.
        assert!(!has(&q, "frm1603Q:selTreaty"));
        let pt = open("1600PT", 3);
        assert!(!has(&pt, "frm1600PT:Pg2Sc1TIN1"), "a payee TIN is typed");
    }

    #[test]
    fn screen_form_open_builds_a_new_draft_with_values_and_locks() {
        setup();
        // Sync half (main thread, reads the DB), then the future.
        let future = module()
            .begin(
                "screen_form_open",
                &args(vec![TIN.into(), "1601C".into(), YEAR.into(), 3.into()]),
            )
            .expect("sync half");
        let opened = to_json(&block_on(future).expect("future"));
        assert_eq!(opened["form_code"], "1601C");
        assert_eq!(opened["period"], 3);
        assert_eq!(opened["status"], "Draft");
        assert_eq!(opened["editable"], true);
        assert_eq!(opened["draft"]["tin"], TIN);
        assert_eq!(opened["draft"]["month"], 3);
        assert!(
            opened["values"]["frm1601c:txtTaxpayerName"]
                .as_str()
                .is_some_and(|name| name.contains("EXAMPLE TRADING CORP"))
        );
        let locks = &opened["locks"];
        assert!(
            locks["frm1601c:txtMonth"]
                .as_str()
                .unwrap()
                .contains("reopen")
        );
        assert!(
            locks["frm1601c:txtTax21"]
                .as_str()
                .unwrap()
                .contains("computed")
        );
        assert!(
            locks["frm1601c:txtRDOCode"]
                .as_str()
                .unwrap()
                .contains("profile")
        );
        assert!(locks.get("frm1601c:txtTax14").is_none());
        assert_error_keys_on_screen("1601C", &opened["errors"]);

        // Wrong period, unknown form, unknown TIN: refused in the sync half.
        let refuse =
            |values: Vec<HostValue>| module().begin("screen_form_open", &args(values)).err();
        assert!(refuse(vec![TIN.into(), "1601C".into(), YEAR.into(), 13.into()]).is_some());
        assert!(refuse(vec![TIN.into(), "1601X".into(), YEAR.into(), 1.into()]).is_some());
        assert!(
            refuse(vec![
                "999999999".into(),
                "1601C".into(),
                YEAR.into(),
                1.into()
            ])
            .is_some()
        );
    }

    #[test]
    fn screen_form_fill_applies_edits_and_rejects_bad_ones() {
        setup();
        let opened = open("1601C", 3);
        let filled = fill(
            "1601C",
            &opened["draft"],
            json!({
                "frm1601c:TaxWithheld_1": "true",
                "frm1601c:txtTax14": "50,000.00",
                "frm1601c:txtTax15": "abc",
                "frm1601c:txtTax21": "1",
                "frm1601c:txtMonth": "4",
            }),
        );
        let values = &filled["values"];
        assert_eq!(values["frm1601c:TaxWithheld_1"], "true");
        assert_eq!(values["frm1601c:TaxWithheld_2"], "false");
        assert_eq!(amount(values, "frm1601c:txtTax14"), 50_000.0);
        // Computed boxes follow the edit.
        assert_eq!(amount(values, "frm1601c:txtTax22"), 50_000.0);
        let rejected = filled["rejected"].as_object().unwrap();
        let mut keys: Vec<&str> = rejected.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "frm1601c:txtMonth",
                "frm1601c:txtTax15",
                "frm1601c:txtTax21"
            ]
        );
        // A rejected edit leaves its field as it was.
        assert_eq!(filled["draft"]["month"], 3);
        assert_eq!(amount(values, "frm1601c:txtTax15"), 0.0);

        // A return that left Draft takes no edits.
        let mut queued = filled["draft"].clone();
        queued["status"] = json!("Queued");
        let refused = fill("1601C", &queued, json!({"frm1601c:txtTax14": "1"}));
        assert!(refused["rejected"]["frm1601c:txtTax14"].is_string());
    }

    #[test]
    fn screen_form_validate_reports_errors_with_schema_keys() {
        setup();
        let opened = open("0619E", 5);
        let filled = fill(
            "0619E",
            &opened["draft"],
            json!({"frm0619E:optWithheld:Y": "true"}),
        );
        let future = module()
            .begin(
                "screen_form_validate",
                &args(vec!["0619E".into(), json(filled["draft"].clone())]),
            )
            .expect("sync half");
        let validated = to_json(&block_on(future).expect("future"));
        let errors = validated["errors"].as_array().unwrap();
        assert!(
            !errors.is_empty(),
            "a withheld return with no remittance fails"
        );
        assert!(
            errors
                .iter()
                .any(|error| !error["keys"].as_array().unwrap().is_empty()),
            "{errors:?}"
        );
        assert_error_keys_on_screen("0619E", &validated["errors"]);
        assert!(validated["values"]["frm0619E:optWithheld:Y"] == "true");
        assert!(
            module()
                .begin(
                    "screen_form_validate",
                    &args(vec!["0619E".into(), "x".into()])
                )
                .is_err()
        );
    }

    #[test]
    fn screen_form_save_saves_drafts_and_refuses_filed_returns() {
        setup();
        let opened = open("1601C", 2);
        let filled = fill(
            "1601C",
            &opened["draft"],
            json!({"frm1601c:txtTax14": "1000"}),
        );
        let saved = save("1601C", &filled["draft"]);
        assert!(saved["id"].as_i64().is_some_and(|id| id > 0));
        assert_eq!(saved["status"], "Draft");
        assert_eq!(saved["draft"]["id"], saved["id"]);

        let mut queued = saved["draft"].clone();
        queued["status"] = json!("Queued");
        assert!(call("screen_form_save", vec!["1601C".into(), json(queued)]).is_err());
    }

    #[test]
    fn every_form_opens() {
        setup();
        let mut failures = Vec::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let period = match bir_core::forms::find_form(code).map(|form| &form.frequency) {
                Some(FilingFrequency::Annual) => 0,
                _ => 1,
            };
            match call_async(
                "screen_form_open",
                vec![TIN.into(), code.into(), YEAR.into(), period.into()],
            ) {
                Ok(opened) => {
                    assert_eq!(opened["status"], "Draft", "{code}");
                    assert!(
                        opened["values"].as_object().is_some_and(|v| !v.is_empty()),
                        "{code}"
                    );
                    assert_error_keys_on_screen(code, &opened["errors"]);
                }
                Err(error) => failures.push(format!("{code}: {error}")),
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    /// open -> fill user boxes by key -> save -> reopen: the values stay.
    fn round_trip(code: &str, period: i64, edits: Value) -> Value {
        let opened = open(code, period);
        for key in edits.as_object().unwrap().keys() {
            assert!(
                schema_keys(code).contains(key),
                "{code}: {key} is on screen"
            );
            assert!(
                opened["locks"].get(key).is_none(),
                "{code}: {key} is editable"
            );
        }
        let filled = fill(code, &opened["draft"], edits.clone());
        assert_eq!(filled["rejected"], json!({}), "{code}");
        let saved = save(code, &filled["draft"]);
        assert!(saved["id"].as_i64().is_some_and(|id| id > 0), "{code}");
        let reopened = open(code, period);
        // Saving the reopened draft again updates the same row.
        save(code, &reopened["draft"]);
        let rows = call("list_drafts", vec![TIN.into(), YEAR.into()]).expect("list_drafts");
        let rows: Vec<&Value> = rows
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["form_code"] == code)
            .collect();
        assert_eq!(rows.len(), 1, "{code}: {rows:?}");
        assert_eq!(rows[0]["id"], saved["id"], "{code}");
        for (key, value) in edits.as_object().unwrap() {
            let wanted = value.as_str().unwrap();
            let got = reopened["values"][key].as_str().unwrap_or_default();
            match agent::parse_amount(wanted) {
                Some(number) if wanted != "true" => {
                    assert_eq!(agent::parse_amount(got), Some(number), "{code}: {key}")
                }
                _ => assert_eq!(got, wanted, "{code}: {key}"),
            }
            assert_eq!(
                reopened["values"][key], filled["values"][key],
                "{code}: {key}"
            );
        }
        assert_error_keys_on_screen(code, &reopened["errors"]);
        let validated = call_async(
            "screen_form_validate",
            vec![code.into(), json(reopened["draft"].clone())],
        )
        .expect("validates");
        assert_error_keys_on_screen(code, &validated["errors"]);
        reopened
    }

    #[test]
    fn form_0619e_round_trips() {
        setup();
        round_trip(
            "0619E",
            5,
            json!({
                "frm0619E:optWithheld:Y": "true",
                "frm0619E:optAmend:N": "true",
                "frm0619E:txtTax14": "2500.50",
            }),
        );
    }

    #[test]
    fn form_1601c_round_trips() {
        setup();
        round_trip(
            "1601C",
            4,
            json!({
                "frm1601c:TaxWithheld_1": "true",
                "frm1601c:CatAgent_P": "true",
                "frm1601c:txtTax14": "120000",
                "frm1601c:txtTax25": "8000",
            }),
        );
    }

    #[test]
    fn form_1601eq_round_trips() {
        setup();
        round_trip(
            "1601EQ",
            2,
            json!({
                "frm1601EQ:optWithheld:Y": "true",
                "frm1601EQ:optCategory:P": "true",
                "frm1601EQ:txtTax20": "1500",
            }),
        );
    }

    #[test]
    fn form_2550q_round_trips() {
        setup();
        round_trip(
            "2550Q",
            1,
            json!({
                "frm2550qv2024:calendarNo1": "true",
                "frm2550qv2024:exemptSales": "10000",
                "frm2550qv2024:domesticPurchase": "4000",
            }),
        );
    }

    #[test]
    fn form_2551q_round_trips() {
        setup();
        let schema = schema_keys("2551Q");
        let def = AgentFormKind::Form2551Q.definition();
        // The first unlocked amount box on screen.
        let key = def
            .fillable()
            .find(|entry| {
                schema.contains(&entry.key)
                    && matches!(
                        def.bindings.get(&entry.key).map(|bound| &bound.binding),
                        Some(agent::Binding::Amount { .. })
                    )
            })
            .expect("2551Q has an amount box")
            .key
            .clone();
        round_trip("2551Q", 2, json!({ key: "750" }));
    }

    #[test]
    fn form_2551q_email_shows_on_both_occurrences() {
        setup();
        let opened = open("2551Q", 2);
        let values = &opened["values"];
        // The field map writes the email once, under its official key; the
        // page binds the two places the official form shows it.
        assert_eq!(values["txtEmail"], "books@example.com");
        assert_eq!(values["txtEmail#occurrence-1"], "books@example.com");
        assert_eq!(values["txtEmail#occurrence-2"], "books@example.com");
        assert!(schema_keys("2551Q").contains("txtEmail#occurrence-1"));
        // The print sheet prints the same value.
        let draft: bir_core::forms::form_2551q::Form2551QDraft =
            serde_json::from_value(opened["draft"].clone()).expect("typed 2551Q");
        assert_eq!(
            draft
                .to_print_field_map()
                .get("txtEmail")
                .map(String::as_str),
            Some("books@example.com")
        );
        // Still read-only: the box is locked with an accurate reason and a
        // write by key is refused, so nothing bypasses the typed draft.
        let lock = opened["locks"]["txtEmail#occurrence-1"].as_str().unwrap();
        assert!(
            lock.contains("repeats the official field `txtEmail`"),
            "{lock}"
        );
        let filled = fill(
            "2551Q",
            &opened["draft"],
            json!({"txtEmail#occurrence-1": "other@example.com"}),
        );
        assert!(filled["rejected"]["txtEmail#occurrence-1"].is_string());
        assert_eq!(
            filled["values"]["txtEmail#occurrence-2"],
            "books@example.com"
        );
    }

    #[test]
    fn a_repeated_occurrence_follows_its_bound_box() {
        setup();
        // 1707A binds Item 11's email; its second occurrence mirrors it.
        let opened = open("1707A", 1);
        let base = "frm1707Av2021:txtI11Email";
        let repeat = "frm1707Av2021:txtI11Email#occurrence-2";
        assert_eq!(opened["values"][repeat], "books@example.com");
        assert_eq!(opened["values"][repeat], opened["values"][base]);
        // The email comes from the profile: the repeat says so too.
        assert!(
            opened["locks"][base]
                .as_str()
                .is_some_and(|lock| lock.contains("profile"))
        );
        assert_eq!(opened["locks"][repeat], opened["locks"][base]);
        // 2200A repeats the TIN and name header on three sheets.
        let opened = open("2200A", 3);
        for official in [
            "frm2200Av2020:tinA",
            "frm2200Av2020:branchCode",
            "frm2200Av2020:registeredName",
        ] {
            let value = &opened["values"][official];
            assert!(value.as_str().is_some_and(|v| !v.is_empty()), "{official}");
            for n in [2, 3] {
                assert_eq!(
                    &opened["values"][format!("{official}#occurrence-{n}")],
                    value
                );
            }
        }
    }

    /// Every `#occurrence-N` key on screen names the official control id the
    /// rules corpus serializes it under (`serialized_key`), which is what
    /// the alias reads.
    #[test]
    fn occurrence_keys_repeat_their_serialized_key() {
        for form in schema() {
            let package = form["id"].as_str().unwrap();
            let code = form["c"].as_str().unwrap();
            let occurrences: Vec<&String> = schema_keys(code)
                .iter()
                .filter(|key| key.contains("#occurrence-"))
                .collect();
            if occurrences.is_empty() {
                continue;
            }
            let path = format!(
                "{}/../../rules/forms/{package}/fields.json",
                env!("CARGO_MANIFEST_DIR")
            );
            let fields: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).expect(&path)).unwrap();
            for key in occurrences {
                let row = fields["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["field_key"] == key.as_str())
                    .unwrap_or_else(|| panic!("{package}: {key} in fields.json"));
                assert_eq!(
                    row["serialized_key"].as_str(),
                    key.split_once("#occurrence-").map(|(official, _)| official),
                    "{package}: {key}"
                );
            }
        }
    }

    /// The existing draft paths read a draft saved through screen_form_save.
    fn existing_paths_read(code: &str, period: i64, slug: &str, edits: Value, check: (&str, f64)) {
        let opened = open(code, period);
        let filled = fill(code, &opened["draft"], edits);
        assert_eq!(filled["rejected"], json!({}), "{code}");
        save(code, &filled["draft"]);

        let rows = call("list_drafts", vec![TIN.into(), YEAR.into()]).expect("list_drafts");
        let row = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["form_code"] == code)
            .unwrap_or_else(|| panic!("{code} listed: {rows}"));
        assert_eq!(row["status"], "Draft");

        let stored = call(
            "get_form_draft",
            vec![TIN.into(), code.into(), YEAR.into(), period.into()],
        )
        .expect("get_form_draft");
        assert_eq!(stored[check.0].as_f64(), Some(check.1), "{code}: {stored}");

        // form_print_html's own steps, short of launching a browser.
        let kind = AgentFormKind::from_code(code).unwrap();
        let fields = match kind {
            AgentFormKind::Form1601C => {
                serde_json::from_value::<bir_core::forms::Form1601CDraft>(stored.clone())
                    .expect("typed 1601C")
                    .to_print_field_map()
            }
            AgentFormKind::Queueable(queueable) => bir_core::with_queueable_kind!(queueable, F => {
                serde_json::from_value::<F>(stored.clone()).expect("typed draft").print_field_map()
            }),
            AgentFormKind::Form2551Q => unreachable!(),
        };
        bir_print::frozen_html::filled_document_with_receipt(slug, &fields, None)
            .expect("print sheet fills");

        // queue_form_draft loads it and runs the queue's own checks: either
        // it queues, or it refuses on validation (never on reading the row).
        match call(
            "queue_form_draft",
            vec![TIN.into(), code.into(), YEAR.into(), period.into()],
        ) {
            Ok(id) => {
                assert!(id.as_i64().is_some());
                let reopened = open(code, period);
                assert_eq!(reopened["status"], "Queued");
                assert_eq!(reopened["editable"], false);
            }
            Err(error) => {
                let message = error.message();
                assert!(
                    !message.contains("No saved") && !message.contains("not queueable"),
                    "{code}: {message}"
                );
            }
        }
    }

    #[test]
    fn existing_paths_read_a_1601c_screen_draft() {
        setup();
        existing_paths_read(
            "1601C",
            6,
            "1601c-2018",
            json!({
                "frm1601c:TaxWithheld_1": "true",
                "frm1601c:CatAgent_P": "true",
                "frm1601c:txtTax14": "100000",
                "frm1601c:txtTax25": "5000",
            }),
            ("tax_14_total_compensation", 100_000.0),
        );
    }

    #[test]
    fn existing_paths_read_a_0619e_screen_draft() {
        setup();
        existing_paths_read(
            "0619E",
            7,
            "0619e-2018",
            json!({
                "frm0619E:optWithheld:Y": "true",
                "frm0619E:optAmend:N": "true",
                "frm0619E:txtTax14": "3000",
            }),
            ("item_14_amount_of_remittance", 3000.0),
        );
    }
}

#[cfg(test)]
#[path = "screen_form_audit.rs"]
mod audit;
