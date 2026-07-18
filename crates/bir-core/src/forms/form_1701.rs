//! BIR Form 1701, January 2018 (ENCS).
//!
//! The semantic model is limited to the four-page official return and the
//! exact reviewed editable saves in `/Users/uriah/Downloads/forms`: the plain
//! save has 837 fields and its encrypted companion has one additional second
//! address-line field. The separate Part X/attachment worksheets are retained
//! losslessly when an exact save is imported, but are not interpreted as tax
//! evidence here.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::form_2551q::{AnnualIncomeTaxElection, annual_income_tax_election};
use super::{FilingPeriod, FilingStatus, FormValidator, TypedBirForm};
use crate::profile::{IncomeTaxElection, TaxClassification, TaxpayerProfile, TaxpayerType};
use crate::validation::{validate_email, validate_ph_phone, validate_zip};

pub const FORM_CODE: &str = "1701";
pub const FORM_REVISION: &str = "2018";
pub const FORM_TYPE_ID: &str = "1701v2018";
pub const EXACT_REVIEWED_XML_FIELD_COUNT: usize = 837;
pub const EXACT_REVIEWED_ENCRYPTED_XML_FIELD_COUNT: usize = 838;
pub const REVIEWED_ENCRYPTED_XML_EXTRA_FIELD: &str = "frm1701:txtPg1I9Address2";
pub const EXACT_REVIEWED_XML_VERSION: &str = "051414";
pub const QUEUE_SUBMISSION_SUPPORTED: bool = false;
pub const OFFICIAL_FORM_SHA256: &str =
    "19be91d78258eb7c255f2615610db2739f10c378f8ac97adc0887c1bf40d1b2e";
pub const REVIEWED_EDITABLE_XML_SHA256: &str =
    "b168c7b3273d30a10f28f4653847519b876d5a88e77ed82911718a80f65c7827";
pub const REVIEWED_ENCRYPTED_XML_SHA256: &str =
    "3771c99c191ef5e84b1b5e4c51499911bfbec6002febc3c53dca3f08730e92e3";
pub const REVIEWED_ATTACHMENT_PDF_SHA256: &str =
    "e71799dc613c08d4c383fcd66bed83032b182ab43721c8665d7b608047766cad";
