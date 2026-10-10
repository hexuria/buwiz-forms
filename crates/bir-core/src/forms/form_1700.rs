//! BIR Form 1700 (January 2018) — Annual Income Tax Return for Individuals
//! Earning Purely Compensation Income.
//!
//! Ported from the official `BIR-Form1700v2018.hta` (eBIRForms 7.9.6.2.1):
//! the taxpayer and spouse compute chain (`computeSched1I5`,
//! `computePg2I44` … `computePg2I59`, `computePg1I30` … `computePg1I36`), the
//! radio rules (`processTaxpayerType`, `processCivilStatus`,
//! `processSpouseHasIncome`, `processFilingStatus`, the Schedule 1 employer
//! boxes), `validate()` with its exact alert texts, and the `saveXML` submit
//! loop through [`crate::official_xml`].
//!
//! Amount entries keep centavos (`round(this,2)`); computed items are
//! `toFixed(0)` whole pesos.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use super::queueable::{QueueableForm, SubmissionLifecycle};
use super::{FilingPeriod, FormValidator};
use crate::official_xml::{official_amount, parse_official_amount};
use crate::profile::TaxpayerProfile;

/// Rule-package id of the official layout.
pub const FORM_1700_FORM_ID: &str = "1700-v2013";
/// Schedule 1 employer rows (Items 1–4).
pub const FORM_1700_EMPLOYER_ROWS: usize = 4;
/// `init()` fixes Item 3.
pub const FORM_1700_ATC: &str = "II011";

/// Items 6 / 20.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form1700TaxpayerType {
    #[default]
    Unanswered,
    /// Employee, graduated rates (Part V.A).
    Employee,
    /// Non-resident alien not engaged in trade or business, 25% (Part V.B).
    Nranetb,
}

/// Item 15.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form1700CivilStatus {
    #[default]
    Unanswered,
    Single,
    Married,
    Separated,
    Widow,
}

/// Item 17.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form1700FilingStatus {
    #[default]
    Unanswered,
    Joint,
    Separate,
}

/// One column (A taxpayer/filer, B spouse) of Parts III and V.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Form1700Column {
    // Part V.A
    pub gross_compensation: f64,
    pub non_taxable: f64,
    pub taxable_compensation: f64,
    pub other_income: f64,
    pub taxable_income: f64,
    pub graduated_tax_due: f64,
    // Part V.B
    pub flat_gross_compensation: f64,
    pub flat_non_taxable: f64,
    pub flat_taxable_compensation: f64,
    pub flat_other_income: f64,
    pub flat_taxable_income: f64,
    pub flat_tax_due: f64,
    // Part V.C
    pub tax_withheld: f64,
    pub previously_filed: f64,
    pub foreign_tax_credits: f64,
    pub other_credits: f64,
    pub total_credits: f64,
    pub net_payable: f64,
    // Part III
    pub tax_due: f64,
    /// 29 — portion allowed for the 2nd installment.
    pub second_installment: f64,
    pub amount_payable: f64,
    pub interest: f64,
    pub surcharge: f64,
    pub compromise: f64,
    pub total_penalties: f64,
    pub total_amount_payable: f64,
}

/// One Schedule 1 employer row.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Form1700Employer {
    /// `true` for the spouse box, `false` for the taxpayer box.
    pub for_spouse: bool,
    pub name: String,
    /// The second name line.
    pub name2: String,
    /// 9 digits plus the branch code.
    pub tin: String,
    /// c — compensation subject to regular rates.
    pub regular: f64,
    /// d — compensation subject to the 25% flat rate.
    pub flat: f64,
    /// e — tax withheld.
    pub withheld: f64,
}

/// Part II — background information on the spouse.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Form1700Spouse {
    pub tin: String,
    pub rdo_code: String,
    pub taxpayer_type: Form1700TaxpayerType,
    pub name: String,
    pub contact_number: String,
    pub citizenship: String,
    pub foreign_tax_credits: Option<bool>,
    pub foreign_tax_number: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Form1700Draft {
    #[serde(default)]
    pub id: Option<i64>,

    /// 14 digits: 9-digit TIN then the 5-digit branch code.
    pub tin: String,
    pub taxable_year: u16,
    pub is_amended: bool,

    // Part I
    pub rdo_code: String,
    #[serde(default)]
    pub taxpayer_type: Form1700TaxpayerType,
    pub taxpayer_name: String,
    pub registered_address: String,
    pub zip_code: String,
    /// Item 9, `MM/DD/YYYY`.
    #[serde(default)]
    pub birth_date: String,
    pub email: String,
    #[serde(default)]
    pub citizenship: String,
    #[serde(default)]
    pub foreign_tax_credits: Option<bool>,
    #[serde(default)]
    pub foreign_tax_number: String,
    pub contact_number: String,
    #[serde(default)]
    pub civil_status: Form1700CivilStatus,
    #[serde(default)]
    pub spouse_has_income: Option<bool>,
    #[serde(default)]
    pub filing_status: Form1700FilingStatus,
    pub line_of_business: String,

    #[serde(default)]
    pub spouse: Form1700Spouse,
    #[serde(default)]
    pub taxpayer: Form1700Column,
    #[serde(default)]
    pub spouse_column: Form1700Column,

    /// Items 45, 49, 51 and 57 descriptions (shared by both columns).
    #[serde(default)]
    pub other_income_description: String,
    #[serde(default)]
    pub flat_non_taxable_description: String,
    #[serde(default)]
    pub flat_other_income_description: String,
    #[serde(default)]
    pub other_credits_description: String,

    /// Schedule 1 Items 1–4.
    #[serde(default)]
    pub employers: Vec<Form1700Employer>,
    /// Schedule 1 Item 5A / 5B (c, d, e).
    #[serde(default)]
    pub schedule_totals: [[f64; 3]; 2],

    /// Item 36.
    #[serde(default)]
    pub aggregate_amount_payable: f64,
    /// Item 37.
    #[serde(default)]
    pub number_of_attachments: u8,

    #[serde(flatten)]
    pub lifecycle: SubmissionLifecycle,
}

/// `toFixed(0)` (half away from zero on the value).
fn fixed0(value: f64) -> f64 {
    if value.is_finite() {
        value.round()
    } else {
        0.0
    }
}

/// `round(this,2)` / `formatCurrency`: the value an entry holds.
fn cents(value: f64) -> f64 {
    parse_official_amount(&official_amount(value)).unwrap_or(0.0)
}

