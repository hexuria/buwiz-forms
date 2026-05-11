//! Registry loader — parses form definitions with temporal boundaries.

use crate::forms::registry::FilingFrequency;
use crate::profile::{ExciseTaxCategory, TaxClassification, TaxpayerType};
use crate::temporal::citations::LegalCitation;
use serde::{Deserialize, Serialize};

/// The regulatory status of a form in the registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RegulatoryStatus {
    Mandatory,
    #[default]
    Active,
    Recommended,
    Optional,
    Transitional,
    Legacy,
    Abolished,
}

/// Which withholding type triggers a form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WithholdingTrigger {
    Compensation,
    Expanded,
    Final,
}

/// A form definition with temporal boundaries.
#[derive(Debug, Clone)]
pub struct TemporalFormDef {
    pub code: String,
    pub title: String,
    pub category: String,
    pub frequency: FilingFrequency,
    pub taxpayer_types: Vec<TaxpayerType>,
    pub classifications: Vec<TaxClassification>,
    pub active_from_year: u16,
    pub active_until_year: Option<u16>,
    pub regulatory_status: RegulatoryStatus,
    pub requires_vat: Option<bool>,
    pub withholding_trigger: Option<WithholdingTrigger>,
    pub excise_category: Option<ExciseTaxCategory>,
    pub exclusive_group: Option<String>,
    pub exclusive_priority: u8,
    pub legal_basis: Vec<LegalCitation>,
}

