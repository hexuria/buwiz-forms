//! BIR Form 0605, July 1999 (ENCS).
//!
//! This model is intentionally limited to behavior established by the pinned
//! two-page official form and two reviewed 235-field editable saves. Form 0605
//! remains manual/external: the reviewed saves prove an editable persistence
//! contract, not an electronic-submission contract.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::{FilingPeriod, FilingStatus, FormValidator, TypedBirForm};
use crate::profile::{TaxpayerProfile, TaxpayerType};
use crate::validation::{validate_email, validate_ph_phone, validate_zip};

pub const FORM_CODE: &str = "0605";
pub const FORM_REVISION: &str = "1999";
pub const FORM_TYPE_ID: &str = "0605v1999";
pub const FORM_VERSION_LABEL: &str = "July 1999 (ENCS)";
pub const QUEUE_SUBMISSION_SUPPORTED: bool = false;

/// Item 1. The two XML flags are derived from this single choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form0605FilingBasis {
    #[default]
    Calendar,
    Fiscal,
}

/// Item 11. The official form defines only Individual and Non-Individual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form0605TaxpayerClassification {
    #[default]
    Individual,
    NonIndividual,
}

/// Item 17. The five voluntary-payment and two audit/delinquency XML flags
/// are one semantic choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form0605MannerOfPayment {
    SelfAssessment,
    TaxDepositOrAdvancePayment,
    IncomeTaxSecondInstallmentIndividual,
    Penalties,
    Others,
    PreliminaryOrFinalAssessmentOrDeficiencyTax,
    AccountsReceivableOrDelinquentAccount,
}

impl Form0605MannerOfPayment {
    pub const ALL: [Self; 7] = [
        Self::SelfAssessment,
        Self::TaxDepositOrAdvancePayment,
        Self::IncomeTaxSecondInstallmentIndividual,
        Self::Penalties,
        Self::Others,
        Self::PreliminaryOrFinalAssessmentOrDeficiencyTax,
        Self::AccountsReceivableOrDelinquentAccount,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::SelfAssessment => "Self-Assessment",
            Self::TaxDepositOrAdvancePayment => "Tax Deposit / Advance Payment",
            Self::IncomeTaxSecondInstallmentIndividual => {
                "Income Tax Second Installment (Individual)"
            }
            Self::Penalties => "Penalties",
            Self::Others => "Others (Specify)",
            Self::PreliminaryOrFinalAssessmentOrDeficiencyTax => {
                "Preliminary / Final Assessment / Deficiency Tax"
            }
            Self::AccountsReceivableOrDelinquentAccount => {
                "Accounts Receivable / Delinquent Account"
            }
        }
    }
}

/// Item 18. The source samples prove XML option 1 is Installment and option 3
/// is Full; the intervening official option is Partial Payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form0605TypeOfPayment {
    Installment,
    PartialPayment,
    FullPayment,
}

impl Form0605TypeOfPayment {
    pub const ALL: [Self; 3] = [Self::Installment, Self::PartialPayment, Self::FullPayment];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Installment => "Installment",
            Self::PartialPayment => "Partial Payment",
            Self::FullPayment => "Full Payment",
        }
    }
}

/// The reviewed XML exposes two BIR-approval flags, but neither sample selects
/// one and the official printable form does not label them. Preserve the exact
/// flags without inventing Yes/No semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form0605ApprovalSelection {
    #[default]
    None,
    XmlOption1,
    XmlOption2,
}

/// Signature lines printed in Item 22 of the locked July 1999 form.
///
/// The reviewed editable saves do not contain keys for these lines. They are
/// persisted in the app draft for semantic HTML output, but are deliberately
/// omitted from the 235-field editable-save payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form0605SignatureDetails {
    pub taxpayer_or_authorized_representative: String,
    pub title_or_position: String,
    pub head_of_office: String,
}

/// Item 24, Check, has all four payment-detail columns on the official form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form0605CheckPayment {
    pub drawee_bank_or_agency: String,
    pub number: String,
    /// Manual `MM/DD/YYYY` value. No payment-channel date rule is inferred.
    pub date: String,
    /// `None` is an officially blank amount; `Some(0.0)` is an entered zero.
    pub amount: Option<f64>,
}

