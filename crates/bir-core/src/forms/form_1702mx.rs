//! Semantic domain for exact form `1702MXv2018C`.
//!
//! The reviewed source set contains a four-page January 2018 return and a
//! separate two-page mandatory-attachment document. This model keeps those
//! documents distinct. It supports local editable-save persistence only; no
//! electronic-submission or attachment-transport contract has been reviewed.

use super::penalty_items::PenaltyState;
use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::forms::queueable::SubmissionLifecycle;
use crate::forms::{FilingPeriod, FormValidator, TypedBirForm};
use crate::profile::TaxpayerProfile;

pub const FORM_CODE: &str = "1702MX";
pub const FORM_TYPE_ID: &str = "1702MXv2018C";
pub const OFFICIAL_BASE_PAGE_COUNT: usize = 4;
pub const OFFICIAL_ATTACHMENT_PAGE_COUNT: usize = 2;
pub const QUEUE_SUBMISSION_SUPPORTED: bool = true;
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

/// Items 17-19 hold whole pesos. A blank item that files zero stays blank,
/// so its source lexeme round-trips.
impl super::penalty_items::PenaltyField for WholePesoInput {
    fn penalty_value(&self) -> f64 {
        self.value_or_zero().0 as f64
    }
    fn set_penalty_value(&mut self, value: f64) {
        let value = value.round() as i64;
        if value != self.value_or_zero().0 {
            self.set(WholePeso(value));
        }
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

/// Schedule 6 has four input rows and one printed total. Item 4's
/// "(Add more...)" modal (`modalTablePg3Sc6`) adds rows 4.1, 4.2, ... without
/// a limit; with two or more saved rows Item 4 reads OTHERS and the modal
/// subtotals (`Save_modalTablePg3Sc6`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXSchedule6 {
    pub rows: [Form1702MXSpecialDeductionRow; 4],
    pub item_5_total: Form1702MXRegimeAmounts,
    /// The modal's rows in page order; row 1 is the Item 4 entry the link
    /// transfers. Empty when the modal is not used.
    #[serde(default)]
    pub more_rows: Vec<Form1702MXSpecialDeductionRow>,
}

impl Form1702MXSchedule6 {
    /// Two or more saved modal rows: Item 4 holds OTHERS and the subtotals.
    pub fn is_folded(&self) -> bool {
        self.more_rows.len() >= 2
    }
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Items 17-19's computed values, overrides, note and engine inputs
    /// (the shared model in [`super::penalty_items`]).
    #[serde(flatten)]
    pub penalty: PenaltyState,
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
    /// Part IV Schedule 1 Items 1-3 and 5-6 (agency, legal basis, registered
    /// activity, effectivity from / until) for columns A, B and C.
    #[serde(default)]
    pub relief_details: Form1702MXReliefDetails,
    /// Pre-queue builds stored their own error text here. Kept so old JSON
    /// round-trips; the generic lifecycle reports `submission_error`.
    #[serde(default)]
    pub last_error: Option<String>,
    /// Status, queue authorization and retry state. Flattened so stored JSON
    /// keeps the keys earlier builds wrote inline (`status`, `created_at`,
    /// `submission_attempts`, ...).
    #[serde(flatten)]
    pub lifecycle: SubmissionLifecycle,
}

/// Part IV Schedule 1 text rows: Items 1, 2, 3, 5 and 6, columns A-C.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Form1702MXReliefDetails {
    pub investment_promotion_agency: [String; 3],
    pub legal_basis: [String; 3],
    pub registered_activity: [String; 3],
    /// MM/DD/YYYY.
    pub effectivity_from: [String; 3],
    /// MM/DD/YYYY.
    pub effectivity_until: [String; 3],
}

