//! BIR Form 1707-A (April 2021) — Annual Capital Gains Tax Return for Onerous
//! Transfer of Shares of Stock Not Traded Through the Local Stock Exchange.
//!
//! Ported from the official `BIR-Form1707Av2021.hta` (eBIRForms 7.9.6.2.1):
//! the amount helpers (`roundElement` / `roundAmount`, which truncate to two
//! decimals and show losses in parentheses, `valForCompute`,
//! `preciseCompute`), the compute chain (`computeSched1*`, `computeSched2*`,
//! `computePart2Item14GainLoss` … `computePart2Item19TotalPayable`),
//! `validateAll()` / `validateSchedules()` with their exact alert texts, and
//! `saveXMLsubmit`, which strips commas and turns `(5.00)` into `-5.00`.
//! Background information comes from the taxpayer profile the way
//! `loadBGData()` fills it (including its page-2 TIN3 = TIN2 slip).
//!
//! Schedules 1 and 2 hold four rows each on the page; the "More" pop-ups are
//! not covered.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use super::official_inputs::{digits_only, split_tin, tin_is_well_formed};
use super::queueable::{QueueableForm, SubmissionLifecycle};
use super::{FilingPeriod, FormValidator};
use crate::profile::TaxpayerProfile;

/// Rule-package id of the official layout.
pub const FORM_1707A_FORM_ID: &str = "1707a-v2021";
/// Rows 1–4 of Schedules 1 and 2.
pub const FORM_1707A_SCHEDULE_ROWS: usize = 4;
/// Item 16 applicable tax rate (`txtI16ApplicableTaxRate`).
pub const FORM_1707A_TAX_RATE: f64 = 15.0;

/// Item 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form1707AAtc {
    /// II030.
    Individual,
    /// IC110.
    Corporation,
}

/// A Schedule 1 (gain) or Schedule 2 (loss) row.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Form1707ARow {
    /// Column A, `MM/DD/YYYY`.
    pub date: String,
    /// Column B (17 characters).
    pub corporation: String,
    /// Column C.
    pub selling_price: f64,
    /// Column D.
    pub cost: f64,
    /// Column E, computed: gain (Schedule 1) or loss (Schedule 2).
    #[serde(default)]
    pub gain_or_loss: f64,
    /// Column F, Schedule 1 only.
    #[serde(default)]
    pub tax_paid: f64,
}

