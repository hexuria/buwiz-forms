//! Tax Rate Tables — effective-dated, citation-backed tax parameters.
//!
//! Rate tables store tax rates, thresholds, exemptions, penalties, and other
//! dated numeric legal parameters. They are resolved by `TemporalContext`
//! so that computation code never uses hardcoded current constants.

use serde::{Deserialize, Serialize};

use super::context::Jurisdiction;

/// A dated tax rate table with legal citations.
///
/// Rate tables are canonical snapshot data with effective windows.
/// Computation modules request rates through `TemporalContext`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxRateTable {
    /// Unique table identifier (e.g., "rate.percentage-tax.train-2018").
    pub table_id: String,
    /// Human-readable title.
    pub title: String,
    /// Jurisdiction this table applies to.
    pub jurisdiction: Jurisdiction,
    /// Tax type category (e.g., "PercentageTax", "IncomeTax", "CIT", "MCIT").
    pub tax_type: String,
    /// First date this table is effective (ISO 8601).
    pub effective_from: String,
    /// Last date this table is effective (ISO 8601). Empty/None = still active.
    pub effective_until: Option<String>,
    /// Individual rate entries within this table.
    pub rates: Vec<TaxRateEntry>,
    /// Legal citation IDs establishing these rates.
    pub citations: Vec<String>,
}

impl TaxRateTable {
    /// Check if this table is effective for a given year.
    pub fn is_effective_for_year(&self, year: u16) -> bool {
        let from_year: u16 = self
            .effective_from
            .split('-')
            .next()
            .and_then(|y| y.parse().ok())
            .unwrap_or(0);

        let until_year: Option<u16> = self.effective_until.as_ref().and_then(|s| {
            if s.is_empty() {
                None
            } else {
                s.split('-').next().and_then(|y| y.parse().ok())
            }
        });

        year >= from_year && until_year.is_none_or(|end| year <= end)
    }

    /// Find a rate entry by key.
    pub fn find_rate(&self, key: &str) -> Option<&TaxRateEntry> {
        self.rates.iter().find(|r| r.key == key)
    }
}

/// A single rate/threshold entry within a rate table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxRateEntry {
    /// Key identifier for this rate (e.g., "default", "msme_20pct", "standard_25pct").
    pub key: String,
    /// Conditions under which this rate applies (DSL expressions).
    #[serde(default)]
    pub applies_when: Vec<String>,
    /// The tax rate as a decimal string (e.g., "0.03" for 3%).
    pub rate: String,
    /// Minimum threshold for this rate bracket.
    pub threshold_min: Option<String>,
    /// Maximum threshold for this rate bracket.
    pub threshold_max: Option<String>,
    /// Fixed amount component (for graduated brackets).
    pub fixed_amount: Option<String>,
}

impl TaxRateEntry {
    /// Parse the rate as an f64.
    pub fn rate_f64(&self) -> f64 {
        self.rate.parse().unwrap_or(0.0)
    }

    /// Parse the threshold_min as an f64.
    pub fn threshold_min_f64(&self) -> Option<f64> {
        self.threshold_min
            .as_ref()
            .and_then(|s| if s.is_empty() { None } else { s.parse().ok() })
    }

    /// Parse the threshold_max as an f64.
    pub fn threshold_max_f64(&self) -> Option<f64> {
        self.threshold_max
            .as_ref()
            .and_then(|s| if s.is_empty() { None } else { s.parse().ok() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rate_table() -> TaxRateTable {
        TaxRateTable {
            table_id: "rate.percentage-tax.train-2018".into(),
            title: "Percentage tax rate during TRAIN era".into(),
            jurisdiction: Jurisdiction::PhBir,
            tax_type: "PercentageTax".into(),
            effective_from: "2018-01-01".into(),
            effective_until: None,
            rates: vec![TaxRateEntry {
                key: "default".into(),
                applies_when: vec![],
                rate: "0.03".into(),
                threshold_min: None,
                threshold_max: None,
                fixed_amount: None,
            }],
            citations: vec!["ra-10963-sec-116".into()],
        }
    }

    #[test]
    fn test_rate_effective_year() {
        let table = test_rate_table();
        assert!(!table.is_effective_for_year(2017));
        assert!(table.is_effective_for_year(2018));
        assert!(table.is_effective_for_year(2026));
    }

    #[test]
    fn test_find_rate() {
        let table = test_rate_table();
        let rate = table.find_rate("default").unwrap();
        assert_eq!(rate.rate_f64(), 0.03);
    }

    #[test]
    fn test_find_rate_missing() {
        let table = test_rate_table();
        assert!(table.find_rate("nonexistent").is_none());
    }
}
