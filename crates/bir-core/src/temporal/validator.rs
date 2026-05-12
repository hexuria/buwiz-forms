//! Temporal Data Validator — validates canonical rule pack data.
//!
//! Used both at build time (by `build.rs`) and at admin-save time
//! to ensure data integrity before it becomes a compiled snapshot.

use std::collections::HashSet;

use super::forms::FormArtifact;
use super::formulas::ComputationFormula;
use super::rates::TaxRateTable;
use super::rule_model::LegalRule;
use super::snapshot::Era;

/// A validation error found in canonical temporal data.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// What kind of error this is.
    pub kind: ValidationErrorKind,
    /// Human-readable description.
    pub message: String,
    /// The ID of the offending entity (if applicable).
    pub entity_id: Option<String>,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref id) = self.entity_id {
            write!(f, "[{:?}] {}: {}", self.kind, id, self.message)
        } else {
            write!(f, "[{:?}] {}", self.kind, self.message)
        }
    }
}

/// Categories of validation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationErrorKind {
    DuplicateId,
    MissingCitation,
    InvalidDateRange,
    UnknownFormTarget,
    UnknownFormulaRef,
    UnknownRateTableRef,
    OverlappingEra,
    PriorityConflict,
    MissingRequiredField,
}

/// Validates the complete set of temporal data.
///
/// Returns a list of validation errors. An empty list means the data is valid.
pub fn validate_temporal_data(
    citation_ids: &[String],
    eras: &[Era],
    artifacts: &[FormArtifact],
    rules: &[LegalRule],
    rate_tables: &[TaxRateTable],
    formulas: &[ComputationFormula],
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let citation_set: HashSet<&str> = citation_ids.iter().map(|s| s.as_str()).collect();
    let form_code_set: HashSet<&str> = artifacts.iter().map(|a| a.form_code.as_str()).collect();
    let formula_id_set: HashSet<&str> = formulas.iter().map(|f| f.formula_id.as_str()).collect();
    let rate_table_id_set: HashSet<&str> =
        rate_tables.iter().map(|r| r.table_id.as_str()).collect();

    // ── Check duplicate IDs ──
    check_duplicates(&mut errors, eras.iter().map(|e| e.era_id.as_str()), "Era");
    check_duplicates(
        &mut errors,
        artifacts.iter().map(|a| a.artifact_id.as_str()),
        "FormArtifact",
    );
    check_duplicates(
        &mut errors,
        rules.iter().map(|r| r.rule_id.as_str()),
        "LegalRule",
    );
    check_duplicates(
        &mut errors,
        rate_tables.iter().map(|r| r.table_id.as_str()),
        "TaxRateTable",
    );
    check_duplicates(
        &mut errors,
        formulas.iter().map(|f| f.formula_id.as_str()),
        "ComputationFormula",
    );

    // ── Check citation references ──
    for artifact in artifacts {
        for cit_id in &artifact.legal_citations {
            if !citation_set.contains(cit_id.as_str()) {
                errors.push(ValidationError {
                    kind: ValidationErrorKind::MissingCitation,
                    message: format!(
                        "Form artifact '{}' references unknown citation '{}'",
                        artifact.artifact_id, cit_id
                    ),
                    entity_id: Some(artifact.artifact_id.clone()),
                });
            }
        }
    }

    for rule in rules {
        for cit_id in &rule.citations {
            if !citation_set.contains(cit_id.as_str()) {
                errors.push(ValidationError {
                    kind: ValidationErrorKind::MissingCitation,
                    message: format!(
                        "Rule '{}' references unknown citation '{}'",
                        rule.rule_id, cit_id
                    ),
                    entity_id: Some(rule.rule_id.clone()),
                });
            }
        }
    }

    // ── Check rule targets reference known forms ──
    for rule in rules {
        for mutation in &rule.mutations {
            if let Some(ref form_code) = mutation.target.form_code {
                if !form_code_set.contains(form_code.as_str()) {
                    errors.push(ValidationError {
                        kind: ValidationErrorKind::UnknownFormTarget,
                        message: format!(
                            "Rule '{}' mutation targets unknown form code '{}'",
                            rule.rule_id, form_code
                        ),
                        entity_id: Some(rule.rule_id.clone()),
                    });
                }
            }
        }
    }

    // ── Check formula references in form artifacts ──
    for artifact in artifacts {
        if let Some(ref formula_ref) = artifact.formula_ref {
            if !formula_ref.is_empty() && !formula_id_set.contains(formula_ref.as_str()) {
                errors.push(ValidationError {
                    kind: ValidationErrorKind::UnknownFormulaRef,
                    message: format!(
                        "Form artifact '{}' references unknown formula '{}'",
                        artifact.artifact_id, formula_ref
                    ),
                    entity_id: Some(artifact.artifact_id.clone()),
                });
            }
        }
    }

    // ── Check rate table references in formulas ──
    for formula in formulas {
        for rate_ref in &formula.rate_table_refs {
            if !rate_table_id_set.contains(rate_ref.as_str()) {
                errors.push(ValidationError {
                    kind: ValidationErrorKind::UnknownRateTableRef,
                    message: format!(
                        "Formula '{}' references unknown rate table '{}'",
                        formula.formula_id, rate_ref
                    ),
                    entity_id: Some(formula.formula_id.clone()),
                });
            }
        }
    }

    // ── Check overlapping primary eras ──
    let primary_eras: Vec<&Era> = eras.iter().filter(|e| !e.is_overlay).collect();
    for (i, era_a) in primary_eras.iter().enumerate() {
        for era_b in primary_eras.iter().skip(i + 1) {
            if eras_overlap(era_a, era_b) {
                errors.push(ValidationError {
                    kind: ValidationErrorKind::OverlappingEra,
                    message: format!(
                        "Primary eras '{}' and '{}' overlap",
                        era_a.era_id, era_b.era_id
                    ),
                    entity_id: Some(era_a.era_id.clone()),
                });
            }
        }
    }

    // ── Check date validity ──
    for artifact in artifacts {
        if let Some(ref until) = artifact.effective_until {
            if !until.is_empty() && until < &artifact.effective_from {
                errors.push(ValidationError {
                    kind: ValidationErrorKind::InvalidDateRange,
                    message: format!(
                        "Form artifact '{}' has effective_until before effective_from",
                        artifact.artifact_id
                    ),
                    entity_id: Some(artifact.artifact_id.clone()),
                });
            }
        }
    }

    for rule in rules {
        if let Some(ref until) = rule.effective_until {
            if !until.is_empty() && until < &rule.effective_from {
                errors.push(ValidationError {
                    kind: ValidationErrorKind::InvalidDateRange,
                    message: format!(
                        "Rule '{}' has effective_until before effective_from",
                        rule.rule_id
                    ),
                    entity_id: Some(rule.rule_id.clone()),
                });
            }
        }
    }

    errors
}