impl Form1707ARow {
    fn date_blank(&self) -> bool {
        self.date.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Form1707ADraft {
    #[serde(default)]
    pub id: Option<i64>,
    /// Dashboard year and open-ended key that identify this return.
    pub filing_year: u16,
    pub open_ended_key: u32,

    // Items 1–4
    pub fiscal: bool,
    pub year_end_month: u8,
    pub year_end_day: u8,
    pub year_end_year: u16,
    pub is_amended: bool,
    #[serde(default)]
    pub number_of_attached_sheets: u16,
    pub atc: Option<Form1707AAtc>,

    // Background information (taxpayer profile)
    pub tin: String,
    pub rdo_code: String,
    pub taxpayer_name: String,
    #[serde(default)]
    pub line_of_business: String,
    pub registered_address: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,
    /// Item 12; `Some` when "Yes", holding Item 12A.
    #[serde(default)]
    pub tax_relief: Option<String>,

    // Schedules
    #[serde(default)]
    pub gains: Vec<Form1707ARow>,
    #[serde(default)]
    pub losses: Vec<Form1707ARow>,
    #[serde(default)]
    pub s1_total_selling_price: f64,
    #[serde(default)]
    pub s1_total_cost: f64,
    #[serde(default)]
    pub s1_total_gains: f64,
    #[serde(default)]
    pub s1_total_tax_paid: f64,
    #[serde(default)]
    pub s2_total_selling_price: f64,
    #[serde(default)]
    pub s2_total_cost: f64,
    #[serde(default)]
    pub s2_total_loss: f64,

    // Part II
    #[serde(default)]
    pub net_capital_gain: f64,
    /// Item 15; the page writes `0.00` for a negative tax due.
    #[serde(default)]
    pub tax_due: f64,
    /// 16B, amended returns only.
    #[serde(default)]
    pub tax_paid_previous: f64,
    /// 16C.
    #[serde(default)]
    pub total_tax_paid: f64,
    /// 17.
    #[serde(default)]
    pub tax_payable: f64,
    #[serde(default)]
    pub surcharge: f64,
    #[serde(default)]
    pub interest: f64,
    #[serde(default)]
    pub compromise: f64,
    #[serde(default)]
    pub total_penalties: f64,
    /// 19.
    #[serde(default)]
    pub total_amount_payable: f64,

    #[serde(flatten)]
    pub lifecycle: SubmissionLifecycle,
}

// ── Official amount helpers ──

/// `Math.round`.
fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// `preciseCompute`: `Math.round(x * 100) / 100`.
fn precise(value: f64) -> f64 {
    js_round(value * 100.0) / 100.0
}

/// `String(number)` for the amounts these handlers produce.
fn js_number_text(value: f64) -> String {
    if value == 0.0 {
        "0".to_string()
    } else {
        format!("{value}")
    }
}

/// `addCommas(Number(text))`.
fn add_commas_of_number(text: &str) -> String {
    let number: f64 = if text.trim().is_empty() {
        0.0
    } else {
        text.parse().unwrap_or(f64::NAN)
    };
    let plain = js_number_text(number);
    let (sign, digits) = match plain.strip_prefix('-') {
        Some(rest) => ("-", rest.to_string()),
        None => ("", plain.clone()),
    };
    if digits.contains('.') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return plain;
    }
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{sign}{grouped}")
}

/// `roundAmount(text)`: truncates to two decimals, groups thousands and
/// writes negatives as `(1,234.50)`.
pub fn round_amount_text(amount: &str) -> String {
    let int_len = amount.split('.').next().unwrap_or("").len();
    let mut out = if int_len > 12 {
        "0.00".to_string()
    } else if let Some((whole, frac)) = amount.split_once('.') {
        let frac = frac.split('.').next().unwrap_or("");
        match frac.len() {
            0 => format!("{}.00", add_commas_of_number(amount)),
            1 => format!("{}.{frac}0", add_commas_of_number(whole)),
            _ => format!("{}.{}", add_commas_of_number(whole), &frac[..2]),
        }
    } else {
        format!("{}.00", add_commas_of_number(amount))
    };
    if let Some(index) = out.find('-') {
        let after = out[index + 1..].split('-').next().unwrap_or("").to_string();
        out = format!("({after})");
    }
    out
}

/// `valForCompute`: reads a shown amount back, parentheses as a minus.
pub fn value_for_compute(text: &str) -> f64 {
    let negative = text.find(')').is_some_and(|i| i > 0);
    let digits: String = text
        .chars()
        .filter(|c| !matches!(c, '(' | ')' | ','))
        .collect();
    let value: f64 = digits.parse().unwrap_or(f64::NAN);
    if negative { -value } else { value }
}

/// The value a computed field shows, as the next handler reads it.
fn shown(value: f64) -> f64 {
    value_for_compute(&round_amount_text(&js_number_text(value)))
}

/// The submit text of a shown amount: commas stripped, `(x)` as `-x`.
fn submit_text(value: f64) -> String {
    let shown = round_amount_text(&js_number_text(value));
    let plain: String = shown.chars().filter(|c| *c != ',').collect();
    match plain.strip_prefix('(').and_then(|p| p.strip_suffix(')')) {
        Some(inner) => format!("-{inner}"),
        None => plain,
    }
}

/// `roundElement` on a typed amount (blank → `0.00`).
fn typed(value: f64) -> f64 {
    if value.is_finite() { shown(value) } else { 0.0 }
}

fn parse_mmddyyyy(text: &str) -> Option<NaiveDate> {
    let parts: Vec<&str> = text.split('/').collect();
    if parts.len() != 3
        || parts[0].len() != 2
        || parts[1].len() != 2
        || parts[2].len() != 4
        || !parts.iter().all(|p| digits_only(p))
    {
        return None;
    }
    let year: i32 = parts[2].parse().ok()?;
    if year < 1800 {
        return None;
    }
    NaiveDate::from_ymd_opt(year, parts[0].parse().ok()?, parts[1].parse().ok()?)
}

impl Form1707ADraft {
    pub const FORM_CODE: &'static str = "1707A";

    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, open_ended_key: u32) -> Self {
        let mut draft = Self {
            id: None,
            filing_year: year,
            open_ended_key,
            fiscal: false,
            year_end_month: 12,
            year_end_day: 31,
            year_end_year: year,
            is_amended: false,
            number_of_attached_sheets: 0,
            atc: None,
            tin: profile.tin.full(),
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            line_of_business: profile.line_of_business.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            tax_relief: None,
            gains: Vec::new(),
            losses: Vec::new(),
            s1_total_selling_price: 0.0,
            s1_total_cost: 0.0,
            s1_total_gains: 0.0,
            s1_total_tax_paid: 0.0,
            s2_total_selling_price: 0.0,
            s2_total_cost: 0.0,
            s2_total_loss: 0.0,
            net_capital_gain: 0.0,
            tax_due: 0.0,
            tax_paid_previous: 0.0,
            total_tax_paid: 0.0,
            tax_payable: 0.0,
            surcharge: 0.0,
            interest: 0.0,
            compromise: 0.0,
            total_penalties: 0.0,
            total_amount_payable: 0.0,
            lifecycle: SubmissionLifecycle::default(),
        };
        draft.recompute();
        draft
    }

