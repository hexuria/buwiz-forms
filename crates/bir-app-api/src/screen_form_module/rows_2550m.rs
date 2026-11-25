//! 2550M Schedules 2, 3A, 3B, 6, 7 and 8. The official page builds their
//! rows in script (`addlistSched2()` … `addlistSched8()`, ids `<stem><row>`
//! from row 0), so the probed agent bindings carry only the `{N>=0}` row
//! families. The host writes each row into the typed draft instead, the way
//! `schedule_row_controls` reads it back.

use bir_core::forms::agent::{self, AgentFormKind};
use bir_core::forms::form_2550m::{
    Form2550MAdvancePaymentRow, Form2550MAmortizedRow, Form2550MCapitalGoodsRow, Form2550MDraft,
    Form2550MWithholdingRow,
};
use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Schedule {
    S2,
    S3A,
    S3B,
    S6,
    S7,
    S8,
}

/// The boxes the filer types, per schedule, in the official row order (an
/// amount precedes the input tax it fills). Computed cells (allowable input
/// tax, balance) are not listed: the page greys them out.
const SCHEDULES: &[(Schedule, &str, &[&str])] = &[
    (
        Schedule::S2,
        "schedule_2",
        &[
            "txtDatePurchased",
            "txtDescription",
            "txtAmt",
            "txtInputTax",
        ],
    ),
    (
        Schedule::S3A,
        "schedule_3a",
        &[
            "txtDatePurchased3A",
            "txtDescription3A",
            "txtAmt3A",
            "txtInputTax3A",
            "txtEstLife3A",
            "txtRecogLife3A",
        ],
    ),
    (
        Schedule::S3B,
        "schedule_3b",
        &[
            "txtDatePurchased3B",
            "txtDescription3B",
            "txtAmt3B",
            "txtBalInputTaxPrevious3B",
            "txtEstLife3B",
            "txtRecogLife3B",
        ],
    ),
    (
        Schedule::S6,
        "schedule_6",
        &[
            "txtPeriodCovered",
            "txtNameAgent",
            "txtIncomePayment",
            "txtTotalWithheld",
            "txtAppliedCurr",
        ],
    ),
    (
        Schedule::S7,
        "schedule_7",
        &[
            "txtPeriodCoveredSch7",
            "txtNameMillerSch7",
            "txtNameTaxPayerSch7",
            "txtORNumSch7",
            "txtAmountPaidSch7",
            "txtAppliedCurrSch7",
        ],
    ),
    (
        Schedule::S8,
        "schedule_8",
        &[
            "txtPeriodCoveredSch8",
            "txtNameAgentSch8",
            "txtIncomePaymentSch8",
            "txtTotalWithheldSch8",
            "txtAppliedCurrSch8",
        ],
    ),
];

/// A schedule row box: `(schedule, column, row)`. The remainder after the
/// stem must be digits only, so `txtAmt3A0` is never Schedule 2's `txtAmt`.
pub(super) fn cell(kind: AgentFormKind, key: &str) -> Option<(Schedule, usize, usize)> {
    if kind.form_code() != "2550M" {
        return None;
    }
    SCHEDULES.iter().find_map(|(schedule, _, stems)| {
        stems.iter().enumerate().find_map(|(col, stem)| {
            let rest = key.strip_prefix(stem)?;
            if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            Some((*schedule, col, rest.parse().ok()?))
        })
    })
}

/// Earlier rows first within a batch, and every base amount before the
/// editable tax box(es) its blur handler fills (`13A` before `13B` and
/// `txtlessStdTaxSched4`, `18E/G/I/K/N` before `18F/H/J/L/O`, and schedule
/// row amounts before their input tax).
pub(super) fn edit_order(kind: AgentFormKind, key: &str) -> Option<usize> {
    if kind.form_code() == "2550M" {
        match key {
            "frm2550m:txtTax13A" | "frm2550m:txtTax18E" | "frm2550m:txtTax18G"
            | "frm2550m:txtTax18I" | "frm2550m:txtTax18K" | "frm2550m:txtTax18N" => return Some(0),
            "frm2550m:txtTax13B"
            | "txtlessStdTaxSched4"
            | "frm2550m:txtTax18F"
            | "frm2550m:txtTax18H"
            | "frm2550m:txtTax18J"
            | "frm2550m:txtTax18L"
            | "frm2550m:txtTax18O" => return Some(1),
            _ => {}
        }
    }
    cell(kind, key).map(|(_, col, row)| 2 + row.saturating_mul(8).saturating_add(col))
}

