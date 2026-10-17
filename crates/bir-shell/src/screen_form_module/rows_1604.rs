//! PR 6: the annual information returns (1604C, 1604F, 1604E). An answer
//! that closes a section clears what the filer typed there (the official
//! handlers, kept by each form's `recompute`): 1604C Item 11 No clears
//! Items 11A to 13 (`changeRefund`), 1604F Item 11 No clears Item 11A
//! (`TaxReliefEnable`). The page asks first and offers Undo. A Part II
//! Date of Remittance the filer clears is cleared.

use bir_core::forms::agent::{AgentFormKind, Binding, DateFormat};
use serde_json::{Map, Value, json};

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

fn text(draft: &Value, field: &str) -> String {
    draft[field].as_str().unwrap_or("").trim().to_string()
}

fn amount(draft: &Value, field: &str) -> f64 {
    draft[field].as_f64().unwrap_or(0.0)
}

/// `1234.5` -> `1,234.50`.
fn pesos(amount: f64) -> String {
    let cents = (amount * 100.0).round() as i64;
    let negative = cents < 0;
    let (whole, cents) = ((cents / 100).abs(), (cents % 100).abs());
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{}{grouped}.{cents:02}", if negative { "-" } else { "" })
}

/// When `edits` turned `before` into `after` by clearing what the filer
/// typed, what the page asks first (`message`), what the Undo toast says
/// (`done`: the items actually cleared) and how many rows went (none: these
/// are items); `None` when nothing typed went. The host returns the edited
/// draft; the page holds it, still showing the old one, and takes it only
/// when the filer chooses Continue (Cancel and Esc drop it).
pub(super) fn removes(
    kind: AgentFormKind,
    before: &Value,
    after: &Value,
    edits: &Map<String, Value>,
) -> Option<Value> {
    let edited = |prefix: &str| edits.keys().any(|key| key.starts_with(prefix));
    let closed = |flag: &str| before[flag] == true && after[flag] == false;
    match kind.form_code() {
        "1604C" if edited("frm1604c:releasedOfFunds") && closed("refunds_released") => {
            let (mut lost, mut items) = (Vec::new(), Vec::new());
            let date = text(before, "refund_date");
            if !date.is_empty() && text(after, "refund_date").is_empty() {
                lost.push(format!("item 11A (date of refund {date})"));
                items.push("11A".to_string());
            }
            let over = amount(before, "overremittance");
            if over != 0.0 && amount(after, "overremittance") == 0.0 {
                lost.push(format!("item 12 (₱{})", pesos(over)));
                items.push("12".to_string());
            }
            let month = before["first_crediting_month"].as_u64().unwrap_or(0);
            if (1..=12).contains(&month)
                && after["first_crediting_month"].as_u64().unwrap_or(0) == 0
            {
                lost.push(format!("item 13 ({})", MONTHS[month as usize - 1]));
                items.push("13".to_string());
            }
            if lost.is_empty() {
                return None;
            }
            let done = match items.as_slice() {
                [one] => format!("Cleared item {one}"),
                _ => format!("Cleared items {}", join(&items)),
            };
            Some(json!({
                "message": format!("Changing item 11 to No clears {}. Continue?", join(&lost)),
                "done": done,
                "rows": 0,
            }))
        }
        "1604F" if edited("frm1604f:taxRelief") && closed("tax_relief") => {
            let relief = text(before, "tax_relief_details");
            if relief.is_empty() || !text(after, "tax_relief_details").is_empty() {
                return None;
            }
            Some(json!({
                "message": format!("Changing item 11 to No clears item 11A ({relief}). Continue?"),
                "done": "Cleared item 11A",
                "rows": 0,
            }))
        }
        _ => None,
    }
}

/// The draft path a cleared Part II Date of Remittance empties: the key is
/// one of the 1604C/E/F schedule date boxes (a whole MM/DD/YYYY date) and
/// the page sent blank. The binding writes only a whole date, so without
/// this a filer could not empty a date they typed; other text is still
/// judged by the binding (MM/DD/YYYY or refused), as on every date box.
pub(super) fn cleared_date(kind: AgentFormKind, key: &str, value: &Value) -> Option<String> {
    if !matches!(kind.form_code(), "1604C" | "1604E" | "1604F") || !is_part_ii_date(key) {
        return None;
    }
    let blank = value.is_null() || value.as_str().is_some_and(|text| text.trim().is_empty());
    if !blank {
        return None;
    }
    match kind
        .definition()
        .bindings
        .get(key)
        .map(|bound| &bound.binding)
    {
        Some(Binding::Text {
            path,
            date: Some(DateFormat::Mdy),
            part: None,
            ..
        }) => Some(path.clone()),
        _ => None,
    }
}

/// A Part II Date of Remittance box of 1604C, 1604E or 1604F.
pub(super) fn is_part_ii_date(key: &str) -> bool {
    [
        "frm1604c:txtSched1Date",
        "frm1604e:txtSched1RemDate",
        "frm1604e:txtSched2RemDate",
        "frm1604f:txtSched1Date",
        "frm1604f:txtSched2Date",
        "frm1604f:txtSched3Date",
    ]
    .iter()
    .any(|stem| {
        key.strip_prefix(stem)
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    })
}

/// `a`, `a and b`, `a, b and c`.
fn join(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [head @ .., last] => format!("{} and {last}", head.join(", ")),
    }
}
