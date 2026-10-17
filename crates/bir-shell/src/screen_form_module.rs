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
//! here because bir-shell cannot depend on bir-desktop. A Form library
//! opening (`screen_form_sample`) always builds the new one and shows its
//! period and date boxes blank; the page never saves it.
//!
//! A box the profile filled is locked until the filer chooses Edit on it;
//! the edit then stays on this return's draft and never reaches the profile.
//! A box the profile left blank is an ordinary box.
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
mod combs;
mod conditions;
mod penalties;
mod rows_1700;
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
    /** Boxes the taxpayer profile filled (the page tucks and marks them); a box it left blank is not here and not locked. */
    profile: string[];
    /** Filled profile boxes the filer may open with Edit; pass the opened ones to `screen_form_fill` as `unlocked`. */
    profile_edit: string[];
    /** Opened from the Form library: a blank sample; saved once its period is set, it becomes that period's return. */
    sample: boolean;
    /** A sample's boxes shown blank while its period is not set: its period parts and everything that follows the period. */
    blank: string[];
    /** A sample's held box that cannot be typed: what it waits for (shown in the box). */
    needs: Record<string, string>;
    /** Why each locked box is shut, in the filer's words (drawn under its item while the box shows blank). */
    lock_notes: Record<string, string>;
    /** A sample's period parts: page inputs (never draft writes) the filer sets the period with. */
    period_parts: ScreenFormPeriodPart[];
    /** A sample rebuilt for a period its filer completed. */
    period_set: boolean;
    /** An amended return's filed version, kept in its history (null when never amended): open it with `screen_form_filed_original`. */
    filed_original?: { id: number; status: string; filed_on: string | null } | null;
    /** Set on the read-only view of a filed original. */
    filed_original_view?: { status: string; filed_on: string | null };
    /** When a return was sent to BIR and confirmed (a read-only view says so). */
    submitted_at: string | null;
    confirmed_at: string | null;
    /** A new amended return (`screen_form_amend`): the status of the filed return it amends. */
    amends?: ScreenFormStatus;
    /** Boxes another answer turns off right now, each with its tag ("Needs item 13 = Yes"; the page fades them). */
    off: Record<string, string>;
    /** Each party's Surcharge, Interest and Compromise boxes: locked at the computed amounts until Edit penalties opens them (sent as `unlocked`). */
    penalties: ScreenFormPenalties[];
    /** Each pooled address (by its first line) as the draft holds it: the page shows this, never the field map's cut lines. */
    pools: Record<string, string>;
    /** A stored Draft whose profile boxes the filer did not edit followed a later tax-profile change (saved): what changed ("RDO 050 → 043"). Empty when nothing moved. */
    profile_updates?: string[];
    errors: ScreenFormError[];
}
export interface ScreenFormPeriodPart {
    key: string;
    part: "year" | "month" | "year_end_month" | "day" | "quarter" | "date";
    /** For a quarter's tick box: which quarter. */
    quarter: number | null;
}
export interface ScreenFormExisting {
    form_code: string;
    year: number;
    period: number;
    label: string;
    status: ScreenFormStatus;
    updated_at: string;
    total_payable: number | null;
    /** What the page may offer: `load` (a draft), `view` (read-only), `amend` (reached BIR, and the form has an amended-return answer), `keep`. */
    actions: ("load" | "view" | "amend" | "keep")[];
    submitted_at: string | null;
    confirmed_at: string | null;
}
export interface ScreenFormSamplePeriod {
    complete: boolean;
    /** A stored return of the form already has this period. */
    existing?: ScreenFormExisting;
    /** No stored return: a fresh sample built for exactly this period. */
    sample?: ScreenFormOpened;
    /** What `screen_form_open` takes for it: year, period, and the exact stored key (`key`, an existing answer) or the key a new return starts under (`new_key`); null: start it with New return. */
    opens?: { year: number; period: number; key: number | null; new_key?: number | null } | null;
}
export interface ScreenFormPenalties {
    keys: string[];
    /** The filer entered the amounts: they file instead of the computed ones; blanks go back to the computed. */
    edited: boolean;
    /** The party is on the return as it stands (a spouse's column with no spouse is not): only then is Edit offered. */
    applies: boolean;
}
export interface ScreenFormFilled {
    draft: object;
    values: Record<string, string>;
    options: Record<string, ScreenFormOption[]>;
    rejected: Record<string, string>;
    penalties: ScreenFormPenalties[];
    /** Each pooled address (by its first line) as the draft holds it: the page shows this, never the field map's cut lines. */
    pools: Record<string, string>;
    /** A sample's boxes still blank (null for a return). */
    blank?: string[] | null;
}
export interface ScreenFormValidated {
    draft: object;
    values: Record<string, string>;
    options: Record<string, ScreenFormOption[]>;
    errors: ScreenFormError[];
    /** Boxes another answer turns off right now, each with its tag. */
    off: Record<string, string>;
    penalties: ScreenFormPenalties[];
    /** Each pooled address (by its first line) as the draft holds it: the page shows this, never the field map's cut lines. */
    pools: Record<string, string>;
    /** A sample's boxes still blank (null for a return). */
    blank?: string[] | null;
}
export interface ScreenFormOption {
    value: string;
    label: string;
    /** Selected by the typed draft when multiple choices share a code. */
    selected?: boolean;
}
export interface ScreenFormSaved {
    /** A Form library sample: false when its period already has a stored return (nothing saved; `existing` and `opens` are the offer). */
    saved?: boolean;
    existing?: ScreenFormExisting;
    /** A saved sample: open the return it became with these (`screen_form_open`'s year, period and key). */
    opens?: { year: number; period: number; key: number | null };
    id?: number;
    status?: ScreenFormStatus;
    stored_period?: number | null;
    draft?: object;
}
export function screen_form_schema(form_code: string): object;
export function screen_forms_index(): ScreenFormIndexEntry[];
/** `key`: the exact stored return (a transaction's MMDD, 2553's quarter; gone, the open fails), else the latest of the period. `new_key`: no stored return, a new one started under that key (the typed date). */
export function screen_form_open(tin: string, form_code: string, year: number, period: number, key?: number | null, new_key?: number | null): Promise<ScreenFormOpened>;
/** The Form library's blank sample of a form for this taxpayer: reads no stored return and computes no penalty (its draft is marked). Saved once its period is set, it becomes that period's return; queue and print take the saved return. */
export function screen_form_sample(tin: string, form_code: string): Promise<ScreenFormOpened>;
/** A sample's typed period parts (key -> value): incomplete, an existing stored return, or a fresh sample for that period. */
/** A NEW amended return copied from a stored one that reached BIR; nothing is stored until it is saved. The filed version is kept in this return's history. */
export function screen_form_amend(tin: string, form_code: string, year: number, period: number, key?: number | null): Promise<ScreenFormOpened>;
/** An amended return's filed version from its history, read-only. */
export function screen_form_filed_original(form_code: string, snapshot_id: number): Promise<ScreenFormOpened>;
export function screen_form_sample_period(tin: string, form_code: string, parts: Record<string, string>): Promise<ScreenFormSamplePeriod>;
/** `unlocked`: profile boxes the page has open (opened with Edit, or left blank by the profile); their edits stay on this return and never reach the profile. */
export function screen_form_fill(form_code: string, draft: object, edits: Record<string, string | number | boolean | null>, unlocked?: string[]): ScreenFormFilled;
export function screen_form_validate(form_code: string, draft: object): Promise<ScreenFormValidated>;
/** Save a draft. A Form library sample saves only once its period is set, never over a stored return of that period (answering the offer instead); an entry-numbered form takes its next free entry. */
export function screen_form_save(form_code: string, draft: object): ScreenFormSaved;
/** Print preview of a saved draft (opens the filled official sheet in the in-app preview window). */
export function screen_form_print(form_code: string, draft: object): { path: string };
/** Live sync: re-fill the open print preview for this form from the draft plus staged edits (in memory). No-op when no preview is open. */
export function screen_form_preview_sync(form_code: string, draft: object, edits: Record<string, string | number | boolean | null>, unlocked?: string[]): boolean;
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
        .async_function("screen_form_sample", screen_form_sample)
        .async_function("screen_form_sample_period", screen_form_sample_period)
        .async_function("screen_form_amend", screen_form_amend)
        .async_function("screen_form_filed_original", screen_form_filed_original)
        .function("screen_form_fill", screen_form_fill)
        .async_function("screen_form_validate", screen_form_validate)
        .function("screen_form_save", screen_form_save)
        .function("screen_form_print", screen_form_print)
        .function("screen_form_preview_sync", screen_form_preview_sync)
        .function("screen_form_queue", screen_form_queue)
        .function("screen_form_cancel_queue", screen_form_cancel_queue)
}

fn host_error(message: impl Into<String>) -> HostError {
    HostError::new(message)
}

// ---------------------------------------------------------------- schema

/// The on-screen form schema: one entry per form, parsed once, with the
/// reviewed corrections in `app/forms/schema-overrides.json` applied.
/// 1700's entry is replaced by `app/forms/screen-1700-v2018.json`: the
/// extracted entry follows the 2013 rule package, while the model, its
/// upload and the printed sheet are the January 2018 revision.
fn schema() -> &'static [Value] {
    static SCHEMA: OnceLock<Vec<Value>> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        let mut forms: Vec<Value> =
            serde_json::from_str(include_str!("../../../app/forms/forms-schema.json"))
                .expect("app/forms/forms-schema.json is a JSON array of forms");
        let form_1700: Value =
            serde_json::from_str(include_str!("../../../app/forms/screen-1700-v2018.json"))
                .expect("app/forms/screen-1700-v2018.json is one form");
        for form in forms.iter_mut().filter(|form| form["c"] == "1700") {
            *form = form_1700.clone();
        }
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
            retype_by_binding(form, &code);
            combs::apply_combs(form, &code);
        }
        forms
    })
}

/// What a number box holds, from the draft field it writes and its label:
/// a count or quantity (`number_of_*`, "No. of …", 2200M and 2200S's
/// volumes) or a tax rate (`tax_rate`,
/// `…_rate_percent`), else an amount. Other rates (2200M and 2200P's
/// specific excise rates, pesos per unit; 1602Q's BSP exchange rate) are
/// pesos and stay amounts.
#[derive(PartialEq)]
enum Quantity {
    Count,
    TaxRate,
    Amount,
}

fn quantity(path: &str, label: &str) -> Quantity {
    let leaf = path
        .rsplit('/')
        .find(|segment| !segment.is_empty() && !segment.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or_default();
    let words: Vec<&str> = leaf.split('_').collect();
    let label = label.to_ascii_lowercase();
    if leaf.starts_with("number_of")
        || ["count", "volume", "quantity"]
            .iter()
            .any(|w| words.contains(w))
        || ["no. of", "no of ", "number of", "volume", "quantity"]
            .iter()
            .any(|w| label.contains(w))
    {
        Quantity::Count
    } else if words.contains(&"rate") && (words.contains(&"tax") || words.contains(&"percent")) {
        Quantity::TaxRate
    } else {
        Quantity::Amount
    }
}

/// A plain text box that is one entry of a draft list another entry of
/// which keeps a whole date (1702MX's effectivity dates, columns A and C).
fn dated_list_entry(def: &AgentFormDef, key: &str) -> bool {
    use bir_core::forms::agent::DateFormat;
    // `/a/b/2` -> `/a/b`: only an entry of a list (a numeric last segment).
    let list = |path: &str| {
        path.rsplit_once('/')
            .filter(|(_, index)| !index.is_empty() && index.chars().all(|c| c.is_ascii_digit()))
            .map(|(list, _)| list.to_string())
    };
    let Some(Binding::Text { path, .. }) = def.bindings.get(key).map(|b| &b.binding) else {
        return false;
    };
    let Some(mine) = list(path) else {
        return false;
    };
    def.bindings.values().any(|bound| {
        matches!(&bound.binding, Binding::Text {
            path,
            date: Some(DateFormat::Mdy | DateFormat::Iso),
            part: None,
            ..
        } if list(path).as_deref() == Some(mine.as_str()))
    })
}

/// How a box is typed on the page: money only where its binding is an
/// amount (rule 7), a date where it holds a whole date (rule 4).
///
/// - A text binding is never money (2550Q's contact number, 1701MS's
///   taxpayer name take what the filer types, never "₱ 281,234,567.00").
/// - An amount binding is a peso box (1606's 17B Interest), a whole-number
///   one keeps the schema's type and, as money, shows whole pesos (`w`):
///   its draft field holds no centavos. Either one holding a count is a
///   number box, and a tax rate a percent box ([`quantity`]).
/// - A text binding that keeps a whole date is a date box, and so is a
///   plain text or unbound box the print sheet prints as a whole date
///   (2200T's payment dates, 2550Q's Return Period From) or a list entry
///   whose sibling keeps one: the page shows it as "Dec 10, 2020" and sends
///   MM/DD/YYYY. A box showing one part of a date stays that part's box.
/// - Unbound and computed boxes otherwise keep the schema's type.
fn page_type(
    def: &AgentFormDef,
    code: &str,
    key: &str,
    schema_type: &str,
    label: &str,
) -> (String, bool) {
    use bir_core::forms::agent::DateFormat;
    let binding = def.bindings.get(key).map(|bound| &bound.binding);
    if matches!(schema_type, "text" | "money" | "int" | "date") {
        let whole_date = matches!(
            binding,
            Some(Binding::Text {
                date: Some(DateFormat::Mdy | DateFormat::Iso),
                part: None,
                ..
            })
        );
        let plain = matches!(
            binding,
            None | Some(Binding::Text {
                date: None,
                part: None,
                ..
            })
        );
        if whole_date
            || (plain && (combs::printed_as_date(code, key) || dated_list_entry(def, key)))
        {
            return ("date".into(), false);
        }
    }
    let kept = (schema_type.to_string(), false);
    match binding {
        Some(Binding::Text { .. }) if schema_type == "money" => ("text".into(), false),
        Some(Binding::Amount { path, .. } | Binding::Whole { path, .. })
            if matches!(schema_type, "text" | "int" | "date" | "money" | "pct") =>
        {
            let whole = matches!(binding, Some(Binding::Whole { .. }));
            match quantity(path, label) {
                Quantity::Count => ("int".into(), false),
                Quantity::TaxRate => ("pct".into(), false),
                Quantity::Amount if whole && schema_type == "money" => ("money".into(), true),
                Quantity::Amount if !whole && schema_type != "pct" => ("money".into(), false),
                Quantity::Amount => kept,
            }
        }
        _ => kept,
    }
}

/// Apply [`page_type`] to every field, and to every schedule column whose
/// bound cells all agree.
fn retype_by_binding(form: &mut Value, code: &str) {
    let Some(kind) = AgentFormKind::from_code(code) else {
        return;
    };
    let def = kind.definition();
    let binding = |key: &str| def.bindings.get(key).map(|bound| &bound.binding);
    let items = form["parts"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array_mut().into_iter().flatten());
    for item in items {
        for field in item["f"].as_array_mut().into_iter().flatten() {
            let (Some(key), Some(t)) = (field["k"].as_str(), field["t"].as_str()) else {
                continue;
            };
            let label = field["l"].as_str().unwrap_or_default().to_string();
            let (t, whole) = page_type(def, code, key, t, &label);
            field["t"] = Value::from(t);
            if whole {
                field["w"] = Value::Bool(true);
            }
        }
        let rows: Vec<Vec<String>> = item["keys"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|row| {
                row.as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|key| key.as_str().map(str::to_string))
                    .collect()
            })
            .collect();
        for (c, col) in item["cols"]
            .as_array_mut()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let Some(t) = col["t"].as_str().map(str::to_string) else {
                continue;
            };
            let label = col["l"].as_str().unwrap_or_default().to_string();
            let bound: Vec<&String> = rows
                .iter()
                .filter_map(|row| row.get(c))
                .filter(|key| binding(key).is_some())
                .collect();
            let mut typed: BTreeSet<(String, bool)> = bound
                .iter()
                .map(|key| page_type(def, code, key, &t, &label))
                .collect();
            // A date column some of whose rows the binding data left as plain
            // text (1604C Part II) is a date column: plain text keeps the
            // MM/DD/YYYY the page sends.
            let plain_text = |key: &&String| {
                matches!(
                    binding(key),
                    Some(Binding::Text {
                        date: None,
                        part: None,
                        ..
                    })
                )
            };
            if typed.contains(&("date".to_string(), false))
                && bound
                    .iter()
                    .all(|key| plain_text(key) || page_type(def, code, key, &t, &label).0 == "date")
            {
                typed = BTreeSet::from([("date".to_string(), false)]);
            }
            if let [(t, whole)] = typed.into_iter().collect::<Vec<_>>().as_slice() {
                col["t"] = Value::from(t.as_str());
                if *whole {
                    col["w"] = Value::Bool(true);
                }
            }
        }
        // A table column whose every cell is bound to text (2200S's
        // `txtsched1TaxBracket{N}`, /others/N/tax_bracket) is a text column.
        let rows: Vec<Vec<String>> = item["keys"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|row| {
                row.as_array()
                    .into_iter()
                    .flatten()
                    .map(|key| key.as_str().unwrap_or("").to_string())
                    .collect()
            })
            .collect();
        for (c, col) in item["cols"]
            .as_array_mut()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let cells: Vec<&String> = rows.iter().filter_map(|row| row.get(c)).collect();
            let text = !cells.is_empty()
                && cells.iter().all(|key| {
                    matches!(
                        def.bindings.get(key.as_str()).map(|bound| &bound.binding),
                        Some(Binding::Text { .. })
                    )
                });
            if text && col["t"] == "money" {
                col["t"] = Value::from("text");
            }
        }
    }
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

