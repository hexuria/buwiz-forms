//! BIR Form 2550Q, April 2024 (ENCS).
//!
//! The model is bounded by the locked two-page official form, its official
//! guidelines, and the reviewed 160-field plain/encrypted editable-save pair.
//! The payloads establish editable persistence, not electronic submission.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use super::{FilingPeriod, FilingStatus, FormValidator, TypedBirForm};
use crate::profile::{EoptTier, TaxpayerProfile};
use crate::validation::{validate_email, validate_ph_phone, validate_zip};

pub const FORM_CODE: &str = "2550Q";
pub const FORM_REVISION: &str = "2024";
pub const FORM_TYPE_ID: &str = "2550Qv2024";
pub const FORM_VERSION_LABEL: &str = "April 2024 (ENCS)";
pub const QUEUE_SUBMISSION_SUPPORTED: bool = false;
pub const OFFICIAL_FORM_SHA256: &str =
    "18eb16925010fdda820cef958221ba2c0d073066efa93a898113e39b31135a25";
pub const OFFICIAL_GUIDELINES_SHA256: &str =
    "b6ee4f090cb48963a44b1ef58fd6cdb4b5865ba4674963c3661c7f164895b120";
pub const REVIEWED_EDITABLE_XML_SHA256: &str =
    "43577fdd70b8959b16dbada9ff7d8418a1fdc5d18e61302c8cbfc8e9bbab4520";
pub const REVIEWED_ENCRYPTED_XML_SHA256: &str =
    "57ccf9d8132c490d54bceaf5c55fc2b4bec01b780951a63600402c61a595cdbe";

const VAT_RATE: f64 = 0.12;
const FIXED_SCHEDULE_ROW_COUNT: usize = 2;

const fn default_year_end_month() -> u8 {
    12
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form2550QFilingBasis {
    #[default]
    Calendar,
    Fiscal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form2550QQuarter {
    First,
    Second,
    Third,
    Fourth,
    /// Retains invalid legacy/caller data so validation can fail closed.
    Unresolved(u8),
}

impl Default for Form2550QQuarter {
    fn default() -> Self {
        Self::Unresolved(0)
    }
}

impl Form2550QQuarter {
    pub const ALL: [Self; 4] = [Self::First, Self::Second, Self::Third, Self::Fourth];

    pub const fn from_number(value: u8) -> Self {
        match value {
            1 => Self::First,
            2 => Self::Second,
            3 => Self::Third,
            4 => Self::Fourth,
            other => Self::Unresolved(other),
        }
    }

    pub const fn number(self) -> Option<u8> {
        match self {
            Self::First => Some(1),
            Self::Second => Some(2),
            Self::Third => Some(3),
            Self::Fourth => Some(4),
            Self::Unresolved(_) => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::First => "1st",
            Self::Second => "2nd",
            Self::Third => "3rd",
            Self::Fourth => "4th",
            Self::Unresolved(_) => "Needs review",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form2550QTaxpayerClassification {
    Micro,
    Small,
    Medium,
    Large,
}

impl Form2550QTaxpayerClassification {
    pub const ALL: [Self; 4] = [Self::Micro, Self::Small, Self::Medium, Self::Large];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Micro => "Micro",
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }
}

impl From<&EoptTier> for Form2550QTaxpayerClassification {
    fn from(value: &EoptTier) -> Self {
        match value {
            EoptTier::Micro => Self::Micro,
            EoptTier::Small => Self::Small,
            EoptTier::Medium => Self::Medium,
            EoptTier::Large => Self::Large,
        }
    }
}

/// The reviewed editable payload uses `1`; its encrypted companion uses `0`.
/// Neither value is promoted to lifecycle truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form2550QXmlFinalFlag {
    Zero,
    #[default]
    One,
    Missing,
    Unknown(String),
}

impl Form2550QXmlFinalFlag {
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

/// A checked Gregorian date with the editable-save formatting used by 2550Q.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Form2550QDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Form2550QDate {
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, String> {
        let value = Self { year, month, day };
        value.validate()?;
        Ok(value)
    }

    pub fn parse_return_period(value: &str) -> Result<Self, String> {
        let trimmed = value.trim();
        let date = ["%m/%d/%Y", "%-m/%d/%Y", "%-m/%-d/%Y"]
            .iter()
            .find_map(|format| chrono::NaiveDate::parse_from_str(trimmed, format).ok())
            .ok_or_else(|| "Date must use MM/DD/YYYY and be a real calendar date".to_string())?;
        Self::from_naive(date)
    }

    pub fn parse_filed_date(value: &str) -> Result<Self, String> {
        let date = chrono::NaiveDate::parse_from_str(value.trim(), "%Y/%m/%d")
            .map_err(|_| "Filed date must use YYYY/MM/DD".to_string())?;
        Self::from_naive(date)
    }

    fn from_naive(date: chrono::NaiveDate) -> Result<Self, String> {
        use chrono::Datelike;
        Ok(Self {
            year: u16::try_from(date.year())
                .map_err(|_| "Date year is outside the supported range".to_string())?,
            month: u8::try_from(date.month())
                .map_err(|_| "Date month is outside the supported range".to_string())?,
            day: u8::try_from(date.day())
                .map_err(|_| "Date day is outside the supported range".to_string())?,
        })
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

    pub fn as_naive(self) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::from_ymd_opt(
            i32::from(self.year),
            u32::from(self.month),
            u32::from(self.day),
        )
    }

    pub fn to_filed_date(self) -> String {
        format!("{:04}/{:02}/{:02}", self.year, self.month, self.day)
    }
}

impl fmt::Display for Form2550QDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The reviewed app save uses an unpadded month and a padded day.
        write!(formatter, "{}/{:02}/{:04}", self.month, self.day, self.year)
    }
}

fn deserialize_optional_date_compat<'de, D>(
    deserializer: D,
) -> Result<Option<Form2550QDate>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DateCompat {
        Typed(Form2550QDate),
        LegacyText(String),
    }

    match Option::<DateCompat>::deserialize(deserializer)? {
        None => Ok(None),
        Some(DateCompat::Typed(value)) => {
            value.validate().map_err(serde::de::Error::custom)?;
            Ok(Some(value))
        }
        Some(DateCompat::LegacyText(value)) if value.trim().is_empty() => Ok(None),
        Some(DateCompat::LegacyText(value)) => Form2550QDate::parse_filed_date(&value)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

/// One of the two fixed Schedule 1 source rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form2550QCapitalGoodRow {
    pub purchase_or_import_date: Option<Form2550QDate>,
    /// Official source code: `D` for domestic purchase or `I` for importation.
    pub source_code: String,
    pub description: String,
    pub purchase_or_import_amount: Option<f64>,
    pub input_tax: Option<f64>,
    pub estimated_life_months: Option<u16>,
    pub recognized_life_months: Option<u16>,
    /// Preserved from the reviewed save. The available evidence does not expose
    /// the number-of-months-in-use input needed to derive this safely.
    pub allowable_input_tax_for_period: Option<f64>,
    pub balance_to_next_period: Option<f64>,
}

