//! BIR Form 2200-S (January 2018) — Excise Tax Return for Sweetened
//! Beverages.
//!
//! Ported from the official `BIR-Form2200S.hta` (eBIRForms 7.9.6.2.1):
//! Schedule 1 (`createStaticFieldForSched1` / `getBeverages`,
//! `computeBasicTaxDue`, the "Others" row `computeSched1BasicTaxDue`,
//! `totalTaxDue`), Part III (`compute17C` … `compute24`, `compute21D`,
//! `payPenalties`), the manner of payment rules (`changeMannerOfPayment`,
//! `dateMonthYear`, `recomputePrepayment`), `validateForm` with its exact
//! alerts and `saveXMLsubmit` through [`crate::official_xml`].
//!
//! Amounts on this page are truncated, not rounded: `roundDownWithAlert`
//! cuts typed text to two decimals and `roundDownComputation` cuts
//! `toPrecision(14)` of each computed value.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::excise_places::{ExcisePlace, is_official_place};
use super::form_2000::{digits_only, split_tin, tin_error};
use super::queueable::{QueueableForm, SubmissionLifecycle};
use super::{FilingPeriod, FormValidator};
use crate::profile::TaxpayerProfile;

/// Rule-package id of the official layout.
pub const FORM_2200S_FORM_ID: &str = "2200s-v2018";
const PLACE_FORM: &str = "2200S";

/// One fixed Schedule 1 beverage row (`getBeverages`, no `2200S` ATCs in
/// `atcCodes.xml`, so the page's built-in list applies).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Form2200SBeverage {
    pub atc: &'static str,
    pub description: &'static str,
    /// The hidden `hideProAppRate` value, pesos per liter.
    pub rate: f64,
}

pub const FORM_2200S_BEVERAGES: [Form2200SBeverage; 10] = [
    Form2200SBeverage {
        atc: "XB010",
        description: "a. Sweetened Juice Drinks",
        rate: 6.0,
    },
    Form2200SBeverage {
        atc: "XB020",
        description: "b. Sweetened Tea",
        rate: 6.0,
    },
    Form2200SBeverage {
        atc: "XB030",
        description: "c. Carbonated Beverages",
        rate: 6.0,
    },
    Form2200SBeverage {
        atc: "XB040",
        description: "d. Flavored Water",
        rate: 6.0,
    },
    Form2200SBeverage {
        atc: "XB050",
        description: "e. Energy and Sports Drinks",
        rate: 6.0,
    },
    Form2200SBeverage {
        atc: "XB060",
        description: "f. Powdered Drinks not classified as Milk, Juice, Tea and Coffee",
        rate: 6.0,
    },
    Form2200SBeverage {
        atc: "XB070",
        description: "g. Cereal and Grain Beverages",
        rate: 6.0,
    },
    Form2200SBeverage {
        atc: "XB080",
        description: "h. Other Non-Alcoholic Beverages that contain added Sugar",
        rate: 6.0,
    },
    Form2200SBeverage {
        atc: "XB090",
        description: "2. Using purely high fructose corn syrup or in combination with any caloric or non-caloric sweeteners",
        rate: 12.0,
    },
    Form2200SBeverage {
        atc: "XB100",
        description: "3. Using purely coconut sap sugar and purely Steviol Glycosides",
        rate: 0.0,
    },
];

// ── Truncating amount helpers (js/string-util2014.js) ──

/// JavaScript `Number#toString` for the amounts this page handles.
fn js_number_text(value: f64) -> String {
    if value == 0.0 {
        "0".to_string()
    } else {
        format!("{value}")
    }
}

/// JavaScript `Number#toPrecision(14)` read back as a number (ties to the
/// larger magnitude, like the spec's "pick the larger n").
fn js_to_precision14(value: f64) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let exact = format!("{:.80e}", value.abs());
    let (mantissa, exponent) = exact.split_once('e').unwrap_or((&exact, "0"));
    let digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    let mut kept: Vec<u8> = digits[..14].to_vec();
    let mut exponent: i32 = exponent.parse().unwrap_or(0);
    if digits[14] >= b'5' {
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, b'1');
                kept.pop();
                exponent += 1;
                break;
            }
            i -= 1;
            if kept[i] == b'9' {
                kept[i] = b'0';
            } else {
                kept[i] += 1;
                break;
            }
        }
    }
    let text = format!(
        "{}.{}e{}",
        kept[0] as char,
        std::str::from_utf8(&kept[1..]).unwrap_or("0"),
        exponent
    );
    let magnitude: f64 = text.parse().unwrap_or(0.0);
    if value < 0.0 { -magnitude } else { magnitude }
}

/// `addCommas` on an integer text, sign kept.
fn add_commas(int: &str) -> String {
    let (sign, digits) = match int.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", int),
    };
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{sign}{out}")
}

/// The text a truncated amount shows (`1,234.50`, `-0.50`).
fn truncated_text(number_text: &str) -> String {
    match number_text.split_once('.') {
        Some((int, frac)) => {
            let int = if int == "-0" {
                "-0".to_string()
            } else {
                add_commas(int)
            };
            let cents = if frac.len() == 1 {
                format!("{frac}0")
            } else {
                frac[..2].to_string()
            };
            format!("{int}.{cents}")
        }
        None => format!("{}.00", add_commas(number_text)),
    }
}

