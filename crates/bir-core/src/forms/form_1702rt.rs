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

use super::{FilingPeriod, FilingStatus, FormValidator, TypedBirForm};
use crate::profile::TaxpayerProfile;
use crate::validation::{validate_email, validate_ph_phone, validate_zip};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
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
    pub status: FilingStatus,
    pub created_at: String,
    pub updated_at: String,
    pub submitted_at: Option<String>,
    pub confirmed_at: Option<String>,
    pub submission_filename: Option<String>,
    pub receipt_id: Option<i64>,
    pub submission_attempts: u32,
    pub next_retry_at: Option<String>,
    pub last_error: Option<String>,
}

impl Form1702RTDraft {
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, month: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
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
            status: FilingStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            ..Self::default()
        };
        draft.recompute();
        draft
    }

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    /// Recompute only formulas printed by the locked official form. No rate,
    /// applicability, or floor-at-zero rule is invented.
    pub fn recompute(&mut self) {
        self.calculation_issues.clear();

        self.schedule_1.item_18_total = checked_sum(
            "schedule_1.item_18_total",
            self.schedule_1
                .source_amounts()
                .into_iter()
                .chain(self.schedule_1.other.iter().map(|row| row.amount)),
            &mut self.calculation_issues,
        );
        self.schedule_2.item_5_total = checked_sum(
            "schedule_2.item_5_total",
            self.schedule_2.rows.iter().map(|row| row.amount),
            &mut self.calculation_issues,
        );

        // Part IV Items 27-33 are required before Schedule III.
        self.part_iv.item_29_net_sales = checked_sub(
            "part_iv.item_29_net_sales",
            self.part_iv.item_27_sales,
            self.part_iv.item_28_sales_returns,
            &mut self.calculation_issues,
        );
        self.part_iv.item_31_gross_income_from_operations = checked_sub(
            "part_iv.item_31_gross_income_from_operations",
            self.part_iv.item_29_net_sales,
            self.part_iv.item_30_cost_of_sales_or_services,
            &mut self.calculation_issues,
        );
        self.part_iv.item_33_total_taxable_income = checked_sum(
            "part_iv.item_33_total_taxable_income",
            [
                self.part_iv.item_31_gross_income_from_operations,
                self.part_iv.item_32_other_taxable_income,
            ],
            &mut self.calculation_issues,
        );

        self.schedule_3.item_1_gross_income = self.part_iv.item_33_total_taxable_income;
        self.schedule_3.item_2_ordinary_deductions = self.schedule_1.item_18_total;
        self.schedule_3.item_3_net_operating_loss = checked_sub(
            "schedule_3.item_3_net_operating_loss",
            self.schedule_3.item_1_gross_income,
            self.schedule_3.item_2_ordinary_deductions,
            &mut self.calculation_issues,
        );
        // The official form explicitly carries Schedule III Item 3 to Item 7A.
        self.schedule_3.rows[3].amount = self.schedule_3.item_3_net_operating_loss;
        for (index, row) in self.schedule_3.rows.iter_mut().enumerate() {
            row.unapplied_balance = checked_sub_many(
                &format!("schedule_3.rows[{index}].unapplied_balance"),
                row.amount,
                [
                    row.applied_previous_years,
                    row.expired,
                    row.applied_current_year,
                ],
                &mut self.calculation_issues,
            );
        }
        self.schedule_3.item_8_total_applied_current_year = checked_sum(
            "schedule_3.item_8_total_applied_current_year",
            self.schedule_3
                .rows
                .iter()
                .map(|row| row.applied_current_year),
            &mut self.calculation_issues,
        );

        for (index, row) in self.schedule_4.rows.iter_mut().enumerate() {
            row.allowable_balance = checked_sub_many(
                &format!("schedule_4.rows[{index}].allowable_balance"),
                row.excess_mcit,
                [
                    row.applied_previous_years,
                    row.expired,
                    row.applied_current_year,
                ],
                &mut self.calculation_issues,
            );
        }
        self.schedule_4.item_4_total_applied_current_year = checked_sum(
            "schedule_4.item_4_total_applied_current_year",
            self.schedule_4
                .rows
                .iter()
                .map(|row| row.applied_current_year),
            &mut self.calculation_issues,
        );

        self.schedule_5.item_4_total = checked_sum(
            "schedule_5.item_4_total",
            std::iter::once(self.schedule_5.item_1_net_income_or_loss_per_books)
                .chain(self.schedule_5.additions.iter().map(|row| row.amount)),
            &mut self.calculation_issues,
        );
        self.schedule_5.item_9_total = checked_sum(
            "schedule_5.item_9_total",
            self.schedule_5
                .non_taxable_income
                .iter()
                .chain(self.schedule_5.special_deductions.iter())
                .map(|row| row.amount),
            &mut self.calculation_issues,
        );
        self.schedule_5.item_10_net_taxable_income_or_loss = checked_sub(
            "schedule_5.item_10_net_taxable_income_or_loss",
            self.schedule_5.item_4_total,
            self.schedule_5.item_9_total,
            &mut self.calculation_issues,
        );

        self.part_iv.item_34_ordinary_itemized_deductions = self.schedule_1.item_18_total;
        self.part_iv.item_35_special_itemized_deductions = self.schedule_2.item_5_total;
        self.part_iv.item_36_nolco = self.schedule_3.item_8_total_applied_current_year;
        self.part_iv.item_37_total_itemized_deductions = checked_sum(
            "part_iv.item_37_total_itemized_deductions",
            [
                self.part_iv.item_34_ordinary_itemized_deductions,
                self.part_iv.item_35_special_itemized_deductions,
                self.part_iv.item_36_nolco,
            ],
            &mut self.calculation_issues,
        );
        match self.deduction_method {
            Form1702RTDeductionMethod::Itemized => {
                self.part_iv.item_38_optional_standard_deduction = WholePeso::ZERO;
                self.part_iv.item_39_net_taxable_income_or_loss = checked_sub(
                    "part_iv.item_39_net_taxable_income_or_loss",
                    self.part_iv.item_33_total_taxable_income,
                    self.part_iv.item_37_total_itemized_deductions,
                    &mut self.calculation_issues,
                );
            }
            Form1702RTDeductionMethod::OptionalStandard => {
                self.part_iv.item_38_optional_standard_deduction = checked_percent(
                    "part_iv.item_38_optional_standard_deduction",
                    self.part_iv.item_33_total_taxable_income,
                    40,
                    &mut self.calculation_issues,
                );
                self.part_iv.item_39_net_taxable_income_or_loss = checked_sub(
                    "part_iv.item_39_net_taxable_income_or_loss",
                    self.part_iv.item_33_total_taxable_income,
                    self.part_iv.item_38_optional_standard_deduction,
                    &mut self.calculation_issues,
                );
            }
            Form1702RTDeductionMethod::Unresolved => {
                self.part_iv.item_38_optional_standard_deduction = WholePeso::ZERO;
                self.part_iv.item_39_net_taxable_income_or_loss = WholePeso::ZERO;
                self.calculation_issues.push((
                    "deduction_method".to_string(),
                    "Item 39 cannot be calculated until Item 13 is selected".to_string(),
                ));
            }
        }
        if self.part_iv.item_40_income_tax_rate_percent == 0 {
            self.part_iv.item_41_normal_income_tax_due = WholePeso::ZERO;
            self.calculation_issues.push((
                "part_iv.item_40_income_tax_rate_percent".to_string(),
                "Item 40 requires an evidenced applicable income-tax rate".to_string(),
            ));
        } else {
            self.part_iv.item_41_normal_income_tax_due = checked_percent(
                "part_iv.item_41_normal_income_tax_due",
                self.part_iv.item_39_net_taxable_income_or_loss,
                self.part_iv.item_40_income_tax_rate_percent,
                &mut self.calculation_issues,
            );
        }
        self.part_iv.item_42_mcit_due = checked_percent(
            "part_iv.item_42_mcit_due",
            self.part_iv.item_33_total_taxable_income,
            2,
            &mut self.calculation_issues,
        );
        self.part_iv.item_43_tax_due = std::cmp::max(
            self.part_iv.item_41_normal_income_tax_due,
            self.part_iv.item_42_mcit_due,
        );
        self.part_iv.tax_credits.item_47_excess_mcit_applied =
            self.schedule_4.item_4_total_applied_current_year;
        self.part_iv.tax_credits.item_55_total = checked_sum(
            "part_iv.tax_credits.item_55_total",
            [
                self.part_iv.tax_credits.item_44_prior_year_excess_credits,
                self.part_iv
                    .tax_credits
                    .item_45_previous_quarter_mcit_payments,
                self.part_iv
                    .tax_credits
                    .item_46_previous_quarter_regular_payments,
                self.part_iv.tax_credits.item_47_excess_mcit_applied,
                self.part_iv
                    .tax_credits
                    .item_48_previous_quarter_withholding,
                self.part_iv.tax_credits.item_49_fourth_quarter_withholding,
                self.part_iv.tax_credits.item_50_foreign_tax_credits,
                self.part_iv.tax_credits.item_51_tax_paid_on_previous_return,
                self.part_iv.tax_credits.item_52_special_tax_credits,
                self.part_iv.tax_credits.item_53_other.amount,
                self.part_iv.tax_credits.item_54_other.amount,
            ],
            &mut self.calculation_issues,
        );
        self.part_iv.item_56_net_tax_payable_or_overpayment = checked_sub(
            "part_iv.item_56_net_tax_payable_or_overpayment",
            self.part_iv.item_43_tax_due,
            self.part_iv.tax_credits.item_55_total,
            &mut self.calculation_issues,
        );

        self.part_v.item_57_special_allowable_deductions_tax_effect = checked_percent(
            "part_v.item_57_special_allowable_deductions_tax_effect",
            self.part_iv.item_35_special_itemized_deductions,
            self.part_iv.item_40_income_tax_rate_percent,
            &mut self.calculation_issues,
        );
        self.part_v.item_58_special_tax_credits =
            self.part_iv.tax_credits.item_52_special_tax_credits;
        self.part_v.item_59_total_tax_relief = checked_sum(
            "part_v.item_59_total_tax_relief",
            [
                self.part_v.item_57_special_allowable_deductions_tax_effect,
                self.part_v.item_58_special_tax_credits,
            ],
            &mut self.calculation_issues,
        );

        self.part_ii.item_14_tax_due = self.part_iv.item_43_tax_due;
        self.part_ii.item_15_total_tax_credits = self.part_iv.tax_credits.item_55_total;
        self.part_ii.item_16_net_tax_payable_or_overpayment =
            self.part_iv.item_56_net_tax_payable_or_overpayment;
        self.part_ii.item_20_total_penalties = checked_sum(
            "part_ii.item_20_total_penalties",
            [
                self.part_ii.item_17_surcharge,
                self.part_ii.item_18_interest,
                self.part_ii.item_19_compromise,
            ],
            &mut self.calculation_issues,
        );
        self.part_ii.item_21_total_amount_payable_or_overpayment = checked_sum(
            "part_ii.item_21_total_amount_payable_or_overpayment",
            [
                self.part_ii.item_16_net_tax_payable_or_overpayment,
                self.part_ii.item_20_total_penalties,
            ],
            &mut self.calculation_issues,
        );

        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        Err(vec![(
            "submission".to_string(),
            "1702RTv2018C electronic submission is disabled: the reviewed XML establishes editable-save persistence only".to_string(),
        )])
    }

    pub fn transition_to_paid(&mut self) -> Result<(), String> {
        if !matches!(self.status, FilingStatus::Confirmed) {
            return Err("Only a confirmed return can be marked paid".to_string());
        }
        self.status = FilingStatus::Paid;
        self.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }

    pub fn revert_to_draft(&mut self) -> Result<(), String> {
        if matches!(self.status, FilingStatus::Paid) {
            return Err("A paid return requires an explicit amendment workflow".to_string());
        }
        self.status = FilingStatus::Draft;
        self.submitted_at = None;
        self.confirmed_at = None;
        self.submission_filename = None;
        self.receipt_id = None;
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }
}