fn check_duplicates<'a>(
    errors: &mut Vec<ValidationError>,
    ids: impl Iterator<Item = &'a str>,
    entity_type: &str,
) {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            errors.push(ValidationError {
                kind: ValidationErrorKind::DuplicateId,
                message: format!("Duplicate {} id: '{}'", entity_type, id),
                entity_id: Some(id.to_string()),
            });
        }
    }
}

fn eras_overlap(a: &Era, b: &Era) -> bool {
    let a_end = a.effective_until_year.unwrap_or(u16::MAX);
    let b_end = b.effective_until_year.unwrap_or(u16::MAX);
    a.effective_from_year <= b_end && b.effective_from_year <= a_end
}

#[cfg(test)]
mod tests {
    use crate::temporal::context::Jurisdiction;

    use super::*;

    fn era(id: &str, from: u16, until: Option<u16>) -> Era {
        Era {
            era_id: id.into(),
            title: id.into(),
            jurisdiction: Jurisdiction::PhBir,
            effective_from_year: from,
            effective_until_year: until,
            is_overlay: false,
            citations: vec![],
        }
    }

    #[test]
    fn test_duplicate_era_id() {
        let eras = vec![era("TRAIN", 2018, None), era("TRAIN", 2020, None)];
        let errs = validate_temporal_data(&[], &eras, &[], &[], &[], &[]);
        assert!(
            errs.iter()
                .any(|e| e.kind == ValidationErrorKind::DuplicateId)
        );
    }