pub const REVIEWED_CONSOLIDATED_PDF_SHA256: &str =
    "eac0ce426cc57c473e24638accb14a978ddd54f8cf795cc4303f527088416871";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Form1701Party {
    Taxpayer,
    Spouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1701TaxpayerType {
    SingleProprietor,
    Professional,
    Estate,
    Trust,
    CompensationEarner,
}

impl Form1701TaxpayerType {
    pub const ALL: [Self; 5] = [
        Self::SingleProprietor,
        Self::Professional,
        Self::Estate,
        Self::Trust,
        Self::CompensationEarner,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::SingleProprietor => "Single Proprietor",
            Self::Professional => "Professional",
            Self::Estate => "Estate",
            Self::Trust => "Trust",
            Self::CompensationEarner => "Compensation Earner",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1701SpouseType {
    SingleProprietor,
    Professional,
    CompensationEarner,
}

impl Form1701SpouseType {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Form1701Atc {
    Ii011,
    Ii012,
    Ii013,
    Ii014,
    Ii015,
    Ii016,
    Ii017,
}

impl Form1701Atc {
    pub const ALL: [Self; 7] = [
        Self::Ii011,
        Self::Ii012,
        Self::Ii013,
        Self::Ii014,
        Self::Ii015,
        Self::Ii016,
        Self::Ii017,
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

    pub const fn tax_rate(self) -> Option<Form1701TaxRate> {
        match self {
            Self::Ii011 => None,
            Self::Ii012 | Self::Ii013 | Self::Ii014 => Some(Form1701TaxRate::Graduated),
            Self::Ii015 | Self::Ii016 | Self::Ii017 => Some(Form1701TaxRate::EightPercent),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1701TaxRate {
    Graduated,
    EightPercent,
}

impl Form1701TaxRate {
    pub const ALL: [Self; 2] = [Self::Graduated, Self::EightPercent];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Graduated => "Graduated Rates",
            Self::EightPercent => "8% IT Rate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1701DeductionMethod {
    Itemized,
    Osd,
}

impl Form1701DeductionMethod {
    pub const ALL: [Self; 2] = [Self::Itemized, Self::Osd];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Itemized => "Itemized Deduction",
            Self::Osd => "Optional Standard Deduction (OSD)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1701CivilStatus {
    Single,
    Married,
    LegallySeparated,
    Widowed,
}

impl Form1701CivilStatus {
    pub const ALL: [Self; 4] = [
        Self::Single,
        Self::Married,
        Self::LegallySeparated,
        Self::Widowed,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::Married => "Married",
            Self::LegallySeparated => "Legally Separated",
            Self::Widowed => "Widow/er",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form1701JointFilingStatus {
    Joint,
    Separate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Form1701OverpaymentDisposition {
    #[default]
    None,
    Refund,
    TaxCreditCertificate,
    CarryOver,
}

/// An amount pair preserves an officially blank cell (`None`) separately from
/// an explicitly entered zero (`Some(0.0)`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701AmountPair {
    pub taxpayer: Option<f64>,
    pub spouse: Option<f64>,
}

impl Form1701AmountPair {
    pub const fn value(&self, party: Form1701Party) -> Option<f64> {
        match party {
            Form1701Party::Taxpayer => self.taxpayer,
            Form1701Party::Spouse => self.spouse,
        }
    }

    pub fn set(&mut self, party: Form1701Party, value: Option<f64>) {
        match party {
            Form1701Party::Taxpayer => self.taxpayer = value,
            Form1701Party::Spouse => self.spouse = value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701EmployerRow {
    pub owner: Option<Form1701Party>,
    pub employer_name: String,
    pub employer_tin: String,
    pub compensation_income: Option<f64>,
    pub tax_withheld: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701SpecialDeductionRow {
    pub description: String,
    pub legal_basis: String,
    pub amount: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701NolcoRow {
    pub year_incurred: String,
    pub amount: Option<f64>,
    pub applied_previous_years: Option<f64>,
    pub expired: Option<f64>,
    pub applied_current_year: Option<f64>,
    pub unapplied: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701PaymentRow {
    pub drawee_bank_or_agency: String,
    pub number: String,
    pub date: String,
    pub amount: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701PaymentDetails {
    pub item_34_cash_or_bank_debit_memo: Form1701PaymentRow,
    pub item_35_check: Form1701PaymentRow,
    pub item_36_tax_debit_memo: Form1701PaymentRow,
    pub item_37_others: Form1701PaymentRow,
    pub item_37_others_description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701Spouse {
    pub enabled: bool,
    pub tin: String,
    pub rdo_code: String,
    pub filer_type: Option<Form1701SpouseType>,
    pub atc: Option<Form1701Atc>,
    pub name: String,
    pub contact_number: String,
    pub citizenship: String,
    pub claims_foreign_tax_credits: Option<bool>,
    pub foreign_tax_number: String,
    pub has_exempt_income: Option<bool>,
    pub has_special_rate_income: Option<bool>,
    pub tax_rate: Option<Form1701TaxRate>,
    pub deduction_method: Option<Form1701DeductionMethod>,
}

/// Main-return computation tables. The official item numbers are the keys.
/// Maps allow the UI/renderer to iterate exact printed lines without keeping
/// hundreds of transport-derived Rust identifiers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Form1701Computations {
    pub part_ii: BTreeMap<u8, Form1701AmountPair>,
    pub part_ii_item_32_aggregate: Option<f64>,
    pub schedule_2: BTreeMap<u8, Form1701AmountPair>,
    pub schedule_3: BTreeMap<u8, Form1701AmountPair>,
    pub schedule_3_descriptions: BTreeMap<u8, String>,
    pub schedule_4: BTreeMap<u8, Form1701AmountPair>,
    /// Schedule 4 Item 17a through 17d.
    pub schedule_4_item_17: [Form1701AmountPair; 4],
    pub schedule_4_item_17d_description: String,
    pub schedule_5_taxpayer: [Form1701SpecialDeductionRow; 2],
    pub schedule_5_spouse: [Form1701SpecialDeductionRow; 2],
    pub schedule_5_total_taxpayer: Option<f64>,
    pub schedule_5_total_spouse: Option<f64>,
    pub schedule_6_summary: BTreeMap<u8, Form1701AmountPair>,
    pub schedule_6_taxpayer_nolco: [Form1701NolcoRow; 4],
    pub schedule_6_spouse_nolco: [Form1701NolcoRow; 4],
    pub schedule_6_total_taxpayer: Option<f64>,
    pub schedule_6_total_spouse: Option<f64>,
    pub part_vi: BTreeMap<u8, Form1701AmountPair>,
    pub part_vii: BTreeMap<u8, Form1701AmountPair>,
    pub part_vii_item_9_description: String,
    pub part_viii: BTreeMap<u8, Form1701AmountPair>,
    pub part_ix: BTreeMap<u8, Form1701AmountPair>,
    pub part_ix_descriptions: BTreeMap<u8, String>,
}

/// Complete local draft for exact identity `1701v2018`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Form1701Draft {
    pub id: Option<i64>,

    // Filing identity.
    pub tin: String,
    pub taxable_year: u16,
    /// The save schema carries an end month. A normal annual return must use
    /// December; another month requires the Short Period choice.
    #[serde(alias = "month")]
    pub period_end_month: u8,
    pub is_amended: bool,
    pub is_short_period: bool,

    // Part I.
    pub rdo_code: String,
    pub taxpayer_type: Option<Form1701TaxpayerType>,
    pub atc: Option<Form1701Atc>,
    pub taxpayer_name: String,
    pub registered_address: String,
    pub zip_code: String,
    pub date_of_birth: String,
    pub email: String,
    pub citizenship: String,
    pub claims_foreign_tax_credits: Option<bool>,
    pub foreign_tax_number: String,
    pub contact_number: String,
    pub civil_status: Option<Form1701CivilStatus>,
    pub spouse_has_income: Option<bool>,
    pub joint_filing_status: Option<Form1701JointFilingStatus>,
    pub has_exempt_income: Option<bool>,
    pub has_special_rate_income: Option<bool>,
    pub tax_rate: Option<Form1701TaxRate>,
    pub deduction_method: Option<Form1701DeductionMethod>,

    // Page 1 remainder and page 2 spouse background.
    pub number_of_attachments: Option<u8>,
    pub overpayment_disposition: Form1701OverpaymentDisposition,
    pub spouse: Form1701Spouse,
    pub employers: [Form1701EmployerRow; 2],
    pub computations: Form1701Computations,
    pub payment_details: Form1701PaymentDetails,
    pub machine_validation_or_receipt_details: String,

    /// The complete raw field map from an exact imported save. Modeled values
    /// overwrite their corresponding keys on export; all attachment and
    /// unknown fields remain byte-value equivalent after parse/generate.
    pub preserved_xml_fields: BTreeMap<String, String>,
    pub has_exact_xml_snapshot: bool,

    // Local lifecycle only. Electronic queue/submission is not certified.
    pub status: FilingStatus,
    pub created_at: String,
    pub updated_at: String,
    pub submitted_at: Option<String>,
    pub confirmed_at: Option<String>,
    pub submission_filename: Option<String>,
    pub receipt_id: Option<i64>,
    pub submission_attempts: u32,
    pub next_retry_at: Option<String>,
    pub last_error: Option<String>,
}

impl Default for Form1701Draft {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            tin: String::new(),
            taxable_year: 2018,
            period_end_month: 12,
            is_amended: false,
            is_short_period: false,
            rdo_code: String::new(),
            taxpayer_type: None,
            atc: None,
            taxpayer_name: String::new(),
            registered_address: String::new(),
            zip_code: String::new(),
            date_of_birth: String::new(),
            email: String::new(),
            citizenship: String::new(),
            claims_foreign_tax_credits: None,
            foreign_tax_number: String::new(),
            contact_number: String::new(),
            civil_status: None,
            spouse_has_income: None,
            joint_filing_status: None,
            has_exempt_income: None,
            has_special_rate_income: None,
            tax_rate: None,
            deduction_method: None,
            number_of_attachments: None,
            overpayment_disposition: Form1701OverpaymentDisposition::None,
            spouse: Form1701Spouse::default(),
            employers: std::array::from_fn(|_| Form1701EmployerRow::default()),
            computations: Form1701Computations::default(),
            payment_details: Form1701PaymentDetails::default(),
            machine_validation_or_receipt_details: String::new(),
            preserved_xml_fields: BTreeMap::new(),
            has_exact_xml_snapshot: false,
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
}

impl Form1701Draft {
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, _legacy_month: u8) -> Self {
        let mut draft = Self {
            tin: profile.tin.full(),
            taxable_year: year,
            period_end_month: 12,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            date_of_birth: profile
                .birth_date
                .map(|date| date.format("%m/%d/%Y").to_string())
                .unwrap_or_default(),
            email: profile.email.clone(),
            contact_number: profile.phone.clone(),
            ..Self::default()
        };

        let recognized_atcs = profile
            .atc_codes
            .iter()
            .filter_map(|code| Form1701Atc::from_code(code))
            .collect::<BTreeSet<_>>();
        draft.atc = (recognized_atcs.len() == 1)
            .then(|| recognized_atcs.iter().next().copied())
            .flatten();
        draft.taxpayer_type = profile_taxpayer_type(profile, draft.atc);
        let annual_election = annual_income_tax_election(profile, year);
        draft.tax_rate = match annual_election {
            AnnualIncomeTaxElection::Graduated => Some(Form1701TaxRate::Graduated),
            AnnualIncomeTaxElection::EightPercent => Some(Form1701TaxRate::EightPercent),
            AnnualIncomeTaxElection::Unrecorded | AnnualIncomeTaxElection::Conflicting => None,
        };
        let has_osd = profile.tax_elections.iter().any(|election| {
            election.taxable_year == year && election.election == IncomeTaxElection::GraduatedOsd
        });
        let has_itemized = profile.tax_elections.iter().any(|election| {
            election.taxable_year == year
                && election.election == IncomeTaxElection::GraduatedItemized
        });
        draft.deduction_method = match (has_osd, has_itemized) {
            (true, false) => Some(Form1701DeductionMethod::Osd),
            (false, true) => Some(Form1701DeductionMethod::Itemized),
            (false, false) | (true, true) => None,
        };
        if draft.tax_rate == Some(Form1701TaxRate::EightPercent) {
            draft.deduction_method = None;
        }
        draft
    }

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub const fn can_queue_for_submission(&self) -> bool {
        QUEUE_SUBMISSION_SUPPORTED
    }

    pub fn xml_evidence_warnings(&self) -> Vec<String> {
        let mut warnings = vec![
            "The reviewed source proves editable-save XML round-trip, not electronic submission semantics; queueing remains disabled."
                .to_string(),
            "The encrypted companion payload is opaque and is not treated as formula or final-flag evidence."
                .to_string(),
            "Part X and attachment worksheet fields are preserved losslessly but are not editable or calculated by this four-page model."
                .to_string(),
        ];
        if !self.has_exact_xml_snapshot {
            warnings.push(
                "This locally-created draft has no imported 837-field exact XML snapshot, so checked XML export is unavailable."
                    .to_string(),
            );
        }
        warnings
    }

    pub fn amount(
        &self,
        section: Form1701AmountSection,
        item: u8,
        party: Form1701Party,
    ) -> Option<f64> {
        self.amount_table(section)
            .get(&item)
            .and_then(|pair| pair.value(party))
    }

    pub fn set_amount(
        &mut self,
        section: Form1701AmountSection,
        item: u8,
        party: Form1701Party,
        value: Option<f64>,
    ) {
        self.amount_table_mut(section)
            .entry(item)
            .or_default()
            .set(party, value);
    }

    fn amount_table(&self, section: Form1701AmountSection) -> &BTreeMap<u8, Form1701AmountPair> {
        match section {
            Form1701AmountSection::PartIi => &self.computations.part_ii,
            Form1701AmountSection::Schedule2 => &self.computations.schedule_2,
            Form1701AmountSection::Schedule3 => &self.computations.schedule_3,
            Form1701AmountSection::Schedule4 => &self.computations.schedule_4,
            Form1701AmountSection::Schedule6 => &self.computations.schedule_6_summary,
            Form1701AmountSection::PartVi => &self.computations.part_vi,
            Form1701AmountSection::PartVii => &self.computations.part_vii,
            Form1701AmountSection::PartViii => &self.computations.part_viii,
            Form1701AmountSection::PartIx => &self.computations.part_ix,
        }
    }

    fn amount_table_mut(
        &mut self,
        section: Form1701AmountSection,
    ) -> &mut BTreeMap<u8, Form1701AmountPair> {
        match section {
            Form1701AmountSection::PartIi => &mut self.computations.part_ii,
            Form1701AmountSection::Schedule2 => &mut self.computations.schedule_2,
            Form1701AmountSection::Schedule3 => &mut self.computations.schedule_3,
            Form1701AmountSection::Schedule4 => &mut self.computations.schedule_4,
            Form1701AmountSection::Schedule6 => &mut self.computations.schedule_6_summary,
            Form1701AmountSection::PartVi => &mut self.computations.part_vi,
            Form1701AmountSection::PartVii => &mut self.computations.part_vii,
            Form1701AmountSection::PartViii => &mut self.computations.part_viii,
            Form1701AmountSection::PartIx => &mut self.computations.part_ix,
        }
    }

    /// Recompute only arithmetic explicitly printed on the January 2018 form.
    /// Blank inputs remain blank until enough upstream evidence exists.
    pub fn recompute(&mut self) {
        self.recompute_employer_totals();
        self.recompute_schedule_4_and_5();
        self.recompute_nolco();
        for party in [Form1701Party::Taxpayer, Form1701Party::Spouse] {
            self.recompute_party(party);
        }
        self.computations.part_ii_item_32_aggregate = sum_present([
            self.amount(Form1701AmountSection::PartIi, 31, Form1701Party::Taxpayer),
            self.amount(Form1701AmountSection::PartIi, 31, Form1701Party::Spouse),
        ]);
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    fn recompute_employer_totals(&mut self) {
        for party in [Form1701Party::Taxpayer, Form1701Party::Spouse] {
            let compensation =
                sum_present(self.employers.iter().filter_map(|row| {
                    (row.owner == Some(party)).then_some(row.compensation_income)
                }));
            let withheld = sum_present(
                self.employers
                    .iter()
                    .filter_map(|row| (row.owner == Some(party)).then_some(row.tax_withheld)),
            );
            self.set_amount(Form1701AmountSection::Schedule2, 4, party, compensation);
            self.set_amount(Form1701AmountSection::PartVii, 5, party, withheld);
        }
    }

    fn recompute_schedule_4_and_5(&mut self) {
        for party in [Form1701Party::Taxpayer, Form1701Party::Spouse] {
            let ordinary = sum_present(
                (1..=16)
                    .map(|item| self.amount(Form1701AmountSection::Schedule4, item, party))
                    .chain(
                        self.computations
                            .schedule_4_item_17
                            .iter()
                            .map(|pair| pair.value(party)),
                    ),
            );
            self.set_amount(Form1701AmountSection::Schedule4, 18, party, ordinary);
        }
        self.computations.schedule_5_total_taxpayer = sum_present(
            self.computations
                .schedule_5_taxpayer
                .iter()
                .map(|row| row.amount),
        );
        self.computations.schedule_5_total_spouse = sum_present(
            self.computations
                .schedule_5_spouse
                .iter()
                .map(|row| row.amount),
        );
    }

    fn recompute_nolco(&mut self) {
        for party in [Form1701Party::Taxpayer, Form1701Party::Spouse] {
            let item_3 = subtract_optional(
                self.amount(Form1701AmountSection::Schedule6, 1, party),
                self.amount(Form1701AmountSection::Schedule6, 2, party),
            );
            self.set_amount(Form1701AmountSection::Schedule6, 3, party, item_3);
        }
        for row in self
            .computations
            .schedule_6_taxpayer_nolco
            .iter_mut()
            .chain(self.computations.schedule_6_spouse_nolco.iter_mut())
        {
            row.unapplied = subtract_many_optional(
                row.amount,
                [
                    row.applied_previous_years,
                    row.expired,
                    row.applied_current_year,
                ],
            );
        }
        self.computations.schedule_6_total_taxpayer = sum_present(
            self.computations
                .schedule_6_taxpayer_nolco
                .iter()
                .map(|row| row.applied_current_year),
        );
        self.computations.schedule_6_total_spouse = sum_present(
            self.computations
                .schedule_6_spouse_nolco
                .iter()
                .map(|row| row.applied_current_year),
        );
    }

    fn recompute_party(&mut self, party: Form1701Party) {
        let rate = self.party_tax_rate(party);
        let deduction = self.party_deduction_method(party);
        let atc = self.party_atc(party);

        let item_4 = self.amount(Form1701AmountSection::Schedule2, 4, party);
        let item_5 = self.amount(Form1701AmountSection::Schedule2, 5, party);
        let item_6 = subtract_optional(item_4, item_5);
        let item_7 =
            item_6.map(|income| round_peso(graduated_income_tax(self.taxable_year, income)));
        self.set_amount(Form1701AmountSection::Schedule2, 6, party, item_6);
        self.set_amount(Form1701AmountSection::Schedule2, 7, party, item_7);

        if rate == Some(Form1701TaxRate::Graduated) {
            let item_10 = subtract_optional(
                self.amount(Form1701AmountSection::Schedule3, 8, party),
                self.amount(Form1701AmountSection::Schedule3, 9, party),
            );
            let item_12 = subtract_optional(
                item_10,
                self.amount(Form1701AmountSection::Schedule3, 11, party),
            );
            let item_16 = sum_present([
                self.amount(Form1701AmountSection::Schedule3, 13, party),
                self.amount(Form1701AmountSection::Schedule3, 14, party),
                self.amount(Form1701AmountSection::Schedule3, 15, party),
            ]);
            let item_17 = item_10.map(|value| round_peso(value * 0.40));
            let item_18 = match deduction {
                Some(Form1701DeductionMethod::Itemized) => subtract_optional(item_12, item_16),
                Some(Form1701DeductionMethod::Osd) => subtract_optional(item_10, item_17),
                None => None,
            };
            let item_22 = sum_present([
                self.amount(Form1701AmountSection::Schedule3, 19, party),
                self.amount(Form1701AmountSection::Schedule3, 20, party),
                self.amount(Form1701AmountSection::Schedule3, 21, party),
            ]);
            let item_23 = add_optional(item_18, item_22);
            let item_24 = add_optional(item_6, item_23);
            let item_25 =
                item_24.map(|income| round_peso(graduated_income_tax(self.taxable_year, income)));
            for (item, value) in [
                (10, item_10),
                (12, item_12),
                (16, item_16),
                (17, item_17),
                (18, item_18),
                (22, item_22),
                (23, item_23),
                (24, item_24),
                (25, item_25),
            ] {
                self.set_amount(Form1701AmountSection::Schedule3, item, party, value);
            }
        } else if rate == Some(Form1701TaxRate::EightPercent) {
            let item_28 = add_optional(
                self.amount(Form1701AmountSection::Schedule3, 26, party),
                self.amount(Form1701AmountSection::Schedule3, 27, party),
            );
            let item_29 = item_28.map(|_| {
                if atc.is_some_and(Form1701Atc::gets_eight_percent_reduction) {
                    250_000.0
                } else {
                    0.0
                }
            });
            let item_30 = subtract_optional(item_28, item_29);
            let item_31 = item_30.map(|income| round_peso(income.max(0.0) * 0.08));
            let item_32 = add_optional(item_7, item_31);
            for (item, value) in [
                (28, item_28),
                (29, item_29),
                (30, item_30),
                (31, item_31),
                (32, item_32),
            ] {
                self.set_amount(Form1701AmountSection::Schedule3, item, party, value);
            }
        }

        let regular_tax_due = match (atc, rate) {
            (Some(Form1701Atc::Ii011), _) => item_7,
            (_, Some(Form1701TaxRate::Graduated)) => {
                self.amount(Form1701AmountSection::Schedule3, 25, party)
            }
            (_, Some(Form1701TaxRate::EightPercent)) => {
                self.amount(Form1701AmountSection::Schedule3, 32, party)
            }
            _ => None,
        };
        self.set_amount(Form1701AmountSection::PartVi, 1, party, regular_tax_due);
        let part_vi_4 = subtract_optional(
            self.amount(Form1701AmountSection::PartVi, 2, party),
            self.amount(Form1701AmountSection::PartVi, 3, party),
        );
        let part_vi_5 = add_optional(regular_tax_due, part_vi_4);
        self.set_amount(Form1701AmountSection::PartVi, 4, party, part_vi_4);
        self.set_amount(Form1701AmountSection::PartVi, 5, party, part_vi_5);

        let credits = sum_present(
            (1..=9).map(|item| self.amount(Form1701AmountSection::PartVii, item, party)),
        );
        self.set_amount(Form1701AmountSection::PartVii, 10, party, credits);

        let relief_3 = add_optional(
            self.amount(Form1701AmountSection::PartViii, 1, party),
            self.amount(Form1701AmountSection::PartViii, 2, party),
        );
        let relief_5 = subtract_optional(
            relief_3,
            self.amount(Form1701AmountSection::PartViii, 4, party),
        );
        let relief_7 = add_optional(
            relief_5,
            self.amount(Form1701AmountSection::PartViii, 6, party),
        );
        let relief_10 = add_optional(
            self.amount(Form1701AmountSection::PartViii, 8, party),
            self.amount(Form1701AmountSection::PartViii, 9, party),
        );
        for (item, value) in [(3, relief_3), (5, relief_5), (7, relief_7), (10, relief_10)] {
            self.set_amount(Form1701AmountSection::PartViii, item, party, value);
        }

        let reconciliation_5 = sum_present(
            (1..=4).map(|item| self.amount(Form1701AmountSection::PartIx, item, party)),
        );
        let reconciliation_10 = sum_present(
            (6..=9).map(|item| self.amount(Form1701AmountSection::PartIx, item, party)),
        );
        let reconciliation_11 = subtract_optional(reconciliation_5, reconciliation_10);
        for (item, value) in [
            (5, reconciliation_5),
            (10, reconciliation_10),
            (11, reconciliation_11),
        ] {
            self.set_amount(Form1701AmountSection::PartIx, item, party, value);
        }

        self.set_amount(Form1701AmountSection::PartIi, 22, party, part_vi_5);
        self.set_amount(Form1701AmountSection::PartIi, 23, party, credits);
        let item_24 = subtract_optional(part_vi_5, credits);
        self.set_amount(Form1701AmountSection::PartIi, 24, party, item_24);
        let item_26 = subtract_optional(
            item_24,
            self.amount(Form1701AmountSection::PartIi, 25, party),
        );
        self.set_amount(Form1701AmountSection::PartIi, 26, party, item_26);
        let penalties = sum_present([
            self.amount(Form1701AmountSection::PartIi, 27, party),
            self.amount(Form1701AmountSection::PartIi, 28, party),
            self.amount(Form1701AmountSection::PartIi, 29, party),
        ]);
        self.set_amount(Form1701AmountSection::PartIi, 30, party, penalties);
        self.set_amount(
            Form1701AmountSection::PartIi,
            31,
            party,
            add_optional(item_26, penalties),
        );
    }

    fn party_atc(&self, party: Form1701Party) -> Option<Form1701Atc> {
        match party {
            Form1701Party::Taxpayer => self.atc,
            Form1701Party::Spouse => self.spouse.enabled.then_some(self.spouse.atc).flatten(),
        }
    }

    fn party_tax_rate(&self, party: Form1701Party) -> Option<Form1701TaxRate> {
        match party {
            Form1701Party::Taxpayer => self.tax_rate,
            Form1701Party::Spouse => self
                .spouse
                .enabled
                .then_some(self.spouse.tax_rate)
                .flatten(),
        }
    }

    fn party_deduction_method(&self, party: Form1701Party) -> Option<Form1701DeductionMethod> {
        match party {
            Form1701Party::Taxpayer => self.deduction_method,
            Form1701Party::Spouse => self
                .spouse
                .enabled
                .then_some(self.spouse.deduction_method)
                .flatten(),
        }
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        let mut errors = self.validate();
        errors.push((
            "submission".to_string(),
            "1701v2018 is manual/external because electronic queue and final-flag semantics are not certified"
                .to_string(),
        ));
        Err(errors)
    }

    pub fn transition_to_submitted(&mut self, _filename: String) -> Result<(), String> {
        Err(
            "1701v2018 cannot transition to Submitted because electronic transport is not certified"
                .to_string(),
        )
    }

    pub fn revert_to_draft(&mut self) -> Result<(), String> {
        if matches!(self.status, FilingStatus::Paid) {
            return Err("A paid 1701 return cannot be reverted directly to Draft".to_string());
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Form1701AmountSection {
    PartIi,
    Schedule2,
    Schedule3,
    Schedule4,
    Schedule6,
    PartVi,
    PartVii,
    PartViii,
    PartIx,
}

impl FormValidator for Form1701Draft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        validate_identity(self, &mut errors);
        validate_choices(self, &mut errors);
        validate_amounts(self, &mut errors);
        validate_payments(self, &mut errors);
        validate_computed_values(self, &mut errors);
        errors
    }
}

impl TypedBirForm for Form1701Draft {
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
        Form1701Draft::recompute(self);
    }

    fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        Form1701Draft::to_bir_field_map(self)
    }
}

fn validate_identity(draft: &Form1701Draft, errors: &mut Vec<(String, String)>) {
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
            "January 2018 Form 1701 supports taxable years 2018 onward".to_string(),
        ));
    }
    if !(1..=12).contains(&draft.period_end_month) {
        errors.push((
            "period_end_month".to_string(),
            "Period-end month must be between 1 and 12".to_string(),
        ));
    } else if !draft.is_short_period && draft.period_end_month != 12 {
        errors.push((
            "period_end_month".to_string(),
            "A non-short-period annual return must end in December".to_string(),
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
        && (draft.rdo_code.len() != 3 || !draft.rdo_code.chars().all(|ch| ch.is_ascii_digit()))
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
    if !draft.contact_number.trim().is_empty() && !validate_ph_phone(&draft.contact_number) {
        errors.push((
            "contact_number".to_string(),
            "Contact number must be a valid Philippine landline or mobile number".to_string(),
        ));
    }
    validate_optional_date("date_of_birth", &draft.date_of_birth, errors);
    if draft.number_of_attachments.is_some_and(|value| value > 99) {
        errors.push((
            "number_of_attachments".to_string(),
            "Item 33 supports at most two digits".to_string(),
        ));
    }
}

fn validate_choices(draft: &Form1701Draft, errors: &mut Vec<(String, String)>) {
    if draft.taxpayer_type.is_none() {
        errors.push((
            "taxpayer_type".to_string(),
            "Select Item 6 taxpayer type".to_string(),
        ));
    }
    if draft.atc.is_none() {
        errors.push(("atc".to_string(), "Select Item 7 ATC".to_string()));
    }
    validate_rate_choice(
        "taxpayer",
        draft.atc,
        draft.tax_rate,
        draft.deduction_method,
        errors,
    );
    for (field, item, choice) in [
        (
            "claims_foreign_tax_credits",
            13,
            draft.claims_foreign_tax_credits,
        ),
        ("has_exempt_income", 19, draft.has_exempt_income),
        ("has_special_rate_income", 20, draft.has_special_rate_income),
    ] {
        if choice.is_none() {
            errors.push((field.to_string(), format!("Answer Item {item} Yes or No")));
        }
    }
    if draft.claims_foreign_tax_credits == Some(true) && draft.foreign_tax_number.trim().is_empty()
    {
        errors.push((
            "foreign_tax_number".to_string(),
            "Item 14 is required when Item 13 is Yes".to_string(),
        ));
    }
    if draft.has_exempt_income == Some(true) || draft.has_special_rate_income == Some(true) {
        errors.push((
            "part_x".to_string(),
            "This return requires Part X/attachment schedules, which are preserved on import but are not yet modeled for safe editing"
                .to_string(),
        ));
    }
    if draft.civil_status == Some(Form1701CivilStatus::Married) {
        if draft.spouse_has_income.is_none() {
            errors.push((
                "spouse_has_income".to_string(),
                "Answer Item 17 for a married filer".to_string(),
            ));
        }
        if draft.spouse_has_income == Some(true) && draft.joint_filing_status.is_none() {
            errors.push((
                "joint_filing_status".to_string(),
                "Select Joint or Separate filing in Item 18".to_string(),
            ));
        }
    }
    if draft.spouse.enabled {
        let spouse_digits = digits(&draft.spouse.tin);
        if !(12..=14).contains(&spouse_digits.len()) {
            errors.push((
                "spouse_tin".to_string(),
                "Spouse TIN must contain 12 to 14 digits".to_string(),
            ));
        }
        if draft.spouse.name.trim().is_empty() {
            errors.push((
                "spouse_name".to_string(),
                "Spouse name is required".to_string(),
            ));
        }
        if draft.spouse.filer_type.is_none() {
            errors.push(("spouse_type".to_string(), "Select spouse type".to_string()));
        }
        if draft.spouse.atc.is_none() {
            errors.push(("spouse_atc".to_string(), "Select spouse ATC".to_string()));
        }
        validate_rate_choice(
            "spouse",
            draft.spouse.atc,
            draft.spouse.tax_rate,
            draft.spouse.deduction_method,
            errors,
        );
        if draft.spouse.claims_foreign_tax_credits.is_none() {
            errors.push((
                "spouse_claims_foreign_tax_credits".to_string(),
                "Answer spouse Item 8 Yes or No".to_string(),
            ));
        }
        if draft.spouse.has_exempt_income == Some(true)
            || draft.spouse.has_special_rate_income == Some(true)
        {
            errors.push((
                "spouse_part_x".to_string(),
                "Spouse exempt/special-rate income requires the unsupported Part X attachment editor"
                    .to_string(),
            ));
        }
    }
}

fn validate_rate_choice(
    prefix: &str,
    atc: Option<Form1701Atc>,
    rate: Option<Form1701TaxRate>,
    deduction: Option<Form1701DeductionMethod>,
    errors: &mut Vec<(String, String)>,
) {
    if atc == Some(Form1701Atc::Ii011) {
        if rate.is_some() || deduction.is_some() {
            errors.push((
                format!("{prefix}_tax_rate"),
                "II011 compensation income does not use the business-rate/deduction choices"
                    .to_string(),
            ));
        }
        return;
    }
    if rate.is_none() {
        errors.push((
            format!("{prefix}_tax_rate"),
            "Select the income tax rate".to_string(),
        ));
    }
    if let (Some(atc), Some(rate)) = (atc, rate)
        && atc.tax_rate().is_some_and(|expected| expected != rate)
    {
        errors.push((
            format!("{prefix}_atc"),
            format!("ATC {} does not match {}", atc.code(), rate.label()),
        ));
    }
    match rate {
        Some(Form1701TaxRate::Graduated) if deduction.is_none() => errors.push((
            format!("{prefix}_deduction_method"),
            "Graduated rates require Itemized or OSD".to_string(),
        )),
        Some(Form1701TaxRate::EightPercent) if deduction.is_some() => errors.push((
            format!("{prefix}_deduction_method"),
            "The deduction-method choice does not apply to the 8% rate".to_string(),
        )),
        _ => {}
    }
}

fn validate_amounts(draft: &Form1701Draft, errors: &mut Vec<(String, String)>) {
    for (section_name, table) in [
        ("part_ii", &draft.computations.part_ii),
        ("schedule_2", &draft.computations.schedule_2),
        ("schedule_3", &draft.computations.schedule_3),
        ("schedule_4", &draft.computations.schedule_4),
        ("schedule_6", &draft.computations.schedule_6_summary),
        ("part_vi", &draft.computations.part_vi),
        ("part_vii", &draft.computations.part_vii),
        ("part_viii", &draft.computations.part_viii),
        ("part_ix", &draft.computations.part_ix),
    ] {
        for (item, pair) in table {
            validate_pair(section_name, *item, pair, errors);
        }
    }
    for (index, pair) in draft.computations.schedule_4_item_17.iter().enumerate() {
        validate_pair("schedule_4_item_17", (index + 1) as u8, pair, errors);
    }
    for (party_name, rows) in [
        ("taxpayer", &draft.computations.schedule_5_taxpayer),
        ("spouse", &draft.computations.schedule_5_spouse),
    ] {
        for (index, row) in rows.iter().enumerate() {
            validate_optional_whole_peso(
                format!("schedule_5_{party_name}_{}", index + 1),
                row.amount,
                errors,
            );
        }
    }
    for (party_name, rows) in [
        ("taxpayer", &draft.computations.schedule_6_taxpayer_nolco),
        ("spouse", &draft.computations.schedule_6_spouse_nolco),
    ] {
        for (index, row) in rows.iter().enumerate() {
            for (column, value) in [
                ("amount", row.amount),
                ("previous", row.applied_previous_years),
                ("expired", row.expired),
                ("current", row.applied_current_year),
                ("unapplied", row.unapplied),
            ] {
                validate_optional_whole_peso(
                    format!("nolco_{party_name}_{}_{}", index + 1, column),
                    value,
                    errors,
                );
            }
        }
    }
    for (index, employer) in draft.employers.iter().enumerate() {
        validate_optional_whole_peso(
            format!("employer_{}_compensation", index + 1),
            employer.compensation_income,
            errors,
        );
        validate_optional_whole_peso(
            format!("employer_{}_withheld", index + 1),
            employer.tax_withheld,
            errors,
        );
        if employer.owner.is_some()
            && (employer.employer_name.trim().is_empty()
                || digits(&employer.employer_tin).is_empty())
        {
            errors.push((
                format!("employer_{}", index + 1),
                "Selected employer rows require both employer name and TIN".to_string(),
            ));
        }
    }

    for (party, rate) in [
        (Form1701Party::Taxpayer, draft.tax_rate),
        (
            Form1701Party::Spouse,
            draft
                .spouse
                .enabled
                .then_some(draft.spouse.tax_rate)
                .flatten(),
        ),
    ] {
        match rate {
            Some(Form1701TaxRate::Graduated) => {
                for item in [26, 27] {
                    if draft
                        .amount(Form1701AmountSection::Schedule3, item, party)
                        .is_some_and(|value| value != 0.0)
                    {
                        errors.push((
                            format!("schedule_3_{item}_{party:?}"),
                            "8% schedule inputs must be blank for a graduated-rate filer"
                                .to_string(),
                        ));
                    }
                }
            }
            Some(Form1701TaxRate::EightPercent) => {
                for item in [8, 9, 11, 13, 14, 15, 19, 20, 21] {
                    if draft
                        .amount(Form1701AmountSection::Schedule3, item, party)
                        .is_some_and(|value| value != 0.0)
                    {
                        errors.push((
                            format!("schedule_3_{item}_{party:?}"),
                            "Graduated-rate schedule inputs must be blank for an 8% filer"
                                .to_string(),
                        ));
                    }
                }
            }
            None => {}
        }
    }

    for party in [Form1701Party::Taxpayer, Form1701Party::Spouse] {
        if let (Some(installment), Some(tax_due)) = (
            draft.amount(Form1701AmountSection::PartIi, 25, party),
            draft.amount(Form1701AmountSection::PartIi, 22, party),
        ) && installment > tax_due.max(0.0) * 0.5
        {
            errors.push((
                format!("item_25_{party:?}"),
                "Item 25 cannot exceed 50% of Item 22".to_string(),
            ));
        }
    }

    let aggregate = draft.computations.part_ii_item_32_aggregate;
    validate_optional_whole_peso("part_ii_item_32".to_string(), aggregate, errors);
    if aggregate.is_some_and(|value| value < 0.0)
        && draft.overpayment_disposition == Form1701OverpaymentDisposition::None
    {
        errors.push((
            "overpayment_disposition".to_string(),
            "Choose one irrevocable overpayment disposition when Item 32 is negative".to_string(),
        ));
    } else if aggregate.is_some_and(|value| value >= 0.0)
        && draft.overpayment_disposition != Form1701OverpaymentDisposition::None
    {
        errors.push((
            "overpayment_disposition".to_string(),
            "Overpayment disposition must be blank when Item 32 is not an overpayment".to_string(),
        ));
    }
}

fn validate_pair(
    section: &str,
    item: u8,
    pair: &Form1701AmountPair,
    errors: &mut Vec<(String, String)>,
) {
    validate_optional_whole_peso(format!("{section}_{item}_taxpayer"), pair.taxpayer, errors);
    validate_optional_whole_peso(format!("{section}_{item}_spouse"), pair.spouse, errors);
}

fn validate_optional_whole_peso(
    field: String,
    value: Option<f64>,
    errors: &mut Vec<(String, String)>,
) {
    let Some(value) = value else {
        return;
    };
    if !value.is_finite() {
        errors.push((field, "Amount must be finite".to_string()));
    } else if (value - value.round()).abs() > 0.000_001 {
        errors.push((
            field,
            "Form 1701 requires whole-peso amounts; do not enter centavos".to_string(),
        ));
    }
}

fn validate_payments(draft: &Form1701Draft, errors: &mut Vec<(String, String)>) {
    for (item, row) in [
        (34, &draft.payment_details.item_34_cash_or_bank_debit_memo),
        (35, &draft.payment_details.item_35_check),
        (36, &draft.payment_details.item_36_tax_debit_memo),
        (37, &draft.payment_details.item_37_others),
    ] {
        validate_optional_whole_peso(format!("payment_{item}_amount"), row.amount, errors);
        validate_optional_date(&format!("payment_{item}_date"), &row.date, errors);
    }
    if !draft
        .payment_details
        .item_36_tax_debit_memo
        .drawee_bank_or_agency
        .trim()
        .is_empty()
    {
        errors.push((
            "payment_36_agency".to_string(),
            "The reviewed 1701 XML schema has no drawee-bank/agency field for Item 36".to_string(),
        ));
    }
    if draft.payment_details.item_37_others.amount.is_some()
        && draft
            .payment_details
            .item_37_others_description
            .trim()
            .is_empty()
    {
        errors.push((
            "payment_37_description".to_string(),
            "Specify the Item 37 other payment type".to_string(),
        ));
    }
}

fn validate_computed_values(draft: &Form1701Draft, errors: &mut Vec<(String, String)>) {
    let mut expected = draft.clone();
    expected.recompute();
    for (section, items) in [
        (Form1701AmountSection::PartIi, &[22, 23, 24, 26, 30, 31][..]),
        (Form1701AmountSection::Schedule2, &[4, 6, 7][..]),
        (
            Form1701AmountSection::Schedule3,
            &[10, 12, 16, 17, 18, 22, 23, 24, 25, 28, 29, 30, 31, 32][..],
        ),
        (Form1701AmountSection::Schedule4, &[18][..]),
        (Form1701AmountSection::Schedule6, &[3][..]),
        (Form1701AmountSection::PartVi, &[1, 4, 5][..]),
        (Form1701AmountSection::PartVii, &[5, 10][..]),
        (Form1701AmountSection::PartViii, &[3, 5, 7, 10][..]),
        (Form1701AmountSection::PartIx, &[5, 10, 11][..]),
    ] {
        for item in items {
            for party in [Form1701Party::Taxpayer, Form1701Party::Spouse] {
                if !amounts_equal(
                    draft.amount(section, *item, party),
                    expected.amount(section, *item, party),
                ) {
                    errors.push((
                        format!("computed_{section:?}_{item}_{party:?}"),
                        "Stored computed value does not match the printed-form arithmetic"
                            .to_string(),
                    ));
                }
            }
        }
    }
    if !amounts_equal(
        draft.computations.part_ii_item_32_aggregate,
        expected.computations.part_ii_item_32_aggregate,
    ) {
        errors.push((
            "computed_part_ii_32".to_string(),
            "Stored Item 32 does not equal Items 31A plus 31B".to_string(),
        ));
    }
}

fn amounts_equal(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => (left - right).abs() < 0.005,
        // The reviewed eBIRForms save materializes some computed blank cells
        // as 0.00. Treat that transport placeholder as arithmetically
        // equivalent while retaining None versus Some(0) in the model and XML.
        (None, Some(value)) | (Some(value), None) => value.abs() < 0.005,
    }
}

fn validate_optional_date(field: &str, value: &str, errors: &mut Vec<(String, String)>) {
    if value.trim().is_empty() {
        return;
    }
    if chrono::NaiveDate::parse_from_str(value.trim(), "%m/%d/%Y").is_err() {
        errors.push((field.to_string(), "Date must use MM/DD/YYYY".to_string()));
    }
}

fn profile_taxpayer_type(
    profile: &TaxpayerProfile,
    atc: Option<Form1701Atc>,
) -> Option<Form1701TaxpayerType> {
    match profile.taxpayer_type {
        TaxpayerType::Estate => Some(Form1701TaxpayerType::Estate),
        TaxpayerType::Trust => Some(Form1701TaxpayerType::Trust),
        TaxpayerType::Individual => match atc {
            Some(Form1701Atc::Ii011) => Some(Form1701TaxpayerType::CompensationEarner),
            Some(Form1701Atc::Ii012 | Form1701Atc::Ii015) => {
                Some(Form1701TaxpayerType::SingleProprietor)
            }
            Some(Form1701Atc::Ii014 | Form1701Atc::Ii017) => {
                Some(Form1701TaxpayerType::Professional)
            }
            Some(Form1701Atc::Ii013 | Form1701Atc::Ii016) | None => {
                match profile.effective_classification() {
                    Some(TaxClassification::PurelyCompensation) => {
                        Some(Form1701TaxpayerType::CompensationEarner)
                    }
                    _ => None,
                }
            }
        },
        TaxpayerType::Corporation | TaxpayerType::Partnership | TaxpayerType::Cooperative => None,
    }
}

fn digits(value: &str) -> String {
    value.chars().filter(|ch| ch.is_ascii_digit()).collect()
}

fn round_peso(value: f64) -> f64 {
    value.round()
}

fn sum_present(values: impl IntoIterator<Item = Option<f64>>) -> Option<f64> {
    let mut any = false;
    let sum = values.into_iter().flatten().fold(0.0, |sum, value| {
        any = true;
        sum + value
    });
    any.then(|| round_peso(sum))
}

fn add_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (None, None) => None,
        (left, right) => Some(round_peso(left.unwrap_or(0.0) + right.unwrap_or(0.0))),
    }
}

fn subtract_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    left.map(|left| round_peso(left - right.unwrap_or(0.0)))
}

fn subtract_many_optional(
    left: Option<f64>,
    rights: impl IntoIterator<Item = Option<f64>>,
) -> Option<f64> {
    left.map(|left| round_peso(left - rights.into_iter().flatten().sum::<f64>()))
}

/// Printed January 2018 Form 1701 Tables 1 and 2.
pub fn graduated_income_tax(taxable_year: u16, taxable_income: f64) -> f64 {
    let income = taxable_income.max(0.0);
    if taxable_year <= 2022 {
        if income <= 250_000.0 {
            0.0
        } else if income <= 400_000.0 {
            (income - 250_000.0) * 0.20
        } else if income <= 800_000.0 {
            30_000.0 + (income - 400_000.0) * 0.25
        } else if income <= 2_000_000.0 {
            130_000.0 + (income - 800_000.0) * 0.30
        } else if income <= 8_000_000.0 {
            490_000.0 + (income - 2_000_000.0) * 0.32
        } else {
            2_410_000.0 + (income - 8_000_000.0) * 0.35
        }
    } else if income <= 250_000.0 {
        0.0
    } else if income <= 400_000.0 {
        (income - 250_000.0) * 0.15
    } else if income <= 800_000.0 {
        22_500.0 + (income - 400_000.0) * 0.20
    } else if income <= 2_000_000.0 {
        102_500.0 + (income - 800_000.0) * 0.25
    } else if income <= 8_000_000.0 {
        402_500.0 + (income - 2_000_000.0) * 0.30
    } else {
        2_202_500.0 + (income - 8_000_000.0) * 0.35
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printed_tax_tables_change_after_2022() {
        assert_eq!(graduated_income_tax(2022, 600_000.0), 80_000.0);
        assert_eq!(graduated_income_tax(2023, 600_000.0), 62_500.0);
        assert_eq!(graduated_income_tax(2025, -1.0), 0.0);
    }

    #[test]
    fn printed_graduated_osd_arithmetic_preserves_overpayment_sign() {
        let mut draft = Form1701Draft {
            taxable_year: 2025,
            atc: Some(Form1701Atc::Ii012),
            tax_rate: Some(Form1701TaxRate::Graduated),
            deduction_method: Some(Form1701DeductionMethod::Osd),
            ..Form1701Draft::default()
        };
        draft.set_amount(
            Form1701AmountSection::Schedule3,
            8,
            Form1701Party::Taxpayer,
            Some(1_000_000.0),
        );
        draft.set_amount(
            Form1701AmountSection::PartVii,
            1,
            Form1701Party::Taxpayer,
            Some(200_000.0),
        );
        draft.recompute();

        assert_eq!(
            draft.amount(
                Form1701AmountSection::Schedule3,
                17,
                Form1701Party::Taxpayer
            ),
            Some(400_000.0)
        );
        assert_eq!(
            draft.amount(
                Form1701AmountSection::Schedule3,
                25,
                Form1701Party::Taxpayer
            ),
            Some(62_500.0)
        );
        assert_eq!(
            draft.amount(Form1701AmountSection::PartIi, 24, Form1701Party::Taxpayer),
            Some(-137_500.0)
        );
        assert_eq!(
            draft.computations.part_ii_item_32_aggregate,
            Some(-137_500.0)
        );
    }

    #[test]
    fn blank_and_explicit_zero_are_distinct() {
        let mut draft = Form1701Draft::default();
        assert_eq!(
            draft.amount(Form1701AmountSection::PartVii, 1, Form1701Party::Taxpayer),
            None
        );
        draft.set_amount(
            Form1701AmountSection::PartVii,
            1,
            Form1701Party::Taxpayer,
            Some(0.0),
        );
        assert_eq!(
            draft.amount(Form1701AmountSection::PartVii, 1, Form1701Party::Taxpayer),
            Some(0.0)
        );
    }

    #[test]
    fn queue_boundary_fails_closed() {
        let mut draft = Form1701Draft::default();
        let errors = draft
            .transition_to_queued()
            .expect_err("queueing must be disabled");
        assert!(errors.iter().any(|(field, _)| field == "submission"));
    }
}
