//! The cached reads over the door (port of `nsh/shell/store.ts`).
//!
//! A frame must not hit the database for every widget: every read here is memoised by key until a write
//! drops it — same contract as the TS store, minus the signals (the CUI frame re-reads every frame, so
//! the cache is what keeps that cheap). A read that fails answers its fallback, is logged once, and is
//! asked again after a backoff (2 s, 4 s, 8 s ... a minute) measured from `Instant`s polled each frame.
//!
//!   let list = store.taxpayers();                  // cached; re-asks after a write drops the key
//!   store.write("set_setting", json!(["k", "v"])); // a write: runs, then drops every cached read

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::bir;
use crate::lib::errors::error_text;

/// A read that failed answers its fallback for this long, then is asked again.
pub const FAILED_READ_RETRY: Duration = Duration::from_secs(2);
/// The longest a failing read waits between two asks.
pub const FAILED_READ_RETRY_MAX: Duration = Duration::from_secs(60);

/// How long a key that has now failed `failures` times in a row answers its fallback.
pub fn failed_read_delay(failures: u32) -> Duration {
    FAILED_READ_RETRY
        .saturating_mul(
            1u32.checked_shl(failures.saturating_sub(1))
                .unwrap_or(u32::MAX),
        )
        .min(FAILED_READ_RETRY_MAX)
}

/// The host functions that queue a return or take it out of the queue; after one, the worker-moved lists
/// are read again now and then (the frame's poll loop reads `take_rereads`).
pub const QUEUE_FUNCTIONS: [&str; 4] = [
    "queue_form_draft",
    "cancel_queued",
    "screen_form_queue",
    "screen_form_cancel_queue",
];

/// Milliseconds after a queue action the lists are read again (the TS QUEUE_REREAD_MS).
pub const QUEUE_REREAD: [u64; 3] = [1000, 3000, 10_000];

/// What the plain 30 s events poll drops (the TS POLL_KEYS, as a matcher): the worker keys less
/// the drafts count and the as-needed forms.
pub fn poll_key(key: &str) -> bool {
    key.starts_with("returns:")
        || key.starts_with("deadlines")
        || key.starts_with("forms:")
        || key == "jobs"
}

/// What the worker changes behind the app's back (the TS WORKER_KEYS, as a matcher).
pub fn worker_key(key: &str) -> bool {
    poll_key(key) || key == "saved_returns" || key.starts_with("as_needed:")
}

struct Entry {
    /// When the cached value was read; `None` = never read or dropped.
    read_at: Option<Instant>,
    /// When the last read failed (the fallback is being served).
    failed_at: Option<Instant>,
    /// Consecutive failures since the key was last forgotten or read well (the backoff step).
    failures: u32,
    value: Value,
}

/// What one queued re-read wants: the instant it is due.
#[derive(Debug)]
pub struct Reread {
    pub at: Instant,
}

