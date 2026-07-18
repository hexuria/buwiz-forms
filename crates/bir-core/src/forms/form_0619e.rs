//! BIR Form 0619-E, January 2018 (ENCS).
//!
//! This model is intentionally limited to behavior corroborated by the locked
//! official form, the reviewed plain/encrypted 0619-E payload pair, and the
//! hash-locked eBIRForms 7.9.5.0 package sources. Rust semantically replays but
//! does not byte-for-byte reproduce the reviewed ciphertext, while the package
//! obtains current transport configuration at runtime and delegates
//! encryption/upload to omitted executables. Form 0619-E therefore remains
//! manual/external for submission.

use super::{FilingPeriod, FilingStatus, FormValidator, TypedBirForm};
use crate::profile::TaxpayerProfile;
use crate::validation::{validate_email, validate_ph_phone, validate_zip};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FORM_CODE: &str = "0619E";
pub const FORM_REVISION: &str = "2018";
pub const FORM_TYPE_ID: &str = "0619Ev2018";
pub const ATC_CODE: &str = "WME10";
pub const TAX_TYPE_CODE: &str = "WE";
pub const QUEUE_SUBMISSION_SUPPORTED: bool = false;

/// Hash-locked source evidence for the exact January 2018 revision.
pub const OFFICIAL_FORM_SHA256: &str =
    "0418160d63d4e6f68c34f2bad553273a5d148c3686d8562d338d35fcdd0c5215";
pub const REVIEWED_EDITABLE_XML_SHA256: &str =
    "a6f21e372a1ce6d707ede13f2447290683ab302d859c3b684a06c55788cbfade";
pub const REVIEWED_ENCRYPTED_XML_SHA256: &str =
    "1c49950df1197906bb73ddbb5d0f5f5e1c3f488f376e05b6d53febc1b32016ab";
pub const REVIEWED_DECRYPTED_XML_SHA256: &str =
    "6a1911a359efedae7e35fa21c2def9af62c7cc1194e768d4bca5f3193c33fef4";
pub const EXACT_REVIEWED_PLAIN_XML_FIELD_COUNT: usize = 58;
pub const EXACT_REVIEWED_ENCRYPTED_XML_FIELD_COUNT: usize = 59;
pub const REVIEWED_ENCRYPTED_XML_EXTRA_FIELD: &str = "frm0619E:txtAddress2";
pub const REVIEWED_LEXICAL_ENCODING_FIELD: &str = "frm0619E:txtLineBus";
pub const CURRENT_RUST_REENCRYPTED_XML_SHA256: &str =
    "46903e58ce8b09500dc87fc63823209b8ab119990d6aee0d8073c5968a32f3a6";

/// Hash-locked native package evidence used only to decide submission safety.
pub const OFFICIAL_PACKAGE_SHA256: &str =
    "3d087545564531de1fbe8fb28f086ce6398e18608c54a0ea33353042665917eb";
pub const OFFICIAL_PACKAGE_VERSION: &str = "7.9.5.0";
pub const OFFICIAL_PACKAGE_MANIFEST_RESOURCE_ID: u32 = 129;
pub const OFFICIAL_PACKAGE_MANIFEST_FILE_OFFSET: usize = 369_216;
pub const OFFICIAL_PACKAGE_MANIFEST_SIZE: usize = 26_828;
pub const OFFICIAL_PACKAGE_MANIFEST_SHA256: &str =
    "c8811837405fd76d8924a1c04a6f283a9ed448e3792753da21aaf6ceea191249";
pub const OFFICIAL_HTA_MANIFEST_INDEX: u32 = 12;
pub const OFFICIAL_HTA_RESOURCE_ID: u32 = 141;
pub const OFFICIAL_HTA_RESOURCE_FILE_OFFSET: usize = 1_055_280;
pub const OFFICIAL_HTA_RESOURCE_DECODED_SIZE: usize = 198_185;
pub const OFFICIAL_HTA_RESOURCE_DECODED_SHA256: &str =
    "a0ef4d8958b28e63c511e7bc961e67e8ec254a6cb5f4e9f93f6d92fc9fe56f47";