    /// Item 1 (`checkCalendarFiscal`): a calendar year ends on 12/31.
    pub fn set_fiscal(&mut self, fiscal: bool) {
        self.fiscal = fiscal;
        if fiscal {
            self.year_end_month = 4;
            self.year_end_day = 1;
        } else {
            self.year_end_month = 12;
            self.year_end_day = 31;
        }
        self.recompute();
    }

    /// Item 4 (`atcClick`): individuals file on a calendar year.
    pub fn set_atc(&mut self, atc: Form1707AAtc) {
        self.atc = Some(atc);
        if atc == Form1707AAtc::Individual {
            self.set_fiscal(false);
        }
        self.recompute();
    }

    /// Item 2 (`checkAmendedForItem16A`): "No" resets 16B to `0.00`.
    pub fn set_amended(&mut self, amended: bool) {
        self.is_amended = amended;
        if !amended {
            self.tax_paid_previous = 0.0;
        }
        self.recompute();
    }

    /// The official compute chain, run as if every changed field was blurred.
    pub fn recompute(&mut self) {
        if !self.is_amended {
            self.tax_paid_previous = 0.0;
        }
        if !self.fiscal {
            self.year_end_month = 12;
            self.year_end_day = 31;
        }
        for row in &mut self.gains {
            row.selling_price = typed(row.selling_price);
            row.cost = typed(row.cost);
            row.tax_paid = typed(row.tax_paid);
            row.gain_or_loss = shown(precise(row.selling_price - row.cost));
        }
        for row in &mut self.losses {
            row.selling_price = typed(row.selling_price);
            row.cost = typed(row.cost);
            row.tax_paid = 0.0;
            row.gain_or_loss = shown(precise(row.cost - row.selling_price));
        }
        let total = |rows: &[Form1707ARow], pick: fn(&Form1707ARow) -> f64| {
            let mut sum = 0.0;
            for row in rows.iter().take(FORM_1707A_SCHEDULE_ROWS) {
                sum = precise(sum + pick(row));
            }
            shown(sum)
        };
        self.s1_total_selling_price = total(&self.gains, |r| r.selling_price);
        self.s1_total_cost = total(&self.gains, |r| r.cost);
        self.s1_total_gains = total(&self.gains, |r| r.gain_or_loss);
        self.s1_total_tax_paid = total(&self.gains, |r| r.tax_paid);
        self.s2_total_selling_price = total(&self.losses, |r| r.selling_price);
        self.s2_total_cost = total(&self.losses, |r| r.cost);
        self.s2_total_loss = total(&self.losses, |r| r.gain_or_loss);

        self.net_capital_gain = shown(precise(self.s1_total_gains - self.s2_total_loss));
        let tax_due = precise(self.net_capital_gain * (FORM_1707A_TAX_RATE / 100.0));
        self.tax_due = if tax_due < 0.0 { 0.0 } else { shown(tax_due) };
        self.tax_paid_previous = typed(self.tax_paid_previous);
        self.total_tax_paid = shown(precise(self.s1_total_tax_paid + self.tax_paid_previous));
        self.tax_payable = shown(precise(self.tax_due - self.total_tax_paid));
        self.surcharge = typed(self.surcharge);
        self.interest = typed(self.interest);
        self.compromise = typed(self.compromise);
        self.total_penalties = shown(precise(self.surcharge + self.interest + self.compromise));
        let total = if self.tax_payable < 0.0 && self.total_penalties > 0.0 {
            self.total_penalties
        } else {
            precise(self.tax_payable + self.total_penalties)
        };
        self.total_amount_payable = shown(total);
    }

