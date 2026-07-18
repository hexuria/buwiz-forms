//! Semantic domain for exact form `1702MXv2018C`.
//!
//! The reviewed source set contains a four-page January 2018 return and a
//! separate two-page mandatory-attachment document. This model keeps those
//! documents distinct. It supports local editable-save persistence only; no
//! electronic-submission or attachment-transport contract has been reviewed.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::forms::{FilingPeriod, FilingStatus, FormValidator, TypedBirForm};
use crate::profile::TaxpayerProfile;

pub const FORM_CODE: &str = "1702MX";
pub const FORM_TYPE_ID: &str = "1702MXv2018C";
pub const OFFICIAL_BASE_PAGE_COUNT: usize = 4;
pub const OFFICIAL_ATTACHMENT_PAGE_COUNT: usize = 2;
pub const QUEUE_SUBMISSION_SUPPORTED: bool = false;
pub const MANDATORY_ATTACHMENT_TRANSPORT_SUPPORTED: bool = false;
pub const OFFICIAL_FORM_SHA256: &str =
    "81c05fffadde6c0b4098aeba8547a9820a0806c6be9b0c6ceac5597cab4263d2";
pub const OFFICIAL_ATTACHMENT_SHA256: &str =
    "36c02d4c84919d2e5b94cd31b339490019be80afa622f5681ce252c8ec3dec26";
pub const REVIEWED_EDITABLE_XML_SHA256: &str =
    "ed96c5b56eecee68f1f73eef50dda00f69a42bd0dc5d0849e2cbe22c6b70b239";
pub const REVIEWED_ENCRYPTED_XML_SHA256: &str =
    "ab4896a21603c7853985b6589a918c3d0189872b1817a4a55453ebea063a47b4";

/// Signed whole-peso amount. Negative values represent losses or overpayments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub struct WholePeso(pub i64);

impl fmt::Display for WholePeso {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A whole-peso XML input that preserves whether the source was blank or zero
/// and retains the reviewed source lexeme for exact round-trips.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WholePesoInput {
    pub amount: Option<WholePeso>,
    pub raw: String,
}

impl WholePesoInput {
    pub fn blank() -> Self {
        Self::default()
    }

    pub fn from_amount(amount: WholePeso) -> Self {
        Self {
            amount: Some(amount),
            raw: amount.to_string(),
        }
    }

    pub fn value_or_zero(&self) -> WholePeso {
        self.amount.unwrap_or_default()
    }

    pub fn set(&mut self, amount: WholePeso) {
        *self = Self::from_amount(amount);
    }
}

/// Percentage stored to hundredths of one percent while preserving its source
/// lexeme (for example `0.0` versus `0.00`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PercentInput {
    pub hundredths: Option<i32>,
    pub raw: String,
}

impl PercentInput {
    pub fn blank() -> Self {
        Self::default()
    }