pub const OFFICIAL_EBIRTOOLS_RESOURCE_ID: u32 = 553;
pub const OFFICIAL_EBIRTOOLS_RESOURCE_FILE_OFFSET: usize = 54_862_324;
pub const OFFICIAL_EBIRTOOLS_RESOURCE_DECODED_SIZE: usize = 6_451;
pub const OFFICIAL_EBIRTOOLS_RESOURCE_DECODED_SHA256: &str =
    "aaf5dbe9593ca81f808540e537353f297f9bd8638e488ea5161673e3985a91bc";
pub const OFFICIAL_ENVIRONMENT_RESOURCE_ID: u32 = 554;
pub const OFFICIAL_ENVIRONMENT_RESOURCE_FILE_OFFSET: usize = 54_868_776;
pub const OFFICIAL_ENVIRONMENT_RESOURCE_DECODED_SIZE: usize = 11_183;
pub const OFFICIAL_ENVIRONMENT_RESOURCE_DECODED_SHA256: &str =
    "01de5f90ad3c5a65af5c1ccdb61a8968d3c61e5d542eb160b6e0eb3432a3be4e";
pub const OFFICIAL_STRING_UTIL_RESOURCE_ID: u32 = 566;
pub const OFFICIAL_STRING_UTIL_RESOURCE_FILE_OFFSET: usize = 56_037_252;
pub const OFFICIAL_STRING_UTIL_RESOURCE_DECODED_SIZE: usize = 55_573;
pub const OFFICIAL_STRING_UTIL_RESOURCE_DECODED_SHA256: &str =
    "8d3f3527e044a5325b1f9019d234717d60c5bb1f72692ea302eb4f9e9cb43d6f";

/// Item 12 on the official form.  One value is authoritative; the inverse XML
/// checkbox is derived at the serialization boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WithholdingAgentCategory {
    #[default]
    Private,
    Government,
}

/// The reviewed plain save has `txtFinalFlag=1`, while its encrypted companion
/// has `txtFinalFlag=0`.  Their lifecycle meaning is not proven.  Preserve the
/// observed value instead of pretending one of them is universally correct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form0619EXmlFinalFlag {
    Zero,
    #[default]
    One,
    Missing,
    Unknown(String),
}

impl Form0619EXmlFinalFlag {
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
pub struct Form0619EPaymentRow {
    pub drawee_bank_or_agency: String,
    pub number: String,
    /// Manual `MM/DD/YYYY` text.  The reviewed evidence does not prove a
    /// payment-channel-specific date rule.
    pub date: String,
    /// `None` preserves an officially blank amount cell; `Some(0.0)` is an
    /// explicitly entered zero.
    pub amount: Option<f64>,
}

impl Form0619EPaymentRow {
    pub fn is_empty(&self) -> bool {
        self.drawee_bank_or_agency.trim().is_empty()
            && self.number.trim().is_empty()
            && self.date.trim().is_empty()
            && self.amount.is_none()
    }
}

/// The official form has exactly four payment rows, not a repeatable schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form0619EPaymentDetails {
    pub cash_or_bank_debit_memo: Form0619EPaymentRow,
    pub check: Form0619EPaymentRow,
    pub tax_debit_memo: Form0619EPaymentRow,
    pub others: Form0619EPaymentRow,
    pub others_description: String,
}

