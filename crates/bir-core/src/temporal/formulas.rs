//! Computation Formulas — versioned formula bindings for form artifacts.
//!
//! Each form artifact can reference a computation formula that specifies
//! required rate tables, input/output fields, and legal citations.
//! MVP formulas are implemented as Rust functions referenced by `formula_id`.

use serde::{Deserialize, Serialize};

/// A versioned computation formula bound to a form artifact.
///
/// Declares what rate tables and fields a computation needs so the
/// temporal engine can validate references at compile time and attach
/// the correct formula version to each form decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputationFormula {
    /// Unique formula identifier (e.g., "formula.2551q.rev-2018").
    pub formula_id: String,
    /// The BIR form code this formula computes (e.g., "2551Q").
    pub form_code: String,
    /// The specific form artifact this formula is bound to.
    pub artifact_id: String,
    /// First date this formula is effective (ISO 8601).
    pub effective_from: String,
    /// Last date this formula is effective (ISO 8601). Empty/None = still active.
    pub effective_until: Option<String>,
    /// Rate table IDs this formula requires for computation.
    pub rate_table_refs: Vec<String>,
    /// Input field names required by this formula.
    pub input_fields: Vec<String>,
    /// Output field names produced by this formula.
    pub output_fields: Vec<String>,
    /// Legal citation IDs establishing this formula.
    pub citations: Vec<String>,
}

impl ComputationFormula {
    /// Check if this formula is effective for a given year.
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

        year >= from_year && until_year.map_or(true, |end| year <= end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formula_effective() {
        let formula = ComputationFormula {
            formula_id: "formula.2551q.rev-2018".into(),
            form_code: "2551Q".into(),
            artifact_id: "bir.2551q.rev-2018".into(),
            effective_from: "2018-01-01".into(),
            effective_until: None,
            rate_table_refs: vec!["rate.percentage-tax.train-2018".into()],
            input_fields: vec!["gross_receipts".into()],
            output_fields: vec!["tax_due".into()],
            citations: vec!["ra-10963-sec-116".into()],
        };

        assert!(!formula.is_effective_for_year(2017));
        assert!(formula.is_effective_for_year(2018));
        assert!(formula.is_effective_for_year(2026));
    }
}