/// Whether the schema marks `key` computed (`c`), as a field or as its
/// table column.
fn schema_computed(form_code: &str, key: &str) -> bool {
    // The schema writes the flag as `1`.
    let flagged = |c: &Value| c.as_bool() == Some(true) || c.as_u64() == Some(1);
    let Some(form) = schema_entry(form_code) else {
        return false;
    };
    form["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array().into_iter().flatten())
        .any(|item| {
            let field = item["f"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|f| f["k"] == key && flagged(&f["c"]));
            let cell = item["keys"].as_array().into_iter().flatten().any(|row| {
                row.as_array()
                    .into_iter()
                    .flatten()
                    .position(|k| k == key)
                    .is_some_and(|c| flagged(&item["cols"][c]["c"]))
            });
            field || cell
        })
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

/// The stored generic-queue draft in `period`'s column; none without a
/// store (a Form library sample never reads the taxpayer's returns).
fn stored<F: QueueableForm>(
    db: Option<&Database>,
    tin: &str,
    year: u16,
    period: i64,
) -> Result<Option<F>, String> {
    let Some(db) = db else {
        return Ok(None);
    };
    db.get_queueable_draft::<F>(tin, year, period)
        .map_err(|error| read_error(F::FORM_CODE, error))
}

/// The most recently updated stored draft among `periods` that `keep` accepts.
fn latest<F: QueueableForm>(
    db: Option<&Database>,
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
    db: Option<&Database>,
    tin: &str,
    year: u16,
) -> Result<Option<F>, String> {
    match (stored::<F>(db, tin, year, 0)?, db) {
        (Some(draft), _) => Ok(Some(draft)),
        (None, Some(db)) => db
            .get_form_draft_v2::<F>(tin, F::FORM_CODE, year, &FilingPeriod::Annual)
            .map_err(|error| read_error(F::FORM_CODE, error)),
        (None, None) => Ok(None),
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

/// An exact-key open or amend whose return is gone.
const NO_LONGER_STORED: &str =
    "That return is no longer stored. Open the form again from its period or New return.";

/// The draft each painted page opens for `(profile, year, period)`: the
/// stored one, else a new one from the profile. `fresh` (a Form library
/// sample) always builds the new one and reads no stored return.
fn load_draft(
    db: &Database,
    kind: AgentFormKind,
    profile: &TaxpayerProfile,
    year: u16,
    period: u8,
    fresh: bool,
    exact: Option<i64>,
) -> Result<Value, String> {
    use bir_core::forms::*;
    let tin = profile.tin.full();
    let tin = tin.as_str();
    let store = (!fresh).then_some(db);
    // Opened by its stored key, the return must still be stored: a miss is
    // an error, never another return of the period or a new one.
    let exact = exact.filter(|_| store.is_some());
    let queueable = match kind {
        AgentFormKind::Form1601C => {
            let month = period.clamp(1, 12);
            let draft = match store
                .map(|db| db.get_1601c_draft(tin, year, month))
                .transpose()
                .map_err(|error| read_error("1601C", error))?
                .flatten()
            {
                Some(draft) => draft,
                None if exact.is_some() => return Err(NO_LONGER_STORED.to_string()),
                None => {
                    form_1601c::Form1601CDraft::new_unanswered_from_profile(profile, year, month)
                }
            };
            return serde_json::to_value(draft).map_err(|error| error.to_string());
        }
        AgentFormKind::Form2551Q => {
            let quarter = period.clamp(1, 4);
            let draft = match store
                .map(|db| db.get_2551q_draft(tin, year, quarter))
                .transpose()
                .map_err(|error| read_error("2551Q", error))?
                .flatten()
            {
                Some(draft) => draft,
                None if exact.is_some() => return Err(NO_LONGER_STORED.to_string()),
                None => form_2551q::Form2551QDraft::new_from_profile(profile, year, quarter),
            };
            return serde_json::to_value(draft).map_err(|error| error.to_string());
        }
        AgentFormKind::Queueable(kind) => kind,
    };
    // The one stored return under `exact` (a transaction's `MMDD`, 2553's
    // quarter) rather than the latest of the year; none stored, a new one.
    if let (Some(key), Some(store)) = (exact, store)
        && queueable != QueueableKind::Form2550Q
    {
        let found = bir_core::with_queueable_kind!(queueable, F => {
            match stored::<F>(Some(store), tin, year, key)? {
                Some(found) => Some(finish(db, profile, found)?),
                None => None,
            }
        });
        return found.ok_or_else(|| NO_LONGER_STORED.to_string());
    }
    let column = i64::from(period);
    macro_rules! simple {
        ($ty:ty, $new:expr) => {{
            let draft: $ty = stored::<$ty>(store, tin, year, column)?.unwrap_or_else(|| $new);
            finish(db, profile, draft)
        }};
    }
    match queueable {
        QueueableKind::Form2550Q => {
            let quarter = period.clamp(1, 4);
            let draft = store
                .map(|db| {
                    db.get_form_draft::<form_2550q::Form2550QDraft>(
                        tin,
                        "2550Q",
                        year,
                        Some(quarter),
                    )
                })
                .transpose()
                .map_err(|error| read_error("2550Q", error))?
                .flatten();
            if draft.is_none() && exact.is_some() {
                return Err(NO_LONGER_STORED.to_string());
            }
            let mut draft = draft.unwrap_or_else(|| {
                form_2550q::Form2550QDraft::new_from_profile(profile, year, quarter)
            });
            bir_core::form_rules::prepare_form_2550q_live_row_identities(&mut draft)
                .map_err(|error| format!("the 2550Q draft's row identities are unsafe: {error}"))?;
            serde_json::to_value(draft).map_err(|error| error.to_string())
        }
        QueueableKind::Form0605 => {
            type F = form_0605::Form0605Draft;
            let slot = period.max(1);
            let draft = match (stored::<F>(store, tin, year, i64::from(slot))?, store) {
                (Some(draft), _) => Some(draft),
                (None, Some(db)) => db
                    .get_form_draft_v2::<F>(
                        tin,
                        "0605",
                        year,
                        &FilingPeriod::OpenEnded(u32::from(slot)),
                    )
                    .map_err(|error| read_error("0605", error))?,
                (None, None) => None,
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
            let draft = stored_annual::<F>(store, tin, year)?.unwrap_or_else(|| {
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
            let draft = stored_annual::<F>(store, tin, year)?.unwrap_or_else(|| {
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
            let draft = stored_annual::<F>(store, tin, year)?.unwrap_or_else(|| {
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
            let mut draft = stored::<F>(store, tin, year, column)?
                .unwrap_or_else(|| F::new_from_profile(profile, year, u32::from(period.max(1))));
            // A TIN fixed in the profile reaches a still-editable draft.
            if draft.lifecycle().is_editable() {
                draft.tin = tin.to_string();
            }
            finish(db, profile, draft)
        }
        QueueableKind::Form1801 => {
            type F = form_1801::Form1801Draft;
            let mut draft = stored::<F>(store, tin, year, column)?
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
            let draft = latest::<F>(store, tin, year, mmdd_keys(0..=12, 0..=31), |draft| {
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
            let draft = latest::<F>(store, tin, year, mmdd_keys(month..=month, 0..=31), |_| true)?
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
            let draft = latest::<F>(store, tin, year, mmdd_keys(months, 0..=31), |_| true)?
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
            let draft = latest::<F>(store, tin, year, mmdd_keys(month..=month, 0..=31), |_| true)?
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
            let draft = latest::<F>(store, tin, year, mmdd_keys(0..=12, 0..=31), |draft| {
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
            let draft = latest::<F>(store, tin, year, 1..=4, |_| true)?
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
    if def.bindings.contains_key(&entry.key) && is_profile_name(&entry.key) {
        return Some(bir_core::forms::agent::Lock::Profile.reason().to_string());
    }
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

/// The filer's own name or registered name. The open draft copies it from
/// the taxpayer profile, but the probe compares raw profile strings and the
/// forms print it in capitals, so it is not always detected as a profile box.
/// Spouse, payee, buyer and seller names stay typed.
fn is_profile_name(key: &str) -> bool {
    let tail = key.rsplit(':').next().unwrap_or(key).to_ascii_lowercase();
    let Some(rest) = tail.strip_prefix("txt") else {
        return false;
    };
    let stem = rest.trim_end_matches(|c: char| c.is_ascii_digit());
    ["taxpayername", "registeredname"].iter().any(|name| {
        stem.strip_suffix(name).is_some_and(|prefix| {
            prefix
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, 'p' | 'g' | 't' | 'i' | 'm'))
        })
    })
}

/// The draft the profile alone makes for a return's period, and its field
/// map: what decides whether the profile filled a box.
struct FromProfile {
    draft: Value,
    values: Value,
}

/// Why each box is locked on the page. A profile box is locked only where
/// the profile gave it a value ([`FromProfile`]; the page's Edit then opens
/// it for this return); one the profile left blank loses its profile lock
/// and is an ordinary box, whatever the filer has typed into it since.
fn locks(kind: AgentFormKind, from_profile: &FromProfile) -> Map<String, Value> {
    let def = kind.definition();
    let profile_reason = bir_core::forms::agent::Lock::Profile.reason();
    let mut locks: Map<String, Value> = def
        .boxes
        .iter()
        .filter_map(|entry| {
            let open = is_profile_box(def, entry) && profile_blank(def, &entry.key, from_profile);
            lock_reason(def, entry)
                .filter(|reason| !(open && reason == profile_reason))
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
                lock: Some(bir_core::forms::agent::Lock::Profile),
                ..
            }) => {
                if profile_blank(def, official, from_profile) {
                    format!("repeats `{official}`; fill it there")
                } else {
                    format!("repeats `{official}` from the tax profile; use Edit on that box")
                }
            }
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

fn has_profile_lock(def: &AgentFormDef, key: &str) -> bool {
    matches!(
        def.bindings.get(key),
        Some(BoundBox {
            lock: Some(bir_core::forms::agent::Lock::Profile),
            ..
        })
    )
}

/// A box the taxpayer profile fills: bound (itself or through its item's
/// group box) with a profile lock, or the filer's own name.
fn is_profile_box(def: &AgentFormDef, entry: &BoxDef) -> bool {
    !entry.computed
        && entry.is_user_box()
        && def.bindings.contains_key(&entry.key)
        && (has_profile_lock(def, &entry.key)
            || is_profile_name(&entry.key)
            || entry
                .group
                .as_deref()
                .is_some_and(|group| has_profile_lock(def, group)))
}

/// Whether the profile left `key` blank. A bound box reads the draft field
/// it writes (a Page 2 header bound to `/taxpayer_name` is filled when the
/// name is, whatever its own field-map slot shows); a field the draft does
/// not have yet falls back to the field map, and a box missing there too
/// counts as filled. Blank: no text, an all-zero code (the official
/// runtime's empty RDO, `000`), zero, false, or no radio of its item ticked.
fn profile_blank(def: &AgentFormDef, key: &str, from_profile: &FromProfile) -> bool {
    let zeros = |text: &str| text.trim().chars().all(|c| c == '0');
    let shown = |key: &str| {
        from_profile.values[key]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string()
    };
    if let Some(
        Binding::Text { path, .. } | Binding::Amount { path, .. } | Binding::Whole { path, .. },
    ) = def.bindings.get(key).map(|bound| &bound.binding)
    {
        return match from_profile.draft.pointer(path) {
            Some(Value::Null) => true,
            Some(Value::String(text)) => zeros(text),
            Some(Value::Number(number)) => number.as_f64() == Some(0.0),
            Some(Value::Bool(set)) => !set,
            Some(_) => false,
            None => from_profile
                .values
                .get(key)
                .is_some_and(|_| zeros(&shown(key))),
        };
    }
    let entry = def.box_def(key);
    let group = match entry {
        Some(entry) if entry.radio_group => Some(entry),
        Some(entry) => entry.group.as_deref().and_then(|group| def.box_def(group)),
        None => None,
    };
    match (group, entry) {
        (Some(group), _) => !group.members.iter().any(|member| shown(member) == "true"),
        (None, Some(entry)) if entry.is_radio() => shown(key) != "true",
        _ => zeros(&shown(key)),
    }
}

/// Boxes the taxpayer profile filled (`from_profile`), plus repeated
/// occurrences of one; a box the profile left blank is not listed. The
/// extracted schema's auto flag is a guess; this is what the app does.
fn profile_keys(kind: AgentFormKind, from_profile: &FromProfile) -> Vec<String> {
    let def = kind.definition();
    let mut keys: Vec<String> = def
        .boxes
        .iter()
        .filter(|entry| is_profile_box(def, entry) && !profile_blank(def, &entry.key, from_profile))
        .map(|entry| entry.key.clone())
        .collect();
    for (key, official) in occurrence_aliases(kind) {
        if has_profile_lock(def, official) && !profile_blank(def, official, from_profile) {
            keys.push(key.clone());
        }
    }
    keys
}

/// The filled profile boxes the page may open with Edit: the edit stays on
/// this return (`screen_form_fill`'s `unlocked`) and never reaches the
/// profile. Repeated occurrences are not here; they follow their box.
fn profile_edit_keys(kind: AgentFormKind, from_profile: &FromProfile) -> Vec<String> {
    let def = kind.definition();
    profile_keys(kind, from_profile)
        .into_iter()
        .filter(|key| {
            def.bindings.contains_key(key)
                && def
                    .box_def(key)
                    .is_some_and(|entry| entry.required != "computed")
        })
        .collect()
}

/// Where a draft keeps `field` (`profile_edits`, `profile_baseline`): at
/// its top level. Every generic draft flattens its lifecycle there, and
/// 1601C and 2551Q keep the field at the top level themselves; only a draft
/// that nests its lifecycle under `lifecycle` (none today) keeps it there.
fn lifecycle_pointer(draft: &Value, field: &str) -> String {
    if draft["lifecycle"].is_object() {
        format!("/lifecycle/{field}")
    } else {
        format!("/{field}")
    }
}

fn profile_edits_pointer(draft: &Value) -> String {
    lifecycle_pointer(draft, "profile_edits")
}

/// The profile boxes the filer edited on this return.
fn profile_edits(draft: &Value) -> BTreeSet<String> {
    draft
        .pointer(&profile_edits_pointer(draft))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|key| key.as_str().map(str::to_string))
        .collect()
}

/// Record that the filer edited profile box `key` on this return.
fn mark_profile_edit(draft: &mut Value, key: &str) {
    let mut keys = profile_edits(draft);
    if keys.insert(key.to_string()) {
        let pointer = profile_edits_pointer(draft);
        set_draft_path(draft, &pointer, json!(keys));
    }
}

/// How a profile box's change reads in the note at the top of a draft:
/// its item's caption without the parenthetical or a trailing "code"
/// ("RDO Code" reads "RDO"), and what the page showed before and after.
fn profile_change_note(code: &str, key: &str, before: &Value, after: &Value) -> Option<String> {
    let form = schema_entry(code)?;
    let (item, field) = form["parts"]
        .as_array()?
        .iter()
        .flat_map(|part| part["items"].as_array().into_iter().flatten())
        .find_map(|item| {
            item["f"]
                .as_array()?
                .iter()
                .find(|field| field["k"] == key)
                .map(|field| (item, field))
        })?;
    let caption = item["l"].as_str().unwrap_or_default();
    let caption = caption.split(" (").next().unwrap_or(caption).trim();
    let caption = caption
        .strip_suffix(" Code")
        .or_else(|| caption.strip_suffix(" code"))
        .unwrap_or(caption);
    let text = |values: &Value| values[key].as_str().unwrap_or_default().trim().to_string();
    let (was, now) = (text(before), text(after));
    if matches!(field["t"].as_str(), Some("radio" | "chk")) {
        // A radio reads as the choice it moved to.
        if now != "true" {
            return None;
        }
        let picked = field["l"].as_str().unwrap_or(key);
        return Some(format!("{caption} → {picked}"));
    }
    let shown = |text: &str| {
        if text.is_empty() {
            "blank".to_string()
        } else {
            text.to_string()
        }
    };
    Some(format!("{caption} {} → {}", shown(&was), shown(&now)))
}

/// The draft fields the form's profile boxes write (period fields aside).
fn profile_paths(kind: AgentFormKind) -> BTreeSet<&'static str> {
    let def = kind.definition();
    def.boxes
        .iter()
        .filter(|entry| is_profile_box(def, entry))
        .flat_map(|entry| bound_paths(def, &entry.key))
        .filter(|path| !is_period_path(path))
        .collect()
}

/// What the profile makes in each of those fields (`from_profile`: the
/// draft the profile alone builds): a draft's `profile_baseline`.
fn profile_values(kind: AgentFormKind, from_profile: &Value) -> Map<String, Value> {
    profile_paths(kind)
        .into_iter()
        .filter_map(|path| Some((path.to_string(), from_profile.pointer(path)?.clone())))
        .collect()
}

/// Record what the profile makes now as the baseline of a Draft that has
/// none (a new return, a sample, an amendment): its first save keeps it.
fn record_profile_baseline(kind: AgentFormKind, draft: &mut Value, from_profile: &Value) {
    let pointer = lifecycle_pointer(draft, "profile_baseline");
    if draft
        .pointer(&pointer)
        .and_then(Value::as_object)
        .is_some_and(|baseline| !baseline.is_empty())
    {
        return;
    }
    let values = profile_values(kind, from_profile);
    if !values.is_empty() {
        set_draft_path(draft, &pointer, Value::Object(values));
    }
}

/// A Draft follows a later change of the tax profile in every profile box
/// the filer did not edit on it (`profile_edits`). The draft keeps the
/// profile values its boxes were built from (`profile_baseline`); a box
/// takes the profile's new value (`from_profile`, what the profile alone
/// makes now) only when the profile changed since that baseline AND the
/// box still holds the baseline value, so a value typed or written by any
/// writer that differs from the profile is never replaced. A draft with no
/// baseline (saved before it existed) changes nothing: it only records the
/// profile as it is now. Returns what changed, in the filer's words, for
/// the note at the top of the page ("RDO 050 → 043"); `None` when neither
/// a value nor the baseline moved, always on a return that left Draft. The
/// profile is only read, never written.
fn follow_profile(
    kind: AgentFormKind,
    draft: &mut Value,
    from_profile: &Value,
) -> Result<Option<Vec<String>>, String> {
    if agent::identity(kind, draft)?.status != FilingStatus::Draft {
        return Ok(None);
    }
    let code = kind.form_code();
    let def = kind.definition();
    let edits = profile_edits(draft);
    // A pooled address is one box: an edit to it keeps all its lines.
    let pools = combs::pools(code);
    let edited = |key: &str| {
        edits.contains(key)
            || pools.values().any(|pool| {
                pool.keys.iter().any(|k| k == key) && pool.keys.iter().any(|k| edits.contains(k))
            })
            || def.box_def(key).is_some_and(|entry| {
                entry.group.as_deref().is_some_and(|group| {
                    edits.contains(group)
                        || def
                            .box_def(group)
                            .is_some_and(|group| group.members.iter().any(|m| edits.contains(m)))
                })
            })
    };
    let profile_boxes: Vec<&String> = def
        .boxes
        .iter()
        .filter(|entry| is_profile_box(def, entry))
        .map(|entry| &entry.key)
        .collect();
    let paths = profile_paths(kind);
    let kept: BTreeSet<&str> = profile_boxes
        .iter()
        .filter(|key| edited(key))
        .flat_map(|key| bound_paths(def, key))
        .collect();
    let pointer = lifecycle_pointer(draft, "profile_baseline");
    let baseline = draft.pointer(&pointer).and_then(Value::as_object).cloned();
    let now = profile_values(kind, from_profile);
    let mut next = draft.clone();
    let mut moved = false;
    if let Some(baseline) = baseline.as_ref().filter(|baseline| !baseline.is_empty()) {
        for path in paths.iter().filter(|path| !kept.contains(*path)) {
            let (Some(was), Some(want)) = (baseline.get(*path), now.get(*path)) else {
                continue;
            };
            if want != was && next.pointer(path) == Some(was) {
                set_draft_path(&mut next, path, want.clone());
                moved = true;
            }
        }
    }
    let rebased = baseline.as_ref() != Some(&now);
    if rebased {
        set_draft_path(&mut next, &pointer, Value::Object(now));
    }
    if !moved {
        if rebased {
            *draft = next;
            return Ok(Some(Vec::new()));
        }
        return Ok(None);
    }
    let mut before = draft.clone();
    agent::compute(kind, &mut before)?;
    let before = values_json(kind, &before)?;
    agent::compute(kind, &mut next)?;
    let after = values_json(kind, &next)?;
    let mut notes: Vec<String> = Vec::new();
    for key in profile_boxes {
        if before[key.as_str()] == after[key.as_str()] {
            continue;
        }
        if let Some(note) = profile_change_note(code, key, &before, &after)
            && !notes.contains(&note)
        {
            notes.push(note);
        }
    }
    *draft = next;
    Ok(Some(notes))
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
                // A penalty item's message names the model's field.
                if keys.is_empty() {
                    keys.extend(penalties::error_keys(kind, field));
                }
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
    // A profile box the field map leaves empty (2200S's Schedule 1 name
    // header, written only once that sheet prints) shows the draft field it
    // writes, so a locked box never reads blank; the map, XML and print are
    // unchanged.
    for entry in def.boxes.iter().filter(|entry| is_profile_box(def, entry)) {
        if map
            .get(&entry.key)
            .is_some_and(|shown| !shown.trim().is_empty())
        {
            continue;
        }
        if let Some(Binding::Text { path, .. }) = def.bindings.get(&entry.key).map(|b| &b.binding)
            && let Some(Value::String(text)) = draft.pointer(path)
            && !text.trim().is_empty()
        {
            map.insert(entry.key.clone(), text.trim().to_string());
        }
    }
    // A box the official runtime stores escape()d (1601EQ's line of
    // business: `Wholesale%20trading`) shows the text the filer typed; the
    // field map, the XML and the print keep the escaped value.
    for (key, bound) in &def.bindings {
        let Binding::Text {
            path,
            date: None,
            part: None,
            ..
        } = &bound.binding
        else {
            continue;
        };
        let Some(Value::String(text)) = draft.pointer(path) else {
            continue;
        };
        let text = text.trim();
        let escaped = bir_core::official_xml::js_escape(text);
        if escaped != text && map.get(key) == Some(&escaped) {
            map.insert(key.clone(), text.to_string());
        }
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

/// The optional list of profile boxes the page has open (see `apply_screen_edits`).
fn unlocked_argument(arguments: &HostArguments, index: usize) -> BTreeSet<String> {
    match arguments.get(index).map(to_json) {
        Some(Value::Array(keys)) => keys
            .iter()
            .filter_map(|key| key.as_str().map(str::to_string))
            .collect(),
        _ => BTreeSet::new(),
    }
}

fn screen_form_open(
    arguments: &HostArguments,
) -> Result<impl Future<Output = HostResult> + Send + use<>, HostError> {
    open_screen_form(arguments, false)
}

/// A Form library opening: the form as a blank sample for the taxpayer. It
/// never reads or replaces a stored return, shows its period and date boxes
/// blank (`blank`) until the filer types one, and is built for next year so
/// no deadline has passed and no penalty is computed. Its draft carries
/// [`SAMPLE_MARK`]: save makes it a return once its period is set
/// ([`save_sample`]); queue and print refuse it until then.
fn screen_form_sample(
    arguments: &HostArguments,
) -> Result<impl Future<Output = HostResult> + Send + use<>, HostError> {
    open_screen_form(arguments, true)
}

/// Marks a Form library sample's draft, with the boxes it holds blank while
/// its period is not set (`{"blank": [...], "set": false}`; `blank` empty
/// and `set` true once the sample is rebuilt for a complete period). The
/// typed draft has no such field, so the host takes it off before any agent
/// call and puts it back on what it returns (`take_sample_mark`,
/// `put_sample_mark`). A sample's period parts are page inputs, never draft
/// writes: no fill lifts a period lock.
const SAMPLE_MARK: &str = "screen_form_sample";

struct SampleMark {
    blank: Vec<String>,
    /// Rebuilt for a complete period: Save makes it that period's return.
    set: bool,
}

/// The sample's mark, or `None` for a return.
fn take_sample_mark(draft: &mut Value) -> Option<SampleMark> {
    let Value::Object(object) = draft else {
        return None;
    };
    match object.remove(SAMPLE_MARK)? {
        Value::Null | Value::Bool(false) => None,
        mark => Some(SampleMark {
            blank: mark["blank"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|key| key.as_str().map(str::to_string))
                .collect(),
            set: mark["set"] == true,
        }),
    }
}

fn put_sample_mark(draft: &mut Value, mark: Option<&SampleMark>) {
    if let (Some(mark), Value::Object(object)) = (mark, draft) {
        object.insert(
            SAMPLE_MARK.into(),
            json!({ "blank": mark.blank, "set": mark.set }),
        );
    }
}

/// Queue and print take a return, never a Form library sample: the page
/// saves a sample first (`screen_form_save`), which makes it a return.
fn refuse_sample(draft: &mut Value, doing: &str) -> Result<(), HostError> {
    if take_sample_mark(draft).is_some() {
        return Err(host_error(format!(
            "a sample from the Form library cannot be {doing}; save it first, which makes it a return"
        )));
    }
    Ok(())
}

fn open_screen_form(
    arguments: &HostArguments,
    sample: bool,
) -> Result<impl Future<Output = HostResult> + Send + use<>, HostError> {
    use chrono::Datelike;
    let tin: String = arguments
        .string(0)?
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let kind = kind_of(arguments.string(1)?)?;
    let code = kind.form_code();
    let (year, period) = if sample {
        // A placeholder period: the sample is rebuilt for the one its filer
        // completes, and holds every period-derived box until then.
        let next = u16::try_from(chrono::Local::now().year() + 1)
            .map_err(|_| host_error("year is out of range"))?;
        (next, page_period(code, 1).map_err(host_error)?)
    } else {
        let year =
            u16::try_from(arguments.integer(2)?).map_err(|_| host_error("year is out of range"))?;
        (
            year,
            page_period(code, arguments.integer(3)?).map_err(host_error)?,
        )
    };
    // The exact stored return to open (an offer's `opens.key`): a
    // transaction's `MMDD`, 2553's quarter; gone, the open fails. Or the key
    // a new return is started under (`opens.new_key`: the typed date of a
    // sample with no stored return).
    let (exact, new_key) = if sample {
        (None, None)
    } else {
        (
            arguments.get(4).and_then(|key| to_json(key).as_i64()),
            arguments.get(5).and_then(|key| to_json(key).as_i64()),
        )
    };
    // The draft to show; the one the profile alone makes for this period
    // (what decides whether the profile filled a box); and for a sample, one
    // for another year and period (what tells which boxes follow the period).
    let (draft, from_profile, build, filed_original, followed) = with_database(|db| {
        let profile = db
            .get_profile_by_tin(&tin)
            .map_err(|error| host_error(error.to_string()))?
            .ok_or_else(|| host_error(format!("no taxpayer profile with TIN {tin}")))?;
        let mut draft =
            load_draft(db, kind, &profile, year, period, sample, exact).map_err(host_error)?;
        let mut from_profile = if sample {
            draft.clone()
        } else {
            load_draft(db, kind, &profile, year, period, true, None).map_err(host_error)?
        };
        // A new return started for the date its key names.
        if let Some(key) = new_key
            && exact.is_none()
            && draft["id"].is_null()
        {
            let parts = sample_period_parts(kind);
            let typed = typed_from_key(year, key);
            write_typed_period(kind, &parts, &typed, &mut draft).map_err(host_error)?;
            write_typed_period(kind, &parts, &typed, &mut from_profile).map_err(host_error)?;
        }
        // A stored Draft follows the tax profile in every box the filer did
        // not edit on it, saved quietly; the page says what changed.
        // Never in the way of opening it: a follow or save that fails leaves
        // the draft open as it is (or as followed, in memory).
        let followed = if sample {
            None
        } else {
            follow_profile(kind, &mut draft, &from_profile).unwrap_or_else(|error| {
                tracing::warn!("{code}: following the tax profile failed: {error}");
                None
            })
        };
        if followed.is_some() && matches!(stored_return(db, kind, &draft), Ok(Some(_))) {
            let mut saved = draft.clone();
            match refuse_replacing_a_stored_return(db, kind, &saved)
                .and_then(|()| agent::save_draft(kind, db, &mut saved))
            {
                Ok(_) => draft = saved,
                Err(error) => tracing::warn!("{code}: saving the followed draft failed: {error}"),
            }
        }
        let followed = followed.unwrap_or_default();
        let build = if sample {
            let period = page_period(code, 2).map_err(host_error)?;
            Some(SampleBuild {
                other: Some(
                    load_draft(db, kind, &profile, year + 1, period, true, None)
                        .map_err(host_error)?,
                ),
            })
        } else {
            None
        };
        let filed_original = if sample {
            Value::Null
        } else {
            filed_original(db, kind, &draft).map_err(host_error)?
        };
        Ok((draft, from_profile, build, filed_original, followed))
    })?;
    Ok(async move {
        let mut opened =
            on_large_stack(move || opened_json(kind, year, period, draft, from_profile, build))
                .and_then(|result| result)
                .map_err(host_error)?;
        opened["filed_original"] = filed_original;
        opened["profile_updates"] = json!(followed);
        Ok(json(opened))
    })
}

/// How a sample is built: `other`, a draft for another year and period
/// while the filer has not set one (`None` once the sample is rebuilt for a
/// complete period).
struct SampleBuild {
    other: Option<Value>,
}

/// Under a locked box the page shows blank (`lock_notes`), why it is shut,
/// in the filer's words: never a key name or the app's internal reason.
const NOTE_COMPUTED: &str = "Computed by the form.";
const NOTE_PROFILE: &str = "Comes from the tax profile.";
const NOTE_IDENTITY: &str = "Set by the return you opened (its taxpayer and period).";
const NOTE_REPEAT: &str = "Repeats another box on this form; fill it there.";
const NOTE_NOT_YET: &str = "Can't be filled on this page yet.";
/// Only for a box a form group is assigned to open (the pinned list).
const NOTE_ASSIGNED: &str = "Can't be filled on this page yet. It will be added for this form.";

/// Shut boxes each assigned to the form group that opens them
/// (`CODE key…: reason | group`), the same list the locked-and-blank
/// invariant pins.
const ASSIGNED_SHUT_BOXES: &str = include_str!("screen_form_module/locked_and_blank.txt");

/// Whether a form group is assigned to open `code`'s box `key`.
fn assigned_to_open(code: &str, key: &str) -> bool {
    ASSIGNED_SHUT_BOXES
        .lines()
        .filter(|line| !line.starts_with('#') && line.contains(" | "))
        .any(|line| {
            let mut words = line.split(' ');
            words.next() == Some(code)
                && words.next().map(|word| word.trim_end_matches(':')) == Some(key)
        })
}

/// A lock's reason as a filer reads it (see the NOTE_* texts), for
/// `code`'s box `key`.
fn lock_note_for(code: &str, key: &str, reason: &str) -> &'static str {
    match lock_note(reason) {
        NOTE_NOT_YET if assigned_to_open(code, key) => NOTE_ASSIGNED,
        note => note,
    }
}

/// A lock's reason as a filer reads it (see the NOTE_* texts).
fn lock_note(reason: &str) -> &'static str {
    use bir_core::forms::agent::Lock;
    if reason.starts_with("computed") {
        NOTE_COMPUTED
    } else if reason == Lock::Profile.reason() || reason.contains("from the tax profile") {
        NOTE_PROFILE
    } else if reason == Lock::Identity.reason() {
        NOTE_IDENTITY
    } else if reason.starts_with("repeats `") && reason.ends_with("; fill it there") {
        NOTE_REPEAT
    } else {
        // Unmapped, derived, written by the app, or a field this page
        // cannot write: shut until the form's own work opens it.
        NOTE_NOT_YET
    }
}

/// What `screen_form_open`, `screen_form_sample` and a rebuilt sample answer.
fn opened_json(
    kind: AgentFormKind,
    year: u16,
    period: u8,
    mut draft: Value,
    from_profile: Value,
    sample: Option<SampleBuild>,
) -> Result<Value, String> {
    let code = kind.form_code();
    let (mut values, mut errors, status) = evaluate_here(kind, &mut draft)?;
    let mut from_profile_draft = from_profile;
    agent::compute(kind, &mut from_profile_draft)?;
    let from_profile = FromProfile {
        values: values_json(kind, &from_profile_draft)?,
        draft: from_profile_draft,
    };
    if status == FilingStatus::Draft {
        record_profile_baseline(kind, &mut draft, &from_profile.draft);
    }
    let mut locks = locks(kind, &from_profile);
    // A computed penalty is locked until Edit penalties opens its group; a
    // return with no due date computes none, and its boxes stay open.
    for group in penalties::computed_groups(kind) {
        if !penalties::edited(&group, &draft) {
            let reason = if penalties::applies(kind, &group, &draft) {
                penalties::LOCK
            } else {
                penalties::NOT_ON_RETURN
            };
            for key in &group.keys {
                locks.insert(key.clone(), json!(reason));
            }
        }
    }
    let profile = profile_keys(kind, &from_profile);
    let profile_edit = profile_edit_keys(kind, &from_profile);
    let parts = if sample.is_some() {
        sample_period_parts(kind)
    } else {
        Vec::new()
    };
    let mut blank: Vec<String> = Vec::new();
    let mut needs = Map::new();
    if let Some(build) = &sample {
        if let Some(other) = &build.other {
            let mut other = other.clone();
            agent::compute(kind, &mut other)?;
            let other = values_json(kind, &other)?;
            // The period parts start blank, and so does everything that
            // follows the period until the filer sets it.
            blank = parts.iter().map(|(key, _)| key.clone()).collect();
            for key in sample_blank_keys(kind, &values, &other) {
                if !blank.contains(&key) {
                    blank.push(key);
                }
            }
        }
        // Period parts are page inputs in a sample, however a return locks
        // them; a period box the form computes stays shut.
        for (key, _) in &parts {
            locks.remove(key);
        }
        for key in DERIVED_PERIOD_BOXES {
            if schema_keys(code).contains(key) {
                locks.insert(key.to_string(), json!("computed from the year-end month"));
            }
        }
        for key in &blank {
            if locks.contains_key(key) {
                needs.insert(key.clone(), json!(SAMPLE_NEEDS));
            }
        }
    }
    blank_sample(&blank, &mut values, &mut errors);
    let lock_notes: Map<String, Value> = locks
        .iter()
        .map(|(key, reason)| {
            (
                key.clone(),
                json!(lock_note_for(code, key, reason.as_str().unwrap_or(""))),
            )
        })
        .collect();
    let stored_period = if sample.is_some() {
        None
    } else {
        stored_period(kind, &draft)
    };
    let options = catalogs::options(kind, &draft);
    let off = if status == FilingStatus::Draft {
        conditions::off_boxes(kind, &draft, &values)
    } else {
        Map::new()
    };
    let penalty_groups = penalties::json(kind, &draft);
    let pool_texts = combs::pool_texts(kind, &draft);
    let mark = sample.as_ref().map(|build| SampleMark {
        blank: blank.clone(),
        set: build.other.is_none(),
    });
    put_sample_mark(&mut draft, mark.as_ref());
    let period_parts: Vec<Value> = parts
        .iter()
        .map(|(key, part)| {
            let quarter = match part {
                PeriodPart::Quarter(quarter) => Some(*quarter),
                _ => None,
            };
            json!({ "key": key, "part": part.name(), "quarter": quarter })
        })
        .collect();
    Ok(json!({
        "form_code": code,
        "year": year,
        "period": period,
        "status": status_name(&status),
        "editable": status == FilingStatus::Draft,
        "stored_period": stored_period,
        "draft": draft,
        "values": values,
        "options": options,
        "locks": locks,
        "lock_notes": lock_notes,
        "profile": profile,
        "profile_edit": profile_edit,
        "sample": sample.is_some(),
        "blank": blank,
        "needs": needs,
        "off": off,
        "penalties": penalty_groups,
        "pools": pool_texts,
        "period_parts": period_parts,
        "period_set": sample.as_ref().is_some_and(|build| build.other.is_none()),
        "submitted_at": draft["submitted_at"].as_str().or_else(|| draft["lifecycle"]["submitted_at"].as_str()),
        "confirmed_at": draft["confirmed_at"].as_str().or_else(|| draft["lifecycle"]["confirmed_at"].as_str()),
        "errors": errors,
    }))
}

/// A sample's filer typed period parts (key -> value). While the period is
/// incomplete the answer is `{complete: false}`. Complete, it is either
/// `existing` (this taxpayer already has a stored return of the form for
/// that period: its period, label, status, last save and total payable),
/// or `sample`, a fresh sample built from the profile for exactly that
/// period, with nothing held and no trace of the placeholder period. Both
/// carry `opens`, the year and period `screen_form_open` takes for it.
fn screen_form_sample_period(
    arguments: &HostArguments,
) -> Result<impl Future<Output = HostResult> + Send + use<>, HostError> {
    let tin: String = arguments
        .string(0)?
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let kind = kind_of(arguments.string(1)?)?;
    let code = kind.form_code();
    let typed: Map<String, Value> = match to_json(arguments.value(2)?) {
        Value::Object(map) => map,
        _ => return Err(host_error("parts must be an object of key -> value")),
    };
    let parts = sample_period_parts(kind);
    let raw_parts = typed.clone();
    let typed = read_typed_period(code, &parts, &typed);
    enum Answer {
        Incomplete,
        Existing(Value, Value),
        Fresh(Value, u16, u8, Value),
    }
    let answer = match complete_period(kind, &parts, &typed) {
        None => Answer::Incomplete,
        Some((year, period)) => with_database(|db| {
            let profile = db
                .get_profile_by_tin(&tin)
                .map_err(|error| host_error(error.to_string()))?
                .ok_or_else(|| host_error(format!("no taxpayer profile with TIN {tin}")))?;
            let mut draft =
                load_draft(db, kind, &profile, year, period, true, None).map_err(host_error)?;
            write_typed_period(kind, &parts, &typed, &mut draft).map_err(host_error)?;
            agent::compute(kind, &mut draft).map_err(host_error)?;
            // A per-transaction form numbered by entry, not by date, cannot
            // match a typed date to a stored return.
            let numbered = NUMBERED_FORMS.contains(&code);
            let stored = if numbered {
                None
            } else {
                stored_for_period(db, kind, &draft).map_err(host_error)?
            };
            Ok(match stored {
                // Opened by its own stored key: exactly the return matched.
                Some(stored) => Answer::Existing(
                    existing_return(db, kind, &stored).map_err(host_error)?,
                    json!({ "year": year, "period": period, "key": stored_period(kind, &stored) }),
                ),
                // Start a return opens a new return for that period; for an
                // entry-numbered form, New return.
                None => {
                    let opens = if numbered {
                        Value::Null
                    } else {
                        json!({ "year": year, "period": period, "key": null, "new_key": stored_period(kind, &draft) })
                    };
                    Answer::Fresh(draft, year, period, opens)
                }
            })
        })?,
    };
    Ok(async move {
        let answer = match answer {
            Answer::Incomplete => json!({ "complete": false }),
            Answer::Existing(existing, opens) => json!({
                "complete": true,
                "existing": existing,
                "opens": opens,
            }),
            Answer::Fresh(draft, year, period, opens) => {
                let build = SampleBuild { other: None };
                let from_profile = draft.clone();
                let mut sample = on_large_stack(move || {
                    opened_json(kind, year, period, draft, from_profile, Some(build))
                })
                .and_then(|result| result)
                .map_err(host_error)?;
                // A period part with no box in the field map (0605's quarter
                // ticks) still shows what the filer set.
                let def = kind.definition();
                for (key, _) in sample_period_parts(kind) {
                    let empty = sample["values"][&key].as_str().is_none_or(str::is_empty);
                    let unbound = !def.bindings.contains_key(&key);
                    if let (true, Some(value)) = (empty || unbound, raw_parts.get(&key)) {
                        sample["values"][&key] = value.clone();
                    }
                }
                json!({
                    "complete": true,
                    "sample": sample,
                    "opens": opens,
                })
            }
        };
        Ok(json(answer))
    })
}

/// Words of a period or date box's key or draft path.
const PERIOD_WORDS: [&str; 7] = ["year", "month", "quarter", "qtr", "date", "day", "period"];
/// Dates that are facts about the taxpayer, kept in a sample: a birth date,
/// a date of incorporation, the start of the business.
const PROFILE_DATE_WORDS: [&str; 3] = ["birth", "incorporat", "start"];

/// The draft paths a box writes (its own binding, else its item's group box).
fn bound_paths<'a>(def: &'a AgentFormDef, key: &str) -> Vec<&'a str> {
    let bound = def.bindings.get(key).or_else(|| {
        def.box_def(key)
            .and_then(|entry| entry.group.as_deref())
            .and_then(|group| def.bindings.get(group))
    });
    match bound.map(|bound| &bound.binding) {
        Some(
            Binding::Text { path, .. } | Binding::Amount { path, .. } | Binding::Whole { path, .. },
        ) => vec![path.as_str()],
        Some(Binding::Choice { options }) => options
            .iter()
            .flat_map(|option| option.set.keys().map(String::as_str))
            .collect(),
        None => Vec::new(),
    }
}

/// Whether a draft path is one of [`agent::PERIOD_FIELDS`] (`/taxable_year`,
/// `/year_end_month`, `/is_short_period`, `/quarter`).
fn is_period_path(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|leaf| agent::PERIOD_FIELDS.contains(&leaf))
}

/// The period and date boxes a Form library sample shows blank: a box bound
/// to a period path ([`is_period_path`]), a box whose value moves with the
/// year and period the draft is built for (`here` against `other`, such as a
/// due date), and a period-named box with no binding. Amounts, boxes the
/// profile fills and the taxpayer's own dates (birth, incorporation) keep
/// their values.
fn sample_blank_keys(kind: AgentFormKind, here: &Value, other: &Value) -> Vec<String> {
    let def = kind.definition();
    let Some(form) = schema_entry(kind.form_code()) else {
        return Vec::new();
    };
    let profile: BTreeSet<String> = def
        .boxes
        .iter()
        .filter(|entry| is_profile_box(def, entry))
        .map(|entry| entry.key.clone())
        .chain(
            occurrence_aliases(kind)
                .into_iter()
                .filter(|(_, official)| has_profile_lock(def, official))
                .map(|(key, _)| key.clone()),
        )
        .collect();
    let has_word = |text: &str, list: &[&str]| {
        let text = text.to_ascii_lowercase();
        list.iter().any(|word| text.contains(word))
    };
    let blank = |key: &str, t: &str| {
        let name = key.split_once(':').map_or(key, |(_, name)| name);
        let paths = bound_paths(def, key);
        let amount = t == "money"
            || matches!(
                def.bindings.get(key).map(|bound| &bound.binding),
                Some(Binding::Amount { .. })
            );
        if amount
            || profile.contains(key)
            || has_word(name, &PROFILE_DATE_WORDS)
            || paths.iter().any(|path| has_word(path, &PROFILE_DATE_WORDS))
        {
            return false;
        }
        let shown = |values: &Value| values[key].as_str().unwrap_or("").to_string();
        let moves = !shown(here).is_empty() && shown(here) != shown(other);
        let named = paths.is_empty()
            && has_word(name, &PERIOD_WORDS)
            && ["sel", "int", "radio", "text", "date"].contains(&t);
        paths.iter().any(|path| is_period_path(path)) || moves || named
    };
    let mut keys = Vec::new();
    let items = form["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array().into_iter().flatten());
    for item in items {
        for field in item["f"].as_array().into_iter().flatten() {
            if let (Some(key), Some(t)) = (field["k"].as_str(), field["t"].as_str())
                && blank(key, t)
            {
                keys.push(key.to_string());
            }
        }
        let cols = item["cols"].as_array();
        for row in item["keys"].as_array().into_iter().flatten() {
            for (c, key) in row.as_array().into_iter().flatten().enumerate() {
                let t = cols
                    .and_then(|cols| cols.get(c))
                    .and_then(|col| col["t"].as_str());
                if let (Some(key), Some(t)) = (key.as_str(), t)
                    && blank(key, t)
                {
                    keys.push(key.to_string());
                }
            }
        }
    }
    keys
}

/// Show a sample's held boxes blank, and hold back the messages that judge
/// the period the sample was only built for: any message naming a held box
/// (held by key, not by item). Once the filer completes the period the
/// sample is rebuilt for it and nothing is held.
fn blank_sample(blank: &[String], values: &mut Value, errors: &mut Value) {
    if blank.is_empty() {
        return;
    }
    if let Value::Object(map) = values {
        for key in blank {
            if let Some(value) = map.get_mut(key) {
                *value = Value::String(String::new());
            }
        }
    }
    let held: BTreeSet<&str> = blank.iter().map(String::as_str).collect();
    if let Value::Array(list) = errors {
        list.retain(|error| {
            !error["keys"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .any(|key| held.contains(key))
        });
    }
}

/// What a sample's held box (a due date, a period-derived line) says while
/// the period is not set.
const SAMPLE_NEEDS: &str = "Shows once the period is set";

/// Per-transaction forms whose returns are numbered by entry (1, 2, ...)
/// rather than keyed by their date: a typed date cannot match a stored one,
/// and Start a return opens New return.
const NUMBERED_FORMS: [&str; 7] = ["0605", "1606", "1706", "1707", "1707A", "1800", "1801"];

/// Period boxes the form computes although nothing marks them computed:
/// 1707A's year-end day is the last day of its year-end month. Never a part
/// a sample's filer sets; shut in a sample with its reason.
const DERIVED_PERIOD_BOXES: [&str; 1] = ["frm1707Av2021:txtI1YearEndDay"];

/// One part of a return's period, as a sample's filer types it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeriodPart {
    Year,
    /// The month the return is for (a monthly or per-transaction return).
    Month,
    /// The month a fiscal year ends (an annual or quarterly return).
    YearEndMonth,
    Day,
    /// One quarter's tick box or radio.
    Quarter(u8),
    /// One box holding a whole date (MM/DD/YYYY).
    Date,
}

impl PeriodPart {
    fn name(self) -> &'static str {
        match self {
            Self::Year => "year",
            Self::Month => "month",
            Self::YearEndMonth => "year_end_month",
            Self::Day => "day",
            Self::Quarter(_) => "quarter",
            Self::Date => "date",
        }
    }
}

/// A form's period parts, the boxes a sample's filer sets its period with:
/// on-screen boxes bound (themselves or through their item's group box) to
/// a period field ([`agent::PERIOD_FIELDS`]), or, with no binding, named for
/// one (`txtPg1I1Month`, `qtr_1`). Due dates, "from/to" ranges and the
/// short-period answer are not parts: they follow the period.
fn sample_period_parts(kind: AgentFormKind) -> Vec<(String, PeriodPart)> {
    let def = kind.definition();
    let Some(form) = schema_entry(kind.form_code()) else {
        return Vec::new();
    };
    let frequency = bir_core::forms::find_form(kind.form_code())
        .map(|form| form.frequency.clone())
        .unwrap_or(FilingFrequency::Annual);
    let year_end = matches!(
        frequency,
        FilingFrequency::Annual | FilingFrequency::Quarterly
    );
    let digit = |name: &str| {
        name.chars()
            .rev()
            .find(|c| c.is_ascii_digit())
            .and_then(|c| c.to_digit(10))
            .and_then(|d| u8::try_from(d).ok())
            .filter(|d| (1..=4).contains(d))
    };
    let classify = |key: &str, t: &str, first_part: bool| -> Option<PeriodPart> {
        let name = key
            .split_once(':')
            .map_or(key, |(_, name)| name)
            .to_ascii_lowercase();
        if name.contains("due")
            || name.contains("from")
            || name.contains("to")
                && ["tomonth", "today", "toyear", "tono", "to_"]
                    .iter()
                    .any(|w| name.contains(w))
            || name.contains("period")
            || PROFILE_DATE_WORDS.iter().any(|w| name.contains(w))
        {
            return None;
        }
        // A computed box (1707A's year-end day) shows its value; it is not
        // something the filer sets.
        if DERIVED_PERIOD_BOXES.contains(&key)
            || def
                .box_def(key)
                .is_some_and(|entry| entry.computed || entry.required == "computed")
        {
            return None;
        }
        // One part of the transaction's own date, kept whole in the draft.
        // Any other date (1604C's refund date) is an answer, not the period.
        if let Some(Binding::Text {
            path,
            part: Some(part),
            ..
        }) = def.bindings.get(key).map(|bound| &bound.binding)
        {
            if path != "/transaction_date" {
                return None;
            }
            return Some(match part {
                agent::DatePart::Month => PeriodPart::Month,
                agent::DatePart::Day => PeriodPart::Day,
                agent::DatePart::Year => PeriodPart::Year,
            });
        }
        let paths = bound_paths(def, key);
        let leaf = |path: &str| path.rsplit('/').next().unwrap_or("").to_string();
        let leaves: Vec<String> = paths.iter().map(|path| leaf(path)).collect();
        if !paths.is_empty() {
            // The return's own period is a top-level field; a schedule row's
            // year (`/nolco/0/year`) is an entry, not the period.
            if paths.iter().any(|path| path.matches('/').count() > 1) {
                return None;
            }
            let one = |names: &[&str]| leaves.iter().all(|l| names.contains(&l.as_str()));
            return if one(&["quarter"]) {
                digit(&name).map(PeriodPart::Quarter)
            } else if one(&[
                "year",
                "taxable_year",
                "transaction_year",
                "year_end_year",
                "donation_year",
                "death_year",
            ]) {
                Some(PeriodPart::Year)
            } else if one(&["year_end_month"]) {
                Some(PeriodPart::YearEndMonth)
            } else if one(&[
                "month",
                "transaction_month",
                "donation_month",
                "death_month",
            ]) {
                Some(if year_end {
                    PeriodPart::YearEndMonth
                } else {
                    PeriodPart::Month
                })
            } else if one(&[
                "day",
                "transaction_day",
                "year_end_day",
                "donation_day",
                "death_day",
            ]) {
                Some(PeriodPart::Day)
            } else if one(&["transaction_date"]) {
                Some(PeriodPart::Date)
            } else {
                None
            };
        }
        // A box with no binding is a period part only where the return names
        // its period, in the form's first part (not a schedule's year column).
        if !first_part {
            return None;
        }
        if name.contains("quarter") || name.contains("qtr") {
            return digit(&name).map(PeriodPart::Quarter);
        }
        if name.contains("month") {
            return Some(
                if year_end || name.contains("yearend") || name.contains("yrend") {
                    PeriodPart::YearEndMonth
                } else {
                    PeriodPart::Month
                },
            );
        }
        if name.contains("year") {
            return Some(PeriodPart::Year);
        }
        if name.contains("day") && t != "date" {
            return Some(PeriodPart::Day);
        }
        None
    };
    form["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .flat_map(|(index, part)| {
            part["items"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|item| item["f"].as_array().into_iter().flatten())
                .map(move |field| (index == 0, field))
        })
        .filter_map(|(first_part, field)| {
            let key = field["k"].as_str()?;
            let t = field["t"].as_str().unwrap_or("");
            if t == "hidden" || field["c"].as_bool() == Some(true) {
                return None;
            }
            classify(key, t, first_part).map(|part| (key.to_string(), part))
        })
        .collect()
}

/// The period a sample's filer typed, read from its period parts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TypedPeriod {
    year: Option<u16>,
    month: Option<u8>,
    year_end_month: Option<u8>,
    day: Option<u8>,
    quarter: Option<u8>,
}

/// A schema field's `m` (its length), when it has one.
fn field_length(code: &str, key: &str) -> Option<u64> {
    schema_entry(code)?["parts"]
        .as_array()?
        .iter()
        .flat_map(|part| part["items"].as_array().into_iter().flatten())
        .flat_map(|item| item["f"].as_array().into_iter().flatten())
        .find(|field| field["k"] == key)
        .and_then(|field| field["m"].as_u64())
}

fn read_typed_period(
    code: &str,
    parts: &[(String, PeriodPart)],
    typed: &Map<String, Value>,
) -> TypedPeriod {
    let text = |key: &str| match typed.get(key) {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(true)) => "true".into(),
        _ => String::new(),
    };
    // `03 - March` and `3` both read as 3.
    let number = |text: &str| -> Option<u32> {
        let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    };
    let mut period = TypedPeriod::default();
    for (key, part) in parts {
        let value = text(key);
        if value.is_empty() {
            continue;
        }
        match part {
            PeriodPart::Year => {
                // All four digits, or both of a two-digit box: "2", "20" and
                // "202" are a year still being typed, never 2002 or 2020.
                // Only a two-digit box's year is in this century ("24" is
                // 2024); four digits are the year as typed, so "0020" is
                // year 20, out of range, never 2020.
                let digits = value.chars().filter(char::is_ascii_digit).count();
                let short = field_length(code, key) == Some(2) && digits == 2;
                let whole = digits == 4 || short;
                period.year = number(&value)
                    .filter(|_| whole && value.chars().all(|c| c.is_ascii_digit()))
                    .map(|year| if short { year + 2000 } else { year })
                    .filter(|year| (1900..=2100).contains(year))
                    .and_then(|year| u16::try_from(year).ok());
            }
            PeriodPart::Month => {
                period.month = number(&value)
                    .filter(|m| (1..=12).contains(m))
                    .and_then(|m| u8::try_from(m).ok());
            }
            PeriodPart::YearEndMonth => {
                period.year_end_month = number(&value)
                    .filter(|m| (1..=12).contains(m))
                    .and_then(|m| u8::try_from(m).ok());
            }
            PeriodPart::Day => {
                period.day = number(&value)
                    .filter(|d| (1..=31).contains(d))
                    .and_then(|d| u8::try_from(d).ok());
            }
            PeriodPart::Quarter(quarter) => {
                if value == "true" {
                    period.quarter = Some(*quarter);
                }
            }
            PeriodPart::Date => {
                let pieces: Vec<u32> = value
                    .split(['/', '-'])
                    .filter_map(|piece| piece.trim().parse().ok())
                    .collect();
                if let [month, day, year] = pieces[..] {
                    period.month = u8::try_from(month).ok().filter(|m| (1..=12).contains(m));
                    period.day = u8::try_from(day).ok().filter(|d| (1..=31).contains(d));
                    period.year = u16::try_from(year)
                        .ok()
                        .filter(|y| (1900..=2100).contains(y));
                }
            }
        }
    }
    period
}

/// The year and page period (as `screen_form_open` takes them) a typed
/// period completes, or `None` while a part the form's frequency needs is
/// missing: monthly, month and year; quarterly, the quarter (or a month
/// inside it) and year; annual, the year; per transaction, its year and,
/// where the form has them, month and day.
fn complete_period(
    kind: AgentFormKind,
    parts: &[(String, PeriodPart)],
    typed: &TypedPeriod,
) -> Option<(u16, u8)> {
    let code = kind.form_code();
    let year = typed.year?;
    let has = |want: fn(&PeriodPart) -> bool| parts.iter().any(|(_, part)| want(part));
    let frequency = bir_core::forms::find_form(code)
        .map(|form| form.frequency.clone())
        .unwrap_or(FilingFrequency::Annual);
    let period = match frequency {
        FilingFrequency::Monthly => typed.month?,
        FilingFrequency::Quarterly => match (typed.quarter, typed.month) {
            (Some(quarter), _) => quarter,
            (None, Some(month)) if !has(|p| matches!(p, PeriodPart::Quarter(_))) => {
                (month - 1) / 3 + 1
            }
            _ => return None,
        },
        FilingFrequency::Annual => 0,
        FilingFrequency::OpenEnded => {
            if has(|p| matches!(p, PeriodPart::Month | PeriodPart::Date)) && typed.month.is_none() {
                return None;
            }
            if has(|p| matches!(p, PeriodPart::Day | PeriodPart::Date)) && typed.day.is_none() {
                return None;
            }
            if has(|p| matches!(p, PeriodPart::Quarter(_))) && typed.quarter.is_none() {
                return None;
            }
            if NUMBERED_FORMS.contains(&code) {
                1
            } else {
                typed.month.or(typed.quarter).unwrap_or(1)
            }
        }
    };
    page_period(code, i64::from(period))
        .ok()
        .map(|period| (year, period))
}

/// Write every typed period part into a draft the host builds for that
/// period (a library sample's rebuild, a new return for a date), through
/// [`agent::write_period_box`]: the host building its own draft, never a
/// page's edit. A quarter with no box of its own goes into the draft's
/// `/quarter` (0605). A refusal is an error, never skipped.
fn write_typed_period(
    kind: AgentFormKind,
    parts: &[(String, PeriodPart)],
    typed: &TypedPeriod,
    draft: &mut Value,
) -> Result<(), String> {
    let def = kind.definition();
    for (key, part) in parts {
        let value = match part {
            PeriodPart::Year => typed.year.map(|year| year.to_string()),
            PeriodPart::Month => typed.month.map(|month| format!("{month:02}")),
            PeriodPart::YearEndMonth => typed.year_end_month.map(|month| format!("{month:02}")),
            PeriodPart::Day => typed.day.map(|day| format!("{day:02}")),
            PeriodPart::Date => match (typed.month, typed.day, typed.year) {
                (Some(month), Some(day), Some(year)) => Some(format!("{month:02}/{day:02}/{year}")),
                _ => None,
            },
            PeriodPart::Quarter(quarter) => match typed.quarter {
                Some(ticked) if ticked == *quarter => Some("true".to_string()),
                _ => None,
            },
        };
        let Some(value) = value else {
            continue;
        };
        if !def.bindings.contains_key(key) {
            if let (PeriodPart::Quarter(quarter), Some(Value::Number(_))) =
                (part, draft.pointer("/quarter"))
            {
                draft["quarter"] = Value::from(*quarter);
            }
            continue;
        }
        agent::write_period_box(kind, draft, key, &Value::from(value.clone()))
            .map_err(|error| format!("{key} = {value}: {error}"))?;
    }
    Ok(())
}

/// The period a stored return's key names: a transaction's `MMDD`, else a
/// quarter (2553), for a new return opened under that key.
fn typed_from_key(year: u16, key: i64) -> TypedPeriod {
    let mut typed = TypedPeriod {
        year: Some(year),
        ..TypedPeriod::default()
    };
    if key >= 101 {
        typed.month = u8::try_from(key / 100).ok();
        typed.day = u8::try_from(key % 100).ok();
    } else {
        typed.quarter = u8::try_from(key).ok();
    }
    typed
}

/// A period's words for the page: `March 2025`, `Q1 2025`, `2025`, or a
/// transaction's `03/16/2025`.
fn period_label(period: &FilingPeriod, year: u16) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    match period {
        FilingPeriod::Monthly(month) => match MONTHS.get(usize::from(*month).saturating_sub(1)) {
            Some(name) => format!("{name} {year}"),
            None => year.to_string(),
        },
        FilingPeriod::Quarterly(quarter) => format!("Q{quarter} {year}"),
        FilingPeriod::Annual => year.to_string(),
        FilingPeriod::OpenEnded(key) if *key >= 101 => {
            format!("{:02}/{:02}/{year}", key / 100, key % 100)
        }
        FilingPeriod::OpenEnded(key) => format!("No. {key}, {year}"),
    }
}

/// What a page needs to offer a stored return: its period and label, status,
/// when it was last saved and its total amount payable.
fn existing_return(db: &Database, kind: AgentFormKind, stored: &Value) -> Result<Value, String> {
    let identity = agent::identity(kind, stored)?;
    let total_payable = stored["id"]
        .as_i64()
        .and_then(|id| db.draft_amount_payable(id).ok().flatten());
    let updated_at = stored["updated_at"]
        .as_str()
        .or_else(|| stored["lifecycle"]["updated_at"].as_str())
        .unwrap_or("");
    // Draft: load it. Queued: view it while it waits. Reached BIR: view it,
    // or amend it where the form has an amended-return answer (never a
    // Paid return, which the app cannot revert).
    let amend = agent::amended_copy(kind, stored).is_ok();
    let mut actions = match identity.status {
        FilingStatus::Draft => vec!["load"],
        _ => vec!["view"],
    };
    if amend {
        actions.push("amend");
    }
    actions.push("keep");
    let stamp = |name: &str| {
        stored[name]
            .as_str()
            .or_else(|| stored["lifecycle"][name].as_str())
            .map(str::to_string)
    };
    Ok(json!({
        "actions": actions,
        "submitted_at": stamp("submitted_at"),
        "confirmed_at": stamp("confirmed_at"),
        "form_code": kind.form_code(),
        "year": identity.taxable_year,
        "period": bir_core::forms::queueable::period_column(&identity.period),
        "label": period_label(&identity.period, identity.taxable_year),
        "status": status_name(&identity.status),
        "updated_at": updated_at,
        "total_payable": total_payable,
    }))
}

fn screen_form_fill(arguments: &HostArguments) -> HostResult {
    let kind = kind_of(arguments.string(0)?)?;
    let mut draft = draft_argument(arguments, 1)?;
    let sample = take_sample_mark(&mut draft);
    let amendment = take_amendment_mark(&mut draft);
    let edits = arguments
        .value(2)?
        .as_object()
        .ok_or_else(|| host_error("edits must be an object of key -> value"))?;
    let editable = agent::identity(kind, &draft).map_err(host_error)?.status == FilingStatus::Draft;
    let edits: Map<String, Value> = edits.iter().map(|(k, v)| (k.clone(), to_json(v))).collect();
    let unlocked = unlocked_argument(arguments, 3);
    // A sample's period is set with `screen_form_sample_period`, never by a
    // fill: every writer here keeps the period locked.
    let rejected = apply_screen_edits(kind, &mut draft, &edits, editable, &unlocked)?;
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
    let mut values = values_json(kind, &draft).map_err(host_error)?;
    let options = catalogs::options(kind, &draft);
    // A held box the filer typed into (a schedule's month) shows what they
    // typed; the rest stays held until the period is set.
    let sample = sample.map(|mut mark| {
        mark.blank
            .retain(|key| !edits.contains_key(key) || rejected.contains_key(key));
        blank_sample(&mark.blank, &mut values, &mut Value::Null);
        mark
    });
    let penalty_groups = penalties::json(kind, &draft);
    let pool_texts = combs::pool_texts(kind, &draft);
    put_sample_mark(&mut draft, sample.as_ref());
    put_amendment_mark(&mut draft, amendment);
    Ok(json(json!({
        "draft": draft,
        "values": values,
        "options": options,
        "rejected": rejected,
        "penalties": penalty_groups,
        "pools": pool_texts,
        "blank": sample.as_ref().map(|mark| mark.blank.clone()),
    })))
}

/// Apply the page's edits to `draft` the one way both the draft fill and the
/// live print preview use, so the preview never prints a different answer
/// than the page shows. Returns the rejected keys with their reasons.
/// `unlocked`: profile boxes the page has open, opened with Edit or left
/// blank by the profile (a radio by its own key or its item's group box);
/// their edits stay on this return.
fn apply_screen_edits(
    kind: AgentFormKind,
    draft: &mut Value,
    edits: &Map<String, Value>,
    editable: bool,
    unlocked: &BTreeSet<String>,
) -> Result<Map<String, Value>, HostError> {
    // An open radio opens its item's group box, and an open group box its radios.
    let def = kind.definition();
    let expand = |keys: &BTreeSet<String>| {
        let mut open: BTreeSet<String> = keys.clone();
        for entry in keys.iter().filter_map(|key| def.box_def(key)) {
            open.extend(entry.group.clone());
            open.extend(entry.members.iter().cloned());
        }
        open
    };
    let profile_open = expand(unlocked);
    let lift = |key: &str| {
        if profile_open.contains(key) {
            catalogs::Lift::Profile
        } else {
            catalogs::Lift::Nothing
        }
    };
    let mut rejected = Map::new();
    let mut edits: Vec<_> = edits.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    edits.sort_by_key(|(key, _)| catalogs::edit_order(kind, key));
    let shown = values_json(kind, draft).map_err(host_error)?;
    let pools = combs::pools(kind.form_code());
    // Whether each penalty group was edited before this batch: an amount in
    // one box of a batch does not open the others, and blanking the group
    // from a partly edited state clears all of it. 0605's group is not
    // computed and is open: it is always "edited".
    let computes = penalties::computes(kind);
    let penalty_groups: Vec<(penalties::Group, bool)> = penalties::groups(kind)
        .into_iter()
        .map(|group| {
            let edited = !computes || penalties::edited(&group, draft);
            (group, edited)
        })
        .collect();
    for (key, step) in plan_choice_edits(kind, draft, &shown, edits) {
        if !editable {
            rejected.insert(key.clone(), json!("this return is no longer a draft"));
            continue;
        }
        // A box the page shows as the profile's (core-locked, or the filer's
        // own name matched by key) takes an edit only once opened: the host
        // keeps the lock it reports.
        if def
            .box_def(&key)
            .is_some_and(|entry| is_profile_box(def, entry))
            && !profile_open.contains(&key)
        {
            let reason = bir_core::forms::agent::Lock::Profile.reason();
            rejected.insert(key.clone(), Value::String(format!("`{key}` {reason}")));
            continue;
        }
        // A radio written by direct draft steps keeps the locks a fill keeps:
        // the return's period and computed boxes stay as they are.
        if matches!(step, ChoiceStep::Set(_))
            && let Some(reason) = set_refusal(def, &key)
        {
            rejected.insert(key.clone(), Value::String(reason));
            continue;
        }
        let mut next = draft.clone();
        // A computed penalty takes an amount only once its group is opened
        // with Edit (sent as unlocked) or already edited; a blank clears the
        // override, so the computed amount files again (rule 8).
        if let Some((_, edited)) = penalty_groups
            .iter()
            .find(|(group, _)| group.keys.contains(&key))
        {
            if !unlocked.contains(&key) && !edited {
                rejected.insert(
                    key.clone(),
                    json!(format!("`{key}` is {}", penalties::LOCK)),
                );
                continue;
            }
            if let (ChoiceStep::Fill(value), Some(path)) = (&step, penalties::path_of(kind, &key))
                && (value.is_null() || value.as_str().is_some_and(|text| text.trim().is_empty()))
            {
                set_draft_path(&mut next, &path, Value::Null);
                *draft = next;
                continue;
            }
        }
        let result = match step {
            // A pooled address (rule 5) comes as one text on its first line.
            ChoiceStep::Fill(value) if pools.contains_key(&key) => {
                combs::write_pool(kind, &pools[&key], &mut next, &value, |line: &str| {
                    lift(line)
                })
            }
            ChoiceStep::Fill(value) => catalogs::fill(kind, &mut next, &key, &value, lift(&key)),
            ChoiceStep::Set(sets) => {
                for (path, value) in sets {
                    set_draft_path(&mut next, &path, value);
                }
                Ok(())
            }
        };
        match result {
            Ok(()) => {
                // A profile box edited here keeps the edit when the profile
                // changes later (`follow_profile`).
                if def
                    .box_def(&key)
                    .is_some_and(|entry| is_profile_box(def, entry))
                {
                    mark_profile_edit(&mut next, &key);
                }
                *draft = next;
            }
            Err(message) => {
                rejected.insert(key.clone(), Value::String(message));
            }
        }
    }
    Ok(rejected)
}

/// Why a box's direct draft steps (`ChoiceStep::Set`) are refused: it is
/// computed, or it (or its item's group box) picks the return's identity.
fn set_refusal(def: &AgentFormDef, key: &str) -> Option<String> {
    let entry = def.box_def(key)?;
    if entry.computed || entry.required == "computed" {
        return Some(format!("`{key}` is computed by BIR's own formulas"));
    }
    let lock = def
        .bindings
        .get(key)
        .or_else(|| {
            entry
                .group
                .as_deref()
                .and_then(|group| def.bindings.get(group))
        })
        .and_then(|bound| bound.lock);
    (lock == Some(agent::Lock::Identity))
        .then(|| format!("`{key}` {}", agent::Lock::Identity.reason()))
}

/// One planned write of `screen_form_fill`.
enum ChoiceStep {
    /// The ordinary box fill.
    Fill(Value),
    /// Direct draft writes (JSON pointer -> value).
    Set(Vec<(String, Value)>),
}

fn set_draft_path(draft: &mut Value, path: &str, value: Value) {
    if let Some(slot) = draft.pointer_mut(path) {
        *slot = value;
    } else if let Some((parent, leaf)) = path.rsplit_once('/')
        && let Some(Value::Object(object)) =
            draft.pointer_mut(if parent.is_empty() { "" } else { parent })
    {
        object.insert(leaf.to_string(), value);
    }
}

fn is_unticked(value: &Value) -> bool {
    match value {
        Value::Bool(b) => !b,
        Value::Null => true,
        Value::String(s) => matches!(s.trim(), "" | "false" | "0" | "N"),
        _ => false,
    }
}

/// Paths a choice box's `"true"` option writes.
fn true_option_paths(def: &AgentFormDef, key: &str) -> Vec<String> {
    match def.bindings.get(key).map(|bound| &bound.binding) {
        Some(Binding::Choice { options }) => options
            .iter()
            .filter(|option| option.value == "true")
            .flat_map(|option| option.set.keys().cloned())
            .collect(),
        _ => Vec::new(),
    }
}

/// 1701 Item 6 and Part IV Item 3: the filer type and its "also a
/// compensation earner" flag. Compensation Earner alone is its own filer
/// type (ATC II011); beside Single Proprietor or Professional it marks mixed
/// income, as the official form's TPFilerType does.
fn form_1701_type_paths(kind: AgentFormKind, key: &str) -> Option<(&'static str, &'static str)> {
    if kind.form_code() != "1701" {
        return None;
    }
    let tail = key.rsplit(':').next().unwrap_or(key);
    if tail.starts_with("rdoPg1I6TaxpayerType") {
        Some(("/taxpayer_type", "/taxpayer_also_compensation_earner"))
    } else if tail.starts_with("rdoPg2I3SpouseType") {
        Some(("/spouse/filer_type", "/spouse_also_compensation_earner"))
    } else {
        None
    }
}

/// Turn the page's checkbox edits into writes that keep a radio group to one
/// answer. The probe binds each member's `"false"` to some other answer
/// (unticking Single Proprietor wrote Professional), and a batch that ticks
/// one member and unticks the rest could end on an unticked member's
/// "other" answer. So:
/// - a group with a ticked member in the batch ignores its unticked members;
/// - unticking a member that is not ticked does nothing;
/// - unticking the ticked member of a group of three or more clears the
///   group (a yes/no pair keeps its official flip).
///
/// Edits keep their dependency order otherwise.
fn plan_choice_edits(
    kind: AgentFormKind,
    draft: &Value,
    shown: &Value,
    edits: Vec<(String, Value)>,
) -> Vec<(String, ChoiceStep)> {
    let def = kind.definition();
    // Radio boxes without a group box (1701 Item 7's ATCs) form one group
    // per draft field their tick writes.
    let implicit_path = |key: &str| -> Option<String> {
        let entry = def.box_def(key)?;
        if entry.group.is_some() || !entry.kind.contains("radio") {
            return None;
        }
        let paths = true_option_paths(def, key);
        (paths.len() == 1).then(|| paths[0].clone())
    };
    let mut implicit: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in &def.boxes {
        if let Some(path) = implicit_path(&entry.key) {
            implicit.entry(path).or_default().push(entry.key.clone());
        }
    }
    // (group id, member count) of a one-answer box.
    let group_info = |key: &str| -> Option<(String, usize)> {
        if let Some(group) = def
            .box_def(key)
            .and_then(|e| e.group.as_deref())
            .and_then(|g| def.box_def(g))
            .filter(|g| g.radio_group)
        {
            return Some((group.key.clone(), group.members.len()));
        }
        let path = implicit_path(key)?;
        let members = implicit.get(&path).map_or(0, Vec::len);
        (members >= 2).then(|| (format!("path:{path}"), members))
    };
    let group_of = |key: &str| -> Option<&BoxDef> {
        let group = def.box_def(key)?.group.as_deref()?;
        def.box_def(group).filter(|entry| entry.radio_group)
    };
    let ticked_groups: BTreeSet<String> = edits
        .iter()
        .filter(|(_, value)| !is_unticked(value))
        .filter_map(|(key, _)| group_info(key).map(|g| g.0))
        .collect();
    let mut plain = Vec::new();
    for (key, value) in edits {
        let unticked = is_unticked(&value);
        if let Some((type_path, flag_path)) = form_1701_type_paths(kind, &key) {
            let current = draft
                .pointer(type_path)
                .and_then(Value::as_str)
                .unwrap_or("");
            let mixed = draft
                .pointer(flag_path)
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let business = matches!(current, "SingleProprietor" | "Professional");
            if key.ends_with('C') && group_of(&key).is_none() {
                let sets = match (unticked, business) {
                    (false, true) => vec![(flag_path.to_string(), json!(true))],
                    (false, false) => vec![
                        (type_path.to_string(), json!("CompensationEarner")),
                        (flag_path.to_string(), json!(false)),
                    ],
                    (true, _) if current == "CompensationEarner" => vec![
                        (type_path.to_string(), Value::Null),
                        (flag_path.to_string(), json!(false)),
                    ],
                    (true, _) => vec![(flag_path.to_string(), json!(false))],
                };
                plain.push((key, ChoiceStep::Set(sets)));
                continue;
            }
            if group_of(&key).is_some() {
                let shown_on = shown.get(&key).and_then(Value::as_str) == Some("true");
                if unticked {
                    if ticked_groups.contains(&group_of(&key).unwrap().key) || !shown_on {
                        continue;
                    }
                    // Unticking the business type of a mixed-income filer
                    // leaves Compensation Earner ticked.
                    let rest = if mixed && business {
                        json!("CompensationEarner")
                    } else {
                        Value::Null
                    };
                    plain.push((
                        key,
                        ChoiceStep::Set(vec![
                            (type_path.to_string(), rest),
                            (flag_path.to_string(), json!(false)),
                        ]),
                    ));
                } else {
                    let pure_comp = current == "CompensationEarner";
                    let mut sets: Vec<(String, Value)> = Vec::new();
                    let option = match def.bindings.get(&key).map(|b| &b.binding) {
                        Some(Binding::Choice { options }) => {
                            options.iter().find(|o| o.value == "true")
                        }
                        _ => None,
                    };
                    let picked = option
                        .and_then(|o| o.set.get(type_path))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let picked_business =
                        matches!(picked.as_str(), "SingleProprietor" | "Professional");
                    sets.push((type_path.to_string(), json!(picked)));
                    sets.push((
                        flag_path.to_string(),
                        json!(picked_business && (mixed || pure_comp)),
                    ));
                    plain.push((key, ChoiceStep::Set(sets)));
                }
                continue;
            }
        }
        match group_info(&key) {
            Some((group, members)) if unticked => {
                if ticked_groups.contains(&group)
                    || shown.get(&key).and_then(Value::as_str) != Some("true")
                {
                    continue;
                }
                if members >= 3 {
                    let sets = true_option_paths(def, &key)
                        .into_iter()
                        .map(|path| (path, Value::Null))
                        .collect();
                    plain.push((key, ChoiceStep::Set(sets)));
                } else {
                    plain.push((key, ChoiceStep::Fill(value)));
                }
            }
            Some(_) => plain.push((key, ChoiceStep::Fill(value))),
            None => plain.push((key, ChoiceStep::Fill(value))),
        }
    }
    plain
}

fn screen_form_validate(
    arguments: &HostArguments,
) -> Result<impl Future<Output = HostResult> + Send + use<>, HostError> {
    let kind = kind_of(arguments.string(0)?)?;
    let mut draft = draft_argument(arguments, 1)?;
    let sample = take_sample_mark(&mut draft);
    let amendment = take_amendment_mark(&mut draft);
    Ok(async move {
        let checked = on_large_stack(move || -> Result<Value, String> {
            let mut draft = draft;
            let (mut values, mut errors, status) = evaluate_here(kind, &mut draft)?;
            if let Some(mark) = &sample {
                blank_sample(&mark.blank, &mut values, &mut errors);
            }
            let options = catalogs::options(kind, &draft);
            let off = if status == FilingStatus::Draft {
                conditions::off_boxes(kind, &draft, &values)
            } else {
                Map::new()
            };
            let blank = sample.as_ref().map(|mark| mark.blank.clone());
            let penalty_groups = penalties::json(kind, &draft);
            let pool_texts = combs::pool_texts(kind, &draft);
            put_sample_mark(&mut draft, sample.as_ref());
            put_amendment_mark(&mut draft, amendment);
            Ok(json!({
                "draft": draft,
                "values": values,
                "options": options,
                "errors": errors,
                "off": off,
                "penalties": penalty_groups,
                "pools": pool_texts,
                "blank": blank,
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
    if let Some(mark) = take_sample_mark(&mut draft) {
        return save_sample(kind, draft, &mark);
    }
    let amendment = take_amendment_mark(&mut draft);
    let id = with_database(|db| match &amendment {
        Some(mark) => save_amendment(db, kind, &mut draft, mark).map_err(host_error),
        None => {
            refuse_replacing_a_stored_return(db, kind, &draft).map_err(host_error)?;
            agent::save_draft(kind, db, &mut draft).map_err(host_error)
        }
    })?;
    let status = agent::identity(kind, &draft).map_err(host_error)?.status;
    Ok(json(json!({
        "id": id,
        "status": status_name(&status),
        "stored_period": stored_period(kind, &draft),
        "draft": draft,
    })))
}

/// Save a Form library sample (decided 2026-10-17): once its period is set
/// it becomes that period's return, with everything typed into it. The
/// matching rules come first: when the period already has a stored return
/// nothing is saved and the answer is `{saved: false, existing, opens}`, the
/// offer the page shows; a sample never replaces a stored return (the save
/// guard stays). An entry-numbered form, which never matches, takes the
/// year's next free entry. Saved: `{saved: true, id, status,
/// stored_period, draft}`, the draft no longer a sample.
fn save_sample(kind: AgentFormKind, mut draft: Value, mark: &SampleMark) -> HostResult {
    let code = kind.form_code();
    if !mark.set {
        return Err(host_error(format!(
            "set this {code} sample's period first: Save makes it the return for that period"
        )));
    }
    with_database(|db| {
        if NUMBERED_FORMS.contains(&code) {
            next_free_entry(db, kind, &mut draft).map_err(host_error)?;
        } else if let Some(stored) = stored_for_period(db, kind, &draft).map_err(host_error)? {
            return Ok(json(json!({
                "saved": false,
                "existing": existing_return(db, kind, &stored).map_err(host_error)?,
                "opens": opens_for(kind, &stored).map_err(host_error)?,
            })));
        }
        refuse_replacing_a_stored_return(db, kind, &draft).map_err(host_error)?;
        let id = agent::save_draft(kind, db, &mut draft).map_err(host_error)?;
        let status = agent::identity(kind, &draft).map_err(host_error)?.status;
        Ok(json(json!({
            "saved": true,
            "id": id,
            "status": status_name(&status),
            "stored_period": stored_period(kind, &draft),
            "opens": opens_for(kind, &draft).map_err(host_error)?,
            "draft": draft,
        })))
    })
}

/// What `screen_form_open` takes to open exactly this stored return: its
/// year, the page period, and its stored key.
fn opens_for(kind: AgentFormKind, stored: &Value) -> Result<Value, String> {
    let identity = agent::identity(kind, stored)?;
    let period = match identity.period {
        FilingPeriod::Monthly(month) => u32::from(month),
        FilingPeriod::Quarterly(quarter) => u32::from(quarter),
        FilingPeriod::Annual => 0,
        FilingPeriod::OpenEnded(key) if key >= 101 => key / 100,
        FilingPeriod::OpenEnded(key) => key,
    };
    Ok(json!({
        "year": identity.taxable_year,
        "period": period,
        "key": stored_period(kind, stored),
    }))
}

/// An entry-numbered form's sample (0605, 1606, 1706, 1707, 1707A, 1800,
/// 1801) takes the first entry of its year with no stored return.
fn next_free_entry(db: &Database, kind: AgentFormKind, draft: &mut Value) -> Result<(), String> {
    let code = kind.form_code();
    let pointer = if draft.pointer("/open_ended_key").is_some() {
        "/open_ended_key"
    } else {
        // 0605 keeps its entry number in `month`.
        "/month"
    };
    for entry in 1..=999u32 {
        let mut candidate = draft.clone();
        set_draft_path(&mut candidate, pointer, json!(entry));
        let identity = agent::identity(kind, &candidate)?;
        if identity.period != FilingPeriod::OpenEnded(entry) {
            return Err(format!("{code}: no entry number to save the sample under"));
        }
        if stored_return(db, kind, &candidate)?.is_none() {
            *draft = candidate;
            return Ok(());
        }
    }
    Err(format!("{code}: every entry number of the year is taken"))
}

/// When the draft was first made (`created_at`, top level or in its
/// lifecycle).
fn created_at(draft: &Value) -> Option<&str> {
    draft["created_at"]
        .as_str()
        .or_else(|| draft["lifecycle"]["created_at"].as_str())
}

/// The stored return for a draft's TIN, form, year and period, if any.
fn stored_return(
    db: &Database,
    kind: AgentFormKind,
    draft: &Value,
) -> Result<Option<Value>, String> {
    use bir_core::forms::*;
    let identity = agent::identity(kind, draft)?;
    let (tin, year) = (identity.tin.as_str(), identity.taxable_year);
    let column = bir_core::forms::queueable::period_column(&identity.period);
    let slot = u8::try_from(column).unwrap_or(0);
    fn to_value<T: serde::Serialize>(draft: Option<T>) -> Result<Option<Value>, String> {
        draft
            .map(|draft| serde_json::to_value(draft).map_err(|error| error.to_string()))
            .transpose()
    }
    match kind {
        AgentFormKind::Form1601C => to_value(
            db.get_1601c_draft(tin, year, slot)
                .map_err(|error| read_error("1601C", error))?,
        ),
        AgentFormKind::Form2551Q => to_value(
            db.get_2551q_draft(tin, year, slot)
                .map_err(|error| read_error("2551Q", error))?,
        ),
        AgentFormKind::Queueable(QueueableKind::Form2550Q) => to_value(
            db.get_form_draft::<form_2550q::Form2550QDraft>(tin, "2550Q", year, Some(slot))
                .map_err(|error| read_error("2550Q", error))?,
        ),
        AgentFormKind::Queueable(queueable) => bir_core::with_queueable_kind!(queueable, F => {
            let found = match stored::<F>(Some(db), tin, year, column)? {
                Some(found) => Some(found),
                None => db
                    .get_form_draft_v2::<F>(tin, F::FORM_CODE, year, &identity.period)
                    .map_err(|error| read_error(F::FORM_CODE, error))?,
            };
            to_value(found)
        }),
    }
}

/// The stored return a sample's completed period matches: the return of the
/// same period (monthly: month and year; quarterly: quarter and year;
/// annual: year; per transaction: its date). How often the form is filed
/// is the screen index's (`freq`): a monthly form whose rows are keyed by
/// date (`MMDD`: 2200AN, 2200C, 2200S) matches any day of the month, the
/// day does not matter; an "As needed" form keyed by its transaction date
/// (2000OT, 2552) matches only that exact date, so a second transaction in
/// the same month is a return of its own.
fn stored_for_period(
    db: &Database,
    kind: AgentFormKind,
    draft: &Value,
) -> Result<Option<Value>, String> {
    let identity = agent::identity(kind, draft)?;
    let monthly =
        schema_entry(kind.form_code()).and_then(|form| form["freq"].as_str()) == Some("Monthly");
    match (kind, identity.period) {
        (AgentFormKind::Queueable(queueable), FilingPeriod::OpenEnded(key))
            if monthly && key >= 101 =>
        {
            let month = i64::from(key / 100);
            bir_core::with_queueable_kind!(queueable, F => {
                let found = latest::<F>(
                    Some(db),
                    &identity.tin,
                    identity.taxable_year,
                    mmdd_keys(month..=month, 0..=31),
                    |_| true,
                )?;
                found
                    .map(|found| serde_json::to_value(found).map_err(|error| error.to_string()))
                    .transpose()
            })
        }
        _ => stored_return(db, kind, draft),
    }
}

/// What a save may write. A draft saves over the stored return of its TIN,
/// form, year and period only when that row is still a Draft and is this
/// very draft: the same row id; for 2550Q, whose rows keep no id, the same
/// `created_at`. A draft with an id saves only to that row. A filed or
/// queued return never changes here (an amendment does, through its own
/// save).
fn refuse_replacing_a_stored_return(
    db: &Database,
    kind: AgentFormKind,
    draft: &Value,
) -> Result<(), String> {
    let code = kind.form_code();
    let id = draft["id"].as_i64();
    let Some(stored) = stored_return(db, kind, draft)? else {
        return match id {
            Some(id) => Err(format!(
                "this {code} draft names row {id}, which is not stored for its period; open the return again"
            )),
            None => Ok(()),
        };
    };
    let status = agent::identity(kind, &stored)?.status;
    if status != FilingStatus::Draft {
        return Err(format!(
            "the {code} return for this period is {}; it can no longer change here (amend it instead)",
            status_name(&status)
        ));
    }
    let same_2550q = kind == AgentFormKind::Queueable(QueueableKind::Form2550Q)
        && created_at(&stored).is_some()
        && created_at(&stored) == created_at(draft);
    match (id, stored["id"].as_i64()) {
        (Some(id), Some(row)) if id == row => Ok(()),
        (Some(id), Some(row)) => Err(format!(
            "this {code} draft names row {id}, but its period is stored as row {row}; open the return again"
        )),
        (_, None) if same_2550q => Ok(()),
        _ => Err(format!(
            "a {code} return for this period is already stored; open it from the taxpayer's returns to change it"
        )),
    }
}

fn filing_status_named(name: &str) -> Option<FilingStatus> {
    Some(match name {
        "Draft" => FilingStatus::Draft,
        "Queued" => FilingStatus::Queued,
        "Submitted" => FilingStatus::Submitted,
        "Confirmed" => FilingStatus::Confirmed,
        "Paid" => FilingStatus::Paid,
        _ => return None,
    })
}

/// An amendment's first save: in one transaction, the user's revert of the
/// filed return (kept byte for byte as its amendment snapshot) and this
/// draft written in its row. Refused, with nothing changed, if the filed row
/// is no longer the one the amendment was copied from (the mark's status and
/// `updated_at`).
fn save_amendment(
    db: &Database,
    kind: AgentFormKind,
    draft: &mut Value,
    mark: &Value,
) -> Result<i64, String> {
    use bir_core::db::AmendmentBase;
    let base = AmendmentBase {
        status: mark["status"]
            .as_str()
            .and_then(filing_status_named)
            .ok_or("this amendment does not say which filed return it amends; amend it again")?,
        updated_at: mark["updated_at"].as_str().unwrap_or("").to_string(),
    };
    agent::compute(kind, draft)?;
    let error = |error: bir_core::db::DbError| error.to_string();
    let id = match kind {
        AgentFormKind::Form1601C => db
            .save_1601c_amendment(
                &serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?,
                &base,
            )
            .map_err(error)?,
        AgentFormKind::Form2551Q => db
            .save_2551q_amendment(
                &serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?,
                &base,
            )
            .map_err(error)?,
        AgentFormKind::Queueable(queueable) => bir_core::with_queueable_kind!(queueable, F => {
            let typed: F = serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?;
            db.save_queueable_amendment(&typed, &base).map_err(error)?
        }),
    };
    draft["id"] = Value::from(id);
    Ok(id)
}

/// Marks a new amended return (`screen_form_amend`) until its first save:
/// the filed return it amends, as it was (`status`, `updated_at`).
const AMENDMENT_MARK: &str = "screen_form_amendment";

fn take_amendment_mark(draft: &mut Value) -> Option<Value> {
    match draft {
        Value::Object(object) => object.remove(AMENDMENT_MARK).filter(|mark| !mark.is_null()),
        _ => None,
    }
}

fn put_amendment_mark(draft: &mut Value, mark: Option<Value>) {
    if let (Some(mark), Value::Object(object)) = (mark, draft) {
        object.insert(AMENDMENT_MARK.into(), mark);
    }
}

/// Open a NEW amended return for a stored return that reached BIR, copied
/// from it (`agent::amended_copy`). Nothing is stored and the filed return
/// is not touched until the amendment is first saved.
fn screen_form_amend(
    arguments: &HostArguments,
) -> Result<impl Future<Output = HostResult> + Send + use<>, HostError> {
    let tin: String = arguments
        .string(0)?
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let kind = kind_of(arguments.string(1)?)?;
    let code = kind.form_code();
    let year =
        u16::try_from(arguments.integer(2)?).map_err(|_| host_error("year is out of range"))?;
    let period = page_period(code, arguments.integer(3)?).map_err(host_error)?;
    let exact = arguments.get(4).and_then(|key| to_json(key).as_i64());
    let (amended, from_profile, status, updated_at) = with_database(|db| {
        let profile = db
            .get_profile_by_tin(&tin)
            .map_err(|error| host_error(error.to_string()))?
            .ok_or_else(|| host_error(format!("no taxpayer profile with TIN {tin}")))?;
        let filed =
            load_draft(db, kind, &profile, year, period, false, exact).map_err(host_error)?;
        let status = agent::identity(kind, &filed).map_err(host_error)?.status;
        let updated_at = filed["updated_at"]
            .as_str()
            .or_else(|| filed["lifecycle"]["updated_at"].as_str())
            .unwrap_or("")
            .to_string();
        let amended = agent::amended_copy(kind, &filed).map_err(host_error)?;
        let from_profile =
            load_draft(db, kind, &profile, year, period, true, None).map_err(host_error)?;
        Ok((amended, from_profile, status, updated_at))
    })?;
    Ok(async move {
        let mut opened =
            on_large_stack(move || opened_json(kind, year, period, amended, from_profile, None))
                .and_then(|result| result)
                .map_err(host_error)?;
        put_amendment_mark(
            &mut opened["draft"],
            Some(json!({ "status": status_name(&status), "updated_at": updated_at })),
        );
        opened["amends"] = json!(status_name(&status));
        Ok(json(opened))
    })
}

/// The filed version an amended return keeps in its history (its newest
/// amendment snapshot): its id, status and when it was filed. Null for a
/// return that was never amended.
fn filed_original(db: &Database, kind: AgentFormKind, draft: &Value) -> Result<Value, String> {
    let identity = agent::identity(kind, draft)?;
    let snapshots = db
        .list_amendment_snapshots(
            kind.form_code(),
            &identity.tin,
            identity.taxable_year,
            &identity.period.to_period_key(),
        )
        .map_err(|error| error.to_string())?;
    Ok(match snapshots.last() {
        Some(snapshot) => json!({
            "id": snapshot.id,
            "status": snapshot.status_at_revert,
            "filed_on": snapshot.confirmed_at.clone().or_else(|| snapshot.submitted_at.clone()),
        }),
        None => Value::Null,
    })
}

/// "View the filed original": an amended return's filed version, read-only,
/// from its history (the amendment snapshot, exactly as it was stored).
fn screen_form_filed_original(
    arguments: &HostArguments,
) -> Result<impl Future<Output = HostResult> + Send + use<>, HostError> {
    let kind = kind_of(arguments.string(0)?)?;
    let id = arguments.integer(1)?;
    let (draft, snapshot) = with_database(|db| {
        let snapshot = db
            .get_amendment_snapshot(id)
            .map_err(|error| host_error(error.to_string()))?
            .filter(|snapshot| snapshot.form_code == kind.form_code())
            .ok_or_else(|| host_error(format!("no filed {} original #{id}", kind.form_code())))?;
        let draft: Value = serde_json::from_str(&snapshot.draft_json)
            .map_err(|error| host_error(error.to_string()))?;
        Ok((draft, snapshot))
    })?;
    let identity = agent::identity(kind, &draft).map_err(host_error)?;
    let year = identity.taxable_year;
    let period =
        u8::try_from(bir_core::forms::queueable::period_column(&identity.period)).unwrap_or(0);
    Ok(async move {
        let from_profile = draft.clone();
        let mut opened =
            on_large_stack(move || opened_json(kind, year, period, draft, from_profile, None))
                .and_then(|result| result)
                .map_err(host_error)?;
        // Never a draft: what was filed, kept as it was.
        opened["editable"] = Value::Bool(false);
        opened["filed_original_view"] = json!({
            "status": snapshot.status_at_revert,
            "filed_on": snapshot.confirmed_at.or(snapshot.submitted_at),
        });
        Ok(json(opened))
    })
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
    let mut draft = draft_argument(arguments, 1)?;
    refuse_sample(&mut draft, "printed")?;
    take_amendment_mark(&mut draft);
    let slug = crate::bir_module::print_slug_for(kind.form_code())
        .ok_or_else(|| host_error(format!("Form {code} has no print sheet")))?;
    let identity = agent::identity(kind, &draft).map_err(host_error)?;
    let year = identity.taxable_year;
    with_database(|db| {
        let (fields, receipt) = print_map(db, kind, &draft)?;
        let profile = db
            .get_profile_by_tin(&identity.tin)
            .ok()
            .flatten()
            .map(|p| p.full_name.clone());
        crate::bir_module::open_print_preview(
            slug,
            kind.form_code(),
            year,
            &fields,
            receipt.as_ref(),
            profile,
        )
    })
}

/// Live sync form -> preview: apply the page's staged edits to a copy of the
/// draft, fill the frozen sheet in memory, staticize it, and hand it to the
/// open preview window for this form (which re-renders only changed sheets).
fn screen_form_preview_sync(arguments: &HostArguments) -> HostResult {
    let started = std::time::Instant::now();
    let code = arguments.string(0)?.to_owned();
    let kind = kind_of(&code)?;
    if !crate::print_preview_window::has_live(kind.form_code()) {
        return Ok(HostValue::Bool(false));
    }
    let mut draft = draft_argument(arguments, 1)?;
    // A sample has no print preview to follow.
    if take_sample_mark(&mut draft).is_some() {
        return Ok(HostValue::Bool(false));
    }
    take_amendment_mark(&mut draft);
    if let Some(edits) = arguments.value(2).ok().and_then(|v| v.as_object()) {
        let edits: Map<String, Value> =
            edits.iter().map(|(k, v)| (k.clone(), to_json(v))).collect();
        if !edits.is_empty() {
            let unlocked = unlocked_argument(arguments, 3);
            let _ = apply_screen_edits(kind, &mut draft, &edits, true, &unlocked);
            let _ = agent::compute(kind, &mut draft);
        }
    }
    let slug = crate::bir_module::print_slug_for(kind.form_code())
        .ok_or_else(|| host_error(format!("Form {code} has no print sheet")))?;
    with_database(|db| {
        let (fields, receipt) = print_map(db, kind, &draft)?;
        let slug = slug.to_string();
        // Template fill + prepare + staticize run off the UI thread.
        let produce = Box::new(move || {
            let html = bir_print::frozen_html::filled_document_with_receipt(
                &slug,
                &fields,
                receipt.as_ref(),
            )
            .unwrap_or_default();
            crate::static_html::staticize(&crate::print_sheet::prepare_html_for(&slug, &html, true))
        });
        crate::print_preview_window::live_update(kind.form_code(), produce, started);
        Ok(HostValue::Bool(true))
    })
}

/// Queue the saved draft the page holds for background submission. 1601C and
/// 2551Q use their own queue path (`queue_form_draft`); every other form is
/// queued from the draft itself, after it validates.
fn screen_form_queue(arguments: &HostArguments) -> HostResult {
    let code = arguments.string(0)?.to_owned();
    let kind = kind_of(&code)?;
    let mut draft = draft_argument(arguments, 1)?;
    refuse_sample(&mut draft, "queued")?;
    take_amendment_mark(&mut draft);
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
    let mut draft = draft_argument(arguments, 1)?;
    refuse_sample(&mut draft, "taken out of the queue")?;
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
            "birth_date": "1980-02-03",
            "business_start_date": "2015-01-02",
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
            // The return as it now stands (saved after the first choice):
            // only that very return saves over its row.
            let current = open("2000", 1);
            let filled = fill(
                "2000",
                &current["draft"],
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
    /// 1700 shows the January 2018 page the model uploads; Schedule 1's
    /// employer boxes and the spouse's TIN are written by the host
    /// (rows_1700), including the "(add more...)" rows once Item 4 folds.
    #[test]
    fn dummy_tin_requires_the_explicit_development_override() {
        const PROBE: &str = "BIR_TIN_OVERRIDE_PROBE";
        if let Ok(mode) = std::env::var(PROBE) {
            setup();
            let opened = open("1700", 0);
            let mut draft = opened["draft"].clone();
            draft["tin"] = json!("00000000000000");
            let checked =
                call_async("screen_form_validate", vec!["1700".into(), json(draft)]).unwrap();
            let invalid_tin = checked["errors"].as_array().unwrap().iter().any(|e| {
                e["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("incorrect TIN"))
            });
            assert_eq!(invalid_tin, mode == "strict" || !cfg!(debug_assertions));
            return;
        }
        // Environment changes are isolated in child processes, so parallel
        // form tests keep their normal validation behavior.
        for mode in ["strict", "demo"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "screen_form_module::tests::dummy_tin_requires_the_explicit_development_override", "--nocapture"])
                .env(PROBE, mode)
                .env("EBIR_RELAXED_VALIDATION", if mode == "demo" { "1" } else { "0" })
                .status().unwrap();
            assert!(status.success(), "{mode} TIN validation probe failed");
        }
    }

    #[test]
    fn form_1700_shows_the_2018_page_and_writes_its_employers() {
        setup();
        let keys = schema_keys("1700");
        assert!(keys.contains("frm1700:txtPg1I1Year"));
        assert!(keys.contains("frm1700:txtPg2I4_12PartVIEmployerName1"));
        assert!(!keys.contains("frm1700:txtYear"), "2013 controls are gone");

        let opened = open("1700", 0);
        let tin = |row: &str| {
            json!({
                format!("frm1700:txtPg2I{row}PartVIEmployerTIN1"): "123",
                format!("frm1700:txtPg2I{row}PartVIEmployerTIN2"): "456",
                format!("frm1700:txtPg2I{row}PartVIEmployerTIN3"): "789",
                format!("frm1700:txtPg2I{row}PartVIEmployerBranchCode"): "00000",
            })
        };
        let mut edits = json!({
            "frm1700:rdoPg1I6TaxpayerTypeE": "true",
            "frm1700:txtPg2I1PartVIEmployerName1": "First Employer",
            "frm1700:txtPg2ISched1c_1REG": "500000",
            "frm1700:txtPg2ISched1e_1TW": "20000",
        });
        edits
            .as_object_mut()
            .unwrap()
            .extend(tin("1").as_object().unwrap().clone());
        let filled = fill("1700", &opened["draft"], edits);
        assert_eq!(filled["rejected"], json!({}));
        assert_eq!(filled["draft"]["employers"][0]["tin"], "12345678900000");
        let values = &filled["values"];
        assert_eq!(
            values["frm1700:txtPg2I1PartVIEmployerName1"],
            "FIRST EMPLOYER"
        );
        assert_eq!(values["frm1700:txtPg2I1PartVIEmployerTIN2"], "456");
        assert_eq!(amount(values, "frm1700:txtPg2I42A"), 500_000.0);

        // The spouse's TIN waits for a joint return.
        let refused = fill(
            "1700",
            &filled["draft"],
            json!({"frm1700:txtPg1I18STIN1": "111"}),
        );
        assert!(refused["rejected"].get("frm1700:txtPg1I18STIN1").is_some());

        // A popup row past the next one is refused; filling 4_2 folds Item 4.
        let early = fill(
            "1700",
            &filled["draft"],
            json!({"frm1700:txtPg2I4_2PartVIEmployerName1": "Too Early"}),
        );
        assert!(
            early["rejected"]
                .get("frm1700:txtPg2I4_2PartVIEmployerName1")
                .is_some()
        );
        let four = fill(
            "1700",
            &filled["draft"],
            json!({
                "frm1700:txtPg2I2PartVIEmployerName1": "Second",
                "frm1700:txtPg2I3PartVIEmployerName1": "Third",
                "frm1700:txtPg2I4PartVIEmployerName1": "Fourth",
                "frm1700:txtPg2ISched1c_4REG": "1000",
            }),
        );
        assert_eq!(four["rejected"], json!({}));
        let folded = fill(
            "1700",
            &four["draft"],
            json!({
                "frm1700:txtPg2I4_2PartVIEmployerName1": "Fifth",
                "frm1700:txtPg2ISched1c_4_2REG": "2000",
            }),
        );
        assert_eq!(folded["rejected"], json!({}));
        let values = &folded["values"];
        assert_eq!(values["frm1700:txtPg2I4PartVIEmployerName1"], "OTHERS");
        assert_eq!(values["frm1700:txtPg2I4_1PartVIEmployerName1"], "FOURTH");
        assert_eq!(values["frm1700:txtPg2I4_2PartVIEmployerName1"], "FIFTH");
        assert_eq!(amount(values, "frm1700:txtPg2Pt6I4SubtotalC"), 3_000.0);
        // Item 4 now reads OTHERS: its employer is changed in row 4_1.
        let item4 = fill(
            "1700",
            &folded["draft"],
            json!({"frm1700:txtPg2I4PartVIEmployerName1": "Renamed"}),
        );
        assert!(
            item4["rejected"]
                .get("frm1700:txtPg2I4PartVIEmployerName1")
                .is_some()
        );
        // Clearing row 4_2 deletes it and unfolds the schedule.
        let cleared = fill(
            "1700",
            &folded["draft"],
            json!({
                "frm1700:txtPg2I4_2PartVIEmployerName1": "",
                "frm1700:txtPg2ISched1c_4_2REG": "",
            }),
        );
        assert_eq!(cleared["draft"]["employers"].as_array().unwrap().len(), 4);
        assert_eq!(
            cleared["values"]["frm1700:txtPg2I4PartVIEmployerName1"],
            "FOURTH"
        );
    }

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
    fn taxpayer_name_boxes_come_from_the_profile() {
        setup();
        for (code, key) in [
            ("1700", "frm1700:txtPg1I7TaxpayerName"),
            ("1700", "frm1700:txtPg2TaxpayerName"),
            ("1701A", "frm1701A:txtTaxpayerName"),
            ("1702Q", "frm1702q:txtTaxpayerName1"),
        ] {
            let opened = open(code, if code == "1702Q" { 1 } else { 0 });
            assert!(opened["locks"].get(key).is_some(), "{key} is read-only");
            assert!(
                opened["profile"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|k| k == key),
                "{key} is marked as from the profile"
            );
        }
        // The spouse's name is typed by the filer.
        let opened = open("1700", 0);
        assert!(opened["locks"].get("frm1700:txtPg1I21SpouseName").is_none());
    }

    #[test]
    fn form_1701_item6_keeps_one_filer_type_and_maps_each_box() {
        setup();
        let opened = open("1701", 0);
        let k = |c: &str| format!("frm1701:rdoPg1I6TaxpayerType{c}");
        let ticked = |f: &Value| -> Vec<&str> {
            ["S", "P", "E", "T", "C"]
                .into_iter()
                .filter(|c| f["values"][k(c)] == "true")
                .collect()
        };
        let item6_error = |f: &Value| {
            let v = call_async(
                "screen_form_validate",
                vec!["1701".into(), json(f["draft"].clone())],
            )
            .unwrap();
            v["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["message"].as_str().is_some_and(|m| m.contains("Item 6.")))
        };
        // Compensation Earner alone is selectable and is a filer type.
        let c = fill("1701", &opened["draft"], json!({ k("C"): "true" }));
        assert_eq!(ticked(&c), ["C"]);
        assert!(!item6_error(&c), "C alone answers Item 6");
        // Each business type ticks its own box only; a pill-style batch too.
        for pick in ["S", "P", "E", "T"] {
            let mut batch = Map::new();
            for other in ["S", "P", "E", "T"] {
                batch.insert(
                    k(other),
                    json!(if other == pick { "true" } else { "false" }),
                );
            }
            let f = fill("1701", &opened["draft"], Value::Object(batch));
            assert_eq!(ticked(&f), [pick], "batch picking {pick}");
            assert!(!item6_error(&f));
            let single = fill("1701", &opened["draft"], json!({ k(pick): "true" }));
            assert_eq!(ticked(&single), [pick], "single tick {pick}");
        }
        // Switching keeps one; unticking clears instead of jumping.
        let s = fill("1701", &opened["draft"], json!({ k("S"): "true" }));
        let p = fill("1701", &s["draft"], json!({ k("P"): "true" }));
        assert_eq!(ticked(&p), ["P"]);
        let none = fill("1701", &p["draft"], json!({ k("P"): "false" }));
        assert_eq!(ticked(&none), Vec::<&str>::new(), "untick clears Item 6");
        let noop = fill("1701", &p["draft"], json!({ k("S"): "false" }));
        assert_eq!(ticked(&noop), ["P"], "unticking an unticked box is a no-op");
        // Mixed income: C beside S or P, in either order.
        let mixed = fill("1701", &p["draft"], json!({ k("C"): "true" }));
        assert_eq!(ticked(&mixed), ["P", "C"]);
        let mixed2 = fill("1701", &c["draft"], json!({ k("S"): "true" }));
        assert_eq!(ticked(&mixed2), ["S", "C"]);
        let back = fill("1701", &mixed2["draft"], json!({ k("S"): "false" }));
        assert_eq!(ticked(&back), ["C"]);
        let estate = fill("1701", &mixed["draft"], json!({ k("E"): "true" }));
        assert_eq!(ticked(&estate), ["E"], "Estate is not mixed income");
        // A three-way radio elsewhere (1700 civil status) also stays single.
        let o = open("1700", 0);
        let cs = |c: &str| format!("frm1700:rdoPg1I15CivilStatus{c}");
        let mut batch = Map::new();
        for c in ["S", "M", "LS", "W"] {
            batch.insert(cs(c), json!(if c == "W" { "true" } else { "false" }));
        }
        let f = fill("1700", &o["draft"], Value::Object(batch));
        let on: Vec<_> = ["S", "M", "LS", "W"]
            .into_iter()
            .filter(|c| f["values"][cs(c)] == "true")
            .collect();
        assert_eq!(on, ["W"]);
    }

    /// The page's row, the draft and the live preview's printed boxes agree
    /// after every Item 6 click, whether the preview gets the edit staged
    /// (before the flush) or the flushed draft.
    #[test]
    fn form_1701_item6_page_draft_and_preview_agree() {
        setup();
        let kind = kind_of("1701").unwrap();
        let opened = open("1701", 0);
        let k = |c: &str| format!("frm1701:rdoPg1I6TaxpayerType{c}");
        let codes = ["S", "P", "E", "T", "C"];
        let printed = |draft: &Value| -> Vec<&str> {
            let map = with_database(|db| print_map(db, kind, draft)).unwrap().0;
            codes
                .into_iter()
                .filter(|c| {
                    let tail = format!("rdoPg1I6TaxpayerType{c}");
                    map.iter().any(|(key, v)| {
                        key.rsplit(':').next() == Some(tail.as_str()) && v == "true"
                    })
                })
                .collect()
        };
        let steps: Vec<(&str, &str, Vec<&str>)> = vec![
            ("S", "true", vec!["S"]),
            ("P", "true", vec!["P"]),
            ("E", "true", vec!["E"]),
            ("T", "true", vec!["T"]),
            ("C", "true", vec!["C"]),
            ("C", "false", vec![]),
            ("P", "true", vec!["P"]),
            ("P", "false", vec![]),
            ("S", "true", vec!["S"]),
            ("P", "true", vec!["P"]),
            ("C", "true", vec!["P", "C"]),
            ("P", "false", vec!["C"]),
            ("C", "false", vec![]),
        ];
        let mut draft = opened["draft"].clone();
        for (code, value, want) in steps {
            let edit = json!({ k(code): value });
            // Preview with the edit staged, as the live sync sends it.
            let mut staged = draft.clone();
            apply_screen_edits(
                kind,
                &mut staged,
                edit.as_object().unwrap(),
                true,
                &BTreeSet::new(),
            )
            .unwrap();
            agent::compute(kind, &mut staged).unwrap();
            let filled = fill("1701", &draft, edit);
            let page: Vec<&str> = codes
                .into_iter()
                .filter(|c| filled["values"][k(c)] == "true")
                .collect();
            assert_eq!(page, want, "page after {code}={value}");
            assert_eq!(
                printed(&staged),
                want,
                "staged preview after {code}={value}"
            );
            assert_eq!(
                printed(&filled["draft"]),
                want,
                "preview of the flushed draft after {code}={value}"
            );
            draft = filled["draft"].clone();
        }
        // Item 7: each ATC ticks its own box on the page and the print, one
        // at a time; unticking the ticked one clears it.
        let atcs = [
            "II012", "II014", "II013", "II011", "II015", "II017", "II016",
        ];
        let a = |c: &str| format!("frm1701:rdoPg1I7ATC_{c}");
        let atc_printed = |draft: &Value| -> Vec<&str> {
            let map = with_database(|db| print_map(db, kind, draft)).unwrap().0;
            atcs.into_iter()
                .filter(|c| map.get(&a(c)).is_some_and(|v| v == "true"))
                .collect()
        };
        let mut draft = opened["draft"].clone();
        for code in atcs {
            let mut batch = Map::new();
            for other in atcs {
                batch.insert(
                    a(other),
                    json!(if other == code { "true" } else { "false" }),
                );
            }
            let filled = fill("1701", &draft, Value::Object(batch));
            let page: Vec<&str> = atcs
                .into_iter()
                .filter(|c| filled["values"][a(c)] == "true")
                .collect();
            assert_eq!(page, [code], "page ATC {code}");
            assert_eq!(atc_printed(&filled["draft"]), [code], "print ATC {code}");
            draft = filled["draft"].clone();
        }
        let cleared = fill("1701", &draft, json!({ a("II016"): "false" }));
        let page: Vec<&str> = atcs
            .into_iter()
            .filter(|c| cleared["values"][a(c)] == "true")
            .collect();
        assert!(page.is_empty(), "untick clears Item 7: {page:?}");
        assert!(atc_printed(&cleared["draft"]).is_empty());
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

    fn page_period_of(code: &str) -> i64 {
        match bir_core::forms::find_form(code).map(|form| &form.frequency) {
            Some(FilingFrequency::Annual) => 0,
            _ => 1,
        }
    }

    /// The year a Form library sample is built for (next year).
    fn sample_year() -> i64 {
        use chrono::Datelike;
        i64::from(chrono::Local::now().year()) + 1
    }

    fn sample(code: &str) -> Value {
        call_async("screen_form_sample", vec![TIN.into(), code.into()])
            .unwrap_or_else(|error| panic!("{code} opens as a sample: {error}"))
    }

    fn open_for(tin: &str, code: &str, year: i64, period: i64) -> Value {
        call_async(
            "screen_form_open",
            vec![tin.into(), code.into(), year.into(), period.into()],
        )
        .unwrap_or_else(|error| panic!("{code} opens: {error}"))
    }

    fn fill_unlocked(code: &str, draft: &Value, edits: Value, unlocked: &[&str]) -> Value {
        call(
            "screen_form_fill",
            vec![
                code.into(),
                json(draft.clone()),
                json(edits),
                json(json!(unlocked)),
            ],
        )
        .unwrap_or_else(|error| panic!("{code} fills: {error}"))
    }

    fn strings(value: &Value) -> BTreeSet<String> {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    }

    /// Every on-screen key with its schema type (fields and table cells).
    fn screen_types(code: &str) -> Vec<(String, String)> {
        let form = schema_entry(code).expect("schema entry");
        let mut out = Vec::new();
        let items = form["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|part| part["items"].as_array().into_iter().flatten());
        for item in items {
            for field in item["f"].as_array().into_iter().flatten() {
                out.push((
                    field["k"].as_str().unwrap_or("").to_string(),
                    field["t"].as_str().unwrap_or("").to_string(),
                ));
            }
            let cols = item["cols"].as_array();
            for row in item["keys"].as_array().into_iter().flatten() {
                for (c, key) in row.as_array().into_iter().flatten().enumerate() {
                    let t = cols
                        .and_then(|cols| cols.get(c))
                        .and_then(|col| col["t"].as_str())
                        .unwrap_or("");
                    out.push((key.as_str().unwrap_or("").to_string(), t.to_string()));
                }
            }
        }
        out
    }

    /// Table cells of the schema (`it.keys`).
    fn table_cells(code: &str) -> BTreeSet<String> {
        let form = schema_entry(code).expect("schema entry");
        form["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|part| part["items"].as_array().into_iter().flatten())
            .flat_map(|item| item["keys"].as_array().into_iter().flatten())
            .flat_map(|row| row.as_array().into_iter().flatten())
            .filter_map(|key| key.as_str().map(str::to_string))
            .collect()
    }

    /// Rule 1: the period and date boxes the form findings saw filled on a
    /// Form library opening (`docs/form-findings/<CODE>/findings.json`, R1).
    /// 1702EX's prior-year excess credits and 1707's total collection are
    /// amounts and 1702RT's item 10 is the date of incorporation: they keep
    /// their values.
    const LIBRARY_PERIOD_BOXES: &[(&str, &[&str])] = &[
        (
            "0605",
            &["frm0605:itemYearEndMonth", "frm0605:txtYearEnded"],
        ),
        (
            "0619E",
            &[
                "frm0619E:txtMonth",
                "frm0619E:txtYear",
                "frm0619E:txtDueMonth",
                "frm0619E:txtDueDay",
                "frm0619E:txtDueYear",
            ],
        ),
        (
            "0619F",
            &[
                "frm0619F:txtMonth",
                "frm0619F:txtYear",
                "frm0619F:txtDueMonth",
                "frm0619F:txtDueDay",
                "frm0619F:txtDueYear",
            ],
        ),
        (
            "1600PT",
            &["frm1600PT:txtPg1I1Month", "frm1600PT:txtPg1I1Year"],
        ),
        (
            "1600VT",
            &["frm1600VT:txtPg1I1Month", "frm1600VT:txtPg1I1Year"],
        ),
        (
            "1600WP",
            &[
                "frm1600WP:DateWithholdingMonth",
                "frm1600WP:DateWithholdingDay",
                "frm1600WP:DateWithholdingYear",
                "frm1600WP:DateWithholdingToMonth",
                "frm1600WP:DateWithholdingToDay",
                "frm1600WP:DateWithholdingToYear",
            ],
        ),
        ("1601C", &["frm1601c:txtMonth", "frm1601c:txtYear"]),
        ("1601EQ", &["frm1601EQ:txtYear"]),
        ("1601FQ", &["frm1601FQ:txtYear", "frm1601FQ:OptQuarter1"]),
        ("1602Q", &["frm1602Q:txtYear", "frm1602Q:qtr_1"]),
        ("1603Q", &["frm1603Q:txtYear", "frm1603Q:optQuarter1"]),
        ("1604C", &["frm1604c:txtYear"]),
        ("1604E", &["frm1604e:txtYear"]),
        ("1604F", &["frm1604f:txtYear"]),
        (
            "1606",
            &[
                "frm1606:txtDateMonth",
                "frm1606:txtDateDay",
                "frm1606:txtDateYear",
            ],
        ),
        ("1700", &["frm1700:txtPg1I1Year"]),
        (
            "1701",
            &[
                "frm1701:txtPg1I1Month",
                "frm1701:txtPg1I1Year",
                "frm1701:rdoPg1I3ShortPeriodNo",
            ],
        ),
        (
            "1701A",
            &[
                "frm1701A:txtMonth",
                "frm1701A:txtYear",
                "frm1701A:optShortPeriod_2",
            ],
        ),
        (
            "1701MS",
            &[
                "frm1701MS:txtMonthNo1",
                "frm1701MS:txtYearNo1",
                "frm1701MS:txtReturnPeriodNoNo3",
            ],
        ),
        ("1701Q", &["frm1701q:txtYear", "frm1701q:DateQuarter_1"]),
        (
            "1702EX",
            &[
                "frm1702EX:ddlPg1I2Date",
                "frm1702EX:txtPg1I2YearEnd",
                "frm1702EX:rdoPg1I4ShortPeriodNo",
            ],
        ),
        (
            "1702MX",
            &[
                "frm1702MX:ddlPg1I2Date",
                "frm1702MX:txtPg1I2YearEnd",
                "frm1702MX:rdoPg1I4ShortPeriodNo",
            ],
        ),
        (
            "1702Q",
            &[
                "frm1702q:rbYrEndMonth",
                "frm1702q:txtYrEndYear",
                "frm1702q:rbQuarter_1",
            ],
        ),
        (
            "1702RT",
            &[
                "frm1702RT:ddlPg1I2Month",
                "frm1702RT:txtPg1I2Year",
                "frm1702RT:rdoPg1I4ShortPeriodNo",
            ],
        ),
        (
            "1706",
            &[
                "frm1706:txtDateMonth",
                "frm1706:txtDateDay",
                "frm1706:txtDateYear",
            ],
        ),
        ("1707", &["frm1707:txtPg1I1Year"]),
        (
            "1707A",
            &[
                "frm1707Av2021:txtI1YearEndMonth",
                "frm1707Av2021:txtI1YearEndDay",
                "frm1707Av2021:txtI1YearEndYear",
            ],
        ),
        ("2000", &["frm2000:txtMonth", "frm2000:txtYear"]),
        (
            "2200A",
            &[
                "frm2200Av2020:txtDateMonth",
                "frm2200Av2020:txtDateDay",
                "frm2200Av2020:txtDateYear",
            ],
        ),
        (
            "2200AN",
            &[
                "frm2200ANv2018:txtMonth",
                "frm2200ANv2018:txtDate",
                "frm2200ANv2018:txtYear",
            ],
        ),
        (
            "2200C",
            &[
                "frm2200C:txtPg1I1Month",
                "frm2200C:txtPg1I1Day",
                "frm2200C:txtPg1I1Year",
            ],
        ),
        (
            "2200M",
            &[
                "frm2200M:txtPg1I1Month",
                "frm2200M:txtPg1I1Day",
                "frm2200M:txtPg1I1Year",
            ],
        ),
        (
            "2200P",
            &[
                "frm2200P:txtPg1I1Month",
                "frm2200P:txtPg1I1Day",
                "frm2200P:txtPg1I1Year",
            ],
        ),
        ("2200S", &["frm2200S:txtMonth1", "frm2200S:txtDate"]),
        (
            "2200T",
            &[
                "frm2200Tv2020:txtDateMonth",
                "frm2200Tv2020:txtDateDay",
                "frm2200Tv2020:txtDateYear",
            ],
        ),
        ("2550M", &["frm2550m:RtnYear", "frm2550m:txtYear"]),
        (
            "2550Q",
            &[
                "frm2550qv2024:selectedMonthNo2",
                "frm2550qv2024:txtYearNo2",
                "frm2550qv2024:OptQuarter1",
                "frm2550qv2024:RtnPeriodFromNo4",
                "frm2550qv2024:RtnPeriodToNo4",
            ],
        ),
        (
            "2551Q",
            &[
                "frm2551Qv2018:rtnMonth",
                "frm2551Qv2018:txtYear",
                "frm2551Qv2018:qtr_1",
            ],
        ),
        ("2552", &["frm2552:txtPg1I1Year"]),
        (
            "2553",
            &["frm2553:itemYearEndMonth", "frm2553:txtYearEnded"],
        ),
    ];

    /// Draft path leaves that name the return's period: a box bound to one
    /// shows blank in a sample. Written out here rather than derived, so the
    /// test does not share the host's own rule.
    const PERIOD_LEAVES: &[&str] = &[
        "year",
        "taxable_year",
        "month",
        "quarter",
        "day",
        "transaction_year",
        "transaction_month",
        "transaction_day",
        "transaction_date",
        "year_end_month",
        "year_end_day",
        "year_end_year",
        "is_short_period",
        "due_day",
    ];

    #[test]
    fn library_sample_shows_blank_period_and_date_boxes_on_every_form() {
        setup();
        let mut failures = Vec::new();
        let mut period_boxes = 0;
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            let opened = sample(code);
            assert_eq!(opened["sample"], true, "{code}");
            assert_eq!(opened["editable"], true, "{code}");
            assert!(opened["stored_period"].is_null(), "{code}");
            assert!(
                opened["draft"]["id"].is_null(),
                "{code}: a sample is no stored row"
            );
            assert_eq!(
                strings(&opened["draft"][SAMPLE_MARK]["blank"]),
                strings(&opened["blank"]),
                "{code}: the draft carries the sample mark and its blank boxes"
            );
            assert_error_keys_on_screen(code, &opened["errors"]);
            let blank = strings(&opened["blank"]);
            let profile = strings(&opened["profile"]);
            for key in &blank {
                let shown = opened["values"][key].as_str().unwrap_or("");
                if !shown.is_empty() {
                    failures.push(format!("{code}: {key} = {shown}"));
                }
            }
            // Every box bound to a period path, and every identity box
            // other than the TIN, is blank.
            for (key, _) in screen_types(code) {
                let bound = def.bindings.get(&key).or_else(|| {
                    def.box_def(&key)
                        .and_then(|entry| entry.group.as_deref())
                        .and_then(|group| def.bindings.get(group))
                });
                let Some(bound) = bound else {
                    continue;
                };
                let paths: Vec<&str> = match &bound.binding {
                    Binding::Text { path, .. }
                    | Binding::Amount { path, .. }
                    | Binding::Whole { path, .. } => vec![path],
                    Binding::Choice { options } => options
                        .iter()
                        .flat_map(|option| option.set.keys().map(String::as_str))
                        .collect(),
                };
                let leaf = |path: &str| path.rsplit('/').next().unwrap_or("").to_string();
                let period = paths
                    .iter()
                    .any(|path| PERIOD_LEAVES.contains(&leaf(path).as_str()))
                    || (bound.lock == Some(agent::Lock::Identity)
                        && !paths.iter().any(|path| path.contains("tin")));
                if period {
                    period_boxes += 1;
                    let shown = opened["values"][&key].as_str().unwrap_or("");
                    if !shown.is_empty() {
                        failures.push(format!("{code}: period box {key} = {shown}"));
                    }
                }
            }
            // Not too much: amounts, profile boxes and the taxpayer's own
            // dates keep their values.
            for key in &blank {
                let money = matches!(
                    def.bindings.get(key).map(|bound| &bound.binding),
                    Some(Binding::Amount { .. })
                );
                let own_date = ["birth", "incorporat", "start"]
                    .iter()
                    .any(|word| key.to_ascii_lowercase().contains(word));
                if money || profile.contains(key) || own_date {
                    failures.push(format!("{code}: {key} should keep its value"));
                }
            }
            let regular = open(code, page_period_of(code));
            for key in &profile {
                if opened["values"][key] != regular["values"][key] {
                    failures.push(format!(
                        "{code}: profile box {key} = {} in the sample, {} on a return",
                        opened["values"][key], regular["values"][key]
                    ));
                }
            }
            // A year the sample was never given is not judged: no message
            // names a blank box.
            for error in opened["errors"].as_array().unwrap() {
                let keys = strings(&error["keys"]);
                assert!(
                    keys.iter().all(|key| !blank.contains(key)),
                    "{code}: {error}"
                );
            }
        }
        assert!(period_boxes > 80, "{period_boxes} period boxes checked");
        for (code, keys) in LIBRARY_PERIOD_BOXES {
            let opened = sample(code);
            let blank = strings(&opened["blank"]);
            for key in *keys {
                if !blank.contains(*key) || opened["values"][*key].as_str() != Some("") {
                    failures.push(format!("{code}: {key} = {}", opened["values"][*key]));
                }
            }
        }
        // The taxpayer's own dates (birth, incorporation) stay in a sample.
        for (code, key) in [
            ("1700", "frm1700:txtPg1I9BirthDate"),
            ("1701", "frm1701:txtPg1I10BirthDate"),
            ("1702RT", "frm1702RT:txtPg1Pt1I10"),
            ("1702EX", "frm1702EX:txtPg1Pt1I10DateofIncorporation"),
            ("1702MX", "frm1702MX:txtPg1Pt1I8"),
        ] {
            let regular = open(code, page_period_of(code));
            let shown = regular["values"][key].as_str().unwrap_or("");
            // 1702MX does not copy the profile's start date into item 8
            // (its own group's fix); it must still not be blanked.
            assert!(
                code == "1702MX" || shown.contains("1980") || shown.contains("2015"),
                "{code}: {key} = `{shown}` from the profile"
            );
            assert_eq!(sample(code)["values"][key], shown, "{code}: {key} stays");
        }
        // Answers that only name a period stay: the Calendar of "For the".
        for (code, key) in [
            ("2551Q", "frm2551Qv2018:forThe_1"),
            ("1702Q", "frm1702q:rbForClndrFscl_1"),
            ("1702EX", "frm1702EX:rdoPg1I1Calendar"),
        ] {
            let opened = sample(code);
            assert!(!strings(&opened["blank"]).contains(key), "{code}: {key}");
            let regular = open(code, page_period_of(code));
            assert_eq!(
                opened["values"][key], regular["values"][key],
                "{code}: {key}"
            );
        }
        assert!(failures.is_empty(), "{failures:#?}");
        // The returns the findings saw a future-year message on.
        for (code, said) in [
            ("1701MS", "Future filing"),
            ("1702EX", "cannot be greater"),
            ("1707A", "future date"),
        ] {
            let opened = sample(code);
            for error in opened["errors"].as_array().unwrap() {
                let message = error["message"].as_str().unwrap_or("");
                assert!(!message.contains(said), "{code}: {message}");
            }
        }
        // A form opened for a taxpayer's return keeps its period.
        let opened = open("2551Q", 3);
        assert_eq!(opened["sample"], false);
        assert_eq!(opened["values"]["frm2551Qv2018:txtYear"], "2026");
        assert!(strings(&opened["blank"]).is_empty());
        assert!(opened["draft"].get(SAMPLE_MARK).is_none());
    }

    /// Numeric draft fields whose name says penalty or amount payable.
    fn penalty_amounts(value: &Value, path: &str, out: &mut Vec<(String, f64)>) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    let path = format!("{path}/{key}");
                    let name = key.to_ascii_lowercase();
                    if let Some(number) = value.as_f64()
                        && !name.contains("version")
                        && [
                            "surcharge",
                            "interest",
                            "compromise",
                            "penalt",
                            "amount_payable",
                        ]
                        .iter()
                        .any(|word| name.contains(word))
                    {
                        out.push((path.clone(), number));
                    }
                    penalty_amounts(value, &path, out);
                }
            }
            Value::Array(list) => {
                for (index, value) in list.iter().enumerate() {
                    penalty_amounts(value, &format!("{path}/{index}"), out);
                }
            }
            _ => {}
        }
    }

    /// What a filer types into a sample's period parts for a period.
    fn typed_parts(code: &str, year: u16, month: u8, day: u8, quarter: u8) -> Value {
        let kind = kind_of(code).unwrap();
        let mut parts = Map::new();
        for (key, part) in sample_period_parts(kind) {
            let value = match part {
                PeriodPart::Year => year.to_string(),
                PeriodPart::Month => format!("{month:02}"),
                PeriodPart::YearEndMonth => "12".to_string(),
                PeriodPart::Day => format!("{day:02}"),
                PeriodPart::Quarter(n) => (n == quarter).to_string(),
                PeriodPart::Date => format!("{month:02}/{day:02}/{year}"),
            };
            parts.insert(key, Value::String(value));
        }
        Value::Object(parts)
    }

    fn sample_period(tin: &str, code: &str, parts: Value) -> Value {
        call_async(
            "screen_form_sample_period",
            vec![tin.into(), code.into(), json(parts)],
        )
        .unwrap_or_else(|error| panic!("{code} takes a period: {error}"))
    }

    #[test]
    fn every_period_part_is_typeable_in_a_sample_on_every_form() {
        setup();
        let mut quarters = 0;
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let opened = sample(code);
            let parts: Vec<(String, String)> = opened["period_parts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    (
                        p["key"].as_str().unwrap().to_string(),
                        p["part"].as_str().unwrap().to_string(),
                    )
                })
                .collect();
            let has = |name: &str| parts.iter().any(|(_, part)| part == name);
            // Every form names its year, by itself or in a whole date.
            assert!(has("year") || has("date"), "{code}: {parts:?}");
            match bir_core::forms::find_form(code).map(|form| form.frequency.clone()) {
                Some(FilingFrequency::Monthly) => {
                    assert!(
                        has("month") || has("date"),
                        "{code}: monthly needs its month"
                    )
                }
                Some(FilingFrequency::Quarterly) => {
                    assert!(
                        has("quarter") || has("month"),
                        "{code}: quarterly needs its quarter"
                    )
                }
                _ => {}
            }
            quarters += parts.iter().filter(|(_, part)| part == "quarter").count();
            let blank = strings(&opened["blank"]);
            for (key, _) in &parts {
                // A page input: open (radios and tick boxes too), blank, held.
                assert!(
                    opened["locks"].get(key).is_none(),
                    "{code}: {key} is typeable"
                );
                assert!(blank.contains(key), "{code}: {key} starts blank");
                let shown = opened["values"][key].as_str().unwrap_or("");
                assert!(shown.is_empty(), "{code}: {key} = {shown}");
            }
            assert_eq!(opened["period_set"], false, "{code}");
        }
        assert!(quarters >= 35, "{quarters} quarter boxes");
    }

    /// Every string and number in `value` that carries `year`.
    fn year_traces(value: &Value, year: i64, path: &str, out: &mut Vec<String>) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    year_traces(value, year, &format!("{path}/{key}"), out);
                }
            }
            Value::Array(list) => {
                for (index, value) in list.iter().enumerate() {
                    year_traces(value, year, &format!("{path}/{index}"), out);
                }
            }
            Value::String(text) if text.contains(&year.to_string()) => {
                out.push(format!("{path} = {text}"))
            }
            Value::Number(number) if number.as_i64() == Some(year) => {
                out.push(format!("{path} = {number}"))
            }
            _ => {}
        }
    }

    #[test]
    fn a_complete_period_rebuilds_a_fresh_sample_with_no_placeholder_trace_on_every_form() {
        setup();
        let placeholder = sample_year();
        let mut failures = Vec::new();
        let mut compared = 0;
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let opened = sample(code);
            // Its placeholder period is there to find in the first place.
            let mut before = Vec::new();
            year_traces(&opened["draft"], placeholder, "", &mut before);
            assert!(
                !before.is_empty(),
                "{code}: the placeholder is in the first draft"
            );

            let answer = sample_period(TIN, code, typed_parts(code, 2024, 3, 15, 1));
            assert_eq!(answer["complete"], true, "{code}: {answer}");
            assert!(
                answer.get("existing").is_none(),
                "{code}: no stored return yet"
            );
            let rebuilt = &answer["sample"];
            assert_eq!(rebuilt["period_set"], true, "{code}");
            assert!(
                strings(&rebuilt["blank"]).is_empty(),
                "{code}: nothing held"
            );
            assert!(rebuilt["needs"].as_object().unwrap().is_empty(), "{code}");
            let mut traces = Vec::new();
            year_traces(&rebuilt["draft"], placeholder, "", &mut traces);
            if !traces.is_empty() {
                failures.push(format!("{code}: {traces:?}"));
            }
            // No message is held once the period is set: the page gets what
            // validation says of the rebuilt draft.
            let validated = call_async(
                "screen_form_validate",
                vec![code.into(), json(rebuilt["draft"].clone())],
            )
            .unwrap();
            assert_eq!(
                validated["errors"], rebuilt["errors"],
                "{code}: nothing held"
            );
            // Built as a return for that period is: due dates and every
            // period-derived line agree, where no typed day or date moved them.
            let parts = sample_period_parts(kind);
            let wrote_more = parts
                .iter()
                .any(|(_, part)| matches!(part, PeriodPart::Day | PeriodPart::Date));
            if !wrote_more {
                let regular = open_for(
                    TIN,
                    code,
                    rebuilt["year"].as_i64().unwrap(),
                    rebuilt["period"].as_i64().unwrap(),
                );
                let part_keys: BTreeSet<&String> = parts.iter().map(|(key, _)| key).collect();
                for (key, value) in regular["values"].as_object().unwrap() {
                    if part_keys.contains(key) {
                        continue;
                    }
                    if rebuilt["values"][key] != *value {
                        failures.push(format!(
                            "{code}: {key} = {} in the sample, {value} on the return",
                            rebuilt["values"][key]
                        ));
                    }
                }
                compared += 1;
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
        assert!(compared > 20, "{compared} forms compared with a return");
    }

    #[test]
    fn an_incomplete_period_is_only_that() {
        setup();
        let answer = sample_period(TIN, "1601C", json!({"frm1601c:txtYear": "2024"}));
        assert_eq!(answer, json!({"complete": false}));
        let answer = sample_period(TIN, "2551Q", json!({"frm2551Qv2018:txtYear": "2024"}));
        assert_eq!(
            answer,
            json!({"complete": false}),
            "a quarterly needs its quarter"
        );
    }

    #[test]
    fn a_complete_period_finds_the_stored_return() {
        setup();
        let existing =
            |code: &str, parts: Value| sample_period(TIN, code, parts)["existing"].clone();
        // Monthly: the same month and year.
        let saved = save("1601C", &open_for(TIN, "1601C", 2024, 3)["draft"]);
        let found = existing("1601C", typed_parts("1601C", 2024, 3, 1, 1));
        assert_eq!(found["year"], 2024);
        assert_eq!(found["period"], 3);
        assert_eq!(found["label"], "March 2024");
        assert_eq!(found["status"], "Draft");
        assert!(found.get("updated_at").is_some() && found.get("total_payable").is_some());
        let _ = saved;
        assert!(
            existing("1601C", typed_parts("1601C", 2024, 4, 1, 1)).is_null(),
            "April is new"
        );
        assert!(
            existing("1601C", typed_parts("1601C", 2023, 3, 1, 1)).is_null(),
            "2023 is new"
        );
        // Monthly with a day: the day does not matter.
        save("1600WP", &open_for(TIN, "1600WP", 2024, 3)["draft"]);
        let found = existing("1600WP", typed_parts("1600WP", 2024, 3, 20, 1));
        assert_eq!(found["label"], "March 2024", "{found}");
        // Monthly, rows keyed by date (2200AN): any day of the month.
        save("2200AN", &open_for(TIN, "2200AN", 2024, 3)["draft"]);
        let found = existing("2200AN", typed_parts("2200AN", 2024, 3, 20, 1));
        assert!(found.is_object(), "2200AN March 2024 found by month");
        assert!(existing("2200AN", typed_parts("2200AN", 2024, 4, 20, 1)).is_null());
        // Quarterly: the quarter ticked, and the year.
        save("2551Q", &open_for(TIN, "2551Q", 2024, 1)["draft"]);
        let found = existing("2551Q", typed_parts("2551Q", 2024, 1, 1, 1));
        assert_eq!(found["label"], "Q1 2024");
        assert!(
            existing("2551Q", typed_parts("2551Q", 2024, 1, 1, 2)).is_null(),
            "Q2 is new"
        );
        // Annual: the year.
        save("1701", &open_for(TIN, "1701", 2024, 0)["draft"]);
        assert_eq!(
            existing("1701", typed_parts("1701", 2024, 12, 31, 1))["label"],
            "2024"
        );
        // Per transaction, keyed by its date (2000OT): that date.
        // A sample rebuilt for 03/15/2024 is the return for that date; store
        // one built the same way, then type the date again.
        let built = sample_period(TIN, "2000OT", typed_parts("2000OT", 2024, 3, 15, 1));
        let mut draft = built["sample"]["draft"].clone();
        draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
        let identity =
            agent::identity(AgentFormKind::from_code("2000OT").unwrap(), &draft).unwrap();
        assert_eq!(
            identity.period,
            FilingPeriod::OpenEnded(315),
            "keyed by its date"
        );
        save("2000OT", &draft);
        let found = existing("2000OT", typed_parts("2000OT", 2024, 3, 15, 1));
        assert_eq!(found["label"], "03/15/2024", "{found}");
        assert!(existing("2000OT", typed_parts("2000OT", 2024, 3, 16, 1)).is_null());
    }

    /// Decided 2026-10-17: a form filed "As needed" and keyed by its
    /// transaction date matches a stored return by that exact date; a
    /// monthly form keeps matching by month and year, whatever the day. On
    /// every form whose period parts include a day (entry-numbered forms
    /// never match), a return stored for 03/15/2024 is found on 03/15; on
    /// 03/16 a monthly form finds it and a per-transaction form builds a
    /// fresh sample, the second transaction of the month; April is new.
    #[test]
    fn a_transaction_date_matches_exactly_and_a_month_by_month() {
        setup();
        let mut exact = Vec::new();
        let mut by_month = Vec::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let has_day = sample_period_parts(kind)
                .iter()
                .any(|(_, part)| matches!(part, PeriodPart::Day | PeriodPart::Date));
            if !has_day || NUMBERED_FORMS.contains(&code) {
                continue;
            }
            let built = sample_period(TIN, code, typed_parts(code, 2024, 3, 15, 1));
            let mut draft = built["sample"]["draft"].clone();
            draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
            save(code, &draft);
            let same = sample_period(TIN, code, typed_parts(code, 2024, 3, 15, 1));
            assert!(
                same["existing"].is_object(),
                "{code}: the same date: {same}"
            );
            let next_day = sample_period(TIN, code, typed_parts(code, 2024, 3, 16, 1));
            if next_day["existing"].is_object() {
                by_month.push(code);
            } else {
                assert!(
                    next_day["sample"].is_object(),
                    "{code}: a fresh sample for the second transaction"
                );
                assert_eq!(next_day["sample"]["sample"], true, "{code}");
                exact.push(code);
            }
            let april = sample_period(TIN, code, typed_parts(code, 2024, 4, 15, 1));
            assert!(april["existing"].is_null(), "{code}: April is new");
        }
        exact.sort_unstable();
        by_month.sort_unstable();
        assert_eq!(exact, ["2000OT", "2552"], "As needed, keyed by the date");
        assert_eq!(
            by_month,
            [
                "1600WP", "2200A", "2200AN", "2200C", "2200M", "2200P", "2200S", "2200T"
            ],
            "Monthly, the day does not matter"
        );
        // The screen index says how often each is filed.
        for code in &exact {
            assert_eq!(schema_entry(code).unwrap()["freq"], "As needed", "{code}");
        }
        for code in &by_month {
            assert_eq!(schema_entry(code).unwrap()["freq"], "Monthly", "{code}");
        }
    }

    /// Decided 2026-10-17: a sample can be saved as a real return. On every
    /// form: the placeholder sample (period not set) is refused with what to
    /// do; one rebuilt for a complete period saves as that period's return,
    /// with what was typed into it; saving it again finds that stored return
    /// (the matching rules) and saves nothing; an entry-numbered form takes
    /// its next free entry instead, so nothing is replaced either way.
    #[test]
    fn a_library_sample_saves_as_its_periods_return_on_every_form() {
        setup();
        let mut carried = 0;
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let placeholder = sample(code);
            let refused = call(
                "screen_form_save",
                vec![code.into(), json(placeholder["draft"].clone())],
            )
            .expect_err("the period is not set")
            .to_string();
            assert!(refused.contains("period first"), "{code}: {refused}");

            let answer = sample_period(TIN, code, typed_parts(code, 2024, 3, 15, 1));
            assert!(answer["sample"].is_object(), "{code}: {answer}");
            let mut built = answer["sample"]["draft"].clone();
            // The first amount box that takes a typed amount and keeps it
            // through the form's own computation (an off or recomputed box
            // does not).
            let def = kind.definition();
            let open_amount = def
                .fillable()
                .filter(|entry| {
                    schema_keys(code).contains(&entry.key)
                        && matches!(
                            def.bindings.get(&entry.key).map(|bound| &bound.binding),
                            Some(Binding::Amount { .. })
                        )
                })
                .take(12)
                .find_map(|entry| {
                    let typed = fill(code, &built, json!({ &entry.key: "1234.00" }));
                    (typed["rejected"].get(&entry.key).is_none()
                        && amount_of(&typed["values"], &entry.key) == 1234.0)
                        .then(|| (entry.key.clone(), typed["draft"].clone()))
                });
            let took = open_amount.is_some();
            let amount = open_amount
                .map(|(key, draft)| {
                    built = draft;
                    key
                })
                .unwrap_or_default();
            assert!(
                built[SAMPLE_MARK]["set"] == true,
                "{code}: rebuilt for its period"
            );
            let saved = call("screen_form_save", vec![code.into(), json(built.clone())])
                .unwrap_or_else(|error| panic!("{code}: {error}"));
            assert_eq!(saved["saved"], true, "{code}: {saved}");
            assert_eq!(saved["status"], "Draft", "{code}");
            assert!(
                saved["draft"].get(SAMPLE_MARK).is_none(),
                "{code}: no longer a sample"
            );
            // Which stored return (2550Q's rows keep no id: its created_at).
            let which = |draft: &Value| {
                if code == "2550Q" {
                    json!(created_at(draft))
                } else {
                    draft["id"].clone()
                }
            };
            let id = which(&saved["draft"]);
            // The page opens it from `opens`, as it opens an offer.
            let stored = opens_of(code, &saved["opens"]);
            assert_eq!(which(&stored["draft"]), id, "{code}: the stored return");
            if took {
                assert_eq!(
                    amount_of(&stored["values"], &amount),
                    1234.0,
                    "{code}: carried"
                );
                carried += 1;
            }
            // Saved again, the same sample never replaces it.
            let again = call("screen_form_save", vec![code.into(), json(built.clone())])
                .unwrap_or_else(|error| panic!("{code}: {error}"));
            if NUMBERED_FORMS.contains(&code) {
                assert_eq!(again["saved"], true, "{code}: the next entry");
                assert_ne!(which(&again["draft"]), id, "{code}: a return of its own");
                assert_ne!(again["stored_period"], saved["stored_period"], "{code}");
            } else {
                assert_eq!(again["saved"], false, "{code}: {again}");
                assert!(again["existing"].is_object(), "{code}: the offer");
                assert_eq!(
                    which(&opens_of(code, &again["opens"])["draft"]),
                    id,
                    "{code}: the offer opens the stored return"
                );
            }
        }
        assert!(carried >= 35, "{carried} forms carried a typed amount");
    }

    fn amount_of(values: &Value, key: &str) -> f64 {
        agent::parse_amount(values[key].as_str().unwrap_or_default()).unwrap_or(f64::NAN)
    }

    #[test]
    fn a_sample_cannot_be_saved_over_a_stored_return_nor_a_mark_forged() {
        setup();
        let (month, tax) = ("frm1601c:txtMonth", "frm1601c:txtTax14");
        // Attack 1: a sample rebuilt for March 2025 before that return was
        // stored, its mark stripped, saved once the return exists.
        let answer = sample_period(TIN, "1601C", typed_parts("1601C", 2025, 3, 1, 1));
        let typed = fill(
            "1601C",
            &answer["sample"]["draft"],
            json!({ tax: "999.00" }),
        );
        let opened = open_for(TIN, "1601C", 2025, 3);
        let filled = fill("1601C", &opened["draft"], json!({ tax: "1234.00" }));
        let saved = save("1601C", &filled["draft"]);
        let mut stripped = typed["draft"].clone();
        stripped.as_object_mut().unwrap().remove(SAMPLE_MARK);
        let refused = call("screen_form_save", vec!["1601C".into(), json(stripped)])
            .expect_err("an id-less draft never replaces a stored return");
        assert!(refused.to_string().contains("already stored"), "{refused}");
        let stored = open_for(TIN, "1601C", 2025, 3);
        assert_eq!(stored["draft"]["id"], saved["id"]);
        assert_eq!(
            amount(&stored["values"], tax),
            1234.0,
            "the stored return is untouched"
        );
        save("1601C", &stored["draft"]);
        // Attack 2: no mark opens a period: a sample's period is never a fill.
        let mut forged = stored["draft"].clone();
        forged[SAMPLE_MARK] = json!({ "blank": [], "open": [month] });
        let refused = fill("1601C", &forged, json!({ month: "04" }));
        assert!(
            refused["rejected"].get(month).is_some(),
            "{}",
            refused["rejected"]
        );
        let shown = sample("1601C");
        let refused = fill("1601C", &shown["draft"], json!({ month: "04" }));
        assert!(
            refused["rejected"].get(month).is_some(),
            "{}",
            refused["rejected"]
        );
    }

    /// A stored draft's JSON with its status set (top level, or in its lifecycle).
    fn with_status(draft: &Value, status: &str) -> Value {
        let mut draft = draft.clone();
        draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
        if draft["lifecycle"].is_object() {
            draft["lifecycle"]["status"] = json!(status);
        } else {
            draft["status"] = json!(status);
        }
        draft
    }

    #[test]
    fn an_existing_returns_offer_follows_its_status() {
        setup();
        let offer = |code: &str, status: &str| {
            let kind = kind_of(code).unwrap();
            let stored = with_status(&open(code, page_period_of(code).max(1))["draft"], status);
            with_database(|db| existing_return(db, kind, &stored).map_err(HostError::new)).unwrap()
                ["actions"]
                .clone()
        };
        for code in ["0619E", "1601C", "2551Q"] {
            assert_eq!(offer(code, "Draft"), json!(["load", "keep"]), "{code}");
            assert_eq!(offer(code, "Queued"), json!(["view", "keep"]), "{code}");
            assert_eq!(
                offer(code, "Submitted"),
                json!(["view", "amend", "keep"]),
                "{code}"
            );
            assert_eq!(
                offer(code, "Confirmed"),
                json!(["view", "amend", "keep"]),
                "{code}"
            );
            // A Paid return cannot be reverted, so it is never amended here.
            assert_eq!(offer(code, "Paid"), json!(["view", "keep"]), "{code}");
        }
    }

    /// Forms with no "Amended Return?" answer (`QueueableForm::mark_amended`
    /// is false): the host never offers to amend them.
    const FORMS_WITHOUT_AN_AMENDED_ANSWER: &[&str] = &["0605"];

    #[test]
    fn amend_is_offered_only_where_the_form_has_an_amended_answer() {
        setup();
        let mut without = Vec::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let stored = with_status(&open(code, page_period_of(code))["draft"], "Submitted");
            let actions =
                with_database(|db| existing_return(db, kind, &stored).map_err(HostError::new))
                    .unwrap()["actions"]
                    .clone();
            let amend = actions.as_array().unwrap().iter().any(|a| a == "amend");
            match agent::amended_copy(kind, &stored) {
                Ok(copy) => {
                    assert!(amend, "{code}: amendable, so offered");
                    let identity = agent::identity(kind, &copy).unwrap();
                    assert_eq!(identity.status, FilingStatus::Draft, "{code}");
                    assert!(copy["id"].is_null(), "{code}: a new return, no row");
                }
                Err(error) => {
                    assert!(!amend, "{code}: {error}");
                    without.push(code);
                }
            }
        }
        assert_eq!(without, FORMS_WITHOUT_AN_AMENDED_ANSWER, "{without:?}");
    }

    #[test]
    fn the_host_amends_only_a_return_that_reached_bir() {
        setup();
        save("0619E", &open("0619E", 6)["draft"]);
        let refused = call_async(
            "screen_form_amend",
            vec![TIN.into(), "0619E".into(), YEAR.into(), 6.into()],
        )
        .expect_err("a stored Draft is loaded, never amended");
        assert!(
            refused.to_string().contains("submitted or confirmed"),
            "{refused}"
        );
    }

    /// A stored row's status column and data, read raw.
    fn raw_row(id: i64) -> (String, String) {
        with_database(|db| {
            db.conn_for_tests()
                .query_row(
                    "SELECT status, data_json FROM form_drafts WHERE id = ?1",
                    [id],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )
                .map_err(|e| HostError::new(e.to_string()))
        })
        .unwrap()
    }

    fn raw_count(sql: &str) -> i64 {
        with_database(|db| {
            db.conn_for_tests()
                .query_row(sql, [], |r| r.get::<_, i64>(0))
                .map_err(|e| HostError::new(e.to_string()))
        })
        .unwrap()
    }

    fn raw_exec(sql: &str, args: &[&str]) {
        with_database(|db| {
            let mut sql = sql.to_string();
            for (i, arg) in args.iter().enumerate() {
                sql = sql.replace(
                    &format!("?{}", i + 1),
                    &format!("'{}'", arg.replace('\'', "''")),
                );
            }
            db.conn_for_tests()
                .execute_batch(&sql)
                .map_err(|e| HostError::new(e.to_string()))
        })
        .unwrap()
    }

    /// Save `draft` (no marks) and turn its row into a return that reached BIR.
    fn store_filed(code: &str, draft: &Value, status: &str) -> i64 {
        let mut draft = draft.clone();
        draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
        let id = save(code, &draft)["id"].as_i64().unwrap();
        let (_, raw) = raw_row(id);
        let mut filed = with_status(&serde_json::from_str(&raw).unwrap(), status);
        if filed["lifecycle"].is_object() {
            filed["lifecycle"]["submitted_at"] = json!("2024-04-10T01:02:03+00:00");
        } else {
            filed["submitted_at"] = json!("2024-04-10T01:02:03+00:00");
        }
        raw_exec(
            &format!("UPDATE form_drafts SET status = ?1, data_json = ?2 WHERE id = {id}"),
            &[status, &serde_json::to_string(&filed).unwrap()],
        );
        id
    }

    fn opens_of(code: &str, opens: &Value) -> Value {
        call_async(
            "screen_form_open",
            vec![
                TIN.into(),
                code.into(),
                json(opens["year"].clone()),
                json(opens["period"].clone()),
                json(opens["key"].clone()),
                json(opens["new_key"].clone()),
            ],
        )
        .unwrap_or_else(|error| panic!("{code} opens {opens}: {error}"))
    }

    #[test]
    fn an_exact_key_that_is_no_longer_stored_fails() {
        setup();
        // Nothing is stored for 2031: the open and the amend by key fail and
        // never fall back to a period scan or a new return.
        for (code, period, key) in [
            ("0619E", 3, 3),
            ("2551Q", 2, 2),
            ("1601C", 4, 4),
            ("2550Q", 1, 1),
            ("1606", 3, 315),
        ] {
            for function in ["screen_form_open", "screen_form_amend"] {
                let error = call_async(
                    function,
                    vec![
                        TIN.into(),
                        code.into(),
                        2031_i64.into(),
                        i64::from(period).into(),
                        i64::from(key).into(),
                    ],
                )
                .expect_err(&format!("{code} {function} by a missing key"));
                assert!(
                    error
                        .to_string()
                        .contains("That return is no longer stored"),
                    "{code} {function}: {error}"
                );
            }
            // Without a key the period opens (a new return).
            call_async(
                "screen_form_open",
                vec![
                    TIN.into(),
                    code.into(),
                    2031_i64.into(),
                    i64::from(period).into(),
                ],
            )
            .unwrap_or_else(|error| panic!("{code}: {error}"));
        }
    }

    #[test]
    fn a_rebuilt_sample_is_exactly_the_typed_period_on_every_form() {
        setup();
        let mut failures = Vec::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let parts = sample_period_parts(kind);
            let typed = typed_parts(code, 2024, 3, 15, 1);
            let answer = sample_period(TIN, code, typed.clone());
            let rebuilt = &answer["sample"];
            // Every period part shows what was typed.
            for (key, part) in &parts {
                let want = typed[key].as_str().unwrap();
                let shown = rebuilt["values"][key].as_str().unwrap_or("");
                let ok = match part {
                    PeriodPart::Quarter(_) => (shown == "true") == (want == "true"),
                    PeriodPart::Year => {
                        shown == want || (shown.len() == 2 && want.ends_with(shown))
                    }
                    _ => shown.trim_start_matches('0') == want.trim_start_matches('0'),
                };
                if !ok {
                    failures.push(format!(
                        "{code}: {key} ({}) = `{shown}`, typed `{want}`",
                        part.name()
                    ));
                }
            }
            // And the draft is the return for that period (an independent
            // check: its identity, not the field map).
            let mut draft = rebuilt["draft"].clone();
            draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
            let identity = agent::identity(kind, &draft).unwrap();
            if identity.taxable_year != 2024 {
                failures.push(format!("{code}: year {}", identity.taxable_year));
            }
            let period_ok = match identity.period {
                FilingPeriod::Monthly(month) => month == 3,
                FilingPeriod::Quarterly(quarter) => quarter == 1,
                FilingPeriod::Annual => true,
                FilingPeriod::OpenEnded(key) => {
                    key == 315 || NUMBERED_FORMS.contains(&code) || (code == "2553" && key == 1)
                }
            };
            if !period_ok {
                failures.push(format!("{code}: period {:?}", identity.period));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn a_year_counts_only_with_all_its_digits() {
        setup();
        // A four-digit year is the year typed: "0020" and "0099" are out of
        // range, never 2020 or 2099.
        for year in ["2", "20", "202", "20x4", "0020", "0099", "0024"] {
            let answer = sample_period(
                TIN,
                "2551Q",
                json!({"frm2551Qv2018:txtYear": year, "frm2551Qv2018:qtr_4": "true"}),
            );
            assert_eq!(answer, json!({"complete": false}), "{year}");
        }
        let answer = sample_period(
            TIN,
            "2551Q",
            json!({"frm2551Qv2018:txtYear": "2024", "frm2551Qv2018:qtr_4": "true"}),
        );
        assert_eq!(answer["complete"], true);
    }

    #[test]
    fn every_existing_answer_opens_the_matched_return() {
        setup();
        let mut failures = Vec::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            if NUMBERED_FORMS.contains(&code) {
                continue;
            }
            let typed = typed_parts(code, 2024, 3, 15, 1);
            let built = sample_period(TIN, code, typed.clone());
            let mut draft = built["sample"]["draft"].clone();
            draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
            let saved = match call("screen_form_save", vec![code.into(), json(draft)]) {
                Ok(saved) => saved,
                Err(error) => {
                    failures.push(format!("{code}: the rebuilt sample does not save: {error}"));
                    continue;
                }
            };
            let answer = sample_period(TIN, code, typed);
            if !answer["existing"].is_object() {
                failures.push(format!("{code}: no existing answer: {answer}"));
                continue;
            }
            let opened = opens_of(code, &answer["opens"]);
            let id = opened["draft"]["id"].clone();
            if !id.is_null() && id != saved["id"] {
                failures.push(format!("{code}: opened row {id}, stored {}", saved["id"]));
            }
            let mut d = opened["draft"].clone();
            d.as_object_mut().unwrap().remove(SAMPLE_MARK);
            let identity = agent::identity(kind, &d).unwrap();
            if identity.taxable_year != 2024 {
                failures.push(format!("{code}: opened {:?}", identity));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn two_returns_in_one_year_each_open_as_themselves() {
        setup();
        for (code, first, second) in [
            (
                "2553",
                typed_parts("2553", 2024, 3, 15, 1),
                typed_parts("2553", 2024, 9, 15, 3),
            ),
            (
                "2552",
                typed_parts("2552", 2024, 3, 15, 1),
                typed_parts("2552", 2024, 9, 20, 3),
            ),
            (
                "2000OT",
                typed_parts("2000OT", 2024, 3, 15, 1),
                typed_parts("2000OT", 2024, 9, 20, 3),
            ),
        ] {
            let mut ids = Vec::new();
            for typed in [&first, &second] {
                let built = sample_period(TIN, code, typed.clone());
                let mut draft = built["sample"]["draft"].clone();
                draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
                ids.push(save(code, &draft)["id"].clone());
            }
            assert_ne!(ids[0], ids[1], "{code}: two returns");
            for (typed, id) in [(&first, &ids[0]), (&second, &ids[1])] {
                let answer = sample_period(TIN, code, typed.clone());
                assert!(answer["existing"].is_object(), "{code}: {answer}");
                assert_eq!(
                    opens_of(code, &answer["opens"])["draft"]["id"],
                    *id,
                    "{code}"
                );
            }
        }
    }

    #[test]
    fn an_entry_numbered_form_starts_with_new_return() {
        setup();
        for code in NUMBERED_FORMS {
            save(code, &open_for(TIN, code, 2024, 3)["draft"]);
            let answer = sample_period(TIN, code, typed_parts(code, 2024, 3, 15, 1));
            assert_eq!(answer["complete"], true, "{code}: {answer}");
            assert!(
                answer["existing"].is_null(),
                "{code}: an entry number never matches a date"
            );
            assert!(
                answer["opens"].is_null(),
                "{code}: Start a return opens New return"
            );
        }
    }

    #[test]
    fn a_0605_quarter_is_its_quarter() {
        setup();
        let answer = sample_period(TIN, "0605", typed_parts("0605", 2024, 3, 15, 3));
        let rebuilt = &answer["sample"];
        assert_eq!(
            rebuilt["draft"]["quarter"], 3,
            "the typed quarter, not an entry number"
        );
        assert_eq!(rebuilt["values"]["itemQuarter_3"], "true");
        let mut draft = rebuilt["draft"].clone();
        draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
        let identity = agent::identity(kind_of("0605").unwrap(), &draft).unwrap();
        assert_eq!(identity.period, FilingPeriod::OpenEnded(1), "entry 1");
    }

    #[test]
    fn a_donation_or_death_date_is_its_returns_period() {
        // 1800's date of donation and 1801's date of death are those
        // returns' periods. They are bound by their own month, day and year
        // fields, not as parts of one date field, so the refund-date rule
        // (a part of a date other than /transaction_date) never applies.
        for (code, expected) in [
            (
                "1800",
                [
                    "frm1800v2018:txtMonth=month",
                    "frm1800v2018:txtDate=day",
                    "frm1800v2018:txtYear=year",
                ],
            ),
            (
                "1801",
                [
                    "frm1801v2018:txtDateMonth=month",
                    "frm1801v2018:txtDateDay=day",
                    "frm1801v2018:txtDateYear=year",
                ],
            ),
        ] {
            let parts: Vec<String> = sample_period_parts(kind_of(code).unwrap())
                .into_iter()
                .map(|(key, part)| format!("{key}={}", part.name()))
                .collect();
            assert_eq!(parts, expected, "{code}");
        }
    }

    #[test]
    fn a_refund_date_and_a_computed_day_are_not_period_parts() {
        setup();
        let parts: Vec<String> = sample_period_parts(kind_of("1604C").unwrap())
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert_eq!(
            parts,
            ["frm1604c:txtYear"],
            "1604C's refund date is an answer"
        );
        let parts: Vec<String> = sample_period_parts(kind_of("1707A").unwrap())
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert!(
            !parts.iter().any(|key| key.ends_with("txtI1YearEndDay")),
            "1707A's year-end day is computed: {parts:?}"
        );
    }

    #[test]
    fn a_future_typed_year_is_judged_and_a_placeholder_holds_its_messages() {
        setup();
        // 2099 typed into a sample is the period the filer chose: judged.
        let answer = sample_period(TIN, "2200M", typed_parts("2200M", 2099, 3, 15, 1));
        let errors = answer["sample"]["errors"].to_string().to_ascii_lowercase();
        assert!(errors.contains("future"), "{errors}");
        // The placeholder's own year is held: the same validation names held
        // boxes, and the sample shows none of those messages.
        let opened = sample("1701MS");
        let blank = strings(&opened["blank"]);
        let mut draft = opened["draft"].clone();
        draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
        let validated =
            call_async("screen_form_validate", vec!["1701MS".into(), json(draft)]).unwrap();
        let held: Vec<&Value> = validated["errors"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| strings(&e["keys"]).iter().any(|k| blank.contains(k)))
            .collect();
        assert!(!held.is_empty(), "the placeholder's own messages exist");
        for error in held {
            assert!(
                !opened["errors"].as_array().unwrap().contains(error),
                "held: {error}"
            );
        }
    }

    #[test]
    fn direct_radio_steps_keep_a_returns_period() {
        setup();
        let opened = open("2550Q", 1);
        let key = "frm2550qv2024:OptQuarter1";
        let refused = fill("2550Q", &opened["draft"], json!({ key: "false" }));
        assert!(
            refused["rejected"].get(key).is_some(),
            "{}",
            refused["rejected"]
        );
        let mut draft = refused["draft"].clone();
        draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
        let identity = agent::identity(AgentFormKind::from_code("2550Q").unwrap(), &draft).unwrap();
        assert_eq!(identity.period, FilingPeriod::Quarterly(1));
    }

    #[test]
    fn a_draft_saves_only_to_its_own_row() {
        setup();
        let march = save("1601C", &open_for(TIN, "1601C", 2024, 3)["draft"]);
        let april = save("1601C", &open_for(TIN, "1601C", 2024, 4)["draft"]);
        let mut wrong = open_for(TIN, "1601C", 2024, 4)["draft"].clone();
        wrong["id"] = march["id"].clone();
        let refused = call("screen_form_save", vec!["1601C".into(), json(wrong)])
            .expect_err("an id from another row");
        assert!(refused.to_string().contains("row"), "{refused}");
        let mut ghost = open_for(TIN, "1601C", 2024, 5)["draft"].clone();
        ghost["id"] = json!(99999);
        let refused = call("screen_form_save", vec!["1601C".into(), json(ghost)])
            .expect_err("an id with no row");
        assert!(refused.to_string().contains("not stored"), "{refused}");
        let _ = april;
    }

    /// `value` with every number as f64: the host's JSON bridge writes 0 for
    /// a stored 0.0.
    fn numbers_as_f64(value: &Value) -> Value {
        match value {
            Value::Number(number) => json!(number.as_f64()),
            Value::Array(list) => Value::Array(list.iter().map(numbers_as_f64).collect()),
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(k, v)| (k.clone(), numbers_as_f64(v)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    /// The first fillable amount box on screen, for tagging an amendment.
    fn first_amount_box(code: &str) -> String {
        let def = kind_of(code).unwrap().definition();
        def.fillable()
            .find(|entry| {
                schema_keys(code).contains(&entry.key)
                    && matches!(
                        def.bindings.get(&entry.key).map(|bound| &bound.binding),
                        Some(Binding::Amount { .. })
                    )
            })
            .map(|entry| entry.key.clone())
            .expect("an amount box")
    }

    #[test]
    fn an_amendments_first_save_is_one_transaction_and_keeps_the_filed_original() {
        setup();
        for (code, period) in [
            ("0619E", 6i64),
            ("1601C", 6),
            ("2551Q", 2),
            ("2550Q", 2),
            ("1701Q", 2),
            ("2200AN", 6),
        ] {
            let opened = open_for(TIN, code, 2024, period);
            let id = store_filed(code, &opened["draft"], "Submitted");
            let original = raw_row(id);
            let filed = open_for(TIN, code, 2024, period);
            let key = stored_period(kind_of(code).unwrap(), &{
                let mut d = filed["draft"].clone();
                d.as_object_mut().unwrap().remove(SAMPLE_MARK);
                d
            });
            let amend = |key: Option<i64>| {
                call_async(
                    "screen_form_amend",
                    vec![
                        TIN.into(),
                        code.into(),
                        2024i64.into(),
                        period.into(),
                        json(json!(key)),
                    ],
                )
                .unwrap_or_else(|error| panic!("{code} amends: {error}"))
            };
            let amended = amend(key);
            assert_eq!(
                amended["draft"][AMENDMENT_MARK]["status"], "Submitted",
                "{code}"
            );
            assert_eq!(raw_row(id), original, "{code}: nothing stored by amending");
            let tag = first_amount_box(code);
            let mut edits = Map::new();
            if code == "0619E" {
                edits.insert("frm0619E:optWithheld:Y".into(), json!("true"));
            }
            edits.insert(tag.clone(), json!("4242.42"));
            let typed = fill(code, &amended["draft"], Value::Object(edits));
            let snapshots = || raw_count("SELECT COUNT(*) FROM amendment_snapshots");
            let before = snapshots();

            // D1: the amendment's own write (a Draft row written as a Draft,
            // after the revert in the same transaction) fails: nothing changes.
            raw_exec(
                "CREATE TRIGGER block_amendment BEFORE UPDATE ON form_drafts WHEN OLD.status = 'Draft' AND NEW.status = 'Draft' BEGIN SELECT RAISE(ABORT, 'blocked'); END;",
                &[],
            );
            let failed = call(
                "screen_form_save",
                vec![code.into(), json(typed["draft"].clone())],
            );
            raw_exec("DROP TRIGGER block_amendment;", &[]);
            assert!(failed.is_err(), "{code}: the blocked save went through");
            assert_eq!(raw_row(id), original, "{code}: the filed row is as it was");
            assert_eq!(
                snapshots(),
                before,
                "{code}: no snapshot from a failed save"
            );

            // D5: the filed row changed since the amendment began: refused.
            let mut moved: Value = serde_json::from_str(&original.1).unwrap();
            if moved["lifecycle"].is_object() {
                moved["lifecycle"]["updated_at"] = json!("2030-01-01T00:00:00Z");
            } else {
                moved["updated_at"] = json!("2030-01-01T00:00:00Z");
            }
            raw_exec(
                &format!("UPDATE form_drafts SET data_json = ?1 WHERE id = {id}"),
                &[&serde_json::to_string(&moved).unwrap()],
            );
            let refused = call(
                "screen_form_save",
                vec![code.into(), json(typed["draft"].clone())],
            )
            .expect_err("the filed return moved");
            assert!(
                refused.to_string().contains("changed since"),
                "{code}: {refused}"
            );
            raw_exec(
                &format!("UPDATE form_drafts SET data_json = ?1 WHERE id = {id}"),
                &[&original.1],
            );

            // The retry: one save, the snapshot is the original byte for byte
            // and the row holds the amendment.
            let saved = save(code, &typed["draft"]);
            assert_eq!(saved["id"], id, "{code}: the amendment takes the row");
            assert!(saved["draft"].get(AMENDMENT_MARK).is_none(), "{code}");
            let after = raw_row(id);
            assert_eq!(after.0, "Draft", "{code}");
            // The row holds the amendment: a Draft with the amended answer,
            // and the typed amount where the form keeps it.
            let stored: Value = serde_json::from_str(&after.1).unwrap();
            assert_eq!(
                agent::identity(kind_of(code).unwrap(), &stored)
                    .unwrap()
                    .status,
                FilingStatus::Draft,
                "{code}: the row holds the amendment"
            );
            assert_ne!(after.1, original.1, "{code}");
            let kept = typed["values"][tag.as_str()]
                .as_str()
                .and_then(agent::parse_amount)
                == Some(4242.42);
            assert_eq!(
                after.1.contains("4242.42"),
                kept,
                "{code}: the typed amount"
            );
            assert_eq!(snapshots(), before + 1, "{code}");
            assert_eq!(
                raw_count(&format!(
                    "SELECT COUNT(*) FROM amendment_snapshots WHERE draft_json = '{}'",
                    original.1.replace('\'', "''")
                )),
                1,
                "{code}: the filed original, byte for byte"
            );
            // A second save is an ordinary save of that row.
            save(code, &saved["draft"]);

            // History view: the filed original opens read-only, as filed.
            let reopened = open_for(TIN, code, 2024, period);
            let snapshot = reopened["filed_original"]["id"]
                .as_i64()
                .expect("in its history");
            let view = call_async(
                "screen_form_filed_original",
                vec![code.into(), snapshot.into()],
            )
            .unwrap();
            assert_eq!(view["editable"], false, "{code}");
            assert_eq!(view["filed_original_view"]["status"], "Submitted", "{code}");
            let original_value: Value = serde_json::from_str(&original.1).unwrap();
            let mut shown = view["draft"].clone();
            shown.as_object_mut().unwrap().remove(SAMPLE_MARK);
            assert_eq!(
                numbers_as_f64(&shown),
                numbers_as_f64(&original_value),
                "{code}: the filed version as filed"
            );
        }
    }

    #[test]
    fn a_filed_2550q_is_never_saved_over() {
        setup();
        // A sample built for Q1 2025, its mark stripped, after Q1 was filed.
        let built = sample_period(TIN, "2550Q", typed_parts("2550Q", 2025, 3, 15, 1));
        let mut stripped = built["sample"]["draft"].clone();
        stripped.as_object_mut().unwrap().remove(SAMPLE_MARK);
        let opened = open_for(TIN, "2550Q", 2025, 1);
        let id = store_filed("2550Q", &opened["draft"], "Submitted");
        let original = raw_row(id);
        let refused = call("screen_form_save", vec!["2550Q".into(), json(stripped)])
            .expect_err("a stripped sample over a filed 2550Q");
        assert!(refused.to_string().contains("Submitted"), "{refused}");
        // A viewed filed 2550Q with its status and id stripped.
        let viewed = open_for(TIN, "2550Q", 2025, 1);
        let mut forged = with_status(&viewed["draft"], "Draft");
        forged.as_object_mut().unwrap().remove("id");
        let refused = call("screen_form_save", vec!["2550Q".into(), json(forged)])
            .expect_err("a viewed filed 2550Q re-saved");
        assert!(refused.to_string().contains("Submitted"), "{refused}");
        assert_eq!(raw_row(id), original, "the filed 2550Q is untouched");
    }

    #[test]
    fn library_sample_computes_no_penalty_on_any_form() {
        setup();
        let mut failures = Vec::new();
        let mut checked = 0;
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let opened = sample(code);
            let mut amounts = Vec::new();
            penalty_amounts(&opened["draft"], "", &mut amounts);
            for (path, number) in amounts {
                checked += 1;
                // Nothing is payable. 1800's official formula leaves an empty
                // return at (0 - 250,000 exempt) x 6% = -15,000.00, which is
                // no penalty and nothing due.
                let payable = path.ends_with("amount_payable");
                if (payable && number > 0.0) || (!payable && number != 0.0) {
                    failures.push(format!("{code}: {path} = {number}"));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
        assert!(checked > 43, "{checked} penalty amounts checked");
        // The 2551Q page boxes the findings saw at 1,000.00.
        let opened = sample("2551Q");
        for key in ["txtTax22", "txtTax23", "txtTax24"] {
            let key = format!("frm2551Qv2018:{key}");
            let shown = opened["values"][&key].as_str().unwrap_or("0");
            assert_eq!(agent::parse_amount(shown).unwrap_or(0.0), 0.0, "{key}");
        }
    }

    #[test]
    fn library_sample_never_reads_or_replaces_a_stored_return() {
        setup();
        let year = sample_year();
        let first_amount = |code: &str| {
            let def = kind_of(code).unwrap().definition();
            def.fillable()
                .find(|entry| {
                    schema_keys(code).contains(&entry.key)
                        && matches!(
                            def.bindings.get(&entry.key).map(|bound| &bound.binding),
                            Some(Binding::Amount { .. })
                        )
                })
                .map(|entry| entry.key.clone())
        };
        // The return the sample would be for, stored first with an amount
        // typed in: its row id and that amount are what a stored-draft read
        // would bring back.
        for code in [
            "1601C", "2551Q", "0619E", "2550Q", "1701", "1702MX", "1702RT", "2000OT", "2200AN",
            "2200C", "2200S", "2552", "2553",
        ] {
            let period = page_period_of(code);
            let opened = open_for(TIN, code, year, period);
            let key = first_amount(code);
            let mut edits = Map::new();
            if code == "0619E" {
                edits.insert("frm0619E:optWithheld:Y".into(), json!("true"));
            }
            if let Some(key) = &key {
                edits.insert(key.clone(), json!("1234.00"));
            }
            let filled = fill(code, &opened["draft"], Value::Object(edits));
            let saved = save(code, &filled["draft"]);
            let stored = open_for(TIN, code, year, period);
            let typed = |values: &Value| {
                key.as_ref().is_some_and(|key| {
                    values[key].as_str().and_then(agent::parse_amount) == Some(1234.0)
                })
            };
            let by_id = stored["draft"]["id"] == saved["id"];
            assert!(by_id || typed(&stored["values"]), "{code}: stored");

            let shown = sample(code);
            assert!(
                shown["draft"]["id"].is_null() && !typed(&shown["values"]),
                "{code}: the sample read the stored return"
            );
            let again = open_for(TIN, code, year, period);
            assert_eq!(
                again["draft"]["id"], stored["draft"]["id"],
                "{code}: untouched"
            );
            assert_eq!(typed(&again["values"]), typed(&stored["values"]), "{code}");
        }
        // 0605's older rows (`form_drafts_v2`) are read when no generic row
        // exists; a sample reads neither.
        let opened = open_for(TIN, "0605", year, 1);
        let def = AgentFormKind::from_code("0605").unwrap().definition();
        let key = def
            .fillable()
            .find(|entry| {
                schema_keys("0605").contains(&entry.key)
                    && matches!(
                        def.bindings.get(&entry.key).map(|bound| &bound.binding),
                        Some(Binding::Amount { .. })
                    )
            })
            .expect("0605 has an amount box")
            .key
            .clone();
        let filled = fill("0605", &opened["draft"], json!({ &key: "1234.00" }));
        let typed: bir_core::forms::form_0605::Form0605Draft =
            serde_json::from_value(filled["draft"].clone()).unwrap();
        with_database(|db| {
            db.save_form_draft_v2(
                TIN,
                "0605",
                u16::try_from(year).unwrap(),
                &FilingPeriod::OpenEnded(1),
                &FilingStatus::Draft,
                &typed,
            )
            .map(|_| ())
            .map_err(|error| HostError::new(error.to_string()))
        })
        .unwrap();
        let stored = open_for(TIN, "0605", year, 1);
        assert_eq!(amount(&stored["values"], &key), 1234.0, "0605 v2 row");
        let shown = sample("0605");
        assert_ne!(
            shown["values"][&key].as_str().and_then(agent::parse_amount),
            Some(1234.0),
            "0605: the sample read the v2 row"
        );
    }

    /// The placeholder sample (its period not set) is never saved, queued
    /// or printed, and never replaces the stored return of the period it
    /// was built for. A sample rebuilt for a complete period saves as a
    /// return: a_library_sample_saves_as_its_periods_return_on_every_form.
    #[test]
    fn a_library_sample_is_never_saved_queued_or_printed() {
        setup();
        let year = sample_year();
        for code in ["1601C", "0619E"] {
            let period = page_period_of(code).max(1);
            let opened = open_for(TIN, code, year, period);
            let key = if code == "1601C" {
                "frm1601c:txtTax14"
            } else {
                "frm0619E:txtTax14"
            };
            let answers = if code == "0619E" {
                json!({"frm0619E:optWithheld:Y": "true", key: "1234.00"})
            } else {
                json!({ key: "1234.00" })
            };
            let filled = fill(code, &opened["draft"], answers);
            let saved = save(code, &filled["draft"]);

            // The sample's draft is for the same TIN, form, year and period.
            let shown = sample(code);
            assert_eq!(shown["year"], year, "{code}");
            assert_eq!(shown["period"], period, "{code}");
            // The mark survives the page's fill and validation.
            let filled = fill(code, &shown["draft"], json!({ key: "5.00" }));
            assert!(
                filled["draft"][SAMPLE_MARK].is_object(),
                "{code}: fill keeps the mark"
            );
            let validated = call_async(
                "screen_form_validate",
                vec![code.into(), json(filled["draft"].clone())],
            )
            .unwrap();
            assert!(
                validated["draft"][SAMPLE_MARK].is_object(),
                "{code}: validate keeps it"
            );
            for name in ["screen_form_save", "screen_form_print", "screen_form_queue"] {
                let refused = call(name, vec![code.into(), json(validated["draft"].clone())]);
                let error = refused.expect_err(name).to_string();
                assert!(error.contains("sample"), "{code} {name}: {error}");
            }
            let stored = open_for(TIN, code, year, period);
            assert_eq!(stored["draft"]["id"], saved["id"], "{code}");
            assert_eq!(
                amount(&stored["values"], key),
                1234.0,
                "{code}: not replaced"
            );
        }
    }

    /// The profile with the facts a filer may leave out left blank.
    fn install_sparse_profile() -> &'static str {
        const SPARSE_TIN: &str = "23456789000000";
        let profile: TaxpayerProfile = serde_json::from_value(json!({
            "id": null,
            "full_name": "Sparse Example Corp",
            "tin": {"segment1": "234", "segment2": "567", "segment3": "890", "branch": "00000"},
            "rdo_code": "",
            "line_of_business": "",
            "registered_address": "",
            "zip_code": "",
            "phone": "",
            "email": "",
            "default_form_type": "1601C",
            "taxpayer_type": "Corporation",
        }))
        .expect("sparse profile parses");
        with_database(|db| {
            db.save_profile(profile)
                .map(|_| ())
                .map_err(|error| HostError::new(error.to_string()))
        })
        .expect("sparse profile saves");
        SPARSE_TIN
    }

    /// Draft fields the sparse profile leaves blank and the full one fills.
    const SPARSE_BLANK_PATHS: &[&str] = &[
        "/rdo_code",
        "/line_of_business",
        "/registered_address",
        "/registered_address_lines/0",
        "/zip_code",
        "/contact_number",
        "/email",
        "/email_address",
    ];

    /// A value a box takes: another of its listed choices, else text.
    fn typed_value(opened: &Value, key: &str) -> String {
        let current = opened["values"][key].as_str().unwrap_or("");
        opened["options"][key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|option| option["value"].as_str())
            .find(|value| !value.is_empty() && *value != current)
            .map(str::to_string)
            .unwrap_or_else(|| other_text(current))
    }

    #[test]
    fn profile_boxes_lock_only_with_a_value_and_open_with_edit_on_every_form() {
        setup();
        let sparse = install_sparse_profile();
        let mut failures = Vec::new();
        let (mut blank_seen, mut filled_seen) = (0, 0);
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            let period = page_period_of(code);
            let on_screen = schema_keys(code);
            // The boxes bound to a fact the sparse profile leaves out.
            let facts: Vec<&String> = def
                .bindings
                .iter()
                .filter(|(key, bound)| {
                    on_screen.contains(*key)
                        && bound.lock == Some(agent::Lock::Profile)
                        && matches!(&bound.binding, Binding::Text { path, .. }
                            if SPARSE_BLANK_PATHS.contains(&path.as_str()))
                })
                .map(|(key, _)| key)
                .collect();
            let full = open_for(TIN, code, YEAR, period);
            let thin = open_for(sparse, code, YEAR, period);
            for key in &facts {
                let key = key.as_str();
                // Full profile: tucked, locked until Edit, and Edit opens it.
                filled_seen += 1;
                if full["locks"].get(key).is_none()
                    || !strings(&full["profile"]).contains(key)
                    || !strings(&full["profile_edit"]).contains(key)
                {
                    failures.push(format!(
                        "{code}: filled {key} is not a profile box with Edit"
                    ));
                }
                // Sparse profile: an ordinary box, typed like any other.
                blank_seen += 1;
                if thin["locks"].get(key).is_some()
                    || strings(&thin["profile"]).contains(key)
                    || strings(&thin["profile_edit"]).contains(key)
                {
                    failures.push(format!("{code}: blank {key} is locked or tucked"));
                }
                let value = typed_value(&thin, key);
                let typed = fill_unlocked(code, &thin["draft"], json!({ key: value }), &[key]);
                if let Some(reason) = typed["rejected"].get(key) {
                    failures.push(format!("{code}: blank {key} refused: {reason}"));
                }
            }
            // No filled profile box stays locked with no way to edit it.
            for key in strings(&full["profile"]) {
                if def.bindings.contains_key(&key)
                    && !strings(&full["profile_edit"]).contains(&key)
                    && def.box_def(&key).is_some_and(|e| e.required != "computed")
                {
                    failures.push(format!("{code}: {key} cannot be edited"));
                }
            }
            // No table cell is a profile box (the page's Edit is per item).
            for cell in table_cells(code) {
                if strings(&full["profile"]).contains(&cell) {
                    failures.push(format!("{code}: table cell {cell} is a profile box"));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
        assert!(
            blank_seen > 150 && filled_seen > 150,
            "{blank_seen} {filled_seen}"
        );
    }

    #[test]
    fn a_box_typed_where_the_profile_is_blank_stays_open_after_reopening() {
        setup();
        let sparse = install_sparse_profile();
        let (phone, zip) = ("frm2551Qv2018:telNo", "frm2551Qv2018:zipCode");
        let opened = open_for(sparse, "2551Q", YEAR, 3);
        assert_eq!(opened["values"][phone], "");
        assert!(opened["locks"].get(phone).is_none() && opened["locks"].get(zip).is_none());
        let filled = fill_unlocked(
            "2551Q",
            &opened["draft"],
            json!({ phone: "0281234567", zip: "1100" }),
            &[phone, zip],
        );
        assert!(
            filled["rejected"].as_object().unwrap().is_empty(),
            "{}",
            filled["rejected"]
        );
        save("2551Q", &filled["draft"]);
        let reopened = open_for(sparse, "2551Q", YEAR, 3);
        assert!(reopened["draft"]["id"].is_i64(), "the stored return");
        for (key, want) in [(phone, "0281234567"), (zip, "1100")] {
            assert_eq!(reopened["values"][key], want, "{key} kept");
            assert!(reopened["locks"].get(key).is_none(), "{key} still open");
            assert!(
                !strings(&reopened["profile"]).contains(key),
                "{key} not folded"
            );
            assert!(
                !strings(&reopened["profile_edit"]).contains(key),
                "{key} no Edit"
            );
        }
    }

    /// A text value of the same shape as `current`, but different.
    fn other_text(current: &str) -> String {
        if !current.is_empty() && current.chars().all(|c| c.is_ascii_digit()) {
            current
                .chars()
                .map(|c| char::from(b'0' + (c as u8 - b'0' + 1) % 10))
                .collect()
        } else {
            "EDITED ON THIS RETURN".to_string()
        }
    }

    /// The test profile changed later: a new RDO, address, ZIP, phone and
    /// email, as the filer changes them in the profile editor.
    fn change_profile() -> Value {
        with_database(|db| {
            let mut profile = db
                .get_profile_by_tin(TIN)
                .map_err(|error| HostError::new(error.to_string()))?
                .expect("profile");
            profile.rdo_code = "043".into();
            profile.registered_address = "2 New Street, Makati City".into();
            profile.zip_code = "1200".into();
            profile.phone = "0287654321".into();
            profile.email = "accounts@example.com".into();
            db.save_profile(profile)
                .map(|saved| serde_json::to_value(saved).unwrap())
                .map_err(|error| HostError::new(error.to_string()))
        })
        .unwrap()
    }

    /// Decided 2026-10-17: a stored Draft follows the current tax profile in
    /// every profile box the filer did not edit on it, quietly, when it
    /// opens. On every form: nothing moves while the profile is unchanged;
    /// after a change each profile box of the reopened draft shows what a
    /// brand-new return shows (the oracle), the change is saved, and the
    /// page lists what changed.
    #[test]
    fn a_stored_draft_follows_a_later_profile_change_on_every_form() {
        setup();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            save(code, &open(code, page_period_of(code))["draft"]);
            let again = open(code, page_period_of(code));
            assert_eq!(again["profile_updates"], json!([]), "{code}: nothing moved");
        }
        change_profile();
        let mut followed = 0;
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let reopened = open(code, page_period_of(code));
            let fresh = open_for(TIN, code, YEAR + 1, page_period_of(code));
            assert!(fresh["draft"]["id"].is_null(), "{code}: a brand-new return");
            for key in strings(&fresh["profile"]) {
                assert_eq!(
                    reopened["values"][&key], fresh["values"][&key],
                    "{code}: {key} follows the profile"
                );
            }
            let notes = strings(&reopened["profile_updates"]);
            if notes.iter().any(|note| note.starts_with("RDO 039 → 043")) {
                followed += 1;
            }
            // Saved: opening it once more changes nothing further.
            let third = open(code, page_period_of(code));
            assert_eq!(third["profile_updates"], json!([]), "{code}: saved");
            assert_eq!(third["values"], reopened["values"], "{code}");
        }
        assert!(followed >= 30, "{followed} forms say RDO 039 → 043");
    }

    /// Rewrite a stored 0619E row's JSON as another writer would.
    fn rewrite_row(id: i64, change: impl FnOnce(&mut Value)) {
        let (_, raw) = raw_row(id);
        let mut data: Value = serde_json::from_str(&raw).unwrap();
        change(&mut data);
        raw_exec(
            &format!("UPDATE form_drafts SET data_json = ?1 WHERE id = {id}"),
            &[&serde_json::to_string(&data).unwrap()],
        );
    }

    /// Only a profile CHANGE moves a box, and only a box that still holds
    /// what the profile gave it (its baseline). A draft saved before
    /// baselines existed changes nothing on open: it records the profile as
    /// it is then, and follows later changes from there. A value another
    /// writer put in a profile box without marking an edit (the desktop
    /// agent, an XML import) is never replaced.
    #[test]
    fn only_a_later_profile_change_moves_a_box_still_holding_the_profiles_value() {
        setup();
        let (code, rdo) = ("0619E", "frm0619E:txtRDOCode");
        let stored = |month: i64| {
            let id = save(code, &open(code, month)["draft"])["id"]
                .as_i64()
                .unwrap();
            let (_, raw) = raw_row(id);
            let data: Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(
                data["profile_baseline"]["/rdo_code"], "039",
                "saved with its baseline"
            );
            id
        };
        // A legacy draft (no baseline) whose RDO differs from the profile,
        // with no edit recorded.
        let legacy = stored(3);
        rewrite_row(legacy, |data| {
            data.as_object_mut().unwrap().remove("profile_baseline");
            data["rdo_code"] = json!("043");
        });
        let opened = open(code, 3);
        assert_eq!(
            opened["values"][rdo], "043",
            "kept: the profile did not change"
        );
        assert_eq!(opened["profile_updates"], json!([]));
        let (_, raw) = raw_row(legacy);
        let data: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(data["rdo_code"], "043");
        assert_eq!(
            data["profile_baseline"]["/rdo_code"], "039",
            "the baseline is recorded"
        );
        // A legacy draft equal to the profile.
        let equal = stored(5);
        rewrite_row(equal, |data| {
            data.as_object_mut().unwrap().remove("profile_baseline");
        });
        assert_eq!(
            open(code, 5)["profile_updates"],
            json!([]),
            "no change on open"
        );
        // A value another writer put in, no edit marked.
        let written = stored(6);
        rewrite_row(written, |data| data["rdo_code"] = json!("050"));

        with_database(|db| {
            let mut profile = db
                .get_profile_by_tin(TIN)
                .map_err(|error| HostError::new(error.to_string()))?
                .expect("profile");
            profile.rdo_code = "044".into();
            db.save_profile(profile)
                .map(|_| ())
                .map_err(|error| HostError::new(error.to_string()))
        })
        .unwrap();
        let first = open(code, 3);
        assert_eq!(
            first["values"][rdo], "043",
            "a divergent legacy value stays"
        );
        let second = open(code, 5);
        assert_eq!(second["values"][rdo], "044", "follows the later change");
        assert!(strings(&second["profile_updates"]).contains("RDO 039 → 044"));
        let third = open(code, 6);
        assert_eq!(
            third["values"][rdo], "050",
            "another writer's value survives"
        );
        assert_eq!(third["profile_updates"], json!([]));
    }

    /// The quiet save on open never makes a draft unopenable: with the
    /// store refusing writes, a draft that would record its baseline (and
    /// one that would follow a change) still opens, as it is in memory.
    #[test]
    fn a_draft_opens_when_its_quiet_save_fails() {
        setup();
        let (code, rdo) = ("0619E", "frm0619E:txtRDOCode");
        let id = save(code, &open(code, 3)["draft"])["id"].as_i64().unwrap();
        rewrite_row(id, |data| {
            data.as_object_mut().unwrap().remove("profile_baseline");
        });
        let before = raw_row(id);
        raw_exec("PRAGMA query_only = ON", &[]);
        let opened = call_async(
            "screen_form_open",
            vec![TIN.into(), code.into(), YEAR.into(), 3.into()],
        );
        raw_exec("PRAGMA query_only = OFF", &[]);
        let opened = opened.expect("opens although the save failed");
        assert_eq!(opened["draft"]["id"], id);
        assert_eq!(opened["values"][rdo], "039");
        assert_eq!(
            opened["draft"]["profile_baseline"]["/rdo_code"], "039",
            "the baseline is kept in memory"
        );
        assert_eq!(raw_row(id), before, "nothing was written");
    }

    /// A box the filer edited on the return keeps the edit when the profile
    /// changes; the unedited ones follow, the note says what changed, and
    /// the profile never takes the return's edit (Rule 0). A queued return
    /// never changes.
    #[test]
    fn a_profile_change_keeps_the_returns_own_edits_and_never_moves_a_queued_return() {
        setup();
        let code = "1601C";
        let (address, rdo, email) = ("frm1601c:txtAddress", "frm1601c:txtRDOCode", "txtEmail");
        let opened = open(code, 3);
        assert_eq!(opened["values"][rdo], "039");
        let edited = "9 EDITED ROAD, PASIG CITY";
        let filled = fill_unlocked(
            code,
            &opened["draft"],
            json!({ address: edited }),
            &[address],
        );
        assert_eq!(filled["rejected"], json!({}));
        assert_eq!(
            profile_edits(&filled["draft"]),
            BTreeSet::from([address.to_string()]),
            "the Edit is recorded on the return"
        );
        save(code, &filled["draft"]);
        // A queued return, from before the profile changes.
        let queued = open("0619E", 4);
        let queued_id = store_filed("0619E", &queued["draft"], "Queued");
        let queued_row = raw_row(queued_id);

        let profile = change_profile();
        let reopened = open(code, 3);
        assert_eq!(reopened["values"][rdo], "043", "the unedited RDO follows");
        assert_eq!(reopened["values"][email], "accounts@example.com");
        assert!(
            reopened["values"][address]
                .as_str()
                .is_some_and(|shown| shown.eq_ignore_ascii_case(edited)),
            "the edited address is kept: {}",
            reopened["values"][address]
        );
        let notes = strings(&reopened["profile_updates"]);
        assert!(notes.contains("RDO 039 → 043"), "{notes:?}");
        assert!(
            notes.contains("Email address books@example.com → accounts@example.com"),
            "{notes:?}"
        );
        assert!(
            !notes
                .iter()
                .any(|note| note.starts_with("Registered Address")),
            "{notes:?}"
        );
        // Saved quietly: the stored row has it, and the edit is still marked.
        let stored = call(
            "get_form_draft",
            vec![TIN.into(), code.into(), YEAR.into(), 3.into()],
        )
        .unwrap();
        assert_eq!(stored["rdo_code"], "043", "{stored}");
        assert_eq!(stored["profile_edits"], json!([address]));
        // Rule 0: the profile keeps its own address.
        assert_eq!(profile["registered_address"], "2 New Street, Makati City");
        let after = with_database(|db| {
            db.get_profile_by_tin(TIN)
                .map_err(|error| HostError::new(error.to_string()))
        })
        .unwrap()
        .unwrap();
        assert_eq!(after.registered_address, "2 New Street, Makati City");
        // The queued return: untouched, row and page alike.
        let shown = open("0619E", 4);
        assert_eq!(shown["status"], "Queued");
        assert_eq!(
            shown["values"]["frm0619E:txtRDOCode"], "039",
            "a queued return never changes"
        );
        assert_eq!(shown["profile_updates"], json!([]));
        assert_eq!(
            raw_row(queued_id),
            queued_row,
            "its stored row is unchanged"
        );
    }

    #[test]
    fn an_edited_profile_box_stays_on_this_return_and_never_reaches_the_profile() {
        setup();
        let before = with_database(|db| {
            db.get_profile_by_tin(TIN)
                .map_err(|error| HostError::new(error.to_string()))
        })
        .unwrap()
        .expect("profile");
        let before = serde_json::to_value(&before).unwrap();
        let mut failures = Vec::new();
        let (mut edited, mut selects) = (0, 0);
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            let period = page_period_of(code);
            let opened = open(code, period);
            let mut edits = Map::new();
            let mut by_path: BTreeMap<String, String> = BTreeMap::new();
            for key in strings(&opened["profile_edit"]) {
                let text = matches!(
                    def.bindings.get(&key).map(|bound| &bound.binding),
                    Some(Binding::Text { date: None, .. })
                );
                let shape = def
                    .box_def(&key)
                    .map(|e| e.kind.clone())
                    .unwrap_or_default();
                if text && (shape == "text" || shape.contains("select")) {
                    selects += usize::from(shape.contains("select"));
                    // Two boxes of one draft field (an RDO drawn as a select
                    // and as text) take one value.
                    let path = bound_paths(def, &key).first().map(|p| p.to_string());
                    let value = path
                        .as_ref()
                        .and_then(|path| by_path.get(path).cloned())
                        .unwrap_or_else(|| typed_value(&opened, &key));
                    if let Some(path) = path {
                        by_path.insert(path, value.clone());
                    }
                    edits.insert(key.clone(), Value::String(value));
                }
            }
            if edits.is_empty() {
                continue;
            }
            let keys: Vec<&str> = edits.keys().map(String::as_str).collect();
            // Without Edit the page's fill keeps refusing a profile-locked box.
            let refused = fill(code, &opened["draft"], Value::Object(edits.clone()));
            for key in &keys {
                if has_profile_lock(def, key) && refused["rejected"].get(*key).is_none() {
                    failures.push(format!("{code}: {key} was written without Edit"));
                }
            }
            let filled = fill_unlocked(code, &opened["draft"], Value::Object(edits.clone()), &keys);
            for (key, reason) in filled["rejected"].as_object().unwrap() {
                failures.push(format!("{code}: {key} refused after Edit: {reason}"));
            }
            save(code, &filled["draft"]);
            let reopened = open(code, period);
            for (key, value) in &edits {
                let want = value.as_str().unwrap();
                let shown = reopened["values"][key].as_str().unwrap_or("");
                if shown.trim().eq_ignore_ascii_case(want) {
                    edited += 1;
                } else {
                    failures.push(format!("{code}: {key} reopened as `{shown}`, not `{want}`"));
                }
                // Still the profile's box: folded, locked until Edit again.
                if reopened["locks"].get(key).is_none()
                    || !strings(&reopened["profile_edit"]).contains(key)
                {
                    failures.push(format!("{code}: edited {key} lost its profile look"));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
        assert!(edited > 43, "{edited} edited profile boxes");
        assert!(selects > 5, "{selects} RDO selects edited");
        // Rule 0: the return's edits never reach the profile.
        let after = with_database(|db| {
            db.get_profile_by_tin(TIN)
                .map_err(|error| HostError::new(error.to_string()))
        })
        .unwrap()
        .expect("profile");
        assert_eq!(serde_json::to_value(&after).unwrap(), before);
    }

    /// Profile boxes the page still leaves in an "Additional details" group,
    /// out of paper order: placing them is each form's own fix (rule 3),
    /// so those forms' R2.0 stays open. A new one fails this test.
    const PROFILE_BOXES_NOT_YET_PLACED: &[&str] = &[
        "0605 txtEmail",
        "1604E frm1604e:txtLineBus",
        "1604E frm1604e:txtPg2TaxpayerName",
        "1604F frm1604f:txtLineBus",
        "1604F frm1604f:txtPg2TaxpayerName",
        "1701 frm1701:txtLineBus",
        "1701 frm1701:txtPg1mTaxpayerName",
        "1701 frm1701:txtPg2mTaxpayerName",
        "1701 frm1701:txtPg2TaxpayerName",
        "1701 frm1701:txtPg3mTaxpayerName",
        "1701 frm1701:txtPg3TaxpayerName",
        "1701 frm1701:txtPg4mTaxpayerName",
        "1701 frm1701:txtPg4TaxpayerName",
        "1701A frm1701A:txtLineBus",
        "1701A frm1701A:txtPg2TaxpayerName",
        "1701MS frm1701MS:Pg2TaxPayer",
        "1701MS frm1701MS:txtRDOCodea",
        "1701MS frm1701MS:txtTelNum10a",
        "1702EX frm1702EX:txtLOB",
        "1702EX frm1702EX:txtPg2RegisteredName",
        "1702EX frm1702EX:txtPg3RegisteredName",
        "1707 frm1707:ProfileAddress1",
        "1707 frm1707:ProfileContactNum",
        "1707 frm1707:ProfileEmailAddr",
        "1707 frm1707:ProfileName",
        "1707 frm1707:ProfileZipCode",
        "1707 frm1707:txtLOB",
        "1707 frm1707:txtPg2RegisteredName",
        "1800 frm1800v2018:txtLineBus",
        "1800 frm1800v2018:txtPg2TaxpayerName",
        "2000 frm2000:txtLineBus",
        "2000 frm2000:txtPg2TaxpayerName",
        "2000 frm2000:txtTelNum",
        "2000 txtEmail",
        "2000OT frm2000OT:txtLineBus",
        "2200C frm2200C:txtLOB",
        "2200M frm2200M:txtPg2RegisteredName",
        "2200P frm2200P:txtLOB",
        "2200P frm2200P:txtPg1Pt1I8ContactNumber",
        "2200P frm2200P:txtPg1Pt1I9Email",
        "2200P frm2200P:txtPg2RegisteredName",
        "2200T frm2200Tv2020:registeredName_2",
        "2200T frm2200Tv2020:registeredName_3",
        "2550Q frm2550qv2024:Pg2TaxPayer",
        "2552 frm2552:txtLOB",
    ];

    #[test]
    fn every_filled_profile_box_is_folded_or_offers_edit_in_paper_order() {
        setup();
        let mut unplaced = Vec::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let opened = open(code, page_period_of(code));
            let editable = strings(&opened["profile_edit"]);
            let form = schema_entry(code).expect("schema entry");
            let items = form["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|part| part["items"].as_array().into_iter().flatten());
            for item in items {
                let label = item["l"].as_str().unwrap_or("");
                for field in item["f"].as_array().into_iter().flatten() {
                    let key = field["k"].as_str().unwrap_or("");
                    if editable.contains(key)
                        && field["t"] != "hidden"
                        && label == "Additional details"
                    {
                        unplaced.push(format!("{code} {key}"));
                    }
                }
            }
        }
        unplaced.sort();
        let mut expected = PROFILE_BOXES_NOT_YET_PLACED.to_vec();
        expected.sort_unstable();
        assert_eq!(unplaced, expected, "{unplaced:#?}");
    }

    #[test]
    fn a_box_the_profile_filled_stays_locked_without_edit_on_every_form() {
        setup();
        let mut failures = Vec::new();
        let (mut checked, mut reported) = (0, 0);
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            let period = page_period_of(code);
            // The draft the profile alone makes, read straight from its fields.
            let alone = with_database(|db| {
                let profile = db.get_profile_by_tin(TIN).unwrap().expect("profile");
                let period = page_period(code, period).map_err(HostError::new)?;
                load_draft(db, kind, &profile, YEAR as u16, period, true, None)
                    .map_err(HostError::new)
            })
            .unwrap();
            let opened = open(code, period);
            let on_screen = schema_keys(code);
            for (key, bound) in &def.bindings {
                let path = match &bound.binding {
                    Binding::Text { path, .. }
                    | Binding::Amount { path, .. }
                    | Binding::Whole { path, .. } => path,
                    Binding::Choice { .. } => continue,
                };
                let filled = match alone.pointer(path) {
                    Some(Value::String(text)) => !text.trim().chars().all(|c| c == '0'),
                    Some(Value::Number(number)) => number.as_f64() != Some(0.0),
                    _ => false,
                };
                if bound.lock != Some(agent::Lock::Profile) || !on_screen.contains(key) || !filled {
                    continue;
                }
                checked += 1;
                if opened["locks"].get(key).is_none() {
                    failures.push(format!("{code}: {key} ({path}) is open without Edit"));
                }
                let typed = fill(code, &opened["draft"], json!({ key: "SOMEONE ELSE INC" }));
                if typed["rejected"].get(key).is_none() {
                    failures.push(format!("{code}: {key} ({path}) took an edit without Edit"));
                }
            }
            // Every box the host reports as the profile's, core-locked or
            // matched by key (the filer's own name), refuses an edit too.
            let profile_reason = agent::Lock::Profile.reason();
            for (key, reason) in opened["locks"].as_object().unwrap() {
                if reason.as_str() != Some(profile_reason) {
                    continue;
                }
                reported += 1;
                let typed = fill(code, &opened["draft"], json!({ key: "SOMEONE ELSE INC" }));
                if typed["rejected"].get(key).is_none() {
                    failures.push(format!("{code}: reported {key} took an edit without Edit"));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
        assert!(checked > 200, "{checked} profile boxes checked");
        assert!(reported > checked, "{reported} reported profile boxes");
    }

    /// Pre-existing boxes that are locked and blank with no visible reason,
    /// each with the form group that fixes it (`CODE key: reason | PR n`).
    const LOCKED_AND_BLANK_NOT_YET_FIXED: &str =
        include_str!("screen_form_module/locked_and_blank.txt");

    /// Every on-screen box with its schema type and computed flag.
    fn screen_fields(code: &str) -> Vec<(String, String, bool)> {
        let form = schema_entry(code).expect("schema entry");
        let mut out = Vec::new();
        let items = form["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|part| part["items"].as_array().into_iter().flatten());
        for item in items {
            for field in item["f"].as_array().into_iter().flatten() {
                out.push((
                    field["k"].as_str().unwrap_or("").to_string(),
                    field["t"].as_str().unwrap_or("").to_string(),
                    field["c"].as_bool().unwrap_or(false),
                ));
            }
            let cols = item["cols"].as_array();
            for row in item["keys"].as_array().into_iter().flatten() {
                for (c, key) in row.as_array().into_iter().flatten().enumerate() {
                    let col = cols.and_then(|cols| cols.get(c));
                    out.push((
                        key.as_str().unwrap_or("").to_string(),
                        col.and_then(|col| col["t"].as_str())
                            .unwrap_or("")
                            .to_string(),
                        col.and_then(|col| col["c"].as_bool()).unwrap_or(false),
                    ));
                }
            }
        }
        out
    }

    /// Boxes that are locked AND blank with no visible reason. A locked box
    /// must show its value (computed, profile, a real return's period) or
    /// say what turns it on (`needs`). Grouped by the boxes' own form group.
    /// None may be silent: each one's lock note (`lock_notes`, drawn under
    /// its item by form_page.js `lockNotesFor`) must be filer words; one
    /// that is not is marked SILENT, which no pinned line can match.
    fn locked_and_blank(opened: &Value, code: &str, ticks: bool) -> Vec<String> {
        let mut out = Vec::new();
        let locked = |key: &str| opened["locks"][key].as_str();
        let ticked = |key: &str| opened["values"][key].as_str() == Some("true");
        // A reason counts where the page draws it: in the box, beside a tick
        // box, under a radio group (form_page.js `reasonFor`).
        let reason = |key: &str| opened["needs"].get(key).is_some();
        let drawn = |key: &str| {
            opened["lock_notes"][key].as_str().is_some_and(|note| {
                !note.is_empty()
                    && !note.contains('`')
                    && !note.contains('/')
                    && !note.contains("frm")
                    && !note.contains("field map")
                    && Some(note) != locked(key)
            })
        };
        let mut out_push = |key: &str, entry: String| {
            out.push(if drawn(key) {
                entry
            } else {
                format!("{entry} SILENT: no lock note drawn")
            });
        };
        let form = schema_entry(code).expect("schema entry");
        let items = form["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|part| part["items"].as_array().into_iter().flatten());
        let mut boxes: Vec<(String, String)> = Vec::new();
        for item in items {
            let fields: Vec<(String, String)> = item["f"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|f| {
                    (
                        f["k"].as_str().unwrap_or("").to_string(),
                        f["t"].as_str().unwrap_or("").to_string(),
                    )
                })
                .collect();
            // A radio group is one box: shut and blank when every radio is
            // locked and none is ticked.
            let radios: Vec<&String> = fields
                .iter()
                .filter(|(_, t)| t == "radio")
                .map(|(k, _)| k)
                .collect();
            if ticks
                && !radios.is_empty()
                && radios.iter().all(|k| locked(k).is_some())
                && !radios.iter().any(|k| ticked(k))
                && !radios.iter().any(|k| reason(k))
            {
                out_push(
                    radios[0],
                    format!(
                        "{code} {} (radio group): {}",
                        radios[0],
                        locked(radios[0]).unwrap_or("")
                    ),
                );
            }
            boxes.extend(fields.into_iter().filter(|(_, t)| t != "radio"));
            let cols = item["cols"].as_array();
            for row in item["keys"].as_array().into_iter().flatten() {
                for (c, key) in row.as_array().into_iter().flatten().enumerate() {
                    let t = cols
                        .and_then(|cols| cols.get(c))
                        .and_then(|col| col["t"].as_str())
                        .unwrap_or("");
                    let t = if t == "radio" { "chk" } else { t };
                    boxes.push((key.as_str().unwrap_or("").to_string(), t.to_string()));
                }
            }
        }
        for (key, t) in boxes {
            let Some(why) = locked(&key) else {
                continue;
            };
            if t == "hidden" || reason(&key) {
                continue;
            }
            if t == "chk" {
                if ticks && !ticked(&key) {
                    out_push(&key, format!("{code} {key} (tick box): {why}"));
                }
                continue;
            }
            // A computed amount shows 0.00 (form_page.js `isComputedBox`).
            let computed = why.starts_with("computed");
            let shown = opened["values"][&key].as_str().unwrap_or("").trim();
            if shown.is_empty() && !(computed && t == "money") {
                out_push(&key, format!("{code} {key}: {why}"));
            }
        }
        out
    }

    #[test]
    fn a_lock_note_is_filer_words() {
        use bir_core::forms::agent::Lock;
        assert_eq!(lock_note("computed by BIR's own formulas"), NOTE_COMPUTED);
        assert_eq!(lock_note("computed from the year-end month"), NOTE_COMPUTED);
        assert_eq!(lock_note(Lock::Profile.reason()), NOTE_PROFILE);
        assert_eq!(lock_note(Lock::Identity.reason()), NOTE_IDENTITY);
        assert_eq!(lock_note("repeats `frm1:txtA`; fill it there"), NOTE_REPEAT);
        assert_eq!(
            lock_note("repeats the official field `txtEmail`, which this page cannot write"),
            NOTE_NOT_YET
        );
        assert_eq!(
            lock_note(
                "cannot be filled here: not written by this app's field map for any sample draft"
            ),
            NOTE_NOT_YET
        );
        // "It will be added" only where a form group is assigned to open
        // the box: 2551Q's repeated email is (PR 1).
        assert!(assigned_to_open("2551Q", "txtEmail#occurrence-1"));
        assert!(assigned_to_open("0605", "itemQuarter_1"));
        assert!(!assigned_to_open("2551Q", "txtEmail"));
        assert!(!assigned_to_open("0619E", "txtEmail#occurrence-1"));
        assert_eq!(
            lock_note_for(
                "2551Q",
                "txtEmail#occurrence-1",
                "repeats the official field `txtEmail`, which this page cannot write"
            ),
            NOTE_ASSIGNED
        );
        assert_eq!(
            lock_note_for(
                "0619E",
                "frm0619E:txtNowhere",
                "repeats the official field `txtX`, which this page cannot write"
            ),
            NOTE_NOT_YET
        );
        assert_eq!(
            lock_note_for(
                "2551Q",
                "txtEmail#occurrence-1",
                "computed by BIR's own formulas"
            ),
            NOTE_COMPUTED
        );
        // 2551Q's repeated email: shut, blank, and now saying why.
        setup();
        let opened = open_for(TIN, "2551Q", YEAR, page_period_of("2551Q"));
        assert_eq!(opened["lock_notes"]["txtEmail#occurrence-1"], NOTE_ASSIGNED);
    }

    #[test]
    fn no_box_is_locked_and_blank_without_a_reason_on_every_form() {
        setup();
        let sparse = install_sparse_profile();
        let mut found = BTreeSet::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let period = page_period_of(code);
            for opened in [
                open_for(TIN, code, YEAR, period),
                open_for(sparse, code, YEAR, period),
            ] {
                found.extend(locked_and_blank(&opened, code, true));
            }
            // A sample, before and after its filer sets the period.
            found.extend(locked_and_blank(&sample(code), code, true));
            let set = sample_period(TIN, code, typed_parts(code, 2024, 3, 15, 1));
            found.extend(locked_and_blank(&set["sample"], code, true));
        }
        let found: Vec<String> = found.into_iter().collect();
        let mut expected: Vec<String> = LOCKED_AND_BLANK_NOT_YET_FIXED
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .map(|line| {
                line.rsplit_once(" | ")
                    .map_or(line, |(entry, _)| entry)
                    .to_string()
            })
            .collect();
        expected.sort();
        let new: Vec<&String> = found.iter().filter(|e| !expected.contains(e)).collect();
        let fixed: Vec<&String> = expected.iter().filter(|e| !found.contains(e)).collect();
        assert!(
            new.is_empty() && fixed.is_empty(),
            "locked and blank, not pinned: {new:#?}\npinned but fixed (remove the line): {fixed:#?}"
        );
    }

    /// Email boxes a profile without an email still leaves shut, and why:
    /// none of them is a profile lock (those open when the profile is
    /// blank); each is a lock of its own form's, for its group to fix.
    const EMAIL_BOXES_SHUT_FOR_OTHER_REASONS: &[&str] = &[
        "1701MS frm1701MS:txtEmaila: cannot be filled here: derived by the app: shows /email combined with /foreign_tax_credits_description",
        "1701MS frm1701MS:txtEmailb: cannot be filled here: derived by the app: shows /spouse_info/email combined with /foreign_tax_credits_description",
        "2551Q txtEmail#occurrence-1: repeats the official field `txtEmail`, which this page cannot write",
    ];

    #[test]
    fn an_email_box_the_profile_leaves_blank_is_open_on_every_form() {
        setup();
        let sparse = install_sparse_profile();
        let profile_reason = agent::Lock::Profile.reason();
        let mut shut = Vec::new();
        let mut open_boxes = 0;
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let opened = open_for(sparse, code, YEAR, page_period_of(code));
            for (key, t) in screen_types(code) {
                if !key.to_ascii_lowercase().contains("email") || t == "hidden" {
                    continue;
                }
                match opened["locks"][&key].as_str() {
                    None => open_boxes += 1,
                    Some(reason) => {
                        assert!(
                            !reason.contains(profile_reason)
                                && !reason.contains("from the tax profile"),
                            "{code}: {key} keeps a profile lock: {reason}"
                        );
                        shut.push(format!("{code} {key}: {reason}"));
                    }
                }
            }
        }
        shut.sort();
        let mut expected = EMAIL_BOXES_SHUT_FOR_OTHER_REASONS.to_vec();
        expected.sort_unstable();
        assert_eq!(shut, expected, "{shut:#?}");
        assert!(open_boxes > 30, "{open_boxes} open email boxes");
    }

    #[test]
    fn a_text_box_the_schema_typed_as_money_is_text_on_every_form() {
        let mut checked = 0;
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            let form = schema_entry(code).unwrap();
            let fields = form["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|part| part["items"].as_array().into_iter().flatten())
                .flat_map(|item| item["f"].as_array().into_iter().flatten());
            for field in fields {
                let key = field["k"].as_str().unwrap_or("");
                if let Some(Binding::Text { .. }) = def.bindings.get(key).map(|b| &b.binding) {
                    checked += 1;
                    assert_ne!(field["t"], "money", "{code}: {key} is a text field");
                }
            }
        }
        assert!(checked > 500, "{checked}");
        // Table cells too: a text-bound cell is never drawn as pesos.
        let mut cells = 0;
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            let table = table_cells(code);
            for (key, t, _) in screen_fields(code) {
                if table.contains(&key)
                    && matches!(
                        def.bindings.get(&key).map(|b| &b.binding),
                        Some(Binding::Text { .. })
                    )
                {
                    cells += 1;
                    assert_ne!(t, "money", "{code}: cell {key} is text");
                }
            }
        }
        assert!(cells > 50, "{cells} text cells");
        let tax_bracket = screen_fields("2200S")
            .into_iter()
            .find(|(key, _, _)| key.ends_with("txtsched1TaxBracket0"))
            .expect("2200S tax bracket cell");
        assert_eq!(tax_bracket.1, "text");
        // The ones the review found drawn as peso boxes.
        for (code, key) in [
            ("2550Q", "frm2550qv2024:taxpayerContactNumber"),
            ("2200C", "frm2200C:txtPg1Pt1I9ContactNumber"),
            ("1701MS", "frm1701MS:txtTaxpayerNo8a"),
        ] {
            let form = schema_entry(code).unwrap();
            let field = form["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|part| part["items"].as_array().into_iter().flatten())
                .flat_map(|item| item["f"].as_array().into_iter().flatten())
                .find(|field| field["k"] == key)
                .unwrap_or_else(|| panic!("{code}: {key} on screen"));
            assert_eq!(field["t"], "text", "{code}: {key}");
        }
    }

    /// Every visible box with a binding, with its page type and whole flag
    /// (fields and schedule cells).
    fn bound_page_types(code: &str) -> Vec<(String, String, bool)> {
        let def = AgentFormKind::from_code(code).unwrap().definition();
        let form = schema_entry(code).unwrap();
        let mut out = Vec::new();
        let items = form["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|part| part["items"].as_array().into_iter().flatten());
        for item in items {
            for field in item["f"].as_array().into_iter().flatten() {
                out.push((
                    field["k"].as_str().unwrap_or("").to_string(),
                    field["t"].as_str().unwrap_or("").to_string(),
                    field["w"].as_bool().unwrap_or(false),
                ));
            }
            let cols = item["cols"].as_array();
            for row in item["keys"].as_array().into_iter().flatten() {
                for (c, key) in row.as_array().into_iter().flatten().enumerate() {
                    let col = cols.and_then(|cols| cols.get(c));
                    out.push((
                        key.as_str().unwrap_or("").to_string(),
                        col.and_then(|col| col["t"].as_str())
                            .unwrap_or("")
                            .to_string(),
                        col.and_then(|col| col["w"].as_bool()).unwrap_or(false),
                    ));
                }
            }
        }
        out.retain(|(key, t, _)| t != "hidden" && def.bindings.contains_key(key));
        out
    }

    /// Rule 7: only an amount looks like money. Every form: a text binding is
    /// never money, an amount binding is a peso box unless it holds a count
    /// (a number box) or a tax rate (a percent box), and a peso box over a
    /// whole-number binding shows no centavos. The counts, rates and amounts
    /// the review named are checked against a hand list, not the rule.
    #[test]
    fn only_an_amount_binding_looks_like_money_on_every_form() {
        let (mut amounts, mut wholes) = (0, 0);
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            for (key, t, whole) in bound_page_types(code) {
                match &def.bindings[&key].binding {
                    Binding::Amount { .. } => {
                        amounts += 1;
                        assert!(
                            matches!(t.as_str(), "money" | "pct" | "int"),
                            "{code}: {key} is an amount drawn as {t}"
                        );
                        assert!(!whole, "{code}: {key} takes centavos");
                    }
                    Binding::Text { .. } => assert_ne!(t, "money", "{code}: {key} is text"),
                    Binding::Whole { .. } => {
                        if t == "money" {
                            wholes += 1;
                        }
                        assert_eq!(whole, t == "money", "{code}: {key} ({t})");
                    }
                    Binding::Choice { .. } => assert_ne!(t, "money", "{code}: {key}"),
                }
            }
        }
        assert!(amounts > 2500, "{amounts} amount boxes");
        assert!(wholes > 200, "{wholes} whole-peso boxes");
        // The ones the review found, each way (fields and schedule cells).
        for (code, key, t) in [
            // Peso amounts the schema drew as text.
            ("1606", "frm1606:txtInterest", "money"),
            ("1606", "frm1606:txtInstallment", "money"),
            ("1706", "frm1706:txtInterest", "money"),
            ("1706", "frm1706:txtInstallment", "money"),
            // Text the schema drew as money.
            ("1701", "frm1701:txtPg1I14ForeignTaxNumber", "text"),
            ("1701A", "frm1701A:txtForeignTaxNumber", "text"),
            ("1701MS", "frm1701MS:txtTaxpayerNo14a", "text"),
            ("1701MS", "frm1701MS:txtTaxpayerNo15a", "text"),
            ("1800", "frm1800v2018:TaxTreaty", "text"),
            ("1801", "frm1801v2018taxkNum", "text"),
            ("2200T", "frm2200Tv2020:taxNum", "text"),
            ("2200A", "frm2200Av2020:paymentOther", "text"),
            ("1702RT", "frm1702RT:txtPg1Pt3I23DebitMemoC2", "text"),
            ("1707", "frm1707:txtPg2P4Sch1I4NumInstallments", "text"),
            // Counts: plain numbers.
            ("0605", "frm0605:txtNumOfInstallment", "int"),
            ("1702EX", "frm1702EX:txtPg1P2I23NumOfAttachments", "int"),
            ("1606", "frm1606:txtTotalN", "int"),
            ("1706", "frm1706:txtTotalN", "int"),
            ("1707", "frm1707:txtPg2P4Sch2NoOfShares2", "int"),
            ("1707", "frm1707:txtPg2Pt4S2_1Col2", "int"),
            ("1707", "frm1707:txtPg2Pt4S2_10Col2", "int"),
            ("2552", "frm2552:txtPg2Pt5Sch1NumShares1", "int"),
            ("2200M", "frm2200M:txtSched1_VOMRITaxableA1", "int"),
            ("2200M", "frm2200M:txtSched1_VOMRIExemptB1", "int"),
            ("2200S", "frm2200S:txtVolumeRemovals0", "int"),
            // Tax rates: percent.
            ("1702RT", "frm1702RT:Pg2Pt4I40IncomeTaxRate", "pct"),
            ("1702EX", "frm1702EX:txtPg2Pt4I40TaxRate", "pct"),
            ("1600PT", "frm1600PT:Pg2Sc1TaxRate1", "pct"),
            ("1600VT", "frm1600VT:Pg2Sc1TaxRate1", "pct"),
            ("1601FQ", "txtRate0", "pct"),
            ("1602Q", "frm1602Q:txtSched2TaxRate1", "pct"),
            ("1602Q", "frm1602Q:txtSched3TaxRate1", "pct"),
            ("2552", "frm2552:txtPg2Pt5Sch2TaxRate1", "pct"),
            // Rates in pesos stay pesos: specific excise per unit, an
            // exchange rate.
            ("2200M", "frm2200M:txtSched1_ImportedTaxRate9", "money"),
            (
                "2200M",
                "frm2200M:txtSched1_LocallyExtractedTaxRateE13",
                "money",
            ),
            ("2200P", "frm2200P:txtSched1ApplicableRate1", "money"),
            ("1602Q", "frm1602Q:txtSched1Rate13", "money"),
            // Zero-rated sales are an amount.
            ("2550Q", "frm2550qv2024:zeroRatedSales", "money"),
        ] {
            let found = bound_page_types(code)
                .into_iter()
                .find(|(k, _, _)| k == key)
                .unwrap_or_else(|| panic!("{code}: {key} on screen"));
            assert_eq!(found.1, t, "{code}: {key}");
        }
    }

    /// Rule 4, host side: a text binding that keeps a whole date is a date
    /// box on the page (the binding data is the oracle), and every date box
    /// a return leaves open takes the MM/DD/YYYY the page sends for
    /// "12102020", "12-10-2020" or "12/10/2020" and keeps it in the form's own
    /// format. Date boxes locked for another reason are pinned with it.
    #[test]
    fn every_open_date_box_takes_a_typed_date_on_every_form() {
        use bir_core::forms::agent::DateFormat;
        setup();
        // Dates whose binding carries no date format, read off the printed
        // sheets by hand: each is a date box.
        for (code, key) in [
            ("2200T", "frm2200Tv2020:cashDate"),
            ("2200T", "frm2200Tv2020:checkDate"),
            ("2200T", "frm2200Tv2020:taxDate"),
            ("2200T", "frm2200Tv2020:othersDate"),
            ("1702MX", "frm1702MX:txtPg2Pt4I35CA"),
            ("1702MX", "frm1702MX:txtPg2Pt4I35CC"),
            ("1702MX", "frm1702MX:txtPg2Pt4I36CA"),
            ("1702MX", "frm1702MX:txtPg2Pt4I36CC"),
            ("2550Q", "frm2550qv2024:RtnPeriodFromNo4"),
            ("2550Q", "frm2550qv2024:RtnPeriodToNo4"),
        ] {
            let t = screen_types(code)
                .into_iter()
                .find(|(k, _)| k == key)
                .unwrap_or_else(|| panic!("{code}: {key} on screen"))
                .1;
            assert_eq!(t, "date", "{code}: {key}");
        }
        let mut locked = Vec::new();
        let (mut typed, mut refused) = (0, 0);
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            for (key, t, _) in bound_page_types(code) {
                if let Binding::Text {
                    date: Some(DateFormat::Mdy | DateFormat::Iso),
                    part: None,
                    ..
                } = &def.bindings[&key].binding
                {
                    assert_eq!(t, "date", "{code}: {key} keeps a whole date");
                }
            }
            let opened = open(code, page_period_of(code));
            for (key, t) in screen_types(code) {
                if t != "date" {
                    continue;
                }
                if let Some(reason) = opened["locks"][&key].as_str() {
                    locked.push(format!("{code} {key}: {reason}"));
                    continue;
                }
                let filled = fill(code, &opened["draft"], json!({ &key: "12/10/2020" }));
                if let Some(reason) = filled["rejected"][&key].as_str() {
                    // Refused by its form's own rule (a later schedule row
                    // before the first, a schedule another answer turns
                    // off), never for the date.
                    assert!(!reason.contains("date"), "{code}: {key}: {reason}");
                    refused += 1;
                    continue;
                }
                let shown = filled["values"][&key].as_str().unwrap_or("");
                if shown != "12/10/2020" {
                    // Taken, then cleared by the form's own computation.
                    locked.push(format!("{code} {key}: cleared to {shown:?}"));
                    continue;
                }
                typed += 1;
            }
        }
        assert!(typed > 90, "{typed} date boxes typed");
        assert!(
            refused < 150,
            "{refused} date boxes refused for another rule"
        );
        let mut expected: Vec<&str> = DATE_BOXES_LOCKED_FOR_OTHER_REASONS.to_vec();
        expected.sort_unstable();
        locked.sort();
        assert_eq!(locked, expected, "{locked:#?}");
    }

    /// Date boxes a return opens locked (unbound, derived, or the return's
    /// own identity) or whose typed date its form clears (a schedule another
    /// answer turns off): each for its form's group, not for rule 4.
    const DATE_BOXES_LOCKED_FOR_OTHER_REASONS: &[&str] = &[
        "0619F txtDate20: cannot be filled here: not written by this app's field map for any sample draft",
        "0619F txtDate21: cannot be filled here: not written by this app's field map for any sample draft",
        "0619F txtDate22: cannot be filled here: not written by this app's field map for any sample draft",
        "1601C frm1601c:txtDate37: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1601C frm1601c:txtDate38: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1601C frm1601c:txtDate39: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1601C frm1601c:txtDate40: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1601C txtDateExpiry: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1601C txtDateIssue: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1701MS frm1701MS:txtTaxpayerNo16a1: cleared to \"\"",
        "1701MS frm1701MS:txtTaxpayerNo16a: cleared to \"\"",
        "1701MS frm1701MS:txtTaxpayerNo16b1: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1701MS frm1701MS:txtTaxpayerNo16b: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1702RT frm1702RT:txtPg1Pt1I10: cannot be filled here: derived by the app: changes with /incorporation_date/day, /incorporation_date/month, /incorporation_date/year but never shows one field's value",
        "1702RT frm1702RT:txtPg1Pt3I23DebitMemoC3Date: cannot be filled here: derived by the app: changes with /payment_details/0/date/day, /payment_details/0/date/month, /payment_details/0/date/year but never shows one field's value",
        "1702RT frm1702RT:txtPg1Pt3I24CheckC3Date: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1702RT frm1702RT:txtPg1Pt3I25TaxDebitDate: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1702RT frm1702RT:txtPg1Pt3I26OthersC3Date: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1707 frm1707:txtPg2P4Sch1I8DateCollection: cleared to \"\"",
        "1707A frm1707Av2021:txtS1C1DateOfTransactionI4: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1707A frm1707Av2021:txtS1C1DateOfTransaction_1: cleared to \"\"",
        "1707A frm1707Av2021:txtS2C1DateOfTransactionI4: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "1707A frm1707Av2021:txtS2C1DateOfTransaction_1: cleared to \"\"",
        "2000 frm2000:sched2:txtPaymentDate0: cleared to \"\"",
        "2000 frm2000:sched2:txtPaymentDate1: cleared to \"\"",
        "2000 frm2000:sched2:txtPaymentDate2: cleared to \"\"",
        "2000 frm2000:sched3:txtPaymentDate0: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "2000 frm2000:sched3:txtPaymentDate1: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "2000 frm2000:sched3:txtPaymentDate2: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "2000 frm2000:sched4:txtRemittanceDate0: cleared to \"\"",
        "2000 frm2000:sched4:txtRemittanceDate1: cleared to \"\"",
        "2000 frm2000:sched4:txtRemittanceDate2: cleared to \"\"",
        "2000 frm2000:txtDate20: cannot be filled here: not written by this app's field map for any sample draft",
        "2000 frm2000:txtDate21: cannot be filled here: not written by this app's field map for any sample draft",
        "2000 frm2000:txtDate22: cannot be filled here: not written by this app's field map for any sample draft",
        "2000OT frm2000OT:txtTransactionDate: picks which return this is; reopen the form for another TIN or period",
        "2550Q frm2550qv2024:RtnPeriodFromNo4: cannot be filled here: derived by the app: changes with /return_period_from/day, /return_period_from/month, /return_period_from/year but never shows one field's value",
        "2550Q frm2550qv2024:RtnPeriodToNo4: cannot be filled here: derived by the app: changes with /return_period_to/day, /return_period_to/month, /return_period_to/year but never shows one field's value",
        "2551Q txtDateExpiry: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "2551Q txtDateIssue: cannot be filled here: written by the app without a draft field (a constant or the period)",
        "2552 frm2552:txtPg2Pt5Sch1Date5: cannot be filled here: derived by the app: changes with /transaction but never shows one field's value",
        "2552 frm2552:txtPg2Pt5Sch2Date5: cannot be filled here: derived by the app: changes with /transaction but never shows one field's value",
        "2552 frm2552:txtPg2Pt5Sch3Date5: cannot be filled here: derived by the app: changes with /transaction but never shows one field's value",
    ];

    /// Every field of a form with its schema entry (hidden ones too).
    fn schema_fields(code: &str) -> Vec<&'static Value> {
        schema_entry(code).unwrap()["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|part| part["items"].as_array().into_iter().flatten())
            .flat_map(|item| item["f"].as_array().into_iter().flatten())
            .collect()
    }

    /// What the XML carries for a pool's lines (the field map), end to end.
    fn xml_pool(kind: AgentFormKind, draft: &Value, keys: &[String]) -> String {
        let map = agent::field_map(kind, draft).unwrap();
        keys.iter()
            .map(|k| map.get(k).cloned().unwrap_or_default())
            .collect::<String>()
    }

    /// The whole address the draft holds, read by hand per form: 0619E and
    /// 0619F join two fields, 1702RT keeps a list of lines, the rest one field.
    fn draft_address(code: &str, draft: &Value) -> String {
        let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
        match code {
            "0619E" | "0619F" => format!(
                "{}{}",
                s(&draft["registered_address"]),
                s(&draft["registered_address_2"])
            ),
            "1702RT" => draft["registered_address_lines"]
                .as_array()
                .into_iter()
                .flatten()
                .map(s)
                .collect(),
            _ => s(&draft["registered_address"]),
        }
    }

    fn squeezed(text: &str) -> String {
        text.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_uppercase()
    }

    /// Rule 5: a printed address that runs over stacked lines is one box on
    /// the page, read from and written to the draft, never from the field
    /// map's cut lines. The oracle is the list of two-line addresses the form
    /// review found (docs/form-findings, R5 "Two-line address") with 1702RT's
    /// three-line one; 1702MX keeps three boxes (its first line is the whole
    /// address but its print reads the list), and the review's 1601C, 1701A,
    /// 1702EX and 1707 print their address through the print map's own lines,
    /// so the sheet gives their boxes no slots. The print side is
    /// `a_long_pooled_address_prints_whole_on_every_pooled_form`.
    #[test]
    fn a_printed_address_over_stacked_lines_is_one_pool() {
        setup();
        let expected: BTreeMap<&str, (Vec<&str>, Vec<u64>)> = BTreeMap::from([
            (
                "0619E",
                (
                    vec!["frm0619E:txtAddress", "frm0619E:txtAddress2"],
                    vec![40, 31],
                ),
            ),
            (
                "0619F",
                (
                    vec!["frm0619F:txtAddress", "frm0619F:txtAddress2"],
                    vec![40, 31],
                ),
            ),
            (
                "1600PT",
                (
                    vec![
                        "frm1600PT:txtPg1Pt1I8RegisteredAddress",
                        "frm1600PT:txtPg1Pt1I8RegisteredAddress2",
                    ],
                    vec![40, 31],
                ),
            ),
            (
                "1600VT",
                (
                    vec![
                        "frm1600VT:txtPg1Pt1I8RegisteredAddress",
                        "frm1600VT:txtPg1Pt1I8RegisteredAddress2",
                    ],
                    vec![40, 31],
                ),
            ),
            (
                "1701",
                (
                    vec!["frm1701:txtPg1I9Address", "frm1701:txtPg1I9Address2"],
                    vec![40, 31],
                ),
            ),
            (
                "1702RT",
                (
                    vec![
                        "frm1702RT:txtPg1Pt1I9Address1",
                        "frm1702RT:txtPg1Pt1I9Address2",
                        "frm1702RT:txtPg1Pt1I9Address3",
                    ],
                    vec![38, 38, 30],
                ),
            ),
            (
                "2200C",
                (
                    vec![
                        "frm2200C:txtPg1Pt1I8RegisteredAddress",
                        "frm2200C:txtPg1Pt1I8RegisteredAddress2",
                    ],
                    vec![40, 31],
                ),
            ),
            (
                "2200M",
                (
                    vec![
                        "frm2200M:txtPg1Pt1I7RegisteredAddress",
                        "frm2200M:txtPg1Pt1I7RegisteredAddress2",
                    ],
                    vec![40, 31],
                ),
            ),
            (
                "2200P",
                (
                    vec![
                        "frm2200P:txtPg1Pt1I7RegisteredAddress",
                        "frm2200P:txtPg1Pt1I7RegisteredAddress2",
                    ],
                    vec![40, 31],
                ),
            ),
            (
                "2552",
                (
                    vec![
                        "frm2552:txtPg1Pt1I7RegisteredAddress",
                        "frm2552:txtPg1Pt1I7RegisteredAddress2",
                    ],
                    vec![40, 31],
                ),
            ),
        ]);
        // 158 characters: a word runs across characters 40, 80, 100 and 127,
        // where the field maps cut their lines.
        let long = "UNIT 1203 TWELFTH FLOOR THE CURVE BUILDING THIRTYSECOND STREET CORNER THIRD AVENUE BONIFACIO GLOBALCITY TAGUIG CITY METRO MANILA PHILIPPINES 1634 ANNEX BLDG";
        let mut found = BTreeMap::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            for field in schema_fields(code) {
                // Email is always a plain field; amounts and dates have no slots.
                if !matches!(
                    field["t"].as_str(),
                    Some("text" | "phone" | "int" | "hidden")
                ) {
                    assert!(field["slots"].is_null(), "{code}: {} has slots", field["k"]);
                }
            }
            for (head, pool) in combs::pools(code) {
                let keys = pool.keys.clone();
                // The later lines are part of the pool, not boxes of their own.
                for key in &keys[1..] {
                    let line = schema_fields(code)
                        .into_iter()
                        .find(|f| f["k"] == key.as_str())
                        .unwrap();
                    assert_eq!(line["t"], "hidden", "{code}: {key}");
                    assert_eq!(line["pooled"], head.as_str(), "{code}: {key}");
                }
                // It writes the first line alone, or every line of one list.
                let bound: Vec<String> = keys
                    .iter()
                    .filter(|k| {
                        matches!(
                            def.bindings.get(*k).map(|b| &b.binding),
                            Some(Binding::Text { .. })
                        )
                    })
                    .cloned()
                    .collect();
                assert!(
                    pool.write == [head.clone()] || pool.write == keys,
                    "{code}: {:?}",
                    pool.write
                );
                assert!(pool.write.iter().all(|k| bound.contains(k)), "{code}");
                let opened = open(code, page_period_of(code));
                let unlocked = [head.as_str()];
                // A long address comes back exactly as typed, and showing it
                // again changes nothing (no space is ever inserted).
                let filled =
                    fill_unlocked(code, &opened["draft"], json!({ &head: long }), &unlocked);
                assert_eq!(filled["rejected"], json!({}), "{code}");
                assert_eq!(
                    filled["pools"][&head], long,
                    "{code}: the box shows what was typed"
                );
                let again = fill_unlocked(
                    code,
                    &filled["draft"],
                    json!({ &head: filled["pools"][&head] }),
                    &unlocked,
                );
                assert_eq!(
                    again["pools"][&head], long,
                    "{code}: unchanged on the next edit"
                );
                assert_eq!(
                    again["draft"], filled["draft"],
                    "{code}: the draft does not move"
                );
                // The page value is the draft's whole address, and the XML
                // carries it (its lines cut where the field map cuts them).
                // The print is a_long_pooled_address_prints_whole_on_every_pooled_form.
                assert_eq!(
                    draft_address(code, &filled["draft"]),
                    long,
                    "{code}: the draft holds what was typed"
                );
                assert_eq!(
                    squeezed(&xml_pool(kind, &filled["draft"], &keys)),
                    squeezed(long),
                    "{code}: the XML carries it"
                );
                found.insert(
                    code,
                    (
                        keys,
                        pool.slots.iter().map(|n| *n as u64).collect::<Vec<_>>(),
                    ),
                );
            }
        }
        let found: BTreeMap<&str, (Vec<&str>, Vec<u64>)> = found
            .iter()
            .map(|(code, (keys, slots))| {
                (
                    *code,
                    (keys.iter().map(String::as_str).collect(), slots.clone()),
                )
            })
            .collect();
        assert_eq!(found, expected);
    }

    /// Upper-case address words, exactly `len` characters, single spaces.
    fn address_of(len: usize) -> String {
        const WORDS: [&str; 10] = [
            "UNIT",
            "1203",
            "TWELFTH",
            "FLOOR",
            "CURVE",
            "BUILDING",
            "BONIFACIO",
            "TAGUIG",
            "METRO",
            "MANILA",
        ];
        let mut text = String::new();
        for word in WORDS.iter().cycle() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(word);
            if text.len() >= len {
                break;
            }
        }
        text.truncate(len);
        if text.ends_with(' ') {
            text.pop();
            text.push('X');
        }
        text
    }

    /// The print cell (first `html_id`) of each writer key, read from the
    /// sheet's writer-cells.json.
    fn print_cells_of(slug: &str, keys: &[String]) -> Vec<String> {
        let path = format!(
            "{}/../../html-frozen/{slug}/writer-cells.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let cells: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        keys.iter()
            .map(|key| {
                cells["joins"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|join| join["writer_key"] == key.as_str())
                    .and_then(|join| join["html_id"].as_str())
                    .unwrap_or_else(|| panic!("{slug}: {key} prints in a cell"))
                    .to_string()
            })
            .collect()
    }

    /// A sheet cell's printed width (pt), read from its `<div id>` style.
    fn cell_width(slug: &str, cell: &str) -> f64 {
        let path = format!(
            "{}/../../html-frozen/{slug}/index.html",
            env!("CARGO_MANIFEST_DIR")
        );
        let html = std::fs::read_to_string(path).unwrap();
        let at = html.find(&format!("<div id=\"{cell}\"")).unwrap();
        let tag = &html[at..at + html[at..].find('>').unwrap()];
        tag.split(";width:")
            .nth(1)
            .unwrap()
            .split("pt")
            .next()
            .unwrap()
            .parse()
            .unwrap()
    }

    /// Every comb cell of a filled sheet, page by page in cell order, as it
    /// prints (`printed_cell`): its boxes, or the plain line over them.
    fn printed_cells_in_order(slug: &str, document: &str) -> String {
        let path = format!(
            "{}/../../html-frozen/{slug}/index.html",
            env!("CARGO_MANIFEST_DIR")
        );
        let html = std::fs::read_to_string(path).unwrap();
        let mut cells: Vec<(u32, u32, String)> = html
            .split("<div id=\"")
            .skip(1)
            .filter_map(|rest| {
                let id = &rest[..rest.find('"')?];
                let tag = &rest[..rest.find('>')?];
                if !tag.contains("data-field-kind=\"comb\"") {
                    return None;
                }
                let (page, cell) = id.strip_prefix('p')?.split_once('c')?;
                Some((page.parse().ok()?, cell.parse().ok()?, id.to_string()))
            })
            .collect();
        cells.sort();
        let printed = bir_print::frozen_html::printed_cells(document);
        cells
            .iter()
            .map(|(_, _, id)| printed.get(id).map(|(text, _)| text.as_str()).unwrap_or(""))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// A sheet's comb cells and their boxes, read from its HTML.
    fn comb_slots(slug: &str) -> BTreeMap<String, usize> {
        let path = format!(
            "{}/../../html-frozen/{slug}/index.html",
            env!("CARGO_MANIFEST_DIR")
        );
        let html = std::fs::read_to_string(path).unwrap();
        html.split("<div id=\"")
            .skip(1)
            .filter_map(|rest| {
                let tag = &rest[..rest.find('>')?];
                if !tag.contains("data-field-kind=\"comb\"") {
                    return None;
                }
                let id = &tag[..tag.find('"')?];
                let at = tag.find("data-comb-slots=\"")? + "data-comb-slots=\"".len();
                let slots = tag[at..].split('"').next()?.parse().ok()?;
                Some((id.to_string(), slots))
            })
            .collect()
    }

    /// Nothing the print map carries is cut on the sheet: every text join
    /// whose value runs past its comb's boxes prints whole there (an item
    /// over stacked lines, across its lines). Returns what was cut.
    fn cut_on_the_sheet(
        slug: &str,
        fields: &BTreeMap<String, String>,
        document: &str,
    ) -> Vec<String> {
        let path = format!(
            "{}/../../html-frozen/{slug}/writer-cells.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let cells: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let slots = comb_slots(slug);
        let stacked = bir_print::frozen_html::stacked_lines(slug).unwrap();
        let mut key_of: BTreeMap<String, String> = BTreeMap::new();
        let mut cut = Vec::new();
        for join in cells["joins"].as_array().unwrap() {
            let key = join["writer_key"].as_str().unwrap();
            let ids = join["html_id"].as_str().into_iter().chain(
                join["html_ids"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str),
            );
            for id in ids {
                key_of.insert(id.to_string(), key.to_string());
            }
        }
        let value = |cell: &str| {
            key_of
                .get(cell)
                .and_then(|key| fields.get(key))
                .cloned()
                .unwrap_or_default()
        };
        let all = bir_print::frozen_html::printed_cells(document);
        let printed = |cell: &str| {
            all.get(cell)
                .map(|(text, _)| text.clone())
                .unwrap_or_default()
        };
        let over = |cell: &str| {
            slots
                .get(cell)
                .is_some_and(|n| value(cell).chars().count() > *n)
        };
        for lines in &stacked {
            if lines.iter().any(|cell| over(cell)) {
                let want: String = lines.iter().map(|cell| value(cell)).collect();
                let got: String = lines.iter().map(|cell| printed(cell)).collect();
                if squeezed(&got) != squeezed(&want) {
                    cut.push(format!("{lines:?}: {want:?} printed {got:?}"));
                }
            }
        }
        for cell in key_of.keys() {
            if stacked.iter().flatten().any(|c| c == cell) || !over(cell) {
                continue;
            }
            if squeezed(&printed(cell)) != squeezed(&value(cell)) {
                cut.push(format!(
                    "{cell}: {:?} printed {:?}",
                    value(cell),
                    printed(cell)
                ));
            }
        }
        cut
    }

    /// The boxes the form review found growing text printed into fixed
    /// character boxes, or taking more characters than the boxes hold
    /// (docs/form-findings R5 "print in fixed char boxes" and "accept more
    /// characters than their printed boxes"), as the review listed them.
    /// Each is typed as long as the page takes it (with the answers that
    /// put it on the return), saved through the draft and printed through
    /// the form's own print map: since rules page question 4 (decided
    /// 2026-10-17) the whole text prints, in its boxes while it fits and as
    /// plain type over them past that, and nothing else the map carries is
    /// cut either (the later pages' repeats of a name included). The boxes
    /// the page never types say what prints them instead.
    #[test]
    fn every_reviewed_long_box_prints_whole_past_its_boxes() {
        setup();
        let reviewed: &[(&str, &[&str])] = &[
            (
                "0619E",
                &[
                    "frm0619E:txtTaxpayerName",
                    "frm0619E:txtAddress",
                    "frm0619E:txtAddress2",
                    "txtEmail",
                ],
            ),
            (
                "0619F",
                &[
                    "frm0619F:txtTaxpayerName",
                    "frm0619F:txtAddress",
                    "frm0619F:txtAddress2",
                    "txtEmail",
                ],
            ),
            (
                "1600PT",
                &[
                    "frm1600PT:txtPg1Pt1I7WithholdingAgentsName",
                    "frm1600PT:txtPg1Pt1I8RegisteredAddress",
                    "frm1600PT:txtPg1Pt1I8RegisteredAddress2",
                    "frm1600PT:txtPg1Pt1I11Email",
                    "frm1600PT:txtPg1Pt1I12TaxReliefSpec",
                    "frm1600PT:txtPg2RegisteredName",
                ],
            ),
            (
                "1600VT",
                &[
                    "frm1600VT:txtPg1Pt1I7WithholdingAgentsName",
                    "frm1600VT:txtPg1Pt1I8RegisteredAddress",
                    "frm1600VT:txtPg1Pt1I8RegisteredAddress2",
                    "frm1600VT:txtPg1Pt1I11Email",
                    "frm1600VT:txtPg1Pt1I12TaxReliefSpec",
                    "frm1600VT:txtPg2RegisteredName",
                ],
            ),
            (
                "1601C",
                &[
                    "frm1601c:txtTaxpayerName",
                    "txtEmail",
                    "frm1601c:txtParticular40",
                    "frm1601c:txtPg2TaxpayerName",
                ],
            ),
            ("1601EQ", &["frm1601EQ:txtTaxpayerName", "txtEmail"]),
            (
                "1601FQ",
                &[
                    "frm1601FQ:txtTaxpayerName",
                    "txtEmail",
                    "frm1601FQ:txtPg2TaxpayerName",
                ],
            ),
            (
                "1602Q",
                &[
                    "frm1602Q:txtTaxpayerName",
                    "txtEmail",
                    "frm1602Q:txtPage2Agent",
                    "frm1602Q:txtPage3Agent",
                ],
            ),
            (
                "1603Q",
                &[
                    "frm1603Q:txtTaxpayerName",
                    "txtEmail",
                    "frm1603Q:txtPg2TaxpayerName",
                ],
            ),
            ("1604C", &["frm1604c:registeredName", "txtEmail"]),
            (
                "1604E",
                &[
                    "frm1604e:txtWthhldngAgntsNme",
                    "txtEmail",
                    "frm1604e:txtPg2TaxpayerName",
                ],
            ),
            (
                "1604F",
                &[
                    "frm1604f:registeredName",
                    "frm1604f:email",
                    "frm1604f:txtPg2TaxpayerName",
                ],
            ),
            (
                "1700",
                &[
                    "frm1700:txtPg1I7TaxpayerName",
                    "frm1700:txtPg1I10Email",
                    "frm1700:txtPg1I21SpouseName",
                ],
            ),
            (
                "1701",
                &[
                    "frm1701:txtPg1I8TaxpayerName",
                    "frm1701:txtPg1I9Address",
                    "frm1701:txtPg1I9Address2",
                    "txtEmail",
                    "frm1701:txtPg2TaxpayerName",
                    "frm1701:txtPg2I5SpouseName",
                    "frm1701:txtPg3TaxpayerName",
                    "frm1701:txtPg3IShed3_27A",
                    "frm1701:txtPg3IShed3_27B",
                    "frm1701:txtPg3IShed5_1Desc",
                    "frm1701:txtPg3IShed5_2Desc",
                    "frm1701:txtPg3IShed5_4Desc",
                ],
            ),
            (
                "1701A",
                &[
                    "frm1701A:txtTaxpayerName",
                    "txtEmail",
                    "frm1701A:txtPg2TaxpayerName",
                    "frm1701A:txtSpouseName",
                ],
            ),
            (
                "1701Q",
                &[
                    "frm1701q:txtTaxpayerName",
                    "frm1701q:txtAddress",
                    "txtEmail",
                    "frm1701q:txtSpouseName",
                ],
            ),
            (
                "1702EX",
                &[
                    "frm1702EX:txtPg1Pt1I12Email",
                    "frm1702EX:txtPg2RegisteredName",
                    "frm1702EX:txtPg3RegisteredName",
                    "frm1702EX:txtPg3Pt6S2I1Description",
                    "frm1702EX:txtPg3Pt6S2I2Description",
                    "frm1702EX:txtPg3Pt6S2I3Description",
                    "frm1702EX:txtPg3Pt6S2I4Description",
                    "frm1702EX:txtPg3Pt6S3I2NonDeductExpenseOtherIncomeDesc",
                    "frm1702EX:txtPg3Pt6S3I3NonDeductExpenseOtherIncomeDesc",
                    "frm1702EX:txtPg3Pt6S3I5NonTaxIncomeAndIncomeSubjectToFinTaxDesc",
                    "frm1702EX:txtPg3Pt6S3I6NonTaxIncomeAndIncomeSubjectToFinTaxDesc",
                    "frm1702EX:txtPg3Pt6S3I7SpecialDeductDesc",
                    "frm1702EX:txtPg3Pt6S3I8SpecialDeductDesc",
                ],
            ),
            (
                "1702MX",
                &[
                    "frm1702MX:txtPg1Pt1I9RegisteredName",
                    "frm1702MX:txtPg1Pt1I9RegisteredName2",
                    "frm1702MX:txtPg1Pt1I9RegisteredName3",
                    "frm1702MX:txtPg1Pt1I10RegisteredAddress",
                    "frm1702MX:txtPg1Pt1I10RegisteredAddress2",
                    "frm1702MX:txtPg1Pt1I10RegisteredAddress3",
                    "frm1702MX:txtPg1Pt1I12Email",
                    "frm1702MX:txtPg2RegisteredName",
                    "frm1702MX:txtPg3RegisteredName",
                    "frm1702MX:txtPg4RegisteredName",
                ],
            ),
            (
                "1702Q",
                &[
                    "txtEmail",
                    "frm1702q:txtPg2TaxpayerName",
                    "frm1702q:Sched4:txtOthrTxCrdts0",
                    "frm1702q:Sched4:txtOthrTxCrdts1",
                ],
            ),
            (
                "1702RT",
                &[
                    "frm1702RT:txtPg1Pt1I8Name1",
                    "frm1702RT:txtPg1Pt1I8Name2",
                    "frm1702RT:txtPg1Pt1I8Name3",
                    "frm1702RT:txtPg1Pt1I9Address1",
                    "frm1702RT:txtPg1Pt1I9Address2",
                    "frm1702RT:txtPg1Pt1I9Address3",
                    "frm1702RT:txtPg1Pt1I12Email",
                    "frm1702RT:txtPg1Pt3I26Others",
                    "frm1702RT:txtPg2RegisteredName",
                    "frm1702RT:txtPg2Pt4I53C1",
                    "frm1702RT:txtPg2Pt4I53C2",
                    "frm1702RT:txtPg2Pt4I54C1",
                    "frm1702RT:txtPg2Pt4I54C2",
                ],
            ),
            ("1706", &["frm1706:txtOthers21"]),
            (
                "1707",
                &[
                    "frm1707:txtPg2P4Sch2NameOfCorpStock1",
                    "frm1707:txtPg2P4Sch2NameOfCorpStock2",
                    "frm1707:txtPg2P4Sch2NameOfCorpStock3",
                    "frm1707:txtPg2P4Sch2NameOfCorpStock4",
                    "frm1707:txtPg2P4Sch3Particulars1",
                    "frm1707:txtPg2P4Sch3Particulars2",
                    "frm1707:txtPg2P4Sch3Particulars3",
                    "frm1707:txtPg2P4Sch3Particulars4",
                    "frm1707:txtPg2RegisteredName",
                ],
            ),
            (
                "1707A",
                &[
                    "frm1707Av2021:txtI8RegisteredName",
                    "frm1707Av2021:txtI8RegisteredNameP2",
                ],
            ),
            (
                "1800",
                &[
                    "frm1800v2018:txtDonorName",
                    "frm1800v2018:txtRegAddress",
                    "frm1800v2018:txtResAddress",
                    "frm1800v2018:txtEmail",
                    "frm1800v2018:txtPg2TaxpayerName",
                ],
            ),
            (
                "1801",
                &[
                    "frm1801v2018:registeredName_1",
                    "frm1801v2018:registeredName_2",
                    "frm1801v2018:registeredAddress_1",
                    "frm1801v2018:adminName",
                    "frm1801v2018:txtEmail",
                ],
            ),
            (
                "2000",
                &[
                    "frm2000:txtTaxpayerName",
                    "frm2000:txtAddress",
                    "frm2000:txtOtherName",
                ],
            ),
            (
                "2000OT",
                &[
                    "frm2000OT:txtRDOCode",
                    "frm2000OT:txtTaxpayerName",
                    "frm2000OT:txtAddress",
                    "txtEmail",
                    "frm2000OT:txtOtherName",
                    "frm2000OT:txtRealLocation",
                    "frm2000OT:txtPg2TaxpayerName",
                ],
            ),
            (
                "2200A",
                &[
                    "frm2200Av2020:registeredName",
                    "frm2200Av2020:treatyY",
                    "frm2200Av2020:paymentOther",
                    "frm2200Av2020:othersDetails",
                ],
            ),
            (
                "2200AN",
                &[
                    "frm2200ANv2018:txtTaxpayerName",
                    "frm2200ANv2018:txtAddress",
                    "txtEmail",
                    "frm2200ANv2018:lstSpecialTax",
                    "frm2200ANv2018:MannerOfPaymentOthers",
                    "frm2200ANv2018:txtPage2Taxpayer",
                ],
            ),
            (
                "2200C",
                &[
                    "frm2200C:txtPg1Pt1I7RegisteredName",
                    "frm2200C:txtPg1Pt1I8RegisteredAddress",
                    "frm2200C:txtPg1Pt1I8RegisteredAddress2",
                    "frm2200C:txtPg1I12TaxReliefSpecify",
                    "frm2200C:txtPg1Pt2MOPOtherDesc",
                ],
            ),
            (
                "2200M",
                &[
                    "frm2200M:txtPg1Pt1I6RegisteredName",
                    "frm2200M:txtPg1Pt1I7RegisteredAddress",
                    "frm2200M:txtPg1Pt1I7RegisteredAddress2",
                    "frm2200M:txtPg1Pt1I9Email",
                    "frm2200M:txtPg1I12TaxReliefSpecify",
                    "frm2200M:txtPg1Pt2MOPOtherDesc",
                    "frm2200M:txtPg2RegisteredName",
                ],
            ),
            (
                "2200P",
                &[
                    "frm2200P:txtPg1Pt1I6RegisteredName",
                    "frm2200P:txtPg1Pt1I7RegisteredAddress",
                    "frm2200P:txtPg1Pt1I7RegisteredAddress2",
                    "frm2200P:txtPg1I12TaxReliefSpecify",
                    "frm2200P:txtPg1Pt2MOPOtherDesc",
                ],
            ),
            (
                "2200S",
                &[
                    "frm2200S:taxpayerName",
                    "txtEmail",
                    "frm2200S:txtOthMannerofPymnt",
                ],
            ),
            (
                "2200T",
                &[
                    "frm2200Tv2020:registeredName",
                    "frm2200Tv2020:txtEmail",
                    "frm2200Tv2020:othersDetails",
                    "frm2200Tv2020:registeredName_2",
                    "frm2200Tv2020:registeredName_3",
                ],
            ),
            (
                "2550Q",
                &[
                    "frm2550qv2024:taxpayerName",
                    "frm2550qv2024:taxpayerEmailAddress",
                    "frm2550qv2024:specifyInternationalTreaty",
                    "frm2550qv2024:Pg2TaxPayer",
                ],
            ),
            ("2551Q", &["frm2551Qv2018:registeredName"]),
            (
                "2552",
                &[
                    "frm2552:txtPg1Pt1I6Name",
                    "frm2552:txtPg1Pt1I7RegisteredAddress",
                    "frm2552:txtPg1Pt1I7RegisteredAddress2",
                    "frm2552:txtPg1Pt1I9Email",
                    "frm2552:txtPg1Pt1I10TaxReliefSpec",
                ],
            ),
        ];
        // The answers that put a box on the return.
        let married_1700: &[&str] = &[
            "frm1700:rdoPg1I15CivilStatusM",
            "frm1700:rdoPg1I16SpouseHasIncomeY",
            "frm1700:rdoPg1I17FilingStatusJ",
        ];
        let married_1701a: &[&str] = &[
            "frm1701A:optCivilStatus_2",
            "frm1701A:optSpouseIncome_1",
            "frm1701A:optFilingStatus_1",
        ];
        let itemized_1701: &[&str] = &[
            "frm1701:rdoPg1I21TaxRateG",
            "frm1701:rdoPg1I21AMethodDeductionI",
        ];
        let itemized_spouse_1701: &[&str] = &[
            "frm1701:rdoPg1I16CivilStatusM",
            "frm1701:rdoPg1I17SpouseIncomeYes",
            "frm1701:rdoPg1I18FilingStatusJ",
            "frm1701:rdoPg2I12TaxRateG",
            "frm1701:rdoPg2I12AMethodDeductionI",
        ];
        let answers: BTreeMap<&str, &[&str]> = BTreeMap::from([
            (
                "frm1600PT:txtPg1Pt1I12TaxReliefSpec",
                &["frm1600PT:rdoPg1I12TaxReliefYes"][..],
            ),
            (
                "frm1600VT:txtPg1Pt1I12TaxReliefSpec",
                &["frm1600VT:rdoPg1I12TaxReliefYes"][..],
            ),
            ("frm1700:txtPg1I21SpouseName", married_1700),
            ("frm1701:txtPg3IShed5_1Desc", itemized_1701),
            ("frm1701:txtPg3IShed5_2Desc", itemized_1701),
            ("frm1701:txtPg3IShed5_4Desc", itemized_spouse_1701),
            ("frm1701A:txtSpouseName", married_1701a),
            ("frm2000:txtOtherName", &["frm2000:optParty_1"][..]),
            (
                "frm2000OT:txtRealLocation",
                &["frm2000OT:rbTransNature_2"][..],
            ),
            ("frm2200Av2020:treatyY", &["frm2200Av2020:optTreaty_1"][..]),
            (
                "frm2200Av2020:paymentOther",
                &["frm2200Av2020:optPayment_3"][..],
            ),
            (
                "frm2200ANv2018:lstSpecialTax",
                &["frm2200ANv2018:OptSpecialTax1"][..],
            ),
            (
                "frm2200ANv2018:MannerOfPaymentOthers",
                &["frm2200ANv2018:MannerOfPayment3"][..],
            ),
            (
                "frm2200C:txtPg1I12TaxReliefSpecify",
                &["frm2200C:rdoPg1I12TaxReliefYes"][..],
            ),
            (
                "frm2200C:txtPg1Pt2MOPOtherDesc",
                &["frm2200C:rdoPg1Pt2MOPOther"][..],
            ),
            (
                "frm2200M:txtPg1I12TaxReliefSpecify",
                &["frm2200M:rdoPg1I12TaxReliefYes"][..],
            ),
            (
                "frm2200M:txtPg1Pt2MOPOtherDesc",
                &["frm2200M:rdoPg1Pt2MOPOther"][..],
            ),
            (
                "frm2200P:txtPg1I12TaxReliefSpecify",
                &["frm2200P:rdoPg1I12TaxReliefYes"][..],
            ),
            (
                "frm2200P:txtPg1Pt2MOPOtherDesc",
                &["frm2200P:rdoPg1Pt2MOPOther"][..],
            ),
            (
                "frm2552:txtPg1Pt1I10TaxReliefSpec",
                &["frm2552:rdoPg1I10TaxReliefYes"][..],
            ),
        ]);
        // Boxes the page does not type, and what prints them.
        let printed_from: BTreeMap<&str, &str> = BTreeMap::from([
            (
                "frm0619E:txtAddress2",
                "the pooled address, typed in frm0619E:txtAddress",
            ),
            (
                "frm0619F:txtAddress2",
                "the pooled address, typed in frm0619F:txtAddress",
            ),
            (
                "frm1600PT:txtPg1Pt1I8RegisteredAddress2",
                "the pooled address, typed in its first line",
            ),
            (
                "frm1600VT:txtPg1Pt1I8RegisteredAddress2",
                "the pooled address, typed in its first line",
            ),
            (
                "frm1701:txtPg1I9Address2",
                "the pooled address, typed in frm1701:txtPg1I9Address",
            ),
            (
                "frm2200C:txtPg1Pt1I8RegisteredAddress2",
                "the pooled address, typed in its first line",
            ),
            (
                "frm2200M:txtPg1Pt1I7RegisteredAddress2",
                "the pooled address, typed in its first line",
            ),
            (
                "frm2200P:txtPg1Pt1I7RegisteredAddress2",
                "the pooled address, typed in its first line",
            ),
            (
                "frm2552:txtPg1Pt1I7RegisteredAddress2",
                "the pooled address, typed in its first line",
            ),
            (
                "frm1602Q:txtPage2Agent",
                "the agent's name repeated (page 2 header)",
            ),
            (
                "frm1602Q:txtPage3Agent",
                "the agent's name repeated (page 3 header)",
            ),
            (
                "frm1601c:txtParticular40",
                "the app writes it (no draft field)",
            ),
            ("frm1701:txtPg3IShed3_27A", "an amount"),
            ("frm1701:txtPg3IShed3_27B", "an amount"),
            (
                "frm1702EX:txtPg3Pt6S2I4Description",
                "the app writes it (no draft field)",
            ),
            (
                "frm1702EX:txtPg3Pt6S3I3NonDeductExpenseOtherIncomeDesc",
                "the app writes it (no draft field)",
            ),
            (
                "frm1702EX:txtPg3Pt6S3I6NonTaxIncomeAndIncomeSubjectToFinTaxDesc",
                "the app writes it (no draft field)",
            ),
            (
                "frm1702EX:txtPg3Pt6S3I8SpecialDeductDesc",
                "the app writes it (no draft field)",
            ),
            (
                "frm1702q:Sched4:txtOthrTxCrdts0",
                "derived from the row's description and ATC",
            ),
            (
                "frm1702q:Sched4:txtOthrTxCrdts1",
                "derived from the row's description and ATC",
            ),
            ("frm1702RT:txtPg2Pt4I53C2", "a whole number"),
            (
                "frm1702RT:txtPg2Pt4I54C2",
                "derived from the item's amounts",
            ),
            (
                "frm1707:txtPg2P4Sch2NameOfCorpStock4",
                "derived from the transaction type",
            ),
            (
                "frm1707:txtPg2P4Sch3Particulars4",
                "derived from the transaction type",
            ),
            (
                "frm2200S:txtOthMannerofPymnt",
                "the app writes it (no draft field)",
            ),
        ]);
        // Typed whole, filed whole, but printed from elsewhere: 1702MX's
        // first address line is the whole address while its print reads the
        // address lines (combs: not pooled), so typing it does not reach
        // the sheet. Not a cut by the print; its finding stays open.
        let print_reads_elsewhere = ["frm1702MX:txtPg1Pt1I10RegisteredAddress"];
        let mut whole = 0;
        for (code, keys) in reviewed {
            let kind = kind_of(code).unwrap();
            let slug = crate::bir_module::print_slug_for(code).unwrap();
            let opened = open(code, page_period_of(code));
            for key in *keys {
                let length = field_length(code, key).map_or(90, |m| m as usize).min(150);
                let text = if key.to_ascii_lowercase().contains("mail") {
                    format!("{}@example.com", "q".repeat(length.saturating_sub(12)))
                } else {
                    address_of(length).replace("UNIT", "ZQXJ")
                };
                let mut draft = opened["draft"].clone();
                for answer in answers.get(key).copied().unwrap_or_default() {
                    let on = fill(code, &draft, json!({ *answer: "true" }));
                    assert_eq!(on["rejected"], json!({}), "{code} {answer}");
                    draft = on["draft"].clone();
                }
                // A schedule row files its description with an amount and
                // a legal basis.
                let mut edits = Map::new();
                if key.contains("Shed5") {
                    let row = key.trim_end_matches("Desc");
                    edits.insert(format!("{row}Amt"), json!("1000"));
                    edits.insert(format!("{row}Legal"), json!("RA 0000"));
                }
                edits.insert(key.to_string(), json!(text));
                let unlocked: Vec<String> = edits.keys().cloned().collect();
                let unlocked: Vec<&str> = unlocked.iter().map(String::as_str).collect();
                let filled = fill_unlocked(code, &draft, Value::Object(edits), &unlocked);
                if let Some(why) = printed_from.get(key) {
                    assert!(
                        filled["rejected"].get(*key).is_some(),
                        "{code} {key}: {why}"
                    );
                    continue;
                }
                assert_eq!(filled["rejected"], json!({}), "{code} {key}");
                let (fields, _) =
                    with_database(|db| print_map(db, kind, &filled["draft"])).unwrap();
                let document = bir_print::frozen_html::filled_document(slug, &fields).unwrap();
                let printed = squeezed(&printed_cells_in_order(slug, &document));
                if print_reads_elsewhere.contains(key) {
                    assert!(
                        !printed.contains(&squeezed(&text)),
                        "{code} {key}: now reaches the sheet; close its finding"
                    );
                    continue;
                }
                assert!(
                    printed.contains(&squeezed(&text)),
                    "{code} {key}: the whole {length} characters print"
                );
                let cut = cut_on_the_sheet(slug, &fields, &document);
                assert!(cut.is_empty(), "{code} {key}: {cut:#?}");
                whole += 1;
            }
        }
        assert_eq!(whole, 158, "boxes typed long and printed whole");
    }

    /// Rules page question 4, decided 2026-10-17, end to end on every pooled
    /// address: typed on the page, saved in the draft, through the form's
    /// print map onto its sheet. Within the boxes the lines fill them; past
    /// them the whole address prints as plain type across every line; too
    /// long for that, smaller type. Nothing is lost: the printed lines put
    /// together are what the XML carries, and as much as the form keeps.
    #[test]
    fn a_long_pooled_address_prints_whole_on_every_pooled_form() {
        setup();
        let mut forms = Vec::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let Some(slug) = crate::bir_module::print_slug_for(code) else {
                continue;
            };
            for (head, pool) in combs::pools(code) {
                forms.push(code);
                let cells = print_cells_of(slug, &pool.keys);
                let capacity: usize = pool.slots.iter().sum();
                for len in [capacity - 10, 90, 200] {
                    let typed = address_of(len);
                    let opened = open(code, page_period_of(code));
                    let filled =
                        fill_unlocked(code, &opened["draft"], json!({ &head: typed }), &[&head]);
                    assert_eq!(filled["rejected"], json!({}), "{code} {len}");
                    let draft = &filled["draft"];
                    let kept = squeezed(&xml_pool(kind, draft, &pool.keys));
                    let (fields, _) = with_database(|db| print_map(db, kind, draft)).unwrap();
                    let document = bir_print::frozen_html::filled_document(slug, &fields).unwrap();
                    let printed: Vec<(String, Option<f64>)> = cells
                        .iter()
                        .map(|cell| bir_print::frozen_html::printed_cell(&document, cell))
                        .collect();
                    let text: String = printed.iter().map(|(t, _)| t.as_str()).collect();
                    // What the return holds: as typed, or, where the form's
                    // own field map cuts the address (1600PT/VT, 2200C/M/P
                    // and 2552 keep 160 characters, as the official form
                    // does), what it files.
                    let expected = if !kept.is_empty() && kept.len() < squeezed(&typed).len() {
                        kept.clone()
                    } else {
                        squeezed(&typed)
                    };
                    assert_eq!(squeezed(&text), expected, "{code} {len}: nothing is lost");
                    let sizes: Vec<f64> = printed.iter().filter_map(|(_, size)| *size).collect();
                    if len < capacity {
                        assert!(sizes.is_empty(), "{code} {len}: in the boxes {printed:?}");
                        continue;
                    }
                    assert_eq!(
                        sizes.len(),
                        cells.len(),
                        "{code} {len}: plain on every line {printed:?}"
                    );
                    assert!(
                        printed.iter().all(|(t, _)| !t.trim().is_empty()),
                        "{code} {len}: every line used"
                    );
                    // At the boxes' 9 pt, plain type holds about
                    // width / (0.58 * 9) characters across the lines.
                    let room: f64 = cells
                        .iter()
                        .map(|cell| (cell_width(slug, cell) - 2.0) / (0.58 * 9.0))
                        .sum();
                    let chars = text.chars().count() + cells.len() - 1;
                    if chars as f64 > room {
                        assert!(
                            sizes.iter().all(|s| *s < 9.0),
                            "{code} {len}: shrunk {sizes:?}"
                        );
                    } else {
                        assert!(sizes.iter().all(|s| *s == 9.0), "{code} {len}: {sizes:?}");
                    }
                }
            }
        }
        forms.sort_unstable();
        assert_eq!(
            forms,
            [
                "0619E", "0619F", "1600PT", "1600VT", "1701", "1702RT", "2200C", "2200M", "2200P",
                "2552"
            ],
            "every pooled form"
        );
    }

    /// Rule 5 on 0619E and 0619F, whose draft keeps a second address line
    /// the form joins to the first: the box shows both lines as the form
    /// joins them, and an edit splits the text at the first line's 40 boxes
    /// (the space kept at the end of line 1), so both lines still fill, the
    /// joined address is what was typed, and nothing grows on later edits.
    #[test]
    fn a_second_address_line_is_not_doubled_on_edit() {
        setup();
        for (code, head) in [
            ("0619E", "frm0619E:txtAddress"),
            ("0619F", "frm0619F:txtAddress"),
        ] {
            let kind = AgentFormKind::from_code(code).unwrap();
            let opened = open(code, 1);
            let keys = combs::pools(code)[head].keys.clone();
            for (first, second) in [
                // The review's two-line example: the form joins them as they are.
                ("1 EXAMPLE STREET", "QUEZON CITY"),
                (
                    "UNIT 1203 TWELFTH FLOOR THE CURVE BLDG ",
                    "32ND STREET BONIFACIO GLOBAL CITY TAGUIG",
                ),
            ] {
                let mut draft = opened["draft"].clone();
                draft["registered_address"] = json!(first);
                draft["registered_address_2"] = json!(second);
                let checked = validated(code, &draft);
                let shown = checked["pools"][head].as_str().unwrap().to_string();
                assert_eq!(
                    shown,
                    format!("{first}{second}"),
                    "{code}: shown as the form joins it"
                );
                let xml = xml_pool(kind, &checked["draft"], &keys);
                let mut current = checked["draft"].clone();
                for _ in 0..3 {
                    let filled = fill_unlocked(code, &current, json!({ head: shown }), &[head]);
                    assert_eq!(filled["rejected"], json!({}), "{code}");
                    assert_eq!(
                        filled["pools"][head],
                        shown.as_str(),
                        "{code}: the address is not doubled"
                    );
                    assert_eq!(
                        draft_address(code, &filled["draft"]),
                        shown,
                        "{code}: the joined draft is what was typed"
                    );
                    let line1 = filled["draft"]["registered_address"].as_str().unwrap();
                    assert!(line1.trim_end().chars().count() <= 40, "{code}: {line1:?}");
                    if shown.chars().count() > 40 {
                        assert_ne!(
                            filled["draft"]["registered_address_2"], "",
                            "{code}: both lines fill"
                        );
                    }
                    current = filled["draft"].clone();
                }
                assert_eq!(
                    xml_pool(kind, &current, &keys),
                    xml,
                    "{code}: the XML carries the same address"
                );
            }
        }
    }

    /// Rule 5: fixed formats keep their printed boxes; the oracle is a few
    /// boxes read off the sheets by hand.
    #[test]
    fn fixed_formats_keep_their_printed_boxes() {
        for (code, key, slots) in [
            ("2551Q", "frm2551Qv2018:zipCode", 4),
            ("2551Q", "frm2551Qv2018:telNo", 12),
            ("2551Q", "frm2551Qv2018:registeredName", 40),
            ("1601C", "frm1601c:txtZipCode", 4),
            ("2552", "frm2552:txtPg1Pt1I7RegisteredAddress", 40),
        ] {
            let field = schema_fields(code)
                .into_iter()
                .find(|f| f["k"] == key)
                .unwrap_or_else(|| panic!("{code}: {key}"));
            assert_eq!(field["slots"], slots, "{code}: {key}");
        }
        // An email is always a plain field, whatever the schema typed it:
        // every box bound to an email field, or keyed as one.
        let mut emails = 0;
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            for f in schema_fields(code) {
                let key = f["k"].as_str().unwrap_or_default();
                let path = match def.bindings.get(key).map(|b| &b.binding) {
                    Some(Binding::Text { path, .. }) => path.to_ascii_lowercase(),
                    _ => String::new(),
                };
                if f["t"] == "email"
                    || path.contains("email")
                    || key.to_ascii_lowercase().contains("email")
                {
                    emails += 1;
                    assert!(f["slots"].is_null(), "{code}: {key} is an email with boxes");
                }
            }
        }
        assert!(emails > 40, "{emails} email boxes, none with slots");
        // The ones the review found drawn with boxes.
        for (code, key) in [
            ("0619E", "txtEmail"),
            ("2550Q", "frm2550qv2024:taxpayerEmailAddress"),
            ("2200T", "frm2200Tv2020:txtEmail"),
        ] {
            let field = schema_fields(code).into_iter().find(|f| f["k"] == key);
            assert!(field.is_some_and(|f| f["slots"].is_null()), "{code}: {key}");
        }
    }

    /// A value shown as nothing: blank, all zeros or unticked.
    fn shows_nothing(value: &Value) -> bool {
        let shown = value.as_str().unwrap_or_default().trim();
        shown == "false" || shown.trim_matches(['0', '.', ',', '-']).is_empty()
    }

    /// The test's own probe values for a box (not the host's list): a few
    /// numbers, words, a date and its parts, a tick, and the box's listed
    /// choices.
    fn test_probes(opened: &Value, code: &str, key: &str) -> Vec<Value> {
        let mut probes: Vec<Value> = [
            "1",
            "1000",
            "A",
            "ABC CORP",
            "01",
            "15",
            "2020",
            "01/15/2020",
            "true",
        ]
        .into_iter()
        .map(|v| json!(v))
        .collect();
        let listed = opened["options"][key]
            .as_array()
            .cloned()
            .unwrap_or_default();
        probes.extend(
            listed
                .iter()
                .filter_map(|o| o["value"].as_str())
                .map(|v| json!(v)),
        );
        probes.extend(
            schema_fields(code)
                .into_iter()
                .filter(|f| f["k"] == key)
                .flat_map(|f| f["o"].as_array().cloned().unwrap_or_default())
                .filter_map(|o| {
                    o.as_str()
                        .map(|o| json!(o.split(" - ").next().unwrap_or(o)))
                }),
        );
        probes.retain(|v| !shows_nothing(v) && v != "-");
        probes
    }

    /// The boxes the host reports off.
    fn off_keys(checked: &Value) -> Vec<String> {
        checked["off"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }

    /// The boxes off by the test's own probing of each conditional box
    /// alone: blank now, at least one probe taken, and every probe taken
    /// cleared by the form through the page's own fill.
    fn oracle_off(code: &str, checked: &Value) -> Vec<String> {
        let corpus = conditions::tests::derived();
        corpus[code]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(key, _)| key.clone())
            .filter(|key| {
                if !shows_nothing(&checked["values"][key]) {
                    return false;
                }
                let taken: Vec<bool> = test_probes(checked, code, key)
                    .into_iter()
                    .filter_map(|probe| {
                        let filled = fill(code, &checked["draft"], json!({ key: probe }));
                        filled["rejected"][key]
                            .is_null()
                            .then(|| shows_nothing(&filled["values"][key]))
                    })
                    .collect();
                !taken.is_empty() && taken.iter().all(|cleared| *cleared)
            })
            .collect()
    }

    /// Rule 6: a box another answer turns off is faded with what turns it
    /// on. The oracles: the corpus's own `enabled_when` (read from
    /// rules/forms) and its reviewed tag, and the form's computation, which
    /// clears every value of the test's own probe list that the page's fill
    /// takes there. On every form the host's batched check finds exactly the
    /// boxes that probing each alone finds.
    #[test]
    fn a_box_another_answer_turns_off_says_why_on_every_form() {
        setup();
        let corpus = conditions::tests::derived();
        let tags: Value =
            serde_json::from_str(include_str!("../../../app/forms/condition-labels.json")).unwrap();
        let mut found = BTreeMap::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let opened = open(code, page_period_of(code));
            for (key, why) in opened["off"].as_object().unwrap() {
                let condition = corpus[code][key]
                    .as_str()
                    .unwrap_or_else(|| panic!("{code}: {key}"));
                let tag = tags["keys"][code][key]
                    .as_str()
                    .or_else(|| tags["conditions"][code][condition].as_str());
                assert_eq!(why.as_str(), tag, "{code}: {key} says what turns it on");
                assert!(
                    opened["locks"][key].is_null(),
                    "{code}: {key} is off, not locked"
                );
                found.entry(code).or_insert_with(Vec::new).push(key.clone());
            }
            assert_eq!(off_keys(&opened), oracle_off(code, &opened), "{code}");
        }
        let found: BTreeMap<&str, usize> = found
            .iter()
            .map(|(code, keys)| (*code, keys.len()))
            .collect();
        assert_eq!(
            found,
            BTreeMap::from([
                ("0605", 3),
                ("0619F", 2),
                ("1601C", 2),
                ("1601EQ", 4),
                ("1602Q", 3),
                ("1604C", 5),
                ("1604F", 1),
                ("1606", 2),
                ("1701Q", 39),
                ("1800", 1),
                ("1801", 1),
                ("2000OT", 2),
                ("2200A", 2),
                ("2200AN", 1),
                ("2200C", 1),
                ("2200M", 1),
                ("2200P", 1),
                ("2200T", 1),
                ("2553", 1),
            ])
        );
    }

    /// The return amended, the way its own editor marks it (1601C by its
    /// Item 2 Yes box).
    fn amended(code: &str, draft: &Value) -> Value {
        match AgentFormKind::from_code(code).unwrap() {
            AgentFormKind::Queueable(kind) => bir_core::with_queueable_kind!(kind, F => {
                let mut form: F = serde_json::from_value(draft.clone()).unwrap();
                assert!(form.mark_amended(), "{code} has an Amended Return item");
                serde_json::to_value(form).unwrap()
            }),
            _ => fill(code, draft, json!({"frm1601c:AmendedRtn_1": "true"}))["draft"].clone(),
        }
    }

    fn validated(code: &str, draft: &Value) -> Value {
        call_async(
            "screen_form_validate",
            vec![code.into(), json(draft.clone())],
        )
        .unwrap_or_else(|error| panic!("{code} validates: {error}"))
    }

    /// Rule 6: each box comes alive when what turns it on holds. Amended
    /// Return Yes is set the way the form's own editor sets it; the other
    /// answers are the corpus's own conditions, typed on the page.
    #[test]
    fn a_box_comes_alive_when_its_answer_holds() {
        setup();
        type Turn = fn(&str, &Value) -> Value;
        type TurnOn = Box<dyn Fn(&str, &Value) -> Value>;
        let by = |edits: Value| {
            move |code: &str, draft: &Value| fill(code, draft, edits.clone())["draft"].clone()
        };
        let cases: Vec<(&str, Vec<&str>, TurnOn)> = vec![
            (
                "1601C",
                vec!["frm1601c:txtTax28"],
                Box::new(amended as Turn),
            ),
            (
                "1601EQ",
                vec!["frm1601EQ:txtTax22"],
                Box::new(amended as Turn),
            ),
            (
                "1800",
                vec!["frm1800v2018:TaxCredit17C"],
                Box::new(amended as Turn),
            ),
            (
                "1801",
                vec!["frm1801v2018:txtCredits_TaxPrev"],
                Box::new(amended as Turn),
            ),
            (
                "2000OT",
                vec!["frm2000OT:txtTax18"],
                Box::new(amended as Turn),
            ),
            (
                "2200A",
                vec!["frm2200Av2020:txtPrevReturn"],
                Box::new(amended as Turn),
            ),
            (
                "2200AN",
                vec!["frm2200ANv2018:txt19"],
                Box::new(amended as Turn),
            ),
            (
                "2200C",
                vec!["frm2200C:txtPg1P3I19PmntOnRtrnPrevFiled"],
                Box::new(amended as Turn),
            ),
            (
                "2200M",
                vec!["frm2200M:txtPg1P3I19PmntOnRtrnPrevFiled"],
                Box::new(amended as Turn),
            ),
            (
                "2200P",
                vec!["frm2200P:txtPg1P3I19PmntOnRtrnPrevFiled"],
                Box::new(amended as Turn),
            ),
            (
                "2200T",
                vec!["frm2200Tv2020:txtPrevReturn"],
                Box::new(amended as Turn),
            ),
            ("2553", vec!["frm2553:txt20A"], Box::new(amended as Turn)),
            (
                "1601C",
                vec!["frm1601c:selTreaty"],
                Box::new(by(json!({"frm1601c:SpecialTax_1": "true"}))),
            ),
            (
                "0605",
                vec!["frm0605:txtNumOfInstallment"],
                Box::new(by(json!({"frm0605:itemModeOfPayment:_1": "true"}))),
            ),
            (
                "0605",
                vec!["frm0605:itemApprovedYN:_1", "frm0605:itemApprovedYN:_2"],
                Box::new(by(json!({"frm0605:itemMannerOfPaymentB:_1": "true"}))),
            ),
            (
                "0619F",
                vec!["frm0619F:txtTax13"],
                Box::new(by(
                    json!({"frm0619F:optWithheld:Y": "true", "frm0619F:txtTaxTypeCode": "WB"}),
                )),
            ),
            (
                "0619F",
                vec!["frm0619F:txtTax14"],
                Box::new(by(
                    json!({"frm0619F:optWithheld:Y": "true", "frm0619F:txtTaxTypeCode": "WF"}),
                )),
            ),
            (
                "1601EQ",
                vec![
                    "frm1601EQ:ifRefund",
                    "frm1601EQ:ifIssueCert",
                    "frm1601EQ:ifCarriedOver",
                ],
                Box::new(by(json!({"frm1601EQ:txtTax20": "1000"}))),
            ),
            (
                "1602Q",
                vec![
                    "frm1602Q:OverRemittance1",
                    "frm1602Q:OverRemittance2",
                    "frm1602Q:OverRemittance3",
                ],
                Box::new(by(json!({"frm1602Q:txt18": "1000"}))),
            ),
            (
                "1604C",
                vec![
                    "frm1604c:txtRefMonth",
                    "frm1604c:txtRefDate",
                    "frm1604c:txtRefYear",
                    "frm1604c:txt12",
                    "frm1604c:select13",
                ],
                Box::new(by(json!({"frm1604c:releasedOfFunds_1": "true"}))),
            ),
            (
                "1604F",
                vec!["frm1604f:availedTaxRelief"],
                Box::new(by(json!({"frm1604f:taxRelief_1": "true"}))),
            ),
            (
                "1606",
                vec!["frm1606:opt37:_1", "frm1606:opt37:_2"],
                Box::new(|code: &str, draft: &Value| {
                    fill(
                        code,
                        &amended(code, draft),
                        json!({"frm1606:txtLess": "1000"}),
                    )["draft"]
                        .clone()
                }),
            ),
            (
                "2000OT",
                vec!["frm2000OT:txtRealLocation"],
                Box::new(by(json!({"frm2000OT:rbTransNature_2": "true"}))),
            ),
            (
                "2200A",
                vec!["frm2200Av2020:treatyY"],
                Box::new(by(json!({"frm2200Av2020:optTreaty_1": "true"}))),
            ),
        ];
        for (code, keys, turn_on) in cases {
            let opened = open(code, page_period_of(code));
            for key in &keys {
                assert!(
                    opened["off"].get(*key).is_some(),
                    "{code}: {key} starts off"
                );
            }
            let on = turn_on(code, &opened["draft"]);
            let checked = validated(code, &on);
            for key in &keys {
                assert!(
                    checked["off"].get(*key).is_none(),
                    "{code}: {key} comes alive: {}",
                    checked["off"]
                );
                // And keeps what is typed there.
                let kept = test_probes(&checked, code, key).into_iter().any(|probe| {
                    let filled = fill(code, &on, json!({ *key: probe }));
                    filled["rejected"][*key].is_null() && !shows_nothing(&filled["values"][*key])
                });
                assert!(kept, "{code}: {key} keeps a typed value once on");
            }
            // In this state too, the host's batched check finds exactly the
            // boxes the test's own probes find off.
            assert_eq!(
                off_keys(&checked),
                oracle_off(code, &checked),
                "{code} after turning on {keys:?}"
            );
        }
    }

    /// Rule 6: the batched off check agrees with probing each box alone,
    /// across single answers that toggle the forms' conditions (amended,
    /// item answers, spouse, taxes withheld, tax type, nature, amounts).
    #[test]
    fn the_off_check_agrees_with_each_box_alone_across_answers() {
        setup();
        let states: Vec<(&str, Value)> = vec![
            ("0605", json!({"frm0605:itemModeOfPayment:_1": "true"})),
            ("0605", json!({"frm0605:itemMannerOfPayment:_5": "true"})),
            ("0619F", json!({"frm0619F:optWithheld:Y": "true"})),
            (
                "0619F",
                json!({"frm0619F:optWithheld:Y": "true", "frm0619F:txtTaxTypeCode": "WF"}),
            ),
            ("1600WP", json!({"frm1600WP:AnyTaxHeld_1": "true"})),
            ("1601C", json!({"frm1601c:SpecialTax_1": "true"})),
            ("1601C", json!({"frm1601c:AmendedRtn_1": "true"})),
            ("1601EQ", json!({"frm1601EQ:txtTax20": "1000"})),
            ("1602Q", json!({"frm1602Q:txt18": "1000"})),
            ("1604C", json!({"frm1604c:releasedOfFunds_1": "true"})),
            ("1604F", json!({"frm1604f:taxRelief_1": "true"})),
            ("1701Q", json!({"frm1701q:optType_1": "true"})),
            (
                "1701Q",
                json!({"frm1701q:optType_1": "true", "frm1701q:txtSpouseName": "JUAN DELA CRUZ"}),
            ),
            ("2000OT", json!({"frm2000OT:rbTransNature_2": "true"})),
            ("2200A", json!({"frm2200Av2020:optTreaty_1": "true"})),
            ("2553", json!({"frm2553:txt13_1": "true"})),
        ];
        for (code, edits) in states {
            let opened = open(code, page_period_of(code));
            let on = fill(code, &opened["draft"], edits.clone());
            let checked = validated(code, &on["draft"]);
            assert_eq!(
                off_keys(&checked),
                oracle_off(code, &checked),
                "{code} after {edits}"
            );
        }
    }

    /// Rule 8 on every form whose binding data puts Surcharge, Interest and
    /// Compromise on the shared penalty model: computed and locked by
    /// default, one Edit (sent as `unlocked`) opens the three, the total and
    /// the amount payable recompute from what was entered, the field map
    /// (the XML) carries it, and blanks bring the computed amounts back.
    #[test]
    fn penalties_are_computed_and_one_edit_overrides_all_three_on_every_form() {
        setup();
        // Parties with all three overrides on the page, read off
        // crates/bir-core/data/agent-forms (`.../penalty_overrides/...`),
        // except 0605, whose boxes are open
        // (only_0605_penalties_are_three_open_boxes).
        let expected: BTreeMap<&str, usize> = BTreeMap::from([
            ("0619E", 1),
            ("0619F", 1),
            ("1600PT", 1),
            ("1600VT", 1),
            ("1600WP", 1),
            ("1601C", 1),
            ("1601EQ", 1),
            ("1602Q", 1),
            ("1603Q", 1),
            ("1606", 1),
            ("1700", 2),
            ("1701A", 2),
            ("1701MS", 2),
            ("1702MX", 1),
            ("1702Q", 1),
            ("1702RT", 1),
            ("1706", 1),
            ("1707", 1),
            ("1707A", 1),
            ("1800", 1),
            ("1801", 1),
            ("2000", 1),
            ("2000OT", 1),
            ("2200A", 1),
            ("2200AN", 1),
            ("2200C", 1),
            ("2200M", 1),
            ("2200P", 1),
            ("2200T", 1),
            ("2550M", 1),
            ("2552", 1),
            ("2553", 1),
        ]);
        let mut found = BTreeMap::new();
        let mut not_on_return = Vec::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            let opened = open(code, page_period_of(code));
            for group in opened["penalties"].as_array().unwrap() {
                *found.entry(code).or_insert(0) += 1;
                assert_eq!(
                    group["edited"], false,
                    "{code}: a new return files the computed amounts"
                );
                let keys: Vec<String> = strings(&group["keys"]).into_iter().collect();
                // The component each box overrides, from its binding.
                let component = |key: &str| match &def.bindings[key].binding {
                    Binding::Amount { path, .. } => path.rsplit('/').next().unwrap().to_string(),
                    other => panic!("{code}: {key} is {other:?}"),
                };
                let typed = |key: &str| match component(key).as_str() {
                    "surcharge" => 100.0,
                    "interest" => 200.0,
                    _ => 300.0,
                };
                for key in &keys {
                    assert!(
                        !opened["values"][key].as_str().unwrap_or("").is_empty(),
                        "{code}: {key} shows its amount"
                    );
                }
                let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
                if group["applies"] == false {
                    // A party not on this return: no Edit, and what is typed
                    // there would not file.
                    for key in &keys {
                        assert_eq!(
                            opened["locks"][key],
                            penalties::NOT_ON_RETURN,
                            "{code}: {key}"
                        );
                    }
                    let edits: Map<String, Value> =
                        keys.iter().map(|k| (k.clone(), json!("1"))).collect();
                    let filled = fill_unlocked(code, &opened["draft"], Value::Object(edits), &refs);
                    let state = filled["penalties"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|g| g["keys"] == group["keys"])
                        .unwrap();
                    assert_eq!(
                        state["edited"], false,
                        "{code}: {keys:?} is cleared by its form"
                    );
                    not_on_return.push(format!("{code} {}", keys[0]));
                    continue;
                }
                for key in &keys {
                    assert_eq!(
                        opened["locks"][key],
                        penalties::LOCK,
                        "{code}: {key} is locked"
                    );
                }
                let refused = fill(code, &opened["draft"], json!({ &keys[0]: "1" }));
                assert!(
                    refused["rejected"][&keys[0]].is_string(),
                    "{code}: a computed penalty needs Edit"
                );
                // One Edit: all three, typed.
                let edits: Map<String, Value> = keys
                    .iter()
                    .map(|k| (k.clone(), json!(typed(k).to_string())))
                    .collect();
                let edited = fill_unlocked(code, &opened["draft"], Value::Object(edits), &refs);
                assert_eq!(edited["rejected"], json!({}), "{code}");
                let state = edited["penalties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|g| g["keys"] == group["keys"])
                    .unwrap();
                assert_eq!(state["edited"], true, "{code}: {keys:?}");
                let map = agent::field_map(kind, &edited["draft"]).unwrap();
                for key in &keys {
                    assert_eq!(
                        amount(&edited["values"], key),
                        typed(key),
                        "{code}: {key} shows the entry"
                    );
                    assert_eq!(
                        agent::parse_amount(&map[key]),
                        Some(typed(key)),
                        "{code}: {key} files the entry"
                    );
                }
                // The total is the sum, and the amount payable moves with it.
                let before: f64 = keys.iter().map(|k| amount(&opened["values"], k)).sum();
                // Every other box the page shows.
                let others: Vec<&String> = edited["values"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|k| !keys.contains(k))
                    .collect();
                let total = others.iter().any(|k| {
                    agent::parse_amount(edited["values"][k.as_str()].as_str().unwrap_or(""))
                        == Some(600.0)
                });
                assert!(total, "{code}: a total shows 600.00");
                // Total Penalties and the amount payable both move by the change.
                let moved = others
                    .iter()
                    .filter(|k| {
                        let was = agent::parse_amount(opened["values"][k.as_str()].as_str().unwrap_or(""));
                        let now = agent::parse_amount(edited["values"][k.as_str()].as_str().unwrap_or(""));
                        matches!((was, now), (Some(was), Some(now)) if (now - was - (600.0 - before)).abs() < 0.005)
                    })
                    .count();
                assert!(
                    moved >= 2,
                    "{code}: the total and the amount payable recompute ({moved})"
                );
                // Edited, the group stays open; blanks give the computed amounts back.
                let again = fill(code, &edited["draft"], json!({ &keys[0]: "50" }));
                assert_eq!(
                    again["rejected"],
                    json!({}),
                    "{code}: an edited group stays open"
                );
                let blanks: Map<String, Value> =
                    keys.iter().map(|k| (k.clone(), json!(""))).collect();
                let back = fill_unlocked(code, &again["draft"], Value::Object(blanks), &refs);
                assert_eq!(back["rejected"], json!({}), "{code}");
                let state = back["penalties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|g| g["keys"] == group["keys"])
                    .unwrap();
                assert_eq!(state["edited"], false, "{code}: blanks clear the overrides");
                for key in &keys {
                    assert_eq!(
                        back["values"][key], opened["values"][key],
                        "{code}: {key} is computed again"
                    );
                }
                // The assistant's fill is as it was.
                agent::fill(kind, &mut opened["draft"].clone(), &keys[0], &json!("1"))
                    .unwrap_or_else(|e| panic!("{code}: {e}"));
            }
        }
        assert_eq!(found, expected);
        // The spouse's column on these returns with no spouse.
        not_on_return.sort();
        assert_eq!(
            not_on_return,
            [
                "1700 frm1700:txtPg1I31B",
                "1701A frm1701A:txt25B",
                "1701MS frm1701MS:txtCompromise28b",
            ]
        );
    }

    /// Rule 8 on 0619E: one Edit opens the three, but only a box the filer
    /// types becomes an override; the others stay computed and follow the
    /// tax due. Blanking a partly edited group clears all of it, a box not
    /// opened stays refused even in a batch with an opened one, and a
    /// penalty message lands on its box.
    /// Decision of 2026-10-17: 0605 has no due date to compute a penalty
    /// from, so its Surcharge, Interest and Compromise (20A-20C) are plain
    /// open boxes, with no Edit and no lock, and 20D Total Penalties stays
    /// computed. That is 0605 alone: across every form, no other penalty
    /// group is open (each keeps its locked group and Edit).
    #[test]
    fn only_0605_penalties_are_three_open_boxes() {
        setup();
        let mut open_forms = Vec::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let groups = penalties::groups(kind);
            let opened = open(code, page_period_of(code));
            if groups.is_empty() || !opened["penalties"].as_array().unwrap().is_empty() {
                continue;
            }
            open_forms.push(code);
            assert_eq!(opened["penalties"], json!([]), "{code}: no Edit");
            let def = kind.definition();
            for group in &groups {
                let typed = |key: &str| match &def.bindings[key].binding {
                    Binding::Amount { path, .. } if path.ends_with("/surcharge") => 100.0,
                    Binding::Amount { path, .. } if path.ends_with("/interest") => 200.0,
                    _ => 300.0,
                };
                for key in &group.keys {
                    assert!(opened["locks"][key].is_null(), "{code}: {key} is open");
                    assert!(opened["lock_notes"][key].is_null(), "{code}: {key}");
                }
                // Typed with nothing opened first, and filed as typed.
                let edits: Map<String, Value> = group
                    .keys
                    .iter()
                    .map(|k| (k.clone(), json!(typed(k).to_string())))
                    .collect();
                let filled = fill(code, &opened["draft"], Value::Object(edits));
                assert_eq!(filled["rejected"], json!({}), "{code}");
                let map = agent::field_map(kind, &filled["draft"]).unwrap();
                for key in &group.keys {
                    assert_eq!(amount(&filled["values"], key), typed(key), "{code}: {key}");
                    assert_eq!(
                        agent::parse_amount(&map[key]),
                        Some(typed(key)),
                        "{code}: {key} files the entry"
                    );
                }
                // The total still adds them up.
                let total = filled["values"].as_object().unwrap().iter().any(|(k, v)| {
                    !group.keys.contains(k)
                        && agent::parse_amount(v.as_str().unwrap_or("")) == Some(600.0)
                });
                assert!(total, "{code}: a total shows 600.00");
                // A blank box clears its amount (nothing is computed: 0).
                let blanks: Map<String, Value> =
                    group.keys.iter().map(|k| (k.clone(), json!(""))).collect();
                let back = fill(code, &filled["draft"], Value::Object(blanks));
                assert_eq!(back["rejected"], json!({}), "{code}");
                for key in &group.keys {
                    assert_eq!(amount(&back["values"], key), 0.0, "{code}: {key}");
                }
            }
        }
        open_forms.sort_unstable();
        assert_eq!(open_forms, ["0605"]);
        // 0605 by name: 20A-20C open, 20D Total Penalties computed.
        let opened = open("0605", 1);
        for key in [
            "frm0605:txtTax20A",
            "frm0605:txtTax20B",
            "frm0605:txtTax20C",
        ] {
            assert!(opened["locks"][key].is_null(), "{key}");
        }
        assert_eq!(opened["lock_notes"]["frm0605:txtTax20D"], NOTE_COMPUTED);
        let filled = fill(
            "0605",
            &opened["draft"],
            json!({"frm0605:txtTax20A": "10", "frm0605:txtTax20B": "20.50", "frm0605:txtTax20C": "30"}),
        );
        assert_eq!(filled["rejected"], json!({}));
        assert_eq!(amount(&filled["values"], "frm0605:txtTax20D"), 60.5);
    }

    #[test]
    fn only_a_typed_penalty_overrides_and_the_others_keep_computing() {
        setup();
        let code = "0619E";
        let [sur, int, comp] = [
            "frm0619E:txtTax17A",
            "frm0619E:txtTax17B",
            "frm0619E:txtTax17C",
        ];
        let opened = open(code, 1);
        let due = fill(
            code,
            &opened["draft"],
            json!({"frm0619E:optWithheld:Y": "true", "frm0619E:txtTax14": "10000"}),
        );
        assert_eq!(due["rejected"], json!({}));
        let computed =
            |draft: &Value, tax: &str| fill(code, draft, json!({"frm0619E:txtTax14": tax}));
        // Typed Compromise only.
        let typed = fill_unlocked(
            code,
            &due["draft"],
            json!({ comp: "500" }),
            &[sur, int, comp],
        );
        assert_eq!(typed["rejected"], json!({}));
        assert_eq!(
            typed["penalties"][0]["edited"], true,
            "an edited group stays open"
        );
        assert!(
            typed["draft"]["penalty_overrides"]["surcharge"].is_null(),
            "surcharge stays computed"
        );
        assert!(
            typed["draft"]["penalty_overrides"]["interest"].is_null(),
            "interest stays computed"
        );
        assert_eq!(amount(&typed["values"], comp), 500.0);
        // The tax due moves: surcharge and interest follow it, compromise holds.
        let moved = computed(&typed["draft"], "20000");
        let reference = computed(&due["draft"], "20000");
        assert_eq!(
            moved["values"][sur], reference["values"][sur],
            "surcharge recomputes"
        );
        assert_eq!(
            moved["values"][int], reference["values"][int],
            "interest keeps accruing"
        );
        assert_ne!(moved["values"][sur], typed["values"][sur]);
        assert_eq!(
            amount(&moved["values"], comp),
            500.0,
            "the typed compromise holds"
        );
        // Edited, a later box needs no Edit; blanks from that partly edited
        // state clear every override, none refused.
        let two = fill(code, &typed["draft"], json!({ sur: "100" }));
        assert_eq!(two["rejected"], json!({}));
        let back = fill(code, &two["draft"], json!({ sur: "", int: "", comp: "" }));
        assert_eq!(
            back["rejected"],
            json!({}),
            "use the computed amounts from a partial state"
        );
        assert_eq!(back["penalties"][0]["edited"], false);
        assert_eq!(back["values"][comp], due["values"][comp]);
        // A box not opened is refused even beside an opened one.
        let batch = fill_unlocked(
            code,
            &due["draft"],
            json!({ sur: "100", int: "200" }),
            &[sur],
        );
        assert!(batch["rejected"][int].is_string(), "{}", batch["rejected"]);
        assert!(batch["draft"]["penalty_overrides"]["interest"].is_null());
        // A negative amount's message sits on its box.
        let negative = fill_unlocked(code, &due["draft"], json!({ sur: "-5000" }), &[sur]);
        let checked = validated(code, &negative["draft"]);
        let message = checked["errors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["field"] == "item_17a_surcharge")
            .expect("a message for the negative surcharge");
        assert_eq!(message["keys"], json!([sur]));
    }

    /// Rule 8 on 1700's spouse column of a joint return: Total Penalties is a
    /// computed box though bound to the compromise override, so it is not one
    /// of the three, and an amount typed in 33B stays.
    #[test]
    fn a_spouse_penalty_typed_on_a_joint_return_stays() {
        setup();
        let code = "1700";
        let opened = open(code, 0);
        let joint = fill(
            code,
            &opened["draft"],
            json!({
                "frm1700:rdoPg1I15CivilStatusM": "true",
                "frm1700:rdoPg1I16SpouseHasIncomeY": "true",
                "frm1700:rdoPg1I17FilingStatusJ": "true",
            }),
        );
        assert_eq!(joint["rejected"], json!({}));
        let group = joint["penalties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| strings(&g["keys"]).contains("frm1700:txtPg1I33B"))
            .expect("the spouse's penalties")
            .clone();
        let keys = strings(&group["keys"]);
        assert!(
            !keys.contains("frm1700:txtPg1I34B"),
            "Total Penalties is not one of the three: {keys:?}"
        );
        assert_eq!(keys.len(), 3, "{keys:?}");
        assert_eq!(
            group["applies"], true,
            "a joint return has the spouse's column"
        );
        let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let typed = fill_unlocked(
            code,
            &joint["draft"],
            json!({"frm1700:txtPg1I33B": "500"}),
            &refs,
        );
        assert_eq!(typed["rejected"], json!({}));
        assert_eq!(
            amount(&typed["values"], "frm1700:txtPg1I33B"),
            500.0,
            "the typed amount stays"
        );
        assert_eq!(
            amount(&typed["values"], "frm1700:txtPg1I34B"),
            500.0
                + amount(&typed["values"], "frm1700:txtPg1I31B")
                + amount(&typed["values"], "frm1700:txtPg1I32B")
        );
    }

    #[test]
    fn an_escaped_official_value_shows_as_typed() {
        setup();
        let opened = open("1601EQ", 1);
        let key = "frm1601EQ:txtLineBus";
        assert_eq!(opened["values"][key], "Wholesale trading");
        // The official field map keeps the runtime's escape().
        let map = agent::field_map(
            AgentFormKind::from_code("1601EQ").unwrap(),
            &opened["draft"],
        )
        .unwrap();
        assert_eq!(map[key], "Wholesale%20trading");
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
        // The repeat is never typed: it names its box, where Edit is.
        assert_eq!(
            opened["locks"][repeat],
            format!("repeats `{base}` from the tax profile; use Edit on that box")
        );
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
