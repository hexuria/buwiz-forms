//! Form type registry — defines all supported BIR forms and their filing rules.

use crate::profile::TaxpayerType;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FilingFrequency {
    /// Filed 4 times per year — one per quarter. Counter: 0/4.
    Quarterly,
    /// Filed once per year. Counter: 0/1.
    Annual,
    /// Filed 12 times per year — one per month. Counter: 0/12.
    Monthly,
    /// Filed as many times as needed (e.g. payment forms). No counter.
    OpenEnded,
}

#[derive(Debug, Clone)]
pub struct FormDefinition {
    pub code: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub frequency: FilingFrequency,
    pub taxpayer_types: &'static [TaxpayerType],
    /// None = applies regardless of VAT status.
    /// Some(true) = only for VAT-registered taxpayers.
    /// Some(false) = only for non-VAT taxpayers.
    pub requires_vat: Option<bool>,
    /// If true, this form only applies to taxpayers with employees
    /// (withholding agent forms).
    pub requires_employees: bool,
    pub is_deprecated: bool,
}

pub const FORM_REGISTRY: &[FormDefinition] = &[
    // ═══════════════════════════════════════════
    // Payment
    // ═══════════════════════════════════════════
    FormDefinition {
        code: "0605",
        title: "Payment Form",
        category: "Payment",
        frequency: FilingFrequency::OpenEnded,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    // ═══════════════════════════════════════════
    // Withholding Tax (employer / withholding agent forms)
    // ═══════════════════════════════════════════
    FormDefinition {
        code: "1600",
        title: "Monthly Remittance Return of VAT and Other Percentage Taxes Withheld",
        category: "Withholding Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: true,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1600WP",
        title: "Remittance Return of Percentage Tax on Winnings and Prizes",
        category: "Withholding Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: true,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1601C",
        title: "Monthly Remittance Return of Income Taxes Withheld on Compensation",
        category: "Withholding Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: true,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1601E",
        title: "Monthly Remittance Return of Creditable Income Taxes Withheld (Expanded)",
        category: "Withholding Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: true,
        is_deprecated: true,
    },
    FormDefinition {
        code: "1601F",
        title: "Monthly Remittance Return of Final Income Tax Withheld",
        category: "Withholding Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: true,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1602",
        title: "Monthly Remittance Return of Final Income Taxes Withheld",
        category: "Withholding Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: true,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1603",
        title: "Quarterly Remittance Return of Final Income Taxes Withheld",
        category: "Withholding Tax",
        frequency: FilingFrequency::Quarterly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: true,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1604CF",
        title: "Annual Information Return of Income Taxes Withheld on Compensation",
        category: "Withholding Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: true,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1604E",
        title: "Annual Information Return of Creditable Income Taxes Withheld",
        category: "Withholding Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: true,
        is_deprecated: false,
    },
    // ═══════════════════════════════════════════
    // Income Tax — Individual
    // ═══════════════════════════════════════════
    FormDefinition {
        code: "1700",
        title: "Annual Income Tax Return (Purely Compensation)",
        category: "Income Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[TaxpayerType::Individual],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1701Q",
        title: "Quarterly Income Tax Return for Individuals, Estates and Trusts",
        category: "Income Tax",
        frequency: FilingFrequency::Quarterly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Estate,
            TaxpayerType::Trust,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1701",
        title: "Annual Income Tax Return for Individuals, Estates and Trusts",
        category: "Income Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Estate,
            TaxpayerType::Trust,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1701A",
        title: "Annual Income Tax Return (8% / OSD)",
        category: "Income Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[TaxpayerType::Individual],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    // ═══════════════════════════════════════════
    // Income Tax — Corporation / Partnership
    // ═══════════════════════════════════════════
    FormDefinition {
        code: "1702Q",
        title: "Quarterly Income Tax Return for Corporations, Partnerships and Cooperatives",
        category: "Income Tax",
        frequency: FilingFrequency::Quarterly,
        taxpayer_types: &[
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
            TaxpayerType::Cooperative,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1702",
        title: "Annual Income Tax Return for Corporations, Partnerships and Cooperatives",
        category: "Income Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
            TaxpayerType::Cooperative,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1702RT",
        title: "Annual Income Tax Return — Regular Taxable",
        category: "Income Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
            TaxpayerType::Cooperative,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1702EX",
        title: "Annual Income Tax Return — Tax-Exempt",
        category: "Income Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
            TaxpayerType::Cooperative,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1702MX",
        title: "Annual Income Tax Return — Mixed Income",
        category: "Income Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
            TaxpayerType::Cooperative,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1704",
        title: "Improperly Accumulated Earnings Tax Return",
        category: "Income Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[TaxpayerType::Corporation],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    // ═══════════════════════════════════════════
    // Value-Added Tax
    // ═══════════════════════════════════════════
    FormDefinition {
        code: "2550M",
        title: "Monthly Value-Added Tax Declaration",
        category: "Value-Added Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: Some(true),
        requires_employees: false,
        is_deprecated: false, // Optional per RMC 52-2023; temporal engine handles era transitions
    },
    FormDefinition {
        code: "2550Q",
        title: "Quarterly Value-Added Tax Return",
        category: "Value-Added Tax",
        frequency: FilingFrequency::Quarterly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: Some(true),
        requires_employees: false,
        is_deprecated: false,
    },
    // ═══════════════════════════════════════════
    // Percentage Tax
    // ═══════════════════════════════════════════
    FormDefinition {
        code: "2551Q",
        title: "Quarterly Percentage Tax Return",
        category: "Percentage Tax",
        frequency: FilingFrequency::Quarterly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: Some(false),
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "2551M",
        title: "Monthly Percentage Tax Return",
        category: "Percentage Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: Some(false),
        requires_employees: false,
        is_deprecated: true, // Abolished by TRAIN Law RA 10963 (2018) — percentage tax is quarterly-only via 2551Q
    },
    FormDefinition {
        code: "2552",
        title: "Percentage Tax Return on Transactions Involving Shares of Stock",
        category: "Percentage Tax",
        frequency: FilingFrequency::Quarterly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "2553",
        title: "Percentage Tax Payable Under Special Laws",
        category: "Percentage Tax",
        frequency: FilingFrequency::Quarterly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    // ═══════════════════════════════════════════
    // Documentary Stamp Tax
    // ═══════════════════════════════════════════
    FormDefinition {
        code: "2000",
        title: "Documentary Stamp Tax Declaration/Return",
        category: "Documentary Stamp Tax",
        frequency: FilingFrequency::OpenEnded,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    // ═══════════════════════════════════════════
    // Excise Tax
    // ═══════════════════════════════════════════
    // Excise Tax — activity-based, applies to any person (individual or non-individual)
    FormDefinition {
        code: "2200A",
        title: "Excise Tax Return for Alcohol Products",
        category: "Excise Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "2200AN",
        title: "Excise Tax Return for Automobiles and Non-Essential Goods",
        category: "Excise Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "2200M",
        title: "Excise Tax Return for Mineral Products",
        category: "Excise Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "2200P",
        title: "Excise Tax Return for Petroleum Products",
        category: "Excise Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "2200T",
        title: "Excise Tax Return for Tobacco Products",
        category: "Excise Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
    FormDefinition {
        code: "0619E",
        title: "Monthly Remittance Form for Creditable Income Taxes Withheld (Expanded)",
        category: "Withholding Tax",
        frequency: FilingFrequency::Monthly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: true,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1601EQ",
        title: "Quarterly Remittance Return of Creditable Income Taxes Withheld (Expanded)",
        category: "Withholding Tax",
        frequency: FilingFrequency::Quarterly,
        taxpayer_types: &[
            TaxpayerType::Individual,
            TaxpayerType::Corporation,
            TaxpayerType::Partnership,
        ],
        requires_vat: None,
        requires_employees: true,
        is_deprecated: false,
    },
    FormDefinition {
        code: "1701MS",
        title: "Annual Income Tax Return for Micro and Small Taxpayers",
        category: "Income Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[TaxpayerType::Individual],
        requires_vat: None,
        requires_employees: false,
        is_deprecated: false,
    },
];

/// Returns forms available to a given taxpayer type.
pub fn forms_for_taxpayer(taxpayer_type: &TaxpayerType) -> Vec<&'static FormDefinition> {
    FORM_REGISTRY
        .iter()
        .filter(|f| f.taxpayer_types.contains(taxpayer_type))
        .collect()
}

/// Find a single form definition by code.
pub fn find_form(code: &str) -> Option<&'static FormDefinition> {
    FORM_REGISTRY.iter().find(|f| f.code == code)
}

/// Returns forms filtered by taxpayer type, VAT status, and employee status.
pub fn forms_for_profile(
    taxpayer_type: &TaxpayerType,
    is_vat_registered: bool,
    has_employees: bool,
) -> Vec<&'static FormDefinition> {
    FORM_REGISTRY
        .iter()
        .filter(|f| {
            f.taxpayer_types.contains(taxpayer_type)
                && match f.requires_vat {
                    None => true,
                    Some(v) => v == is_vat_registered,
                }
                && (!f.requires_employees || has_employees)
        })
        .collect()
}