impl FormValidator for Form1702RTDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        let compact_tin = self
            .tin
            .chars()
            .filter(|character| character.is_ascii_digit())
            .collect::<String>();
        if !matches!(compact_tin.len(), 12..=14)
            || compact_tin.len() != self.tin.chars().filter(|c| c.is_ascii_digit()).count()
            || self.tin.chars().any(|c| !c.is_ascii_digit() && c != '-')
        {
            errors.push((
                "tin".to_string(),
                "TIN must contain 12 to 14 digits, optionally separated by dashes".to_string(),
            ));
        }
        if !(2000..=2099).contains(&self.taxable_year) {
            errors.push((
                "taxable_year".to_string(),
                "Item 2 prints a fixed MM/20YY year and supports taxable years 2000 through 2099"
                    .to_string(),
            ));
        }
        if !(1..=12).contains(&self.month) {
            errors.push((
                "month".to_string(),
                "Year-end month must be between 1 and 12".to_string(),
            ));
        }
        for (field, value) in [
            ("rdo_code", self.rdo_code.as_str()),
            ("taxpayer_name", self.taxpayer_name.as_str()),
            ("registered_address", self.registered_address.as_str()),
        ] {
            if value.trim().is_empty() {
                errors.push((
                    field.to_string(),
                    "This profile-prefilled value is required".to_string(),
                ));
            }
        }
        if !self.zip_code.trim().is_empty() && !validate_zip(self.zip_code.trim()) {
            errors.push((
                "zip_code".to_string(),
                "ZIP code must contain four digits".to_string(),
            ));
        }
        if !self.contact_number.trim().is_empty() && !validate_ph_phone(&self.contact_number) {
            errors.push((
                "contact_number".to_string(),
                "Contact number is not a recognized Philippine phone number".to_string(),
            ));
        }
        if !self.email.trim().is_empty() && !validate_email(&self.email) {
            errors.push(("email".to_string(), "Email address is invalid".to_string()));
        }
        if matches!(self.deduction_method, Form1702RTDeductionMethod::Unresolved) {
            errors.push((
                "deduction_method".to_string(),
                "Item 13 method of deductions must be selected".to_string(),
            ));
        }
        if self.part_iv.item_40_income_tax_rate_percent == 0
            || self.part_iv.item_40_income_tax_rate_percent > 100
        {
            errors.push((
                "part_iv.item_40_income_tax_rate_percent".to_string(),
                "Item 40 must be an explicitly reviewed percentage from 1 to 100".to_string(),
            ));
        }
        if !self.atc.printed_mcit_selected && !self.atc.other_selected {
            errors.push((
                "atc".to_string(),
                "Item 5 ATC selection is unresolved".to_string(),
            ));
        }
        if self.atc.other_selected {
            if self.atc.other_code.trim().is_empty() {
                errors.push((
                    "atc.other_code".to_string(),
                    "The alternate ATC selector requires an exact code".to_string(),
                ));
            } else if reviewed_alternate_atc_description(&self.atc.other_code).is_none() {
                errors.push((
                    "atc.other_code".to_string(),
                    format!(
                        "Alternate ATC {} has no reviewed 1702RTv2018C code/description evidence",
                        self.atc.other_code.trim()
                    ),
                ));
            }
        }
        for (index, row) in self.payment_details.iter().enumerate() {
            let item = index + 23;
            if item != 26 && !row.specification.trim().is_empty() {
                errors.push((
                    format!("payment_details[{index}].specification"),
                    format!("Official Item {item} has no specification field"),
                ));
            }
            if item == 25 && !row.drawee_bank_or_agency.trim().is_empty() {
                errors.push((
                    "payment_details[2].drawee_bank_or_agency".to_string(),
                    "Official Item 25 Tax Debit Memo has no Drawee Bank/Agency field".to_string(),
                ));
            }
        }
        if !self.is_amended
            && self
                .part_iv
                .tax_credits
                .item_51_tax_paid_on_previous_return
                .0
                != 0
        {
            errors.push((
                "part_iv.tax_credits.item_51_tax_paid_on_previous_return".to_string(),
                "Item 51 applies only to an amended return".to_string(),
            ));
        }
        if self.part_ii.item_21_total_amount_payable_or_overpayment.0 < 0
            && self.part_ii.overpayment_disposition.is_none()
        {
            errors.push((
                "part_ii.overpayment_disposition".to_string(),
                "An overpayment requires exactly one irrevocable disposition".to_string(),
            ));
        }
        if self.part_ii.item_21_total_amount_payable_or_overpayment.0 >= 0
            && self.part_ii.overpayment_disposition.is_some()
        {
            errors.push((
                "part_ii.overpayment_disposition".to_string(),
                "The overpayment disposition boxes apply only when Item 21 is negative".to_string(),
            ));
        }
        for (field, row) in self
            .schedule_1
            .other
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                (row.amount.0 != 0 && row.description.trim().is_empty())
                    .then_some((format!("schedule_1.other[{index}].description"), row))
            })
        {
            let _ = row;
            errors.push((
                field,
                "An Others deduction amount requires a description".to_string(),
            ));
        }
        for (index, row) in self.schedule_2.rows.iter().enumerate() {
            if row.amount.0 != 0
                && (row.description.trim().is_empty() || row.legal_basis.trim().is_empty())
            {
                errors.push((
                    format!("schedule_2.rows[{index}]"),
                    "A special deduction amount requires both description and legal basis"
                        .to_string(),
                ));
            }
        }
        for (field, row) in [
            (
                "part_iv.tax_credits.item_53_other",
                &self.part_iv.tax_credits.item_53_other,
            ),
            (
                "part_iv.tax_credits.item_54_other",
                &self.part_iv.tax_credits.item_54_other,
            ),
        ] {
            if row.amount.0 != 0 && row.description.trim().is_empty() {
                errors.push((
                    field.to_string(),
                    "An other tax credit requires a description".to_string(),
                ));
            }
        }
        if !matches!(self.xml_final_flag.as_str(), "0" | "1") {
            errors.push((
                "xml_final_flag".to_string(),
                "txtFinalFlag must be one of the two reviewed values 0 or 1".to_string(),
            ));
        }

        errors.extend(self.calculation_issues.clone());
        let mut expected = self.clone();
        expected.recompute();
        errors.extend(expected.calculation_issues.clone());
        compare_derived_values(self, &expected, &mut errors);
        errors
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

