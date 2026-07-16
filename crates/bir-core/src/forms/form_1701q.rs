//! BIR Form 1701Q, January 2018 (ENCS).
//!
//! This semantic draft is backed by the locked two-page official PDF. The
//! source pack has no exact-revision saved XML, so local draft persistence and
//! preview data are supported while XML export and electronic submission stay
//! fail-closed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::form_2551q::{AnnualIncomeTaxElection, annual_income_tax_election};
use super::{FilingPeriod, FilingStatus, FormValidator, TypedBirForm};
use crate::profile::{TaxpayerProfile, TaxpayerType};
use crate::validation::{validate_email, validate_zip};

pub const FORM_CODE: &str = "1701Q";
pub const FORM_REVISION: &str = "2018";
pub const FORM_TYPE_ID: &str = "1701Qv2018";
pub const XML_ROUND_TRIP_SUPPORTED: bool = false;
pub const QUEUE_SUBMISSION_SUPPORTED: bool = false;

/// Item 7 on page 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1701QFilerType {
    SingleProprietor,
    Professional,
    Estate,
    Trust,
}

impl Form1701QFilerType {
    pub const ALL: [Self; 4] = [
        Self::SingleProprietor,
        Self::Professional,
        Self::Estate,
        Self::Trust,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::SingleProprietor => "Single Proprietor",
            Self::Professional => "Professional",
            Self::Estate => "Estate",
            Self::Trust => "Trust",
        }
    }
}

/// Item 19 on page 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1701QSpouseType {
    SingleProprietor,
    Professional,
    CompensationEarner,
}

impl Form1701QSpouseType {
    pub const ALL: [Self; 3] = [
        Self::SingleProprietor,
        Self::Professional,
        Self::CompensationEarner,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::SingleProprietor => "Single Proprietor",
            Self::Professional => "Professional",
            Self::CompensationEarner => "Compensation Earner",
        }
    }
}

/// The exact ATC choices printed in Items 8 and 20.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1701QAtc {
    Ii011,
    Ii012,
    Ii013,
    Ii014,
    Ii015,
    Ii016,
    Ii017,
}

impl Form1701QAtc {
    pub const TAXPAYER_CHOICES: [Self; 6] = [
        Self::Ii012,
        Self::Ii014,
        Self::Ii013,
        Self::Ii015,
        Self::Ii017,
        Self::Ii016,
    ];
    pub const SPOUSE_CHOICES: [Self; 7] = [
        Self::Ii012,
        Self::Ii014,
        Self::Ii013,
        Self::Ii011,
        Self::Ii015,
        Self::Ii017,
        Self::Ii016,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::Ii011 => "II011",
            Self::Ii012 => "II012",
            Self::Ii013 => "II013",
            Self::Ii014 => "II014",
            Self::Ii015 => "II015",
            Self::Ii016 => "II016",
            Self::Ii017 => "II017",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Ii011 => "Compensation Income",
            Self::Ii012 => "Business Income - Graduated IT Rates",
            Self::Ii013 => "Mixed Income - Graduated IT Rates",
            Self::Ii014 => "Income from Profession - Graduated IT Rates",
            Self::Ii015 => "Business Income - 8% IT Rate",
            Self::Ii016 => "Mixed Income - 8% IT Rate",
            Self::Ii017 => "Income from Profession - 8% IT Rate",
        }
    }

    pub const fn tax_rate(self) -> Option<Form1701QTaxRate> {
        match self {
            Self::Ii012 | Self::Ii013 | Self::Ii014 => Some(Form1701QTaxRate::Graduated),
            Self::Ii015 | Self::Ii016 | Self::Ii017 => Some(Form1701QTaxRate::EightPercent),
            Self::Ii011 => None,
        }
    }

    pub const fn gets_eight_percent_reduction(self) -> bool {
        matches!(self, Self::Ii015 | Self::Ii017)
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_ascii_uppercase().as_str() {
            "II011" => Some(Self::Ii011),
            "II012" => Some(Self::Ii012),
            "II013" => Some(Self::Ii013),
            "II014" => Some(Self::Ii014),
            "II015" => Some(Self::Ii015),
            "II016" => Some(Self::Ii016),
            "II017" => Some(Self::Ii017),
            _ => None,
        }
    }
}

/// Items 16 and 25.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1701QTaxRate {
    Graduated,
    EightPercent,
}

impl Form1701QTaxRate {
    pub const ALL: [Self; 2] = [Self::Graduated, Self::EightPercent];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Graduated => "Graduated Rates",
            Self::EightPercent => "8% IT Rate",
        }
    }
}

/// Items 16A and 25A. It applies only when the corresponding rate is graduated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1701QDeductionMethod {
    Itemized,
    Osd,
}

impl Form1701QDeductionMethod {
    pub const ALL: [Self; 2] = [Self::Itemized, Self::Osd];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Itemized => "Itemized Deduction",
            Self::Osd => "Optional Standard Deduction (OSD)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form1701QParty {
    Taxpayer,
    Spouse,
}

/// A paired amount cell. `None` preserves an officially blank cell and is not
/// equivalent to an entered zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701QAmountPair {
    pub taxpayer: Option<f64>,
    pub spouse: Option<f64>,
}

impl Form1701QAmountPair {
    pub const fn value(&self, party: Form1701QParty) -> Option<f64> {
        match party {
            Form1701QParty::Taxpayer => self.taxpayer,
            Form1701QParty::Spouse => self.spouse,
        }
    }

    pub fn set(&mut self, party: Form1701QParty, value: Option<f64>) {
        match party {
            Form1701QParty::Taxpayer => self.taxpayer = value,
            Form1701QParty::Spouse => self.spouse = value,
        }
    }
}

/// Every paired amount line printed in Parts III and V.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701QAmounts {
    pub item_26: Form1701QAmountPair,
    pub item_27: Form1701QAmountPair,
    pub item_28: Form1701QAmountPair,
    pub item_29: Form1701QAmountPair,
    pub item_30: Form1701QAmountPair,
    pub item_36: Form1701QAmountPair,
    pub item_37: Form1701QAmountPair,
    pub item_38: Form1701QAmountPair,
    pub item_39: Form1701QAmountPair,
    pub item_40: Form1701QAmountPair,
    pub item_41: Form1701QAmountPair,
    pub item_42: Form1701QAmountPair,
    pub item_43: Form1701QAmountPair,
    pub item_44: Form1701QAmountPair,
    pub item_45: Form1701QAmountPair,
    pub item_46: Form1701QAmountPair,
    pub item_47: Form1701QAmountPair,
    pub item_48: Form1701QAmountPair,
    pub item_49: Form1701QAmountPair,
    pub item_50: Form1701QAmountPair,
    pub item_51: Form1701QAmountPair,
    pub item_52: Form1701QAmountPair,
    pub item_53: Form1701QAmountPair,
    pub item_54: Form1701QAmountPair,
    pub item_55: Form1701QAmountPair,
    pub item_56: Form1701QAmountPair,
    pub item_57: Form1701QAmountPair,
    pub item_58: Form1701QAmountPair,
    pub item_59: Form1701QAmountPair,
    pub item_60: Form1701QAmountPair,
    pub item_61: Form1701QAmountPair,
    pub item_62: Form1701QAmountPair,
    pub item_63: Form1701QAmountPair,
    pub item_64: Form1701QAmountPair,
    pub item_65: Form1701QAmountPair,
    pub item_66: Form1701QAmountPair,
    pub item_67: Form1701QAmountPair,
    pub item_68: Form1701QAmountPair,
}