    pub fn from_hundredths(hundredths: i32) -> Self {
        Self {
            hundredths: Some(hundredths),
            raw: if hundredths % 100 == 0 {
                format!("{}.0", hundredths / 100)
            } else {
                format!(
                    "{}.{:02}",
                    hundredths / 100,
                    hundredths.unsigned_abs() % 100
                )
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form1702MXFilingBasis {
    #[default]
    Calendar,
    Fiscal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form1702MXDeductionMethod {
    #[default]
    Unresolved,
    Itemized,
    OptionalStandard,
}

impl Form1702MXDeductionMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unresolved => "Deduction method needs review",
            Self::Itemized => "Itemized deduction",
            Self::OptionalStandard => "Optional Standard Deduction (40%)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1702MXOverpaymentDisposition {
    Refund,
    TaxCreditCertificate,
    CarryOver,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXAtcSelection {
    pub mcit_selected: bool,
    pub other_selected: bool,
    pub other_code: String,
}

/// Amounts for the four printed Part IV tax-regime columns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXRegimeAmounts {
    pub exempt: WholePesoInput,
    pub special: WholePesoInput,
    pub regular: WholePesoInput,
    pub total: WholePesoInput,
}

impl Form1702MXRegimeAmounts {
    fn recompute_total(&mut self, field: &str, issues: &mut Vec<(String, String)>) {
        self.total.set(checked_sum(
            field,
            [
                self.exempt.value_or_zero(),
                self.special.value_or_zero(),
                self.regular.value_or_zero(),
            ],
            issues,
        ));
    }

    fn column(&self, index: usize) -> WholePeso {
        match index {
            0 => self.exempt.value_or_zero(),
            1 => self.special.value_or_zero(),
            2 => self.regular.value_or_zero(),
            _ => self.total.value_or_zero(),
        }
    }

    fn column_mut(&mut self, index: usize) -> &mut WholePesoInput {
        match index {
            0 => &mut self.exempt,
            1 => &mut self.special,
            2 => &mut self.regular,
            _ => &mut self.total,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXPartII {
    pub item_14_total_tax_due_or_overpayment: WholePeso,
    pub item_15_total_tax_credits: WholePeso,
    pub item_16_net_tax_payable_or_overpayment: WholePeso,
    pub item_17_surcharge: WholePesoInput,
    pub item_18_interest: WholePesoInput,
    pub item_19_compromise: WholePesoInput,
    pub item_20_total_penalties: WholePesoInput,
    pub item_21_total_amount_payable_or_overpayment: WholePesoInput,
    pub overpayment_disposition: Option<Form1702MXOverpaymentDisposition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXReliefBasis {
    pub instruction_single_activity: bool,
    pub instruction_multiple_activities: bool,
    pub special_tax_rate: PercentInput,
}

/// Schedule 2 has exactly nineteen printed rows and four regime columns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXSchedule2 {
    pub items: [Form1702MXRegimeAmounts; 19],
    pub item_14_special_rate: PercentInput,
    pub item_14_regular_rate: PercentInput,
}

/// Schedule 3 has Items 20 through 33 (fourteen fixed rows).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXSchedule3 {
    pub items_20_to_33: [Form1702MXRegimeAmounts; 14],
    pub item_30_description: String,
    pub item_31_description: String,
}

/// Schedule 4 has exactly seven printed rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXSchedule4 {
    pub items: [Form1702MXRegimeAmounts; 7],
}

/// Schedule 5 contains Items 1-16, 17a-17c, six fixed 17d-17i rows,
/// and Item 18 total: twenty-six fixed amount rows in all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXSchedule5 {
    pub amounts: [Form1702MXRegimeAmounts; 26],
    pub other_descriptions_17d_to_17i: [String; 6],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXSpecialDeductionRow {
    pub description: String,
    pub legal_basis: String,
    pub amounts: Form1702MXRegimeAmounts,
}

/// Schedule 6 has exactly four input rows and one printed total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXSchedule6 {
    pub rows: [Form1702MXSpecialDeductionRow; 4],
    pub item_5_total: Form1702MXRegimeAmounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXNolcoRow {
    pub year_incurred: String,
    pub amount: WholePesoInput,
    pub applied_previous_years: WholePesoInput,
    pub expired: WholePesoInput,
    pub applied_current_year: WholePesoInput,
    pub unapplied: WholePesoInput,
}

/// Schedules 7.1 and 8.1 each have four fixed NOLCO rows (Items 4-7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXNolcoTable {
    pub rows: [Form1702MXNolcoRow; 4],
    pub item_8_total_applied_current_year: WholePesoInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXNolcoComputation {
    pub item_1_gross_income: WholePesoInput,
    pub item_2_ordinary_itemized_deductions: WholePesoInput,
    pub item_3_net_operating_loss: WholePesoInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXMcitRow {
    pub year: String,
    pub normal_income_tax: WholePesoInput,
    pub mcit: WholePesoInput,
    pub excess_mcit: WholePesoInput,
    pub applied_previous_years: WholePesoInput,
    pub expired: WholePesoInput,
    pub applied_current_year: WholePesoInput,
    pub balance: WholePesoInput,
}

/// Schedule 9 has exactly three printed MCIT rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXSchedule9 {
    pub rows: [Form1702MXMcitRow; 3],
    pub item_4_total_applied_current_year: WholePesoInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXSchedule10 {
    pub items: [Form1702MXRegimeAmounts; 10],
    pub descriptions: [String; 10],
}

/// Fields belonging to the separate two-page mandatory attachment. They are
/// preserved for audit, but are never treated as pages five and six of the
/// four-page base return and cannot be electronically transported by this app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXMandatoryAttachment {
    pub current_index: String,
    pub total_count: String,
    pub exempt_activity: bool,
    pub special_rate_activity: bool,
    pub schedule_a_effectivity_from: String,
    pub schedule_a_effectivity_until: String,
    pub schedule_d_other_description: String,
    pub schedule_f_year: String,
    pub descriptions_20_to_24: [String; 5],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXPaymentDetail {
    pub particulars: String,
    pub drawee: String,
    pub number: String,
    pub date_or_amount: String,
}

/// Complete local draft for exact revision `1702MXv2018C`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Form1702MXDraft {
    pub id: Option<i64>,
    pub tin: String,
    pub taxable_year: u16,
    pub month: u8,
    pub filing_basis: Form1702MXFilingBasis,
    pub is_amended: bool,
    pub is_short_period: bool,
    pub atc: Form1702MXAtcSelection,
    pub rdo_code: String,
    pub taxpayer_name: String,
    pub registered_name_lines: [String; 3],
    pub registered_address: String,
    pub registered_address_lines: [String; 3],
    pub zip_code: String,
    pub incorporation_date: String,
    pub contact_number: String,
    pub email: String,
    pub deduction_method: Form1702MXDeductionMethod,
    pub part_ii: Form1702MXPartII,
    pub relief_basis: Form1702MXReliefBasis,
    pub schedule_2: Form1702MXSchedule2,
    pub schedule_3: Form1702MXSchedule3,
    pub schedule_4: Form1702MXSchedule4,
    pub schedule_5: Form1702MXSchedule5,
    pub schedule_6: Form1702MXSchedule6,
    pub regular_nolco: Form1702MXNolcoComputation,
    pub schedule_7_1: Form1702MXNolcoTable,
    pub special_nolco: Form1702MXNolcoComputation,
    pub schedule_8_1: Form1702MXNolcoTable,
    pub schedule_9: Form1702MXSchedule9,
    pub schedule_10: Form1702MXSchedule10,
    pub mandatory_attachment: Form1702MXMandatoryAttachment,
    pub authorized_representative: String,
    pub treasurer: String,
    pub number_of_attachments: String,
    pub president_title: String,
    pub president_tin: String,
    pub treasurer_title: String,
    pub treasurer_tin: String,
    pub payment_details: [Form1702MXPaymentDetail; 4],
    pub xml_final_flag: String,
    #[serde(default)]
    pub preserved_xml_fields: BTreeMap<String, String>,
    #[serde(default)]
    pub calculation_issues: Vec<(String, String)>,
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

impl Form1702MXDraft {
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let name_lines = split_fixed_lines(&profile.full_name);
        let address_lines = split_fixed_lines(&profile.registered_address);
        Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            month: 12,
            filing_basis: Form1702MXFilingBasis::Calendar,
            is_amended: false,
            is_short_period: false,
            atc: Form1702MXAtcSelection {
                mcit_selected: false,
                other_selected: false,
                other_code: String::new(),
            },
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_name_lines: name_lines,
            registered_address: profile.registered_address.clone(),
            registered_address_lines: address_lines,
            zip_code: profile.zip_code.clone(),
            incorporation_date: String::new(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            deduction_method: Form1702MXDeductionMethod::Unresolved,
            part_ii: Form1702MXPartII::default(),
            relief_basis: Form1702MXReliefBasis::default(),
            schedule_2: Form1702MXSchedule2::default(),
            schedule_3: Form1702MXSchedule3::default(),
            schedule_4: Form1702MXSchedule4::default(),
            schedule_5: Form1702MXSchedule5::default(),
            schedule_6: Form1702MXSchedule6::default(),
            regular_nolco: Form1702MXNolcoComputation::default(),
            schedule_7_1: Form1702MXNolcoTable::default(),
            special_nolco: Form1702MXNolcoComputation::default(),
            schedule_8_1: Form1702MXNolcoTable::default(),
            schedule_9: Form1702MXSchedule9::default(),
            schedule_10: Form1702MXSchedule10::default(),
            mandatory_attachment: Form1702MXMandatoryAttachment::default(),
            authorized_representative: String::new(),
            treasurer: String::new(),
            number_of_attachments: "00".to_string(),
            president_title: String::new(),
            president_tin: String::new(),
            treasurer_title: String::new(),
            treasurer_tin: String::new(),
            payment_details: std::array::from_fn(|_| Form1702MXPaymentDetail::default()),
            xml_final_flag: "1".to_string(),
            preserved_xml_fields: BTreeMap::new(),
            calculation_issues: Vec::new(),
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

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    /// Recompute only formulas printed on the reviewed January 2018 form.
    pub fn recompute(&mut self) {
        self.calculation_issues.clear();

        for (index, item) in self.schedule_2.items.iter_mut().enumerate() {
            item.recompute_total(
                &format!("schedule_2.items[{}].total", index + 1),
                &mut self.calculation_issues,
            );
        }
        for (index, item) in self.schedule_3.items_20_to_33.iter_mut().enumerate() {
            item.recompute_total(
                &format!("schedule_3.item_{}.total", index + 20),
                &mut self.calculation_issues,
            );
        }
        for (index, item) in self.schedule_4.items.iter_mut().enumerate() {
            item.recompute_total(
                &format!("schedule_4.items[{}].total", index + 1),
                &mut self.calculation_issues,
            );
        }
        for (index, item) in self.schedule_5.amounts.iter_mut().enumerate() {
            item.recompute_total(
                &format!("schedule_5.amounts[{index}].total"),
                &mut self.calculation_issues,
            );
        }
        for (index, row) in self.schedule_6.rows.iter_mut().enumerate() {
            row.amounts.recompute_total(
                &format!("schedule_6.rows[{index}].amounts.total"),
                &mut self.calculation_issues,
            );
        }
        for (index, item) in self.schedule_10.items.iter_mut().enumerate() {
            item.recompute_total(
                &format!("schedule_10.items[{}].total", index + 1),
                &mut self.calculation_issues,
            );
        }

        self.recompute_schedule_2();
        self.recompute_schedule_3();
        self.recompute_schedule_4();
        self.recompute_schedule_5_and_6();
        recompute_nolco_table(
            "schedule_7_1",
            &mut self.schedule_7_1,
            &mut self.calculation_issues,
        );
        recompute_nolco_table(
            "schedule_8_1",
            &mut self.schedule_8_1,
            &mut self.calculation_issues,
        );
        recompute_mcit(&mut self.schedule_9, &mut self.calculation_issues);
        self.recompute_schedule_10();

        self.part_ii.item_14_total_tax_due_or_overpayment =
            self.schedule_2.items[18].total.value_or_zero();
        self.part_ii.item_15_total_tax_credits =
            self.schedule_3.items_20_to_33[12].total.value_or_zero();
        self.part_ii.item_16_net_tax_payable_or_overpayment =
            self.schedule_3.items_20_to_33[13].total.value_or_zero();
        self.part_ii.item_20_total_penalties.set(checked_sum(
            "part_ii.item_20_total_penalties",
            [
                self.part_ii.item_17_surcharge.value_or_zero(),
                self.part_ii.item_18_interest.value_or_zero(),
                self.part_ii.item_19_compromise.value_or_zero(),
            ],
            &mut self.calculation_issues,
        ));
        self.part_ii
            .item_21_total_amount_payable_or_overpayment
            .set(checked_sum(
                "part_ii.item_21_total_amount_payable_or_overpayment",
                [
                    self.part_ii.item_16_net_tax_payable_or_overpayment,
                    self.part_ii.item_20_total_penalties.value_or_zero(),
                ],
                &mut self.calculation_issues,
            ));
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    fn recompute_schedule_2(&mut self) {
        for column in 0..4 {
            let item_3 = checked_sub(
                "schedule_2.item_3",
                self.schedule_2.items[0].column(column),
                self.schedule_2.items[1].column(column),
                &mut self.calculation_issues,
            );
            self.schedule_2.items[2].column_mut(column).set(item_3);
            let item_5 = checked_sub(
                "schedule_2.item_5",
                item_3,
                self.schedule_2.items[3].column(column),
                &mut self.calculation_issues,
            );
            self.schedule_2.items[4].column_mut(column).set(item_5);
            let item_7 = checked_sum(
                "schedule_2.item_7",
                [item_5, self.schedule_2.items[5].column(column)],
                &mut self.calculation_issues,
            );
            self.schedule_2.items[6].column_mut(column).set(item_7);
            let item_11 = checked_sum(
                "schedule_2.item_11",
                [
                    self.schedule_2.items[7].column(column),
                    self.schedule_2.items[8].column(column),
                    self.schedule_2.items[9].column(column),
                ],
                &mut self.calculation_issues,
            );
            self.schedule_2.items[10].column_mut(column).set(item_11);
            let item_12 = if matches!(
                self.deduction_method,
                Form1702MXDeductionMethod::OptionalStandard
            ) {
                checked_percent(
                    "schedule_2.item_12",
                    item_7,
                    4_000,
                    &mut self.calculation_issues,
                )
            } else {
                WholePeso::default()
            };
            self.schedule_2.items[11].column_mut(column).set(item_12);
            let deduction = if matches!(
                self.deduction_method,
                Form1702MXDeductionMethod::OptionalStandard
            ) {
                item_12
            } else {
                item_11
            };
            self.schedule_2.items[12]
                .column_mut(column)
                .set(checked_sub(
                    "schedule_2.item_13",
                    item_7,
                    deduction,
                    &mut self.calculation_issues,
                ));
            let item_17 = checked_sub(
                "schedule_2.item_17",
                self.schedule_2.items[14].column(column),
                self.schedule_2.items[15].column(column),
                &mut self.calculation_issues,
            );
            self.schedule_2.items[16].column_mut(column).set(item_17);
        }

        let mcit = checked_percent(
            "schedule_2.item_18.regular",
            self.schedule_2.items[6].regular.value_or_zero(),
            200,
            &mut self.calculation_issues,
        );
        self.schedule_2.items[17].regular.set(mcit);
        let special_tax = self.schedule_2.items[16].special.value_or_zero();
        self.schedule_2.items[18].special.set(special_tax);
        let regular_tax = std::cmp::max(
            self.schedule_2.items[14].regular.value_or_zero(),
            self.schedule_2.items[17].regular.value_or_zero(),
        );
        self.schedule_2.items[18].regular.set(regular_tax);
        self.schedule_2.items[18].total.set(checked_sum(
            "schedule_2.item_19.total",
            [special_tax, regular_tax],
            &mut self.calculation_issues,
        ));
    }

    fn recompute_schedule_3(&mut self) {
        for column in 0..4 {
            let total = checked_sum(
                "schedule_3.item_32",
                (0..12).map(|index| self.schedule_3.items_20_to_33[index].column(column)),
                &mut self.calculation_issues,
            );
            self.schedule_3.items_20_to_33[12]
                .column_mut(column)
                .set(total);
            self.schedule_3.items_20_to_33[13]
                .column_mut(column)
                .set(checked_sub(
                    "schedule_3.item_33",
                    self.schedule_2.items[18].column(column),
                    total,
                    &mut self.calculation_issues,
                ));
        }
    }

    fn recompute_schedule_4(&mut self) {
        for column in 0..4 {
            let item_3 = checked_sum(
                "schedule_4.item_3",
                [
                    self.schedule_4.items[0].column(column),
                    self.schedule_4.items[1].column(column),
                ],
                &mut self.calculation_issues,
            );
            self.schedule_4.items[2].column_mut(column).set(item_3);
            let item_5 = checked_sub(
                "schedule_4.item_5",
                item_3,
                self.schedule_4.items[3].column(column),
                &mut self.calculation_issues,
            );
            self.schedule_4.items[4].column_mut(column).set(item_5);
            let item_6 = self.schedule_4.items[5].column(column);
            let item_7 = checked_sum(
                "schedule_4.item_7",
                [item_5, item_6],
                &mut self.calculation_issues,
            );
            self.schedule_4.items[6].column_mut(column).set(item_7);
        }
    }

    fn recompute_schedule_5_and_6(&mut self) {
        for column in 0..4 {
            let item_18 = checked_sum(
                "schedule_5.item_18",
                (0..25).map(|index| self.schedule_5.amounts[index].column(column)),
                &mut self.calculation_issues,
            );
            self.schedule_5.amounts[25].column_mut(column).set(item_18);
            let schedule_6_total = checked_sum(
                "schedule_6.item_5",
                self.schedule_6
                    .rows
                    .iter()
                    .map(|row| row.amounts.column(column)),
                &mut self.calculation_issues,
            );
            self.schedule_6
                .item_5_total
                .column_mut(column)
                .set(schedule_6_total);
        }
    }

    fn recompute_schedule_10(&mut self) {
        for column in 0..4 {
            let item_4 = checked_sum(
                "schedule_10.item_4",
                (0..3).map(|index| self.schedule_10.items[index].column(column)),
                &mut self.calculation_issues,
            );
            self.schedule_10.items[3].column_mut(column).set(item_4);
            let item_9 = checked_sum(
                "schedule_10.item_9",
                (4..8).map(|index| self.schedule_10.items[index].column(column)),
                &mut self.calculation_issues,
            );
            self.schedule_10.items[8].column_mut(column).set(item_9);
            self.schedule_10.items[9]
                .column_mut(column)
                .set(checked_sub(
                    "schedule_10.item_10",
                    item_4,
                    item_9,
                    &mut self.calculation_issues,
                ));
        }
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        Err(vec![(
            "submission".to_string(),
            "1702MXv2018C supports local editable-save persistence only; the encrypted electronic-submission and mandatory-attachment transport contracts are unreviewed".to_string(),
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

impl FormValidator for Form1702MXDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        let tin_digits = self
            .tin
            .chars()
            .filter(|character| character.is_ascii_digit())
            .collect::<String>();
        if !(12..=14).contains(&tin_digits.len())
            || self.tin.chars().any(|c| !c.is_ascii_digit() && c != '-')
        {
            errors.push((
                "tin".to_string(),
                "TIN must contain 12 to 14 digits, optionally separated by dashes".to_string(),
            ));
        }
        if !(1900..=2200).contains(&self.taxable_year) {
            errors.push((
                "taxable_year".to_string(),
                "Taxable year is outside the supported range".to_string(),
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
        if !self.atc.mcit_selected && !self.atc.other_selected {
            errors.push((
                "atc".to_string(),
                "Item 5 requires at least one reviewed ATC selection".to_string(),
            ));
        }
        if self.atc.other_selected && self.atc.other_code.trim().is_empty() {
            errors.push((
                "atc.other_code".to_string(),
                "The alternate ATC selection requires an exact code".to_string(),
            ));
        }
        if matches!(self.deduction_method, Form1702MXDeductionMethod::Unresolved) {
            errors.push((
                "deduction_method".to_string(),
                "Item 13 method of deduction must be selected".to_string(),
            ));
        }
        if self.relief_basis.instruction_single_activity
            && self.relief_basis.instruction_multiple_activities
        {
            errors.push((
                "relief_basis.instructions".to_string(),
                "Part IV instruction A and B cannot both be selected".to_string(),
            ));
        }
        if self.relief_basis.instruction_multiple_activities {
            errors.push((
                "mandatory_attachment".to_string(),
                "Multiple exempt/special activities require the separate two-page mandatory attachment; attachment transport is not implemented".to_string(),
            ));
        }
        if self
            .part_ii
            .item_21_total_amount_payable_or_overpayment
            .value_or_zero()
            .0
            < 0
            && self.part_ii.overpayment_disposition.is_none()
        {
            errors.push((
                "part_ii.overpayment_disposition".to_string(),
                "An overpayment requires exactly one irrevocable disposition".to_string(),
            ));
        }
        if self
            .part_ii
            .item_21_total_amount_payable_or_overpayment
            .value_or_zero()
            .0
            >= 0
            && self.part_ii.overpayment_disposition.is_some()
        {
            errors.push((
                "part_ii.overpayment_disposition".to_string(),
                "Overpayment disposition applies only when Item 21 is negative".to_string(),
            ));
        }
        if !matches!(self.xml_final_flag.as_str(), "0" | "1") {
            errors.push((
                "xml_final_flag".to_string(),
                "txtFinalFlag must be one of the two observed values 0 or 1".to_string(),
            ));
        }
        errors.extend(self.calculation_issues.clone());
        errors
    }
}

impl TypedBirForm for Form1702MXDraft {
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
        Form1702MXDraft::recompute(self);
    }

    fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        Form1702MXDraft::to_bir_field_map(self)
    }
}

fn split_fixed_lines(value: &str) -> [String; 3] {
    let mut result = std::array::from_fn(|_| String::new());
    result[0] = value.to_string();
    result
}

fn checked_sum(
    field: &str,
    values: impl IntoIterator<Item = WholePeso>,
    issues: &mut Vec<(String, String)>,
) -> WholePeso {
    let value = values
        .into_iter()
        .fold(0_i128, |sum, value| sum + i128::from(value.0));
    checked_i128(field, value, issues)
}

fn checked_sub(
    field: &str,
    left: WholePeso,
    right: WholePeso,
    issues: &mut Vec<(String, String)>,
) -> WholePeso {
    checked_i128(field, i128::from(left.0) - i128::from(right.0), issues)
}

fn checked_percent(
    field: &str,
    amount: WholePeso,
    rate_hundredths: i32,
    issues: &mut Vec<(String, String)>,
) -> WholePeso {
    let numerator = i128::from(amount.0) * i128::from(rate_hundredths);
    let rounded = if numerator >= 0 {
        (numerator + 5_000) / 10_000
    } else {
        (numerator - 5_000) / 10_000
    };
    checked_i128(field, rounded, issues)
}

fn checked_i128(field: &str, value: i128, issues: &mut Vec<(String, String)>) -> WholePeso {
    match i64::try_from(value) {
        Ok(value) => WholePeso(value),
        Err(_) => {
            issues.push((
                field.to_string(),
                "Whole-peso calculation overflowed the supported signed range".to_string(),
            ));
            WholePeso::default()
        }
    }
}

fn recompute_nolco_table(
    field: &str,
    table: &mut Form1702MXNolcoTable,
    issues: &mut Vec<(String, String)>,
) {
    for (index, row) in table.rows.iter_mut().enumerate() {
        let used = checked_sum(
            &format!("{field}.rows[{index}].used"),
            [
                row.applied_previous_years.value_or_zero(),
                row.expired.value_or_zero(),
                row.applied_current_year.value_or_zero(),
            ],
            issues,
        );
        row.unapplied.set(checked_sub(
            &format!("{field}.rows[{index}].unapplied"),
            row.amount.value_or_zero(),
            used,
            issues,
        ));
    }
    table.item_8_total_applied_current_year.set(checked_sum(
        &format!("{field}.item_8_total_applied_current_year"),
        table
            .rows
            .iter()
            .map(|row| row.applied_current_year.value_or_zero()),
        issues,
    ));
}

fn recompute_mcit(schedule: &mut Form1702MXSchedule9, issues: &mut Vec<(String, String)>) {
    for (index, row) in schedule.rows.iter_mut().enumerate() {
        row.excess_mcit.set(checked_sub(
            &format!("schedule_9.rows[{index}].excess_mcit"),
            row.mcit.value_or_zero(),
            row.normal_income_tax.value_or_zero(),
            issues,
        ));
        let used = checked_sum(
            &format!("schedule_9.rows[{index}].used"),
            [
                row.applied_previous_years.value_or_zero(),
                row.expired.value_or_zero(),
                row.applied_current_year.value_or_zero(),
            ],
            issues,
        );
        row.balance.set(checked_sub(
            &format!("schedule_9.rows[{index}].balance"),
            row.excess_mcit.value_or_zero(),
            used,
            issues,
        ));
    }
    schedule.item_4_total_applied_current_year.set(checked_sum(
        "schedule_9.item_4_total_applied_current_year",
        schedule
            .rows
            .iter()
            .map(|row| row.applied_current_year.value_or_zero()),
        issues,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printed_penalty_total_preserves_signed_whole_pesos() {
        let part = Form1702MXPartII {
            item_17_surcharge: WholePesoInput::from_amount(WholePeso(1_000)),
            item_18_interest: WholePesoInput::from_amount(WholePeso(500)),
            item_19_compromise: WholePesoInput::from_amount(WholePeso(250)),
            ..Form1702MXPartII::default()
        };
        let total = checked_sum(
            "test",
            [
                part.item_17_surcharge.value_or_zero(),
                part.item_18_interest.value_or_zero(),
                part.item_19_compromise.value_or_zero(),
            ],
            &mut Vec::new(),
        );
        assert_eq!(total, WholePeso(1_750));
    }

    #[test]
    fn schedule_capacities_match_the_four_page_form() {
        let draft = test_draft();
        assert_eq!(OFFICIAL_BASE_PAGE_COUNT, 4);
        assert_eq!(OFFICIAL_ATTACHMENT_PAGE_COUNT, 2);
        assert_eq!(
            (
                draft.schedule_2.items.len(),
                draft.schedule_3.items_20_to_33.len(),
                draft.schedule_6.rows.len(),
                draft.schedule_7_1.rows.len(),
                draft.schedule_8_1.rows.len(),
                draft.schedule_9.rows.len(),
            ),
            (19, 14, 4, 4, 4, 3)
        );
    }

    #[test]
    fn recompute_follows_reviewed_schedule_and_part_two_cross_references() {
        let mut draft = test_draft();
        draft.deduction_method = Form1702MXDeductionMethod::OptionalStandard;
        draft.schedule_2.items[0].regular.set(WholePeso(1_000_000));
        draft.schedule_2.items[1].regular.set(WholePeso(100_000));
        draft.schedule_2.items[3].regular.set(WholePeso(300_000));
        draft.schedule_2.items[5].regular.set(WholePeso(100_000));
        draft.schedule_2.items[14].regular.set(WholePeso(100_000));
        draft.schedule_2.items[15].regular.set(WholePeso(10_000));
        draft.schedule_3.items_20_to_33[0]
            .regular
            .set(WholePeso(20_000));
        draft.part_ii.item_17_surcharge.set(WholePeso(1_000));
        draft.part_ii.item_18_interest.set(WholePeso(500));
        draft.part_ii.item_19_compromise.set(WholePeso(250));

        draft.recompute();

        assert_eq!(
            draft.schedule_2.items[2].regular.value_or_zero(),
            WholePeso(900_000)
        );
        assert_eq!(
            draft.schedule_2.items[6].regular.value_or_zero(),
            WholePeso(700_000)
        );
        assert_eq!(
            draft.schedule_2.items[11].regular.value_or_zero(),
            WholePeso(280_000)
        );
        assert_eq!(
            draft.schedule_2.items[12].regular.value_or_zero(),
            WholePeso(420_000)
        );
        assert_eq!(
            draft.schedule_2.items[17].regular.value_or_zero(),
            WholePeso(14_000)
        );
        assert_eq!(
            draft.schedule_2.items[18].total.value_or_zero(),
            WholePeso(100_000)
        );
        assert_eq!(
            draft.schedule_3.items_20_to_33[12].total.value_or_zero(),
            WholePeso(20_000)
        );
        assert_eq!(
            draft.schedule_3.items_20_to_33[13].total.value_or_zero(),
            WholePeso(80_000)
        );
        assert_eq!(
            draft.part_ii.item_14_total_tax_due_or_overpayment,
            WholePeso(100_000)
        );
        assert_eq!(draft.part_ii.item_15_total_tax_credits, WholePeso(20_000));
        assert_eq!(
            draft.part_ii.item_16_net_tax_payable_or_overpayment,
            WholePeso(80_000)
        );
        assert_eq!(
            draft
                .part_ii
                .item_21_total_amount_payable_or_overpayment
                .value_or_zero(),
            WholePeso(81_750)
        );
        assert!(draft.calculation_issues.is_empty());
    }

    #[test]
    fn submission_and_multiple_activity_attachment_fail_closed() {
        let mut draft = test_draft();
        draft.relief_basis.instruction_multiple_activities = true;
        assert!(draft.validate().iter().any(|(field, message)| {
            field == "mandatory_attachment"
                && message.contains("attachment transport is not implemented")
        }));
        const {
            assert!(!QUEUE_SUBMISSION_SUPPORTED);
            assert!(!MANDATORY_ATTACHMENT_TRANSPORT_SUPPORTED);
        }
        assert!(draft.transition_to_queued().is_err());
        assert_eq!(draft.status, FilingStatus::Draft);
    }

    fn test_draft() -> Form1702MXDraft {
        Form1702MXDraft {
            id: None,
            tin: "00000000000000".to_string(),
            taxable_year: 2025,
            month: 12,
            filing_basis: Form1702MXFilingBasis::Calendar,
            is_amended: false,
            is_short_period: false,
            atc: Form1702MXAtcSelection::default(),
            rdo_code: "018".to_string(),
            taxpayer_name: "TEST".to_string(),
            registered_name_lines: std::array::from_fn(|_| String::new()),
            registered_address: "TEST".to_string(),
            registered_address_lines: std::array::from_fn(|_| String::new()),
            zip_code: String::new(),
            incorporation_date: String::new(),
            contact_number: String::new(),
            email: String::new(),
            deduction_method: Form1702MXDeductionMethod::Unresolved,
            part_ii: Form1702MXPartII::default(),
            relief_basis: Form1702MXReliefBasis::default(),
            schedule_2: Form1702MXSchedule2::default(),
            schedule_3: Form1702MXSchedule3::default(),
            schedule_4: Form1702MXSchedule4::default(),
            schedule_5: Form1702MXSchedule5::default(),
            schedule_6: Form1702MXSchedule6::default(),
            regular_nolco: Form1702MXNolcoComputation::default(),
            schedule_7_1: Form1702MXNolcoTable::default(),
            special_nolco: Form1702MXNolcoComputation::default(),
            schedule_8_1: Form1702MXNolcoTable::default(),
            schedule_9: Form1702MXSchedule9::default(),
            schedule_10: Form1702MXSchedule10::default(),
            mandatory_attachment: Form1702MXMandatoryAttachment::default(),
            authorized_representative: String::new(),
            treasurer: String::new(),
            number_of_attachments: String::new(),
            president_title: String::new(),
            president_tin: String::new(),
            treasurer_title: String::new(),
            treasurer_tin: String::new(),
            payment_details: std::array::from_fn(|_| Form1702MXPaymentDetail::default()),
            xml_final_flag: "1".to_string(),
            preserved_xml_fields: BTreeMap::new(),
            calculation_issues: Vec::new(),
            status: FilingStatus::Draft,
            created_at: String::new(),
            updated_at: String::new(),
            submitted_at: None,
            confirmed_at: None,
            submission_filename: None,
            receipt_id: None,
            submission_attempts: 0,
            next_retry_at: None,
            last_error: None,
        }
    }
}