#[derive(Default)]
struct State {
    entries: HashMap<String, Entry>,
    /// The TIN root a taxpayer sign-in narrows everything to, or `None` (all taxpayers).
    scope: Option<String>,
    /// Pending queue-action re-reads, soonest first.
    rereads: Vec<Instant>,
    /// When the last queue action happened, for the Queued watch's fast phase.
    last_queue_action: Option<Instant>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// A screen failure for the daily log and the Logs tab (`record_screen_log`); logging never hides it.
pub fn log_screen(level: &str, message: &str) {
    let _ = bir::call(
        "record_screen_log",
        vec![Value::from(level), Value::from(message)],
    );
}

/// An explicit drop: what read the key asks again, and its backoff starts over.
pub fn forget(key: &str) {
    state(|s| {
        if let Some(e) = s.entries.get_mut(key) {
            e.read_at = None;
            e.failed_at = None;
            e.failures = 0;
        }
    });
}

/// Drops every cached read whose key matches `test`.
pub fn forget_where(test: impl Fn(&str) -> bool) {
    let keys: Vec<String> = state(|s| s.entries.keys().filter(|k| test(k)).cloned().collect());
    for key in keys {
        forget(&key);
    }
}

/// Drops every cached read. `written` names the host function that was written; a queue or
/// cancel-queue also schedules the worker-list re-reads.
pub fn bump(written: Option<&str>) {
    state(|s| {
        for e in s.entries.values_mut() {
            e.read_at = None;
            e.failed_at = None;
            e.failures = 0;
        }
    });
    if let Some(name) = written
        && QUEUE_FUNCTIONS.contains(&name)
    {
        reread_soon();
    }
}

/// A return was queued or taken out of the queue by a path that does not go through `write`.
pub fn after_queue_change() {
    reread_soon();
}

fn reread_soon() {
    let now = Instant::now();
    state(|s| {
        s.last_queue_action = Some(now);
        s.rereads = QUEUE_REREAD
            .iter()
            .map(|ms| now + Duration::from_millis(*ms))
            .collect();
    });
    forget_where(worker_key);
}

/// The pending re-reads after a queue action, due ones removed (the frame calls this each tick and
/// drops worker keys when one comes due).
pub fn take_due_rereads() -> bool {
    let now = Instant::now();
    let due = state(|s| {
        let mut due = false;
        while s.rereads.first().is_some_and(|at| *at <= now) {
            s.rereads.remove(0);
            due = true;
        }
        due
    });
    if due {
        forget_where(worker_key);
    }
    due
}

/// Cancels the pending re-reads of a queue action (the session they were for has ended).
pub fn cancel_rereads() {
    state(|s| s.rereads.clear());
}

/// When the last queue action happened, for the Queued watch's fast phase.
pub fn last_queue_action() -> Option<Instant> {
    state(|s| s.last_queue_action)
}

/// Whether `key` holds an answer already (a read that is not dropped yet).
pub fn has(key: &str) -> bool {
    state(|s| s.entries.get(key).is_some_and(|e| e.read_at.is_some()))
}

/// Whether the last read of `key` failed and holds its fallback.
pub fn failed(key: &str) -> bool {
    state(|s| {
        s.entries.get(key).is_some_and(|e| {
            e.failed_at.is_some()
                && e.failed_at
                    .is_some_and(|at| at.elapsed() < failed_read_delay(e.failures))
        })
    })
}

/// The session's scope: the TIN root a taxpayer sign-in narrows everything to, or `None`.
pub fn scope() -> Option<String> {
    state(|s| s.scope.clone())
}

/// Narrows `taxpayers()` to one taxpayer for a taxpayer sign-in; `None` shows all. Only the app state calls it.
pub fn set_scope(root: Option<String>) {
    state(|s| s.scope = root);
}

/// Whether the session may see a taxpayer's deadlines and returns.
pub fn can_see(root: &str) -> bool {
    match scope().as_deref() {
        Some(s) => s == root,
        None => true,
    }
}

/// A memoised read of `key`: `f` once, then the cached answer until forgotten; `fallback` while `f`
/// fails (asked again after the backoff). `f` returns the host value or an `Err` sentence.
pub fn read<T>(key: &str, fallback: T, f: impl FnOnce() -> Result<T, String>) -> T
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone,
{
    // Fast path: a good cached value, or a failure still inside its backoff (serve the fallback).
    enum Hit<T> {
        Value(T),
        Fallback,
        Miss,
    }
    let hit = state(|s| match s.entries.get(key) {
        Some(e) if e.read_at.is_some() => {
            if e.failed_at.is_none() {
                Hit::Value(
                    serde_json::from_value::<T>(e.value.clone())
                        .unwrap_or_else(|_| fallback.clone()),
                )
            } else if e
                .failed_at
                .is_some_and(|at| at.elapsed() < failed_read_delay(e.failures))
            {
                Hit::Fallback
            } else {
                Hit::Miss
            }
        }
        _ => Hit::Miss,
    });
    match hit {
        Hit::Value(v) => return v,
        Hit::Fallback => return fallback,
        Hit::Miss => {}
    }
    match f() {
        Ok(value) => {
            let json = serde_json::to_value(&value).unwrap_or(Value::Null);
            state(|s| {
                let e = s.entries.entry(key.to_owned()).or_insert(Entry {
                    read_at: None,
                    failed_at: None,
                    failures: 0,
                    value: Value::Null,
                });
                e.read_at = Some(Instant::now());
                e.failed_at = None;
                e.failures = 0;
                e.value = json;
            });
            value
        }
        Err(error) => {
            // Log once per distinct error.
            let text = error.clone();
            let first = state(|s| {
                let e = s.entries.entry(key.to_owned()).or_insert(Entry {
                    read_at: None,
                    failed_at: None,
                    failures: 0,
                    value: Value::Null,
                });
                let first = e.value.as_str() != Some(text.as_str());
                let now = Instant::now();
                e.read_at = Some(now);
                e.failed_at = Some(now);
                e.failures += 1;
                e.value = Value::from(text);
                first
            });
            if first {
                log_screen("error", &format!("{key}: {}", error_text(&error)));
            }
            fallback
        }
    }
}

/// A host call that is not cached (a verify, a one-off read). Throws what the host throws.
pub fn call(name: &str, args: Vec<Value>) -> Result<Value, String> {
    bir::call(name, args)
}

/// A write: runs the host function, then drops every cached read (also when it throws); a failure is
/// logged and returned as `Err`.
pub fn write(name: &str, args: Vec<Value>) -> Result<Value, String> {
    let result = call(name, args.clone());
    if let Err(error) = &result {
        log_screen("error", &format!("bir.{name}: {}", error_text(error)));
    }
    bump(Some(name));
    result
}

// -------------------------------------------------------------------------------- typed reads
// The memoised reads the screens share. Each returns the host's JSON decoded into the shapes the
// screens already know (serde structs live beside their screen or in `types.rs`).

/// `store.taxpayers()`: the taxpayers the session may see.
pub fn taxpayers() -> Vec<crate::types::Taxpayer> {
    let all: Vec<crate::types::Taxpayer> = read("taxpayers", Vec::new(), || {
        bir::call0("list_taxpayers")
            .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    });
    match scope().as_deref() {
        Some(s) => all.into_iter().filter(|t| t.tin_root == s).collect(),
        None => all,
    }
}

/// `store.taxYears(tin)`: the branch's tax years.
pub fn tax_years(tin: &str) -> Vec<i64> {
    read(&format!("years:{tin}"), Vec::new(), || {
        bir::call1("list_tax_years", Value::from(tin))
            .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    })
}

/// `store.yearlyForms(tin, year)`: the form codes the branch files in the year.
pub fn yearly_forms(tin: &str, year: i64) -> Vec<String> {
    read(&format!("forms:{tin}:{year}"), Vec::new(), || {
        bir::call("yearly_forms", vec![Value::from(tin), Value::from(year)])
            .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    })
}

/// `store.templates(tin)`: the branch's form templates.
pub fn templates(tin: &str) -> Vec<crate::types::FormTemplateSummary> {
    read(&format!("templates:{tin}"), Vec::new(), || {
        bir::call1("list_form_templates", Value::from(tin))
            .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    })
}

/// `store.catalog()`: every form's catalog entry.
pub fn catalog() -> Vec<crate::types::FormCatalogEntry> {
    read("forms_catalog", Vec::new(), || {
        bir::call0("forms_catalog")
            .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    })
}

/// `store.systemInfo()`: os/arch/paths/hotkey.
pub fn system_info() -> crate::types::SystemInfo {
    read("system_info", crate::types::SystemInfo::default(), || {
        bir::call0("system_info").and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    })
}

/// `store.authState()`: the access mode and whether the admin has a code.
pub fn auth_state() -> crate::types::AuthState {
    read("auth_state", crate::types::AuthState::default(), || {
        bir::call0("auth_state").and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    })
}

/// `store.screenIndex()`: the form library's index, kept for the run past bump() (the library never
/// changes while the app runs).
pub fn screen_index() -> Vec<crate::types::ScreenFormIndexEntry> {
    read("screen_forms_index", Vec::new(), || {
        bir::call0("screen_forms_index")
            .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    })
}

/// `store.yearBounds()`: the tax years that may be added.
pub fn year_bounds() -> crate::types::TaxYearBounds {
    read(
        "tax_year_bounds",
        crate::types::TaxYearBounds {
            min: 2010,
            max: chrono::Local::now()
                .format("%Y")
                .to_string()
                .parse::<i64>()
                .unwrap_or(2030)
                + 1,
        },
        || {
            bir::call0("tax_year_bounds")
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        },
    )
}

/// `store.formDeadlines(tin, code, year, from, to)`: one branch's deadlines for one form and tax
/// year. The range is read once per data version and grouped, as the old store did.
pub fn form_deadlines(
    tin: &str,
    code: &str,
    year: i64,
    from: &str,
    to: &str,
) -> Vec<crate::types::DeadlineRow> {
    let rows: Vec<crate::types::DeadlineRow> =
        read(&format!("deadlines:{from}:{to}"), Vec::new(), || {
            bir::call(
                "deadlines_between",
                vec![Value::from(from), Value::from(to)],
            )
            .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        });
    rows.into_iter()
        .filter(|d| d.tin == tin && d.form_code == code && d.taxable_year == year)
        .collect()
}

/// `store.setups(taxpayerType)`: the setups offered to a taxpayer type: built-in first, then the
/// user's.
pub fn setups(taxpayer_type: Option<&str>) -> Vec<crate::types::Setup> {
    match taxpayer_type {
        Some(t) => read(&format!("setups:{t}"), Vec::new(), || {
            bir::call1("list_setups", Value::from(t))
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        }),
        None => Vec::new(),
    }
}

/// `store.annualConflicts(codes)`: annual income tax returns among codes that stand for one
/// another, as groups.
pub fn annual_conflicts(codes: &[String]) -> Vec<Vec<String>> {
    if codes.len() <= 1 {
        return Vec::new();
    }
    let mut list = codes.to_vec();
    list.sort();
    read(
        &format!("annual_conflicts:{}", list.join(",")),
        Vec::new(),
        || {
            bir::call("annual_return_conflicts", vec![Value::from(list.clone())])
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        },
    )
}

/// `store.inboxState(tin)`: the confirmation-inbox state for one branch profile; Null when the
/// read failed or there is no branch.
pub fn inbox_state(tin: Option<&str>) -> Value {
    match tin {
        Some(t) => read(&format!("inbox:{t}"), Value::Null, || {
            bir::call1("inbox_state", Value::from(t))
        }),
        None => Value::Null,
    }
}

/// `store.sendGrants(tin)`: standing send OKs for one branch.
pub fn send_grants(tin: Option<&str>) -> Vec<crate::types::SendGrant> {
    match tin {
        Some(t) => read(&format!("grants:{t}"), Vec::new(), || {
            bir::call1("list_send_grants", Value::from(t))
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        }),
        None => Vec::new(),
    }
}

/// `store.taxTypes(tin, year)`: the tax types the year's forms list gives, as chips.
pub fn tax_types(tin: Option<&str>, year: Option<i64>) -> Vec<crate::types::TaxTypeChip> {
    match (tin, year) {
        (Some(t), Some(y)) => read(&format!("tax_types:{t}:{y}"), Vec::new(), || {
            bir::call("tax_types_for_year", vec![Value::from(t), Value::from(y)])
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        }),
        _ => Vec::new(),
    }
}
