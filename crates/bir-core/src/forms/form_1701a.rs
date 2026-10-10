//! BIR Form 1701A (January 2018) — Annual Income Tax Return for Individuals
//! Earning Income PURELY from Business/Profession (graduated rates with OSD,
//! or the 8% rate).
//!
//! Ported from the official `BIR-Form1701A.hta` (eBIRForms 7.9.6.2.1): the
//! taxpayer and spouse compute chain (`computeTxt20` … `computeTxt65`), the
//! radio rules (`processTaxpayerType`, `processATC`, `processAmend`,
//! `processFTC`, `processCivilStatus`, `processSpouseIncome`,
//! `processFilingStatus`, `enable/disablePartIVA/IVB`, `enable/disableSpouse`),
//! `validate()` with its exact alert texts, and `saveXMLsubmit` through
//! [`crate::official_xml`].
//!
//! Amount entries keep centavos (`round(this,2)`); computed items are
//! `toFixed(0)` whole pesos. The page's disable handlers write a bare `0`
//! into the controls they clear, and that text is what it submits. The field
//! map reproduces the state of a filer who answers the radio items in form
//! order (Items 2, 3, 6, 7, 13, 16–18, then the spouse items) before typing
//! amounts; see [`Form1701ADraft::to_bir_field_map`].

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use super::queueable::{QueueableForm, SubmissionLifecycle};
use super::{FilingPeriod, FormValidator};
use crate::official_xml::{official_amount, parse_official_amount};
use crate::profile::TaxpayerProfile;

/// Rule-package id of the official layout.
pub const FORM_1701A_FORM_ID: &str = "1701a-v2018";

const ITEM_53_ALERT: &str = "Your Gross Sales/Receipts and Other Non-Operating Income exceeds VAT Threshold (P3M), thus, not qualified to 8% tax rate and shall be subjected to graduated rates. Please choose ATC and fill in Schedule IV.A if method of Deduction is OSD. Otherwise, use BIR Form 1701.";

/// Items 6 / 68.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form1701AFilerType {
    #[default]
    Unanswered,
    SingleProprietor,
    Professional,
}

/// Items 7 / 69.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form1701AAtc {
    #[default]
    Unanswered,
    /// II012 Business income, graduated rates.
    II012,
    /// II014 Income from profession, graduated rates.
    II014,
    /// II015 Business income, 8%.
    II015,
    /// II017 Income from profession, 8%.
    II017,
}

impl Form1701AAtc {
    pub fn is_graduated(self) -> bool {
        matches!(self, Self::II012 | Self::II014)
    }

    pub fn is_eight_percent(self) -> bool {
        matches!(self, Self::II015 | Self::II017)
    }

    /// `processTaxpayerType`: proprietors pick II012/II015, professionals
    /// II014/II017.
    fn allowed_for(self, filer: Form1701AFilerType) -> bool {
        match filer {
            Form1701AFilerType::Unanswered => false,
            Form1701AFilerType::SingleProprietor => matches!(self, Self::II012 | Self::II015),
            Form1701AFilerType::Professional => matches!(self, Self::II014 | Self::II017),
        }
    }
}

/// Item 16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form1701ACivilStatus {
    #[default]
    Unanswered,
    Single,
    Married,
    Separated,
    Widow,
}

/// Item 18.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form1701AFilingStatus {
    #[default]
    Unanswered,
    Joint,
    Separate,
}

/// The overpayment boxes after Item 30.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form1701AOverpayment {
    #[default]
    None,
    Refund,
    TaxCreditCertificate,
    CarryOver,
}

/// One column (A taxpayer/filer, B spouse) of Parts II and IV.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Form1701AColumn {
    // Part IV.A — graduated rates with OSD.
    pub sales: f64,
    pub sales_returns: f64,
    pub net_sales: f64,
    pub osd: f64,
    pub net_income: f64,
    pub other_income_41: f64,
    pub other_income_42: f64,
    /// 43 — share in a general professional partnership.
    pub gpp_share: f64,
    pub total_other_income: f64,
    pub taxable_income: f64,
    pub graduated_tax_due: f64,
    // Part IV.B — 8%.
    pub eight_sales: f64,
    pub eight_sales_returns: f64,
    pub eight_net_sales: f64,
    pub eight_other_income_50: f64,
    pub eight_other_income_51: f64,
    pub eight_total_other_income: f64,
    pub eight_total_income: f64,
    /// 54 — the ₱250,000 reduction (at most 250,000).
    pub eight_reduction: f64,
    pub eight_taxable_income: f64,
    pub eight_tax_due: f64,
    // Part IV.C — credits.
    pub prior_year_excess: f64,
    pub quarterly_payments: f64,
    pub cwt_q1_q3: f64,
    pub cwt_q4: f64,
    pub previously_filed: f64,
    pub foreign_tax_credits: f64,
    pub other_credits: f64,
    pub total_credits: f64,
    pub net_payable: f64,
    // Part II.
    pub tax_due: f64,
    /// 23 — portion allowed for the 2nd installment.
    pub second_installment: f64,
    pub amount_payable: f64,
    pub surcharge: f64,
    pub interest: f64,
    pub compromise: f64,
    pub total_penalties: f64,
    pub total_amount_payable: f64,
}

/// Part V — background information on the spouse.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Form1701ASpouse {
    pub tin: String,
    pub rdo_code: String,
    pub filer_type: Form1701AFilerType,
    pub atc: Form1701AAtc,
    pub name: String,
    pub contact_number: String,
    pub citizenship: String,
    /// `None` until Item 72 is answered.
    pub foreign_tax_credits: Option<bool>,
    pub foreign_tax_number: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Form1701ADraft {
    #[serde(default)]
    pub id: Option<i64>,

    // Items 1–3
    /// 14 digits: 9-digit TIN then the 5-digit branch code.
    pub tin: String,
    pub taxable_year: u16,
    /// Item 1 month: 12 unless a short period return.
    pub month: u8,
    pub is_amended: bool,
    pub is_short_period: bool,

    // Part I
    pub rdo_code: String,
    #[serde(default)]
    pub filer_type: Form1701AFilerType,
    #[serde(default)]
    pub atc: Form1701AAtc,
    pub taxpayer_name: String,
    pub registered_address: String,
    pub zip_code: String,
    /// Item 10, `MM/DD/YYYY`.
    #[serde(default)]
    pub birth_date: String,
    pub email: String,
    #[serde(default)]
    pub citizenship: String,
    #[serde(default)]
    pub foreign_tax_credits: bool,
    #[serde(default)]
    pub foreign_tax_number: String,
    pub contact_number: String,
    #[serde(default)]
    pub civil_status: Form1701ACivilStatus,
    /// Item 17, only when married.
    #[serde(default)]
    pub spouse_has_income: Option<bool>,
    #[serde(default)]
    pub filing_status: Form1701AFilingStatus,
    pub line_of_business: String,

    #[serde(default)]
    pub taxpayer: Form1701AColumn,
    #[serde(default)]
    pub spouse_column: Form1701AColumn,
    #[serde(default)]
    pub spouse: Form1701ASpouse,

    /// Items 41, 42, 50, 51 and 63 descriptions (shared by both columns).
    #[serde(default)]
    pub other_income_41_description: String,
    #[serde(default)]
    pub other_income_42_description: String,
    #[serde(default)]
    pub eight_other_income_50_description: String,
    #[serde(default)]
    pub eight_other_income_51_description: String,
    #[serde(default)]
    pub other_credits_description: String,

    /// Item 30.
    #[serde(default)]
    pub aggregate_amount_payable: f64,
    #[serde(default)]
    pub overpayment: Form1701AOverpayment,
    /// Item 31.
    #[serde(default)]
    pub number_of_attachments: u8,

    #[serde(flatten)]
    pub lifecycle: SubmissionLifecycle,
}

