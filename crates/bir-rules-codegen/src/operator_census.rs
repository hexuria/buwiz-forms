//! Exact census of the structured v1 construct surface, the executable v2
//! operator surface, and the untranslated gap.
//!
//! The legacy v1 corpus intentionally stores conditions and formulae as evidence-backed
//! prose. Guessing operators from that prose would turn an implementation heuristic into
//! authority. This report therefore treats those strings as opaque: it counts only typed v1
//! facts and operators that are structurally present in audited v2 IR.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::audit::{AuditOptions, audit};
use crate::corpus::{CorpusReport, ValidateV1Options, validate_v1};
use crate::error::{CodegenError, Result};
use crate::files::read_tracked_bytes;
use crate::json::{JsonValue, parse_typed};
use crate::model::ReviewStatus;
use crate::path::{canonical_repo_root, resolve_existing_under};

#[derive(Clone, Debug)]
pub struct OperatorCensusOptions {
    pub repo_root: PathBuf,
}

impl OperatorCensusOptions {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct OperatorCounts {
    pub field_coercions: BTreeMap<String, usize>,
    pub normalizations: BTreeMap<String, usize>,
    pub predicates: BTreeMap<String, usize>,
    pub expressions: BTreeMap<String, usize>,
    pub effects: BTreeMap<String, usize>,
    pub calculation_scopes: BTreeMap<String, usize>,
    pub rule_scopes: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SnapshotOperatorCensus {
    pub rule_set_id: String,
    pub review_status: String,
    pub fields: usize,
    pub validation_rules: usize,
    pub calculations: usize,
    pub operators: OperatorCounts,
}

/// Exact structured facts carried by the legacy validation records.
///
/// Condition and behavior prose is intentionally absent from this type. A
/// non-null string is counted for presence where appropriate, never parsed.
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct V1ValidationConstructCensus {
    pub records: usize,
    pub phases: BTreeMap<String, usize>,
    pub assessments: BTreeMap<String, usize>,
    pub confidences: BTreeMap<String, usize>,
    pub evidence_types: BTreeMap<String, usize>,
    pub exact_message_present: usize,
    pub exact_message_absent: usize,
    pub field_arities: BTreeMap<usize, usize>,
    pub ordered_records: usize,
    pub unordered_records: usize,
}

/// Exact structured facts carried by the legacy calculation records.
///
/// Formula and condition text is intentionally absent. Rounding descriptors
/// and triggers are retained only as opaque, exact strings.
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct V1CalculationConstructCensus {
    pub records: usize,
    pub assessments: BTreeMap<String, usize>,
    pub confidences: BTreeMap<String, usize>,
    pub condition_present: usize,
    pub condition_absent: usize,
    pub rounding_descriptors: BTreeMap<String, usize>,
    pub dependency_arities: BTreeMap<usize, usize>,
    pub triggers: BTreeMap<String, usize>,
    pub output_arities: BTreeMap<usize, usize>,
    pub input_arities: BTreeMap<usize, usize>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct OperatorCensusReport {
    pub v1_forms: usize,
    pub v1_fields: usize,
    pub v1_validation_rules: usize,
    pub v1_calculations: usize,
    pub v2_forms: usize,
    pub v2_fields: usize,
    pub v2_validation_rules: usize,
    pub v2_calculations: usize,
    pub untranslated_fields: usize,
    pub untranslated_validation_rules: usize,
    pub untranslated_calculations: usize,
    pub v1_validation_constructs: V1ValidationConstructCensus,
    pub v1_calculation_constructs: V1CalculationConstructCensus,
    pub snapshots: Vec<SnapshotOperatorCensus>,
}

pub fn operator_census(options: &OperatorCensusOptions) -> Result<OperatorCensusReport> {
    let v1 = validate_v1(&ValidateV1Options::new(&options.repo_root))?;
    let v2 = audit(&AuditOptions::tracked_checkout(&options.repo_root))?;
    let (v1_validation_constructs, v1_calculation_constructs) =
        census_v1_constructs(&options.repo_root, &v1)?;

    let mut snapshots = Vec::with_capacity(v2.snapshots.len());
    for snapshot in &v2.snapshots {
        let document = &snapshot.document;
        let mut operators = OperatorCounts::default();

        for field in &document.fields {
            let Some(official) = profile_branch(field, "behavior", "official") else {
                continue;
            };
            if state(official) != Some("executable") {
                continue;
            }
            if let Some(coercion) = object_value(official, "coercion") {
                count_direct_kind(coercion, &mut operators.field_coercions);
            }
            if let Some(normalization) = array_value(official, "normalization") {
                for step in normalization {
                    count_direct_kind(step, &mut operators.normalizations);
                }
            }
            if let Some(events) = array_value(official, "event_normalization") {
                for event in events {
                    if let Some(normalization) = array_value(event, "normalization") {
                        for step in normalization {
                            count_direct_kind(step, &mut operators.normalizations);
                        }
                    }
                }
            }
        }

        for calculation in &document.calculations {
            count_scope(calculation, &mut operators.calculation_scopes);
            let Some(official) = profile_branch(calculation, "profiles", "official") else {
                continue;
            };
            if state(official) != Some("executable") {
                continue;
            }
            if let Some(condition) = object_value(official, "condition") {
                count_predicate_tree(
                    condition,
                    &mut operators.predicates,
                    &mut operators.expressions,
                );
            }
            if let Some(outputs) = array_value(official, "outputs") {
                for output in outputs {
                    if let Some(value) = object_value(output, "value") {
                        count_expression_members(value, &mut operators.expressions);
                    }
                }
            }
        }

        for rule in &document.rules {
            count_scope(rule, &mut operators.rule_scopes);
            let Some(official) = profile_branch(rule, "profiles", "official") else {
                continue;
            };
            if state(official) != Some("executable") {
                continue;
            }
            if let Some(predicate) = object_value(official, "predicate") {
                count_predicate_tree(
                    predicate,
                    &mut operators.predicates,
                    &mut operators.expressions,
                );
            }
            if let Some(effects) = array_value(official, "effects") {
                for effect in effects {
                    count_direct_kind(effect, &mut operators.effects);
                    // Effects may embed typed expressions. Count those separately rather
                    // than treating nested expression nodes as effect kinds.
                    if let Some(object) = effect.object() {
                        for (key, value) in object {
                            if key != "kind" {
                                count_expression_members(value, &mut operators.expressions);
                            }
                        }
                    }
                }
            }
        }

        snapshots.push(SnapshotOperatorCensus {
            rule_set_id: document.identity.rule_set_id.clone(),
            review_status: match document.review_status {
                ReviewStatus::Skeleton => "skeleton",
                ReviewStatus::Candidate => "candidate",
                ReviewStatus::Reviewed => "reviewed",
            }
            .to_owned(),
            fields: document.fields.len(),
            validation_rules: document.rules.len(),
            calculations: document.calculations.len(),
            operators,
        });
    }
    snapshots.sort_by(|left, right| left.rule_set_id.cmp(&right.rule_set_id));

    let v2_fields = snapshots.iter().map(|snapshot| snapshot.fields).sum();
    let v2_validation_rules = snapshots
        .iter()
        .map(|snapshot| snapshot.validation_rules)
        .sum();
    let v2_calculations = snapshots.iter().map(|snapshot| snapshot.calculations).sum();
    let untranslated_fields = untranslated_count("field", v1.fields, v2_fields)?;
    let untranslated_validation_rules =
        untranslated_count("validation", v1.validations, v2_validation_rules)?;
    let untranslated_calculations =
        untranslated_count("calculation", v1.calculations, v2_calculations)?;

    Ok(OperatorCensusReport {
        v1_forms: v1.forms_audited,
        v1_fields: v1.fields,
        v1_validation_rules: v1.validations,
        v1_calculations: v1.calculations,
        v2_forms: snapshots.len(),
        v2_fields,
        v2_validation_rules,
        v2_calculations,
        untranslated_fields,
        untranslated_validation_rules,
        untranslated_calculations,
        v1_validation_constructs,
        v1_calculation_constructs,
        snapshots,
    })
}

#[derive(Debug, Deserialize)]
struct V1IndexDocument {
    forms: Vec<V1IndexEntry>,
}

#[derive(Debug, Deserialize)]
struct V1IndexEntry {
    form_id: String,
    revision: String,
    path: String,
}

#[derive(Debug, Deserialize)]
struct V1ValidationsDocument {
    form_id: String,
    revision: String,
    rules: Vec<V1ValidationRecord>,
}

#[derive(Debug, Deserialize)]
struct V1ValidationRecord {
    rule_id: String,
    form_id: String,
    revision: String,
    phase: String,
    order: Option<u64>,
    fields: Vec<String>,
    exact_message: Option<String>,
    evidence_type: Vec<String>,
    assessment: String,
    confidence: String,
}

#[derive(Debug, Deserialize)]
struct V1CalculationsDocument {
    form_id: String,
    revision: String,
    evaluation_order: Vec<String>,
    calculations: Vec<V1CalculationRecord>,
}

#[derive(Debug, Deserialize)]
struct V1CalculationRecord {
    calculation_id: String,
    outputs: Vec<String>,
    inputs: Vec<String>,
    condition: Option<String>,
    rounding: String,
    trigger: String,
    depends_on: Vec<String>,
    assessment: String,
    confidence: String,
}

fn census_v1_constructs(
    repo_root: &Path,
    audited: &CorpusReport,
) -> Result<(V1ValidationConstructCensus, V1CalculationConstructCensus)> {
    let repo_root = canonical_repo_root(repo_root)?;
    let rules_root = resolve_existing_under(&repo_root, "rules", "rules directory")?;
    let index: V1IndexDocument = load_v1_document(&rules_root, "index.json", "rules index")?;

    if index.forms.len() != audited.forms_audited {
        return Err(CodegenError::new(format!(
            "operator census v1 form count drift: index has {}, validate-v1 audited {}",
            index.forms.len(),
            audited.forms_audited
        )));
    }

    let mut expected_forms: BTreeMap<String, (usize, usize)> = audited
        .form_results
        .iter()
        .map(|form| (form.form_id.clone(), (form.validations, form.calculations)))
        .collect();
    if expected_forms.len() != audited.form_results.len() {
        return Err(CodegenError::new(
            "operator census found duplicate validate-v1 form IDs",
        ));
    }

    let mut seen_index_ids = BTreeSet::new();
    let mut validation_counts = V1ValidationConstructCensus::default();
    let mut calculation_counts = V1CalculationConstructCensus::default();

    for indexed in index.forms {
        require_non_empty_id(&indexed.form_id, "v1 index form_id")?;
        if !seen_index_ids.insert(indexed.form_id.clone()) {
            return Err(CodegenError::new(format!(
                "operator census found duplicate v1 index form_id `{}`",
                indexed.form_id
            )));
        }
        let (expected_validations, expected_calculations) =
            expected_forms.remove(&indexed.form_id).ok_or_else(|| {
                CodegenError::new(format!(
                    "operator census index form `{}` was not audited by validate-v1",
                    indexed.form_id
                ))
            })?;

        let form_dir = indexed
            .path
            .strip_suffix("/manifest.json")
            .filter(|path| !path.is_empty())
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "operator census index path is not a form manifest: `{}`",
                    indexed.path
                ))
            })?;
        let validations_path = format!("{form_dir}/validations.json");
        let calculations_path = format!("{form_dir}/calculations.json");
        let validations: V1ValidationsDocument =
            load_v1_document(&rules_root, &validations_path, "v1 validations")?;
        let calculations: V1CalculationsDocument =
            load_v1_document(&rules_root, &calculations_path, "v1 calculations")?;

