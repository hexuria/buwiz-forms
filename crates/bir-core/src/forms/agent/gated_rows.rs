//! Gates for schedule and specification boxes whose own recompute clears a
//! typed value while the gate is shut.
//!
//! The gate test is deliberately the form's own compute: a probe is written
//! to the box's bound draft path, the draft is recomputed, and the box is
//! shut exactly when the form clears that probe. That keeps the page, the
//! assistant and a stored draft on the same rule as the official transition
//! instead of maintaining a second copy of every schedule condition. The
//! only exception is 1701's spouse employer-name box: it shares one stored
//! name with the taxpayer's box, so its probe also checks the row owner.

use serde_json::{Map, Value, json};

use crate::forms::agent::{self, AgentFormKind, Binding};
use crate::forms::form_1701::{Form1701Atc, Form1701Draft, Form1701Party};

#[derive(Clone, Copy)]
struct Gate {
    gate: &'static str,
    lock: &'static str,
    note: &'static str,
    cleared: &'static str,
}

const C1701_EMPLOYER_COMPENSATION: &str =
    "Schedule 1 applies only to compensation income (ATC II011, II013 or II016).";
const C1701_EMPLOYER_OWNER: &str =
    "Schedule 1 row 1 or 2 is marked for the taxpayer or the spouse.";
const C1701_EMPLOYER_TAXPAYER: &str =
    "The taxpayer's ATC is compensation income (ATC II011, II013 or II016).";
const C1701_EMPLOYER_SPOUSE: &str =
    "A joint return and the spouse's ATC is compensation income (ATC II011, II013 or II016).";
const C1701_EIGHT_ANY: &str = "Item 7 or Part IV Item 4 ATC is II015, II016 or II017 (8% IT rate).";
const C1701_FOREIGN: &str = "Part I Item 13 is Yes.";
const C1701_ITEMIZED_A: &str =
    "Item 7 ATC is II012, II013 or II014 and Item 21A is Itemized Deduction.";
const C1701_ITEMIZED_B: &str = "A joint return (Items 16 to 18), Part IV Item 4 is II012, II013 or II014 and Part IV Item 12A is Itemized Deduction.";
const C1701A_JOINT: &str = "A joint return: Item 16 Married, Item 17 Yes and Item 18 Joint Filing.";
const C1701A_JOINT_AMENDED: &str = "A joint return (Items 16 to 18) and Amended Return Yes.";
const C1701A_JOINT_FOREIGN: &str =
    "A joint return (Items 16 to 18) and the spouse's foreign-tax-credit answer Yes.";
const C1701A_GRADUATED: &str = "The column's ATC is II012, II013 or II014 (graduated rates).";
const C1701A_EIGHT: &str = "The column's ATC is II015, II016 or II017 (8% rate).";
const C1701A_FOREIGN: &str = "Item 13 is Yes.";
const C1701MS_COMPENSATION: &str = "Column A has compensation income at Item 11.";
const C1701MS_B1: &str = "Column A has business income at Item 11 and Graduated, Exempt or Special/Preferential Rate at Item 12.";
const C1701MS_ITEMIZED: &str = "Column A has business income at Item 11 and either Exempt or Special/Preferential Rate at Item 12, or Graduated there with Itemized Deduction at Item 17.";
const C1701MS_RELIEF: &str = "Item 12 column A is Exempt or Special/Preferential Rate.";
const C1701MS_EIGHT: &str =
    "Column A has business income at Item 11 and the 8% income tax rate at Item 12.";
const C1701MS_PREVIOUS: &str = "The return is amended or a short-period return.";
const C1706_CLASS_OTHER: &str = "Item 15 classification is Others.";
const C1707_TRANSACTION_OTHER: &str = "Item 9 transaction is Others.";
const C1707_INSTALLMENT: &str = "Item 9 transaction is Installment Sale.";
const C1702MX_AMENDED: &str = "Amended Return is Yes.";
const C1702MX_MCIT: &str = "MCIT applies to this return.";
const C1702RT_MCIT: &str = "MCIT applies to this return.";
const C1702RT_AMENDED: &str = "Amended Return is Yes.";
const C2000_OTHER: &str = "Item 11 names a creditor or debtor.";
const C2000_BALANCE: &str = "Amended Return is Yes.";
const C2552_RELIEF: &str = "Item 10 tax relief is Yes.";

