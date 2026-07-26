//! Record-level reconciliation between the prose-oriented v1 corpus and v2 IR.
//!
//! A v1 record is represented only when a v2 entity cites its exact source ID and
//! JSON-pointer locator. Records with no runtime target must carry one of the closed,
//! source-backed classifications in `legacy_v1.record_classifications`. Everything else
//! remains visibly unclassified; omission can never become executable by accident.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Serialize;

use crate::audit::{AuditOptions, audit};
use crate::error::Result;
use crate::json::JsonValue;
use crate::model::{LegacyArtifact, LegacyRecordClassification, RuleSetDocument};

#[derive(Clone, Debug)]
pub struct ReconciliationOptions {
    pub repo_root: PathBuf,
}

impl ReconciliationOptions {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ArtifactReconciliation {
    pub artifact: String,
    pub legacy_records: usize,
    pub represented_records: usize,
    pub represented_targets: usize,
    pub multiply_represented_records: usize,
    pub intentionally_non_runtime_records: usize,
    pub unresolved_records: usize,
    pub unclassified_records: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FormReconciliation {
    pub form_id: String,
    pub rule_set_id: String,
    pub artifacts: Vec<ArtifactReconciliation>,
    pub complete_for_library: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ReconciliationReport {
    pub forms_with_v2_snapshot: usize,
    pub complete_forms: usize,
    pub legacy_records: usize,
    pub represented_records: usize,
    pub intentionally_non_runtime_records: usize,
    pub unresolved_records: usize,
    pub unclassified_records: usize,
    pub forms: Vec<FormReconciliation>,
}

pub fn reconciliation(options: &ReconciliationOptions) -> Result<ReconciliationReport> {
    let audit = audit(&AuditOptions::new(&options.repo_root))?;
    let mut forms = audit
        .snapshots
        .iter()
        .map(|snapshot| reconcile_form(&snapshot.document))
        .collect::<Vec<_>>();
    forms.sort_by(|left, right| left.form_id.cmp(&right.form_id));

    let artifact_sum = |select: fn(&ArtifactReconciliation) -> usize| {
        forms
            .iter()
            .flat_map(|form| form.artifacts.iter())
            .map(select)
            .sum()
    };
    Ok(ReconciliationReport {
        forms_with_v2_snapshot: forms.len(),
        complete_forms: forms
            .iter()
            .filter(|form| form.complete_for_library)
            .count(),
        legacy_records: artifact_sum(|artifact| artifact.legacy_records),
        represented_records: artifact_sum(|artifact| artifact.represented_records),
        intentionally_non_runtime_records: artifact_sum(|artifact| {
            artifact.intentionally_non_runtime_records
        }),
        unresolved_records: artifact_sum(|artifact| artifact.unresolved_records),
        unclassified_records: artifact_sum(|artifact| artifact.unclassified_records),
        forms,
    })
}

fn reconcile_form(document: &RuleSetDocument) -> FormReconciliation {
    let source_ids = document
        .legacy_v1
        .mappings
        .iter()
        .map(|mapping| (mapping.artifact, mapping.source_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut target_counts: BTreeMap<(LegacyArtifact, String), usize> = BTreeMap::new();

    collect_collection_targets(
        LegacyArtifact::Fields,
        "fields",
        Some("group_id"),
        &document.field_groups,
        source_ids[&LegacyArtifact::Fields],
        &mut target_counts,
    );
    collect_collection_targets(
        LegacyArtifact::Fields,
        "fields",
        Some("field_id"),
        &document.fields,
        source_ids[&LegacyArtifact::Fields],
        &mut target_counts,
    );
    collect_collection_targets(
        LegacyArtifact::Validations,
        "rules",
        Some("rule_id"),
        &document.rules,
        source_ids[&LegacyArtifact::Validations],
        &mut target_counts,
    );
    collect_collection_targets(
        LegacyArtifact::Calculations,
        "calculations",
        Some("calculation_id"),
        &document.calculations,
        source_ids[&LegacyArtifact::Calculations],
        &mut target_counts,
    );
    if let Some(workflow) = document.workflow.object() {
        if let Some(JsonValue::Array(states)) = workflow.get("states") {
            collect_collection_targets(
                LegacyArtifact::Workflow,
                "phases",
                None,
                states,
                source_ids[&LegacyArtifact::Workflow],
                &mut target_counts,
            );
        }
        if let Some(JsonValue::Array(transitions)) = workflow.get("transitions") {
            collect_collection_targets(
                LegacyArtifact::Workflow,
                "transitions",
                None,
                transitions,
                source_ids[&LegacyArtifact::Workflow],
                &mut target_counts,
            );
        }
    }

    let mut non_runtime = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    for classification in &document.legacy_v1.record_classifications {
        let key = (
            classification.artifact(),
            classification.locator().to_owned(),
        );
        match classification {
            LegacyRecordClassification::NonRuntime { .. } => {
                non_runtime.insert(key);
            }
            LegacyRecordClassification::Unresolved { .. } => {
                unresolved.insert(key);
            }
        }
    }

    let mut artifacts = Vec::new();
    for mapping in &document.legacy_v1.mappings {
        if mapping.artifact == LegacyArtifact::Manifest {
            continue;
        }
        let represented = target_counts
            .iter()
            .filter(|((artifact, _), _)| *artifact == mapping.artifact)
            .collect::<Vec<_>>();
        let represented_records = represented.len();
        let represented_targets = represented.iter().map(|(_, count)| **count).sum();
        let multiply_represented_records =
            represented.iter().filter(|(_, count)| **count > 1).count();
        let intentionally_non_runtime_records = non_runtime
            .iter()
            .filter(|(artifact, _)| *artifact == mapping.artifact)
            .count();
        let unresolved_records = unresolved
            .iter()
            .filter(|(artifact, _)| *artifact == mapping.artifact)
            .count();
        let accounted =
            represented_records + intentionally_non_runtime_records + unresolved_records;
        let legacy_records = match mapping.artifact {
            LegacyArtifact::Workflow => document
                .legacy_v1
                .declared_counts
                .workflow_states
                .saturating_add(document.legacy_v1.declared_counts.workflow_transitions)
                as usize,
            _ => mapping.record_count as usize,
        };
        artifacts.push(ArtifactReconciliation {
            artifact: mapping.artifact.label().to_owned(),
            legacy_records,
            represented_records,
            represented_targets,
            multiply_represented_records,
            intentionally_non_runtime_records,
            unresolved_records,
            unclassified_records: legacy_records.saturating_sub(accounted),
        });
    }
    artifacts.sort_by(|left, right| left.artifact.cmp(&right.artifact));
    let complete_for_library = artifacts.iter().all(|artifact| {
        artifact.unclassified_records == 0
            && artifact.unresolved_records == 0
            && artifact.multiply_represented_records == 0
            && artifact.represented_records + artifact.intentionally_non_runtime_records
                == artifact.legacy_records
    });
    FormReconciliation {
        form_id: document.legacy_v1.form_id.clone(),
        rule_set_id: document.identity.rule_set_id.clone(),
        artifacts,
        complete_for_library,
    }
}

fn collect_collection_targets(
    artifact: LegacyArtifact,
    legacy_array_key: &str,
    target_id_key: Option<&str>,
    entities: &[JsonValue],
    source_id: &str,
    target_counts: &mut BTreeMap<(LegacyArtifact, String), usize>,
) {
    let prefix = format!("#/{legacy_array_key}/");
    for entity in entities {
        if let Some(target_id_key) = target_id_key {
            // ID-bearing fields, validations, and calculations cannot become
            // reconciliation authority without their stable target ID. V1
            // workflow records are the sole exception: phases and transitions
            // are ID-less and derive identity from the exact source locator.
            if entity
                .object()
                .and_then(|entity| entity.get(target_id_key))
                .and_then(JsonValue::as_str)
                .is_none()
            {
                continue;
            }
        }
        let Some(JsonValue::Array(source_refs)) =
            entity.object().and_then(|entity| entity.get("source_refs"))
        else {
            continue;
        };
        for source_ref in source_refs {
            let Some(source_ref) = source_ref.object() else {
                continue;
            };
            if source_ref.get("source_id").and_then(JsonValue::as_str) != Some(source_id) {
                continue;
            }
            let Some(locator) = source_ref.get("locator").and_then(JsonValue::as_str) else {
                continue;
            };
            if locator
                .strip_prefix(&prefix)
                .is_some_and(|index| index.parse::<usize>().is_ok())
            {
                *target_counts
                    .entry((artifact, locator.to_owned()))
                    .or_default() += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ReconciliationOptions, reconcile_form, reconciliation};
    use crate::json::JsonValue;
    use crate::model::{
        LegacyArtifact, LegacyNonRuntimeReason, LegacyRecordClassification, RuleSetDocument,
        SourceRef,
    };

    #[test]
    fn landed_candidate_exposes_its_unclassified_legacy_gap() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report =
            reconciliation(&ReconciliationOptions::new(root)).expect("reconciliation report");
        assert_eq!(report.forms_with_v2_snapshot, 1);
        assert_eq!(report.complete_forms, 0);
        assert!(report.represented_records > 0);
        assert!(report.unclassified_records > 0);
        assert_eq!(report.intentionally_non_runtime_records, 0);
        assert_eq!(report.unresolved_records, 0);

        let form = &report.forms[0];
        assert_eq!(form.form_id, "2550q-v2024");
        assert_eq!(form.rule_set_id, "2550q-v2024-p7.9.6.0");
        let rules = form
            .artifacts
            .iter()
            .find(|artifact| artifact.artifact == "validations")
            .expect("validation reconciliation");
        assert_eq!(rules.legacy_records, 38);
        assert_eq!(rules.represented_records, 27);
        assert_eq!(rules.unclassified_records, 11);

        let workflow = form
            .artifacts
            .iter()
            .find(|artifact| artifact.artifact == "workflow")
            .expect("workflow reconciliation");
        assert_eq!(workflow.legacy_records, 10);
        assert_eq!(workflow.represented_records, 8);
        assert_eq!(workflow.represented_targets, 8);
        assert_eq!(workflow.unclassified_records, 2);
    }

    #[test]
    fn idless_workflow_reconciles_both_arrays_and_cannot_omit_a_phase() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = root.join("rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json");
        let bytes = std::fs::read(&path).expect("read landed rule set");
        let (mut document, _) =
            crate::json::parse_typed::<RuleSetDocument>(&bytes, &path).expect("parse rule set");

        document.field_groups.clear();
        document.fields.clear();
        document.calculations.clear();
        document.rules.clear();
        document.legacy_v1.record_classifications.clear();
        for mapping in &mut document.legacy_v1.mappings {
            mapping.record_count = match mapping.artifact {
                LegacyArtifact::Manifest => 1,
                // v1 mapping schema 1.0.0 retains its transition-count wire
                // value even though reconciliation inventories both arrays.
                LegacyArtifact::Workflow => 5,
                _ => 0,
            };
        }
        let counts = &mut document.legacy_v1.declared_counts;
        counts.typed_fields = 0;
        counts.concrete_union_fields = 0;
        counts.unbounded_family_members = 0;
        counts.validation_rules = 0;
        counts.calculations = 0;
        counts.workflow_states = 5;
        counts.workflow_transitions = 5;

        let states = (0..4)
            .map(|index| {
                serde_json::from_value(json!({
                    "state_id": format!("state-{index}"),
                    "source_refs": [{
                        "source_id": "v1-workflow",
                        "locator": format!("#/phases/{index}")
                    }]
                }))
                .expect("synthetic workflow state")
            })
            .collect::<Vec<JsonValue>>();
        let transitions = (0..4)
            .map(|index| {
                serde_json::from_value(json!({
                    "transition_id": format!("transition-{index}"),
                    "source_refs": [{
                        "source_id": "v1-workflow",
                        "locator": format!("#/transitions/{index}")
                    }]
                }))
                .expect("synthetic workflow transition")
            })
            .collect::<Vec<JsonValue>>();
        document.workflow = serde_json::from_value(json!({
            "states": states,
            "transitions": transitions
        }))
        .expect("synthetic ID-less legacy workflow");
        document.legacy_v1.record_classifications = vec![
            workflow_non_runtime("#/phases/4"),
            workflow_non_runtime("#/transitions/4"),
        ];

        let complete = reconcile_form(&document);
        assert!(complete.complete_for_library);
        let workflow = complete
            .artifacts
            .iter()
            .find(|artifact| artifact.artifact == "workflow")
            .expect("workflow reconciliation");
        assert_eq!(workflow.legacy_records, 10);
        assert_eq!(workflow.represented_records, 8);
        assert_eq!(workflow.intentionally_non_runtime_records, 2);
        assert_eq!(workflow.unclassified_records, 0);

        document
            .legacy_v1
            .record_classifications
            .retain(|classification| classification.locator() != "#/phases/4");
        let omitted_phase = reconcile_form(&document);
        assert!(!omitted_phase.complete_for_library);
        let workflow = omitted_phase
            .artifacts
            .iter()
            .find(|artifact| artifact.artifact == "workflow")
            .expect("workflow reconciliation");
        assert_eq!(workflow.legacy_records, 10);
        assert_eq!(workflow.unclassified_records, 1);
    }

    fn workflow_non_runtime(locator: &str) -> LegacyRecordClassification {
        LegacyRecordClassification::NonRuntime {
            artifact: LegacyArtifact::Workflow,
            legacy_id: None,
            locator: locator.to_owned(),
            reason: LegacyNonRuntimeReason::NonValidationUiBehavior,
            source_refs: vec![SourceRef {
                source_id: "v1-workflow".to_owned(),
                locator: Some(locator.to_owned()),
            }],
        }
    }
}
