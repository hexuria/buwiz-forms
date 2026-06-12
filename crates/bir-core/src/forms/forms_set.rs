//! Per-year **Forms Set** — the user-owned, authoritative list of which BIR forms a
//! taxpayer files in a given taxable year.
//!
//! This replaces the rule-based temporal suggestion engine. A Forms Set is established
//! once per taxable year, either from exact form codes extracted from a reviewed
//! Certificate of Registration (COR), from the registered-tax-type fallback when the COR
//! has no exact form list, or by manual selection ([`FormSetSource::Manual`]). It is
//! persisted in the `per_year_forms` table and read by the dashboard and deadline
//! resolver. Different years may hold different sets.

use crate::forms::registry::{FilingFrequency, find_form};
use serde::{Deserialize, Serialize};

/// How a [`FormSetEntry`] came to exist — for audit and revert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormSetSource {
    /// Hand-picked by the user.
    Manual,
    /// Proposed from reviewed COR evidence, then confirmed.
    CorAi,
    /// Seeded by the one-time migration backfill from existing profile versions.
    MigrationBackfill,
}

impl FormSetSource {
    /// Stable string for DB persistence.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::CorAi => "cor_ai",
            Self::MigrationBackfill => "migration_backfill",
        }
    }

    /// Parse from the DB string; unknown values fall back to `Manual`.
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "cor_ai" => Self::CorAi,
            "migration_backfill" => Self::MigrationBackfill,
            _ => Self::Manual,
        }
    }
}

/// One form a taxpayer files in a given taxable year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormSetEntry {
    /// Canonical BIR form code, e.g. `"2551Q"`. May be a non-registry code when `custom`.
    pub form_code: String,
    /// Filing cadence. Defaults from the registry but is overridable per entry.
    pub frequency: FilingFrequency,
    /// `false` = explicitly suppressed for the year (kept for audit rather than deleted).
    pub active: bool,
    /// Where this entry came from.
    pub source: FormSetSource,
    /// `true` = user-added form not in `FORM_REGISTRY` (skips registry validation; will
    /// have no calendar deadlines unless a matching rule exists).
    pub custom: bool,
    /// Optional human note (e.g. "per accountant", "registered for VAT mid-year").
    pub reason: Option<String>,
}

impl FormSetEntry {
    /// Build an active entry for `form_code`, defaulting the frequency from the registry
    /// (falling back to [`FilingFrequency::OpenEnded`] and `custom = true` for codes not
    /// present in the registry).
    pub fn from_code(form_code: impl Into<String>, source: FormSetSource) -> Self {
        let form_code = form_code.into();
        match find_form(&form_code) {
            Some(def) => Self {
                form_code,
                frequency: def.frequency.clone(),
                active: true,
                source,
                custom: false,
                reason: None,
            },
            None => Self {
                form_code,
                frequency: FilingFrequency::OpenEnded,
                active: true,
                source,
                custom: true,
                reason: None,
            },
        }
    }
}

/// Lightweight card representation of a form for UI / suggestion purposes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormCardData {
    pub form_code: String,
    pub category: String,
    pub title: String,
    pub frequency: FilingFrequency,
}

/// The full Forms Set for one `(TIN, taxable_year)`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerYearFormsSet {
    pub taxable_year: u16,
    pub entries: Vec<FormSetEntry>,
}

impl PerYearFormsSet {
    pub fn new(taxable_year: u16) -> Self {
        Self {
            taxable_year,
            entries: Vec::new(),
        }
    }

    /// Build a set from a list of form codes, all active, with the given source.
    pub fn from_codes<I, S>(taxable_year: u16, codes: I, source: FormSetSource) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            taxable_year,
            entries: codes
                .into_iter()
                .map(|c| FormSetEntry::from_code(c, source))
                .collect(),
        }
    }

    /// Whether any entry exists for this year (active or not).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The active form codes — the authoritative list the dashboard files against.
    pub fn active_form_codes(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.active)
            .map(|e| e.form_code.clone())
            .collect()
    }

    /// Whether `form_code` is present and active.
    pub fn contains_active(&self, form_code: &str) -> bool {
        self.entries
            .iter()
            .any(|e| e.active && e.form_code == form_code)
    }

    /// Look up an entry by code.
    pub fn entry(&self, form_code: &str) -> Option<&FormSetEntry> {
        self.entries.iter().find(|e| e.form_code == form_code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_code_uses_registry_frequency() {
        let entry = FormSetEntry::from_code("2551Q", FormSetSource::Manual);
        assert_eq!(entry.frequency, FilingFrequency::Quarterly);
        assert!(entry.active);
        assert!(!entry.custom);
    }

    #[test]
    fn from_code_unknown_is_custom_openended() {
        let entry = FormSetEntry::from_code("ZZZZ", FormSetSource::Manual);
        assert!(entry.custom);
        assert_eq!(entry.frequency, FilingFrequency::OpenEnded);
    }

    #[test]
    fn active_codes_exclude_inactive() {
        let mut set =
            PerYearFormsSet::from_codes(2026, ["2551Q", "1701Q", "1701"], FormSetSource::CorAi);
        // Suppress one
        set.entries[1].active = false;
        let codes = set.active_form_codes();
        assert!(codes.contains(&"2551Q".to_string()));
        assert!(codes.contains(&"1701".to_string()));
        assert!(!codes.contains(&"1701Q".to_string()));
        assert!(set.contains_active("2551Q"));
        assert!(!set.contains_active("1701Q"));
    }
}