/// The row boxes a validation message about `schedule_2[i]` (one row) or
/// `schedule_2` (the whole schedule, shown at its first row) belongs to.
pub(super) fn error_keys(kind: AgentFormKind, field: &str) -> Vec<String> {
    if kind.form_code() != "2550M" {
        return Vec::new();
    }
    let (name, row) = match field.split_once('[') {
        Some((name, rest)) => match rest.strip_suffix(']').and_then(|r| r.parse().ok()) {
            Some(row) => (name, row),
            None => return Vec::new(),
        },
        None => (field, 0usize),
    };
    SCHEDULES
        .iter()
        .find(|(_, n, _)| *n == name)
        .map(|(_, _, stems)| stems.iter().map(|s| format!("{s}{row}")).collect())
        .unwrap_or_default()
}

fn text_of(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        other => other.as_str().unwrap_or_default().to_string(),
    }
}

/// The page's `numbersonly` keypress filter: no sign, no letters.
fn amount(text: &str) -> Result<f64, String> {
    if text.trim().is_empty() {
        return Ok(0.0);
    }
    match agent::parse_amount(text) {
        Some(value) if value >= 0.0 => Ok(value),
        _ => Err(format!("`{}` is not an amount.", text.trim())),
    }
}

/// A text box with the official `maxlength`.
fn limited(text: &str, max: usize) -> Result<String, String> {
    let text = text.trim();
    if text.chars().count() > max {
        return Err(format!("This box holds at most {max} characters."));
    }
    Ok(text.to_string())
}

/// Estimated life: `maxlength='3'`, digits only; blank is the row's 0.
fn estimated_life(text: &str) -> Result<u16, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(0);
    }
    text.parse::<u16>()
        .ok()
        .filter(|n| *n <= 999)
        .ok_or_else(|| "Enter a value 1 to 999 for Estimated Life.".to_string())
}

/// Recognized life: 1 to 60 months, since allowable input tax divides by
/// it. Blank is the row's starting 1 (`Schedule3()`).
fn recognized_life(text: &str) -> Result<u16, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(1);
    }
    text.parse::<u16>()
        .ok()
        .filter(|n| (1..=60).contains(n))
        .ok_or_else(|| "Enter a value 1 to 60 for Recognized Life.".to_string())
}

/// The row being written: the next row starts one (`addlistSched…`), a
/// row past it would leave a gap the page cannot have.
fn row_mut<T: Default>(rows: &mut Vec<T>, row: usize) -> Result<&mut T, String> {
    if row == rows.len() {
        rows.push(T::default());
    }
    let next = rows.len();
    rows.get_mut(row)
        .ok_or_else(|| format!("Fill row {next} of this schedule first."))
}

/// Drop the row when every box is blank again, as ticking it and pressing
/// Delete does on the page; later rows move up.
fn prune<T: Default + PartialEq>(rows: &mut Vec<T>, row: usize) {
    if rows.get(row).is_some_and(|r| *r == T::default()) {
        rows.remove(row);
    }
}

