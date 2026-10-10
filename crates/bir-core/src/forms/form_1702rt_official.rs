//! Form 1702-RT (January 2018, `1702RTv2018C`) on the generic submission path.
//!
//! Ported from the official `BIR-Form1702RTv2018C.hta` (eBIRForms 7.9.6.2.1):
//! the compute chain (`computeP2Pt4I29` … `computeP2Pt5I59`, Schedules I–V,
//! `computeP1Pt2I16` … `computeP1Pt2I21`), `checkDateOfIncorporation` for the
//! IC055 ATC, `validate()` / `initialValidateBeforeSave()` and the field
//! checks with their exact alert texts, and `saveXMLsubmit` through
//! [`crate::official_xml`].
//!
//! Amounts are whole pesos. The page shows a negative amount as `(1,234)`
//! and `saveXMLsubmit` writes it through `NumWithParenthesis`, so the
//! submitted text is `-1,234` ([`WholePeso::format_bir`]).
//!
//! The page picks its tax rules from `taxableYear`, computed at load from
//! `new Date().getYear()`. Under the HTA host (Internet Explorer) that is the
//! four-digit year, so every return filed since 2021 takes the `>= 20`
//! branch: Item 40 (rate), Item 42 (MCIT) and Part V Item 57 are entered by
//! the filer, and Item 41 is Item 39 times the rate.
//!
//! The "Add more..." modals (Items 54, 17i, Schedule II Item 4, Schedule
//! IIIA Item 7, Schedule V Items 3, 6 and 8) are not modelled; the fixed
//! rows of the page are.

use std::collections::BTreeMap;

use super::form_1702rt::{
    Form1702RTDeductionMethod, Form1702RTDraft, Form1702RTFilingBasis, WholePeso,
};
use super::queueable::{QueueableForm, SubmissionLifecycle};
use super::{FilingPeriod, FormValidator};

/// Rule-package id of the official layout.
pub const FORM_1702RT_LAYOUT_ID: &str = "1702rt-v2018c";
/// Official `formType` and PROD SFTP folder (`ftpTargetFolder.PROD['1702RTv2018C']`).
pub const FORM_1702RT_FORM_TYPE: &str = "1702RTv2018C";

const P: &str = "frm1702RT:";
const ZERO: WholePeso = WholePeso(0);

fn sum(values: impl IntoIterator<Item = WholePeso>) -> WholePeso {
    WholePeso(values.into_iter().map(|v| v.0).sum())
}

/// `getProduct2`: `Math.round(value * multiplier)`.
fn product(value: WholePeso, multiplier: f64) -> WholePeso {
    WholePeso((value.0 as f64 * multiplier + 0.5).floor() as i64)
}

/// `capitalize()`: `$.trim(value.toUpperCase())`.
fn capital(value: &str) -> String {
    value.to_uppercase().trim().to_string()
}

impl Form1702RTDraft {
    pub fn is_itemized(&self) -> bool {
        self.deduction_method == Form1702RTDeductionMethod::Itemized
    }

    /// `checkDateOfIncorporation`: IC055 (MCIT) is ticked once four or more
    /// years separate the incorporation year from the Item 2 year.
    pub fn subject_to_mcit(&self) -> bool {
        self.incorporation_date
            .is_some_and(|date| i32::from(self.taxable_year) - i32::from(date.year) >= 4)
    }

