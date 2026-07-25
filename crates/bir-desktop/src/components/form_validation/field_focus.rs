use std::collections::BTreeMap;
use std::{error::Error, fmt};

use bir_core::form_rules::FieldInstance;
use bir_core::{RuleSeverity, RuleViolation};

/// Exact semantic field identities mapped to view-specific focus targets.
///
/// Construction rejects duplicate semantic identities instead of silently
/// replacing an earlier control with a later one.
#[derive(Debug, Clone)]
pub struct SemanticFieldTargets<T> {
    targets: BTreeMap<FieldInstance, T>,
}

impl<T> SemanticFieldTargets<T> {
    pub fn try_new(
        targets: impl IntoIterator<Item = (FieldInstance, T)>,
    ) -> Result<Self, DuplicateSemanticFieldTarget> {
        let mut by_field = BTreeMap::new();
        for (field, target) in targets {
            if by_field.insert(field.clone(), target).is_some() {
                return Err(DuplicateSemanticFieldTarget { field });
            }
        }
        Ok(Self { targets: by_field })
    }

    pub fn get(&self, field: &FieldInstance) -> Option<&T> {
        self.targets.get(field)
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Select a target from the first blocking violation only.
    ///
    /// If that violation has no mapped control, this returns `None`; it never
    /// skips to a later blocking violation and changes the issue being
    /// presented as first.
    pub fn first_blocking_target<'a>(&'a self, violations: &[RuleViolation]) -> Option<&'a T> {
        let first_blocking = violations
            .iter()
            .find(|violation| violation.severity() == RuleSeverity::Blocking)?;
        first_blocking
            .fields()
            .iter()
            .find_map(|field| self.targets.get(field.field()))
    }
}

impl<T> Default for SemanticFieldTargets<T> {
    fn default() -> Self {
        Self {
            targets: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateSemanticFieldTarget {
    field: FieldInstance,
}

impl DuplicateSemanticFieldTarget {
    pub fn field(&self) -> &FieldInstance {
        &self.field
    }
}

impl fmt::Display for DuplicateSemanticFieldTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "duplicate focus target for semantic field {}",
            self.field.field_id()
        )
    }
}

impl Error for DuplicateSemanticFieldTarget {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn field(field_id: &str) -> FieldInstance {
        serde_json::from_value(json!({
            "field_id": field_id,
            "group_path": [],
        }))
        .expect("valid semantic field")
    }

    fn violation(
        rule_order: u32,
        severity: RuleSeverity,
        fields: &[FieldInstance],
    ) -> RuleViolation {
        let fields = fields
            .iter()
            .map(|field| {
                json!({
                    "field": field,
                    "xml_key": null,
                    "serialized_occurrence": null,
                })
            })
            .collect::<Vec<_>>();
        serde_json::from_value(json!({
            "execution": {
                "rule_id": format!("rule-{rule_order}"),
                "instance": null,
            },
            "phase": "validate",
            "order": {
                "rule_order": rule_order,
                "occurrence": 0,
            },
            "fields": fields,
            "official_message": null,
            "message": format!("Rule {rule_order}"),
            "assessment": "verified-correct",
            "severity": match severity {
                RuleSeverity::Advisory => "advisory",
                RuleSeverity::Blocking => "blocking",
            },
            "profile": "filing_safe",
        }))
        .expect("valid rule violation")
    }

    #[test]
    fn duplicate_semantic_targets_are_rejected() {
        let amount = field("amount");
        let error =
            SemanticFieldTargets::try_new([(amount.clone(), "first"), (amount.clone(), "second")])
                .expect_err("duplicate semantic identity must fail");

        assert_eq!(error.field(), &amount);
    }

    #[test]
    fn first_blocking_violation_does_not_fall_through_to_a_later_issue() {
        let first_unmapped = field("first-unmapped");
        let later_mapped = field("later-mapped");
        let targets =
            SemanticFieldTargets::try_new([(later_mapped.clone(), "later target")]).unwrap();
        let violations = vec![
            violation(1, RuleSeverity::Advisory, &[later_mapped.clone()]),
            violation(2, RuleSeverity::Blocking, &[first_unmapped]),
            violation(3, RuleSeverity::Blocking, &[later_mapped]),
        ];

        assert_eq!(targets.first_blocking_target(&violations), None);
    }

    #[test]
    fn first_blocking_violation_uses_its_first_mapped_semantic_field() {
        let unmapped = field("unmapped");
        let amount = field("amount");
        let tax = field("tax");
        let targets = SemanticFieldTargets::try_new([
            (amount.clone(), "amount target"),
            (tax.clone(), "tax target"),
        ])
        .unwrap();
        let violations = vec![
            violation(1, RuleSeverity::Blocking, &[unmapped, amount, tax.clone()]),
            violation(2, RuleSeverity::Blocking, &[tax]),
        ];

        assert_eq!(
            targets.first_blocking_target(&violations),
            Some(&"amount target")
        );
    }
}
