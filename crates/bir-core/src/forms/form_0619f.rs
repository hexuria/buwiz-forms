//! BIR Form 0619-F, January 2018 (ENCS).
//!
//! The model is limited to behavior corroborated by the locked official form
//! and the reviewed plain/encrypted payload pair. In particular, the observed
//! due day is not generalized into a filing rule, penalties remain manual, and
//! submission stays manual/external.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{FilingPeriod, FilingStatus, FormValidator, TypedBirForm};
use crate::profile::TaxpayerProfile;
use crate::validation::{validate_email, validate_ph_phone, validate_zip};

pub const FORM_CODE: &str = "0619F";
pub const FORM_REVISION: &str = "2018";
pub const FORM_TYPE_ID: &str = "0619Fv2018";
pub const ITEM_13_ATC_CODE: &str = "WMF10";
pub const ITEM_14_ATC_CODE: &str = "WMF20";
pub const TAX_TYPE_CODE: &str = "WB";
pub const QUEUE_SUBMISSION_SUPPORTED: bool = false;
pub const OFFICIAL_FORM_SHA256: &str =
    "edd7357390b1f0d95f2a38c9bb76252341c15b54b82bffd338bd540452ff15e1";
pub const REVIEWED_EDITABLE_XML_SHA256: &str =
    "f7a1f2481104b8c23b22f92aef263ae02f768227ec6961cb10e4daf0817f8a18";
pub const REVIEWED_ENCRYPTED_XML_SHA256: &str =
    "d561ce34a44a732e52047552c6d4c0b975b3c45042dc0aba4907abfda89b53fb";

/// Item 11 on the official form. The inverse XML checkbox is derived at the
/// serialization boundary instead of being stored independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WithholdingAgentCategory {
    #[default]
    Private,
    Government,
}

/// The reviewed editable save uses `txtFinalFlag=1`; its encrypted companion
/// uses `txtFinalFlag=0`. Their lifecycle meaning is not proven, so imports
/// retain the observed value rather than treating either one as authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form0619FXmlFinalFlag {
    Zero,
    #[default]
    One,
    Missing,
    Unknown(String),
}

impl Form0619FXmlFinalFlag {
    pub fn as_xml_value(&self) -> &str {
        match self {
            Self::Zero => "0",
            Self::One => "1",
            Self::Missing => "",
            Self::Unknown(value) => value,
        }
    }

    pub fn requires_review(&self) -> bool {
        matches!(self, Self::Missing | Self::Unknown(_))
    }
}

/// One of the four fixed Part III payment rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form0619FPaymentRow {
    pub drawee_bank_or_agency: String,
    pub number: String,
    /// Manual `MM/DD/YYYY` text. The evidence does not establish a
    /// payment-channel-specific date rule.
    pub date: String,
    /// `None` retains an officially blank amount cell; `Some(0.0)` is an
    /// explicitly entered zero.
    pub amount: Option<f64>,
}

impl Form0619FPaymentRow {
    pub fn is_empty(&self) -> bool {
        self.drawee_bank_or_agency.trim().is_empty()
            && self.number.trim().is_empty()
            && self.date.trim().is_empty()
            && self.amount.is_none()
    }
}

/// The official form has exactly four payment rows, not a repeatable schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form0619FPaymentDetails {
    pub cash_or_bank_debit_memo: Form0619FPaymentRow,
    pub check: Form0619FPaymentRow,
    pub tax_debit_memo: Form0619FPaymentRow,
    pub others: Form0619FPaymentRow,
    pub others_description: String,
}