impl Form1701QAmounts {
    pub fn get(&self, item: u8) -> Option<&Form1701QAmountPair> {
        Some(match item {
            26 => &self.item_26,
            27 => &self.item_27,
            28 => &self.item_28,
            29 => &self.item_29,
            30 => &self.item_30,
            36 => &self.item_36,
            37 => &self.item_37,
            38 => &self.item_38,
            39 => &self.item_39,
            40 => &self.item_40,
            41 => &self.item_41,
            42 => &self.item_42,
            43 => &self.item_43,
            44 => &self.item_44,
            45 => &self.item_45,
            46 => &self.item_46,
            47 => &self.item_47,
            48 => &self.item_48,
            49 => &self.item_49,
            50 => &self.item_50,
            51 => &self.item_51,
            52 => &self.item_52,
            53 => &self.item_53,
            54 => &self.item_54,
            55 => &self.item_55,
            56 => &self.item_56,
            57 => &self.item_57,
            58 => &self.item_58,
            59 => &self.item_59,
            60 => &self.item_60,
            61 => &self.item_61,
            62 => &self.item_62,
            63 => &self.item_63,
            64 => &self.item_64,
            65 => &self.item_65,
            66 => &self.item_66,
            67 => &self.item_67,
            68 => &self.item_68,
            _ => return None,
        })
    }

    pub fn get_mut(&mut self, item: u8) -> Option<&mut Form1701QAmountPair> {
        Some(match item {
            26 => &mut self.item_26,
            27 => &mut self.item_27,
            28 => &mut self.item_28,
            29 => &mut self.item_29,
            30 => &mut self.item_30,
            36 => &mut self.item_36,
            37 => &mut self.item_37,
            38 => &mut self.item_38,
            39 => &mut self.item_39,
            40 => &mut self.item_40,
            41 => &mut self.item_41,
            42 => &mut self.item_42,
            43 => &mut self.item_43,
            44 => &mut self.item_44,
            45 => &mut self.item_45,
            46 => &mut self.item_46,
            47 => &mut self.item_47,
            48 => &mut self.item_48,
            49 => &mut self.item_49,
            50 => &mut self.item_50,
            51 => &mut self.item_51,
            52 => &mut self.item_52,
            53 => &mut self.item_53,
            54 => &mut self.item_54,
            55 => &mut self.item_55,
            56 => &mut self.item_56,
            57 => &mut self.item_57,
            58 => &mut self.item_58,
            59 => &mut self.item_59,
            60 => &mut self.item_60,
            61 => &mut self.item_61,
            62 => &mut self.item_62,
            63 => &mut self.item_63,
            64 => &mut self.item_64,
            65 => &mut self.item_65,
            66 => &mut self.item_66,
            67 => &mut self.item_67,
            68 => &mut self.item_68,
            _ => return None,
        })
    }
}

/// One row in Part IV. The exact saved-file transport mapping is intentionally
/// unknown, but these official PDF fields persist in the app-owned draft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701QPaymentRow {
    pub drawee_bank_or_agency: String,
    pub number: String,
    pub date: String,
    pub amount: Option<f64>,
}

