//! Form type registry — defines all supported BIR forms and their filing rules.

use crate::profile::TaxpayerType;

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

pub const FORM_REGISTRY: &[FormDefinition] = &[
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
    },
    FormDefinition {
        code: "1702Q",
        title: "Quarterly Income Tax Return for Corporations, Partnerships",
        category: "Income Tax",
        frequency: FilingFrequency::Quarterly,
        taxpayer_types: &[TaxpayerType::Corporation, TaxpayerType::Partnership],
    },
    FormDefinition {
        code: "1702RT",
        title: "Annual Income Tax Return for Corporations, Partnerships",
        category: "Income Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[TaxpayerType::Corporation, TaxpayerType::Partnership],
    },
    FormDefinition {
        code: "1701Q",
        title: "Quarterly Income Tax Return for Individuals, Estates and Trusts",
        category: "Income Tax",
        frequency: FilingFrequency::Quarterly,
        taxpayer_types: &[TaxpayerType::Individual],
    },
    FormDefinition {
        code: "1701",
        title: "Annual Income Tax Return for Individuals",
        category: "Income Tax",
        frequency: FilingFrequency::Annual,
        taxpayer_types: &[TaxpayerType::Individual],
    },
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