    /// The official compute chain, settled the way the page's handlers leave
    /// it once every entry has been blurred.
    pub(super) fn official_recompute(&mut self) {
        if self.filing_basis == Form1702RTFilingBasis::Calendar && !self.is_short_period {
            // checkFilingYear: calendar returns end in December.
            self.month = 12;
        }
        let mcit = self.subject_to_mcit();
        self.atc.printed_mcit_selected = mcit;
        let itemized = self.is_itemized();

        if !itemized {
            // methodOfDeductionsOptional clears Schedules I, II and IIIA.
            let s1 = &mut self.schedule_1;
            for amount in [
                &mut s1.amortizations,
                &mut s1.bad_debts,
                &mut s1.charitable_contributions,
                &mut s1.depletion,
                &mut s1.depreciation,
                &mut s1.entertainment,
                &mut s1.fringe_benefits,
                &mut s1.interest,
                &mut s1.losses,
                &mut s1.pension_trusts,
                &mut s1.rental,
                &mut s1.research_and_development,
                &mut s1.salaries_wages_allowances,
                &mut s1.statutory_contributions,
                &mut s1.taxes_and_licenses,
                &mut s1.transportation_and_travel,
                &mut s1.janitorial_and_messengerial,
                &mut s1.professional_fees,
                &mut s1.security_services,
            ] {
                *amount = ZERO;
            }
            for row in &mut s1.other {
                *row = Default::default();
            }
            for row in &mut self.schedule_2.rows {
                *row = Default::default();
            }
            for row in &mut self.schedule_3.rows {
                *row = Default::default();
            }
            self.part_v.item_57_special_allowable_deductions_tax_effect = ZERO;
        }
        if !mcit {
            // enableMCITFields(false)
            self.part_iv.item_42_mcit_due = ZERO;
            self.part_iv
                .tax_credits
                .item_45_previous_quarter_mcit_payments = ZERO;
            for row in &mut self.schedule_4.rows {
                *row = Default::default();
            }
        }
        if !self.is_amended {
            self.part_iv.tax_credits.item_51_tax_paid_on_previous_return = ZERO;
        }

        // Schedule I (computeP3Sc1I18TotalOrdinaryAllowable)
        let s1 = &mut self.schedule_1;
        s1.item_18_total = sum(s1
            .source_amounts()
            .into_iter()
            .chain(s1.other.iter().map(|row| row.amount)));
        // Schedule II (computeP3Sc2I5TotalSpecialAllowable)
        self.schedule_2.item_5_total = sum(self.schedule_2.rows.iter().map(|row| row.amount));

        let p4 = &mut self.part_iv;
        p4.item_29_net_sales = WholePeso(p4.item_27_sales.0 - p4.item_28_sales_returns.0);
        p4.item_31_gross_income_from_operations =
            WholePeso(p4.item_29_net_sales.0 - p4.item_30_cost_of_sales_or_services.0);
        p4.item_33_total_taxable_income = WholePeso(
            p4.item_31_gross_income_from_operations.0 + p4.item_32_other_taxable_income.0,
        );
        let gross = p4.item_33_total_taxable_income;
        let ordinary = self.schedule_1.item_18_total;
        p4.item_34_ordinary_itemized_deductions = ordinary;

        // Schedule III (computeItem18and33, fillNetOperatingLossItems).
        let s3 = &mut self.schedule_3;
        if itemized && ordinary.0 > gross.0 {
            s3.item_1_gross_income = gross;
            s3.item_2_ordinary_deductions = ordinary;
            s3.item_3_net_operating_loss = WholePeso(gross.0 - ordinary.0);
        } else {
            s3.item_1_gross_income = ZERO;
            s3.item_2_ordinary_deductions = ZERO;
            s3.item_3_net_operating_loss = ZERO;
        }
        // Item 4 is this year's loss; its columns B to D stay disabled.
        let current = &mut s3.rows[0];
        current.applied_previous_years = ZERO;
        current.expired = ZERO;
        current.applied_current_year = ZERO;
        if s3.item_3_net_operating_loss.0 < 0 {
            current.year_incurred = format!("20{:02}", self.taxable_year % 100);
            current.amount = WholePeso(s3.item_3_net_operating_loss.0.abs());
        } else {
            current.year_incurred = String::new();
            current.amount = ZERO;
        }
        for row in &mut s3.rows {
            row.unapplied_balance = WholePeso(
                row.amount.0
                    - (row.applied_previous_years.0 + row.expired.0 + row.applied_current_year.0),
            );
        }
        s3.item_8_total_applied_current_year =
            sum(s3.rows.iter().map(|row| row.applied_current_year));

        // Schedule IV (computeP4Sc4I1C4 … computeP4Sc4I4TotalExcessMCIT).
        let s4 = &mut self.schedule_4;
        for row in &mut s4.rows {
            row.excess_mcit = WholePeso((row.mcit.0 - row.normal_income_tax.0).max(0));
            row.allowable_balance = WholePeso(
                row.excess_mcit.0
                    - (row.applied_previous_years.0 + row.expired.0 + row.applied_current_year.0),
            );
        }
        s4.item_4_total_applied_current_year =
            sum(s4.rows.iter().map(|row| row.applied_current_year));

        let p4 = &mut self.part_iv;
        if itemized {
            p4.item_35_special_itemized_deductions = self.schedule_2.item_5_total;
            p4.item_36_nolco = self.schedule_3.item_8_total_applied_current_year;
            p4.item_37_total_itemized_deductions = sum([
                p4.item_34_ordinary_itemized_deductions,
                p4.item_35_special_itemized_deductions,
                p4.item_36_nolco,
            ]);
            p4.item_38_optional_standard_deduction = ZERO;
            p4.item_39_net_taxable_income_or_loss =
                WholePeso(gross.0 - p4.item_37_total_itemized_deductions.0);
        } else {
            p4.item_35_special_itemized_deductions = ZERO;
            p4.item_36_nolco = self.schedule_3.item_8_total_applied_current_year;
            p4.item_37_total_itemized_deductions = ZERO;
            p4.item_38_optional_standard_deduction = product(gross, 0.40);
            p4.item_39_net_taxable_income_or_loss =
                WholePeso(gross.0 - p4.item_38_optional_standard_deduction.0);
        }
        // computeP2Pt4I40 / computeP2Pt4I41 / computeP2Pt4I43
        let net = p4.item_39_net_taxable_income_or_loss;
        p4.item_41_normal_income_tax_due = if net.0 <= 0 {
            ZERO
        } else {
            product(net, f64::from(p4.item_40_income_tax_rate_percent) / 100.0)
        };
        p4.item_43_tax_due = if p4.item_41_normal_income_tax_due.0 >= p4.item_42_mcit_due.0 {
            p4.item_41_normal_income_tax_due
        } else {
            p4.item_42_mcit_due
        };
        let credits = &mut p4.tax_credits;
        credits.item_47_excess_mcit_applied = self.schedule_4.item_4_total_applied_current_year;
        credits.item_55_total = sum([
            credits.item_44_prior_year_excess_credits,
            credits.item_45_previous_quarter_mcit_payments,
            credits.item_46_previous_quarter_regular_payments,
            credits.item_47_excess_mcit_applied,
            credits.item_48_previous_quarter_withholding,
            credits.item_49_fourth_quarter_withholding,
            credits.item_50_foreign_tax_credits,
            credits.item_51_tax_paid_on_previous_return,
            credits.item_52_special_tax_credits,
            credits.item_53_other.amount,
            credits.item_54_other.amount,
        ]);
        p4.item_56_net_tax_payable_or_overpayment =
            WholePeso(p4.item_43_tax_due.0 - credits.item_55_total.0);

        // Part V
        let p5 = &mut self.part_v;
        p5.item_58_special_tax_credits = p4.tax_credits.item_52_special_tax_credits;
        p5.item_59_total_tax_relief = sum([
            p5.item_57_special_allowable_deductions_tax_effect,
            p5.item_58_special_tax_credits,
        ]);

        // Schedule V
        let s5 = &mut self.schedule_5;
        s5.item_4_total = sum(std::iter::once(s5.item_1_net_income_or_loss_per_books)
            .chain(s5.additions.iter().map(|row| row.amount)));
        s5.item_9_total = sum(s5
            .non_taxable_income
            .iter()
            .chain(s5.special_deductions.iter())
            .map(|row| row.amount));
        s5.item_10_net_taxable_income_or_loss = WholePeso(s5.item_4_total.0 - s5.item_9_total.0);

        // Part II (computeP1Pt2I16 … computeP1Pt2I21)
        let p2 = &mut self.part_ii;
        p2.item_14_tax_due = p4.item_43_tax_due;
        p2.item_15_total_tax_credits = p4.tax_credits.item_55_total;
        p2.item_16_net_tax_payable_or_overpayment = p4.item_56_net_tax_payable_or_overpayment;
        p2.item_20_total_penalties = sum([
            p2.item_17_surcharge,
            p2.item_18_interest,
            p2.item_19_compromise,
        ]);
        let net_tax = p2.item_16_net_tax_payable_or_overpayment;
        p2.item_21_total_amount_payable_or_overpayment = if net_tax.0 >= 0 {
            WholePeso(net_tax.0 + p2.item_20_total_penalties.0)
        } else if p2.item_20_total_penalties.0 > 0 {
            p2.item_20_total_penalties
        } else {
            net_tax
        };
        if net_tax.0 >= 0 {
            p2.overpayment_disposition = None;
        }
    }