fn compare_derived_values(
    actual: &Form1702RTDraft,
    expected: &Form1702RTDraft,
    errors: &mut Vec<(String, String)>,
) {
    let pairs = [
        (
            "schedule_1.item_18_total",
            actual.schedule_1.item_18_total,
            expected.schedule_1.item_18_total,
        ),
        (
            "schedule_2.item_5_total",
            actual.schedule_2.item_5_total,
            expected.schedule_2.item_5_total,
        ),
        (
            "schedule_3.item_3_net_operating_loss",
            actual.schedule_3.item_3_net_operating_loss,
            expected.schedule_3.item_3_net_operating_loss,
        ),
        (
            "schedule_3.item_8_total_applied_current_year",
            actual.schedule_3.item_8_total_applied_current_year,
            expected.schedule_3.item_8_total_applied_current_year,
        ),
        (
            "schedule_4.item_4_total_applied_current_year",
            actual.schedule_4.item_4_total_applied_current_year,
            expected.schedule_4.item_4_total_applied_current_year,
        ),
        (
            "schedule_5.item_4_total",
            actual.schedule_5.item_4_total,
            expected.schedule_5.item_4_total,
        ),
        (
            "schedule_5.item_9_total",
            actual.schedule_5.item_9_total,
            expected.schedule_5.item_9_total,
        ),
        (
            "schedule_5.item_10_net_taxable_income_or_loss",
            actual.schedule_5.item_10_net_taxable_income_or_loss,
            expected.schedule_5.item_10_net_taxable_income_or_loss,
        ),
        (
            "part_iv.item_29_net_sales",
            actual.part_iv.item_29_net_sales,
            expected.part_iv.item_29_net_sales,
        ),
        (
            "part_iv.item_31_gross_income_from_operations",
            actual.part_iv.item_31_gross_income_from_operations,
            expected.part_iv.item_31_gross_income_from_operations,
        ),
        (
            "part_iv.item_33_total_taxable_income",
            actual.part_iv.item_33_total_taxable_income,
            expected.part_iv.item_33_total_taxable_income,
        ),
        (
            "part_iv.item_37_total_itemized_deductions",
            actual.part_iv.item_37_total_itemized_deductions,
            expected.part_iv.item_37_total_itemized_deductions,
        ),
        (
            "part_iv.item_38_optional_standard_deduction",
            actual.part_iv.item_38_optional_standard_deduction,
            expected.part_iv.item_38_optional_standard_deduction,
        ),
        (
            "part_iv.item_39_net_taxable_income_or_loss",
            actual.part_iv.item_39_net_taxable_income_or_loss,
            expected.part_iv.item_39_net_taxable_income_or_loss,
        ),
        (
            "part_iv.item_41_normal_income_tax_due",
            actual.part_iv.item_41_normal_income_tax_due,
            expected.part_iv.item_41_normal_income_tax_due,
        ),
        (
            "part_iv.item_42_mcit_due",
            actual.part_iv.item_42_mcit_due,
            expected.part_iv.item_42_mcit_due,
        ),
        (
            "part_iv.item_43_tax_due",
            actual.part_iv.item_43_tax_due,
            expected.part_iv.item_43_tax_due,
        ),
        (
            "part_iv.tax_credits.item_55_total",
            actual.part_iv.tax_credits.item_55_total,
            expected.part_iv.tax_credits.item_55_total,
        ),
        (
            "part_iv.item_56_net_tax_payable_or_overpayment",
            actual.part_iv.item_56_net_tax_payable_or_overpayment,
            expected.part_iv.item_56_net_tax_payable_or_overpayment,
        ),
        (
            "part_v.item_59_total_tax_relief",
            actual.part_v.item_59_total_tax_relief,
            expected.part_v.item_59_total_tax_relief,
        ),
        (
            "part_ii.item_20_total_penalties",
            actual.part_ii.item_20_total_penalties,
            expected.part_ii.item_20_total_penalties,
        ),
        (
            "part_ii.item_21_total_amount_payable_or_overpayment",
            actual.part_ii.item_21_total_amount_payable_or_overpayment,
            expected.part_ii.item_21_total_amount_payable_or_overpayment,
        ),
    ];
    for (field, actual, expected) in pairs {
        if actual != expected {
            errors.push((
                field.to_string(),
                format!(
                    "Stored value {} does not match the official printed formula result {}",
                    actual, expected
                ),
            ));
        }
    }
    for (index, (actual, expected)) in actual
        .schedule_3
        .rows
        .iter()
        .zip(expected.schedule_3.rows.iter())
        .enumerate()
    {
        if actual.unapplied_balance != expected.unapplied_balance {
            errors.push((
                format!("schedule_3.rows[{index}].unapplied_balance"),
                "Stored NOLCO balance does not match E = A - (B + C + D)".to_string(),
            ));
        }
    }
    for (index, (actual, expected)) in actual
        .schedule_4
        .rows
        .iter()
        .zip(expected.schedule_4.rows.iter())
        .enumerate()
    {
        if actual.allowable_balance != expected.allowable_balance {
            errors.push((
                format!("schedule_4.rows[{index}].allowable_balance"),
                "Stored MCIT balance does not match G = C - (D + E + F)".to_string(),
            ));
        }
    }
}

