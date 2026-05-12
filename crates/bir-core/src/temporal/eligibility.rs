//! Form eligibility types — the compliance state output system.
//!
//! Upgraded from 6 states to 9 states per REQ-004.
//! Each state maps to a UI visibility policy.

use super::citations::LegalCitation;
use crate::forms::registry::FilingFrequency;
use serde::{Deserialize, Serialize};

/// The resolved compliance state of a form for a given profile + temporal context.
///
/// This is the engine's UI-facing result. It replaces the old `FormEligibility`.
/// Every mutation records before state, after state, rule id, citation id, and explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceState {
    /// No rule has evaluated this form yet.
    Unknown,
    /// The form is applicable and can be filed (fail-open baseline).
    Applicable,
    /// A rule has determined this form is mandatory.
    Required(String),
    /// BIR recommends this form (e.g., 1701-MS for Micro/Small).
    Recommended(String),
    /// The form may be filed at the taxpayer's discretion.
    Optional(String),
    /// A rule has hidden this form. It is not applicable.
    Suppressed(String),
    /// The form was abolished for this timeline but exists historically.
    Deprecated(String),
    /// The form is retired and only available in audit/admin views.
    Archived(String),
    /// The form cannot legally be filed for this period.
    IllegalForPeriod(String),
}

impl ComplianceState {
    /// Returns true if the form should be visible in the primary dashboard view.
    pub fn is_visible(&self) -> bool {
        matches!(
            self,
            Self::Unknown
                | Self::Applicable
                | Self::Required(_)
                | Self::Recommended(_)
                | Self::Optional(_)
        )
    }

    /// Returns true if the form is hidden but exists (for "View Hidden" toggle).
    pub fn is_hidden(&self) -> bool {
        matches!(
            self,
            Self::Suppressed(_)
                | Self::Deprecated(_)
                | Self::Archived(_)
                | Self::IllegalForPeriod(_)
        )
    }

    /// Returns the attached reason string, if any.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Unknown | Self::Applicable => None,
            Self::Required(r)
            | Self::Recommended(r)
            | Self::Optional(r)
            | Self::Suppressed(r)
            | Self::Deprecated(r)
            | Self::Archived(r)
            | Self::IllegalForPeriod(r) => Some(r),
        }
    }

    /// Returns `true` for the legacy `is_suggested` compat — anything visible is suggested.
    pub fn is_suggested(&self) -> bool {
        self.is_visible()
    }

    /// Returns a display label suitable for a UI badge.
    pub fn badge_label(&self) -> Option<&'static str> {
        match self {
            Self::Required(_) => Some("Required"),
            Self::Recommended(_) => Some("Recommended"),
            Self::Optional(_) => Some("Optional"),
            Self::Deprecated(_) => Some("Legacy"),
            _ => None,
        }
    }

    /// Returns the sort rank for deterministic output ordering.
    /// Lower rank = higher priority in display.
    pub fn sort_rank(&self) -> u8 {
        match self {
            Self::Required(_) => 0,
            Self::Recommended(_) => 1,
            Self::Applicable => 2,
            Self::Optional(_) => 3,
            Self::Unknown => 4,
            Self::Suppressed(_) => 5,
            Self::Deprecated(_) => 6,
            Self::Archived(_) => 7,
            Self::IllegalForPeriod(_) => 8,
        }
    }
}

// ──── Legacy compat: keep FormEligibility as an alias ────

/// Legacy alias for backward compatibility. New code should use `ComplianceState`.
pub type FormEligibility = ComplianceState;

impl ComplianceState {
    /// Legacy compat: create an `Allowed` state (maps to `Applicable`).
    pub fn allowed() -> Self {
        Self::Applicable
    }
}

/// A single rule application recorded in the audit log.
#[derive(Debug, Clone, Serialize)]
pub struct RuleApplication {
    /// Name/ID of the rule that fired.
    pub rule_name: String,
    /// The law or issuance this rule implements.
    pub law: String,
    /// The state before this rule ran.
    pub before: ComplianceState,
    /// The state after this rule ran.
    pub after: ComplianceState,
    /// Human-readable reason for the transition.
    pub reason: String,
}