/// Complete typed draft for exact identity `0619Fv2018`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Form0619FDraft {
    pub id: Option<i64>,

    // Filing period.
    pub tin: String,
    pub taxable_year: u16,
    pub month: u8,

    // Header choices. XML yes/no pairs are derived from these values.
    pub is_amended: bool,
    #[serde(default, alias = "opt_withheld_y")]
    pub any_taxes_withheld: bool,
    #[serde(
        default,
        alias = "opt_category_g",
        deserialize_with = "deserialize_withholding_agent_category"
    )]
    pub withholding_agent_category: WithholdingAgentCategory,

    // The PDF proves a due-date field. One reviewed payload pair contains day
    // 10, but that is insufficient evidence for a universal deadline rule.
    #[serde(default, alias = "txt_due_day")]
    pub due_day: Option<u8>,

    // Profile-prefilled semantic values.
    pub rdo_code: String,
    pub taxpayer_name: String,
    #[serde(default, alias = "txt_line_bus")]
    pub line_of_business: String,
    pub registered_address: String,
    #[serde(default)]
    pub registered_address_2: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,

    // Part II. User-entered and computed items are named exactly as printed.
    #[serde(alias = "txt_tax13")]
    pub item_13_interest_final_tax_withheld: f64,
    #[serde(alias = "txt_tax14")]
    pub item_14_other_final_tax_withheld: f64,
    #[serde(alias = "txt_tax15")]
    pub item_15_total: f64,
    #[serde(alias = "txt_tax16")]
    pub item_16_remitted_previously: f64,
    #[serde(alias = "txt_tax17")]
    pub item_17_net_amount_of_remittance: f64,
    #[serde(alias = "txt_tax18a")]
    pub item_18a_surcharge: f64,
    #[serde(alias = "txt_tax18b")]
    pub item_18b_interest: f64,
    #[serde(alias = "txt_tax18c")]
    pub item_18c_compromise: f64,
    #[serde(alias = "txt_tax18d")]
    pub item_18d_total_penalties: f64,
    #[serde(alias = "txt_tax19")]
    pub item_19_total_amount_of_remittance: f64,

    // Signature/tax-agent fields in the exact XML union.
    #[serde(default)]
    pub tax_agent_accreditation_number: String,
    #[serde(default)]
    pub tax_agent_date_of_issue: String,
    #[serde(default)]
    pub tax_agent_date_of_expiry: String,

    // Part III - four fixed official rows.
    #[serde(default)]
    pub payment_details: Form0619FPaymentDetails,

    // XML evidence. Unknown transport keys are retained; modeled values
    // overwrite them during export.
    #[serde(default)]
    pub xml_final_flag: Form0619FXmlFinalFlag,
    #[serde(default)]
    pub preserved_unmodeled_xml_fields: BTreeMap<String, String>,

    // Lifecycle.
    pub status: FilingStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub submitted_at: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<String>,
    #[serde(default)]
    pub submission_filename: Option<String>,
    #[serde(default)]
    pub receipt_id: Option<i64>,
    #[serde(default)]
    pub submission_attempts: u32,
    #[serde(default)]
    pub next_retry_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

fn deserialize_withholding_agent_category<'de, D>(
    deserializer: D,
) -> Result<WithholdingAgentCategory, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum CategoryValue {
        Current(WithholdingAgentCategory),
        LegacyGovernmentFlag(bool),
    }

    Ok(match CategoryValue::deserialize(deserializer)? {
        CategoryValue::Current(value) => value,
        CategoryValue::LegacyGovernmentFlag(true) => WithholdingAgentCategory::Government,
        CategoryValue::LegacyGovernmentFlag(false) => WithholdingAgentCategory::Private,
    })
}

