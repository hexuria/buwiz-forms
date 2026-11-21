//! Opening advice, not new Queue requirements: use the return's own applicability
//! and validation rules to name missing profile context before the filer queues.

use bir_core::forms::FormValidator;
use bir_core::forms::agent::AgentFormKind;
use bir_core::forms::form_2551q::{AnnualIncomeTaxElection, Form2551QDraft};
use serde_json::{Value, json};

pub(super) fn of(kind: AgentFormKind, draft: &Value) -> Result<Value, String> {
    if kind != AgentFormKind::Form2551Q {
        return Ok(json!([]));
    }
    let draft: Form2551QDraft =
        serde_json::from_value(draft.clone()).map_err(|error| error.to_string())?;
    let mut missing = Vec::new();
    if (2..=4).contains(&draft.quarter) {
        if draft.business_start_date.is_none() && draft.item_13_is_applicable().is_none() {
            missing.push(json!({
                "field": "business_start_date",
                "label": "Business start date",
                "message": "Set the tax profile's business start date to determine whether Item 13 applies to this quarter."
            }));
        }
        if draft.annual_income_tax_election == Some(AnnualIncomeTaxElection::Unrecorded)
            && draft
                .validate()
                .iter()
                .any(|(field, _)| field == "annual_income_tax_election")
        {
            missing.push(json!({
                "field": "income_tax_election",
                "label": "Annual income-tax election",
                "message": format!("Record the tax profile's annual income-tax election for {} before filing this later-quarter return.", draft.taxable_year)
            }));
        }
    }
    Ok(json!(missing))
}