/// `roundDownWithAlert` on a typed amount: its text cut to two decimals.
pub fn round_down_input_text(value: f64) -> String {
    truncated_text(&js_number_text(value))
}

/// `roundDownComputation` on a computed amount.
pub fn round_down_computation_text(value: f64) -> String {
    truncated_text(&js_number_text(js_to_precision14(value)))
}

fn parse_amount(text: &str) -> f64 {
    text.replace(',', "").parse().unwrap_or(0.0)
}

/// A typed amount as the field holds it after its blur.
fn typed(value: f64) -> f64 {
    parse_amount(&round_down_input_text(value))
}

/// A computed amount as `roundDownComputation` leaves it.
fn computed(value: f64) -> f64 {
    parse_amount(&round_down_computation_text(value))
}

fn has_cent_precision(value: f64) -> bool {
    value.is_finite() && ((value * 100.0) - (value * 100.0).round()).abs() < 1e-7
}

// ── Model ──

/// Item 14 — tax relief under a Special Law or International Tax Treaty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form2200STaxRelief {
    #[default]
    No,
    /// "Yes" with nothing picked in Item 14A yet.
    YesUnspecified,
    SpecialLaw,
    InternationalTaxTreaty,
}

impl Form2200STaxRelief {
    fn list_value(self) -> &'static str {
        match self {
            Self::No | Self::YesUnspecified => "0",
            Self::SpecialLaw => "1",
            Self::InternationalTaxTreaty => "2",
        }
    }
}

/// Part II — Manner of payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form2200SManner {
    #[default]
    Unanswered,
    /// Payment on Actual Removal: Schedule 1 is required, penalties are off.
    ActualRemoval,
    /// Prepayment/Advance Deposit: Item 1 is the filing date, Item 18 is zero.
    Prepayment,
}

/// One Schedule 1 beverage row (the page's ten fixed rows).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Form2200SBeverageRow {
    pub sales_value: f64,
    /// Liters.
    pub volume: f64,
    #[serde(default)]
    pub basic_tax_due: f64,
}

/// The Schedule 1 "Others (specify)" row.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Form2200SOtherRow {
    /// `XB` + three digits, not one of the fixed rows' ATCs.
    pub atc: String,
    pub description: String,
    pub tax_bracket: String,
    pub rate: f64,
    pub sales_value: f64,
    pub volume: f64,
    #[serde(default)]
    pub basic_tax_due: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Form2200SDraft {
    #[serde(default)]
    pub id: Option<i64>,

    /// Item 1.
    pub month: u8,
    pub day: u8,
    pub year: u16,
    /// Item 2.
    pub is_amended: bool,
    /// Item 4: 9-digit TIN then the 5-digit branch code.
    pub tin: String,
    /// Item 5.
    pub rdo_code: String,
    /// Item 6.
    pub taxpayer_name: String,
    /// Item 7.
    pub registered_address: String,
    /// Item 7A.
    pub zip_code: String,
    /// Item 8.
    pub contact_number: String,
    /// Item 9.
    pub line_of_business: String,
    /// Item 10 (4 to 6 digits).
    #[serde(default)]
    pub psic: String,
    /// Item 11.
    pub email: String,
    /// Item 12.
    #[serde(default)]
    pub place_of_production: ExcisePlace,
    /// Item 13.
    #[serde(default)]
    pub place_of_removal: ExcisePlace,
    /// Items 14 / 14A.
    #[serde(default)]
    pub tax_relief: Form2200STaxRelief,
    /// Items 15 / 16.
    #[serde(default)]
    pub manner: Form2200SManner,
    /// Item 17 "Other similar schemes", only with a prepayment.
    #[serde(default)]
    pub other_scheme: bool,
    #[serde(default)]
    pub other_scheme_text: String,

    /// Schedule 1 fixed rows, in `FORM_2200S_BEVERAGES` order.
    #[serde(default)]
    pub beverages: Vec<Form2200SBeverageRow>,
    #[serde(default)]
    pub others: Option<Form2200SOtherRow>,
    /// Schedule 1 total tax due.
    #[serde(default)]
    pub schedule_total: f64,

    // Part III (printed item numbers)
    /// 18.
    #[serde(default)]
    pub excise_tax_due: f64,
    /// 19A.
    #[serde(default)]
    pub balance_carried_over: f64,
    /// 19B.
    #[serde(default)]
    pub creditable_excise_tax: f64,
    /// 19C.
    #[serde(default)]
    pub total_credits: f64,
    /// 20.
    #[serde(default)]
    pub net_tax_due: f64,
    /// 21, only on an amended return.
    #[serde(default)]
    pub previous_payment: f64,
    /// 22.
    #[serde(default)]
    pub tax_still_due: f64,
    /// 23A–23D (prepayment only).
    #[serde(default)]
    pub surcharge: f64,
    #[serde(default)]
    pub interest: f64,
    #[serde(default)]
    pub compromise: f64,
    #[serde(default)]
    pub total_penalties: f64,
    /// 24.
    #[serde(default)]
    pub amount_payable: f64,
    /// 25A.
    #[serde(default)]
    pub tax_payment: f64,
    /// 25B (penalties paid with the return).
    #[serde(default)]
    pub penalties_paid: f64,
    /// 25C.
    #[serde(default)]
    pub total_payment: f64,
    /// 26.
    #[serde(default)]
    pub balance_carried_forward: f64,

    #[serde(flatten)]
    pub lifecycle: SubmissionLifecycle,
}