impl Form0619FDraft {
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, month: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            month,
            is_amended: false,
            any_taxes_withheld: false,
            withholding_agent_category: if profile.is_government_withholding_entity {
                WithholdingAgentCategory::Government
            } else {
                WithholdingAgentCategory::Private
            },
            due_day: None,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            line_of_business: profile.line_of_business.clone(),
            registered_address: profile.registered_address.clone(),
            registered_address_2: String::new(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            item_13_interest_final_tax_withheld: 0.0,
            item_14_other_final_tax_withheld: 0.0,
            item_15_total: 0.0,
            item_16_remitted_previously: 0.0,
            item_17_net_amount_of_remittance: 0.0,
            item_18a_surcharge: 0.0,
            item_18b_interest: 0.0,
            item_18c_compromise: 0.0,
            item_18d_total_penalties: 0.0,
            item_19_total_amount_of_remittance: 0.0,
            tax_agent_accreditation_number: String::new(),
            tax_agent_date_of_issue: String::new(),
            tax_agent_date_of_expiry: String::new(),
            payment_details: Form0619FPaymentDetails::default(),
            // One is the observed editable/plain-save value only. It is not
            // reused as evidence for an encrypted submission payload.
            xml_final_flag: Form0619FXmlFinalFlag::One,
            preserved_unmodeled_xml_fields: BTreeMap::new(),
            status: FilingStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            submitted_at: None,
            confirmed_at: None,
            submission_filename: None,
            receipt_id: None,
            submission_attempts: 0,
            next_retry_at: None,
            last_error: None,
        }
    }

    pub const fn item_13_atc_code(&self) -> &'static str {
        ITEM_13_ATC_CODE
    }

    pub const fn item_14_atc_code(&self) -> &'static str {
        ITEM_14_ATC_CODE
    }

    pub const fn tax_type_code(&self) -> &'static str {
        TAX_TYPE_CODE
    }

    /// Month/year of the due-date field. This proves only calendar rollover,
    /// not which due day is legally applicable.
    pub fn due_month_and_year(&self) -> (u8, u16) {
        if self.month == 12 {
            (1, self.taxable_year.saturating_add(1))
        } else {
            (self.month.saturating_add(1), self.taxable_year)
        }
    }

    /// Compute only the four formulas printed on the official form.
    /// No clamping, penalty inference, date lookup, or timestamp mutation is
    /// performed here.
    pub fn recompute(&mut self) {
        self.item_15_total =
            self.item_13_interest_final_tax_withheld + self.item_14_other_final_tax_withheld;
        self.item_17_net_amount_of_remittance =
            self.item_15_total - self.item_16_remitted_previously;
        self.item_18d_total_penalties =
            self.item_18a_surcharge + self.item_18b_interest + self.item_18c_compromise;
        self.item_19_total_amount_of_remittance =
            self.item_17_net_amount_of_remittance + self.item_18d_total_penalties;
    }

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub const fn can_queue_for_submission(&self) -> bool {
        QUEUE_SUBMISSION_SUPPORTED
    }

    pub fn xml_evidence_warnings(&self) -> Vec<String> {
        let mut warnings = vec![
            "The reviewed plain 0619-F save uses txtFinalFlag=1 while its encrypted companion uses txtFinalFlag=0; the observed value is preserved and no submission meaning is inferred."
                .to_string(),
        ];
        if self.xml_final_flag.requires_review() {
            warnings.push(format!(
                "txtFinalFlag value {:?} is outside the two reviewed source values",
                self.xml_final_flag
            ));
        }
        if !self.preserved_unmodeled_xml_fields.is_empty() {
            warnings.push(format!(
                "Preserved unmodeled XML keys require review: {}",
                self.preserved_unmodeled_xml_fields
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        warnings
    }

    /// Queueing remains unavailable until a reviewed submission contract exists.
    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        let mut errors = self.validate();
        errors.push((
            "submission".to_string(),
            "0619Fv2018 submission is manual/external; queue transport is not certified"
                .to_string(),
        ));
        Err(errors)
    }

    pub fn transition_to_submitted(&mut self, filename: String) -> Result<(), String> {
        if !matches!(self.status, FilingStatus::Queued) {
            return Err("0619-F can transition to Submitted only from Queued".to_string());
        }
        let now = chrono::Utc::now().to_rfc3339();
        self.status = FilingStatus::Submitted;
        self.submitted_at = Some(now.clone());
        self.submission_filename = Some(filename);
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = now;
        Ok(())
    }

    pub fn transition_to_confirmed(
        &mut self,
        confirmed_at: String,
        receipt_id: Option<i64>,
        filename: Option<String>,
    ) -> Result<(), String> {
        if !matches!(self.status, FilingStatus::Submitted) {
            return Err("0619-F can transition to Confirmed only from Submitted".to_string());
        }
        self.status = FilingStatus::Confirmed;
        self.confirmed_at = Some(confirmed_at);
        self.receipt_id = receipt_id;
        if let Some(filename) = filename {
            self.submission_filename = Some(filename);
        }
        self.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }

    pub fn transition_to_paid(&mut self) -> Result<(), String> {
        if !matches!(self.status, FilingStatus::Confirmed) {
            return Err("0619-F can transition to Paid only from Confirmed".to_string());
        }
        self.status = FilingStatus::Paid;
        self.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }

    pub fn revert_to_draft(&mut self) -> Result<(), String> {
        if matches!(self.status, FilingStatus::Paid) {
            return Err("A paid 0619-F record cannot be reverted automatically".to_string());
        }
        self.status = FilingStatus::Draft;
        self.submitted_at = None;
        self.confirmed_at = None;
        self.receipt_id = None;
        self.submission_filename = None;
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }

    pub fn record_submission_failure(&mut self, error_msg: String) -> Result<(), String> {
        if !matches!(self.status, FilingStatus::Queued) {
            return Err("Only a queued 0619-F can record a submission failure".to_string());
        }
        self.submission_attempts = self.submission_attempts.saturating_add(1);
        self.last_error = Some(error_msg);
        // Automatic retry behavior is not part of the reviewed contract.
        self.next_retry_at = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }
}

impl FormValidator for Form0619FDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        let tin_digits: String = self
            .tin
            .chars()
            .filter(|character| character.is_ascii_digit())
            .collect();
        if !matches!(tin_digits.len(), 12..=14)
            || self
                .tin
                .chars()
                .any(|character| !character.is_ascii_digit() && character != '-')
        {
            errors.push((
                "tin".to_string(),
                "TIN must contain 12 to 14 digits, with optional dashes".to_string(),
            ));
        }
        if !(1..=12).contains(&self.month) {
            errors.push((
                "month".to_string(),
                "Month must be from 1 to 12".to_string(),
            ));
        }
        if !(1900..=9999).contains(&self.taxable_year) {
            errors.push((
                "taxable_year".to_string(),
                "Taxable year must contain four digits".to_string(),
            ));
        }
        for (field, label, value) in [
            ("rdo_code", "RDO code", self.rdo_code.as_str()),
            (
                "taxpayer_name",
                "Withholding agent name",
                self.taxpayer_name.as_str(),
            ),
            (
                "registered_address",
                "Registered address",
                self.registered_address.as_str(),
            ),
            ("zip_code", "ZIP code", self.zip_code.as_str()),
            (
                "contact_number",
                "Contact number",
                self.contact_number.as_str(),
            ),
            ("email", "Email address", self.email.as_str()),
        ] {
            if value.trim().is_empty() {
                errors.push((field.to_string(), format!("{label} is required")));
            }
        }
        if !self.rdo_code.trim().is_empty()
            && (self.rdo_code.len() != 3
                || !self
                    .rdo_code
                    .chars()
                    .all(|character| character.is_ascii_digit()))
        {
            errors.push((
                "rdo_code".to_string(),
                "RDO code must be 3 digits".to_string(),
            ));
        }
        if !self.zip_code.trim().is_empty() && !validate_zip(self.zip_code.trim()) {
            errors.push((
                "zip_code".to_string(),
                "ZIP code must be 4 digits".to_string(),
            ));
        }
        if !self.contact_number.trim().is_empty() && !validate_ph_phone(&self.contact_number) {
            errors.push((
                "contact_number".to_string(),
                "Contact number must be a valid Philippine mobile or landline number".to_string(),
            ));
        }
        if !self.email.trim().is_empty() && !validate_email(&self.email) {
            errors.push(("email".to_string(), "Email address is invalid".to_string()));
        }

        match self.due_day {
            None => errors.push((
                "due_day".to_string(),
                "Due day must be entered manually; the reviewed evidence does not prove a universal 0619-F due-day rule"
                    .to_string(),
            )),
            Some(day) => {
                let (month, year) = self.due_month_and_year();
                if chrono::NaiveDate::from_ymd_opt(
                    i32::from(year),
                    u32::from(month),
                    u32::from(day),
                )
                .is_none()
                {
                    errors.push((
                        "due_day".to_string(),
                        "Due day is not valid for the computed due month/year".to_string(),
                    ));
                }
            }
        }

        for (field, value) in [
            (
                "item_13_interest_final_tax_withheld",
                self.item_13_interest_final_tax_withheld,
            ),
            (
                "item_14_other_final_tax_withheld",
                self.item_14_other_final_tax_withheld,
            ),
            (
                "item_16_remitted_previously",
                self.item_16_remitted_previously,
            ),
            ("item_18a_surcharge", self.item_18a_surcharge),
            ("item_18b_interest", self.item_18b_interest),
            ("item_18c_compromise", self.item_18c_compromise),
        ] {
            if !value.is_finite() || value < 0.0 {
                errors.push((
                    field.to_string(),
                    "Amount must be a finite, non-negative number".to_string(),
                ));
            }
        }
        if !self.is_amended && self.item_16_remitted_previously != 0.0 {
            errors.push((
                "item_16_remitted_previously".to_string(),
                "Item 16 applies only to an amended form".to_string(),
            ));
        }
        if self.item_16_remitted_previously > self.item_15_total {
            errors.push((
                "item_16_remitted_previously".to_string(),
                "Item 16 exceeds Item 15; Item 17 would be negative and is not clamped".to_string(),
            ));
        }
        if !self.any_taxes_withheld
            && (self.item_13_interest_final_tax_withheld != 0.0
                || self.item_14_other_final_tax_withheld != 0.0)
        {
            errors.push((
                "any_taxes_withheld".to_string(),
                "Item 4 is No but Item 13 or Item 14 contains a remittance amount".to_string(),
            ));
        }

        validate_optional_date(
            "tax_agent_date_of_issue",
            &self.tax_agent_date_of_issue,
            &mut errors,
        );
        validate_optional_date(
            "tax_agent_date_of_expiry",
            &self.tax_agent_date_of_expiry,
            &mut errors,
        );

        let expected_15 =
            self.item_13_interest_final_tax_withheld + self.item_14_other_final_tax_withheld;
        let expected_17 = expected_15 - self.item_16_remitted_previously;
        let expected_18d =
            self.item_18a_surcharge + self.item_18b_interest + self.item_18c_compromise;
        let expected_19 = expected_17 + expected_18d;
        for (field, actual, expected) in [
            ("item_15_total", self.item_15_total, expected_15),
            (
                "item_17_net_amount_of_remittance",
                self.item_17_net_amount_of_remittance,
                expected_17,
            ),
            (
                "item_18d_total_penalties",
                self.item_18d_total_penalties,
                expected_18d,
            ),
            (
                "item_19_total_amount_of_remittance",
                self.item_19_total_amount_of_remittance,
                expected_19,
            ),
        ] {
            if !actual.is_finite() || actual < 0.0 || (actual - expected).abs() > 0.001 {
                errors.push((
                    field.to_string(),
                    format!("Computed amount must equal {expected:.2} and be non-negative"),
                ));
            }
        }

        validate_payment_row(
            "payment_20_cash_or_bank_debit_memo",
            &self.payment_details.cash_or_bank_debit_memo,
            false,
            &self.payment_details.others_description,
            &mut errors,
        );
        validate_payment_row(
            "payment_21_check",
            &self.payment_details.check,
            false,
            &self.payment_details.others_description,
            &mut errors,
        );
        validate_payment_row(
            "payment_22_tax_debit_memo",
            &self.payment_details.tax_debit_memo,
            false,
            &self.payment_details.others_description,
            &mut errors,
        );
        validate_payment_row(
            "payment_23_others",
            &self.payment_details.others,
            true,
            &self.payment_details.others_description,
            &mut errors,
        );

        if self.xml_final_flag.requires_review() {
            errors.push((
                "xml_final_flag".to_string(),
                "The imported txtFinalFlag value is unreviewed and cannot be exported safely"
                    .to_string(),
            ));
        }

        errors
    }
}

