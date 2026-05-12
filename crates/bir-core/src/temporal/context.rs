//! Temporal Context — the explicit query input for all temporal evaluations.
//!
//! Every evaluation in the temporal engine receives this context instead of
//! reading the system clock. This is the canonical replacement for
//! `Local::now().year()` in eligibility paths.

use serde::{Deserialize, Serialize};

/// The explicit temporal context for all engine evaluations.
///
/// Rules must receive this context instead of reading the system date.
/// The dashboard constructs this from `SmartDateFilterEvent.year`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalContext {
    /// The taxable year being evaluated (e.g. 2024).
    pub taxable_year: u16,
    /// The filing period within the taxable year.
    pub period: FilingPeriod,
    /// How this evaluation is being used.
    pub filing_mode: FilingMode,
    /// Jurisdiction for MVP (always Philippines/BIR).
    pub jurisdiction: Jurisdiction,
    /// The compiled snapshot to evaluate against.
    pub snapshot_id: SnapshotId,
}

impl TemporalContext {
    /// Create a context for current compliance in the given year.
    ///
    /// This is the most common construction path from the dashboard.
    pub fn current_compliance(taxable_year: u16) -> Self {
        Self {
            taxable_year,
            period: FilingPeriod::Annual,
            filing_mode: FilingMode::CurrentCompliance,
            jurisdiction: Jurisdiction::PhBir,
            snapshot_id: SnapshotId::Current,
        }
    }

    /// Create a context for retroactive filing of a prior year.
    pub fn retroactive(taxable_year: u16) -> Self {
        Self {
            taxable_year,
            period: FilingPeriod::Annual,
            filing_mode: FilingMode::RetroactiveFiling,
            jurisdiction: Jurisdiction::PhBir,
            snapshot_id: SnapshotId::Current,
        }
    }

    /// Return the context with a specific filing period.
    pub fn with_period(mut self, period: FilingPeriod) -> Self {
        self.period = period;
        self
    }

    /// Return the context with a specific filing mode.
    pub fn with_mode(mut self, mode: FilingMode) -> Self {
        self.filing_mode = mode;
        self
    }
}

/// The filing period within a taxable year.
///
/// The existing quarter and month chips map to this type when present.
/// If no quarter or month is selected, the dashboard uses `Annual` for
/// annual cards and a form-specific period for card actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilingPeriod {
    Annual,
    Quarterly { quarter: u8 },
    Monthly { month: u8 },
    OpenEnded,
}

impl Default for FilingPeriod {
    fn default() -> Self {
        Self::Annual
    }
}

/// How the evaluation is being used.
///
/// MVP default for the dashboard is `CurrentCompliance` for the current year
/// and `RetroactiveFiling` for prior years.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FilingMode {
    /// Reconstructing obligations for a historical period.
    HistoricalReconstruction,
    /// Normal current-year compliance check.
    #[default]
    CurrentCompliance,
    /// Filing a return for a prior year (late filing, amendment).
    RetroactiveFiling,
}

/// Tax jurisdiction.
///
/// MVP supports only Philippines/BIR. Future: extend for other jurisdictions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Jurisdiction {
    #[default]
    PhBir,
}

impl std::fmt::Display for Jurisdiction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PhBir => write!(f, "PH_BIR"),
        }
    }
}

/// Identifies which compiled snapshot to use.
///
/// `Current` always refers to the embedded snapshot compiled at build time.
/// Named snapshots are for future use (versioned historical snapshots).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotId {
    /// Use the current embedded snapshot.
    Current,
    /// Use a specific named snapshot (future).
    Named(String),
}

impl Default for SnapshotId {
    fn default() -> Self {
        Self::Current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_compliance_defaults() {
        let ctx = TemporalContext::current_compliance(2024);
        assert_eq!(ctx.taxable_year, 2024);
        assert_eq!(ctx.period, FilingPeriod::Annual);
        assert_eq!(ctx.filing_mode, FilingMode::CurrentCompliance);
        assert_eq!(ctx.jurisdiction, Jurisdiction::PhBir);
        assert_eq!(ctx.snapshot_id, SnapshotId::Current);
    }

    #[test]
    fn test_retroactive_defaults() {
        let ctx = TemporalContext::retroactive(2017);
        assert_eq!(ctx.taxable_year, 2017);
        assert_eq!(ctx.filing_mode, FilingMode::RetroactiveFiling);
    }

    #[test]
    fn test_builder_methods() {
        let ctx = TemporalContext::current_compliance(2024)
            .with_period(FilingPeriod::Quarterly { quarter: 1 })
            .with_mode(FilingMode::HistoricalReconstruction);
        assert_eq!(ctx.period, FilingPeriod::Quarterly { quarter: 1 });
        assert_eq!(ctx.filing_mode, FilingMode::HistoricalReconstruction);
    }
}