/// `toFixed(0)` (half away from zero on the exact value).
fn fixed0(value: f64) -> f64 {
    if value.is_finite() {
        value.round()
    } else {
        0.0
    }
}

/// `round(this,2)`: the value an entry holds.
fn cents(value: f64) -> f64 {
    parse_official_amount(&official_amount(value)).unwrap_or(0.0)
}

fn digits(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}

/// `(TIN1, TIN2, TIN3, branch)` from a stored TIN with or without dashes.
fn split_tin(tin: &str) -> (String, String, String, String) {
    let digits = digits(tin);
    let part = |range: std::ops::Range<usize>| digits.get(range).unwrap_or("").to_string();
    let branch = digits.get(9..).unwrap_or("");
    (part(0..3), part(3..6), part(6..9), format!("{branch:0>5}"))
}

/// `computeTxt46`: the graduated table for the taxable year.
pub fn form_1701a_graduated_tax(year: u16, income: f64) -> f64 {
    let x = income;
    let tax = if x <= 0.0 {
        0.0
    } else if (2018..=2022).contains(&year) {
        if x <= 250_000.0 {
            0.0
        } else if x <= 400_000.0 {
            (x - 250_000.0) * (20.0 / 100.0)
        } else if x <= 800_000.0 {
            (x - 400_000.0) * (25.0 / 100.0) + 30_000.0
        } else if x <= 2_000_000.0 {
            (x - 800_000.0) * (30.0 / 100.0) + 130_000.0
        } else if x <= 8_000_000.0 {
            (x - 2_000_000.0) * (32.0 / 100.0) + 490_000.0
        } else {
            (x - 8_000_000.0) * (35.0 / 100.0) + 2_410_000.0
        }
    } else if year > 2022 {
        if x <= 250_000.0 {
            0.0
        } else if x <= 400_000.0 {
            (x - 250_000.0) * (15.0 / 100.0)
        } else if x <= 800_000.0 {
            (x - 400_000.0) * (20.0 / 100.0) + 22_500.0
        } else if x <= 2_000_000.0 {
            (x - 800_000.0) * (25.0 / 100.0) + 102_500.0
        } else if x <= 8_000_000.0 {
            (x - 2_000_000.0) * (30.0 / 100.0) + 402_500.0
        } else {
            (x - 8_000_000.0) * (35.0 / 100.0) + 2_202_500.0
        }
    } else if x < 10_000.0 {
        x * (5.0 / 100.0)
    } else if x < 30_000.0 {
        (x - 10_000.0) * (10.0 / 100.0) + 500.0
    } else if x < 70_000.0 {
        (x - 30_000.0) * (15.0 / 100.0) + 2_500.0
    } else if x < 140_000.0 {
        (x - 70_000.0) * (20.0 / 100.0) + 8_500.0
    } else if x < 250_000.0 {
        (x - 140_000.0) * (25.0 / 100.0) + 22_500.0
    } else if x < 500_000.0 {
        (x - 250_000.0) * (30.0 / 100.0) + 50_000.0
    } else {
        (x - 500_000.0) * (32.0 / 100.0) + 125_000.0
    };
    fixed0(tax)
}

/// `checkBirthDate`: the official message for an invalid Item 10, if any.
fn birth_date_error(text: &str, current_year: i32) -> Option<&'static str> {
    let parts: Vec<&str> = text.split('/').collect();
    if parts.len() != 3 {
        return Some("Invalid birthdate.Format should be MM/DD/YYYY");
    }
    let invalid = Some("Please enter a valid date in Item 10.");
    let (Ok(month), Ok(day), Ok(year)) = (
        parts[0].parse::<u32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<i32>(),
    ) else {
        return invalid;
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || year > current_year {
        return invalid;
    }
    // The official checks reject impossible days of month the same way.
    NaiveDate::from_ymd_opt(year, month, day).map_or(invalid, |_| None)
}

impl Form1701AColumn {
    /// Round entries the way `round(this,2)` leaves them.
    fn round_inputs(&mut self) {
        for value in [
            &mut self.sales,
            &mut self.sales_returns,
            &mut self.other_income_41,
            &mut self.other_income_42,
            &mut self.gpp_share,
            &mut self.eight_sales,
            &mut self.eight_sales_returns,
            &mut self.eight_other_income_50,
            &mut self.eight_other_income_51,
            &mut self.eight_reduction,
            &mut self.prior_year_excess,
            &mut self.quarterly_payments,
            &mut self.cwt_q1_q3,
            &mut self.cwt_q4,
            &mut self.previously_filed,
            &mut self.foreign_tax_credits,
            &mut self.other_credits,
            &mut self.second_installment,
            &mut self.surcharge,
            &mut self.interest,
            &mut self.compromise,
        ] {
            *value = cents(*value);
        }
    }

    fn clear_part_iva(&mut self) {
        self.sales = 0.0;
        self.sales_returns = 0.0;
        self.other_income_41 = 0.0;
        self.other_income_42 = 0.0;
        self.gpp_share = 0.0;
    }

    fn clear_part_ivb(&mut self) {
        self.eight_sales = 0.0;
        self.eight_sales_returns = 0.0;
        self.eight_other_income_50 = 0.0;
        self.eight_other_income_51 = 0.0;
        self.eight_reduction = 0.0;
    }