fn validate_payment_row(
    field: &str,
    row: &Form0619FPaymentRow,
    is_others: bool,
    others_description: &str,
    errors: &mut Vec<(String, String)>,
) {
    if let Some(amount) = row.amount
        && (!amount.is_finite() || amount < 0.0)
    {
        errors.push((
            format!("{field}.amount"),
            "Payment amount must be a finite, non-negative number".to_string(),
        ));
    }
    if !row.date.trim().is_empty()
        && chrono::NaiveDate::parse_from_str(row.date.trim(), "%m/%d/%Y").is_err()
    {
        errors.push((
            format!("{field}.date"),
            "Payment date must use MM/DD/YYYY".to_string(),
        ));
    }
    if is_others && !row.is_empty() && others_description.trim().is_empty() {
        errors.push((
            "payment_23_others_description".to_string(),
            "Item 23 payment details require an Others description".to_string(),
        ));
    }
}

fn validate_optional_date(field: &str, value: &str, errors: &mut Vec<(String, String)>) {
    if !value.trim().is_empty()
        && chrono::NaiveDate::parse_from_str(value.trim(), "%m/%d/%Y").is_err()
    {
        errors.push((field.to_string(), "Date must use MM/DD/YYYY".to_string()));
    }
}

impl TypedBirForm for Form0619FDraft {
    fn form_code(&self) -> &'static str {
        FORM_CODE
    }

    fn form_type_id(&self) -> &'static str {
        FORM_TYPE_ID
    }

    fn filing_period(&self) -> FilingPeriod {
        FilingPeriod::Monthly(self.month)
    }

    fn recompute(&mut self) {
        Form0619FDraft::recompute(self);
    }

    fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        Form0619FDraft::to_bir_field_map(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_profile() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "JUAN DELA CRUZ",
            "tin": {
                "segment1": "000",
                "segment2": "000",
                "segment3": "000",
                "branch": "00000"
            },
            "rdo_code": "018",
            "line_of_business": "SOFTWARE DEVELOPMENT",
            "registered_address": "OLONGAPO",
            "zip_code": "2200",
            "phone": "09123456789",
            "email": "codeitlikemiley@gmail.com",
            "default_form_type": "0619Fv2018",
            "taxpayer_type": "Individual"
        }))
        .expect("test profile must deserialize")
    }

    fn valid_draft() -> Form0619FDraft {
        let mut draft = Form0619FDraft::new_from_profile(&test_profile(), 2026, 4);
        draft.due_day = Some(10);
        draft.any_taxes_withheld = true;
        draft.item_13_interest_final_tax_withheld = 1_000.0;
        draft.item_18a_surcharge = 100.0;
        draft.item_18b_interest = 30.0;
        draft.item_18c_compromise = 100.0;
        draft.recompute();
        draft
    }

    #[test]
    fn exact_identity_and_fixed_codes_are_not_mutable_draft_fields() {
        let draft = valid_draft();
        assert_eq!(draft.form_type_id(), "0619Fv2018");
        assert_eq!(draft.item_13_atc_code(), "WMF10");
        assert_eq!(draft.item_14_atc_code(), "WMF20");
        assert_eq!(draft.tax_type_code(), "WB");
    }

    #[test]
    fn official_formulas_are_deterministic_and_do_not_rewrite_timestamps() {
        let mut draft = valid_draft();
        draft.updated_at = "fixed".to_string();
        draft.recompute();
        assert_eq!(draft.item_15_total, 1_000.0);
        assert_eq!(draft.item_17_net_amount_of_remittance, 1_000.0);
        assert_eq!(draft.item_18d_total_penalties, 230.0);
        assert_eq!(draft.item_19_total_amount_of_remittance, 1_230.0);
        assert_eq!(draft.updated_at, "fixed");
    }

    #[test]
    fn item_17_is_not_clamped_when_item_16_exceeds_item_15() {
        let mut draft = valid_draft();
        draft.is_amended = true;
        draft.item_16_remitted_previously = 1_500.0;
        draft.recompute();
        assert_eq!(draft.item_17_net_amount_of_remittance, -500.0);
        assert!(
            draft
                .validate()
                .iter()
                .any(|(field, _)| field == "item_16_remitted_previously")
        );
    }

    #[test]
    fn manual_penalties_are_never_replaced_by_a_clock_based_engine() {
        let mut draft = valid_draft();
        draft.item_18a_surcharge = 7.0;
        draft.item_18b_interest = 8.0;
        draft.item_18c_compromise = 9.0;
        draft.recompute();
        assert_eq!(draft.item_18a_surcharge, 7.0);
        assert_eq!(draft.item_18b_interest, 8.0);
        assert_eq!(draft.item_18c_compromise, 9.0);
        assert_eq!(draft.item_18d_total_penalties, 24.0);
    }

    #[test]
    fn december_due_period_rolls_into_the_next_year() {
        let draft = Form0619FDraft::new_from_profile(&test_profile(), 2026, 12);
        assert_eq!(draft.due_month_and_year(), (1, 2027));
    }

    #[test]
    fn due_day_is_manual_and_missing_evidence_fails_closed() {
        let draft = Form0619FDraft::new_from_profile(&test_profile(), 2026, 4);
        assert!(draft.validate().iter().any(|(field, message)| {
            field == "due_day" && message.contains("entered manually")
        }));
    }

    #[test]
    fn queue_transition_is_non_panicking_and_always_disabled() {
        let mut draft = valid_draft();
        let errors = draft
            .transition_to_queued()
            .expect_err("0619-F queueing must remain disabled");
        assert_eq!(draft.status, FilingStatus::Draft);
        assert!(errors.iter().any(|(field, _)| field == "submission"));
    }

    #[test]
    fn invalid_lifecycle_transitions_return_errors_instead_of_panicking() {
        let mut draft = valid_draft();
        assert!(draft.transition_to_submitted("x.xml".to_string()).is_err());
        assert!(
            draft
                .transition_to_confirmed("now".to_string(), None, None)
                .is_err()
        );
        assert!(draft.transition_to_paid().is_err());
        assert!(
            draft
                .record_submission_failure("failure".to_string())
                .is_err()
        );
    }

    #[test]
    fn payment_details_have_exactly_four_named_rows() {
        let mut draft = valid_draft();
        draft.payment_details.cash_or_bank_debit_memo.number = "BDM-1".to_string();
        draft.payment_details.check.number = "CHECK-2".to_string();
        draft.payment_details.tax_debit_memo.number = "TDM-3".to_string();
        draft.payment_details.others.number = "OTHER-4".to_string();
        draft.payment_details.others_description = "MANUAL PAYMENT".to_string();
        assert_eq!(
            draft.payment_details.cash_or_bank_debit_memo.number,
            "BDM-1"
        );
        assert_eq!(draft.payment_details.check.number, "CHECK-2");
        assert_eq!(draft.payment_details.tax_debit_memo.number, "TDM-3");
        assert_eq!(draft.payment_details.others.number, "OTHER-4");
    }

    #[test]
    fn json_roundtrip_preserves_typed_payment_rows_and_evidence() {
        let mut draft = valid_draft();
        draft.payment_details.others = Form0619FPaymentRow {
            drawee_bank_or_agency: "AAB".to_string(),
            number: "REF".to_string(),
            date: "05/10/2026".to_string(),
            amount: Some(1_230.0),
        };
        draft.payment_details.others_description = "OTHER CHANNEL".to_string();
        draft
            .preserved_unmodeled_xml_fields
            .insert("source:futureKey".to_string(), "PRESERVE ME".to_string());
        let json = serde_json::to_string(&draft).expect("draft should serialize");
        let reopened: Form0619FDraft =
            serde_json::from_str(&json).expect("draft should deserialize");
        assert_eq!(reopened, draft);
    }

    #[test]
    fn legacy_scaffold_json_aliases_migrate_to_semantic_fields() {
        let draft = valid_draft();
        let mut value = serde_json::to_value(&draft).expect("draft should serialize");
        let object = value.as_object_mut().expect("draft JSON is an object");
        object.insert("opt_category_g".to_string(), serde_json::json!(true));
        object.remove("withholding_agent_category");
        object.insert("opt_withheld_y".to_string(), serde_json::json!(true));
        object.remove("any_taxes_withheld");
        object.insert("txt_due_day".to_string(), serde_json::json!(10));
        object.remove("due_day");
        object.insert(
            "txt_line_bus".to_string(),
            serde_json::json!("LEGACY BUSINESS"),
        );
        object.remove("line_of_business");
        for (legacy, current) in [
            ("txt_tax13", "item_13_interest_final_tax_withheld"),
            ("txt_tax14", "item_14_other_final_tax_withheld"),
            ("txt_tax15", "item_15_total"),
            ("txt_tax16", "item_16_remitted_previously"),
            ("txt_tax17", "item_17_net_amount_of_remittance"),
            ("txt_tax18a", "item_18a_surcharge"),
            ("txt_tax18b", "item_18b_interest"),
            ("txt_tax18c", "item_18c_compromise"),
            ("txt_tax18d", "item_18d_total_penalties"),
            ("txt_tax19", "item_19_total_amount_of_remittance"),
        ] {
            let field_value = object.remove(current).expect("current field exists");
            object.insert(legacy.to_string(), field_value);
        }

        let migrated: Form0619FDraft =
            serde_json::from_value(value).expect("legacy aliases should deserialize");
        assert_eq!(migrated.due_day, Some(10));
        assert_eq!(migrated.line_of_business, "LEGACY BUSINESS");
        assert!(migrated.any_taxes_withheld);
        assert_eq!(
            migrated.withholding_agent_category,
            WithholdingAgentCategory::Government
        );
        assert_eq!(migrated.item_19_total_amount_of_remittance, 1_230.0);
    }
}
