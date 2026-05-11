//! Form eligibility types — the 6-state output system.

use super::citations::LegalCitation;
use crate::forms::registry::FilingFrequency;
use serde::{Deserialize, Serialize};

/// The resolved eligibility state of a form for a given profile + year.
///
/// Replaces the binary `is_suggested: bool` with a 6-state system.
/// The default (fail-open) state is `Allowed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormEligibility {
    /// MUST file this form. A rule has determined it is mandatory.
    Required(String),

    /// BIR recommends this form (e.g., 1701-MS for Micro/Small).
    /// The taxpayer may choose an alternative.
    Recommended(String),

    /// Default state. The form is applicable and can be filed.
    /// This is the fail-open baseline — no rule has touched it.
    Allowed,

    /// The form may be filed at the taxpayer's discretion.
    /// (e.g., 2550M after RMC 52-2023)
    Optional(String),

    /// A rule has hidden this form. It is not applicable.
    /// (e.g., 2551Q when 8% is elected, or entity mismatch)
    Suppressed(String),

    /// The form was abolished for this timeline but exists historically.
    /// UI can show it behind a "View Past Forms" toggle.
    Deprecated(String),
}

impl FormEligibility {
    /// Returns true if the form should be visible in the primary dashboard view.
    pub fn is_visible(&self) -> bool {
        matches!(
            self,
            Self::Required(_) | Self::Recommended(_) | Self::Allowed | Self::Optional(_)
        )
    }

    /// Returns true if the form is hidden but exists (for "View Hidden" toggle).
    pub fn is_hidden(&self) -> bool {
        matches!(self, Self::Suppressed(_) | Self::Deprecated(_))
    }

    /// Returns the attached reason string, if any.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Required(r)
            | Self::Recommended(r)
            | Self::Optional(r)
            | Self::Suppressed(r)
            | Self::Deprecated(r) => Some(r),
            Self::Allowed => None,
        }
    }

    /// Returns `true` for the legacy `is_suggested` compat — anything visible is suggested.
    pub fn is_suggested(&self) -> bool {
        self.is_visible()
    }
}

/// A single rule application recorded in the audit log.
#[derive(Debug, Clone, Serialize)]
pub struct RuleApplication {
    /// Name of the rule that fired.
    pub rule_name: String,
    /// The law or issuance this rule implements.
    pub law: String,
    /// The state before this rule ran.
    pub before: FormEligibility,
    /// The state after this rule ran.
    pub after: FormEligibility,
    /// Human-readable reason for the transition.
    pub reason: String,
}

/// The result of evaluating a single form against a profile for a target year.
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
    /// The resolved eligibility state.
    pub eligibility: FormEligibility,
    /// Ordered list of rules that modified this form's eligibility.
    pub audit_log: Vec<RuleApplication>,
    /// Structured legal citations from all applied rules.
    pub legal_citations: Vec<LegalCitation>,
}