impl Form2200SDraft {
    pub const FORM_CODE: &'static str = "2200S";

    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16) -> Self {
        let today = chrono::Local::now().date_naive();
        let (month, day) = if i32::from(year) == chrono::Datelike::year(&today) {
            (
                chrono::Datelike::month(&today) as u8,
                chrono::Datelike::day(&today) as u8,
            )
        } else {
            (1, 1)
        };
        let mut draft = Self {
            id: None,
            month,
            day,
            year,
            is_amended: false,
            tin: profile.tin.full(),
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            line_of_business: profile.line_of_business.clone(),
            psic: String::new(),
            email: profile.email.clone(),
            place_of_production: ExcisePlace::default(),
            place_of_removal: ExcisePlace::default(),
            tax_relief: Form2200STaxRelief::No,
            manner: Form2200SManner::Unanswered,
            other_scheme: false,
            other_scheme_text: String::new(),
            beverages: Vec::new(),
            others: None,
            schedule_total: 0.0,
            excise_tax_due: 0.0,
            balance_carried_over: 0.0,
            creditable_excise_tax: 0.0,
            total_credits: 0.0,
            net_tax_due: 0.0,
            previous_payment: 0.0,
            tax_still_due: 0.0,
            surcharge: 0.0,
            interest: 0.0,
            compromise: 0.0,
            total_penalties: 0.0,
            amount_payable: 0.0,
            tax_payment: 0.0,
            penalties_paid: 0.0,
            total_payment: 0.0,
            balance_carried_forward: 0.0,
            lifecycle: SubmissionLifecycle::default(),
        };
        draft.recompute();
        draft
    }

    /// The `Others` row's rate as `setApplicableTaxrate` leaves its text.
    fn others_rate_text(rate: f64) -> String {
        if !(0.0..=999.99).contains(&rate) {
            return "0.00".to_string();
        }
        round_down_input_text(rate)
    }

    /// The official compute chain.
    pub fn recompute(&mut self) {
        let actual = self.manner == Form2200SManner::ActualRemoval;
        if !actual {
            // Schedule 1 belongs to a payment on actual removal.
            self.beverages.clear();
            self.others = None;
        }
        if self.manner != Form2200SManner::Prepayment {
            self.other_scheme = false;
            // disablePenalties
            self.surcharge = 0.0;
            self.interest = 0.0;
            self.compromise = 0.0;
        }
        if !self.other_scheme {
            self.other_scheme_text.clear();
        }
        if !self.is_amended {
            self.previous_payment = 0.0;
        }
        self.beverages
            .resize_with(if actual { 10 } else { 0 }, Default::default);

        // totalTaxDue: rows 0–8 of the fixed table, then the Others row.
        let mut total = 0.0;
        for (row, beverage) in self.beverages.iter_mut().zip(FORM_2200S_BEVERAGES) {
            row.sales_value = typed(row.sales_value);
            row.volume = typed(row.volume);
            row.basic_tax_due = computed(beverage.rate * row.volume);
        }
        for row in self.beverages.iter().take(9) {
            total += row.basic_tax_due;
        }
        if let Some(other) = &mut self.others {
            other.atc = other.atc.trim().to_uppercase();
            other.rate = parse_amount(&Self::others_rate_text(other.rate));
            other.sales_value = typed(other.sales_value);
            other.volume = typed(other.volume);
            other.basic_tax_due = computed(other.rate * other.volume);
            total += other.basic_tax_due;
        }
        self.schedule_total = computed(total);

        self.excise_tax_due = if actual { self.schedule_total } else { 0.0 };
        self.balance_carried_over = typed(self.balance_carried_over);
        self.creditable_excise_tax = typed(self.creditable_excise_tax);
        self.total_credits = computed(self.balance_carried_over + self.creditable_excise_tax);
        self.net_tax_due = computed(self.excise_tax_due - self.total_credits);
        self.previous_payment = typed(self.previous_payment);
        self.tax_still_due = computed(self.net_tax_due - self.previous_payment);
        self.surcharge = typed(self.surcharge);
        self.interest = typed(self.interest);
        self.compromise = typed(self.compromise);
        self.total_penalties = computed(self.surcharge + self.interest + self.compromise);
        self.amount_payable = computed(self.tax_still_due + self.total_penalties);
        self.tax_payment = typed(self.tax_payment);
        // compute21D ticks "Pay Penalties?" whenever there are penalties.
        self.penalties_paid = if self.total_penalties != 0.0 {
            self.total_penalties
        } else {
            0.0
        };
        self.total_payment = computed(self.tax_payment + self.penalties_paid);
        self.balance_carried_forward = computed(self.amount_payable - self.total_payment);
    }

    /// The official field values `saveXMLsubmit` reads, keyed by element id.
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let mut put = |key: &str, value: String| {
            fields.insert(format!("frm2200S:{key}"), value);
        };
        let flag = |on: bool| on.to_string();
        let money = round_down_input_text;
        let sum = round_down_computation_text;

        put("txtMonth1", format!("{:02}", self.month));
        put("txtDate", format!("{:02}", self.day));
        put("txtForYr", self.year.to_string());
        put("amendedRtn_1", flag(self.is_amended));
        put("amendedRtn_2", flag(!self.is_amended));
        put("txtSheets", "0".to_string());
        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);
        put("txtTIN1", tin1.clone());
        put("txtTIN2", tin2.clone());
        put("txtTIN3", tin3.clone());
        put("txtBranchCode", branch.clone());
        put("txtRDOCode", self.rdo_code.trim().to_string());
        // loadBGData uppercases the profile name, address and line of business.
        let name = self.taxpayer_name.trim().to_uppercase();
        put("taxpayerName", name.clone());
        put("txtAddress", self.registered_address.trim().to_uppercase());
        put("txtZipCode", self.zip_code.trim().to_string());
        put("txtTelNum", self.contact_number.trim().to_string());
        put("txtLineBus", self.line_of_business.trim().to_uppercase());
        put("txtPSIC", self.psic.trim().to_string());
        put(
            "optTreaty_1",
            flag(self.tax_relief != Form2200STaxRelief::No),
        );
        put(
            "optTreaty_2",
            flag(self.tax_relief == Form2200STaxRelief::No),
        );
        put("lstTaxTreaty", self.tax_relief.list_value().to_string());
        put(
            "chkPymntManner_1",
            flag(self.manner == Form2200SManner::ActualRemoval),
        );
        put(
            "chkPymntManner_2",
            flag(self.manner == Form2200SManner::Prepayment),
        );
        put("chkPymntManner_3", flag(self.other_scheme));
        put(
            "txtOthMannerofPymnt",
            self.other_scheme_text.trim().to_string(),
        );

        put("txtTax16", sum(self.excise_tax_due));
        put("txtTax17A", money(self.balance_carried_over));
        put("txtTax17B", money(self.creditable_excise_tax));
        put("txtTax17C", sum(self.total_credits));
        put("txtTax18", sum(self.net_tax_due));
        put("txtTax19", money(self.previous_payment));
        put("txtTax20", sum(self.tax_still_due));
        put("txtTax21A", money(self.surcharge));
        put("txtTax21B", money(self.interest));
        put("txtTax21C", money(self.compromise));
        put("txtTax21D", sum(self.total_penalties));
        put("txtTax22", sum(self.amount_payable));
        put("txtTax23A", money(self.tax_payment));
        put("PayPenalties", flag(self.penalties_paid != 0.0));
        put("txtTax23B", sum(self.penalties_paid));
        put("txtTax23C", sum(self.total_payment));
        put("txtTax24", sum(self.balance_carried_forward));

        // showSched1 copies the TIN and name into the Schedule 1 header when
        // the schedule is opened, which a payment on actual removal requires.
        if self.manner == Form2200SManner::ActualRemoval {
            put("txtSched1TIN1", tin1);
            put("txtSched1TIN2", tin2);
            put("txtSched1TIN3", tin3);
            put("txtSched1BranchCode", branch);
            put("txtSched1TaxpayerName", name);
        }
        for (index, row) in self.beverages.iter().enumerate().take(10) {
            put(&format!("txtSalesValue{index}"), money(row.sales_value));
            put(&format!("txtVolumeRemovals{index}"), money(row.volume));
            put(&format!("txtBasicTaxDue{index}"), sum(row.basic_tax_due));
        }
        if let Some(other) = &self.others {
            put("txtsched1Atc0", other.atc.clone());
            put("txtsched1Desc0", other.description.trim().to_string());
            put("txtsched1TaxBracket0", other.tax_bracket.trim().to_string());
            put("txtsched1AppTaxRate0", Self::others_rate_text(other.rate));
            put("txtsched1SalesValue0", money(other.sales_value));
            put("txtsched1VolumeRemovals0", money(other.volume));
            put("txtsched1BasicTaxDue0", sum(other.basic_tax_due));
        }
        put("totalTaxDue", sum(self.schedule_total));

        let [region, province, city] = self.place_of_production.values();
        fields.insert("frm2200SoptRegion".into(), region);
        fields.insert("frm2200SoptProvince".into(), province);
        fields.insert("frm2200SoptCity".into(), city);
        let [region, province, city] = self.place_of_removal.values();
        fields.insert("frm2200SoptRegion1".into(), region);
        fields.insert("frm2200SoptProvince1".into(), province);
        fields.insert("frm2200SoptCity1".into(), city);
        fields.insert("txtEmail".to_string(), self.email.trim().to_string());
        fields
    }

    /// The exact official submit plaintext.
    pub fn to_bir_xml_payload(&self) -> Result<String, Vec<(String, String)>> {
        self.official_payload()
    }

    fn date(&self) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::from_ymd_opt(
            i32::from(self.year),
            u32::from(self.month),
            u32::from(self.day),
        )
    }

    /// `HHMMSS` of the moment the return was queued (Philippine time): the
    /// official file name carries the save time after the date.
    fn file_time(&self) -> String {
        let stamp = self
            .lifecycle
            .queue_authorization
            .as_ref()
            .map(|auth| auth.authorized_at.clone())
            .unwrap_or_else(|| self.lifecycle.updated_at.clone());
        let manila = chrono::FixedOffset::east_opt(8 * 3600).expect("UTC+8");
        chrono::DateTime::parse_from_rfc3339(&stamp)
            .map(|time| time.with_timezone(&manila).format("%H%M%S").to_string())
            .unwrap_or_else(|_| "000000".to_string())
    }
}

