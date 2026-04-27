use super::taxpayer::TaxpayerClass;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterestRateRule {
    pub taxpayer_class: TaxpayerClass,
    pub annual_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurchargeRateRule {
    pub taxpayer_class: TaxpayerClass,
    pub is_fraud: bool,
    pub rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PenaltyConfig {
    pub interest_rates: Vec<InterestRateRule>,
    pub surcharge_rates: Vec<SurchargeRateRule>,
    // In a real system, compromise tables would also be here.
    // For now, we encapsulate them in compromise.rs to keep it simple,
    // but the architecture allows for loading them.
}

impl PenaltyConfig {
    pub fn default_rules() -> Self {
        Self {
            interest_rates: vec![
                InterestRateRule {
                    taxpayer_class: TaxpayerClass::Micro,
                    annual_rate: 0.06,
                },
                InterestRateRule {
                    taxpayer_class: TaxpayerClass::Small,
                    annual_rate: 0.06,
                },
                InterestRateRule {
                    taxpayer_class: TaxpayerClass::Medium,
                    annual_rate: 0.12,
                },
                InterestRateRule {
                    taxpayer_class: TaxpayerClass::Large,
                    annual_rate: 0.12,
                },
                InterestRateRule {
                    taxpayer_class: TaxpayerClass::Regular,
                    annual_rate: 0.12,
                },
            ],
            surcharge_rates: vec![
                // Non-fraud
                SurchargeRateRule {
                    taxpayer_class: TaxpayerClass::Micro,
                    is_fraud: false,
                    rate: 0.10,
                },
                SurchargeRateRule {
                    taxpayer_class: TaxpayerClass::Small,
                    is_fraud: false,
                    rate: 0.10,
                },
                SurchargeRateRule {
                    taxpayer_class: TaxpayerClass::Medium,
                    is_fraud: false,
                    rate: 0.25,
                },
                SurchargeRateRule {
                    taxpayer_class: TaxpayerClass::Large,
                    is_fraud: false,
                    rate: 0.25,
                },
                SurchargeRateRule {
                    taxpayer_class: TaxpayerClass::Regular,
                    is_fraud: false,
                    rate: 0.25,
                },
                // Fraud
                SurchargeRateRule {
                    taxpayer_class: TaxpayerClass::Micro,
                    is_fraud: true,
                    rate: 0.50,
                },
                SurchargeRateRule {
                    taxpayer_class: TaxpayerClass::Small,
                    is_fraud: true,
                    rate: 0.50,
                },
                SurchargeRateRule {
                    taxpayer_class: TaxpayerClass::Medium,
                    is_fraud: true,
                    rate: 0.50,
                },
                SurchargeRateRule {
                    taxpayer_class: TaxpayerClass::Large,
                    is_fraud: true,
                    rate: 0.50,
                },
                SurchargeRateRule {
                    taxpayer_class: TaxpayerClass::Regular,
                    is_fraud: true,
                    rate: 0.50,
                },
            ],
        }
    }

    pub fn get_interest_rate(&self, class: TaxpayerClass) -> f64 {
        self.interest_rates
            .iter()
            .find(|r| r.taxpayer_class == class)
            .map(|r| r.annual_rate)
            .unwrap_or(0.12)
    }

    pub fn get_surcharge_rate(&self, class: TaxpayerClass, is_fraud: bool) -> f64 {
        self.surcharge_rates
            .iter()
            .find(|r| r.taxpayer_class == class && r.is_fraud == is_fraud)
            .map(|r| r.rate)
            .unwrap_or(if is_fraud { 0.50 } else { 0.25 })
    }
}
