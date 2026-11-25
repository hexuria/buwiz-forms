//! 2550Q Schedules 1, 3 and 4 rows. The official upload writes a schedule as
//! its modal rows (`addlistSched1/3/4`) with empty rows left out, so a row
//! the filer leaves empty ahead of a filled one would move that row up on
//! the way out. The host keeps the rows gap-free instead: the next row
//! starts on its first box, a row past it is refused, and clearing a row
//! removes it, as the modal's Delete does. A schedule keeps the two rows the
//! form opens with.

use bir_core::forms::agent::{self, AgentFormKind};
use bir_core::forms::form_2550q::Form2550QDate;
use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Date,
    Text,
    Money,
    Whole,
}

/// `(key stem, draft list, field, kind)` in row order; a longer stem first
/// where one starts another (`txtDateCovered3To`, `txtDateCovered3`).
const CELLS: &[(&str, &str, &str, Kind)] = &[
    (
        "txtDatePurchase1",
        "/schedule_1",
        "purchase_or_import_date",
        Kind::Date,
    ),
    ("txtSourceCode1", "/schedule_1", "source_code", Kind::Text),
    ("txtDescription1", "/schedule_1", "description", Kind::Text),
    (
        "txtAmountPurchase1",
        "/schedule_1",
        "purchase_or_import_amount",
        Kind::Money,
    ),
    ("txtInputTax1", "/schedule_1", "input_tax", Kind::Money),
    (
        "txtEstimatedLife1",
        "/schedule_1",
        "estimated_life_months",
        Kind::Whole,
    ),
    (
        "txtRecognizedLife1",
        "/schedule_1",
        "recognized_life_months",
        Kind::Whole,
    ),
    ("txtDateCovered3To", "/schedule_3", "period_to", Kind::Date),
    ("txtDateCovered3", "/schedule_3", "period_from", Kind::Date),
    (
        "txtNameWithHoldingAgent3",
        "/schedule_3",
        "withholding_agent_name",
        Kind::Text,
    ),
    (
        "txtIncomePayment3",
        "/schedule_3",
        "income_payment",
        Kind::Money,
    ),
    (
        "txtTotalTaxWithHeld3",
        "/schedule_3",
        "tax_withheld",
        Kind::Money,
    ),
    ("txtDate4To", "/schedule_4", "period_to", Kind::Date),
    ("txtDate4", "/schedule_4", "period_from", Kind::Date),
    ("txtNameOfMiller4", "/schedule_4", "miller_name", Kind::Text),
    (
        "txtNameOfTaxpayer4",
        "/schedule_4",
        "taxpayer_name",
        Kind::Text,
    ),
    (
        "txtOfficialReceiptNumber4",
        "/schedule_4",
        "official_receipt_number",
        Kind::Text,
    ),
    ("txtAmountPaid4", "/schedule_4", "amount_paid", Kind::Money),
];

/// Rows a schedule keeps (`new_from_profile` opens each with two).
const MIN_ROWS: usize = 2;

/// `(index into CELLS, row)` of a 2550Q schedule box.
pub(super) fn cell(kind: AgentFormKind, key: &str) -> Option<(usize, usize)> {
    if kind.form_code() != "2550Q" {
        return None;
    }
    CELLS.iter().enumerate().find_map(|(at, (stem, ..))| {
        let rest = key.strip_prefix(stem)?;
        if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some((at, rest.parse().ok()?))
    })
}

/// Earlier rows first in a batch, then the row's own order.
pub(super) fn edit_order(kind: AgentFormKind, key: &str) -> Option<usize> {
    cell(kind, key).map(|(at, row)| 2 + row.saturating_mul(CELLS.len()).saturating_add(at))
}

fn blank(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => matches!(text.trim(), "" | "0.00"),
        Value::Number(number) => number.as_f64() == Some(0.0),
        Value::Bool(flag) => !flag,
        Value::Array(items) => items.is_empty(),
        Value::Object(_) => false,
    }
}

/// Every box of the row is empty (its stable row ID aside).
fn empty(row: &Value) -> bool {
    row.as_object().is_none_or(|fields| {
        fields
            .iter()
            .filter(|(name, _)| name.as_str() != "instance_id")
            .all(|(_, value)| blank(value))
    })
}

fn parse(kind: Kind, text: &str) -> Result<Value, String> {
    let text = text.trim();
    Ok(match kind {
        Kind::Date if text.is_empty() => Value::Null,
        Kind::Date => serde_json::to_value(Form2550QDate::parse_return_period(text)?)
            .map_err(|e| e.to_string())?,
        Kind::Text => Value::from(text),
        Kind::Money if text.is_empty() => Value::from(0.0),
        Kind::Money => match agent::parse_amount(text) {
            Some(amount) if amount >= 0.0 => Value::from(amount),
            _ => return Err(format!("`{text}` is not an amount.")),
        },
        Kind::Whole if text.is_empty() => Value::from(0),
        Kind::Whole => Value::from(
            text.parse::<u16>()
                .map_err(|_| format!("`{text}` is not a number of months."))?,
        ),
    })
}