impl Form1701QPaymentRow {
    pub fn is_empty(&self) -> bool {
        self.drawee_bank_or_agency.trim().is_empty()
            && self.number.trim().is_empty()
            && self.date.trim().is_empty()
            && self.amount.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701QPaymentDetails {
    pub item_32_cash_or_bank_debit_memo: Form1701QPaymentRow,
    pub item_33_check: Form1701QPaymentRow,
    pub item_34_tax_debit_memo: Form1701QPaymentRow,
    pub item_35_others: Form1701QPaymentRow,
    pub item_35_others_description: String,
    pub machine_validation_or_receipt_details: String,
}

pub const USER_ENTERED_AMOUNT_ITEMS: &[u8] = &[
    36, 37, 39, 42, 43, 44, 47, 48, 50, 55, 56, 57, 58, 59, 60, 61, 64, 65, 66,
];

const SIGNED_INPUT_ITEMS: &[u8] = &[42, 50];
const DERIVED_AMOUNT_ITEMS: &[u8] = &[
    26, 27, 28, 29, 30, 38, 40, 41, 45, 46, 49, 51, 52, 53, 54, 62, 63, 67, 68,
];

/// App-owned semantic draft for the exact January 2018 revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701QDraft {
    pub id: Option<i64>,

    // Items 1-4.
    pub taxable_year: u16,
    pub quarter: u8,
    #[serde(default)]
    pub is_amended: bool,
    #[serde(default)]
    pub number_of_sheets: u8,

    // Items 5-16A.
    pub tin: String,
    pub rdo_code: String,
    #[serde(default)]
    pub filer_type: Option<Form1701QFilerType>,
    #[serde(default)]
    pub atc: Option<Form1701QAtc>,
    pub taxpayer_name: String,
    /// Page 2 prints this separately from Item 9's full name. It remains blank
    /// until explicitly supplied because the profile owns only an unstructured
    /// full-name string.
    #[serde(default)]
    pub taxpayer_last_name: String,
    pub registered_address: String,
    #[serde(default)]
    pub registered_address_2: String,
    pub zip_code: String,
    #[serde(default)]
    pub date_of_birth: String,
    pub email: String,
    #[serde(default)]
    pub citizenship: String,
    #[serde(default)]
    pub foreign_tax_number: String,
    #[serde(default)]
    pub claims_foreign_tax_credits: Option<bool>,
    #[serde(default)]
    pub tax_rate: Option<Form1701QTaxRate>,
    #[serde(default)]
    pub deduction_method: Option<Form1701QDeductionMethod>,
    /// Profile metadata retained for the generic render envelope. The official
    /// January 2018 Form 1701Q does not print a contact-number field.
    pub contact_number: String,

    // Items 17-25A.
    #[serde(default)]
    pub has_spouse: bool,
    #[serde(default)]
    pub spouse_tin: String,
    #[serde(default)]
    pub spouse_rdo_code: String,
    #[serde(default)]
    pub spouse_type: Option<Form1701QSpouseType>,
    #[serde(default)]
    pub spouse_atc: Option<Form1701QAtc>,
    #[serde(default)]
    pub spouse_name: String,
    #[serde(default)]
    pub spouse_citizenship: String,
    #[serde(default)]
    pub spouse_foreign_tax_number: String,
    #[serde(default)]
    pub spouse_claims_foreign_tax_credits: Option<bool>,
    #[serde(default)]
    pub spouse_tax_rate: Option<Form1701QTaxRate>,
    #[serde(default)]
    pub spouse_deduction_method: Option<Form1701QDeductionMethod>,

    // Parts III and V.
    #[serde(default)]
    pub amounts: Form1701QAmounts,
    #[serde(default)]
    pub item_31_aggregate_amount_payable: Option<f64>,
    #[serde(default)]
    pub item_43_non_operating_income_description: String,
    #[serde(default)]
    pub item_48_non_operating_income_description: String,
    #[serde(default)]
    pub item_61_other_tax_credit_description: String,

    // Part IV.
    #[serde(default)]
    pub payment_details: Form1701QPaymentDetails,

    // Compatibility aggregates used by existing dashboard/preview callers.
    #[serde(default)]
    pub total_tax_due: f64,
    #[serde(default)]
    pub total_amount_payable: f64,

    // Lifecycle.
    pub status: FilingStatus,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
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

impl Form1701QDraft {
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, quarter: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let (tax_rate, deduction_method) = match annual_income_tax_election(profile, year) {
            AnnualIncomeTaxElection::EightPercent => (Some(Form1701QTaxRate::EightPercent), None),
            AnnualIncomeTaxElection::Graduated => {
                let method = profile
                    .tax_elections
                    .iter()
                    .rev()
                    .find(|entry| entry.taxable_year == year)
                    .and_then(|entry| match entry.election {
                        crate::profile::IncomeTaxElection::GraduatedOsd => {
                            Some(Form1701QDeductionMethod::Osd)
                        }
                        crate::profile::IncomeTaxElection::GraduatedItemized => {
                            Some(Form1701QDeductionMethod::Itemized)
                        }
                        crate::profile::IncomeTaxElection::GraduatedUnspecified
                        | crate::profile::IncomeTaxElection::EightPercent => None,
                    });
                (Some(Form1701QTaxRate::Graduated), method)
            }
            AnnualIncomeTaxElection::Unrecorded | AnnualIncomeTaxElection::Conflicting => {
                (None, None)
            }
        };
        let atc = profile
            .atc_codes
            .iter()
            .filter_map(|code| Form1701QAtc::from_code(code))
            .find(|candidate| *candidate != Form1701QAtc::Ii011);
        let filer_type = match profile.taxpayer_type {
            TaxpayerType::Estate => Some(Form1701QFilerType::Estate),
            TaxpayerType::Trust => Some(Form1701QFilerType::Trust),
            TaxpayerType::Individual
            | TaxpayerType::Corporation
            | TaxpayerType::Partnership
            | TaxpayerType::Cooperative => None,
        };

        Self {
            id: None,
            taxable_year: year,
            quarter,
            is_amended: false,
            number_of_sheets: 0,
            tin: profile.tin.full(),
            rdo_code: profile.rdo_code.clone(),
            filer_type,
            atc,
            taxpayer_name: profile.full_name.clone(),
            taxpayer_last_name: String::new(),
            registered_address: profile.registered_address.clone(),
            registered_address_2: String::new(),
            zip_code: profile.zip_code.clone(),
            date_of_birth: profile
                .birth_date
                .map(|date| date.format("%m/%d/%Y").to_string())
                .unwrap_or_default(),
            email: profile.email.clone(),
            citizenship: String::new(),
            foreign_tax_number: String::new(),
            claims_foreign_tax_credits: None,
            tax_rate,
            deduction_method,
            contact_number: profile.phone.clone(),
            has_spouse: false,
            spouse_tin: String::new(),
            spouse_rdo_code: String::new(),
            spouse_type: None,
            spouse_atc: None,
            spouse_name: String::new(),
            spouse_citizenship: String::new(),
            spouse_foreign_tax_number: String::new(),
            spouse_claims_foreign_tax_credits: None,
            spouse_tax_rate: None,
            spouse_deduction_method: None,
            amounts: Form1701QAmounts::default(),
            item_31_aggregate_amount_payable: None,
            item_43_non_operating_income_description: String::new(),
            item_48_non_operating_income_description: String::new(),
            item_61_other_tax_credit_description: String::new(),
            payment_details: Form1701QPaymentDetails::default(),
            total_tax_due: 0.0,
            total_amount_payable: 0.0,
            status: FilingStatus::Draft,
            created_at: Some(now.clone()),
            updated_at: Some(now),
            submitted_at: None,
            confirmed_at: None,
            submission_filename: None,
            receipt_id: None,
            submission_attempts: 0,
            next_retry_at: None,
            last_error: None,
        }
    }

    pub const fn form_code(&self) -> &'static str {
        FORM_CODE
    }

