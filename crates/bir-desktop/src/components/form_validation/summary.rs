//! Presentation-ready summary of one validation report.
//!
//! This is a pure view-model. It contains no GPUI elements, no form-specific
//! rules, no tax arithmetic, and no persistence — a view renders what this
//! produces and nothing more. Deriving a summary in a view would be duplicating
//! filing logic into the UI, which the architecture forbids.
//!
//! Everything here is a *projection* of a report the evaluator already
//! produced. It never decides whether an action is permitted: presentation
//! carries no authority, and a caller must still obtain a fresh blocking report
//! from `bir-core` before any checked export, Final Copy, queue or submit.

use std::collections::BTreeMap;

use bir_core::{BehaviorProfile, RuleSeverity, RuleViolation, ValidationPhase};

/// Why no summary could be produced. Absence is never rendered as success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryUnavailable {
    /// No evaluation has been accepted yet.
    NoResult,
    /// The most recent capture was incomplete, so the report cannot be read as
    /// a verdict on the form.
    IncompleteCapture,
    /// The evaluator could not run.
    EvaluatorUnavailable,
}

/// One issue, flattened for display, in official order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryIssue {
    pub severity: RuleSeverity,
    pub message: String,
    /// The exact official message when the presented one differs, so a
    /// filing-safe rewording never hides what the package actually said.
    pub official_message: Option<String>,
    pub field_count: usize,
}

/// A read-only projection of one report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationSummary {
    phase: ValidationPhase,
    profile: BehaviorProfile,
    issues: Vec<SummaryIssue>,
    blocking: usize,
    advisory: usize,
}

impl ValidationSummary {
    /// Builds a summary from the violations of a single report.
    ///
    /// Input order is preserved deliberately. `ValidationReport::try_new`
    /// already enforces strictly increasing issue order, so the violations
    /// arrive in official first-error order and re-sorting here could only
    /// corrupt it. In particular, presentation must never sort by severity: the
    /// taxpayer is sent to the first issue the package would have raised, not
    /// the first one this UI considers most serious.
    pub fn from_violations(
        phase: ValidationPhase,
        profile: BehaviorProfile,
        violations: &[&RuleViolation],
    ) -> Self {
        let issues: Vec<SummaryIssue> = violations
            .iter()
            .map(|violation| SummaryIssue {
                severity: violation.severity(),
                message: violation.message().to_owned(),
                official_message: violation
                    .official_message()
                    .filter(|official| *official != violation.message())
                    .map(str::to_owned),
                field_count: violation.fields().len(),
            })
            .collect();

        let blocking = issues
            .iter()
            .filter(|issue| issue.severity == RuleSeverity::Blocking)
            .count();
        let advisory = issues.len() - blocking;

        Self {
            phase,
            profile,
            issues,
            blocking,
            advisory,
        }
    }

    pub fn phase(&self) -> ValidationPhase {
        self.phase
    }

    pub fn profile(&self) -> BehaviorProfile {
        self.profile
    }

    pub fn issues(&self) -> &[SummaryIssue] {
        &self.issues
    }

    pub fn blocking_count(&self) -> usize {
        self.blocking
    }

    pub fn advisory_count(&self) -> usize {
        self.advisory
    }

    /// The issue a first-error focus action should target: the lowest-ordered
    /// blocking issue. Advisory issues never steal focus.
    pub fn first_blocking(&self) -> Option<&SummaryIssue> {
        self.issues
            .iter()
            .find(|issue| issue.severity == RuleSeverity::Blocking)
    }

    /// True when this report raised nothing at all.
    ///
    /// This is emphatically **not** "the form may be filed". It says one
    /// evaluation, in one phase, under one profile, produced no issues. Filing
    /// authority is reconstructed and revalidated by `bir-core`.
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }

    /// Issue counts per severity, for a compact header.
    pub fn counts_by_severity(&self) -> BTreeMap<&'static str, usize> {
        BTreeMap::from([("blocking", self.blocking), ("advisory", self.advisory)])
    }
}

#[cfg(test)]
mod tests {
    use super::{RuleSeverity, SummaryIssue, ValidationSummary};
    use bir_core::{BehaviorProfile, ValidationPhase};

    fn issue(order: u32, severity: RuleSeverity) -> SummaryIssue {
        SummaryIssue {
            severity,
            message: format!("issue {order}"),
            official_message: None,
            field_count: 0,
        }
    }

    fn summary(issues: Vec<SummaryIssue>) -> ValidationSummary {
        let blocking = issues
            .iter()
            .filter(|issue| issue.severity == RuleSeverity::Blocking)
            .count();
        let advisory = issues.len() - blocking;
        ValidationSummary {
            phase: ValidationPhase::Validate,
            profile: BehaviorProfile::OfficialCompatibility,
            issues,
            blocking,
            advisory,
        }
    }

    /// Official first-error order, not severity order. Sorting by severity
    /// would send the taxpayer to a different field than the package does.
    #[test]
    fn first_blocking_follows_official_order_not_severity_order() {
        let report = summary(vec![
            issue(1, RuleSeverity::Advisory),
            issue(2, RuleSeverity::Blocking),
            issue(3, RuleSeverity::Blocking),
        ]);
        assert_eq!(
            report.first_blocking().map(|issue| issue.message.as_str()),
            Some("issue 2")
        );
    }

    #[test]
    fn advisory_only_reports_have_no_focus_target() {
        let report = summary(vec![issue(1, RuleSeverity::Advisory)]);
        assert!(report.first_blocking().is_none());
        assert_eq!(report.blocking_count(), 0);
        assert_eq!(report.advisory_count(), 1);
        assert!(!report.is_clean());
    }

    #[test]
    fn a_clean_report_is_empty_not_authorized() {
        let report = summary(Vec::new());
        assert!(report.is_clean());
        assert!(report.first_blocking().is_none());
        assert_eq!(report.counts_by_severity()["blocking"], 0);
    }
}