    fn year_end(&self) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(
            i32::from(self.year_end_year),
            u32::from(self.year_end_month),
            u32::from(self.year_end_day),
        )
    }

    fn row(rows: &[Form1707ARow], index: usize) -> Form1707ARow {
        rows.get(index).cloned().unwrap_or_default()
    }

    /// The official field values `saveXMLsubmit` writes, keyed by element id.
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let mut put = |key: &str, value: String| {
            fields.insert(format!("frm1707Av2021:{key}"), value);
        };
        let flag = |on: bool| on.to_string();
        let amount = submit_text;

        put("rdoI1Calendar", flag(!self.fiscal));
        put("rdoI1Fiscal", flag(self.fiscal));
        put("txtI1YearEndMonth", format!("{:02}", self.year_end_month));
        put("txtI1YearEndDay", format!("{:02}", self.year_end_day));
        put("txtI1YearEndYear", self.year_end_year.to_string());
        put("rdoI2AmemdedYes", flag(self.is_amended));
        put("rdoI2AmemdedNo", flag(!self.is_amended));
        put("txtI3Sheets", self.number_of_attached_sheets.to_string());
        put(
            "rdoI4ATCII030",
            flag(self.atc == Some(Form1707AAtc::Individual)),
        );
        put("txtI4TCII030", "II030".to_string());
        put(
            "rdoI4ATCIC110",
            flag(self.atc == Some(Form1707AAtc::Corporation)),
        );
        put("txtI4ATCIC110", "IC110".to_string());

        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);
        put("txtI5TIN1", tin1.clone());
        put("txtI5TIN2", tin2.clone());
        put("txtI5TIN3", tin3);
        put("txtI5BranchMask", branch.clone());
        put("hdnI5TIN4", branch.clone());
        let rdo = if self.rdo_code.trim().is_empty() {
            "000".to_string()
        } else {
            self.rdo_code.trim().to_string()
        };
        put("hdnRDO", rdo.clone());
        put("rdoI6RDO", rdo);
        put("txtI8RegisteredName", self.taxpayer_name.to_uppercase());
        put("txtLineOfBusiness", self.line_of_business.clone());
        put(
            "txtI9RegisteredAddress",
            self.registered_address.to_uppercase(),
        );
        put("txtI10ZipCode", self.zip_code.clone());
        put("txtI7TelNo", self.contact_number.clone());
        put("txtI11Email", self.email.clone());
        put("rdoI11TaxTreatyYes", flag(self.tax_relief.is_some()));
        put("rdoI11TaxTreatyNo", flag(self.tax_relief.is_none()));
        put(
            "txtI11ASpecify",
            self.tax_relief
                .as_deref()
                .map(|s| s.trim().to_uppercase())
                .unwrap_or_default(),
        );

        put("txtI12TotalCapitalGains", amount(self.s1_total_gains));
        put("txtI13TotalCapitalLoss", amount(self.s2_total_loss));
        put("txtI14NetCapitalGainLoss", amount(self.net_capital_gain));
        put("txtI16ApplicableTaxRate", "15".to_string());
        put("txtI15TaxDue", amount(self.tax_due));
        put("txtI16ATotalTaxPaid", amount(self.s1_total_tax_paid));
        put("txtI16BTotalTaxPaid", amount(self.tax_paid_previous));
        put("txtI16CTotalTaxPaid", amount(self.total_tax_paid));
        put("txtI17TaxStillPayable", amount(self.tax_payable));
        put("txtI18ASurcharge", amount(self.surcharge));
        put("txtI18BInterest", amount(self.interest));
        put("txtI18CCompromise", amount(self.compromise));
        put("txtI18DPenalties", amount(self.total_penalties));
        put(
            "txtI19TotalAmountPayable",
            amount(self.total_amount_payable),
        );

        // Page 2 header, as loadBGData fills it (TIN3 gets TIN2).
        put("txtI5TIN1P2", tin1);
        put("txtI5TIN2P2", tin2.clone());
        put("txtI5TIN3P2", tin2);
        put("txtI5BranchMaskP2", branch.clone());
        put("hdnI5TIN4P2", branch);
        put("txtI8RegisteredNameP2", self.taxpayer_name.clone());

        for index in 0..FORM_1707A_SCHEDULE_ROWS {
            let n = index + 1;
            let gain = Self::row(&self.gains, index);
            put(
                &format!("txtS1C1DateOfTransactionI{n}"),
                gain.date.trim().to_string(),
            );
            put(
                &format!("txtS1C2NameOfCorporateStockI{n}"),
                gain.corporation.trim().to_uppercase(),
            );
            put(
                &format!("txtS1C3SellingPriceI{n}"),
                amount(gain.selling_price),
            );
            put(&format!("txtS1C4CostI{n}"), amount(gain.cost));
            put(
                &format!("txtS1C5CapitalGainsI{n}"),
                amount(gain.gain_or_loss),
            );
            put(&format!("txtS1C6TaxPaidI{n}"), amount(gain.tax_paid));
            let loss = Self::row(&self.losses, index);
            put(
                &format!("txtS2C1DateOfTransactionI{n}"),
                loss.date.trim().to_string(),
            );
            put(
                &format!("txtS2C2NameOfCorporateStockI{n}"),
                loss.corporation.trim().to_uppercase(),
            );
            put(
                &format!("txtS2C3SellingPriceI{n}"),
                amount(loss.selling_price),
            );
            put(&format!("txtS2C4CostI{n}"), amount(loss.cost));
            put(
                &format!("txtS2C5CapitalLossI{n}"),
                amount(loss.gain_or_loss),
            );
        }
        put("txtS1I20TotalCost", amount(self.s1_total_cost));
        put(
            "txtS1I20TotalSellingPrice",
            amount(self.s1_total_selling_price),
        );
        put("txtS1I20TotalCapitalGains", amount(self.s1_total_gains));
        put("txtS1I20TotalTaxPaid", amount(self.s1_total_tax_paid));
        put(
            "txtS2I21TotalSellingPrice",
            amount(self.s2_total_selling_price),
        );
        put("txtS2I21TotalCost", amount(self.s2_total_cost));
        put("txtS2I21TotalCapitalLoss", amount(self.s2_total_loss));
        fields
    }

    /// The exact official submit plaintext.
    pub fn to_bir_xml_payload(&self) -> Result<String, Vec<(String, String)>> {
        self.official_payload()
    }

    /// `validateAll()` against a given "today" (the page uses the clock).
    pub fn validate_on(&self, today: NaiveDate) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        let mut err = |field: &str, message: &str| {
            errors.push((field.to_string(), message.to_string()));
        };

        // validateYearEnd, then checkVersion.
        match self.year_end() {
            None => err(
                "year_end",
                "Please provide a valid date (MM/DD/YYYY format) on Item 1",
            ),
            Some(year_end) => {
                if year_end < NaiveDate::from_ymd_opt(2001, 7, 1).unwrap_or(year_end) {
                    err(
                        "year_end",
                        "Valid input for the Month and Year is June 2001 onwards on Item 1.",
                    );
                } else if year_end > today {
                    err(
                        "year_end",
                        "Year Ended should not be a future date on Item 1.",
                    );
                } else if self.fiscal && self.year_end_month == 12 {
                    err("year_end", "Month cannot be equal to December on Item 1.");
                } else if year_end <= NaiveDate::from_ymd_opt(2021, 3, 31).unwrap_or(year_end) {
                    err(
                        "year_end",
                        "Year shall not be greater than the present year and not earlier than April 2021.",
                    );
                }
            }
        }
        match self.atc {
            None => err("atc", "Please select an ATC Code on Item 5."),
            Some(Form1707AAtc::Individual) if self.fiscal => err(
                "atc",
                "If you are filing as Individual, please select Calendar on item 1.",
            ),
            Some(_) => {}
        }
        let rdo = self.rdo_code.trim();
        if rdo.is_empty() || rdo == "000" || !crate::validation::rdo_code_is_official_option(rdo) {
            err("rdo_code", "Please provide a valid RDO Code on Item 7.");
        }
        let name_limit = if self.atc == Some(Form1707AAtc::Corporation) {
            50
        } else {
            90
        };
        if self.taxpayer_name.trim().is_empty() {
            err(
                "taxpayer_name",
                "Please provide a valid Line of Business/Occupation on Item 8.",
            );
        } else if self.taxpayer_name.trim().chars().count() > name_limit {
            err(
                "taxpayer_name",
                &format!("Item 8 holds at most {name_limit} characters."),
            );
        }
        if self.registered_address.trim().is_empty() {
            err(
                "registered_address",
                "Please provide a valid Registered Address on Item 9.",
            );
        } else if self.registered_address.trim().chars().count() > 60 {
            err("registered_address", "Item 9 holds at most 60 characters.");
        }
        // The page only warns here; a submission needs the zip code.
        if self.zip_code.trim().is_empty() {
            err(
                "zip_code",
                "Zip Code is required, please validate your registration information and update accordingly using BIR Form 1905, if necessary.",
            );
        }
        if let Some(spec) = &self.tax_relief {
            if spec.trim().is_empty() {
                err("tax_relief", "Please specify the tax relief on item 12A.");
            } else if spec.trim().chars().count() > 20 {
                err("tax_relief", "Item 12A holds at most 20 characters.");
            }
        }
        if self.s1_total_selling_price == 0.0
            && self.s1_total_cost == 0.0
            && self.s2_total_selling_price == 0.0
            && self.s2_total_cost == 0.0
        {
            err(
                "gains",
                "You need to have at least one value on either Schedule 1 or Schedule 2.",
            );
        }

        // validateSchedules for Schedule 1, then Schedule 2.
        let year_end = self.year_end();
        let year_from = year_end.and_then(|d| d.with_year(d.year() - 1));
        for (schedule, field, rows, is_sched1) in [
            (" Schedule 1", "gains", &self.gains, true),
            (" Schedule 2", "losses", &self.losses, false),
        ] {
            if rows.len() > FORM_1707A_SCHEDULE_ROWS {
                err(field, &format!("{} holds rows 1 to 4.", schedule.trim()));
            }
            for (index, row) in rows.iter().enumerate().take(FORM_1707A_SCHEDULE_ROWS) {
                let n = index + 1;
                let key = format!("{field}[{index}]");
                let mut empty = 0;
                if row.date_blank() {
                    empty += 1;
                } else {
                    match parse_mmddyyyy(row.date.trim()) {
                        None => {
                            err(
                                &key,
                                &format!(
                                    "Please provide a valid date. (MM/DD/YYYY format) in Date of Transaction on row {n}{schedule}"
                                ),
                            );
                            continue;
                        }
                        Some(date) => {
                            if date.year() < 2001 {
                                err(
                                    &key,
                                    &format!(
                                        "The year should not be less than 2001 in Date of Transaction on row {n}{schedule}"
                                    ),
                                );
                                continue;
                            } else if year_end.is_some_and(|end| date > end) {
                                err(
                                    &key,
                                    &format!(
                                        "The Date of Transaction should not be greater than the Taxable Period on row {n}{schedule}"
                                    ),
                                );
                                continue;
                            } else if year_from.is_some_and(|from| date < from) {
                                err(
                                    &key,
                                    &format!(
                                        "The Date of Transaction should be in the range of up to one year until the Taxable Period on row {n}{schedule}"
                                    ),
                                );
                                continue;
                            }
                        }
                    }
                }
                let corporation = row.corporation.trim();
                if corporation.is_empty() {
                    empty += 1;
                } else if corporation.parse::<f64>().is_ok() {
                    err(
                        &key,
                        &format!(
                            "Name of Corporate Stock should not be all numeric in row {n}{schedule}"
                        ),
                    );
                    continue;
                }
                if row.selling_price == 0.0 {
                    empty += 1;
                }
                if row.cost == 0.0 {
                    empty += 1;
                }
                if empty != 0 && empty != 4 {
                    err(
                        &key,
                        &format!(
                            "There is an empty item on {schedule}. Please fill in all items in row {n}"
                        ),
                    );
                    continue;
                }
                if is_sched1 {
                    if row.tax_paid != 0.0 && empty > 0 {
                        err(
                            &key,
                            &format!(
                                "There is an empty item on {schedule}. Please fill in all items in row {n}"
                            ),
                        );
                        continue;
                    }
                    if row.gain_or_loss < 0.0 {
                        err(
                            &key,
                            &format!(
                                "Your Capital Gains is negative in row {n}{schedule}\n Please move and encode this in Schedule 2"
                            ),
                        );
                    }
                } else if row.gain_or_loss < 0.0 {
                    err(
                        &key,
                        &format!(
                            "Your Capital Loss is negative in row {n}{schedule}\n Please move and encode this in Schedule 1"
                        ),
                    );
                }
                // `.txtCorporate` keeps 17 of [a-zA-Z 0-9.,#@'()_-].
                if corporation.chars().count() > 17
                    || !corporation
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || " .,#@'()_-".contains(c))
                {
                    err(
                        &key,
                        "Name of Corporate Stock holds 17 letters, digits, spaces or .,#@'()_- only.",
                    );
                }
            }
        }

        // Typing limits.
        let mut amounts: Vec<(String, f64)> = vec![
            ("tax_paid_previous".into(), self.tax_paid_previous),
            ("surcharge".into(), self.surcharge),
            ("interest".into(), self.interest),
            ("compromise".into(), self.compromise),
        ];
        for (field, rows) in [("gains", &self.gains), ("losses", &self.losses)] {
            for (index, row) in rows.iter().enumerate() {
                amounts.push((format!("{field}[{index}].selling_price"), row.selling_price));
                amounts.push((format!("{field}[{index}].cost"), row.cost));
                amounts.push((format!("{field}[{index}].tax_paid"), row.tax_paid));
            }
        }
        for (field, value) in amounts {
            if value < 0.0 || value.abs() >= 1_000_000_000_000.0 {
                err(
                    &field,
                    "Enter a non-negative amount below 1,000,000,000,000.",
                );
            }
        }
        if !self.is_amended && self.tax_paid_previous != 0.0 {
            err(
                "tax_paid_previous",
                "Item 16B applies only to an amended return.",
            );
        }
        if self.number_of_attached_sheets > 99 {
            err(
                "number_of_attached_sheets",
                "Item 3 holds at most two digits.",
            );
        }
        let email = self.email.trim();
        if email.is_empty()
            || !email.contains('@')
            || email.contains(char::is_whitespace)
            || email.contains('#')
            || email.chars().count() > 60
        {
            err(
                "email",
                "Enter the email address BIR sends the filing confirmation to.",
            );
        }
        let (tin1, tin2, tin3, _) = split_tin(&self.tin);
        if !tin_is_well_formed(&self.tin) {
            err("tin", "The taxpayer profile needs a valid TIN.");
        } else if !crate::validation::relaxed_dev_mode()
            && crate::validation::official_tin_check_code(&format!("{tin1}{tin2}{tin3}")) != 0
        {
            err("tin", crate::validation::OFFICIAL_INVALID_TIN_MESSAGE);
        }

        let mut expected = self.clone();
        expected.recompute();
        if expected != *self {
            err(
                "total_amount_payable",
                "Totals are out of date. Recompute the return.",
            );
        }
        errors
    }
}

