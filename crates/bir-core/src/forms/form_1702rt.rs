//! BIR Form 1702-RT, January 2018 (ENCS), exact revision `1702RTv2018C`.
//!
//! The semantic model is bounded by the locked four-page official form and the
//! reviewed 258-field plain/encrypted save pair. Amounts are whole pesos: the
//! official form explicitly says to drop 49 centavos or less and round up 50
//! centavos or more. XML persistence is supported, but electronic submission
//! remains disabled until an independently reviewed submission contract exists.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::queueable::SubmissionLifecycle;
use super::{FilingPeriod, FormValidator, TypedBirForm};
use crate::profile::TaxpayerProfile;

pub const FORM_CODE: &str = "1702RT";
pub const FORM_REVISION: &str = "2018C";
pub const FORM_TYPE_ID: &str = "1702RTv2018C";
pub const FORM_VERSION_LABEL: &str = "January 2018 (ENCS)";
pub const OFFICIAL_PAGE_COUNT: usize = 4;
pub const OFFICIAL_PAGE_WIDTH_POINTS: u16 = 612;
pub const OFFICIAL_PAGE_HEIGHT_POINTS: u16 = 936;
pub const XML_ROUND_TRIP_SUPPORTED: bool = true;
pub const QUEUE_SUBMISSION_SUPPORTED: bool = false;
pub const OFFICIAL_FORM_SHA256: &str =
    "d9a6a8a13e0114934261151c4eb269a1573042e7ce670eaf12b15f169d308d2d";
pub const REVIEWED_EDITABLE_XML_SHA256: &str =
    "a5316d974ffca1db2359d92208fd4f6b15533e5330fcfc73922becd6b2c29299";
pub const REVIEWED_ENCRYPTED_XML_SHA256: &str =
    "e45db05bb89c2513054e7f075e41a09e9ec35c9590982619dcfb1dfb57602501";

/// Alternate Item 5 ATC evidence reviewed from the companion editable save and
/// the captured January 2018 application UI. Other dropdown entries remain
/// unsupported until their exact code/description pair is independently
/// reviewed; printing an unreviewed description would be an unsafe inference.
pub const REVIEWED_ALTERNATE_ATC_IC010_DESCRIPTION: &str =
    "CORPORATION IN GENERAL - JAN 1, 2009 (2009)";

pub fn reviewed_alternate_atc_description(code: &str) -> Option<&'static str> {
    match code.trim() {
        "IC010" => Some(REVIEWED_ALTERNATE_ATC_IC010_DESCRIPTION),
        _ => None,
    }
}

/// A signed, whole-peso amount. This deliberately cannot represent centavos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WholePeso(pub i64);

impl WholePeso {
    pub const ZERO: Self = Self(0);

    pub fn parse_bir(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() {
            return Err("whole-peso amount is blank".to_string());
        }
        let (sign, digits) = match value.as_bytes().first() {
            Some(b'-') => (-1_i64, &value[1..]),
            Some(b'+') => (1_i64, &value[1..]),
            _ => (1_i64, value),
        };
        if digits.is_empty() || digits.contains('.') {
            return Err("amount must contain whole pesos only".to_string());
        }
        let groups = digits.split(',').collect::<Vec<_>>();
        let grouping_is_valid = if groups.len() == 1 {
            !groups[0].is_empty() && groups[0].chars().all(|c| c.is_ascii_digit())
        } else {
            (1..=3).contains(&groups[0].len())
                && groups[0].chars().all(|c| c.is_ascii_digit())
                && groups[1..]
                    .iter()
                    .all(|group| group.len() == 3 && group.chars().all(|c| c.is_ascii_digit()))
        };
        if !grouping_is_valid {
            return Err("amount has invalid thousands grouping".to_string());
        }
        let compact = groups.concat();
        let absolute = compact
            .parse::<i64>()
            .map_err(|_| "amount is outside the supported whole-peso range".to_string())?;
        absolute
            .checked_mul(sign)
            .map(Self)
            .ok_or_else(|| "amount is outside the supported whole-peso range".to_string())
    }