    fn compute(&mut self, year: u16, atc: Form1701AAtc) {
        // Part IV.A.
        self.net_sales = fixed0(self.sales - self.sales_returns);
        self.osd = fixed0(self.net_sales * 40.0 / 100.0);
        self.net_income = fixed0(self.net_sales - self.osd);
        self.total_other_income =
            fixed0(self.other_income_41 + self.other_income_42 + self.gpp_share);
        self.taxable_income = fixed0(self.net_income + self.total_other_income);
        self.graduated_tax_due = form_1701a_graduated_tax(year, self.taxable_income);
        // Part IV.B.
        self.eight_net_sales = fixed0(self.eight_sales - self.eight_sales_returns);
        self.eight_total_other_income =
            fixed0(self.eight_other_income_50 + self.eight_other_income_51);
        self.eight_total_income = fixed0(self.eight_net_sales + self.eight_total_other_income);
        self.eight_taxable_income = fixed0(self.eight_total_income - self.eight_reduction);
        self.eight_tax_due = if self.eight_taxable_income < 0.0 {
            0.0
        } else {
            fixed0(self.eight_taxable_income * 8.0 / 100.0)
        };
        // Part IV.C.
        self.total_credits = fixed0(
            self.prior_year_excess
                + self.quarterly_payments
                + self.cwt_q1_q3
                + self.cwt_q4
                + self.previously_filed
                + self.foreign_tax_credits
                + self.other_credits,
        );
        self.tax_due = if atc.is_graduated() {
            self.graduated_tax_due
        } else {
            self.eight_tax_due
        };
        self.net_payable = fixed0(self.tax_due - self.total_credits);
        // Part II.
        self.amount_payable = fixed0(self.net_payable - self.second_installment);
        self.total_penalties = fixed0(self.surcharge + self.interest + self.compromise);
        self.total_amount_payable = if self.amount_payable < 0.0 {
            self.amount_payable
        } else {
            fixed0(self.amount_payable + self.total_penalties)
        };
    }
}