    pub const fn form_type_id(&self) -> &'static str {
        FORM_TYPE_ID
    }

    pub const fn taxable_year_u16(&self) -> u16 {
        self.taxable_year
    }

    pub const fn quarter_u8(&self) -> u8 {
        self.quarter
    }

    pub fn period_code(&self) -> String {
        format!("{}Q{}", self.taxable_year, self.quarter)
    }

    pub fn default_submission_filename(&self) -> String {
        format!(
            "{}-{FORM_TYPE_ID}-{}.xml",
            self.tin.replace('-', ""),
            self.period_code()
        )
    }

    pub fn amount(&self, item: u8, party: Form1701QParty) -> Option<f64> {
        self.amounts.get(item).and_then(|pair| pair.value(party))
    }

    pub fn set_amount(&mut self, item: u8, party: Form1701QParty, value: Option<f64>) {
        if let Some(pair) = self.amounts.get_mut(item) {
            pair.set(party, value);
        }
    }

    /// Recomputes only arithmetic printed on the official January 2018 form.
    /// It does not infer penalties, payment dates, or submission-only values.
    pub fn recompute(&mut self) {
        self.recompute_party(Form1701QParty::Taxpayer);
        self.recompute_party(Form1701QParty::Spouse);

        let item_30_taxpayer = self.amount(30, Form1701QParty::Taxpayer);
        let item_30_spouse = self.amount(30, Form1701QParty::Spouse);
        self.item_31_aggregate_amount_payable = match (item_30_taxpayer, item_30_spouse) {
            (None, None) => None,
            (taxpayer, spouse) => Some(round_peso(taxpayer.unwrap_or(0.0) + spouse.unwrap_or(0.0))),
        };
        self.total_tax_due = round_peso(
            self.amount(26, Form1701QParty::Taxpayer).unwrap_or(0.0)
                + self.amount(26, Form1701QParty::Spouse).unwrap_or(0.0),
        );
        self.total_amount_payable = self.item_31_aggregate_amount_payable.unwrap_or(0.0);
    }

    fn recompute_party(&mut self, party: Form1701QParty) {
        let applies = matches!(party, Form1701QParty::Taxpayer) || self.has_spouse;
        for item in DERIVED_AMOUNT_ITEMS {
            self.set_amount(*item, party, None);
        }
        if !applies {
            return;
        }

        let rate = match party {
            Form1701QParty::Taxpayer => self.tax_rate,
            Form1701QParty::Spouse => self.spouse_tax_rate,
        };
        let deduction = match party {
            Form1701QParty::Taxpayer => self.deduction_method,
            Form1701QParty::Spouse => self.spouse_deduction_method,
        };
        let atc = match party {
            Form1701QParty::Taxpayer => self.atc,
            Form1701QParty::Spouse => self.spouse_atc,
        };
        let (Some(rate), Some(atc)) = (rate, atc) else {
            return;
        };
        if atc == Form1701QAtc::Ii011 || atc.tax_rate() != Some(rate) {
            return;
        }

        if rate == Form1701QTaxRate::Graduated {
            let item_38 = round_peso(self.value_or_zero(36, party) - self.value_or_zero(37, party));
            self.set_amount(38, party, Some(item_38));
            let selected_deduction = match deduction {
                Some(Form1701QDeductionMethod::Itemized) => Some(self.value_or_zero(39, party)),
                Some(Form1701QDeductionMethod::Osd) => {
                    let item_40 = round_peso(self.value_or_zero(36, party) * 0.40);
                    self.set_amount(40, party, Some(item_40));
                    Some(item_40)
                }
                None => None,
            };
            if let Some(selected_deduction) = selected_deduction {
                let item_41 = round_peso(item_38 - selected_deduction);
                let item_45 = round_peso(
                    item_41
                        + self.value_or_zero(42, party)
                        + self.value_or_zero(43, party)
                        + self.value_or_zero(44, party),
                );
                let item_46 = round_peso(graduated_tax_due(self.taxable_year, item_45));
                self.set_amount(41, party, Some(item_41));
                self.set_amount(45, party, Some(item_45));
                self.set_amount(46, party, Some(item_46));
            }
        }

        if rate == Form1701QTaxRate::EightPercent {
            let item_49 = round_peso(self.value_or_zero(47, party) + self.value_or_zero(48, party));
            let item_51 = round_peso(item_49 + self.value_or_zero(50, party));
            let item_52 = if atc.gets_eight_percent_reduction() {
                250_000.0
            } else {
                0.0
            };
            let item_53 = round_peso(item_51 - item_52);
            // A line labelled TAX DUE cannot be negative. Unlike Items 63
            // and 68, this is not an overpayment line.
            let item_54 = round_peso(item_53.max(0.0) * 0.08);
            self.set_amount(52, party, Some(item_52));
            self.set_amount(53, party, Some(item_53));
            self.set_amount(54, party, Some(item_54));
            self.set_amount(49, party, Some(item_49));
            self.set_amount(51, party, Some(item_51));
        }

        let item_62 = round_peso((55..=61).map(|item| self.value_or_zero(item, party)).sum());
        let item_67 = round_peso((64..=66).map(|item| self.value_or_zero(item, party)).sum());
        self.set_amount(62, party, Some(item_62));
        self.set_amount(67, party, Some(item_67));

        let selected_tax_due = match rate {
            Form1701QTaxRate::Graduated => self.amount(46, party),
            Form1701QTaxRate::EightPercent => self.amount(54, party),
        };
        if let Some(item_26) = selected_tax_due {
            let item_63 = round_peso(item_26 - item_62);
            let item_68 = round_peso(item_63 + item_67);
            self.set_amount(63, party, Some(item_63));
            self.set_amount(68, party, Some(item_68));
            self.set_amount(26, party, Some(item_26));
            self.set_amount(27, party, Some(item_62));
            self.set_amount(28, party, Some(item_63));
            self.set_amount(29, party, Some(item_67));
            self.set_amount(30, party, Some(item_68));
        }
    }

    fn value_or_zero(&self, item: u8, party: Form1701QParty) -> f64 {
        self.amount(item, party).unwrap_or(0.0)
    }

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub const fn can_queue_for_submission(&self) -> bool {
        QUEUE_SUBMISSION_SUPPORTED
    }

    pub fn evidence_warnings(&self) -> Vec<String> {
        vec![
            "No exact-revision 1701Qv2018 saved XML is present in the reviewed source pack; XML round-trip and electronic submission are disabled."
                .to_string(),
            "The available BIRForm1701QScript.js targets an older incompatible Items 26-41 layout and is not used for January 2018 formulas or transport mapping."
                .to_string(),
            "Only arithmetic and rate tables printed on the locked January 2018 official PDF are computed by this draft."
                .to_string(),
        ]
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        let mut errors = self.validate();
        errors.push((
            "submission".to_string(),
            "1701Qv2018 is manual/external until an exact-revision XML and submission contract is reviewed"
                .to_string(),
        ));
        Err(errors)
    }

    pub fn transition_to_submitted(&mut self, _filename: String) -> Result<(), String> {
        Err(
            "1701Qv2018 cannot transition to Submitted because queue/submission transport is not certified"
                .to_string(),
        )
    }

    pub fn revert_to_draft(&mut self) -> Result<(), String> {
        if matches!(self.status, FilingStatus::Paid) {
            return Err("A paid 1701Q return cannot be reverted directly to Draft".to_string());
        }
        self.status = FilingStatus::Draft;
        self.submitted_at = None;
        self.confirmed_at = None;
        self.submission_filename = None;
        self.receipt_id = None;
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
        Ok(())
    }
}

impl FormValidator for Form1701QDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        validate_identity(self, &mut errors);
        validate_choices(self, &mut errors);
        validate_amount_inputs(self, &mut errors);
        validate_payment_details(self, &mut errors);
        validate_computed_values(self, &mut errors);
        errors
    }
}

