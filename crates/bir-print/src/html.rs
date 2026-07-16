//! Versioned data contract for the embedded HTML form renderer.
//!
//! Rust remains authoritative for calculations and validation. The web
//! renderer receives this read-only envelope and is responsible only for
//! deterministic layout, preview, and printing.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const RENDER_CONTRACT_VERSION: &str = "1.0";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RenderEnvelopeV1 {
    pub schema_version: String,
    pub form: RenderFormIdentity,
    pub locale: String,
    pub taxpayer: RenderTaxpayer,
    pub period: RenderPeriod,
    pub fields: BTreeMap<String, RenderValue>,
    pub schedules: Vec<RenderSchedule>,
    pub validation: Vec<RenderValidationMessage>,
}

impl RenderEnvelopeV1 {
    pub(crate) fn new(
        form: RenderFormIdentity,
        taxpayer: RenderTaxpayer,
        period: RenderPeriod,
    ) -> Self {
        Self {
            schema_version: RENDER_CONTRACT_VERSION.to_string(),
            form,
            locale: "en-PH".to_string(),
            taxpayer,
            period,
            fields: BTreeMap::new(),
            schedules: Vec::new(),
            validation: Vec::new(),
        }
    }
}

/// Compatibility alias while callers migrate to the explicitly versioned name.
pub type RenderEnvelope = RenderEnvelopeV1;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RenderFormIdentity {
    pub code: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RenderTaxpayer {
    pub tin: String,
    pub name: String,
    pub rdo_code: String,
    pub registered_address: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RenderPeriod {
    pub taxable_year: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quarter: Option<u8>,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum RenderValue {
    Text(String),
    Boolean(bool),
    Integer(i64),
    Decimal(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RenderSchedule {
    pub id: String,
    pub columns: Vec<RenderColumn>,
    pub rows: Vec<RenderRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RenderColumn {
    pub key: String,
    pub label: String,
    pub alignment: RenderAlignment,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenderAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RenderRow {
    pub key: String,
    pub cells: BTreeMap<String, RenderValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RenderValidationMessage {
    pub field_path: String,
    pub code: String,
    pub message: String,
    pub severity: RenderValidationSeverity,
    pub rule_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenderValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, thiserror::Error)]
pub enum HtmlRendererError {
    #[error("failed to serialize render envelope: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub fn serialize_envelope(envelope: &RenderEnvelopeV1) -> Result<String, HtmlRendererError> {
    Ok(serde_json::to_string(envelope)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::forms::form_2551q::{
        Form2551QDraft, Item13Election, OverpaymentDisposition, Schedule1Row, TaxPeriodBasis,
    };

    #[test]
    fn render_2551q_preserves_identity_taxpayer_and_period() {
        let draft = test_2551q();
        let envelope = RenderEnvelopeV1::from(&draft);

        assert_eq!(envelope.schema_version, "1.0");
        assert_eq!(envelope.form.code, "2551Q");
        assert_eq!(envelope.form.version, "2018");
        assert_eq!(envelope.taxpayer.tin, draft.tin);
        assert_eq!(envelope.period.month, Some(12));
        assert_eq!(envelope.period.quarter, Some(1));
        assert_eq!(envelope.period.label, "Q1 year ended 12/2026");
    }

    #[test]
    fn render_2551q_exposes_owned_header_and_legal_elections() {
        let mut draft = test_2551q();
        draft.tax_period_basis = TaxPeriodBasis::Fiscal;
        draft.year_end_month = 6;
        draft.number_of_attached_sheets = 2;
        draft.tax_relief = true;
        draft.tax_relief_specification = "Special law".to_string();
        draft.item_13_election = Item13Election::Graduated;
        draft.other_tax_credit_description = "Prior payment".to_string();
        draft.creditable_tax_withheld = 10_000.0;
        draft.recompute(None);
        assert!(draft.total_amount_payable < 0.0);
        draft.overpayment_disposition = OverpaymentDisposition::Refund;

        let envelope = RenderEnvelopeV1::from(&draft);

        assert_eq!(envelope.period.month, Some(6));
        assert_eq!(
            envelope.fields["tax_period_basis"],
            RenderValue::Text("fiscal".to_string())
        );
        assert_eq!(
            envelope.fields["number_of_attached_sheets"],
            RenderValue::Integer(2)
        );
        assert_eq!(
            envelope.fields["tax_relief_specification"],
            RenderValue::Text("Special law".to_string())
        );
        assert_eq!(
            envelope.fields["item_13_election"],
            RenderValue::Text("graduated".to_string())
        );
        assert_eq!(
            envelope.fields["other_tax_credit_description"],
            RenderValue::Text("Prior payment".to_string())
        );
        assert_eq!(
            envelope.fields["overpayment_disposition"],
            RenderValue::Text("refund".to_string())
        );
    }

    #[test]
    fn render_2551q_preserves_rust_owned_item_13_applicability_outcomes() {
        let applicable = test_2551q();
        assert_eq!(applicable.item_13_is_applicable(), Some(true));
        let applicable_envelope = RenderEnvelopeV1::from(&applicable);
        assert_eq!(
            applicable_envelope.fields["item_13_election"],
            RenderValue::Text("graduated".to_string())
        );
        assert!(applicable_envelope
            .validation
            .iter()
            .all(|issue| issue.field_path != "item_13_election"));

        let mut inapplicable = applicable.clone();
        inapplicable.quarter = 2;
        inapplicable.item_13_election = Item13Election::NotApplicable;
        assert_eq!(inapplicable.item_13_is_applicable(), Some(false));
        let inapplicable_envelope = RenderEnvelopeV1::from(&inapplicable);
        assert_eq!(
            inapplicable_envelope.fields["item_13_election"],
            RenderValue::Text("not_applicable".to_string())
        );
        assert!(inapplicable_envelope
            .validation
            .iter()
            .all(|issue| issue.field_path != "item_13_election"));

        let mut missing_business_start = inapplicable.clone();
        missing_business_start.business_start_date = None;
        assert_eq!(missing_business_start.item_13_is_applicable(), None);
        let missing_business_start_envelope = RenderEnvelopeV1::from(&missing_business_start);
        assert!(missing_business_start_envelope
            .validation
            .iter()
            .any(|issue| issue.field_path == "business_start_date"));

        let mut unknown = applicable;
        unknown.taxpayer_type = None;
        let unknown_envelope = RenderEnvelopeV1::from(&unknown);
        assert!(unknown_envelope
            .validation
            .iter()
            .any(|issue| issue.field_path == "taxpayer_type"));
    }

    #[test]
    fn render_2551q_exposes_every_available_part_two_amount() {
        let mut draft = test_2551q();
        draft.creditable_tax_withheld = 10.0;
        draft.tax_paid_previous = 20.0;
        draft.other_tax_credit = 30.0;
        draft.surcharge = 40.0;
        draft.interest = 50.0;
        draft.compromise = 60.0;
        draft.recompute(None);

        let envelope = RenderEnvelopeV1::from(&draft);

        for key in [
            "total_tax_due",
            "creditable_tax_withheld",
            "tax_paid_previous",
            "other_tax_credit",
            "total_tax_credits",
            "tax_payable",
            "surcharge",
            "interest",
            "compromise",
            "total_penalties",
            "total_amount_payable",
        ] {
            assert!(envelope.fields.contains_key(key), "missing {key}");
        }
    }

    #[test]
    fn render_2551q_never_prints_item_16_on_a_non_amended_return() {
        let mut draft = test_2551q();
        draft.is_amended = false;
        draft.tax_paid_previous = 500.0;
        assert_eq!(
            RenderEnvelopeV1::from(&draft).fields["tax_paid_previous"],
            RenderValue::Decimal(0.0)
        );

        draft.is_amended = true;
        assert_eq!(
            RenderEnvelopeV1::from(&draft).fields["tax_paid_previous"],
            RenderValue::Decimal(500.0)
        );
    }

    #[test]
    fn render_2551q_preserves_all_schedule_rows_in_stable_order() {
        let mut draft = test_2551q();
        draft.schedule_1 = schedule_rows(10);

        let envelope = RenderEnvelopeV1::from(&draft);
        let rows = &envelope.schedules[0].rows;

        assert_eq!(rows.len(), 10);
        assert_eq!(rows.first().unwrap().key, "schedule-1-1-PT010");
        assert_eq!(rows.last().unwrap().key, "schedule-1-10-PT019");
    }

    #[test]
    fn render_2551q_owns_page_two_carry_forward_only_for_overflow() {
        let six_rows = RenderEnvelopeV1::from(&test_2551q());
        assert!(!six_rows.fields.contains_key("schedule_1_page_2_subtotal"));

        let mut overflow = test_2551q();
        overflow.schedule_1 = schedule_rows(10);
        overflow.recompute(None);
        let overflow = RenderEnvelopeV1::from(&overflow);

        assert_eq!(
            overflow.fields["schedule_1_page_2_subtotal"],
            RenderValue::Decimal(6_300.0)
        );
        assert_eq!(
            overflow.fields["total_tax_due"],
            RenderValue::Decimal(16_500.0)
        );

        let mut fractional = test_2551q();
        fractional.schedule_1 = schedule_rows(7);
        for row in fractional.schedule_1.iter_mut().take(6) {
            row.tax_due = 0.001;
        }
        let fractional = RenderEnvelopeV1::from(&fractional);
        assert_eq!(
            fractional.fields["schedule_1_page_2_subtotal"],
            RenderValue::Decimal(0.01)
        );
    }

    #[test]
    fn render_2551q_maps_main_validation_errors_deterministically() {
        let mut draft = test_2551q();
        draft.schedule_1.clear();

        let envelope = RenderEnvelopeV1::from(&draft);
        let issue = envelope
            .validation
            .iter()
            .find(|issue| issue.field_path == "schedule_1")
            .expect("missing Schedule 1 should be reported");

        assert_eq!(issue.code, "invalid");
        assert_eq!(issue.severity, RenderValidationSeverity::Error);
        assert_eq!(issue.rule_version, "2551q-main-v1");
    }

    #[test]
    fn serialize_envelope_is_stable_and_versioned() {
        let envelope = RenderEnvelopeV1::from(&test_2551q());

        let first = serialize_envelope(&envelope).expect("envelope should serialize");
        let second = serialize_envelope(&envelope).expect("envelope should serialize");

        assert_eq!(first, second);
        assert!(first.contains("\"schema_version\":\"1.0\""));
    }

    fn schedule_rows(row_count: usize) -> Vec<Schedule1Row> {
        (0..row_count)
            .map(|index| Schedule1Row {
                atc: format!("PT{:03}", index + 10),
                atc_description: format!("Representative percentage tax activity {}", index + 1),
                taxable_amount: (index + 1) as f64 * 10_000.0,
                tax_rate: 0.03,
                tax_due: (index + 1) as f64 * 300.0,
            })
            .collect()
    }

    fn test_2551q() -> Form2551QDraft {
        let mut draft: Form2551QDraft = serde_json::from_value(serde_json::json!({
            "id": null,
            "tin": "12345678900000",
            "taxpayer_type": "Individual",
            "business_start_date": "2010-01-01",
            "taxable_year": 2026,
            "quarter": 1,
            "tax_period_basis": "calendar",
            "year_end_month": 12,
            "eopt_tier": null,
            "is_amended": true,
            "original_return_filed_and_paid_on_time": true,
            "number_of_attached_sheets": 0,
            "tax_relief": false,
            "tax_relief_specification": "",
            "item_13_election": "graduated",
            "rdo_code": "018",
            "taxpayer_name": "Renderer Fixture Corporation",
            "registered_address": "53 Santol Extension, New Cabalan, Olongapo City",
            "zip_code": "2200",
            "contact_number": "09123456789",
            "email": "renderer@example.com",
            "schedule_1": [],
            "total_tax_due": 0.0,
            "creditable_tax_withheld": 125.0,
            "tax_paid_previous": 50.0,
            "other_tax_credit": 25.0,
            "other_tax_credit_description": "Validated prior payment",
            "total_tax_credits": 200.0,
            "tax_payable": 0.0,
            "auto_compute_penalties": false,
            "surcharge": 10.0,
            "interest": 5.0,
            "compromise": 1000.0,
            "total_penalties": 1015.0,
            "total_amount_payable": 0.0,
            "overpayment_disposition": "none",
            "status": "Draft",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "submitted_at": null,
            "confirmed_at": null,
            "submission_filename": null,
            "receipt_id": null,
            "submission_attempts": 0,
            "next_retry_at": null,
            "last_error": null,
            "carried_forward_from": null,
            "payment_receipt_path": null
        }))
        .expect("test fixture should deserialize");
        draft.schedule_1 = schedule_rows(6);
        draft.recompute(None);
        draft
    }
}