impl FormValidator for Form1707ADraft {
    fn validate(&self) -> Vec<(String, String)> {
        self.validate_on(chrono::Local::now().date_naive())
    }
}

impl QueueableForm for Form1707ADraft {
    const FORM_CODE: &'static str = "1707A";
    /// Official `formType` and SFTP folder (`ftpTargetFolder.PROD['1707Av2021']`).
    const FORM_TYPE: &'static str = "1707Av2021";
    const LAYOUT_ID: &'static str = FORM_1707A_FORM_ID;

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
        self.filing_year
    }
    fn filing_period(&self) -> FilingPeriod {
        FilingPeriod::OpenEnded(self.open_ended_key)
    }
    /// Year-end `MM + DD + YYYY`, as in `createXMLFileName`.
    fn period_code(&self) -> String {
        format!(
            "{:02}{:02}{:04}",
            self.year_end_month, self.year_end_day, self.year_end_year
        )
    }
    /// The filename names the year end, not the dashboard's open-ended key,
    /// so a receipt cannot be mapped back to a draft row.
    fn parse_period_code(_code: &str) -> Option<(u16, FilingPeriod)> {
        None
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
        NaiveDate::from_ymd_opt(2026, 6, 30).unwrap()
    }

    fn row(date: &str, name: &str, selling: f64, cost: f64, tax_paid: f64) -> Form1707ARow {
        Form1707ARow {
            date: date.into(),
            corporation: name.into(),
            selling_price: selling,
            cost,
            gain_or_loss: 0.0,
            tax_paid,
        }
    }

    pub(crate) fn sample() -> Form1707ADraft {
        let mut draft = Form1707ADraft {
            id: None,
            filing_year: 2025,
            open_ended_key: 1,
            fiscal: false,
            year_end_month: 12,
            year_end_day: 31,
            year_end_year: 2025,
            is_amended: false,
            number_of_attached_sheets: 0,
            atc: Some(Form1707AAtc::Individual),
            tin: "12345678800000".into(),
            rdo_code: "039".into(),
            taxpayer_name: "Sample Taxpayer".into(),
            line_of_business: "Investor".into(),
            registered_address: "1 Sample St".into(),
            zip_code: "1100".into(),
            contact_number: "09170000000".into(),
            email: "sample.taxpayer@example.com".into(),
            tax_relief: None,
            gains: vec![
                row(
                    "02/15/2025",
                    "Sample Corp",
                    500_000.009,
                    120_000.0,
                    15_000.5,
                ),
                row(
                    "06/30/2025",
                    "Example Mining",
                    1_234_567.891,
                    1_000_000.105,
                    0.0,
                ),
            ],
            losses: vec![row("09/01/2025", "Loss Co", 50_000.0, 80_000.55, 0.0)],
            s1_total_selling_price: 0.0,
            s1_total_cost: 0.0,
            s1_total_gains: 0.0,
            s1_total_tax_paid: 0.0,
            s2_total_selling_price: 0.0,
            s2_total_cost: 0.0,
            s2_total_loss: 0.0,
            net_capital_gain: 0.0,
            tax_due: 0.0,
            tax_paid_previous: 0.0,
            total_tax_paid: 0.0,
            tax_payable: 0.0,
            surcharge: 25.5,
            interest: 10.009,
            compromise: 0.0,
            total_penalties: 0.0,
            total_amount_payable: 0.0,
            lifecycle: SubmissionLifecycle::default(),
        };
        draft.recompute();
        draft
    }

    fn messages(draft: &Form1707ADraft) -> Vec<String> {
        draft
            .validate_on(today())
            .into_iter()
            .map(|(_, m)| m)
            .collect()
    }

    #[test]
    fn amount_helpers_match_the_page() {
        assert_eq!(round_amount_text("10.009"), "10.00");
        assert_eq!(round_amount_text("1234567.8"), "1,234,567.80");
        assert_eq!(round_amount_text("-12.5"), "(12.50)");
        assert_eq!(round_amount_text("-0.5"), "0.50");
        assert_eq!(round_amount_text(""), "0.00");
        assert_eq!(value_for_compute("(1,234.50)"), -1234.5);
        assert_eq!(submit_text(-1234.5), "-1234.50");
        assert_eq!(submit_text(1234.5), "1234.50");
    }

    #[test]
    fn compute_chain_matches_the_official_page() {
        // Values the official page shows for the same entries (runtime.js).
        let d = sample();
        assert_eq!(d.gains[0].selling_price, 500_000.0);
        assert_eq!(d.gains[1].cost, 1_000_000.1);
        assert_eq!(d.s1_total_gains, 614_567.79);
        assert_eq!(d.s2_total_loss, 30_000.55);
        assert_eq!(d.net_capital_gain, 584_567.24);
        assert_eq!(d.tax_due, 87_685.09);
        assert_eq!(d.tax_payable, 72_684.59);
        assert_eq!(d.total_penalties, 35.5);
        assert_eq!(d.total_amount_payable, 72_720.09);
        assert!(messages(&d).is_empty(), "{:?}", messages(&d));
    }

    #[test]
    fn field_map_uses_official_formats() {
        let d = sample();
        let f = d.to_bir_field_map();
        assert_eq!(f["frm1707Av2021:txtI12TotalCapitalGains"], "614567.79");
        assert_eq!(f["frm1707Av2021:txtI5TIN3P2"], "456");
        assert_eq!(f["frm1707Av2021:txtI8RegisteredName"], "SAMPLE TAXPAYER");
        assert_eq!(f["frm1707Av2021:txtI8RegisteredNameP2"], "Sample Taxpayer");
        assert_eq!(
            d.submission_filename(),
            "12345678800000-1707Av2021-12312025#sample.taxpayer@example.com#.xml"
        );
    }

    #[test]
    fn overpayment_with_penalties_pays_penalties_only() {
        let mut d = sample();
        d.set_amended(true);
        d.tax_paid_previous = 100_000.0;
        d.recompute();
        assert_eq!(d.tax_payable, -27_315.41);
        assert_eq!(d.total_amount_payable, 35.5);
        assert_eq!(
            d.to_bir_field_map()["frm1707Av2021:txtI17TaxStillPayable"],
            "-27315.41"
        );
    }

    #[test]
    fn official_negative_cases_are_rejected_with_their_alerts() {
        let check = |mutate: &dyn Fn(&mut Form1707ADraft), expected: &str| {
            let mut d = sample();
            mutate(&mut d);
            d.recompute();
            let found = messages(&d);
            assert!(
                found.iter().any(|m| m == expected),
                "{expected:?} not in {found:?}"
            );
        };
        check(
            &|d| d.year_end_year = 2026,
            "Year Ended should not be a future date on Item 1.",
        );
        check(
            &|d| d.year_end_year = 2000,
            "Valid input for the Month and Year is June 2001 onwards on Item 1.",
        );
        check(
            &|d| d.year_end_year = 2020,
            "Year shall not be greater than the present year and not earlier than April 2021.",
        );
        check(
            &|d| {
                d.atc = Some(Form1707AAtc::Corporation);
                d.fiscal = true;
                d.year_end_month = 2;
                d.year_end_day = 30;
            },
            "Please provide a valid date (MM/DD/YYYY format) on Item 1",
        );
        check(
            &|d| {
                d.fiscal = true;
                d.year_end_month = 6;
                d.year_end_day = 30;
            },
            "If you are filing as Individual, please select Calendar on item 1.",
        );
        check(&|d| d.atc = None, "Please select an ATC Code on Item 5.");
        check(
            &|d| d.rdo_code = "000".into(),
            "Please provide a valid RDO Code on Item 7.",
        );
        check(
            &|d| d.taxpayer_name.clear(),
            "Please provide a valid Line of Business/Occupation on Item 8.",
        );
        check(
            &|d| d.registered_address.clear(),
            "Please provide a valid Registered Address on Item 9.",
        );
        check(
            &|d| d.tax_relief = Some(" ".into()),
            "Please specify the tax relief on item 12A.",
        );
        check(
            &|d| {
                d.gains.clear();
                d.losses.clear();
            },
            "You need to have at least one value on either Schedule 1 or Schedule 2.",
        );
        check(
            &|d| d.gains[0].date = "13/01/2025".into(),
            "Please provide a valid date. (MM/DD/YYYY format) in Date of Transaction on row 1 Schedule 1",
        );
        check(
            &|d| d.gains[0].date = "12/01/2024".into(),
            "The Date of Transaction should be in the range of up to one year until the Taxable Period on row 1 Schedule 1",
        );
        check(
            &|d| d.gains[0].date = "01/02/2026".into(),
            "The Date of Transaction should not be greater than the Taxable Period on row 1 Schedule 1",
        );
        check(
            &|d| d.gains[0].corporation = "12345".into(),
            "Name of Corporate Stock should not be all numeric in row 1 Schedule 1",
        );
        check(
            &|d| d.gains[0].corporation.clear(),
            "There is an empty item on  Schedule 1. Please fill in all items in row 1",
        );
        check(
            &|d| d.gains[0].cost = 600_000.0,
            "Your Capital Gains is negative in row 1 Schedule 1\n Please move and encode this in Schedule 2",
        );
        check(
            &|d| d.losses[0].selling_price = 90_000.0,
            "Your Capital Loss is negative in row 1 Schedule 2\n Please move and encode this in Schedule 1",
        );
    }

    #[test]
    fn queue_and_revalidate_through_the_generic_path() {
        let mut d = sample();
        d.queue(crate::filing_queue::QueueAuthSource::Gui).unwrap();
        assert!(d.revalidate_queued_before_submission().is_ok());
        d.surcharge = 99.0;
        assert!(d.revalidate_queued_before_submission().is_err());
    }
}
