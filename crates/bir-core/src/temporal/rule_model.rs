//! Legal Rule Model — compiled rule data structures.
//!
//! These are the data representations of rules loaded from canonical TOML files
//! at build time. They define phases, priorities, conditions, and mutations
//! that the engine evaluates deterministically.

use serde::{Deserialize, Serialize};

/// A compiled legal rule from a canonical TOML file.
///
/// Rules are data first. Handwritten Rust rules are allowed only for complex
/// predicates that cannot be represented declaratively, and those rules must
/// still be registered through metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalRule {
    /// Unique rule identifier (e.g., "rule.train.8-percent.suppress-percentage-tax").
    pub rule_id: String,
    /// Human-readable title.
    pub title: String,
    /// The era this rule belongs to.
    pub era_id: String,
    /// First date this rule is effective (ISO 8601).
    pub effective_from: String,
    /// Last date this rule is effective (ISO 8601). Empty/None = still active.
    pub effective_until: Option<String>,
    /// The evaluation phase this rule belongs to.
    pub phase: RulePhase,
    /// Priority within the phase (ascending — lower numbers run first).
    pub priority: i32,
    /// The condition that must be true for this rule to fire.
    pub when: RuleCondition,
    /// The state mutations this rule applies when it fires.
    pub mutations: Vec<RuleMutation>,
    /// Legal citation IDs for this rule.
    pub citations: Vec<String>,
    /// Problem description — why this rule exists.
    pub problem: String,
    /// Solution description — what this rule does.
    pub solution: String,
}

impl LegalRule {
    /// Check if this rule is active for a given year.
    pub fn is_active_for_year(&self, year: u16) -> bool {
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

/// Evaluation phases — the compiler sorts by phase, then priority, then rule_id.
///
/// Each phase represents a logical layer of the evaluation:
/// 1. Timeline — is the form even legal in this era?
/// 2. EntityEligibility — is the entity type correct?
/// 3. Registration — VAT, withholding, excise requirements
/// 4. Election — taxpayer choices (8%, OSD, etc.)
/// 5. Obligation — mandatory filing triggers
/// 6. Exclusivity — mutual exclusion groups (1702 variants)
/// 7. Lifecycle — deprecation, transition, abolition
/// 8. UiClassification — recommendation and ranking
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RulePhase {
    Timeline,
    EntityEligibility,
    Registration,
    Election,
    Obligation,
    Exclusivity,
    Lifecycle,
    UiClassification,
}

/// A condition that determines whether a rule fires.
///
/// MVP uses a simple expression list. Future versions can use a richer DSL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCondition {
    /// All conditions must be true (AND logic).
    #[serde(default)]
    pub all: Vec<String>,
    /// Any condition must be true (OR logic).
    #[serde(default)]
    pub any: Vec<String>,
}

impl RuleCondition {
    /// An empty condition (always true — unconditional rule).
    pub fn always() -> Self {
        Self {
            all: vec![],
            any: vec![],
        }
    }

    /// Returns true if no conditions are specified (unconditional).
    pub fn is_unconditional(&self) -> bool {
        self.all.is_empty() && self.any.is_empty()
    }
}

/// A state mutation that a rule applies to a form.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMutation {
    /// The type of mutation.
    #[serde(rename = "type")]
    pub mutation_type: MutationType,
    /// The form(s) this mutation targets.
    pub target: FormSelector,
    /// Human-readable reason for this mutation.
    pub reason: String,
}

/// The type of state mutation a rule can apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationType {
    SetState,
    Suppress,
    Require,
    Recommend,
    MarkOptional,
    Deprecate,
}

/// Selects which form(s) a mutation targets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSelector {
    /// Target a specific form code.
    pub form_code: Option<String>,
    /// Target all forms in a category.
    pub category: Option<String>,
    /// Target all forms in an exclusive group.
    pub exclusive_group: Option<String>,
}

impl FormSelector {
    /// Create a selector targeting a specific form code.
    pub fn by_code(code: impl Into<String>) -> Self {
        Self {
            form_code: Some(code.into()),
            category: None,
            exclusive_group: None,
        }
    }

    /// Check if this selector matches a given form code, category, or group.
    pub fn matches(&self, form_code: &str, category: &str, exclusive_group: Option<&str>) -> bool {
        if let Some(ref target_code) = self.form_code {
            if target_code == form_code {
                return true;
            }
        }
        if let Some(ref target_cat) = self.category {
            if target_cat == category {
                return true;
            }
        }
        if let Some(ref target_group) = self.exclusive_group {
            if let Some(group) = exclusive_group {
                if target_group == group {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_active_for_year() {
        let rule = LegalRule {
            rule_id: "test-rule".into(),
            title: "Test".into(),
            era_id: "TRAIN_2018".into(),
            effective_from: "2018-01-01".into(),
            effective_until: Some("2023-12-31".into()),
            phase: RulePhase::Election,
            priority: 100,
            when: RuleCondition::always(),
            mutations: vec![],
            citations: vec![],
            problem: "".into(),
            solution: "".into(),
        };

        assert!(!rule.is_active_for_year(2017));
        assert!(rule.is_active_for_year(2018));
        assert!(rule.is_active_for_year(2023));
        assert!(!rule.is_active_for_year(2024));
    }

    #[test]
    fn test_phase_ordering() {
        assert!(RulePhase::Timeline < RulePhase::EntityEligibility);
        assert!(RulePhase::EntityEligibility < RulePhase::Registration);
        assert!(RulePhase::Registration < RulePhase::Election);
        assert!(RulePhase::Election < RulePhase::Obligation);
        assert!(RulePhase::Obligation < RulePhase::Exclusivity);
        assert!(RulePhase::Exclusivity < RulePhase::Lifecycle);
        assert!(RulePhase::Lifecycle < RulePhase::UiClassification);
    }

    #[test]
    fn test_form_selector_by_code() {
        let sel = FormSelector::by_code("2551Q");
        assert!(sel.matches("2551Q", "Percentage Tax", None));
        assert!(!sel.matches("2550Q", "Value-Added Tax", None));
    }

    #[test]
    fn test_condition_unconditional() {
        assert!(RuleCondition::always().is_unconditional());
        assert!(
            !RuleCondition {
                all: vec!["test".into()],
                any: vec![],
            }
            .is_unconditional()
        );
    }
}