fn validate_identity(draft: &Form1701QDraft, errors: &mut Vec<(String, String)>) {
    let tin_digits = digits(&draft.tin);
    if !(12..=14).contains(&tin_digits.len()) {
        errors.push((
            "tin".to_string(),
            "TIN must contain 12 to 14 digits, with optional separators".to_string(),
        ));
    }
    if !(2018..=9999).contains(&draft.taxable_year) {
        errors.push((
            "taxable_year".to_string(),
            "January 2018 Form 1701Q supports taxable years 2018 onward".to_string(),
        ));
    }
    if !(1..=3).contains(&draft.quarter) {
        errors.push((
            "quarter".to_string(),
            "1701Q quarter must be 1, 2, or 3".to_string(),
        ));
    }
    if draft.number_of_sheets > 99 {
        errors.push((
            "number_of_sheets".to_string(),
            "Item 4 supports at most two digits".to_string(),
        ));
    }
    for (field, label, value) in [
        ("rdo_code", "RDO code", draft.rdo_code.as_str()),
        (
            "taxpayer_name",
            "Taxpayer/filer name",
            draft.taxpayer_name.as_str(),
        ),
        (
            "taxpayer_last_name",
            "Taxpayer/filer last name for page 2",
            draft.taxpayer_last_name.as_str(),
        ),
        (
            "registered_address",
            "Registered address",
            draft.registered_address.as_str(),
        ),
        ("zip_code", "ZIP code", draft.zip_code.as_str()),
        ("email", "Email address", draft.email.as_str()),
    ] {
        if value.trim().is_empty() {
            errors.push((field.to_string(), format!("{label} is required")));
        }
    }
    if !draft.rdo_code.trim().is_empty()
        && (draft.rdo_code.len() != 3
            || !draft
                .rdo_code
                .chars()
                .all(|character| character.is_ascii_digit()))
    {
        errors.push((
            "rdo_code".to_string(),
            "RDO code must be 3 digits".to_string(),
        ));
    }
    if !draft.zip_code.trim().is_empty() && !validate_zip(draft.zip_code.trim()) {
        errors.push((
            "zip_code".to_string(),
            "ZIP code must be 4 digits".to_string(),
        ));
    }
    if !draft.email.trim().is_empty() && !validate_email(&draft.email) {
        errors.push(("email".to_string(), "Email address is invalid".to_string()));
    }
    validate_optional_date("date_of_birth", &draft.date_of_birth, errors);

    if draft.has_spouse {
        let spouse_tin_digits = digits(&draft.spouse_tin);
        if !(12..=14).contains(&spouse_tin_digits.len()) {
            errors.push((
                "spouse_tin".to_string(),
                "Item 17 spouse TIN must contain 12 to 14 digits".to_string(),
            ));
        }
        if draft.spouse_name.trim().is_empty() {
            errors.push((
                "spouse_name".to_string(),
                "Item 21 spouse name is required when spouse information is enabled".to_string(),
            ));
        }
        if !draft.spouse_rdo_code.trim().is_empty()
            && (draft.spouse_rdo_code.len() != 3
                || !draft
                    .spouse_rdo_code
                    .chars()
                    .all(|character| character.is_ascii_digit()))
        {
            errors.push((
                "spouse_rdo_code".to_string(),
                "Item 18 spouse RDO code must be 3 digits".to_string(),
            ));
        }
    }
}

fn validate_choices(draft: &Form1701QDraft, errors: &mut Vec<(String, String)>) {
    if draft.filer_type.is_none() {
        errors.push((
            "filer_type".to_string(),
            "Select the Item 7 taxpayer/filer type".to_string(),
        ));
    }
    if draft.atc.is_none() {
        errors.push(("atc".to_string(), "Select the Item 8 ATC".to_string()));
    }
    if draft.atc == Some(Form1701QAtc::Ii011) {
        errors.push((
            "atc".to_string(),
            "II011 is printed only as a spouse ATC in Item 20".to_string(),
        ));
    }
    validate_rate_and_deduction(
        "taxpayer",
        draft.atc,
        draft.tax_rate,
        draft.deduction_method,
        errors,
    );
    if draft.claims_foreign_tax_credits.is_none() {
        errors.push((
            "claims_foreign_tax_credits".to_string(),
            "Answer Item 15 Yes or No".to_string(),
        ));
    }

    if draft.has_spouse {
        if draft.spouse_type.is_none() {
            errors.push((
                "spouse_type".to_string(),
                "Select the Item 19 spouse type".to_string(),
            ));
        }
        if draft.spouse_atc.is_none() {
            errors.push((
                "spouse_atc".to_string(),
                "Select the Item 20 spouse ATC".to_string(),
            ));
        }
        if draft.spouse_claims_foreign_tax_credits.is_none() {
            errors.push((
                "spouse_claims_foreign_tax_credits".to_string(),
                "Answer Item 24 Yes or No".to_string(),
            ));
        }
        if draft.spouse_atc == Some(Form1701QAtc::Ii011) {
            if draft.spouse_type != Some(Form1701QSpouseType::CompensationEarner) {
                errors.push((
                    "spouse_atc".to_string(),
                    "II011 requires Item 19 Compensation Earner".to_string(),
                ));
            }
            if draft.spouse_tax_rate.is_some() || draft.spouse_deduction_method.is_some() {
                errors.push((
                    "spouse_tax_rate".to_string(),
                    "Items 25 and 25A do not apply to the II011 compensation-only choice"
                        .to_string(),
                ));
            }
        } else if draft.spouse_type == Some(Form1701QSpouseType::CompensationEarner) {
            errors.push((
                "spouse_atc".to_string(),
                "Item 19 Compensation Earner requires the Item 20 II011 ATC".to_string(),
            ));
        } else {
            validate_rate_and_deduction(
                "spouse",
                draft.spouse_atc,
                draft.spouse_tax_rate,
                draft.spouse_deduction_method,
                errors,
            );
        }
    }
}

fn validate_rate_and_deduction(
    prefix: &str,
    atc: Option<Form1701QAtc>,
    rate: Option<Form1701QTaxRate>,
    deduction: Option<Form1701QDeductionMethod>,
    errors: &mut Vec<(String, String)>,
) {
    if rate.is_none() {
        errors.push((
            format!("{prefix}_tax_rate"),
            "Select the printed income tax rate".to_string(),
        ));
    }
    if let (Some(atc), Some(rate)) = (atc, rate)
        && atc.tax_rate().is_some_and(|atc_rate| atc_rate != rate)
    {
        errors.push((
            format!("{prefix}_atc"),
            format!(
                "ATC {} does not match the selected {}",
                atc.code(),
                rate.label()
            ),
        ));
    }
    match rate {
        Some(Form1701QTaxRate::Graduated) if deduction.is_none() => errors.push((
            format!("{prefix}_deduction_method"),
            "Graduated rate requires Itemized or OSD".to_string(),
        )),
        Some(Form1701QTaxRate::EightPercent) if deduction.is_some() => errors.push((
            format!("{prefix}_deduction_method"),
            "The deduction-method choice applies only to graduated rates".to_string(),
        )),
        _ => {}
    }
}