fn digits(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}

/// `(TIN1, TIN2, TIN3, branch)` from a stored TIN with or without dashes;
/// the branch keeps what was entered (empty when none).
fn split_tin(tin: &str) -> (String, String, String, String) {
    let digits = digits(tin);
    let part = |range: std::ops::Range<usize>| digits.get(range).unwrap_or("").to_string();
    (
        part(0..3),
        part(3..6),
        part(6..9),
        digits.get(9..).unwrap_or("").to_string(),
    )
}

fn tin_is_valid(tin: &str) -> bool {
    let (a, b, c, branch) = split_tin(tin);
    a.len() == 3
        && b.len() == 3
        && c.len() == 3
        && branch.len() <= 5
        && tin.chars().all(|ch| ch.is_ascii_digit() || ch == '-')
}

fn check_digit_ok(tin: &str) -> bool {
    let (a, b, c, _) = split_tin(tin);
    crate::validation::relaxed_dev_mode()
        || crate::validation::official_tin_check_code(&format!("{a}{b}{c}")) == 0
}

/// `computeTaxDue`: the table for the taxable year, `toFixed(2)` then the
/// `toFixed(0)` the caller applies.
pub fn form_1700_graduated_tax(year: u16, income: f64) -> f64 {
    let x = income;
    let tax = if year >= 2023 {
        if x >= 8_000_000.0 {
            (x - 8_000_000.0) * (35.0 / 100.0) + 2_202_500.0
        } else if x >= 2_000_000.0 {
            (x - 2_000_000.0) * (30.0 / 100.0) + 402_500.0
        } else if x >= 800_000.0 {
            (x - 800_000.0) * (25.0 / 100.0) + 102_500.0
        } else if x >= 400_000.0 {
            (x - 400_000.0) * (20.0 / 100.0) + 22_500.0
        } else if x >= 250_000.0 {
            (x - 250_000.0) * (15.0 / 100.0)
        } else {
            0.0
        }
    } else if x >= 8_000_000.0 {
        (x - 8_000_000.0) * (35.0 / 100.0) + 2_410_000.0
    } else if x >= 2_000_000.0 {
        (x - 2_000_000.0) * (32.0 / 100.0) + 490_000.0
    } else if x >= 800_000.0 {
        (x - 800_000.0) * (30.0 / 100.0) + 130_000.0
    } else if x >= 400_000.0 {
        (x - 400_000.0) * (25.0 / 100.0) + 30_000.0
    } else if x >= 250_000.0 {
        (x - 250_000.0) * (20.0 / 100.0)
    } else {
        0.0
    };
    fixed0(cents(tax))
}

/// `validateMonthDayYearDate`: `true` when the text is not a valid
/// `MM/DD/YYYY` date.
fn birth_date_invalid(text: &str) -> bool {
    let parts: Vec<&str> = text.split('/').collect();
    if parts.len() != 3 || parts[0].len() != 2 || parts[1].len() != 2 || parts[2].len() != 4 {
        return true;
    }
    let (Ok(month), Ok(day), Ok(year)) = (
        parts[0].parse::<u32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<i32>(),
    ) else {
        return true;
    };
    NaiveDate::from_ymd_opt(year, month, day).is_none()
}

impl Form1700Column {
    fn round_inputs(&mut self) {
        for value in [
            &mut self.non_taxable,
            &mut self.other_income,
            &mut self.flat_non_taxable,
            &mut self.flat_other_income,
            &mut self.previously_filed,
            &mut self.foreign_tax_credits,
            &mut self.other_credits,
            &mut self.second_installment,
            &mut self.interest,
            &mut self.surcharge,
            &mut self.compromise,
        ] {
            *value = cents(*value);
        }
    }

    /// Part V and Part III for one column. `spouse` follows the page's
    /// slightly different rounding of Item 44B.
    fn compute(&mut self, year: u16, kind: Form1700TaxpayerType, spouse: bool) {
        self.taxable_compensation = if spouse {
            fixed0(self.gross_compensation - self.non_taxable)
        } else {
            self.gross_compensation - fixed0(self.non_taxable)
        };
        self.taxable_income = fixed0(self.taxable_compensation + self.other_income);
        self.graduated_tax_due = form_1700_graduated_tax(year, self.taxable_income);
        self.flat_taxable_compensation =
            fixed0(self.flat_gross_compensation - self.flat_non_taxable);
        self.flat_taxable_income = fixed0(self.flat_taxable_compensation + self.flat_other_income);
        self.flat_tax_due = fixed0(self.flat_taxable_income * (25.0 / 100.0));
        self.total_credits = fixed0(
            self.tax_withheld
                + self.previously_filed
                + self.foreign_tax_credits
                + self.other_credits,
        );
        self.tax_due = match kind {
            Form1700TaxpayerType::Nranetb => self.flat_tax_due,
            _ => self.graduated_tax_due,
        };
        self.net_payable = cents(self.tax_due - self.total_credits);
        let item28 = fixed0(self.net_payable);
        self.amount_payable = cents(item28 - fixed0(self.second_installment));
        self.total_penalties = fixed0(self.interest + self.surcharge + self.compromise);
        self.total_amount_payable = if self.amount_payable >= 0.0 {
            cents(self.amount_payable + self.total_penalties)
        } else if self.total_penalties == 0.0 {
            self.amount_payable
        } else {
            self.total_penalties
        };
    }
}