impl FormValidator for Form2200SDraft {
    /// `validateForm` in order with its alert texts, the Schedule 1 checks
    /// (`isValidDataOnSched1`, `validateSchedule1`) and the typing limits.
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        let mut err = |field: &str, message: &str| {
            errors.push((field.to_string(), message.to_string()));
        };
        let leap = chrono::NaiveDate::from_ymd_opt(i32::from(self.year), 2, 29).is_some();
        let today = chrono::Local::now().date_naive();

        if !(1..=12).contains(&self.month) {
            err("month", "Invalid date entry on Item no.1.");
        } else if !leap && self.month == 2 && self.day == 29 {
            err("day", "Filing year is not a leap year.");
        } else if self.month == 2 && self.day > 29 {
            err("day", "Invalid date entry on item 1.");
        } else if self.day == 0 {
            err("day", "Please enter valid day on item 1.");
        } else if self.year == 0 {
            err("year", "Please enter valid year on item 1.");
        } else if self.year < 2013 {
            err(
                "year",
                "Invalid date entry on Item no.1. Entry should not be lower than 1900.",
            );
        } else if self.date().is_none() {
            err("day", "Invalid date entry on Item no.1.");
        } else if self.date().is_some_and(|date| date > today) {
            err("day", "Date on Item 1 cannot be a future date.");
        }
        if let Some(message) = tin_error(&self.tin, "Please enter a valid TIN number on Item 4.") {
            err("tin", &message);
        }
        if !crate::validation::rdo_code_is_official_option(self.rdo_code.trim()) {
            err("rdo_code", "Please enter a valid RDO Code on Item 5.");
        }
        if self.taxpayer_name.trim().is_empty() {
            err(
                "taxpayer_name",
                "Please enter a valid Taxpayer Name on Item 6.",
            );
        }
        if self.registered_address.trim().is_empty() {
            err(
                "registered_address",
                "Please enter Taxpayer's Registered Address on Item 7.",
            );
        }
        let zip = self.zip_code.trim();
        if zip.len() != 4 || !digits_only(zip) {
            err("zip_code", "Please enter Taxpayer's Zip Code on Item 7A.");
        }
        let phone = self.contact_number.trim();
        if phone.is_empty() || phone.len() > 20 || !digits_only(phone) {
            err("contact_number", "Please enter Contact Number on Item 8.");
        }
        if self.line_of_business.trim().is_empty() {
            err(
                "line_of_business",
                "Please enter a valid Main Line of Business on Item 9.",
            );
        }
        let psic = self.psic.trim();
        if !(4..=6).contains(&psic.len()) || !digits_only(psic) {
            err("psic", "Please enter a valid PSIC on Item 10.");
        }
        let email = self.email.trim();
        if email.is_empty() {
            err("email", "Please enter Email Address on Item 11.");
        } else if !email_matches_official_pattern(email) {
            err("email", "Please enter a valid Email Address on Item 11.");
        }
        let place_ok = |place: &ExcisePlace| {
            is_official_place(PLACE_FORM, &place.region, &place.province, &place.city)
        };
        if !place_ok(&self.place_of_production) {
            err(
                "place_of_production",
                "Please enter values on item no. 12. All entries must not be empty.",
            );
        }
        if !place_ok(&self.place_of_removal) {
            err(
                "place_of_removal",
                "Please enter values on item no. 13. All entries must not be empty.",
            );
        }
        if self.tax_relief == Form2200STaxRelief::YesUnspecified {
            err(
                "tax_relief",
                "Please select If yes, please specify on Item 14A.",
            );
        }
        if self.manner == Form2200SManner::Unanswered {
            err(
                "manner",
                "Please select an option on Part II Manner of Payment",
            );
        }
        if self.other_scheme && self.other_scheme_text.trim().is_empty() {
            err("other_scheme_text", "Please enter a value on Item 17.");
        }
        if self.manner == Form2200SManner::Prepayment && self.tax_payment <= 0.0 {
            err(
                "tax_payment",
                "Please enter Tax Payment / Deposit on Item 25A.",
            );
        }
        if self.balance_carried_forward > 0.0 {
            err(
                "tax_payment",
                "Payment not sufficient to cover amount payable. Please check amount in Item 25C.",
            );
        }
        if self.manner == Form2200SManner::ActualRemoval {
            let others_volume = self.others.as_ref().map(|o| o.volume).unwrap_or(0.0);
            if !self.beverages.iter().any(|row| row.volume > 0.0) && others_volume <= 0.0 {
                err("beverages", "Please fill up Schedule 1.");
            }
        }
        if let Some(other) = &self.others {
            let quoted = "\"Others\"";
            let atc = other.atc.trim().to_uppercase();
            if atc.is_empty() || atc == "XB" {
                err(
                    "others.atc",
                    &format!("Please enter a valid ATC in {quoted} at Row 1."),
                );
            } else if !atc.starts_with("XB") {
                err(
                    "others.atc",
                    &format!("The supplied ATC code in {quoted} at Row 1 should start with 'XB'."),
                );
            } else if FORM_2200S_BEVERAGES.iter().any(|b| b.atc == atc) {
                err(
                    "others.atc",
                    &format!(
                        "Please enter a valid ATC code in {quoted} at Row 1.\n\nAdded ATC code should be differ from table."
                    ),
                );
            } else if atc.len() != 5 || !digits_only(&atc[2..]) {
                err("others.atc", "Invalid ATC.");
            }
            if other.description.trim().is_empty() {
                err(
                    "others.description",
                    &format!("Please enter a Description in {quoted} at Row 1."),
                );
            } else if other.tax_bracket.trim().is_empty() {
                err(
                    "others.tax_bracket",
                    &format!("Please enter a Tax Bracket in {quoted} at Row 1."),
                );
            } else if other.rate <= 0.0 || other.rate > 999.99 {
                err(
                    "others.rate",
                    &format!("Please enter a valid Applicable rate in {quoted} at Row 1."),
                );
            } else if other.volume <= 0.0 {
                err(
                    "others.volume",
                    &format!(
                        "Please enter a value on Volume Removals column in {quoted} at Row 1."
                    ),
                );
            }
            let allowed = |c: char| c.is_ascii_alphanumeric() || " .,#@'()_-".contains(c);
            if !other.description.chars().all(allowed) || !other.tax_bracket.chars().all(allowed) {
                err(
                    "others.description",
                    "Schedule 1 Others: use letters, digits, spaces and . , # @ ' ( ) _ - only.",
                );
            }
        }