const G1701_SPOUSE_NAME: Gate = Gate {
    gate: "1701_spouse_employer_name",
    lock: C1701_EMPLOYER_COMPENSATION,
    note: "Schedule 1 is for compensation income of the spouse on a joint return.",
    cleared: "The spouse employer name was cleared: Schedule 1 applies only to compensation income on a joint return.",
};
const G1701_ITEM27_DESC: Gate = Gate {
    gate: "1701_schedule3_item27_description",
    lock: C1701_EIGHT_ANY,
    note: "Schedule 3 item 27 is for the 8% rate.",
    cleared: "The Schedule 3 item 27 description was cleared: it applies only to the 8% rate.",
};
const G1701_FOREIGN: Gate = Gate {
    gate: "1701_foreign_tax_number",
    lock: C1701_FOREIGN,
    note: "Item 14 needs Part I Item 13 Yes.",
    cleared: "The foreign tax number was cleared: Item 13 is not Yes.",
};
const G1701_SCHED5_A: Gate = Gate {
    gate: "1701_schedule5_taxpayer",
    lock: C1701_ITEMIZED_A,
    note: "Schedule 5 needs graduated rates with itemized deductions.",
    cleared: "Schedule 5 was cleared: it applies only to graduated rates with itemized deductions.",
};
const G1701_SCHED5_B: Gate = Gate {
    gate: "1701_schedule5_spouse",
    lock: C1701_ITEMIZED_B,
    note: "Schedule 5 column B needs a joint return, graduated rates and itemized deductions.",
    cleared: "Schedule 5 column B was cleared: it needs a joint return, graduated rates and itemized deductions.",
};
const G1701A_JOINT: Gate = Gate {
    gate: "1701a_joint_column",
    lock: C1701A_JOINT,
    note: "Column B is for a joint return.",
    cleared: "Column B was cleared: it applies only to a joint return.",
};
const G1701A_JOINT_AMENDED: Gate = Gate {
    gate: "1701a_joint_amended",
    lock: C1701A_JOINT_AMENDED,
    note: "This credit needs a joint amended return.",
    cleared: "This credit was cleared: it needs a joint amended return.",
};
const G1701A_JOINT_FOREIGN: Gate = Gate {
    gate: "1701a_joint_foreign",
    lock: C1701A_JOINT_FOREIGN,
    note: "This credit needs a joint return and the spouse's foreign-tax-credit answer Yes.",
    cleared: "This credit was cleared: it needs a joint return and the spouse's foreign-tax-credit answer Yes.",
};
const G1701A_GRADUATED: Gate = Gate {
    gate: "1701a_graduated_schedule",
    lock: C1701A_GRADUATED,
    note: "This schedule applies only to graduated rates.",
    cleared: "This schedule was cleared: it applies only to graduated rates.",
};
const G1701A_EIGHT: Gate = Gate {
    gate: "1701a_eight_percent_schedule",
    lock: C1701A_EIGHT,
    note: "This schedule applies only to the 8% rate.",
    cleared: "This schedule was cleared: it applies only to the 8% rate.",
};
const G1701A_FOREIGN: Gate = Gate {
    gate: "1701a_foreign_tax_number",
    lock: C1701A_FOREIGN,
    note: "Item 14 needs Item 13 Yes.",
    cleared: "The foreign tax number was cleared: Item 13 is not Yes.",
};
const G1701MS_COMPENSATION: Gate = Gate {
    gate: "1701ms_schedule_a",
    lock: C1701MS_COMPENSATION,
    note: "Schedule A is for compensation income.",
    cleared: "Schedule A was cleared: it applies only to compensation income.",
};
const G1701MS_B1: Gate = Gate {
    gate: "1701ms_schedule_b1",
    lock: C1701MS_B1,
    note: "Schedule B1 needs business income at graduated, exempt or special rates.",
    cleared: "Schedule B1 was cleared: it needs business income at graduated, exempt or special rates.",
};
const G1701MS_ITEMIZED: Gate = Gate {
    gate: "1701ms_itemized",
    lock: C1701MS_ITEMIZED,
    note: "This deduction needs business income at exempt, special or graduated itemized rates.",
    cleared: "This deduction was cleared: it needs business income at exempt, special or graduated itemized rates.",
};
const G1701MS_RELIEF: Gate = Gate {
    gate: "1701ms_relief_items",
    lock: C1701MS_RELIEF,
    note: "Items 13 to 16 are for exempt or special/preferential rate income.",
    cleared: "Items 13 to 16 were cleared: they apply only to exempt or special/preferential rate income.",
};
const G1701MS_EIGHT: Gate = Gate {
    gate: "1701ms_eight_percent_schedule",
    lock: C1701MS_EIGHT,
    note: "Schedule B2 is for the 8% rate.",
    cleared: "Schedule B2 was cleared: it applies only to the 8% rate.",
};
const G1701MS_PREVIOUS: Gate = Gate {
    gate: "1701ms_previous_payment",
    lock: C1701MS_PREVIOUS,
    note: "Part V Item 6 is for an amended or short-period return.",
    cleared: "Part V Item 6 was cleared: the return is not amended or a short-period return.",
};
const G1706_CLASS_OTHER: Gate = Gate {
    gate: "1706_property_class_other",
    lock: C1706_CLASS_OTHER,
    note: "The property-class detail needs Item 15 Others.",
    cleared: "The property-class detail was cleared: Item 15 is not Others.",
};
const G1707_TRANSACTION_OTHER: Gate = Gate {
    gate: "1707_transaction_other",
    lock: C1707_TRANSACTION_OTHER,
    note: "The transaction detail needs Item 9 Others.",
    cleared: "The transaction detail was cleared: Item 9 is not Others.",
};
const G1707_INSTALLMENT: Gate = Gate {
    gate: "1707_installment_schedule",
    lock: C1707_INSTALLMENT,
    note: "Schedule 1 is for an installment sale.",
    cleared: "Schedule 1 was cleared: Item 9 is not Installment Sale.",
};
const G1702MX_REGIME: Gate = Gate {
    gate: "1702mx_schedule3_item27",
    lock: C1702MX_AMENDED,
    note: "Schedule 3 item 27 is for an amended return.",
    cleared: "Schedule 3 item 27 was cleared: this is not an amended return.",
};
const G1702MX_MCIT: Gate = Gate {
    gate: "1702mx_schedule9_mcit",
    lock: C1702MX_MCIT,
    note: "Schedule 9 applies only when MCIT applies.",
    cleared: "Schedule 9 was cleared: MCIT does not apply.",
};
const G1702RT_MCIT: Gate = Gate {
    gate: "1702rt_mcit_rows",
    lock: C1702RT_MCIT,
    note: "This MCIT entry applies only when MCIT applies.",
    cleared: "This MCIT entry was cleared: MCIT does not apply.",
};
const G1702RT_AMENDED: Gate = Gate {
    gate: "1702rt_previous_payment",
    lock: C1702RT_AMENDED,
    note: "Item 51 is for an amended return.",
    cleared: "Item 51 was cleared: this is not an amended return.",
};
const G2000_OTHER: Gate = Gate {
    gate: "2000_other_party_name",
    lock: C2000_OTHER,
    note: "The other-party details need Item 11 Creditor or Debtor.",
    cleared: "The other-party details were cleared: Item 11 names no other party.",
};
const G2000_BALANCE: Gate = Gate {
    gate: "2000_balance_carried_over",
    lock: C2000_BALANCE,
    note: "Item 15A is for an amended return.",
    cleared: "Item 15A was cleared: this is not an amended return.",
};
const G2552_RELIEF: Gate = Gate {
    gate: "2552_tax_relief_specification",
    lock: C2552_RELIEF,
    note: "Item 10A needs Item 10 Yes.",
    cleared: "The tax-relief specification was cleared: Item 10 is not Yes.",
};