    /// `validate()` and `initialValidateBeforeSave()` in order with their
    /// alert texts, plus the checks the page makes while typing.
    pub(super) fn official_errors(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        let mut err = |field: &str, message: &str| {
            errors.push((field.to_string(), message.to_string()));
        };
        let today = chrono::Local::now().date_naive();
        let current_year = chrono::Datelike::year(&today);

        // Page 1 Item 2 (validate, validateYearEnd, checkFilingYear)
        let year = i32::from(self.taxable_year);
        if !(1..=12).contains(&self.month)
            || self.taxable_year.is_multiple_of(100)
            || !(2000..=2099).contains(&year)
        {
            err(
                "taxable_year",
                "Please provide a valid Year Ended on Page 1 Item 2",
            );
        } else if year < 2018 {
            err(
                "taxable_year",
                "Invalid Year. Year should not be earlier than 2018.",
            );
        } else {
            match self.filing_basis {
                Form1702RTFilingBasis::Fiscal => {
                    let month = i32::from(self.month);
                    if year > current_year
                        || (year == current_year && month > chrono::Datelike::month(&today) as i32)
                    {
                        err(
                            "taxable_year",
                            "Date (Page 1 Item 2) cannot be greater than current date when filing for Fiscal Year.",
                        );
                    } else if self.month == 12 {
                        err(
                            "month",
                            "Date (Page 1 Item 2) Month cannot be equal to December.",
                        );
                    }
                }
                Form1702RTFilingBasis::Calendar => {
                    if !self.is_short_period && year >= current_year {
                        err(
                            "taxable_year",
                            "Year (Page 1 Item 2) cannot be greater than or equal to current year when filing for Calendar Year.",
                        );
                    } else if self.is_short_period && year > current_year {
                        err(
                            "taxable_year",
                            "Year (Page 1 Item 2) cannot be greater than the current year when filing for Calendar Year.",
                        );
                    } else if !self.is_short_period && self.month != 12 {
                        err(
                            "month",
                            "A calendar-year return ends in December (Page 1 Item 2).",
                        );
                    }
                }
            }
        }

        // Page 1 Item 6
        let digits: String = self.tin.chars().filter(char::is_ascii_digit).collect();
        if !(12..=14).contains(&digits.len())
            || !self.tin.chars().all(|ch| ch.is_ascii_digit() || ch == '-')
        {
            err(
                "tin",
                "Please provide a valid TIN (must have 3 numbers per box).",
            );
        } else if !crate::validation::relaxed_dev_mode()
            && crate::validation::official_tin_check_code(&digits[..9]) != 0
        {
            err("tin", crate::validation::OFFICIAL_INVALID_TIN_MESSAGE);
        }
        // Page 1 Item 7
        let rdo = self.rdo_code.trim();
        if rdo.is_empty() || rdo == "000" || !crate::validation::rdo_code_is_official_option(rdo) {
            err(
                "rdo_code",
                "Please select a RDO code on Page 1 Part 1 Item 7",
            );
        }
        // Page 1 Item 10
        match self.incorporation_date {
            None => err(
                "incorporation_date",
                "Please provide a valid Date of Incorporation/Organization on Page 1 Part I Item 10",
            ),
            Some(date) => {
                let naive = chrono::NaiveDate::from_ymd_opt(
                    i32::from(date.year),
                    u32::from(date.month),
                    u32::from(date.day),
                );
                if naive.is_none() || date.year < 1800 {
                    err(
                        "incorporation_date",
                        "Please provide a valid date. (MM/DD/YYYY format)",
                    );
                } else if naive.is_some_and(|day| day > today) {
                    err("incorporation_date", "This date cannot be a future date.");
                } else if i32::from(date.year) > year
                    || (i32::from(date.year) == year && date.month > self.month)
                {
                    err(
                        "incorporation_date",
                        "Date of Incorporation cannot be greater than Page 1 Item 2 Date.",
                    );
                }
            }
        }
        // Page 1 Items 8, 9, 11, 12
        if self.registered_name_lines[0].trim().is_empty() {
            err(
                "registered_name_lines",
                "Please provide a Registered Name on Page 1 Part I Item 8",
            );
        }
        if self.registered_address_lines[0].trim().is_empty() {
            err(
                "registered_address_lines",
                "Please provide a Registered Address on Page 1 Part I Item 9",
            );
        }
        let contact = self.contact_number.trim();
        if contact.is_empty() || contact.len() > 20 || !contact.bytes().all(|b| b.is_ascii_digit())
        {
            err(
                "contact_number",
                "Please provide a valid Contact Number on Page 1 Part I Item 11",
            );
        }
        let email = self.email.trim();
        if email.is_empty() {
            err(
                "email",
                "Please provide a valid Email Address on Page 1 Part I Item 12",
            );
        } else if !email_matches_official_format(email) {
            err("email", "You have entered an invalid email address format!");
        }
        for (field, lines, max) in [
            ("registered_name_lines", &self.registered_name_lines, 100),
            (
                "registered_address_lines",
                &self.registered_address_lines,
                100,
            ),
        ] {
            if lines.iter().any(|line| line.chars().count() > max) {
                err(field, "Each line holds at most 100 characters.");
            }
        }
        if self.deduction_method == Form1702RTDeductionMethod::Unresolved {
            err(
                "deduction_method",
                "Select the method of deductions on Page 1 Part I Item 13.",
            );
        }

        // Page 1 Part II Items 16 and 21
        let p2 = &self.part_ii;
        if p2.item_16_net_tax_payable_or_overpayment.0 < 0 && p2.overpayment_disposition.is_none() {
            err(
                "part_ii.overpayment_disposition",
                "Please select an option for Overpayment Radio Button on Page 1 Part II ",
            );
        } else if p2.item_21_total_amount_payable_or_overpayment.0 < 0
            && p2.overpayment_disposition.is_none()
        {
            err(
                "part_ii.overpayment_disposition",
                "Please select an option for Overpayment Radio Button on Page 1 Part",
            );
        }

        // Page 1 Part III (validate_nullDescription; the official labels are
        // numbered three ahead of the items they check).
        let pay = &self.payment_details;
        for (index, label, texts) in [
            (
                0,
                "Page 1 Part III Item 26",
                vec![&pay[0].drawee_bank_or_agency, &pay[0].number],
            ),
            (
                1,
                "Page 1 Part III Item 27",
                vec![&pay[1].drawee_bank_or_agency, &pay[1].number],
            ),
            (2, "Page 1 Part III Item 28", vec![&pay[2].number]),
            (
                3,
                "Page 1 Part III Item 29",
                vec![
                    &pay[3].specification,
                    &pay[3].drawee_bank_or_agency,
                    &pay[3].number,
                ],
            ),
        ] {
            let row = &pay[index];
            let filled: Vec<bool> = texts
                .iter()
                .map(|text| !text.trim().is_empty())
                .chain(std::iter::once(row.date.is_some()))
                .collect();
            if null_description(row.amount, &filled) {
                err(
                    &format!("payment_details[{index}]"),
                    &format!("Please provide data on {label}."),
                );
            }
        }
        let paid = sum(pay.iter().map(|row| row.amount));
        if paid.0 != 0 && paid != p2.item_21_total_amount_payable_or_overpayment {
            err(
                "payment_details",
                "Sum of Amount fields in Details of Payment (Page 1 Part III) Segment must be equal to TOTAL AMOUNT PAYABLE",
            );
        }

        // Page 2 Items 53 and 54
        let credits = &self.part_iv.tax_credits;
        for (field, row, label) in [
            (
                "part_iv.tax_credits.item_53_other",
                &credits.item_53_other,
                "Page 2 Part 4 Item 53",
            ),
            (
                "part_iv.tax_credits.item_54_other",
                &credits.item_54_other,
                "Page 2 Part 4 Item 54",
            ),
        ] {
            if null_description(row.amount, &[!row.description.trim().is_empty()]) {
                err(field, &format!("Please provide data on {label}."));
            }
        }
        if self.part_iv.tax_credits.item_54_other.amount.0 != 0
            && self
                .part_iv
                .tax_credits
                .item_53_other
                .description
                .trim()
                .is_empty()
        {
            err(
                "part_iv.tax_credits.item_54_other",
                "Item 54 opens only after Item 53 is complete.",
            );
        }
        if self.is_itemized()
            && self.part_iv.item_35_special_itemized_deductions.0 > 0
            && self
                .part_v
                .item_57_special_allowable_deductions_tax_effect
                .0
                == 0
        {
            err(
                "part_v.item_57_special_allowable_deductions_tax_effect",
                "Please provide a value for this field (Part V Item 57)",
            );
        }

        // Page 3 Schedule I Items 17d to 17i (17h reuses the 17g label).
        for (index, label) in ["17d", "17e", "17f", "17g", "17g", "17i"]
            .into_iter()
            .enumerate()
        {
            let row = &self.schedule_1.other[index];
            if null_description(row.amount, &[!row.description.trim().is_empty()]) {
                err(
                    &format!("schedule_1.other[{index}]"),
                    &format!("Please provide data on Page 3 Schedule 1 Item {label}."),
                );
            }
        }
        // Page 3 Schedule II
        for (index, row) in self.schedule_2.rows.iter().enumerate() {
            if null_description(
                row.amount,
                &[
                    !row.description.trim().is_empty(),
                    !row.legal_basis.trim().is_empty(),
                ],
            ) {
                err(
                    &format!("schedule_2.rows[{index}]"),
                    &format!(
                        "Please provide data on Page 3 Schedule 2 Item {}.",
                        index + 1
                    ),
                );
            }
        }
        // Page 4 Schedule IIIA (validate_nullDescription, checkYear,
        // computeP4Sc3AI8TotalNOLCO).
        let s3 = &self.schedule_3;
        if s3.item_1_gross_income.0 > s3.item_2_ordinary_deductions.0
            && s3.item_2_ordinary_deductions.0 > 0
        {
            err(
                "schedule_3",
                "In Page 4 Schedule 3 Computation for NOLCO, Gross Income should not be greater than Ordinary Allowable Deductions.",
            );
        }
        for (index, row) in s3.rows.iter().enumerate() {
            let item = index + 4;
            if null_description(row.amount, &[!row.year_incurred.trim().is_empty()]) {
                err(
                    &format!("schedule_3.rows[{index}]"),
                    &format!("Please provide data on Page 4 Schedule 3A Item {item}."),
                );
            }
            if row.applied_current_year.0 > row.amount.0 {
                err(
                    &format!("schedule_3.rows[{index}].applied_current_year"),
                    &format!(
                        "Page 4 Schedule IIIA Item {item} Column D (NOLCO Applied Current Year) cannot be more than Page 4 Schedule 3A Item {item} Column A (Amount)"
                    ),
                );
            } else if index > 0
                && row.applied_previous_years.0 + row.expired.0 + row.applied_current_year.0
                    > row.amount.0
            {
                err(
                    &format!("schedule_3.rows[{index}]"),
                    &format!(
                        "Page 4 Schedule IIIA Item {item}: Column B + C + D cannot be greater than Column A"
                    ),
                );
            }
        }
        let years: Vec<&str> = s3.rows.iter().map(|row| row.year_incurred.trim()).collect();
        for (index, row) in s3.rows.iter().enumerate().skip(1) {
            let text = row.year_incurred.trim();
            if text.is_empty() {
                continue;
            }
            let field = format!("schedule_3.rows[{index}].year_incurred");
            let Ok(value) = text.parse::<i32>() else {
                err(&field, "Year Incurred must be a four-digit year.");
                continue;
            };
            let incorporated = self.incorporation_date.map(|date| i32::from(date.year));
            if value == year {
                err(
                    &field,
                    "Page 4 Schedule IIIA Item 5-7 Year Incurred cannot be equal to Page 1 Item 2 Year Ended.",
                );
            } else if value >= year - 3 && value <= year {
                if incorporated.is_some_and(|incorporated| incorporated > value) {
                    err(
                        &field,
                        "Year Incurred cannot be less than year in Page 1 Part 1 Item 10 (Date of Incorporation/Organization)",
                    );
                } else if years.iter().filter(|other| **other == text).count() != 1 {
                    err(&field, "Year Incurred per row must be unique.");
                }
            } else if value > year {
                err(
                    &field,
                    "Year Incurred cannot be more than Page 1 Part 1 Item 2 (Year Ended)",
                );
            } else {
                err(
                    &field,
                    "Year Incurred Cannot be less than 3 years in Page 1 Item 2 (Year Ended)",
                );
            }
        }
        // Page 4 Schedule V
        for (field, row, item) in [
            ("schedule_5.additions[0]", &self.schedule_5.additions[0], 2),
            ("schedule_5.additions[1]", &self.schedule_5.additions[1], 3),
            (
                "schedule_5.non_taxable_income[0]",
                &self.schedule_5.non_taxable_income[0],
                5,
            ),
            (
                "schedule_5.non_taxable_income[1]",
                &self.schedule_5.non_taxable_income[1],
                6,
            ),
            (
                "schedule_5.special_deductions[0]",
                &self.schedule_5.special_deductions[0],
                7,
            ),
            (
                "schedule_5.special_deductions[1]",
                &self.schedule_5.special_deductions[1],
                8,
            ),
        ] {
            if null_description(row.amount, &[!row.description.trim().is_empty()]) {
                err(
                    field,
                    &format!("Please provide data on Page 4 Schedule 5 Item {item}."),
                );
            }
        }
        if self.part_iv.item_39_net_taxable_income_or_loss
            != self.schedule_5.item_10_net_taxable_income_or_loss
        {
            err(
                "schedule_5.item_10_net_taxable_income_or_loss",
                "Item Page 4 Schedule V Item 10 must be equal Item 39 on Page 2 Part IV",
            );
        }
        // Page 4 Schedule IV
        let mcit_years: Vec<&str> = self
            .schedule_4
            .rows
            .iter()
            .map(|row| row.year.trim())
            .collect();
        for (index, row) in self.schedule_4.rows.iter().enumerate() {
            if row.allowable_balance.0 < 0 {
                err(
                    &format!("schedule_4.rows[{index}]"),
                    "Page 4, Schedule IV: The sum of Columns D,E & F should not be greater than the amount in Column C. Please re-enter the correct values.",
                );
            }
            let text = row.year.trim();
            if text.is_empty() {
                continue;
            }
            let field = format!("schedule_4.rows[{index}].year");
            let value = text.parse::<i32>().unwrap_or(0);
            let incorporated = self.incorporation_date.map(|date| i32::from(date.year));
            if mcit_years.iter().filter(|other| **other == text).count() > 1 {
                err(&field, "Year cannot have duplicates.");
            } else if value >= year || value < year - 3 {
                err(
                    &field,
                    &format!(
                        "Please enter a value for the year between {} and {}.",
                        year - 3,
                        year - 1
                    ),
                );
            } else if incorporated.is_some_and(|incorporated| value < incorporated) {
                err(
                    &field,
                    &format!(
                        "Year incurred cannot be lower than Year of Incorporation [{}] (Page 1 Item 8).",
                        incorporated.unwrap_or_default()
                    ),
                );
            }
        }

        // Amounts the filer types (checkNumValue turns a negative into 0).
        let p4 = &self.part_iv;
        if p4.item_40_income_tax_rate_percent == 0 || p4.item_40_income_tax_rate_percent >= 100 {
            err(
                "part_iv.item_40_income_tax_rate_percent",
                "Percentage cannot be greater than or equal to 100%",
            );
        }
        let negative = [
            p2.item_17_surcharge,
            p2.item_18_interest,
            p2.item_19_compromise,
            p4.item_27_sales,
            p4.item_28_sales_returns,
            p4.item_30_cost_of_sales_or_services,
            p4.item_32_other_taxable_income,
            p4.item_42_mcit_due,
            credits.item_44_prior_year_excess_credits,
            credits.item_45_previous_quarter_mcit_payments,
            credits.item_46_previous_quarter_regular_payments,
            credits.item_48_previous_quarter_withholding,
            credits.item_49_fourth_quarter_withholding,
            credits.item_50_foreign_tax_credits,
            credits.item_51_tax_paid_on_previous_return,
            credits.item_52_special_tax_credits,
            credits.item_53_other.amount,
            credits.item_54_other.amount,
            self.part_v.item_57_special_allowable_deductions_tax_effect,
        ]
        .into_iter()
        .chain(self.schedule_1.source_amounts())
        .chain(self.schedule_1.other.iter().map(|row| row.amount))
        .chain(self.schedule_2.rows.iter().map(|row| row.amount))
        .chain(self.payment_details.iter().map(|row| row.amount))
        .any(|amount| amount.0 < 0);
        if negative {
            err("amounts", "Amounts are whole pesos and cannot be negative.");
        }
        if self
            .registered_name_lines
            .iter()
            .chain(std::iter::once(&self.taxpayer_name))
            .any(|line| line.contains(char::is_control))
        {
            err(
                "registered_name_lines",
                "Names cannot contain control characters.",
            );
        }

        // Derived items must be what the official compute chain produces.
        let mut expected = self.clone();
        expected.official_recompute();
        if expected.part_ii != self.part_ii
            || expected.part_iv != self.part_iv
            || expected.part_v != self.part_v
            || expected.schedule_1 != self.schedule_1
            || expected.schedule_2 != self.schedule_2
            || expected.schedule_3 != self.schedule_3
            || expected.schedule_4 != self.schedule_4
            || expected.schedule_5 != self.schedule_5
            || expected.atc != self.atc
        {
            err(
                "part_ii.item_21_total_amount_payable_or_overpayment",
                "Totals are out of date. Recompute the return.",
            );
        }
        errors
    }