impl Form1700Draft {
    pub const FORM_CODE: &'static str = "1700";

    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16) -> Self {
        let mut draft = Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            is_amended: false,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_type: Form1700TaxpayerType::Unanswered,
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            birth_date: profile
                .birth_date
                .map(|d| d.format("%m/%d/%Y").to_string())
                .unwrap_or_default(),
            email: profile.email.clone(),
            citizenship: String::new(),
            foreign_tax_credits: None,
            foreign_tax_number: String::new(),
            contact_number: profile.phone.clone(),
            civil_status: Form1700CivilStatus::Unanswered,
            spouse_has_income: None,
            filing_status: Form1700FilingStatus::Unanswered,
            line_of_business: profile.line_of_business.clone(),
            spouse: Form1700Spouse::default(),
            taxpayer: Form1700Column::default(),
            spouse_column: Form1700Column::default(),
            other_income_description: String::new(),
            flat_non_taxable_description: String::new(),
            flat_other_income_description: String::new(),
            other_credits_description: String::new(),
            employers: Vec::new(),
            schedule_totals: [[0.0; 3]; 2],
            aggregate_amount_payable: 0.0,
            number_of_attachments: 0,
            lifecycle: SubmissionLifecycle::default(),
        };
        draft.recompute();
        draft
    }

    /// Part II and column B are open only on a joint return.
    pub fn is_joint(&self) -> bool {
        self.civil_status == Form1700CivilStatus::Married
            && self.spouse_has_income == Some(true)
            && self.filing_status == Form1700FilingStatus::Joint
    }

    fn spouse_type(&self) -> Form1700TaxpayerType {
        if self.is_joint() {
            self.spouse.taxpayer_type
        } else {
            Form1700TaxpayerType::Unanswered
        }
    }

    fn any_type(&self, kind: Form1700TaxpayerType) -> bool {
        self.taxpayer_type == kind || self.spouse_type() == kind
    }

    /// Schedule 1 column c is open when either filer is an employee.
    pub fn regular_column_open(&self) -> bool {
        self.any_type(Form1700TaxpayerType::Employee)
    }

    /// Schedule 1 column d is open when either filer is an NRANETB.
    pub fn flat_column_open(&self) -> bool {
        self.any_type(Form1700TaxpayerType::Nranetb)
    }

    /// The radio rules, then the official compute chain.
    pub fn recompute(&mut self) {
        if self.civil_status != Form1700CivilStatus::Married {
            self.spouse_has_income = None;
        }
        if self.spouse_has_income != Some(true) {
            self.filing_status = Form1700FilingStatus::Unanswered;
        }
        if !self.is_joint() {
            self.spouse = Form1700Spouse::default();
            self.spouse_column = Form1700Column::default();
            for row in &mut self.employers {
                if row.for_spouse {
                    *row = Form1700Employer::default();
                }
            }
        }
        if self.foreign_tax_credits != Some(true) {
            self.foreign_tax_number.clear();
            self.taxpayer.foreign_tax_credits = 0.0;
        }
        if self.spouse.foreign_tax_credits != Some(true) {
            self.spouse.foreign_tax_number.clear();
            self.spouse_column.foreign_tax_credits = 0.0;
        }
        if !self.is_amended {
            self.taxpayer.previously_filed = 0.0;
            self.spouse_column.previously_filed = 0.0;
        }
        let tp_type = self.taxpayer_type;
        let sp_type = self.spouse_type();
        // disablePartVA*/VB*: the part a filer type does not use is zero.
        for (column, kind) in [
            (&mut self.taxpayer, tp_type),
            (&mut self.spouse_column, sp_type),
        ] {
            if kind != Form1700TaxpayerType::Employee {
                column.non_taxable = 0.0;
                column.other_income = 0.0;
                column.second_installment = 0.0;
            }
            if kind != Form1700TaxpayerType::Nranetb {
                column.flat_non_taxable = 0.0;
                column.flat_other_income = 0.0;
            }
        }
        if !self.any_type(Form1700TaxpayerType::Employee) {
            self.other_income_description.clear();
        }
        if !self.any_type(Form1700TaxpayerType::Nranetb) {
            self.flat_non_taxable_description.clear();
            self.flat_other_income_description.clear();
        }
        let regular_open = self.regular_column_open();
        let flat_open = self.flat_column_open();
        self.employers.truncate(FORM_1700_EMPLOYER_ROWS);
        for row in &mut self.employers {
            if !regular_open {
                row.regular = 0.0;
            }
            if !flat_open {
                row.flat = 0.0;
            }
            row.regular = cents(row.regular);
            row.flat = cents(row.flat);
            row.withheld = cents(row.withheld);
        }
        while self
            .employers
            .last()
            .is_some_and(|row| *row == Form1700Employer::default())
        {
            self.employers.pop();
        }
        self.taxpayer.round_inputs();
        self.spouse_column.round_inputs();

        // computeSched1I5 (an unticked row counts toward the spouse total).
        let mut totals = [[0.0f64; 3]; 2];
        for row in &self.employers {
            let who = usize::from(row.for_spouse);
            totals[who][0] += row.regular;
            totals[who][1] += row.flat;
            totals[who][2] += row.withheld;
        }
        for total in totals.iter_mut().flatten() {
            *total = fixed0(*total);
        }
        self.schedule_totals = totals;
        let year = self.taxable_year;
        self.taxpayer.gross_compensation = 0.0;
        self.taxpayer.flat_gross_compensation = 0.0;
        if tp_type == Form1700TaxpayerType::Employee {
            self.taxpayer.gross_compensation = totals[0][0];
        } else {
            self.taxpayer.flat_gross_compensation = totals[0][1];
        }
        self.taxpayer.tax_withheld = totals[0][2];
        self.spouse_column.gross_compensation = 0.0;
        self.spouse_column.flat_gross_compensation = 0.0;
        match sp_type {
            Form1700TaxpayerType::Employee => self.spouse_column.gross_compensation = totals[1][0],
            Form1700TaxpayerType::Nranetb => {
                self.spouse_column.flat_gross_compensation = totals[1][1]
            }
            Form1700TaxpayerType::Unanswered => {}
        }
        self.spouse_column.tax_withheld = if self.is_joint() { totals[1][2] } else { 0.0 };
        self.taxpayer.compute(year, tp_type, false);
        self.spouse_column.compute(year, sp_type, true);
        self.aggregate_amount_payable =
            cents(self.taxpayer.total_amount_payable + self.spouse_column.total_amount_payable);
    }

    /// Texts typed before a later `capital()` trigger in form order are
    /// upper-cased: Item 11 (always), Items 21/23 on a joint return, and the
    /// Schedule 1 employer names on page 2.
    fn caps_after(&self, position: u8) -> bool {
        let employer_names = self
            .employers
            .iter()
            .any(|row| !row.name.trim().is_empty() || !row.name2.trim().is_empty());
        let spouse_trigger = self.is_joint()
            && (!self.spouse.name.trim().is_empty() || !self.spouse.citizenship.trim().is_empty());
        (position < 11 && !self.citizenship.trim().is_empty())
            || (position < 23 && spouse_trigger)
            || (position < 60 && employer_names)
    }

    /// The official field values the submit loop reads, keyed by element id.
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let mut put = |key: &str, value: String| {
            fields.insert(format!("frm1700:{key}"), value);
        };
        let flag = |on: bool| on.to_string();
        let money = official_amount;
        let typed = |value: &str, position: u8| {
            if self.caps_after(position) {
                value.to_uppercase()
            } else {
                value.to_string()
            }
        };

        put("txtPg1I1Year", self.taxable_year.to_string());
        put("txtPg1I3ATC", FORM_1700_ATC.to_string());
        put("rdoPg1I2AmendedYes", flag(self.is_amended));
        put("rdoPg1I2AmendedNo", flag(!self.is_amended));
        let (t1, t2, t3, branch) = split_tin(&self.tin);
        let branch = format!("{branch:0>5}");
        for (a, b, c, br) in [
            (
                "txtPg1I4TIN1",
                "txtPg1I4TIN2",
                "txtPg1I4TIN3",
                "txtPg1I4BranchCode",
            ),
            ("txtPg2TIN1", "txtPg2TIN2", "txtPg2TIN3", "txtPg2BranchCode"),
        ] {
            put(a, t1.clone());
            put(b, t2.clone());
            put(c, t3.clone());
            put(br, branch.clone());
        }
        put("txtRDOCode", self.rdo_code.trim().to_string());
        use Form1700TaxpayerType as T;
        put(
            "rdoPg1I6TaxpayerTypeE",
            flag(self.taxpayer_type == T::Employee),
        );
        put(
            "rdoPg1I6TaxpayerTypeN",
            flag(self.taxpayer_type == T::Nranetb),
        );
        let name = self.taxpayer_name.trim().to_uppercase();
        put("txtPg1I7TaxpayerName", name.clone());
        put(
            "txtPg2TaxpayerName",
            name.split(',').next().unwrap_or("").to_string(),
        );
        // loadBGData splits the address at 127 characters.
        let address = self.registered_address.trim().to_uppercase();
        let split = address
            .char_indices()
            .nth(127)
            .map_or(address.len(), |(at, _)| at);
        put("txtPg1I8Address", address[..split].to_string());
        put("txtPg1I8Address2", address[split..].to_string());
        put("txtPg1I8AZipCode", self.zip_code.trim().to_string());
        put("txtPg1I9BirthDate", self.birth_date.trim().to_string());
        // loadBGData shows Item 10 in capitals; the IAF filename keeps it.
        put("txtPg1I10Email", self.email.trim().to_uppercase());
        put(
            "txtPg1I11Citizenship",
            self.citizenship.trim().to_uppercase(),
        );
        put(
            "rdoPg1I12ForeignTaxCreditsYes",
            flag(self.foreign_tax_credits == Some(true)),
        );
        put(
            "rdoPg1I12ForeignTaxCreditsNo",
            flag(self.foreign_tax_credits == Some(false)),
        );
        put(
            "txtPg1I13ForeignTaxNumber",
            typed(&self.foreign_tax_number, 13),
        );
        put("txtPg1I14TelNum", self.contact_number.trim().to_string());
        use Form1700CivilStatus as C;
        put(
            "rdoPg1I15CivilStatusS",
            flag(self.civil_status == C::Single),
        );
        put(
            "rdoPg1I15CivilStatusM",
            flag(self.civil_status == C::Married),
        );
        put(
            "rdoPg1I15CivilStatusLS",
            flag(self.civil_status == C::Separated),
        );
        put("rdoPg1I15CivilStatusW", flag(self.civil_status == C::Widow));
        put(
            "rdoPg1I16SpouseHasIncomeY",
            flag(self.spouse_has_income == Some(true)),
        );
        put(
            "rdoPg1I16SpouseHasIncomeN",
            flag(self.spouse_has_income == Some(false)),
        );
        put(
            "rdoPg1I17FilingStatusJ",
            flag(self.filing_status == Form1700FilingStatus::Joint),
        );
        put(
            "rdoPg1I17FilingStatusS",
            flag(self.filing_status == Form1700FilingStatus::Separate),
        );

        // Part II: disableSpouseBGInformation empties the spouse TIN boxes,
        // the branch code included, whenever the return is not joint.
        let s = &self.spouse;
        let (s1, s2, s3, sb) = if self.is_joint() {
            split_tin(&s.tin)
        } else {
            Default::default()
        };
        put("txtPg1I18STIN1", s1);
        put("txtPg1I18STIN2", s2);
        put("txtPg1I18STIN3", s3);
        put("txtPg1I18SBranchCode", sb);
        put(
            "txtSpouseRDOCode",
            if self.is_joint() && !s.rdo_code.trim().is_empty() {
                s.rdo_code.trim().to_string()
            } else {
                "000".to_string()
            },
        );
        put(
            "rdoPg1I20SpouseTaxpayerTypeE",
            flag(s.taxpayer_type == T::Employee),
        );
        put(
            "rdoPg1I20SpouseTaxpayerTypeN",
            flag(s.taxpayer_type == T::Nranetb),
        );
        put("txtPg1I21SpouseName", s.name.trim().to_uppercase());
        put(
            "txtPg1I22TSpouseTelNum",
            s.contact_number.trim().to_string(),
        );
        put(
            "txtPg1I23SpouseCitizenship",
            s.citizenship.trim().to_uppercase(),
        );
        put(
            "rdoPg1I24SpouseForeignTaxCreditsYes",
            flag(s.foreign_tax_credits == Some(true)),
        );
        put(
            "rdoPg1I24SpouseForeignTaxCreditsNo",
            flag(s.foreign_tax_credits == Some(false)),
        );
        put(
            "txtPg1I25SpouseForeignTaxNumber",
            typed(&s.foreign_tax_number, 25),
        );

        for (suffix, column) in [("A", &self.taxpayer), ("B", &self.spouse_column)] {
            let mut p1 = |n: u8, value: f64| put(&format!("txtPg1I{n}{suffix}"), money(value));
            p1(26, column.tax_due);
            p1(27, column.total_credits);
            p1(28, fixed0(column.net_payable));
            p1(29, column.second_installment);
            p1(30, column.amount_payable);
            p1(31, column.interest);
            p1(32, column.surcharge);
            p1(33, column.compromise);
            p1(34, column.total_penalties);
            p1(35, column.total_amount_payable);
            let mut p2 = |n: u8, value: f64| put(&format!("txtPg2I{n}{suffix}"), money(value));
            p2(42, column.gross_compensation);
            p2(43, column.non_taxable);
            p2(44, column.taxable_compensation);
            p2(45, column.other_income);
            p2(46, column.taxable_income);
            p2(47, column.graduated_tax_due);
            p2(48, column.flat_gross_compensation);
            p2(49, column.flat_non_taxable);
            p2(50, column.flat_taxable_compensation);
            p2(51, column.flat_other_income);
            p2(52, column.flat_taxable_income);
            p2(53, column.flat_tax_due);
            p2(54, column.tax_withheld);
            p2(55, column.previously_filed);
            p2(56, column.foreign_tax_credits);
            p2(57, column.other_credits);
            p2(58, column.total_credits);
            p2(59, column.net_payable);
        }
        put("txtPg1I36", money(self.aggregate_amount_payable));
        put(
            "txtPg1I37NumberOfAttachments",
            if self.number_of_attachments == 0 {
                "00".to_string()
            } else {
                self.number_of_attachments.to_string()
            },
        );
        put("txtPg2I45Desc", typed(&self.other_income_description, 45));
        put(
            "txtPg2I49Desc",
            typed(&self.flat_non_taxable_description, 49),
        );
        put(
            "txtPg2I51Desc",
            typed(&self.flat_other_income_description, 51),
        );
        put("txtPg2I57Desc", typed(&self.other_credits_description, 57));

        for index in 0..FORM_1700_EMPLOYER_ROWS {
            let n = index + 1;
            let row = self.employers.get(index).cloned().unwrap_or_default();
            let used = row != Form1700Employer::default();
            put(
                &format!("rdoPg2I{n}PartVIEmployeeT"),
                flag(used && !row.for_spouse),
            );
            put(
                &format!("rdoPg2I{n}PartVIEmployeeS"),
                flag(used && row.for_spouse),
            );
            put(
                &format!("txtPg2I{n}PartVIEmployerName1"),
                row.name.trim().to_uppercase(),
            );
            put(
                &format!("txtPg2I{n}PartVIEmployerName2"),
                row.name2.trim().to_uppercase(),
            );
            let (e1, e2, e3, eb) = split_tin(&row.tin);
            put(&format!("txtPg2I{n}PartVIEmployerTIN1"), e1);
            put(&format!("txtPg2I{n}PartVIEmployerTIN2"), e2);
            put(&format!("txtPg2I{n}PartVIEmployerTIN3"), e3);
            put(&format!("txtPg2I{n}PartVIEmployerBranchCode"), eb);
            put(&format!("txtPg2ISched1c_{n}REG"), money(row.regular));
            put(&format!("txtPg2ISched1d_{n}CIFR"), money(row.flat));
            put(&format!("txtPg2ISched1e_{n}TW"), money(row.withheld));
        }
        for (who, suffix) in [(0usize, "A"), (1, "B")] {
            put(
                &format!("txtPg2ISched1c_5{suffix}REG"),
                money(self.schedule_totals[who][0]),
            );
            put(
                &format!("txtPg2ISched1d_5{suffix}CIFR"),
                money(self.schedule_totals[who][1]),
            );
            put(
                &format!("txtPg2ISched1e_5{suffix}TW"),
                money(self.schedule_totals[who][2]),
            );
        }
        // The submit loop writes "1" for the current page.
        put("txtCurrentPage", "1".to_string());
        put("txtLOB", self.line_of_business.trim().to_uppercase());
        fields
    }

    /// The exact official submit plaintext.
    pub fn to_bir_xml_payload(&self) -> Result<String, Vec<(String, String)>> {
        self.official_payload()
    }

    fn validate_on(&self, today: NaiveDate) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        let mut err = |field: &str, message: &str| {
            errors.push((field.to_string(), message.to_string()));
        };
        let year = i32::from(self.taxable_year);

        // validate(), in order.
        if self.taxable_year == 0 {
            err("taxable_year", "Please enter a valid year in Item 1.");
        }
        if !tin_is_valid(&self.tin) || split_tin(&self.tin).3.len() < 3 {
            err("tin", "Please enter a valid TIN number on Item 4.");
        } else if !check_digit_ok(&self.tin) {
            err("tin", crate::validation::OFFICIAL_INVALID_TIN_MESSAGE);
        }
        if !crate::validation::rdo_code_is_official_option(self.rdo_code.trim()) {
            err("rdo_code", "Please enter a valid RDO Code on Item 5.");
        }
        if self.taxpayer_type == Form1700TaxpayerType::Unanswered {
            err("taxpayer_type", "Please select Taxpayer Type on Item no. 6");
        }
        let name = self.taxpayer_name.trim();
        if name.is_empty() || name.chars().count() > 50 {
            err(
                "taxpayer_name",
                "Please enter a valid Taxpayer Name on Item 7.",
            );
        }
        let address = self.registered_address.trim();
        if address.is_empty() || address.chars().count() > 227 {
            err(
                "registered_address",
                "Please enter Taxpayer's Registered Address on Item 8.",
            );
        }
        let zip = self.zip_code.trim();
        if zip.is_empty() || zip.len() > 12 || !zip.bytes().all(|b| b.is_ascii_digit()) {
            err(
                "zip_code",
                "Please enter Taxpayer's Registered Address on Item 8A.",
            );
        }
        let birth = self.birth_date.trim();
        if birth.is_empty() {
            err(
                "birth_date",
                "Please indicate Birth Date of Taxpayer on item 9.",
            );
        } else if birth_date_invalid(birth) {
            err(
                "birth_date",
                "Invalid birth date on item 9 of Taxpayer.  Please check date format.",
            );
        }
        if self.citizenship.trim().is_empty() {
            err(
                "citizenship",
                "Please enter Taxpayer's Citizenship on Item 11",
            );
        }
        if self.civil_status == Form1700CivilStatus::Unanswered {
            err("civil_status", "Please indicate Civil Status on Item 15");
        }
        if self.civil_status == Form1700CivilStatus::Married {
            match self.spouse_has_income {
                Some(true) => match self.filing_status {
                    Form1700FilingStatus::Joint => {
                        let s = &self.spouse;
                        let (a, b, c, _) = split_tin(&s.tin);
                        if a.is_empty() || b.is_empty() || c.is_empty() {
                            err("spouse.tin", "Please enter Spouse TIN");
                        } else if !tin_is_valid(&s.tin) {
                            err("spouse.tin", "Please enter a valid TIN number on Item 18.");
                        } else if !check_digit_ok(&s.tin) {
                            err(
                                "spouse.tin",
                                &format!(
                                    "{} on Item 18.",
                                    crate::validation::OFFICIAL_INVALID_TIN_MESSAGE
                                ),
                            );
                        }
                        if !crate::validation::rdo_code_is_official_option(s.rdo_code.trim()) {
                            err(
                                "spouse.rdo_code",
                                "Please enter a valid RDO Code on Item 19.",
                            );
                        }
                        if s.name.trim().is_empty() {
                            err("spouse.name", "Please enter Spouse Name on Item 21.");
                        }
                        if s.taxpayer_type == Form1700TaxpayerType::Unanswered {
                            // Not checked by validate(); the spouse column has
                            // no tax base without it.
                            err(
                                "spouse.taxpayer_type",
                                "Please select Taxpayer Type on Item no. 20",
                            );
                        }
                    }
                    Form1700FilingStatus::Separate => {}
                    Form1700FilingStatus::Unanswered => {
                        err("filing_status", "Please choose Yes or No on Item 17");
                    }
                },
                Some(false) => {}
                None => err("spouse_has_income", "Please choose Yes or No on Item 16"),
            }
        }
        for (index, row) in self.employers.iter().enumerate() {
            let n = index + 1;
            if *row == Form1700Employer::default() {
                continue;
            }
            if row.name.trim().is_empty() && row.name2.trim().is_empty() {
                err(
                    &format!("employers[{index}].name"),
                    &format!("Please enter Employer's name on Part VI Item {n}A "),
                );
                continue;
            }
            let (a, b, c, branch) = split_tin(&row.tin);
            if a.is_empty() || b.is_empty() || c.is_empty() {
                err(
                    &format!("employers[{index}].tin"),
                    &format!("Please enter Employer's TIN on Part VI Item {n}B"),
                );
                continue;
            }
            if !tin_is_valid(&row.tin) || branch.len() < 3 {
                err(
                    &format!("employers[{index}].tin"),
                    &format!("Please enter valid TIN on Part VI Item {n}B"),
                );
                continue;
            }
            if !check_digit_ok(&row.tin) {
                err(
                    &format!("employers[{index}].tin"),
                    &format!(
                        "{} Part VI Item {n}B.",
                        crate::validation::OFFICIAL_INVALID_TIN_MESSAGE
                    ),
                );
                continue;
            }
            let zero_message = format!("Page 2 Item {n} Compensation Income should not be zero.");
            if self.regular_column_open() && row.regular == 0.0 {
                err(&format!("employers[{index}].regular"), &zero_message);
                continue;
            }
            if self.flat_column_open() && row.flat == 0.0 {
                err(&format!("employers[{index}].flat"), &zero_message);
            }
        }

        // checkYear and the Item 29 rules applied while typing.
        if year > today.year() {
            err(
                "taxable_year",
                "Invalid year input. Year should not be later to the Current Year.",
            );
        }
        if self.taxable_year != 0 && year < 2018 {
            err(
                "taxable_year",
                "Invalid year input. Year should not be lower than 2018.",
            );
        }
        if let Ok(date) = NaiveDate::parse_from_str(birth, "%m/%d/%Y")
            && date > today
        {
            err("birth_date", "Birth year should not be a future date.");
        }
        for (suffix, column) in [("A", &self.taxpayer), ("B", &self.spouse_column)] {
            let field = if suffix == "A" {
                "taxpayer"
            } else {
                "spouse_column"
            };
            if column.tax_due * 0.5 < column.second_installment {
                err(
                    &format!("{field}.second_installment"),
                    &format!(
                        "Amount in Item 29{suffix} cannot be more than 50% of Item 26{suffix}."
                    ),
                );
            } else if column.total_credits != 0.0
                && column.second_installment != 0.0
                && column.second_installment > fixed0(column.net_payable)
            {
                err(
                    &format!("{field}.second_installment"),
                    &format!("Amount in Item 29{suffix} cannot be greater than Item 28{suffix}."),
                );
            }
            for (name, value) in [
                ("non_taxable", column.non_taxable),
                ("other_income", column.other_income),
                ("flat_non_taxable", column.flat_non_taxable),
                ("flat_other_income", column.flat_other_income),
                ("previously_filed", column.previously_filed),
                ("foreign_tax_credits", column.foreign_tax_credits),
                ("other_credits", column.other_credits),
                ("second_installment", column.second_installment),
                ("interest", column.interest),
                ("surcharge", column.surcharge),
                ("compromise", column.compromise),
            ] {
                if value < 0.0 {
                    err(&format!("{field}.{name}"), "Enter a non-negative amount.");
                }
            }
        }
        for (index, row) in self.employers.iter().enumerate() {
            if row.for_spouse && !self.is_joint() {
                err(
                    &format!("employers[{index}].for_spouse"),
                    "A spouse employer row needs a joint return (Item 17).",
                );
            }
            if row.regular < 0.0 || row.flat < 0.0 || row.withheld < 0.0 {
                err(
                    &format!("employers[{index}]"),
                    "Enter a non-negative amount.",
                );
            }
        }
        let email = self.email.trim();
        if email.is_empty() || !email.contains('@') || email.contains(char::is_whitespace) {
            err(
                "email",
                "Enter the email address BIR sends the filing confirmation to.",
            );
        }
        for (field, text, limit) in [
            ("citizenship", &self.citizenship, 20),
            ("foreign_tax_number", &self.foreign_tax_number, 20),
            (
                "other_income_description",
                &self.other_income_description,
                25,
            ),
            (
                "flat_non_taxable_description",
                &self.flat_non_taxable_description,
                25,
            ),
            (
                "flat_other_income_description",
                &self.flat_other_income_description,
                25,
            ),
            (
                "other_credits_description",
                &self.other_credits_description,
                25,
            ),
        ] {
            if text.chars().count() > limit {
                err(field, &format!("At most {limit} characters."));
            }
        }
        if self.number_of_attachments > 99 {
            err("number_of_attachments", "Item 37 holds at most two digits.");
        }

        // Derived items must be what the official compute chain produces.
        let mut expected = self.clone();
        expected.recompute();
        if expected != *self {
            err(
                "aggregate_amount_payable",
                "Totals are out of date. Recompute the return.",
            );
        }
        errors
    }
}