/// Build the complete temporal form registry (hardcoded for now, TOML later).
pub fn load_registry() -> Vec<TemporalFormDef> {
    use ExciseTaxCategory::*;
    use FilingFrequency::*;
    use WithholdingTrigger as WT;

    // Use fully-qualified paths to avoid Corporation name collision
    let tp = TaxpayerType::Individual; // just to bring the module in scope
    let _ = tp;

    let all_individual = vec![TaxpayerType::Individual];
    let all_corp = vec![TaxpayerType::Corporation, TaxpayerType::Partnership, TaxpayerType::Cooperative];
    let all_entities = vec![TaxpayerType::Individual, TaxpayerType::Corporation, TaxpayerType::Partnership];
    let all_entities_plus = vec![TaxpayerType::Individual, TaxpayerType::Corporation, TaxpayerType::Partnership, TaxpayerType::Cooperative, TaxpayerType::Estate, TaxpayerType::Trust];

    vec![
        // ═══ Payment ═══
        TemporalFormDef {
            code: "0605".into(), title: "Payment Form".into(),
            category: "Payment".into(), frequency: OpenEnded,
            taxpayer_types: all_entities_plus.clone(),
            classifications: vec![], active_from_year: 1997, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        // ═══ Withholding Tax ═══
        wh("1601C", "Monthly Remittance Return of Income Taxes Withheld on Compensation",
           Monthly, 1997, None, WT::Compensation),
        wh("1601E", "Monthly Remittance Return of Creditable Income Taxes Withheld (Expanded)",
           Monthly, 1997, Some(2017), WT::Expanded),
        wh("0619E", "Monthly Remittance Form of Creditable Income Taxes Withheld (Expanded)",
           Monthly, 2018, None, WT::Expanded),
        wh("1601EQ", "Quarterly Remittance Return of Creditable Income Taxes Withheld (Expanded)",
           Quarterly, 2018, None, WT::Expanded),
        wh("1601F", "Monthly Remittance Return of Final Income Tax Withheld",
           Monthly, 1997, None, WT::Final),
        wh("1602", "Monthly Remittance Return of Final Income Taxes Withheld on Interest",
           Monthly, 1997, None, WT::Final),
        wh("1603", "Quarterly Remittance Return of Final Income Taxes Withheld",
           Quarterly, 1997, None, WT::Final),
        wh("1604CF", "Annual Information Return of Income Taxes Withheld on Compensation and Final",
           Annual, 1997, None, WT::Compensation),
        wh("1604E", "Annual Information Return of Creditable Income Taxes Withheld (Expanded)",
           Annual, 1997, None, WT::Expanded),
        wh("1600", "Monthly Remittance Return of VAT and Other Percentage Taxes Withheld",
           Monthly, 1997, None, WT::Compensation),
        wh("1600WP", "Remittance Return of Percentage Tax on Winnings and Prizes",
           Monthly, 1997, None, WT::Final),
        // ═══ Income Tax — Individual ═══
        TemporalFormDef {
            code: "1700".into(), title: "Annual Income Tax Return (Purely Compensation)".into(),
            category: "Income Tax".into(), frequency: Annual,
            taxpayer_types: all_individual.clone(),
            classifications: vec![TaxClassification::PurelyCompensation],
            active_from_year: 1997, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "1701Q".into(), title: "Quarterly Income Tax Return for Individuals, Estates and Trusts".into(),
            category: "Income Tax".into(), frequency: Quarterly,
            taxpayer_types: vec![TaxpayerType::Individual, TaxpayerType::Estate, TaxpayerType::Trust],
            classifications: vec![TaxClassification::ProfessionalOrFreelancer, TaxClassification::SoleProprietorNonVat, TaxClassification::SoleProprietorVat, TaxClassification::MixedIncome, TaxClassification::EstateOrTrust],
            active_from_year: 1997, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "1701".into(), title: "Annual Income Tax Return for Individuals, Estates and Trusts".into(),
            category: "Income Tax".into(), frequency: Annual,
            taxpayer_types: vec![TaxpayerType::Individual, TaxpayerType::Estate, TaxpayerType::Trust],
            classifications: vec![TaxClassification::ProfessionalOrFreelancer, TaxClassification::SoleProprietorNonVat, TaxClassification::SoleProprietorVat, TaxClassification::MixedIncome, TaxClassification::EstateOrTrust],
            active_from_year: 1997, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "1701A".into(), title: "Annual Income Tax Return (8% / OSD)".into(),
            category: "Income Tax".into(), frequency: Annual,
            taxpayer_types: all_individual.clone(),
            classifications: vec![TaxClassification::ProfessionalOrFreelancer, TaxClassification::SoleProprietorNonVat, TaxClassification::SoleProprietorVat],
            active_from_year: 2018, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "1701MS".into(), title: "Annual Income Tax Return (Simplified) for Micro/Small Taxpayers".into(),
            category: "Income Tax".into(), frequency: Annual,
            taxpayer_types: all_individual.clone(),
            classifications: vec![TaxClassification::ProfessionalOrFreelancer, TaxClassification::SoleProprietorNonVat, TaxClassification::SoleProprietorVat],
            active_from_year: 2024, active_until_year: None,
            regulatory_status: RegulatoryStatus::Recommended, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        // ═══ Income Tax — Corporate ═══
        TemporalFormDef {
            code: "1702Q".into(), title: "Quarterly Income Tax Return for Corporations, Partnerships and Cooperatives".into(),
            category: "Income Tax".into(), frequency: Quarterly,
            taxpayer_types: all_corp.clone(),
            classifications: vec![TaxClassification::Corporation, TaxClassification::CooperativeExempt, TaxClassification::CooperativeTaxable, TaxClassification::CooperativeMixed],
            active_from_year: 1997, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "1702".into(), title: "Annual Income Tax Return for Corporations (Pre-TRAIN)".into(),
            category: "Income Tax".into(), frequency: Annual,
            taxpayer_types: all_corp.clone(),
            classifications: vec![TaxClassification::Corporation, TaxClassification::CooperativeExempt, TaxClassification::CooperativeTaxable, TaxClassification::CooperativeMixed],
            active_from_year: 1997, active_until_year: Some(2017),
            regulatory_status: RegulatoryStatus::Abolished, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "1702RT".into(), title: "Annual Income Tax Return — Regular Taxable".into(),
            category: "Income Tax".into(), frequency: Annual,
            taxpayer_types: all_corp.clone(),
            classifications: vec![TaxClassification::Corporation, TaxClassification::CooperativeTaxable],
            active_from_year: 2018, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: Some("ANNUAL_CORPORATE_ITR".into()), exclusive_priority: 3, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "1702EX".into(), title: "Annual Income Tax Return — Tax-Exempt".into(),
            category: "Income Tax".into(), frequency: Annual,
            taxpayer_types: all_corp.clone(),
            classifications: vec![TaxClassification::CooperativeExempt],
            active_from_year: 2018, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: Some("ANNUAL_CORPORATE_ITR".into()), exclusive_priority: 2, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "1702MX".into(), title: "Annual Income Tax Return — Mixed Income".into(),
            category: "Income Tax".into(), frequency: Annual,
            taxpayer_types: all_corp.clone(),
            classifications: vec![TaxClassification::CooperativeMixed],
            active_from_year: 2018, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: Some("ANNUAL_CORPORATE_ITR".into()), exclusive_priority: 1, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "1704".into(), title: "Improperly Accumulated Earnings Tax Return".into(),
            category: "Income Tax".into(), frequency: Annual,
            taxpayer_types: vec![TaxpayerType::Corporation],
            classifications: vec![TaxClassification::Corporation],
            active_from_year: 1997, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        // ═══ VAT ═══
        TemporalFormDef {
            code: "2550M".into(), title: "Monthly Value-Added Tax Declaration".into(),
            category: "Value-Added Tax".into(), frequency: Monthly,
            taxpayer_types: all_entities.clone(),
            classifications: vec![TaxClassification::SoleProprietorVat, TaxClassification::Corporation, TaxClassification::MixedIncome],
            active_from_year: 1997, active_until_year: None,
            regulatory_status: RegulatoryStatus::Optional,
            requires_vat: Some(true),
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "2550Q".into(), title: "Quarterly Value-Added Tax Return".into(),
            category: "Value-Added Tax".into(), frequency: Quarterly,
            taxpayer_types: all_entities.clone(),
            classifications: vec![TaxClassification::SoleProprietorVat, TaxClassification::Corporation, TaxClassification::MixedIncome, TaxClassification::ProfessionalOrFreelancer],
            active_from_year: 1997, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory,
            requires_vat: Some(true),
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        // ═══ Percentage Tax ═══
        TemporalFormDef {
            code: "2551M".into(), title: "Monthly Percentage Tax Return".into(),
            category: "Percentage Tax".into(), frequency: Monthly,
            taxpayer_types: all_entities.clone(),
            classifications: vec![],
            active_from_year: 1997, active_until_year: Some(2017),
            regulatory_status: RegulatoryStatus::Abolished,
            requires_vat: Some(false),
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "2551Q".into(), title: "Quarterly Percentage Tax Return".into(),
            category: "Percentage Tax".into(), frequency: Quarterly,
            taxpayer_types: all_entities.clone(),
            classifications: vec![TaxClassification::SoleProprietorNonVat, TaxClassification::ProfessionalOrFreelancer, TaxClassification::MixedIncome, TaxClassification::Corporation],
            active_from_year: 2018, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory,
            requires_vat: Some(false),
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "2552".into(), title: "Percentage Tax Return on Shares of Stock Transactions".into(),
            category: "Percentage Tax".into(), frequency: Quarterly,
            taxpayer_types: all_entities.clone(), classifications: vec![],
            active_from_year: 1997, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        TemporalFormDef {
            code: "2553".into(), title: "Percentage Tax Payable Under Special Laws".into(),
            category: "Percentage Tax".into(), frequency: Quarterly,
            taxpayer_types: all_entities.clone(), classifications: vec![],
            active_from_year: 1997, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        // ═══ DST ═══
        TemporalFormDef {
            code: "2000".into(), title: "Documentary Stamp Tax Declaration/Return".into(),
            category: "Documentary Stamp Tax".into(), frequency: OpenEnded,
            taxpayer_types: all_entities.clone(), classifications: vec![],
            active_from_year: 1997, active_until_year: None,
            regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
            withholding_trigger: None, excise_category: None,
            exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
        },
        // ═══ Excise Tax ═══
        excise("2200A", "Excise Tax Return for Alcohol Products", Alcohol),
        excise("2200AN", "Excise Tax Return for Automobiles and Non-Essential Goods", AutomobilesAndNonEssential),
        excise("2200M", "Excise Tax Return for Mineral Products", Mineral),
        excise("2200P", "Excise Tax Return for Petroleum Products", Petroleum),
        excise("2200T", "Excise Tax Return for Tobacco Products", Tobacco),
    ]
}

// Helper: create a withholding form definition.
fn wh(code: &str, title: &str, freq: FilingFrequency, from: u16, until: Option<u16>, trigger: WithholdingTrigger) -> TemporalFormDef {
    TemporalFormDef {
        code: code.into(), title: title.into(),
        category: "Withholding Tax".into(), frequency: freq,
        taxpayer_types: vec![TaxpayerType::Individual, TaxpayerType::Corporation, TaxpayerType::Partnership],
        classifications: vec![],
        active_from_year: from, active_until_year: until,
        regulatory_status: if until.is_some() { RegulatoryStatus::Abolished } else { RegulatoryStatus::Mandatory },
        requires_vat: None, withholding_trigger: Some(trigger), excise_category: None,
        exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
    }
}

// Helper: create an excise tax form definition.
fn excise(code: &str, title: &str, cat: ExciseTaxCategory) -> TemporalFormDef {
    TemporalFormDef {
        code: code.into(), title: title.into(),
        category: "Excise Tax".into(), frequency: FilingFrequency::Monthly,
        taxpayer_types: vec![TaxpayerType::Individual, TaxpayerType::Corporation, TaxpayerType::Partnership],
        classifications: vec![],
        active_from_year: 1997, active_until_year: None,
        regulatory_status: RegulatoryStatus::Mandatory, requires_vat: None,
        withholding_trigger: None, excise_category: Some(cat),
        exclusive_group: None, exclusive_priority: 0, legal_basis: vec![],
    }
}