pub(super) fn fill(
    draft: &mut Value,
    (schedule, col, row): (Schedule, usize, usize),
    value: &Value,
) -> Result<(), String> {
    let text = text_of(value);
    let mut typed: Form2550MDraft =
        serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?;
    match schedule {
        Schedule::S2 => {
            let entry: &mut Form2550MCapitalGoodsRow = row_mut(&mut typed.schedule_2, row)?;
            match col {
                0 => entry.date_purchased = limited(&text, 10)?,
                1 => entry.description = limited(&text, 50)?,
                2 => {
                    let value = amount(&text)?;
                    typed.set_schedule_2_amount(row, value);
                }
                _ => entry.input_tax = amount(&text)?,
            }
            typed.recompute();
            prune(&mut typed.schedule_2, row);
        }
        Schedule::S3A | Schedule::S3B => {
            let rows = if schedule == Schedule::S3A {
                &mut typed.schedule_3a
            } else {
                &mut typed.schedule_3b
            };
            let entry: &mut Form2550MAmortizedRow = row_mut(rows, row)?;
            match col {
                0 => entry.date_purchased = limited(&text, 10)?,
                1 => entry.description = limited(&text, 50)?,
                2 if schedule == Schedule::S3A => {
                    let value = amount(&text)?;
                    typed.set_schedule_3a_amount(row, value);
                }
                2 => entry.amount = amount(&text)?,
                3 => entry.input_tax = amount(&text)?,
                4 => entry.estimated_life = estimated_life(&text)?,
                _ => entry.recognized_life = recognized_life(&text)?,
            }
            typed.recompute();
            let rows = if schedule == Schedule::S3A {
                &mut typed.schedule_3a
            } else {
                &mut typed.schedule_3b
            };
            prune(rows, row);
        }
        Schedule::S6 | Schedule::S8 => {
            // The page enables Schedule 8 only once Item 13A has a value.
            if schedule == Schedule::S8
                && row >= typed.schedule_8.len()
                && typed.sales_to_government <= 0.0
                && !text.trim().is_empty()
            {
                return Err(
                    "Enter Item 13A (Sales to Government) to be able to load Schedule 8."
                        .to_string(),
                );
            }
            let rows = if schedule == Schedule::S6 {
                &mut typed.schedule_6
            } else {
                &mut typed.schedule_8
            };
            let entry: &mut Form2550MWithholdingRow = row_mut(rows, row)?;
            match col {
                0 => entry.period_covered = limited(&text, 10)?,
                1 => entry.withholding_agent = limited(&text, 50)?,
                2 => entry.income_payment = amount(&text)?,
                3 => entry.total_withheld = amount(&text)?,
                _ => entry.applied_current_month = amount(&text)?,
            }
            typed.recompute();
            let rows = if schedule == Schedule::S6 {
                &mut typed.schedule_6
            } else {
                &mut typed.schedule_8
            };
            prune(rows, row);
        }
        Schedule::S7 => {
            let entry: &mut Form2550MAdvancePaymentRow = row_mut(&mut typed.schedule_7, row)?;
            match col {
                0 => entry.period_covered = limited(&text, 10)?,
                1 => entry.miller = limited(&text, 50)?,
                2 => entry.taxpayer_name = limited(&text, 17)?,
                3 => entry.or_number = limited(&text, 20)?,
                4 => entry.amount_paid = amount(&text)?,
                _ => entry.applied_current_month = amount(&text)?,
            }
            typed.recompute();
            prune(&mut typed.schedule_7, row);
        }
    }
    typed.recompute();
    *draft = serde_json::to_value(typed).map_err(|e| e.to_string())?;
    Ok(())
}