impl FormValidator for Form1700Draft {
    /// `validate()` in order, with its alert texts, plus the input limits
    /// the official page enforces while typing.
    fn validate(&self) -> Vec<(String, String)> {
        self.validate_on(chrono::Local::now().date_naive())
    }
}

impl QueueableForm for Form1700Draft {
    const FORM_CODE: &'static str = "1700";
    /// Official `formType` and SFTP folder (`ftpTargetFolder.PROD['1700v2018']`).
    const FORM_TYPE: &'static str = "1700v2018";
    const LAYOUT_ID: &'static str = FORM_1700_FORM_ID;

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
    fn filing_period(&self) -> FilingPeriod {
        FilingPeriod::Annual
    }
    /// `txtPg1I1Year`, as in `createXMLFileName`.
    fn period_code(&self) -> String {
        self.taxable_year.to_string()
    }
    fn parse_period_code(code: &str) -> Option<(u16, FilingPeriod)> {
        (code.len() == 4 && code.bytes().all(|b| b.is_ascii_digit()))
            .then(|| code.parse().ok())
            .flatten()
            .map(|year| (year, FilingPeriod::Annual))
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
        self.to_bir_field_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()
    }

    pub(crate) fn sample() -> Form1700Draft {
        let mut d = Form1700Draft {
            id: None,
            tin: "12345678800000".to_string(),
            taxable_year: 2025,
            is_amended: false,
            rdo_code: "039".to_string(),
            taxpayer_type: Form1700TaxpayerType::Employee,
            taxpayer_name: "Dummy, Sample Taxpayer".to_string(),
            registered_address: "123 Sample Street, Barangay Example, Quezon City".to_string(),
            zip_code: "1100".to_string(),
            birth_date: "01/15/1980".to_string(),
            email: "sample.taxpayer@example.com".to_string(),
            citizenship: "Filipino".to_string(),
            foreign_tax_credits: Some(false),
            foreign_tax_number: String::new(),
            contact_number: "09170000000".to_string(),
            civil_status: Form1700CivilStatus::Single,
            spouse_has_income: None,
            filing_status: Form1700FilingStatus::Unanswered,
            line_of_business: "Sample Consulting Services".to_string(),
            spouse: Form1700Spouse::default(),
            taxpayer: Form1700Column {
                non_taxable: 90_000.0,
                other_income: 10_000.49,
                other_credits: 100.0,
                surcharge: 1_000.0,
                ..Form1700Column::default()
            },
            spouse_column: Form1700Column::default(),
            other_income_description: "Prize".to_string(),
            flat_non_taxable_description: String::new(),
            flat_other_income_description: String::new(),
            other_credits_description: "Sample credit".to_string(),
            employers: vec![Form1700Employer {
                for_spouse: false,
                name: "Sample Employer Inc".to_string(),
                name2: String::new(),
                tin: "12345678800000".to_string(),
                regular: 600_000.5,
                flat: 0.0,
                withheld: 40_000.0,
            }],
            schedule_totals: [[0.0; 3]; 2],
            aggregate_amount_payable: 0.0,
            number_of_attachments: 0,
            lifecycle: SubmissionLifecycle::default(),
        };
        d.recompute();
        d
    }