impl Form1701ADraft {
    pub const FORM_CODE: &'static str = "1701A";

    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16) -> Self {
        let mut draft = Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            month: 12,
            is_amended: false,
            is_short_period: false,
            rdo_code: profile.rdo_code.clone(),
            filer_type: Form1701AFilerType::Unanswered,
            atc: Form1701AAtc::Unanswered,
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            birth_date: profile
                .birth_date
                .map(|d| d.format("%m/%d/%Y").to_string())
                .unwrap_or_default(),
            email: profile.email.clone(),
            citizenship: String::new(),
            foreign_tax_credits: false,
            foreign_tax_number: String::new(),
            contact_number: profile.phone.clone(),
            civil_status: Form1701ACivilStatus::Unanswered,
            spouse_has_income: None,
            filing_status: Form1701AFilingStatus::Unanswered,
            line_of_business: profile.line_of_business.clone(),
            taxpayer: Form1701AColumn::default(),
            spouse_column: Form1701AColumn::default(),
            spouse: Form1701ASpouse::default(),
            other_income_41_description: String::new(),
            other_income_42_description: String::new(),
            eight_other_income_50_description: String::new(),
            eight_other_income_51_description: String::new(),
            other_credits_description: String::new(),
            aggregate_amount_payable: 0.0,
            overpayment: Form1701AOverpayment::None,
            number_of_attachments: 0,
            lifecycle: SubmissionLifecycle::default(),
        };
        draft.recompute();
        draft
    }

    /// Part V and column B are open only on a joint return.
    pub fn is_joint(&self) -> bool {
        self.civil_status == Form1701ACivilStatus::Married
            && self.spouse_has_income == Some(true)
            && self.filing_status == Form1701AFilingStatus::Joint
    }

    fn spouse_atc(&self) -> Form1701AAtc {
        if self.is_joint() {
            self.spouse.atc
        } else {
            Form1701AAtc::Unanswered
        }
    }

    /// Items 41/42 apply while either column uses graduated rates.
    pub fn part_iva_descriptions_open(&self) -> bool {
        self.atc.is_graduated() || self.spouse_atc().is_graduated()
    }

    /// Items 50/51 apply while either column uses the 8% rate.
    pub fn part_ivb_descriptions_open(&self) -> bool {
        self.atc.is_eight_percent() || self.spouse_atc().is_eight_percent()
    }

    /// The overpayment boxes are open only while Item 30 is negative.
    pub fn overpayment_open(&self) -> bool {
        self.aggregate_amount_payable < 0.0
    }

    /// The radio rules, then the official compute chain.
    pub fn recompute(&mut self) {
        if !self.is_short_period {
            self.month = 12;
        }
        if !self.atc.allowed_for(self.filer_type) {
            self.atc = Form1701AAtc::Unanswered;
        }
        if !self.foreign_tax_credits {
            self.foreign_tax_number.clear();
            self.taxpayer.foreign_tax_credits = 0.0;
        }
        if self.civil_status != Form1701ACivilStatus::Married {
            self.spouse_has_income = None;
        }
        if self.spouse_has_income != Some(true) {
            self.filing_status = Form1701AFilingStatus::Unanswered;
        }
        if !self.is_joint() {
            // disableSpouse: Part V and the spouse entries reset.
            self.spouse = Form1701ASpouse::default();
            self.spouse_column = Form1701AColumn::default();
        }
        if !self.spouse.atc.allowed_for(self.spouse.filer_type) {
            self.spouse.atc = Form1701AAtc::Unanswered;
        }
        if self.spouse.foreign_tax_credits != Some(true) {
            self.spouse.foreign_tax_number.clear();
            self.spouse_column.foreign_tax_credits = 0.0;
        }
        if !self.is_amended {
            self.taxpayer.previously_filed = 0.0;
            self.spouse_column.previously_filed = 0.0;
        }
        let atc = self.atc;
        let spouse_atc = self.spouse_atc();
        if !atc.is_graduated() {
            self.taxpayer.clear_part_iva();
        }
        if !atc.is_eight_percent() {
            self.taxpayer.clear_part_ivb();
        }
        if !spouse_atc.is_graduated() {
            self.spouse_column.clear_part_iva();
        }
        if !spouse_atc.is_eight_percent() {
            self.spouse_column.clear_part_ivb();
        }
        if !self.part_iva_descriptions_open() {
            self.other_income_41_description.clear();
            self.other_income_42_description.clear();
        }
        if !self.part_ivb_descriptions_open() {
            self.eight_other_income_50_description.clear();
            self.eight_other_income_51_description.clear();
        }
        self.taxpayer.round_inputs();
        self.spouse_column.round_inputs();
        let year = self.taxable_year;
        self.taxpayer.compute(year, atc);
        self.spouse_column.compute(year, spouse_atc);
        self.aggregate_amount_payable =
            fixed0(self.taxpayer.total_amount_payable + self.spouse_column.total_amount_payable);
        if !self.overpayment_open() {
            self.overpayment = Form1701AOverpayment::None;
        }
    }

    /// `disableLinkIfEmpty`: the "add more" link of rows 41/42 or 50/51 is
    /// enabled once both rows are complete for every graduated column.
    fn more_link_enabled(&self, d1: &str, d2: &str, amounts: [(f64, f64); 2]) -> bool {
        let taxpayer_checked = self.atc.is_graduated();
        let spouse_checked = self.spouse_atc().is_graduated();
        !d1.trim().is_empty()
            && !d2.trim().is_empty()
            && amounts
                .iter()
                .all(|(a, b)| (!taxpayer_checked || *a > 0.0) && (!spouse_checked || *b > 0.0))
    }

    fn enabled_links(&self) -> String {
        let tp = &self.taxpayer;
        let sp = &self.spouse_column;
        let mut links = String::new();
        if self.part_iva_descriptions_open()
            && self.more_link_enabled(
                &self.other_income_41_description,
                &self.other_income_42_description,
                [
                    (tp.other_income_41, sp.other_income_41),
                    (tp.other_income_42, sp.other_income_42),
                ],
            )
        {
            links.push_str(",frm1701A:lnkPartIVAMore");
        }
        if self.part_ivb_descriptions_open()
            && self.more_link_enabled(
                &self.eight_other_income_50_description,
                &self.eight_other_income_51_description,
                [
                    (tp.eight_other_income_50, sp.eight_other_income_50),
                    (tp.eight_other_income_51, sp.eight_other_income_51),
                ],
            )
        {
            links.push_str(",frm1701A:lnkPartIVBMore");
        }
        links
    }

    /// The official field values `saveXMLsubmit` reads, keyed by element id.
    ///
    /// Controls the page cleared through a disable handler hold `0` until the
    /// filer types into them; the rest keep the page's `0.00`:
    /// - Item 6 runs `processATC` with no ATC yet, clearing Part IV.A and
    ///   IV.B of column A; Item 7 then clears the other part again.
    /// - Item 2 "No" clears Item 61; Item 13 "No" clears Item 62.
    /// - Item 16 runs `disableSpouse`, clearing Items 23, 25–27, 57–60 and
    ///   63 of column B; a spouse ATC clears the other part of column B.
    /// - Items 44 and 52 are recomputed only when one of their rows is typed.
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let mut put = |key: &str, value: String| {
            fields.insert(format!("frm1701A:{key}"), value);
        };
        let flag = |on: bool| on.to_string();
        let money = official_amount;
        // An entry the filer typed, or the bare 0 a disable handler left.
        let entry = |value: f64, cleared: bool| {
            if value == 0.0 && cleared {
                "0".to_string()
            } else {
                official_amount(value)
            }
        };

        put("txtMonth", format!("{:02}", self.month));
        put("txtYear", self.taxable_year.to_string());
        put("optAmendedReturn_1", flag(self.is_amended));
        put("optAmendedReturn_2", flag(!self.is_amended));
        put("optShortPeriod_1", flag(self.is_short_period));
        put("optShortPeriod_2", flag(!self.is_short_period));
        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);
        for prefix in ["txt", "txtPg2"] {
            put(&format!("{prefix}TIN1"), tin1.clone());
            put(&format!("{prefix}TIN2"), tin2.clone());
            put(&format!("{prefix}TIN3"), tin3.clone());
        }
        put("txtBranchCode", branch.clone());
        put("txtPg2BranchCode", branch);
        put("txtRDOCode", self.rdo_code.trim().to_string());
        put(
            "optTaxType_1",
            flag(self.filer_type == Form1701AFilerType::SingleProprietor),
        );
        put(
            "optTaxType_2",
            flag(self.filer_type == Form1701AFilerType::Professional),
        );
        put("optATC_1", flag(self.atc == Form1701AAtc::II012));
        put("optATC_2", flag(self.atc == Form1701AAtc::II014));
        put("optATC_3", flag(self.atc == Form1701AAtc::II015));
        put("optATC_4", flag(self.atc == Form1701AAtc::II017));

        // Profile values arrive in capitals (loadBGData); the page splits the
        // address at 100 characters and shows the last name on page 2.
        let name = self.taxpayer_name.trim().to_uppercase();
        let address = self.registered_address.trim().to_uppercase();
        let split = address
            .char_indices()
            .nth(100)
            .map_or(address.len(), |(at, _)| at);
        put("txtTaxpayerName", name.clone());
        put(
            "txtPg2TaxpayerName",
            name.split(',').next().unwrap_or("").to_string(),
        );
        put("txtAddress", address[..split].to_string());
        put("txtAddress2", address[split..].to_string());
        put("txtZipCode", self.zip_code.trim().to_string());
        put("txtBirthDate", self.birth_date.trim().to_string());
        put("txtTelNum", self.contact_number.trim().to_string());
        put("txtLineBus", self.line_of_business.trim().to_uppercase());

        // On a joint return the spouse name's capital() upper-cases every
        // text control of the page except txtEmail, so everything typed
        // before it (all of Parts I to IV in form order) and the page's own
        // state controls; the spouse items typed after it keep their case.
        let joint_caps = self.is_joint() && !self.spouse.name.trim().is_empty();
        let typed = |value: &str| {
            if joint_caps {
                value.to_uppercase()
            } else {
                value.to_string()
            }
        };
        put("txtCitizenship", typed(&self.citizenship));
        put("optForeignTaxCredits_1", flag(self.foreign_tax_credits));
        put("optForeignTaxCredits_2", flag(!self.foreign_tax_credits));
        put("txtForeignTaxNumber", typed(&self.foreign_tax_number));
        use Form1701ACivilStatus as C;
        put("optCivilStatus_1", flag(self.civil_status == C::Single));
        put("optCivilStatus_2", flag(self.civil_status == C::Married));
        put("optCivilStatus_3", flag(self.civil_status == C::Separated));
        put("optCivilStatus_4", flag(self.civil_status == C::Widow));
        put(
            "optSpouseIncome_1",
            flag(self.spouse_has_income == Some(true)),
        );
        put(
            "optSpouseIncome_2",
            flag(self.spouse_has_income == Some(false)),
        );
        put(
            "optFilingStatus_1",
            flag(self.filing_status == Form1701AFilingStatus::Joint),
        );
        put(
            "optFilingStatus_2",
            flag(self.filing_status == Form1701AFilingStatus::Separate),
        );
        put("optTaxRate_1", flag(self.atc.is_graduated()));
        put("optTaxRate_2", flag(self.atc.is_eight_percent()));

        let spouse_atc = self.spouse_atc();
        let civil_answered = self.civil_status != C::Unanswered;
        for (suffix, column, atc) in [
            ("A", &self.taxpayer, self.atc),
            ("B", &self.spouse_column, spouse_atc),
        ] {
            let a = suffix == "A";
            // Column A: Item 6 cleared both parts; column B: only a spouse
            // ATC clears the part it does not use.
            let iva_cleared = if a { true } else { atc.is_eight_percent() };
            let ivb_cleared = if a { true } else { atc.is_graduated() };
            // Column B entries disableSpouse resets on Item 16.
            let spouse_reset = !a && civil_answered;
            let mut item = |n: &str, value: String| put(&format!("txt{n}{suffix}"), value);
            item("20", money(column.tax_due));
            item("21", money(column.total_credits));
            item("22", money(column.net_payable));
            item("23", entry(column.second_installment, spouse_reset));
            item("24", money(column.amount_payable));
            item("25", entry(column.surcharge, spouse_reset));
            item("26", entry(column.interest, spouse_reset));
            item("27", entry(column.compromise, spouse_reset));
            item("28", money(column.total_penalties));
            item("29", money(column.total_amount_payable));
            item("36", entry(column.sales, iva_cleared));
            item("37", entry(column.sales_returns, iva_cleared));
            item("38", money(column.net_sales));
            item("39", money(column.osd));
            item("40", money(column.net_income));
            item("41", entry(column.other_income_41, iva_cleared));
            item("42", entry(column.other_income_42, iva_cleared));
            item("43", entry(column.gpp_share, iva_cleared));
            let typed_44 = column.other_income_41 != 0.0
                || column.other_income_42 != 0.0
                || column.gpp_share != 0.0;
            item(
                "44",
                if iva_cleared && !(atc.is_graduated() && typed_44) {
                    "0".to_string()
                } else {
                    money(column.total_other_income)
                },
            );
            item("45", money(column.taxable_income));
            item("46", money(column.graduated_tax_due));
            item("47", entry(column.eight_sales, ivb_cleared));
            item("48", entry(column.eight_sales_returns, ivb_cleared));
            item("49", money(column.eight_net_sales));
            item("50", entry(column.eight_other_income_50, ivb_cleared));
            item("51", entry(column.eight_other_income_51, ivb_cleared));
            let typed_52 =
                column.eight_other_income_50 != 0.0 || column.eight_other_income_51 != 0.0;
            item(
                "52",
                if ivb_cleared && !(atc.is_eight_percent() && typed_52) {
                    "0".to_string()
                } else {
                    money(column.eight_total_other_income)
                },
            );
            item("53", money(column.eight_total_income));
            item("54", entry(column.eight_reduction, ivb_cleared));
            item("55", money(column.eight_taxable_income));
            item(
                "56",
                if column.eight_taxable_income < 0.0 {
                    "0".to_string()
                } else {
                    money(column.eight_tax_due)
                },
            );
            item("57", entry(column.prior_year_excess, spouse_reset));
            item("58", entry(column.quarterly_payments, spouse_reset));
            item("59", entry(column.cwt_q1_q3, spouse_reset));
            item("60", entry(column.cwt_q4, spouse_reset));
            item("61", entry(column.previously_filed, !self.is_amended));
            let ftc_no = if a {
                !self.foreign_tax_credits
            } else {
                self.spouse.foreign_tax_credits == Some(false)
            };
            item("62", entry(column.foreign_tax_credits, ftc_no));
            item("63", entry(column.other_credits, spouse_reset));
            item("64", money(column.total_credits));
            item("65", money(column.net_payable));
        }
        put("txt30", money(self.aggregate_amount_payable));
        put(
            "optRefund_1",
            flag(self.overpayment == Form1701AOverpayment::Refund),
        );
        put(
            "optRefund_2",
            flag(self.overpayment == Form1701AOverpayment::TaxCreditCertificate),
        );
        put(
            "optRefund_3",
            flag(self.overpayment == Form1701AOverpayment::CarryOver),
        );
        put(
            "txtNumberAttachments",
            self.number_of_attachments.to_string(),
        );
        put("txt41Desc", typed(&self.other_income_41_description));
        put("txt42Desc", typed(&self.other_income_42_description));
        put("txt50Desc", typed(&self.eight_other_income_50_description));
        put("txt51Desc", typed(&self.eight_other_income_51_description));
        put("txt63Desc", typed(&self.other_credits_description));

        let s = &self.spouse;
        let (s1, s2, s3, sb) = if self.is_joint() {
            let (a, b, c, d) = split_tin(&s.tin);
            (
                a,
                b,
                c,
                if digits(&s.tin).len() > 9 {
                    d
                } else {
                    String::new()
                },
            )
        } else {
            Default::default()
        };
        put("txtSpouseTIN1", s1);
        put("txtSpouseTIN2", s2);
        put("txtSpouseTIN3", s3);
        put("txtSpouseBranchCode", sb);
        put(
            "txtSpouseRDOCode",
            if self.is_joint() && !s.rdo_code.trim().is_empty() {
                s.rdo_code.trim().to_string()
            } else {
                "000".to_string()
            },
        );
        put(
            "optSpouseTaxType_1",
            flag(s.filer_type == Form1701AFilerType::SingleProprietor),
        );
        put(
            "optSpouseTaxType_2",
            flag(s.filer_type == Form1701AFilerType::Professional),
        );
        put("optSpouseATC_1", flag(s.atc == Form1701AAtc::II012));
        put("optSpouseATC_2", flag(s.atc == Form1701AAtc::II014));
        put("optSpouseATC_3", flag(s.atc == Form1701AAtc::II015));
        put("optSpouseATC_4", flag(s.atc == Form1701AAtc::II017));
        put("txtSpouseName", s.name.trim().to_uppercase());
        put("txtSpouseTelNum", s.contact_number.trim().to_string());
        put("txtSpouseCitizenship", s.citizenship.clone());
        put("optSpouseFTC_1", flag(s.foreign_tax_credits == Some(true)));
        put("optSpouseFTC_2", flag(s.foreign_tax_credits == Some(false)));
        put("txtSpouseFTN", s.foreign_tax_number.clone());
        put("optSpouseTaxRate_1", flag(s.atc.is_graduated()));
        put("optSpouseTaxRate_2", flag(s.atc.is_eight_percent()));
        put("txtEnabledLinks", self.enabled_links());
        // capital() also upper-cases the page's own state controls.
        put("txtIsTaxFilerDisabled", typed("false"));

        fields.insert("txtEmail".to_string(), self.email.trim().to_string());
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
        let current_year = today.year();

        // validate(), in order.
        if self.taxable_year == 0 {
            err("taxable_year", "Please enter a valid year in Item 1.");
        } else if self.is_short_period && year > current_year {
            err(
                "taxable_year",
                "Invalid date entry on Item #1. Since Short Return Period is Yes, valid year entry is only until current year.",
            );
        } else if !self.is_short_period && year > current_year {
            err(
                "taxable_year",
                "Invalid date entry on Item #1. Year entry cannot be greater than to the current year.",
            );
        }
        if self.taxable_year != 0 && year < 2018 {
            err(
                "taxable_year",
                "Please file using the old version of the form.",
            );
        }
        if !(1..=12).contains(&self.month) {
            err("month", "Please enter a valid year in Item 1.");
        }

        let (tin1, tin2, tin3, _) = split_tin(&self.tin);
        if tin1.len() != 3
            || tin2.len() != 3
            || tin3.len() != 3
            || digits(&self.tin).len() > 14
            || !self.tin.chars().all(|c| c.is_ascii_digit() || c == '-')
        {
            err("tin", "Please enter a valid TIN number on Item 4.");
        } else if !crate::validation::relaxed_dev_mode()
            && crate::validation::official_tin_check_code(&format!("{tin1}{tin2}{tin3}")) != 0
        {
            err("tin", crate::validation::OFFICIAL_INVALID_TIN_MESSAGE);
        }
        if !crate::validation::rdo_code_is_official_option(self.rdo_code.trim()) {
            err("rdo_code", "Please enter a valid RDO Code on Item 5.");
        }
        let name = self.taxpayer_name.trim();
        if name.is_empty() || name.chars().count() > 50 {
            err(
                "taxpayer_name",
                "Please enter a valid Taxpayer Name on Item 8.",
            );
        }
        let address = self.registered_address.trim();
        if address.is_empty() || address.chars().count() > 150 {
            err(
                "registered_address",
                "Please enter Taxpayer's Registered Address on Item 9.",
            );
        }
        if self.birth_date.trim().is_empty() {
            err(
                "birth_date",
                "Please indicate Birth Date of Taxpayer on item 10.",
            );
        } else if let Some(message) = birth_date_error(self.birth_date.trim(), current_year) {
            err("birth_date", message);
        }
        let zip = self.zip_code.trim();
        if zip.is_empty() || zip.len() > 12 || !zip.bytes().all(|b| b.is_ascii_digit()) {
            err("zip_code", "Please enter Zip Code on Item 9A.");
        }

        let s = &self.spouse;
        if self.is_joint() {
            let (s1, s2, s3, _) = split_tin(&s.tin);
            let spouse_digits = digits(&s.tin);
            if s1.len() != 3
                || s2.len() != 3
                || s3.len() != 3
                || spouse_digits.len() > 14
                || !s.tin.chars().all(|c| c.is_ascii_digit() || c == '-')
            {
                err("spouse.tin", "Please enter a valid TIN number on Item 66.");
            } else {
                if !crate::validation::relaxed_dev_mode()
                    && crate::validation::official_tin_check_code(&format!("{s1}{s2}{s3}")) != 0
                {
                    err(
                        "spouse.tin",
                        &format!(
                            "{} on Item 66.",
                            crate::validation::OFFICIAL_INVALID_TIN_MESSAGE
                        ),
                    );
                }
                if !crate::validation::rdo_code_is_official_option(s.rdo_code.trim()) {
                    err(
                        "spouse.rdo_code",
                        "Please enter a valid RDO Code on Item 67.",
                    );
                }
                if s.name.trim().is_empty() {
                    err("spouse.name", "Please enter Spouse Name on Item 70.");
                }
                if s.atc == Form1701AAtc::Unanswered {
                    err("spouse.atc", "Please select an option for Item 69.");
                }
            }
        }
        if self.filer_type == Form1701AFilerType::Unanswered {
            err("filer_type", "Please select an option for Item 6.");
        }
        if self.atc == Form1701AAtc::Unanswered {
            err("atc", "Please select an option for Item 7.");
        }
        if !self.atc.is_graduated() && !self.atc.is_eight_percent() {
            err("atc", "Please select an option for Item 19.");
        }
        if self.is_joint() && !s.name.trim().is_empty() {
            if s.filer_type == Form1701AFilerType::Unanswered {
                err("spouse.filer_type", "Please select an option for Item 68.");
            }
            if s.atc == Form1701AAtc::Unanswered {
                err("spouse.atc", "Please select an option for Item 69.");
            }
        }
        if self.overpayment_open() && self.overpayment == Form1701AOverpayment::None {
            err(
                "overpayment",
                "Please select an Overpayment option in Page 1 after Item 30.",
            );
        }

        // Limits and rules the page applies while typing.
        if self.civil_status == Form1701ACivilStatus::Unanswered {
            err("civil_status", "Please select an option for Item 16.");
        }
        if self.civil_status == Form1701ACivilStatus::Married && self.spouse_has_income.is_none() {
            err("spouse_has_income", "Please select an option for Item 17.");
        }
        if self.spouse_has_income == Some(true)
            && self.filing_status == Form1701AFilingStatus::Unanswered
        {
            err("filing_status", "Please select an option for Item 18.");
        }
        let email = self.email.trim();
        if email.is_empty() || !email.contains('@') || email.contains(char::is_whitespace) {
            err(
                "email",
                "Enter the email address BIR sends the filing confirmation to.",
            );
        }
        for (field, column) in [
            ("taxpayer", &self.taxpayer),
            ("spouse_column", &self.spouse_column),
        ] {
            let suffix = if field == "taxpayer" { "A" } else { "B" };
            if column.eight_total_income > 3_000_000.0 {
                err(&format!("{field}.eight_sales"), ITEM_53_ALERT);
            }
            if column.eight_reduction > 250_000.0 {
                err(
                    &format!("{field}.eight_reduction"),
                    &format!("Item 54{suffix} cannot be more than P250,000."),
                );
            }
            if column.second_installment > column.tax_due / 2.0 {
                // Item 23 is "50% or less of Item 20".
                err(
                    &format!("{field}.second_installment"),
                    &format!("Item 23{suffix} cannot exceed 50% of Item 20{suffix}."),
                );
            }
            for (name, value) in [
                ("sales", column.sales),
                ("sales_returns", column.sales_returns),
                ("other_income_41", column.other_income_41),
                ("other_income_42", column.other_income_42),
                ("gpp_share", column.gpp_share),
                ("eight_sales", column.eight_sales),
                ("eight_sales_returns", column.eight_sales_returns),
                ("eight_other_income_50", column.eight_other_income_50),
                ("eight_other_income_51", column.eight_other_income_51),
                ("eight_reduction", column.eight_reduction),
                ("prior_year_excess", column.prior_year_excess),
                ("quarterly_payments", column.quarterly_payments),
                ("cwt_q1_q3", column.cwt_q1_q3),
                ("cwt_q4", column.cwt_q4),
                ("previously_filed", column.previously_filed),
                ("foreign_tax_credits", column.foreign_tax_credits),
                ("other_credits", column.other_credits),
                ("second_installment", column.second_installment),
                ("surcharge", column.surcharge),
                ("interest", column.interest),
                ("compromise", column.compromise),
            ] {
                if value < 0.0 {
                    err(&format!("{field}.{name}"), "Enter a non-negative amount.");
                }
            }
        }
        for (field, text, limit) in [
            ("citizenship", &self.citizenship, 20),
            ("foreign_tax_number", &self.foreign_tax_number, 20),
            (
                "other_income_41_description",
                &self.other_income_41_description,
                50,
            ),
            (
                "other_income_42_description",
                &self.other_income_42_description,
                50,
            ),
            (
                "eight_other_income_50_description",
                &self.eight_other_income_50_description,
                50,
            ),
            (
                "eight_other_income_51_description",
                &self.eight_other_income_51_description,
                50,
            ),
            (
                "other_credits_description",
                &self.other_credits_description,
                25,
            ),
            ("line_of_business", &self.line_of_business, 30),
        ] {
            if text.chars().count() > limit {
                err(field, &format!("At most {limit} characters."));
            }
        }
        if self.number_of_attachments > 99 {
            err("number_of_attachments", "Item 31 holds at most two digits.");
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

impl FormValidator for Form1701ADraft {
    /// `validate()` in order, with its alert texts, plus the input limits
    /// the official page enforces while typing.
    fn validate(&self) -> Vec<(String, String)> {
        self.validate_on(chrono::Local::now().date_naive())
    }
}

impl QueueableForm for Form1701ADraft {
    const FORM_CODE: &'static str = "1701A";
    /// Official `formType` and SFTP folder (`ftpTargetFolder.PROD['1701A']`).
    const FORM_TYPE: &'static str = "1701A";
    const LAYOUT_ID: &'static str = FORM_1701A_FORM_ID;

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
    /// `txtMonth + txtYear`, as in `createXMLFileName`.
    fn period_code(&self) -> String {
        format!("{:02}{}", self.month, self.taxable_year)
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
        self.to_bir_field_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()
    }

    pub(crate) fn sample() -> Form1701ADraft {
        let mut d = Form1701ADraft {
            id: None,
            tin: "12345678800000".to_string(),
            taxable_year: 2025,
            month: 12,
            is_amended: false,
            is_short_period: false,
            rdo_code: "039".to_string(),
            filer_type: Form1701AFilerType::SingleProprietor,
            atc: Form1701AAtc::II012,
            taxpayer_name: "Dummy, Sample Taxpayer".to_string(),
            registered_address: "123 Sample Street, Barangay Example, Quezon City".to_string(),
            zip_code: "1100".to_string(),
            birth_date: "01/15/1980".to_string(),
            email: "sample.taxpayer@example.com".to_string(),
            citizenship: "Filipino".to_string(),
            foreign_tax_credits: false,
            foreign_tax_number: String::new(),
            contact_number: "09170000000".to_string(),
            civil_status: Form1701ACivilStatus::Single,
            spouse_has_income: None,
            filing_status: Form1701AFilingStatus::Unanswered,
            line_of_business: "Sample Consulting Services".to_string(),
            taxpayer: Form1701AColumn {
                sales: 1_234_567.89,
                sales_returns: 10_000.5,
                other_income_41: 5_000.55,
                gpp_share: 20_000.0,
                prior_year_excess: 1_000.0,
                quarterly_payments: 30_000.0,
                cwt_q1_q3: 5_000.25,
                cwt_q4: 2_500.0,
                other_credits: 100.0,
                surcharge: 1_000.0,
                ..Form1701AColumn::default()
            },
            spouse_column: Form1701AColumn::default(),
            spouse: Form1701ASpouse::default(),
            other_income_41_description: "Interest income".to_string(),
            other_income_42_description: String::new(),
            eight_other_income_50_description: String::new(),
            eight_other_income_51_description: String::new(),
            other_credits_description: "Sample credit".to_string(),
            aggregate_amount_payable: 0.0,
            overpayment: Form1701AOverpayment::None,
            number_of_attachments: 0,
            lifecycle: SubmissionLifecycle::default(),
        };
        d.recompute();
        d
    }

    fn messages(d: &Form1701ADraft) -> Vec<String> {
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
        assert_eq!(tp.net_sales, 1_224_567.0);
        assert_eq!(tp.osd, 489_827.0);
        assert_eq!(tp.net_income, 734_740.0);
        assert_eq!(tp.total_other_income, 25_001.0);
        assert_eq!(tp.taxable_income, 759_741.0);
        assert_eq!(tp.graduated_tax_due, 94_448.0);
        assert_eq!(tp.total_credits, 38_600.0);
        assert_eq!(tp.net_payable, 55_848.0);
        assert_eq!(tp.total_amount_payable, 56_848.0);
        assert_eq!(d.aggregate_amount_payable, 56_848.0);
        assert!(messages(&d).is_empty(), "{:?}", messages(&d));
    }

    #[test]
    fn eight_percent_column() {
        let mut d = sample();
        d.atc = Form1701AAtc::II015;
        d.taxpayer.eight_sales = 1_500_000.0;
        d.taxpayer.eight_other_income_50 = 10_000.4;
        d.eight_other_income_50_description = "Rental".into();
        d.taxpayer.eight_reduction = 250_000.0;
        d.recompute();
        let tp = &d.taxpayer;
        assert_eq!(tp.sales, 0.0);
        assert_eq!(tp.eight_total_income, 1_510_000.0);
        assert_eq!(tp.eight_taxable_income, 1_260_000.0);
        assert_eq!(tp.eight_tax_due, 100_800.0);
        assert_eq!(tp.tax_due, 100_800.0);
        assert!(d.other_income_41_description.is_empty());
        let f = d.to_bir_field_map();
        assert_eq!(f["frm1701A:txt36A"], "0");
        assert_eq!(f["frm1701A:txt44A"], "0");
        assert_eq!(f["frm1701A:txt52A"], "10,000.00");
        assert_eq!(f["frm1701A:optTaxRate_2"], "true");
    }

    #[test]
    fn field_map_uses_official_formats() {
        let d = sample();
        let f = d.to_bir_field_map();
        assert_eq!(f["frm1701A:txtTaxpayerName"], "DUMMY, SAMPLE TAXPAYER");
        assert_eq!(f["frm1701A:txtPg2TaxpayerName"], "DUMMY");
        assert_eq!(f["frm1701A:txt36A"], "1,234,567.89");
        assert_eq!(f["frm1701A:txt42A"], "0");
        assert_eq!(f["frm1701A:txt47A"], "0");
        assert_eq!(f["frm1701A:txt49A"], "0.00");
        assert_eq!(f["frm1701A:txt52A"], "0");
        assert_eq!(f["frm1701A:txt61A"], "0");
        assert_eq!(f["frm1701A:txt62A"], "0");
        assert_eq!(f["frm1701A:txt23B"], "0");
        assert_eq!(f["frm1701A:txt36B"], "0.00");
        assert_eq!(f["frm1701A:txtSpouseRDOCode"], "000");
        assert_eq!(f["frm1701A:txtCitizenship"], "Filipino");
        assert_eq!(f["frm1701A:txtEnabledLinks"], "");
        assert_eq!(
            d.submission_filename(),
            "12345678800000-1701A-122025#sample.taxpayer@example.com#.xml"
        );
        assert!(d.to_bir_xml_payload().is_ok());
    }

    #[test]
    fn period_codes_round_trip() {
        let d = sample();
        assert_eq!(d.period_code(), "122025");
        assert_eq!(
            Form1701ADraft::parse_period_code("122025"),
            Some((2025, FilingPeriod::Annual))
        );
        assert_eq!(Form1701ADraft::parse_period_code("002025"), None);
    }

    #[test]
    fn official_negative_cases_are_rejected_with_their_alerts() {
        let check = |mutate: &dyn Fn(&mut Form1701ADraft), expected: &str| {
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
            &|d| d.taxable_year = 2027,
            "Invalid date entry on Item #1. Year entry cannot be greater than to the current year.",
        );
        check(
            &|d| {
                d.is_short_period = true;
                d.month = 6;
                d.taxable_year = 2027;
                d.recompute();
            },
            "Invalid date entry on Item #1. Since Short Return Period is Yes, valid year entry is only until current year.",
        );
        check(
            &|d| d.taxable_year = 2017,
            "Please file using the old version of the form.",
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
            &|d| d.taxpayer_name.clear(),
            "Please enter a valid Taxpayer Name on Item 8.",
        );
        check(
            &|d| d.registered_address.clear(),
            "Please enter Taxpayer's Registered Address on Item 9.",
        );
        check(
            &|d| d.birth_date.clear(),
            "Please indicate Birth Date of Taxpayer on item 10.",
        );
        check(
            &|d| d.birth_date = "1980-01-15".into(),
            "Invalid birthdate.Format should be MM/DD/YYYY",
        );
        check(
            &|d| d.birth_date = "02/30/1980".into(),
            "Please enter a valid date in Item 10.",
        );
        check(&|d| d.zip_code.clear(), "Please enter Zip Code on Item 9A.");
        let joint = |d: &mut Form1701ADraft| {
            d.civil_status = Form1701ACivilStatus::Married;
            d.spouse_has_income = Some(true);
            d.filing_status = Form1701AFilingStatus::Joint;
            d.spouse = Form1701ASpouse {
                tin: "12345678800000".into(),
                rdo_code: "039".into(),
                filer_type: Form1701AFilerType::Professional,
                atc: Form1701AAtc::II014,
                name: "Dummy, Sample Spouse".into(),
                ..Form1701ASpouse::default()
            };
        };
        check(
            &|d| {
                joint(d);
                d.spouse.tin = "12".into();
                d.recompute();
            },
            "Please enter a valid TIN number on Item 66.",
        );
        check(
            &|d| {
                joint(d);
                d.spouse.tin = "12345678900000".into();
                d.recompute();
            },
            "You have entered an incorrect TIN on Item 66.",
        );
        check(
            &|d| {
                joint(d);
                d.spouse.rdo_code.clear();
                d.recompute();
            },
            "Please enter a valid RDO Code on Item 67.",
        );
        check(
            &|d| {
                joint(d);
                d.spouse.name.clear();
                d.recompute();
            },
            "Please enter Spouse Name on Item 70.",
        );
        check(
            &|d| {
                joint(d);
                d.spouse.atc = Form1701AAtc::Unanswered;
                d.recompute();
            },
            "Please select an option for Item 69.",
        );
        check(
            &|d| {
                joint(d);
                d.spouse.filer_type = Form1701AFilerType::Unanswered;
                d.recompute();
            },
            "Please select an option for Item 68.",
        );
        check(
            &|d| {
                d.filer_type = Form1701AFilerType::Unanswered;
                d.recompute();
            },
            "Please select an option for Item 6.",
        );
        check(
            &|d| {
                d.atc = Form1701AAtc::Unanswered;
                d.recompute();
            },
            "Please select an option for Item 7.",
        );
        check(
            &|d| {
                d.atc = Form1701AAtc::Unanswered;
                d.recompute();
            },
            "Please select an option for Item 19.",
        );
        check(
            &|d| {
                d.taxpayer.cwt_q4 = 200_000.0;
                d.recompute();
            },
            "Please select an Overpayment option in Page 1 after Item 30.",
        );
        check(
            &|d| {
                d.atc = Form1701AAtc::II015;
                d.taxpayer.eight_sales = 3_000_001.0;
                d.recompute();
            },
            ITEM_53_ALERT,
        );
        check(
            &|d| {
                d.atc = Form1701AAtc::II015;
                d.taxpayer.eight_sales = 1_000_000.0;
                d.taxpayer.eight_reduction = 250_001.0;
                d.recompute();
            },
            "Item 54A cannot be more than P250,000.",
        );
    }

    #[test]
    fn joint_returns_follow_the_spouse_rules() {
        let mut d = sample();
        d.civil_status = Form1701ACivilStatus::Married;
        d.spouse_has_income = Some(true);
        d.filing_status = Form1701AFilingStatus::Joint;
        d.spouse = Form1701ASpouse {
            tin: "12345678800000".into(),
            rdo_code: "039".into(),
            filer_type: Form1701AFilerType::Professional,
            atc: Form1701AAtc::II017,
            name: "Dummy, Sample Spouse".into(),
            foreign_tax_credits: Some(false),
            ..Form1701ASpouse::default()
        };
        d.spouse_column.eight_sales = 900_000.0;
        d.spouse_column.eight_reduction = 250_000.0;
        d.recompute();
        assert_eq!(d.spouse_column.eight_tax_due, 52_000.0);
        assert_eq!(d.aggregate_amount_payable, 56_848.0 + 52_000.0);
        let f = d.to_bir_field_map();
        assert_eq!(f["frm1701A:txtCitizenship"], "FILIPINO");
        assert_eq!(f["frm1701A:txt36B"], "0");
        assert_eq!(f["frm1701A:txt44B"], "0");
        assert_eq!(f["frm1701A:txt47B"], "900,000.00");
        assert_eq!(f["frm1701A:txt52B"], "0.00");
        assert_eq!(f["frm1701A:txt62B"], "0");
        assert!(messages(&d).is_empty(), "{:?}", messages(&d));
        d.filing_status = Form1701AFilingStatus::Separate;
        d.recompute();
        assert_eq!(d.spouse, Form1701ASpouse::default());
        assert_eq!(d.spouse_column, Form1701AColumn::default());
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
