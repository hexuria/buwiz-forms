//! BIR Form 2550M (February 2007 ENCS) — Monthly Value-Added Tax Declaration.
//!
//! Ported from the official `BIR-Form2550M.hta`: the compute chain
//! (`compute13B` … `compute24`, `getRequiredWithheld`,
//! `totalAmountandOutputTax`, `computeInputTaxSched4`,
//! `computeInputTaxSched5`), `validate` with its exact alert texts, and
//! `saveXMLsubmit` through [`crate::official_xml`].
//!
//! Schedule 1 (vatable sales per ATC) writes one row of controls per ATC the
//! filer ticks, inside the modal ahead of `frm2550M:txtmodaltxtTotal12A`;
//! [`Form2550MDraft::official_payload`] splices those rows into the fixed
//! layout exactly where the page's own submit loop writes them.
//!
//! Schedules 2, 3, 6, 7 and 8 (capital goods, creditable VAT withheld,
//! advance payments, VAT withheld on sales to government) also add rows of
//! their own; this model does not offer them, so Items 18A–18D, 20A and
//! 23A–23C stay `0.00`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::queueable::{QueueableForm, SubmissionLifecycle};
use super::{FilingPeriod, FormValidator};
use crate::official_xml::{official_amount, parse_official_amount};
use crate::profile::TaxpayerProfile;

/// Rule-package id of the official layout.
pub const FORM_2550M_FORM_ID: &str = "2550m-v2007";
/// VAT rate the page applies (`* 0.12`).
pub const FORM_2550M_VAT_RATE: f64 = 0.12;
/// The modal control Schedule 1 rows are written ahead of.
const SCHEDULE_1_ANCHOR: &str = "<div>frm2550M:txtmodaltxtTotal12A=";

/// The ATC popup (`ATCList`): every `xml/atcCodes.xml` entry tagged 2550M,
/// in file order. Checkbox `AtcCode<n>` is entry `n` (1-based).
pub const FORM_2550M_ATCS: &[(&str, &str)] = &[
    ("VB010", "VAT-ON BUSINESS SERVICES-IN GENERAL"),
    ("VB100", "VAT-ON BUS SERV-HOTEL, MOTELS, ETC"),
    ("VB101", "VAT -ON BUSINESS SERVICES -RESTAURANTS, CATERERS"),
    (
        "VB102",
        "VAT -ON BUS. SERVICES -DEALERS ON SEC/LENDING INVESTORS",
    ),
    ("VB105", "VAT -COMMON CARRIERS -LAND BASED (ROAD FREIGHT)"),
    ("VB106", "VAT -COMMON CARRIERS -DOMESTIC OCEAN-GOING VESSEL"),
    (
        "VB107",
        "VAT -COMMON CARRIERS -INTER-ISLAND SHIPPING VESSEL",
    ),
    ("VB108", "VAT -COMMON CARRIERS -AIRCRAFT"),
    ("VB109", "VAT -FRANCHISE HOLDERS -TELEPHONE"),
    ("VB111", "VAT -FRANCHISE HOLDERS -RADIO/TELEPHONE BROAD."),
    ("VB112", "VAT -FRANCHISE HOLDERS -OTHERS"),
    ("VB113", "VAT -NON-LIFE INSURANCE COMPANIES"),
    ("VC010", "VAT -ON CONSTRUCTION"),
    (
        "VD010",
        "COMPULSORY SOCIAL SECURITY PUBLIC ADMIN AND DEFENSE",
    ),
    (
        "VH010",
        "VAT -ON COMMUNITY, PERSONAL AND HOUSEHOLD SERVICES",
    ),
    ("VI010", "VAT -ON IMPORTATION OF GOODS"),
    ("VM010", "VAT -MANUFACTURING IN GEN."),
    (
        "VM020",
        "VAT ON MANUFACTURING - FOOD, PRODUCTS AND BEVERAGES",
    ),
    ("VM030", "VAT ON MANUFACTURING - CEMENT"),
    ("VM040", "VAT ON MANUFACTURING - TOBACCO"),
    ("VM050", "VAT ON MANUFACTURING - FLOUR"),
    ("VM100", "VAT -MANUFACTURING -PESTICIDES"),
    ("VM110", "VAT ON MANUFACTURING - ALCOHOL"),
    ("VM120", "VAT ON MANUFACTURING - PETROLEUM"),
    ("VM130", "VAT ON MANUFACTURING - AUTOMOBILES"),
    ("VM140", "VAT ON MANUFACTURING - NON ESSENTIALS"),
    ("VM150", "VAT ON MANUFACTURING - PHARMACUETICALS"),
    ("VM160", "VAT ON MANUFACTURING - SUGAR"),
    ("VP100", "VAT - SALE OF REAL PROPERTY"),
    ("VP101", "VAT - LEASE OF REAL PROPERTY"),
    ("VP102", "VAT - SALE/LEASE OF INTANGIBLE PROPERTY"),
    ("VQ010", "VAT ON MINING AND QUARYING"),
    ("VS010", "VAT -ON STORAGE AND WAREHOUSING"),
    ("VT010", "VAT -ON WHOLESALE AND RETAIL TRADE"),
    (
        "VS062",
        "ON SERVICES RENDERED BY PERSONS ENGAGED IN THE PRACTICE OF PROFESSION OR CALLING AND PROFESSIONAL SERVICES RENDERED BY GENERAL PROFESSIONAL PARTNERSHIPS",
    ),
    (
        "VS210",
        "ON SERVICES RENDERED BY STOCK, REAL ESTATE, COMMERCIAL, CUSTOMS, INSURANCE AND IMMIGRATION BROKERS",
    ),
];

/// Position of an ATC in the official popup.
pub fn form_2550m_atc_index(code: &str) -> Option<usize> {
    FORM_2550M_ATCS.iter().position(|(atc, _)| *atc == code)
}

/// Item 11 (`OptSpecialTax` + `lstSpecialTax`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form2550MTaxRelief {
    #[default]
    No,
    /// "Yes" with nothing picked from the list yet.
    YesUnspecified,
    SpecialRate,
    InternationalTaxTreaty,
}

impl Form2550MTaxRelief {
    pub fn is_yes(self) -> bool {
        !matches!(self, Self::No)
    }

    fn list_value(self) -> &'static str {
        match self {
            Self::No | Self::YesUnspecified => "0",
            Self::SpecialRate => "1",
            Self::InternationalTaxTreaty => "2",
        }
    }
}