fn prefixed<'a>(key: &'a str, prefix: &str) -> Option<&'a str> {
    key.strip_prefix(prefix)
}

/// The gate `key` belongs to, or `None` for an ungated box.
fn gate_of(form: &str, key: &str) -> Option<Gate> {
    match form {
        "1701" => {
            if matches!(
                key,
                "frm1701:txtPg2IShed1a_1SName"
                    | "frm1701:txtPg2IShed2a_2SName"
                    | "frm1701:txtPg2IShed1a_1TPName"
                    | "frm1701:txtPg2IShed2a_2TPName"
            ) {
                return Some(G1701_SPOUSE_NAME);
            }
            if key == "frm1701:txtPg3IShed3_27Desc" {
                return Some(G1701_ITEM27_DESC);
            }
            if key == "frm1701:txtPg1I14ForeignTaxNumber" {
                return Some(G1701_FOREIGN);
            }
            let rest = prefixed(key, "frm1701:txtPg3IShed5_")?;
            if rest.starts_with('1') || rest.starts_with('2') {
                return Some(G1701_SCHED5_A);
            }
            if rest.starts_with('4') || rest.starts_with('5') {
                return Some(G1701_SCHED5_B);
            }
            None
        }
        "1701A" => {
            let rest = prefixed(key, "frm1701A:txt")?;
            if rest == "ForeignTaxNumber" {
                return Some(G1701A_FOREIGN);
            }
            if rest == "23B" {
                return Some(G1701A_JOINT);
            }
            if rest == "61B" {
                return Some(G1701A_JOINT_AMENDED);
            }
            if rest == "62B" {
                return Some(G1701A_JOINT_FOREIGN);
            }
            if matches!(rest, "57B" | "58B" | "59B" | "60B" | "63B") {
                return Some(G1701A_JOINT);
            }
            if matches!(
                rest,
                "36A"
                    | "36B"
                    | "37A"
                    | "37B"
                    | "41A"
                    | "41B"
                    | "41Desc"
                    | "42B"
                    | "42Desc"
                    | "43A"
                    | "43B"
            ) {
                return Some(G1701A_GRADUATED);
            }
            if matches!(
                rest,
                "47A"
                    | "47B"
                    | "48A"
                    | "48B"
                    | "50A"
                    | "50B"
                    | "50Desc"
                    | "51B"
                    | "51Desc"
                    | "54A"
                    | "54B"
            ) {
                return Some(G1701A_EIGHT);
            }
            None
        }
        "1701MS" => {
            let rest = prefixed(key, "frm1701MS:txt")?;
            match rest {
                "Grossa" => Some(G1701MS_COMPENSATION),
                "SalesRevenueNo5a" | "SalesReturnsNo6a" => Some(G1701MS_B1),
                "CostofSalesNo8a"
                | "AllowableItemizedDeductionsNo10a"
                | "NetOperatingLossNo10a" => Some(G1701MS_ITEMIZED),
                "TaxpayerNo13a" | "TaxpayerNo14a" | "TaxpayerNo15a" | "TaxpayerNo16a"
                | "TaxpayerNo16a1" => Some(G1701MS_RELIEF),
                "SalesRevenuesFeesANo19a" => Some(G1701MS_EIGHT),
                "TaxPaidPreviouslyFiledNo6a" => Some(G1701MS_PREVIOUS),
                _ => None,
            }
        }
        "1706" if key == "frm1706:j_id391:_7" => Some(G1706_CLASS_OTHER),
        "1707" => {
            if key == "frm1707:txtPg1I9TransDescSpec" {
                return Some(G1707_TRANSACTION_OTHER);
            }
            let rest = prefixed(key, "frm1707:txtPg2P4Sch1I")?;
            if [
                "1SellPrice",
                "2CostExpense",
                "3AssumMortgage",
                "4NumInstallments",
                "5InstallmentAmt",
                "8DateCollection",
                "9TotCollection",
            ]
            .contains(&rest)
            {
                return Some(G1707_INSTALLMENT);
            }
            None
        }
        "1702MX" => {
            if key.starts_with("frm1702MX:txtPg2Sc3It27") {
                return Some(G1702MX_REGIME);
            }
            if key.starts_with("frm1702MX:txtPg7Sc9I") {
                return Some(G1702MX_MCIT);
            }
            None
        }
        "1702RT" => {
            if key == "frm1702RT:txtPg2Pt4I45IncomeTaxPaymentUnderMCIT"
                || (key.starts_with("frm1702RT:txtPg4Sc4I") && !key.ends_with("C4"))
            {
                return Some(G1702RT_MCIT);
            }
            if key == "frm1702RT:txtPg2Pt4I51TaxPaidInReturn" {
                return Some(G1702RT_AMENDED);
            }
            None
        }
        "2000" => match key {
            "frm2000:txtOtherName" | "frm2000:txtOtherTIN" => Some(G2000_OTHER),
            "frm2000:txtTax15A" => Some(G2000_BALANCE),
            _ => None,
        },
        "2552" if key == "frm2552:txtPg1Pt1I10TaxReliefSpec" => Some(G2552_RELIEF),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum ProbeKind {
    Text,
    Amount,
    Whole,
}

fn binding_path(binding: &Binding) -> Option<(&str, ProbeKind)> {
    match binding {
        Binding::Text { path, .. } => Some((path, ProbeKind::Text)),
        Binding::Amount { path, .. } => Some((path, ProbeKind::Amount)),
        Binding::Whole { path, .. } => Some((path, ProbeKind::Whole)),
        Binding::Choice { .. } => None,
    }
}

fn set_path(root: &mut Value, path: &str, probe: Value) -> bool {
    let parts: Vec<&str> = path.split('/').skip(1).collect();
    let Some((last, parents)) = parts.split_last() else {
        return false;
    };
    let mut current = root;
    for part in parents {
        let numeric = part.parse::<usize>().ok();
        current = match (numeric, current) {
            (Some(index), Value::Array(rows)) => {
                if rows.len() <= index {
                    rows.resize(index + 1, Value::Null);
                }
                &mut rows[index]
            }
            (_, Value::Object(map)) => map
                .entry((*part).to_string())
                .or_insert_with(|| Value::Object(Map::new())),
            _ => return false,
        };
    }
    match (last.parse::<usize>().ok(), current) {
        (Some(index), Value::Array(rows)) => {
            if rows.len() <= index {
                rows.resize(index + 1, Value::Null);
            }
            rows[index] = probe;
        }
        (_, Value::Object(map)) => {
            map.insert((*last).to_string(), probe);
        }
        _ => return false,
    }
    true
}

fn nothing(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => {
            let text = text.trim();
            text.is_empty() || text.parse::<f64>().is_ok_and(|n| n.abs() < 0.005)
        }
        Value::Number(n) => n.as_f64().unwrap_or(0.0).abs() < 0.005,
        Value::Bool(on) => !on,
        other => other.as_str().is_none_or(|text| text.trim().is_empty()),
    }
}

fn empty_for_kind(value: &Value, probe_kind: ProbeKind) -> bool {
    match probe_kind {
        ProbeKind::Text => match value {
            Value::Null => true,
            Value::String(text) => text.trim().is_empty(),
            _ => false,
        },
        ProbeKind::Amount | ProbeKind::Whole => nothing(value),
    }
}

/// 1701's spouse employer-name box is open only when its row is marked for
/// a spouse whose ATC is compensation income on a joint return.
fn employer_name_lock_reason(draft: &Value, key: &str) -> Option<&'static str> {
    let Ok(form) = serde_json::from_value::<Form1701Draft>(draft.clone()) else {
        return Some(C1701_EMPLOYER_COMPENSATION);
    };
    let index = if key.contains("Shed1a") { 0 } else { 1 };
    let compensation = |atc| {
        matches!(
            atc,
            Some(Form1701Atc::Ii011 | Form1701Atc::Ii013 | Form1701Atc::Ii016)
        )
    };
    let Some(owner) = form.employers.get(index).and_then(|row| row.owner) else {
        return Some(C1701_EMPLOYER_OWNER);
    };
    match owner {
        Form1701Party::Taxpayer => (!compensation(form.atc)).then_some(C1701_EMPLOYER_TAXPAYER),
        Form1701Party::Spouse => (!(form.spouse_column_active() && compensation(form.spouse.atc)))
            .then_some(C1701_EMPLOYER_SPOUSE),
    }
}