fn checked_sum(
    field: &str,
    values: impl IntoIterator<Item = WholePeso>,
    issues: &mut Vec<(String, String)>,
) -> WholePeso {
    let total = values
        .into_iter()
        .try_fold(0_i64, |total, value| total.checked_add(value.0));
    match total {
        Some(total) => WholePeso(total),
        None => {
            issues.push((
                field.to_string(),
                "Whole-peso calculation overflowed".to_string(),
            ));
            WholePeso::ZERO
        }
    }
}

fn checked_sub(
    field: &str,
    left: WholePeso,
    right: WholePeso,
    issues: &mut Vec<(String, String)>,
) -> WholePeso {
    match left.0.checked_sub(right.0) {
        Some(value) => WholePeso(value),
        None => {
            issues.push((
                field.to_string(),
                "Whole-peso calculation overflowed".to_string(),
            ));
            WholePeso::ZERO
        }
    }
}

fn checked_sub_many(
    field: &str,
    minuend: WholePeso,
    subtrahends: impl IntoIterator<Item = WholePeso>,
    issues: &mut Vec<(String, String)>,
) -> WholePeso {
    let result = subtrahends
        .into_iter()
        .try_fold(minuend.0, |value, subtrahend| {
            value.checked_sub(subtrahend.0)
        });
    match result {
        Some(value) => WholePeso(value),
        None => {
            issues.push((
                field.to_string(),
                "Whole-peso calculation overflowed".to_string(),
            ));
            WholePeso::ZERO
        }
    }
}