/// Item 25, Tax Debit Memo, has Number, Date, and Amount fields only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form0605TaxDebitMemoPayment {
    pub number: String,
    /// Manual `MM/DD/YYYY` value. No payment-channel date rule is inferred.
    pub date: String,
    /// `None` is an officially blank amount; `Some(0.0)` is an entered zero.
    pub amount: Option<f64>,
}

/// Item 26, Others, has all four payment-detail columns on the official form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form0605OtherPayment {
    pub drawee_bank_or_agency: String,
    pub number: String,
    /// Manual `MM/DD/YYYY` value. No payment-channel date rule is inferred.
    pub date: String,
    /// `None` is an officially blank amount; `Some(0.0)` is an entered zero.
    pub amount: Option<f64>,
}

/// The four fixed Part III rows on page 1 of the official form.
///
/// These values are PDF-backed draft/renderer data. The two reviewed editable
/// saves have no corresponding XML keys, so they never expand the exact
/// 235-field save contract or imply an electronic-submission contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form0605PaymentDetails {
    pub cash_or_bank_debit_memo_amount: Option<f64>,
    pub check: Form0605CheckPayment,
    pub tax_debit_memo: Form0605TaxDebitMemoPayment,
    pub others: Form0605OtherPayment,
    pub machine_validation_or_receipt_details: String,
}

/// A complete Gregorian date retained as three exact XML components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Form0605Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Form0605Date {
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, String> {
        let value = Self { year, month, day };
        value.validate()?;
        Ok(value)
    }

    pub fn parse_mm_dd_yyyy(value: &str) -> Result<Self, String> {
        let date = chrono::NaiveDate::parse_from_str(value.trim(), "%m/%d/%Y")
            .map_err(|_| "Date must use MM/DD/YYYY and be a real calendar date".to_string())?;
        let year = u16::try_from(chrono::Datelike::year(&date))
            .map_err(|_| "Date year is outside the supported range".to_string())?;
        let month = u8::try_from(chrono::Datelike::month(&date))
            .map_err(|_| "Date month is outside the supported range".to_string())?;
        let day = u8::try_from(chrono::Datelike::day(&date))
            .map_err(|_| "Date day is outside the supported range".to_string())?;
        Ok(Self { year, month, day })
    }

    pub fn validate(self) -> Result<(), String> {
        chrono::NaiveDate::from_ymd_opt(
            i32::from(self.year),
            u32::from(self.month),
            u32::from(self.day),
        )
        .map(|_| ())
        .ok_or_else(|| "Date is not a real calendar date".to_string())
    }
}

impl fmt::Display for Form0605Date {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:02}/{:02}/{:04}",
            self.month, self.day, self.year
        )
    }
}

/// Source-proven ATC choices. Their XML indexes come from the two reviewed
/// editable saves, not from the visual ordering of the official table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form0605ReviewedAtc {
    Fp010,
    Ii011,
}

impl Form0605ReviewedAtc {
    pub const ALL: [Self; 2] = [Self::Fp010, Self::Ii011];

    pub const fn code(self) -> &'static str {
        match self {
            Self::Fp010 => "FP010",
            Self::Ii011 => "II011",
        }
    }

    pub const fn xml_index(self) -> u16 {
        match self {
            Self::Fp010 => 1,
            Self::Ii011 => 24,
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Fp010 => "Fines and Penalties",
            Self::Ii011 => "Pure Compensation Income",
        }
    }
}

/// Source-proven Tax Type choices and indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form0605ReviewedTaxType {
    Do,
    It,
}

impl Form0605ReviewedTaxType {
    pub const ALL: [Self; 2] = [Self::Do, Self::It];

    pub const fn code(self) -> &'static str {
        match self {
            Self::Do => "DO",
            Self::It => "IT",
        }
    }

    pub const fn xml_index(self) -> u16 {
        match self {
            Self::Do => 4,
            Self::It => 9,
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Do => "Documentary Stamp Tax - One Time",
            Self::It => "Income Tax",
        }
    }
}

/// Distinguishes app-selectable reviewed mappings from exact imported pairs
/// that can be retained but cannot be treated as certified choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form0605CodeEvidence {
    ReviewedPair,
    ImportedExact,
}