    /// The official field values `saveXMLsubmit` writes, keyed by element id.
    pub fn to_official_field_map(&self) -> BTreeMap<String, String> {
        let layout =
            crate::official_xml::layout(FORM_1702RT_LAYOUT_ID).expect("1702RT layout is packaged");
        let keys = layout.keys();
        let mut fields: BTreeMap<String, String> = self
            .to_bir_field_map()
            .into_iter()
            .filter(|(key, _)| keys.contains(key.as_str()))
            .collect();
        // Page state the filer never edits keeps its official default.
        for key in [
            "BranchMaskP1",
            "txtBranchMaskP2",
            "txtBranchMaskP3",
            "txtBranchMaskP4",
            "driveSelectTPExport",
            "txtFinalFlag",
            "txtEnroll",
            "ebirOnlineConfirmUsername",
            "ebirOnlineUsername",
            "ebirOnlineSecret",
        ] {
            fields.remove(key);
        }
        fields.retain(|key, _| !key.ends_with("CtrModal") && !key.ends_with("Subtotal"));
        let email_key = format!("{P}txtPg1Pt1I12Email");
        for (key, value) in fields.iter_mut() {
            if *key != email_key && value != "true" && value != "false" {
                *value = capital(value);
            }
        }
        let mut put = |key: &str, value: String| {
            fields.insert(format!("{P}{key}"), value);
        };
        put("txtPg1Pt1I12Email", self.email.trim().to_string());
        put(
            "drpPg1I5AtcOther",
            if self.atc.other_code.trim().is_empty() {
                "IC010".to_string()
            } else {
                self.atc.other_code.trim().to_string()
            },
        );
        for page in 2..=4 {
            put(
                &format!("txtPg{page}RegisteredName"),
                capital(&self.taxpayer_name),
            );
        }
        // getRdo() builds this select inside div#rdoContainer at load; the
        // packaged layout (generated from the static page) lacks it.
        put("drpPg1Pt1I7RDOCode", self.rdo_code.trim().to_string());
        fields
    }