/// Whether the form's own recompute clears a valid probe at `key`'s path.
fn probe_closed(kind: AgentFormKind, draft: &Value, key: &str) -> bool {
    let Some(bound) = kind.definition().bindings.get(key) else {
        return false;
    };
    let Some((path, probe_kind)) = binding_path(&bound.binding) else {
        return false;
    };
    let mut copy = draft.clone();
    let probe = match probe_kind {
        ProbeKind::Text => json!("PROBE"),
        ProbeKind::Amount => json!(42.0),
        ProbeKind::Whole => json!(42),
    };
    if !set_path(&mut copy, path, probe) {
        return false;
    }
    if agent::compute(kind, &mut copy).is_err() {
        return false;
    }
    copy.pointer(path)
        .is_none_or(|value| empty_for_kind(value, probe_kind))
}

/// The reviewed lock reason for a shut gated row box.
pub fn lock_reason(kind: AgentFormKind, draft: &Value, key: &str) -> Option<&'static str> {
    let gate = gate_of(kind.form_code(), key)?;
    if gate.gate == G1701_SPOUSE_NAME.gate {
        return employer_name_lock_reason(draft, key);
    }
    let open = !probe_closed(kind, draft, key);
    (!open).then_some(gate.lock)
}

/// Every gated row key of `form`, for the page's locks.
pub fn keys(kind: AgentFormKind) -> Vec<&'static str> {
    kind.definition()
        .bindings
        .keys()
        .filter_map(|key| gate_of(kind.form_code(), key).map(|_| key.as_str()))
        .collect()
}