        require_document_identity(
            &indexed,
            &validations.form_id,
            &validations.revision,
            &validations_path,
        )?;
        require_document_identity(
            &indexed,
            &calculations.form_id,
            &calculations.revision,
            &calculations_path,
        )?;
        if validations.rules.len() != expected_validations {
            return Err(CodegenError::new(format!(
                "operator census validation count drift for `{}`: loaded {}, validate-v1 audited {}",
                indexed.form_id,
                validations.rules.len(),
                expected_validations
            )));
        }
        if calculations.calculations.len() != expected_calculations {
            return Err(CodegenError::new(format!(
                "operator census calculation count drift for `{}`: loaded {}, validate-v1 audited {}",
                indexed.form_id,
                calculations.calculations.len(),
                expected_calculations
            )));
        }

        for record in &validations.rules {
            require_non_empty_id(&record.rule_id, "v1 validation rule_id")?;
            if record.form_id != indexed.form_id || record.revision != indexed.revision {
                return Err(CodegenError::new(format!(
                    "operator census validation `{}` identity does not match index form `{}` revision `{}`",
                    record.rule_id, indexed.form_id, indexed.revision
                )));
            }
            count_v1_validation(record, &mut validation_counts);
        }

        let calculation_ids: BTreeSet<&str> = calculations
            .calculations
            .iter()
            .map(|calculation| calculation.calculation_id.as_str())
            .collect();
        if calculation_ids.len() != calculations.calculations.len() {
            return Err(CodegenError::new(format!(
                "operator census found duplicate calculation IDs for `{}`",
                indexed.form_id
            )));
        }
        for calculation in &calculations.calculations {
            require_non_empty_id(&calculation.calculation_id, "v1 calculation_id")?;
            count_v1_calculation(calculation, &mut calculation_counts);
        }
        let ordered_ids: BTreeSet<&str> = calculations
            .evaluation_order
            .iter()
            .map(String::as_str)
            .collect();
        if calculations.evaluation_order.len() != calculation_ids.len()
            || ordered_ids != calculation_ids
        {
            return Err(CodegenError::new(format!(
                "operator census calculation evaluation_order drift for `{}`",
                indexed.form_id
            )));
        }
    }

    if !expected_forms.is_empty() {
        return Err(CodegenError::new(format!(
            "operator census validate-v1 forms absent from index: {}",
            expected_forms
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    if validation_counts.records != audited.validations {
        return Err(CodegenError::new(format!(
            "operator census v1 validation total drift: counted {}, validate-v1 audited {}",
            validation_counts.records, audited.validations
        )));
    }
    if calculation_counts.records != audited.calculations {
        return Err(CodegenError::new(format!(
            "operator census v1 calculation total drift: counted {}, validate-v1 audited {}",
            calculation_counts.records, audited.calculations
        )));
    }

    Ok((validation_counts, calculation_counts))
}

fn load_v1_document<T>(rules_root: &Path, relative: &str, label: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let path = resolve_existing_under(rules_root, relative, label)?;
    let bytes = read_tracked_bytes(&path)?;
    let (document, _) = parse_typed(&bytes, &path)?;
    Ok(document)
}

fn require_document_identity(
    indexed: &V1IndexEntry,
    form_id: &str,
    revision: &str,
    relative: &str,
) -> Result<()> {
    if form_id != indexed.form_id || revision != indexed.revision {
        return Err(CodegenError::new(format!(
            "operator census document `rules/{relative}` identity does not match index form `{}` revision `{}`",
            indexed.form_id, indexed.revision
        )));
    }
    Ok(())
}

fn require_non_empty_id(value: &str, label: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(CodegenError::new(format!(
            "operator census found empty {label}"
        )));
    }
    Ok(())
}

fn count_v1_validation(record: &V1ValidationRecord, counts: &mut V1ValidationConstructCensus) {
    counts.records += 1;
    increment(&mut counts.phases, &record.phase);
    increment(&mut counts.assessments, &record.assessment);
    increment(&mut counts.confidences, &record.confidence);
    for evidence_type in &record.evidence_type {
        increment(&mut counts.evidence_types, evidence_type);
    }
    increment_arity(&mut counts.field_arities, record.fields.len());
    if record.exact_message.is_some() {
        counts.exact_message_present += 1;
    } else {
        counts.exact_message_absent += 1;
    }
    if record.order.is_some() {
        counts.ordered_records += 1;
    } else {
        counts.unordered_records += 1;
    }
}

fn count_v1_calculation(record: &V1CalculationRecord, counts: &mut V1CalculationConstructCensus) {
    counts.records += 1;
    increment(&mut counts.assessments, &record.assessment);
    increment(&mut counts.confidences, &record.confidence);
    if record.condition.is_some() {
        counts.condition_present += 1;
    } else {
        counts.condition_absent += 1;
    }
    increment(&mut counts.rounding_descriptors, &record.rounding);
    increment_arity(&mut counts.dependency_arities, record.depends_on.len());
    increment(&mut counts.triggers, &record.trigger);
    increment_arity(&mut counts.output_arities, record.outputs.len());
    increment_arity(&mut counts.input_arities, record.inputs.len());
}

fn increment(counts: &mut BTreeMap<String, usize>, value: &str) {
    *counts.entry(value.to_owned()).or_default() += 1;
}

fn increment_arity(counts: &mut BTreeMap<usize, usize>, arity: usize) {
    *counts.entry(arity).or_default() += 1;
}

fn untranslated_count(label: &str, v1: usize, v2: usize) -> Result<usize> {
    v1.checked_sub(v2).ok_or_else(|| {
        CodegenError::new(format!(
            "operator census {label} count drift: v2 has {v2}, v1 has {v1}"
        ))
    })
}

fn profile_branch<'a>(
    value: &'a JsonValue,
    profiles_key: &str,
    profile: &str,
) -> Option<&'a JsonValue> {
    value.object()?.get(profiles_key)?.object()?.get(profile)
}