    pub fn format_bir(self) -> String {
        let negative = self.0 < 0;
        let digits = self.0.unsigned_abs().to_string();
        let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
        for (index, character) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index).is_multiple_of(3) {
                grouped.push(',');
            }
            grouped.push(character);
        }
        if negative {
            format!("-{grouped}")
        } else {
            grouped
        }
    }
}

impl fmt::Display for WholePeso {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.format_bir())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form1702RTFilingBasis {
    #[default]
    Calendar,
    Fiscal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form1702RTDeductionMethod {
    Itemized,
    OptionalStandard,
    #[default]
    Unresolved,
}

impl Form1702RTDeductionMethod {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Itemized => "Itemized deductions",
            Self::OptionalStandard => "Optional Standard Deduction (40%)",
            Self::Unresolved => "Needs review",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1702RTOverpaymentDisposition {
    Refund,
    TaxCreditCertificate,
    CarryOver,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTAtcSelection {
    /// The official page prints IC055 for MCIT.
    pub printed_mcit_selected: bool,
    /// The reviewed save exposes a second ATC selector without enough evidence
    /// to infer mutual exclusivity with the printed IC055 control.
    pub other_selected: bool,
    pub other_code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Form1702RTDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Form1702RTDate {
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, String> {
        let value = Self { year, month, day };
        value.validate()?;
        Ok(value)
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        use chrono::Datelike;
        let parsed = chrono::NaiveDate::parse_from_str(value.trim(), "%m/%d/%Y")
            .map_err(|_| "date must use MM/DD/YYYY and be a real calendar date".to_string())?;
        Self::new(
            u16::try_from(parsed.year()).map_err(|_| "date year is unsupported".to_string())?,
            u8::try_from(parsed.month()).map_err(|_| "date month is unsupported".to_string())?,
            u8::try_from(parsed.day()).map_err(|_| "date day is unsupported".to_string())?,
        )
    }

    pub fn validate(self) -> Result<(), String> {
        chrono::NaiveDate::from_ymd_opt(
            i32::from(self.year),
            u32::from(self.month),
            u32::from(self.day),
        )
        .map(|_| ())
        .ok_or_else(|| "date is not a real calendar date".to_string())
    }
}

impl fmt::Display for Form1702RTDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:02}/{:02}/{:04}",
            self.month, self.day, self.year
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTNamedAmount {
    pub description: String,
    pub amount: WholePeso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTPaymentDetail {
    pub specification: String,
    pub drawee_bank_or_agency: String,
    pub number: String,
    pub date: Option<Form1702RTDate>,
    pub amount: WholePeso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTPartII {
    pub item_14_tax_due: WholePeso,
    pub item_15_total_tax_credits: WholePeso,
    pub item_16_net_tax_payable_or_overpayment: WholePeso,
    pub item_17_surcharge: WholePeso,
    pub item_18_interest: WholePeso,
    pub item_19_compromise: WholePeso,
    pub item_20_total_penalties: WholePeso,
    pub item_21_total_amount_payable_or_overpayment: WholePeso,
    pub overpayment_disposition: Option<Form1702RTOverpaymentDisposition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTTaxCredits {
    pub item_44_prior_year_excess_credits: WholePeso,
    pub item_45_previous_quarter_mcit_payments: WholePeso,
    pub item_46_previous_quarter_regular_payments: WholePeso,
    pub item_47_excess_mcit_applied: WholePeso,
    pub item_48_previous_quarter_withholding: WholePeso,
    pub item_49_fourth_quarter_withholding: WholePeso,
    pub item_50_foreign_tax_credits: WholePeso,
    pub item_51_tax_paid_on_previous_return: WholePeso,
    pub item_52_special_tax_credits: WholePeso,
    pub item_53_other: Form1702RTNamedAmount,
    pub item_54_other: Form1702RTNamedAmount,
    pub item_55_total: WholePeso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTPartIV {
    pub item_27_sales: WholePeso,
    pub item_28_sales_returns: WholePeso,
    pub item_29_net_sales: WholePeso,
    pub item_30_cost_of_sales_or_services: WholePeso,
    pub item_31_gross_income_from_operations: WholePeso,
    pub item_32_other_taxable_income: WholePeso,
    pub item_33_total_taxable_income: WholePeso,
    pub item_34_ordinary_itemized_deductions: WholePeso,
    pub item_35_special_itemized_deductions: WholePeso,
    pub item_36_nolco: WholePeso,
    pub item_37_total_itemized_deductions: WholePeso,
    pub item_38_optional_standard_deduction: WholePeso,
    pub item_39_net_taxable_income_or_loss: WholePeso,
    /// An explicit percentage from Item 40. Zero means unresolved, never a
    /// silent 25% or 30% default.
    pub item_40_income_tax_rate_percent: u8,
    pub item_41_normal_income_tax_due: WholePeso,
    pub item_42_mcit_due: WholePeso,
    pub item_43_tax_due: WholePeso,
    pub tax_credits: Form1702RTTaxCredits,
    pub item_56_net_tax_payable_or_overpayment: WholePeso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTPartV {
    pub item_57_special_allowable_deductions_tax_effect: WholePeso,
    pub item_58_special_tax_credits: WholePeso,
    pub item_59_total_tax_relief: WholePeso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTSchedule1 {
    pub amortizations: WholePeso,
    pub bad_debts: WholePeso,
    pub charitable_contributions: WholePeso,
    pub depletion: WholePeso,
    pub depreciation: WholePeso,
    pub entertainment: WholePeso,
    pub fringe_benefits: WholePeso,
    pub interest: WholePeso,
    pub losses: WholePeso,
    pub pension_trusts: WholePeso,
    pub rental: WholePeso,
    pub research_and_development: WholePeso,
    pub salaries_wages_allowances: WholePeso,
    pub statutory_contributions: WholePeso,
    pub taxes_and_licenses: WholePeso,
    pub transportation_and_travel: WholePeso,
    pub janitorial_and_messengerial: WholePeso,
    pub professional_fees: WholePeso,
    pub security_services: WholePeso,
    /// Official fixed capacity: Items 17d through 17i.
    pub other: [Form1702RTNamedAmount; 6],
    pub item_18_total: WholePeso,
}

impl Form1702RTSchedule1 {
    pub fn source_amounts(&self) -> [WholePeso; 19] {
        [
            self.amortizations,
            self.bad_debts,
            self.charitable_contributions,
            self.depletion,
            self.depreciation,
            self.entertainment,
            self.fringe_benefits,
            self.interest,
            self.losses,
            self.pension_trusts,
            self.rental,
            self.research_and_development,
            self.salaries_wages_allowances,
            self.statutory_contributions,
            self.taxes_and_licenses,
            self.transportation_and_travel,
            self.janitorial_and_messengerial,
            self.professional_fees,
            self.security_services,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTSpecialDeductionRow {
    pub description: String,
    pub legal_basis: String,
    pub amount: WholePeso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTSchedule2 {
    /// Official fixed capacity: four rows.
    pub rows: [Form1702RTSpecialDeductionRow; 4],
    pub item_5_total: WholePeso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTNolcoRow {
    pub year_incurred: String,
    pub amount: WholePeso,
    pub applied_previous_years: WholePeso,
    pub expired: WholePeso,
    pub applied_current_year: WholePeso,
    pub unapplied_balance: WholePeso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTSchedule3 {
    pub item_1_gross_income: WholePeso,
    pub item_2_ordinary_deductions: WholePeso,
    pub item_3_net_operating_loss: WholePeso,
    /// Official fixed capacity: Items 4 through 7.
    pub rows: [Form1702RTNolcoRow; 4],
    pub item_8_total_applied_current_year: WholePeso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTMcitRow {
    pub year: String,
    pub normal_income_tax: WholePeso,
    pub mcit: WholePeso,
    /// Item C is retained as an explicit input. The official label identifies
    /// it as excess MCIT but does not print a formula or floor-at-zero rule.
    pub excess_mcit: WholePeso,
    pub applied_previous_years: WholePeso,
    pub expired: WholePeso,
    pub applied_current_year: WholePeso,
    pub allowable_balance: WholePeso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTSchedule4 {
    /// Official fixed capacity: three rows.
    pub rows: [Form1702RTMcitRow; 3],
    pub item_4_total_applied_current_year: WholePeso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702RTSchedule5 {
    pub item_1_net_income_or_loss_per_books: WholePeso,
    pub additions: [Form1702RTNamedAmount; 2],
    pub item_4_total: WholePeso,
    pub non_taxable_income: [Form1702RTNamedAmount; 2],
    pub special_deductions: [Form1702RTNamedAmount; 2],
    pub item_9_total: WholePeso,
    pub item_10_net_taxable_income_or_loss: WholePeso,
}

/// Full four-page editable draft. Transport-only modal/subtotal fields are
/// retained separately so an imported official save can round-trip exactly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Form1702RTDraft {
    pub id: Option<i64>,
    pub tin: String,
    pub taxable_year: u16,
    pub month: u8,
    pub filing_basis: Form1702RTFilingBasis,
    pub is_amended: bool,
    pub is_short_period: bool,
    pub atc: Form1702RTAtcSelection,
    pub rdo_code: String,
    pub taxpayer_name: String,
    pub registered_name_lines: [String; 3],
    pub registered_address: String,
    pub registered_address_lines: [String; 3],
    pub zip_code: String,
    pub incorporation_date: Option<Form1702RTDate>,
    pub contact_number: String,
    pub email: String,
    pub deduction_method: Form1702RTDeductionMethod,
    pub part_ii: Form1702RTPartII,
    /// Official fixed payment rows: Cash/Bank Debit Memo, Check, Tax Debit
    /// Memo, and Others.
    pub payment_details: [Form1702RTPaymentDetail; 4],
    pub part_iv: Form1702RTPartIV,
    pub part_v: Form1702RTPartV,
    pub schedule_1: Form1702RTSchedule1,
    pub schedule_2: Form1702RTSchedule2,
    pub schedule_3: Form1702RTSchedule3,
    pub schedule_4: Form1702RTSchedule4,
    pub schedule_5: Form1702RTSchedule5,
    pub president_signature: String,
    pub treasurer_signature: String,
    /// XML `txtPg1Pt2Signatory1` is printed under "Title of Signatory".
    #[serde(alias = "president_signatory_name")]
    pub president_signatory_title: String,
    pub president_signatory_tin: String,
    /// XML `txtPg1Pt2Signatory2` is printed under "Title of Signatory".
    #[serde(alias = "treasurer_signatory_name")]
    pub treasurer_signatory_title: String,
    pub treasurer_signatory_tin: String,
    /// Item 22 is a three-character field in the reviewed save.
    pub number_of_attachments: String,
    /// Reviewed values are `0` (encrypted companion) and `1` (plain save).
    pub xml_final_flag: String,
    #[serde(default)]
    pub preserved_transport_fields: BTreeMap<String, String>,
    #[serde(default)]
    pub calculation_issues: Vec<(String, String)>,
    /// Pre-queue builds stored their own error text here. Kept so old JSON
    /// round-trips; the generic lifecycle reports `submission_error`.
    pub last_error: Option<String>,
    /// Status, queue authorization and retry state. Flattened so stored JSON
    /// keeps the keys earlier builds wrote inline (`status`, `created_at`,
    /// `submission_attempts`, ...).
    #[serde(flatten)]
    pub lifecycle: SubmissionLifecycle,
}

impl Form1702RTDraft {
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, month: u8) -> Self {
        let incorporation_date = profile.business_start_date.and_then(|date| {
            use chrono::Datelike;
            Form1702RTDate::new(
                u16::try_from(date.year()).ok()?,
                u8::try_from(date.month()).ok()?,
                u8::try_from(date.day()).ok()?,
            )
            .ok()
        });
        let mut draft = Self {
            tin: profile.tin.full(),
            taxable_year: year,
            month,
            filing_basis: Form1702RTFilingBasis::Calendar,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_name_lines: [profile.full_name.clone(), String::new(), String::new()],
            registered_address: profile.registered_address.clone(),
            registered_address_lines: [
                profile.registered_address.clone(),
                String::new(),
                String::new(),
            ],
            zip_code: profile.zip_code.clone(),
            incorporation_date,
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            number_of_attachments: "000".to_string(),
            xml_final_flag: "1".to_string(),
            ..Self::default()
        };
        draft.recompute();
        draft
    }

    pub fn is_editable(&self) -> bool {
        self.lifecycle.is_editable()
    }

    /// The official compute chain (see `form_1702rt_official`).
    pub fn recompute(&mut self) {
        self.calculation_issues.clear();
        self.official_recompute();
    }
}

impl FormValidator for Form1702RTDraft {
    /// The official `validate()` port; see `form_1702rt_official`.
    fn validate(&self) -> Vec<(String, String)> {
        self.official_errors()
    }
}

impl TypedBirForm for Form1702RTDraft {
    fn form_code(&self) -> &'static str {
        FORM_CODE
    }

    fn form_type_id(&self) -> &'static str {
        FORM_TYPE_ID
    }

    fn filing_period(&self) -> FilingPeriod {
        FilingPeriod::Annual
    }

    fn recompute(&mut self) {
        Form1702RTDraft::recompute(self);
    }

    fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        Form1702RTDraft::to_bir_field_map(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_peso_parser_preserves_negative_values_and_rejects_centavos() {
        assert_eq!(WholePeso::parse_bir("-8,000"), Ok(WholePeso(-8_000)));
        assert_eq!(WholePeso(-8_000).format_bir(), "-8,000");
        assert!(WholePeso::parse_bir("1,2").is_err());
        assert!(WholePeso::parse_bir("100.50").is_err());
    }

    #[test]
    fn local_json_persistence_preserves_signed_amounts_and_fixed_schedule_rows() {
        let mut draft = Form1702RTDraft::default();
        draft.part_iv.item_31_gross_income_from_operations = WholePeso(-1_000);
        draft.schedule_3.rows[3].amount = WholePeso(-8_000);
        draft.schedule_4.rows[2].year = "2025".to_string();

        let json = serde_json::to_string(&draft).expect("semantic draft serializes");
        let restored: Form1702RTDraft =
            serde_json::from_str(&json).expect("semantic draft deserializes");

        assert_eq!(restored, draft);
        assert_eq!(restored.schedule_3.rows.len(), 4);
        assert_eq!(restored.schedule_4.rows.len(), 3);
    }

    #[test]
    fn reviewed_ic010_description_and_legacy_signatory_title_aliases_are_exact() {
        assert_eq!(
            reviewed_alternate_atc_description("IC010"),
            Some(REVIEWED_ALTERNATE_ATC_IC010_DESCRIPTION)
        );
        assert_eq!(reviewed_alternate_atc_description("IC020"), None);

        let restored: Form1702RTDraft = serde_json::from_str(
            r#"{"president_signatory_name":"PRESIDENT","treasurer_signatory_name":"TREASURER","status":"Draft","created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z"}"#,
        )
        .expect("legacy signatory keys remain readable");
        assert_eq!(restored.president_signatory_title, "PRESIDENT");
        assert_eq!(restored.treasurer_signatory_title, "TREASURER");
    }
}