/// Add this layer's locks to `locks`.
pub fn relock(
    kind: AgentFormKind,
    draft: &Value,
    locks: &mut Map<String, Value>,
    only: Option<&str>,
) {
    for key in keys(kind) {
        if only.is_some_and(|wanted| wanted != key) {
            continue;
        }
        // Open is silence here, never removal: another layer may own a lock
        // on the same key (a computed total, for example). This layer only
        // adds the gate's reason while the gate is shut.
        if let Some(reason) = lock_reason(kind, draft, key) {
            locks.insert(key.to_string(), json!(reason));
        }
    }
}

/// This layer's locks as a fill leaves them (`null` opens the box).
pub fn live_locks(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
    let mut locks = Map::new();
    for key in keys(kind) {
        locks.insert(
            key.to_string(),
            lock_reason(kind, draft, key).map_or(Value::Null, |reason| json!(reason)),
        );
    }
    locks
}

/// The longer sentence drawn under a locked box, looked up by the box
/// itself. Reasons are shared across forms, so the reason string is never
/// enough to choose the note.
pub fn lock_note(form: &str, key: &str, reason: &str) -> Option<&'static str> {
    let gate = gate_of(form, key)?;
    if gate.gate == G1701_SPOUSE_NAME.gate {
        if reason == C1701_EMPLOYER_OWNER {
            return Some("Schedule 1 needs the row marked for the taxpayer or the spouse.");
        }
        if key.contains("TPName") {
            return Some("Schedule 1 is for compensation income of the taxpayer.");
        }
    }
    Some(gate.note)
}