/// Non-blocking Check notices for 2550M drafts where a base amount is non-zero
/// while its paired tax box is still 0.00 (or Schedule 4 standard input tax
/// still holds the static 5.00 default), such as a draft saved before the
/// blur-calculation fix. Never alters saved values and does not block queueing.
pub(super) fn notices(kind: AgentFormKind, draft: &Value) -> Vec<Value> {
    if kind.form_code() != "2550M" || !super::is_draft_return(kind, draft) {
        return Vec::new();
    }
    let Ok(form) = serde_json::from_value::<Form2550MDraft>(draft.clone()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if form.sales_to_government != 0.0 {
        let base_str = bir_core::official_xml::official_amount(form.sales_to_government);
        if form.output_tax_government == 0.0 {
            out.push(serde_json::json!({
                "field": "output_tax_government",
                "message": format!(
                    "Item 13B is 0.00, but sales to government (13A) are {base_str}. The BIR's program fills 13B with 12% of 13A. Re-enter 13A to fill it, or type 13B."
                ),
                "keys": ["frm2550m:txtTax13B"],
            }));
        }
        let expected_sched4 =
            (form.sales_to_government.abs() * 0.07 * 100.0 + 0.500_000_000_01).floor() / 100.0
                * form.sales_to_government.signum();
        if form.standard_input_tax_government == 5.0 && expected_sched4 != 5.0 {
            out.push(serde_json::json!({
                "field": "standard_input_tax_government",
                "message": format!(
                    "Schedule 4 standard input tax is still at the 5.00 default, but sales to government (13A) are {base_str}. The BIR's program fills it with 7% of 13A. Re-enter 13A to fill it, or type the Schedule 4 amount."
                ),
                "keys": ["txtlessStdTaxSched4"],
            }));
        }
    }
    for (base, tax, base_item, tax_item, base_desc, field, key) in [
        (
            form.domestic_goods,
            form.domestic_goods_input_tax,
            "18E",
            "18F",
            "domestic purchases of goods (18E)",
            "domestic_goods_input_tax",
            "frm2550m:txtTax18F",
        ),
        (
            form.imported_goods,
            form.imported_goods_input_tax,
            "18G",
            "18H",
            "importation of goods (18G)",
            "imported_goods_input_tax",
            "frm2550m:txtTax18H",
        ),
        (
            form.domestic_services,
            form.domestic_services_input_tax,
            "18I",
            "18J",
            "domestic purchase of services (18I)",
            "domestic_services_input_tax",
            "frm2550m:txtTax18J",
        ),
        (
            form.non_resident_services,
            form.non_resident_services_input_tax,
            "18K",
            "18L",
            "services rendered by non-residents (18K)",
            "non_resident_services_input_tax",
            "frm2550m:txtTax18L",
        ),
        (
            form.other_purchases,
            form.other_purchases_input_tax,
            "18N",
            "18O",
            "other purchases (18N)",
            "other_purchases_input_tax",
            "frm2550m:txtTax18O",
        ),
    ] {
        if base != 0.0 && tax == 0.0 {
            let base_str = bir_core::official_xml::official_amount(base);
            out.push(serde_json::json!({
                "field": field,
                "message": format!(
                    "Item {tax_item} is 0.00, but {base_desc} are {base_str}. The BIR's program fills {tax_item} with 12% input tax on {base_item}. Re-enter {base_item} to fill it, or type {tax_item}."
                ),
                "keys": [key],
            }));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::forms::queueable::QueueableKind;
    use serde_json::json;

    #[test]
    fn edit_order_puts_bases_before_paired_tax_boxes() {
        let kind = AgentFormKind::Queueable(QueueableKind::Form2550M);
        for (base, tax) in [
            ("frm2550m:txtTax13A", "frm2550m:txtTax13B"),
            ("frm2550m:txtTax13A", "txtlessStdTaxSched4"),
            ("frm2550m:txtTax18E", "frm2550m:txtTax18F"),
            ("frm2550m:txtTax18G", "frm2550m:txtTax18H"),
            ("frm2550m:txtTax18I", "frm2550m:txtTax18J"),
            ("frm2550m:txtTax18K", "frm2550m:txtTax18L"),
            ("frm2550m:txtTax18N", "frm2550m:txtTax18O"),
            ("txtAmt0", "txtInputTax0"),
            ("txtAmt3A0", "txtInputTax3A0"),
        ] {
            assert!(
                edit_order(kind, base) < edit_order(kind, tax),
                "{base} must sort before {tax}"
            );
        }
    }

    #[test]
    fn schedule_2_and_3a_compute_12_percent_and_preserve_override() {
        let kind = AgentFormKind::Queueable(QueueableKind::Form2550M);
        let profile: bir_core::profile::TaxpayerProfile = serde_json::from_value(json!({
            "id": null,
            "full_name": "Juana Santos",
            "tin": {"segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000"},
            "rdo_code": "008",
            "line_of_business": "Retail",
            "registered_address": "1 Sample Street, Makati City",
            "zip_code": "1003",
            "phone": "09171234567",
            "email": "juana.santos@example.com",
            "default_form_type": "2550M",
            "taxpayer_type": "Corporation",
        }))
        .unwrap();
        let mut draft =
            serde_json::to_value(Form2550MDraft::new_from_profile(&profile, 2022, 6)).unwrap();
        fill(&mut draft, cell(kind, "txtAmt0").unwrap(), &json!("120000")).unwrap();
        assert_eq!(draft["schedule_2"][0]["input_tax"], json!(14400.0));
        fill(
            &mut draft,
            cell(kind, "txtInputTax0").unwrap(),
            &json!("14000"),
        )
        .unwrap();
        assert_eq!(draft["schedule_2"][0]["input_tax"], json!(14000.0));

        fill(
            &mut draft,
            cell(kind, "txtAmt3A0").unwrap(),
            &json!("1500000.25"),
        )
        .unwrap();
        assert_eq!(draft["schedule_3a"][0]["input_tax"], json!(180000.03));
        fill(
            &mut draft,
            cell(kind, "txtInputTax3A0").unwrap(),
            &json!("179000"),
        )
        .unwrap();
        assert_eq!(draft["schedule_3a"][0]["input_tax"], json!(179000.0));
    }

    fn test_2550m_draft() -> Form2550MDraft {
        let profile: bir_core::profile::TaxpayerProfile = serde_json::from_value(json!({
            "id": null,
            "full_name": "Juana Santos",
            "tin": {"segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000"},
            "rdo_code": "008",
            "line_of_business": "Retail",
            "registered_address": "1 Sample Street, Makati City",
            "zip_code": "1003",
            "phone": "09171234567",
            "email": "juana.santos@example.com",
            "default_form_type": "2550M",
            "taxpayer_type": "Corporation",
        }))
        .unwrap();
        Form2550MDraft::new_from_profile(&profile, 2025, 6)
    }

    #[test]
    fn notices_warn_on_13a_with_zero_13b_and_default_sched4_without_blocking() {
        let kind = AgentFormKind::Queueable(QueueableKind::Form2550M);
        let mut form = test_2550m_draft();
        form.sales_to_government = 250_000.01;
        // Pre-fix draft state: 13B stays 0.00, Schedule 4 stays at 5.00 default.
        form.recompute();
        let draft = serde_json::to_value(&form).unwrap();
        let list = notices(kind, &draft);
        assert_eq!(list.len(), 2, "expected 13B and Sched 4 notices: {list:?}");
        assert_eq!(list[0]["keys"], json!(["frm2550m:txtTax13B"]));
        assert_eq!(
            list[0]["message"],
            json!(
                "Item 13B is 0.00, but sales to government (13A) are 250,000.01. The BIR's program fills 13B with 12% of 13A. Re-enter 13A to fill it, or type 13B."
            )
        );
        assert_eq!(list[1]["keys"], json!(["txtlessStdTaxSched4"]));
        assert!(
            list[1]["message"]
                .as_str()
                .unwrap()
                .contains("5.00 default")
                && list[1]["message"].as_str().unwrap().contains("250,000.01")
        );
        // Non-blocking: Form2550MDraft's own queue validation still passes.
        assert!(
            bir_core::forms::queueable::QueueableForm::validate(&form).is_empty(),
            "2550M notices must not block queue validation"
        );

        // Non-zero user-typed values clear both notices.
        form.output_tax_government = 1.0;
        form.standard_input_tax_government = 17_500.0;
        let cleared = notices(kind, &serde_json::to_value(&form).unwrap());
        assert!(cleared.is_empty(), "expected no notices: {cleared:?}");
    }

    #[test]
    fn notices_warn_on_each_item_18_pair_and_clear_on_nonzero_tax() {
        let kind = AgentFormKind::Queueable(QueueableKind::Form2550M);
        let cases: &[(&str, &str, fn(&mut Form2550MDraft, f64, f64))] = &[
            ("18F", "frm2550m:txtTax18F", |f, b, t| {
                f.domestic_goods = b;
                f.domestic_goods_input_tax = t;
            }),
            ("18H", "frm2550m:txtTax18H", |f, b, t| {
                f.imported_goods = b;
                f.imported_goods_input_tax = t;
            }),
            ("18J", "frm2550m:txtTax18J", |f, b, t| {
                f.domestic_services = b;
                f.domestic_services_input_tax = t;
            }),
            ("18L", "frm2550m:txtTax18L", |f, b, t| {
                f.non_resident_services = b;
                f.non_resident_services_input_tax = t;
            }),
            ("18O", "frm2550m:txtTax18O", |f, b, t| {
                f.other_purchases = b;
                f.other_purchases_input_tax = t;
            }),
        ];
        for &(item, key, set_pair) in cases {
            let mut form = test_2550m_draft();
            set_pair(&mut form, 20_000.0, 0.0);
            let list = notices(kind, &serde_json::to_value(&form).unwrap());
            assert_eq!(list.len(), 1, "expected 1 notice for {item}: {list:?}");
            assert_eq!(list[0]["keys"], json!([key]));
            let msg = list[0]["message"].as_str().unwrap();
            assert!(
                msg.contains(&format!("Item {item} is 0.00"))
                    && msg.contains("20,000.00")
                    && msg.contains("input tax"),
                "unexpected message for {item}: {msg}"
            );

            set_pair(&mut form, 20_000.0, 0.01);
            let cleared = notices(kind, &serde_json::to_value(&form).unwrap());
            assert!(
                cleared.is_empty(),
                "non-zero {item} must clear notice: {cleared:?}"
            );
        }
    }
}
