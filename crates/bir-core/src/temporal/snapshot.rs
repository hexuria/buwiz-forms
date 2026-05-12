//! Compiled Rule Snapshot — the immutable runtime container for all temporal data.
//!
//! This module defines the data structures that the build script generates
//! from canonical TOML files. At runtime, the engine evaluates only this
//! embedded snapshot — it never reads editable rule files from disk.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::citations::LegalCitation;
use super::context::Jurisdiction;
use super::forms::FormArtifact;
use super::formulas::ComputationFormula;
use super::rates::TaxRateTable;
use super::rule_model::LegalRule;

/// A compiled, immutable snapshot of all temporal tax data.
///
/// Generated at build time from canonical TOML files and embedded in the binary.
/// Runtime engine evaluates only this snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledRuleSnapshot {
    /// Unique snapshot identifier (format: `ttce.YYYY.MM.DD.short_hash`).
    pub snapshot_id: String,
    /// SHA-256 hash of all source file contents + compiler version.
    pub content_hash: String,
    /// ISO 8601 timestamp when the snapshot was generated.
    pub generated_at: String,
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// All regulatory eras defined in the snapshot.
    pub eras: Vec<Era>,
    /// All form artifacts with effective windows.
    pub form_artifacts: Vec<FormArtifact>,
    /// All compiled legal rules.
    pub rules: Vec<LegalRule>,
    /// All tax rate tables.
    pub rate_tables: Vec<TaxRateTable>,
    /// All computation formulas.
    pub formulas: Vec<ComputationFormula>,
    /// All legal citations referenced by rules and artifacts.
    pub citations: Vec<LegalCitation>,
}

impl CompiledRuleSnapshot {
    /// Returns an empty snapshot for testing purposes.
    pub fn empty() -> Self {
        Self {
            snapshot_id: "empty".into(),
            content_hash: "".into(),
            generated_at: "".into(),
            schema_version: 1,
            eras: vec![],
            form_artifacts: vec![],
            rules: vec![],
            rate_tables: vec![],
            formulas: vec![],
            citations: vec![],
        }
    }

    /// Find a citation by its ID.
    pub fn find_citation(&self, citation_id: &str) -> Option<&LegalCitation> {
        self.citations.iter().find(|c| c.citation_id == citation_id)
    }

    /// Returns true when the snapshot knows at least one artifact for a form code.
    pub fn has_form_code(&self, form_code: &str) -> bool {
        self.form_artifacts
            .iter()
            .any(|artifact| artifact.form_code == form_code)
    }

    /// Returns all distinct form codes known to this snapshot.
    pub fn form_codes(&self) -> Vec<String> {
        self.form_artifacts
            .iter()
            .map(|artifact| artifact.form_code.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Find an era that covers the given taxable year.
    pub fn find_primary_era(&self, taxable_year: u16) -> Option<&Era> {
        self.eras.iter().find(|era| {
            !era.is_overlay
                && taxable_year >= era.effective_from_year
                && era
                    .effective_until_year
                    .is_none_or(|end| taxable_year <= end)
        })
    }

    /// Find all overlay eras active for the given taxable year.
    pub fn find_overlay_eras(&self, taxable_year: u16) -> Vec<&Era> {
        self.eras
            .iter()
            .filter(|era| {
                era.is_overlay
                    && taxable_year >= era.effective_from_year
                    && era
                        .effective_until_year
                        .is_none_or(|end| taxable_year <= end)
            })
            .collect()
    }
}

/// A regulatory era — a named period of tax law (e.g., TRAIN 2018–2023).
///
/// Exactly one primary era must resolve for any supported taxable year.
/// Supplemental eras may attach if marked `is_overlay = true`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Era {
    /// Unique era identifier (e.g., "TRAIN_2018").
    pub era_id: String,
    /// Human-readable title.
    pub title: String,
    /// Jurisdiction this era applies to.
    pub jurisdiction: Jurisdiction,
    /// First year this era is active (inclusive).
    pub effective_from_year: u16,
    /// Last year this era is active (inclusive). None = still active.
    pub effective_until_year: Option<u16>,
    /// If true, this era overlays onto a primary era rather than replacing it.
    pub is_overlay: bool,
    /// Legal citations establishing this era.
    pub citations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_snapshot() -> CompiledRuleSnapshot {
        let mut snapshot = CompiledRuleSnapshot::empty();
        snapshot.eras = vec![
            Era {
                era_id: "PRE_TRAIN".into(),
                title: "Pre-TRAIN".into(),
                jurisdiction: Jurisdiction::PhBir,
                effective_from_year: 2016,
                effective_until_year: Some(2017),
                is_overlay: false,
                citations: vec![],
            },
            Era {
                era_id: "TRAIN_2018".into(),
                title: "TRAIN Law".into(),
                jurisdiction: Jurisdiction::PhBir,
                effective_from_year: 2018,
                effective_until_year: Some(2023),
                is_overlay: false,
                citations: vec!["ra-10963".into()],
            },
            Era {
                era_id: "EOPT_2024".into(),
                title: "EOPT Act".into(),
                jurisdiction: Jurisdiction::PhBir,
                effective_from_year: 2024,
                effective_until_year: None,
                is_overlay: false,
                citations: vec!["ra-11976".into()],
            },
        ];
        snapshot
    }

    #[test]
    fn test_find_primary_era_2017() {
        let snapshot = test_snapshot();
        let era = snapshot.find_primary_era(2017).unwrap();
        assert_eq!(era.era_id, "PRE_TRAIN");
    }

    #[test]
    fn test_find_primary_era_2018() {
        let snapshot = test_snapshot();
        let era = snapshot.find_primary_era(2018).unwrap();
        assert_eq!(era.era_id, "TRAIN_2018");
    }

    #[test]
    fn test_find_primary_era_2024() {
        let snapshot = test_snapshot();
        let era = snapshot.find_primary_era(2024).unwrap();
        assert_eq!(era.era_id, "EOPT_2024");
    }

    #[test]
    fn test_find_primary_era_2026_open_ended() {
        let snapshot = test_snapshot();
        let era = snapshot.find_primary_era(2026).unwrap();
        assert_eq!(era.era_id, "EOPT_2024");
    }
}