    fn messages(d: &Form1700Draft) -> Vec<String> {
        d.validate_on(today())
            .into_iter()
            .map(|(_, message)| message)
            .collect()
    }

    #[test]
    fn compute_chain_matches_the_official_page() {
        // Values the official page shows for the same entries (runtime.js).
        let d = sample();
        let tp = &d.taxpayer;
        assert_eq!(d.schedule_totals[0], [600_001.0, 0.0, 40_000.0]);
        assert_eq!(tp.gross_compensation, 600_001.0);
        assert_eq!(tp.taxable_compensation, 510_001.0);
        assert_eq!(tp.taxable_income, 520_001.0);
        assert_eq!(tp.graduated_tax_due, 46_500.0);
        assert_eq!(tp.total_credits, 40_100.0);
        assert_eq!(tp.net_payable, 6_400.0);
        assert_eq!(tp.total_amount_payable, 7_400.0);
        assert_eq!(d.aggregate_amount_payable, 7_400.0);
        assert!(messages(&d).is_empty(), "{:?}", messages(&d));
    }

    #[test]
    fn field_map_uses_official_formats() {
        let d = sample();
        let f = d.to_bir_field_map();
        assert_eq!(f["frm1700:txtPg1I7TaxpayerName"], "DUMMY, SAMPLE TAXPAYER");
        assert_eq!(f["frm1700:txtPg2TaxpayerName"], "DUMMY");
        assert_eq!(f["frm1700:txtPg1I10Email"], "SAMPLE.TAXPAYER@EXAMPLE.COM");
        assert_eq!(f["frm1700:txtPg1I11Citizenship"], "FILIPINO");
        assert_eq!(f["frm1700:txtPg2I45Desc"], "PRIZE");
        assert_eq!(
            f["frm1700:txtPg2I1PartVIEmployerName1"],
            "SAMPLE EMPLOYER INC"
        );
        assert_eq!(f["frm1700:txtPg1I18SBranchCode"], "");
        assert_eq!(f["frm1700:txtSpouseRDOCode"], "000");
        assert_eq!(f["frm1700:txtPg1I37NumberOfAttachments"], "00");
        assert_eq!(
            d.submission_filename(),
            "12345678800000-1700v2018-2025#sample.taxpayer@example.com#.xml"
        );
        assert!(d.to_bir_xml_payload().is_ok());
    }