    /// The exact official submit plaintext.
    pub fn to_official_xml_payload(&self) -> Result<String, Vec<(String, String)>> {
        self.official_payload()
    }
}

/// `validate_nullDescription` for one amount: a non-zero amount needs every
/// description, and a zero amount allows none.
fn null_description(amount: WholePeso, filled: &[bool]) -> bool {
    if amount.0 != 0 {
        filled.iter().any(|filled| !filled)
    } else {
        filled.iter().any(|filled| *filled)
    }
}

/// `validateEmail`: `/\b[a-zA-Z0-9._%+-]+@(?:[a-zA-Z0-9-]+\.)+[a-zA-Z]{2,4}\b/`.
fn email_matches_official_format(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    let local_ok = !local.is_empty()
        && local
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "._%+-".contains(ch));
    let labels: Vec<&str> = domain.split('.').collect();
    let domain_ok = labels.len() >= 2
        && labels[..labels.len() - 1].iter().all(|label| {
            !label.is_empty()
                && label
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        })
        && labels.last().is_some_and(|tld| {
            (2..=4).contains(&tld.len()) && tld.chars().all(|ch| ch.is_ascii_alphabetic())
        });
    local_ok && domain_ok
}

impl QueueableForm for Form1702RTDraft {
    const FORM_CODE: &'static str = "1702RT";
    const FORM_TYPE: &'static str = FORM_1702RT_FORM_TYPE;
    const LAYOUT_ID: &'static str = FORM_1702RT_LAYOUT_ID;