impl Form2550QCapitalGoodRow {
    pub fn is_empty(&self) -> bool {
        self.purchase_or_import_date.is_none()
            && self.source_code.trim().is_empty()
            && self.description.trim().is_empty()
            && self
                .purchase_or_import_amount
                .is_none_or(|value| value == 0.0)
            && self.input_tax.is_none_or(|value| value == 0.0)
            && self.estimated_life_months.is_none_or(|value| value == 0)
            && self.recognized_life_months.is_none_or(|value| value == 0)
            && self
                .allowable_input_tax_for_period
                .is_none_or(|value| value == 0.0)
            && self.balance_to_next_period.is_none_or(|value| value == 0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form2550QSchedule2 {
    pub input_tax_directly_attributable_to_exempt_sales: Option<f64>,
    pub vat_exempt_sales: Option<f64>,
    pub input_tax_not_directly_attributable: Option<f64>,
    pub total_sales: Option<f64>,
    pub ratable_input_tax: Option<f64>,
    pub total_input_tax_attributable_to_exempt_sales: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form2550QCreditableVatRow {
    pub period_from: Option<Form2550QDate>,
    pub period_to: Option<Form2550QDate>,
    pub withholding_agent_name: String,
    pub income_payment: Option<f64>,
    pub tax_withheld: Option<f64>,
}

impl Form2550QCreditableVatRow {
    pub fn is_empty(&self) -> bool {
        self.period_from.is_none()
            && self.period_to.is_none()
            && self.withholding_agent_name.trim().is_empty()
            && self.income_payment.is_none_or(|value| value == 0.0)
            && self.tax_withheld.is_none_or(|value| value == 0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form2550QAdvanceVatRow {
    pub period_from: Option<Form2550QDate>,
    pub period_to: Option<Form2550QDate>,
    pub miller_name: String,
    pub taxpayer_name: String,
    /// Official receipt numbers are identifiers, never money.
    pub official_receipt_number: String,
    pub amount_paid: Option<f64>,
}

impl Form2550QAdvanceVatRow {
    pub fn is_empty(&self) -> bool {
        self.period_from.is_none()
            && self.period_to.is_none()
            && self.miller_name.trim().is_empty()
            && self.taxpayer_name.trim().is_empty()
            && matches!(self.official_receipt_number.trim(), "" | "0.00")
            && self.amount_paid.is_none_or(|value| value == 0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form2550QPartIV {
    pub item_31a_vatable_sales: Option<f64>,
    pub item_31b_output_tax: Option<f64>,
    pub item_32a_zero_rated_sales: Option<f64>,
    pub item_33a_exempt_sales: Option<f64>,
    pub item_34a_total_sales: Option<f64>,
    pub item_34b_output_tax_due: Option<f64>,
    pub item_35b_less_output_vat_uncollected: Option<f64>,
    pub item_36b_add_output_vat_recovered: Option<f64>,
    pub item_37b_adjusted_output_tax_due: Option<f64>,
    pub item_38b_input_tax_carried: Option<f64>,
    pub item_39b_input_tax_deferred: Option<f64>,
    pub item_40b_transitional_input_tax: Option<f64>,
    pub item_41b_presumptive_input_tax: Option<f64>,
    pub item_42_description: String,
    pub item_42b_other_input_tax: Option<f64>,
    pub item_43b_total_prior_input_tax: Option<f64>,
    pub item_44a_domestic_purchases: Option<f64>,
    pub item_44b_domestic_input_tax: Option<f64>,
    pub item_45a_nonresident_services: Option<f64>,
    pub item_45b_nonresident_service_input_tax: Option<f64>,
    pub item_46a_importations: Option<f64>,
    pub item_46b_import_input_tax: Option<f64>,
    pub item_47_description: String,
    pub item_47a_other_purchases: Option<f64>,
    pub item_47b_other_input_tax: Option<f64>,
    pub item_48a_domestic_purchases_no_input_tax: Option<f64>,
    pub item_49a_vat_exempt_importations: Option<f64>,
    pub item_50a_total_current_purchases: Option<f64>,
    pub item_50b_total_current_input_tax: Option<f64>,
    pub item_51b_total_available_input_tax: Option<f64>,
    pub item_52b_deferred_capital_goods_input_tax: Option<f64>,
    pub item_53b_input_tax_attributable_to_exempt_sales: Option<f64>,
    pub item_54b_vat_refund_or_tcc_claimed: Option<f64>,
    pub item_55b_input_vat_on_unpaid_payables: Option<f64>,
    pub item_56_description: String,
    pub item_56b_other_deduction: Option<f64>,
    pub item_57b_total_deductions: Option<f64>,
    pub item_58b_input_vat_on_settled_payables: Option<f64>,
    pub item_59b_adjusted_deductions: Option<f64>,
    pub item_60b_total_allowable_input_tax: Option<f64>,
    pub item_61b_net_vat_payable_or_excess: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form2550QPartII {
    pub item_15_net_vat_payable_or_excess: Option<f64>,
    pub item_16_creditable_vat_withheld: Option<f64>,
    pub item_17_advance_vat_payments: Option<f64>,
    pub item_18_paid_on_previous_return: Option<f64>,
    pub item_19_description: String,
    pub item_19_other_credit_or_payment: Option<f64>,
    pub item_20_total_credits_or_payments: Option<f64>,
    pub item_21_tax_payable_or_excess_credits: Option<f64>,
    pub item_22_surcharge: Option<f64>,
    pub item_23_interest: Option<f64>,
    pub item_24_compromise: Option<f64>,
    pub item_25_total_penalties: Option<f64>,
    pub item_26_total_amount_payable_or_excess: Option<f64>,
}

/// Page-one fields not present in either reviewed XML payload. They are local
/// draft/HTML values and never expand the exact 160-field save contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form2550QLocalPrintFields {
    pub taxpayer_or_authorized_representative: String,
    pub representative_title: String,
    pub non_individual_authorized_officer: String,
    pub tax_agent_accreditation_or_roll_number: String,
    pub tax_agent_date_of_issue: String,
    pub tax_agent_date_of_expiry: String,
    pub cash_or_bank_debit_advice_amount: Option<f64>,
    pub check_bank: String,
    pub check_number: String,
    pub check_date: String,
    pub check_amount: Option<f64>,
    pub tax_debit_memo_number: String,
    pub tax_debit_memo_date: String,
    pub tax_debit_memo_amount: Option<f64>,
    pub other_payment_description: String,
    pub other_payment_bank: String,
    pub other_payment_number: String,
    pub other_payment_date: String,
    pub other_payment_amount: Option<f64>,
    pub machine_validation_or_receipt_details: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Form2550QDraft {
    pub id: Option<i64>,

    // Filing period.
    pub tin: String,
    pub taxable_year: u16,
    #[serde(default)]
    pub filing_basis: Form2550QFilingBasis,
    #[serde(default = "default_year_end_month")]
    pub year_end_month: u8,
    #[serde(default)]
    pub quarter: Form2550QQuarter,
    #[serde(default)]
    pub return_period_from: Option<Form2550QDate>,
    #[serde(default)]
    pub return_period_to: Option<Form2550QDate>,
    pub is_amended: bool,
    #[serde(default)]
    pub is_short_period_return: bool,

    // Profile-prefilled values.
    pub rdo_code: String,
    pub taxpayer_name: String,
    pub registered_address: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,
    #[serde(default)]
    pub taxpayer_classification: Option<Form2550QTaxpayerClassification>,
    #[serde(default)]
    pub is_availing_tax_relief: bool,
    #[serde(default)]
    pub tax_relief_details: String,

    #[serde(default)]
    pub part_ii: Form2550QPartII,
    #[serde(default)]
    pub part_iv: Form2550QPartIV,
    #[serde(default)]
    pub schedule_1: Vec<Form2550QCapitalGoodRow>,
    #[serde(default)]
    pub schedule_2: Form2550QSchedule2,
    #[serde(default)]
    pub schedule_3: Vec<Form2550QCreditableVatRow>,
    #[serde(default)]
    pub schedule_4: Vec<Form2550QAdvanceVatRow>,
    #[serde(default)]
    pub local_print_fields: Form2550QLocalPrintFields,

    // Editable-save metadata. Online credential fields are deliberately not
    // persisted because both reviewed sources contain them only as blanks.
    #[serde(default)]
    pub xml_final_flag: Form2550QXmlFinalFlag,
    #[serde(default)]
    pub xml_contact_email: String,
    #[serde(default, deserialize_with = "deserialize_optional_date_compat")]
    pub date_filed: Option<Form2550QDate>,
    #[serde(default)]
    pub preserved_unmodeled_xml_fields: BTreeMap<String, String>,
    #[serde(default)]
    pub migration_review_items: Vec<String>,

    /// Unknown JSON properties are retained for forward compatibility. When
    /// the old scaffold-only `month` marker is present, the editor performs a
    /// one-way migration from those flat properties and then clears them.
    #[serde(default, flatten, skip_serializing_if = "BTreeMap::is_empty")]
    pub legacy_flat_draft_fields: BTreeMap<String, serde_json::Value>,

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

impl Form2550QDraft {
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, quarter: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let quarter = Form2550QQuarter::from_number(quarter);
        let (return_period_from, return_period_to) = default_calendar_period(year, quarter)
            .map_or((None, None), |(from, to)| (Some(from), Some(to)));
        let zero = Some(0.0);
        let mut draft = Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            filing_basis: Form2550QFilingBasis::Calendar,
            year_end_month: 12,
            quarter,
            return_period_from,
            return_period_to,
            is_amended: false,
            is_short_period_return: false,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            taxpayer_classification: profile.eopt_tier.as_ref().map(Into::into),
            is_availing_tax_relief: false,
            tax_relief_details: String::new(),
            part_ii: Form2550QPartII {
                item_18_paid_on_previous_return: zero,
                item_19_other_credit_or_payment: zero,
                item_22_surcharge: zero,
                item_23_interest: zero,
                item_24_compromise: zero,
                ..Form2550QPartII::default()
            },
            part_iv: Form2550QPartIV {
                item_31a_vatable_sales: zero,
                item_32a_zero_rated_sales: zero,
                item_33a_exempt_sales: zero,
                item_35b_less_output_vat_uncollected: zero,
                item_36b_add_output_vat_recovered: zero,
                item_38b_input_tax_carried: zero,
                item_40b_transitional_input_tax: zero,
                item_41b_presumptive_input_tax: zero,
                item_42b_other_input_tax: zero,
                item_44a_domestic_purchases: zero,
                item_44b_domestic_input_tax: zero,
                item_45a_nonresident_services: zero,
                item_45b_nonresident_service_input_tax: zero,
                item_46a_importations: zero,
                item_46b_import_input_tax: zero,
                item_47a_other_purchases: zero,
                item_47b_other_input_tax: zero,
                item_48a_domestic_purchases_no_input_tax: zero,
                item_49a_vat_exempt_importations: zero,
                item_54b_vat_refund_or_tcc_claimed: zero,
                item_55b_input_vat_on_unpaid_payables: zero,
                item_56b_other_deduction: zero,
                item_58b_input_vat_on_settled_payables: zero,
                ..Form2550QPartIV::default()
            },
            schedule_1: vec![zero_capital_good_row(), zero_capital_good_row()],
            schedule_2: Form2550QSchedule2 {
                input_tax_directly_attributable_to_exempt_sales: zero,
                vat_exempt_sales: zero,
                input_tax_not_directly_attributable: zero,
                ..Form2550QSchedule2::default()
            },
            schedule_3: vec![zero_creditable_vat_row(), zero_creditable_vat_row()],
            schedule_4: vec![zero_advance_vat_row(), zero_advance_vat_row()],
            local_print_fields: Form2550QLocalPrintFields::default(),
            xml_final_flag: Form2550QXmlFinalFlag::One,
            xml_contact_email: String::new(),
            date_filed: None,
            preserved_unmodeled_xml_fields: BTreeMap::new(),
            migration_review_items: Vec::new(),
            legacy_flat_draft_fields: BTreeMap::new(),
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
        };
        draft.recompute();
        draft
    }

    /// Upgrade the old scaffold-only flat JSON shape into the reviewed
    /// semantic model. The migration is intentionally one-way and reports
    /// every legacy computed value that no longer agrees with the official
    /// formula chain.
    pub fn migrate_legacy_flat_draft(&mut self) -> bool {
        if !self.legacy_flat_draft_fields.contains_key("month") {
            return false;
        }

        let legacy = std::mem::take(&mut self.legacy_flat_draft_fields);
        self.migration_review_items.clear();
        self.filing_basis = if legacy_bool(&legacy, "fiscal_no1").unwrap_or(false) {
            Form2550QFilingBasis::Fiscal
        } else {
            Form2550QFilingBasis::Calendar
        };
        self.year_end_month = 12;
        self.quarter = Form2550QQuarter::from_number(legacy_u8(&legacy, "month").unwrap_or(0));
        let default_period = default_calendar_period(self.taxable_year, self.quarter);
        self.return_period_from = legacy_return_date(&legacy, "rtn_period_from_no4")
            .or_else(|| default_period.map(|(from, _)| from));
        self.return_period_to = legacy_return_date(&legacy, "rtn_period_to_no4")
            .or_else(|| default_period.map(|(_, to)| to));
        self.is_short_period_return = legacy_bool(&legacy, "opt_short_prd1").unwrap_or(false);
        self.taxpayer_classification = [
            (
                "tax_payer_classification1",
                Form2550QTaxpayerClassification::Micro,
            ),
            (
                "tax_payer_classification2",
                Form2550QTaxpayerClassification::Small,
            ),
            (
                "tax_payer_classification3",
                Form2550QTaxpayerClassification::Medium,
            ),
            (
                "tax_payer_classification4",
                Form2550QTaxpayerClassification::Large,
            ),
        ]
        .into_iter()
        .find_map(|(key, classification)| {
            legacy_bool(&legacy, key)
                .unwrap_or(false)
                .then_some(classification)
        });
        self.is_availing_tax_relief =
            legacy_bool(&legacy, "international_treaty_yn").unwrap_or(false);
        self.tax_relief_details = legacy_text(&legacy, "specify_international_treaty");

        self.part_ii = Form2550QPartII {
            item_18_paid_on_previous_return: legacy_money(&legacy, "vat_paid_return"),
            item_19_description: legacy_text(&legacy, "add_specify_no19"),
            item_19_other_credit_or_payment: legacy_money(&legacy, "other_credits_no19"),
            item_22_surcharge: legacy_money(&legacy, "surcharge"),
            item_23_interest: legacy_money(&legacy, "interest"),
            item_24_compromise: legacy_money(&legacy, "compromise"),
            ..Form2550QPartII::default()
        };
        self.part_iv = Form2550QPartIV {
            item_31a_vatable_sales: legacy_money(&legacy, "vatable_sales"),
            item_32a_zero_rated_sales: legacy_money(&legacy, "zero_rated_sales"),
            item_33a_exempt_sales: legacy_money(&legacy, "exempt_sales"),
            item_35b_less_output_vat_uncollected: legacy_money(&legacy, "less_output_vat"),
            item_36b_add_output_vat_recovered: legacy_money(&legacy, "add_output_vat"),
            item_38b_input_tax_carried: legacy_money(&legacy, "input_tax_carried"),
            item_40b_transitional_input_tax: legacy_money(&legacy, "transitional_input_tax"),
            item_41b_presumptive_input_tax: legacy_money(&legacy, "presumptive_input_tax"),
            item_42_description: legacy_text(&legacy, "add_specify_no42"),
            item_42b_other_input_tax: legacy_money(&legacy, "other_specify42"),
            item_44a_domestic_purchases: legacy_money(&legacy, "domestic_purchase"),
            item_44b_domestic_input_tax: legacy_money(&legacy, "domestic_input_tax"),
            item_45a_nonresident_services: legacy_money(&legacy, "services_purchase"),
            item_45b_nonresident_service_input_tax: legacy_money(&legacy, "service_input_tax"),
            item_46a_importations: legacy_money(&legacy, "import_purchase"),
            item_46b_import_input_tax: legacy_money(&legacy, "import_input_tax"),
            item_47_description: legacy_text(&legacy, "add_specify_no47"),
            item_47a_other_purchases: legacy_money(&legacy, "other_specify47"),
            item_47b_other_input_tax: legacy_money(&legacy, "other_specify47b"),
            item_48a_domestic_purchases_no_input_tax: legacy_money(
                &legacy,
                "domestic_purchase_no_tax",
            ),
            item_49a_vat_exempt_importations: legacy_money(&legacy, "vat_exempt_imports"),
            item_54b_vat_refund_or_tcc_claimed: legacy_money(&legacy, "vat_refund"),
            item_55b_input_vat_on_unpaid_payables: legacy_money(&legacy, "input_vat_unpaid"),
            item_56_description: legacy_text(&legacy, "add_specify_no56"),
            item_56b_other_deduction: legacy_money(&legacy, "other_specify56"),
            item_58b_input_vat_on_settled_payables: legacy_money(&legacy, "add_input_vat"),
            ..Form2550QPartIV::default()
        };
        self.schedule_1 = [10, 11]
            .into_iter()
            .map(|suffix| legacy_capital_good_row(&legacy, suffix))
            .collect();
        self.schedule_2 = Form2550QSchedule2 {
            input_tax_directly_attributable_to_exempt_sales: legacy_money(
                &legacy,
                "sched2input_tax_direct",
            ),
            vat_exempt_sales: legacy_money(&legacy, "sched2vat_exempt_sale"),
            input_tax_not_directly_attributable: legacy_money(&legacy, "sched2amount_input_tax"),
            ..Form2550QSchedule2::default()
        };
        self.schedule_3 = [30, 31]
            .into_iter()
            .map(|suffix| legacy_creditable_vat_row(&legacy, suffix))
            .collect();
        self.schedule_4 = [40, 41]
            .into_iter()
            .map(|suffix| legacy_advance_vat_row(&legacy, suffix))
            .collect();

        self.recompute();
        let comparisons = [
            ("output_vat_sales", self.part_iv.item_31b_output_tax),
            ("total_sales", self.part_iv.item_34a_total_sales),
            ("output_tax_due", self.part_iv.item_34b_output_tax_due),
            (
                "total_adj_output",
                self.part_iv.item_37b_adjusted_output_tax_due,
            ),
            (
                "input_tax_deferred",
                self.part_iv.item_39b_input_tax_deferred,
            ),
            ("total43", self.part_iv.item_43b_total_prior_input_tax),
            (
                "total_cur_purchase",
                self.part_iv.item_50a_total_current_purchases,
            ),
            (
                "total_cur_input_tax",
                self.part_iv.item_50b_total_current_input_tax,
            ),
            (
                "total_avail_input_tax",
                self.part_iv.item_51b_total_available_input_tax,
            ),
            (
                "import_capital_input_tax",
                self.part_iv.item_52b_deferred_capital_goods_input_tax,
            ),
            (
                "input_tax_attr",
                self.part_iv.item_53b_input_tax_attributable_to_exempt_sales,
            ),
            ("total_deductions", self.part_iv.item_57b_total_deductions),
            ("adj_deductions", self.part_iv.item_59b_adjusted_deductions),
            (
                "total_allow_input_tax",
                self.part_iv.item_60b_total_allowable_input_tax,
            ),
            (
                "net_vat_payable",
                self.part_iv.item_61b_net_vat_payable_or_excess,
            ),
            (
                "excess_input_tax",
                self.part_ii.item_15_net_vat_payable_or_excess,
            ),
            (
                "creditable_vat",
                self.part_ii.item_16_creditable_vat_withheld,
            ),
            ("adv_vat_payment", self.part_ii.item_17_advance_vat_payments),
            (
                "total_tax_credits",
                self.part_ii.item_20_total_credits_or_payments,
            ),
            (
                "excess_credits",
                self.part_ii.item_21_tax_payable_or_excess_credits,
            ),
            ("penalties", self.part_ii.item_25_total_penalties),
            (
                "total_payable",
                self.part_ii.item_26_total_amount_payable_or_excess,
            ),
        ];
        for (legacy_key, recomputed) in comparisons {
            if let (Some(old), Some(new)) = (legacy_money(&legacy, legacy_key), recomputed)
                && (old - new).abs() > 0.005
            {
                self.migration_review_items.push(format!(
                    "Legacy {legacy_key} was {old:.2}; the reviewed formula recomputes {new:.2}"
                ));
            }
        }
        if matches!(self.filing_basis, Form2550QFilingBasis::Fiscal) {
            self.migration_review_items.push(
                "The scaffold-era draft did not retain a reliable fiscal year-end month; review Item 2"
                    .to_string(),
            );
        }
        true
    }

    /// Recompute only formulas stated on the official April 2024 form or
    /// corroborated by the reviewed editable-save pair.
    pub fn recompute(&mut self) {
        if self.migrate_legacy_flat_draft() {
            return;
        }
        let p4 = &mut self.part_iv;
        p4.item_31b_output_tax = p4
            .item_31a_vatable_sales
            .map(|value| money(value * VAT_RATE));
        p4.item_34a_total_sales = sum_money(&[
            p4.item_31a_vatable_sales,
            p4.item_32a_zero_rated_sales,
            p4.item_33a_exempt_sales,
        ]);
        p4.item_34b_output_tax_due = p4.item_31b_output_tax;
        p4.item_37b_adjusted_output_tax_due = add_subtract_money(
            p4.item_34b_output_tax_due,
            p4.item_36b_add_output_vat_recovered,
            p4.item_35b_less_output_vat_uncollected,
        );

        let schedule_1_previous = sum_money(
            &self
                .schedule_1
                .iter()
                .map(|row| row.input_tax)
                .collect::<Vec<_>>(),
        );
        let schedule_1_next = sum_money(
            &self
                .schedule_1
                .iter()
                .map(|row| row.balance_to_next_period)
                .collect::<Vec<_>>(),
        );
        p4.item_39b_input_tax_deferred = schedule_1_previous;
        p4.item_43b_total_prior_input_tax = sum_money(&[
            p4.item_38b_input_tax_carried,
            p4.item_39b_input_tax_deferred,
            p4.item_40b_transitional_input_tax,
            p4.item_41b_presumptive_input_tax,
            p4.item_42b_other_input_tax,
        ]);

        p4.item_50a_total_current_purchases = sum_money(&[
            p4.item_44a_domestic_purchases,
            p4.item_45a_nonresident_services,
            p4.item_46a_importations,
            p4.item_47a_other_purchases,
            p4.item_48a_domestic_purchases_no_input_tax,
            p4.item_49a_vat_exempt_importations,
        ]);
        p4.item_50b_total_current_input_tax = sum_money(&[
            p4.item_44b_domestic_input_tax,
            p4.item_45b_nonresident_service_input_tax,
            p4.item_46b_import_input_tax,
            p4.item_47b_other_input_tax,
        ]);
        p4.item_51b_total_available_input_tax = sum_money(&[
            p4.item_43b_total_prior_input_tax,
            p4.item_50b_total_current_input_tax,
        ]);
        p4.item_52b_deferred_capital_goods_input_tax = schedule_1_next;

        self.schedule_2.total_sales = p4.item_34a_total_sales;
        self.schedule_2.ratable_input_tax = compute_ratable_input_tax(&self.schedule_2);
        self.schedule_2.total_input_tax_attributable_to_exempt_sales = sum_money(&[
            self.schedule_2
                .input_tax_directly_attributable_to_exempt_sales,
            self.schedule_2.ratable_input_tax,
        ]);
        p4.item_53b_input_tax_attributable_to_exempt_sales =
            self.schedule_2.total_input_tax_attributable_to_exempt_sales;

        p4.item_57b_total_deductions = sum_money(&[
            p4.item_52b_deferred_capital_goods_input_tax,
            p4.item_53b_input_tax_attributable_to_exempt_sales,
            p4.item_54b_vat_refund_or_tcc_claimed,
            p4.item_55b_input_vat_on_unpaid_payables,
            p4.item_56b_other_deduction,
        ]);
        p4.item_59b_adjusted_deductions = sum_money(&[
            p4.item_57b_total_deductions,
            p4.item_58b_input_vat_on_settled_payables,
        ]);
        p4.item_60b_total_allowable_input_tax = subtract_money(
            p4.item_51b_total_available_input_tax,
            p4.item_59b_adjusted_deductions,
        );
        p4.item_61b_net_vat_payable_or_excess = subtract_money(
            p4.item_37b_adjusted_output_tax_due,
            p4.item_60b_total_allowable_input_tax,
        );

        let schedule_3_tax = sum_money(
            &self
                .schedule_3
                .iter()
                .map(|row| row.tax_withheld)
                .collect::<Vec<_>>(),
        );
        let schedule_4_amount = sum_money(
            &self
                .schedule_4
                .iter()
                .map(|row| row.amount_paid)
                .collect::<Vec<_>>(),
        );

        let p2 = &mut self.part_ii;
        p2.item_15_net_vat_payable_or_excess = p4.item_61b_net_vat_payable_or_excess;
        p2.item_16_creditable_vat_withheld = schedule_3_tax;
        p2.item_17_advance_vat_payments = schedule_4_amount;
        p2.item_20_total_credits_or_payments = sum_money(&[
            p2.item_16_creditable_vat_withheld,
            p2.item_17_advance_vat_payments,
            p2.item_18_paid_on_previous_return,
            p2.item_19_other_credit_or_payment,
        ]);
        p2.item_21_tax_payable_or_excess_credits = subtract_money(
            p2.item_15_net_vat_payable_or_excess,
            p2.item_20_total_credits_or_payments,
        );
        p2.item_25_total_penalties = sum_money(&[
            p2.item_22_surcharge,
            p2.item_23_interest,
            p2.item_24_compromise,
        ]);
        // The form labels Item 26 as payable/(excess credits). The reviewed
        // save corroborates that negative excess credits do not offset cash
        // penalties: Item 21 = -2,440 and Item 25 = 1,300 produce Item 26 =
        // 1,300. Preserve the negative credit in Item 21 and add only a
        // positive payable component to penalties.
        p2.item_26_total_amount_payable_or_excess = match (
            p2.item_21_tax_payable_or_excess_credits,
            p2.item_25_total_penalties,
        ) {
            (Some(payable_or_credit), Some(penalties)) => {
                Some(money(payable_or_credit.max(0.0) + penalties))
            }
            _ => None,
        };

        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub fn quarter_number(&self) -> Option<u8> {
        self.quarter.number()
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        Err(vec![(
            "queue_submission".to_string(),
            "2550Qv2024 has reviewed editable-save evidence only; electronic queue/submission is not certified"
                .to_string(),
        )])
    }

    pub fn revert_to_draft(&mut self) -> Result<(), String> {
        if matches!(self.status, FilingStatus::Paid) {
            return Err("A paid 2550Q cannot be reverted to draft".to_string());
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

    pub(crate) fn schedule_1_previous_total(&self) -> Option<f64> {
        sum_money(
            &self
                .schedule_1
                .iter()
                .map(|row| row.input_tax)
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn schedule_1_next_total(&self) -> Option<f64> {
        sum_money(
            &self
                .schedule_1
                .iter()
                .map(|row| row.balance_to_next_period)
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn schedule_3_income_total(&self) -> Option<f64> {
        sum_money(
            &self
                .schedule_3
                .iter()
                .map(|row| row.income_payment)
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn schedule_3_tax_total(&self) -> Option<f64> {
        sum_money(
            &self
                .schedule_3
                .iter()
                .map(|row| row.tax_withheld)
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn schedule_4_amount_total(&self) -> Option<f64> {
        sum_money(
            &self
                .schedule_4
                .iter()
                .map(|row| row.amount_paid)
                .collect::<Vec<_>>(),
        )
    }
}

impl FormValidator for Form2550QDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        validate_identity(self, &mut errors);
        validate_period(self, &mut errors);

        if self.taxpayer_classification.is_none() {
            errors.push((
                "taxpayer_classification".to_string(),
                "Item 13 taxpayer classification is required".to_string(),
            ));
        }
        if self.is_availing_tax_relief && self.tax_relief_details.trim().is_empty() {
            errors.push((
                "tax_relief_details".to_string(),
                "Item 14A must identify the Special Law or International Tax Treaty".to_string(),
            ));
        }
        if self.xml_final_flag.requires_review() {
            errors.push((
                "xml_final_flag".to_string(),
                "Imported txtFinalFlag is outside the reviewed editable/encrypted values 0 and 1"
                    .to_string(),
            ));
        }
        for (index, message) in self.migration_review_items.iter().enumerate() {
            errors.push((format!("legacy_migration[{index}]"), message.clone()));
        }

        for (field, value) in user_money_fields(self) {
            validate_required_non_negative_money(field, value, &mut errors);
        }
        for (field, description, value) in [
            (
                "item_19_description",
                self.part_ii.item_19_description.as_str(),
                self.part_ii.item_19_other_credit_or_payment,
            ),
            (
                "item_42_description",
                self.part_iv.item_42_description.as_str(),
                self.part_iv.item_42b_other_input_tax,
            ),
            (
                "item_56_description",
                self.part_iv.item_56_description.as_str(),
                self.part_iv.item_56b_other_deduction,
            ),
        ] {
            if value.is_some_and(|amount| amount != 0.0) && description.trim().is_empty() {
                errors.push((
                    field.to_string(),
                    "An Others amount requires a description".to_string(),
                ));
            }
        }
        if (self
            .part_iv
            .item_47a_other_purchases
            .is_some_and(|amount| amount != 0.0)
            || self
                .part_iv
                .item_47b_other_input_tax
                .is_some_and(|amount| amount != 0.0))
            && self.part_iv.item_47_description.trim().is_empty()
        {
            errors.push((
                "item_47_description".to_string(),
                "An Item 47A or 47B amount requires an Others description".to_string(),
            ));
        }
        if !self.is_amended
            && self
                .part_ii
                .item_18_paid_on_previous_return
                .is_some_and(|value| value != 0.0)
        {
            errors.push((
                "item_18_paid_on_previous_return".to_string(),
                "Item 18 applies only to an amended return".to_string(),
            ));
        }

        validate_schedule_rows(self, &mut errors);
        validate_local_print_fields(&self.local_print_fields, &mut errors);
        if self.schedule_2.total_sales == Some(0.0)
            && self
                .schedule_2
                .vat_exempt_sales
                .is_some_and(|value| value != 0.0)
            && self
                .schedule_2
                .input_tax_not_directly_attributable
                .is_some_and(|value| value != 0.0)
        {
            errors.push((
                "schedule_2.total_sales".to_string(),
                "Schedule 2 cannot allocate ratable input tax when total sales is zero".to_string(),
            ));
        }

        for (field, value) in computed_money_fields(self) {
            if value.is_none() {
                errors.push((
                    field.to_string(),
                    "Computed amount is unresolved because an input is blank or invalid"
                        .to_string(),
                ));
            } else if value.is_some_and(|amount| !amount.is_finite()) {
                errors.push((
                    field.to_string(),
                    "Computed amount is not finite".to_string(),
                ));
            }
        }
        errors
    }
}

impl TypedBirForm for Form2550QDraft {
    fn form_code(&self) -> &'static str {
        FORM_CODE
    }

    fn form_type_id(&self) -> &'static str {
        FORM_TYPE_ID
    }

    fn filing_period(&self) -> FilingPeriod {
        FilingPeriod::Quarterly(self.quarter.number().unwrap_or(0))
    }

    fn recompute(&mut self) {
        Form2550QDraft::recompute(self);
    }

    fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        Form2550QDraft::to_bir_field_map(self)
    }
}

fn zero_capital_good_row() -> Form2550QCapitalGoodRow {
    Form2550QCapitalGoodRow {
        purchase_or_import_amount: Some(0.0),
        input_tax: Some(0.0),
        estimated_life_months: Some(0),
        recognized_life_months: Some(0),
        allowable_input_tax_for_period: Some(0.0),
        balance_to_next_period: Some(0.0),
        ..Form2550QCapitalGoodRow::default()
    }
}

fn zero_creditable_vat_row() -> Form2550QCreditableVatRow {
    Form2550QCreditableVatRow {
        income_payment: Some(0.0),
        tax_withheld: Some(0.0),
        ..Form2550QCreditableVatRow::default()
    }
}

fn zero_advance_vat_row() -> Form2550QAdvanceVatRow {
    Form2550QAdvanceVatRow {
        official_receipt_number: "0.00".to_string(),
        amount_paid: Some(0.0),
        ..Form2550QAdvanceVatRow::default()
    }
}

fn default_calendar_period(
    taxable_year: u16,
    quarter: Form2550QQuarter,
) -> Option<(Form2550QDate, Form2550QDate)> {
    let quarter = quarter.number()?;
    let start_month = quarter.checked_sub(1)?.checked_mul(3)?.checked_add(1)?;
    let end_month = start_month.checked_add(2)?;
    let next_month = if end_month == 12 { 1 } else { end_month + 1 };
    let next_year = if end_month == 12 {
        taxable_year.checked_add(1)?
    } else {
        taxable_year
    };
    let next_start =
        chrono::NaiveDate::from_ymd_opt(i32::from(next_year), u32::from(next_month), 1)?;
    let end = next_start.pred_opt()?;
    Some((
        Form2550QDate::new(taxable_year, start_month, 1).ok()?,
        Form2550QDate::from_naive(end).ok()?,
    ))
}

fn legacy_bool(fields: &BTreeMap<String, serde_json::Value>, key: &str) -> Option<bool> {
    fields.get(key)?.as_bool()
}

fn legacy_money(fields: &BTreeMap<String, serde_json::Value>, key: &str) -> Option<f64> {
    let value = fields.get(key)?;
    let parsed = value
        .as_f64()
        .or_else(|| value.as_str()?.trim().replace(',', "").parse::<f64>().ok())?;
    parsed.is_finite().then_some(parsed)
}

fn legacy_text(fields: &BTreeMap<String, serde_json::Value>, key: &str) -> String {
    fields
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn legacy_u8(fields: &BTreeMap<String, serde_json::Value>, key: &str) -> Option<u8> {
    let value = fields.get(key)?;
    value
        .as_u64()
        .and_then(|number| u8::try_from(number).ok())
        .or_else(|| value.as_str()?.trim().parse::<u8>().ok())
}

fn legacy_u16(fields: &BTreeMap<String, serde_json::Value>, key: &str) -> Option<u16> {
    let value = legacy_money(fields, key)?;
    if value < 0.0 || value.fract() != 0.0 {
        return None;
    }
    u16::try_from(value as u64).ok()
}

fn legacy_return_date(
    fields: &BTreeMap<String, serde_json::Value>,
    key: &str,
) -> Option<Form2550QDate> {
    let value = legacy_text(fields, key);
    if value.trim().is_empty() {
        None
    } else {
        Form2550QDate::parse_return_period(&value).ok()
    }
}

fn legacy_identifier(fields: &BTreeMap<String, serde_json::Value>, key: &str) -> String {
    let Some(value) = fields.get(key) else {
        return String::new();
    };
    if let Some(text) = value.as_str() {
        text.to_string()
    } else if let Some(number) = value.as_f64().filter(|number| number.is_finite()) {
        format!("{number:.2}")
    } else {
        String::new()
    }
}

fn legacy_capital_good_row(
    fields: &BTreeMap<String, serde_json::Value>,
    suffix: u8,
) -> Form2550QCapitalGoodRow {
    Form2550QCapitalGoodRow {
        purchase_or_import_date: legacy_return_date(fields, &format!("txt_date_purchase{suffix}")),
        source_code: legacy_text(fields, &format!("txt_source_code{suffix}")),
        description: legacy_text(fields, &format!("txt_description{suffix}")),
        purchase_or_import_amount: legacy_money(fields, &format!("txt_amount_purchase{suffix}")),
        input_tax: legacy_money(fields, &format!("txt_input_tax{suffix}")),
        estimated_life_months: legacy_u16(fields, &format!("txt_estimated_life{suffix}")),
        recognized_life_months: legacy_u16(fields, &format!("txt_recognized_life{suffix}")),
        allowable_input_tax_for_period: legacy_money(
            fields,
            &format!("txt_allowed_input_tax{suffix}"),
        ),
        balance_to_next_period: legacy_money(fields, &format!("txt_balance_input_tax{suffix}")),
    }
}

fn legacy_creditable_vat_row(
    fields: &BTreeMap<String, serde_json::Value>,
    suffix: u8,
) -> Form2550QCreditableVatRow {
    let row_index = suffix.saturating_sub(30);
    Form2550QCreditableVatRow {
        period_from: legacy_return_date(fields, &format!("txt_date_covered{suffix}")),
        period_to: legacy_return_date(fields, &format!("txt_date_covered3to{row_index}")),
        withholding_agent_name: legacy_text(
            fields,
            &format!("txt_name_with_holding_agent{suffix}"),
        ),
        income_payment: legacy_money(fields, &format!("txt_income_payment{suffix}")),
        tax_withheld: legacy_money(fields, &format!("txt_total_tax_with_held{suffix}")),
    }
}

fn legacy_advance_vat_row(
    fields: &BTreeMap<String, serde_json::Value>,
    suffix: u8,
) -> Form2550QAdvanceVatRow {
    let row_index = suffix.saturating_sub(40);
    Form2550QAdvanceVatRow {
        period_from: legacy_return_date(fields, &format!("txt_date{suffix}")),
        period_to: legacy_return_date(fields, &format!("txt_date4to{row_index}")),
        miller_name: legacy_text(fields, &format!("txt_name_of_miller{suffix}")),
        taxpayer_name: legacy_text(fields, &format!("txt_name_of_taxpayer{suffix}")),
        official_receipt_number: legacy_identifier(
            fields,
            &format!("txt_official_receipt_number{suffix}"),
        ),
        amount_paid: legacy_money(fields, &format!("txt_amount_paid{suffix}")),
    }
}

fn money(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn sum_money(values: &[Option<f64>]) -> Option<f64> {
    values
        .iter()
        .try_fold(0.0, |total, value| value.map(|amount| total + amount))
        .map(money)
}

fn subtract_money(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    Some(money(left? - right?))
}

fn add_subtract_money(
    base: Option<f64>,
    addition: Option<f64>,
    deduction: Option<f64>,
) -> Option<f64> {
    Some(money(base? + addition? - deduction?))
}

fn compute_ratable_input_tax(schedule: &Form2550QSchedule2) -> Option<f64> {
    let exempt_sales = schedule.vat_exempt_sales?;
    let total_sales = schedule.total_sales?;
    let indirect_input_tax = schedule.input_tax_not_directly_attributable?;
    if total_sales == 0.0 {
        if exempt_sales == 0.0 || indirect_input_tax == 0.0 {
            Some(0.0)
        } else {
            None
        }
    } else {
        Some(money(exempt_sales / total_sales * indirect_input_tax))
    }
}

fn validate_identity(draft: &Form2550QDraft, errors: &mut Vec<(String, String)>) {
    let tin_digits: String = draft
        .tin
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    if !matches!(tin_digits.len(), 12..=14)
        || draft
            .tin
            .chars()
            .any(|character| !character.is_ascii_digit() && character != '-')
    {
        errors.push((
            "tin".to_string(),
            "TIN must contain 12 to 14 digits, with optional dashes".to_string(),
        ));
    }
    for (field, label, value) in [
        ("rdo_code", "RDO code", draft.rdo_code.as_str()),
        (
            "taxpayer_name",
            "Taxpayer name",
            draft.taxpayer_name.as_str(),
        ),
        (
            "registered_address",
            "Registered address",
            draft.registered_address.as_str(),
        ),
        ("zip_code", "ZIP code", draft.zip_code.as_str()),
        (
            "contact_number",
            "Contact number",
            draft.contact_number.as_str(),
        ),
        ("email", "Email address", draft.email.as_str()),
    ] {
        if value.trim().is_empty() {
            errors.push((field.to_string(), format!("{label} is required")));
        }
    }
    if !draft.rdo_code.trim().is_empty()
        && (draft.rdo_code.len() != 3
            || !draft
                .rdo_code
                .chars()
                .all(|character| character.is_ascii_digit()))
    {
        errors.push((
            "rdo_code".to_string(),
            "RDO code must be 3 digits".to_string(),
        ));
    }
    if !draft.zip_code.trim().is_empty() && !validate_zip(draft.zip_code.trim()) {
        errors.push((
            "zip_code".to_string(),
            "ZIP code must be 4 digits".to_string(),
        ));
    }
    if !draft.contact_number.trim().is_empty() && !validate_ph_phone(&draft.contact_number) {
        errors.push((
            "contact_number".to_string(),
            "Contact number must be a valid Philippine mobile or landline number".to_string(),
        ));
    }
    if !draft.email.trim().is_empty() && !validate_email(&draft.email) {
        errors.push(("email".to_string(), "Email address is invalid".to_string()));
    }
}

fn validate_period(draft: &Form2550QDraft, errors: &mut Vec<(String, String)>) {
    if !(1900..=9999).contains(&draft.taxable_year) {
        errors.push((
            "taxable_year".to_string(),
            "Taxable year must contain four digits".to_string(),
        ));
    }
    if !(1..=12).contains(&draft.year_end_month) {
        errors.push((
            "year_end_month".to_string(),
            "Year-end month must be from 1 to 12".to_string(),
        ));
    }
    if draft.quarter.number().is_none() {
        errors.push((
            "quarter".to_string(),
            "Quarter must be from 1 to 4".to_string(),
        ));
    }
    match (draft.return_period_from, draft.return_period_to) {
        (Some(from), Some(to)) => {
            if from.validate().is_err() || to.validate().is_err() {
                errors.push((
                    "return_period".to_string(),
                    "Return period dates must be real calendar dates".to_string(),
                ));
            } else if from.as_naive() > to.as_naive() {
                errors.push((
                    "return_period".to_string(),
                    "Return period From date cannot be after the To date".to_string(),
                ));
            }
        }
        _ => errors.push((
            "return_period".to_string(),
            "Both return period dates are required".to_string(),
        )),
    }
}

fn validate_required_non_negative_money(
    field: &str,
    value: Option<f64>,
    errors: &mut Vec<(String, String)>,
) {
    match value {
        None => errors.push((
            field.to_string(),
            "Amount is blank; enter 0.00 explicitly when there is no amount".to_string(),
        )),
        Some(amount) if !amount.is_finite() || amount < 0.0 => errors.push((
            field.to_string(),
            "Amount must be a finite, non-negative number".to_string(),
        )),
        Some(_) => {}
    }
}

fn validate_schedule_rows(draft: &Form2550QDraft, errors: &mut Vec<(String, String)>) {
    if draft.schedule_1.len() != FIXED_SCHEDULE_ROW_COUNT
        || draft.schedule_3.len() != FIXED_SCHEDULE_ROW_COUNT
        || draft.schedule_4.len() != FIXED_SCHEDULE_ROW_COUNT
    {
        errors.push((
            "schedules".to_string(),
            "The reviewed 2550Q editable-save contract contains exactly two rows in each schedule"
                .to_string(),
        ));
    }
    for (index, row) in draft.schedule_1.iter().enumerate() {
        if row.is_empty() {
            continue;
        }
        let field = format!("schedule_1[{index}]");
        if row.purchase_or_import_date.is_none()
            || row.source_code.trim().is_empty()
            || row.description.trim().is_empty()
            || row.estimated_life_months.is_none()
            || row.recognized_life_months.is_none()
        {
            errors.push((
                field.clone(),
                "A used Schedule 1 row requires date, source code, description, and useful-life fields"
                    .to_string(),
            ));
        }
        if !row.source_code.trim().is_empty()
            && !matches!(
                row.source_code.trim().to_ascii_uppercase().as_str(),
                "D" | "I"
            )
        {
            errors.push((
                format!("{field}.source_code"),
                "Schedule 1 source code must be D (domestic) or I (importation)".to_string(),
            ));
        }
        for (suffix, value) in [
            ("purchase_or_import_amount", row.purchase_or_import_amount),
            ("input_tax", row.input_tax),
            (
                "allowable_input_tax_for_period",
                row.allowable_input_tax_for_period,
            ),
            ("balance_to_next_period", row.balance_to_next_period),
        ] {
            validate_required_non_negative_money(&format!("{field}.{suffix}"), value, errors);
        }
    }
    for (index, row) in draft.schedule_3.iter().enumerate() {
        if row.is_empty() {
            continue;
        }
        let field = format!("schedule_3[{index}]");
        if row.period_from.is_none()
            || row.period_to.is_none()
            || row.withholding_agent_name.trim().is_empty()
        {
            errors.push((
                field.clone(),
                "A used Schedule 3 row requires period and withholding-agent name".to_string(),
            ));
        }
        if matches!(
            (row.period_from, row.period_to),
            (Some(from), Some(to)) if from.as_naive() > to.as_naive()
        ) {
            errors.push((
                format!("{field}.period"),
                "Schedule 3 period-from date must not be after period-to date".to_string(),
            ));
        }
        validate_required_non_negative_money(
            &format!("{field}.income_payment"),
            row.income_payment,
            errors,
        );
        validate_required_non_negative_money(
            &format!("{field}.tax_withheld"),
            row.tax_withheld,
            errors,
        );
    }
    for (index, row) in draft.schedule_4.iter().enumerate() {
        if row.is_empty() {
            continue;
        }
        let field = format!("schedule_4[{index}]");
        if row.period_from.is_none()
            || row.period_to.is_none()
            || row.miller_name.trim().is_empty()
            || row.taxpayer_name.trim().is_empty()
            || matches!(row.official_receipt_number.trim(), "" | "0.00")
        {
            errors.push((
                field.clone(),
                "A used Schedule 4 row requires period, miller, taxpayer, and receipt number"
                    .to_string(),
            ));
        }
        if matches!(
            (row.period_from, row.period_to),
            (Some(from), Some(to)) if from.as_naive() > to.as_naive()
        ) {
            errors.push((
                format!("{field}.period"),
                "Schedule 4 period-from date must not be after period-to date".to_string(),
            ));
        }
        validate_required_non_negative_money(
            &format!("{field}.amount_paid"),
            row.amount_paid,
            errors,
        );
    }
}

fn validate_local_print_fields(
    fields: &Form2550QLocalPrintFields,
    errors: &mut Vec<(String, String)>,
) {
    for (field, value) in [
        (
            "tax_agent_date_of_issue",
            fields.tax_agent_date_of_issue.as_str(),
        ),
        (
            "tax_agent_date_of_expiry",
            fields.tax_agent_date_of_expiry.as_str(),
        ),
        ("check_date", fields.check_date.as_str()),
        ("tax_debit_memo_date", fields.tax_debit_memo_date.as_str()),
        ("other_payment_date", fields.other_payment_date.as_str()),
    ] {
        if !value.trim().is_empty()
            && chrono::NaiveDate::parse_from_str(value.trim(), "%m/%d/%Y").is_err()
        {
            errors.push((
                format!("local_print_fields.{field}"),
                "Date must use MM/DD/YYYY".to_string(),
            ));
        }
    }
    for (field, value) in [
        (
            "cash_or_bank_debit_advice_amount",
            fields.cash_or_bank_debit_advice_amount,
        ),
        ("check_amount", fields.check_amount),
        ("tax_debit_memo_amount", fields.tax_debit_memo_amount),
        ("other_payment_amount", fields.other_payment_amount),
    ] {
        if value.is_some_and(|amount| !amount.is_finite() || amount < 0.0) {
            errors.push((
                format!("local_print_fields.{field}"),
                "Payment amount must be finite and non-negative".to_string(),
            ));
        }
    }
}

fn user_money_fields(draft: &Form2550QDraft) -> Vec<(&'static str, Option<f64>)> {
    let p2 = &draft.part_ii;
    let p4 = &draft.part_iv;
    vec![
        (
            "item_18_paid_on_previous_return",
            p2.item_18_paid_on_previous_return,
        ),
        (
            "item_19_other_credit_or_payment",
            p2.item_19_other_credit_or_payment,
        ),
        ("item_22_surcharge", p2.item_22_surcharge),
        ("item_23_interest", p2.item_23_interest),
        ("item_24_compromise", p2.item_24_compromise),
        ("item_31a_vatable_sales", p4.item_31a_vatable_sales),
        ("item_32a_zero_rated_sales", p4.item_32a_zero_rated_sales),
        ("item_33a_exempt_sales", p4.item_33a_exempt_sales),
        (
            "item_35b_less_output_vat_uncollected",
            p4.item_35b_less_output_vat_uncollected,
        ),
        (
            "item_36b_add_output_vat_recovered",
            p4.item_36b_add_output_vat_recovered,
        ),
        ("item_38b_input_tax_carried", p4.item_38b_input_tax_carried),
        (
            "item_40b_transitional_input_tax",
            p4.item_40b_transitional_input_tax,
        ),
        (
            "item_41b_presumptive_input_tax",
            p4.item_41b_presumptive_input_tax,
        ),
        ("item_42b_other_input_tax", p4.item_42b_other_input_tax),
        (
            "item_44a_domestic_purchases",
            p4.item_44a_domestic_purchases,
        ),
        (
            "item_44b_domestic_input_tax",
            p4.item_44b_domestic_input_tax,
        ),
        (
            "item_45a_nonresident_services",
            p4.item_45a_nonresident_services,
        ),
        (
            "item_45b_nonresident_service_input_tax",
            p4.item_45b_nonresident_service_input_tax,
        ),
        ("item_46a_importations", p4.item_46a_importations),
        ("item_46b_import_input_tax", p4.item_46b_import_input_tax),
        ("item_47a_other_purchases", p4.item_47a_other_purchases),
        ("item_47b_other_input_tax", p4.item_47b_other_input_tax),
        (
            "item_48a_domestic_purchases_no_input_tax",
            p4.item_48a_domestic_purchases_no_input_tax,
        ),
        (
            "item_49a_vat_exempt_importations",
            p4.item_49a_vat_exempt_importations,
        ),
        (
            "item_54b_vat_refund_or_tcc_claimed",
            p4.item_54b_vat_refund_or_tcc_claimed,
        ),
        (
            "item_55b_input_vat_on_unpaid_payables",
            p4.item_55b_input_vat_on_unpaid_payables,
        ),
        ("item_56b_other_deduction", p4.item_56b_other_deduction),
        (
            "item_58b_input_vat_on_settled_payables",
            p4.item_58b_input_vat_on_settled_payables,
        ),
        (
            "schedule_2.input_tax_directly_attributable",
            draft
                .schedule_2
                .input_tax_directly_attributable_to_exempt_sales,
        ),
        (
            "schedule_2.vat_exempt_sales",
            draft.schedule_2.vat_exempt_sales,
        ),
        (
            "schedule_2.input_tax_not_directly_attributable",
            draft.schedule_2.input_tax_not_directly_attributable,
        ),
    ]
}

fn computed_money_fields(draft: &Form2550QDraft) -> Vec<(&'static str, Option<f64>)> {
    let p2 = &draft.part_ii;
    let p4 = &draft.part_iv;
    vec![
        ("item_15", p2.item_15_net_vat_payable_or_excess),
        ("item_16", p2.item_16_creditable_vat_withheld),
        ("item_17", p2.item_17_advance_vat_payments),
        ("item_20", p2.item_20_total_credits_or_payments),
        ("item_21", p2.item_21_tax_payable_or_excess_credits),
        ("item_25", p2.item_25_total_penalties),
        ("item_26", p2.item_26_total_amount_payable_or_excess),
        ("item_31b", p4.item_31b_output_tax),
        ("item_34a", p4.item_34a_total_sales),
        ("item_34b", p4.item_34b_output_tax_due),
        ("item_37b", p4.item_37b_adjusted_output_tax_due),
        ("item_39b", p4.item_39b_input_tax_deferred),
        ("item_43b", p4.item_43b_total_prior_input_tax),
        ("item_50a", p4.item_50a_total_current_purchases),
        ("item_50b", p4.item_50b_total_current_input_tax),
        ("item_51b", p4.item_51b_total_available_input_tax),
        ("item_52b", p4.item_52b_deferred_capital_goods_input_tax),
        (
            "item_53b",
            p4.item_53b_input_tax_attributable_to_exempt_sales,
        ),
        ("item_57b", p4.item_57b_total_deductions),
        ("item_59b", p4.item_59b_adjusted_deductions),
        ("item_60b", p4.item_60b_total_allowable_input_tax),
        ("item_61b", p4.item_61b_net_vat_payable_or_excess),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> TaxpayerProfile {
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
            "default_form_type": "2550Qv2024",
            "taxpayer_type": "Individual",
            "tax_classification": "SelfEmployed",
            "is_vat_registered": true,
            "eopt_tier": "Medium",
            "withholding_obligations": [],
            "excise_tax_liabilities": [],
            "atc_codes": [],
            "business_start_date": null,
            "birth_date": null,
            "registration_activity_status": "Active",
            "is_dormant_entity": false,
            "is_government_withholding_entity": false,
            "is_gpp_partner": false,
            "is_top_withholding_agent": false,
            "income_tax_elections": { "elections": {} },
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }))
        .expect("valid test profile")
    }

    #[test]
    fn new_draft_uses_quarter_not_year_end_month() {
        let draft = Form2550QDraft::new_from_profile(&profile(), 2026, 2);
        assert_eq!(draft.quarter, Form2550QQuarter::Second);
        assert_eq!(draft.year_end_month, 12);
        assert_eq!(
            draft.return_period_from.expect("from").to_string(),
            "4/01/2026"
        );
        assert_eq!(draft.return_period_to.expect("to").to_string(), "6/30/2026");
    }

    #[test]
    fn scaffold_era_flat_json_is_migrated_without_losing_user_amounts() {
        let json = serde_json::json!({
            "id": null,
            "tin": "00000000000000",
            "taxable_year": 2026,
            "month": 2,
            "is_amended": false,
            "rdo_code": "018",
            "taxpayer_name": "JUAN DELA CRUZ",
            "registered_address": "OLONGAPO",
            "zip_code": "2200",
            "contact_number": "09123456789",
            "email": "codeitlikemiley@gmail.com",
            "date_filed": "",
            "vatable_sales": 1000.0,
            "zero_rated_sales": 200.0,
            "exempt_sales": 300.0,
            "less_output_vat": 0.0,
            "add_output_vat": 0.0,
            "tax_payer_classification1": true,
            "status": "Draft",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        let mut draft: Form2550QDraft =
            serde_json::from_value(json).expect("legacy JSON must deserialize");

        assert!(draft.migrate_legacy_flat_draft());
        assert_eq!(draft.quarter, Form2550QQuarter::Second);
        assert_eq!(draft.part_iv.item_31a_vatable_sales, Some(1_000.0));
        assert_eq!(draft.part_iv.item_31b_output_tax, Some(120.0));
        assert_eq!(
            draft.taxpayer_classification,
            Some(Form2550QTaxpayerClassification::Micro)
        );
        assert!(draft.legacy_flat_draft_fields.is_empty());
        assert!(
            !serde_json::to_value(&draft)
                .expect("current JSON serializes")
                .as_object()
                .expect("draft is an object")
                .contains_key("month")
        );
    }

    #[test]
    fn current_json_round_trips_typed_filed_date() {
        let mut draft = Form2550QDraft::new_from_profile(&profile(), 2026, 1);
        draft.date_filed = Some(Form2550QDate::new(2026, 4, 25).expect("valid date"));
        let json = serde_json::to_value(&draft).expect("current JSON serializes");
        let restored: Form2550QDraft =
            serde_json::from_value(json).expect("current JSON deserializes");

        assert_eq!(restored.date_filed, draft.date_filed);
        assert!(restored.legacy_flat_draft_fields.is_empty());
    }

    #[test]
    fn reviewed_sample_formula_chain_is_reproduced_without_guessed_penalties() {
        let mut draft = Form2550QDraft::new_from_profile(&profile(), 2025, 1);
        let p4 = &mut draft.part_iv;
        p4.item_31a_vatable_sales = Some(1_000.0);
        p4.item_32a_zero_rated_sales = Some(1_000.0);
        p4.item_33a_exempt_sales = Some(1_000.0);
        p4.item_35b_less_output_vat_uncollected = Some(1_000.0);
        p4.item_36b_add_output_vat_recovered = Some(1_000.0);
        p4.item_38b_input_tax_carried = Some(1_000.0);
        p4.item_40b_transitional_input_tax = Some(1_000.0);
        p4.item_41b_presumptive_input_tax = Some(1_000.0);
        p4.item_42b_other_input_tax = Some(10_000.0);
        p4.item_44a_domestic_purchases = Some(1_000.0);
        p4.item_44b_domestic_input_tax = Some(120.0);
        p4.item_45a_nonresident_services = Some(1_000.0);
        p4.item_45b_nonresident_service_input_tax = Some(120.0);
        p4.item_46a_importations = Some(10_000.0);
        p4.item_46b_import_input_tax = Some(1_200.0);
        p4.item_47a_other_purchases = Some(1_000.0);
        p4.item_47b_other_input_tax = Some(120.0);
        p4.item_48a_domestic_purchases_no_input_tax = Some(1_000.0);
        p4.item_49a_vat_exempt_importations = Some(1_000.0);
        p4.item_54b_vat_refund_or_tcc_claimed = Some(1_000.0);
        p4.item_55b_input_vat_on_unpaid_payables = Some(1_000.0);
        p4.item_56b_other_deduction = Some(1_000.0);
        p4.item_58b_input_vat_on_settled_payables = Some(10_000.0);
        draft.part_ii.item_19_other_credit_or_payment = Some(1_000.0);
        draft.part_ii.item_22_surcharge = Some(1_000.0);
        draft.part_ii.item_23_interest = Some(100.0);
        draft.part_ii.item_24_compromise = Some(200.0);
        draft.recompute();

        assert_eq!(
            draft.part_iv.item_50a_total_current_purchases,
            Some(15_000.0)
        );
        assert_eq!(
            draft.part_iv.item_61b_net_vat_payable_or_excess,
            Some(-1_440.0)
        );
        assert_eq!(
            draft.part_ii.item_21_tax_payable_or_excess_credits,
            Some(-2_440.0)
        );
        assert_eq!(
            draft.part_ii.item_26_total_amount_payable_or_excess,
            Some(1_300.0)
        );
    }

    #[test]
    fn invalid_or_blank_money_is_not_coerced_to_zero() {
        let mut draft = Form2550QDraft::new_from_profile(&profile(), 2026, 1);
        draft.part_iv.item_31a_vatable_sales = None;
        draft.recompute();
        let errors = draft.validate();
        assert!(
            errors
                .iter()
                .any(|(field, _)| field == "item_31a_vatable_sales")
        );
        assert_eq!(draft.part_iv.item_31b_output_tax, None);
    }

    #[test]
    fn item_47_description_is_required_for_either_column() {
        let mut draft = Form2550QDraft::new_from_profile(&profile(), 2026, 1);
        draft.part_iv.item_47a_other_purchases = Some(0.0);
        draft.part_iv.item_47b_other_input_tax = Some(10.0);
        draft.part_iv.item_47_description.clear();
        draft.recompute();

        assert!(
            draft
                .validate()
                .iter()
                .any(|(field, _)| field == "item_47_description")
        );
    }

    #[test]
    fn schedule_4_placeholder_is_not_accepted_as_a_receipt_number() {
        let mut draft = Form2550QDraft::new_from_profile(&profile(), 2026, 1);
        draft.schedule_4[0].period_from = Some(Form2550QDate::new(2026, 1, 1).expect("valid date"));
        draft.schedule_4[0].period_to = Some(Form2550QDate::new(2026, 3, 31).expect("valid date"));
        draft.schedule_4[0].miller_name = "MILLER".to_string();
        draft.schedule_4[0].taxpayer_name = "TAXPAYER".to_string();
        draft.schedule_4[0].amount_paid = Some(10.0);
        draft.recompute();

        assert!(
            draft
                .validate()
                .iter()
                .any(|(field, _)| field == "schedule_4[0]")
        );
    }

    #[test]
    fn queue_transition_always_fails_closed() {
        let mut draft = Form2550QDraft::new_from_profile(&profile(), 2026, 1);
        let result = draft.transition_to_queued();
        assert!(result.is_err());
        assert_eq!(draft.status, FilingStatus::Draft);
    }
}