/// Semantic code plus the one selected checkbox index in the 142/37-field
/// legacy matrix. Unknown imported pairs survive round-trip without becoming
/// app-authorized mappings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Form0605IndexedCode {
    code: String,
    xml_index: u16,
    evidence: Form0605CodeEvidence,
}

impl Form0605IndexedCode {
    pub fn reviewed_atc(value: Form0605ReviewedAtc) -> Self {
        Self {
            code: value.code().to_string(),
            xml_index: value.xml_index(),
            evidence: Form0605CodeEvidence::ReviewedPair,
        }
    }

    pub fn reviewed_tax_type(value: Form0605ReviewedTaxType) -> Self {
        Self {
            code: value.code().to_string(),
            xml_index: value.xml_index(),
            evidence: Form0605CodeEvidence::ReviewedPair,
        }
    }

    pub(crate) fn imported_atc(code: String, xml_index: u16) -> Self {
        let is_reviewed = Form0605ReviewedAtc::ALL
            .iter()
            .copied()
            .any(|candidate| candidate.code() == code && candidate.xml_index() == xml_index);
        let evidence = if is_reviewed {
            Form0605CodeEvidence::ReviewedPair
        } else {
            Form0605CodeEvidence::ImportedExact
        };
        Self {
            code,
            xml_index,
            evidence,
        }
    }