    fn lifecycle(&self) -> &SubmissionLifecycle {
        &self.lifecycle
    }
    fn lifecycle_mut(&mut self) -> &mut SubmissionLifecycle {
        &mut self.lifecycle
    }
    fn tin(&self) -> &str {
        &self.tin
    }
    fn taxable_year(&self) -> u16 {
        self.taxable_year
    }
    /// Annual. Rows saved before the generic queue have a NULL period column
    /// with period key `A`; the generic save adopts them.
    fn filing_period(&self) -> FilingPeriod {
        FilingPeriod::Annual
    }
    /// `ddlPg1I2Month + "20" + txtPg1I2Year`, as in `createEncXMLFileName`.
    fn period_code(&self) -> String {
        format!("{:02}20{:02}", self.month, self.taxable_year % 100)
    }
    fn parse_period_code(code: &str) -> Option<(u16, FilingPeriod)> {
        if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let month: u8 = code.get(..2)?.parse().ok()?;
        let year: u16 = code.get(2..)?.parse().ok()?;
        (1..=12)
            .contains(&month)
            .then_some((year, FilingPeriod::Annual))
    }
    fn submission_email(&self) -> &str {
        self.email.trim()
    }
    fn compute(&mut self) {
        self.recompute();
    }
    fn validate(&self) -> Vec<(String, String)> {
        <Self as FormValidator>::validate(self)
    }
    fn field_map(&self) -> BTreeMap<String, String> {
        self.to_official_field_map()
    }

