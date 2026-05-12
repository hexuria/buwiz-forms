//! Form Identity and Artifact — separating form code from legal revision.
//!
//! A form code like "1701" can represent multiple legal artifacts across
//! revisions. This module models that distinction so the engine can
//! resolve the correct artifact for any taxable year.

use crate::forms::registry::FilingFrequency;
use crate::profile::{TaxClassification, TaxpayerType};
use serde::{Deserialize, Serialize};

/// A stable form identity — the human-recognizable form code.
///
/// Example: `FormIdentity { code: "2551Q", title: "Quarterly Percentage Tax Return", category: "Percentage Tax" }`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormIdentity {
    /// BIR form code (e.g., "2551Q").
    pub code: String,
    /// Human-readable title.
    pub title: String,
    /// Tax category (e.g., "Income Tax", "Percentage Tax").
    pub category: String,
}

/// A specific legal revision of a form — an immutable artifact with effective dates.
///
/// Multiple artifacts can exist for the same form code across different eras.
/// The engine selects the artifact valid for the `TemporalContext`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormArtifact {
    /// Unique artifact identifier (e.g., "bir.2551q.rev-2018").
    pub artifact_id: String,
    /// The BIR form code this artifact represents (e.g., "2551Q").
    pub form_code: String,
    /// Human-readable title.
    pub title: String,
    /// Tax category (e.g., "Income Tax", "Value-Added Tax").
    pub category: String,
    /// Revision identifier (e.g., "2018", "Jan-2018").
    pub revision: String,
    /// First date this artifact is legally effective (inclusive).
    pub effective_from: String,
    /// Last date this artifact is legally effective (inclusive). Empty = still active.
    pub effective_until: Option<String>,
    /// Lifecycle state of this artifact.
    pub lifecycle: ArtifactLifecycle,
    /// Filing frequency.
    pub frequency: FilingFrequency,
    /// Entity types this artifact applies to.
    pub taxpayer_types: Vec<TaxpayerType>,
    /// Refined classifications this artifact applies to.
    /// Empty means "all classifications for the given taxpayer types".
    pub classifications: Vec<TaxClassification>,
    /// Legal citation IDs establishing this artifact.
    pub legal_citations: Vec<String>,
    /// Reference to the JSON schema for this form version.
    pub schema_ref: Option<String>,
    /// Reference to the template/form-type for rendering.
    pub template_ref: Option<String>,
    /// Reference to the computation formula for this artifact.
    pub formula_ref: Option<String>,
    /// Whether this form requires VAT registration. None = not a VAT/percentage tax form.
    pub requires_vat: Option<bool>,
    /// Which withholding type triggers this form.
    pub withholding_trigger: Option<String>,
    /// Excise tax category this form requires.
    pub excise_category: Option<String>,
    /// Exclusive group this form belongs to (e.g., "ANNUAL_CORPORATE_ITR").
    pub exclusive_group: Option<String>,
    /// Priority within the exclusive group (higher = preferred).
    pub exclusive_priority: u8,
}

impl FormArtifact {
    /// Check if this artifact is effective for a given year.
    pub fn is_effective_for_year(&self, year: u16) -> bool {
        let from_year = self.effective_from_year();
        let until_year = self.effective_until_year();

        year >= from_year && until_year.map_or(true, |end| year <= end)
    }

    /// Extract the year from the `effective_from` date string.
    pub fn effective_from_year(&self) -> u16 {
        self.effective_from
            .split('-')
            .next()
            .and_then(|y| y.parse().ok())
            .unwrap_or(0)
    }

    /// Extract the year from the `effective_until` date string.
    pub fn effective_until_year(&self) -> Option<u16> {
        self.effective_until.as_ref().and_then(|s| {
            if s.is_empty() {
                None
            } else {
                s.split('-').next().and_then(|y| y.parse().ok())
            }
        })
    }
}

/// The lifecycle state of a form artifact in the legal system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ArtifactLifecycle {
    /// Currently in force and accepting filings.
    #[default]
    Active,
    /// Legal but being phased out; a successor exists.
    Transitional,
    /// No longer the preferred version but still legally valid for historical filing.
    Deprecated,
    /// Abolished by law — cannot be filed for current periods.
    Abolished,
    /// Not yet effective — becomes active on its effective_from date.
    Future,
}

/// Legal availability — separates legal validity from operational preference.
///
/// This is richer than a simple boolean `is_deprecated`. Examples:
/// - `2550M` after 2023 can be `Optional`, not `Deprecated`.
/// - `1701MS` can be `Recommended` for micro/small taxpayers while `1701A` remains `Allowed`.
/// - `2551M` after 2017 can be `Abolished` for current filing while still queryable for historical periods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormAvailability {
    /// Filing is mandatory for the applicable taxpayer type/period.
    Mandatory,
    /// Filing is legally allowed.
    Allowed,
    /// Filing is optional (taxpayer may choose).
    Optional,
    /// BIR recommends this form for the taxpayer's situation.
    Recommended,
    /// Legacy filing allowed for historical periods.
    LegacyAllowed,
    /// Form is in a transition period between old and new versions.
    Transitional,
    /// Form is no longer the preferred or recommended option.
    Deprecated,
    /// Form has been abolished by law.
    Abolished,
}

/// Dashboard ranking without rewriting legal state.
///
/// This drives how the dashboard orders and presents forms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationalRecommendation {
    /// Primary recommended form for this category/period.
    Primary,
    /// A valid alternative that BIR recommends under certain conditions.
    RecommendedAlternative,
    /// A valid alternative that the taxpayer may choose.
    AllowedAlternative,
    /// Only relevant for historical filings.
    HistoricalOnly,
    /// Hidden from the primary dashboard by default.
    HiddenByDefault,
    /// Cannot be filed for this period.
    BlockedForPeriod,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_artifact() -> FormArtifact {
        FormArtifact {
            artifact_id: "bir.2551q.rev-2018".into(),
            form_code: "2551Q".into(),
            title: "Quarterly Percentage Tax Return".into(),
            category: "Percentage Tax".into(),
            revision: "2018".into(),
            effective_from: "2018-01-01".into(),
            effective_until: None,
            lifecycle: ArtifactLifecycle::Active,
            frequency: FilingFrequency::Quarterly,
            taxpayer_types: vec![TaxpayerType::Individual, TaxpayerType::Corporation],
            classifications: vec![],
            legal_citations: vec!["ra-10963-sec-116".into()],
            schema_ref: None,
            template_ref: None,
            formula_ref: Some("formula.2551q.rev-2018".into()),
            requires_vat: Some(false),
            withholding_trigger: None,
            excise_category: None,
            exclusive_group: None,
            exclusive_priority: 0,
        }
    }

    #[test]
    fn test_artifact_effective_for_year() {
        let art = test_artifact();
        assert!(!art.is_effective_for_year(2017));
        assert!(art.is_effective_for_year(2018));
        assert!(art.is_effective_for_year(2024));
    }

    #[test]
    fn test_artifact_with_end_date() {
        let mut art = test_artifact();
        art.effective_until = Some("2023-12-31".into());
        assert!(art.is_effective_for_year(2023));
        assert!(!art.is_effective_for_year(2024));
    }

    #[test]
    fn test_artifact_year_extraction() {
        let art = test_artifact();
        assert_eq!(art.effective_from_year(), 2018);
        assert_eq!(art.effective_until_year(), None);
    }
}