impl Form1702MXDraft {
    /// The print sheet's field map: the upload map plus `derived:` keys
    /// that only shape the print.
    ///
    /// - `derived:tin_digits`: the nine TIN digits of the page 2-4 header
    ///   combs (the sheet preprints the branch 00000 after them).
    /// - `derived:year_end_mm1` / `_mm2` / `_yy1` / `_yy2`: Item 2 Year
    ///   Ended (MM/20YY) over its four digit boxes.
    pub fn to_print_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = self.to_bir_field_map();
        let tin_digits = super::print_map::digits_of(
            &fields,
            &[
                "frm1702MX:txtPg2TIN1",
                "frm1702MX:txtPg2TIN2",
                "frm1702MX:txtPg2TIN3",
            ],
        );
        fields.insert("derived:tin_digits".to_string(), tin_digits);
        // Item 9's first line prints the registered address the return
        // files there (the official map's `txtPg1Pt1I10RegisteredAddress`),
        // not the first entry of the line list, which the page does not
        // write: a long address prints whole over the item's lines.
        fields.insert(
            "frm1702MX:txtPg1Pt1I10RegisteredAddress".to_string(),
            self.registered_address.trim().to_string(),
        );
        // A later line the address already holds (lines kept from when the
        // address was split over them) is not printed a second time.
        let whole = self.registered_address.trim().to_uppercase();
        for (index, key) in [
            (1, "frm1702MX:txtPg1Pt1I10RegisteredAddress2"),
            (2, "frm1702MX:txtPg1Pt1I10RegisteredAddress3"),
        ] {
            let line = self.registered_address_lines[index].trim().to_uppercase();
            if !line.is_empty() && whole.contains(&line) {
                fields.insert(key.to_string(), String::new());
            }
        }
        for (key, name) in [
            ("frm1702MX:ddlPg1I2Date", "mm"),
            ("frm1702MX:txtPg1I2YearEnd", "yy"),
        ] {
            let digits: Vec<String> = fields
                .get(key)
                .map(|value| value.chars().map(String::from).collect())
                .unwrap_or_default();
            for (at, digit) in digits.iter().take(2).enumerate() {
                fields.insert(format!("derived:year_end_{name}{}", at + 1), digit.clone());
            }
        }
        // The upload map leaves the schedule amount columns and Part II
        // items 14-16 at their empty expected values; the print fills them
        // with whole pesos shaped to each cell's capacity.
        for (key, value, width) in self.print_amount_cells() {
            // Width 0 marks a box the row does not use: it stays blank
            // even though the upload map carries a defaulted "0".
            if width == 0 {
                fields.insert(key, String::new());
            } else {
                fields.insert(key, whole_peso_cell(value, width));
            }
        }
        // Tax rates print over an integer comb and a fraction cell beside
        // the preprinted "." and "%", never as a lone fragment.
        for (stem, rate) in [
            ("derived:rate_sc1", &self.relief_basis.special_tax_rate),
            ("derived:rate_s2b", &self.schedule_2.item_14_special_rate),
            ("derived:rate_s2c", &self.schedule_2.item_14_regular_rate),
        ] {
            if let Some(hundredths) = rate.hundredths {
                if hundredths / 100 < 100 {
                    fields.insert(format!("{stem}_int"), format!("{:>2}", hundredths / 100));
                    let frac = hundredths.unsigned_abs() % 100;
                    let frac_text = if frac % 10 == 0 {
                        format!("{}", frac / 10)
                    } else {
                        format!("{frac:02}")
                    };
                    fields.insert(format!("{stem}_frac"), frac_text);
                }
            }
        }
        fields
    }

    /// The sheet's whole-peso amount cells, one `(key, value, width)`
    /// triple each: `width` is the cell's capacity — the comb slot count
    /// or the text box's character width — so the digits sit right.
    /// Keys follow the writer-cell joins; a column the sheet does not
    /// draw a box for keeps its reviewed `not_joined` reason.
    fn print_amount_cells(&self) -> Vec<(String, i64, usize)> {
        let mut cells: Vec<(String, i64, usize)> = Vec::new();
        // Part II items 14-16, each a twelve-slot comb.
        for (key, value) in [
            (
                "txtPg1Pt2I14TotalIncome",
                self.part_ii.item_14_total_tax_due_or_overpayment,
            ),
            (
                "txtPg1Pt2I15LessTotalTax",
                self.part_ii.item_15_total_tax_credits,
            ),
            (
                "txtPg1Pt2I16NetTaxPayable",
                self.part_ii.item_16_net_tax_payable_or_overpayment,
            ),
        ] {
            cells.push((format!("frm1702MX:{key}"), value.0, 12));
        }
        // Schedule 2 items 1-19 (rows 10-19 spell their key without the m).
        for (index, item) in self.schedule_2.items.iter().enumerate() {
            let n = index + 1;
            let stem = if n < 10 {
                format!("txtPg2Sc2Itm{n}")
            } else {
                format!("txtPg2Sc2It{n}")
            };
            push_regime_cells(&mut cells, stem, item, 12);
        }
        // Schedule 3 items 20-33.
        for (index, item) in self.schedule_3.items_20_to_33.iter().enumerate() {
            push_regime_cells(&mut cells, format!("txtPg2Sc3It{}", index + 20), item, 12);
        }
        // Schedule 4: item 1's boxes sit on the page 3 sheet portion.
        push_regime_cells(
            &mut cells,
            "txtPg3Sc4Itm1".to_string(),
            &self.schedule_4.items[0],
            12,
        );
        for (index, item) in self.schedule_4.items[1..].iter().enumerate() {
            push_regime_cells(&mut cells, format!("txtPg2Sc4Itm{}", index + 2), item, 12);
        }
        // Schedule 5: items 1-16, the 17a-17i lettered rows, item 18.
        for (index, item) in self.schedule_5.amounts.iter().enumerate() {
            let stem = match index {
                0..=15 => format!("txtPg3Sc5Itm{}", index + 1),
                16..=24 => format!("txtPg3Sc5Itm17{}", (b'a' + (index - 16) as u8) as char),
                _ => "txtPg3Sc5Itm18".to_string(),
            };
            push_regime_cells(&mut cells, stem, item, 12);
        }
        // Schedule 6 rows 1-4 and the item 5 total.
        for (index, row) in self.schedule_6.rows.iter().enumerate() {
            let stem = if index < 3 {
                format!("txtPg6Sc6I{}C", index + 1)
            } else {
                "txtPg3Sc6I4C".to_string()
            };
            push_regime_cells(&mut cells, stem, &row.amounts, 12);
        }
        push_regime_cells(
            &mut cells,
            "txtPg6Sc6I5C".to_string(),
            &self.schedule_6.item_5_total,
            12,
        );
        // Schedule 10 items 1-10; item 10's cells are named like a
        // Schedule 2 row on the sheet.
        for (index, item) in self.schedule_10.items.iter().enumerate() {
            let stem = if index < 9 {
                format!("txtPg4Sc10Itm{}", index + 1)
            } else {
                "txtPg2Sc2Itm10".to_string()
            };
            push_regime_cells(&mut cells, stem, item, 12);
        }
        // NOLCO computations, Schedules 7 and 8 items 1-3 (twelve slots).
        for (key, input) in [
            ("txtPg6Sc7I1", &self.regular_nolco.item_1_gross_income),
            (
                "txtPg6Sc7I2",
                &self.regular_nolco.item_2_ordinary_itemized_deductions,
            ),
            ("txtPg6Sc7I3", &self.regular_nolco.item_3_net_operating_loss),
            ("txtPg4Sc8It1", &self.special_nolco.item_1_gross_income),
            (
                "txtPg4Sc8It2",
                &self.special_nolco.item_2_ordinary_itemized_deductions,
            ),
            (
                "txtPg4Sc8It3",
                &self.special_nolco.item_3_net_operating_loss,
            ),
        ] {
            if let Some(value) = input.amount {
                cells.push((format!("frm1702MX:{key}"), value.0, 12));
            }
        }
        // NOLCO tables 7.1 and 8.1, columns A-E (wide text cells) and the
        // item 8 total.
        for (schedule, table) in [(7_u8, &self.schedule_7_1), (8, &self.schedule_8_1)] {
            for (index, row) in table.rows.iter().enumerate() {
                // A NOLCO year row with no amounts is unused; its boxes stay
                // blank rather than printing "0".
                let unused = [
                    &row.amount,
                    &row.applied_previous_years,
                    &row.expired,
                    &row.applied_current_year,
                    &row.unapplied,
                ]
                .iter()
                .all(|input| input.amount.is_none_or(|value| value.0 == 0));
                let item = index + 4;
                let amount_page = if schedule == 7 { 3 } else { 4 };
                if unused {
                    for suffix in ["A", "B", "C", "D", "E"] {
                        let page = if schedule == 8 && index == 2 && suffix == "C" {
                            3
                        } else {
                            amount_page
                        };
                        cells.push((
                            format!("frm1702MX:txtPg{page}IShed{schedule}_{item}{suffix}"),
                            0,
                            0,
                        ));
                    }
                    continue;
                }
                for (suffix, input) in [
                    ("A", &row.amount),
                    ("B", &row.applied_previous_years),
                    ("C", &row.expired),
                    ("D", &row.applied_current_year),
                    ("E", &row.unapplied),
                ] {
                    let page = if schedule == 8 && index == 2 && suffix == "C" {
                        3
                    } else {
                        amount_page
                    };
                    if let Some(value) = input.amount {
                        cells.push((
                            format!("frm1702MX:txtPg{page}IShed{schedule}_{item}{suffix}"),
                            value.0,
                            13,
                        ));
                    }
                }
            }
            if let Some(value) = table.item_8_total_applied_current_year.amount {
                cells.push((format!("frm1702MX:txtPg3IShed{schedule}8D"), value.0, 13));
            }
        }
        // Schedule 9 MCIT rows: columns A-C sit in eleven-slot combs and
        // D-G in nine-slot combs; the year and the item 4 total print in
        // their own cells.
        for (index, row) in self.schedule_9.rows.iter().enumerate() {
            // A row with no year and no amounts is unused; its boxes stay
            // blank rather than printing "0".
            let unused = row.year.trim().is_empty()
                && [
                    &row.normal_income_tax,
                    &row.mcit,
                    &row.excess_mcit,
                    &row.applied_previous_years,
                    &row.expired,
                    &row.applied_current_year,
                    &row.balance,
                ]
                .iter()
                .all(|input| input.amount.is_none_or(|value| value.0 == 0));
            let item = index + 1;
            if unused {
                for suffix in ["CA", "CB", "CC", "CD", "CE", "CF", "CG"] {
                    cells.push((format!("frm1702MX:txtPg7Sc9I{item}{suffix}"), 0, 0));
                }
                continue;
            }
            for (suffix, input) in [
                ("CA", &row.normal_income_tax),
                ("CB", &row.mcit),
                ("CC", &row.excess_mcit),
            ] {
                if let Some(value) = input.amount {
                    cells.push((format!("frm1702MX:txtPg7Sc9I{item}{suffix}"), value.0, 11));
                }
            }
            for (suffix, input) in [
                ("CD", &row.applied_previous_years),
                ("CE", &row.expired),
                ("CF", &row.applied_current_year),
                ("CG", &row.balance),
            ] {
                if let Some(value) = input.amount {
                    cells.push((format!("frm1702MX:txtPg7Sc9I{item}{suffix}"), value.0, 9));
                }
            }
        }
        let schedule_9_all_unused = self
            .schedule_9
            .rows
            .iter()
            .all(|row| row.year.trim().is_empty());
        if let Some(value) = self.schedule_9.item_4_total_applied_current_year.amount {
            if schedule_9_all_unused && value.0 == 0 {
                cells.push(("frm1702MX:txtPg7Sc9I4".to_string(), 0, 0));
            } else {
                cells.push(("frm1702MX:txtPg7Sc9I4".to_string(), value.0, 10));
            }
        }
        cells
    }

    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16) -> Self {
        let name_lines = split_fixed_lines(&profile.full_name);
        let address_lines = split_fixed_lines(&profile.registered_address);
        Self {
            penalty: crate::forms::penalty_items::PenaltyState::for_profile(profile),
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
            relief_details: Form1702MXReliefDetails::default(),
            last_error: None,
            lifecycle: SubmissionLifecycle::default(),
        }
    }

    pub fn is_editable(&self) -> bool {
        self.lifecycle.is_editable()
    }

    /// The official compute chain (see `form_1702mx_official`).
    pub fn recompute(&mut self) {
        self.official_recompute();
    }
}