/// The result of evaluating a single form against a profile for a temporal context.
#[derive(Debug, Clone, Serialize)]
pub struct FormDecision {
    /// The BIR form code (e.g., "2551Q").
    pub form_code: String,
    /// Human-readable form title.
    pub title: String,
    /// The form category (e.g., "Income Tax", "Excise Tax").
    pub category: String,
    /// Filing frequency.
    pub frequency: FilingFrequency,
    /// The resolved compliance state.
    pub eligibility: ComplianceState,
    /// Ordered list of rules that modified this form's eligibility.
    pub audit_log: Vec<RuleApplication>,
    /// Structured legal citations from all applied rules.
    pub legal_citations: Vec<LegalCitation>,
    /// The artifact ID resolved for this decision (if any).
    pub artifact_id: Option<String>,
    /// The formula ID resolved for this decision (if any).
    pub formula_id: Option<String>,
    /// Rate table IDs resolved for this decision.
    pub rate_table_ids: Vec<String>,
}

impl FormDecision {
    /// Returns true if this form should be visible in the primary dashboard.
    pub fn is_primary_visible(&self) -> bool {
        self.eligibility.is_visible()
    }
}

/// A view model for the dashboard, adapted from `FormDecision`.
///
/// This preserves the existing card rendering while adding temporal metadata.
#[derive(Debug, Clone, Serialize)]
pub struct DashboardFormDecision {
    /// The BIR form code.
    pub form_code: String,
    /// Human-readable form title.
    pub title: String,
    /// The form category.
    pub category: String,
    /// Filing frequency.
    pub frequency: FilingFrequency,
    /// The resolved compliance state.
    pub state: ComplianceState,
    /// Optional badge text for UI display.
    pub badge: Option<String>,
    /// Whether this form should appear in the primary dashboard view.
    pub is_primary_visible: bool,
    /// Short audit summary for explanation views.
    pub audit_summary: String,
    /// The artifact ID resolved for this decision.
    pub artifact_id: Option<String>,
    /// The formula ID resolved for this decision.
    pub formula_id: Option<String>,
}

impl From<&FormDecision> for DashboardFormDecision {
    fn from(decision: &FormDecision) -> Self {
        let badge = decision.eligibility.badge_label().map(|s| s.to_string());
        let audit_summary = if decision.audit_log.is_empty() {
            "No rules applied".to_string()
        } else {
            decision
                .audit_log
                .iter()
                .map(|a| format!("{}: {}", a.rule_name, a.reason))
                .collect::<Vec<_>>()
                .join("; ")
        };

        Self {
            form_code: decision.form_code.clone(),
            title: decision.title.clone(),
            category: decision.category.clone(),
            frequency: decision.frequency.clone(),
            state: decision.eligibility.clone(),
            badge,
            is_primary_visible: decision.is_primary_visible(),
            audit_summary,
            artifact_id: decision.artifact_id.clone(),
            formula_id: decision.formula_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visibility_mapping() {
        assert!(ComplianceState::Unknown.is_visible());
        assert!(ComplianceState::Applicable.is_visible());
        assert!(ComplianceState::Required("test".into()).is_visible());
        assert!(ComplianceState::Recommended("test".into()).is_visible());
        assert!(ComplianceState::Optional("test".into()).is_visible());
        assert!(!ComplianceState::Suppressed("test".into()).is_visible());
        assert!(!ComplianceState::Deprecated("test".into()).is_visible());
        assert!(!ComplianceState::Archived("test".into()).is_visible());
        assert!(!ComplianceState::IllegalForPeriod("test".into()).is_visible());
    }

    #[test]
    fn test_sort_rank_order() {
        assert!(
            ComplianceState::Required("".into()).sort_rank()
                < ComplianceState::Recommended("".into()).sort_rank()
        );
        assert!(
            ComplianceState::Recommended("".into()).sort_rank()
                < ComplianceState::Applicable.sort_rank()
        );
        assert!(
            ComplianceState::Optional("".into()).sort_rank()
                < ComplianceState::Suppressed("".into()).sort_rank()
        );
    }

    #[test]
    fn test_badge_labels() {
        assert_eq!(
            ComplianceState::Required("".into()).badge_label(),
            Some("Required")
        );
        assert_eq!(
            ComplianceState::Recommended("".into()).badge_label(),
            Some("Recommended")
        );
        assert_eq!(ComplianceState::Applicable.badge_label(), None);
    }
}
