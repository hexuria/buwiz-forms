//! BIR Form 1601C (Monthly Remittance Return of Income Taxes Withheld on Compensation)
//!
//! Data model and auto-computation logic based on 1601Cv2018 ENCS offline forms.

use super::{FilingStatus, FormValidator};
use crate::penalties::{
    PenaltyConfig, PenaltyContext, PenaltyEngine, PenaltyProfile, TaxpayerClass,
};
use crate::profile::TaxpayerProfile;
use crate::validation::{validate_ph_phone, validate_zip};
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// The 1601-C January 2018 XML contract exposes exactly three Schedule I rows.
pub const MAX_SCHEDULE_1_ROWS: usize = 3;

/// Fixed Item 5 ATC for BIR Form 1601-C January 2018.
///
/// Evidence: the pinned official blank PDF
/// `c8faaa71015337a73b4ceb96bfb265c539589ab5e10eb27899bb81f87f417397`
/// prints `WW010`, and the reviewed plain XML sample
/// `794892fc33c0fd7882a91327095f396fb1683d5b3c0d4cb1cb63916f981cad4c`
/// stores `frm1601c:txtATC=WW010`.
pub const FORM_1601C_ATC: &str = "WW010";

/// One official Schedule 1 adjustment row from Part IV of Form 1601-C.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1601CSchedule1Row {
    /// Item 1: previous month in `MM/YYYY` format.
    pub previous_month: String,
    /// Item 2: date paid in `MM/DD/YYYY` format.
    pub date_paid: String,
    /// Item 3: drawee bank/bank code or agency.
    pub drawee_bank_code_or_agency: String,
    /// Item 4: payment reference number.
    pub payment_number: String,
    /// Item 5: tax paid, excluding penalties for the month.
    pub tax_paid: f64,
    /// Item 6: tax that should have been due for the month.
    pub should_be_tax_due: f64,
    /// Item 7: derived adjustment (`Item 6 less Item 5`).
    #[serde(default)]
    pub adjustment: f64,
}

impl Form1601CSchedule1Row {
    /// Recompute the official Item 7 formula using centavo precision.
    pub fn recompute(&mut self) {
        self.adjustment = round_currency(self.should_be_tax_due - self.tax_paid);
    }
}