pub(super) fn fill(
    kind: AgentFormKind,
    draft: &mut Value,
    (at, row): (usize, usize),
    value: &Value,
) -> Result<(), String> {
    let (_, list, field, cell_kind) = CELLS[at];
    let text = match value {
        Value::Number(n) => n.to_string(),
        other => other.as_str().unwrap_or_default().to_string(),
    };
    let parsed = parse(cell_kind, &text)?;
    let template = kind
        .definition()
        .rows
        .get(list)
        .cloned()
        .ok_or_else(|| format!("rows of `{list}` cannot be added here"))?;
    let rows = draft
        .pointer_mut(list)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("draft has no `{list}`"))?;
    let filling = !blank(&parsed);
    // The first row nothing is typed in yet: rows fill in order.
    let open = rows.iter().position(empty).unwrap_or(rows.len());
    let starts_row = rows.get(row).is_none_or(empty);
    if filling && starts_row && row > open {
        return Err(format!("Fill row {} of this schedule first.", open + 1));
    }
    if row >= rows.len() {
        if !filling {
            return Ok(());
        }
        rows.resize(row + 1, template.clone());
    }
    rows[row][field] = parsed;
    if empty(&rows[row]) {
        rows.remove(row);
        while rows.len() < MIN_ROWS {
            rows.push(template.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use bir_core::forms::agent::{self, AgentFormKind};
    use bir_core::forms::form_2550q::{Form2550QDate, Form2550QDraft};
    use bir_core::profile::TaxpayerProfile;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    fn sample_profile() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Sample Dummy Taxpayer",
            "tin": {
                "segment1": "123",
                "segment2": "456",
                "segment3": "788",
                "branch": "00000"
            },
            "rdo_code": "039",
            "line_of_business": "Sample Consulting Services",
            "registered_address": "123 Sample Street, Barangay Example, Quezon City",
            "zip_code": "1100",
            "phone": "09170000000",
            "email": "sample.taxpayer@example.com",
            "default_form_type": "2550Q",
            "taxpayer_type": "Corporation",
            "business_start_date": "2020-01-15",
            "tax_elections": [{
                "taxable_year": 2025,
                "election": "GraduatedUnspecified",
                "elected_at": "2025-01-15T00:00:00",
                "source_form": "sample"
            }]
        }))
        .expect("dummy profile must deserialize")
    }

    #[test]
    fn screen_path_reproduces_2550q_schedules_sample_without_differences() {
        let kind = AgentFormKind::from_code("2550Q").expect("2550Q kind");
        let schema_val = schema_entry("2550Q").expect("2550Q schema");
        let mut computed = BTreeSet::new();
        if let Some(parts) = schema_val["parts"].as_array() {
            for part in parts {
                if let Some(items) = part["items"].as_array() {
                    for item in items {
                        if let Some(fields) = item["fields"].as_array() {
                            for field in fields {
                                if field["c"].as_i64() == Some(1) {
                                    if let Some(k) = field["k"].as_str() {
                                        computed.insert(k.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let def = kind.definition();
        for b in &def.boxes {
            if b.computed || b.layout_only {
                computed.insert(b.key.clone());
            }
        }
        assert!(
            !computed.contains("txtTotalTaxWithHeld30")
                && !computed.contains("txtTotalTaxWithHeld31"),
            "Schedule 3 rows 0 and 1 tax withheld must not be marked computed"
        );

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../bir-core/tests/sample-payloads/2550Q-122025Q1-schedules.official.xml");
        let official_xml = std::fs::read_to_string(&path).expect("2550Q schedules official XML");
        let (controls, _) = official_xml
            .split_once("<dateFiled>")
            .expect("2550Q upload appends <dateFiled>");

        let mut official_map = BTreeMap::new();
        for line in controls.lines() {
            let trimmed = line.trim();
            let Some(inner) = trimmed
                .strip_prefix("<div>")
                .and_then(|s| s.strip_suffix("</div>"))
            else {
                continue;
            };
            let Some((k, v)) = inner.split_once('=') else {
                continue;
            };
            let clean_val = v.strip_suffix(&format!("{k}=")).unwrap_or(v);
            official_map.insert(k.to_string(), clean_val.to_string());
        }

        let mut base = Form2550QDraft::new_from_profile(&sample_profile(), 2025, 1);
        base.date_filed = Form2550QDate::new(2025, 4, 25).ok();
        let mut draft = serde_json::to_value(&base).expect("base serializes");

        let mut edits = Vec::new();
        for (k, v) in &official_map {
            if computed.contains(k) {
                continue;
            }
            if v == "false" && def.boxes.iter().any(|b| b.key == *k && b.kind == "radio") {
                continue;
            }
            if let Some(binding) = def.bindings.get(k) {
                if binding.lock.is_some() {
                    continue;
                }
            }
            edits.push((k.clone(), Value::String(v.clone())));
        }

        // Apply row edits in row-index order so rows 0, 1, 2 fill sequentially.
        edits.sort_by_key(|(k, _)| {
            let row = super::cell(kind, k).map(|(_, r)| r).unwrap_or(usize::MAX);
            (row, k.clone())
        });

        let unlocked = BTreeSet::new();
        for (k, v) in edits {
            let mut single = serde_json::Map::new();
            single.insert(k.clone(), v);
            let _ = apply_screen_edits(kind, &mut draft, &single, true, &unlocked);
        }
        agent::compute(kind, &mut draft).expect("compute succeeds");

        let our_map = agent::field_map(kind, &draft).expect("field map");
        let mut diffs = Vec::new();
        for (k, official_val) in &official_map {
            let our_val = our_map.get(k).map(String::as_str).unwrap_or("");
            if our_val != official_val {
                diffs.push((k.clone(), official_val.clone(), our_val.to_string()));
            }
        }
        assert!(
            diffs.is_empty(),
            "2550Q-122025Q1-schedules has differences on the screen path: {diffs:#?}"
        );
        assert_eq!(
            our_map.get("txtTotalTaxWithHeld30").map(String::as_str),
            Some("5,000.00")
        );
        assert_eq!(
            our_map.get("txtTotalTaxWithHeld31").map(String::as_str),
            Some("10,000.00")
        );
        assert_eq!(
            our_map.get("sched3TotalTax").map(String::as_str),
            Some("17,500.01")
        );
        assert_eq!(
            our_map
                .get("frm2550qv2024:totalPayable")
                .map(String::as_str),
            Some("47,798.74")
        );
    }
}