    #[test]
    fn period_codes_round_trip() {
        let d = sample();
        assert_eq!(d.period_code(), "2025");
        assert_eq!(
            Form1700Draft::parse_period_code("2025"),
            Some((2025, FilingPeriod::Annual))
        );
        assert_eq!(Form1700Draft::parse_period_code("122025"), None);
    }

    #[test]
    fn official_negative_cases_are_rejected_with_their_alerts() {
        let check = |mutate: &dyn Fn(&mut Form1700Draft), expected: &str| {
            let mut d = sample();
            mutate(&mut d);
            let found = messages(&d);
            assert!(
                found.iter().any(|m| m == expected),
                "{expected:?} not in {found:?}"
            );
        };
        check(
            &|d| d.taxable_year = 0,
            "Please enter a valid year in Item 1.",
        );
        check(
            &|d| d.tin = "1".into(),
            "Please enter a valid TIN number on Item 4.",
        );
        check(
            &|d| d.rdo_code.clear(),
            "Please enter a valid RDO Code on Item 5.",
        );
        check(
            &|d| {
                d.taxpayer_type = Form1700TaxpayerType::Unanswered;
                d.recompute();
            },
            "Please select Taxpayer Type on Item no. 6",
        );
        check(
            &|d| d.taxpayer_name.clear(),
            "Please enter a valid Taxpayer Name on Item 7.",
        );
        check(
            &|d| d.registered_address.clear(),
            "Please enter Taxpayer's Registered Address on Item 8.",
        );
        check(
            &|d| d.zip_code.clear(),
            "Please enter Taxpayer's Registered Address on Item 8A.",
        );
        check(
            &|d| d.birth_date.clear(),
            "Please indicate Birth Date of Taxpayer on item 9.",
        );
        check(
            &|d| d.birth_date = "1/15/1980".into(),
            "Invalid birth date on item 9 of Taxpayer.  Please check date format.",
        );
        check(
            &|d| d.citizenship.clear(),
            "Please enter Taxpayer's Citizenship on Item 11",
        );
        check(
            &|d| {
                d.civil_status = Form1700CivilStatus::Unanswered;
                d.recompute();
            },
            "Please indicate Civil Status on Item 15",
        );
        check(
            &|d| {
                d.civil_status = Form1700CivilStatus::Married;
                d.recompute();
            },
            "Please choose Yes or No on Item 16",
        );
        check(
            &|d| {
                d.civil_status = Form1700CivilStatus::Married;
                d.spouse_has_income = Some(true);
                d.recompute();
            },
            "Please choose Yes or No on Item 17",
        );
        let joint = |d: &mut Form1700Draft| {
            d.civil_status = Form1700CivilStatus::Married;
            d.spouse_has_income = Some(true);
            d.filing_status = Form1700FilingStatus::Joint;
            d.spouse = Form1700Spouse {
                tin: "12345678800000".into(),
                rdo_code: "039".into(),
                taxpayer_type: Form1700TaxpayerType::Employee,
                name: "Dummy, Sample Spouse".into(),
                ..Form1700Spouse::default()
            };
        };
        check(
            &|d| {
                joint(d);
                d.spouse.tin.clear();
                d.recompute();
            },
            "Please enter Spouse TIN",
        );
        check(
            &|d| {
                joint(d);
                d.spouse.tin = "12345678900000".into();
                d.recompute();
            },
            "You have entered an incorrect TIN on Item 18.",
        );
        check(
            &|d| {
                joint(d);
                d.spouse.rdo_code.clear();
                d.recompute();
            },
            "Please enter a valid RDO Code on Item 19.",
        );
        check(
            &|d| {
                joint(d);
                d.spouse.name.clear();
                d.recompute();
            },
            "Please enter Spouse Name on Item 21.",
        );
        check(
            &|d| {
                d.employers[0].name.clear();
                d.recompute();
            },
            "Please enter Employer's name on Part VI Item 1A ",
        );
        check(
            &|d| {
                d.employers[0].tin.clear();
                d.recompute();
            },
            "Please enter Employer's TIN on Part VI Item 1B",
        );
        check(
            &|d| {
                d.employers[0].tin = "123456788".into();
                d.recompute();
            },
            "Please enter valid TIN on Part VI Item 1B",
        );
        check(
            &|d| {
                d.employers[0].tin = "12345678900000".into();
                d.recompute();
            },
            "You have entered an incorrect TIN Part VI Item 1B.",
        );
        check(
            &|d| {
                d.employers[0].regular = 0.0;
                d.recompute();
            },
            "Page 2 Item 1 Compensation Income should not be zero.",
        );
        check(
            &|d| d.taxable_year = 2027,
            "Invalid year input. Year should not be later to the Current Year.",
        );
        check(
            &|d| {
                d.taxpayer.second_installment = 30_000.0;
                d.recompute();
            },
            "Amount in Item 29A cannot be more than 50% of Item 26A.",
        );
    }