fn round_currency(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Complete draft or filed return for Form 1601C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form1601CDraft {
    /// Database row ID (None before first save)
    pub id: Option<i64>,

    // === Filing Period ===
    pub tin: String,
    pub taxable_year: u16,
    pub month: u8, // 1–12

    // === Header Options ===
    pub is_amended: bool,
    pub any_taxes_withheld: bool, // Line 3
    pub number_of_sheets: u32,    // Line 4
    pub atc: String,              // Line 5

    // === Part I — pre-filled from profile ===
    pub rdo_code: String,
    pub line_of_business: String,
    pub taxpayer_name: String,
    pub contact_number: String,
    pub registered_address: String,
    /// Optional second address line present in the encrypted 1601-C payload.
    #[serde(default)]
    pub registered_address_2: String,
    pub zip_code: String,
    pub category_of_agent: String, // Item 11: "P" for Private, "G" for Government
    pub email_address: String,

    // === Item 13 — Tax Relief / Treaty ===
    #[serde(default)]
    pub tax_relief: bool,
    #[serde(default)]
    pub tax_relief_specification: String,

    // === Part II — Computation of Tax ===
    #[serde(default)]
    pub tax_14_total_compensation: f64,

    // Less: Non-Taxable/Exempt Compensation
    #[serde(default)]
    pub tax_15_statutory_minimum_wage: f64,
    #[serde(default)]
    pub tax_16_holiday_pay: f64,
    #[serde(default)]
    pub tax_17_13th_month_pay: f64,
    #[serde(default)]
    pub tax_18_de_minimis: f64,
    #[serde(default)]
    pub tax_19_sss_gsis: f64,

    #[serde(default)]
    pub tax_20_other_name: String,
    #[serde(default)]
    pub tax_20_other_amount: f64,

    // Computed Total Non-Taxable Compensation
    #[serde(default)]
    pub tax_21_total_non_taxable: f64,

    // Computed Total Taxable Compensation
    #[serde(default)]
    pub tax_22_total_taxable: f64,

    #[serde(default)]
    pub tax_23_not_subject: f64,

    // Computed Net Taxable Compensation
    #[serde(default)]
    pub tax_24_net_taxable: f64,

    // Total Taxes Withheld
    #[serde(default)]
    pub tax_25_total_taxes_withheld: f64,

    // Add/Less: Adjustment of Taxes Withheld from Previous Months
    #[serde(default)]
    pub tax_26_adjustment: f64,

    /// Part IV Schedule I rows. The verified XML capacity is three rows.
    #[serde(default)]
    pub schedule_1: Vec<Form1601CSchedule1Row>,

    // Taxes Withheld for Remittance
    #[serde(default)]
    pub tax_27_taxes_withheld_for_remittance: f64,

    // Less: Tax Remitted in Return Previously Filed
    #[serde(default)]
    pub tax_28_tax_remitted_previously: f64,

    #[serde(default)]
    pub tax_29_other_remittances_name: String,
    #[serde(default)]
    pub tax_29_other_remittances_amount: f64,

    // Total Tax Remittances Made
    #[serde(default)]
    pub tax_30_total_tax_remittances: f64,

    // Tax Still Due/(Overremittance)
    #[serde(default)]
    pub tax_31_tax_still_due: f64,

    // === Penalties ===
    #[serde(default = "default_true")]
    pub auto_compute_penalties: bool,
    #[serde(default)]
    pub tax_32_surcharge: f64,
    #[serde(default)]
    pub tax_33_interest: f64,
    #[serde(default)]
    pub tax_34_compromise: f64,

    // Computed Total Penalties
    #[serde(default)]
    pub tax_35_total_penalties: f64,

    // Computed Total Amount Payable
    #[serde(default)]
    pub tax_36_total_amount_payable: f64,

    // === Status & Audit ===
    pub status: FilingStatus,
    pub created_at: String,
    pub updated_at: String,

    #[serde(default)]
    pub submission_attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_retry_at: Option<String>,
}

impl Form1601CDraft {
    /// Create a new draft pre-filled from a profile.
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, month: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            month,
            is_amended: false,
            any_taxes_withheld: true,
            number_of_sheets: 0,
            atc: FORM_1601C_ATC.to_string(),
            rdo_code: profile.rdo_code.clone(),
            line_of_business: profile.line_of_business.clone(),
            taxpayer_name: profile.full_name.clone(),
            contact_number: profile.phone.clone(),
            registered_address: profile.registered_address.clone(),
            registered_address_2: String::new(),
            zip_code: profile.zip_code.clone(),
            category_of_agent: "P".to_string(), // Default Private
            email_address: profile.email.clone(),
            tax_relief: false,
            tax_relief_specification: String::new(),

            tax_14_total_compensation: 0.0,
            tax_15_statutory_minimum_wage: 0.0,
            tax_16_holiday_pay: 0.0,
            tax_17_13th_month_pay: 0.0,
            tax_18_de_minimis: 0.0,
            tax_19_sss_gsis: 0.0,
            tax_20_other_name: String::new(),
            tax_20_other_amount: 0.0,
            tax_21_total_non_taxable: 0.0,
            tax_22_total_taxable: 0.0,
            tax_23_not_subject: 0.0,
            tax_24_net_taxable: 0.0,
            tax_25_total_taxes_withheld: 0.0,
            tax_26_adjustment: 0.0,
            schedule_1: Vec::new(),
            tax_27_taxes_withheld_for_remittance: 0.0,
            tax_28_tax_remitted_previously: 0.0,
            tax_29_other_remittances_name: String::new(),
            tax_29_other_remittances_amount: 0.0,
            tax_30_total_tax_remittances: 0.0,
            tax_31_tax_still_due: 0.0,

            auto_compute_penalties: true,
            tax_32_surcharge: 0.0,
            tax_33_interest: 0.0,
            tax_34_compromise: 0.0,
            tax_35_total_penalties: 0.0,
            tax_36_total_amount_payable: 0.0,

            status: FilingStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            submission_attempts: 0,
            submission_error: None,
            next_retry_at: None,
        }
    }

    pub fn period_code(&self) -> String {
        format!("{:02}{:04}", self.month, self.taxable_year)
    }

    pub fn default_submission_filename(&self) -> String {
        format!(
            "{}-1601Cv2018-{}.xml",
            self.tin.replace("-", ""),
            self.period_code()
        )
    }

    pub fn transition_to_submitted(&mut self, _filename: String) {
        self.status = FilingStatus::Submitted;
        self.updated_at = chrono::Utc::now().to_rfc3339();
        self.submission_error = None;
        self.next_retry_at = None;
    }

    pub fn revert_to_draft(&mut self) {
        self.status = FilingStatus::Draft;
        self.submission_attempts = 0;
        self.submission_error = None;
        self.next_retry_at = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn record_submission_failure(&mut self, error_msg: String) {
        self.submission_attempts += 1;
        self.submission_error = Some(error_msg);
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Recalculate computed fields based on user inputs.
    pub fn compute(&mut self) {
        // Official Schedule I Item 7 = Item 6 less Item 5. The Schedule I
        // total feeds Part II Item 26.
        for row in &mut self.schedule_1 {
            row.recompute();
        }
        self.tax_26_adjustment =
            round_currency(self.schedule_1.iter().map(|row| row.adjustment).sum());

        // Line 21 = 15 + 16 + 17 + 18 + 19 + 20
        self.tax_21_total_non_taxable = round_currency(
            self.tax_15_statutory_minimum_wage
                + self.tax_16_holiday_pay
                + self.tax_17_13th_month_pay
                + self.tax_18_de_minimis
                + self.tax_19_sss_gsis
                + self.tax_20_other_amount,
        );

        // Line 22 = 14 - 21
        self.tax_22_total_taxable =
            round_currency(self.tax_14_total_compensation - self.tax_21_total_non_taxable);

        // Line 24 = 22 - 23
        self.tax_24_net_taxable =
            round_currency(self.tax_22_total_taxable - self.tax_23_not_subject);

        // Line 27 = 25 + 26
        self.tax_27_taxes_withheld_for_remittance =
            round_currency(self.tax_25_total_taxes_withheld + self.tax_26_adjustment);

        // Line 30 = 28 + 29
        self.tax_30_total_tax_remittances = round_currency(
            self.tax_28_tax_remitted_previously + self.tax_29_other_remittances_amount,
        );

        // Line 31 = 27 - 30
        self.tax_31_tax_still_due = round_currency(
            self.tax_27_taxes_withheld_for_remittance - self.tax_30_total_tax_remittances,
        );

        // Auto-compute penalties if enabled and still in Draft
        if self.auto_compute_penalties && matches!(self.status, FilingStatus::Draft) {
            // 1601C monthly deadline: 10th of the following month (15th for December)
            let (deadline_year, deadline_month) = if self.month == 12 {
                (self.taxable_year as i32 + 1, 1u32)
            } else {
                (self.taxable_year as i32, self.month as u32 + 1)
            };
            let deadline_day = if self.month == 12 { 15 } else { 10 };

            if let Some(deadline) =
                chrono::NaiveDate::from_ymd_opt(deadline_year, deadline_month, deadline_day)
            {
                let today = chrono::Local::now().date_naive();
                let config = PenaltyConfig::default_rules();

                let penalty_tax_base = self.tax_31_tax_still_due.max(0.0);

                let ctx = PenaltyContext {
                    form_code: "1601Cv2018".to_string(),
                    tax_type: PenaltyProfile::Withholding,
                    taxpayer_class: TaxpayerClass::Regular,
                    taxable_period: format!("M{:02} {}", self.month, self.taxable_year),
                    is_amended_return: self.is_amended,
                    original_was_on_time: false,
                    is_fraud_or_willful_neglect: false,
                    basic_tax_due: penalty_tax_base,
                    amount_paid_before_deadline: 0.0,
                    gross_sales_or_receipts: self.tax_14_total_compensation,
                    due_date: deadline,
                    filing_date: today,
                    payment_date: None,
                };

                let penalties = PenaltyEngine::calculate(&ctx, &config);
                self.tax_32_surcharge = penalties.surcharge;
                self.tax_33_interest = penalties.interest;
                self.tax_34_compromise = penalties.compromise;
            }
        }

        // Line 35 = 32 + 33 + 34
        self.tax_35_total_penalties =
            round_currency(self.tax_32_surcharge + self.tax_33_interest + self.tax_34_compromise);

        // Line 36 = 31 + 35
        self.tax_36_total_amount_payable =
            round_currency(self.tax_31_tax_still_due + self.tax_35_total_penalties);

        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}

impl FormValidator for Form1601CDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();

        if !(1900..=9999).contains(&self.taxable_year) {
            errors.push((
                "taxable_year".to_string(),
                "Taxable year must be a 4-digit year".to_string(),
            ));
        }

        if !(1..=12).contains(&self.month) {
            errors.push((
                "month".to_string(),
                "Month must be between 1 and 12".to_string(),
            ));
        }

        if self.tin.trim().is_empty() {
            errors.push(("tin".to_string(), "TIN is required".to_string()));
        }

        if self.rdo_code.trim().is_empty() {
            errors.push(("rdo_code".to_string(), "RDO is required".to_string()));
        }

        if self.taxpayer_name.trim().is_empty() {
            errors.push(("taxpayer_name".to_string(), "Name is required".to_string()));
        }

        if self.registered_address.trim().is_empty() {
            errors.push((
                "registered_address".to_string(),
                "Address is required".to_string(),
            ));
        }

        if !validate_zip(&self.zip_code) {
            errors.push((
                "zip_code".to_string(),
                "Valid ZIP Code required".to_string(),
            ));
        }

        if !validate_ph_phone(&self.contact_number) {
            errors.push((
                "contact_number".to_string(),
                "Valid Philippine phone number required".to_string(),
            ));
        }

        // eFPS Parity Validations
        if self.category_of_agent != "P" && self.category_of_agent != "G" {
            errors.push((
                "category_of_agent".to_string(),
                "Please select an option for Category of Withholding Agent (Item 11)".to_string(),
            ));
        }

        if self.atc != FORM_1601C_ATC {
            errors.push((
                "atc".to_string(),
                format!("Item 5 ATC must be {FORM_1601C_ATC} for BIR Form 1601-C January 2018"),
            ));
        }

        if self.number_of_sheets > 99 {
            errors.push((
                "number_of_sheets".to_string(),
                "Number of sheets must fit the official two-digit field".to_string(),
            ));
        }

        if self.tax_relief && self.tax_relief_specification.trim().is_empty() {
            errors.push((
                "tax_relief_specification".to_string(),
                "Item 13A is required when payees avail of tax relief".to_string(),
            ));
        }

        if self.schedule_1.len() > MAX_SCHEDULE_1_ROWS {
            errors.push((
                "schedule_1".to_string(),
                format!(
                    "Schedule I supports at most {MAX_SCHEDULE_1_ROWS} rows in the verified 1601-C XML contract"
                ),
            ));
        }

        for (index, row) in self.schedule_1.iter().enumerate() {
            let field = format!("schedule_1_row_{}", index + 1);
            if !valid_month_year(&row.previous_month) {
                errors.push((
                    field.clone(),
                    format!(
                        "Schedule I row {} requires month in MM/YYYY format",
                        index + 1
                    ),
                ));
            }
            if !valid_calendar_date(&row.date_paid) {
                errors.push((
                    field.clone(),
                    format!(
                        "Schedule I row {} requires a valid MM/DD/YYYY date paid",
                        index + 1
                    ),
                ));
            }
            if row.drawee_bank_code_or_agency.trim().is_empty() {
                errors.push((
                    field.clone(),
                    format!(
                        "Schedule I row {} requires a drawee bank/code or agency",
                        index + 1
                    ),
                ));
            }
            if row.payment_number.trim().is_empty() {
                errors.push((
                    field.clone(),
                    format!(
                        "Schedule I row {} requires a payment reference number",
                        index + 1
                    ),
                ));
            }
            if !row.tax_paid.is_finite() || row.tax_paid < 0.0 {
                errors.push((
                    field.clone(),
                    format!(
                        "Schedule I row {} tax paid must be a non-negative amount",
                        index + 1
                    ),
                ));
            }
            if !row.should_be_tax_due.is_finite() || row.should_be_tax_due < 0.0 {
                errors.push((
                    field.clone(),
                    format!(
                        "Schedule I row {} tax that should be due must be a non-negative amount",
                        index + 1
                    ),
                ));
            }
            let expected_adjustment = round_currency(row.should_be_tax_due - row.tax_paid);
            validate_computed_money(
                &mut errors,
                &field,
                row.adjustment,
                expected_adjustment,
                &format!(
                    "Schedule I row {} adjustment must equal Item 6 less Item 5",
                    index + 1
                ),
            );
        }

        let expected_schedule_total =
            round_currency(self.schedule_1.iter().map(|row| row.adjustment).sum());
        validate_computed_money(
            &mut errors,
            "tax_26_adjustment",
            self.tax_26_adjustment,
            expected_schedule_total,
            "Item 26 must equal the total of Schedule I adjustments",
        );

        for (field, value) in [
            ("tax_14_total_compensation", self.tax_14_total_compensation),
            (
                "tax_15_statutory_minimum_wage",
                self.tax_15_statutory_minimum_wage,
            ),
            ("tax_16_holiday_pay", self.tax_16_holiday_pay),
            ("tax_17_13th_month_pay", self.tax_17_13th_month_pay),
            ("tax_18_de_minimis", self.tax_18_de_minimis),
            ("tax_19_sss_gsis", self.tax_19_sss_gsis),
            ("tax_20_other_amount", self.tax_20_other_amount),
            ("tax_23_not_subject", self.tax_23_not_subject),
            (
                "tax_25_total_taxes_withheld",
                self.tax_25_total_taxes_withheld,
            ),
            (
                "tax_28_tax_remitted_previously",
                self.tax_28_tax_remitted_previously,
            ),
            (
                "tax_29_other_remittances_amount",
                self.tax_29_other_remittances_amount,
            ),
            ("tax_32_surcharge", self.tax_32_surcharge),
            ("tax_33_interest", self.tax_33_interest),
            ("tax_34_compromise", self.tax_34_compromise),
        ] {
            validate_non_negative_money(&mut errors, field, value);
        }

        let expected_21 = round_currency(
            self.tax_15_statutory_minimum_wage
                + self.tax_16_holiday_pay
                + self.tax_17_13th_month_pay
                + self.tax_18_de_minimis
                + self.tax_19_sss_gsis
                + self.tax_20_other_amount,
        );
        let expected_22 = round_currency(self.tax_14_total_compensation - expected_21);
        let expected_24 = round_currency(expected_22 - self.tax_23_not_subject);
        let expected_27 =
            round_currency(self.tax_25_total_taxes_withheld + expected_schedule_total);
        let expected_30 = round_currency(
            self.tax_28_tax_remitted_previously + self.tax_29_other_remittances_amount,
        );
        let expected_31 = round_currency(expected_27 - expected_30);
        let expected_35 =
            round_currency(self.tax_32_surcharge + self.tax_33_interest + self.tax_34_compromise);
        let expected_36 = round_currency(expected_31 + expected_35);

        for (field, actual, expected, message) in [
            (
                "tax_21_total_non_taxable",
                self.tax_21_total_non_taxable,
                expected_21,
                "Item 21 must equal the sum of Items 15 through 20",
            ),
            (
                "tax_22_total_taxable",
                self.tax_22_total_taxable,
                expected_22,
                "Item 22 must equal Item 14 less Item 21",
            ),
            (
                "tax_24_net_taxable",
                self.tax_24_net_taxable,
                expected_24,
                "Item 24 must equal Item 22 less Item 23",
            ),
            (
                "tax_27_taxes_withheld_for_remittance",
                self.tax_27_taxes_withheld_for_remittance,
                expected_27,
                "Item 27 must equal Item 25 plus Item 26",
            ),
            (
                "tax_30_total_tax_remittances",
                self.tax_30_total_tax_remittances,
                expected_30,
                "Item 30 must equal Item 28 plus Item 29",
            ),
            (
                "tax_31_tax_still_due",
                self.tax_31_tax_still_due,
                expected_31,
                "Item 31 must equal Item 27 less Item 30",
            ),
            (
                "tax_35_total_penalties",
                self.tax_35_total_penalties,
                expected_35,
                "Item 35 must equal the sum of Items 32 through 34",
            ),
            (
                "tax_36_total_amount_payable",
                self.tax_36_total_amount_payable,
                expected_36,
                "Item 36 must equal Item 31 plus Item 35",
            ),
        ] {
            validate_computed_money(&mut errors, field, actual, expected, message);
        }

        if self.any_taxes_withheld {
            if self.tax_14_total_compensation <= 0.0 {
                errors.push((
                    "tax_14_total_compensation".to_string(),
                    "Invalid amount in Item 14. Value must be greater than zero(0) when Any Taxes Withheld is YES.".to_string(),
                ));
            }
            if self.tax_25_total_taxes_withheld <= 0.0 {
                errors.push((
                    "tax_25_total_taxes_withheld".to_string(),
                    "Invalid amount in Item 25. Value must be greater than zero(0) when Any Taxes Withheld is YES.".to_string(),
                ));
            }
        }

        errors
    }
}