fn state(value: &JsonValue) -> Option<&str> {
    value.object()?.get("state")?.as_str()
}

fn object_value<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object()?.get(key)
}

fn array_value<'a>(value: &'a JsonValue, key: &str) -> Option<&'a Vec<JsonValue>> {
    match value.object()?.get(key)? {
        JsonValue::Array(values) => Some(values),
        _ => None,
    }
}

fn count_scope(value: &JsonValue, counts: &mut BTreeMap<String, usize>) {
    let Some(scope) = object_value(value, "scope") else {
        return;
    };
    count_direct_kind(scope, counts);
}

fn count_direct_kind(value: &JsonValue, counts: &mut BTreeMap<String, usize>) {
    let Some(kind) = value
        .object()
        .and_then(|object| object.get("kind"))
        .and_then(JsonValue::as_str)
    else {
        return;
    };
    *counts.entry(kind.to_owned()).or_default() += 1;
}

fn count_predicate_tree(
    value: &JsonValue,
    predicates: &mut BTreeMap<String, usize>,
    expressions: &mut BTreeMap<String, usize>,
) {
    match value {
        JsonValue::Object(object) => {
            // Every expression node declares its result type. Count the whole
            // expression subtree in the expression census and do not mistake
            // its `kind` (or a nested field-instance selector) for a predicate.
            if object.contains_key("result_type") {
                count_expression_members(value, expressions);
                return;
            }
            // A field reference may contain an instance selector with its own
            // `kind`, but neither object is a predicate operator.
            if object.contains_key("field_id") {
                return;
            }
            if let Some(kind) = object.get("kind").and_then(JsonValue::as_str) {
                *predicates.entry(kind.to_owned()).or_default() += 1;
            }
            for child in object.values() {
                count_predicate_tree(child, predicates, expressions);
            }
        }
        JsonValue::Array(values) => {
            for child in values {
                count_predicate_tree(child, predicates, expressions);
            }
        }
        _ => {}
    }
}