        // ── Implicit rules and typing limits ──
        if self.manner == Form2200SManner::Prepayment && self.date() != Some(today) {
            err(
                "day",
                "For a Prepayment/Advance Deposit, Item 1 is today's filing date.",
            );
        }
        if self.beverages.len() > 10 {
            err("beverages", "Schedule 1 has ten beverage rows.");
        }
        let amounts = self
            .beverages
            .iter()
            .flat_map(|row| [row.sales_value, row.volume])
            .chain(
                self.others
                    .iter()
                    .flat_map(|other| [other.sales_value, other.volume]),
            )
            .chain([
                self.balance_carried_over,
                self.creditable_excise_tax,
                self.previous_payment,
                self.surcharge,
                self.interest,
                self.compromise,
                self.tax_payment,
            ]);
        for value in amounts {
            if value < 0.0 || !has_cent_precision(value) || value >= 1e12 {
                err(
                    "amounts",
                    "Enter non-negative amounts in pesos and centavos below one trillion.",
                );
                break;
            }
        }
        if !self.is_amended && self.previous_payment != 0.0 {
            err(
                "previous_payment",
                "Item 21 applies only to an amended return.",
            );
        }
        if self.other_scheme_text.chars().count() > 50 {
            err("other_scheme_text", "Item 17 takes up to 50 characters.");
        }
        let mut expected = self.clone();
        expected.recompute();
        if expected != *self {
            err(
                "balance_carried_forward",
                "Totals are out of date. Recompute the return.",
            );
        }
        errors
    }
}