fn validate_non_negative_money(errors: &mut Vec<(String, String)>, field: &str, value: f64) {
    if !value.is_finite() || value < 0.0 {
        errors.push((
            field.to_string(),
            format!("{field} must be a finite, non-negative amount"),
        ));
    }
}

fn validate_computed_money(
    errors: &mut Vec<(String, String)>,
    field: &str,
    actual: f64,
    expected: f64,
    mismatch_message: &str,
) {
    if !actual.is_finite() || !expected.is_finite() {
        errors.push((
            field.to_string(),
            format!("{field} must be a finite computed amount"),
        ));
    } else if (actual - expected).abs() > 0.001 {
        errors.push((field.to_string(), mismatch_message.to_string()));
    }
}

fn valid_month_year(value: &str) -> bool {
    value.len() == 7
        && value.as_bytes().get(2) == Some(&b'/')
        && value
            .chars()
            .enumerate()
            .all(|(index, ch)| index == 2 || ch.is_ascii_digit())
        && chrono::NaiveDate::parse_from_str(&format!("01/{value}"), "%d/%m/%Y").is_ok()
}

fn valid_calendar_date(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes().get(2) == Some(&b'/')
        && value.as_bytes().get(5) == Some(&b'/')
        && value
            .chars()
            .enumerate()
            .all(|(index, ch)| matches!(index, 2 | 5) || ch.is_ascii_digit())
        && chrono::NaiveDate::parse_from_str(value, "%m/%d/%Y").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::TaxpayerProfile;

    fn test_profile() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Test Withholding Agent",
            "tin": {
                "segment1": "123",
                "segment2": "456",
                "segment3": "789",
                "branch": "00000"
            },
            "rdo_code": "018",
            "line_of_business": "Software Development",
            "registered_address": "Olongapo",
            "zip_code": "2200",
            "phone": "09123456789",
            "email": "tax@example.com",
            "default_form_type": "1601Cv2018",
            "taxpayer_type": "Corporation"
        }))
        .expect("test profile must deserialize")
    }

    fn schedule_row(month: &str, paid: f64, due: f64) -> Form1601CSchedule1Row {
        Form1601CSchedule1Row {
            previous_month: month.to_string(),
            date_paid: "05/10/2026".to_string(),
            drawee_bank_code_or_agency: "AAB-001".to_string(),
            payment_number: "REF-001".to_string(),
            tax_paid: paid,
            should_be_tax_due: due,
            adjustment: 0.0,
        }
    }

    #[test]
    fn schedule_adjustments_drive_item_26_and_item_27() {
        let mut draft = Form1601CDraft::new_from_profile(&test_profile(), 2026, 6);
        draft.auto_compute_penalties = false;
        draft.tax_25_total_taxes_withheld = 1_000.0;
        draft.schedule_1 = vec![
            schedule_row("04/2026", 900.25, 1_000.5),
            schedule_row("05/2026", 500.0, 450.0),
        ];

        draft.compute();

        assert_eq!(draft.schedule_1[0].adjustment, 100.25);
        assert_eq!(draft.schedule_1[1].adjustment, -50.0);
        assert_eq!(draft.tax_26_adjustment, 50.25);
        assert_eq!(draft.tax_27_taxes_withheld_for_remittance, 1_050.25);
    }

    #[test]
    fn validation_rejects_rows_beyond_verified_schedule_capacity() {
        let mut draft = Form1601CDraft::new_from_profile(&test_profile(), 2026, 6);
        draft.any_taxes_withheld = false;
        draft.schedule_1 = vec![schedule_row("05/2026", 1.0, 1.0); 4];
        draft.compute();

        assert!(
            draft
                .validate()
                .iter()
                .any(|(field, _)| field == "schedule_1")
        );
    }

    #[test]
    fn schedule_dates_require_exact_official_formats() {
        let mut draft = Form1601CDraft::new_from_profile(&test_profile(), 2026, 6);
        draft.any_taxes_withheld = false;
        let mut row = schedule_row("5/2026", 1.0, 1.0);
        row.date_paid = "5/1/2026".to_string();
        draft.schedule_1 = vec![row];
        draft.compute();

        let row_errors = draft
            .validate()
            .into_iter()
            .filter(|(field, _)| field == "schedule_1_row_1")
            .count();
        assert_eq!(row_errors, 2);
    }

    #[test]
    fn item_13_requires_specification_only_when_selected() {
        let mut draft = Form1601CDraft::new_from_profile(&test_profile(), 2026, 6);
        draft.any_taxes_withheld = false;
        draft.tax_relief = true;

        assert!(
            draft
                .validate()
                .iter()
                .any(|(field, _)| field == "tax_relief_specification")
        );

        draft.tax_relief_specification = "Special Law 123".to_string();
        assert!(
            draft
                .validate()
                .iter()
                .all(|(field, _)| field != "tax_relief_specification")
        );
    }

    #[test]
    fn validation_requires_the_official_fixed_item_5_atc_and_item_11_category() {
        let mut draft = Form1601CDraft::new_from_profile(&test_profile(), 2026, 6);
        draft.any_taxes_withheld = false;
        assert_eq!(draft.atc, FORM_1601C_ATC);

        draft.atc = "WC010".to_string();
        draft.category_of_agent.clear();
        let errors = draft.validate();

        assert!(errors.iter().any(|(field, message)| {
            field == "atc" && message.contains("Item 5 ATC must be WW010")
        }));
        assert!(errors.iter().any(|(field, message)| {
            field == "category_of_agent" && message.contains("Item 11")
        }));
        assert!(
            errors
                .iter()
                .all(|(_, message)| !message.contains("Item 12"))
        );
    }

    #[test]
    fn validation_rejects_stale_totals_and_non_finite_inputs() {
        let mut draft = Form1601CDraft::new_from_profile(&test_profile(), 2026, 6);
        draft.any_taxes_withheld = false;
        draft.auto_compute_penalties = false;
        draft.tax_14_total_compensation = 10.0;
        draft.tax_15_statutory_minimum_wage = 1.0;
        draft.compute();

        draft.tax_15_statutory_minimum_wage = 2.0;
        draft.tax_29_other_remittances_amount = f64::NAN;

        let errors = draft.validate();
        assert!(
            errors
                .iter()
                .any(|(field, _)| field == "tax_21_total_non_taxable")
        );
        assert!(
            errors
                .iter()
                .any(|(field, _)| field == "tax_29_other_remittances_amount")
        );
    }
}