/// Item 18 purchases whose input tax the page fills at 12%
/// (`compute18TransactionbyRow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form2550MPurchase {
    /// 18E/F — domestic purchases of goods other than capital goods.
    DomesticGoods,
    /// 18G/H — importation of goods other than capital goods.
    ImportedGoods,
    /// 18I/J — domestic purchase of services.
    DomesticServices,
    /// 18K/L — services rendered by non-residents.
    NonResidentServices,
    /// 18N/O — others.
    Others,
}

/// Schedule 1 — one ATC's vatable sales to private buyers.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Form2550MSalesRow {
    pub atc_code: String,
    pub amount: f64,
    /// `getRequiredWithheld`: 12% of the amount.
    pub output_tax: f64,
}

/// Schedule 4 / 5 inputs: input tax directly attributable, and the input tax
/// not directly attributable that the page allocates by sales.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Form2550MAllocation {
    pub direct_input_tax: f64,
    pub not_direct_input_tax: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Form2550MDraft {
    #[serde(default)]
    pub id: Option<i64>,

    // Items 1–3
    pub month: u8,
    pub taxable_year: u16,
    pub is_amended: bool,
    #[serde(default)]
    pub number_of_attached_sheets: u16,

    // Part I
    /// 14 digits: 9-digit TIN then the 5-digit branch code.
    pub tin: String,
    pub rdo_code: String,
    pub line_of_business: String,
    pub taxpayer_name: String,
    pub contact_number: String,
    pub registered_address: String,
    pub zip_code: String,
    /// `txtEmail`, never upper-cased.
    pub email: String,
    #[serde(default)]
    pub tax_relief: Form2550MTaxRelief,

    // Part II — sales
    /// Schedule 1 rows in popup order.
    #[serde(default)]
    pub sales_schedule: Vec<Form2550MSalesRow>,
    /// 12A / 12B.
    #[serde(default)]
    pub vatable_sales_private: f64,
    #[serde(default)]
    pub output_tax_private: f64,
    /// 13A / 13B (13B starts at 12% of 13A and stays editable).
    #[serde(default)]
    pub sales_to_government: f64,
    #[serde(default)]
    pub output_tax_government: f64,
    /// 14, 15.
    #[serde(default)]
    pub zero_rated_sales: f64,
    #[serde(default)]
    pub exempt_sales: f64,
    /// 16A / 16B.
    #[serde(default)]
    pub total_sales: f64,
    #[serde(default)]
    pub total_output_tax: f64,

    // Item 17
    #[serde(default)]
    pub input_tax_carried_over: f64,
    #[serde(default)]
    pub input_tax_deferred_capital_goods: f64,
    #[serde(default)]
    pub transitional_input_tax: f64,
    #[serde(default)]
    pub presumptive_input_tax: f64,
    #[serde(default)]
    pub other_input_tax: f64,
    #[serde(default)]
    pub total_input_tax_17f: f64,

    // Item 18 (purchase, input tax)
    #[serde(default)]
    pub domestic_goods: f64,
    #[serde(default)]
    pub domestic_goods_input_tax: f64,
    #[serde(default)]
    pub imported_goods: f64,
    #[serde(default)]
    pub imported_goods_input_tax: f64,
    #[serde(default)]
    pub domestic_services: f64,
    #[serde(default)]
    pub domestic_services_input_tax: f64,
    #[serde(default)]
    pub non_resident_services: f64,
    #[serde(default)]
    pub non_resident_services_input_tax: f64,
    /// 18M.
    #[serde(default)]
    pub purchases_not_qualified: f64,
    #[serde(default)]
    pub other_purchases: f64,
    #[serde(default)]
    pub other_purchases_input_tax: f64,
    /// 18P, 19.
    #[serde(default)]
    pub total_current_purchases: f64,
    #[serde(default)]
    pub total_available_input_tax: f64,

    // Item 20
    /// Schedule 4 (20B), opened only with sales to government.
    #[serde(default)]
    pub schedule_4: Option<Form2550MAllocation>,
    /// Schedule 4 "Less: standard input tax" (`txtlessStdTaxSched4`): 5.00
    /// on a fresh page, then 7% of 13A whenever 13A is entered.
    #[serde(default = "default_standard_input_tax")]
    pub standard_input_tax_government: f64,
    #[serde(default)]
    pub schedule_4_attributed: f64,
    #[serde(default)]
    pub schedule_4_total: f64,
    /// Schedule 5 (20C), opened only with exempt sales.
    #[serde(default)]
    pub schedule_5: Option<Form2550MAllocation>,
    #[serde(default)]
    pub schedule_5_attributed: f64,
    /// 20B, 20C, 20D, 20E, 20F.
    #[serde(default)]
    pub input_tax_sales_to_government: f64,
    #[serde(default)]
    pub input_tax_exempt_sales: f64,
    #[serde(default)]
    pub vat_refund_claimed: f64,
    #[serde(default)]
    pub other_deductions: f64,
    #[serde(default)]
    pub total_deductions: f64,
    /// 21, 22.
    #[serde(default)]
    pub total_allowable_input_tax: f64,
    #[serde(default)]
    pub net_vat_payable: f64,

    // Item 23
    /// 23D, amended returns only.
    #[serde(default)]
    pub vat_paid_previous: f64,
    /// 23E, 23F, 23G.
    #[serde(default)]
    pub advance_payments: f64,
    #[serde(default)]
    pub other_credits: f64,
    #[serde(default)]
    pub total_credits: f64,
    /// 24.
    #[serde(default)]
    pub tax_still_payable: f64,
    /// 25A–25D, 26.
    #[serde(default)]
    pub surcharge: f64,
    #[serde(default)]
    pub interest: f64,
    #[serde(default)]
    pub compromise: f64,
    #[serde(default)]
    pub total_penalties: f64,
    #[serde(default)]
    pub total_amount_payable: f64,

    #[serde(flatten)]
    pub lifecycle: SubmissionLifecycle,
}

fn default_standard_input_tax() -> f64 {
    5.0
}

/// `round(this,2)` / `formatCurrency`: the value an amount field holds.
fn cents(value: f64) -> f64 {
    if !value.is_finite() {
        // formatCurrency(NaN) writes 0.00 (Schedule 4/5 with no sales).
        return 0.0;
    }
    parse_official_amount(&official_amount(value)).unwrap_or(0.0)
}