/// `validateEmail`'s pattern:
/// `\b[a-zA-Z0-9._%+-]+@(?:[a-zA-Z0-9-]+\.)+[a-zA-Z]{2,4}\b`, which the
/// official check only needs to find somewhere in the address.
fn email_matches_official_pattern(email: &str) -> bool {
    let Some((local, domain)) = email.rsplit_once('@') else {
        return false;
    };
    let local_ok = local
        .chars()
        .last()
        .is_some_and(|c| c.is_ascii_alphanumeric() || "._%+-".contains(c));
    let labels: Vec<&str> = domain.split('.').collect();
    let tld = labels.last().copied().unwrap_or("");
    let tld_ok = tld.len() >= 2 && tld.chars().take(4).all(|c| c.is_ascii_alphabetic());
    local_ok
        && labels.len() >= 2
        && labels[..labels.len() - 1]
            .iter()
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && tld_ok
        && !email.contains(char::is_whitespace)
}

impl QueueableForm for Form2200SDraft {
    const FORM_CODE: &'static str = "2200S";
    /// Official `formType` and PROD SFTP folder (`ftpTargetFolder.PROD['2200S']`).
    const FORM_TYPE: &'static str = "2200S";
    const LAYOUT_ID: &'static str = FORM_2200S_FORM_ID;

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
        self.year
    }
    /// One return per Item 1 date: `MMDD` keys the draft in its year.
    fn filing_period(&self) -> FilingPeriod {
        FilingPeriod::OpenEnded(u32::from(self.month) * 100 + u32::from(self.day))
    }
    /// `txtMonth1 + txtDate + txtForYr` (`createXMLFileName` then appends
    /// the save time; see [`QueueableForm::submission_filename`]).
    fn period_code(&self) -> String {
        format!("{:02}{:02}{}", self.month, self.day, self.year)
    }
    fn parse_period_code(code: &str) -> Option<(u16, FilingPeriod)> {
        if !(code.len() == 8 || code.len() == 14) || !digits_only(code) {
            return None;
        }
        let month: u32 = code[..2].parse().ok()?;
        let day: u32 = code[2..4].parse().ok()?;
        let year: u16 = code[4..8].parse().ok()?;
        chrono::NaiveDate::from_ymd_opt(i32::from(year), month, day)?;
        Some((year, FilingPeriod::OpenEnded(month * 100 + day)))
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
    /// `TIN+branch-2200S-MMDDYYYYHHMMSS#email#.xml`: `createXMLFileName`
    /// appends `getHHMMSS()` to the Item 1 date.
    fn submission_filename(&self) -> String {
        format!(
            "{}-{}-{}{}#{}#.xml",
            self.tin.replace('-', ""),
            Self::FORM_TYPE,
            self.period_code(),
            self.file_time(),
            self.submission_email()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place() -> ExcisePlace {
        ExcisePlace {
            region: "130000000".into(),
            province: "137400000".into(),
            city: "137404000".into(),
        }
    }

    pub(crate) fn sample() -> Form2200SDraft {
        let mut draft = Form2200SDraft {
            id: None,
            month: 6,
            day: 30,
            year: 2025,
            is_amended: false,
            tin: "12345678800000".into(),
            rdo_code: "039".into(),
            taxpayer_name: "Sample Beverages Inc".into(),
            registered_address: "123 Sample St Quezon City".into(),
            zip_code: "1100".into(),
            contact_number: "0281234567".into(),
            line_of_business: "Beverages".into(),
            psic: "1104".into(),
            email: "sample.taxpayer@example.com".into(),
            place_of_production: place(),
            place_of_removal: place(),
            tax_relief: Form2200STaxRelief::No,
            manner: Form2200SManner::ActualRemoval,
            other_scheme: false,
            other_scheme_text: String::new(),
            beverages: Vec::new(),
            others: None,
            schedule_total: 0.0,
            excise_tax_due: 0.0,
            balance_carried_over: 0.0,
            creditable_excise_tax: 0.0,
            total_credits: 0.0,
            net_tax_due: 0.0,
            previous_payment: 0.0,
            tax_still_due: 0.0,
            surcharge: 0.0,
            interest: 0.0,
            compromise: 0.0,
            total_penalties: 0.0,
            amount_payable: 0.0,
            tax_payment: 0.0,
            penalties_paid: 0.0,
            total_payment: 0.0,
            balance_carried_forward: 0.0,
            lifecycle: SubmissionLifecycle::default(),
        };
        draft.recompute();
        draft.beverages[2].sales_value = 500_000.0;
        draft.beverages[2].volume = 10_000.509;
        draft.beverages[8].volume = 100.0;
        draft.tax_payment = 61_203.0;
        draft.recompute();
        draft
    }

    fn messages(draft: &Form2200SDraft) -> Vec<String> {
        <Form2200SDraft as FormValidator>::validate(draft)
            .into_iter()
            .map(|(_, message)| message)
            .collect()
    }

    #[test]
    fn truncating_helpers_match_string_util2014() {
        assert_eq!(round_down_input_text(1234.567), "1,234.56");
        assert_eq!(round_down_input_text(1000.0), "1,000.00");
        assert_eq!(round_down_input_text(0.5), "0.50");
        assert_eq!(round_down_computation_text(6.0 * 10_000.5), "60,003.00");
        assert_eq!(round_down_computation_text(-0.5), "-0.50");
        assert_eq!(round_down_computation_text(0.1 + 0.2), "0.30");
        assert_eq!(round_down_computation_text(1.005 * 3.0), "3.01");
        assert_eq!(js_to_precision14(0.1 + 0.2), 0.3);
    }

    #[test]
    fn schedule_one_and_part_three_follow_the_official_chain() {
        let draft = sample();
        assert_eq!(draft.beverages[2].volume, 10_000.50);
        assert_eq!(draft.beverages[2].basic_tax_due, 60_003.0);
        assert_eq!(draft.beverages[8].basic_tax_due, 1_200.0);
        assert_eq!(draft.schedule_total, 61_203.0);
        assert_eq!(draft.excise_tax_due, 61_203.0);
        assert_eq!(draft.amount_payable, 61_203.0);
        assert_eq!(draft.balance_carried_forward, 0.0);
        assert!(messages(&draft).is_empty(), "{:?}", messages(&draft));
        let fields = draft.to_bir_field_map();
        assert_eq!(fields["frm2200S:txtVolumeRemovals2"], "10,000.50");
        assert_eq!(fields["frm2200S:txtTax16"], "61,203.00");
        assert_eq!(fields["frm2200S:taxpayerName"], "SAMPLE BEVERAGES INC");
        assert_eq!(
            fields["frm2200S:txtSched1TaxpayerName"],
            "SAMPLE BEVERAGES INC"
        );
        assert_eq!(fields["frm2200SoptCity"], "137404000");
    }

    #[test]
    fn prepayment_has_no_schedule_and_pays_penalties() {
        let mut draft = sample();
        draft.manner = Form2200SManner::Prepayment;
        draft.surcharge = 100.0;
        draft.tax_payment = 0.0;
        draft.recompute();
        assert!(draft.beverages.is_empty());
        assert_eq!(draft.excise_tax_due, 0.0);
        assert_eq!(draft.total_penalties, 100.0);
        assert_eq!(draft.penalties_paid, 100.0);
        assert_eq!(draft.balance_carried_forward, 0.0);
        assert!(
            messages(&draft)
                .contains(&"Please enter Tax Payment / Deposit on Item 25A.".to_string())
        );
        assert_eq!(draft.to_bir_field_map()["frm2200S:PayPenalties"], "true");
    }

    #[test]
    fn filename_carries_the_queue_time() {
        let mut draft = sample();
        draft
            .queue(crate::filing_queue::QueueAuthSource::Gui)
            .unwrap();
        let name = draft.submission_filename();
        assert!(name.starts_with("12345678800000-2200S-06302025"), "{name}");
        assert_eq!(
            name.len(),
            "12345678800000-2200S-06302025HHMMSS#sample.taxpayer@example.com#.xml".len()
        );
        assert!(draft.revalidate_queued_before_submission().is_ok());
    }

    #[test]
    fn period_codes_round_trip() {
        let draft = sample();
        assert_eq!(draft.period_code(), "06302025");
        assert_eq!(
            Form2200SDraft::parse_period_code("06302025"),
            Some((2025, FilingPeriod::OpenEnded(630)))
        );
        assert_eq!(
            Form2200SDraft::parse_period_code("06302025101112"),
            Some((2025, FilingPeriod::OpenEnded(630)))
        );
        assert_eq!(Form2200SDraft::parse_period_code("02302025"), None);
    }

    #[test]
    fn official_negative_cases_are_rejected_with_their_alerts() {
        let check = |mutate: &dyn Fn(&mut Form2200SDraft), expected: &str| {
            let mut draft = sample();
            mutate(&mut draft);
            draft.recompute();
            let found = messages(&draft);
            assert!(
                found.iter().any(|m| m == expected),
                "{expected:?} not in {found:?}"
            );
        };
        check(
            &|d| {
                d.month = 2;
                d.day = 29;
            },
            "Filing year is not a leap year.",
        );
        check(&|d| d.day = 0, "Please enter valid day on item 1.");
        check(
            &|d| d.year = 2012,
            "Invalid date entry on Item no.1. Entry should not be lower than 1900.",
        );
        check(
            &|d| {
                d.month = 4;
                d.day = 31;
            },
            "Invalid date entry on Item no.1.",
        );
        check(
            &|d| d.year = 2999,
            "Date on Item 1 cannot be a future date.",
        );
        check(
            &|d| d.tin = "1".into(),
            "Please enter a valid TIN number on Item 4.",
        );
        check(
            &|d| d.rdo_code = "000".into(),
            "Please enter a valid RDO Code on Item 5.",
        );
        check(
            &|d| d.taxpayer_name.clear(),
            "Please enter a valid Taxpayer Name on Item 6.",
        );
        check(
            &|d| d.registered_address.clear(),
            "Please enter Taxpayer's Registered Address on Item 7.",
        );
        check(
            &|d| d.zip_code = "110".into(),
            "Please enter Taxpayer's Zip Code on Item 7A.",
        );
        check(
            &|d| d.contact_number.clear(),
            "Please enter Contact Number on Item 8.",
        );
        check(
            &|d| d.line_of_business.clear(),
            "Please enter a valid Main Line of Business on Item 9.",
        );
        check(
            &|d| d.psic = "12".into(),
            "Please enter a valid PSIC on Item 10.",
        );
        check(
            &|d| d.email.clear(),
            "Please enter Email Address on Item 11.",
        );
        check(
            &|d| d.place_of_production.city.clear(),
            "Please enter values on item no. 12. All entries must not be empty.",
        );
        check(
            &|d| d.place_of_removal = ExcisePlace::default(),
            "Please enter values on item no. 13. All entries must not be empty.",
        );
        check(
            &|d| d.tax_relief = Form2200STaxRelief::YesUnspecified,
            "Please select If yes, please specify on Item 14A.",
        );
        check(
            &|d| d.manner = Form2200SManner::Unanswered,
            "Please select an option on Part II Manner of Payment",
        );
        check(
            &|d| d.tax_payment = 0.0,
            "Payment not sufficient to cover amount payable. Please check amount in Item 25C.",
        );
        check(
            &|d| {
                for row in &mut d.beverages {
                    row.volume = 0.0;
                }
                d.tax_payment = 0.0;
            },
            "Please fill up Schedule 1.",
        );
        check(
            &|d| {
                d.others = Some(Form2200SOtherRow {
                    atc: "XB010".into(),
                    ..Default::default()
                })
            },
            "Please enter a valid ATC code in \"Others\" at Row 1.\n\nAdded ATC code should be differ from table.",
        );
        check(
            &|d| {
                d.others = Some(Form2200SOtherRow {
                    atc: "XB200".into(),
                    description: "Other drink".into(),
                    tax_bracket: "Per liter".into(),
                    rate: 0.0,
                    ..Default::default()
                })
            },
            "Please enter a valid Applicable rate in \"Others\" at Row 1.",
        );
    }
}