fn validate_amount_inputs(draft: &Form1701QDraft, errors: &mut Vec<(String, String)>) {
    for item in USER_ENTERED_AMOUNT_ITEMS {
        for party in [Form1701QParty::Taxpayer, Form1701QParty::Spouse] {
            let Some(value) = draft.amount(*item, party) else {
                continue;
            };
            let party_name = match party {
                Form1701QParty::Taxpayer => "taxpayer",
                Form1701QParty::Spouse => "spouse",
            };
            let field = format!("item_{item}_{party_name}");
            if !value.is_finite() {
                errors.push((field, "Amount must be finite".to_string()));
                continue;
            }
            if !SIGNED_INPUT_ITEMS.contains(item) && value < 0.0 {
                errors.push((
                    field,
                    "This official input line cannot contain a negative amount".to_string(),
                ));
            } else if !is_whole_peso(value) {
                errors.push((
                    field,
                    "Form 1701Q requires whole-peso amounts; do not enter centavos".to_string(),
                ));
            }
        }
    }

    validate_schedule_applicability(
        draft,
        Form1701QParty::Taxpayer,
        draft.tax_rate,
        draft.deduction_method,
        errors,
    );
    if draft.has_spouse {
        validate_schedule_applicability(
            draft,
            Form1701QParty::Spouse,
            draft.spouse_tax_rate,
            draft.spouse_deduction_method,
            errors,
        );
        if draft.spouse_atc == Some(Form1701QAtc::Ii011)
            && USER_ENTERED_AMOUNT_ITEMS
                .iter()
                .any(|item| draft.amount(*item, Form1701QParty::Spouse).is_some())
        {
            errors.push((
                "spouse_amounts".to_string(),
                "Part V spouse amount cells must be blank for the II011 compensation-only choice"
                    .to_string(),
            ));
        }
    } else if USER_ENTERED_AMOUNT_ITEMS
        .iter()
        .any(|item| draft.amount(*item, Form1701QParty::Spouse).is_some())
    {
        errors.push((
            "spouse_amounts".to_string(),
            "Spouse amount cells must be blank when spouse information is disabled".to_string(),
        ));
    }

    if !draft.is_amended {
        for party in [Form1701QParty::Taxpayer, Form1701QParty::Spouse] {
            if draft.amount(59, party).is_some() {
                let field = match party {
                    Form1701QParty::Taxpayer => "item_59_taxpayer",
                    Form1701QParty::Spouse => "item_59_spouse",
                };
                errors.push((
                    field.to_string(),
                    "Item 59 applies only to an amended return".to_string(),
                ));
            }
        }
    }
    for (party, claim) in [
        (Form1701QParty::Taxpayer, draft.claims_foreign_tax_credits),
        (
            Form1701QParty::Spouse,
            draft.spouse_claims_foreign_tax_credits,
        ),
    ] {
        if draft.amount(60, party).is_some_and(|value| value > 0.0) && claim != Some(true) {
            let field = match party {
                Form1701QParty::Taxpayer => "item_60_taxpayer",
                Form1701QParty::Spouse => "item_60_spouse",
            };
            errors.push((
                field.to_string(),
                "Foreign tax credits require the corresponding Item 15/24 Yes choice".to_string(),
            ));
        }
    }

    for (item, field, description) in [
        (
            43,
            "item_43_non_operating_income_description",
            draft.item_43_non_operating_income_description.as_str(),
        ),
        (
            48,
            "item_48_non_operating_income_description",
            draft.item_48_non_operating_income_description.as_str(),
        ),
        (
            61,
            "item_61_other_tax_credit_description",
            draft.item_61_other_tax_credit_description.as_str(),
        ),
    ] {
        let has_value = [Form1701QParty::Taxpayer, Form1701QParty::Spouse]
            .into_iter()
            .any(|party| draft.amount(item, party).is_some_and(|value| value != 0.0));
        if has_value && description.trim().is_empty() {
            errors.push((
                field.to_string(),
                format!("Item {item} requires the printed specify description"),
            ));
        }
    }
}

fn validate_schedule_applicability(
    draft: &Form1701QDraft,
    party: Form1701QParty,
    rate: Option<Form1701QTaxRate>,
    deduction: Option<Form1701QDeductionMethod>,
    errors: &mut Vec<(String, String)>,
) {
    let party_name = match party {
        Form1701QParty::Taxpayer => "taxpayer",
        Form1701QParty::Spouse => "spouse",
    };
    let graduated_inputs = [36, 37, 39, 42, 43, 44];
    let eight_percent_inputs = [47, 48, 50];
    if matches!(rate, Some(Form1701QTaxRate::Graduated))
        && eight_percent_inputs
            .iter()
            .any(|item| draft.amount(*item, party).is_some())
    {
        errors.push((
            format!("{party_name}_schedule_ii"),
            "Schedule II inputs must be blank for a graduated-rate filer".to_string(),
        ));
    }
    if matches!(rate, Some(Form1701QTaxRate::EightPercent))
        && graduated_inputs
            .iter()
            .any(|item| draft.amount(*item, party).is_some())
    {
        errors.push((
            format!("{party_name}_schedule_i"),
            "Schedule I inputs must be blank for an 8%-rate filer".to_string(),
        ));
    }
    if matches!(rate, Some(Form1701QTaxRate::EightPercent))
        && draft
            .amount(51, party)
            .is_some_and(|cumulative_income| cumulative_income > 3_000_000.0)
    {
        errors.push((
            format!("{party_name}_tax_rate"),
            "The printed 1701Q note requires an 8% election to change to graduated rates when cumulative gross sales/receipts and other non-operating income exceed P3,000,000"
                .to_string(),
        ));
    }
    if matches!(deduction, Some(Form1701QDeductionMethod::Osd))
        && [37, 39]
            .iter()
            .any(|item| draft.amount(*item, party).is_some())
    {
        errors.push((
            format!("{party_name}_deduction_method"),
            "Cost of sales/services and itemized deductions must be blank when OSD is selected"
                .to_string(),
        ));
    }
}

fn validate_payment_details(draft: &Form1701QDraft, errors: &mut Vec<(String, String)>) {
    for (field, row, is_others) in [
        (
            "payment_32_cash_or_bank_debit_memo",
            &draft.payment_details.item_32_cash_or_bank_debit_memo,
            false,
        ),
        (
            "payment_33_check",
            &draft.payment_details.item_33_check,
            false,
        ),
        (
            "payment_34_tax_debit_memo",
            &draft.payment_details.item_34_tax_debit_memo,
            false,
        ),
        (
            "payment_35_others",
            &draft.payment_details.item_35_others,
            true,
        ),
    ] {
        if let Some(amount) = row.amount
            && (!amount.is_finite() || amount < 0.0)
        {
            errors.push((
                format!("{field}.amount"),
                "Payment amount must be finite and non-negative".to_string(),
            ));
        }
        if !row.date.trim().is_empty()
            && chrono::NaiveDate::parse_from_str(row.date.trim(), "%m/%d/%Y").is_err()
        {
            errors.push((
                format!("{field}.date"),
                "Payment date must use MM/DD/YYYY and be a real date".to_string(),
            ));
        }
        if is_others
            && !row.is_empty()
            && draft
                .payment_details
                .item_35_others_description
                .trim()
                .is_empty()
        {
            errors.push((
                "payment_35_others_description".to_string(),
                "Item 35 payment details require an Others description".to_string(),
            ));
        }
    }
}