    #[test]
    fn flat_rate_column() {
        let mut d = sample();
        d.taxpayer_type = Form1700TaxpayerType::Nranetb;
        d.employers[0].regular = 0.0;
        d.employers[0].flat = 400_000.0;
        d.employers[0].withheld = 90_000.0;
        d.recompute();
        let tp = &d.taxpayer;
        assert_eq!(tp.non_taxable, 0.0);
        assert_eq!(tp.flat_gross_compensation, 400_000.0);
        assert_eq!(tp.flat_tax_due, 100_000.0);
        assert_eq!(tp.tax_due, 100_000.0);
        assert!(d.other_income_description.is_empty());
        assert!(messages(&d).is_empty(), "{:?}", messages(&d));
    }

    #[test]
    fn joint_returns_add_the_spouse_column() {
        let mut d = sample();
        d.civil_status = Form1700CivilStatus::Married;
        d.spouse_has_income = Some(true);
        d.filing_status = Form1700FilingStatus::Joint;
        d.spouse = Form1700Spouse {
            tin: "12345678800000".into(),
            rdo_code: "039".into(),
            taxpayer_type: Form1700TaxpayerType::Employee,
            name: "Dummy, Sample Spouse".into(),
            ..Form1700Spouse::default()
        };
        d.employers.push(Form1700Employer {
            for_spouse: true,
            name: "Sample Spouse Employer".into(),
            tin: "12345678800000".into(),
            regular: 400_000.0,
            withheld: 20_000.0,
            ..Form1700Employer::default()
        });
        d.recompute();
        let sp = &d.spouse_column;
        assert_eq!(d.schedule_totals[1], [400_000.0, 0.0, 20_000.0]);
        assert_eq!(sp.graduated_tax_due, 22_500.0);
        assert_eq!(sp.net_payable, 2_500.0);
        assert!(messages(&d).is_empty(), "{:?}", messages(&d));
        d.filing_status = Form1700FilingStatus::Separate;
        d.recompute();
        assert_eq!(d.employers.len(), 1);
        assert_eq!(d.spouse_column, Form1700Column::default());
    }

    #[test]
    fn queue_and_revalidate_through_the_generic_path() {
        let mut d = sample();
        d.queue(crate::filing_queue::QueueAuthSource::Gui).unwrap();
        assert!(d.revalidate_queued_before_submission().is_ok());
        d.taxpayer.surcharge = 99.0;
        assert!(d.revalidate_queued_before_submission().is_err());
    }
}