/// Complete typed draft for exact identity `0619Ev2018`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Form0619EDraft {
    pub id: Option<i64>,

    // Filing period.
    pub tin: String,
    pub taxable_year: u16,
    pub month: u8,

    // Header choices.  XML yes/no pairs are derived from these values.
    pub is_amended: bool,
    #[serde(default, alias = "opt_withheld_y")]
    pub any_taxes_withheld: bool,
    #[serde(
        default,
        alias = "opt_category_g",
        deserialize_with = "deserialize_withholding_agent_category"
    )]
    pub withholding_agent_category: WithholdingAgentCategory,

    // The official PDF proves only the due-date field.  The reviewed payloads
    // contain day 10, but one payload cannot establish a universal rule.
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

    // Part II — Tax Remittance.  Item names match the official form.
    #[serde(alias = "txt_tax14")]
    pub item_14_amount_of_remittance: f64,
    #[serde(alias = "txt_tax15")]
    pub item_15_amount_remitted_previously: f64,
    #[serde(alias = "txt_tax16")]
    pub item_16_net_amount_of_remittance: f64,
    #[serde(alias = "txt_tax17a")]
    pub item_17a_surcharge: f64,
    #[serde(alias = "txt_tax17b")]
    pub item_17b_interest: f64,
    #[serde(alias = "txt_tax17c")]
    pub item_17c_compromise: f64,
    #[serde(alias = "txt_tax17d")]
    pub item_17d_total_penalties: f64,
    #[serde(alias = "txt_tax18")]
    pub item_18_total_amount_of_remittance: f64,

    // Signature/tax-agent fields present in the exact XML union.
    #[serde(default)]
    pub tax_agent_accreditation_number: String,
    #[serde(default)]
    pub tax_agent_date_of_issue: String,
    #[serde(default)]
    pub tax_agent_date_of_expiry: String,

    // Part III — four fixed official rows.
    #[serde(default)]
    pub payment_details: Form0619EPaymentDetails,

    // XML evidence.  Unmodeled/transport keys are retained and surfaced to the
    // caller; authoritative fields overwrite them during export.
    #[serde(default)]
    pub xml_final_flag: Form0619EXmlFinalFlag,
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

impl Form0619EDraft {
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
            // Deliberately manual/fail-closed: the sample's day 10 is not a
            // sufficient legal deadline rule.
            due_day: None,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            line_of_business: profile.line_of_business.clone(),
            registered_address: profile.registered_address.clone(),
            registered_address_2: String::new(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            item_14_amount_of_remittance: 0.0,
            item_15_amount_remitted_previously: 0.0,
            item_16_net_amount_of_remittance: 0.0,
            item_17a_surcharge: 0.0,
            item_17b_interest: 0.0,
            item_17c_compromise: 0.0,
            item_17d_total_penalties: 0.0,
            item_18_total_amount_of_remittance: 0.0,
            tax_agent_accreditation_number: String::new(),
            tax_agent_date_of_issue: String::new(),
            tax_agent_date_of_expiry: String::new(),
            payment_details: Form0619EPaymentDetails::default(),
            // Value one is the reviewed editable/plain-save value.  We never
            // reuse it as evidence for an encrypted submission payload.
            xml_final_flag: Form0619EXmlFinalFlag::One,
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

    pub const fn atc_code(&self) -> &'static str {
        ATC_CODE
    }