fn validate_computed_values(draft: &Form1701QDraft, errors: &mut Vec<(String, String)>) {
    let mut expected = draft.clone();
    expected.recompute();
    for item in DERIVED_AMOUNT_ITEMS {
        for party in [Form1701QParty::Taxpayer, Form1701QParty::Spouse] {
            let actual = draft.amount(*item, party);
            let wanted = expected.amount(*item, party);
            if !optional_amount_equal(actual, wanted) {
                let party_name = match party {
                    Form1701QParty::Taxpayer => "taxpayer",
                    Form1701QParty::Spouse => "spouse",
                };
                errors.push((
                    format!("item_{item}_{party_name}"),
                    format!(
                        "Computed amount must match the official printed arithmetic: {wanted:?}"
                    ),
                ));
            }
        }
    }
    if !optional_amount_equal(
        draft.item_31_aggregate_amount_payable,
        expected.item_31_aggregate_amount_payable,
    ) {
        errors.push((
            "item_31_aggregate_amount_payable".to_string(),
            format!(
                "Item 31 must equal Item 30A plus Item 30B: {:?}",
                expected.item_31_aggregate_amount_payable
            ),
        ));
    }
}

fn optional_amount_equal(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => left.is_finite() && (left - right).abs() < 0.001,
        _ => false,
    }
}

fn validate_optional_date(field: &str, value: &str, errors: &mut Vec<(String, String)>) {
    if !value.trim().is_empty()
        && chrono::NaiveDate::parse_from_str(value.trim(), "%m/%d/%Y").is_err()
    {
        errors.push((
            field.to_string(),
            "Date must use MM/DD/YYYY and be a real calendar date".to_string(),
        ));
    }
}