fn has_cent_precision(value: f64) -> bool {
    value.is_finite() && ((value * 100.0) - (value * 100.0).round()).abs() < 1e-7
}

fn digits_only(value: &str) -> bool {
    value.bytes().all(|b| b.is_ascii_digit())
}

/// `capital()`: every text control except `txtEmail` is upper-cased.
fn caps(value: &str) -> String {
    value.trim().to_uppercase()
}

/// `(TIN1, TIN2, TIN3, branch)` from a stored TIN with or without dashes.
fn split_tin(tin: &str) -> (String, String, String, String) {
    let digits: String = tin.chars().filter(char::is_ascii_digit).collect();
    let part = |range: std::ops::Range<usize>| digits.get(range).unwrap_or("").to_string();
    let branch = digits.get(9..).unwrap_or("");
    (part(0..3), part(3..6), part(6..9), format!("{branch:0>5}"))
}

impl Form2550MDraft {
    pub const FORM_CODE: &'static str = "2550M";

    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, month: u8) -> Self {
        let mut draft = Self {
            id: None,
            month,
            taxable_year: year,
            is_amended: false,
            number_of_attached_sheets: 0,
            tin: profile.tin.full(),
            rdo_code: profile.rdo_code.clone(),
            line_of_business: profile.line_of_business.clone(),
            taxpayer_name: profile.full_name.clone(),
            contact_number: profile.phone.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            email: profile.email.clone(),
            tax_relief: Form2550MTaxRelief::No,
            sales_schedule: Vec::new(),
            vatable_sales_private: 0.0,
            output_tax_private: 0.0,
            sales_to_government: 0.0,
            output_tax_government: 0.0,
            zero_rated_sales: 0.0,
            exempt_sales: 0.0,
            total_sales: 0.0,
            total_output_tax: 0.0,
            input_tax_carried_over: 0.0,
            input_tax_deferred_capital_goods: 0.0,
            transitional_input_tax: 0.0,
            presumptive_input_tax: 0.0,
            other_input_tax: 0.0,
            total_input_tax_17f: 0.0,
            domestic_goods: 0.0,
            domestic_goods_input_tax: 0.0,
            imported_goods: 0.0,
            imported_goods_input_tax: 0.0,
            domestic_services: 0.0,
            domestic_services_input_tax: 0.0,
            non_resident_services: 0.0,
            non_resident_services_input_tax: 0.0,
            purchases_not_qualified: 0.0,
            other_purchases: 0.0,
            other_purchases_input_tax: 0.0,
            total_current_purchases: 0.0,
            total_available_input_tax: 0.0,
            schedule_4: None,
            standard_input_tax_government: default_standard_input_tax(),
            schedule_4_attributed: 0.0,
            schedule_4_total: 0.0,
            schedule_5: None,
            schedule_5_attributed: 0.0,
            input_tax_sales_to_government: 0.0,
            input_tax_exempt_sales: 0.0,
            vat_refund_claimed: 0.0,
            other_deductions: 0.0,
            total_deductions: 0.0,
            total_allowable_input_tax: 0.0,
            net_vat_payable: 0.0,
            vat_paid_previous: 0.0,
            advance_payments: 0.0,
            other_credits: 0.0,
            total_credits: 0.0,
            tax_still_payable: 0.0,
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

    /// Item 13A on blur: `round`, `compute13B` (12%) and `changedTxtTax13A`
    /// (Schedule 4 standard input tax, 7%).
    pub fn set_sales_to_government(&mut self, amount: f64) {
        self.sales_to_government = cents(amount);
        self.output_tax_government = cents(self.sales_to_government * FORM_2550M_VAT_RATE);
        self.standard_input_tax_government = cents(self.sales_to_government * 7.0 / 100.0);
        self.recompute();
    }

    /// An Item 18 purchase on blur: `round` and `compute18TransactionbyRow`,
    /// which fills its input tax at 12%. The input tax stays editable.
    pub fn set_purchase(&mut self, purchase: Form2550MPurchase, amount: f64) {
        let amount = cents(amount);
        let tax = cents(amount * FORM_2550M_VAT_RATE);
        let (base, input) = self.purchase_fields(purchase);
        *base = amount;
        *input = tax;
        self.recompute();
    }

    fn purchase_fields(&mut self, purchase: Form2550MPurchase) -> (&mut f64, &mut f64) {
        match purchase {
            Form2550MPurchase::DomesticGoods => {
                (&mut self.domestic_goods, &mut self.domestic_goods_input_tax)
            }
            Form2550MPurchase::ImportedGoods => {
                (&mut self.imported_goods, &mut self.imported_goods_input_tax)
            }
            Form2550MPurchase::DomesticServices => (
                &mut self.domestic_services,
                &mut self.domestic_services_input_tax,
            ),
            Form2550MPurchase::NonResidentServices => (
                &mut self.non_resident_services,
                &mut self.non_resident_services_input_tax,
            ),
            Form2550MPurchase::Others => (
                &mut self.other_purchases,
                &mut self.other_purchases_input_tax,
            ),
        }
    }

    /// Tick an ATC in the Schedule 1 popup (`getATCCode`). Rows follow the
    /// popup order; an ATC appears once.
    pub fn add_sales_atc(&mut self, code: &str) -> Result<(), String> {
        if form_2550m_atc_index(code).is_none() {
            return Err(format!("{code} is not a 2550M ATC."));
        }
        if self.sales_schedule.iter().any(|row| row.atc_code == code) {
            return Err(format!("{code} is already in Schedule 1."));
        }
        self.sales_schedule.push(Form2550MSalesRow {
            atc_code: code.to_string(),
            ..Default::default()
        });
        self.recompute();
        Ok(())
    }

    /// Untick an ATC.
    pub fn remove_sales_atc(&mut self, code: &str) {
        self.sales_schedule.retain(|row| row.atc_code != code);
        self.recompute();
    }

    /// The official compute chain; every amount is the formatted value of
    /// the items it reads.
    pub fn recompute(&mut self) {
        // optAmendFunc clears 23D on an original return.
        if !self.is_amended {
            self.vat_paid_previous = 0.0;
        }

        // Schedule 1 (getRequiredWithheld, totalAmountandOutputTax) in popup order.
        self.sales_schedule
            .sort_by_key(|row| form_2550m_atc_index(&row.atc_code).unwrap_or(usize::MAX));
        let mut sales = 0.0;
        let mut output = 0.0;
        for row in &mut self.sales_schedule {
            row.amount = cents(row.amount);
            row.output_tax = cents(row.amount * FORM_2550M_VAT_RATE);
            sales += row.amount;
            output += row.output_tax;
        }
        self.vatable_sales_private = cents(sales);
        self.output_tax_private = cents(output);

        self.sales_to_government = cents(self.sales_to_government);
        self.output_tax_government = cents(self.output_tax_government);
        self.zero_rated_sales = cents(self.zero_rated_sales);
        self.exempt_sales = cents(self.exempt_sales);
        self.total_sales = cents(
            self.vatable_sales_private
                + self.sales_to_government
                + self.zero_rated_sales
                + self.exempt_sales,
        );
        self.total_output_tax = cents(self.output_tax_private + self.output_tax_government);

        for value in [
            &mut self.input_tax_carried_over,
            &mut self.input_tax_deferred_capital_goods,
            &mut self.transitional_input_tax,
            &mut self.presumptive_input_tax,
            &mut self.other_input_tax,
            &mut self.domestic_goods,
            &mut self.domestic_goods_input_tax,
            &mut self.imported_goods,
            &mut self.imported_goods_input_tax,
            &mut self.domestic_services,
            &mut self.domestic_services_input_tax,
            &mut self.non_resident_services,
            &mut self.non_resident_services_input_tax,
            &mut self.purchases_not_qualified,
            &mut self.other_purchases,
            &mut self.other_purchases_input_tax,
            &mut self.standard_input_tax_government,
            &mut self.vat_refund_claimed,
            &mut self.other_deductions,
            &mut self.vat_paid_previous,
            &mut self.advance_payments,
            &mut self.other_credits,
            &mut self.surcharge,
            &mut self.interest,
            &mut self.compromise,
        ] {
            *value = cents(*value);
        }
        self.total_input_tax_17f = cents(
            self.input_tax_carried_over
                + self.input_tax_deferred_capital_goods
                + self.transitional_input_tax
                + self.presumptive_input_tax
                + self.other_input_tax,
        );
        // compute18P (18A and 18C come from Schedules 2 and 3: 0.00 here).
        self.total_current_purchases = cents(
            0.0 + 0.0
                + self.domestic_goods
                + self.imported_goods
                + self.domestic_services
                + self.non_resident_services
                + self.purchases_not_qualified
                + self.other_purchases,
        );
        // compute19 (18B and 18D likewise).
        self.total_available_input_tax = cents(
            self.total_input_tax_17f
                + 0.0
                + 0.0
                + self.domestic_goods_input_tax
                + self.imported_goods_input_tax
                + self.domestic_services_input_tax
                + self.non_resident_services_input_tax
                + self.other_purchases_input_tax,
        );

        // computeInputTaxSched4 with Item 13A and 16A copied in on opening.
        match &mut self.schedule_4 {
            Some(schedule) => {
                schedule.direct_input_tax = cents(schedule.direct_input_tax);
                schedule.not_direct_input_tax = cents(schedule.not_direct_input_tax);
                self.schedule_4_attributed = cents(
                    self.sales_to_government / self.total_sales * schedule.not_direct_input_tax,
                );
                self.schedule_4_total =
                    cents(schedule.direct_input_tax + self.schedule_4_attributed);
                // getSched4Modal copies the Schedule 4 total into 20B.
                self.input_tax_sales_to_government =
                    cents(self.schedule_4_total - self.standard_input_tax_government);
            }
            None => {
                self.schedule_4_attributed = 0.0;
                self.schedule_4_total = 0.0;
                self.input_tax_sales_to_government = 0.0;
            }
        }
        // computeInputTaxSched5 with Item 15 and 16A copied in on opening.
        match &mut self.schedule_5 {
            Some(schedule) => {
                schedule.direct_input_tax = cents(schedule.direct_input_tax);
                schedule.not_direct_input_tax = cents(schedule.not_direct_input_tax);
                self.schedule_5_attributed =
                    cents(self.exempt_sales / self.total_sales * schedule.not_direct_input_tax);
                self.input_tax_exempt_sales =
                    cents(schedule.direct_input_tax + self.schedule_5_attributed);
            }
            None => {
                self.schedule_5_attributed = 0.0;
                self.input_tax_exempt_sales = 0.0;
            }
        }

        // compute20F (20A comes from Schedule 3: 0.00 here) … compute24.
        self.total_deductions = cents(
            0.0 + self.input_tax_sales_to_government
                + self.input_tax_exempt_sales
                + self.vat_refund_claimed
                + self.other_deductions,
        );
        self.total_allowable_input_tax =
            cents(self.total_available_input_tax - self.total_deductions);
        self.net_vat_payable = cents(self.total_output_tax - self.total_allowable_input_tax);
        // compute23G (23A–23C come from Schedules 6–8: 0.00 here).
        self.total_credits = cents(
            0.0 + 0.0 + 0.0 + self.vat_paid_previous + self.advance_payments + self.other_credits,
        );
        self.tax_still_payable = cents(self.net_vat_payable - self.total_credits);
        self.total_penalties = cents(self.surcharge + self.interest + self.compromise);
        self.total_amount_payable = cents(self.tax_still_payable + self.total_penalties);
    }

    /// The official field values `saveXMLsubmit` reads, keyed by element id,
    /// including the Schedule 1 row controls.
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = self.layout_fields();
        for (index, row) in self.sales_schedule.iter().enumerate() {
            let n = index + 1;
            fields.insert(format!("frm2550m:txtAtcCde{n}"), row.atc_code.clone());
            fields.insert(
                format!("frm2550m:txtAmountSales{n}"),
                official_amount(row.amount),
            );
            fields.insert(
                format!("frm2550m:txtOutputTax{n}"),
                official_amount(row.output_tax),
            );
        }
        fields
    }

    /// The controls of the fixed official layout.
    fn layout_fields(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let mut put = |key: &str, value: String| {
            fields.insert(key.to_string(), value);
        };
        let flag = |on: bool| on.to_string();
        let money = official_amount;

        put("frm2550m:RtnYear", format!("{:02}", self.month));
        put("frm2550m:txtYear", self.taxable_year.to_string());
        put("frm2550m:OptAmendedYN1", flag(self.is_amended));
        put("frm2550m:OptAmendedYN2", flag(!self.is_amended));
        put(
            "frm2550m:txtSheets",
            self.number_of_attached_sheets.to_string(),
        );
        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);
        put("frm2550m:txtTIN1", tin1);
        put("frm2550m:txtTIN2", tin2);
        put("frm2550m:txtTIN3", tin3);
        put("frm2550m:txtBranchCode", branch);
        put("frm2550m:txtRDOCode", self.rdo_code.trim().to_string());
        put("frm2550m:txtLineBus", caps(&self.line_of_business));
        put("frm2550m:txtTaxPayerName", caps(&self.taxpayer_name));
        put(
            "frm2550m:txtTelephoneNum",
            self.contact_number.trim().to_string(),
        );
        put("frm2550m:txtAddress", caps(&self.registered_address));
        put("frm2550m:txtZipCode", self.zip_code.trim().to_string());
        put("frm2550m:OptSpecialTax1", flag(self.tax_relief.is_yes()));
        put("frm2550m:OptSpecialTax2", flag(!self.tax_relief.is_yes()));
        put(
            "frm2550m:lstSpecialTax",
            self.tax_relief.list_value().to_string(),
        );

        let items: [(&str, f64); 46] = [
            ("12A", self.vatable_sales_private),
            ("12B", self.output_tax_private),
            ("13A", self.sales_to_government),
            ("13B", self.output_tax_government),
            ("14", self.zero_rated_sales),
            ("15", self.exempt_sales),
            ("16A", self.total_sales),
            ("16B", self.total_output_tax),
            ("17A", self.input_tax_carried_over),
            ("17B", self.input_tax_deferred_capital_goods),
            ("17C", self.transitional_input_tax),
            ("17D", self.presumptive_input_tax),
            ("17E", self.other_input_tax),
            ("17F", self.total_input_tax_17f),
            ("18A", 0.0),
            ("18B", 0.0),
            ("18C", 0.0),
            ("18D", 0.0),
            ("18E", self.domestic_goods),
            ("18F", self.domestic_goods_input_tax),
            ("18G", self.imported_goods),
            ("18H", self.imported_goods_input_tax),
            ("18I", self.domestic_services),
            ("18J", self.domestic_services_input_tax),
            ("18K", self.non_resident_services),
            ("18L", self.non_resident_services_input_tax),
            ("18M", self.purchases_not_qualified),
            ("18N", self.other_purchases),
            ("18O", self.other_purchases_input_tax),
            ("18P", self.total_current_purchases),
            ("19", self.total_available_input_tax),
            ("20A", 0.0),
            ("20B", self.input_tax_sales_to_government),
            ("20C", self.input_tax_exempt_sales),
            ("20D", self.vat_refund_claimed),
            ("20E", self.other_deductions),
            ("20F", self.total_deductions),
            ("21", self.total_allowable_input_tax),
            ("22", self.net_vat_payable),
            ("23A", 0.0),
            ("23B", 0.0),
            ("23C", 0.0),
            ("23D", self.vat_paid_previous),
            ("23E", self.advance_payments),
            ("23F", self.other_credits),
            ("23G", self.total_credits),
        ];
        for (item, value) in items {
            put(&format!("frm2550m:txtTax{item}"), money(value));
        }
        put("frm2550m:txtTax24", money(self.tax_still_payable));
        put("frm2550m:txtTax25A", money(self.surcharge));
        put("frm2550m:txtTax25B", money(self.interest));
        put("frm2550m:txtTax25C", money(self.compromise));
        put("frm2550m:txtTax25D", money(self.total_penalties));
        put("frm2550m:txtTax26", money(self.total_amount_payable));

        // Schedule 1 modal totals and the popup's checkboxes.
        put(
            "frm2550M:txtmodaltxtTotal12A",
            money(self.vatable_sales_private),
        );
        put(
            "frm2550M:txtmodaltxtTotal12B",
            money(self.output_tax_private),
        );
        for (index, (code, _)) in FORM_2550M_ATCS.iter().enumerate() {
            let ticked = self.sales_schedule.iter().any(|row| row.atc_code == *code);
            put(&format!("AtcCode{}", index + 1), flag(ticked));
        }

        // Schedule 4 and 5 modal controls.
        let schedule_4 = self.schedule_4.unwrap_or_default();
        let opened_4 = self.schedule_4.is_some();
        put("txtinputtaxSched4", money(schedule_4.direct_input_tax));
        put(
            "txtTaxableSaleSched4",
            money(if opened_4 {
                self.sales_to_government
            } else {
                0.0
            }),
        );
        put(
            "txtInputTaxnotDirectSched4",
            money(schedule_4.not_direct_input_tax),
        );
        put(
            "txtTotalInputTaxnotDirectSched4",
            money(self.schedule_4_attributed),
        );
        put(
            "txtTotalSaleSched4",
            money(if opened_4 { self.total_sales } else { 0.0 }),
        );
        put(
            "txtTotalInputSaletoGovernmentSched4",
            money(self.schedule_4_total),
        );
        put(
            "txtlessStdTaxSched4",
            money(self.standard_input_tax_government),
        );
        put(
            "txtTotal20BSched4",
            money(self.input_tax_sales_to_government),
        );
        let schedule_5 = self.schedule_5.unwrap_or_default();
        let opened_5 = self.schedule_5.is_some();
        put("txtinputtaxSched5", money(schedule_5.direct_input_tax));
        put(
            "txtTotSaleSched5",
            money(if opened_5 { self.exempt_sales } else { 0.0 }),
        );
        put(
            "txtAmountInputnotDirectSched5",
            money(schedule_5.not_direct_input_tax),
        );
        put(
            "txtProductnotDirectSched5",
            money(self.schedule_5_attributed),
        );
        put(
            "txtSumTotalSaleSched5",
            money(if opened_5 { self.total_sales } else { 0.0 }),
        );
        put("txtTotal20CSched5", money(self.input_tax_exempt_sales));

        put("txtEmail", self.email.trim().to_string());
        fields
    }

    /// The exact official submit plaintext.
    pub fn to_bir_xml_payload(&self) -> Result<String, Vec<(String, String)>> {
        self.official_payload()
    }

    fn validate_inner(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        let mut err = |field: &str, message: &str| {
            errors.push((field.to_string(), message.to_string()));
        };

        // `validate` in order.
        if self.tax_relief == Form2550MTaxRelief::YesUnspecified {
            err(
                "tax_relief",
                "Please select an option on item no. 11. Entry must not be empty.",
            );
        }
        if self.taxable_year == 0 {
            err("taxable_year", "Please indicate a valid Year.");
        } else if self.taxable_year < 2000 {
            err(
                "taxable_year",
                "Invalid date entry on Item no.1. Entry should not be lower than 2000.",
            );
        } else if self.taxable_year > 9999 {
            err("taxable_year", "Item 1 year holds four digits.");
        }
        let (tin1, tin2, tin3, _) = split_tin(&self.tin);
        let tin_digits: String = self.tin.chars().filter(char::is_ascii_digit).collect();
        if tin1.len() != 3
            || tin2.len() != 3
            || tin3.len() != 3
            || tin_digits.len() > 14
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
        let lob = self.line_of_business.trim();
        if lob.is_empty() || lob.chars().count() > 60 {
            err(
                "line_of_business",
                "Please enter a valid Line of Business on Item 6.",
            );
        }
        let name = self.taxpayer_name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            err(
                "taxpayer_name",
                "Please enter a valid Taxpayer Name on Item 7.",
            );
        }
        let phone = self.contact_number.trim();
        if phone.is_empty() {
            err(
                "contact_number",
                "Please enter Taxpayer's telephone number on Item 8.",
            );
        } else if phone.len() > 15 || !digits_only(phone) {
            err(
                "contact_number",
                "Item 8 Telephone No. takes up to 15 digits only.",
            );
        }
        let address = self.registered_address.trim();
        if address.is_empty() {
            err(
                "registered_address",
                "Please enter Taxpayer's Registered Address on Item 9.",
            );
        } else if address.chars().count() > 150 {
            err(
                "registered_address",
                "Item 9 Registered Address holds at most 150 characters.",
            );
        }
        let zip = self.zip_code.trim();
        if zip.is_empty() {
            err("zip_code", "Please enter Taxpayer's Zip code on Item 10.");
        } else if zip.len() > 8 || !digits_only(zip) {
            err("zip_code", "Item 10 Zip Code takes up to 8 digits only.");
        }

        // The page leaves these to its controls; the filename needs them.
        if !(1..=12).contains(&self.month) {
            err("month", "Select the month on Item 1.");
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
            || email.chars().count() > 60
        {
            err(
                "email",
                "Enter the email address BIR sends the filing confirmation to.",
            );
        }

        // Schedule 1.
        for (index, row) in self.sales_schedule.iter().enumerate() {
            let field = format!("sales_schedule[{index}]");
            if form_2550m_atc_index(&row.atc_code).is_none() {
                err(
                    &field,
                    &format!("{} is not an ATC the official form offers.", row.atc_code),
                );
            }
            if self.sales_schedule[..index]
                .iter()
                .any(|other| other.atc_code == row.atc_code)
            {
                err(&field, "Each ATC appears once in Schedule 1.");
            }
            if row.amount <= 0.0 {
                err(
                    &field,
                    &format!(
                        "Please enter valid value for the Amount of Sales/Receipt for the period on row {}",
                        index + 1
                    ),
                );
            }
        }
        // showSched4 / showSched5 refuse to open without the sales they allocate.
        if self.schedule_4.is_some() && self.sales_to_government <= 0.0 {
            err(
                "schedule_4",
                "Please enter a valid value on Item 13A to be able to load the Schedule 4.",
            );
        }
        if self.schedule_5.is_some() && self.exempt_sales <= 0.0 {
            err(
                "schedule_5",
                "Please enter a valid value on Item 15 to be able to load the Schedule 5.",
            );
        }

        // `round` keeps 12 integer digits; `numbersonly` takes no minus sign.
        let mut amounts: Vec<(String, f64)> = vec![
            ("sales_to_government".into(), self.sales_to_government),
            ("output_tax_government".into(), self.output_tax_government),
            ("zero_rated_sales".into(), self.zero_rated_sales),
            ("exempt_sales".into(), self.exempt_sales),
            ("input_tax_carried_over".into(), self.input_tax_carried_over),
            (
                "input_tax_deferred_capital_goods".into(),
                self.input_tax_deferred_capital_goods,
            ),
            ("transitional_input_tax".into(), self.transitional_input_tax),
            ("presumptive_input_tax".into(), self.presumptive_input_tax),
            ("other_input_tax".into(), self.other_input_tax),
            ("domestic_goods".into(), self.domestic_goods),
            (
                "domestic_goods_input_tax".into(),
                self.domestic_goods_input_tax,
            ),
            ("imported_goods".into(), self.imported_goods),
            (
                "imported_goods_input_tax".into(),
                self.imported_goods_input_tax,
            ),
            ("domestic_services".into(), self.domestic_services),
            (
                "domestic_services_input_tax".into(),
                self.domestic_services_input_tax,
            ),
            ("non_resident_services".into(), self.non_resident_services),
            (
                "non_resident_services_input_tax".into(),
                self.non_resident_services_input_tax,
            ),
            (
                "purchases_not_qualified".into(),
                self.purchases_not_qualified,
            ),
            ("other_purchases".into(), self.other_purchases),
            (
                "other_purchases_input_tax".into(),
                self.other_purchases_input_tax,
            ),
            (
                "standard_input_tax_government".into(),
                self.standard_input_tax_government,
            ),
            ("vat_refund_claimed".into(), self.vat_refund_claimed),
            ("other_deductions".into(), self.other_deductions),
            ("vat_paid_previous".into(), self.vat_paid_previous),
            ("advance_payments".into(), self.advance_payments),
            ("other_credits".into(), self.other_credits),
            ("surcharge".into(), self.surcharge),
            ("interest".into(), self.interest),
            ("compromise".into(), self.compromise),
        ];
        for (index, row) in self.sales_schedule.iter().enumerate() {
            amounts.push((format!("sales_schedule[{index}].amount"), row.amount));
        }
        for (name, schedule) in [
            ("schedule_4", self.schedule_4),
            ("schedule_5", self.schedule_5),
        ] {
            if let Some(schedule) = schedule {
                amounts.push((
                    format!("{name}.direct_input_tax"),
                    schedule.direct_input_tax,
                ));
                amounts.push((
                    format!("{name}.not_direct_input_tax"),
                    schedule.not_direct_input_tax,
                ));
            }
        }
        for (field, value) in amounts {
            if !value.is_finite() || !(0.0..1e12).contains(&value) || !has_cent_precision(value) {
                err(
                    &field,
                    "Enter a non-negative amount below 1,000,000,000,000 in pesos and centavos.",
                );
            }
        }
        if !self.is_amended && self.vat_paid_previous != 0.0 {
            err(
                "vat_paid_previous",
                "Item 23D applies only to an amended return.",
            );
        }

        // Derived items must be what the official compute chain produces.
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

impl FormValidator for Form2550MDraft {
    /// `validate` in order with its alert texts, plus the limits the page
    /// enforces while typing (`maxlength`, `wholenumber`, `numbersonly`) and
    /// the Schedule 1, 4 and 5 rules.
    fn validate(&self) -> Vec<(String, String)> {
        self.validate_inner()
    }
}

impl QueueableForm for Form2550MDraft {
    const FORM_CODE: &'static str = "2550M";
    /// Official `formType` and SFTP folder (`ftpTargetFolder.PROD['2550M']`).
    const FORM_TYPE: &'static str = "2550M";
    const LAYOUT_ID: &'static str = FORM_2550M_FORM_ID;

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
        FilingPeriod::Monthly(self.month)
    }
    /// `RtnYear + txtYear`, as in `createXMLFileName`.
    fn period_code(&self) -> String {
        format!("{:02}{}", self.month, self.taxable_year)
    }
    fn parse_period_code(code: &str) -> Option<(u16, FilingPeriod)> {
        if code.len() != 6 || !digits_only(code) {
            return None;
        }
        let month: u8 = code.get(..2)?.parse().ok()?;
        let year: u16 = code.get(2..)?.parse().ok()?;
        (1..=12)
            .contains(&month)
            .then_some((year, FilingPeriod::Monthly(month)))
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

    /// The fixed layout, with the Schedule 1 row controls the submit loop
    /// writes between Item 26 and the Schedule 1 totals.
    fn official_payload(&self) -> Result<String, Vec<(String, String)>> {
        let errors = <Self as QueueableForm>::validate(self);
        if !errors.is_empty() {
            return Err(errors);
        }
        let xml_error = |error: String| vec![("xml".to_string(), error)];
        let layout = crate::official_xml::layout(Self::LAYOUT_ID)
            .map_err(|error| xml_error(error.to_string()))?;
        let fixed = crate::official_xml::write(layout, &self.layout_fields())
            .map_err(|error| xml_error(error.to_string()))?;
        if self.sales_schedule.is_empty() {
            return Ok(fixed);
        }
        let at = fixed
            .find(SCHEDULE_1_ANCHOR)
            .ok_or_else(|| xml_error("Schedule 1 totals are missing from the layout".into()))?;
        // Every control on this page is followed by the same separator.
        let separator = &layout.lead;
        let mut rows = String::new();
        for (index, row) in self.sales_schedule.iter().enumerate() {
            let n = index + 1;
            for (id, value) in [
                (format!("frm2550m:txtAtcCde{n}"), row.atc_code.clone()),
                (
                    format!("frm2550m:txtAmountSales{n}"),
                    official_amount(row.amount),
                ),
                (
                    format!("frm2550m:txtOutputTax{n}"),
                    official_amount(row.output_tax),
                ),
            ] {
                rows.push_str(&format!("<div>{id}={value}{id}=</div>{separator}"));
            }
        }
        let mut payload = fixed;
        payload.insert_str(at, &rows);
        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> Form2550MDraft {
        let mut draft = Form2550MDraft {
            id: None,
            month: 6,
            taxable_year: 2022,
            is_amended: false,
            number_of_attached_sheets: 0,
            tin: "12345678800000".to_string(),
            rdo_code: "039".to_string(),
            line_of_business: "Sample Consulting Services".to_string(),
            taxpayer_name: "Sample Dummy Taxpayer".to_string(),
            contact_number: "09170000000".to_string(),
            registered_address: "123 Sample Street, Quezon City".to_string(),
            zip_code: "1100".to_string(),
            email: "Sample.Taxpayer@example.com".to_string(),
            tax_relief: Form2550MTaxRelief::No,
            ..Form2550MDraft::blank()
        };
        draft.add_sales_atc("VT010").unwrap();
        draft.add_sales_atc("VB010").unwrap();
        // Rows follow the popup order: VB010, then VT010.
        draft.sales_schedule[0].amount = 2_345.67;
        draft.sales_schedule[1].amount = 100_000.005;
        draft.set_sales_to_government(50_000.0);
        draft.exempt_sales = 10_000.0;
        draft.input_tax_carried_over = 1_000.0;
        draft.set_purchase(Form2550MPurchase::DomesticGoods, 20_000.0);
        draft.set_purchase(Form2550MPurchase::DomesticServices, 3_333.33);
        draft.schedule_4 = Some(Form2550MAllocation {
            direct_input_tax: 500.0,
            not_direct_input_tax: 1_200.0,
        });
        draft.schedule_5 = Some(Form2550MAllocation {
            direct_input_tax: 100.0,
            not_direct_input_tax: 300.0,
        });
        draft.surcharge = 25.5;
        draft.recompute();
        draft
    }

    impl Form2550MDraft {
        fn blank() -> Self {
            let profile: TaxpayerProfile = serde_json::from_value(serde_json::json!({
                "id": null,
                "full_name": "",
                "tin": {"segment1": "", "segment2": "", "segment3": "", "branch": ""},
                "rdo_code": "",
                "line_of_business": "",
                "registered_address": "",
                "zip_code": "",
                "phone": "",
                "email": "",
                "default_form_type": "2550M",
                "taxpayer_type": "Corporation",
                "business_start_date": "2020-01-15",
                "tax_elections": []
            }))
            .expect("blank profile");
            Self::new_from_profile(&profile, 2022, 6)
        }
    }

    fn messages(draft: &Form2550MDraft) -> Vec<String> {
        <Form2550MDraft as FormValidator>::validate(draft)
            .into_iter()
            .map(|(_, message)| message)
            .collect()
    }

    #[test]
    fn compute_chain_matches_the_official_page() {
        let d = sample();
        // Popup order: VB010 before VT010.
        assert_eq!(d.sales_schedule[0].atc_code, "VB010");
        assert_eq!(d.sales_schedule[1].amount, 100_000.01);
        assert_eq!(d.sales_schedule[1].output_tax, 12_000.0);
        assert_eq!(d.vatable_sales_private, 102_345.68);
        assert_eq!(d.output_tax_private, 12_281.48);
        assert_eq!(d.output_tax_government, 6_000.0);
        assert_eq!(d.standard_input_tax_government, 3_500.0);
        assert_eq!(d.total_sales, 162_345.68);
        assert_eq!(d.total_output_tax, 18_281.48);
        assert_eq!(d.domestic_goods_input_tax, 2_400.0);
        assert_eq!(d.domestic_services_input_tax, 400.0);
        assert_eq!(d.total_current_purchases, 23_333.33);
        assert_eq!(d.total_available_input_tax, 3_800.0);
        assert_eq!(d.schedule_4_attributed, 369.58);
        assert_eq!(d.schedule_4_total, 869.58);
        assert_eq!(d.input_tax_sales_to_government, -2_630.42);
        assert_eq!(d.schedule_5_attributed, 18.48);
        assert_eq!(d.input_tax_exempt_sales, 118.48);
        assert_eq!(d.total_deductions, -2_511.94);
        assert_eq!(d.total_allowable_input_tax, 6_311.94);
        assert_eq!(d.net_vat_payable, 11_969.54);
        assert_eq!(d.total_amount_payable, 11_995.04);
        assert!(messages(&d).is_empty(), "{:?}", messages(&d));
    }

    #[test]
    fn field_map_uses_official_formats() {
        let d = sample();
        let fields = d.to_bir_field_map();
        assert_eq!(fields["frm2550m:RtnYear"], "06");
        assert_eq!(fields["frm2550m:txtTaxPayerName"], "SAMPLE DUMMY TAXPAYER");
        assert_eq!(fields["txtEmail"], "Sample.Taxpayer@example.com");
        assert_eq!(fields["frm2550m:txtBranchCode"], "00000");
        assert_eq!(fields["frm2550m:txtTax12A"], "102,345.68");
        assert_eq!(fields["frm2550m:txtAtcCde2"], "VT010");
        assert_eq!(fields["frm2550m:txtAmountSales2"], "100,000.01");
        assert_eq!(fields["AtcCode1"], "true");
        assert_eq!(fields["AtcCode34"], "true");
        assert_eq!(fields["AtcCode2"], "false");
        assert_eq!(fields["txtlessStdTaxSched4"], "3,500.00");
        assert_eq!(fields["frm2550m:txtTax20B"], "-2,630.42");
        assert_eq!(
            d.submission_filename(),
            "12345678800000-2550M-062022#Sample.Taxpayer@example.com#.xml"
        );
        let payload = d.to_bir_xml_payload().unwrap();
        let rows = payload.find("frm2550m:txtAtcCde1=VB010").unwrap();
        let totals = payload.find(SCHEDULE_1_ANCHOR).unwrap();
        let item_26 = payload.find("<div>frm2550m:txtTax26=").unwrap();
        assert!(item_26 < rows && rows < totals);
    }

    #[test]
    fn fresh_return_keeps_the_page_defaults() {
        let mut d = Form2550MDraft::blank();
        d.recompute();
        assert_eq!(d.standard_input_tax_government, 5.0);
        let fields = d.layout_fields();
        assert_eq!(fields["txtlessStdTaxSched4"], "5.00");
        assert_eq!(fields["txtTotal20BSched4"], "0.00");
    }

    #[test]
    fn period_codes_round_trip() {
        let d = sample();
        assert_eq!(d.period_code(), "062022");
        assert_eq!(
            Form2550MDraft::parse_period_code("062022"),
            Some((2022, FilingPeriod::Monthly(6)))
        );
        assert_eq!(Form2550MDraft::parse_period_code("132022"), None);
        assert_eq!(Form2550MDraft::parse_period_code("62022"), None);
    }

    #[test]
    fn official_negative_cases_are_rejected_with_their_alerts() {
        let check = |mutate: &dyn Fn(&mut Form2550MDraft), expected: &str| {
            let mut d = sample();
            mutate(&mut d);
            let found = messages(&d);
            assert!(
                found.iter().any(|m| m == expected),
                "{expected:?} not in {found:?}"
            );
        };
        check(
            &|d| d.tax_relief = Form2550MTaxRelief::YesUnspecified,
            "Please select an option on item no. 11. Entry must not be empty.",
        );
        check(&|d| d.taxable_year = 0, "Please indicate a valid Year.");
        check(
            &|d| d.taxable_year = 1999,
            "Invalid date entry on Item no.1. Entry should not be lower than 2000.",
        );
        check(
            &|d| d.tin = "1234".into(),
            "Please enter a valid TIN number on Item 4.",
        );
        check(
            &|d| d.tin = "12345678900000".into(),
            "You have entered an incorrect TIN",
        );
        check(
            &|d| d.rdo_code = "000".into(),
            "Please enter a valid RDO Code on Item 5.",
        );
        check(
            &|d| d.line_of_business.clear(),
            "Please enter a valid Line of Business on Item 6.",
        );
        check(
            &|d| d.taxpayer_name.clear(),
            "Please enter a valid Taxpayer Name on Item 7.",
        );
        check(
            &|d| d.contact_number.clear(),
            "Please enter Taxpayer's telephone number on Item 8.",
        );
        check(
            &|d| d.registered_address.clear(),
            "Please enter Taxpayer's Registered Address on Item 9.",
        );
        check(
            &|d| d.zip_code.clear(),
            "Please enter Taxpayer's Zip code on Item 10.",
        );
        check(
            &|d| {
                d.sales_schedule[0].amount = 0.0;
                d.recompute();
            },
            "Please enter valid value for the Amount of Sales/Receipt for the period on row 1",
        );
        check(
            &|d| {
                d.set_sales_to_government(0.0);
            },
            "Please enter a valid value on Item 13A to be able to load the Schedule 4.",
        );
        check(
            &|d| {
                d.exempt_sales = 0.0;
                d.recompute();
            },
            "Please enter a valid value on Item 15 to be able to load the Schedule 5.",
        );
        check(&|d| d.month = 0, "Select the month on Item 1.");
    }

    #[test]
    fn schedule_1_popup_rules() {
        let mut d = sample();
        assert!(d.add_sales_atc("VB010").is_err());
        assert!(d.add_sales_atc("XX999").is_err());
        d.remove_sales_atc("VB010");
        assert_eq!(d.sales_schedule.len(), 1);
        assert_eq!(d.to_bir_field_map()["AtcCode1"], "false");
    }

    #[test]
    fn amended_flag_controls_item_23d() {
        let mut d = sample();
        d.vat_paid_previous = 100.0;
        d.recompute();
        assert_eq!(d.vat_paid_previous, 0.0);
        d.is_amended = true;
        d.vat_paid_previous = 100.0;
        d.recompute();
        assert_eq!(d.total_credits, 100.0);
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