    pub const fn tax_type_code(&self) -> &'static str {
        TAX_TYPE_CODE
    }

    /// Month/year of the due-date field.  This rollover is arithmetic evidence,
    /// not a claim about the legally correct due day.
    pub fn due_month_and_year(&self) -> (u8, u16) {
        if self.month == 12 {
            (1, self.taxable_year.saturating_add(1))
        } else {
            (self.month.saturating_add(1), self.taxable_year)
        }
    }

    /// Only the three formulas printed by the official form are computed.
    /// No clamping, penalty calculation, date lookup, or timestamp mutation is
    /// performed here.
    pub fn recompute(&mut self) {
        self.item_16_net_amount_of_remittance =
            self.item_14_amount_of_remittance - self.item_15_amount_remitted_previously;
        self.item_17d_total_penalties =
            self.item_17a_surcharge + self.item_17b_interest + self.item_17c_compromise;
        self.item_18_total_amount_of_remittance =
            self.item_16_net_amount_of_remittance + self.item_17d_total_penalties;
    }

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub const fn can_queue_for_submission(&self) -> bool {
        QUEUE_SUBMISSION_SUPPORTED
    }

    pub fn xml_evidence_warnings(&self) -> Vec<String> {
        let mut warnings = vec![
            "The reviewed plain 0619-E save uses txtFinalFlag=1 while its encrypted companion uses txtFinalFlag=0; the observed value is preserved and no submission meaning is inferred."
                .to_string(),
            "Rust decrypts and semantically replays the reviewed encrypted companion, but the current Rust compression writer does not reproduce the locked ciphertext byte-for-byte; exact outbound generation is not certified."
                .to_string(),
            "Queue submission remains disabled: eBIRForms 7.9.5.0 fetches endpoint, mode, port, username, and password from tinDispatcher.php at runtime, then invokes Encrypt.exe and cFTPSend.exe, which are absent from the reviewed package manifest."
                .to_string(),
            "The native helper exit code reports upload completion only and the HTA says submission remains subject to BIR validation; no reviewed 0619-E confirmation response or durable queue-claim persistence contract exists."
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

    /// Queueing is deliberately unavailable until the dynamic endpoint,
    /// helper binaries, confirmation semantics, and immutable persistence
    /// contract are all reviewed.
    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        let mut errors = self.validate();
        errors.push((
            "submission".to_string(),
            "0619Ev2018 queueing is disabled: exact ciphertext generation, the dynamic tinDispatcher endpoint/credentials, omitted Encrypt.exe and cFTPSend.exe helpers, BIR validation response, and durable claim persistence are not certified"
                .to_string(),
        ));
        Err(errors)
    }

    pub fn transition_to_submitted(&mut self, _filename: String) -> Result<(), String> {
        Err(
            "0619Ev2018 cannot transition to Submitted because queue/submission transport is not certified"
                .to_string(),
        )
    }

    pub fn transition_to_confirmed(
        &mut self,
        _confirmed_at: String,
        _receipt_id: Option<i64>,
        _filename: Option<String>,
    ) -> Result<(), String> {
        Err(
            "0619Ev2018 cannot transition to Confirmed because its submission-response contract is not certified"
                .to_string(),
        )
    }

    pub fn transition_to_paid(&mut self) -> Result<(), String> {
        Err(
            "0619Ev2018 cannot transition to Paid through an uncertified submission lifecycle"
                .to_string(),
        )
    }

    pub fn revert_to_draft(&mut self) -> Result<(), String> {
        if matches!(self.status, FilingStatus::Paid) {
            return Err("A paid 0619-E record cannot be reverted automatically".to_string());
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

    pub fn record_submission_failure(&mut self, _error_msg: String) -> Result<(), String> {
        Err("0619Ev2018 has no certified queue attempt or retry persistence contract".to_string())
    }
}

impl FormValidator for Form0619EDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        if self.printable_tin_segments().is_none() {
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
                "Due day must be entered manually; the reviewed evidence does not prove a universal 0619-E due-day rule"
                    .to_string(),
            )),
            Some(day) => {
                let (month, year) = self.due_month_and_year();
                if chrono::NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day))
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
                "item_14_amount_of_remittance",
                self.item_14_amount_of_remittance,
            ),
            (
                "item_15_amount_remitted_previously",
                self.item_15_amount_remitted_previously,
            ),
            ("item_17a_surcharge", self.item_17a_surcharge),
            ("item_17b_interest", self.item_17b_interest),
            ("item_17c_compromise", self.item_17c_compromise),
        ] {
            if !value.is_finite() || value < 0.0 {
                errors.push((
                    field.to_string(),
                    "Amount must be a finite, non-negative number".to_string(),
                ));
            }
        }
        if !self.is_amended && self.item_15_amount_remitted_previously != 0.0 {
            errors.push((
                "item_15_amount_remitted_previously".to_string(),
                "Item 15 applies only to an amended form".to_string(),
            ));
        }
        if self.item_15_amount_remitted_previously > self.item_14_amount_of_remittance {
            errors.push((
                "item_15_amount_remitted_previously".to_string(),
                "Item 15 exceeds Item 14; Item 16 would be negative and is not clamped".to_string(),
            ));
        }
        if !self.any_taxes_withheld && self.item_14_amount_of_remittance != 0.0 {
            errors.push((
                "any_taxes_withheld".to_string(),
                "Item 4 is No but Item 14 contains an amount of remittance".to_string(),
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

        let expected_16 =
            self.item_14_amount_of_remittance - self.item_15_amount_remitted_previously;
        let expected_17d =
            self.item_17a_surcharge + self.item_17b_interest + self.item_17c_compromise;
        let expected_18 = expected_16 + expected_17d;
        for (field, actual, expected) in [
            (
                "item_16_net_amount_of_remittance",
                self.item_16_net_amount_of_remittance,
                expected_16,
            ),
            (
                "item_17d_total_penalties",
                self.item_17d_total_penalties,
                expected_17d,
            ),
            (
                "item_18_total_amount_of_remittance",
                self.item_18_total_amount_of_remittance,
                expected_18,
            ),
        ] {
            if !actual.is_finite() || (actual - expected).abs() > 0.001 {
                errors.push((
                    field.to_string(),
                    format!("Computed amount must equal {expected:.2}"),
                ));
            }
        }

        validate_payment_row(
            "payment_19_cash_or_bank_debit_memo",
            &self.payment_details.cash_or_bank_debit_memo,
            false,
            &self.payment_details.others_description,
            &mut errors,
        );
        validate_payment_row(
            "payment_20_check",
            &self.payment_details.check,
            false,
            &self.payment_details.others_description,
            &mut errors,
        );
        validate_payment_row(
            "payment_21_tax_debit_memo",
            &self.payment_details.tax_debit_memo,
            false,
            &self.payment_details.others_description,
            &mut errors,
        );
        if !self
            .payment_details
            .tax_debit_memo
            .drawee_bank_or_agency
            .trim()
            .is_empty()
        {
            errors.push((
                "payment_21_tax_debit_memo.drawee_bank_or_agency".to_string(),
                "Item 21 Drawee Bank/Agency is non-applicable on the official January 2018 form"
                    .to_string(),
            ));
        }
        validate_payment_row(
            "payment_22_others",
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
    row: &Form0619EPaymentRow,
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
            "payment_22_others_description".to_string(),
            "Item 22 payment details require an Others description".to_string(),
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

impl TypedBirForm for Form0619EDraft {
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
        Form0619EDraft::recompute(self);
    }

    fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        Form0619EDraft::to_bir_field_map(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;

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
            "default_form_type": "0619Ev2018",
            "taxpayer_type": "Individual"
        }))
        .expect("test profile must deserialize")
    }

    fn valid_draft() -> Form0619EDraft {
        let mut draft = Form0619EDraft::new_from_profile(&test_profile(), 2026, 4);
        draft.due_day = Some(10);
        draft.any_taxes_withheld = true;
        draft.item_14_amount_of_remittance = 1_000.0;
        draft.item_17a_surcharge = 100.0;
        draft.item_17b_interest = 30.0;
        draft.item_17c_compromise = 100.0;
        draft.recompute();
        draft
    }

    #[test]
    fn exact_identity_and_fixed_codes_are_not_mutable_draft_fields() {
        let draft = valid_draft();
        assert_eq!(draft.form_type_id(), "0619Ev2018");
        assert_eq!(draft.atc_code(), "WME10");
        assert_eq!(draft.tax_type_code(), "WE");
    }

    #[test]
    fn official_formulas_are_deterministic_and_do_not_rewrite_timestamps() {
        let mut draft = valid_draft();
        draft.updated_at = "fixed".to_string();
        draft.recompute();
        assert_eq!(draft.item_16_net_amount_of_remittance, 1_000.0);
        assert_eq!(draft.item_17d_total_penalties, 230.0);
        assert_eq!(draft.item_18_total_amount_of_remittance, 1_230.0);
        assert_eq!(draft.updated_at, "fixed");
    }

    #[test]
    fn item_16_is_not_clamped_when_item_15_exceeds_item_14() {
        let mut draft = valid_draft();
        draft.is_amended = true;
        draft.item_15_amount_remitted_previously = 1_500.0;
        draft.recompute();
        assert_eq!(draft.item_16_net_amount_of_remittance, -500.0);
        assert!(
            draft
                .validate()
                .iter()
                .any(|(field, _)| { field == "item_15_amount_remitted_previously" })
        );
    }

    #[test]
    fn manual_penalties_are_never_replaced_by_a_clock_based_engine() {
        let mut draft = valid_draft();
        draft.item_17a_surcharge = 7.0;
        draft.item_17b_interest = 8.0;
        draft.item_17c_compromise = 9.0;
        draft.recompute();
        assert_eq!(draft.item_17a_surcharge, 7.0);
        assert_eq!(draft.item_17b_interest, 8.0);
        assert_eq!(draft.item_17c_compromise, 9.0);
        assert_eq!(draft.item_17d_total_penalties, 24.0);
    }

    #[test]
    fn december_due_period_rolls_into_the_next_year() {
        let draft = Form0619EDraft::new_from_profile(&test_profile(), 2026, 12);
        assert_eq!(draft.due_month_and_year(), (1, 2027));
    }

    #[test]
    fn due_day_is_manual_and_missing_evidence_fails_closed() {
        let draft = Form0619EDraft::new_from_profile(&test_profile(), 2026, 4);
        assert!(draft.validate().iter().any(|(field, message)| {
            field == "due_day" && message.contains("entered manually")
        }));
    }

    #[test]
    fn queue_transition_is_non_panicking_and_always_disabled() {
        let mut draft = valid_draft();
        let errors = draft
            .transition_to_queued()
            .expect_err("0619-E queueing must remain disabled");
        assert_eq!(draft.status, FilingStatus::Draft);
        assert!(errors.iter().any(|(field, _)| field == "submission"));
    }

    #[test]
    fn invalid_lifecycle_transitions_return_errors_instead_of_panicking() {
        let mut draft = valid_draft();
        draft.status = FilingStatus::Queued;
        assert!(draft.transition_to_submitted("x.xml".to_string()).is_err());
        assert_eq!(draft.status, FilingStatus::Queued);
        assert!(
            draft
                .record_submission_failure("failure".to_string())
                .is_err()
        );
        assert_eq!(draft.submission_attempts, 0);
        draft.status = FilingStatus::Submitted;
        assert!(
            draft
                .transition_to_confirmed("now".to_string(), None, None)
                .is_err()
        );
        assert_eq!(draft.status, FilingStatus::Submitted);
        draft.status = FilingStatus::Confirmed;
        assert!(draft.transition_to_paid().is_err());
        assert_eq!(draft.status, FilingStatus::Confirmed);
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
    fn item_21_rejects_the_officially_non_applicable_drawee_bank_field() {
        let mut draft = valid_draft();
        draft.payment_details.tax_debit_memo.drawee_bank_or_agency = "MUST NOT PRINT".to_string();

        assert!(draft.validate().iter().any(|(field, message)| {
            field == "payment_21_tax_debit_memo.drawee_bank_or_agency"
                && message.contains("non-applicable")
        }));
        assert!(draft.try_to_bir_xml_payload().is_err());
    }

    #[test]
    fn json_roundtrip_preserves_typed_payment_rows_and_evidence() {
        let mut draft = valid_draft();
        draft.payment_details.others = Form0619EPaymentRow {
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
        let reopened: Form0619EDraft =
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
        object.insert("txt_due_day".to_string(), serde_json::json!(10));
        object.remove("due_day");
        object.insert(
            "txt_line_bus".to_string(),
            serde_json::json!("LEGACY BUSINESS"),
        );
        object.remove("line_of_business");
        for (legacy, current) in [
            ("txt_tax14", "item_14_amount_of_remittance"),
            ("txt_tax15", "item_15_amount_remitted_previously"),
            ("txt_tax16", "item_16_net_amount_of_remittance"),
            ("txt_tax17a", "item_17a_surcharge"),
            ("txt_tax17b", "item_17b_interest"),
            ("txt_tax17c", "item_17c_compromise"),
            ("txt_tax17d", "item_17d_total_penalties"),
            ("txt_tax18", "item_18_total_amount_of_remittance"),
        ] {
            let field_value = object.remove(current).expect("current field exists");
            object.insert(legacy.to_string(), field_value);
        }

        let migrated: Form0619EDraft =
            serde_json::from_value(value).expect("legacy aliases should deserialize");
        assert_eq!(migrated.due_day, Some(10));
        assert_eq!(migrated.line_of_business, "LEGACY BUSINESS");
        assert_eq!(
            migrated.withholding_agent_category,
            WithholdingAgentCategory::Government
        );
        assert_eq!(migrated.item_18_total_amount_of_remittance, 1_230.0);
    }

    #[test]
    fn test_profile_fixture_uses_supported_tin_shape() {
        let profile = test_profile();
        assert_eq!(
            profile.tin.full(),
            Tin {
                segment1: "000".into(),
                segment2: "000".into(),
                segment3: "000".into(),
                branch: "00000".into(),
            }
            .full()
        );
    }
}