    pub(crate) fn imported_tax_type(code: String, xml_index: u16) -> Self {
        let is_reviewed = Form0605ReviewedTaxType::ALL
            .iter()
            .copied()
            .any(|candidate| candidate.code() == code && candidate.xml_index() == xml_index);
        let evidence = if is_reviewed {
            Form0605CodeEvidence::ReviewedPair
        } else {
            Form0605CodeEvidence::ImportedExact
        };
        Self {
            code,
            xml_index,
            evidence,
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub const fn xml_index(&self) -> u16 {
        self.xml_index
    }

    pub const fn evidence(&self) -> Form0605CodeEvidence {
        self.evidence
    }

    pub const fn requires_review(&self) -> bool {
        matches!(self.evidence, Form0605CodeEvidence::ImportedExact)
    }
}

/// Complete editable draft for exact identity `0605v1999`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Form0605Draft {
    pub id: Option<i64>,

    // Database/open-ended filing slot. It is not used to derive official
    // dates, quarter, or year-ended fields.
    pub tin: String,
    pub taxable_year: u16,
    pub month: u8,

    // Items 1-8.
    #[serde(default)]
    pub filing_basis: Form0605FilingBasis,
    #[serde(default = "default_quarter")]
    pub quarter: u8,
    #[serde(default = "default_year_end_month")]
    pub year_end_month: u8,
    #[serde(default)]
    pub due_date: Option<Form0605Date>,
    #[serde(default)]
    pub return_period: Option<Form0605Date>,
    #[serde(default)]
    pub number_of_sheets: u16,
    #[serde(default)]
    pub atc: Option<Form0605IndexedCode>,
    #[serde(default)]
    pub tax_type: Option<Form0605IndexedCode>,

    // Items 9-16.
    pub rdo_code: String,
    pub taxpayer_name: String,
    #[serde(default)]
    pub classification: Form0605TaxpayerClassification,
    #[serde(default, alias = "txt_line_bus")]
    pub line_of_business: String,
    pub registered_address: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,

    // Items 17-18.
    #[serde(default)]
    pub manner_of_payment: Option<Form0605MannerOfPayment>,
    #[serde(default)]
    pub other_manner_description: String,
    #[serde(default)]
    pub type_of_payment: Option<Form0605TypeOfPayment>,
    #[serde(default)]
    pub number_of_installments: Option<u16>,

    // Items 19-21. The aliases retain amount data from the generated scaffold.
    #[serde(alias = "txt_tax19")]
    pub item_19_basic_tax_or_payment: f64,
    #[serde(alias = "txt_tax20a")]
    pub item_20a_surcharge: f64,
    #[serde(alias = "txt_tax20b")]
    pub item_20b_interest: f64,
    #[serde(alias = "txt_tax20c")]
    pub item_20c_compromise: f64,
    #[serde(alias = "txt_tax20d")]
    pub item_20d_total_penalties: f64,
    #[serde(alias = "txt_tax21")]
    pub item_21_total_amount_payable: f64,

    // The only approval-related fields present in either exact XML source.
    #[serde(default)]
    pub approval_selection: Form0605ApprovalSelection,

    // Item 22 and Part III. These are official-PDF-backed app fields, but are
    // absent from both reviewed 235-field editable saves.
    #[serde(default)]
    pub signatures: Form0605SignatureDetails,
    #[serde(default)]
    pub payment_details: Form0605PaymentDetails,

    // Unknown future/source transport keys are retained, while every modeled
    // value overwrites the same key during export.
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

const fn default_quarter() -> u8 {
    1
}

const fn default_year_end_month() -> u8 {
    12
}

impl Form0605Draft {
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, period_slot: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            month: period_slot.clamp(1, 12),
            filing_basis: Form0605FilingBasis::Calendar,
            quarter: period_slot.clamp(1, 4),
            year_end_month: 12,
            due_date: None,
            return_period: None,
            number_of_sheets: 0,
            atc: None,
            tax_type: None,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            classification: if matches!(profile.taxpayer_type, TaxpayerType::Individual) {
                Form0605TaxpayerClassification::Individual
            } else {
                Form0605TaxpayerClassification::NonIndividual
            },
            line_of_business: profile.line_of_business.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            manner_of_payment: None,
            other_manner_description: String::new(),
            type_of_payment: None,
            number_of_installments: None,
            item_19_basic_tax_or_payment: 0.0,
            item_20a_surcharge: 0.0,
            item_20b_interest: 0.0,
            item_20c_compromise: 0.0,
            item_20d_total_penalties: 0.0,
            item_21_total_amount_payable: 0.0,
            approval_selection: Form0605ApprovalSelection::None,
            signatures: Form0605SignatureDetails::default(),
            payment_details: Form0605PaymentDetails::default(),
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

    pub fn select_reviewed_atc(&mut self, value: Form0605ReviewedAtc) {
        self.atc = Some(Form0605IndexedCode::reviewed_atc(value));
    }

    pub fn select_reviewed_tax_type(&mut self, value: Form0605ReviewedTaxType) {
        self.tax_type = Some(Form0605IndexedCode::reviewed_tax_type(value));
    }

    /// Compute only the two formulas printed on the official form. No penalty
    /// inference, non-negative clamp, or timestamp mutation occurs here.
    pub fn recompute(&mut self) {
        self.item_20d_total_penalties =
            self.item_20a_surcharge + self.item_20b_interest + self.item_20c_compromise;
        self.item_21_total_amount_payable =
            self.item_19_basic_tax_or_payment + self.item_20d_total_penalties;
    }

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub const fn can_queue_for_submission(&self) -> bool {
        QUEUE_SUBMISSION_SUPPORTED
    }

    pub fn evidence_warnings(&self) -> Vec<String> {
        let mut warnings = vec![
            "Only FP010↔AtcCode1, II011↔AtcCode24, DO↔TaxTypeCode4, and IT↔TaxTypeCode9 are source-proven editable mappings. Other imported pairs are retained but not certified."
                .to_string(),
            "Item 22 signatures and Part III payment details are backed by the official PDF and persist in the app draft, but the two reviewed 235-field saves contain no corresponding XML keys; they are omitted from editable-save export."
                .to_string(),
            "The meaning of frm0605:itemApprovedYN options 1 and 2 is not established because both reviewed saves leave both flags false."
                .to_string(),
        ];
        if self
            .atc
            .as_ref()
            .is_some_and(Form0605IndexedCode::requires_review)
        {
            warnings.push(
                "The imported ATC/index pair requires review before safe export.".to_string(),
            );
        }
        if self
            .tax_type
            .as_ref()
            .is_some_and(Form0605IndexedCode::requires_review)
        {
            warnings.push(
                "The imported Tax Type/index pair requires review before safe export.".to_string(),
            );
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

    /// Queueing remains unavailable until an exact submission contract exists.
    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        let mut errors = self.validate();
        errors.push((
            "submission".to_string(),
            "0605v1999 submission is manual/external; queue transport is not certified".to_string(),
        ));
        Err(errors)
    }

    pub fn transition_to_submitted(&mut self, _filename: String) -> Result<(), String> {
        Err(
            "0605v1999 cannot transition to Submitted because queue/submission transport is not certified"
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
            "0605v1999 cannot transition to Confirmed because its submission-response contract is not certified"
                .to_string(),
        )
    }

    pub fn transition_to_paid(&mut self) -> Result<(), String> {
        Err(
            "0605v1999 cannot transition to Paid through an uncertified submission lifecycle"
                .to_string(),
        )
    }

    pub fn revert_to_draft(&mut self) -> Result<(), String> {
        if matches!(self.status, FilingStatus::Paid) {
            return Err("A paid 0605 record cannot be reverted automatically".to_string());
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

    pub fn record_submission_failure(&mut self, _error_message: String) -> Result<(), String> {
        Err("0605v1999 has no certified queue attempt or retry persistence contract".to_string())
    }
}

impl FormValidator for Form0605Draft {
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
                "TIN must contain 12 to 14 digits including the 3- to 5-digit branch code"
                    .to_string(),
            ));
        }
        if !(1..=12).contains(&self.month) {
            errors.push((
                "month".to_string(),
                "Open-ended persistence slot must be from 1 to 12".to_string(),
            ));
        }
        if !(1..=4).contains(&self.quarter) {
            errors.push((
                "quarter".to_string(),
                "Quarter must be selected independently from 1 to 4".to_string(),
            ));
        }
        if !(1..=12).contains(&self.year_end_month) {
            errors.push((
                "year_end_month".to_string(),
                "Year Ended month must be from 1 to 12".to_string(),
            ));
        }
        if !(1900..=9999).contains(&self.taxable_year) {
            errors.push((
                "taxable_year".to_string(),
                "Year Ended must contain four digits".to_string(),
            ));
        }
        validate_required_date("due_date", self.due_date, &mut errors);
        validate_required_date("return_period", self.return_period, &mut errors);

        for (field, label, value) in [
            ("rdo_code", "RDO code", self.rdo_code.as_str()),
            (
                "taxpayer_name",
                "Taxpayer name",
                self.taxpayer_name.as_str(),
            ),
            (
                "line_of_business",
                "Line of business / occupation",
                self.line_of_business.as_str(),
            ),
            (
                "registered_address",
                "Registered address",
                self.registered_address.as_str(),
            ),
            ("zip_code", "ZIP code", self.zip_code.as_str()),
            (
                "contact_number",
                "Telephone number",
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
                "Telephone number must be a valid Philippine mobile or landline number".to_string(),
            ));
        }
        if !self.email.trim().is_empty() && !validate_email(&self.email) {
            errors.push(("email".to_string(), "Email address is invalid".to_string()));
        }

        validate_code_selection(
            "atc",
            self.atc.as_ref(),
            142,
            |selection| {
                Form0605ReviewedAtc::ALL.iter().copied().any(|candidate| {
                    candidate.code() == selection.code()
                        && candidate.xml_index() == selection.xml_index()
                })
            },
            &mut errors,
        );
        validate_code_selection(
            "tax_type",
            self.tax_type.as_ref(),
            37,
            |selection| {
                Form0605ReviewedTaxType::ALL
                    .iter()
                    .copied()
                    .any(|candidate| {
                        candidate.code() == selection.code()
                            && candidate.xml_index() == selection.xml_index()
                    })
            },
            &mut errors,
        );

        match self.manner_of_payment {
            None => errors.push((
                "manner_of_payment".to_string(),
                "Item 17 Manner of Payment is required".to_string(),
            )),
            Some(Form0605MannerOfPayment::Others) => {
                if self.other_manner_description.trim().is_empty() {
                    errors.push((
                        "other_manner_description".to_string(),
                        "Item 17 Others requires a description".to_string(),
                    ));
                }
            }
            Some(_) if !self.other_manner_description.trim().is_empty() => errors.push((
                "other_manner_description".to_string(),
                "Clear the Others description unless Item 17 Others is selected".to_string(),
            )),
            Some(_) => {}
        }

        match self.type_of_payment {
            None => errors.push((
                "type_of_payment".to_string(),
                "Item 18 Type of Payment is required".to_string(),
            )),
            Some(Form0605TypeOfPayment::Installment) => {
                if self.number_of_installments.is_none_or(|count| count == 0) {
                    errors.push((
                        "number_of_installments".to_string(),
                        "Installment payment requires a positive number of installments"
                            .to_string(),
                    ));
                }
            }
            Some(_) if self.number_of_installments.is_some() => errors.push((
                "number_of_installments".to_string(),
                "Number of installments applies only when Installment is selected".to_string(),
            )),
            Some(_) => {}
        }

        for (field, value) in [
            (
                "item_19_basic_tax_or_payment",
                self.item_19_basic_tax_or_payment,
            ),
            ("item_20a_surcharge", self.item_20a_surcharge),
            ("item_20b_interest", self.item_20b_interest),
            ("item_20c_compromise", self.item_20c_compromise),
        ] {
            if !value.is_finite() || value < 0.0 {
                errors.push((
                    field.to_string(),
                    "Amount must be a finite, non-negative number".to_string(),
                ));
            }
        }

        for (field, value) in [
            (
                "payment_23_cash_or_bank_debit_memo.amount",
                self.payment_details.cash_or_bank_debit_memo_amount,
            ),
            ("payment_24_check.amount", self.payment_details.check.amount),
            (
                "payment_25_tax_debit_memo.amount",
                self.payment_details.tax_debit_memo.amount,
            ),
            (
                "payment_26_others.amount",
                self.payment_details.others.amount,
            ),
        ] {
            if value.is_some_and(|amount| !amount.is_finite() || amount < 0.0) {
                errors.push((
                    field.to_string(),
                    "Payment amount must be a finite, non-negative number".to_string(),
                ));
            }
        }

        for (field, value) in [
            (
                "payment_24_check.date",
                self.payment_details.check.date.as_str(),
            ),
            (
                "payment_25_tax_debit_memo.date",
                self.payment_details.tax_debit_memo.date.as_str(),
            ),
            (
                "payment_26_others.date",
                self.payment_details.others.date.as_str(),
            ),
        ] {
            if !value.trim().is_empty()
                && chrono::NaiveDate::parse_from_str(value.trim(), "%m/%d/%Y").is_err()
            {
                errors.push((
                    field.to_string(),
                    "Payment date must use MM/DD/YYYY and be a real calendar date".to_string(),
                ));
            }
        }

        let expected_20d =
            self.item_20a_surcharge + self.item_20b_interest + self.item_20c_compromise;
        let expected_21 = self.item_19_basic_tax_or_payment + expected_20d;
        for (field, actual, expected) in [
            (
                "item_20d_total_penalties",
                self.item_20d_total_penalties,
                expected_20d,
            ),
            (
                "item_21_total_amount_payable",
                self.item_21_total_amount_payable,
                expected_21,
            ),
        ] {
            if !actual.is_finite() || actual < 0.0 || (actual - expected).abs() > 0.001 {
                errors.push((
                    field.to_string(),
                    format!("Computed amount must equal {expected:.2} and be non-negative"),
                ));
            }
        }

        errors
    }
}