    #[test]
    fn test_overlapping_primary_eras() {
        let eras = vec![era("ERA_A", 2018, Some(2023)), era("ERA_B", 2020, None)];
        let errs = validate_temporal_data(&[], &eras, &[], &[], &[], &[]);
        assert!(
            errs.iter()
                .any(|e| e.kind == ValidationErrorKind::OverlappingEra)
        );
    }

    #[test]
    fn test_non_overlapping_eras_ok() {
        let eras = vec![
            era("PRE_TRAIN", 2016, Some(2017)),
            era("TRAIN", 2018, Some(2023)),
            era("EOPT", 2024, None),
        ];
        let errs = validate_temporal_data(&[], &eras, &[], &[], &[], &[]);
        assert!(
            !errs
                .iter()
                .any(|e| e.kind == ValidationErrorKind::OverlappingEra),
            "got: {:?}",
            errs
        );
    }

    #[test]
    fn test_missing_citation_on_rule() {
        use crate::temporal::rule_model::*;

        let rules = vec![LegalRule {
            rule_id: "test-rule".into(),
            title: "Test".into(),
            era_id: "TRAIN".into(),
            effective_from: "2018-01-01".into(),
            effective_until: None,
            phase: RulePhase::Election,
            priority: 100,
            when: RuleCondition::always(),
            mutations: vec![],
            citations: vec!["nonexistent-citation".into()],
            problem: "".into(),
            solution: "".into(),
        }];

        let errs = validate_temporal_data(&[], &[], &[], &rules, &[], &[]);
        assert!(
            errs.iter()
                .any(|e| e.kind == ValidationErrorKind::MissingCitation)
        );
    }

    #[test]
    fn test_unknown_form_target() {
        use crate::temporal::rule_model::*;

        let rules = vec![LegalRule {
            rule_id: "test-rule".into(),
            title: "Test".into(),
            era_id: "TRAIN".into(),
            effective_from: "2018-01-01".into(),
            effective_until: None,
            phase: RulePhase::Election,
            priority: 100,
            when: RuleCondition::always(),
            mutations: vec![RuleMutation {
                mutation_type: MutationType::Suppress,
                target: FormSelector::by_code("UNKNOWN_FORM"),
                reason: "test".into(),
            }],
            citations: vec![],
            problem: "".into(),
            solution: "".into(),
        }];

        let errs = validate_temporal_data(&[], &[], &[], &rules, &[], &[]);
        assert!(
            errs.iter()
                .any(|e| e.kind == ValidationErrorKind::UnknownFormTarget)
        );
    }

    #[test]
    fn test_unknown_formula_ref_on_artifact() {
        use crate::forms::registry::FilingFrequency;
        use crate::temporal::forms::*;

        let artifacts = vec![FormArtifact {
            artifact_id: "bir.test.rev-2018".into(),
            form_code: "TEST".into(),
            title: "Test".into(),
            category: "Test".into(),
            revision: "2018".into(),
            effective_from: "2018-01-01".into(),
            effective_until: None,
            lifecycle: ArtifactLifecycle::Active,
            frequency: FilingFrequency::Quarterly,
            taxpayer_types: vec![],
            classifications: vec![],
            legal_citations: vec![],
            schema_ref: None,
            template_ref: None,
            formula_ref: Some("nonexistent-formula".into()),
            requires_vat: None,
            withholding_trigger: None,
            excise_category: None,
            exclusive_group: None,
            exclusive_priority: 0,
        }];

        let errs = validate_temporal_data(&[], &[], &artifacts, &[], &[], &[]);
        assert!(
            errs.iter()
                .any(|e| e.kind == ValidationErrorKind::UnknownFormulaRef)
        );
    }

    #[test]
    fn test_invalid_date_range_on_rule() {
        use crate::temporal::rule_model::*;

        let rules = vec![LegalRule {
            rule_id: "bad-dates".into(),
            title: "Test".into(),
            era_id: "TRAIN".into(),
            effective_from: "2023-01-01".into(),
            effective_until: Some("2018-01-01".into()),
            phase: RulePhase::Timeline,
            priority: 0,
            when: RuleCondition::always(),
            mutations: vec![],
            citations: vec![],
            problem: "".into(),
            solution: "".into(),
        }];

        let errs = validate_temporal_data(&[], &[], &[], &rules, &[], &[]);
        assert!(
            errs.iter()
                .any(|e| e.kind == ValidationErrorKind::InvalidDateRange)
        );
    }
}