fn checked_percent(
    field: &str,
    amount: WholePeso,
    percent: u8,
    issues: &mut Vec<(String, String)>,
) -> WholePeso {
    let numerator = i128::from(amount.0) * i128::from(percent);
    // The form's whole-peso instruction rounds an absolute 0.50 upward. For
    // negative values this is symmetric, away from zero at the half boundary.
    let rounded = if numerator >= 0 {
        (numerator + 50) / 100
    } else {
        (numerator - 50) / 100
    };
    match i64::try_from(rounded) {
        Ok(value) => WholePeso(value),
        Err(_) => {
            issues.push((
                field.to_string(),
                "Whole-peso percentage overflowed".to_string(),
            ));
            WholePeso::ZERO
        }
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
    fn printed_part_iv_and_schedule_formulas_are_recomputed_without_a_rate_default() {
        let mut draft = Form1702RTDraft {
            deduction_method: Form1702RTDeductionMethod::OptionalStandard,
            part_iv: Form1702RTPartIV {
                item_27_sales: WholePeso(100_000),
                item_28_sales_returns: WholePeso(10_000),
                item_30_cost_of_sales_or_services: WholePeso(20_000),
                item_32_other_taxable_income: WholePeso(5_000),
                item_40_income_tax_rate_percent: 30,
                ..Form1702RTPartIV::default()
            },
            ..Form1702RTDraft::default()
        };
        draft.recompute();
        assert_eq!(draft.part_iv.item_29_net_sales, WholePeso(90_000));
        assert_eq!(
            draft.part_iv.item_31_gross_income_from_operations,
            WholePeso(70_000)
        );
        assert_eq!(
            draft.part_iv.item_33_total_taxable_income,
            WholePeso(75_000)
        );
        assert_eq!(
            draft.part_iv.item_38_optional_standard_deduction,
            WholePeso(30_000)
        );
        assert_eq!(
            draft.part_iv.item_39_net_taxable_income_or_loss,
            WholePeso(45_000)
        );
        assert_eq!(
            draft.part_iv.item_41_normal_income_tax_due,
            WholePeso(13_500)
        );
        assert_eq!(draft.part_iv.item_42_mcit_due, WholePeso(1_500));
        assert_eq!(draft.part_iv.item_43_tax_due, WholePeso(13_500));
    }

    #[test]
    fn queue_submission_is_explicitly_disabled() {
        let mut draft = Form1702RTDraft::default();
        let errors = draft.transition_to_queued().expect_err("must fail closed");
        assert!(errors[0].1.contains("editable-save persistence only"));
        assert_eq!(draft.status, FilingStatus::Draft);
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
    fn page_one_semantics_fail_closed_for_unreviewed_atc_years_and_payment_cells() {
        let mut draft = Form1702RTDraft {
            taxable_year: 2100,
            atc: Form1702RTAtcSelection {
                other_selected: true,
                other_code: "IC999".to_string(),
                ..Form1702RTAtcSelection::default()
            },
            ..Form1702RTDraft::default()
        };
        draft.payment_details[0].specification = "NOT AN OFFICIAL ITEM 23 FIELD".to_string();
        draft.payment_details[2].drawee_bank_or_agency = "NOT AN ITEM 25 FIELD".to_string();

        let errors = draft.validate();
        assert!(
            errors
                .iter()
                .any(|(field, message)| { field == "taxable_year" && message.contains("MM/20YY") })
        );
        assert!(errors.iter().any(|(field, message)| {
            field == "atc.other_code" && message.contains("no reviewed 1702RTv2018C")
        }));
        assert!(errors.iter().any(|(field, message)| {
            field == "payment_details[0].specification" && message.contains("Item 23")
        }));
        assert!(errors.iter().any(|(field, message)| {
            field == "payment_details[2].drawee_bank_or_agency" && message.contains("no Drawee")
        }));
    }

    #[test]
    fn reviewed_ic010_description_and_legacy_signatory_title_aliases_are_exact() {
        assert_eq!(
            reviewed_alternate_atc_description("IC010"),
            Some(REVIEWED_ALTERNATE_ATC_IC010_DESCRIPTION)
        );
        assert_eq!(reviewed_alternate_atc_description("IC020"), None);

        let restored: Form1702RTDraft = serde_json::from_str(
            r#"{"president_signatory_name":"PRESIDENT","treasurer_signatory_name":"TREASURER"}"#,
        )
        .expect("legacy signatory keys remain readable");
        assert_eq!(restored.president_signatory_title, "PRESIDENT");
        assert_eq!(restored.treasurer_signatory_title, "TREASURER");
    }
}