fn validate_required_date(
    field: &str,
    value: Option<Form0605Date>,
    errors: &mut Vec<(String, String)>,
) {
    match value {
        None => errors.push((field.to_string(), "Date is required".to_string())),
        Some(date) => {
            if let Err(message) = date.validate() {
                errors.push((field.to_string(), message));
            }
        }
    }
}

fn validate_code_selection(
    field: &str,
    selection: Option<&Form0605IndexedCode>,
    maximum_index: u16,
    is_reviewed_pair: impl Fn(&Form0605IndexedCode) -> bool,
    errors: &mut Vec<(String, String)>,
) {
    let Some(selection) = selection else {
        errors.push((field.to_string(), format!("{field} selection is required")));
        return;
    };
    if selection.code().trim().is_empty()
        || selection.xml_index() == 0
        || selection.xml_index() > maximum_index
    {
        errors.push((
            field.to_string(),
            format!("{field} code/index pair is invalid"),
        ));
    }
    if selection.requires_review() {
        errors.push((
            field.to_string(),
            format!(
                "Imported {field} pair {}↔index {} is preserved but lacks reviewed mapping evidence",
                selection.code(),
                selection.xml_index()
            ),
        ));
    } else if !is_reviewed_pair(selection) {
        errors.push((
            field.to_string(),
            format!(
                "Persisted {field} pair {}↔index {} is marked reviewed but does not match locked source evidence",
                selection.code(),
                selection.xml_index()
            ),
        ));
    }
}