impl FormValidator for Form1702MXDraft {
    /// The official `validate()` port; see `form_1702mx_official`.
    fn validate(&self) -> Vec<(String, String)> {
        self.official_errors()
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

/// Pushes the four regime columns of one row, skipping blank inputs.
fn push_regime_cells(
    cells: &mut Vec<(String, i64, usize)>,
    stem: String,
    amounts: &Form1702MXRegimeAmounts,
    width: usize,
) {
    // An unused write-in row stores null columns and a defaulted 0 total;
    // its amount boxes stay blank rather than printing "0".
    if [
        &amounts.exempt,
        &amounts.special,
        &amounts.regular,
        &amounts.total,
    ]
    .iter()
    .all(|input| input.amount.is_none_or(|value| value.0 == 0))
    {
        for column in ["A", "B", "C", "D"] {
            cells.push((format!("frm1702MX:{stem}{column}"), 0, 0));
        }
        return;
    }
    for (column, input) in [
        ("A", &amounts.exempt),
        ("B", &amounts.special),
        ("C", &amounts.regular),
        ("D", &amounts.total),
    ] {
        if let Some(value) = input.amount {
            cells.push((format!("frm1702MX:{stem}{column}"), value.0, width));
        }
    }
}

/// A whole-peso amount as the sheet prints it: negative in parentheses,
/// padded so the digits sit at the right edge of a `width`-character box.
/// Comb cells spread one character per slot, so the same padding also
/// right-aligns inside a `width`-slot comb.
fn whole_peso_cell(value: i64, width: usize) -> String {
    let text = if value < 0 {
        format!("({})", value.unsigned_abs())
    } else {
        value.to_string()
    };
    format!("{text:>width$}")
}

fn split_fixed_lines(value: &str) -> [String; 3] {
    let mut result = std::array::from_fn(|_| String::new());
    result[0] = value.to_string();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_capacities_match_the_four_page_form() {
        let draft = Form1702MXSchedule2::default();
        assert_eq!(OFFICIAL_BASE_PAGE_COUNT, 4);
        assert_eq!(OFFICIAL_ATTACHMENT_PAGE_COUNT, 2);
        assert_eq!(
            (
                draft.items.len(),
                Form1702MXSchedule3::default().items_20_to_33.len(),
                Form1702MXSchedule6::default().rows.len(),
                Form1702MXNolcoTable::default().rows.len(),
                Form1702MXSchedule9::default().rows.len(),
            ),
            (19, 14, 4, 4, 3)
        );
        const {
            assert!(!MANDATORY_ATTACHMENT_TRANSPORT_SUPPORTED);
        }
    }
}