    /// The layout replayed over the field map, plus the RDO select that
    /// `getRdo()` adds right after `txtRDO` when the page loads.
    fn official_payload(&self) -> Result<String, Vec<(String, String)>> {
        let errors = <Self as QueueableForm>::validate(self);
        if !errors.is_empty() {
            return Err(errors);
        }
        let xml_error = |error: crate::official_xml::OfficialXmlError| {
            vec![("xml".to_string(), error.to_string())]
        };
        let layout = crate::official_xml::layout(Self::LAYOUT_ID).map_err(xml_error)?;
        let mut fields = self.field_map();
        let rdo_key = format!("{P}drpPg1Pt1I7RDOCode");
        let rdo = fields.remove(&rdo_key).unwrap_or_default();
        let mut text = crate::official_xml::write(layout, &fields).map_err(xml_error)?;
        let anchor = format!("{P}txtRDO=</div>");
        let separator = "\t\n            ";
        let at = text
            .find(&anchor)
            .map(|at| at + anchor.len() + separator.len())
            .filter(|at| text.get(at - separator.len()..*at) == Some(separator))
            .ok_or_else(|| {
                vec![(
                    "xml".to_string(),
                    "txtRDO is missing from the layout".to_string(),
                )]
            })?;
        text.insert_str(
            at,
            &format!("<div>{rdo_key}={rdo}{rdo_key}=</div>{separator}"),
        );
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filing_queue::QueueAuthSource;
    use crate::forms::FilingStatus;
    use crate::forms::form_1702rt::{
        Form1702RTDate, Form1702RTNamedAmount, Form1702RTOverpaymentDisposition,
    };

    fn profile() -> crate::profile::TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Sample Dummy Corporation",
            "tin": {"segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000"},
            "rdo_code": "039",
            "line_of_business": "Sample Trading",
            "registered_address": "123 Sample Street, Quezon City",
            "zip_code": "1100",
            "phone": "0281234567",
            "email": "sample.taxpayer@example.com",
            "default_form_type": "1702RTv2018C",
            "taxpayer_type": "Corporation",
            "business_start_date": "2015-03-10"
        }))
        .expect("dummy profile")
    }

    pub(crate) fn sample() -> Form1702RTDraft {
        let mut draft = Form1702RTDraft::new_from_profile(&profile(), 2025, 12);
        draft.deduction_method = Form1702RTDeductionMethod::Itemized;
        let p4 = &mut draft.part_iv;
        p4.item_40_income_tax_rate_percent = 25;
        p4.item_27_sales = WholePeso(5_000_000);
        p4.item_28_sales_returns = WholePeso(100_000);
        p4.item_30_cost_of_sales_or_services = WholePeso(2_000_000);
        p4.item_32_other_taxable_income = WholePeso(50_001);
        p4.item_42_mcit_due = WholePeso(59_000);
        p4.tax_credits.item_48_previous_quarter_withholding = WholePeso(120_000);
        draft.schedule_1.salaries_wages_allowances = WholePeso(1_200_000);
        draft.schedule_1.rental = WholePeso(240_000);
        draft.recompute();
        let net = draft.part_iv.item_39_net_taxable_income_or_loss;
        draft.schedule_5.item_1_net_income_or_loss_per_books = net;
        draft.recompute();
        draft
    }

    fn messages(draft: &Form1702RTDraft) -> Vec<String> {
        <Form1702RTDraft as QueueableForm>::validate(draft)
            .into_iter()
            .map(|(_, message)| message)
            .collect()
    }

    #[test]
    fn compute_chain_matches_the_official_page() {
        let draft = sample();
        let p4 = &draft.part_iv;
        assert!(draft.atc.printed_mcit_selected);
        assert_eq!(p4.item_33_total_taxable_income, WholePeso(2_950_001));
        assert_eq!(p4.item_37_total_itemized_deductions, WholePeso(1_440_000));
        assert_eq!(p4.item_39_net_taxable_income_or_loss, WholePeso(1_510_001));
        assert_eq!(p4.item_41_normal_income_tax_due, WholePeso(377_500));
        assert_eq!(p4.item_43_tax_due, WholePeso(377_500));
        assert_eq!(
            draft.part_ii.item_21_total_amount_payable_or_overpayment,
            WholePeso(257_500)
        );
        assert!(messages(&draft).is_empty(), "{:?}", messages(&draft));
    }

    #[test]
    fn optional_standard_deduction_and_net_operating_loss() {
        let mut draft = sample();
        draft.deduction_method = Form1702RTDeductionMethod::OptionalStandard;
        draft.recompute();
        assert_eq!(
            draft.part_iv.item_38_optional_standard_deduction,
            WholePeso(1_180_000)
        );
        assert_eq!(draft.schedule_1.item_18_total, WholePeso(0));

        let mut loss = sample();
        loss.schedule_1.salaries_wages_allowances = WholePeso(4_000_000);
        loss.recompute();
        assert_eq!(
            loss.schedule_3.item_3_net_operating_loss,
            WholePeso(-1_289_999)
        );
        assert_eq!(loss.schedule_3.rows[0].year_incurred, "2025");
        assert_eq!(loss.schedule_3.rows[0].amount, WholePeso(1_289_999));
        assert_eq!(loss.part_iv.item_41_normal_income_tax_due, WholePeso(0));
        assert_eq!(loss.part_iv.item_43_tax_due, WholePeso(59_000));
    }

    #[test]
    fn field_map_uses_official_formats() {
        let mut draft = sample();
        draft
            .part_iv
            .tax_credits
            .item_48_previous_quarter_withholding = WholePeso(500_000);
        draft.part_ii.overpayment_disposition = Some(Form1702RTOverpaymentDisposition::CarryOver);
        draft.recompute();
        let fields = draft.to_official_field_map();
        assert_eq!(fields["frm1702RT:txtPg1Pt2I16NetTax"], "-122,500");
        assert_eq!(fields["frm1702RT:txtPg1I2Year"], "25");
        assert_eq!(fields["frm1702RT:ddlPg1I2Month"], "12");
        assert_eq!(
            fields["frm1702RT:txtPg1Pt1I8Name1"],
            "SAMPLE DUMMY CORPORATION"
        );
        assert_eq!(
            fields["frm1702RT:txtPg1Pt1I12Email"],
            "sample.taxpayer@example.com"
        );
        assert_eq!(fields["frm1702RT:Pg2Pt4I40IncomeTaxRate"], "25");
        assert!(!fields.contains_key("txtFinalFlag"));
        assert_eq!(
            draft.submission_filename(),
            "12345678800000-1702RTv2018C-122025#sample.taxpayer@example.com#.xml"
        );
        assert!(draft.to_official_xml_payload().is_ok());
    }

    #[test]
    fn period_codes_round_trip() {
        let draft = sample();
        assert_eq!(draft.period_code(), "122025");
        assert_eq!(
            Form1702RTDraft::parse_period_code("122025"),
            Some((2025, FilingPeriod::Annual))
        );
        assert_eq!(Form1702RTDraft::parse_period_code("132025"), None);
        assert_eq!(Form1702RTDraft::parse_period_code("122025Q1"), None);
        assert_eq!(draft.period_column(), 0);
    }

    #[test]
    fn official_negative_cases_are_rejected_with_their_alerts() {
        let check = |mutate: &dyn Fn(&mut Form1702RTDraft), expected: &str| {
            let mut draft = sample();
            mutate(&mut draft);
            let found = messages(&draft);
            assert!(
                found.iter().any(|m| m == expected),
                "{expected:?} not in {found:?}"
            );
        };
        check(
            &|d| d.month = 0,
            "Please provide a valid Year Ended on Page 1 Item 2",
        );
        check(
            &|d| d.taxable_year = 2017,
            "Invalid Year. Year should not be earlier than 2018.",
        );
        check(
            &|d| d.taxable_year = 2099,
            "Year (Page 1 Item 2) cannot be greater than or equal to current year when filing for Calendar Year.",
        );
        check(
            &|d| {
                d.filing_basis = Form1702RTFilingBasis::Fiscal;
                d.month = 12;
            },
            "Date (Page 1 Item 2) Month cannot be equal to December.",
        );
        check(
            &|d| d.rdo_code = "000".into(),
            "Please select a RDO code on Page 1 Part 1 Item 7",
        );
        check(
            &|d| d.incorporation_date = None,
            "Please provide a valid Date of Incorporation/Organization on Page 1 Part I Item 10",
        );
        check(
            &|d| d.incorporation_date = Form1702RTDate::new(2026, 1, 1).ok(),
            "Date of Incorporation cannot be greater than Page 1 Item 2 Date.",
        );
        check(
            &|d| d.registered_name_lines[0].clear(),
            "Please provide a Registered Name on Page 1 Part I Item 8",
        );
        check(
            &|d| d.registered_address_lines[0].clear(),
            "Please provide a Registered Address on Page 1 Part I Item 9",
        );
        check(
            &|d| d.contact_number.clear(),
            "Please provide a valid Contact Number on Page 1 Part I Item 11",
        );
        check(
            &|d| d.email.clear(),
            "Please provide a valid Email Address on Page 1 Part I Item 12",
        );
        check(
            &|d| d.email = "not-an-email".into(),
            "You have entered an invalid email address format!",
        );
        check(
            &|d| {
                d.part_iv.tax_credits.item_48_previous_quarter_withholding = WholePeso(500_000);
                d.recompute();
            },
            "Please select an option for Overpayment Radio Button on Page 1 Part II ",
        );
        check(
            &|d| d.payment_details[1].amount = WholePeso(257_500),
            "Please provide data on Page 1 Part III Item 27.",
        );
        check(
            &|d| {
                d.payment_details[0].amount = WholePeso(1_000);
                d.payment_details[0].drawee_bank_or_agency = "BANK".into();
                d.payment_details[0].number = "123".into();
                d.payment_details[0].date = Form1702RTDate::new(2026, 1, 5).ok();
            },
            "Sum of Amount fields in Details of Payment (Page 1 Part III) Segment must be equal to TOTAL AMOUNT PAYABLE",
        );
        check(
            &|d| {
                d.part_iv.tax_credits.item_53_other = Form1702RTNamedAmount {
                    description: String::new(),
                    amount: WholePeso(10),
                };
                d.recompute();
            },
            "Please provide data on Page 2 Part 4 Item 53.",
        );
        check(
            &|d| {
                d.schedule_2.rows[0].amount = WholePeso(10_000);
                d.schedule_2.rows[0].description = "Special".into();
                d.recompute();
            },
            "Please provide data on Page 3 Schedule 2 Item 1.",
        );
        check(
            &|d| {
                d.schedule_2.rows[0].amount = WholePeso(10_000);
                d.schedule_2.rows[0].description = "Special".into();
                d.schedule_2.rows[0].legal_basis = "RA 1".into();
                d.recompute();
            },
            "Please provide a value for this field (Part V Item 57)",
        );
        check(
            &|d| d.schedule_1.other[4].description = "Misc".into(),
            "Please provide data on Page 3 Schedule 1 Item 17g.",
        );
        check(
            &|d| {
                d.schedule_5.item_1_net_income_or_loss_per_books = WholePeso(1);
                d.recompute();
            },
            "Item Page 4 Schedule V Item 10 must be equal Item 39 on Page 2 Part IV",
        );
        check(
            &|d| {
                d.schedule_3.rows[1].year_incurred = "2025".into();
                d.schedule_3.rows[1].amount = WholePeso(1);
            },
            "Page 4 Schedule IIIA Item 5-7 Year Incurred cannot be equal to Page 1 Item 2 Year Ended.",
        );
        check(
            &|d| {
                d.schedule_3.rows[1].year_incurred = "2019".into();
                d.schedule_3.rows[1].amount = WholePeso(1);
            },
            "Year Incurred Cannot be less than 3 years in Page 1 Item 2 (Year Ended)",
        );
        check(
            &|d| {
                d.schedule_4.rows[0].year = "2024".into();
                d.schedule_4.rows[0].mcit = WholePeso(100);
                d.schedule_4.rows[0].applied_current_year = WholePeso(500);
                d.recompute();
            },
            "Page 4, Schedule IV: The sum of Columns D,E & F should not be greater than the amount in Column C. Please re-enter the correct values.",
        );
        check(
            &|d| {
                d.schedule_4.rows[0].year = "2020".into();
            },
            "Please enter a value for the year between 2022 and 2024.",
        );
        check(
            &|d| d.part_iv.item_27_sales = WholePeso(1),
            "Totals are out of date. Recompute the return.",
        );
    }

    #[test]
    fn old_stored_json_still_loads() {
        let mut json = serde_json::to_value(sample()).unwrap();
        let object = json.as_object_mut().unwrap();
        for key in ["queued_submission_fingerprint", "queue_authorization"] {
            object.remove(key);
        }
        for key in [
            "submitted_at",
            "confirmed_at",
            "submission_filename",
            "receipt_id",
            "next_retry_at",
        ] {
            object.insert(key.into(), serde_json::Value::Null);
        }
        object.insert("submission_attempts".into(), serde_json::json!(1));
        object.insert("last_error".into(), serde_json::json!("old"));
        object.insert("status".into(), serde_json::json!("Draft"));
        object.insert(
            "created_at".into(),
            serde_json::json!("2025-04-01T00:00:00+00:00"),
        );
        object.insert(
            "updated_at".into(),
            serde_json::json!("2025-04-02T00:00:00+00:00"),
        );
        let draft: Form1702RTDraft = serde_json::from_value(json).unwrap();
        assert_eq!(draft.lifecycle.status, FilingStatus::Draft);
        assert_eq!(draft.lifecycle.created_at, "2025-04-01T00:00:00+00:00");
        assert_eq!(draft.lifecycle.submission_attempts, 1);
        assert_eq!(draft.last_error.as_deref(), Some("old"));
        let back = serde_json::to_value(&draft).unwrap();
        assert_eq!(back["status"], "Draft");
        assert!(back.get("lifecycle").is_none());
    }

    #[test]
    fn queue_and_revalidate_through_the_generic_path() {
        let mut draft = sample();
        draft.queue(QueueAuthSource::Gui).unwrap();
        assert_eq!(draft.lifecycle.status, FilingStatus::Queued);
        assert!(draft.clone().revalidate_queued_before_submission().is_ok());
        draft.part_ii.item_17_surcharge = WholePeso(99);
        assert!(draft.revalidate_queued_before_submission().is_err());
    }
}