impl TypedBirForm for Form0605Draft {
    fn form_code(&self) -> &'static str {
        FORM_CODE
    }

    fn form_type_id(&self) -> &'static str {
        FORM_TYPE_ID
    }

    fn filing_period(&self) -> FilingPeriod {
        FilingPeriod::OpenEnded(u32::from(self.month))
    }

    fn recompute(&mut self) {
        Form0605Draft::recompute(self);
    }

    fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        Form0605Draft::to_bir_field_map(self)
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
            "default_form_type": "0605v1999",
            "taxpayer_type": "Individual"
        }))
        .expect("test profile must deserialize")
    }

    fn valid_draft() -> Form0605Draft {
        let mut draft = Form0605Draft::new_from_profile(&test_profile(), 2026, 1);
        draft.due_date = Some(Form0605Date::new(2026, 1, 1).unwrap());
        draft.return_period = Some(Form0605Date::new(2026, 1, 31).unwrap());
        draft.select_reviewed_atc(Form0605ReviewedAtc::Fp010);
        draft.select_reviewed_tax_type(Form0605ReviewedTaxType::Do);
        draft.manner_of_payment = Some(Form0605MannerOfPayment::SelfAssessment);
        draft.type_of_payment = Some(Form0605TypeOfPayment::FullPayment);
        draft.item_19_basic_tax_or_payment = 10.0;
        draft.recompute();
        draft
    }

    #[test]
    fn exact_identity_uses_july_1999_revision() {
        let draft = valid_draft();
        assert_eq!(draft.form_type_id(), "0605v1999");
        assert_eq!(FORM_VERSION_LABEL, "July 1999 (ENCS)");
    }

    #[test]
    fn official_formulas_are_deterministic_without_timestamp_mutation() {
        let mut draft = valid_draft();
        draft.item_19_basic_tax_or_payment = 1_000.0;
        draft.item_20a_surcharge = 10.0;
        draft.item_20b_interest = 20.0;
        draft.item_20c_compromise = 1_000.0;
        draft.updated_at = "fixed".to_string();
        draft.recompute();
        assert_eq!(draft.item_20d_total_penalties, 1_030.0);
        assert_eq!(draft.item_21_total_amount_payable, 2_030.0);
        assert_eq!(draft.updated_at, "fixed");
    }

    #[test]
    fn quarter_is_not_derived_from_return_period_month() {
        let mut draft = valid_draft();
        draft.quarter = 1;
        draft.return_period = Some(Form0605Date::new(2025, 12, 31).unwrap());
        assert_eq!(draft.quarter, 1);
    }

    #[test]
    fn only_source_proven_code_pairs_are_app_selectable() {
        let mut draft = valid_draft();
        draft.select_reviewed_atc(Form0605ReviewedAtc::Ii011);
        draft.select_reviewed_tax_type(Form0605ReviewedTaxType::It);
        assert_eq!(draft.atc.as_ref().unwrap().xml_index(), 24);
        assert_eq!(draft.tax_type.as_ref().unwrap().xml_index(), 9);
    }

    #[test]
    fn imported_unknown_code_pair_is_preserved_but_fails_closed() {
        let mut draft = valid_draft();
        draft.atc = Some(Form0605IndexedCode::imported_atc(
            "UNREVIEWED".to_string(),
            77,
        ));
        assert_eq!(draft.atc.as_ref().unwrap().code(), "UNREVIEWED");
        assert!(draft.validate().iter().any(|(field, _)| field == "atc"));
    }

    #[test]
    fn forged_reviewed_evidence_cannot_authorize_an_unknown_code_pair() {
        let mut draft = valid_draft();
        draft.atc = Some(Form0605IndexedCode {
            code: "UNREVIEWED".to_string(),
            xml_index: 77,
            evidence: Form0605CodeEvidence::ReviewedPair,
        });

        assert!(draft.validate().iter().any(|(field, message)| {
            field == "atc" && message.contains("does not match locked source evidence")
        }));
    }

    #[test]
    fn invalid_date_is_rejected_without_panicking() {
        assert!(Form0605Date::new(2026, 2, 30).is_err());
        assert!(Form0605Date::parse_mm_dd_yyyy("02/30/2026").is_err());
    }

    #[test]
    fn installment_requires_positive_count() {
        let mut draft = valid_draft();
        draft.type_of_payment = Some(Form0605TypeOfPayment::Installment);
        draft.number_of_installments = None;
        assert!(
            draft
                .validate()
                .iter()
                .any(|(field, _)| field == "number_of_installments")
        );
    }

    #[test]
    fn queue_transition_is_non_panicking_and_always_disabled() {
        let mut draft = valid_draft();
        let errors = draft
            .transition_to_queued()
            .expect_err("0605 queueing must remain disabled");
        assert_eq!(draft.status, FilingStatus::Draft);
        assert!(errors.iter().any(|(field, _)| field == "submission"));
    }

    #[test]
    fn injected_queued_status_cannot_transition_to_submitted() {
        let mut draft = valid_draft();
        draft.status = FilingStatus::Queued;
        let result = draft.transition_to_submitted("x.xml".to_string());
        assert!(result.is_err() && matches!(draft.status, FilingStatus::Queued));
    }

    #[test]
    fn injected_submitted_status_cannot_transition_to_confirmed() {
        let mut draft = valid_draft();
        draft.status = FilingStatus::Submitted;
        let result = draft.transition_to_confirmed("now".to_string(), None, None);
        assert!(
            result.is_err() && matches!(draft.status, FilingStatus::Submitted),
            "uncertified confirmation transition mutated the draft"
        );
    }

    #[test]
    fn injected_confirmed_status_cannot_transition_to_paid() {
        let mut draft = valid_draft();
        draft.status = FilingStatus::Confirmed;
        let result = draft.transition_to_paid();
        assert!(
            result.is_err() && matches!(draft.status, FilingStatus::Confirmed),
            "uncertified payment transition mutated the draft"
        );
    }

    #[test]
    fn injected_queued_status_cannot_record_submission_failure() {
        let mut draft = valid_draft();
        draft.status = FilingStatus::Queued;
        let result = draft.record_submission_failure("failure".to_string());
        assert!(
            result.is_err()
                && matches!(draft.status, FilingStatus::Queued)
                && draft.submission_attempts == 0
                && draft.last_error.is_none(),
            "uncertified retry transition mutated the draft"
        );
    }

    #[test]
    fn json_roundtrip_preserves_semantic_selections_and_dates() {
        let mut draft = valid_draft();
        draft.signatures.taxpayer_or_authorized_representative = "JUAN DELA CRUZ".to_string();
        draft.signatures.title_or_position = "OWNER".to_string();
        draft.payment_details.check.drawee_bank_or_agency = "AAB".to_string();
        draft.payment_details.check.number = "000123".to_string();
        draft.payment_details.check.date = "01/31/2026".to_string();
        draft.payment_details.check.amount = Some(10.0);
        let json = serde_json::to_string(&draft).expect("draft should serialize");
        let reopened: Form0605Draft =
            serde_json::from_str(&json).expect("draft should deserialize");
        assert_eq!(reopened, draft);
    }

    #[test]
    fn pdf_only_signature_and_payment_fields_do_not_expand_editable_xml_contract() {
        let mut draft = valid_draft();
        draft.signatures.taxpayer_or_authorized_representative = "JUAN DELA CRUZ".to_string();
        draft.payment_details.cash_or_bank_debit_memo_amount = Some(10.0);
        draft.payment_details.check.number = "CHECK-1".to_string();
        draft.payment_details.check.date = "01/31/2026".to_string();
        draft.payment_details.check.amount = Some(10.0);

        let fields = draft.to_bir_field_map();

        assert_eq!(fields.len(), 235);
        assert!(
            fields
                .keys()
                .all(|key| !key.contains("Signature") && !key.starts_with("payment_"))
        );
    }

    #[test]
    fn invalid_payment_values_fail_validation_without_panicking() {
        let mut draft = valid_draft();
        draft.payment_details.check.date = "02/30/2026".to_string();
        draft.payment_details.check.amount = Some(f64::NAN);

        let errors = draft.validate();

        assert!(
            errors
                .iter()
                .any(|(field, _)| field == "payment_24_check.date")
        );
        assert!(
            errors
                .iter()
                .any(|(field, _)| field == "payment_24_check.amount")
        );
    }
}