fn digits(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

fn is_whole_peso(value: f64) -> bool {
    (value - value.round()).abs() < 0.001
}

fn round_peso(value: f64) -> f64 {
    value.round()
}

/// Applies only the two rate tables printed on page 2 of the locked form.
pub fn graduated_tax_due(taxable_year: u16, taxable_income: f64) -> f64 {
    let income = taxable_income;
    if income <= 250_000.0 {
        return 0.0;
    }
    if taxable_year <= 2022 {
        match income {
            value if value <= 400_000.0 => (value - 250_000.0) * 0.20,
            value if value <= 800_000.0 => 30_000.0 + (value - 400_000.0) * 0.25,
            value if value <= 2_000_000.0 => 130_000.0 + (value - 800_000.0) * 0.30,
            value if value <= 8_000_000.0 => 490_000.0 + (value - 2_000_000.0) * 0.32,
            value => 2_410_000.0 + (value - 8_000_000.0) * 0.35,
        }
    } else {
        match income {
            value if value <= 400_000.0 => (value - 250_000.0) * 0.15,
            value if value <= 800_000.0 => 22_500.0 + (value - 400_000.0) * 0.20,
            value if value <= 2_000_000.0 => 102_500.0 + (value - 800_000.0) * 0.25,
            value if value <= 8_000_000.0 => 402_500.0 + (value - 2_000_000.0) * 0.30,
            value => 2_202_500.0 + (value - 8_000_000.0) * 0.35,
        }
    }
}

impl TypedBirForm for Form1701QDraft {
    fn form_code(&self) -> &'static str {
        FORM_CODE
    }

    fn form_type_id(&self) -> &'static str {
        FORM_TYPE_ID
    }

    fn filing_period(&self) -> FilingPeriod {
        FilingPeriod::Quarterly(self.quarter)
    }

    fn recompute(&mut self) {
        Form1701QDraft::recompute(self);
    }

    fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        Form1701QDraft::to_bir_field_map(self)
    }

    fn to_bir_xml(&self) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_draft() -> Form1701QDraft {
        let mut draft = Form1701QDraft {
            taxable_year: 2021,
            quarter: 2,
            tin: "12345678900000".to_string(),
            rdo_code: "018".to_string(),
            filer_type: Some(Form1701QFilerType::SingleProprietor),
            atc: Some(Form1701QAtc::Ii012),
            taxpayer_name: "JUAN DELA CRUZ".to_string(),
            taxpayer_last_name: "DELA CRUZ".to_string(),
            registered_address: "OLONGAPO CITY".to_string(),
            zip_code: "2200".to_string(),
            contact_number: "09123456789".to_string(),
            email: "juan@example.com".to_string(),
            citizenship: "FILIPINO".to_string(),
            claims_foreign_tax_credits: Some(false),
            tax_rate: Some(Form1701QTaxRate::Graduated),
            deduction_method: Some(Form1701QDeductionMethod::Osd),
            status: FilingStatus::Draft,
            ..Default::default()
        };
        draft.set_amount(36, Form1701QParty::Taxpayer, Some(1_000_000.0));
        draft.set_amount(55, Form1701QParty::Taxpayer, Some(10_000.0));
        draft.set_amount(64, Form1701QParty::Taxpayer, Some(1_000.0));
        draft.recompute();
        draft
    }

    #[test]
    fn recompute_should_apply_printed_graduated_osd_and_part_iii_arithmetic() {
        let draft = valid_draft();

        assert_eq!(draft.amount(40, Form1701QParty::Taxpayer), Some(400_000.0));
        assert_eq!(draft.amount(45, Form1701QParty::Taxpayer), Some(600_000.0));
        assert_eq!(draft.amount(46, Form1701QParty::Taxpayer), Some(80_000.0));
        assert_eq!(draft.amount(63, Form1701QParty::Taxpayer), Some(70_000.0));
        assert_eq!(draft.item_31_aggregate_amount_payable, Some(71_000.0));
    }

    #[test]
    fn recompute_should_preserve_overpayment_signs_through_items_63_68_and_31() {
        let mut draft = valid_draft();
        draft.set_amount(36, Form1701QParty::Taxpayer, Some(0.0));
        draft.set_amount(55, Form1701QParty::Taxpayer, Some(100_000.0));
        draft.set_amount(64, Form1701QParty::Taxpayer, Some(5_000.0));

        draft.recompute();

        assert_eq!(draft.amount(63, Form1701QParty::Taxpayer), Some(-100_000.0));
        assert_eq!(draft.amount(68, Form1701QParty::Taxpayer), Some(-95_000.0));
        assert_eq!(draft.item_31_aggregate_amount_payable, Some(-95_000.0));
        assert_eq!(draft.total_amount_payable, -95_000.0);
    }

    #[test]
    fn recompute_should_apply_purely_self_employed_eight_percent_reduction() {
        let mut draft = valid_draft();
        draft.atc = Some(Form1701QAtc::Ii015);
        draft.tax_rate = Some(Form1701QTaxRate::EightPercent);
        draft.deduction_method = None;
        draft.set_amount(36, Form1701QParty::Taxpayer, None);
        draft.set_amount(47, Form1701QParty::Taxpayer, Some(500_000.0));
        draft.set_amount(48, Form1701QParty::Taxpayer, Some(10_000.0));

        draft.recompute();

        assert_eq!(draft.amount(52, Form1701QParty::Taxpayer), Some(250_000.0));
        assert_eq!(draft.amount(53, Form1701QParty::Taxpayer), Some(260_000.0));
        assert_eq!(draft.amount(54, Form1701QParty::Taxpayer), Some(20_800.0));
        assert_eq!(draft.amount(38, Form1701QParty::Taxpayer), None);
    }

    #[test]
    fn recompute_should_leave_compensation_only_spouse_computation_blank() {
        let mut draft = valid_draft();
        draft.has_spouse = true;
        draft.spouse_type = Some(Form1701QSpouseType::CompensationEarner);
        draft.spouse_atc = Some(Form1701QAtc::Ii011);
        draft.spouse_tax_rate = None;
        draft.spouse_deduction_method = None;

        draft.recompute();

        assert_eq!(draft.amount(26, Form1701QParty::Spouse), None);
        assert_eq!(draft.amount(62, Form1701QParty::Spouse), None);
        assert_eq!(draft.amount(67, Form1701QParty::Spouse), None);
    }

    #[test]
    fn recompute_should_fail_closed_when_atc_and_rate_conflict() {
        let mut draft = valid_draft();
        draft.atc = Some(Form1701QAtc::Ii015);
        draft.tax_rate = Some(Form1701QTaxRate::Graduated);

        draft.recompute();

        assert_eq!(draft.amount(38, Form1701QParty::Taxpayer), None);
        assert_eq!(draft.amount(46, Form1701QParty::Taxpayer), None);
        assert_eq!(draft.amount(26, Form1701QParty::Taxpayer), None);
    }

    #[test]
    fn graduated_tax_due_should_use_2023_onward_printed_table() {
        assert_eq!(graduated_tax_due(2026, 600_000.0), 62_500.0);
    }

    #[test]
    fn validation_should_reject_negative_non_loss_input_but_allow_item_42_loss() {
        let mut draft = valid_draft();
        draft.set_amount(36, Form1701QParty::Taxpayer, Some(-1.0));
        draft.set_amount(42, Form1701QParty::Taxpayer, Some(-100_000.0));
        draft.recompute();

        let errors = draft.validate();

        assert!(errors.iter().any(|(field, _)| field == "item_36_taxpayer"));
        assert!(!errors.iter().any(|(field, _)| field == "item_42_taxpayer"));
    }

    #[test]
    fn validation_should_reject_non_finite_input() {
        let mut draft = valid_draft();
        draft.set_amount(55, Form1701QParty::Taxpayer, Some(f64::NAN));
        draft.recompute();

        let errors = draft.validate();

        assert!(errors.iter().any(|(field, _)| field == "item_55_taxpayer"));
    }

    #[test]
    fn validation_should_preserve_blank_semantics_for_inactive_schedule_inputs() {
        let mut draft = valid_draft();
        draft.set_amount(47, Form1701QParty::Taxpayer, Some(0.0));
        draft.recompute();

        let errors = draft.validate();

        assert!(
            errors
                .iter()
                .any(|(field, _)| field == "taxpayer_schedule_ii")
        );
    }

    #[test]
    fn validation_should_enforce_printed_eight_percent_three_million_limit() {
        let mut draft = valid_draft();
        draft.atc = Some(Form1701QAtc::Ii015);
        draft.tax_rate = Some(Form1701QTaxRate::EightPercent);
        draft.deduction_method = None;
        draft.set_amount(36, Form1701QParty::Taxpayer, None);
        draft.set_amount(47, Form1701QParty::Taxpayer, Some(3_000_001.0));
        draft.recompute();

        let errors = draft.validate();

        assert!(errors.iter().any(|(field, _)| field == "taxpayer_tax_rate"));
    }

    #[test]
    fn validation_should_apply_amended_item_59_rule_to_spouse_column() {
        let mut draft = valid_draft();
        draft.has_spouse = true;
        draft.set_amount(59, Form1701QParty::Spouse, Some(1_000.0));
        draft.recompute();

        let errors = draft.validate();

        assert!(errors.iter().any(|(field, _)| field == "item_59_spouse"));
    }

    #[test]
    fn validation_should_not_require_profile_contact_metadata() {
        let mut draft = valid_draft();
        draft.contact_number.clear();

        let errors = draft.validate();

        assert!(!errors.iter().any(|(field, _)| field == "contact_number"));
    }

    #[test]
    fn validation_should_require_explicit_page_two_last_name() {
        let mut draft = valid_draft();
        draft.taxpayer_last_name.clear();

        let errors = draft.validate();

        assert!(
            errors
                .iter()
                .any(|(field, _)| field == "taxpayer_last_name")
        );
    }

    #[test]
    fn validation_should_require_descriptions_for_specify_amount_lines() {
        let mut draft = valid_draft();
        draft.set_amount(61, Form1701QParty::Taxpayer, Some(500.0));
        draft.recompute();

        let errors = draft.validate();

        assert!(
            errors
                .iter()
                .any(|(field, _)| field == "item_61_other_tax_credit_description")
        );
    }

    #[test]
    fn transition_to_queued_should_fail_closed_without_mutating_status() {
        let mut draft = valid_draft();

        let result = draft.transition_to_queued();

        assert!(result.is_err());
        assert_eq!(draft.status, FilingStatus::Draft);
    }

    #[test]
    fn lifecycle_transitions_should_return_errors_instead_of_panicking() {
        let mut draft = valid_draft();
        draft.status = FilingStatus::Paid;

        let revert = draft.revert_to_draft();
        let submit = draft.transition_to_submitted("blocked.xml".to_string());

        assert!(revert.is_err());
        assert!(submit.is_err());
        assert_eq!(draft.status, FilingStatus::Paid);
    }

    #[test]
    fn amount_registry_should_own_every_official_paired_line() {
        let amounts = Form1701QAmounts::default();
        let expected = [
            26, 27, 28, 29, 30, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52,
            53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68,
        ];

        assert!(expected.into_iter().all(|item| amounts.get(item).is_some()));
    }

    #[test]
    fn json_round_trip_should_preserve_choices_blanks_and_signed_amounts() {
        let mut draft = valid_draft();
        draft.has_spouse = true;
        draft.spouse_type = Some(Form1701QSpouseType::CompensationEarner);
        draft.set_amount(42, Form1701QParty::Taxpayer, Some(-25_000.0));
        draft.set_amount(43, Form1701QParty::Taxpayer, None);
        draft.recompute();

        let json = serde_json::to_string(&draft).expect("1701Q draft should serialize");
        let restored: Form1701QDraft =
            serde_json::from_str(&json).expect("1701Q draft should deserialize");

        assert_eq!(restored.taxable_year, 2021);
        assert_eq!(restored.quarter, 2);
        assert_eq!(restored.taxpayer_last_name, "DELA CRUZ");
        assert_eq!(
            restored.spouse_type,
            Some(Form1701QSpouseType::CompensationEarner)
        );
        assert_eq!(
            restored.amount(42, Form1701QParty::Taxpayer),
            Some(-25_000.0)
        );
        assert_eq!(restored.amount(43, Form1701QParty::Taxpayer), None);
    }
}