fn count_expression_members(value: &JsonValue, counts: &mut BTreeMap<String, usize>) {
    match value {
        JsonValue::Object(object) => {
            // Expression objects always declare result_type. This avoids counting
            // nested field-instance selectors and other unrelated `kind` objects.
            if object.contains_key("result_type") {
                if let Some(kind) = object.get("kind").and_then(JsonValue::as_str) {
                    *counts.entry(kind.to_owned()).or_default() += 1;
                }
            }
            for child in object.values() {
                count_expression_members(child, counts);
            }
        }
        JsonValue::Array(values) => {
            for child in values {
                count_expression_members(child, counts);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        OperatorCensusOptions, V1CalculationConstructCensus, V1CalculationRecord,
        V1ValidationConstructCensus, V1ValidationRecord, count_v1_calculation, count_v1_validation,
        operator_census, untranslated_count,
    };

    fn string_counts(entries: &[(&str, usize)]) -> BTreeMap<String, usize> {
        entries
            .iter()
            .map(|(value, count)| ((*value).to_owned(), *count))
            .collect()
    }

    #[test]
    fn landed_census_reports_exact_v1_constructs_and_structured_v2_operators() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = operator_census(&OperatorCensusOptions::new(root)).expect("operator census");

        assert_eq!(report.v1_forms, 43);
        assert_eq!(report.v1_fields, 9_592);
        assert_eq!(report.v1_validation_rules, 2_007);
        assert_eq!(report.v1_calculations, 623);
        assert_eq!(report.v2_forms, 1);
        assert_eq!(report.v2_fields, 94);
        assert_eq!(report.v2_validation_rules, 27);
        assert_eq!(report.v2_calculations, 1);
        assert_eq!(report.untranslated_fields, 9_498);
        assert_eq!(report.untranslated_validation_rules, 1_980);
        assert_eq!(report.untranslated_calculations, 622);

        let validations = &report.v1_validation_constructs;
        assert_eq!(validations.records, 2_007);
        assert_eq!(
            validations.phases,
            string_counts(&[
                ("blur/change", 225),
                ("final-copy", 30),
                ("input", 60),
                ("page navigation", 64),
                ("save", 229),
                ("submit", 24),
                ("validate", 1_375),
            ])
        );
        assert_eq!(
            validations.assessments,
            string_counts(&[
                ("ambiguous", 25),
                ("incorrect-official-behavior", 306),
                ("obsolete", 59),
                ("official-bug-compatible", 267),
                ("unverified", 23),
                ("verified-correct", 1_327),
            ])
        );
        assert_eq!(
            validations.confidences,
            string_counts(&[("high", 1_994), ("medium", 13)])
        );
        assert_eq!(
            validations.evidence_types,
            string_counts(&[
                ("official-guide", 3),
                ("official-pdf", 1),
                ("repository-code", 2),
                ("runtime-binary", 4),
                ("runtime-dom", 7),
                ("source", 2_004),
                ("ui-observation", 5),
                ("xml", 4),
            ])
        );
        assert_eq!(validations.exact_message_present, 1_706);
        assert_eq!(validations.exact_message_absent, 301);
        assert_eq!(validations.ordered_records, 1_785);
        assert_eq!(validations.unordered_records, 222);
        assert_eq!(
            validations.field_arities,
            BTreeMap::from([
                (0, 16),
                (1, 1_167),
                (2, 403),
                (3, 198),
                (4, 169),
                (5, 26),
                (6, 16),
                (7, 6),
                (8, 3),
                (10, 1),
                (11, 1),
                (12, 1),
            ])
        );

        let calculations = &report.v1_calculation_constructs;
        assert_eq!(calculations.records, 623);
        assert_eq!(
            calculations.assessments,
            string_counts(&[
                ("ambiguous", 32),
                ("incorrect-official-behavior", 56),
                ("obsolete", 5),
                ("official-bug-compatible", 46),
                ("unverified", 1),
                ("verified-correct", 483),
            ])
        );
        assert_eq!(calculations.confidences, string_counts(&[("high", 623)]));
        assert_eq!(calculations.condition_present, 112);
        assert_eq!(calculations.condition_absent, 511);
        assert_eq!(calculations.rounding_descriptors.len(), 64);
        assert_eq!(
            calculations.rounding_descriptors.values().sum::<usize>(),
            623
        );
        assert_eq!(
            calculations.rounding_descriptors.get("formatCurrency."),
            Some(&14)
        );
        assert_eq!(calculations.triggers.len(), 396);
        assert_eq!(calculations.triggers.values().sum::<usize>(), 623);
        assert_eq!(calculations.triggers.get("pageOneComputation"), Some(&24));
        assert_eq!(
            calculations.dependency_arities,
            BTreeMap::from([
                (0, 283),
                (1, 187),
                (2, 139),
                (3, 10),
                (4, 2),
                (6, 1),
                (28, 1),
            ])
        );
        assert_eq!(
            calculations.output_arities,
            BTreeMap::from([
                (1, 420),
                (2, 165),
                (3, 24),
                (4, 8),
                (5, 3),
                (6, 1),
                (7, 1),
                (12, 1),
            ])
        );
        assert_eq!(
            calculations.input_arities,
            BTreeMap::from([
                (1, 171),
                (2, 311),
                (3, 76),
                (4, 39),
                (5, 13),
                (6, 5),
                (7, 2),
                (8, 2),
                (12, 4),
            ])
        );

        let operators = &report.snapshots[0].operators;
        assert!(operators.effects.contains_key("emit-issue"));
        assert!(operators.predicates.contains_key("compare"));
        assert!(operators.predicates.contains_key("is-empty"));
        assert!(
            operators.predicates.contains_key("constant"),
            "the executable calculation condition must be counted"
        );
        assert!(
            operators
                .expressions
                .contains_key("javascript-parse-int-radix10"),
            "calculation outputs[].value expressions must be counted"
        );
        assert!(operators.expressions.contains_key("field"));
        assert!(
            !operators.predicates.contains_key("field"),
            "expression kinds must not leak into the predicate census"
        );
        assert!(
            !operators.predicates.contains_key("singleton"),
            "field-instance selectors are not predicate operators"
        );
        assert!(operators.field_coercions.contains_key("string"));
    }

    #[test]
    fn synthetic_v1_census_ignores_prose_and_preserves_only_structured_facts() {
        let validation: V1ValidationRecord = serde_json::from_str(
            r#"{
                "rule_id": "synthetic-rule",
                "form_id": "synthetic-v1",
                "revision": "2026-01-01",
                "phase": "input",
                "order": 7,
                "condition": "compare(is-empty(field(singleton))) then emit-issue",
                "fields": ["field-a", "field-b"],
                "exact_message": "javascript-parse-int-radix10",
                "evidence_type": ["runtime-binary", "source"],
                "assessment": "ambiguous",
                "official_behavior": "sum and round the prose",
                "recommended_app_behavior": "do not execute this prose",
                "confidence": "low",
                "unresolved_questions": []
            }"#,
        )
        .expect("synthetic validation");
        let calculation: V1CalculationRecord = serde_json::from_str(
            r#"{
                "calculation_id": "synthetic-calculation",
                "outputs": ["output-a", "output-b"],
                "inputs": ["input-a"],
                "condition": "compare(is-empty(field(singleton)))",
                "official_formula": "javascript-parse-int-radix10 then emit-issue",
                "rounding": "opaque compare/round descriptor",
                "trigger": "opaque emit-issue trigger",
                "depends_on": ["first", "second", "third"],
                "source_refs": ["synthetic"],
                "assessment": "unverified",
                "recommended_app_behavior": "do not execute this prose",
                "confidence": "unknown"
            }"#,
        )
        .expect("synthetic calculation");

        let mut validations = V1ValidationConstructCensus::default();
        count_v1_validation(&validation, &mut validations);
        assert_eq!(validations.phases, string_counts(&[("input", 1)]));
        assert_eq!(
            validations.evidence_types,
            string_counts(&[("runtime-binary", 1), ("source", 1)])
        );
        assert_eq!(validations.field_arities, BTreeMap::from([(2, 1)]));
        assert_eq!(validations.exact_message_present, 1);
        assert_eq!(validations.ordered_records, 1);

        let mut calculations = V1CalculationConstructCensus::default();
        count_v1_calculation(&calculation, &mut calculations);
        assert_eq!(calculations.condition_present, 1);
        assert_eq!(
            calculations.rounding_descriptors,
            string_counts(&[("opaque compare/round descriptor", 1)])
        );
        assert_eq!(
            calculations.triggers,
            string_counts(&[("opaque emit-issue trigger", 1)])
        );
        assert_eq!(calculations.dependency_arities, BTreeMap::from([(3, 1)]));
        assert_eq!(calculations.output_arities, BTreeMap::from([(2, 1)]));
        assert_eq!(calculations.input_arities, BTreeMap::from([(1, 1)]));
    }

    #[test]
    fn construct_serialization_is_deterministic_across_record_order() {
        let first = V1CalculationRecord {
            calculation_id: "first".to_owned(),
            outputs: vec!["out".to_owned()],
            inputs: vec!["in".to_owned()],
            condition: None,
            rounding: "z-rounding".to_owned(),
            trigger: "z-trigger".to_owned(),
            depends_on: Vec::new(),
            assessment: "verified-correct".to_owned(),
            confidence: "high".to_owned(),
        };
        let second = V1CalculationRecord {
            calculation_id: "second".to_owned(),
            outputs: vec!["out".to_owned()],
            inputs: vec!["in".to_owned()],
            condition: Some("opaque".to_owned()),
            rounding: "a-rounding".to_owned(),
            trigger: "a-trigger".to_owned(),
            depends_on: vec!["first".to_owned()],
            assessment: "ambiguous".to_owned(),
            confidence: "low".to_owned(),
        };
        let mut forward = V1CalculationConstructCensus::default();
        count_v1_calculation(&first, &mut forward);
        count_v1_calculation(&second, &mut forward);
        let mut reverse = V1CalculationConstructCensus::default();
        count_v1_calculation(&second, &mut reverse);
        count_v1_calculation(&first, &mut reverse);

        assert_eq!(forward, reverse);
        assert_eq!(
            serde_json::to_vec(&forward).expect("serialize forward"),
            serde_json::to_vec(&reverse).expect("serialize reverse")
        );
    }

    #[test]
    fn untranslated_totals_reject_reverse_count_drift() {
        assert_eq!(untranslated_count("validation", 10, 3).unwrap(), 7);
        let error = untranslated_count("validation", 3, 10).unwrap_err();
        assert!(error.message().contains("count drift"));
    }
}