/// One clearing a save or fill just made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clearance {
    pub gate: String,
    pub text: String,
    pub keys: Vec<&'static str>,
}

type RowLine = (&'static str, &'static str);
type ClearanceUnit = (String, Gate, Vec<&'static str>, Option<RowLine>);

/// The stable printed-line identity of a row box, never its transient vector
/// index. These schedules have fixed printed lines; when a row moves in a
/// compacting table the identity belongs to the row content, not to the key
/// position it moved into.
fn row_line(form: &str, key: &str) -> Option<(&'static str, &'static str)> {
    match form {
        "1701" => {
            if key.contains("Shed1a_1") {
                return Some(("employer-line-1", "1"));
            }
            if key.contains("Shed2a_2") {
                return Some(("employer-line-2", "2"));
            }
            let rest = key.strip_prefix("frm1701:txtPg3IShed5_")?;
            match rest.chars().next()? {
                '1' => Some(("schedule5-line-1", "1")),
                '2' => Some(("schedule5-line-2", "2")),
                '4' => Some(("schedule5-line-4", "4")),
                '5' => Some(("schedule5-line-5", "5")),
                _ => None,
            }
        }
        "1702MX" => {
            let rest = key.strip_prefix("frm1702MX:txtPg7Sc9I")?;
            match rest.chars().next()? {
                '1' => Some(("mcit-line-1", "1")),
                '2' => Some(("mcit-line-2", "2")),
                '3' => Some(("mcit-line-3", "3")),
                _ => None,
            }
        }
        "1702RT" => {
            let rest = key.strip_prefix("frm1702RT:txtPg4Sc4I")?;
            match rest.chars().next()? {
                '1' => Some(("mcit-line-1", "1")),
                '2' => Some(("mcit-line-2", "2")),
                '3' => Some(("mcit-line-3", "3")),
                _ => None,
            }
        }
        _ => None,
    }
}

fn row_cleared_text(gate: Gate, line: &str) -> String {
    match gate.gate {
        "1701_spouse_employer_name" => format!(
            "Employer row {line} was cleared: Schedule 1 applies only to compensation income on a joint return."
        ),
        "1701_schedule5_taxpayer" | "1701_schedule5_spouse" => format!(
            "Schedule 5 row {line} was cleared: it applies only to graduated rates with itemized deductions."
        ),
        "1702mx_schedule9_mcit" => {
            format!("Schedule 9 row {line} was cleared: MCIT does not apply.")
        }
        "1702rt_mcit_rows" => {
            format!("Schedule 4 row {line} was cleared: MCIT does not apply.")
        }
        _ => format!("{} Row {line}.", gate.cleared),
    }
}

/// Registered clearings whose target paths went from held content to blank
/// or zero while their gate is shut after the write. A schedule row is a
/// clearance unit of its own, keyed by its stable printed-line id; scalar
/// boxes of one gate stay one unit.
pub fn clearances(kind: AgentFormKind, before: &Value, after: &Value) -> Vec<Clearance> {
    let mut units: Vec<ClearanceUnit> = Vec::new();
    for key in kind.definition().bindings.keys() {
        let Some(gate) = gate_of(kind.form_code(), key) else {
            continue;
        };
        let line = row_line(kind.form_code(), key);
        let unit_gate = line.map_or_else(
            || gate.gate.to_string(),
            |(row_id, _)| format!("{}:{row_id}", gate.gate),
        );
        if let Some((_, _, keys, _)) = units.iter_mut().find(|(name, _, _, _)| *name == unit_gate) {
            keys.push(key.as_str());
        } else {
            units.push((unit_gate, gate, vec![key.as_str()], line));
        }
    }

    let mut cleared = Vec::new();
    for (unit_gate, gate, keys, line) in units {
        let cleared_keys: Vec<&'static str> = keys
            .iter()
            .copied()
            .filter(|key| {
                let Some(bound) = kind.definition().bindings.get(*key) else {
                    return false;
                };
                let Some((path, probe_kind)) = binding_path(&bound.binding) else {
                    return false;
                };
                let held = before
                    .pointer(path)
                    .is_some_and(|value| !empty_for_kind(value, probe_kind));
                let gone = after
                    .pointer(path)
                    .is_none_or(|value| empty_for_kind(value, probe_kind));
                held && gone && lock_reason(kind, after, key).is_some()
            })
            .collect();
        if !cleared_keys.is_empty() {
            let text = line.map_or_else(
                || gate.cleared.to_string(),
                |(_, printed_line)| row_cleared_text(gate, printed_line),
            );
            cleared.push(Clearance {
                gate: unit_gate,
                text,
                keys: cleared_keys,
            });
        }
    }
    cleared
}
